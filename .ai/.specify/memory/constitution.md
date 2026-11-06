# Constituição do Project Depot

## Princípios fundamentais

### I. Caminho próprio

O Project Depot é mantido de forma independente e segue o próprio caminho.

- O Depot NÃO acompanha, não mescla e não faz rebase sobre o openOMSI, e não envia mudanças de
  volta para ele. Compatibilidade com o openOMSI não é restrição de projeto: mover, renomear ou
  dividir código herdado não conta como custo.
- A "integração seletiva" de `DEPOT_BASELINE.md` vale dentro do próprio Depot: cada mudança é
  revisada e integrada individualmente, nunca em bloco.
- O histórico do baseline (openOMSI `v0.2.16`) é mantido para atribuição, e a licença MIT e os
  avisos de copyright originais ficam intactos.

Justificativa: o projeto decidiu evoluir a física, a simulação e a interface sem amarrar cada
mudança à base de onde veio.

### II. Fidelidade ao OMSI 2 só com validação

O objetivo é compatibilidade 1:1 com o OMSI 2, mas fidelidade é uma afirmação que precisa de prova.

- Um comportamento só é descrito como igual ao do OMSI 2 quando há um teste caixa-preta que o
  demonstra ou uma verificação no jogo feita pelo mantenedor. Sem isso, o texto descreve apenas o
  que o Depot faz e aponta a validação pendente (issue #49).
- Uma mudança NÃO pode quebrar um mapa original ou um mod que funcionava. Mudanças maiores rodam
  `omsi-check` sobre uma instalação do OMSI 2 antes e depois.
- Uma mudança que altera de propósito a física do veículo ou o estado do jogo leva o rótulo
  correspondente (`physics-delta`, `offscreen-delta`) e uma justificativa pública; sem o rótulo,
  os traços de física e de estado têm de sair idênticos.

Justificativa: "como o OMSI 2 faz" sem prova já levou a regras erradas que depois tiveram de ser
desfeitas; a prova é o que separa compatibilidade de suposição.

### III. O jogo não compensa conteúdo mal configurado

O Depot lê o conteúdo como ele é declarado e mostra os erros dele.

- O jogo NÃO corrige, em tempo de execução, animações 3D, modelos ou configurações de veículos mal
  feitos (por exemplo, recolocar rodas que o modelo tirou do lugar ou aceitar nomes que o OMSI 2
  não aceitaria). O erro aparece como está, e quem corrige é o modelo.
- Valores ausentes usam apenas os padrões do próprio OMSI 2, sem mínimos nem valores inventados.
  Padrões antigos mantidos atrás de feature (como `model-fallbacks`) são provisórios e saem quando
  a física for validada no jogo.

Justificativa: compensar um modelo errado esconde o defeito do autor do mod e afasta o Depot do
comportamento do OMSI 2.

### IV. Domínios organizados em pacotes

O código se organiza por domínio e, quando couber, por subdomínio. Cada domínio é um pacote com
três partes:

- `domain/`: os objetos do domínio (structs, enums, estados, constantes e seus acessores);
- `interface/`: as portas, com `input.rs` (o que os outros domínios chamam) e `output.rs` (o que o
  domínio pede de fora);
- `implementation/`: os procedimentos, um módulo por subdomínio.

Regras do pacote:

- `implementation/` só fala com outro domínio pela `interface/input.rs` dele e só alcança o que é
  de fora pela própria `interface/output.rs`.
- `domain/` NÃO importa `implementation/` nem `interface/`.
- O resto do crate importa o domínio só pelo que o `mod.rs` reexporta.
- Trait na porta de saída só onde houver duas implementações de verdade; nos outros casos
  `output.rs` reexporta ou embrulha os tipos concretos.
- Um domínio pequeno pode usar `domain.rs` e `implementation.rs` no lugar de pastas, mas mantém as
  três partes.

Justificativa: fronteiras explícitas deixam cada domínio ser refatorado, testado e substituído sem
arrastar o resto do jogo.

### V. Testes fora de `src/`

- Os testes ficam na pasta `tests/` de cada crate, NUNCA em `src/`. O que um teste precisa de
  dentro do crate vem do módulo `testing`, atrás da feature `test-support`. Um crate que ainda tem
  testes em `src/` os move para fora quando for refatorado.
- Testes que precisam de conteúdo do OMSI 2 o leem de uma instalação local (`OMSI_ROOT`) e se pulam
  sozinhos sem ela; os dados de teste do repositório são pequenos e feitos para o teste.
- `cargo test --workspace` e `cargo build --release` passam em todo PR.

Justificativa: tirar os testes de `src/` mantém o código de produção limpo e obriga cada crate a
expor uma superfície de teste explícita.

### VI. Proveniência e texto público

- NADA de uma instalação do OMSI 2 entra no repositório: texturas, modelos, sons, mapas, scripts.
- Texto público (código, comentários, commits, PRs, issues, `CHANGELOG.md` e documentação) NÃO
  traz endereços, nomes de API internas nem limites derivados do executável de referência. Achados
  de análise estática podem orientar o trabalho, mas o texto público descreve só o comportamento
  observado e o teste que o demonstra.
- Commits e PRs NÃO creditam agentes de IA como autores e NÃO trazem links para projetos ou sessões
  privadas de assistentes.
- Arquivos de terceiros mantêm a própria licença; uma licença nova precisa da concordância do
  mantenedor antes de entrar.

Justificativa: o repositório é público e MIT; o que entra nele precisa poder ser publicado e
atribuído corretamente.

### VII. Mudanças pequenas e rastreáveis

- Um PR faz uma mudança só, com uma descrição que diz o que o jogador percebe.
- Toda branch usa um dos três prefixos, conforme o que contém:
  - `feat/<nome>`: funcionalidades novas ou refatorações (pode ter código, configuração e
    documentação);
  - `fix/<nome>`: só correções de problemas em funcionalidades existentes (e a documentação delas);
  - `docs/<nome>`: só documentação, sem código nem configuração.
  Uma branch que extrapola o prefixo é renomeada ou divide o trabalho extra numa branch própria.
- Refatoração cede a correções abertas: PRs de refatoração são curtos, quem move código porta as
  branches abertas afetadas, e formatação entra crate a crate.
- Um PR que muda o que o jogador percebe adiciona uma linha em **Unreleased** no `CHANGELOG.md`.

Justificativa: PRs curtos e de um assunto só são revisáveis, revertíveis e convivem com o trabalho
de vários colaboradores em paralelo.

## Restrições do projeto

- **Plataforma**: Windows x64 é a única plataforma suportada; a CI compila e testa nela.
- **Linguagem e estilo**: Rust; `rustfmt` com 100 colunas; comentários explicam o porquê. O
  `rustfmt` bloqueia nos crates já formatados; `clippy`, `cargo audit` e `cargo deny` são
  informativos até o backlog de `docs/QUALITY.md` ser zerado.
- **Dependências**: `Cargo.lock` fica fixo; atualizar dependências é trabalho separado.
- **Conteúdo não confiável**: os parsers leem conteúdo de terceiros; o perfil de release mantém
  `overflow-checks = true`.
- **Arquivos**: nenhum arquivo de 1 MB ou mais, nada gerado ou baixado (builds, caches, texturas
  convertidas, mapas de exemplo); imagens só as da documentação, em `docs/images/`.
- **Código sem uso**: código que ficou inalcançável NÃO é apagado por padrão; sai da compilação
  (feature ou `cfg`) e fica na árvore para ser readaptado. Apagar exige decisão explícita do
  mantenedor.
- **Releases**: feitas à mão pelos mantenedores, numeradas `MAJOR.MINOR.PATCH`
  (`docs/VERSIONING.md`). A CI pode compilar e testar, mas NÃO publica releases, tags, sites nem
  pacotes.

## Fluxo de desenvolvimento

- O detalhe operacional (branches, commits, formatação, testes, traduções, issues) está em
  `CONTRIBUTING.md`; as instruções para agentes de IA estão em `AGENTS.md` e as skills em
  `.ai/skills/`.
- Funcionalidades especificadas com o Spec Kit ficam em `.ai/specs/`. O `/speckit-plan` confere
  cada plano contra esta constituição (Constitution Check) antes do desenho e de novo depois dele;
  um desvio só entra registrado em Complexity Tracking com a justificativa e a alternativa
  rejeitada.
- Issues são escritas em inglês. Problemas de segurança seguem `SECURITY.md`, não issues.
- Cada PR passa pela CI (build, testes, traços de física e de estado) e pela revisão de um
  mantenedor antes do merge.

## Governança

- Esta constituição prevalece sobre outras práticas do projeto. Quando um documento ou plano a
  contradiz, a constituição vale até que seja emendada.
- Emendas são feitas por PR `docs/` que altera este arquivo, aprovado pelo mantenedor (Zane). O PR
  traz o relatório de impacto: versão antiga e nova, princípios alterados e o que mais precisa
  mudar (templates, `CONTRIBUTING.md`, `AGENTS.md`).
- Versionamento semântico da constituição:
  - MAJOR: remoção ou redefinição incompatível de um princípio ou regra de governança;
  - MINOR: princípio ou seção nova, ou orientação ampliada de forma material;
  - PATCH: esclarecimento, redação ou correção sem mudança de sentido.
- Revisão de conformidade: todo PR e toda revisão verificam os princípios acima; complexidade que
  os contraria precisa ser justificada no PR.

**Versão**: 1.0.0 | **Ratificada**: 2026-11-06 | **Última emenda**: 2026-11-06
