// Cada arquivo de `tests/` é um binário SEPARADO, e cada um compila a sua
// própria cópia deste módulo — então o que a bateria da API não usa aparece como
// código morto para ela, e vice-versa. O `allow` é sobre isso, e não sobre
// helper esquecido: o conjunto todo é usado, só não pelo mesmo binário.
#![allow(dead_code)]

//! Andaime compartilhado pelas baterias de integração.
//!
//! A regra que molda este arquivo: **o teste atravessa o router de produção**.
//! `App::router` é a mesma função que o `main` serve, com os mesmos middlewares
//! na mesma ordem — cabeçalhos de segurança, renovação de sessão, span da
//! requisição. Um teste que montasse o seu próprio `Router::new()` provaria
//! apenas que o handler funciona, e deixaria de fora justamente as camadas que
//! ninguém confere à mão.
//!
//! Nada aqui abre socket: o `oneshot` do tower empurra uma `Request` pela pilha
//! e devolve a `Response`. Sem porta, sem espera, sem teste dependendo de qual
//! porta estava livre na máquina de quem rodou.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use axum::response::Response as AxumResponse;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use wallet::app::{App, AppState};
use wallet::config::Config;

/// Segredos fixos e conhecidos. São de TESTE: o valor não importa, importa que
/// sejam determinísticos, para que a asserção sobre uma sessão assinada não
/// dependa do ambiente de quem rodou.
pub const ADMIN_SECRET: &str = "segredo-de-teste-do-admin";
const JWT_SECRET: &str = "segredo-de-teste-do-jwt-com-tamanho-suficiente";

/// Configuração equivalente à de produção, com os jobs DESLIGADOS.
///
/// Zerar `QUOTES_SYNC_MINUTES` e `MARKET_SYNC_SECONDS` não é conveniência: um
/// teste que subisse os jobs bateria na Coinbase e na CoinGecko de verdade, e a
/// suíte passaria a falhar quando a rede oscilasse — ou pior, passaria a
/// depender da cotação do dia. As integrações externas são testadas com payload
/// versionado, em `payload_market.rs` e `payload_quotes.rs`.
pub fn test_config(database_url: String) -> Arc<Config> {
    Arc::new(Config {
        database_url,
        admin_secret_key: ADMIN_SECRET.to_string(),
        jwt_secret: JWT_SECRET.to_string(),
        // Sem HTTPS no teste: com `Secure`, o cookie não voltaria e todo fluxo
        // de sessão falharia por um motivo que não é o que está sendo testado.
        cookie_secure: false,
        bind_addr: "127.0.0.1:0".parse().expect("endereço de teste"),
        session_ttl_minutes: 10,
        refresh_ttl_days: 14,
        quotes_sync_minutes: 0,
        market_sync_seconds: 0,
    })
}

/// O app inteiro sobre um banco efêmero já migrado (o que o `#[sqlx::test]`
/// entrega).
pub fn app(db: PgPool) -> axum::Router {
    let config = test_config(String::new());
    App::router(AppState::with_pool(db, config))
}

/// Empurra a requisição pela pilha completa e devolve a resposta.
pub async fn send(app: &axum::Router, request: Request<Body>) -> AxumResponse {
    app.clone()
        .oneshot(request)
        .await
        .expect("o router não deve falhar em nível de serviço")
}

/// `GET` simples.
pub fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("requisição válida")
}

/// `GET` com cabeçalhos de sessão (cookies) já colados.
pub fn get_with_cookies(uri: &str, cookies: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::COOKIE, cookies)
        .body(Body::empty())
        .expect("requisição válida")
}

/// `POST` de JSON, como um consumidor da API faria.
pub fn post_json(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("requisição válida")
}

/// `POST` de JSON assinado com a credencial de admin.
pub fn post_json_as_admin(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, ADMIN_SECRET)
        .body(Body::from(body.to_string()))
        .expect("requisição válida")
}

/// `PATCH` de JSON assinado com a credencial de admin.
pub fn patch_json_as_admin(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, ADMIN_SECRET)
        .body(Body::from(body.to_string()))
        .expect("requisição válida")
}

/// `POST` de formulário com os cookies da sessão — o caminho do navegador.
pub fn post_form(uri: &str, cookies: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, cookies)
        .body(Body::from(body.to_string()))
        .expect("requisição válida")
}

/// Corpo da resposta como texto.
pub async fn text(response: Response<Body>) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo legível")
        .to_bytes();

    String::from_utf8_lossy(&bytes).into_owned()
}

/// Todos os `Set-Cookie` da resposta, reunidos no formato de um cabeçalho
/// `Cookie` — é assim que o navegador devolve a sessão na requisição seguinte.
pub fn cookies_from(response: &Response<Body>) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        // `nome=valor; Path=/; HttpOnly` → só o par nome=valor volta.
        .filter_map(|value| value.split(';').next())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Valor de um cookie específico na resposta (para o token CSRF, que o
/// formulário precisa repetir no corpo).
pub fn cookie_value(response: &Response<Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .find_map(|pair| {
            pair.strip_prefix(&format!("{name}="))
                .map(|value| value.to_string())
        })
}

/// Registra um usuário e devolve os cookies da sessão dele.
///
/// Passa pelo formulário REAL (`POST /register`), com token CSRF — e não por um
/// insert direto na tabela. É de propósito: a sessão que os testes seguintes
/// usam é uma sessão que o app emitiu, com o mesmo JWT, o mesmo refresh e a
/// mesma rotação que um navegador receberia.
pub async fn register(app: &axum::Router, username: &str, password: &str) -> String {
    // O GET traz o cookie do token CSRF e o campo escondido com o mesmo valor.
    // O nome do cookie vem da constante do próprio app: se ele for renomeado, o
    // teste quebra na compilação em vez de virar um 403 misterioso.
    let form = send(app, get("/register")).await;
    let cookies = cookies_from(&form);
    let csrf = cookie_value(&form, wallet::auth::csrf::CSRF_COOKIE)
        .expect("o formulário emite o token CSRF");

    let body = format!("username={username}&password={password}&csrf_token={csrf}");
    let response = send(app, post_form("/register", &cookies, &body)).await;

    assert_eq!(
        response.status(),
        StatusCode::SEE_OTHER,
        "registro deveria redirecionar para a carteira: {:?}",
        response.headers()
    );

    cookies_from(&response)
}
