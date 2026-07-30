# Política de segurança

## Escopo deste documento

Como relatar uma vulnerabilidade no `wallet-live` e o que esperar em resposta.

> **Contexto que calibra as expectativas:** o `wallet-live` é um **projeto educacional
> de autor único**, sem equipe de segurança, sem plantão e sem instância em produção.
> Não movimenta dinheiro real, não integra meio de pagamento e não custodia ativo de
> terceiro. Os prazos abaixo são intenções de melhor esforço, **não compromissos
> contratuais**.

## Versões suportadas

| Versão | Suporte |
| --- | --- |
| `master` (ramo principal) | Sim |
| Versões anteriores | **Não há releases publicados nem tags** |

Como não existem versões publicadas, correções são aplicadas ao ramo principal.

## Como relatar uma vulnerabilidade

**Não abra uma issue pública** para vulnerabilidade de segurança.

Use um destes canais:

1. **GitHub Security Advisories** (preferido) — em
   <https://github.com/ESousa97/wallet-live/security/advisories/new>. Permite discussão
   privada e coordenação da divulgação.
2. **Contato direto com o autor** pelo perfil do GitHub, caso o canal acima não esteja
   disponível.

### O que incluir

Um relato útil contém:

| Item | Por quê |
| --- | --- |
| Descrição da vulnerabilidade | O que está errado |
| **Passos para reproduzir** | Sem isso, a verificação é lenta ou impossível |
| Impacto | O que um atacante consegue |
| Versão ou commit | `git rev-parse HEAD` |
| Ambiente | SO, versão do Rust, configuração relevante |
| Sugestão de correção | Opcional, mas acelera |

> **Não inclua credenciais, tokens ou dados pessoais reais** no relato. Use valores
> fictícios claramente identificados.

### O que esperar

| Etapa | Prazo pretendido |
| --- | --- |
| Confirmação de recebimento | 7 dias |
| Avaliação inicial | 14 dias |
| Correção ou plano | Conforme severidade |
| Divulgação coordenada | Após a correção, com crédito ao relator se desejado |

## Escopo

### Dentro do escopo

- O código-fonte deste repositório
- Configuração de segurança (cabeçalhos, cookies, CSP)
- Autenticação, sessão e autorização
- Validação de entrada e tratamento de erro
- Lógica financeira (saldo, posições, custo médio)
- Migrações e integridade do schema
- Dependências, quando alcançáveis por este código

### Fora do escopo

| Item | Motivo |
| --- | --- |
| **Limitações já documentadas** | Ver [known-limitations.md](docs/decisions/known-limitations.md) e [technical-debt.md](docs/decisions/technical-debt.md) — não são achados novos |
| Configuração de quem opera | TLS, proxy reverso, rede |
| APIs de terceiros (Coinbase, CoinGecko) | Relate ao respectivo fornecedor |
| Ausência de recurso de segurança nunca implementado | Ex.: MFA, criptografia em repouso — já registrados |
| Ataques que exigem acesso ao host | O operador está dentro da fronteira de confiança |
| Engenharia social | — |
| Denúncia automatizada de scanner sem análise | Sem verificação de alcançabilidade |
| Instâncias de terceiros | Este repositório não opera nenhuma instância pública |

> **Antes de relatar, verifique se o achado já está documentado.** Os documentos de
> débitos técnicos, limitações conhecidas e modelo de ameaças registram deliberadamente
> as fragilidades conhecidas — incluindo várias que um scanner apontaria.

## Vulnerabilidades conhecidas e aceitas

Registradas com justificativa. **Não precisam ser relatadas.**

| Item | Situação |
| --- | --- |
| **RUSTSEC-2023-0071** (`rsa`, via `jwt-simple`) | Sem correção upstream. **Não alcançável**: a aplicação usa exclusivamente HS256; o código RSA nunca é exercitado. Registrado em `.cargo/audit.toml` |
| Sem criptografia em repouso | Decisão consciente para o escopo educacional |
| Access token não revogável por até 10 min | Consequência do JWT stateless ([ADR-0007](docs/adr/0007-sessao-jwt-curto-com-refresh-rotativo.md)) |
| Lockout de login em memória | Adequado a instância única (DT-01) |
| Sem *rate limiting* global | Mitigável no proxy reverso (RR-12) |
| Catálogo de ativos público | Decisão consciente — não é dado de usuário |

Lista completa: [modelo de ameaças](docs/security/threat-model.md) §5.

## Riscos conhecidos **ainda não corrigidos**

Registrados com transparência. Relatos que apenas os reafirmem não trazem informação
nova, mas **contribuições que os corrijam são bem-vindas**:

| ID | Risco | Prioridade |
| --- | --- | --- |
| **DT-04** | `COOKIE_SECURE` comparado literalmente com `"true"` — `TRUE`/`1`/`yes` falham **em silêncio** | **Alta** |
| **DT-07** | Segredos validados só por presença, não por qualidade — `JWT_SECRET=a` é aceito | **Alta** |
| **DT-23** | `DATABASE_URL` completa vai para o log em erro de conexão | **Alta** |
| **DT-12** | Escala monetária não garantida pelo schema | **Alta** |
| **DT-13** | Sem trilha de auditoria de alteração de preço | Média |

Detalhes em [technical-debt.md](docs/decisions/technical-debt.md).

## Práticas de segurança do projeto

Para contexto de quem avalia:

| Prática | Estado |
| --- | --- |
| `cargo audit` no CI | A cada push e pull request |
| `Cargo.lock` versionado | Sim |
| **Zero dependências npm** | O build não herda a cadeia JS |
| **Zero requisições a CDN** | htmx e CSS servidos do binário |
| CSP sem `'unsafe-inline'` | Travado por teste |
| Cabeçalhos de segurança em toda resposta | Verificado no sucesso **e** no erro |
| Erros 5xx censurados | Detalhe só no log |
| Queries parametrizadas | Todas; injeção de SQL não alcançável |
| Senhas com argon2 | Via `password-auth` |
| Refresh token só como hash | SHA-256; o valor em claro nunca toca o banco |
| Comparação de segredo em tempo constante | `subtle::ConstantTimeEq` |
| Container sem privilégio | `uid 10001` |
| Modelo de ameaças documentado | [threat-model.md](docs/security/threat-model.md) |

## Divulgação

Preferência por **divulgação coordenada**: correção primeiro, publicação depois, com
crédito ao relator se desejado.

Como não há usuários em produção, o risco de divulgação imediata é baixo — mas a
coordenação é preferida para que o relato venha acompanhado da correção.

## Licença

> ⚠️ **Este repositório ainda não tem licença definida.** Isso afeta como o código pode
> ser usado, inclusive em prova de conceito de um relato. Ver
> [licensing.md](docs/decisions/licensing.md).
