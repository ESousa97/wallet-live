use std::convert::Infallible;

use askama::Template;
use axum::Router;
use axum::extract::{Form, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::csrf::{ensure_csrf_token, verify_csrf};
use crate::auth::session::{
    REFRESH_COOKIE, RefreshToken, access_cookie, hash_token, refresh_cookie, session_expiry,
};
use crate::auth::user::{TOKEN_COOKIE, UnauthenticatedUser, User};
use crate::config::Config;
use crate::error::AppError;
use crate::i18n::{Locale, Strings, lang_cookie};
use crate::models::{Asset, Holding, Transaction, WalletSummary};
use crate::quotes::sync_quotes_round;
use crate::repository::Repository;
use crate::routes::flash::{Flash, business_flash, set_flash, take_flash};
use crate::services::portfolio::{EquityChart, PortfolioService, WalletView};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/static/app.css", get(app_css))
        .route("/static/htmx.js", get(htmx_js))
        .route("/login", get(login_page).post(login))
        .route("/register", get(register_page).post(register))
        .route("/logout", get(logout))
        .route("/assets", get(assets_page))
        .route("/transactions.csv", get(transactions_csv))
        .route("/deposit", get(deposit_page).post(deposit))
        .route("/buy", get(buy_page).post(buy_asset))
        .route("/sell", get(sell_page).post(sell_asset))
        .route("/quotes/sync", get(assets_page).post(sync_quotes))
        .route("/lang/{code}", get(set_language))
}

#[derive(Deserialize)]
struct LangQuery {
    next: Option<String>,
}

/// Troca o idioma da interface: grava o cookie `lang` e volta para a página de
/// origem (`?next=`). Código desconhecido não grava nada — só redireciona.
#[instrument(skip_all)]
async fn set_language(
    State(state): State<AppState>,
    Path(code): Path<String>,
    jar: CookieJar,
    Query(query): Query<LangQuery>,
) -> (CookieJar, Redirect) {
    let jar = match Locale::from_tag(&code) {
        Some(locale) => jar.add(lang_cookie(locale, state.config.cookie_secure)),
        None => jar,
    };

    (jar, Redirect::to(sanitized_next(query.next.as_deref())))
}

/// Valida o destino do retorno pós-troca de idioma: só caminhos locais
/// absolutos ("/algo"). Nada de "//host" (URL relativa a protocolo) nem URLs
/// completas — um `next` vindo da query string nunca vira open redirect.
fn sanitized_next(next: Option<&str>) -> &str {
    match next {
        Some(path) if path.starts_with('/') && !path.starts_with("//") => path,
        _ => "/",
    }
}

/// CSS da interface, servido do próprio binário (`include_str!` embute o
/// arquivo em tempo de compilação, como o askama faz com os templates): nada de
/// CDN de terceiros — sem dependência externa em runtime, sem telemetria
/// alheia, e a CSP trava `style-src` em `'self'`.
///
/// É CSS **pré-compilado** (ver `styles/app.css`), não o Play CDN do Tailwind:
/// aquele era um compilador rodando no navegador, que injetava `<style>` em
/// runtime e por isso exigia `'unsafe-inline'` na política.
#[instrument(skip_all)]
async fn app_css() -> impl axum::response::IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            // Um dia de cache: o arquivo só muda quando o binário muda.
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        include_str!("../../static/app.css"),
    )
}

/// htmx servido do binário, pelo mesmo motivo do Tailwind acima: zero CDN e a
/// CSP continua com `script-src 'self'`. É ele que transforma os links e
/// formulários da carteira (atributos `hx-*` nos templates) em trocas parciais
/// de HTML — o servidor segue renderizando tudo (SSR); só muda o quanto de
/// página viaja e é re-desenhado por operação.
#[instrument(skip_all)]
async fn htmx_js() -> impl axum::response::IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        include_str!("../../static/htmx.js"),
    )
}

/// `true` quando a resposta deve ser o fragmento parcial da carteira, não a
/// página inteira: o htmx marca suas requisições com `HX-Request: true`. A
/// exceção é a restauração de histórico (voltar/avançar com o cache local
/// expirado), que vem com `HX-History-Restore-Request` e espera a página
/// COMPLETA para reconstruir o estado do zero.
fn is_partial_request(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .is_some_and(|value| value == "true")
        && !headers.contains_key("hx-history-restore-request")
}

/// Extrator do sinal acima. Sem JavaScript (ou sem o header), tudo cai no
/// caminho clássico de página cheia — htmx aqui é *progressive enhancement*.
struct HxRequest(bool);

impl<S: Send + Sync> FromRequestParts<S> for HxRequest {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        Ok(Self(is_partial_request(&parts.headers)))
    }
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    is_register: bool,
    csrf_token: String,
    flash: Option<Flash>,
    t: &'static Strings,
}

impl LoginPage {
    /// Caminho desta tela (login ou cadastro): destino do formulário e retorno
    /// (`?next=`) da troca de idioma.
    fn form_path(&self) -> &'static str {
        if self.is_register {
            "/register"
        } else {
            "/login"
        }
    }
}

/// Toda página com formulário garante um token CSRF na jar e o embute num campo
/// oculto; os POSTs correspondentes conferem os dois (ver `auth::csrf`). O
/// flash (se houver) é consumido aqui e vira o banner de feedback.
#[instrument(skip_all)]
async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
    locale: Locale,
) -> Result<(CookieJar, Html<String>), AppError> {
    let (jar, flash) = take_flash(jar);
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);
    let page = LoginPage {
        is_register: false,
        csrf_token,
        flash,
        t: locale.strings(),
    };
    Ok((jar, Html(page.render()?)))
}

#[instrument(skip_all)]
async fn register_page(
    State(state): State<AppState>,
    jar: CookieJar,
    locale: Locale,
) -> Result<(CookieJar, Html<String>), AppError> {
    let (jar, flash) = take_flash(jar);
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);
    let page = LoginPage {
        is_register: true,
        csrf_token,
        flash,
        t: locale.strings(),
    };
    Ok((jar, Html(page.render()?)))
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
    csrf_token: String,
}

#[instrument(skip_all)]
async fn login(
    State(state): State<AppState>,
    repository: Repository,
    jar: CookieJar,
    locale: Locale,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    match authenticate_form(&state, &repository, &jar, form).await {
        Ok(user) => {
            let jar = start_session(jar, &user, &repository, &state.config).await?;
            Ok((jar, Redirect::to("/")))
        }
        // Erro de negócio vira banner na própria tela de login; erro interno
        // propaga (o `?` do business_flash) para o fluxo de 500.
        Err(error) => {
            let flash = business_flash(error, locale.strings())?;
            Ok((
                set_flash(jar, &flash, state.config.cookie_secure),
                Redirect::to("/login"),
            ))
        }
    }
}

/// O miolo do login: CSRF, lockout e conferência de credencial.
async fn authenticate_form(
    state: &AppState,
    repository: &Repository,
    jar: &CookieJar,
    form: LoginForm,
) -> Result<User, AppError> {
    verify_csrf(jar, &form.csrf_token)?;

    // Lockout ANTES de conferir a senha: durante o bloqueio nem a senha certa
    // passa, então força bruta não extrai sinal nenhum das tentativas.
    state.login_throttle.ensure_allowed(&form.username).await?;

    match UnauthenticatedUser::new(form.username.clone(), form.password)
        .authenticate(repository)
        .await
    {
        Ok(user) => {
            state.login_throttle.record_success(&form.username).await;
            Ok(user)
        }
        // Só falhas de credencial alimentam o contador — sondagem de username
        // e senha errada contam igual; erro de banco não.
        Err(error @ (AppError::InvalidCredentials | AppError::UserDoesNotExist)) => {
            state.login_throttle.record_failure(&form.username).await;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

#[instrument(skip_all)]
async fn register(
    State(state): State<AppState>,
    repository: Repository,
    jar: CookieJar,
    locale: Locale,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    let outcome = async {
        verify_csrf(&jar, &form.csrf_token)?;
        UnauthenticatedUser::new(form.username, form.password)
            .register(&repository)
            .await
    }
    .await;

    match outcome {
        Ok(user) => {
            let jar = start_session(jar, &user, &repository, &state.config).await?;
            Ok((jar, Redirect::to("/")))
        }
        Err(error) => {
            let flash = business_flash(error, locale.strings())?;
            Ok((
                set_flash(jar, &flash, state.config.cookie_secure),
                Redirect::to("/register"),
            ))
        }
    }
}

/// Exporta o extrato completo do usuário autenticado como download CSV.
#[instrument(skip_all)]
async fn transactions_csv(
    user: User,
    repository: Repository,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let transactions = repository.list_all_transactions(user.id()).await?;

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"extrato.csv\"",
            ),
        ],
        transactions_to_csv(&transactions),
    ))
}

/// Monta o CSV do extrato no padrão pt-BR de planilha: separador `;` e decimais
/// com vírgula (é o que o Excel/LibreOffice em português esperam). Campos de
/// texto vão entre aspas, com aspas internas dobradas (RFC 4180).
fn transactions_to_csv(transactions: &[Transaction]) -> String {
    let mut csv = String::from("data;tipo;ativo;quantidade;preco_unitario;movimento_caixa\n");

    for tx in transactions {
        let date = tx
            .created_at
            .format(
                &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]")
                    .expect("static format"),
            )
            .unwrap_or_default();
        let kind = match tx.kind.as_str() {
            "deposit" => "deposito",
            "buy" => "compra",
            "sell" => "venda",
            other => other,
        };
        let asset = tx.asset_name.as_deref().unwrap_or("-");
        let quantity = tx
            .quantity
            .map(|q| decimal_ptbr(&q))
            .unwrap_or_else(|| "-".to_string());
        let unit_value = tx
            .unit_value
            .map(|v| decimal_ptbr(&v))
            .unwrap_or_else(|| "-".to_string());

        csv.push_str(&format!(
            "{date};{kind};{};{quantity};{unit_value};{}\n",
            csv_field(asset),
            decimal_ptbr(&tx.cash_delta)
        ));
    }

    csv
}

/// Decimal com vírgula, sem zeros supérfluos.
fn decimal_ptbr(value: &Decimal) -> String {
    value.normalize().to_string().replace('.', ",")
}

/// Campo de texto CSV: aspas em volta, aspas internas dobradas.
fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Emite o par de cookies da sessão: o JWT de acesso (curto) e o refresh token
/// (longo), cujo registro vai para a tabela `sessions` — é ele que permite
/// renovar o acesso sem novo login e revogar a sessão no logout.
async fn start_session(
    jar: CookieJar,
    user: &User,
    repository: &Repository,
    config: &Config,
) -> Result<CookieJar, AppError> {
    let refresh = RefreshToken::generate();
    repository
        .create_session(user.id(), &refresh.hash(), session_expiry(config))
        .await?;

    Ok(jar
        .add(access_cookie(user, config)?)
        .add(refresh_cookie(&refresh, config)))
}

/// Logout com revogação REAL: além de remover os cookies do navegador, mata a
/// sessão no servidor — o refresh token para de funcionar em qualquer cópia.
#[instrument(skip_all)]
async fn logout(repository: Repository, jar: CookieJar) -> (CookieJar, Redirect) {
    if let Some(refresh) = jar.get(REFRESH_COOKIE) {
        // Falha ao revogar (banco fora etc.) não impede o logout local; a
        // sessão ainda expira sozinha pelo expires_at.
        let _ = repository
            .revoke_session(&hash_token(refresh.value()))
            .await;
    }

    (
        jar.remove(Cookie::build(TOKEN_COOKIE).path("/").build())
            .remove(Cookie::build(REFRESH_COOKIE).path("/").build()),
        Redirect::to("/login"),
    )
}

#[instrument(skip_all)]
async fn index(maybe_user: Option<User>) -> Redirect {
    match maybe_user {
        Some(_) => Redirect::to("/assets"),
        None => Redirect::to("/login"),
    }
}

/// Tudo o que o miolo da carteira (o `<div id="wallet">`) precisa para se
/// desenhar. É compartilhado pelos DOIS templates: a página completa
/// (`assets.html`, que inclui o fragmento) e o fragmento sozinho
/// (`wallet.html`), devolvido nas requisições parciais do htmx.
struct WalletData {
    holdings: Vec<Holding>,
    available_assets: Vec<Asset>,
    transactions: Vec<Transaction>,
    summary: WalletSummary,
    action: WalletAction,
    csrf_token: String,
    page: u32,
    has_prev: bool,
    has_next: bool,
    flash: Option<Flash>,
    chart: EquityChart,
    t: &'static Strings,
}

impl WalletData {
    // Qual formulário está aberto. Os templates usam isto para marcar o botão
    // correspondente como ativo (`aria-current`), já que o askama não avalia
    // `matches!` sobre o enum direto.
    fn is_deposit(&self) -> bool {
        matches!(self.action, WalletAction::Deposit)
    }

    fn is_buy(&self) -> bool {
        matches!(self.action, WalletAction::Buy)
    }

    fn is_sell(&self) -> bool {
        matches!(self.action, WalletAction::Sell)
    }

    fn new(
        view: WalletView,
        action: WalletAction,
        csrf_token: String,
        flash: Option<Flash>,
        locale: Locale,
    ) -> Self {
        Self {
            holdings: view.holdings,
            available_assets: view.available_assets,
            transactions: view.transactions,
            summary: view.summary,
            action,
            csrf_token,
            page: view.page,
            has_prev: view.has_prev,
            has_next: view.has_next,
            flash,
            chart: view.chart,
            t: locale.strings(),
        }
    }
}

#[derive(Template)]
#[template(path = "assets.html")]
struct AssetsPage {
    user: User,
    wallet: WalletData,
    t: &'static Strings,
}

#[derive(Template)]
#[template(path = "wallet.html")]
struct WalletFragment {
    wallet: WalletData,
}

enum WalletAction {
    None,
    Deposit,
    Buy,
    Sell,
}

#[derive(Deserialize)]
struct PageQuery {
    page: Option<u32>,
}

#[instrument(skip_all)]
async fn assets_page(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(RenderWallet {
        state,
        user,
        portfolio,
        jar,
        hx,
        locale,
        action: WalletAction::None,
        page: query.page,
    })
    .await
}

#[instrument(skip_all)]
async fn deposit_page(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(RenderWallet {
        state,
        user,
        portfolio,
        jar,
        hx,
        locale,
        action: WalletAction::Deposit,
        page: query.page,
    })
    .await
}

#[instrument(skip_all)]
async fn buy_page(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(RenderWallet {
        state,
        user,
        portfolio,
        jar,
        hx,
        locale,
        action: WalletAction::Buy,
        page: query.page,
    })
    .await
}

#[instrument(skip_all)]
async fn sell_page(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(RenderWallet {
        state,
        user,
        portfolio,
        jar,
        hx,
        locale,
        action: WalletAction::Sell,
        page: query.page,
    })
    .await
}

/// Tudo que uma renderização da carteira precisa (os handlers acima só variam
/// a `action` e o resto vem dos extratores).
struct RenderWallet {
    state: AppState,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    action: WalletAction,
    page: Option<u32>,
}

/// O handler só faz HTTP: garante o token CSRF, pede a visão pronta ao serviço
/// e renderiza o template. Toda a montagem (consultas, paginação) é do serviço.
/// Requisição parcial (htmx) recebe só o fragmento da carteira; navegação
/// normal (e restauração de histórico) recebe a página completa.
async fn render_wallet(request: RenderWallet) -> Result<(CookieJar, Html<String>), AppError> {
    let RenderWallet {
        state,
        user,
        portfolio,
        jar,
        hx,
        locale,
        action,
        page,
    } = request;

    let (jar, flash) = take_flash(jar);
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);

    let view = portfolio.wallet_view(user.id(), page.unwrap_or(1)).await?;
    let wallet = WalletData::new(view, action, csrf_token, flash, locale);

    let html = if hx.0 {
        WalletFragment { wallet }.render()?
    } else {
        AssetsPage {
            user,
            wallet,
            t: locale.strings(),
        }
        .render()?
    };
    Ok((jar, Html(html)))
}

#[derive(Deserialize)]
struct AmountForm {
    amount: Decimal,
    csrf_token: String,
}

/// Como apresentar o desfecho de uma operação da carteira: a mensagem de
/// sucesso e, no erro de negócio, para qual formulário voltar (caminho + qual
/// seção de formulário re-abrir), para o banner aparecer no lugar certo e o
/// `autofocus` reposicionar o foco.
struct OperationFeedback {
    on_success: Flash,
    error_path: &'static str,
    error_action: WalletAction,
    locale: Locale,
}

/// Converte o desfecho de uma operação da carteira na resposta certa para cada
/// modo. Sucesso vira a mensagem dada, erro de negócio vira banner, erro
/// interno propaga.
///
/// - Clássico (sem JavaScript): flash em cookie + redirect — o padrão PRG de
///   sempre, em duas requisições.
/// - Parcial (htmx): o fragmento da carteira já atualizado volta NA MESMA
///   resposta, com o flash inline (nada de cookie: a mensagem não deve
///   sobreviver a um F5) e `HX-Push-Url` para a barra de endereço acompanhar.
///   Uma requisição só, sem recarregar a página.
async fn wallet_outcome(
    state: &AppState,
    user: User,
    portfolio: &PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    outcome: Result<(), AppError>,
    feedback: OperationFeedback,
) -> Result<Response, AppError> {
    let (flash, path, action) = match outcome {
        Ok(()) => (feedback.on_success, "/assets", WalletAction::None),
        Err(error) => (
            business_flash(error, feedback.locale.strings())?,
            feedback.error_path,
            feedback.error_action,
        ),
    };

    if hx.0 {
        let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);
        let view = portfolio.wallet_view(user.id(), 1).await?;
        let fragment = WalletFragment {
            wallet: WalletData::new(view, action, csrf_token, Some(flash), feedback.locale),
        };
        Ok((jar, [("hx-push-url", path)], Html(fragment.render()?)).into_response())
    } else {
        Ok((
            set_flash(jar, &flash, state.config.cookie_secure),
            Redirect::to(path),
        )
            .into_response())
    }
}

#[instrument(skip_all)]
async fn deposit(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Form(form): Form<AmountForm>,
) -> Result<Response, AppError> {
    let outcome = async {
        verify_csrf(&jar, &form.csrf_token)?;
        portfolio.deposit(user.id(), form.amount).await
    }
    .await;

    wallet_outcome(
        &state,
        user,
        &portfolio,
        jar,
        hx,
        outcome,
        OperationFeedback {
            on_success: Flash::success(locale.strings().flash_deposit_done),
            error_path: "/deposit",
            error_action: WalletAction::Deposit,
            locale,
        },
    )
    .await
}

#[derive(Deserialize)]
struct TradeAssetForm {
    asset_id: i64,
    quantity: Decimal,
    csrf_token: String,
}

#[instrument(skip_all)]
async fn buy_asset(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Form(form): Form<TradeAssetForm>,
) -> Result<Response, AppError> {
    let outcome = async {
        verify_csrf(&jar, &form.csrf_token)?;
        portfolio.buy(user.id(), form.asset_id, form.quantity).await
    }
    .await;

    wallet_outcome(
        &state,
        user,
        &portfolio,
        jar,
        hx,
        outcome,
        OperationFeedback {
            on_success: Flash::success(locale.strings().flash_buy_done),
            error_path: "/buy",
            error_action: WalletAction::Buy,
            locale,
        },
    )
    .await
}

#[instrument(skip_all)]
async fn sell_asset(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Form(form): Form<TradeAssetForm>,
) -> Result<Response, AppError> {
    let outcome = async {
        verify_csrf(&jar, &form.csrf_token)?;
        portfolio
            .sell(user.id(), form.asset_id, form.quantity)
            .await
    }
    .await;

    wallet_outcome(
        &state,
        user,
        &portfolio,
        jar,
        hx,
        outcome,
        OperationFeedback {
            on_success: Flash::success(locale.strings().flash_sell_done),
            error_path: "/sell",
            error_action: WalletAction::Sell,
            locale,
        },
    )
    .await
}

/// O formulário de sincronizar cotações não tem campo de dado nenhum, mas ainda
/// é um POST que muda estado — então também carrega (e valida) o token CSRF.
#[derive(Deserialize)]
struct SyncQuotesForm {
    csrf_token: String,
}

#[instrument(skip_all)]
async fn sync_quotes(
    State(state): State<AppState>,
    user: User,
    portfolio: PortfolioService,
    jar: CookieJar,
    hx: HxRequest,
    locale: Locale,
    Form(form): Form<SyncQuotesForm>,
) -> Result<Response, AppError> {
    let outcome = async {
        verify_csrf(&jar, &form.csrf_token)?;
        sync_quotes_round(&Repository::from_state(&state))
            .await
            .map(|_| ())
    }
    .await;

    wallet_outcome(
        &state,
        user,
        &portfolio,
        jar,
        hx,
        outcome,
        OperationFeedback {
            on_success: Flash::success(locale.strings().flash_quotes_done),
            error_path: "/assets",
            error_action: WalletAction::None,
            locale,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    #[test]
    fn htmx_marks_partial_requests_but_history_restore_wants_the_full_page() {
        let mut headers = HeaderMap::new();
        assert!(!is_partial_request(&headers), "navegação normal");

        headers.insert("hx-request", "true".parse().unwrap());
        assert!(is_partial_request(&headers), "requisição do htmx");

        // Voltar/avançar com o cache de histórico expirado: o htmx refaz o GET
        // mas precisa da página inteira para reconstruir o DOM.
        headers.insert("hx-history-restore-request", "true".parse().unwrap());
        assert!(!is_partial_request(&headers), "restauração de histórico");
    }

    #[test]
    fn language_switch_only_follows_local_absolute_paths() {
        assert_eq!(sanitized_next(Some("/assets")), "/assets");
        assert_eq!(sanitized_next(Some("/login")), "/login");
        // Protocolo-relativo, URL absoluta e lixo caem no fallback seguro.
        assert_eq!(sanitized_next(Some("//evil.example")), "/");
        assert_eq!(sanitized_next(Some("https://evil.example")), "/");
        assert_eq!(sanitized_next(Some("assets")), "/");
        assert_eq!(sanitized_next(None), "/");
    }

    fn test_wallet(locale: Locale) -> WalletData {
        WalletData {
            holdings: vec![],
            available_assets: vec![],
            transactions: vec![],
            summary: WalletSummary {
                balance: dec!(10),
                holdings_value: Decimal::ZERO,
                total_value: dec!(10),
                total_invested: Decimal::ZERO,
                total_delta: Decimal::ZERO,
            },
            action: WalletAction::None,
            csrf_token: "tok".to_string(),
            page: 1,
            has_prev: false,
            has_next: false,
            t: locale.strings(),
            flash: Some(Flash::success("depósito realizado.")),
            chart: EquityChart::empty(),
        }
    }

    #[test]
    fn the_wallet_fragment_is_partial_html_embedded_by_the_full_page() {
        let fragment = WalletFragment {
            wallet: test_wallet(Locale::PtBr),
        }
        .render()
        .expect("fragment renders");

        // O fragmento é exatamente o alvo do swap (`outerHTML` de #wallet),
        // com o flash inline — e nada de esqueleto de página em volta.
        assert!(fragment.starts_with("<div id=\"wallet\">"));
        assert!(!fragment.contains("<!DOCTYPE"));
        assert!(fragment.contains("depósito realizado."));

        let full = AssetsPage {
            user: User::new(1, "breno".to_string(), "user".to_string()),
            wallet: test_wallet(Locale::PtBr),
            t: Locale::PtBr.strings(),
        }
        .render()
        .expect("full page renders");

        // A página completa embute o MESMO fragmento (id único do swap) dentro
        // do esqueleto, com o htmx carregado do próprio binário.
        assert!(full.contains("<!DOCTYPE html>"));
        assert!(full.contains("<div id=\"wallet\">"));
        assert!(full.contains("/static/htmx.js"));
        assert_eq!(full.matches("id=\"wallet\"").count(), 1);
    }

    /// A CSP fecha `script-src` e `style-src` em `'self'` (nada de
    /// `'unsafe-inline'`). Isso só se sustenta enquanto NENHUMA página emitir
    /// `<style>` ou `<script>` inline — um bloco desses passaria despercebido
    /// em revisão e o navegador simplesmente o ignoraria em produção, gerando
    /// um bug visual difícil de rastrear. O teste trava o invariante.
    #[test]
    fn pages_carry_no_inline_style_or_script() {
        let full = AssetsPage {
            user: User::new(1, "breno".to_string(), "user".to_string()),
            wallet: test_wallet(Locale::PtBr),
            t: Locale::PtBr.strings(),
        }
        .render()
        .expect("page renders");

        let login = LoginPage {
            is_register: false,
            csrf_token: "tok".to_string(),
            flash: None,
            t: Locale::PtBr.strings(),
        }
        .render()
        .expect("login renders");

        for (name, html) in [("assets", &full), ("login", &login)] {
            assert!(!html.contains("<style"), "{name}: <style> inline");
            // Todo `<script>` da página tem de ser externo (`src=`).
            for fragment in html.split("<script").skip(1) {
                let tag = fragment.split('>').next().unwrap_or_default();
                assert!(tag.contains("src="), "{name}: <script> sem src ({tag})");
            }
            assert!(html.contains("/static/app.css"), "{name}: css externo");
        }
    }

    #[test]
    fn the_wallet_page_renders_in_both_languages() {
        let render = |locale: Locale| {
            AssetsPage {
                user: User::new(1, "breno".to_string(), "user".to_string()),
                wallet: test_wallet(locale),
                t: locale.strings(),
            }
            .render()
            .expect("page renders")
        };

        let pt = render(Locale::PtBr);
        assert!(pt.contains("lang=\"pt-BR\""));
        assert!(pt.contains("posições"));
        assert!(pt.contains("patrimônio"));

        let en = render(Locale::En);
        assert!(en.contains("lang=\"en\""));
        assert!(en.contains("positions"));
        assert!(en.contains("net worth"));
        // O dinheiro continua na convenção do DADO (BRL), não da interface.
        assert!(en.contains("R$ 10,00"));
    }

    #[test]
    fn the_login_page_renders_in_both_languages() {
        let render = |locale: Locale, is_register: bool| {
            LoginPage {
                is_register,
                csrf_token: "tok".to_string(),
                flash: None,
                t: locale.strings(),
            }
            .render()
            .expect("login renders")
        };

        assert!(render(Locale::PtBr, false).contains("entre para acessar"));
        assert!(render(Locale::En, false).contains("sign in"));
        assert!(render(Locale::En, true).contains("create account"));
    }

    #[test]
    fn csv_export_formats_the_statement_in_ptbr_conventions() {
        let transactions = vec![
            Transaction {
                id: 2,
                kind: "buy".to_string(),
                asset_name: Some("bitcoin \"btc\"".to_string()),
                quantity: Some(dec!(0.50)),
                unit_value: Some(dec!(100.25)),
                cash_delta: dec!(-50.125),
                created_at: datetime!(2026-07-17 12:30 UTC),
            },
            Transaction {
                id: 1,
                kind: "deposit".to_string(),
                asset_name: None,
                quantity: None,
                unit_value: None,
                cash_delta: dec!(1000),
                created_at: datetime!(2026-07-17 12:00 UTC),
            },
        ];

        let csv = transactions_to_csv(&transactions);
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(
            lines[0],
            "data;tipo;ativo;quantidade;preco_unitario;movimento_caixa"
        );
        // Decimais com vírgula, aspas internas dobradas, tipo em pt-BR.
        assert_eq!(
            lines[1],
            "2026-07-17 12:30;compra;\"bitcoin \"\"btc\"\"\";0,5;100,25;-50,125"
        );
        // Depósito não tem ativo/quantidade/preço.
        assert_eq!(lines[2], "2026-07-17 12:00;deposito;\"-\";-;-;1000");
    }
}

pub mod filters {
    use askama::Values;
    use rust_decimal::Decimal;
    use time::OffsetDateTime;

    #[askama::filter_fn]
    pub fn human_datetime(value: &OffsetDateTime, _: &dyn Values) -> askama::Result<String> {
        let format = time::format_description::parse("[year]-[month]-[day] [hour]:[minute]")
            .map_err(askama::Error::custom)?;

        value.format(&format).map_err(askama::Error::custom)
    }

    #[askama::filter_fn]
    pub fn money(value: &Decimal, _: &dyn Values) -> askama::Result<String> {
        let raw = format!("{:.2}", value.abs());
        let (integer, cents) = raw
            .split_once('.')
            .ok_or_else(|| askama::Error::custom("invalid decimal format"))?;

        let mut grouped = String::new();
        for (index, character) in integer.chars().rev().enumerate() {
            if index > 0 && index % 3 == 0 {
                grouped.push('.');
            }
            grouped.push(character);
        }
        let integer = grouped.chars().rev().collect::<String>();
        let sign = if value.is_sign_negative() { "- " } else { "" };

        Ok(format!("{sign}R$ {integer},{cents}"))
    }

    #[askama::filter_fn]
    pub fn quantity(value: &Decimal, _: &dyn Values) -> askama::Result<String> {
        Ok(value.normalize().to_string())
    }

    #[askama::filter_fn]
    pub fn nonnegative(value: &Decimal, _: &dyn Values) -> askama::Result<bool> {
        Ok(*value >= Decimal::ZERO)
    }

    /// Percentual em valor ABSOLUTO, com vírgula decimal. O sinal fica por
    /// conta do template, que o emite junto da seta ▲/▼ — direção nunca é
    /// comunicada só pela cor (verde e vermelho são indistinguíveis para
    /// deuteranopia; medido: ΔE 4,6).
    #[askama::filter_fn]
    pub fn percent(value: &Decimal, _: &dyn Values) -> askama::Result<String> {
        Ok(format!("{:.2}", value.abs()).replace('.', ","))
    }
}
