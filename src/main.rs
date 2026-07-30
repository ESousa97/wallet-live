// A main fica o mais enxuta possível: o tokio::main cria um runtime assíncrono
// e tudo o que governa o serviço vive em `App::start` — dentro da biblioteca
// (`src/lib.rs`), para que a suíte de integração em `tests/` possa importar os
// mesmos módulos que o servidor executa.
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    wallet::app::App::start().await
}
