# Análise de licenciamento

## Objetivo

Analisar a situação de licenciamento do projeto, comparar as licenças aplicáveis e
apresentar uma recomendação técnica fundamentada — sem tomar a decisão, que depende de
fatos que só o titular do código pode confirmar.

## Escopo

Coberto: situação atual, análise de titularidade, compatibilidade com dependências,
matriz comparativa e recomendação condicionada. Não coberto: inventário de
dependências (ver
[../development/dependency-management.md](../development/dependency-management.md)).

> ## ⚠️ Este documento é análise técnica, não orientação jurídica
>
> As considerações abaixo baseiam-se no que é verificável no repositório. Elas **não
> constituem aconselhamento jurídico** e não substituem a consulta a um profissional
> habilitado quando houver dúvida sobre titularidade, obrigação contratual ou uso
> comercial.

---

## 1. Situação atual: **não há licença**

**O repositório não contém arquivo `LICENSE`, `LICENSE.md` ou `COPYING`.** O
`Cargo.toml` também não declara o campo `license`.

### O que isso significa, concretamente

Um repositório **público** sem licença não é "de domínio público" nem "livre para
usar". Sob as convenções de direito autoral aplicáveis à maioria das jurisdições, a
ausência de licença significa **todos os direitos reservados**:

| Ação de terceiro | Permitida hoje? |
| --- | --- |
| Ler o código no GitHub | Sim |
| Fazer *fork* pela interface do GitHub | Sim — permitido pelos Termos de Serviço do GitHub |
| Clonar localmente | Sim, na prática |
| **Usar o código em outro projeto** | **Não** |
| **Modificar e redistribuir** | **Não** |
| **Usar comercialmente** | **Não** |
| **Contribuir com pull request** | Juridicamente ambíguo — não há termo que defina como a contribuição é licenciada |

> **A consequência prática é o oposto da intenção aparente.** Um projeto publicado
> como portfólio técnico, sem licença, não pode ser legalmente reaproveitado por
> ninguém — inclusive por quem quisesse apenas estudar e adaptar um trecho.

**Situação verificada:** repositório público em
`github.com/ESousa97/wallet-live`, 36 commits, autor único (`esousa97`).

## 2. Análise de titularidade

### 2.1 Os termos da DIO — **verificado**

Consulta aos [Termos de Uso da DIO](https://www.dio.me/terms) em 2026-07-30. As duas
cláusulas relevantes:

**Cláusula 2.1 — sobre o conteúdo do usuário ("Suas Informações"):**

> "Nós não clamamos propriedade de suas **Informações** nem das trocas de mensagens
> realizadas por você."

**Cláusula 11.1 — sobre o que a DIO reivindica:**

> "Todos os direitos autorais do **Conteúdo** e da **Plataforma** (incluindo, mas não
> se limitando a imagens, fotografias, animações, vídeos, áudio, música, texto, layout
> e look and feel incorporados na **Plataforma**) são de propriedade da **DIO**."

A distinção está na **definição de "Conteúdo"** (cláusula 3.6):

> "instruções ao vivo e/ou gravadas, tutorial e serviços de aprendizagem através de
> aulas, projetos, desafios, exercícios e atividades on-line"

**"Conteúdo" é o material didático produzido pela DIO** — as aulas, o enunciado do
desafio, o material do bootcamp. A palavra "projetos" nessa definição designa *o
projeto proposto como exercício*, não a implementação escrita pelo aluno.

Além disso, **não há cláusula de cessão ou de licença** sobre obras criadas pelo
usuário — nenhuma concessão do tipo "licença não exclusiva, irrevogável, isenta de
royalties" que plataformas frequentemente incluem.

### 2.2 Os editais Santander/DIO — **verificado parcialmente**

Os editais de bootcamps patrocinados são documentos de **processo seletivo**:
apresentação do programa, público-alvo, período de inscrição, critérios de seleção,
cronograma e suporte.

**Não contêm cláusula de propriedade intelectual, direitos autorais sobre código ou
cessão de direitos sobre trabalhos dos participantes.**

> **Ressalva de verificação:** um dos editais examinados usa fontes CID e não permitiu
> extração de texto. A conclusão acima vale para os editais efetivamente lidos.

### 2.3 Resumo da titularidade

| Fator | Estado | Conclusão |
| --- | --- | --- |
| Autoria | 36 commits, autor único | Obra de autor único |
| **Termos da DIO** | **Verificado** | **A DIO não reivindica propriedade sobre o código do aluno** |
| **Editais Santander** | **Verificado (parcial)** | **Não tratam de propriedade intelectual** |
| Termo específico do bootcamp | **Não verificado** | A cláusula 1.11 prevê que "cada Bootcamp terá os seus termos e condições específicos" — vale conferir o que foi aceito na inscrição |
| **Vínculo empregatício** | **Não verificável a partir do repositório** | Só o autor pode confirmar |
| Código de terceiro | htmx (0BSD) + dependências | Ver §4 |
| Marcas | "DIO" e "Santander" citados | Uso nominativo — ver §2.4 |
| Dados de cliente / segredos comerciais | Nenhum | Sem restrição |

**A titularidade do código é do autor.** O que reforça essa conclusão, além dos termos:
o projeto se afasta deliberadamente da versão didática em praticamente todas as
decisões relevantes — `Decimal` em vez de ponto flutuante, `holdings` + livro-razão em
vez de log append-only, refresh token rotativo, CSP fechada, camada de serviço,
observabilidade. É obra autoral independente, não a entrega de um exercício.

### 2.4 Sobre as marcas citadas

O projeto menciona "DIO" e "Santander" ao descrever sua origem acadêmica. Isso é uso
nominativo — descrever de onde o projeto veio — e não sugere endosso. Recomenda-se
**não** usar logotipos e **não** dar a entender patrocínio ou aprovação.

### 2.5 O que ainda recomenda cautela

Dois pontos permanecem, ambos de baixa probabilidade:

1. **Termo específico do bootcamp** (cláusula 1.11). Os editais examinados não tratam
   de PI, mas o termo aceito na inscrição do bootcamp de Rust não foi localizado.
2. **Contexto de emprego.** Se o código foi escrito com equipamento, tempo ou em
   função de vínculo empregatício, a titularidade pode ser afetada — independentemente
   da DIO.

## 3. Natureza do projeto

Fatores que orientam a escolha:

| Fator | Situação |
| --- | --- |
| Repositório | **Público** |
| Finalidade | Educacional e de portfólio técnico |
| Uso comercial esperado | Nenhum, atualmente |
| Distribuição de binários | Nenhuma; imagem Docker construída localmente |
| Contribuições externas | Nenhuma até hoje |
| Modelo de negócio | Nenhum |
| Dados de terceiros | Nenhum |
| Patentes | Nenhuma envolvida |

## 4. Compatibilidade com as dependências

### O que é verificável

| Item | Licença | Situação |
| --- | --- | --- |
| **htmx 2.0.8** (vendorado em `static/htmx.js`) | **0BSD** | Permissiva; **não exige atribuição**. Compatível com qualquer licença |
| **Tailwind CSS CLI** | MIT | Usado só em **build-time**; o CSS gerado é obra derivada dos próprios templates. Não é redistribuído |
| 392 crates Rust | **Requer verificação** | Ver abaixo |

### O que **não** foi verificado

> **Não foi possível verificar as licenças das 392 dependências neste ambiente.** As
> dependências não estão em cache local e `cargo metadata --offline` falha. **Nenhuma
> afirmação sobre elas é feita aqui sem verificação.**

O ecossistema Rust é predominantemente **MIT OR Apache-2.0**, e os crates usados são
amplamente adotados — mas isso é expectativa, não verificação.

**Esta verificação é pré-requisito da decisão:**

```bash
cargo install cargo-license && cargo license --tsv > licencas.tsv
```

```bash
cargo install cargo-deny && cargo deny check licenses
```

O que procurar no resultado:

| Achado | Implicação |
| --- | --- |
| Só MIT / Apache-2.0 / BSD / ISC / Unicode | **Nenhuma restrição** — qualquer licença desta análise serve |
| Alguma **GPL** ou **AGPL** | **Restringe severamente**: obrigaria o projeto a adotar licença compatível |
| Alguma **LGPL** | Restringe se houver vinculação estática (que é o caso em Rust) |
| Alguma **MPL 2.0** | Copyleft de arquivo; compatível, mas exige atenção |
| Licença **não declarada** | Investigar individualmente |

## 5. Matriz comparativa

| Licença | Uso comercial | Modificação | Redistribuição | Copyleft | Patentes | Adequação a este projeto |
| --- | :---: | :---: | :---: | :---: | :---: | --- |
| **MIT** | Sim | Sim | Sim | Não | **Não** | **Muito boa** — a mais simples e reconhecida; ideal para portfólio |
| **Apache-2.0** | Sim | Sim | Sim | Não | **Sim** (concessão expressa) | **Muito boa** — MIT + proteção de patente e exigência de aviso de mudanças |
| **BSD 3-Clause** | Sim | Sim | Sim | Não | Não | Boa — equivalente à MIT, com cláusula de não endosso |
| **MPL 2.0** | Sim | Sim | Sim | **Por arquivo** | Sim | Adequada se houver intenção de manter modificações abertas sem afetar quem apenas integra |
| **LGPLv3** | Sim | Sim | Sim | **Fraco** | Sim | **Inadequada** — em Rust a vinculação é estática, o que torna as obrigações onerosas |
| **GPLv3** | Sim | Sim | Sim | **Forte** | Sim | Inadequada ao objetivo de portfólio: obriga derivados a adotarem a mesma licença |
| **AGPLv3** | Sim | Sim | Sim | **Forte + rede** | Sim | Inadequada — estenderia o copyleft a quem apenas **executasse** o serviço |
| **Proprietária** | Controlado | Não | Não | — | — | Adequada apenas se houver intenção comercial |
| **Sem licença** (atual) | **Não** | **Não** | **Não** | — | — | **Inadequada** — contradiz a publicação como portfólio |

### Por que AGPL merece uma nota

O AGPLv3 estende as obrigações de copyleft a quem executa o software **como serviço em
rede**. Como este projeto **é** uma aplicação web, adotá-lo significaria que qualquer
pessoa que o hospedasse teria de disponibilizar o código-fonte, incluindo suas
modificações. É um efeito forte, e **quase certamente indesejado** para um projeto de
portfólio.

## 6. Recomendação técnica

**Titularidade verificada (§2): a DIO não reivindica propriedade sobre o código do
aluno.** Resta apenas a verificação das licenças das dependências (§4), que é rápida e
tem alta probabilidade de não trazer impedimento.

### Recomendação principal: **MIT**

| Motivo | Detalhe |
| --- | --- |
| Objetivo é portfólio | Demonstrar competência técnica; a licença deve **maximizar** a possibilidade de leitura e reaproveitamento |
| Simplicidade | ~170 palavras; compreendida sem consulta jurídica |
| Reconhecimento | A licença mais usada do ecossistema Rust — expectativa alinhada |
| Compatibilidade | Compatível com tudo, inclusive com projetos GPL |
| Sem obrigação prática | Só exige preservar o aviso de copyright |

### Alternativa: **Apache-2.0**

Preferível se houver qualquer preocupação com patentes ou com uso corporativo:

| Vantagem sobre a MIT | Detalhe |
| --- | --- |
| **Concessão expressa de patente** | Contribuidores concedem licença de patente; há cláusula de retaliação |
| Exigência de aviso de modificação | Rastreabilidade de derivados |
| Preferência corporativa | Muitas organizações a preferem por segurança jurídica |

Custo: texto mais longo (~10× a MIT) e exigência de arquivo `NOTICE`.

> **Convenção do ecossistema Rust:** a maioria dos projetos usa **licenciamento duplo
> `MIT OR Apache-2.0`**, deixando a escolha com quem usa. É a opção mais idiomática
> para um projeto Rust, e vale ser considerada.

### O que **não** recomendar aqui

| Opção | Por quê |
| --- | --- |
| **Permanecer sem licença** | Contradiz a publicação. Torna o código legalmente inutilizável |
| GPLv3 / AGPLv3 | Copyleft forte, desalinhado com portfólio |
| LGPLv3 | Vinculação estática do Rust torna as obrigações onerosas |
| Proprietária | Sem modelo de negócio que a justifique |

## 7. Passos para decidir

| # | Passo | Estado |
| --- | --- | --- |
| 1 | Confirmar que a DIO não reivindica titularidade | ✅ **Feito** — §2.1 |
| 2 | Confirmar que os editais não tratam de PI | ✅ **Feito (parcial)** — §2.2 |
| 3 | Conferir se o bootcamp teve termo específico (cláusula 1.11) | ⬜ Recomendado, baixa probabilidade de alterar a conclusão |
| 4 | Confirmar ausência de vínculo empregatício sobre o código | ⬜ Só o autor pode |
| 5 | **Verificar as licenças das dependências** com `cargo license` ou `cargo deny` | ⬜ **Único bloqueante técnico restante** |
| 6 | Decidir entre MIT, Apache-2.0 ou o duplo `MIT OR Apache-2.0` | ⬜ |
| 7 | Criar o arquivo `LICENSE` com o texto oficial e o ano/titular corretos | ⬜ |
| 8 | Declarar no `Cargo.toml`: `license = "MIT"` (ou o que for escolhido) | ⬜ |
| 9 | Referenciar a licença no `README.md` | ⬜ |
| 10 | Se aceitar contribuições, definir como elas são licenciadas (o padrão implícito da Apache-2.0 resolve; a MIT não trata do assunto) | ⬜ |

O passo 5 é rápido e independente dos demais:

```bash
cargo install cargo-license && cargo license --tsv > licencas.tsv
```

Se o resultado trouxer apenas licenças permissivas — que é a expectativa para esta
árvore de dependências —, **não há impedimento restante** para adotar MIT ou
Apache-2.0.

### Se a decisão for MIT

Criar `LICENSE` com o texto oficial (obtido de <https://opensource.org/license/mit>),
substituindo ano e titular. Depois:

```toml
[package]
name = "wallet"
version = "0.1.0"
edition = "2024"
license = "MIT"
```

## 8. Por que este documento não cria o arquivo `LICENSE`

Adicionar uma licença é um **ato de disposição de direitos**, com efeito jurídico real
e praticamente irreversível: uma vez publicada sob licença permissiva, qualquer cópia
obtida naquele momento permanece legitimamente licenciada, mesmo que a licença seja
alterada depois.

A escolha entre MIT, Apache-2.0 e o duplo licenciamento é uma decisão do titular sobre
os próprios direitos — não uma conclusão técnica que esta análise possa tomar por ele.
Some-se a isso a verificação de dependências ainda pendente (§4).

> **Histórico desta seção.** Uma versão anterior deste documento classificava a
> titularidade como **bloqueante não resolvida**, sem ter consultado os termos da DIO.
> Era cautela apresentada como achado — o que é pior do que a ausência de pesquisa,
> por sugerir um risco identificado onde havia apenas ausência de verificação. A
> consulta foi feita (§2.1, §2.2) e a conclusão é que **a DIO não reivindica
> propriedade sobre o código do aluno**.

## 9. Estado de cada afirmação

| Afirmação | Estado |
| --- | --- |
| Não há arquivo de licença no repositório | **Verificado** |
| O `Cargo.toml` não declara `license` | **Verificado** |
| O repositório é público | **Verificado** |
| Autor único nos 36 commits | **Verificado** |
| htmx é 0BSD | **Verificado** (conhecimento público do projeto htmx) |
| **A DIO não reivindica propriedade sobre o conteúdo do usuário** | **Verificado** — cláusula 2.1 dos Termos de Uso |
| **"Conteúdo" nos termos = material didático da DIO** | **Verificado** — cláusula 3.6 |
| **Não há cláusula de cessão sobre obras do usuário** | **Verificado** — ausência nos Termos de Uso |
| **Editais Santander não tratam de PI** | **Verificado parcialmente** — um edital não permitiu extração de texto |
| Termo específico do bootcamp de Rust | **Não localizado** — cláusula 1.11 prevê que possa existir |
| Ausência de vínculo empregatício sobre o código | **Não verificável a partir do repositório** |
| Licenças das 392 dependências | **Requer validação** — único bloqueante técnico |

## 10. Evidências

```text
- (ausência de) LICENSE, LICENSE.md, COPYING
- Cargo.toml            (sem campo license)
- git log               (36 commits, autor único)
- git remote -v         (github.com/ESousa97/wallet-live — público)
- static/htmx.js        (htmx 2.0.8, 0BSD)
- docs/delivery/course-delivery.md (contexto do bootcamp)

Fontes externas consultadas em 2026-07-30:
- https://www.dio.me/terms   (cláusulas 2.1, 3.6, 11.1, 11.4, 1.11)
- Editais de seleção Santander Open Academy / DIO
  (assets.santanderopenacademy.com — sem cláusula de PI)
```
