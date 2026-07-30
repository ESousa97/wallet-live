# Entrega final — wallet-live

## Resumo

`wallet-live` é a carteira digital de investimentos proposta no projeto final
do Santander 2026 — Rust AI Developer. Backend, regras financeiras e HTML
renderizado no servidor são escritos em Rust. A aplicação mantém o escopo
central do curso e acrescenta acabamento de produto sem virar uma SPA ou um
conjunto de microserviços.

Stack principal: **Axum + Tokio + SQLx/PostgreSQL + Askama + JWT/cookies**.

> Esta é uma simulação educacional. Não movimenta dinheiro real e não oferece
> recomendação de investimento.

## Matriz de aderência ao curso

| Requisito das aulas finais | Evidência no projeto | Status |
| --- | --- | --- |
| Aplicação full-stack em Rust | Axum, Askama e um único binário | ✅ |
| API REST de ativos | `GET/POST/PATCH /api/assets` e `/api/v1/assets` | ✅ |
| Escritas administrativas protegidas | sessão com papel `admin` ou `Authorization` de serviço | ✅ |
| PostgreSQL e migrations | `migrations/`, SQLx e migração automática no boot | ✅ |
| Repository encapsulando o banco | `src/repository.rs` | ✅ |
| DTOs separados dos modelos | `src/routes/api.rs` e `src/models.rs` | ✅ |
| Cadastro e login com senha protegida | Argon2 via `password-auth`; hash nunca serializado | ✅ |
| Sessão JWT em cookie | access token curto + refresh rotativo e revogável | ✅ |
| Redirecionamento por autenticação | `/` decide entre `/login` e `/assets`; htmx usa `HX-Redirect` | ✅ |
| Carteira por usuário | saldo, posições, custo médio, resultado e patrimônio | ✅ |
| Registro de compra | usuário vem da sessão; preço vem do catálogo | ✅ |
| Histórico | extrato imutável, paginado e exportável em CSV | ✅ |
| Consultas independentes concorrentes | `tokio::try_join!` no serviço da carteira | ✅ |
| Erro central e status HTTP | `AppError` + respostas 5xx censuradas | ✅ |
| Testes SQLx e snapshots da API | bancos efêmeros, fixtures e Insta | ✅ |
| Interface dark e minimalista | Tailwind compilado, SSR e paleta semântica | ✅ |
| Execução reproduzível | `.env.example`, Docker Compose, Dockerfile e CI | ✅ |
| IA como apoio, não como feature artificial | nenhuma integração com LLM no produto | ✅ |

## Melhorias deliberadas sobre a versão didática

- `Decimal/NUMERIC` substitui ponto flutuante no núcleo financeiro.
- Compra, venda e depósito são transacionais e validam escala, saldo e posição.
- CSRF, cookies seguros, lockout de login, refresh revogável e cabeçalhos de
  segurança completam o fluxo de sessão.
- Login e cadastro têm telas separadas para reduzir ambiguidade.
- Compra, venda e depósito usam rotas explícitas, mantendo os handlers finos.
- O histórico por compra virou um livro-razão único, mais simples de auditar.
- htmx atualiza somente o fragmento da carteira; sem JavaScript, o fluxo
  clássico por redirect continua funcionando. O cache de histórico do htmx
  fica desligado para não reexibir dados da carteira após o logout.
- A primeira sincronização de cotações cria o catálogo mínimo com preços reais.
  Ativos sem cotação e operações cujo total arredonde para zero são rejeitados.
- A tela de mercado é informativa e isolada: seus números não entram nos
  cálculos financeiros da carteira.
- O mercado deixou de ser uma tabela de 100 linhas e virou painel: uma moeda em
  foco com série temporal e indicadores, e a lista completa num cartão fixo com
  rolagem própria. A troca de moeda ou de janela é servida do snapshot em
  memória, sem chamada externa nenhuma.

## Roteiro de demonstração (5–7 minutos)

1. Suba a aplicação:

   ```powershell
   docker compose --profile app up --build
   ```

2. Abra <http://localhost:3000>, crie uma conta e mostre que a senha não aparece
   em respostas nem no usuário autenticado.
3. Faça um depósito. Se o catálogo ainda estiver carregando, use **atualizar
   cotações**; a primeira rodada cria USD, EUR, BTC, ETH e SOL.
4. Compre um ativo, mostre saldo, posição, custo médio e extrato. Em seguida,
   venda parte da posição e exporte o CSV.
5. Abra **mercado**: escolha uma moeda no cartão lateral (que rola sozinho,
   com busca por nome ou ticker) e mostre o painel dela — cotação, faixa de
   negociação do dia, gráfico temporal em 24 h e 7 d, capitalização, volume e
   oferta. Aponte a atualização automática e a indicação de alta/baixa por
   seta, sinal e cor.
6. Mostre `GET /api/v1/assets` e a especificação em
   <http://localhost:3000/api/v1/openapi.json>.
7. Encerre com a suíte:

   ```powershell
   cargo fmt --all --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   cargo build --release
   ```

## Critério de pronto

- Build release sem warnings.
- Suíte completa e snapshots aprovados.
- CSS recompilado e coerente com os templates.
- `docker compose config --quiet` válido.
- Fluxo cadastro → depósito → compra → venda → extrato demonstrável.
- Nenhum segredo real versionado.
- Arquivos novos do mercado incluídos no commit da entrega.

Não há, nas transcrições do curso, exigência formal de deploy público. A
entrega é reproduzível localmente e está preparada para container; publicar é
uma decisão posterior de infraestrutura, não uma dependência para avaliação.
