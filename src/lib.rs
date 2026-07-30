//! Biblioteca da carteira. O binário (`src/main.rs`) é só um `main` que chama
//! `app::App::start`; tudo o que governa o serviço mora aqui.
//!
//! **Por que existe um alvo de biblioteca num projeto que entrega um servidor.**
//! Testes de integração (a pasta `tests/`) são *crates separados*: eles só
//! conseguem `use` de uma biblioteca, nunca de um binário. Enquanto os módulos
//! viviam dentro de `main.rs`, a única forma de testar era `#[cfg(test)] mod
//! tests` dentro de cada arquivo — o que é o idioma correto de Rust para
//! internos privados, mas deixa a suíte invisível para quem abre o repositório
//! e esconde o que é teste de contrato (payload de terceiro, resposta HTTP,
//! esquema da API) no meio do que é teste de unidade.
//!
//! Com a biblioteca exposta, as duas camadas ficam nos lugares certos:
//!
//! * `src/**/#[cfg(test)] mod tests` — unidade, com acesso ao que é privado
//!   (projeção do gráfico, inversão de taxa, montagem de URL).
//! * `tests/*.rs` — contrato, atravessando as mesmas funções públicas que o
//!   servidor atravessa, com os payloads REAIS versionados em
//!   `tests/payloads/` (ver `tests/payloads/README.md`).
//!
//! Os módulos são todos `pub` porque o consumidor é a própria suíte de
//! integração: é um crate de aplicação, não uma biblioteca publicada, e não há
//! API externa a preservar.

pub mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod i18n;
pub mod market;
pub mod models;
pub mod quotes;
pub mod repository;
pub mod routes;
pub mod services;
