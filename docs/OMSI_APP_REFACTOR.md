# Refatoração do `omsi-app` por domínio (plano)

Plano de 23/10/2026 para o crate `omsi-app`, medido no `main` em `e99b624` (PR #398). Só leitura de código; contagens de linha e de função são aproximadas (casamento de chaves). É a **Trilha App** do [plano macro](MACRO_REFACTOR_DRAFT.md#trilha-app-omsi-app-por-domínio) e segue as regras de lá: um PR por vez, reservado na issue #61, branches `feat/` para estrutura e `fix/` para qualquer mudança de comportamento.

O plano macro trata o `omsi-app` por frentes soltas (B, C2, C4, D1, E1 a E3, o passo 2 da C6), todas na Trilha 4, e nenhuma delas cobre o código que entrou depois da revisão de 16/10/2026: a nova interface do jogo, o launcher como processo próprio e a saída pelo renderer Direct3D 9Ex. Esta trilha junta tudo o que mexe no `omsi-app` num lugar só, com a formatação do crate como primeiro passo.

## Regras herdadas

- **Refatoração não muda comportamento.** Cada PR prova isso com o traço de estado do offscreen (`offscreen-traces`), o golden no lavapipe, os traços da física e da IA quando o PR toca o passo do jogador ou do tráfego, a mesma lista de testes (`cargo test -p omsi-app -- --list`, comparando os nomes sem o caminho do módulo) e `cargo check -p omsi-app --all-features`. Qualquer diferença achada no caminho vira um PR `fix/` separado.
- **A refatoração cede.** Correção nunca espera refatoração. Um PR desta trilha reserva a unidade na #61 e deixa de fora os arquivos com PR `fix/` aberto, ou espera.
- **Nada se apaga.** Código que deixa de ser usado (o desenho em CPU do HUD, o launcher antigo) sai da compilação do jogo atrás de uma feature desligada, como o multiplayer na A1.
- **O jogo não compensa modelos mal configurados.** Nenhum passo desta trilha acrescenta compensação; a retirada das heurísticas de material de veículo continua sendo `fix/` na C4.
- **Redação.** Comentários e documentos descrevem comportamento observável, sem identificadores do executável original.

## Diagnóstico

O crate tem 155 arquivos `.rs` e 144,3 mil linhas em `src/` (era 124,8 mil em 11/10/2026), mais 834 em `tests/`. A lib se chama `openomsi_game` e o binário `depot`; o mesmo binário abre o jogo, o launcher (sem argumentos) e o ajudante de prévia (`--preview-helper`).

### Por domínio

| Domínio | Linhas | Arquivos | O que tem |
| --- | --- | --- | --- |
| Gameplay | 39.287 | 25 | `traffic.rs` (8.374), `schedule.rs` (7.199), `humans.rs` (7.087), `player.rs`, `humans_pax.rs`, `driver.rs`, `bus_service.rs`, `on_foot.rs`, `spawn.rs`, `journey.rs`, `career.rs`, `money.rs` e vizinhos |
| Mundo e cena | 21.533 | 14 | `scene.rs` (14.600) e `scene/`, `tiles.rs`, `map_surface.rs`, `world_load.rs`, `editor.rs`, `placing.rs`, `export.rs` |
| Launcher | 19.217 | 25 | `launcher/` (era 12,8 mil): `mod.rs`, `pages.rs`, `drive.rs`, `state.rs`, `settings.rs`, `timetable.rs`, `phone.rs`, `keyboard.rs`, `preview.rs`, `mapview.rs`, `setup.rs`, `tray.rs` e outros |
| Núcleo e laço de frame | 12.843 | 23 | `app_events.rs` (3.865), `offscreen.rs` (3.203), `app.rs`, `lib.rs`, `startup.rs`, `cli.rs`, `android.rs` e utilitários |
| Entrada | 12.459 | 19 | `controllers.rs` (2.816), `dinput.rs`, `touch.rs`, `input/`, `actions.rs`, `evdev_*`, `mac_*`, `headtrack.rs` |
| Online e plugins, desligados | 10.707 | 11 | `lan.rs`, `lan_world.rs`, `plugin_ui.rs`, `voice.rs`, `lan_mods.rs`, `admin.rs` e os outros atrás de `lan`, `server` e `plugins` (A1) |
| HUD e menus antigos | 10.533 | 13 | `game_lists.rs` (3.189), `ui.rs` (2.431), `nav/`, `hud.rs`, `menu.rs` |
| Nova interface | 6.801 | 7 | `new_ui.rs` (2.486), `new_ui_windows.rs`, `new_ui_alt.rs`, `new_ui_pages.rs`, `new_ui_report.rs`, `new_ui_add.rs`, `new_ui_loading.rs` |
| Saída gráfica | 5.440 | 8 | `d3d9_output.rs`, `openxr.rs`, `vr_navigator.rs`, `mirror_hud.rs`, `camera_arm.rs`, `camera_util.rs`, `head_idle.rs`, `lights.rs` |
| Clima | 3.649 | 8 | `weather_setup.rs`, `weather_model.rs`, `weather_time.rs`, `weather_cycle.rs`, `condensation.rs`, `rain.rs`, `puddles.rs`, `ambience.rs` |
| Settings | 1.859 | 2 | `settings.rs` e `settings/round_trip.rs` |

### Formatação

O `omsi-app` está fora do `fmt --check` bloqueante: `cargo fmt --check -p omsi-app` acusa 5.480 trechos, e o passo do CI que cobre o workspace inteiro é informativo. É o crate mais tocado do repositório, então o commit de `cargo fmt` conflita com quase toda branch aberta nele. O `scripts/fmt-merge.sh` e o `.git-blame-ignore-revs`, previstos na Frente A2 para este caso, nunca foram criados.

Os crates que entraram com o código novo já estão quase formatados: `omsi-render-d3d9` e `omsi-launcher-core` não acusam nada, e `omsi-launcher-ui` acusa um trecho. Nenhum dos três está na lista bloqueante.

### Testes misturados com o código

Cerca de 19,6 mil linhas de `src/` (14%) são testes, em módulos `#[cfg(test)]` de 101 arquivos e em cinco arquivos `*_tests.rs`/`tests.rs` (três deles ligados a `scene.rs` por `#[path]`). Os maiores: `schedule.rs` (1.817), `new_ui.rs` (1.450), `scene.rs` (1.364), `humans.rs` (1.159) e `controllers.rs` (1.129). Em `tests/` só há o golden, o traço de estado e o túnel do offscreen. Um único teste da nova interface, `the_pages_behind_the_menu_draw_in_the_new_look`, tem 662 linhas.

### Funções gigantes

27 funções passam de 300 linhas e 99 de 150. As maiores: `window_event` (3.144, `app_events.rs`), `run_offscreen` (3.026), `place_step` (1.251, `scene.rs`), `Traffic::tick` (1.182), `nav::city::city` (657), `on_key` (604), `place_tile` (596), `upload_vehicle` (537), `spawn_departure` (516), `spawn_player` (496), `openxr::render` (452), `nav::minimap::draw` (440), `ui::draw_menu` (406), `make_app` (340).

### O código novo

- **Nova interface.** O menu Esc e as páginas atrás dele, a barra Alt e suas janelas, a janela de adicionar veículo, o relatório de fim de viagem e a tela de carregamento são desenhados por `omsi-ui`, por sete arquivos planos na raiz do crate, seis deles com `impl App`. As páginas novas ainda tiram os dados e os retângulos de clique do HUD antigo (`new_ui_pages` usa `ui::Frame`, `MenuKind`, `DropdownView` e `write_hits`), e as linhas vêm de `game_lists.rs` como texto separado por `\u{1f}`.
- **HUD antigo.** O `ui.rs` continua rasterizando em CPU o chat, os avisos, os nomes, o tutorial, o horário, a barra de informações e a lista de escolha (`draw_menu`). O "OPENOMSI" do menu (`ui.rs:1396`) e a cópia da Roboto continuam lá. São dois caminhos de desenho para a mesma tela, o problema que a C6 queria resolver.
- **Launcher.** Virou processo próprio e ganhou o crate `omsi-launcher-ui` (a barra lateral, as páginas de veículos e mapas, o `look.rs`), mas a maior parte continua em `omsi-app/src/launcher/`, que usa módulos do jogo (`controllers`, `weather_setup`, `spawn`, `situation`, `scene`, `player`, `nav`, `cli`) e é usada por eles (`weather_time.rs`, `game_lists.rs`, `server.rs`, `android.rs`).
- **Saída Direct3D 9Ex.** `d3d9_output.rs` liga o plano de cada quadro e os eventos de asset do `omsi-render` ao `omsi-render-d3d9`, mas a escolha do backend está espalhada: 20 menções em `app.rs`, 9 em `startup.rs`, 4 em `app_events.rs`, mais `settings.rs`, `api_limits.rs`, `game_lists.rs`, `new_ui.rs`, `new_ui_loading.rs` e `launcher/settings.rs`. Com ela, o frame tem três caminhos (janela no wgpu, janela no Direct3D 9 e offscreen), e só o último é coberto pelo CI.

### `impl App` espalhado

`impl App` aparece em 23 arquivos. O portão da C3 (nenhum arquivo com `impl App` acima de 1.500 linhas, exceto `app_events.rs`) ainda vale, mas `new_ui.rs` (2.486, com 1.450 de testes) só passa por causa dos testes.

## Convenção de testes

**Testes fora de `src/` são padrão no projeto** (decisão do mantenedor, 23/10/2026), não só neste crate. Como na Trilha S: nenhum `#[cfg(test)]`, `#[test]` ou `tests.rs` em `src/`; um binário de teste por crate (`tests/app/main.rs`, com um módulo por domínio espelhando `src/`) e os harnesses que o CI chama pelo nome como binários próprios; os auxílios em `tests/support/`; o que é interno chega aos testes por `omsi_app::testing`, compilado só com a feature `test-support`, ligada por uma dev-dependency do próprio crate.

## Princípios, aplicados a este crate

Os mesmos da Trilha S: uma responsabilidade por arquivo, com o nome do assunto; função longa vira etapas com nome, na mesma ordem; cada domínio grande tem um `mod.rs` que expõe só o que o resto do jogo usa; trait só onde houver duas implementações de verdade (aqui, o backend gráfico); estado global vira parâmetro só junto da Frente B.

Duas regras próprias do app:

1. **Um domínio é uma pasta.** O crate deixa de ter 109 arquivos na raiz. Cada pasta tem um `mod.rs` e os arquivos do domínio; os métodos do `App` de um domínio ficam na pasta dele, até a C2 transformá-los em métodos dos sub-structs.
2. **O código novo entra direto na pasta do domínio.** Depois que uma pasta existir, nenhum `new_ui_*.rs` ou `d3d9_*.rs` novo na raiz.
3. **Cada domínio é um pacote em três partes** (decisão do mantenedor, 23/10/2026), no modelo do pacote `omsi-sim/src/physics/`. O PR que cria a pasta de um domínio já reorganiza os arquivos dela nessa hierarquia; não há um PR só de mover arquivos seguido de outro de reestruturar.

```
<domínio>/
  mod.rs              declara as três partes e reexporta só a interface
  domain/             os objetos do domínio: structs, enums, estados e constantes, com os acessores
                      que leem os campos; sem cálculo, sem renderer, sem disco, sem App
  interface/
    input.rs          a entrada: o que o resto do jogo chama ou entrega ao domínio (funções,
                      métodos públicos, eventos), e o único caminho para dentro
    output.rs         a saída: o que o domínio devolve (estados, eventos, pedidos) e o que ele
                      precisa de fora (renderer, cena, áudio, disco, outros domínios), reunidos
                      num lugar só, em vez de chamados de qualquer arquivo
  implementation/     os procedimentos, um módulo por subdomínio, na ordem em que rodam
```

   Regras do pacote: `implementation/` só fala com outro domínio pela `interface/input.rs` dele, e só chega ao que é de fora pela própria `interface/output.rs`; `domain/` não importa `implementation/` nem `interface/`; o resto do crate importa o domínio só pelo que o `mod.rs` reexporta. Trait na saída só onde houver duas implementações de verdade (o backend gráfico de `output/`); nos outros casos `output.rs` reexporta ou embrulha os tipos concretos. Um domínio pequeno pode ter `domain.rs` e `implementation.rs` em vez de pastas, mas mantém as três partes. Os métodos de `impl App` de um domínio ficam em `implementation/` até a C2 trocá-los por métodos dos sub-structs.

## Arquitetura alvo

Só a primeira camada; o detalhe de cada pasta é decidido no PR dela, medido na hora. Cada pasta abaixo é um pacote com `domain/`, `interface/` (`input.rs` e `output.rs`) e `implementation/`, como na regra 3 dos princípios. A pasta da nova interface se chama `ui/` (decisão do mantenedor), para que a camada de entrada e saída dela fique em `ui/interface/`, sem repetir o nome.

```
src/
  lib.rs, main.rs       reexportações e o ponto de entrada
  core/                 App, make_app, startup, cli, threads, applog, memory, platform, android
  frame/                o laço de frame: window_event e, depois da Frente B, App::frame e Session
  offscreen/            run_offscreen, o traço de estado e as verificações de mapa (diagnostics/map_checks.rs)
  output/               a saída gráfica: Backend { Wgpu, D3d9 } escolhido num ponto, d3d9.rs, xr/ (openxr,
                        vr_navigator), mirrors/ (mirror_hud), camera/ (camera_arm, camera_util, head_idle)
  ui/                   a nova interface: mod.rs (NewUi, Shared, o gancho de overlay), esc_menu.rs, pages.rs,
                        alt/ (barra e janelas), add_vehicle.rs, trip_report.rs, loading.rs
  hud/                  o HUD: frame.rs (os dados e os retângulos de clique que as duas interfaces usam),
                        legacy/ (o desenho em CPU de ui.rs, depois atrás de uma feature), hud.rs, menu.rs,
                        lists/ (game_lists por assunto), nav/
  launcher/             o que sobrar do launcher no app: a ligação com o processo do jogo; o resto vai
                        para omsi-launcher-ui e omsi-launcher-core
  input/                como está, mais controllers, dinput, evdev, mac, touch, headtrack, hpattern, ffb,
                        keys, actions, input_script (o futuro omsi-input da E2)
  world/                scene/ (dividido na C4), tiles, map_surface, world_load, placing, editor, export
  weather/              weather_*, condensation, rain, puddles, ambience (a E3)
  gameplay/             traffic/, schedule/, humans/, player/, on_foot, driver, duty, journey, career, money,
                        bus_service (o futuro omsi-gameplay da E2)
  settings/             como está, depois a tabela da D1
  online/               lan, lan_world, lan_mods, voice, admin, server, plugins, plugin_ui e os *_off,
                        atrás das mesmas features de hoje
  testing.rs            só com `test-support`, se a decisão sobre os testes for a da Trilha S

tests/
  support/              ScratchPack, raiz sintética e o que os testes do app repetem
  app/                  um binário com um módulo por domínio, espelhando src/
  offscreen_golden.rs, offscreen_state_trace.rs, offscreen_tunnel.rs
```

Alvos de tamanho: nenhum arquivo acima de 3.000 linhas (o portão da C4) e, fora de `scene/` e `gameplay/` até a Trilha S chegar neles, nenhum acima de 1.500; nenhuma função acima de 300, a não ser tabelas de despacho. O alvo do plano macro continua: `omsi-app` abaixo de 50 mil linhas na configuração padrão, depois da E2.

## Sequência

Duas fases. A primeira é de estrutura, em arquivos mornos, e pode começar já. A segunda é a antiga Trilha 4, que passa a ser executada dentro desta trilha, com o mesmo conteúdo e as mesmas janelas anunciadas.

### Fase 1: formatação, testes e o código novo

| Item | Branch | O que entra, em commits | Pronto quando |
| --- | --- | --- | --- |
| AP1 | `feat/app-fmt` | (1) `scripts/fmt-merge.sh` (merge do commit anterior ao fmt, `cargo fmt` na própria branch, merge do commit de fmt com `-s ours`) e a configuração de git recomendada no CONTRIBUTING (`merge.conflictStyle = zdiff3`, `rerere.enabled = true`, `diff.renameLimit` alto); (2) `omsi-render-d3d9`, `omsi-launcher-core` e `omsi-launcher-ui` formatados e na lista bloqueante; (3) `cargo fmt -p omsi-app` sozinho, o crate na lista bloqueante e o passo informativo do workspace atualizado em `docs/QUALITY.md`; (4) `.git-blame-ignore-revs` com o hash do commit (3) | `fmt --check` bloqueante nos três crates; testes, `offscreen-traces` e golden iguais. O `omsi-app` foi formatado (#403), mas o check dele fica informativo por enquanto (decisão do mantenedor, 24/10/2026) |
| AP2 | `feat/app-testes-*` | Os testes de `src/` para `tests/`, como na [convenção de testes](#convenção-de-testes); um PR por grupo de domínios (gameplay; mundo e cena; interface, HUD e launcher; entrada e o resto), porque são cerca de 19,6 mil linhas. O teste de 662 linhas da nova interface vira um teste por página | Nenhum teste em `src/`; a mesma lista de testes, pelos nomes; `cargo build -p omsi-app` sem a feature não tem `testing` |
| AP3 | `feat/app-ui` | `ui/` com os sete `new_ui*.rs`, divididos por tela no pacote (`domain/` com o estado de cada tela, `interface/` com o que o frame chama e o que a interface pede ao jogo, `implementation/` com o desenho e o clique); `hud/frame.rs` com `Frame`, `MenuKind`, `DropdownView`, `Preview` e os retângulos de clique, tirados de `ui.rs`, para que a nova interface deixe de depender do desenho antigo | Nenhum `new_ui*.rs` na raiz; `ui/` não importa `hud::legacy` |
| AP4 | `feat/app-saida` | `output/` como pacote, com `d3d9_output.rs`, `openxr.rs`, `vr_navigator.rs`, `mirror_hud.rs` e a câmera em `implementation/`; um `Backend { Wgpu, D3d9 }` em `interface/output.rs`, decidido uma vez na abertura da janela, no lugar das checagens espalhadas por `app.rs`, `startup.rs`, `app_events.rs` e pelos menus | Menções ao Direct3D 9 fora de `output/` e das opções de configuração só por `Backend`; o job do Windows verde no PR (é ele que compila esse código) |
| AP5 | `feat/app-hud` | `hud/` como pacote, com `ui.rs` dividido em texto, chat e avisos, informações e a lista de escolha; `game_lists.rs` dividido por assunto em `hud/lists/`; `nav/` dentro de `hud/` | Nenhum arquivo de `hud/` acima de 1.500 linhas |
| AP6 | `feat/app-launcher-*` | Em passos: (a) a lógica de domínio que mora na interface do launcher (`Choice::to_duty`, `default_hof`, estação, escolha de viagem, gravação do horário, favoritos, aeroporto mais próximo) para `omsi-launcher-core`, com testes; (b) o jogo deixa de importar `launcher` (`weather_time.rs`, `game_lists.rs`, `server.rs`, `android.rs`); (c) as páginas para `omsi-launcher-ui`, uma por PR, cortando as importações de módulos do jogo; (d) o que sobrar em `omsi-app/src/launcher/` é a ligação com o processo do jogo | `launcher/` no app abaixo de 2 mil linhas; `omsi-launcher-ui` sem dependência do `omsi-app` |
| AP7 | `feat/app-dominios` | As pastas `core/`, `input/`, `world/`, `weather/`, `gameplay/` e `online/`, um PR por pasta, cada uma já como pacote: os tipos vão para `domain/`, o que os outros domínios chamam para `interface/input.rs`, o que o domínio pede de fora para `interface/output.rs` e os procedimentos para `implementation/`. Os quatro arquivos gigantes (`scene.rs`, `traffic.rs`, `schedule.rs` e `humans.rs`) entram em `implementation/` com os tipos já separados em `domain/`; a divisão do resto deles fica para a fase 2 (C4 e E1). `online/` só ganha a hierarquia de pastas, sem reescrever o código desligado | Nenhum arquivo de código na raiz além de `lib.rs` e `main.rs`; cada pasta com as três partes |

AP1 vem primeiro, porque o rustfmt deixa os PRs de movimento legíveis e porque cada PR seguinte já nasce formatado. AP2 vem antes da divisão, pelo mesmo motivo da Trilha S: com os testes fora de `src/`, mover um arquivo deixa de levar os testes junto. AP3 a AP7 não dependem entre si e podem trocar de ordem conforme o que estiver aberto no repositório.

Mudanças que não são só de estrutura ficam fora desta fase, como PRs próprios:

- **Aposentar o desenho em CPU do HUD** (o passo 1 da C6, reescrito): levar o chat, os avisos, os nomes, o tutorial, o horário, a barra de informações e a lista de escolha para `omsi-ui` muda o que aparece na tela. São PRs da trilha da nova interface (#166), com capturas antes e depois. Quando nada mais usar `hud/legacy/`, ele vai para trás de uma feature desligada, sem ser apagado, e a cópia da Roboto sai.
- **O "OPENOMSI" do menu** é um `fix/` de uma linha.
- **O launcher antigo como alternativa** atrás de uma feature, até a regressão do launcher novo passar, segue o passo 4 da migração em `docs/NEW_UI_FOUNDATIONS.md`.

### Fase 2: áreas quentes (a antiga Trilha 4)

A ordem e o conteúdo são os da [Trilha 4](MACRO_REFACTOR_DRAFT.md#trilha-4-áreas-quentes-por-janelas), com o método de janelas curtas e anunciadas:

1. **Perfil por escopo** (`enum Stage` e uma guarda só, no lugar dos 29 `profile.entry` de `app_events.rs`).
2. **Frente B** (`Session::build` e `Session::step`, o `run_offscreen` abaixo de 400 linhas, as três divergências em `fix/`). Com AP4 feito, o `Session::step` não precisa saber do backend; a escolha fica no `frame_render`.
3. **C2** (`App` em sub-structs, `view` como `enum View`, o frame em métodos `frame_*`) e o **passo 2 da C6** (linhas tipadas no lugar do texto separado por `\u{1f}`, que agora alimentam também `omsi_ui::menu_page`).
4. **C4** (`world/scene/` em submódulos e as heurísticas de material de veículo como `fix/`).
5. **D1 e D3** do lado do app (`settings/` lendo a tabela compartilhada; nenhum `Settings::load()` fora da abertura, eram 23).
6. **E1, E3 e E2**: pedidos em vez de chamadas ao renderer; `weather/` com `WeatherState` e `WeatherChoice`; e as extrações de `input/` para `omsi-input` e de `gameplay/` para `omsi-gameplay`. A linha `omsi-launcher` da E2 deixa de existir: o launcher sai do app pela AP6.

## Onde esta trilha encosta nas outras

- **Trilha 3.** O fix-25 ainda tem os `unsafe` do `omsi-app` por comentar (124, com 22 `SAFETY`). Para não brigar com o rustfmt, a parte do app do fix-25 vem depois de AP1, ou antes dele por inteiro.
- **Trilha S.** Sem arquivo em comum. A AP7 só move os arquivos do app que usam `omsi_sim::traffic` e `omsi_sim::people`.
- **Nova interface (#166) e configurações (#151).** São trabalho de produto, não refatoração, e têm prioridade: a AP3, a AP5 e a AP6 só acontecem sem PR de interface aberto nos mesmos arquivos. A D1 e o registro de configurações do #151 são o mesmo trabalho.
- **Renderer.** O contrato que o Direct3D 9 usa (`FramePlan`, `MirrorEvent`, `AssetMirror`) é do `omsi-render` e fica fora desta trilha; a AP4 só organiza o lado do app.
- **Correções do launcher.** Os PRs `fix/launcher-*` abertos (#244 a #280) mexem em `omsi-launcher-core/src/lib.rs` e em `settings/round_trip.rs`. O rustfmt da AP1 toca o segundo, então eles entram antes dela ou são portados com o `scripts/fmt-merge.sh`; a AP6 e a D1 esperam por eles.

## Decisões

Tomadas pelo mantenedor em 23/10/2026:

- **Testes:** fora de `src/`, como padrão do projeto (ver a [convenção de testes](#convenção-de-testes)).
- **Quando começa:** já, pela AP1.
- **Destino do launcher:** `omsi-launcher-ui` para a interface e `omsi-launcher-core` para a lógica, sem crate novo.
- **Nome da pasta da nova interface:** `ui/`, com o HUD antigo em `hud/legacy/` (antes `interface/`, trocado para evitar `interface/interface/`).
- **Hierarquia dos pacotes:** cada domínio criado já sai reestruturado em `domain/`, `interface/` (entrada e saída) e `implementation/` (regra 3 dos princípios).

## Critério de sucesso

- `fmt --check` bloqueante no `omsi-app` (informativo por enquanto, decisão de 24/10/2026), no `omsi-launcher-ui`, no `omsi-launcher-core` e no `omsi-render-d3d9`.
- Nenhum arquivo de código na raiz de `src/` além de `lib.rs` e `main.rs`; cada domínio com `domain/`, `interface/` e `implementation/`, e nenhum import entre domínios que não passe pela interface; nenhum teste em `src/`.
- A nova interface sem dependência do desenho em CPU do HUD; o backend gráfico escolhido num ponto só.
- O launcher fora do app, a não ser a ligação com o processo do jogo.
- No fim da fase 2, os portões da antiga Trilha 4: traço B0 idêntico em cada PR, `run_offscreen` abaixo de 400 linhas, `omsi-gameplay` compilando e testando sem `wgpu`, `omsi-app` abaixo de 50 mil linhas na configuração padrão, nenhum arquivo da cena acima de 3.000 linhas e nenhuma divergência conhecida no teste de ida e volta do `settings.cfg`.
