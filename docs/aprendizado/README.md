# Aprendizados aplicados ao projeto

## Objetivo

Registrar aprendizados técnicos aplicados durante a evolução do `wallet-live`, sem
reproduzir aulas, transcrições, materiais exclusivos, avaliações ou conteúdo interno
do bootcamp.

## Escopo

Coberto: conclusões próprias do autor sobre as decisões técnicas do projeto e sobre o
processo de trabalho. Não coberto: descrição de como o sistema funciona hoje — para
isso, ver [../architecture/system-overview.md](../architecture/system-overview.md) — e
a justificativa formal de cada decisão, registrada em [../adr/](../adr/).

## Origem acadêmica

O projeto foi iniciado no contexto do Santander Bootcamp 2026 — Rust AI Developer,
oferecido pela DIO. O curso apresentou fundamentos de Rust, desenvolvimento web,
persistência, testes e uso de ferramentas de IA como apoio ao desenvolvimento.

Este documento registra apenas decisões, experiências e conclusões do autor a partir
da implementação do projeto. Ele não constitui resumo das aulas nem reprodução do
material didático.

A relação entre a versão inicial e a evolução posterior está descrita em
[../decisions/licensing.md](../decisions/licensing.md).

## Aprendizados técnicos

**Rust em um backend web.** Compilar cedo o que outras stacks só descobrem em runtime
muda o custo do erro. Templates, SQL e catálogo de traduções verificados em tempo de
compilação transformam uma classe inteira de defeito de produção em falha de build.

**Separação entre rotas, serviços e persistência.** Manter HTTP nas rotas,
orquestração nos serviços e todo o SQL no repositório foi o que permitiu testar regras
financeiras sem subir servidor e testar o roteamento sem inventar dublê de banco. A
regra só se sustenta quando é verificável: uma consulta escrita fora do repositório é
uma violação visível na revisão.

**Tipos decimais para valores financeiros.** Ponto flutuante acumula erro e o erro
aparece justamente onde dói — no extrato do usuário. Adotar um tipo decimal exato com
escala canônica única, do formulário ao banco, foi a decisão de maior efeito prático
do projeto ([ADR-0004](../adr/0004-decimal-e-numeric-para-dinheiro.md)). O aprendizado
associado é que a escala precisa ser imposta também nos agregados SQL, não só na
escrita.

**Modelagem de posição e histórico.** Um log de compras que só sabe somar não
representa venda. Separar posição materializada de livro-razão imutável resolveu isso
e tornou a auditoria possível ([ADR-0005](../adr/0005-holdings-materializados-e-livro-razao.md)).

**Testes contra PostgreSQL real.** Para caminho de dinheiro, dublê de banco não prova
nada: transação, constraint, `FOR UPDATE` e arredondamento de agregado são
comportamentos do banco. Bancos efêmeros por teste custam tempo de execução e pagam em
confiança.

**Segurança de sessão.** Token assinado não é token revogável. Entender essa distinção
levou a separar acesso curto de refresh opaco rotativo, com revogação real
([ADR-0007](../adr/0007-sessao-jwt-curto-com-refresh-rotativo.md)) — e a tratar CSRF,
lockout e cabeçalhos como parte do mesmo problema, não como itens avulsos.

**Documentação arquitetural.** Escrever ADR obriga a nomear a alternativa descartada.
Boa parte do valor está aí: uma decisão sem alternativa registrada costuma ser um
hábito, não uma decisão.

**Observabilidade.** Log estruturado com identificador de requisição vale mais do que
volume de log. Manter a exportação de telemetria opcional evitou acoplar o
desenvolvimento local a uma infraestrutura que nem sempre existe.

**Declarar ausências.** Registrar o que o sistema não faz — e o que não é medido — foi
tão útil quanto documentar o que existe. Ausência não declarada é lida como omissão.

**Uso responsável de ferramentas de IA.** Assistência é útil para pesquisa, rascunho,
refatoração e revisão; ela não substitui o critério de quem integra o resultado. O
processo que funcionou foi o de sempre revisar antes de aceitar, e tratar cada
sugestão como proposta a ser verificada contra testes e contra o restante do sistema.
O registro dessa prática está em [../decisions/licensing.md](../decisions/licensing.md).

**Protótipo didático e evolução de engenharia são coisas diferentes.** Um protótipo
demonstra um conceito; um produto precisa sobreviver a entrada inválida, a
concorrência, a rollback e a auditoria. A maior parte do esforço deste projeto está
nessa diferença, e ela é o que separa a versão inicial do estado atual.

## Limites

Não estão incluídos neste repositório:

- transcrições;
- slides;
- avaliações;
- respostas de exercícios;
- reconstruções aula por aula;
- trechos extensos do conteúdo ministrado;
- materiais internos ou exclusivos da plataforma.

Marcas, logotipos e materiais exclusivos da DIO ou do Santander também não são
incorporados ao repositório.
