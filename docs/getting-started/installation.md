# Instalação e execução

## Objetivo

Levar o projeto de um repositório recém-clonado a uma aplicação funcionando, por
dois caminhos: desenvolvimento local e stack completo em Docker.

## Escopo

Coberto: requisitos, instalação, primeira execução, verificação e ferramentas
opcionais. Não coberto: variáveis de ambiente em detalhe (ver
[configuration.md](configuration.md)), problemas conhecidos (ver
[troubleshooting.md](troubleshooting.md)) e implantação (ver
[../operations/deployment.md](../operations/deployment.md)).

---

## 1. Requisitos

| Requisito | Versão | Necessário para |
| --- | --- | --- |
| [Rust](https://rustup.rs) | 1.95+ (edition 2024) | Compilar |
| Docker + Compose | Recente | Postgres local e imagem de produção |
| PostgreSQL | 18 | Alternativa ao Docker, se preferir instância própria |
| Git | Qualquer | Clonar |

**Não são necessários:** Node, npm ou qualquer ferramenta JavaScript. O CSS é
compilado por um executável standalone, e o único JS (htmx, máscara monetária) é
vendorado ([ADR-0010](../adr/0010-css-compilado-em-build-time.md)).

O `sqlx-cli` é **opcional** — só é preciso para criar migrações ou regenerar o cache
de queries. O cache versionado permite compilar sem ele e sem banco.

## 2. Caminho A — desenvolvimento local

### 2.1 Clonar

```bash
git clone https://github.com/ESousa97/wallet-live.git
```

```bash
cd wallet-live
```

### 2.2 Subir o Postgres

```bash
docker compose up -d db
```

Sobe apenas o banco (perfil padrão), com healthcheck e volume nomeado `pgdata`.

### 2.3 Configurar o ambiente

```bash
cp .env.example .env
```

```powershell
Copy-Item .env.example .env
```

Edite `.env` e **substitua os dois valores de exemplo** — `ADMIN_SECRET_KEY` e
`JWT_SECRET` vêm com `change-me`, que é público neste repositório.

Gerar valores adequados:

```bash
openssl rand -base64 48
```

```powershell
[Convert]::ToBase64String((1..48 | ForEach-Object { Get-Random -Max 256 }))
```

### 2.4 Executar

```bash
cargo run
```

O primeiro build compila toda a árvore de dependências e leva alguns minutos. No
boot, nesta ordem: configuração validada → migrações aplicadas → jobs iniciados →
porta aberta.

Abra <http://localhost:3000>.

### 2.5 Verificar

```bash
curl -fsS http://localhost:3000/healthz
```

```bash
curl -fsS http://localhost:3000/readyz
```

O primeiro responde `200` se o processo está vivo; o segundo, se o banco também
responde. Ambos com `200` significam serviço pronto.

## 3. Caminho B — stack completo em Docker

```bash
docker compose --profile app up --build
```

Builda a imagem de produção (multi-stage, binário único com templates e migrações
embutidos) e sobe app + banco com healthchecks. As migrações rodam sozinhas.

> O compose define valores padrão para os segredos
> (`dev-admin-secret-change-me`). São adequados para uso local e **nunca** para
> ambiente exposto. Sobrescreva com `ADMIN_SECRET_KEY` e `JWT_SECRET` no ambiente.

Máquinas atrás de proxy corporativo ou antivírus com inspeção TLS precisam de um
passo extra — ver [troubleshooting.md](troubleshooting.md) §3.

## 4. Primeiro uso

1. Abra <http://localhost:3000> e **cadastre um usuário** (username 3–32, senha
   8–128).
2. **Deposite** um valor.
3. Se o catálogo estiver vazio, use **atualizar cotações**. A primeira rodada
   bem-sucedida cria USD, EUR, BTC, ETH e SOL com preços reais.
4. **Compre** um ativo, veja saldo, posição, custo médio e extrato.
5. **Venda** parte da posição e **exporte o CSV**.
6. Abra **mercado** e escolha uma moeda no cartão lateral.

> Sem acesso à internet, a sincronização falha e o catálogo fica vazio. O estado
> vazio oferece o botão de tentar novamente, e a API administrativa permite cadastrar
> um ativo manualmente — ver [../api/endpoints.md](../api/endpoints.md) §3.

### Tornar-se admin

Não há rota para promoção. Exige `UPDATE` manual:

```bash
docker compose exec db psql -U postgres -c "UPDATE users SET role = 'admin' WHERE username = 'alice';"
```

A sessão só reflete o novo papel após a expiração do token de acesso (≤ 10 min por
padrão) ou um novo login.

## 5. Testes

A suíte completa precisa do Postgres de pé:

```bash
cargo test
```

Só o que não toca banco:

```bash
cargo test --test payload_market --test payload_quotes
```

Detalhes em [../testing/test-strategy.md](../testing/test-strategy.md).

## 6. Verificações do CI, localmente

```bash
cargo fmt --all --check
```

```bash
cargo clippy --all-targets -- -D warnings
```

```bash
cargo build --release
```

## 7. Ferramentas opcionais

### `sqlx-cli` — criar migrações e regenerar o cache

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
```

Necessário apenas ao alterar o schema ou as queries. Ver
[../data/migrations.md](../data/migrations.md).

### CLI do Tailwind — recompilar o CSS

Necessário apenas ao editar `styles/app.css` ou usar uma classe nova nos templates.
**Versão exata: 4.3.3** — é a que o CI usa, e divergir produz `diff` espúrio.

```bash
curl -fsSL -o tools/tailwindcss https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-linux-x64
```

```bash
chmod +x tools/tailwindcss && ./tools/tailwindcss -i styles/app.css -o static/app.css --minify
```

O diretório `tools/` está no `.gitignore`; `static/app.css` **é versionado** e
precisa ser commitado após recompilar, senão o CI falha.

### Coletor OTLP local

```bash
docker compose --profile observability up -d otel-collector
```

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 cargo run
```

```powershell
$env:OTEL_EXPORTER_OTLP_ENDPOINT = 'http://localhost:4318'; cargo run
```

`docker compose logs -f otel-collector` mostra cada trace e ponto de métrica
recebido. O coletor só imprime — não repassa a lugar nenhum.

## 8. Parar e limpar

```bash
docker compose down
```

Remove os containers, **preservando** o volume `pgdata`.

```bash
docker compose down -v
```

> ⚠️ **`-v` apaga o volume e todos os dados** — usuários, carteiras e o extrato
> completo. Não há backup automático.

## 9. Estrutura do repositório

```text
src/
  main.rs            # 8 linhas: tokio::main -> App::start()
  lib.rs             # os módulos como BIBLIOTECA — é o que permite tests/ existir
  app.rs             # boot, AppState, router, camadas, sondas, tracing
  config.rs          # Config: lê e valida o ambiente uma vez (fail-fast)
  i18n.rs            # catálogo tipado pt-BR/en + extrator
  models.rs          # Asset, UserRecord, WalletSummary, Holding, Transaction
  error.rs           # AppError (21 variantes) + IntoResponse
  quotes.rs          # cotações Coinbase -> preços dos ativos (lastreia dinheiro)
  market.rs          # snapshot CoinGecko em memória (informativo)
  repository.rs      # todo o acesso ao banco + 26 testes
  auth/              # user, session, admin, csrf, throttle
  services/          # portfolio: orquestração da carteira
  routes/            # api (JSON), frontend (SSR), flash
tests/               # suíte de contrato (35 testes)
  payloads/          # respostas REAIS da CoinGecko e da Coinbase, versionadas
templates/           # 7 templates Askama
migrations/          # 11 pares up/down
docs/                # esta documentação
static/              # app.css, htmx.js, money-input.js (embutidos no binário)
styles/              # fonte do CSS (compilado para static/app.css)
```

## 10. Evidências

```text
- Cargo.toml, Cargo.lock
- docker-compose.yaml       (perfis db, app, observability)
- Dockerfile                (multi-stage)
- .env.example
- .github/workflows/ci.yml  (comandos que o CI executa)
- src/app.rs                · App::start (ordem do boot)
```
