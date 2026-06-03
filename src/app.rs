use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Estado compartilhado entre as rotas. Agora guarda a pool de conexões com o
/// Postgres no lugar do HashMap em memória. A `PgPool` é clonável (é um `Arc`
/// por dentro), então o `#[derive(Clone)]` continua barato: clona-se o ponteiro,
/// não as conexões.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

impl AppState {
    pub async fn new() -> color_eyre::Result<Self> {
        // A string de conexão vem de uma variável de ambiente (ver .env).
        let database_url = std::env::var("DATABASE_URL")?;
        let db = PgPool::connect(&database_url).await?;
        Ok(Self { db })
    }
}

/// Governa o serviço. Mantém a `main` enxuta e concentra aqui a inicialização.
pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        // Carrega as variáveis do arquivo .env antes de construir o estado.
        dotenvy::dotenv()?;

        // Observabilidade: subscriber de tracing escrevendo no terminal.
        let fmt_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry().with(fmt_layer).init();

        let state = AppState::new().await?;

        let listener = TcpListener::bind("0.0.0.0:3000").await?;
        info!("starting service");

        let router = Router::new()
            .nest("/api", crate::routes::api::router())
            .with_state(state);

        axum::serve(listener, router).await?;

        Ok(())
    }
}
