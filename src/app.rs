use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::auth::throttle::LoginThrottle;
use crate::config::Config;

/// Estado compartilhado entre as rotas. Guarda a pool de conexões com o Postgres,
/// a configuração do serviço (segredos já lidos do ambiente) e o contador de
/// falhas de login. A `PgPool` é clonável (é um `Arc` por dentro) e o resto vai
/// em `Arc`, então o `#[derive(Clone)]` continua barato: clona-se ponteiro, não
/// conexões nem strings.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub login_throttle: Arc<LoginThrottle>,
}

impl AppState {
    pub async fn build(config: Arc<Config>) -> color_eyre::Result<Self> {
        let db = PgPool::connect(&config.database_url).await?;
        Ok(Self {
            db,
            config,
            login_throttle: Arc::new(LoginThrottle::default()),
        })
    }
}

/// Governa o serviço. Mantém a `main` enxuta e concentra aqui a inicialização.
pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        // Hooks de erro/panic mais legíveis (backtraces formatadas). Combina com a
        // `main` devolvendo `color_eyre::Result`.
        color_eyre::install()?;

        // Carrega o `.env` se existir. NÃO é fatal não existir: em produção as
        // variáveis vêm do ambiente de verdade, sem arquivo nenhum.
        let _ = dotenvy::dotenv();

        init_tracing();

        // Toda a configuração é validada já aqui: se faltar um segredo, o serviço
        // morre no boot com uma mensagem clara em vez de na primeira requisição.
        let config = Arc::new(Config::from_env()?);
        let bind_addr = config.bind_addr;

        let state = AppState::build(config).await?;

        let listener = TcpListener::bind(bind_addr).await?;
        info!(%bind_addr, "starting service");

        let router = Router::new()
            .route("/health", get(health))
            // Caminho canônico e versionado da API: mudanças incompatíveis
            // futuras entram como /api/v2 sem quebrar consumidores do v1.
            .nest("/api/v1", crate::routes::api::router())
            // Alias de compatibilidade para consumidores existentes de /api.
            .nest("/api", crate::routes::api::router())
            // `merge` monta as rotas do front-end na raiz (sem prefixo), ao
            // contrário do `nest` da API.
            .merge(crate::routes::frontend::router())
            .with_state(state.clone())
            // Renova sessões com access token expirado e refresh token válido
            // (rotacionando o refresh) antes de qualquer handler rodar.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::auth::session::refresh_session,
            ))
            // Cabeçalhos de segurança em TODA resposta (camada mais externa).
            .layer(axum::middleware::from_fn_with_state(
                state,
                security_headers,
            ));

        // `with_graceful_shutdown` deixa as requisições em voo terminarem quando
        // chega um Ctrl+C, em vez de cortar conexões no meio.
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

/// Subscriber de tracing escrevendo no terminal, com nível controlável via
/// `RUST_LOG` (ex.: `RUST_LOG=wallet=debug`). Sem a variável, usa `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Cabeçalhos de segurança aplicados a toda resposta:
///
/// - **CSP** restringe de onde a página pode carregar código/estilo: scripts só
///   do próprio site e do CDN do Tailwind (dependência atual dos templates);
///   `style-src 'unsafe-inline'` porque o Tailwind em modo CDN injeta `<style>`
///   em runtime; `frame-ancestors 'none'` bloqueia clickjacking; `form-action
///   'self'` impede formulários de postarem para fora.
/// - **nosniff** impede o navegador de "adivinhar" content-type.
/// - **Referrer-Policy** não vaza URLs internas para sites de destino.
/// - **HSTS** só quando o serviço está atrás de HTTPS (mesmo sinal do cookie
///   `Secure`); enviá-lo em HTTP local só causaria confusão.
async fn security_headers(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' https://cdn.tailwindcss.com; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             connect-src 'self'; frame-ancestors 'none'; form-action 'self'; \
             base-uri 'self'; object-src 'none'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );

    if state.config.cookie_secure {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=63072000; includeSubDomains"),
        );
    }

    response
}

/// Sonda de saúde: confirma que o serviço está de pé E que o banco responde.
/// Útil para health checks de orquestradores (Docker, Kubernetes, load balancer).
async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// Resolve quando o processo recebe um Ctrl+C. Alimenta o desligamento gracioso.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_ok() {
        info!("shutdown signal received");
    }
}
