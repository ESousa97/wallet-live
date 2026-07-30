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
    /// Validade do token de ACESSO (JWT), em minutos. Curto de propósito: quem
    /// mantém a sessão viva é o refresh token.
    pub session_ttl_minutes: u64,
    /// Validade do REFRESH token (e da linha em `sessions`), em dias.
    pub refresh_ttl_days: u64,
    /// Intervalo do job de cotações, em minutos. Zero desliga o job.
    pub quotes_sync_minutes: u64,
    /// Intervalo do job da tela de mercado, em segundos. Zero desliga o job.
    /// O padrão acompanha o cache da fonte (~60 s): buscar mais rápido não
    /// traria número novo, só gastaria requisição do limite gratuito.
    pub market_sync_seconds: u64,
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
            session_ttl_minutes: optional_positive("SESSION_TTL_MINUTES", 10)?,
            refresh_ttl_days: optional_positive("REFRESH_TTL_DAYS", 14)?,
            quotes_sync_minutes: optional_non_negative("QUOTES_SYNC_MINUTES", 10)?,
            market_sync_seconds: optional_non_negative("MARKET_SYNC_SECONDS", 60)?,
        })
    }
}

/// Como `optional_positive`, mas aceita zero — usado onde zero significa
/// "desligado" (ex.: o job de cotações).
fn optional_non_negative(key: &str, default: u64) -> color_eyre::Result<u64> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| eyre!("{key} deve ser um inteiro >= 0 (recebido: {value:?})")),
    }
}

/// Lê um inteiro positivo opcional do ambiente, com um padrão. Zero é rejeitado:
/// um TTL de zero significaria sessões que já nascem expiradas.
fn optional_positive(key: &str, default: u64) -> color_eyre::Result<u64> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(parsed) if parsed > 0 => Ok(parsed),
            _ => Err(eyre!(
                "{key} deve ser um inteiro positivo (recebido: {value:?})"
            )),
        },
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
