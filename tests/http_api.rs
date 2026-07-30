//! Contrato HTTP da API de administração (`/api/v1/assets`).
//!
//! **A lacuna que esta bateria fecha.** Os testes de unidade em
//! `src/routes/api.rs` chamam os handlers direto — `create_asset(Admin, db, …)`
//! — passando o `Admin` como um valor construído à mão. Isso prova que a
//! escrita no banco funciona, e **passa por cima de tudo o que fica entre a rede
//! e o handler**: o roteamento, o alias `/api`, a desserialização do corpo, o
//! status HTTP devolvido e, o mais grave, o extrator `Admin` — a autorização
//! nunca é exercida, porque o teste a fabrica.
//!
//! Aqui a entrada é um payload JSON numa requisição HTTP atravessando o router
//! de produção inteiro. Se a autorização quebrar num refactor, é este arquivo
//! que grita.

mod common;

use axum::http::StatusCode;
use common::{app, get, patch_json_as_admin, post_json, post_json_as_admin, send, text};

/// **O teste de autorização que faltava.** A rota de escrita exige a credencial
/// de admin; sem ela, ou com uma errada, não passa. Um `403`/`401` aqui é a
/// única coisa entre um estranho e o preço que lastreia todas as operações da
/// carteira.
#[sqlx::test]
async fn writing_to_the_catalogue_requires_the_admin_credential(db: sqlx::PgPool) {
    let app = app(db);
    let payload = r#"{"name":"bitcoin","unit_value":"325611.00"}"#;

    // Sem credencial nenhuma.
    let anonymous = send(&app, post_json("/api/v1/assets", payload)).await;
    assert!(
        anonymous.status().is_client_error(),
        "criar ativo sem credencial devolveu {}",
        anonymous.status()
    );

    // Com credencial errada.
    let wrong = send(
        &app,
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/assets")
            .header("content-type", "application/json")
            .header("authorization", "nao-e-o-segredo")
            .body(axum::body::Body::from(payload))
            .expect("requisição válida"),
    )
    .await;
    assert!(
        wrong.status().is_client_error(),
        "credencial errada devolveu {}",
        wrong.status()
    );

    // Com a credencial certa, passa.
    let allowed = send(&app, post_json_as_admin("/api/v1/assets", payload)).await;
    assert_eq!(allowed.status(), StatusCode::OK, "admin deveria conseguir");
}

/// O ciclo de vida completo de um ativo pelo HTTP: criar, listar, corrigir. É o
/// caminho que um operador do catálogo percorre de verdade.
#[sqlx::test]
async fn the_catalogue_round_trips_through_real_http_requests(db: sqlx::PgPool) {
    let app = app(db);

    let created = send(
        &app,
        post_json_as_admin(
            "/api/v1/assets",
            r#"{"name":"bitcoin","unit_value":"325611.00"}"#,
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);

    let body = text(created).await;
    assert!(body.contains("\"name\":\"bitcoin\""), "resposta: {body}");
    // Dinheiro atravessa a API como STRING, nunca como número JSON: um `f64` no
    // meio do caminho é exatamente o que este projeto evita de ponta a ponta.
    assert!(
        body.contains("\"unit_value\":\"325611.00\""),
        "valor monetário deveria sair como string: {body}"
    );

    let listed = text(send(&app, get("/api/v1/assets")).await).await;
    assert!(listed.contains("bitcoin"));

    // Correção de preço pelo PATCH, com o id que o POST devolveu.
    let patched = send(
        &app,
        patch_json_as_admin("/api/v1/assets", r#"{"id":1,"unit_value":"400000.00"}"#),
    )
    .await;
    assert_eq!(patched.status(), StatusCode::OK);
    assert!(text(patched).await.contains("400000.00"));
}

/// A validação da borda: nome vazio e preço negativo são recusados com erro de
/// CLIENTE, não com 500. Um 500 aqui significaria que a validação está no banco
/// e não na borda — e o cliente receberia "internal server error" para um erro
/// que é dele.
#[sqlx::test]
async fn invalid_payloads_are_rejected_at_the_edge_as_client_errors(db: sqlx::PgPool) {
    let app = app(db);

    for (case, payload) in [
        ("nome vazio", r#"{"name":"","unit_value":"10.00"}"#),
        ("nome em branco", r#"{"name":"   ","unit_value":"10.00"}"#),
        ("preço negativo", r#"{"name":"btc","unit_value":"-1.00"}"#),
    ] {
        let response = send(&app, post_json_as_admin("/api/v1/assets", payload)).await;
        let status = response.status();

        assert!(
            status.is_client_error(),
            "{case}: devolveu {status}, e um payload inválido do cliente nunca é 5xx"
        );
        // A mensagem tem de dizer o que está errado — não "internal server
        // error", que é o texto reservado para falha NOSSA.
        let body = text(response).await;
        assert!(
            !body.contains("internal server error"),
            "{case}: erro do cliente vazou como erro interno: {body}"
        );
    }
}

/// Corpo que não é o JSON esperado (campo ausente, tipo trocado, JSON quebrado)
/// vira 4xx, nunca pânico nem 500.
#[sqlx::test]
async fn malformed_json_bodies_never_reach_the_handler(db: sqlx::PgPool) {
    let app = app(db);

    for (case, payload) in [
        ("json quebrado", "{"),
        ("vazio", ""),
        ("campo obrigatório ausente", r#"{"name":"btc"}"#),
        ("tipo trocado", r#"{"name":42,"unit_value":"10.00"}"#),
        (
            "preço não numérico",
            r#"{"name":"btc","unit_value":"muito"}"#,
        ),
    ] {
        let status = send(&app, post_json_as_admin("/api/v1/assets", payload))
            .await
            .status();

        assert!(status.is_client_error(), "{case}: devolveu {status}");
    }
}

/// Corrigir um ativo que não existe é `404`, não `500` nem `200` silencioso.
#[sqlx::test]
async fn patching_an_unknown_asset_is_a_404(db: sqlx::PgPool) {
    let app = app(db);

    let response = send(
        &app,
        patch_json_as_admin("/api/v1/assets", r#"{"id":9999,"unit_value":"1.00"}"#),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// O caminho canônico é `/api/v1`; `/api` fica como alias de compatibilidade
/// para consumidores que já existiam. Os dois têm de servir a MESMA coisa — o
/// versionamento só vale se o alias não divergir com o tempo.
#[sqlx::test(fixtures("catalogue"))]
async fn the_unversioned_alias_serves_the_same_thing_as_v1(db: sqlx::PgPool) {
    let app = app(db);

    let versioned = text(send(&app, get("/api/v1/assets")).await).await;
    let alias = text(send(&app, get("/api/assets")).await).await;

    assert_eq!(versioned, alias, "o alias /api divergiu de /api/v1");
    assert!(versioned.contains("bitcoin"));
}

/// A especificação OpenAPI é gerada do CÓDIGO (utoipa), então ela é a
/// documentação que não pode mentir — e é servida pela mesma pilha.
#[sqlx::test]
async fn the_openapi_spec_is_served_and_describes_the_real_routes(db: sqlx::PgPool) {
    let app = app(db);

    let response = send(&app, get("/api/v1/openapi.json")).await;
    assert_eq!(response.status(), StatusCode::OK);

    let spec = text(response).await;
    assert!(spec.contains("/api/v1/assets"), "spec: {spec}");
    assert!(spec.contains("CreateAssetRequest"));
    assert!(spec.contains("UpdateAssetRequest"));

    // A spec tem de ser JSON válido: uma spec malformada é pior que nenhuma,
    // porque um gerador de cliente a consome sem perguntar.
    let parsed: serde_json::Value = serde_json::from_str(&spec).expect("spec é JSON válido");
    assert!(parsed.get("openapi").is_some(), "falta a versão do OpenAPI");
    assert!(parsed.get("paths").is_some(), "falta a seção de caminhos");
}

/// **Toda** resposta carrega os cabeçalhos de segurança, inclusive as da API e
/// inclusive as de erro. É a camada mais externa do router, e o teste passa pela
/// pilha completa exatamente para provar que ela não foi contornada.
#[sqlx::test]
async fn every_api_response_carries_the_security_headers(db: sqlx::PgPool) {
    let app = app(db);

    // Uma resposta de sucesso e uma de erro de autorização.
    for request in [
        get("/api/v1/assets"),
        post_json("/api/v1/assets", r#"{"name":"x","unit_value":"1.00"}"#),
    ] {
        let response = send(&app, request).await;
        let headers = response.headers();

        let csp = headers
            .get("content-security-policy")
            .expect("CSP ausente")
            .to_str()
            .expect("ascii");
        // `'unsafe-inline'` em `style-src` é a diretiva que mais enfraquece uma
        // política, e foi removida de propósito quando o Tailwind saiu do CDN.
        assert!(
            !csp.contains("unsafe-inline"),
            "a CSP voltou a permitir estilo/script inline: {csp}"
        );
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
        // Resposta com dado privado nunca fica em cache compartilhado.
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    }
}
