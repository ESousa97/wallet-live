# Backup e recuperação

## Objetivo

Registrar o estado real da proteção de dados do projeto — que é a **ausência de
qualquer mecanismo de backup** — e fornecer os procedimentos necessários para
implementá-la.

## Escopo

Coberto: o que existe hoje, o que se perde num incidente, procedimentos de backup e
restauração, e recomendações. Não coberto: incidentes operacionais (ver
[runbooks.md](runbooks.md)) e modelo de dados (ver
[../data/data-model.md](../data/data-model.md)).

---

## 1. Estado atual: não há backup

> **Nenhum backup é executado por este projeto.** Não há job, script, cron, snapshot
> automático nem replicação. O único mecanismo de persistência é o volume Docker
> nomeado `pgdata`.

Isso significa, em termos concretos:

| Evento | Consequência |
| --- | --- |
| `docker compose down` | Dados **preservados** (o volume sobrevive) |
| `docker compose down -v` | **Perda total e irreversível** |
| Volume corrompido | **Perda total** |
| Disco do host falha | **Perda total** |
| `DROP TABLE` acidental | **Perda irreversível** |
| Migração destrutiva | **Perda irreversível** |

O comando `docker compose down -v` aparece na documentação de instalação como forma
legítima de limpar o ambiente de desenvolvimento — e é exatamente o comando que apaga
tudo. Em desenvolvimento é o comportamento desejado; em qualquer outro contexto seria
catastrófico.

Registrado como **DT-05** em
[../decisions/technical-debt.md](../decisions/technical-debt.md).

## 2. O que se perde

| Dado | Recuperável? | Consequência da perda |
| --- | --- | --- |
| `users` | **Não** | Contas e senhas perdidas; **não há recuperação de senha** de qualquer forma |
| `transactions` | **Não** | **O livro-razão.** É histórico imutável e insubstituível |
| `holdings` | Parcialmente | Reconstruível de `transactions` — que também se perdeu |
| `users.balance` | **Não** | Reconstruível da soma de `cash_delta` — idem |
| `assets` | **Sim** | A próxima sincronização recria o catálogo mínimo com preços reais |
| `portfolio_snapshots` | **Não** | Série histórica do patrimônio |
| `sessions` | Irrelevante | Usuários fazem login novamente |
| Snapshot de mercado | Irrelevante | Vive em memória; recriado em ≤ 60 s |

**`transactions` é o ativo insubstituível.** Todo o resto ou é reconstruível a partir
dele, ou é recriado automaticamente. É o livro-razão imutável — a razão de ele existir
como tabela separada é justamente ser auditável, e sem backup essa propriedade não
sobrevive a um incidente de disco.

## 3. Procedimento de backup

### Backup lógico (`pg_dump`)

Adequado para este volume de dados. Formato customizado (`-Fc`) permite restauração
seletiva e é comprimido:

```bash
docker compose exec -T db pg_dump -U postgres -Fc postgres > backup-$(date +%Y%m%d-%H%M%S).dump
```

```powershell
docker compose exec -T db pg_dump -U postgres -Fc postgres > "backup-$(Get-Date -Format 'yyyyMMdd-HHmmss').dump"
```

Apenas o esquema, para conferência:

```bash
docker compose exec -T db pg_dump -U postgres --schema-only postgres > schema.sql
```

Apenas o livro-razão — o dado mais crítico:

```bash
docker compose exec -T db pg_dump -U postgres -Fc -t transactions -t users postgres > ledger-$(date +%Y%m%d).dump
```

### Backup do volume

Alternativa que copia os arquivos do Postgres. **Exige o banco parado** para ser
consistente:

```bash
docker compose stop db
```

```bash
docker run --rm -v wallet-live_pgdata:/data -v "$(pwd)":/backup alpine tar czf /backup/pgdata-$(date +%Y%m%d).tar.gz -C /data .
```

```bash
docker compose start db
```

> Confirme o nome real do volume com `docker volume ls` — ele recebe o prefixo do
> diretório do projeto.

**Prefira `pg_dump`**: não exige parada, é portátil entre versões do Postgres e
permite restauração seletiva.

## 4. Procedimento de restauração

> ⚠️ **A restauração sobrescreve o banco atual.** Faça um backup do estado corrente
> antes, mesmo que ele pareça corrompido — ele pode conter dado que o backup antigo
> não tem.

### 1. Parar a aplicação

```bash
docker compose --profile app stop app
```

Necessário: as migrações rodam no boot e os jobs escrevem no banco. Restaurar com a
aplicação ativa produz estado inconsistente.

### 2. Salvar o estado atual

```bash
docker compose exec -T db pg_dump -U postgres -Fc postgres > pre-restore-$(date +%Y%m%d-%H%M%S).dump
```

### 3. Restaurar

```bash
docker compose exec -T db pg_restore -U postgres -d postgres --clean --if-exists < backup-AAAAMMDD-HHMMSS.dump
```

`--clean --if-exists` remove os objetos existentes antes de recriar.

### 4. Verificar a integridade

```bash
docker compose exec db psql -U postgres -c "SELECT (SELECT count(*) FROM users) users, (SELECT count(*) FROM transactions) transacoes, (SELECT count(*) FROM holdings) posicoes, (SELECT count(*) FROM assets) ativos;"
```

Conferir o estado das migrações:

```bash
docker compose exec db psql -U postgres -c "SELECT version, description, success FROM _sqlx_migrations ORDER BY version;"
```

Todas devem ter `success = true`.

### 5. Reconciliar saldo contra o livro-razão

**Esta verificação é específica deste sistema** e vale a pena após qualquer
restauração: o saldo é estado mutável, e o extrato é a fonte imutável. Os dois devem
concordar.

```bash
docker compose exec db psql -U postgres -c "SELECT u.id, u.username, u.balance, COALESCE(SUM(t.cash_delta), 0) soma_extrato, u.balance - COALESCE(SUM(t.cash_delta), 0) diferenca FROM users u LEFT JOIN transactions t ON t.user_id = u.id GROUP BY u.id, u.username, u.balance HAVING u.balance <> COALESCE(SUM(t.cash_delta), 0);"
```

**Resultado vazio significa que todos os saldos batem com o extrato.** Qualquer linha
retornada indica divergência que precisa ser investigada antes de liberar o serviço.

Conferir também a escala monetária, que é o invariante mais frágil do sistema:

```bash
docker compose exec db psql -U postgres -c "SELECT 'assets' t, count(*) FROM assets WHERE scale(unit_value) > 8 UNION ALL SELECT 'holdings', count(*) FROM holdings WHERE scale(avg_cost) > 8 UNION ALL SELECT 'users', count(*) FROM users WHERE scale(balance) > 8;"
```

Todas as contagens devem ser zero. Se não forem, aplicar o saneamento de
[runbooks.md](runbooks.md) §R-06.

### 6. Subir e validar

```bash
docker compose --profile app start app
```

```bash
curl -fsS http://localhost:3000/readyz
```

Validação funcional: entrar com um usuário conhecido e confirmar que saldo, posições
e extrato estão coerentes.

## 5. Objetivos de recuperação

**Nenhum RPO ou RTO está definido**, porque não há backup. Os valores abaixo são
**recomendações**, não compromissos vigentes:

| Métrica | Recomendado | Justificativa |
| --- | --- | --- |
| **RPO** (perda máxima aceitável) | 24 h | Backup diário. Perder um dia de operações é ruim mas recuperável por conferência com o extrato dos usuários |
| **RTO** (tempo até restaurar) | 1 h | O volume de dados é pequeno; a restauração é rápida |
| Retenção | 30 dias diários + 12 meses mensais | Permite voltar antes de uma corrupção não detectada de imediato |
| Local | **Fora do host** | Um backup no mesmo disco não protege contra falha de disco |
| **Teste de restauração** | Trimestral | **Backup nunca restaurado não é backup** |

## 6. Recomendações, em ordem

| # | Ação | Esforço | Reduz |
| --- | --- | --- | --- |
| 1 | **Backup diário automatizado com `pg_dump`** | Baixo | Perda total |
| 2 | **Armazenar fora do host** | Baixo | Falha de disco |
| 3 | Testar a restauração ao menos uma vez | Baixo | Backup inválido descoberto no pior momento |
| 4 | Definir e documentar RPO/RTO | Baixo | Expectativa indefinida |
| 5 | Verificação de integridade pós-restauração automatizada | Médio | Restauração silenciosamente incompleta |
| 6 | Replicação ou instância gerenciada com backup | Médio | Ponto único de falha |
| 7 | Backup antes de toda migração | Baixo | Migração destrutiva irreversível |

> **A recomendação 7 é a de melhor relação custo-benefício no estado atual.** As
> migrações são aplicadas automaticamente no boot, e três delas
> (`money_to_numeric`, `holdings_and_transactions`, `normalize_money_scales`) são
> **irreversíveis ou destrutivas**. Um `pg_dump` antes de cada implantação que
> contenha migração nova custa segundos e evita perda irrecuperável.

## 7. Relação com outras decisões

| Decisão | Implicação para backup |
| --- | --- |
| Migrações no boot ([ADR-0006](../adr/0006-sqlx-com-checagem-em-compilacao.md)) | Um deploy pode alterar o schema **sem intervenção humana** — daí a recomendação 7 |
| `transactions` imutável ([ADR-0005](../adr/0005-holdings-materializados-e-livro-razao.md)) | É o ativo insubstituível; todo o resto deriva dele |
| Snapshot de mercado em memória ([ADR-0009](../adr/0009-snapshot-de-mercado-em-memoria.md)) | **Não precisa de backup** — é dado de terceiro, recriado em ≤ 60 s |
| `assets` recriado pela sincronização | Catálogo mínimo se autorrecupera |
| Sem criptografia em repouso | **O arquivo de backup contém extrato e saldos em texto** — é um ativo sensível e deve ser protegido como tal |

## 8. Evidências

```text
- docker-compose.yaml   (volume nomeado pgdata; nenhum serviço de backup)
- .github/workflows/ci.yml (nenhum passo de backup)
- migrations/           (11 migrações; três com reversão destrutiva ou impossível)
- src/app.rs            · AppState::build (migrações no boot)
- docs/data/migrations.md §3 (quais reversões perdem dado)
```
