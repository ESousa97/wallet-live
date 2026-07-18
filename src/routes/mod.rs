// Cada conjunto de rotas vive no seu próprio submódulo: a API REST do admin
// (JSON), o front-end SSR (HTML) e as flash messages que dão feedback nos
// formulários do front-end.
pub mod api;
pub mod flash;
pub mod frontend;
