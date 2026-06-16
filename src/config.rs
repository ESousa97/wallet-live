use std::net::SocketAddr;

use color_eyre::eyre::{Context, eyre};

/// Configuração do serviço, lida UMA vez na inicialização a partir do ambiente.
///
/// Centralizar isto aqui (em vez de chamar `std::env::var` espalhado pelo código)
/// dá dois ganhos de gente grande:
///
///  1. **Falha rápido.** Se um segredo obrigatório faltar, o serviço nem sobe — e
///     a mensagem diz exatamente qual variável está faltando. Antes, um
///     `JWT_SECRET` ausente só aparecia na primeira requisição, disfarçado de
///     `401 invalid credentials` (um erro de cliente para um problema de
///     configuração nosso).
///  2. **Sem releitura por requisição.** Validar um token ou conferir a credencial
///     do admin lia a variável de ambiente a cada chamada. Agora os segredos ficam
///     na mão, no estado compartilhado.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub admin_secret_key: String,
    pub jwt_secret: String,
    pub cookie_secure: bool,
    pub bind_addr: SocketAddr,
}

impl Config {
    /// Monta a configuração a partir das variáveis de ambiente. Os três segredos
    /// são obrigatórios; o resto tem padrão sensato para desenvolvimento.
    pub fn from_env() -> color_eyre::Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            admin_secret_key: required("ADMIN_SECRET_KEY")?,
            jwt_secret: required("JWT_SECRET")?,
            // Só é seguro (cookie `Secure`) atrás de HTTPS; default `false` para o
            // dev local em `http://localhost`.
            cookie_secure: std::env::var("COOKIE_SECURE")
                .map(|value| value == "true")
                .unwrap_or(false),
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
                .parse()
                .wrap_err("BIND_ADDR não é um endereço de socket válido (ex.: 0.0.0.0:3000)")?,
        })
    }
}

/// Lê uma variável obrigatória, rejeitando também o valor vazio — um segredo em
/// branco é tão perigoso quanto um ausente.
fn required(key: &str) -> color_eyre::Result<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(eyre!(
            "variável de ambiente obrigatória ausente ou vazia: {key}"
        )),
    }
}
