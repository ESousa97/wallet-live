use serde::Serialize;

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
