//! Cotações de mercado: preço em BRL, variações e série temporal das
//! criptomoedas acompanhadas.
//!
//! Fonte: API pública da CoinGecko (`/coins/markets`), que responde sem chave,
//! já converte para BRL e devolve variação percentual e série pronta — 100
//! moedas numa requisição. O lado de lá recalcula a cada ~60 s, então esse é o
//! piso real de "tempo real" nesta fonte; pedir mais rápido só gastaria
//! requisição.
//!
//! **Este feed é informativo e não move dinheiro.** Ele alimenta a tela de
//! mercado e nada além dela. O preço que lastreia compra, venda e saldo
//! continua vindo do catálogo (`assets.unit_value`), gravado a partir de taxas
//! que a API de câmbio entrega como **string** e viram `Decimal` sem passar
//! por ponto flutuante (ver `quotes.rs`). Aqui a CoinGecko devolve números
//! JSON, que o `serde_json` decodifica como `f64` — precisão suficiente para
//! exibir uma cotação, insuficiente para contabilizar patrimônio.

use std::sync::Arc;

use rust_decimal::Decimal;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::MONEY_SCALE;

/// 100 moedas por capitalização, em BRL, numa chamada.
///
/// `sparkline=true` traz a série HORÁRIA dos últimos 7 dias (168 amostras por
/// moeda) — é ela que sustenta o gráfico temporal do painel, e vem no mesmo
/// pedido: nenhuma chamada extra por moeda selecionada, nenhum limite de taxa
/// dependente de quantas pessoas abriram a tela. `price_change_percentage`
/// acrescenta as janelas de 1 h e 7 d à de 24 h, que já vinha por padrão.
const MARKETS_URL: &str = "https://api.coingecko.com/api/v3/coins/markets\
?vs_currency=brl&order=market_cap_desc&per_page=100&page=1&locale=pt\
&sparkline=true&price_change_percentage=1h,24h,7d";

/// Casas decimais da variação percentual. Duas bastam: a fonte não publica
/// mais que isso de significativo.
const CHANGE_SCALE: u32 = 2;

/// Casas decimais dos agregados (capitalização, volume, oferta). São números
/// grandes e puramente informativos — nunca entram numa soma da carteira —, e
/// exibi-los com escala de dinheiro só gastaria dígitos.
const AGGREGATE_SCALE: u32 = 2;

/// A CoinGecko responde **403 a requisição sem `User-Agent`** — o `reqwest`
/// não manda nenhum por padrão, então sem isto o feed nunca sobe. Descoberto
/// na primeira rodada real: a mesma URL respondia 200 no navegador e no
/// PowerShell (que mandam UA) e 403 no serviço.
const USER_AGENT: &str = concat!("wallet/", env!("CARGO_PKG_VERSION"));

/// Cliente reaproveitado entre as rodadas: mantém o pool de conexões e o
/// handshake TLS em vez de refazer os dois a cada minuto.
fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("cliente HTTP do mercado")
    })
}

/// Uma moeda como aparece na tela de mercado.
///
/// Os campos de variação e de agregado guardam ZERO quando a fonte não publica
/// o número (moeda recém-listada, sem série de 24 h, sem oferta divulgada). É
/// um "não informado" que o template trata como neutro — a linha continua útil
/// pelo preço, que é o único campo sem o qual a moeda é descartada.
#[derive(Clone)]
// Só os testes de renderização constroem uma moeda campo a campo — e eles
// preenchem apenas o que a asserção olha. Fora deles, `Coin` nasce de uma
// resposta da fonte, e um valor "vazio" não teria sentido nenhum.
#[cfg_attr(test, derive(Default))]
pub struct Coin {
    /// Identificador da CoinGecko (`bitcoin`, `ethereum`, …). É a chave de
    /// seleção da tela: estável, única e legível na barra de endereço — ao
    /// contrário do ticker, que a fonte não garante único.
    pub id: String,
    pub rank: i64,
    pub symbol: String,
    pub name: String,
    pub price: Decimal,
    /// Variação percentual da última hora.
    pub change_1h: Decimal,
    /// Variação percentual das últimas 24 h.
    pub change_24h: Decimal,
    /// Variação percentual dos últimos 7 dias.
    pub change_7d: Decimal,
    /// Capitalização de mercado (preço × oferta em circulação).
    pub market_cap: Decimal,
    /// Volume financeiro negociado em 24 h.
    pub volume_24h: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    /// Máxima histórica (ATH) e a distância percentual até ela (negativa).
    pub ath: Decimal,
    pub ath_change_pct: Decimal,
    pub circulating_supply: Decimal,
    /// Série horária dos últimos 7 dias, como a fonte entrega. Fica em `f64`
    /// de propósito: é coordenada de desenho, não dinheiro — o que a tela
    /// mostra como número (mínima, máxima, último) volta a `Decimal` na hora
    /// de formatar. Só o projetor do gráfico a lê; ninguém contabiliza com
    /// ela.
    pub(crate) series: Vec<f64>,
}

impl Coin {
    /// Casa a moeda com um termo de busca **já normalizado** (minúsculo e sem
    /// espaços nas pontas). Ticker e nome contam: quem procura "btc" e quem
    /// procura "bitcoin" chegam no mesmo lugar.
    pub fn matches(&self, needle: &str) -> bool {
        self.symbol.to_lowercase().contains(needle) || self.name.to_lowercase().contains(needle)
    }

    /// Posição da cotação dentro da faixa de negociação de 24 h, já convertida
    /// para a coordenada do marcador no `viewBox` do medidor.
    ///
    /// Vem pronta em coordenada porque a CSP fecha `style-src` em `'self'`:
    /// sem `'unsafe-inline'` não existe `style="width:63%"`, e todo indicador
    /// proporcional desta interface é geometria de SVG — atributo XML, que a
    /// política não bloqueia. A margem lateral existe para que o marcador
    /// continue inteiro nos extremos (cotação no fundo ou no topo do dia é o
    /// caso comum, não o raro).
    pub fn trading_range_x(&self) -> Option<String> {
        if self.low_24h <= Decimal::ZERO || self.high_24h <= self.low_24h {
            return None;
        }

        // A fonte publica preço, mínima e máxima em apurações diferentes; um
        // preço fora da faixa por alguns centavos acontece e não pode empurrar
        // o marcador para fora do medidor.
        let ratio = ((self.price - self.low_24h) / (self.high_24h - self.low_24h))
            .clamp(Decimal::ZERO, Decimal::ONE);
        let pad = Decimal::from(METER_PAD as i64);
        let span = Decimal::from((METER_W - 2.0 * METER_PAD) as i64);

        Some(format!("{:.1}", pad + ratio * span))
    }

    /// Projeta a janela pedida da série no `viewBox` do gráfico. `None` quando
    /// a fonte não mandou série suficiente (um ponto não é uma linha).
    ///
    /// `updated_at` ancora o eixo do tempo: as amostras são horárias e a
    /// última é a do instante da coleta, então cada ponto recua uma hora a
    /// partir dali. É uma aproximação — a fonte fecha a última amostra na
    /// virada da hora, não no segundo em que buscamos —, e é por isso que os
    /// rótulos do eixo marcam hora e dia, nunca minuto exato.
    pub fn chart(&self, range: Range, updated_at: Option<OffsetDateTime>) -> Option<PriceChart> {
        project(range.window(&self.series), updated_at)
    }
}

/// Janela do gráfico temporal.
///
/// As duas saem da MESMA série horária de 7 dias que já veio na resposta: 24 h
/// é o rabo dela. Trocar de janela não custa requisição nenhuma à fonte.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Range {
    /// Últimas 24 h (25 amostras horárias fecham 24 intervalos).
    Day,
    /// Últimos 7 dias — a série inteira que a fonte entrega.
    #[default]
    Week,
}

impl Range {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "24h" => Some(Self::Day),
            "7d" => Some(Self::Week),
            _ => None,
        }
    }

    /// Valor canônico na query string (e aceito de volta por `from_tag`).
    pub fn tag(self) -> &'static str {
        match self {
            Self::Day => "24h",
            Self::Week => "7d",
        }
    }

    pub fn is_day(self) -> bool {
        self == Self::Day
    }

    pub fn is_week(self) -> bool {
        self == Self::Week
    }

    /// O trecho final da série que esta janela cobre.
    fn window(self, series: &[f64]) -> &[f64] {
        match self {
            Self::Day => &series[series.len().saturating_sub(HOURS_IN_DAY + 1)..],
            Self::Week => series,
        }
    }
}

/// Amostras horárias que fecham um dia.
const HOURS_IN_DAY: usize = 24;

/// `viewBox` do medidor da faixa de negociação, e a folga lateral do raio do
/// marcador (ver `Coin::trading_range_x`).
const METER_W: f64 = 600.0;
const METER_PAD: f64 = 10.0;

/// Dimensões do `viewBox` do gráfico. A proporção é fixa e o SVG escala
/// uniformemente, para que o marcador do último ponto continue redondo em
/// qualquer largura de tela.
const CHART_W: f64 = 640.0;
const CHART_H: f64 = 176.0;
/// Margem lateral: o marcador tem raio 5 e ganha um anel de 2 px, então precisa
/// de folga para não ser cortado pela borda.
const PLOT_PAD_X: f64 = 10.0;
const PLOT_TOP: f64 = 12.0;
const PLOT_BOTTOM: f64 = CHART_H - 12.0;
/// Linhas de grade horizontais (incluindo topo e base da área de plotagem).
const GRID_LINES: usize = 4;
/// Marcas de tempo no eixo horizontal.
const AXIS_TICKS: usize = 5;

/// Série temporal pronta para desenhar: os caminhos já vêm projetados no
/// `viewBox` do SVG, então o template só interpola strings — zero JavaScript,
/// amigável à CSP. Mesmo desenho do gráfico de patrimônio da carteira
/// (`services::portfolio`), com o eixo do tempo a mais.
pub struct PriceChart {
    /// Caminho da linha (`d` de um `<path>`).
    pub line: String,
    /// Mesmo caminho fechado contra a base, para o preenchimento em wash.
    pub area: String,
    /// Coordenadas do último ponto, onde vai o marcador.
    pub last_x: String,
    pub last_y: String,
    /// Alturas das linhas de grade.
    pub grid: Vec<String>,
    /// Rótulos do eixo do tempo, do mais antigo ao mais recente e igualmente
    /// espaçados — como as amostras que eles datam. Saem como TEXTO HTML sob o
    /// gráfico, e não como `<text>` dentro do SVG: o desenho escala com a
    /// largura da tela, e a tipografia não pode escalar junto (num celular o
    /// rótulo viraria uma mancha de 5 px).
    pub ticks: Vec<String>,
    /// O `viewBox` e as bordas da área de plotagem, para que o template não
    /// repita nenhum número desta geometria — ele desenha onde o projetor
    /// mandar.
    pub view_box: String,
    pub plot_left: String,
    pub plot_right: String,
    /// Primeira cotação da janela (abertura do período).
    pub open: Decimal,
    pub last: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    /// Variação da abertura ao último ponto da janela, em %.
    pub delta_pct: Option<Decimal>,
}

impl PriceChart {
    /// A janela fechou em alta? Decide a cor do traço — sempre acompanhada do
    /// percentual com sinal, nunca cor sozinha.
    pub fn is_up(&self) -> bool {
        self.delta_pct.unwrap_or(Decimal::ZERO) > Decimal::ZERO
    }

    pub fn is_down(&self) -> bool {
        self.delta_pct.unwrap_or(Decimal::ZERO) < Decimal::ZERO
    }
}

/// Projeta a janela no `viewBox`: x distribui as amostras uniformemente (elas
/// são equidistantes no tempo), y escala entre a mínima e a máxima do período.
/// Série constante vira uma linha reta no meio — sem divisão por zero.
fn project(series: &[f64], updated_at: Option<OffsetDateTime>) -> Option<PriceChart> {
    if series.len() < 2 || series.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let min = series.iter().copied().fold(f64::INFINITY, f64::min);
    let max = series.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;

    let last_index = (series.len() - 1) as f64;
    let span_x = CHART_W - 2.0 * PLOT_PAD_X;
    let span_y = PLOT_BOTTOM - PLOT_TOP;

    let coords: Vec<(f64, f64)> = series
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = PLOT_PAD_X + (index as f64 / last_index) * span_x;
            let y = if range <= f64::EPSILON {
                PLOT_TOP + span_y / 2.0
            } else {
                PLOT_TOP + (1.0 - (value - min) / range) * span_y
            };
            (x, y)
        })
        .collect();

    let line = coords
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            let verb = if index == 0 { 'M' } else { 'L' };
            format!("{verb}{x:.2} {y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ");

    // O preenchimento é a mesma linha fechada até a base da área de plotagem.
    let (first_x, _) = coords[0];
    let (last_x, last_y) = *coords.last().expect("série não vazia");
    let area = format!("{line} L{last_x:.2} {PLOT_BOTTOM:.2} L{first_x:.2} {PLOT_BOTTOM:.2} Z");

    let grid = (0..GRID_LINES)
        .map(|index| {
            let y = PLOT_TOP + span_y * index as f64 / (GRID_LINES - 1) as f64;
            format!("{y:.2}")
        })
        .collect();

    let open = decimal_from_f64(series[0], MONEY_SCALE)?;
    let last = decimal_from_f64(*series.last().expect("série não vazia"), MONEY_SCALE)?;

    Some(PriceChart {
        line,
        area,
        last_x: format!("{last_x:.2}"),
        last_y: format!("{last_y:.2}"),
        grid,
        ticks: axis_ticks(series.len(), updated_at),
        view_box: format!("0 0 {CHART_W:.0} {CHART_H:.0}"),
        plot_left: format!("{PLOT_PAD_X:.0}"),
        plot_right: format!("{:.0}", CHART_W - PLOT_PAD_X),
        open,
        last,
        min: decimal_from_f64(min, MONEY_SCALE)?,
        max: decimal_from_f64(max, MONEY_SCALE)?,
        delta_pct: crate::models::percent_of(last - open, open),
    })
}

/// Rótulos do eixo do tempo, um para cada fração igual da janela — a mesma
/// distribuição das amostras, que são equidistantes.
///
/// Sem `updated_at` (antes da primeira rodada) não há como datar a série, e o
/// gráfico sai sem eixo em vez de sair com rótulo inventado.
fn axis_ticks(points: usize, updated_at: Option<OffsetDateTime>) -> Vec<String> {
    let Some(end) = updated_at else {
        return Vec::new();
    };
    let last_index = points - 1;
    // Janela curta o bastante para caber em um dia recebe rótulo de hora;
    // acima disso, de data — o que muda entre as pontas é o dia, não a hora.
    let hourly = last_index <= HOURS_IN_DAY;

    (0..AXIS_TICKS)
        .map(|tick| {
            let index = last_index * tick / (AXIS_TICKS - 1);
            let at = end - time::Duration::hours((last_index - index) as i64);

            if hourly {
                format!("{:02}:{:02}", at.hour(), at.minute())
            } else {
                format!("{:02}/{:02}", at.day(), at.month() as u8)
            }
        })
        .collect()
}

/// O que a tela consome: a lista e o instante em que ela foi buscada.
///
/// A lista vai num `Arc` para que ler o snapshot seja clonar um ponteiro — o
/// lock de leitura fecha imediatamente, e a renderização acontece fora dele.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub coins: Arc<Vec<Coin>>,
    pub updated_at: Option<OffsetDateTime>,
    /// A última tentativa falhou (ou o feed foi desativado). O último snapshot
    /// bom, se existir, continua disponível e pode ser mostrado como defasado.
    pub refresh_failed: bool,
}

impl Snapshot {
    /// A moeda de `id`, ou a primeira do ranking quando o pedido não casa com
    /// nada — a tela sempre abre com um painel preenchido, nunca com um vazio
    /// por causa de um link velho ou de um parâmetro digitado à mão.
    pub fn select(&self, id: Option<&str>) -> Option<&Coin> {
        id.and_then(|id| self.coins.iter().find(|coin| coin.id == id))
            .or_else(|| self.coins.first())
    }

    /// Verdadeiro antes da primeira rodada bem-sucedida.
    pub fn is_empty(&self) -> bool {
        self.coins.is_empty()
    }
}

/// Cache do mercado, compartilhado entre as rotas via `AppState`.
#[derive(Default)]
pub struct Market {
    inner: RwLock<Snapshot>,
}

impl Market {
    pub async fn snapshot(&self) -> Snapshot {
        self.inner.read().await.clone()
    }

    /// Busca e substitui o snapshot. Devolve quantas moedas entraram.
    pub async fn refresh(&self) -> Result<usize, AppError> {
        let coins = match fetch().await {
            Ok(coins) => coins,
            Err(error) => {
                self.inner.write().await.refresh_failed = true;
                return Err(error);
            }
        };
        let total = coins.len();

        *self.inner.write().await = Snapshot {
            coins: Arc::new(coins),
            updated_at: Some(OffsetDateTime::now_utc()),
            refresh_failed: false,
        };

        Ok(total)
    }

    async fn mark_unavailable(&self) {
        self.inner.write().await.refresh_failed = true;
    }
}

/// Sobe o job do mercado: uma rodada imediata no boot e depois uma a cada
/// `MARKET_SYNC_SECONDS` (zero desliga). Falha de rodada é logada e a próxima
/// tenta de novo — a tela segue mostrando o último snapshot bom, com o horário
/// dele à vista, em vez de esvaziar.
pub fn spawn_scheduled_refresh(market: Arc<Market>, seconds: u64) {
    if seconds == 0 {
        info!("scheduled market refresh disabled");
        tokio::spawn(async move {
            market.mark_unavailable().await;
        });
        return;
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(seconds));

        loop {
            // O primeiro tick resolve na hora: o boot já busca.
            interval.tick().await;
            match market.refresh().await {
                Ok(coins) => info!(coins, "market refresh"),
                Err(error) => warn!(?error, "market refresh failed"),
            }
        }
    });
}

#[derive(Deserialize)]
struct MarketRow {
    id: String,
    symbol: String,
    name: String,
    current_price: Option<f64>,
    price_change_percentage_1h_in_currency: Option<f64>,
    price_change_percentage_24h: Option<f64>,
    price_change_percentage_7d_in_currency: Option<f64>,
    market_cap: Option<f64>,
    market_cap_rank: Option<i64>,
    total_volume: Option<f64>,
    high_24h: Option<f64>,
    low_24h: Option<f64>,
    ath: Option<f64>,
    ath_change_percentage: Option<f64>,
    circulating_supply: Option<f64>,
    sparkline_in_7d: Option<Series>,
}

#[derive(Deserialize)]
struct Series {
    price: Vec<f64>,
}

impl MarketRow {
    fn into_coin(self) -> Option<Coin> {
        // Moeda sem preço (recém-listada, sem par em BRL) não entra na tela:
        // uma linha vazia é pior que uma linha a menos.
        let price = decimal_from_f64(self.current_price?, MONEY_SCALE)?;

        Some(Coin {
            id: self.id,
            rank: self.market_cap_rank.unwrap_or(i64::MAX),
            symbol: self.symbol.to_uppercase(),
            name: self.name,
            price,
            change_1h: percent_or_zero(self.price_change_percentage_1h_in_currency),
            change_24h: percent_or_zero(self.price_change_percentage_24h),
            change_7d: percent_or_zero(self.price_change_percentage_7d_in_currency),
            market_cap: aggregate_or_zero(self.market_cap),
            volume_24h: aggregate_or_zero(self.total_volume),
            high_24h: money_or_zero(self.high_24h),
            low_24h: money_or_zero(self.low_24h),
            ath: money_or_zero(self.ath),
            ath_change_pct: percent_or_zero(self.ath_change_percentage),
            circulating_supply: aggregate_or_zero(self.circulating_supply),
            series: self.sparkline_in_7d.map(|s| s.price).unwrap_or_default(),
        })
    }
}

/// Campo ausente (ou não representável) vira zero: a moeda continua na tela
/// pelo preço, e o template lê zero como "sem variação/sem número".
fn percent_or_zero(value: Option<f64>) -> Decimal {
    scaled_or_zero(value, CHANGE_SCALE)
}

fn money_or_zero(value: Option<f64>) -> Decimal {
    scaled_or_zero(value, MONEY_SCALE)
}

fn aggregate_or_zero(value: Option<f64>) -> Decimal {
    scaled_or_zero(value, AGGREGATE_SCALE)
}

fn scaled_or_zero(value: Option<f64>, scale: u32) -> Decimal {
    value
        .and_then(|value| decimal_from_f64(value, scale))
        .unwrap_or(Decimal::ZERO)
}

async fn fetch() -> Result<Vec<Coin>, AppError> {
    let body = client()
        .get(MARKETS_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_markets(&body)
}

/// Decodifica o corpo da resposta da fonte na lista que a tela consome.
///
/// **Por que é uma função pública separada do `fetch`.** Enquanto decodificar
/// era um `.json()` no meio da chamada HTTP, a única forma de exercitar o
/// contrato com a CoinGecko era inventar um `MarketRow` campo a campo — ou
/// seja, testar contra a nossa ideia do formato, nunca contra o formato. Com o
/// parse separado, a suíte de integração atravessa exatamente este caminho com
/// a resposta REAL versionada em `tests/payloads/coingecko_markets.json`: se a
/// fonte renomear um campo, trocar um número por string ou passar a mandar
/// `null` onde mandava valor, o teste quebra aqui — em vez de a tela silenciar
/// em produção.
///
/// A ordenação por ranking é nossa, não da fonte: `order=market_cap_desc` já
/// vem ordenado, mas depender disso deixaria a lista à mercê de um parâmetro
/// numa URL.
pub fn parse_markets(body: &str) -> Result<Vec<Coin>, AppError> {
    let rows: Vec<MarketRow> = serde_json::from_str(body)?;

    let mut coins: Vec<Coin> = rows.into_iter().filter_map(MarketRow::into_coin).collect();
    coins.sort_by_key(|coin| coin.rank);
    Ok(coins)
}

/// `f64` da resposta JSON para `Decimal`, com a escala travada.
///
/// O `round_dp` não é cosmético: `from_f64_retain` traz o erro de
/// representação do binário (0,1 vira 0,1000000000000000055…), e um decimal
/// de 28 casas escapando para o resto do sistema foi exatamente o que já
/// derrubou o resumo da carteira uma vez. Trava aqui, na fronteira.
fn decimal_from_f64(value: f64, scale: u32) -> Option<Decimal> {
    if !value.is_finite() {
        return None;
    }
    Some(Decimal::from_f64_retain(value)?.round_dp(scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    fn row(price: Option<f64>, change: Option<f64>) -> MarketRow {
        MarketRow {
            id: "bitcoin".into(),
            symbol: "btc".into(),
            name: "Bitcoin".into(),
            current_price: price,
            price_change_percentage_1h_in_currency: Some(0.5),
            price_change_percentage_24h: change,
            price_change_percentage_7d_in_currency: Some(-1.257),
            market_cap: Some(6_512_345_678_901.0),
            market_cap_rank: Some(1),
            total_volume: Some(98_765_432.0),
            high_24h: Some(330_000.0),
            low_24h: Some(320_000.0),
            ath: Some(400_000.0),
            ath_change_percentage: Some(-18.5),
            circulating_supply: Some(19_800_000.0),
            sparkline_in_7d: Some(Series {
                price: (0..168).map(|i| 100.0 + i as f64).collect(),
            }),
        }
    }

    #[test]
    fn coin_normaliza_simbolo_e_trava_a_escala() {
        let coin = row(Some(325_611.123_456_789_9), Some(-2.5))
            .into_coin()
            .expect("coin");

        assert_eq!(coin.id, "bitcoin");
        assert_eq!(coin.symbol, "BTC");
        assert!(coin.price.scale() <= MONEY_SCALE);
        assert_eq!(coin.change_24h, dec!(-2.50));
        // As três janelas de variação chegam juntas na mesma resposta.
        assert_eq!(coin.change_1h, dec!(0.50));
        assert_eq!(coin.change_7d, dec!(-1.26));
        // Agregados ficam em duas casas: são informativos, nunca somados.
        assert_eq!(coin.market_cap, dec!(6512345678901.00));
        assert_eq!(coin.volume_24h, dec!(98765432.00));
        assert_eq!(coin.circulating_supply, dec!(19800000.00));
        assert_eq!(coin.ath_change_pct, dec!(-18.50));
    }

    #[test]
    fn variacao_ausente_vira_zero_e_a_moeda_permanece() {
        let coin = row(Some(10.0), None).into_coin().expect("coin");

        assert_eq!(coin.change_24h, Decimal::ZERO);
    }

    #[test]
    fn moeda_sem_preco_ou_com_preco_invalido_e_descartada() {
        assert!(row(None, Some(1.0)).into_coin().is_none());
        assert!(row(Some(f64::NAN), Some(1.0)).into_coin().is_none());
        assert!(row(Some(f64::INFINITY), None).into_coin().is_none());
    }

    #[test]
    fn medidor_da_faixa_de_24h_fica_dentro_do_viewbox() {
        let mut coin = row(Some(325_000.0), Some(1.0)).into_coin().expect("coin");

        // 325.000 entre 320.000 e 330.000: metade da faixa, meio do medidor.
        assert_eq!(coin.trading_range_x().as_deref(), Some("300.0"));

        // Preço fora da faixa (a fonte publica os três campos em momentos
        // diferentes) não pode empurrar o marcador para fora do medidor — nos
        // extremos ele para na folga que o mantém inteiro.
        coin.price = dec!(400000);
        assert_eq!(coin.trading_range_x().as_deref(), Some("590.0"));
        coin.price = dec!(1);
        assert_eq!(coin.trading_range_x().as_deref(), Some("10.0"));

        // Sem faixa publicada não há medidor — melhor omitir que desenhar zero.
        coin.low_24h = Decimal::ZERO;
        assert!(coin.trading_range_x().is_none());
    }

    #[test]
    fn janela_de_24h_e_o_rabo_da_serie_semanal() {
        let series: Vec<f64> = (0..168).map(|i| i as f64).collect();

        assert_eq!(Range::Week.window(&series).len(), 168);
        // 25 amostras horárias fecham 24 intervalos.
        assert_eq!(Range::Day.window(&series), &series[143..]);

        // Série mais curta que a janela não estoura o slice.
        let curta = [1.0, 2.0];
        assert_eq!(Range::Day.window(&curta), &curta);

        assert_eq!(Range::from_tag("24h"), Some(Range::Day));
        assert_eq!(Range::from_tag("7d"), Some(Range::Week));
        assert_eq!(Range::from_tag("30d"), None);
        assert_eq!(Range::default().tag(), "7d");
    }

    #[test]
    fn grafico_projeta_a_serie_no_viewbox_com_eixo_do_tempo() {
        let coin = row(Some(200.0), Some(1.0)).into_coin().expect("coin");
        let chart = coin
            .chart(Range::Day, Some(datetime!(2026-07-28 15:00 UTC)))
            .expect("chart");

        // A janela de 24 h abre em 143 e fecha em 267 (série 100..268).
        assert_eq!(chart.open, dec!(243));
        assert_eq!(chart.last, dec!(267));
        assert_eq!(chart.min, dec!(243));
        assert_eq!(chart.max, dec!(267));
        // Série ascendente: primeiro ponto na base, último no topo.
        assert!(chart.line.starts_with("M10.00 164.00"));
        assert!(chart.line.ends_with("L630.00 12.00"));
        assert_eq!(
            (chart.last_x.as_str(), chart.last_y.as_str()),
            ("630.00", "12.00")
        );
        // O preenchimento fecha contra a base da área de plotagem.
        assert!(chart.area.ends_with("L630.00 164.00 L10.00 164.00 Z"));
        assert!(chart.is_up() && !chart.is_down());

        // Eixo do tempo: cinco marcas equidistantes. A janela de um dia é
        // rotulada por hora e termina no instante da coleta.
        assert_eq!(chart.ticks, ["15:00", "21:00", "03:00", "09:00", "15:00"]);
        assert_eq!(chart.ticks.len(), AXIS_TICKS);
        assert_eq!(chart.grid.len(), GRID_LINES);
    }

    #[test]
    fn janela_semanal_e_rotulada_por_data() {
        let coin = row(Some(200.0), Some(1.0)).into_coin().expect("coin");
        let chart = coin
            .chart(Range::Week, Some(datetime!(2026-07-28 15:00 UTC)))
            .expect("chart");

        // 168 amostras horárias: sete dias para trás, rotulados por dia/mês.
        assert_eq!(chart.ticks, ["21/07", "23/07", "25/07", "26/07", "28/07"]);
    }

    #[test]
    fn serie_constante_vira_linha_reta_e_serie_curta_nao_vira_grafico() {
        let flat = project(&[50.0, 50.0], Some(datetime!(2026-07-28 15:00 UTC))).expect("chart");
        assert_eq!(flat.line, "M10.00 88.00 L630.00 88.00");
        assert_eq!(flat.delta_pct, Some(dec!(0)));
        assert!(!flat.is_up() && !flat.is_down());

        // Um ponto não é uma linha; NaN da fonte também não vira desenho.
        assert!(project(&[50.0], None).is_none());
        assert!(project(&[], None).is_none());
        assert!(project(&[1.0, f64::NAN], None).is_none());
    }

    #[test]
    fn grafico_sem_horario_de_coleta_sai_sem_eixo() {
        let chart = project(&[1.0, 2.0], None).expect("chart");
        assert!(chart.ticks.is_empty(), "rótulo de tempo exige a âncora");
        assert!(!chart.line.is_empty(), "a linha continua desenhada");
    }

    #[test]
    fn selecao_cai_na_primeira_do_ranking_quando_o_id_nao_existe() {
        let coins = vec![
            row(Some(1.0), None).into_coin().expect("coin"),
            Coin {
                id: "ethereum".into(),
                ..row(Some(2.0), None).into_coin().expect("coin")
            },
        ];
        let snapshot = Snapshot {
            coins: Arc::new(coins),
            updated_at: None,
            refresh_failed: false,
        };

        assert_eq!(
            snapshot.select(Some("ethereum")).expect("coin").id,
            "ethereum"
        );
        assert_eq!(
            snapshot.select(Some("nao-existe")).expect("coin").id,
            "bitcoin"
        );
        assert_eq!(snapshot.select(None).expect("coin").id, "bitcoin");
        assert!(Snapshot::default().select(Some("bitcoin")).is_none());
    }

    #[test]
    fn busca_casa_ticker_e_nome() {
        let coin = row(Some(1.0), None).into_coin().expect("coin");

        assert!(coin.matches("btc"));
        assert!(coin.matches("bitcoin"));
        assert!(coin.matches("coin"));
        assert!(!coin.matches("ethereum"));
    }
}
