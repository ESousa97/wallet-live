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
