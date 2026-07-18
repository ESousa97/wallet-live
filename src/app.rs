use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use rand::RngCore;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::{Instrument, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::auth::throttle::LoginThrottle;
use crate::config::Config;

/// Estado compartilhado entre as rotas. Guarda a pool de conexões com o Postgres,
/// a configuração do serviço (segredos já lidos do ambiente) e o contador de
/// falhas de login. A `PgPool` é clonável (é um `Arc` por dentro) e o resto vai
/// em `Arc`, então o `#[derive(Clone)]` continua barato: clona-se ponteiro, não
/// conexões nem strings.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub login_throttle: Arc<LoginThrottle>,
}

impl AppState {
    pub async fn build(config: Arc<Config>) -> color_eyre::Result<Self> {
        let db = PgPool::connect(&config.database_url).await?;

        // Migrações embutidas no binário (`migrate!` lê `migrations/` em tempo
        // de compilação) e aplicadas no boot: o serviço nunca sobe com um schema
        // defasado, e o deploy dispensa um passo manual de migração. É
        // idempotente — migrações já aplicadas são puladas — e falha o boot se
        // uma migração falhar, que é o comportamento certo: melhor não subir do
        // que subir contra um schema pela metade.
        sqlx::migrate!().run(&db).await?;

        Ok(Self {
            db,
            config,
            login_throttle: Arc::new(LoginThrottle::default()),
        })
    }
}

/// Governa o serviço. Mantém a `main` enxuta e concentra aqui a inicialização.
pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        // Hooks de erro/panic mais legíveis (backtraces formatadas). Combina com a
        // `main` devolvendo `color_eyre::Result`.
        color_eyre::install()?;

        // Carrega o `.env` se existir. NÃO é fatal não existir: em produção as
        // variáveis vêm do ambiente de verdade, sem arquivo nenhum.
        let _ = dotenvy::dotenv();

        init_tracing();

        // Toda a configuração é validada já aqui: se faltar um segredo, o serviço
        // morre no boot com uma mensagem clara em vez de na primeira requisição.
        let config = Arc::new(Config::from_env()?);
        let bind_addr = config.bind_addr;

        let state = AppState::build(config).await?;

        let listener = TcpListener::bind(bind_addr).await?;
        info!(%bind_addr, "starting service");

        let router = Router::new()
            // Sondas separadas: liveness (o processo responde?) nunca depende do
            // banco — reiniciar o app não conserta um banco fora do ar; já a
            // readiness (pode receber tráfego?) exige o banco são. /health fica
            // como alias histórico da readiness.
            .route("/healthz", get(liveness))
            .route("/readyz", get(readiness))
            .route("/health", get(readiness))
            // Caminho canônico e versionado da API: mudanças incompatíveis
            // futuras entram como /api/v2 sem quebrar consumidores do v1.
            .nest("/api/v1", crate::routes::api::router())
            // Alias de compatibilidade para consumidores existentes de /api.
            .nest("/api", crate::routes::api::router())
            // `merge` monta as rotas do front-end na raiz (sem prefixo), ao
            // contrário do `nest` da API.
            .merge(crate::routes::frontend::router())
            .with_state(state.clone())
            // Renova sessões com access token expirado e refresh token válido
            // (rotacionando o refresh) antes de qualquer handler rodar.
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::auth::session::refresh_session,
            ))
            // Cabeçalhos de segurança em TODA resposta.
            .layer(axum::middleware::from_fn_with_state(
                state,
                security_headers,
            ))
            // Camada MAIS externa: abre o span da requisição (com request_id)
            // antes de tudo — assim até os logs dos middlewares internos saem
            // correlacionados ao mesmo id.
            .layer(axum::middleware::from_fn(request_tracing));

        // `with_graceful_shutdown` deixa as requisições em voo terminarem quando
        // chega um Ctrl+C, em vez de cortar conexões no meio.
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

/// Subscriber de tracing escrevendo no terminal, com nível controlável via
/// `RUST_LOG` (ex.: `RUST_LOG=wallet=debug`; padrão `info`) e formato via
/// `LOG_FORMAT`: `json` emite uma linha JSON por evento (para agregadores como
/// CloudWatch/Loki); qualquer outro valor usa o formato legível de terminal.
///
/// `LOG_FORMAT` é lido aqui (e não na `Config`) de propósito: o logging precisa
/// existir ANTES de a configuração ser validada, para que os próprios erros de
/// configuração já saiam no formato certo.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);

    let json = std::env::var("LOG_FORMAT").is_ok_and(|value| value.eq_ignore_ascii_case("json"));
    if json {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}

/// Header padrão de correlação de requisições.
const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Envolve cada requisição num span com `request_id`, método e caminho, e loga
/// a conclusão com status e latência. O id vem do header `x-request-id` quando
/// um proxy/gateway já o gerou (propagação), senão é gerado aqui; em ambos os
/// casos ele volta na resposta — o cliente pode citá-lo num reporte de erro e o
/// log correspondente é encontrado na hora.
async fn request_tracing(request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        // Só aceitamos ids bem-comportados de fora: limita tamanho e alfabeto
        // para um header malicioso não injetar lixo nos logs.
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        .map(str::to_owned)
        .unwrap_or_else(new_request_id);

    let span = tracing::info_span!(
        "request",
        %request_id,
        method = %request.method(),
        path = %request.uri().path()
    );

    let start = std::time::Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;

    span.in_scope(|| {
        info!(
            status = response.status().as_u16(),
            latency_ms = start.elapsed().as_millis() as u64,
            "request completed"
        );
    });

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

/// 8 bytes aleatórios em hexa — curto o bastante para logs, único o bastante
/// para correlação.
fn new_request_id() -> String {
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Cabeçalhos de segurança aplicados a toda resposta:
///
/// - **CSP** restringe de onde a página pode carregar código/estilo: scripts
///   apenas do próprio site (o Tailwind é servido pelo binário em
///   `/static/tailwind.js`); `style-src 'unsafe-inline'` porque esse bundle
///   injeta `<style>` em runtime; `frame-ancestors 'none'` bloqueia
///   clickjacking; `form-action 'self'` impede formulários de postarem para
///   fora.
/// - **nosniff** impede o navegador de "adivinhar" content-type.
/// - **Referrer-Policy** não vaza URLs internas para sites de destino.
/// - **HSTS** só quando o serviço está atrás de HTTPS (mesmo sinal do cookie
///   `Secure`); enviá-lo em HTTP local só causaria confusão.
async fn security_headers(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             connect-src 'self'; frame-ancestors 'none'; form-action 'self'; \
             base-uri 'self'; object-src 'none'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );

    if state.config.cookie_secure {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=63072000; includeSubDomains"),
        );
    }

    response
}

/// Liveness: o processo está vivo e atendendo. Se isto falhar, o orquestrador
/// deve REINICIAR o container — por isso não consulta o banco: reiniciar o app
/// não ressuscita um Postgres fora do ar.
async fn liveness() -> StatusCode {
    StatusCode::OK
}

/// Readiness: o serviço pode receber tráfego — exige o banco respondendo. Se
/// falhar, o orquestrador tira a instância do balanceador (sem reiniciá-la) até
/// a dependência voltar.
async fn readiness(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// Resolve quando o processo recebe um Ctrl+C. Alimenta o desligamento gracioso.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_ok() {
        info!("shutdown signal received");
    }
}
