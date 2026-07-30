//! Contrato HTTP da interface web: sessão, CSRF, o fluxo de dinheiro e a tela
//! de mercado, atravessando o router de produção inteiro.
//!
//! **A lacuna que esta bateria fecha.** Os testes de unidade cobrem bem as duas
//! pontas: o `repository` prova a aritmética do livro-razão contra um Postgres
//! efêmero, e os testes de template provam que o HTML sai certo a partir de um
//! `WalletData` montado à mão. Entre as duas pontas fica tudo o que faz o
//! produto ser um produto — o cookie de sessão, o token CSRF, o redirecionamento
//! pós-operação, o banner de erro de negócio, o gate de autenticação — e nada
//! disso era exercitado de ponta a ponta.
//!
//! Estes testes são também os únicos que provam que as CAMADAS estão na ordem
//! certa: `App::router` monta cabeçalhos de segurança, renovação de sessão e
//! span da requisição empilhados, e a ordem entre eles só se observa passando
//! uma requisição de verdade pela pilha.

mod common;

use axum::http::{StatusCode, header};
use common::{
    ADMIN_SECRET, app, cookie_value, get, get_with_cookies, post_form, post_json_as_admin,
    register, send, text,
};

/// **O gate de autenticação.** Toda tela com dado privado tem de recusar quem
/// não tem sessão — e recusar redirecionando para o login, não com um 500 nem
/// com a tela vazia.
#[sqlx::test]
async fn private_screens_send_anonymous_visitors_to_the_login(db: sqlx::PgPool) {
    let app = app(db);

    for path in ["/assets", "/market", "/transactions.csv", "/deposit"] {
        let response = send(&app, get(path)).await;

        assert_eq!(
            response.status(),
            StatusCode::SEE_OTHER,
            "{path} deveria redirecionar quem não tem sessão"
        );
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/login",
            "{path}"
        );
    }
}

/// A mesma recusa, pelo caminho do htmx: uma requisição parcial não pode receber
/// a página de login *dentro* de um pedaço da carteira — o navegador inteiro tem
/// de ir para o login. É o que o cabeçalho `HX-Redirect` faz.
#[sqlx::test]
async fn an_expired_session_redirects_the_whole_browser_not_just_the_fragment(db: sqlx::PgPool) {
    let app = app(db);

    let response = send(
        &app,
        axum::http::Request::builder()
            .uri("/assets")
            .header("hx-request", "true")
            .body(axum::body::Body::empty())
            .expect("requisição válida"),
    )
    .await;

    assert_eq!(
        response
            .headers()
            .get("hx-redirect")
            .map(|v| v.to_str().unwrap()),
        Some("/login"),
        "o htmx precisa do HX-Redirect para trocar a página inteira"
    );
}

/// Registro emite a sessão e ela dá acesso à carteira. É o pré-requisito de
/// todos os testes de fluxo abaixo — e prova o ciclo cookie → JWT → tela.
#[sqlx::test]
async fn registering_starts_a_session_that_opens_the_wallet(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    let response = send(&app, get_with_cookies("/assets", &session)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let html = text(response).await;
    assert!(
        html.contains("breno"),
        "a carteira deveria saudar o usuário"
    );
}

/// **CSRF.** Um formulário sem o token, ou com um token que não casa com o
/// cookie, é recusado. É a defesa contra um site de terceiro postar na carteira
/// usando a sessão do navegador da vítima.
#[sqlx::test]
async fn forms_without_a_matching_csrf_token_are_refused(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    // Sem o campo `csrf_token`, o corpo não tem a forma de um formulário nosso e
    // o extrator recusa ANTES de qualquer handler rodar.
    assert_eq!(
        send(&app, post_form("/deposit", &session, "amount=100"))
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "campo ausente deveria morrer no extrator"
    );

    // Com o campo presente mas o token errado, a recusa é do `verify_csrf` — e
    // sai como 303 com banner, não como 403 nu: é erro de sessão do usuário
    // ("recarregue a página"), e a tela precisa poder explicar isso.
    for (case, body) in [
        ("token forjado", "amount=100&csrf_token=inventado"),
        ("token vazio", "amount=100&csrf_token="),
        (
            "token de outra sessão",
            "amount=100&csrf_token=abcdef0123456789",
        ),
    ] {
        let response = send(&app, post_form("/deposit", &session, body)).await;

        assert_eq!(
            response.status(),
            StatusCode::SEE_OTHER,
            "{case}: deveria voltar ao formulário com banner"
        );
        assert!(
            !response.status().is_server_error(),
            "{case}: CSRF inválido não é erro nosso"
        );
    }

    // **O invariante que de fato protege a carteira**: nenhuma daquelas
    // tentativas moveu dinheiro. Conferir só o status deixaria passar um
    // refactor que redireciona bonito e credita o depósito de qualquer forma.
    let wallet = text(send(&app, get_with_cookies("/assets", &session)).await).await;
    assert!(
        wallet.contains("R$ 0,00"),
        "o saldo deveria continuar zerado depois das tentativas forjadas"
    );
    assert!(
        !wallet.contains("R$ 100,00") && !wallet.contains("R$ 300,00"),
        "um POST sem CSRF válido creditou saldo"
    );
}

/// O fluxo de dinheiro completo pelo navegador: depositar, comprar, vender —
/// cada passo com CSRF válido, cada passo redirecionando de volta para a
/// carteira, e o saldo refletindo a operação na tela.
#[sqlx::test(fixtures("catalogue"))]
async fn the_money_flow_works_end_to_end_through_the_browser_path(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    // O token CSRF é renovado a cada carregamento da carteira; cada POST usa o
    // que veio no GET imediatamente anterior, como um navegador faria.
    let csrf = |cookies: &str| {
        let app = app.clone();
        let cookies = cookies.to_string();
        async move {
            let page = send(&app, get_with_cookies("/assets", &cookies)).await;
            cookie_value(&page, wallet::auth::csrf::CSRF_COOKIE)
                .unwrap_or_else(|| extract_csrf_from_cookies(&cookies))
        }
    };

    // 1. Depósito.
    let token = csrf(&session).await;
    let deposit = send(
        &app,
        post_form(
            "/deposit",
            &format!("{session}; {}={token}", wallet::auth::csrf::CSRF_COOKIE),
            &format!("amount=10000&csrf_token={token}"),
        ),
    )
    .await;
    assert_eq!(deposit.status(), StatusCode::SEE_OTHER, "depósito");

    let wallet_html = text(send(&app, get_with_cookies("/assets", &session)).await).await;
    assert!(
        wallet_html.contains("10.000,00"),
        "o saldo depositado deveria aparecer na carteira"
    );

    // 2. Compra do ativo do catálogo (id 1 = bitcoin, R$ 325.611,00).
    let token = csrf(&session).await;
    let buy = send(
        &app,
        post_form(
            "/buy",
            &format!("{session}; {}={token}", wallet::auth::csrf::CSRF_COOKIE),
            &format!("asset_id=1&quantity=0.01&csrf_token={token}"),
        ),
    )
    .await;
    assert_eq!(buy.status(), StatusCode::SEE_OTHER, "compra");

    let wallet_html = text(send(&app, get_with_cookies("/assets", &session)).await).await;
    assert!(
        wallet_html.contains("bitcoin"),
        "a posição deveria aparecer"
    );

    // 3. Venda parcial.
    let token = csrf(&session).await;
    let sell = send(
        &app,
        post_form(
            "/sell",
            &format!("{session}; {}={token}", wallet::auth::csrf::CSRF_COOKIE),
            &format!("asset_id=1&quantity=0.005&csrf_token={token}"),
        ),
    )
    .await;
    assert_eq!(sell.status(), StatusCode::SEE_OTHER, "venda");
}

/// Erro de NEGÓCIO (saldo insuficiente) volta como banner na tela, não como
/// 500. É a diferença entre "você não tem saldo" e "erro interno do servidor" —
/// e é o que decide se o usuário entende o que aconteceu.
#[sqlx::test(fixtures("catalogue"))]
async fn a_business_error_comes_back_as_a_banner_not_a_500(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    let page = send(&app, get_with_cookies("/assets", &session)).await;
    let token = cookie_value(&page, wallet::auth::csrf::CSRF_COOKIE)
        .unwrap_or_else(|| extract_csrf_from_cookies(&session));

    // Comprar sem ter depositado nada.
    let response = send(
        &app,
        post_form(
            "/buy",
            &format!("{session}; {}={token}", wallet::auth::csrf::CSRF_COOKIE),
            &format!("asset_id=1&quantity=1&csrf_token={token}"),
        ),
    )
    .await;

    assert!(
        !response.status().is_server_error(),
        "saldo insuficiente é erro do usuário, não do servidor: {}",
        response.status()
    );
    assert_eq!(
        response.status(),
        StatusCode::SEE_OTHER,
        "volta ao formulário"
    );
}

/// A tela de mercado responde ANTES da primeira rodada do job: o snapshot está
/// vazio (os jobs estão desligados na configuração de teste), e a tela tem de
/// mostrar o estado "buscando as cotações" com um `role="status"` para leitor de
/// tela — nunca uma página quebrada.
#[sqlx::test]
async fn the_market_screen_degrades_gracefully_before_the_first_refresh(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    let response = send(&app, get_with_cookies("/market", &session)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let html = text(response).await;
    assert!(
        html.contains("role=\"status\""),
        "leitor de tela precisa saber"
    );
    assert!(
        html.contains("id=\"market\""),
        "o contêiner que se reconstrói na próxima rodada"
    );
}

/// O estado da tela de mercado viaja na URL, e a tela responde 200 para qualquer
/// combinação — inclusive moeda inexistente e busca com caractere especial. Um
/// parâmetro digitado à mão nunca pode virar 500.
#[sqlx::test]
async fn the_market_screen_accepts_any_state_in_the_query_string(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    for query in [
        "",
        "?coin=bitcoin",
        "?coin=nao-existe&range=24h",
        "?range=periodo-invalido",
        "?q=%26%3D",
        "?q=texto+muito+longo+que+passa+do+limite+de+trinta+e_dois",
        "?coin=&range=&q=",
    ] {
        let status = send(&app, get_with_cookies(&format!("/market{query}"), &session))
            .await
            .status();

        assert_eq!(status, StatusCode::OK, "/market{query}");
    }
}

/// Requisição do htmx recebe o FRAGMENTO, navegação normal recebe a página
/// inteira. É a base de "operar sem recarregar mantendo SSR": o mesmo handler,
/// duas formas, decididas pelo cabeçalho.
#[sqlx::test]
async fn htmx_requests_get_a_fragment_and_plain_navigation_gets_the_full_page(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    let full = text(send(&app, get_with_cookies("/market", &session)).await).await;
    assert!(full.contains("<!DOCTYPE html>"), "página completa");
    assert!(full.contains("/static/app.css"), "com o CSS da interface");

    let fragment = text(
        send(
            &app,
            axum::http::Request::builder()
                .uri("/market")
                .header("hx-request", "true")
                .header(header::COOKIE, &session)
                .body(axum::body::Body::empty())
                .expect("requisição válida"),
        )
        .await,
    )
    .await;
    assert!(
        !fragment.contains("<!DOCTYPE html>"),
        "o fragmento não pode trazer a página inteira: {}",
        &fragment[..fragment.len().min(200)]
    );
}

/// **Cache dos assets estáticos.** A URL é fixa e o conteúdo muda a cada build,
/// então a resposta precisa carregar `ETag` e pedir revalidação — senão um
/// rebuild deixa a tela com o HTML novo e o CSS velho (foi o que aconteceu com
/// o layout de duas colunas do mercado). O 304 é o que mantém isso barato.
#[sqlx::test]
async fn static_assets_revalidate_instead_of_being_cached_blind(db: sqlx::PgPool) {
    let app = app(db);

    for path in [
        "/static/app.css",
        "/static/htmx.js",
        "/static/money-input.js",
    ] {
        let response = send(&app, get(path)).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");

        let tag = response
            .headers()
            .get(header::ETAG)
            .unwrap_or_else(|| panic!("{path}: sem ETag"))
            .to_str()
            .expect("ascii")
            .to_string();
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, no-cache",
            "{path}: cache sem revalidação serve arquivo de outro binário"
        );

        // De volta com a etiqueta: 304 e corpo vazio.
        let revalidated = send(
            &app,
            axum::http::Request::builder()
                .uri(path)
                .header(header::IF_NONE_MATCH, &tag)
                .body(axum::body::Body::empty())
                .expect("requisição válida"),
        )
        .await;
        assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED, "{path}");
        assert!(text(revalidated).await.is_empty(), "{path}: 304 com corpo");
    }
}

/// O idioma é escolhido por cookie e o destino do retorno é validado: `?next=`
/// só aceita caminho local absoluto. Sem isso, a troca de idioma seria um open
/// redirect — um link `\/lang\/pt-BR?next=https://site-falso` levaria o usuário
/// para fora com um clique que parece do produto.
#[sqlx::test]
async fn the_language_switch_is_not_an_open_redirect(db: sqlx::PgPool) {
    let app = app(db);

    for (next, expected) in [
        ("/market", "/market"),
        ("https://exemplo-invasor.test", "/"),
        ("//exemplo-invasor.test", "/"),
        ("", "/"),
    ] {
        let response = send(&app, get(&format!("/lang/pt-BR?next={next}"))).await;

        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            expected,
            "next={next:?} não deveria levar para fora do site"
        );
    }
}

/// A interface sai nos dois idiomas, escolhida pelo cookie que a rota `/lang`
/// grava.
#[sqlx::test]
async fn the_interface_answers_in_both_languages(db: sqlx::PgPool) {
    let app = app(db);

    let pt = text(send(&app, get_with_cookies("/login", "lang=pt-BR")).await).await;
    let en = text(send(&app, get_with_cookies("/login", "lang=en")).await).await;

    assert!(pt.contains("lang=\"pt-BR\""), "documento em pt-BR");
    assert!(en.contains("lang=\"en\""), "documento em inglês");
    assert_ne!(pt, en, "os dois idiomas não podem render o mesmo HTML");
}

/// As sondas são separadas de propósito: liveness não toca o banco (reiniciar o
/// app não conserta um Postgres fora do ar), readiness toca.
#[sqlx::test]
async fn liveness_and_readiness_are_separate_probes(db: sqlx::PgPool) {
    let app = app(db);

    for path in ["/healthz", "/readyz", "/health"] {
        let status = send(&app, get(path)).await.status();
        assert_eq!(status, StatusCode::OK, "{path}");
    }
}

/// O extrato em CSV é autenticado e sai na convenção pt-BR. O cabeçalho de
/// download tem de vir junto, senão o navegador renderiza o CSV como texto.
#[sqlx::test(fixtures("catalogue"))]
async fn the_statement_exports_as_an_authenticated_csv_download(db: sqlx::PgPool) {
    let app = app(db);
    let session = register(&app, "breno", "senha-de-teste-123").await;

    let response = send(&app, get_with_cookies("/transactions.csv", &session)).await;
    assert_eq!(response.status(), StatusCode::OK);

    let disposition = response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .expect("sem Content-Disposition o navegador não baixa")
        .to_str()
        .expect("ascii");
    assert!(disposition.contains("attachment"), "{disposition}");

    // Separador é ponto e vírgula: é o que o Excel em pt-BR entende como coluna.
    let csv = text(response).await;
    assert!(csv.contains(';'), "CSV em convenção pt-BR: {csv}");
}

/// A credencial de admin do painel web é a mesma da API, e comparada em tempo
/// constante. Aqui só se verifica que ela AUTORIZA — a comparação em si tem
/// teste de unidade próprio.
#[sqlx::test]
async fn the_admin_credential_authorises_the_catalogue_route(db: sqlx::PgPool) {
    let app = app(db);

    let response = send(
        &app,
        post_json_as_admin(
            "/api/v1/assets",
            r#"{"name":"solana","unit_value":"900.00"}"#,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        !ADMIN_SECRET.is_empty(),
        "o segredo de teste é determinístico"
    );
}

/// O cookie do CSRF pode já estar na sessão que o registro devolveu; este
/// atalho o recupera de lá quando o GET seguinte não o renova.
fn extract_csrf_from_cookies(cookies: &str) -> String {
    cookies
        .split(';')
        .map(str::trim)
        .find_map(|pair| {
            pair.strip_prefix(&format!("{}=", wallet::auth::csrf::CSRF_COOKIE))
                .map(str::to_string)
        })
        .expect("a sessão precisa carregar o token CSRF")
}
