use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::Config;

/// Estado compartilhado entre as rotas. Guarda a pool de conexões com o Postgres
/// e a configuração do serviço (segredos já lidos do ambiente). A `PgPool` é
/// clonável (é um `Arc` por dentro) e a `Config` vai num `Arc`, então o
/// `#[derive(Clone)]` continua barato: clona-se ponteiro, não conexões nem
/// strings.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
}

impl AppState {
    pub async fn build(config: Arc<Config>) -> color_eyre::Result<Self> {
        let db = PgPool::connect(&config.database_url).await?;
        Ok(Self { db, config })
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
            .nest("/api", crate::routes::api::router())
            // `merge` monta as rotas do front-end na raiz (sem prefixo), ao
            // contrário do `nest` da API.
            .merge(crate::routes::frontend::router())
            .with_state(state);

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
