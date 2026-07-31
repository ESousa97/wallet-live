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

O autor detém os direitos sobre suas contribuições originais. A possibilidade de
licenciar o repositório inteiro depende também dos direitos aplicáveis ao código-base
preexistente.

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

O edital não transfere expressamente à DIO ou ao Santander a titularidade integral do
software criado pelo participante. Entretanto, a cláusula 10.1 concede autorização
ampla de uso de textos, comentários, ideias e outros materiais submetidos ao processo
para as finalidades previstas no programa.

Essa autorização não constitui uma licença *open source* para o público e não resolve
o direito de sublicenciamento do código-base.

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

Permanecem, portanto, duas frentes independentes: a proveniência e autorização do
código-base e a compatibilidade das licenças das dependências.

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

## Evidências e limites da análise

```text
- ausência de LICENSE, LICENSE.md e COPYING
- Cargo.toml sem campo license
- histórico Git e primeiro commit do wallet-live
- upstream digitalinnovationone/rust-fullstack-carteira-investimentos
- evolução posterior registrada no código, migrations, testes e documentação
- Termos de Uso da DIO
- edital Santander Bootcamp 2026, inclusive cláusula 10.1
```

Esta análise deve ser atualizada se surgir autorização escrita da DIO, termo
específico do bootcamp, reescrita dos componentes derivados ou inventário conclusivo
das licenças das dependências.
