use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::AppError;

/// Tentativas de login toleradas antes de começar o lockout.
const FREE_ATTEMPTS: u32 = 5;
/// Duração do primeiro bloqueio; dobra a cada falha seguinte (backoff).
const BASE_LOCK: Duration = Duration::from_secs(30);
/// Teto do bloqueio, para o backoff não crescer sem limite.
const MAX_LOCK: Duration = Duration::from_secs(15 * 60);
/// Falhas antigas são perdoadas depois deste intervalo sem novas falhas.
const FORGET_AFTER: Duration = Duration::from_secs(60 * 60);
/// Acima disto, entradas vencidas são varridas ao registrar novas falhas —
/// impede que um atacante infle o mapa com usernames inventados.
const PRUNE_THRESHOLD: usize = 4096;

/// Mitigação de força bruta no login: conta falhas consecutivas POR USUÁRIO e,
/// a partir de `FREE_ATTEMPTS`, impõe um bloqueio com backoff exponencial.
/// Login correto zera o contador.
///
/// O estado vive em memória (reinicia com o processo) — suficiente para uma
/// instância única. Com múltiplas réplicas, este estado migraria para um
/// armazenamento compartilhado.
#[derive(Default)]
pub struct LoginThrottle {
    entries: Mutex<HashMap<String, Entry>>,
}

struct Entry {
    failures: u32,
    last_failure: Instant,
}

impl LoginThrottle {
    /// Rejeita com `TooManyAttempts` se o usuário está em período de bloqueio.
    /// Chamar ANTES de verificar a senha: durante o bloqueio nem a senha certa
    /// passa — é isso que tira o lucro do ataque de força bruta.
    pub async fn ensure_allowed(&self, username: &str) -> Result<(), AppError> {
        let key = normalize(username);
        let mut entries = self.entries.lock().await;

        let Some(entry) = entries.get(&key) else {
            return Ok(());
        };

        // Falhas velhas demais são perdoadas.
        if entry.last_failure.elapsed() >= FORGET_AFTER {
            entries.remove(&key);
            return Ok(());
        }

        if entry.failures >= FREE_ATTEMPTS
            && entry.last_failure.elapsed() < lock_duration(entry.failures)
        {
            return Err(AppError::TooManyAttempts);
        }

        Ok(())
    }

    pub async fn record_failure(&self, username: &str) {
        let key = normalize(username);
        let mut entries = self.entries.lock().await;

        // Higiene do mapa: sem isto, falhas com usernames aleatórios fariam a
        // memória crescer para sempre.
        if entries.len() >= PRUNE_THRESHOLD {
            entries.retain(|_, entry| entry.last_failure.elapsed() < FORGET_AFTER);
        }

        let entry = entries.entry(key).or_insert(Entry {
            failures: 0,
            last_failure: Instant::now(),
        });
        entry.failures += 1;
        entry.last_failure = Instant::now();
    }

    pub async fn record_success(&self, username: &str) {
        self.entries.lock().await.remove(&normalize(username));
    }
}

/// O mesmo usuário digitado com caixa diferente conta como o mesmo alvo.
fn normalize(username: &str) -> String {
    username.trim().to_lowercase()
}

/// Backoff exponencial: 30s no primeiro bloqueio, dobrando a cada falha extra,
/// com teto de 15 minutos.
fn lock_duration(failures: u32) -> Duration {
    let exponent = failures.saturating_sub(FREE_ATTEMPTS).min(5);
    BASE_LOCK.saturating_mul(2u32.pow(exponent)).min(MAX_LOCK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_attempts_below_the_threshold() {
        let throttle = LoginThrottle::default();

        for _ in 0..FREE_ATTEMPTS - 1 {
            throttle.record_failure("alice").await;
        }

        assert!(throttle.ensure_allowed("alice").await.is_ok());
    }

    #[tokio::test]
    async fn locks_after_too_many_failures() {
        let throttle = LoginThrottle::default();

        for _ in 0..FREE_ATTEMPTS {
            throttle.record_failure("alice").await;
        }

        assert!(matches!(
            throttle.ensure_allowed("alice").await,
            Err(AppError::TooManyAttempts)
        ));
        // A caixa do username não escapa do bloqueio.
        assert!(matches!(
            throttle.ensure_allowed("  ALICE ").await,
            Err(AppError::TooManyAttempts)
        ));
        // Outros usuários não são afetados.
        assert!(throttle.ensure_allowed("bob").await.is_ok());
    }

    #[tokio::test]
    async fn success_clears_the_counter() {
        let throttle = LoginThrottle::default();

        for _ in 0..FREE_ATTEMPTS {
            throttle.record_failure("alice").await;
        }
        throttle.record_success("alice").await;

        assert!(throttle.ensure_allowed("alice").await.is_ok());
    }

    #[test]
    fn lock_duration_backs_off_exponentially_with_a_cap() {
        assert_eq!(lock_duration(FREE_ATTEMPTS), Duration::from_secs(30));
        assert_eq!(lock_duration(FREE_ATTEMPTS + 1), Duration::from_secs(60));
        assert_eq!(lock_duration(FREE_ATTEMPTS + 2), Duration::from_secs(120));
        // Muito acima do teto: trava em 15 minutos.
        assert_eq!(lock_duration(FREE_ATTEMPTS + 40), MAX_LOCK);
    }
}
