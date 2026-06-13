use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use time::OffsetDateTime;

/// Um ativo financeiro que pode ser adquirido na plataforma (ex.: bitcoin).
/// Por enquanto `unit_value` é um `f64`; no mundo real você usaria um tipo
/// decimal de uma biblioteca como `rust_decimal`.
#[derive(Clone, Serialize)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
}

/// Um usuário como ele está no banco de dados — incluindo a `password_hash`.
/// De propósito NÃO deriva `Serialize`: não queremos formatá-lo como resposta de
/// nenhum endpoint nem arriscar vazar a hash da senha. É só uma representação
/// crua da linha da tabela; o usuário "de verdade" (autenticado) é outra struct,
/// no módulo `auth::user`.
pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

/// Uma linha crua da tabela `owned_assets`: o usuário `user_id` comprou
/// `quantity` unidades do ativo `asset_id` por `unit_value` cada. `id` e
/// `bought_at` são gerados pelo próprio banco (BIGSERIAL e `DEFAULT NOW()`).
pub struct OwnedAssetRecord {
    pub id: i64,
    pub user_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub unit_value: f64,
    pub bought_at: OffsetDateTime,
}

/// Uma compra dentro do histórico de um ativo. Diferente do `OwnedAssetRecord`,
/// este tipo não vem direto de colunas: o Postgres o monta como JSON
/// (`json_agg`/`json_build_object`) dentro da query de `list_owned_assets`, então
/// precisa de `Deserialize`. O Postgres emite `timestamptz` em JSON no formato
/// RFC 3339, daí o `time::serde::rfc3339` em `bought_at`.
#[derive(Debug, Deserialize)]
pub struct PurchaseRecord {
    #[serde(with = "time::serde::rfc3339")]
    pub bought_at: OffsetDateTime,
    pub unit_value: f64,
    pub quantity: f64,
    pub value_delta: f64,
}

/// Resumo de um ativo que o usuário possui hoje: quanto ele tem, quanto
/// lucrou/perdeu no total e o histórico de compras por trás desse total. Não
/// mapeia 1:1 com nenhuma tabela — é o resultado de agregar, por `asset_id`,
/// todas as linhas de `owned_assets` daquele usuário.
pub struct OwnedAsset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
    pub value_delta: f64,
    pub quantity_owned: f64,
    pub purchase_history: Json<Vec<PurchaseRecord>>,
}
