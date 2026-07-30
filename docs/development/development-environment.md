# Ambiente de desenvolvimento

## Objetivo

Descrever o ciclo de trabalho diário: como rodar, o que regenerar após cada tipo de
mudança, e as particularidades do ambiente que já causaram atrito.

## Escopo

Coberto: ferramental, ciclo de desenvolvimento, artefatos gerados e especificidades de
plataforma. Não coberto: instalação inicial (ver
[../getting-started/installation.md](../getting-started/installation.md)), padrões de
código (ver [coding-standards.md](coding-standards.md)) e problemas conhecidos (ver
[../getting-started/troubleshooting.md](../getting-started/troubleshooting.md)).

---

## 1. Ferramental

### Obrigatório

| Ferramenta | Versão | Para quê |
| --- | --- | --- |
| Rust | 1.95+ (edition 2024) | Compilar |
| Docker | Recente | Postgres local |

### Opcional, e quando é preciso

| Ferramenta | Necessária quando | Instalação |
| --- | --- | --- |
| `sqlx-cli` | Criar migração ou alterar query | `cargo install sqlx-cli --no-default-features --features postgres,rustls` |
| Tailwind CLI **4.3.3** | Editar `styles/app.css` ou usar classe nova | Download do GitHub Releases |
| `cargo-insta` | Revisar mudança de contrato JSON | `cargo install cargo-insta` |
| `cargo-audit` | Auditar localmente | `cargo install cargo-audit --locked` |

**Não é preciso instalar nada de JavaScript.** Sem Node, sem npm — nem para o CSS.

## 2. Ciclo diário

```bash
docker compose up -d db
```

```bash
cargo run
```

O banco fica de pé entre sessões; só a aplicação é reiniciada. As migrações são
aplicadas automaticamente no boot, então alterar `migrations/` e reiniciar basta.

### Recompilação automática

```bash
cargo install cargo-watch
```

```bash
cargo watch -x run
```

## 3. Artefatos gerados que precisam ser regenerados

**Esta é a particularidade mais importante do ambiente.** Três artefatos são gerados,
**versionados** e podem descolar da fonte — e o CI falha quando isso acontece:

| Mudou | Regenerar | Comando | Se esquecer |
| --- | --- | --- | --- |
| Query SQL ou schema | `.sqlx/` | `cargo sqlx prepare` | CI falha em `prepare --check` |
| `styles/app.css` ou classe nova em template | `static/app.css` | `tailwindcss -i styles/app.css -o static/app.css --minify` | **Estilo faltando em produção** |
| Formato de resposta da API | `src/routes/snapshots/` | `cargo insta review` | Teste de snapshot falha |

### Cache do SQLx

```bash
cargo sqlx prepare
```

Precisa do banco de pé. Regenera os 31 arquivos de `.sqlx/`, que permitem compilar
**sem** banco (`SQLX_OFFLINE=true` no `.cargo/config.toml`).

Isso é o que faz o `lint` do CI, o build Docker e o rust-analyzer funcionarem com o
Postgres desligado.

### CSS

```bash
./tools/tailwindcss -i styles/app.css -o static/app.css --minify
```

> **Use exatamente a versão 4.3.3** — é a que o CI baixa. Versão diferente produz
> `diff` espúrio e quebra o job `lint` sem motivo real.

O binário fica em `tools/`, que está no `.gitignore`. Já `static/app.css` **é
versionado** e precisa ser commitado.

Motivo do risco: o binário embute o CSS via `include_str!`. Uma classe nova usada sem
recompilar deixa o estilo faltando em produção, e **nada no build de Rust perceberia**.

### Snapshots

```bash
cargo insta review
```

Cada mudança de formato precisa ser **aprovada explicitamente** — é o que impede uma
quebra de contrato de passar despercebida.

## 4. Verificar antes de commitar

```bash
cargo fmt --all
```

```bash
cargo clippy --all-targets -- -D warnings
```

```bash
cargo test
```

Lista completa em [coding-standards.md](coding-standards.md) §9.

## 5. Testes durante o desenvolvimento

Um teste isolado:

```bash
cargo test buying_more_averages_the_cost_basis
```

Um módulo:

```bash
cargo test repository::
```

Rápidos, sem banco:

```bash
cargo test --test payload_market --test payload_quotes
```

Com a saída de `println!` visível:

```bash
cargo test <nome> -- --nocapture
```

`#[sqlx::test]` cria um banco **efêmero por teste**, isolado e paralelo — nenhum
depende da ordem nem do estado deixado por outro.

## 6. Banco de desenvolvimento

```bash
docker compose exec db psql -U postgres
```

```bash
docker compose exec db psql -U postgres -c "SELECT id, name, unit_value FROM assets ORDER BY id;"
```

Promover um usuário a admin (não há rota para isso):

```bash
docker compose exec db psql -U postgres -c "UPDATE users SET role = 'admin' WHERE username = 'alice';"
```

O papel só passa a valer após a expiração do access token (≤ 10 min) ou novo login,
porque o `role` viaja nas claims assinadas.

Recomeçar do zero:

```bash
docker compose down -v && docker compose up -d db
```

> ⚠️ **`-v` apaga todos os dados.** Não há backup automático — ver
> [../operations/backup-and-recovery.md](../operations/backup-and-recovery.md).

## 7. Rodar sem internet

A suíte inteira roda sem rede: **nenhum teste faz chamada externa**. Os payloads das
integrações são arquivos versionados em `tests/payloads/`.

O que **não** funciona sem internet: a sincronização de cotações (o catálogo fica
vazio) e a tela de mercado.

Para desligar os dois jobs e evitar erros de rede no log:

```bash
QUOTES_SYNC_MINUTES=0 MARKET_SYNC_SECONDS=0 cargo run
```

Nesse modo, cadastre um ativo pela API administrativa para poder operar.

## 8. Editor

O `.cargo/config.toml` define `SQLX_OFFLINE = "true"`, o que é essencial para o
rust-analyzer: sem isso, todo `query_as!` acusa erro quando o banco está desligado
(`os error 10061`), e o arquivo inteiro fica vermelho.

Se o editor ainda acusar erro: reinicie o rust-analyzer e verifique se
`SQLX_OFFLINE=false` está definido no ambiente — variáveis do ambiente **vencem** o
config do cargo.

## 9. Particularidades de plataforma

### Windows

| Item | Situação |
| --- | --- |
| `CRYPT_E_NO_REVOCATION_CHECK` | Resolvido — `.cargo/config.toml` desativa só a checagem de revogação |
| `cmake` para `jwt-simple` | Resolvido — feature `pure-rust` |
| Tailwind CLI | Baixar a variante Windows (`tailwindcss-windows-x64.exe`) |
| Proxy corporativo / antivírus com inspeção TLS | Ver [../getting-started/troubleshooting.md](../getting-started/troubleshooting.md) §3 |

### Linux

Para compilar `reqwest` com `native-tls`:

```bash
sudo apt-get install -y pkg-config libssl-dev
```

### Postgres 18

O ponto de mount mudou para `/var/lib/postgresql` — os dados ficam numa subpasta
versionada, não mais em `/var/lib/postgresql/data`. Já ajustado no
`docker-compose.yaml`; um compose próprio herdado de versão anterior quebra.

## 10. Mapa: onde mexer para cada tipo de mudança

| Quero mudar | Arquivos | Regenerar |
| --- | --- | --- |
| Uma rota | `src/routes/frontend.rs` ou `api.rs` | — |
| Regra de negócio financeira | `src/repository.rs` | `cargo sqlx prepare` se houver SQL novo |
| Montagem da tela da carteira | `src/services/portfolio.rs` | — |
| HTML | `templates/` | `static/app.css` se houver classe nova |
| Estilo | `styles/app.css` | **`static/app.css`** |
| Texto da interface | `src/i18n.rs` | — (falta em um idioma é erro de compilação) |
| Schema | `migrations/` | **`cargo sqlx prepare`** |
| Variável de ambiente | `src/config.rs`, `.env.example` | — |
| Contrato da API | `src/routes/api.rs` | **`cargo insta review`** |
| Cabeçalho de segurança | `src/app.rs` · `security_headers` | — |
| Integração externa | `src/quotes.rs` ou `src/market.rs` | Recapturar payload se o formato mudou |

## 11. Evidências

```text
- .cargo/config.toml       (SQLX_OFFLINE, check-revoke)
- Cargo.toml               (features escolhidas)
- docker-compose.yaml      (perfis; mount do Postgres 18)
- .github/workflows/ci.yml (comandos e versão do Tailwind)
- .gitignore               (/tools/, .env, *.snap.new)
- .sqlx/                   (31 queries)
- styles/app.css           (fonte, com instrução de regeneração)
```
