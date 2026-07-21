use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Histogram;
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
/// a configuração do serviço (segredos já lidos do ambiente), o contador de
/// falhas de login e os instrumentos de métricas. A `PgPool` é clonável (é um
/// `Arc` por dentro), `RequestMetrics` guarda só handles `Arc` dos instrumentos
/// (baratos de clonar) e o resto vai em `Arc`, então o `#[derive(Clone)]`
/// continua barato: clona-se ponteiro, não conexões nem strings.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub login_throttle: Arc<LoginThrottle>,
    metrics: RequestMetrics,
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
            metrics: RequestMetrics::new(),
        })
    }
}

/// Instrumentos de métrica usados pelo middleware de requisição. Construídos a
/// partir do `Meter` GLOBAL (`opentelemetry::global`): se nenhum `MeterProvider`
/// foi instalado (ver `init_otel`), os handles funcionam do mesmo jeito, só que
/// descartam tudo que é registrado — sem `Option`, sem ramificação no caminho
/// quente, o serviço nunca precisa saber se a exportação está ligada.
#[derive(Clone)]
struct RequestMetrics {
    request_duration: Histogram<f64>,
}

impl RequestMetrics {
    fn new() -> Self {
        let meter = opentelemetry::global::meter("wallet");
        Self {
            request_duration: meter
                .f64_histogram("http.server.request.duration")
                .with_unit("s")
                .with_description("Duração das requisições HTTP, do recebimento à resposta.")
                .build(),
        }
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

        // Vive até o fim de `start` (early return por erro incluído, via drop
        // na desconstrução da stack): o `Drop` do guard escoa os spans e
        // métricas ainda no buffer antes do processo morrer.
        let _otel_guard = init_tracing();

        // Toda a configuração é validada já aqui: se faltar um segredo, o serviço
        // morre no boot com uma mensagem clara em vez de na primeira requisição.
        let config = Arc::new(Config::from_env()?);
        let bind_addr = config.bind_addr;

        let state = AppState::build(config).await?;

        // Job de cotações em segundo plano (rodada imediata + intervalo).
        crate::quotes::spawn_scheduled_sync(state.clone());

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
                state.clone(),
                security_headers,
            ))
            // Camada MAIS externa: abre o span da requisição (com request_id)
            // antes de tudo — assim até os logs dos middlewares internos saem
            // correlacionados ao mesmo id. Também onde a métrica de duração é
            // registrada, pelo mesmo motivo: cobre a requisição inteira.
            .layer(axum::middleware::from_fn_with_state(state, request_tracing));

        // `with_graceful_shutdown` deixa as requisições em voo terminarem quando
        // chega um Ctrl+C, em vez de cortar conexões no meio.
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

/// Subscriber de tracing escrevendo no terminal, com nível controlável via
/// `RUST_LOG` (ex.: `RUST_LOG=wallet=debug`; padrão `info`), formato via
/// `LOG_FORMAT` (`json` emite uma linha JSON por evento, para agregadores como
/// CloudWatch/Loki; qualquer outro valor usa o formato legível de terminal) e,
/// opcionalmente, exportação OTLP (ver `init_otel`).
///
/// `LOG_FORMAT` é lido aqui (e não na `Config`) de propósito: o logging precisa
/// existir ANTES de a configuração ser validada, para que os próprios erros de
/// configuração já saiam no formato certo. A exportação OTel segue a mesma
/// regra pelo mesmo motivo.
fn init_tracing() -> OtelGuard {
    use opentelemetry::trace::TracerProvider;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // `fmt::layer()` puro e `fmt::layer().json()` são tipos diferentes — cada
    // um fica no seu próprio `Option`, e só um dos dois é `Some`. `Layer` é
    // implementado para `Option<L>` (`None` vira no-op), então dá pra compor
    // sem `Box<dyn Layer>`.
    let json = std::env::var("LOG_FORMAT").is_ok_and(|value| value.eq_ignore_ascii_case("json"));
    let (json_layer, plain_layer) = if json {
        (Some(tracing_subscriber::fmt::layer().json()), None)
    } else {
        (None, Some(tracing_subscriber::fmt::layer()))
    };

    let otel = init_otel();
    let otel_trace_layer = otel
        .tracer_provider
        .as_ref()
        .map(|provider| tracing_opentelemetry::layer().with_tracer(provider.tracer("wallet")));

    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(plain_layer)
        .with(otel_trace_layer)
        .init();

    otel
}

/// Guarda os providers OTel vivos até o fim do processo. Nenhum dos dois expõe
/// um jeito de fazer flush pela ponta do `tracing`, então o `Drop` é a única
/// garantia de que o lote de spans/métricas ainda no buffer sai antes do
/// processo morrer — inclusive numa saída antecipada por erro (o `?` de
/// `App::start`, que desmonta a stack e derruba este guard no caminho).
#[derive(Default)]
struct OtelGuard {
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(provider) = &self.tracer_provider
            && let Err(error) = provider.shutdown()
        {
            tracing::error!(%error, "failed to flush the otel tracer provider on shutdown");
        }
        if let Some(provider) = &self.meter_provider
            && let Err(error) = provider.shutdown()
        {
            tracing::error!(%error, "failed to flush the otel meter provider on shutdown");
        }
    }
}

/// Liga a exportação OTLP (traces + métricas via HTTP/protobuf) quando
/// `OTEL_EXPORTER_OTLP_ENDPOINT` está definido; sem a variável, devolve um
/// guard vazio e o serviço roda como sempre rodou — nenhuma tentativa de
/// conexão, nenhum overhead. `OTEL_SERVICE_NAME` nomeia o serviço no backend
/// (padrão `wallet`; sem isso a maioria dos backends chama tudo de
/// `unknown_service`).
///
/// Falha ao montar um exportador (endpoint malformado, por exemplo) NÃO derruba
/// o boot, ao contrário de um segredo obrigatório ausente em `Config`:
/// observabilidade é infraestrutura auxiliar, não algo pelo qual vale a pena
/// recusar servir requisições financeiras. `eprintln!` (em vez de
/// `tracing::error!`) porque isto roda antes do subscriber global existir —
/// mesma razão pela qual `LOG_FORMAT` também é lido cru, sem passar pelo
/// `tracing`.
fn init_otel() -> OtelGuard {
    use opentelemetry::global;
    use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
    use opentelemetry_sdk::Resource;
    use opentelemetry_sdk::metrics::SdkMeterProvider;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        return OtelGuard::default();
    }

    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "wallet".to_string());
    let resource = Resource::builder().with_service_name(service_name).build();

    let tracer_provider = match SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
    {
        Ok(exporter) => {
            let provider = SdkTracerProvider::builder()
                .with_resource(resource.clone())
                .with_batch_exporter(exporter)
                .build();
            global::set_tracer_provider(provider.clone());
            Some(provider)
        }
        Err(error) => {
            eprintln!("failed to build the otel span exporter, traces stay local: {error}");
            None
        }
    };

    let meter_provider = match MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .build()
    {
        Ok(exporter) => {
            let provider = SdkMeterProvider::builder()
                .with_resource(resource)
                .with_periodic_exporter(exporter)
                .build();
            global::set_meter_provider(provider.clone());
            Some(provider)
        }
        Err(error) => {
            eprintln!("failed to build the otel metric exporter, metrics stay local: {error}");
            None
        }
    };

    OtelGuard {
        tracer_provider,
        meter_provider,
    }
}

/// Header padrão de correlação de requisições.
const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Envolve cada requisição num span com `request_id`, método e caminho, loga a
/// conclusão com status e latência, e registra a mesma latência no histograma
/// `http.server.request.duration`. O id vem do header `x-request-id` quando um
/// proxy/gateway já o gerou (propagação), senão é gerado aqui; em ambos os
/// casos ele volta na resposta — o cliente pode citá-lo num reporte de erro e o
/// log correspondente é encontrado na hora.
async fn request_tracing(State(state): State<AppState>, request: Request, next: Next) -> Response {
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

    // Extraídos ANTES do `next.run` (que consome `request`) para sobrar depois
    // dele, na hora de rotular a métrica com o método/rota da requisição.
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let span = tracing::info_span!(
        "request",
        %request_id,
        %method,
        %path
    );

    let start = std::time::Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let elapsed = start.elapsed();

    span.in_scope(|| {
        info!(
            status = response.status().as_u16(),
            latency_ms = elapsed.as_millis() as u64,
            "request completed"
        );
    });

    state.metrics.request_duration.record(
        elapsed.as_secs_f64(),
        &[
            KeyValue::new("http.request.method", method.to_string()),
            KeyValue::new("http.route", path),
            KeyValue::new(
                "http.response.status_code",
                i64::from(response.status().as_u16()),
            ),
        ],
    );

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
