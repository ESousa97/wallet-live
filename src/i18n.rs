//! Internacionalização dos templates.
//!
//! Em vez de um catálogo dinâmico (chave-string → tradução, resolvido em
//! runtime), cada idioma é uma instância `const` de um MESMO struct `Strings`:
//! esquecer um texto num idioma novo é erro de compilação, e um template que
//! referencia um campo inexistente também não compila (o askama checa os
//! campos). Zero alocação, zero busca em runtime — o mesmo espírito do resto
//! do projeto (SQL e templates checados em compilação).
//!
//! O idioma NÃO muda formato de dinheiro nem de data: os valores são BRL e as
//! convenções de moeda/planilha (R$ 1.234,56, CSV com `;`) são do DADO, não da
//! interface — como num extrato bancário exibido em inglês.

use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::header::ACCEPT_LANGUAGE;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

/// Cookie que fixa a escolha explícita de idioma (rota `/lang/{code}`).
pub const LANG_COOKIE: &str = "lang";

/// Idiomas suportados pela interface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Locale {
    PtBr,
    En,
}

impl Locale {
    /// Reconhece uma tag de idioma ("pt", "pt-BR", "en-US", ...). Só o idioma
    /// primário importa — qualquer região de `pt`/`en` cai no mesmo catálogo.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag.trim().split(['-', '_']).next()?;
        match primary.to_ascii_lowercase().as_str() {
            "pt" => Some(Self::PtBr),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    /// O catálogo de textos deste idioma.
    pub fn strings(self) -> &'static Strings {
        match self {
            Self::PtBr => &PT_BR,
            Self::En => &EN,
        }
    }

    /// Valor canônico gravado no cookie (e aceito de volta por `from_tag`).
    pub fn tag(self) -> &'static str {
        match self {
            Self::PtBr => "pt-BR",
            Self::En => "en",
        }
    }
}

/// Resolve o idioma da requisição: escolha explícita (cookie) > preferência do
/// navegador (`Accept-Language`) > pt-BR. É a ordem clássica: o que o usuário
/// pediu no produto vence o que o navegador diz, que vence o padrão do site.
fn resolve(lang_cookie: Option<&str>, accept_language: Option<&str>) -> Locale {
    lang_cookie
        .and_then(Locale::from_tag)
        .or_else(|| accept_language.and_then(preferred_from_accept_language))
        .unwrap_or(Locale::PtBr)
}

/// Primeiro idioma suportado na lista do `Accept-Language`. Os navegadores já
/// enviam a lista em ordem de preferência, então a ordem de aparição basta —
/// não pesamos os `;q=` explicitamente.
fn preferred_from_accept_language(header: &str) -> Option<Locale> {
    header
        .split(',')
        .find_map(|part| Locale::from_tag(part.split(';').next()?))
}

/// Extrator: qualquer handler que declare `Locale` recebe o idioma já
/// resolvido. Nunca falha — na pior hipótese cai no padrão pt-BR.
impl<S: Send + Sync> FromRequestParts<S> for Locale {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        let jar = CookieJar::from_headers(&parts.headers);
        Ok(resolve(
            jar.get(LANG_COOKIE).map(|cookie| cookie.value()),
            parts
                .headers
                .get(ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok()),
        ))
    }
}

/// Cookie de idioma: um ano de validade (escolha de preferência, não sessão),
/// mesmos atributos de segurança dos demais cookies do serviço.
pub fn lang_cookie(locale: Locale, cookie_secure: bool) -> Cookie<'static> {
    Cookie::build((LANG_COOKIE, locale.tag()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(cookie_secure)
        .path("/")
        .max_age(time::Duration::days(365))
        .build()
}

/// Todos os textos visíveis da interface. Um campo por texto; os templates
/// referenciam `t.<campo>` e o askama garante em compilação que o campo existe.
pub struct Strings {
    /// Tag BCP 47 usada no `<html lang>` e para marcar o idioma ativo.
    pub lang: &'static str,
    pub meta_description: &'static str,
    pub title_login: &'static str,
    pub title_wallet: &'static str,
    pub lang_label: &'static str,

    // Tela de login/cadastro.
    pub auth_badge: &'static str,
    pub intro_login: &'static str,
    pub intro_register: &'static str,
    pub password_requirements: &'static str,
    pub username: &'static str,
    pub password: &'static str,
    pub sign_in: &'static str,
    pub create_account: &'static str,
    pub have_account: &'static str,
    pub new_here: &'static str,

    // Cabeçalho da carteira.
    pub skip_link: &'static str,
    pub welcome: &'static str,
    pub nav_deposit: &'static str,
    pub nav_buy: &'static str,
    pub nav_sell: &'static str,
    pub nav_sync_quotes: &'static str,
    pub nav_market: &'static str,
    pub nav_logout: &'static str,

    // Tela de mercado.
    pub title_market: &'static str,
    pub market_heading: &'static str,
    pub market_subtitle: &'static str,
    pub market_updated_at: &'static str,
    pub market_loading: &'static str,
    pub market_unavailable: &'static str,
    pub market_stale: &'static str,
    pub market_note: &'static str,
    pub market_back: &'static str,

    // Painel da moeda selecionada.
    pub market_rank: &'static str,
    pub market_change_1h: &'static str,
    pub market_change_24h: &'static str,
    pub market_change_7d: &'static str,
    pub market_market_cap: &'static str,
    pub market_volume_24h: &'static str,
    pub market_trading_range: &'static str,
    pub market_low_24h: &'static str,
    pub market_high_24h: &'static str,
    pub market_ath: &'static str,
    pub market_from_ath: &'static str,
    pub market_supply: &'static str,
    pub market_open: &'static str,

    // Gráfico temporal da cotação.
    pub market_chart_title: &'static str,
    pub market_chart_aria: &'static str,
    pub market_chart_source: &'static str,
    pub market_chart_unavailable: &'static str,
    pub market_period: &'static str,
    pub market_period_day: &'static str,
    pub market_period_week: &'static str,

    // Lista lateral de moedas.
    pub market_list_title: &'static str,
    pub market_list_hint: &'static str,
    pub market_search_label: &'static str,
    pub market_search_placeholder: &'static str,
    pub market_search_submit: &'static str,
    pub market_no_results: &'static str,

    /// Sufixos de escala dos agregados (capitalização, volume, oferta). O
    /// NÚMERO segue a convenção do dado (pt-BR, como o R$), mas a abreviação é
    /// palavra — e palavra se traduz.
    pub unit_thousand: &'static str,
    pub unit_million: &'static str,
    pub unit_billion: &'static str,
    pub unit_trillion: &'static str,

    // Resumo do patrimônio.
    pub sum_balance: &'static str,
    pub sum_holdings: &'static str,
    pub sum_net_worth: &'static str,
    pub sum_invested: &'static str,
    pub sum_result: &'static str,

    // Gráfico de evolução.
    pub chart_title: &'static str,
    pub chart_aria: &'static str,
    pub chart_min: &'static str,
    pub chart_latest: &'static str,
    pub chart_max: &'static str,
    pub chart_period: &'static str,

    // Formulários de operação.
    pub deposit_title: &'static str,
    pub deposit_amount_aria: &'static str,
    pub deposit_placeholder: &'static str,
    pub confirm: &'static str,
    pub buy_title: &'static str,
    pub buy_select_aria: &'static str,
    pub select_asset: &'static str,
    pub buy_qty_aria: &'static str,
    pub sell_title: &'static str,
    pub sell_select_aria: &'static str,
    pub select_position: &'static str,
    pub available: &'static str,
    pub sell_qty_aria: &'static str,
    pub quantity: &'static str,

    // Posições.
    pub positions: &'static str,
    pub empty_title: &'static str,
    pub empty_hint: &'static str,
    pub empty_unquoted_hint: &'static str,
    pub th_asset: &'static str,
    pub th_price: &'static str,
    pub th_avg: &'static str,
    pub th_value: &'static str,
    pub th_result: &'static str,

    // Extrato.
    pub tx_title: &'static str,
    pub export_csv: &'static str,
    pub no_tx: &'static str,
    pub units_at: &'static str,
    pub pages_aria: &'static str,
    pub newer: &'static str,
    pub older: &'static str,
    pub page_label: &'static str,
    pub kind_deposit: &'static str,
    pub kind_buy: &'static str,
    pub kind_sell: &'static str,
    pub kind_other: &'static str,

    // Mensagens de feedback (flash).
    pub flash_deposit_done: &'static str,
    pub flash_buy_done: &'static str,
    pub flash_sell_done: &'static str,
    pub flash_quotes_done: &'static str,
    pub flash_invalid_amount: &'static str,
    pub flash_trade_too_small: &'static str,
    pub flash_insufficient_balance: &'static str,
    pub flash_insufficient_holdings: &'static str,
    pub flash_asset_missing: &'static str,
    pub flash_bad_credentials: &'static str,
    pub flash_username_taken: &'static str,
    pub flash_invalid_registration: &'static str,
    pub flash_too_many_attempts: &'static str,
    pub flash_csrf: &'static str,
    pub flash_quotes_unavailable: &'static str,
    pub flash_quotes_wait: &'static str,
}

impl Strings {
    /// Rótulo do tipo de uma transação do extrato (valores da coluna
    /// `transactions.kind`). O CSV não passa por aqui: exportação segue a
    /// convenção documentada de planilha pt-BR.
    pub fn tx_kind(&self, kind: &str) -> &'static str {
        match kind {
            "deposit" => self.kind_deposit,
            "buy" => self.kind_buy,
            "sell" => self.kind_sell,
            _ => self.kind_other,
        }
    }
}

pub static PT_BR: Strings = Strings {
    lang: "pt-BR",
    meta_description: "carteira de investimentos — acompanhe ativos, registre compras, veja o lucro.",
    title_login: "wallet :: entrar",
    title_wallet: "wallet :: carteira",
    lang_label: "idioma",

    auth_badge: "simulação educacional · 100% Rust",
    intro_login: "entre para acessar saldo, posições e histórico.",
    intro_register: "crie sua conta para simular uma carteira de investimentos.",
    password_requirements: "use pelo menos 8 caracteres.",
    username: "usuário",
    password: "senha",
    sign_in: "entrar",
    create_account: "criar conta",
    have_account: "já tem conta?",
    new_here: "novo por aqui?",

    skip_link: "pular para o conteúdo",
    welcome: "bem-vindo,",
    nav_deposit: "depositar",
    nav_buy: "comprar",
    nav_sell: "vender",
    nav_sync_quotes: "atualizar cotações",
    nav_market: "mercado",
    nav_logout: "sair",

    title_market: "wallet :: mercado",
    market_heading: "mercado",
    market_subtitle: "as 100 maiores criptomoedas por capitalização, cotadas em reais",
    market_updated_at: "atualizado em",
    market_loading: "buscando as cotações…",
    market_unavailable: "mercado indisponível no momento — tentaremos novamente em breve.",
    market_stale: "a última atualização falhou; exibindo o snapshot anterior.",
    market_note: "dados informativos da CoinGecko; suas operações usam as cotações da carteira.",
    market_back: "voltar para a carteira",

    market_rank: "ranking por capitalização",
    market_change_1h: "variação (1 h)",
    market_change_24h: "variação (24 h)",
    market_change_7d: "variação (7 d)",
    market_market_cap: "capitalização de mercado",
    market_volume_24h: "volume negociado (24 h)",
    market_trading_range: "faixa de negociação (24 h)",
    market_low_24h: "mínima (24 h)",
    market_high_24h: "máxima (24 h)",
    market_ath: "máxima histórica",
    market_from_ath: "desde a máxima histórica",
    market_supply: "oferta em circulação",
    market_open: "abertura do período",

    market_chart_title: "histórico de cotação",
    market_chart_aria: "gráfico temporal da cotação da moeda selecionada no período escolhido",
    market_chart_source: "série horária em BRL",
    market_chart_unavailable: "sem série temporal para esta moeda.",
    market_period: "período do gráfico",
    market_period_day: "24 h",
    market_period_week: "7 d",

    market_list_title: "todas as moedas",
    market_list_hint: "selecione uma moeda para abrir o painel",
    market_search_label: "buscar moeda",
    market_search_placeholder: "nome ou ticker",
    market_search_submit: "buscar",
    market_no_results: "nenhuma moeda encontrada.",

    unit_thousand: "mil",
    unit_million: "mi",
    unit_billion: "bi",
    unit_trillion: "tri",

    sum_balance: "saldo",
    sum_holdings: "ativos",
    sum_net_worth: "patrimônio",
    sum_invested: "investido",
    sum_result: "resultado dos ativos",

    chart_title: "evolução do patrimônio",
    chart_aria: "gráfico da evolução do patrimônio ao longo das últimas atualizações de cotação",
    chart_min: "mín",
    chart_latest: "último",
    chart_max: "máx",
    chart_period: "no período",

    deposit_title: "depositar saldo",
    deposit_amount_aria: "valor do depósito em reais",
    deposit_placeholder: "valor em R$",
    confirm: "confirmar",
    buy_title: "comprar ativo",
    buy_select_aria: "ativo para comprar",
    select_asset: "selecione um ativo",
    buy_qty_aria: "quantidade a comprar",
    sell_title: "vender ativo",
    sell_select_aria: "posição para vender",
    select_position: "selecione uma posição",
    available: "disponível:",
    sell_qty_aria: "quantidade a vender",
    quantity: "quantidade",

    positions: "posições",
    empty_title: "nenhum ativo ainda.",
    empty_hint: "deposite saldo e compre seu primeiro ativo.",
    empty_unquoted_hint: "as cotações ainda não chegaram — atualize para liberar o catálogo.",
    th_asset: "ativo",
    th_price: "preço",
    th_avg: "médio",
    th_value: "valor",
    th_result: "resultado",

    tx_title: "últimas transações",
    export_csv: "exportar csv",
    no_tx: "sem movimentações.",
    units_at: "un. @",
    pages_aria: "páginas do extrato",
    newer: "‹ mais recentes",
    older: "mais antigas ›",
    page_label: "página",
    kind_deposit: "deposito",
    kind_buy: "compra",
    kind_sell: "venda",
    kind_other: "movimentacao",

    flash_deposit_done: "depósito realizado.",
    flash_buy_done: "compra realizada.",
    flash_sell_done: "venda realizada.",
    flash_quotes_done: "cotações atualizadas.",
    flash_invalid_amount: "quantia inválida — informe um valor positivo.",
    flash_trade_too_small: "operação muito pequena — o total precisa ser de pelo menos R$ 0,00000001.",
    flash_insufficient_balance: "saldo insuficiente para esta compra.",
    flash_insufficient_holdings: "posição insuficiente para esta venda.",
    flash_asset_missing: "ativo inexistente.",
    flash_bad_credentials: "usuário ou senha incorretos.",
    flash_username_taken: "este nome de usuário já está em uso.",
    flash_invalid_registration: "use um nome de 3 a 32 caracteres e uma senha de 8 a 128 caracteres.",
    flash_too_many_attempts: "muitas tentativas — aguarde um instante e tente novamente.",
    flash_csrf: "a sessão do formulário expirou — tente novamente.",
    flash_quotes_unavailable: "cotações indisponíveis no momento — tente mais tarde.",
    flash_quotes_wait: "as cotações acabaram de ser atualizadas — aguarde alguns segundos.",
};

pub static EN: Strings = Strings {
    lang: "en",
    meta_description: "investment wallet — track assets, record purchases, watch your returns.",
    title_login: "wallet :: sign in",
    title_wallet: "wallet :: portfolio",
    lang_label: "language",

    auth_badge: "educational simulation · 100% Rust",
    intro_login: "sign in to see your balance, positions and history.",
    intro_register: "create your account to simulate an investment portfolio.",
    password_requirements: "use at least 8 characters.",
    username: "username",
    password: "password",
    sign_in: "sign in",
    create_account: "create account",
    have_account: "already have an account?",
    new_here: "new here?",

    skip_link: "skip to content",
    welcome: "welcome,",
    nav_deposit: "deposit",
    nav_buy: "buy",
    nav_sell: "sell",
    nav_sync_quotes: "refresh quotes",
    nav_market: "market",
    nav_logout: "sign out",

    title_market: "wallet :: market",
    market_heading: "market",
    market_subtitle: "the 100 largest cryptocurrencies by market cap, quoted in reais",
    market_updated_at: "updated at",
    market_loading: "fetching quotes…",
    market_unavailable: "market data is unavailable right now — we will retry shortly.",
    market_stale: "the latest refresh failed; showing the previous snapshot.",
    market_note: "informational CoinGecko data; your trades use the wallet catalog quotes.",
    market_back: "back to the wallet",

    market_rank: "market cap rank",
    market_change_1h: "change (1h)",
    market_change_24h: "change (24h)",
    market_change_7d: "change (7d)",
    market_market_cap: "market cap",
    market_volume_24h: "traded volume (24h)",
    market_trading_range: "trading range (24h)",
    market_low_24h: "low (24h)",
    market_high_24h: "high (24h)",
    market_ath: "all-time high",
    market_from_ath: "from all-time high",
    market_supply: "circulating supply",
    market_open: "period open",

    market_chart_title: "price history",
    market_chart_aria: "time series chart of the selected coin over the chosen period",
    market_chart_source: "hourly series in BRL",
    market_chart_unavailable: "no time series for this coin.",
    market_period: "chart period",
    market_period_day: "24h",
    market_period_week: "7d",

    market_list_title: "all coins",
    market_list_hint: "pick a coin to load the dashboard",
    market_search_label: "search coin",
    market_search_placeholder: "name or ticker",
    market_search_submit: "search",
    market_no_results: "no coin matches your search.",

    unit_thousand: "K",
    unit_million: "M",
    unit_billion: "B",
    unit_trillion: "T",

    sum_balance: "cash",
    sum_holdings: "holdings",
    sum_net_worth: "net worth",
    sum_invested: "invested",
    sum_result: "holdings result",

    chart_title: "net worth over time",
    chart_aria: "chart of net worth across the latest quote updates",
    chart_min: "min",
    chart_latest: "latest",
    chart_max: "max",
    chart_period: "over the period",

    deposit_title: "deposit funds",
    deposit_amount_aria: "deposit amount in reais",
    deposit_placeholder: "amount in R$",
    confirm: "confirm",
    buy_title: "buy asset",
    buy_select_aria: "asset to buy",
    select_asset: "select an asset",
    buy_qty_aria: "quantity to buy",
    sell_title: "sell asset",
    sell_select_aria: "position to sell",
    select_position: "select a position",
    available: "available:",
    sell_qty_aria: "quantity to sell",
    quantity: "quantity",

    positions: "positions",
    empty_title: "no assets yet.",
    empty_hint: "deposit funds and buy your first asset.",
    empty_unquoted_hint: "quotes have not arrived yet — refresh to unlock the catalog.",
    th_asset: "asset",
    th_price: "price",
    th_avg: "avg cost",
    th_value: "value",
    th_result: "result",

    tx_title: "latest transactions",
    export_csv: "export csv",
    no_tx: "no activity.",
    units_at: "units @",
    pages_aria: "statement pages",
    newer: "‹ newer",
    older: "older ›",
    page_label: "page",
    kind_deposit: "deposit",
    kind_buy: "buy",
    kind_sell: "sell",
    kind_other: "transaction",

    flash_deposit_done: "deposit completed.",
    flash_buy_done: "purchase completed.",
    flash_sell_done: "sale completed.",
    flash_quotes_done: "quotes refreshed.",
    flash_invalid_amount: "invalid amount — enter a positive value.",
    flash_trade_too_small: "trade is too small — the total must be at least R$ 0.00000001.",
    flash_insufficient_balance: "insufficient balance for this purchase.",
    flash_insufficient_holdings: "insufficient position for this sale.",
    flash_asset_missing: "asset does not exist.",
    flash_bad_credentials: "incorrect username or password.",
    flash_username_taken: "this username is already taken.",
    flash_invalid_registration: "use a 3–32 character username and an 8–128 character password.",
    flash_too_many_attempts: "too many attempts — wait a moment and try again.",
    flash_csrf: "the form session expired — try again.",
    flash_quotes_unavailable: "quotes unavailable right now — try again later.",
    flash_quotes_wait: "quotes were just refreshed — wait a few seconds.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_resolve_ignoring_case_and_region() {
        assert_eq!(Locale::from_tag("pt"), Some(Locale::PtBr));
        assert_eq!(Locale::from_tag("pt-BR"), Some(Locale::PtBr));
        assert_eq!(Locale::from_tag("PT_br"), Some(Locale::PtBr));
        assert_eq!(Locale::from_tag("en-US"), Some(Locale::En));
        assert_eq!(Locale::from_tag(" en "), Some(Locale::En));
        assert_eq!(Locale::from_tag("es"), None);
        assert_eq!(Locale::from_tag(""), None);
    }

    #[test]
    fn accept_language_takes_the_first_supported_entry() {
        assert_eq!(
            preferred_from_accept_language("en-US,en;q=0.9,pt;q=0.8"),
            Some(Locale::En)
        );
        // Idioma não suportado na frente não atrapalha: pega o próximo.
        assert_eq!(
            preferred_from_accept_language("fr-FR,fr;q=0.9,pt-BR;q=0.8"),
            Some(Locale::PtBr)
        );
        assert_eq!(preferred_from_accept_language("fr,es"), None);
    }

    #[test]
    fn explicit_cookie_beats_browser_preference_and_default_is_ptbr() {
        assert_eq!(resolve(Some("en"), Some("pt-BR,pt;q=0.9")), Locale::En);
        assert_eq!(resolve(None, Some("en-US,en;q=0.9")), Locale::En);
        assert_eq!(resolve(Some("lixo"), Some("fr")), Locale::PtBr);
        assert_eq!(resolve(None, None), Locale::PtBr);
    }

    #[test]
    fn transaction_kinds_localize_with_a_fallback() {
        assert_eq!(PT_BR.tx_kind("deposit"), "deposito");
        assert_eq!(EN.tx_kind("deposit"), "deposit");
        assert_eq!(EN.tx_kind("???"), "transaction");
    }
}
