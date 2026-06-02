use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::models::Asset;

/// Estado compartilhado entre todas as rotas. Precisa implementar `Clone`,
/// porque cada rota recebe a sua própria cópia do estado. O `Arc<Mutex<...>>`
/// garante que, mesmo clonadas, todas as rotas compartilham o mesmo
/// armazenamento por baixo (e o acesso mutável é serializado pela Mutex).
#[derive(Clone)]
pub struct AppState {
    pub assets: Arc<Mutex<HashMap<i64, Asset>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            assets: Default::default(),
        }
    }
}

/// Governa o serviço. Mantém a `main` enxuta e concentra aqui a inicialização.
pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        // Observabilidade: registra um subscriber de tracing que escreve os
        // logs no terminal usando a formatação padrão.
        let fmt_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry().with(fmt_layer).init();

        let listener = TcpListener::bind("0.0.0.0:3000").await?;
        info!("starting service");

        // Cada módulo de rotas devolve o seu próprio router; aqui só dizemos
        // que o sub-router vive sob /api e fornecemos o estado da aplicação.
        let router = Router::new()
            .nest("/api", crate::routes::api::router())
            .with_state(AppState::new());

        axum::serve(listener, router).await?;

        Ok(())
    }
}
