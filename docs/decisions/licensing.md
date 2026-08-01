# Análise de licenciamento

## 1. Objetivo e aviso jurídico

Este documento registra, de forma técnica e auditável, a proveniência conhecida do
projeto, o estado atual de licenciamento e as pendências que precisam ser resolvidas
antes da adoção de uma licença *open source*.

> Esta é uma análise documental, não um parecer jurídico. As conclusões se limitam
> às evidências identificadas no repositório e aos documentos mencionados. Questões
> de titularidade, autorização, sublicenciamento ou uso comercial relevante devem ser
> avaliadas por profissional habilitado quando necessário.

## 2. Estado atual

- O repositório é público no GitHub.
- Não há arquivo `LICENSE`, `LICENSE.md` ou `COPYING`.
- O `Cargo.toml` não possui campo `license`.
- Na ausência de licença expressa, aplicam-se os direitos autorais padrão.

Portanto, este repositório não deve ser apresentado como *open source*. Sua
publicação permite visualização e as funcionalidades próprias oferecidas pelo GitHub,
mas não concede ao público autorização geral para copiar, modificar, redistribuir,
sublicenciar ou explorar comercialmente o código.

## 3. Proveniência do código-base

O projeto teve origem no desafio didático da DIO associado ao repositório:

<https://github.com/digitalinnovationone/rust-fullstack-carteira-investimentos>

A versão inicial do `wallet-live` continuou a implementação construída no módulo final
do curso, preservando parte de sua estrutura, nomenclatura, rotas, modelos e decisões
técnicas. O sistema foi posteriormente ampliado e transformado de maneira substancial.

Não se trata, portanto, de coincidência conceitual nem de inspiração genérica: o ponto
de partida foi código-base preexistente, e as correspondências abaixo são
identificáveis no histórico.

## 4. Correspondências verificadas no histórico inicial

O histórico inicial do `wallet-live` contém correspondências identificáveis com esse
projeto-base, entre elas:

- organização inicial em `app`, `auth`, `error`, `models`, `routes` e `repository`;
- estruturas `App` e `AppState` e o método `App::start`;
- rota `/assets`;
- handlers `list_assets`, `create_asset` e `update_asset`;
- estruturas `CreateAssetRequest` e `UpdateAssetRequest`;
- modelo `Asset` com `id`, `name` e `unit_value`;
- autenticação administrativa por extrator `Admin`;
- erro `AssetDoesNotExist`;
- uso inicial de `f64` para valores monetários.

## 5. Evolução substancial posterior

Depois desse ponto de partida, o projeto recebeu evolução substancial, incluindo
valores monetários com `Decimal`, holdings materializados, livro-razão, compra e
venda, custo médio, autenticação avançada, refresh token rotativo, CSRF, lockout,
integrações externas, observabilidade, testes, internacionalização e documentação
arquitetural.

## 6. Distinção entre código-base e contribuições próprias

Em princípio, o autor detém os direitos sobre suas contribuições originais,
ressalvadas eventual cessão de direitos, situação de coautoria ou relação contratual
que disponha de forma diferente. A possibilidade de licenciar o repositório inteiro
depende também dos direitos aplicáveis ao código-base preexistente.

A classificação documental adequada para o conjunto é:

> Projeto derivado e substancialmente transformado, contendo extensa contribuição
> autoral própria.

O repositório não é uma mera cópia do projeto didático, pois seu comportamento,
modelo de dados, segurança, integrações e documentação foram ampliados de forma
substancial. Ao mesmo tempo, o histórico inicial documenta elementos derivados do
upstream; por isso, o projeto não deve ser descrito como implementação integralmente
independente.

## 7. Termos da DIO

Os Termos de Uso da DIO distinguem informações ou conteúdo original do usuário do
conteúdo e da plataforma mantidos pela DIO. Essa distinção não torna a DIO
automaticamente proprietária de todo código original escrito pelo participante.

Ela também não autoriza o participante a sublicenciar código preexistente fornecido
pela própria DIO. A propriedade sobre contribuições originais e a autorização para
usar, modificar, distribuir ou sublicenciar o upstream são questões diferentes.

Assim, a declaração da DIO de que não reivindica automaticamente a propriedade das
informações do usuário não resolve, sozinha, a proveniência ou o relicenciamento dos
componentes derivados do projeto-base.

## 8. Cláusula 10.1 do edital Santander Bootcamp 2026

A redação literal da cláusula 10.1 refere-se a fotografias, comentários, informações,
textos, vídeos, feedback, ideias criativas, sugestões e outros materiais submetidos ao
Processo de Seleção como parte da inscrição.

A cláusula concede ao Santander e à DIO autorização gratuita, irrevogável e
irretratável para as utilizações promocionais descritas no edital, pelo prazo de dois
anos, empregando também expressões relacionadas a exclusividade, cessão e
sublicenciamento. O edital preserva a possibilidade de manutenção histórica de
materiais já utilizados.

O texto não identifica expressamente código-fonte entregue posteriormente durante a
fase educacional do bootcamp. Portanto, não é possível afirmar apenas com base nessa
cláusula que o repositório do projeto esteja incluído ou excluído de seu alcance.

Essa autorização:

- não constitui licença *open source* para o público;
- não concede ao participante direito de sublicenciar código-base da DIO;
- não transfere expressamente a titularidade integral do software;
- não resolve a proveniência dos componentes derivados;
- deve ser interpretada dentro das finalidades e do contexto descritos no edital.

## 9. Restrições relacionadas a marcas e conteúdo exclusivo

O edital também contém restrições relativas ao uso das marcas Santander e DIO e à
divulgação de conteúdo exclusivo de aulas, avaliações e materiais internos. Por isso,
marcas, logotipos e materiais exclusivos não devem ser incorporados ao repositório
sem autorização.

## 10. Repositório público não significa open source

Um repositório público não é automaticamente *open source*. A visibilidade pública e
o botão de *fork* decorrem dos recursos e termos da plataforma, mas não equivalem a
uma autorização geral de sublicenciamento.

Na ausência de licença expressa, os direitos autorais permanecem reservados. Um
terceiro não deve inferir permissão para redistribuir o projeto sob MIT, Apache-2.0 ou
qualquer outra licença apenas porque o código pode ser visualizado ou bifurcado no
GitHub.

## 11. Dependências como segunda camada da análise

A auditoria das dependências com `cargo-license` e `cargo-deny` continua necessária,
mas é independente e posterior à análise de proveniência do código principal. Ela não
é o único ponto pendente.

As verificações recomendadas são:

```bash
cargo install cargo-license
cargo license --tsv
```

```bash
cargo install cargo-deny
cargo deny check licenses
```

Os resultados devem ser revisados quanto a licenças incompatíveis, obrigações de
atribuição, copyleft e dependências sem licença declarada. Um resultado compatível
nessa auditoria não resolve, por si só, a autorização sobre o upstream.

## 12. Pendências atuais

| Questão | Estado | Tratamento necessário |
| --- | --- | --- |
| Autorização ou reescrita do código-base | **Pendente** | Obter autorização expressa da DIO ou substituir os trechos derivados por implementação independente |
| Termo específico do bootcamp | **Pendente** | Localizar e revisar os termos aceitos na inscrição |
| Licenças das dependências | **Pendente** | Executar e revisar `cargo-license` e `cargo-deny` |
| Vínculo empregatício | **Confirmação exclusiva do autor** | Verificar se alguma relação contratual afeta contribuições originais |
| Marcas e materiais DIO/Santander | **Uso restrito** | Não incorporar marcas, logotipos ou conteúdo exclusivo sem autorização |

Entre as pendências atuais, existem duas frentes técnicas centrais: a proveniência e
a autorização do código-base e a compatibilidade das licenças das dependências. Elas
não afastam as verificações adicionais relacionadas ao termo específico do bootcamp,
ao eventual vínculo contratual e ao uso de marcas ou materiais exclusivos.

## 13. Opções futuras

1. **Manter o repositório sem licença.** Preserva o estado de direitos reservados
   enquanto as pendências são investigadas.
2. **Solicitar autorização formal à DIO.** A autorização deve cobrir uso,
   modificação, distribuição e, se pretendido, sublicenciamento do código-base.
3. **Reescrever os componentes derivados.** A substituição deve ser independente e
   documentada, sem copiar expressão protegida do upstream.
4. **Separar uma implementação nova e independente.** Uma nova base pode facilitar a
   delimitação de proveniência, desde que a independência seja real e auditável.
5. **Consultar profissional jurídico.** Recomendado antes de exploração comercial
   relevante ou quando os documentos disponíveis não forem suficientes.

## 14. Recomendação atual: manter sem LICENSE

Até a resolução da proveniência, recomenda-se:

- manter o projeto sem arquivo `LICENSE`;
- não adicionar o campo `license` ao `Cargo.toml`;
- não apresentar o repositório como *open source*;
- preservar o histórico Git;
- registrar corretamente sua origem acadêmica e derivada;
- executar a auditoria das licenças das dependências;
- solicitar confirmação escrita à DIO sobre os direitos de uso, modificação,
  distribuição e sublicenciamento do código-base;
- considerar a reescrita dos componentes derivados caso a autorização não seja
  obtida.

Não há base documental suficiente, neste momento, para aplicar MIT, Apache-2.0 ou
`MIT OR Apache-2.0` ao repositório inteiro.

## 15. Critérios futuros para escolha de licença

Uma licença para o repositório inteiro somente deve ser escolhida depois que a
proveniência e a autorização do código-base estiverem resolvidas e a compatibilidade
das dependências tiver sido auditada.

Nesse cenário futuro:

- MIT pode ser avaliada pela simplicidade;
- Apache-2.0 pode ser avaliada pela concessão expressa de patentes;
- `MIT OR Apache-2.0` pode ser considerada pela convenção do ecossistema Rust.

Essas são opções de avaliação futura, não licenças atualmente autorizadas para o
projeto.

## 16. Fontes documentais consultadas

As fontes abaixo sustentam as afirmações deste documento. Nenhuma delas é reproduzida
aqui em trechos extensos — apenas os pontos relevantes são resumidos.

| Fonte | Versão ou data | Pontos relevantes |
| --- | --- | --- |
| Termos de Uso da DIO | Atualizados em 05/09/2025; consultados em 31/07/2026 | Informações do usuário, definição de Conteúdo, restrições de reprodução e direitos da plataforma |
| Edital Santander Bootcamp 2026 — Rust AI Developer | Edital do programa consultado pelo autor | Cláusulas 5.22, 8.4, 10.1, 10.4–10.7 e 12.10 |
| Repositório-base da DIO (`digitalinnovationone/rust-fullstack-carteira-investimentos`) | Estado público consultado na data da análise | Estrutura e código-base sem licença *open source* expressa identificada |
| Histórico Git do `wallet-live` | Do primeiro commit até a branch atual | Proveniência inicial e evolução posterior |

Limites desta seção: não foi obtida manifestação escrita da DIO ou do Santander sobre
o caso concreto, e nenhuma das fontes acima decide, isoladamente, a questão do
sublicenciamento do código-base.

## 17. Ferramentas de IA no desenvolvimento

Ferramentas de IA foram utilizadas como apoio ao desenvolvimento, incluindo pesquisa,
geração assistida, revisão, refatoração, testes e documentação.

As decisões de produto, seleção das sugestões, integração, revisão e validação
permaneceram sob responsabilidade do mantenedor humano.

Referências a ferramentas de IA ou trailers `Co-Authored-By` no histórico Git
registram assistência técnica e não constituem, por si só, conclusão jurídica sobre
autoria, coautoria ou titularidade.

O registro dos aprendizados associados a esse processo está em
[../aprendizado/README.md](../aprendizado/README.md).

## Evidências e limites da análise

```text
- ausência de LICENSE, LICENSE.md e COPYING
- Cargo.toml sem campo license
- histórico Git e primeiro commit do wallet-live
- upstream digitalinnovationone/rust-fullstack-carteira-investimentos
- evolução posterior registrada no código, migrations, testes e documentação
- Termos de Uso da DIO
- edital Santander Bootcamp 2026, inclusive cláusula 10.1
- uso de ferramentas de IA como apoio, registrado na seção 17
```

Esta análise deve ser atualizada se surgir autorização escrita da DIO, termo
específico do bootcamp, reescrita dos componentes derivados ou inventário conclusivo
das licenças das dependências.
