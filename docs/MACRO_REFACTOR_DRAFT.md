# Macro-refatoração do depot (rascunho)

Rascunho de 10/10/2026, revisto em 11/10/2026 sobre o `main` em `0a38bf4`.

## Resumo

O depot já tem as redes de segurança que o plano pedia, mas a estrutura do código ainda não mudou fora da física. Desde o rascunho entraram o CI próprio, o teste golden do offscreen, o harness de traços da física e a divisão de `rigid.rs` em módulos. Enquanto isso, o `omsi-app` continua com 124,8 mil das 227,8 mil linhas de Rust (55%), e as três funções gigantes estão do mesmo tamanho.

Escopo desta revisão: `main` em `0a38bf4` (PR #60), comparado com o rascunho original (`a521843`) e alinhado ao mapa do projeto de 11/10/2026. As medidas de tamanho são de `0ee5f9b` (PR #27); os PRs seguintes mexeram pouco fora da física, e a contagem que mudou foi a de variáveis `OMSI_*` (297 em 79 arquivos). Só leitura de código. Contagens de função são aproximadas (casamento de chaves).

Recomendações revistas, na ordem em que devem acontecer:

1. **A física terminou, e a refatoração anda em paralelo à paridade e à triagem.** O plano de física concluiu as Fases 0 a 4. Por decisão do mantenedor (11/10/2026), a refatoração não espera as fases 1 e 2 do mapa do projeto: ela se divide em unidades (crate ou grupo de módulos), cede a qualquer correção aberta e só move código em PRs curtos.
2. **Tirar o código desligado da compilação do jogo, sem apagá-lo.** Multiplayer, servidor dedicado, voz e plugins estão inalcançáveis desde o PR #1, mas cerca de 23 mil linhas continuam compiladas (`omsi-net`, `omsi-plugin` e 10,7 mil linhas do app). Por decisão do mantenedor, esse código fica no repositório para ser readaptado mais tarde; o plano o põe atrás de features de Cargo desligadas por padrão.
3. **Formatar crate por crate** e tornar o `fmt --check` bloqueante nos crates já formatados. Um commit único de rustfmt quebraria toda branch aberta de paridade e triagem.
4. **Unificar o passo de simulação** entre janela e offscreen num `Session::step`. A divergência conhecida (`signal_aspects`) já está fixada por teste.
5. **Quebrar `omsi-render/src/lib.rs` e as funções gigantes** em módulos e etapas nomeadas, sem mudar ordem nem lógica.
6. **Registrar as 297 variáveis `OMSI_*`** num registro tipado, partindo do padrão que já existe no código: a lista `ALLOWED` de `env_file.rs`. O outro modelo, `rigid/toggles.rs`, saiu com os switches A/B no PR #32.

O depot segue caminho próprio, sem integrar com o openOMSI, então mover arquivos e dividir crates não tem custo de divergência. A "integração seletiva" do `DEPOT_BASELINE.md` vale para o próprio Depot: cada mudança revista e integrada individualmente, o que combina com PRs pequenos de refatoração.

## O que já mudou desde o rascunho

Dos 32 PRs que entraram, onze adiantam este plano diretamente; os outros são correções, site e limpeza de comentários. Do mais recente ao mais antigo:

| PR | O que trouxe | Efeito no plano |
| --- | --- | --- |
| #32 | Remoção dos três switches A/B da física (`OMSI_TYRE_SUSPENSION`, `OMSI_NO_WHEEL_SLIP`, `OMSI_KINEMATIC_TRAILERS`), com registro em `docs/removed/PHYSICS_AB_SWITCHES_REMOVED.md` | Fase 2 da física concluída; `rigid/toggles.rs` deixou de existir. Mostra o modelo de limpeza que o registro de flags (D2) pode repetir nos `DEBUG_*` |
| #31 | `enum Dynamics { Rigid, Kinematic }` no veículo | Fronteira única entre veículo e modelo físico, aberta a uma variante para a IA |
| #30 | `vehicle/physics_bridge.rs` e os cenários B5 e B6 no harness | O app fala com a física por um só módulo |
| #29 | Snapshot `VehicleMotion` e acessores; 23 leitores do `omsi-app` migrados | É a interface que o `Session::step` (Frente B) vai consumir |
| #27 | `rigid.rs` dividido em `rigid/{params, toggles, ground, wheel, suspension, tyre, body, collide}.rs`, todos abaixo de 800 linhas; 78 constantes nomeadas em `params.rs` | Fase 1 do plano de física concluída, com 88 traços idênticos bit a bit |
| #26 | Remoção do reajuste das rodas desenhadas (`seat_wheels`) | Firma o princípio de não compensar modelos mal configurados; menos código especial em `omsi-sim` |
| #15, #17, #19, #20, #21, #24 | Comentários e docs descrevem comportamento observável, sem identificadores do executável original | Regra de redação que vale também para este documento e para a cópia em `docs/` |
| #16 | Harness de traços da física (22 cenários), invariantes e os jobs `physics-traces` (bloqueante) e `physics-coverage` | Fase 0 do plano de física; cobre `rigid/` com 91% das linhas |
| #14 | Teste golden do offscreen sobre uma raiz sintética, mais um teste que fixa a divergência de `signal_aspects` | Primeiro teste de caracterização fora da física. As imagens só rodam com `OMSI_GOLDEN_RENDER=1` e um adaptador gráfico, ainda não no CI |
| #9 | Padrão de branches `feat/`, `fix/`, `docs/` no CONTRIBUTING | Cada frente deste plano vira branches `feat/` (refatoração conta como feature) |
| #8 | Articulação multicorpo: `rigid/train.rs` e `vehicle/articulation.rs` | O pré-requisito da antiga Fase 2 foi cumprido |
| #1 | CI do Depot (`depot-ci.yml`), `docs/QUALITY.md`, `deny.toml`; multiplayer, servidor, plugins, updater e Discord desativados | Guarda-corpos parciais: build e testes bloqueiam, fmt/clippy/audit/deny são informativos. Deixou o código desligado ainda compilado |

## Panorama atual

O workspace no main tem cerca de 227,8 mil linhas de Rust em 21 crates. O `omsi-app` sozinho soma 124,8 mil, ou 55% do total, e mistura gameplay, mundo, entrada, launcher, clima e o laço de eventos num único crate. Os crates de runtime somam cerca de 77,5 mil linhas, das quais o `omsi-sim` é o maior (34,4 mil). Os crates de formato (\~23,8 mil) estão saudáveis e não são alvo desta refatoração.

O dado novo em relação ao rascunho anterior é o código desligado: LAN, servidor e plugins foram desligados no PR #1, mas continuam compilando. São cerca de 23 mil linhas (10,7 mil dentro do `omsi-app`, mais `omsi-net` e `omsi-plugin`) que entram em cada build, em cada aviso do clippy e em cada busca no código, sem rodar nunca. Esse código fica no repositório para ser readaptado mais tarde; o plano só o tira da compilação do jogo (Frente A1).

| Camada | Linhas de Rust |
| --- | --- |
| `omsi-app` | 124.800 (gameplay 29.200; mundo 17.900; entrada 13.500; launcher 12.800; LAN, servidor e plugins 10.700, desligados; UI 7.300; clima 3.000; laço, eventos, VR e o resto 30.100) |
| Crates de runtime | \~77.500 (`omsi-sim` 34.400; `omsi-render` 20.000; `omsi-net` 9.200, desligado; `omsi-launcher-core` 6.000; `omsi-plugin` 3.300, desligado; `omsi-audio` e `omsi-ui` 4.600) |
| Crates de formato | \~23.800 |

## Code smells, remedidos

Fora de `omsi-sim/src/rigid/`, quase todos os números ficaram iguais ou pioraram um pouco; o único smell novo é o código morto deixado pelo PR #1. A coluna "Antes" é o rascunho em `a521843`; "Agora" é `0ee5f9b`.

| Smell | Antes | Agora | Evidência atual |
| --- | --- | --- | --- |
| Código desligado compilado | não havia | \~23.200 linhas | `omsi-net` (9.204), `omsi-plugin` (3.281) e no app `lan.rs` 3.947, `lan_world.rs` 1.554, `plugin_ui.rs` 1.488, `voice.rs` 981, `lan_mods.rs` 923, `admin.rs` 840, `server.rs` 389, `plugins.rs` 356, `launcher/multiplayer.rs` 258. Inalcançáveis segundo `docs/removed/MULTIPLAYER_DISABLED.md`; ficam no repositório por decisão do mantenedor |
| Crate monolítico | 124.713 linhas | 124.842 | `omsi-app` ainda depende de 18 crates internos; o Navigator novo entrou nele |
| Arquivos gigantes | 6 acima de 6.000 | 5 | `omsi-render/src/lib.rs` 15.830; `scene.rs` 14.863; `traffic.rs` 8.311; `schedule.rs` 6.890; `humans.rs` 6.843. Saiu da lista `rigid.rs` (dividido) e `vehicle.rs` caiu para 5.821 |
| Funções gigantes | 40 acima de 300 linhas | 40 | `Renderer::new_on` \~5.500 (lib.rs:2490), `window_event` \~3.000 (app\_events.rs:67), `run_offscreen` \~3.000 (offscreen.rs:5), `render_inner` \~2.400 (lib.rs:8673), `Scene::build` \~2.000 (lib.rs:2845), `place_step` \~1.250 (scene.rs:6764), `Traffic::tick` \~1.180 (traffic.rs:5220) |
| Laço de frame duplicado | divergente | divergente, fixado | `signal_aspects` recebe o trilho do jogador na janela (app\_events.rs:629) e `None` no offscreen (offscreen.rs:638). O teste `window_and_offscreen_differ_in_the_rail_given_to_signal_aspects` falha se qualquer um mudar |
| God objects | `App` \~190 campos | \~186 | `Settings` \~139, `Renderer` \~129, `Humans` \~100, `Traffic` \~78, `World` \~76, `Args` \~73 |
| Métodos do `App` em arquivos com outro nome | não medido | \~10.800 linhas em 10 arquivos | `input_script.rs` tem 4.393 linhas de `impl App` (teclado, mouse, menus, clima, LAN, voz), embora se descreva como o roteiro `OMSI_INPUT`. Ver C3 |
| Simulação acoplada ao renderer | igual | igual | `traffic.rs` 34 menções a `Renderer`/`Scene`, `humans.rs` 30, `schedule.rs` 12 |
| Variáveis `OMSI_*` | \~297 em 77 arquivos | 297 em 79 | 127 são ganchos `DEBUG`/`FAKE`/`NO`/`TEST`; \~120 aparecem em `docs/`. Duas listas manuais descrevem subconjuntos: `env_file.rs::ALLOWED` (o que o `env.txt` do Android pode definir) e a documentação |
| Namespace achatado | 16 globs no lib.rs, 32 `use super::*` | 17 e 33 | `vehicle/articulation.rs` entrou com `use super::*` |
| Instrumentação manual | 28 + 29 | 28 + 29 | `let __tN = Instant::now()` e `self.profile.entry("…")` em `app_events.rs` |
| Settings com parser manual | \~137 campos | \~139 | `Settings::from_text` (settings.rs:432), com um segundo parser no launcher (ver D1) |
| Launcher-core com JSON solto | 223 | 227 | `serde_json::Value`/`json!` em `omsi-launcher-core/src/lib.rs` |
| Nomes legados | 3 | 2 | lib `omsi_launcher_lib` importada como `core`; `#[path = "humans_pax.rs"]` (humans.rs:57). Os `#[path]` de testes em `scene.rs` são aceitáveis |
| `unwrap` fora de testes | \~1.350 | \~1.290 | O PR #1 trocou parte deles por limites e erros nos parsers |
| Lints silenciados | 47 `too_many_arguments` | 51 | Sintoma dos god objects |
| Formatação | 8.151 trechos | \~8.000 | `cargo fmt --check` informativo no CI; clippy com \~550 avisos (`docs/QUALITY.md`) |

Continuam saudáveis: o grafo entre os crates de formato, as dependências externas centralizadas em `[workspace.dependencies]`, e agora também `omsi-sim/src/rigid/`, que virou o exemplo de módulo bem dividido do repositório.

## Frente A: guarda-corpos e código desligado

Esta frente é barata e destrava as outras: tira 23 mil linhas da compilação do jogo, sem apagar nenhuma, e deixa o resto do código formatado antes de qualquer movimentação grande.

### A1. Multiplayer, servidor e plugins fora da compilação do jogo

Decisão do mantenedor (10/10/2026): LAN, servidor dedicado, voz e plugins não saem do projeto, porque serão reaproveitados e readaptados mais tarde. Esta frente só os tira da compilação do jogo; nenhum arquivo é apagado.

O PR #1 desligou essas funções no ponto de entrada (`lan::start` devolve `None`, `--server` termina com erro, `plugins::load()` devolve vazio), mas o código continua compilado e entrelaçado com o laço do jogo. Medido no main atual:

- **Nove arquivos do app existem só para isso**, com 10,7 mil linhas: `lan.rs`, `lan_world.rs`, `lan_mods.rs`, `admin.rs`, `server.rs`, `voice.rs`, `plugins.rs`, `plugin_ui.rs` e `launcher/multiplayer.rs`.
- **O código vivo toca neles em cerca de 100 pontos.** O campo `lan` do `App` é lido em nove arquivos (34 vezes em `input_script.rs`, 16 em `offscreen.rs`, 10 em `app_events.rs`, 9 em `game_lists.rs`), quase sempre como `l.role == omsi_net::Role::Client` ou `Host`. Os eventos de plugin aparecem 14 vezes em `app_events.rs`, e `voice::` e `server::` são chamados de `input_script.rs`, `lib.rs` e `offscreen.rs`.
- **Dois utilitários vivos moram no crate de rede.** `omsi_net::human_path` resolve o caminho de uma figura humana para `driver.rs:402` e `on_foot.rs:1191`, e `omsi_net::describe_join` é usado por `omsi-launcher-core` (lib.rs:2703 e :2747). Sem movê-los, desligar `omsi-net` quebra o jogo.
- **`run_offscreen` é também o laço do servidor dedicado.** Ele recebe `lan_off: Option<LanSession>`, lê `server::SERVER_METAR` e `SERVER_ADMIN` e chama `server::enforce_vehicles`.

Desenho proposto:

- Três features de Cargo no `omsi-app`, desligadas por padrão: `lan` (LAN, voz, admin e mods da sessão), `server` (implica `lan`) e `plugins`. `omsi-net` e `omsi-plugin` viram dependências `optional` e continuam membros do workspace.
- Cada módulo desligado recebe `#[cfg(feature = "…")]` no `lib.rs`. Os campos espalhados do `App` (`lan`, `remotes`, `voice`, `plugins`, `plugin_events`, `plugin_panels`, `remote_walkers`, `inside_remote`, `remote_added`, `is_admin`) vão para um sub-struct `Online` atrás da feature.
- O código vivo deixa de comparar `omsi_net::Role` e passa a perguntar `self.is_lan_client()` ou `self.is_lan_host()`, que devolvem `false` sem a feature. Isso troca cerca de 20 comparações espalhadas por uma chamada legível.
- `plugins::queue_event` ganha uma versão vazia e `#[inline]` quando `plugins` está desligada, então os 14 pontos de `app_events.rs` não mudam.
- No launcher, a consulta de servidores de `launcher/state.rs` e `launcher/phone.rs` vai para trás de `lan`, e `omsi-launcher-core` ganha a sua própria feature `lan` para `describe_join`.

Sequência, um PR `feat/` por passo:

1. **Mover os utilitários vivos.** `human_path` e o `relative_path` que ele usa saem de `omsi-net` para `omsi-content`; `omsi-net` passa a reexportá-los, para que a API dele não mude quando o multiplayer voltar.
2. **Plugins.** Feature `plugins`, `omsi-plugin` opcional, `queue_event` vazio, `plugin_ui.rs` e os campos de plugin atrás da feature.
3. **LAN e servidor.** Features `lan` e `server`, os métodos `is_lan_client`/`is_lan_host`, o sub-struct `Online`, `run_offscreen` recebendo o estado de servidor só com a feature e a feature `lan` do launcher-core.
4. **CI.** O job de build e testes continua na configuração padrão, que é o jogo. Um job a mais roda `cargo check --workspace --all-features` e os testes de `omsi-net` e `omsi-plugin`, para que o código guardado não quebre a cada refatoração do app. `scripts/build-server.sh` continua saindo com erro até o servidor ser readaptado; quando voltar, ele compila com `--features server`.

Como provar que nada mudou: o jogo já não alcança esse código, então build padrão, testes, golden, traços da física e `omsi-check` devem dar o mesmo resultado. Um passo do CI confere com `cargo tree -p omsi-app -e normal` que `omsi-net` e `omsi-plugin` não entram no binário padrão. O `docs/removed/MULTIPLAYER_DISABLED.md` ganha um parágrafo dizendo como religar (`--features lan,server,plugins`).

O ganho é que o binário do jogo deixa de compilar cerca de 23 mil linhas, o clippy da configuração padrão para de contar esse código e o passo único da Frente B nasce sem os ramos de LAN. O custo é manter duas configurações compilando, e o job `--all-features` cobre isso.

### A2. Formatação

1. Decidir a largura. Há cerca de 2.500 linhas acima de 140 colunas; se o estilo largo for desejado, um `rustfmt.toml` com `max_width = 140` e `use_small_heuristics = "Max"` reduz o diff. Recomendação: aceitar o padrão de 100 colunas, que é o que o CONTRIBUTING já promete.
2. Um PR de `cargo fmt -p <crate>` por crate, sem nenhuma outra mudança, começando pelos crates frios e deixando por último os quentes (`omsi-app`, `omsi-launcher-core`, `omsi-sim`). Esses entram num momento sem PRs abertos neles, ou com o `scripts/fmt-merge.sh` descrito nas regras de trabalho em paralelo. O rustfmt segue as declarações `mod` sem avaliar `cfg`, então os módulos atrás das features também saem formatados.
3. Adicionar `.git-blame-ignore-revs` com o hash desse commit.
4. Tirar o `continue-on-error` do passo de fmt em `depot-ci.yml` e atualizar a linha em `docs/QUALITY.md`.

O risco é baixo: rustfmt não muda semântica, e os traços da física e o golden confirmam isso.

### A3. Clippy por crate

Os \~550 avisos se concentram em poucos lints (`chunks_exact` com tamanho constante, tipos muito complexos, `is_multiple_of`, empréstimos e clones desnecessários, `too_many_arguments`). Ordem sugerida: crates de formato primeiro (`omsi-cfg`, `-o3d`, `-model`, `-map`, `-vehicle`, `-scenery`, `-texture`, `-timetable`, `-content`, `-script`), depois `omsi-geometry`, `omsi-audio`, `omsi-ui`, `omsi-sim` e por último `omsi-render` e `omsi-app`. Cada crate limpo ganha `#![deny(clippy::all)]` no `lib.rs`, até o workspace inteiro poder usar `-D warnings`. Os `too_many_arguments` ficam por último: eles somem com as Frentes B e C, não com edições locais.

### A4. Lints do workspace

Adicionar `[workspace.lints]` ao `Cargo.toml` raiz e `lints.workspace = true` em cada crate, com `clippy::unwrap_used = "warn"` só nos crates de formato, onde conteúdo de mod malformado não pode derrubar o processo. Nos outros crates, `unwrap` em invariantes internos é aceitável.

## Frente B: um único passo de simulação

O objetivo é que a janela e o offscreen executem exatamente o mesmo código de simulação, para que o golden e qualquer teste offscreen testem o jogo de verdade. Hoje o frame da janela é o braço `WindowEvent::RedrawRequested` de `window_event` (app\_events.rs:302, cerca de 2.700 linhas), e o offscreen repete a mesma orquestração dentro de `run_offscreen`.


### O que o frame faz hoje

As chaves de perfil de `app_events.rs` mostram a ordem real das etapas: `streaming`, `traffic.populate`, `traffic.schedule`, `traffic.tick`, `traffic.audio`, `traffic.sync`, `player.hover`, `player`, `lan`, `humans`, `lights.*`, `scripted.boards`, `hud.navigator`, `acquire`, `mirrors`, `render`, `gpu`, `present`, `limiter`. Tudo até `humans` é simulação; o resto é apresentação. O passo de horário do motorista (`schedule_paper::update_vehicle`) aparece em cinco lugares: `app.rs:850`, `app_events.rs:1554`, `offscreen.rs:114` e `:663`, `game_lists.rs:2924`.

### Desenho proposto

- `Session` possui o estado de simulação que hoje está solto no `App`: `clock`, `weather`, `envir`, `traffic`, `schedule`, `humans`, `player`, `duty`, `duty_places`, `world_day`, `career` e o `journey`.
- `StepInput` carrega o que vem de fora: `dt`, os controles já traduzidos (teclado, controle, mouse ou o roteiro do offscreen), `paused` e a posição da câmera para o streaming.
- `StepOutput` devolve os eventos do frame (parada atendida, troca de viagem, colisão) e os tempos por etapa, num `enum Stage` em vez de chaves de texto.
- Até a Frente E acontecer, o `step` ainda recebe `&Renderer` e `&mut Scene`, porque tráfego e pessoas criam e liberam veículos direto na cena. Isso é aceitável como passo intermediário.

### Sequência de PRs

1. **B0, traço de estado do offscreen.** Uma opção de teste que grava, por frame, posição e velocidade do jogador, contagem e posições dos carros de IA, aspectos dos sinais e o estado do duty, mais um script de comparação como o `physics-trace-diff.sh`. Não precisa de adaptador gráfico, então pode rodar no CI, ao contrário das imagens do golden.
2. **B1, passo do tráfego.** Extrair uma função única usada pelos dois lados, recebendo o trilho do jogador como parâmetro. A divergência de `signal_aspects` continua existindo, agora explícita numa chamada só; o teste que a fixa é atualizado.
3. **B2, passo do duty.** Os cinco usos de `schedule_paper::update_vehicle` e a lógica de `skip_to`, `trip_for_ibis` e `career.stop_served` viram uma função.
4. **B3, passo das pessoas.** Inclui `stop_wishes`, que hoje o offscreen passa ao tráfego manualmente (offscreen.rs:1027).
5. **B4, struct `Session`.** Os campos saem do `App` e as funções de B1 a B3 viram métodos chamados por `step`.
6. **B5, offscreen fino.** `run_offscreen` fica reduzido a: carregar o mundo, para cada frame aplicar o roteiro e chamar `session.step`, renderizar e salvar.
7. **B6, decidir `signal_aspects`.** Com tudo unificado, passar o trilho do jogador também no offscreen é uma linha; vira PR `fix/` com o traço mostrando a diferença esperada.

Cada PR de B1 a B5 deve deixar o traço de B0 idêntico. Com A1 feito antes, os ramos de LAN e de jogadores remotos (`lan_outlines`, `tick_lan`, o relógio do host) já estão atrás da feature `lan`; o `Session::step` recebe um único gancho opcional para eles, em vez de carregá-los espalhados por B1 e B4.

## Frente C: renderer e App

As duas partes são movimentação pura de código: nenhum algoritmo muda, só onde ele mora e como se chama. Podem andar em paralelo à Frente B, desde que não toquem o braço `RedrawRequested` ao mesmo tempo.

### C1. `omsi-render/src/lib.rs` em módulos

O arquivo tem 15.830 linhas, 343 funções e 73 blocos `struct`/`impl` de primeiro nível, mais 22 shaders `.wgsl` ao lado. A divisão segue os grupos que o próprio arquivo já tem:

| Módulo novo | O que vai para ele (linhas atuais aproximadas) |
| --- | --- |
| `gpu.rs` | Escolha de adaptador, memória de vídeo e caminhos de compatibilidade: `ADAPTER_*_MB`, `gl_backend`, `ArrayPath`, `GpuArray`, `wait_gpu`, `dedicated_vram_mb`, `vulkan_vram_mb` (1431 a 1990) |
| `camera.rs` | `Camera`, `Vertex`, `CameraUniform`, `ViewCulling` (11 a 80, 564 a 628) |
| `lighting.rs` | `Lighting`, `PointLight`, `Corona`, `LampShadow`, `LightMode`, a grade de luzes e as constantes de exposição (213 a 500, 629 a 828) |
| `material.rs` | `Material`, `MaterialExtra`, `MaterialMaps`, `PbrMaps`, `AlphaMode`, `TexAddressing`, `GpuTexture`, os layouts de bind group (503 a 560, 853 a 1072, 1590 a 1900) |
| `scene.rs` | `Scene`, `Instance`, `InstanceBounds`, `RenderPhase`, `Scene::build` e o culling por blocos (1073 a 1400, 2845 em diante) |
| `targets.rs` | `HdrTargets`, `AoTargets`, `Probe`, o cubo do céu (130 a 260) |
| `pipelines.rs` | `PostPipelines`, `PassPipelines` e a criação dos \~30 pipelines que hoje está dentro de `new_on` |
| `passes/` | Uma função por passada, extraída de `render_inner`: sombras, principal, reflexos e espelhos, pós-processamento, overlay |
| `renderer.rs` | O struct `Renderer` (\~129 campos), `RenderOptions` e a orquestração |

Para `Renderer::new_on` (\~5.500 linhas), o alvo é uma sequência curta: `open_device`, `Targets::new`, `Pipelines::new`, `Lighting::new`, `SkyResources::new`, `PostChain::new`, cada um devolvendo um struct que o `Renderer` compõe. Isso também agrupa os \~129 campos do `Renderer` por responsabilidade. Para `render_inner` (\~2.400 linhas), um `FrameContext` carrega o que hoje são variáveis locais compartilhadas entre as passadas.

Como provar: o golden do offscreen nas máquinas do mantenedor (o PR #14 já prevê rodar antes e depois de cada passo) e um ou dois cenários renderizados nos modos `vanilla` e `enhanced`. Ordem: primeiro mover tipos sem lógica (`camera`, `targets`, `material`), depois `gpu` e `lighting`, por último `pipelines` e `passes`, que exigem quebrar as funções.

### C2. `App` em sub-structs

Os \~186 campos do `App` (app.rs:7) caem em grupos claros pelo nome:

- **Sessão de simulação** (Frente B): `clock`, `weather`, `envir`, `traffic`, `schedule`, `humans`, `player`, `duty`, `duty_places`, `world_day`.
- **Janela e cursor**: `window`, `surface`, `window_focused`, `window_hidden`, `input_away`, `cursor`, `cursor_hidden`, `center_cursor`.
- **Entrada**: `keys`, `door_key_triggers`, `buttons_held`, `mmb_held`, `both_drag`, `mouse_*`, `pad_*`, `controllers`, `input_script`, `last_ctl_steer`, `hover*`.
- **Menus**: `game_menu`, `menu*`, `dd_scroll_drag`, `pane_scroll*`, `swap_pending`, `dropdown`, `chooser`, `vehicle_list`, `vehicle_meta`.
- **VR e head tracking**: `vr*`, `headtrack*`.
- **Espelhos**: `mirror_budget`, `mirrors_seen`, `mirror_turn`, `frozen_mirrors`, `mirror_hud`.
- **Editor**: `editor`, `editor_drag`, `editor_sync_t`, `placed`.
- **Perfil**: `profile`, `profile_prev`, `fps`, `total_frames`.
- **Atrás das features da Frente A1**, no sub-struct `Online`: `lan`, `remotes`, `plugins`, `plugin_keys`, `plugin_events`, `plugin_panels`, `voice`, `remote_walkers`, `inside_remote`, `remote_added`, `is_admin`.

Com os grupos, `window_event` vira um `match` curto que despacha para `on_redraw`, `on_keyboard`, `on_mouse`, `on_focus` e `on_resize`, cada um recebendo só os sub-structs que usa. Isso permite ao compilador emprestar partes do `App` separadamente e tira a maioria dos `too_many_arguments`.

Outras funções do app que pedem o mesmo tratamento, por tamanho: `place_step` (\~1.250, scene.rs:6764), `Traffic::tick` (\~1.180), `place_tile` (\~600), `input_script::on_key` (\~590), `upload_vehicle` (\~540), `spawn_departure` (\~520), `spawn_player` (\~490).

### C3. Métodos do `App` em arquivos com o nome de outra coisa

`impl App` aparece em 10 arquivos e soma cerca de 10.800 linhas. O maior bloco não está em `app.rs`: está em `input_script.rs`, que se descreve como o roteiro `OMSI_INPUT` de teclado e mouse para testes, mas tem 4.393 linhas de métodos do `App`. Lá moram `on_key` (\~590), `run_input_script` (\~260), `game_action` (\~190), `on_left`, `move_cursor`, `page_action`, `menu_choose`, `update_hover`, `place_vehicle`, `editor_key`, `tick_weather`, `tick_lan` e `tick_voice`. O roteiro de entrada propriamente dito é uma fração pequena do arquivo, e quem procura onde o jogo trata uma tecla não pensa em abrir `input_script.rs`.

| Arquivo | Linhas de `impl App` |
| --- | --- |
| `input_script.rs` | 4.393 |
| `app_events.rs` | 3.404 |
| `on_foot.rs` | 984 |
| `touch.rs` | 822 |
| `app.rs` | 689 |
| `vr_navigator.rs` | 227 |
| `placing.rs` | 171 |
| `game_lists.rs`, `applog.rs`, `plugin_ui.rs` | 151 |

Proposta, como movimentação pura e antes de C2:

- `input/keyboard.rs`: `on_key`, `script_key`, `route_char`, `chooser_key`, `editor_key`.
- `input/mouse.rs`: `on_left`, `move_cursor`, `update_hover`, `chase_orbit_step`, `precision_zoom_step`, `reset_blend`.
- `actions.rs`: `game_action`, `page_action`, `menu_choose`, `place_vehicle`.
- `input_script.rs` fica só com o roteiro: `run_input_script` e `parse_input_script`.
- `tick_weather` vai para junto do clima, e `tick_lan` e `tick_voice` para trás da feature `lan` (A1).

Depois disso, C2 transforma esses métodos em métodos dos sub-structs (`Input`, `Menus`) e cada arquivo deixa de precisar do `App` inteiro. Portão: `input_script.rs` abaixo de 500 linhas e nenhum arquivo com `impl App` acima de 1.500, exceto `app_events.rs` até a Frente B terminar.

## Frente D: configuração

O depot lê configuração por três caminhos que não conversam: variáveis de ambiente, o `settings.cfg` e a linha de comando. O problema mais sério que esta revisão encontrou está aqui: o `settings.cfg` tem **dois parsers com dois conjuntos de padrões**.

### D1. Um só `Settings` para jogo e launcher

O jogo lê o arquivo em `Settings::from_text` (omsi-app/src/settings.rs:439, 149 chaves num `match`) com os padrões de `Settings::default`. O launcher lê o mesmo arquivo em `settings_from_text` (omsi-launcher-core/src/lib.rs:1922), devolvendo um `serde_json::Value` com cerca de 168 nomes e os padrões escritos de novo em `json!` (`msaa: 4`, `anisotropy: 8`, `vr_scale: 0.65`, …). Uma opção nova precisa ser adicionada em seis lugares (campo, padrão, leitura e escrita no jogo; padrão e leitura no launcher), e um padrão diferente entre os dois lados aparece como "o launcher mostra uma coisa, o jogo faz outra".

Proposta:

1. Mover o struct `Settings` (sem os métodos que dependem de `omsi_render`, como `render_options`) para `omsi-launcher-core`, ou para um crate pequeno `omsi-settings` que os dois usam.
2. Ler e escrever com uma tabela declarativa: cada opção em uma linha com chave, campo, padrão e validação. Pode ser `serde` com um formato `chave=valor` próprio ou uma macro; o formato do arquivo não muda, para não quebrar perfis existentes.
3. O launcher passa a trabalhar com o struct tipado; o `Value` só aparece na borda da CLI `--cli`.
4. Teste de ida e volta: todo `settings.cfg` escrito pelo jogo é lido igual pelo launcher, e vice-versa. Os testes de ida e volta que já existem em `settings.rs` (triple screen, FOV externo, espelhos) viram casos dele.

### D2. Registro de variáveis `OMSI_*`

São 297 nomes em 79 arquivos, 45 deles no `omsi-app`. Por prefixo: 76 `DEBUG_*`, 47 `NO_*`, 6 `TRACE_*`, 4 `DUMP_*`, 3 `GOLDEN_*` e cerca de 160 outros (caminhos, ajustes de desempenho, modos). Hoje duas listas manuais descrevem subconjuntos sem vínculo entre si:

- `omsi-app/src/env_file.rs::ALLOWED`: as 35 variáveis que o `env.txt` do Android pode definir. É um filtro de segurança: caminhos (`OMSI_ROOT`, `OMSI_BIN`), entradas de script e programas externos ficam de fora para que escrever um arquivo no armazenamento compartilhado não mude o que o jogo lê ou executa.
- A documentação em `docs/` e no README, que cita cerca de 120 nomes.

O `rigid/toggles.rs`, que lia os três switches A/B da física uma vez num `OnceLock`, era o terceiro exemplo e o modelo deste registro; ele saiu no PR #32 junto com os switches.

Desenho proposto, um módulo `omsi_cfg::flags` (o crate já tem o cache de variáveis em lib.rs:1193):

- Uma macro declara cada variável uma vez: nome, tipo (`bool`, número, texto, caminho), padrão, categoria, se o `env.txt` pode defini-la e a descrição.
- Categorias: **usuário** (documentadas, por exemplo `OMSI_TEXTURE_MEMORY`, `OMSI_MAX_FPS`), **caminho** (nunca liberadas pelo `env.txt`), **diagnóstico** (`DEBUG_*`, `TRACE_*`, `DUMP_*`) e **teste** (`GOLDEN_*`, `FAKE_*`, `TEST_*`). Cada diagnóstico leva o módulo dono e o motivo, para que se saiba quando ele pode sair.
- O código lê `flags::TEXTURE_MEMORY.get()` já tipado, em vez de `omsi_cfg::env::var("…").ok().and_then(|v| v.parse().ok())`.
- `env_file::ALLOWED` passa a ser derivado da macro, e um teste falha se uma variável de caminho for marcada como permitida.
- Um teste gera a tabela das categorias usuário e diagnóstico num `docs/ENVIRONMENT.md` e falha se o arquivo estiver desatualizado.

Ordem: criar o registro e migrar `omsi-render`, `omsi-audio` e os crates pequenos; depois `omsi-app` por módulo. O PR #32 já mostrou o passo seguinte: variáveis que perderam o propósito saem, com o registro em `docs/removed/`. Com dono e motivo declarados, a mesma revisão pode ser feita nos 76 `DEBUG_*` e nos 47 `NO_*`.

### D3. Launcher-core tipado e renomeado

Além das configurações, `get_keybindings`, `save_keybindings`, `option_presets` e `graphics_profiles` trocam `Value` com a UI (227 usos de `Value`/`json!` em lib.rs). Cada uma ganha um struct com `Serialize`/`Deserialize`. Junto, renomear a lib `omsi_launcher_lib` para `omsi_launcher_core` e parar de importá-la como `core` em `launcher/mod.rs`, que sombreia o `core` da linguagem.

### D4. CLI

`Args` (cli.rs:10) tem \~73 campos. Depois de A1, os campos `--lan-*` e `--server` ficam atrás das features `lan` e `server` e somem do `--help` do jogo padrão. O resto pode ficar como está; o único ajuste útil é agrupar os campos de cenário (`--map`, `--bus`, `--time`, `--traffic`, `--schedule`) num struct que o `Session` recebe, para que o offscreen e o launcher montem a mesma coisa.

## Frente E: simulação separada da apresentação e divisão de crates

O código já tem metade da separação: `Traffic` e `Humans` têm um `tick` que avança o estado e um `sync` que copia o estado para a cena (`traffic.rs:7046`, `humans.rs:4880`). O que falta é tirar o renderer de dentro do `tick`.

### E1. Pedidos em vez de chamadas ao renderer

Hoje 9 funções de `traffic.rs`, 6 de `humans.rs` e 1 de `schedule.rs` recebem `&Renderer`. As que importam são as que criam ou liberam coisas no meio da simulação: `pull_out_parked` (traffic.rs:2302), `wake_dormant` (:2614), `set_trailers` e `turn_train` (:1540, :1556), que chamam `world.add_vehicle_shared` e `world.release_vehicle`; em `humans.rs`, `stops_tick` (:2655) e `spawn_waiting` (:2797) criam malhas, texturas e materiais direto (`renderer.add_mesh`, `add_material`).

Proposta: o `tick` empilha pedidos (`SpawnVehicle { id, ty, scheme }`, `ReleaseVehicle { id }`, `SpawnHuman { … }`) e o `sync` os aplica antes de copiar transformações. Um veículo pedido num frame aparece na cena no mesmo frame, porque o `sync` roda logo depois do `tick`; o traço de B0 confirma que nada muda. `precache_random` e `clear_random` já são operações de apresentação e podem continuar recebendo o renderer.

Com E1 feito, `Session::step` deixa de receber `&Renderer`, e o passo de simulação pode rodar sem GPU: testes de tráfego e de passageiros como testes comuns de unidade.

### E2. Divisão de crates

Ordem, das áreas com menos dependências para as com mais. Cada extração é um PR de movimentação pura, seguido de PRs de ajuste de visibilidade (`pub(crate)` vira `pub` só onde precisa).

| Crate novo | De onde sai | Tamanho | Pré-requisito |
| --- | --- | --- | --- |
| `omsi-html` | `omsi-sim/src/htmlengine` (DOM, layout, canvas, interpretador JS) | \~5.000 | nenhum; não é simulação |
| `omsi-input` | `controllers.rs`, `dinput.rs`, `evdev_*`, `mac_hid.rs`, `mac_game_controller.rs`, `touch.rs`, `headtrack.rs`, `hpattern.rs`, `ffb_calibration.rs`, `keys.rs`, `stock_keys.rs` | \~8.500 | C2 (estado de entrada fora do `App`) |
| `omsi-launcher` | `launcher/` | \~13.000 | D1 e D3 ajudam, não são obrigatórios |
| `omsi-gameplay` | `traffic.rs`, `schedule.rs`, `schedule_paper.rs`, `humans.rs`, `humans_pax.rs`, `career.rs`, `driver.rs`, `duty_start.rs`, `journey.rs`, `money.rs`, `bus_service.rs` | \~29.000 | B4 e E1 |
| `omsi-world` (opcional) | `scene.rs`, `tiles.rs`, `world_load.rs`, `placing.rs` | \~18.000 | E1; só se `omsi-gameplay` precisar de `World` sem o resto do app |

O código de LAN do app (`lan.rs`, `lan_world.rs`, `lan_mods.rs`, `voice.rs`, `admin.rs`, `server.rs`) não entra nesta divisão: ele fica no app atrás da feature `lan` (A1). Quando o multiplayer for readaptado, o lugar natural dele é um crate opcional `omsi-lan-game`, que depende de `omsi-gameplay` e é ligado pela mesma feature. O `input_script.rs` sai bem menor da Frente C3, e o que sobrar de mapeamento de teclas vai para `omsi-input`.

Dentro de `omsi-render` não é preciso crate novo: a Frente C1 resolve com módulos. O alvo de tamanho para o `omsi-app` depois de A1 e E2 é ficar abaixo de 50 mil linhas, com janela, laço de eventos, UI, menus e a cola entre os crates.

## Alinhamento com o mapa do projeto

O mapa do projeto (11/10/2026) põe a refatoração como fase 3, depois da paridade (fase 1) e da triagem (fase 2), com duas regras: testes de caracterização primeiro e nenhuma mudança de comportamento. Por decisão do mantenedor (11/10/2026), a refatoração passa a correr em paralelo às fases 1 e 2, sem impedir os outros colaboradores de continuar mudando o código. O pré-requisito anterior está cumprido: o plano de física concluiu as Fases 0 a 4, e a Fase 5 (física da IA) está pausada.

O que este plano assume do mapa:

- **Premissa 1:1 com o OMSI 2.** Refatoração é só estrutura. Todo PR deste plano deixa o comportamento igual, provado por traço ou teste, e qualquer diferença encontrada vira um PR `fix/` separado, como o B6.
- **Fluxo de uma mudança.** Branch curta com um escopo, PR, CI e merge de um PR por vez, com o main sempre integrável. Os PRs de refatoração entram nessa mesma fila, sem fila própria.
- **Fase 4, performance.** Ela começa medindo com `OMSI_PROFILE`. O perfil por escopo (`enum Stage`, da Frente B) é o instrumento dessa medição, então deve sair antes dela.
- **Fontes proibidas e redação.** Nada muda: vale a regra que os PRs #15 a #24 já seguem.

Duas diferenças entre o mapa e as decisões já registradas, para corrigir no mapa:

- O mapa diz que a remoção definitiva de rede e plugins "ainda é decisão em aberto". Ela foi tomada em 10/10/2026: o código fica no repositório e sai só da compilação do jogo (A1).
- O mapa mostra branches `claude/*`, mas o CONTRIBUTING pede `feat/`, `fix/` e `docs/`. Este plano usa `feat/` para refatoração.

## Refatoração em paralelo à paridade e à triagem

### Onde as fases ativas mexem

Medido no histórico do main (216 commits desde 07/10/2026, incluindo os PRs de paridade, triagem e física):

| Crate | Linhas | Commits que o tocaram | Risco de conflito |
| --- | --- | --- | --- |
| `omsi-app` | 124,8 mil | 72 | alto |
| `omsi-sim` | 34,4 mil | 39 | médio: a física terminou, mas `vehicle.rs` teve 15 |
| `omsi-launcher-core` | 6,0 mil | 20 | alto: `lib.rs` é o arquivo mais tocado do repositório (19) |
| `omsi-render` | 20,0 mil | 10 | baixo |
| `omsi-audio`, `omsi-ui`, `omsi-geometry` | 8,5 mil | 3 a 8 cada | baixo |
| Crates de formato (`omsi-cfg`, `-vehicle`, `-texture`, `-map`, `-content`, `-model`, `-scenery`, `-o3d`, `-script`, `-timetable`) | \~21 mil | 2 a 7 cada | baixo |
| `omsi-net`, `omsi-plugin` | 12,5 mil | 3 e 4 | nenhum: desligados |

Dentro do `omsi-app`, os commits se concentram no laço de frame (`app_events.rs`, `app.rs`, `lib.rs`, `offscreen.rs`: 54), no gameplay (`humans.rs`, `traffic.rs`, `schedule.rs` e vizinhos: 52) e no `launcher/` (47). HUD e menus (23), mundo (21), entrada (20), LAN (19) e settings (11) são mornos.

A conclusão: os alvos que mais melhoram a legibilidade (o `omsi-render` e os crates de formato) são frios e podem começar já. As áreas quentes são exatamente onde paridade e triagem corrigem bugs, e elas precisam de outro tratamento.

### Unidade de divisão: crate onde der, grupo de módulos no `omsi-app`

Quebrar a refatoração por crate funciona bem para 19 dos 21 crates. Não basta para o `omsi-app`, que tem 55% do código e metade dos commits: como unidade única, ele travaria todo mundo. A proposta é usar o crate como unidade nos crates frios e o grupo de módulos dentro do `omsi-app` (laço de frame, gameplay, launcher, mundo, entrada, settings, LAN, HUD e menus). Cada unidade tem um responsável por vez e uma lista fechada de arquivos.

### Regras para não travar os outros colaboradores

1. **A refatoração cede.** Se há um PR de paridade ou triagem aberto num arquivo, a unidade que contém esse arquivo espera ou o deixa de fora. Correção de bug nunca espera refatoração.
2. **Só PRs de movimentação curtos.** Cada PR move ou renomeia sem mudar lógica, toca uma unidade, fica aberto no máximo um dia útil e entra na fila assim que o CI passa. Um PR que precisaria de dias aberto é dividido.
3. **Reserva visível.** Uma issue fixada no GitHub ("Refatoração: unidades em andamento") lista cada unidade reservada, seus arquivos e o PR. Quem vai mexer num arquivo reservado olha a issue, e quem refatora confere os PRs abertos antes de reservar. Ela fica no GitHub porque é o lugar que todos os colaboradores já acompanham.
4. **Quem move porta.** Depois de cada PR de movimentação, quem o fez atualiza as branches abertas que tocavam aqueles arquivos (merge do main e resolução do conflito), em vez de deixar o conflito para o colaborador. A rotina diária de revisão já faz o merge do main nos PRs abertos e pode assumir essa parte.
5. **Fachada antes de mover, nas áreas quentes.** No laço de frame, no gameplay e no `omsi-launcher-core`, a mudança entra primeiro como código novo ao lado do antigo: um módulo novo, com o antigo delegando para ele. A remoção do caminho antigo vem num PR curto depois, num momento sem PRs abertos naqueles arquivos.
6. **Formatação por crate, não num commit só.** O CI passa a exigir `fmt --check` só nos crates já formatados. Para os crates quentes, um script `scripts/fmt-merge.sh` aplica na branch do colaborador a receita que evita o conflito: merge do commit anterior ao fmt, `cargo fmt` na própria branch e merge do commit de fmt com `-s ours`.
7. **Configuração de git recomendada no CONTRIBUTING.** `merge.conflictStyle = zdiff3`, `rerere.enabled = true` e um `diff.renameLimit` alto fazem o git seguir arquivos movidos e lembrar resoluções repetidas.

## Plano em trilhas

Em vez de fases em sequência, quatro trilhas que podem andar juntas, cada uma com seu portão. Todas as branches são `feat/<nome>`.

1. **Trilha 0: caracterização, que já pode começar.** B0 (traço de estado do offscreen no CI), teste de ida e volta do `settings.cfg` entre jogo e launcher, e o golden do offscreen rodado nas máquinas do mantenedor como referência. São só arquivos novos de teste e não conflitam com nada. Portão: B0 rodando no CI e verde nos PRs de paridade e triagem.
2. **Trilha 1: crates frios, em paralelo à trilha 0.** Por crate: `cargo fmt -p`, clippy limpo e `-D warnings` (A3, A4) nos crates de formato, `omsi-audio`, `omsi-ui` e `omsi-geometry`. Depois C1 (`omsi-render/src/lib.rs` em módulos), o maior ganho de legibilidade num crate com 10 commits em todo o histórico, e a extração de `omsi-html` de `omsi-sim/src/htmlengine`. Portão: crates de formato com `-D warnings`, nenhum arquivo de `omsi-render` acima de 3.000 linhas e golden igual nos modos `vanilla` e `enhanced`.
3. **Trilha 2: grupos mornos do `omsi-app`, depois do portão 0.** A1 (features `lan`, `server` e `plugins`), que toca muitos arquivos mas com trechos pequenos; C3 (`input_script.rs` por assunto); D2 (registro de `OMSI_*`), primeiro nos outros crates e depois por módulo; imports explícitos no lugar dos globs; e a divisão do restante de `vehicle.rs` em `omsi-sim`. Portão: `omsi-net` e `omsi-plugin` fora do `cargo tree` do jogo, job `--all-features` rodando, `input_script.rs` abaixo de 500 linhas e toda `OMSI_*` declarada.
4. **Trilha 3: áreas quentes, por janelas.** Frente B (`Session::step`), C2 (`App` em sub-structs), D1 e D3 (um só `Settings` e o launcher-core tipado), E1 e E2. Cada uma entra numa janela curta e anunciada: a unidade é reservada, os PRs de paridade e triagem naqueles arquivos são mesclados ou portados antes, e a janela termina com o main integrável. O perfil por escopo (`enum Stage`) sai primeiro, porque a fase de performance do mapa depende dele. Portão: traço B0 idêntico em cada PR, `run_offscreen` abaixo de 400 linhas, `omsi-gameplay` compilando e testando sem `wgpu`, e `omsi-app` abaixo de 50 mil linhas na configuração padrão.

O que se espera no fim, em números medidos hoje: o `omsi-app` compilado no jogo padrão cai de 124,8 mil para menos de 50 mil linhas, com cerca de 10,7 mil atrás das features da Frente A1 e 50 mil movidas para crates próprios. O código de LAN continua no repositório e conta à parte até virar o crate opcional `omsi-lan-game`. O maior arquivo cai de 15.830 para menos de 3.000 linhas, e as 40 funções acima de 300 linhas ficam reduzidas às que são parsers ou tabelas por natureza (`tokenize`, `handle_keyword`, `parse_objects`).

## Decisões pendentes

Nenhuma destas decisões bloqueia a paridade nem a triagem. A recomendação vem primeiro em cada linha.

| Decisão | Recomendação | Alternativa | Por quê |
| --- | --- | --- | --- |
| Unidade de divisão da refatoração | Crate nos crates frios, grupo de módulos no `omsi-app` | Só por crate | O `omsi-app` tem 55% do código e metade dos commits; como uma unidade só, travaria todo mundo |
| Prioridade entre refatoração e correções | A refatoração cede | Janelas de congelamento longas | Paridade e triagem são as fases ativas do mapa |
| Formatação (A2) | Por crate, com `scripts/fmt-merge.sh` para os quentes | Um commit único | O commit único quebra toda branch aberta |
| Onde fica a reserva de unidades | Issue fixada no GitHub | Arquivo fora do repositório | Todos os colaboradores acompanham o GitHub |
| Quem porta branches abertas depois de uma movimentação | Quem moveu, ou a rotina diária | O dono de cada branch | Quem moveu sabe para onde cada trecho foi |
| Multiplayer, servidor e plugins (A1) | **Decidido (10/10/2026):** ficam no repositório, fora da compilação do jogo | Nenhuma; apagar foi descartado | Serão reaproveitados e readaptados mais tarde |
| Granularidade das features (A1) | Três: `lan`, `server` (implica `lan`) e `plugins` | Uma só, `online` | Plugins não dependem de rede; separar permite readaptar um sem o outro |
| CI do código guardado (A1) | Job `cargo check --workspace --all-features`, informativo no começo e bloqueante depois | Não compilar o código guardado | Sem o job, o código guardado quebra a cada refatoração do app |
| Largura do rustfmt (A2) | Padrão de 100 colunas | `max_width = 140` num `rustfmt.toml` | É o que o CONTRIBUTING já promete |
| `signal_aspects` no offscreen (B6) | Passar o trilho do jogador, igual à janela | Manter `None` | O offscreen deve reproduzir o jogo; a diferença só afeta trens do jogador em roteiros |
| Onde mora o `Settings` compartilhado (D1) | Em `omsi-launcher-core` | Crate novo `omsi-settings` | O launcher-core já é dependência do app; um crate a menos para manter |
| Crate `omsi-world` (E2) | Decidir na trilha 3 | Criar junto com `omsi-gameplay` | Depende de quanto do `World` o gameplay ainda precisar depois de E1 |
| Renomear `omsi-*` para `depot-*` | Não por enquanto | Renomear crates e identificadores | Custa um PR grande e não melhora a legibilidade |

## Riscos e critério de sucesso

- **Regressão silenciosa de comportamento.** Muitos testes pulam sem `OMSI_ROOT`, e as imagens do golden ainda não rodam no CI. Mitigação: o traço de estado B0 no CI, os traços da física (bloqueantes) e `omsi-check` antes e depois de cada fase, como o CONTRIBUTING já pede.
- **Golden dependente de máquina.** As imagens só batem no mesmo adaptador gráfico. Mitigação: o B0 compara estado, não pixels, e roda em qualquer máquina; o golden fica para as Frentes C1 e o render.
- **Conflito com paridade e triagem.** As áreas quentes da refatoração são as mesmas onde os bugs são corrigidos. Mitigação: as regras de trabalho em paralelo (a refatoração cede, PRs curtos, reserva visível, quem move porta) e as áreas quentes só por janelas anunciadas.
- **Branches paralelas.** Sem upstream, o conflito que importa é entre branches do próprio depot. Cada movimentação grande (A2, C1, E2) é anunciada e mergeada rápido, num momento sem branches longas abertas; o `CHANGELOG.md`, que já conflita em quase todo merge, pode passar a usar fragmentos por PR.
- **PRs grandes demais para revisar.** Cada extração é um PR de movimentação pura, sem mudar lógica, seguido de PRs de ajuste.
- **Redação.** Comentários e documentos criados pela refatoração descrevem comportamento observável, sem identificadores do executável original, como fizeram os PRs #15 a #24.

Critério de sucesso: uma mudança típica de gameplay, como uma nova regra de tráfego, toca um crate e um ou dois arquivos abaixo de 2.000 linhas, roda igual no jogo e no offscreen, e pode ser testada sem GPU.
