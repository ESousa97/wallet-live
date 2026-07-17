mod app;
pub mod auth;
mod config;
mod error;
pub mod models;
mod quotes;
pub mod repository;
pub mod routes;
pub mod services;

// A main fica o mais enxuta possível: o tokio::main cria um runtime assíncrono
// e tudo o que governa o serviço vive em `App::start`.
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    app::App::start().await
}
