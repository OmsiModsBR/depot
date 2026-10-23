# Macro-refatoração do depot (rascunho)

Rascunho de 10/10/2026, fechado para as trilhas 0 e 1 em 11/10/2026. Revisto em 16/10/2026 com a auditoria estática do código (seção [Auditoria de 16/10/2026](#auditoria-de-16102026)). Revisto de novo em 23/10/2026 (seção [Revisão de 23/10/2026](#revisão-de-23102026)), com a nova interface, o launcher como processo próprio e o renderer Direct3D 9, e ganhou a [Trilha App](#trilha-app-omsi-app-por-domínio), que junta tudo o que mexe no `omsi-app`, inclusive o código novo da interface, do launcher e da saída Direct3D 9.

## Revisão de 23/10/2026

Revisão do plano contra o `main` em `e99b624` (PR #398), a issue #61 e os PRs abertos.

**O que terminou desde 16/10/2026.** A Trilha F (F0 a F3, PRs #144 a #178), a Trilha S (S1 a S5, até o #386) e, da Trilha 3, D5, D6, C5 e o passo 3 da C6. Na Trilha 3 sobram o fix-25 e o passo 1 da C6, que esta revisão reescreve. A Trilha 4 não começou.

**O que entrou e o plano não previa.**

- **A nova interface** (#166, PRs de cerca de #250 a #380). O menu Esc e as páginas atrás dele, a barra Alt e suas janelas, a janela de adicionar veículo, o relatório de fim de viagem e a tela de carregamento são desenhados por `omsi-ui`, a partir de `omsi-app/src/new_ui*.rs` (6,8 mil linhas), também na tela tripla e no VR. O foco por teclado do kit entrou em `omsi_ui::nav` (#321), desligado até uma página pedir. O desenho está em `docs/NEW_UI_FOUNDATIONS.md`.
- **O launcher como processo próprio.** Sem argumentos, o binário abre só o launcher, com o pipeline da interface e sem cena, scripts ou áudio; a sessão é um processo filho, e a prévia do ônibus vem de um ajudante (`--preview-helper`). Entrou o crate `omsi-launcher-ui` (a barra lateral, as páginas de veículos e mapas e os tokens do `look.rs`), mas `omsi-app/src/launcher/` cresceu de 12,8 mil para 19,2 mil linhas.
- **O renderer Direct3D 9Ex** (`omsi-render-d3d9`, PRs #341 a #358 e #371), uma opção de renderização desligada por padrão, só no Windows. Nesse modo o `omsi-render` roda num dispositivo que não desenha e faz a contabilidade (cena, culling, nível de detalhe, ordem de alpha); ele entrega o plano de cada quadro e os eventos de asset (`set_plan_sink`, `set_asset_mirror`, `api/mirror.rs`) ao crate novo, pela cola em `omsi-app/src/d3d9_output.rs`.

**Números de hoje.** O `omsi-app` tem 144,3 mil linhas (eram 124,8 mil), o `omsi-ui` 13,9 mil (eram cerca de 4,6 mil), o `omsi-launcher-core` 10,4 mil (eram 6,0 mil), o `omsi-sim` 42,0 mil, o `omsi-render` 28,6 mil, e os crates novos têm 3,2 mil (`omsi-launcher-ui`) e 6,5 mil (`omsi-render-d3d9`). O alvo de "`omsi-app` abaixo de 50 mil linhas" ficou mais longe.

**O que muda no plano.**

1. **Uma trilha só para o `omsi-app`**, a [Trilha App](#trilha-app-omsi-app-por-domínio), pedida pelo mantenedor: a formatação do crate, os testes fora de `src/`, a nova interface, a saída gráfica, o HUD, o launcher e as pastas por domínio na fase 1; a Trilha 4 passa a ser a fase 2 dela.
2. **O passo 1 da C6 é reescrito.** Levar o menu do jogo para o kit, um de cada vez, já aconteceu por outro caminho. O que sobra é separar os dados e os retângulos de clique do HUD do desenho em CPU de `ui.rs` (Trilha App) e aposentar esse desenho (trilha da nova interface). Ver [C6](#c6-uma-pilha-de-ui-nova-16102026).
3. **A D1 é o registro de configurações do #151.** A tabela `SettingSpec` passa a alimentar quatro consumidores: o jogo, o launcher antigo, as páginas novas do launcher e as janelas de configuração do menu Esc.
4. **A linha `omsi-launcher` da E2 sai.** O launcher deixa o app para `omsi-launcher-ui` (interface) e `omsi-launcher-core` (lógica), no passo AP6 da Trilha App.
5. **Entra a C7**, o contrato do segundo renderer: o plano de quadro e os eventos de asset do `omsi-render` documentados e testados, e as constantes que Rust, WGSL e HLSL compartilham num lugar só. Ver [C7](#c7-o-contrato-do-segundo-renderer-nova-23102026).
6. **A rede de segurança não cobre o Direct3D 9.** O golden roda no lavapipe, sobre o wgpu, e os testes de dispositivo do Direct3D 9 só rodam no Windows com driver; o job do Windows no CI é informativo. O frame passa a ter três caminhos (janela no wgpu, janela no Direct3D 9 e offscreen), o que reforça a Frente B.

**Sobras registradas dos itens concluídos.** Da trilha 0, o `offscreen-golden` ainda é informativo. Da C5, o `GrowBuffer`, o reaproveitamento dos bind groups do overlay no VR, as partes de `Renderer::open_on` que só falam com o wgpu (para `gpu/`) e os `Vec` locais que ainda restam em `draw_lists`. Da D6, o índice de texturas que pula `.jpeg`, se isso vira `fix/`. Da Trilha S, `people/omsi_anim.rs::advance` (206 linhas). Da A2, o `scripts/fmt-merge.sh` e o `.git-blame-ignore-revs`, que nunca foram criados; os crates fora da lista bloqueante do `fmt --check` são `omsi-app`, `omsi-launcher-core`, `omsi-launcher-ui`, `omsi-render-d3d9`, `omsi-html`, `omsi-net` e `omsi-plugin`. A3 e A4 não começaram. A parte de testes de `.o3d` do fix-25 já foi feita pelo passo 1 da D6 (#208).

## Revisão de 16/10/2026

As trilhas 0, 1 e 2 terminaram entre 12 e 13/10/2026 (PRs #62 a #128). A auditoria de 16/10/2026 leu o `main` em `c113ab2` (cerca de 257 mil linhas em 426 arquivos `.rs`, agrupados em 25 segmentos, dos quais os 10 mais complexos foram revistos em detalhe) e mudou o plano em quatro pontos:

1. **A física de veículo ainda tem uma rodada pela frente.** A conversão do `.bus` em parâmetros físicos está copiada em quatro lugares, e o jogo ainda compensa modelos mal configurados na física. Isso vira a nova [Trilha F](#trilha-f-física-de-veículo-segunda-rodada), em quatro PRs, que vem antes do restante da macro-refatoração, como o projeto exige. Ela inclui a física do tráfego IA, só como refatoração: testes próprios provam que a IA se comporta igual no jogo.
2. **O maior risco estrutural é o offscreen não rodar o código da janela.** O golden e o traço de estado, que são a rede de segurança da refatoração, só exercitam o offscreen, e as duas versões do frame já divergem em três pontos. A Frente B sobe de prioridade dentro da trilha das áreas quentes.
3. **Entram frentes novas** que a auditoria achou e o rascunho não tinha: divisão do `scene.rs` (C4), pendências do renderer e dos shaders (C5), uma pilha de UI só (C6), utilitários do `omsi-cfg` (D5), leitura de formatos só nos crates de formato (D6) e um módulo de clima (E3).
4. **Sete bugs achados de passagem viram PRs `fix/`**, fora da refatoração. Os quatro mais graves já estão sendo corrigidos em PRs separados, a partir do #140.

O antigo plano em quatro trilhas continua abaixo; a seção [Plano em trilhas](#plano-em-trilhas) foi atualizada com o que está feito e com a ordem nova.

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

## Auditoria de 16/10/2026

Leitura estática do `main` em `c113ab2`, sem compilar. Os números de linha são desse commit e devem ser conferidos antes de cada PR. Segmentos revistos em detalhe: núcleo e efeitos do renderer, formatos e conteúdo, física de veículo, núcleo do app, entrada, launcher, mundo e clima, UI e HUD, cena. Os outros 15 (dados do mundo, online e plugins, script e HTML, serviço e linhas, rede, humanos, tráfego, ferramentas e CI, entre outros) foram só mapeados.

### O que ela encontrou

1. **Não existe um núcleo de simulação compartilhado.** `run_offscreen` (`offscreen.rs`, uma função de cerca de 3.030 linhas com uns 40 `let mut` que copiam campos do `App`) recria a montagem da sessão e o passo por frame em vez de chamar o código da janela. As versões já divergem em três pontos: o brilho e os mips dos LEDs faltam na luz do offscreen, o trilho dado a `signal_aspects` é diferente, e a densidade de pedestres e os alvos das paradas são definidos uma vez no offscreen e a cada frame na janela. Como o golden e o traço de estado só rodam o offscreen, eles não protegem o caminho da janela. Vai para a Frente B.
2. **Três god objects bloqueiam os movimentos grandes:** `App` (cerca de 180 campos `pub(crate)`, alterados em 17 arquivos), `scene::World` (76 campos, 67 atrás de locks) e o braço `RedrawRequested` de `app_events.rs` (cerca de 2.830 linhas, o frame inteiro). Vão para as Frentes C2 e C4.
3. **A conversão de `.bus` para física está espalhada.** Heurísticas de unidade e valores padrão inventados para parâmetros ausentes aparecem copiados em `rigid/`, `trailer.rs`, `vehicle_type.rs` e `physics.rs`, às vezes com pisos diferentes: a heurística de massa e inércia aparece cinco vezes, e uma das cópias (`motion.rs:308`) usa piso 0 em vez de 500 kg. Corrigir só `body.rs` deixaria três cópias. Vai para a Trilha F.
4. **O jogo ainda compensa modelos mal configurados**, contra a decisão do projeto. Na física: `[rot_pnt_long] 0` lido como "ausente" e trocado pelo eixo mais distante (`trailer.rs:339`), acoplamentos padrão quando faltam `[coupling_front]`/`[coupling_back]` (`trailer.rs:191`), `[boundingbox]` cortada ao tamanho da malha (`vehicle_type.rs:376`), altura do cubo da física tirada da malha da roda (`vehicle_type.rs:473`) e o piso `MIN_LOAD_SHARE` por roda. No render do veículo (`scene.rs:10938-11107` e `12730-13006`): alpha, noZwrite, vidro e "carroceria" adivinhados pelo nome da textura, e `repair_body_depth`, hoje opcional por `OMSI_REPAIR_BODY_DEPTH`, ainda compilado. Física na Trilha F; render na Frente C4.
5. **Settings tem duas fontes de verdade, e na prática quatro:** o `Settings` do jogo (142 campos), o JSON solto do launcher (44 chaves acessadas por texto), o escritor próprio de `game_lists.rs` e as listas de opções copiadas no menu do jogo. O `round_trip.rs` da T0.3 já documenta 51 divergências, e `Settings::load()` lê o disco em uns 20 lugares, um deles dentro do frame. Vai para a Frente D1.
6. **Formatos OMSI são lidos fora dos crates de formato.** O `gamectrler.cfg` que o jogo usa é lido em `omsi-app/src/controllers.rs`, e o de `omsi-content` só serve ao `omsi-check`. Também são lidos à mão `[cloudtype]`, fontes `.oft`, o `[name]` no launcher e os textos de tile do editor. A decodificação de texto está dividida em três caminhos (com BOM, sem BOM, 1252 fixo), e o escritor UTF-16 aparece copiado quatro vezes. Vai para as Frentes D5 e D6.
7. **O jogo tem duas pilhas de UI**: `ui.rs` rasteriza na CPU, `omsi-ui` desenha na GPU, e `hud.rs` tem um terceiro caminho de texto. Um kit completo de widgets imediatos já existe em `launcher/ui.rs`. Vai para a Frente C6.
8. **No renderer, a duplicação está nos shaders e no código repetitivo de wgpu.** O struct Camera aparece cinco vezes em WGSL, o vertex shader de tela cheia nove e o FXAA duas. Oito passes de tela cheia não usam `post_pass`, `api/` ainda tem lógica e passes que o `crates/omsi-render/ARCHITECTURE.md` diz que não deveria ter, e o script de camadas não enxerga imports agrupados. Vai para a Frente C5.
9. **Há alocações por frame em quase todos os segmentos**: `Controllers::poll`, `update_scripted`, `sync_materials`, `draw_lists` (mais de 15 `Vec` e um `HashMap` por frame), bind groups do RT e da UI do VR, streaming de tiles, e no solver da física a cada substep.
10. **Helpers pequenos estão duplicados no workspace:** o caminho `~/.depot` (cerca de 10 crates e 21 `var_os("HOME")` só no app), `smoothstep` (4 crates), xorshift (4), a fórmula de Magnus (4, com constantes diferentes), sRGB (3), gravação atômica tmp+rename (7) e o "mover o ônibus para cá" (3, em `teleport.rs`, `lan.rs` e `placing.rs`). Sete documentos ainda citam `~/.openomsi`. Vai para a Frente D5.
11. **Texto no lugar de tipos:** `view: String` (32 comparações), `args.weather` com prefixos (`metar:`, `custom:`, `cycle`), ids de ação como `"tour X\u{1}Y"`, linhas de menu separadas por `\u{1f}` e o tipo de seta `"L"`/`"R"`/`"busstop"`. A proposta é trocar por enums (`View`, `WeatherChoice`, `MenuAction`, `ArrowKind`) nas frentes que mexem em cada um. O menu ainda mostra "OPENOMSI" (`ui.rs:1430`).

A detecção automática de nomes duplicados achou 124 pares, quase todos falsos (`run`, `names`, `row`); as duplicações acima vêm da leitura. Os desvios de nome apontados são testes e FFI, então a convenção de nomes do Rust já é seguida.

### Bugs achados de passagem

Não são refatoração. Cada um é um PR `fix/` próprio, na fila normal e com prioridade sobre qualquer item deste plano.

| # | Onde | Problema | Situação |
| --- | --- | --- | --- |
| B1 | `omsi-render/src/renderer/prepare.rs:213,248` | O fallback sem MSAA ou sem RT recria o `Renderer` inteiro e perde o que o jogo já enviou: texturas de corona e fumaça, tiles do atlas de light-map, `dynamic_scale` e as opções em tempo de execução | corrigido |
| B2 | `omsi-app/src/condensation.rs:236` | Umidade absoluta (g/m³) usada como ponto de orvalho (°C), o que embaça os vidros cedo demais no frio | corrigido |
| B3 | `omsi-app/src/world_load.rs:18` | `season_folder` carrega o clima só para ler a neve: liga o modelo global, escreve globais e, com `metar:`, busca o METAR uma segunda vez com bloqueio de até 6 s | corrigido |
| B4 | `omsi-app/src/launcher/pages.rs:2085` | `save_gamectrler` grava sem rename atômico e sem a checagem de ida e volta, e sem pasta de conteúdo salva em `~/.depot/Inputs`, que o jogo não lê | corrigido (#140) |
| B5 | `omsi-render/src/output/diagnostics.rs` | `GpuTimers::collect` lê o buffer mapeado sem a proteção contra pânico depois da perda do dispositivo | corrigido: a leitura passa por `catch` |
| B6 | `omsi-app/src/weather_time.rs:101` | `change_weather` busca o METAR na thread principal | a conferir (23/10/2026) |
| B7 | `omsi-render/src/passes/depth.rs` | O blur do SSAO usa o rótulo `"ssao"` do passe anterior | aberto: o tempo já se chama `"ssao blur"`, o passe continua `"ssao"` |

### Backlog da auditoria e onde cada item entra

Os ids `fix-N` são do backlog da auditoria, não nomes de branch. Prioridade P0 (bug) a P3 (limpeza); esforço S, M ou L.

| Id | Prio | Item | Entra em |
| --- | --- | --- | --- |
| fix-1 a fix-4 | P0 | Bugs B1 a B4 | PRs `fix/`, já em andamento |
| fix-5 | P1 | Um `PhysicsSpec::from(&Vehicle)` onde unidades e valores ausentes são decididos uma vez | Trilha F |
| fix-6 | P1 | Tirar da física as compensações de modelo (item 4 acima) | Trilha F |
| fix-7 | P1 | Colisão dentro do substep, substep de tamanho fixo, buffers do solver reaproveitados | Trilha F |
| fix-8 | P1 | Fechar os campos de `RigidBody`/`RigidWheel`; `VehicleInstance::teleport` | Trilha F |
| fix-9 | P1 | `DriveGround`, `drive_probe` e a montagem de colisão de `scene.rs:2549-2965` para `scene/ground_query.rs` | Trilha F |
| fix-10 | P1 | Tirar do jogo as heurísticas de material de veículo; `repair_body_depth` atrás de feature, sem apagar | C4 |
| fix-11 | P1 | Um passo de simulação para janela e offscreen | B |
| fix-12 | P1 | `App` em cerca de 14 sub-structs; `view` vira enum | C2 |
| fix-13 | P1 | Frame em métodos `frame_*`; uma guarda única de perfil | C2 |
| fix-14 | P1 | `scene.rs` em cerca de 18 submódulos, começando por `vehicle_render/` | C4 |
| fix-15 | P1 | Schema único de settings (`SettingSpec`); carregar uma vez | D1 |
| fix-16 | P2 | `omsi_cfg::paths`, `write_atomic`, um decode e um encode de texto; `omsi-cfg/src/lib.rs` em módulos | D5 |
| fix-17 | P2 | `gamectrler.cfg` lido e escrito só em `omsi-content` | D6 |
| fix-18 | P2 | Crate `omsi-input` com `trait ControllerBackend`; `on_key` em etapas | E2 |
| fix-19 | P2 | Módulo `weather/` com `WeatherState` e `WeatherChoice` | E3 |
| fix-20 | P2 | Lógica e passes fora de `api/`; `post_pass` mais amplo; script de camadas com imports agrupados | C5 |
| fix-21 | P2 | Includes WGSL comuns (Camera, VS de tela cheia, sRGB, FXAA, guarda contra NaN) | C5 |
| fix-22 | P2 | Um kit de widgets só, em `omsi-ui`, usado também pelo menu do jogo | C6 (o menu do jogo já é do `omsi-ui` desde a nova interface; ver a revisão de 23/10/2026) |
| fix-23 | P2 | `navigator.rs` em `nav/`; o caminho mais curto para `omsi-sim` | C6 |
| fix-24 | P3 | Alocações por frame | Fase de performance do mapa, item a item |
| fix-25 | P3 | Comentários `SAFETY` nos `unsafe`, `bytemuck` no lugar de slices à mão, testes de `.o3d` | Trilha 3 (os testes de `.o3d` entraram na D6, #208; `omsi-render` feito no #398) |

### Regras que valem para o backlog

- **Física de veículo primeiro.** fix-5 a fix-9 terminam antes dos itens da macro-refatoração geral (Trilha F).
- **Física do tráfego IA na refatoração, sem mudar comportamento** (decisão de 16/10/2026). O código da IA é reorganizado na Trilha F com testes próprios que provam que ela se comporta igual no jogo. As compensações que só existem na IA (`ai_lift` em `vehicle.rs:337-355`, `ai_rest_offset` em `visuals.rs:197-204`, o limite de ±1,2 rad no ângulo de eixo em `vehicle/ai.rs:90-94`) mudam de lugar e ficam como estão; a troca de modelo da IA continua pausada.
- **Sem compensar modelos.** Tirar uma compensação muda o comportamento de propósito: é um PR `fix/` com o delta previsto, e o modelo errado passa a aparecer como está.
- **Nada se apaga.** Código que deixa de ser usado, como `repair_body_depth`, vai para trás de uma feature de Cargo desligada.

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

O `omsi-app`, junto com o `scripts/fmt-merge.sh`, o `.git-blame-ignore-revs` e os crates que entraram depois (`omsi-launcher-ui`, `omsi-launcher-core` e `omsi-render-d3d9`), é o passo AP1 da [Trilha App](#trilha-app-omsi-app-por-domínio).

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

**Revisão de 16/10/2026.** A auditoria confirmou que esta é a frente de maior risco e achou mais duas divergências além de `signal_aspects`: o brilho e os mips dos LEDs, que faltam na luz do offscreen, e a densidade de pedestres e os alvos das paradas, que o offscreen define uma vez e a janela a cada frame. Mudanças no desenho:

- Além do passo, a montagem também é compartilhada: `Session::build(args, settings, world, renderer, scene)` substitui a cópia de `app.rs:671-920` em `offscreen.rs:88-249`. Os blocos por frame a unificar estão em `app_events.rs:585-723` e `1475-1660` e em `offscreen.rs:583-1141`.
- B6 passa a cobrir as três divergências, cada uma no seu PR `fix/`.
- O teste que fixa a divergência de `signal_aspects` procura texto no código-fonte (`tests/offscreen_golden.rs:269-279`); com B1 ele vira um teste da chamada única.
- As verificações de mapa que moram em `offscreen.rs` (linhas 2050 a 2520 e 2820 a 2996) vão para `diagnostics/map_checks.rs`, para que `run_offscreen` chegue às 400 linhas do portão.
- O passo do jogador fica no meio do frame, então B1 a B5 começam depois da Trilha F.
- Um `lan_off::answering_while` que só repassa a chamada tira as chamadas duplicadas com `cfg(feature = "lan")` de `offscreen.rs:74-120`, e um tipo vazio para o VR fora do Windows faz o mesmo com os 21 `cfg` de `app_events.rs`.

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

**Revisão de 16/10/2026.** Hoje são cerca de 180 campos `pub(crate)`, alterados em 17 arquivos. A auditoria propõe 14 sub-structs, cada um com `Default`, o que reduz `make_app` a umas 20 linhas: `Gfx`, `FrameStats`, `MenuState`, `Pointer`, `DriveInput`, `Keys`, `ViewState`, `HeadTrack`, `Mirrors`, `Environment`, a sessão da Frente B, `PluginsState`, `EditorState` e `SessionLog`. Junto, `view: String` (32 comparações de texto) vira `enum View`. O braço `RedrawRequested` (`app_events.rs:352-3186`) vira `App::frame(dt)`, que chama `frame_governor`, `frame_traffic`, `frame_input_devices`, `frame_player_camera`, `frame_humans`, `frame_environment`, `frame_hud` e `frame_render`; as cerca de 30 medições manuais de tempo passam por uma guarda única (`ProfileScope`, ou o `stage!` que `humans.rs` já tem), que é o mesmo `enum Stage` da Frente B.

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

C3 foi concluída em 12/10/2026 (PR #84). O que sobrou dela para C2: `on_key` (`input/keyboard.rs`, cerca de 590 linhas) vira uma cadeia ordenada de etapas `fn(&mut App, &KeyEvent) -> Handled` (editor de espelhos, menu inicial, chat, VR, menu do jogo, editor, tutorial, atalhos globais, direção), com um struct `Modifiers` calculado uma vez no lugar das 20 checagens de Shift, Ctrl e Alt.

### C4. `scene.rs` em submódulos (nova, 16/10/2026)

O `omsi-app/src/scene.rs` tem 14.918 linhas e o `place_step`, 1.252. A auditoria mapeou cerca de 18 submódulos pelos intervalos de linha do arquivo. A ordem proposta:

1. **`scene/ground_query.rs`**, com `DriveGround`, `drive_probe` e a montagem de colisão (`scene.rs:2549-2965`). Este passo é da Trilha F (fix-9), porque é a fronteira entre a cena e a física; depois ele pode descer para `omsi-sim`.
2. **`scene/vehicle_render/`**, com o upload e os materiais de veículo (`scene.rs:10082-13139`), em PR de movimentação pura.
3. **Heurísticas de material de veículo fora do jogo** (fix-10, `scene.rs:10938-11107` e `12730-13006`): alpha, noZwrite, vidro e carroceria passam a valer como o modelo declara em `[matl_alpha]` e `[matl_noZwrite]`, e `repair_body_depth` com seus helpers vai para trás de uma feature de Cargo (por exemplo `vehicle-heuristics`), sem ser apagado. É mudança de comportamento: um PR `fix/` por heurística, com o golden mostrando a diferença esperada.
4. **O resto dos submódulos**, um PR por grupo. Junto: um cache genérico `RefCounted<K, V>` no lugar das três famílias de quatro cópias de recycle/release (`scene.rs:1229-1706`, `12000-12330`), o `FreeList` e o orçamento de textura para `omsi-render`, a leitura de `ObjectType` para `omsi-content` com um `mesh_path` compartilhado com `vehicle_type.rs:165`, e índices de variáveis e chaves em minúsculas guardados no upload em vez de recalculados por frame (`scene.rs:9728-10026`, `10260-10450`, `2420-2547`).

`scene::World` (76 campos, 67 atrás de locks) é dividido junto com os submódulos, cada um levando os campos que só ele usa. Portão: nenhum arquivo da cena acima de 3.000 linhas, golden e B0 idênticos nos PRs de movimentação.

### C5. Pendências do renderer (nova, 16/10/2026)

O `omsi-render` foi dividido em domínios e camadas nas trilhas 1 e 2 (T1.4 e T1.6, PRs #70 a #111), e a checagem de camadas bloqueia no CI. A auditoria achou o que ficou para trás:

- **Lógica fora de `api/`** (fix-20). `prepare` vai para `renderer/upload.rs`; a composição de três telas e seus targets para `output/triple.rs`; `render_xr_ui` para `output/xr.rs`, reaproveitando os bind groups do overlay; `new_on` para `gpu/open.rs::open_device`; `drop_top_levels` e `upload_speed_mb_s` para `assets/texture.rs`. Cada método de `api` fica com menos de 10 linhas.
  Vai em passos curtos, um por destino. No passo 2a, o corpo de `prepare` foi para `renderer/upload.rs` como `Renderer::upload_instances`, junto com `instance_entries`, que estava em `renderer/prepare.rs`; o corpo não mudou, e `api::prepare` só chama `upload_instances`. No passo 2b, a tela tripla saiu de `render_triple`: os targets dos painéis e a composição com o HUD viraram `Presenter::ensure_triple_targets` e `Presenter::composite_triple` em `output/triple.rs`, e o laço dos três painéis ficou em `renderer/output.rs` como `Renderer::render_triple_frame`, que chama os dois. Como `output` fica abaixo de `renderer`, o `Presenter` recebe device, fila, formato e sampler por parâmetro; o resto do código não mudou. No passo 2c, `render_xr_ui` foi inteiro para `output/xr.rs` como `Presenter::render_xr_ui`, que recebe device, fila e sampler; o corpo não mudou. Reaproveitar os bind groups do overlay, como o plano pede, muda o que é alocado a cada frame e fica para um passo à parte. No passo 2d, o corpo de `new_on` (cerca de 460 linhas: a memória de textura e as opções que o adaptador aguenta, as features e os limites do device, e as pipelines refeitas com menos quando o driver falha) foi para `renderer/open.rs` como `Renderer::open_on`, sem mudar. Ele ficou em `renderer` e não em `gpu/open.rs`, como o plano dizia, porque mexe em `RenderOptions` e chama `Renderer::build`, que estão acima de `gpu`; separar as partes que só falam com o wgpu fica para depois. No passo 2e, os corpos de `drop_top_levels` e `upload_speed_mb_s` foram para `assets/texture.rs` como funções livres que recebem device e fila; `drop_top_levels` devolve a textura menor, e o método de `api` só a coloca na cena. O código copiado não mudou.
- **Script de camadas** passa a entender imports agrupados (`use crate::{a, b}`), que hoje escapam da checagem.
  Feito no passo 1: o script lê cada `crate::` com um `perl` pequeno, tanto em caminho (`crate::gpu::Gpu`) quanto dentro de um grupo, em várias linhas e aninhado, e aponta a linha do nome. Na árvore de hoje a lista de imports sai igual à de antes (os dois grupos que existem só nomeiam itens da raiz), então nada novo aparece.
- **Helpers de wgpu** em `gpu/util.rs`: `target_2d`, `fullscreen_pipeline`, os grupos textura+sampler, um `GrowBuffer`, `mip_levels` e `Vertex::LAYOUT`. `post_pass` vira um builder e passa a servir aos oito passes de tela cheia que ainda não o usam e a `generate_mip_chain` e `encode_present`.
  Vai em passos curtos, um helper ou dois por vez. No passo 3a, `gpu/util.rs` ganhou `target_2d`, que cria uma textura 2D de um nível de mip sem outros formatos de view, e `mip_levels`, a conta da cadeia completa de mips. `target_2d` trocou os 29 descritores de textura iguais espalhados por `api`, `renderer`, `output`, `effects`, `passes`, `targets`, `sky`, `lighting`, `assets` e `gpu`, e `mip_levels` as três cópias da conta; rótulos, tamanhos, formatos e usos ficaram os mesmos. No passo 3b, `fullscreen_pipeline` (um triângulo na tela toda, sem vertex buffer, sem profundidade e com um alvo de cor) trocou nove descritores de pipeline: os mips, o traçado e o desfoque das poças, o upscale, o overlay, a UI do VR, o pós-processamento (bloom, exposição, tonemap e FXAA), o SSAO, os faróis de neblina e o cubo do céu. Os que têm profundidade, vertex buffer, constantes de shader ou mais de um alvo ficam como estão. No passo 3c, as entradas de layout de bind group que se repetiam viraram `texture_entry` (textura 2D filtrável no fragment shader, 21 cópias), `sampler_entry` (14), `depth_texture_entry` (4) e `uniform_entry` (11, com os estágios por parâmetro, mais o closure igual de `sky/resources.rs`); as entradas com outro tipo, outros estágios ou tamanho mínimo ficam escritas por extenso. No passo 3d, `Vertex::LAYOUT`, ao lado do `Vertex` em `assets/ids.rs`, trocou as três cópias do layout do vértice de cena (o pipeline principal e os dois das poças); os layouts das coronas e do céu têm um uso só e ficam onde estão. O `GrowBuffer` não entrou: os buffers que crescem (luzes, modelos e parâmetros, lista de desenho e os dois do traçado de raios) têm cada um sua regra de capacidade e seu efeito colateral, e unificar a regra mudaria o tamanho das alocações. No passo 3e, `post_pass` virou o builder `PostPass` em `passes/draw.rs` (rótulo e view, com `load` e `timer` opcionais, e `draw` ou `begin` para quem desenha mais que o triângulo): além dos oito usos de antes, ele abre a composição do traçado de raios, os mips (`generate_mip_chain`), o upscale (`encode_present`), o tonemap e o FXAA, os faróis de neblina, o SSAO, a apresentação das reflexões clássicas, o cubo do céu e a composição da tela tripla. A sonda de reflexão, com vários alvos, e o menu do VR, que desenha quadrados, ficam escritos por extenso.
- **Shaders comuns** (fix-21) em `shaders/common/`: `camera.wgsl` (Camera, PointLight e `CELL_CAP`, hoje em cinco cópias), `fullscreen.wgsl` (nove cópias do vertex shader), `math.wgsl` (PI, guarda contra NaN, hashes), `fxaa.wgsl` (duas cópias) e o sRGB de `colour.wgsl`. Um teste confere que cada campo do Camera em WGSL tem o mesmo offset que em `CameraUniform`.
  Vai em passos, um arquivo comum por vez. No passo 4a, `shaders/common/camera.wgsl` ganhou o `Camera`, o `PointLight` e o `CELL_CAP`, e os shaders da cena, do céu, da neve, dos faróis de neblina e das coronas passaram a incluí-lo no lugar das suas cópias (as coronas tinham só o começo do struct e agora têm o inteiro). O teste `camera_wgsl_matches_camera_uniform` compara o offset de cada campo com `CameraUniform` e o `CELL_CAP` com `LIGHT_CELL_CAP`. No passo 4b, `shaders/common/fullscreen.wgsl` ganhou `FullscreenVertex` e `fullscreen_vertex`, o triângulo de tela cheia com o uv, no lugar das nove cópias: SSAO, pós-processamento, upscale, mips, poças, composição do traçado de raios, sonda do céu, faróis de neblina e a limpeza das sombras. `shaders::with_fullscreen` põe o arquivo à frente dos shaders que vêm de um arquivo só. Os pontos de entrada e os nomes dos structs ficam os mesmos (com `alias` onde o struct era igual). No passo 4c, `shaders/common/math.wgsl` ganhou o `PI` (antes em `enhanced_common.wgsl` e `rt_common.wgsl`), o `all_finite` da cena e `finite_clamped`, que as poças e os reflexos do traçado de raios usam no lugar das suas cópias da guarda contra NaN (cada um com o seu teto, 65000 e 60000). O pós-processamento passou a usar `srgb_encode`/`srgb_decode` de `colour.wgsl` no lugar de `to_srgb`/`from_srgb`, que davam o mesmo resultado nas entradas que recebem (sempre entre 0 e 1). Os hashes e as duas `luma` ficam onde estão: as fórmulas são diferentes. O FXAA só existe no upscale, então não há o que juntar.
- **`FrameContext`**, que a T1.4 deixou de fora: `begin_frame` devolve um `FrameFeatures` no lugar de 21 booleanos, e `draw_lists` se divide em `shadow_casters`, `prepass_batches` e `main_batches`, com os `Vec` temporários num `FrameScratch` guardado no `Renderer`.
  No primeiro passo, `FrameFeatures` (em `renderer/frame.rs`) junta as treze flags que dizem o que a imagem desenha (`vanilla_fxaa`, `glass_on`, `scaled`, `xr_view`, `lead_view`, `enhanced_frame`, `enhanced`, `puddles_wanted`, `reflection_frame`, `masked_frame`, `rt_frame`, `ao_on`, `prepass_on`) e o `FrameContext` passa a carregá-lo em `features`. As outras flags (sombras, sobreposições, `probe_redraw`) ficam onde estão. Só mudou a forma dos dados: `begin_frame` monta o struct com os mesmos locais, e as leituras em `lists.rs` e `encode.rs` vão por `f.features.*`. Os passos de `begin_frame` e `draw_lists` continuam a ser divididos depois.
  No segundo passo, `draw_lists` (em `renderer/lists.rs`) se divide em `shadow_casters` (os casters das quatro cascatas), `main_items` (as fases do mundo e o veículo da câmera) e `prepass_items` (a função livre do prepass, rodada em paralelo com o principal). A ordem de escrita na lista de desenho é a mesma de antes: sombras, depois o principal, e por fim o prepass. O `FrameScratch` ainda não entrou: os `Vec` temporários continuam locais a cada quadro, para que a saída não mude.
  No terceiro passo, `begin_frame` (em `renderer/frame.rs`) deixa de carregar os testes de falha do dispositivo: `fake_device_loss` (a perda de `OMSI_FAKE_GPU_ERROR=lost` e o `lost-poll`) roda antes da checagem de dispositivo perdido, e `fake_gpu_errors` (a falta de memória e o alvo que falha) roda depois dela, na mesma ordem de antes. O `FrameScratch` ficou para o passo seguinte, com a prova de que a saída não muda.

  No quarto passo, `FrameScratch` (em `renderer/state.rs`) guarda a lista de desenho de um quadro para o seguinte: `draw_lists` pega o `Vec` do `Renderer` e o limpa antes de usar, e `draw_frame` o devolve depois de submeter o quadro. Só o `Vec` da lista entrou no quarto passo; os outros `Vec` temporários de `draw_lists` continuam locais e ficam para um passo seguinte. Como a lista é limpa antes de cada uso, o conteúdo que o quadro grava não muda: só a capacidade é reaproveitada.

  No quinto passo, os três `Vec<DrawItem>` do quadro (`items` e `cab_items` de `main_items`, e os `items` do prepass) também vão para o `FrameScratch`: `draw_lists` os pega limpos e os devolve ao fim do quadro. Continuam locais os `Vec` de `visible`, de `by_phase` e dos lotes, que saem no `DrawLists` ou são pequenos.

  No sexto passo, dois itens do plano. `MaterialDesc` (em `assets/material.rs`) descreve um material com seus mapas, cor e `MaterialExtra`; `Renderer::add_material_desc` o cria por `add_material_inner`, e as chamadas `add_material_ex`, `_night`, `_lit`, `_env`, `_all`, `_extra`, `_wet`, `add_terrain_material` e `add_terrain_layer_material` passam a montar um `MaterialDesc`, com as mesmas assinaturas públicas. `add_material_full` saiu, porque só a escada usava. As constantes compartilhadas: `CLOUD_FIELD_TILE` (14000 m) vive em `shaders/mod.rs` e `sky_shader_source` a escreve no WGSL, no lugar do `const` que estava em `sky.wgsl`; e os índices dos pipelines de sombra (`shadow_pipe(cascade, kind)`, dois por cascata, com `LAMP_CASCADE` para as tiles das lâmpadas) trocam os números soltos `4 + pipe`, `cascade * 2 + pipe` e `6 + 2 * k + pipe`. Os valores não mudam; o teste de validação do WGSL (naga) passa com o céu gerado.
- **Um `MaterialDesc`** com `Renderer::add_material_desc`, no lugar da escada de `add_material_*`; as variantes antigas ficam como embrulhos finos.
- **Constantes com nome** para o que Rust e WGSL compartilham (modelo de nuvem, códigos de pipeline de sombra).

O fallback de MSAA e RT (B1) é corrigido antes, num PR `fix/`, com `Renderer::rebuild_pipelines` no lugar de `*self = Self::build(..)`. Portão: golden idêntico no lavapipe a cada PR.

### C6. Uma pilha de UI (nova, 16/10/2026)

Hoje o jogo tem `ui.rs`, que rasteriza na CPU, `omsi-ui`, que desenha na GPU, e um terceiro caminho de texto em `hud.rs`. O kit de widgets imediatos completo mora em `launcher/ui.rs`.

1. **Widgets em `omsi-ui`** (fix-22). `launcher/ui.rs` vai para `crates/omsi-ui/src/widgets.rs`, e o menu, as configurações e o menu do jogo de `ui.rs` passam a usá-lo, junto com a rolagem e os cliques dos painéis do navigator. Isso junta seis helpers de quebra e corte de texto, três barras de rolagem, três easings e 25 campos de hitbox. A cópia embutida da fonte Roboto em `ui.rs:13` sai.
   O passo vai em PRs curtos: **1a** move o kit para `omsi-ui` sem mudar nada (`launcher/ui.rs` fica como reexportação, e `omsi-ui` passa a depender do `winit` pelas teclas, modificadores e cursores da entrada); **1b** em diante levam o menu, as configurações e o menu do jogo para o kit, um de cada vez, e por último a cópia da fonte sai.
2. **Configurações do menu por tabela.** O despacho por texto de `game_lists.rs:1042-1919` e `2182-2250`, com cerca de 300 braços de `match`, dá lugar à mesma tabela `SettingSpec` da Frente D1, e a view recebe linhas tipadas em vez de texto separado por `\u{1f}`. Isso tira cerca de mil linhas.
3. **`navigator.rs` em `nav/`** (fix-23): rota, geometria, minimapa, mapa da cidade e linhas. `way_back` e `lanes_near` vão para `omsi-sim` como `Network::shortest_path_to_any`.
   Em dois PRs: **3a** divide o arquivo em `nav/` sem mudar nada (`mod.rs` com o `Navigator` e os tipos que os dois mapas dividem; `minimap.rs`, `city.rs`, `route.rs`, `geometry.rs`, `streets.rs`, `lines.rs`, `draw.rs` e `tests.rs`); **3b** leva a busca do caminho para `omsi-sim`.
4. **Um só arquivo de perfil por ônibus** para `mirror_hud.cfg`, `vr-navigator.json`, `mirrors.cfg` e os assentos, gravado de forma atômica e só no fim da edição, não a cada passo do redimensionamento.
5. **Um catálogo de conteúdo** (mapas, veículos, situações, climas) extraído de `menu.rs`, para os quatro lugares que hoje listam pastas por conta própria.

O "OPENOMSI" do menu (`ui.rs:1430`) é um PR `fix/` de uma linha, sem esperar esta frente.

**Revisão de 23/10/2026.** A nova interface fez o essencial do passo 1 por outro caminho: o menu Esc, as páginas atrás dele (configurações, listas, drop-downs), a barra Alt e suas janelas e a tela de carregamento já são desenhados por `omsi_ui` (`pause_menu`, `menu_page`, `alt_bar`, `alt_windows`, `loading`), e o foco por teclado que o 1b esperava entrou no #321. O `hud.rs` caiu para 202 linhas. O que sobra:

- O `ui.rs` (2.431 linhas) ainda desenha em CPU o chat, os avisos, os nomes, o tutorial, o horário, a barra de informações e a lista de escolha (`draw_menu`, 406 linhas), e é a fonte dos dados e dos retângulos de clique das páginas novas (`new_ui_pages` usa `ui::Frame` e `write_hits`). Separar os dados do desenho é estrutura e entra na Trilha App (AP3 e AP5). Levar o que o `ui.rs` ainda desenha para o `omsi-ui` muda o que aparece na tela e fica com a trilha da nova interface (#166), com capturas antes e depois. Quando nada mais usar o desenho em CPU, ele vai para trás de uma feature desligada e a cópia da Roboto sai.
- O "OPENOMSI" continua lá, agora em `ui.rs:1396`.
- O passo 2 continua valendo e rende mais: as linhas de `game_lists.rs` alimentam também o `omsi_ui::menu_page`. Vai para a fase 2 da Trilha App, junto da C2.
- O passo 4 (um arquivo de perfil por ônibus) não começou.
- O passo 5 foi atendido em parte pelo índice de conteúdo do `omsi-launcher-core` (`index.rs`); falta conferir se o jogo ainda lista pastas por conta própria.
- O `omsi_ui::theme` e o `look.rs` do `omsi-launcher-ui` são dois conjuntos de tokens, de propósito, até o launcher novo virar o padrão (passo 5 da migração em `docs/NEW_UI_FOUNDATIONS.md`).

### C7. O contrato do segundo renderer (nova, 23/10/2026)

Com a opção "DirectX9 (D3D9Ex)", o `omsi-render` passou a ter um consumidor de fora: o `omsi-render-d3d9` lê o plano de cada quadro (`FramePlan`) e os eventos de asset (`MirrorEvent`, `AssetMirror`), e os seus shaders HLSL (`scene.hlsl`, `ui.hlsl`) seguem os layouts do renderer padrão (o vértice, os parâmetros de instância, os 11 vetores do material, o uniform da câmera). Nenhum teste prende esse contrato, e o portão da C5 ("nenhum Camera copiado em WGSL") não enxerga o HLSL.

- Documentar o contrato no `crates/omsi-render/ARCHITECTURE.md` e cobri-lo com testes do lado do `omsi-render`, para que uma mudança de estrutura no renderer não quebre o Direct3D 9 sem aviso.
- Pôr num lugar só as constantes que Rust, WGSL e HLSL compartilham (`ALPHA_CUT`, `ALPHA_TEST_REF`, o tamanho do material, o layout do vértice e da instância), com um teste no estilo de `camera_wgsl_matches_camera_uniform`.
- Manter o desenho de hoje (o `omsi-render` contabiliza e entrega o plano) sem um trait de backend comum, até haver uma terceira implementação.
- Do lado do app, a escolha do backend sai das checagens espalhadas e fica num ponto só (Trilha App, AP4).

Os comentários `SAFETY` do crate novo já cobrem 57 dos 62 `unsafe`; os que faltam entram no fix-25.

## Frente D: configuração

O depot lê configuração por três caminhos que não conversam: variáveis de ambiente, o `settings.cfg` e a linha de comando. O problema mais sério que esta revisão encontrou está aqui: o `settings.cfg` tem **dois parsers com dois conjuntos de padrões**.

### D1. Um só `Settings` para jogo e launcher

O jogo lê o arquivo em `Settings::from_text` (omsi-app/src/settings.rs:439, 149 chaves num `match`) com os padrões de `Settings::default`. O launcher lê o mesmo arquivo em `settings_from_text` (omsi-launcher-core/src/lib.rs:1922), devolvendo um `serde_json::Value` com cerca de 168 nomes e os padrões escritos de novo em `json!` (`msaa: 4`, `anisotropy: 8`, `vr_scale: 0.65`, …). Uma opção nova precisa ser adicionada em seis lugares (campo, padrão, leitura e escrita no jogo; padrão e leitura no launcher), e um padrão diferente entre os dois lados aparece como "o launcher mostra uma coisa, o jogo faz outra".

Proposta:

1. Mover o struct `Settings` (sem os métodos que dependem de `omsi_render`, como `render_options`) para `omsi-launcher-core`, ou para um crate pequeno `omsi-settings` que os dois usam.
2. Ler e escrever com uma tabela declarativa: cada opção em uma linha com chave, campo, padrão e validação. Pode ser `serde` com um formato `chave=valor` próprio ou uma macro; o formato do arquivo não muda, para não quebrar perfis existentes.
3. O launcher passa a trabalhar com o struct tipado; o `Value` só aparece na borda da CLI `--cli`.
4. Teste de ida e volta: todo `settings.cfg` escrito pelo jogo é lido igual pelo launcher, e vice-versa. Os testes de ida e volta que já existem em `settings.rs` (triple screen, FOV externo, espelhos) viram casos dele.

**Revisão de 16/10/2026.** O teste de ida e volta existe desde a T0.3 (PR #64) e lista 51 divergências conhecidas. A auditoria achou duas fontes de verdade a mais: o escritor próprio de `game_lists.rs` e as listas de opções copiadas no menu do jogo. O desenho fica assim:

- Uma tabela declarativa `SettingSpec { key, kind, default, range, options, round, tab, label }` num crate compartilhado (fix-15), que produz o struct `Settings`, os padrões, o leitor, o escritor (com limite na leitura e na escrita), as linhas das páginas do launcher, as linhas do menu do jogo (Frente C6) e os casos do teste de ida e volta. As divergências conhecidas somem por construção.
- No launcher, `setting_row(ui, spec, &mut valor)` substitui `sel_setting`, `toggle_setting`, os 33 sliders feitos à mão e os 48 `dirty = 0.3` de `pages.rs:265-1200`.
- `Settings` é carregado uma vez e passado como `Arc<Settings>`, no lugar dos cerca de 20 `Settings::load()`, um deles dentro do frame.
- Os leitores de `mirrors.cfg` viram um único `per_bus_file<T>(prefixo)`.
- A lógica de domínio que mora na UI do launcher (`Choice` com `to_duty`, `default_hof`, estação, escolha de viagem, gravação do horário, favoritos, aeroporto mais próximo, em `state.rs`, `timetable.rs` e `drive.rs`) vai para `omsi-launcher-core`, com testes, e o jogo deixa de depender da UI do launcher (`weather_time.rs:181`, `server.rs:220`, `game_lists.rs:2197`). `pages.rs` vira uma pasta `pages/`, e os portões `if true { return }` do multiplayer viram a feature `lan`.

**Revisão de 23/10/2026.** A D1 e o registro de configurações do #151 (OPT-01), que a nova interface consome, são o mesmo trabalho: a tabela `SettingSpec` é esse registro e alimenta quatro consumidores, o jogo, o launcher antigo, as páginas novas do launcher e as janelas de configuração do menu Esc. Hoje são 23 chamadas a `Settings::load()` e 362 usos de `Value`/`json!` no `omsi-launcher-core`. Os PRs `fix/launcher-*` abertos (#244 a #280) corrigem à mão, um por um, divergências que a D1 elimina por construção; como correção não espera refatoração, eles entram antes. A lógica de domínio que mora na interface do launcher vai para o `omsi-launcher-core` no passo AP6 da Trilha App.

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

### D5. Utilitários compartilhados no `omsi-cfg` (nova, 16/10/2026)

Pequenos, em crates frios e sem mudar comportamento (fix-16). Cada um é um PR que cria o utilitário e troca as cópias:

- **`omsi_cfg::paths::{data_dir, settings, inputs, mirrors, seats}`**, no lugar das cerca de 25 cópias do caminho `~/.depot` (21 `var_os("HOME")` só no app, e o caso do Android). Os sete documentos que ainda dizem `~/.openomsi` são corrigidos no mesmo PR.
- **`omsi_cfg::write_atomic(path, bytes)`**, no lugar das sete gravações tmp+rename feitas à mão, do `save_settings` do launcher (`omsi-launcher-core/src/lib.rs:2156`), do `Choice::save` e do `mirror_hud`.
- **Texto OMSI**: um decode, com detecção de BOM e code page, e um `text::encode_utf16_bom`, no lugar das quatro cópias do escritor UTF-16 (`driver.rs:62`, `situation.rs:385`, `omsi-app/src/editor.rs:580`, `omsi-launcher-core/src/lib.rs:2574`). `codepage::decode` fica `pub(crate)` e o app passa a chamar `decode_text`.
- **`omsi-cfg/src/lib.rs` em módulos** (`text.rs`, `paths.rs`, `install.rs`, `env.rs`), reexportados para a API pública não mudar.

  Feito primeiro, por domínios e subdomínios (pedido do Zane, 17/10/2026): `text/` (`codepage`, `number`, decode e `split_lines`), `format/` (`file`, `keyword`, `keywords`, `reader`), `content/` (`listing`, `roots`, `resolve`, `packages`, `vfs`), `install/` (`original`, `layout`, `home`, `search`) e `env.rs`; `flags.rs` e `limits.rs` ficam na raiz. A raiz reexporta tudo, então a API pública não muda. `paths` entrou em `install/paths.rs` (`omsi_cfg::paths::{home, data_dir, root_memo, settings, inputs, mirrors, seats}`) no passo 2; o `omsi-check` lia `~/.openomsi-root`, um bug corrigido num PR `fix/` à parte. No passo 3, `omsi_cfg::write_atomic` (em `fs.rs`, domínio próprio) trocou as gravações tmp+rename à mão e passou a gravar o `settings.cfg` do launcher, o `Choice::save` e o `mirror_hud`; as gravações que conferem o arquivo temporário antes de trocar (`keyboard.cfg`) ou mexem nele (permissões do `cloudflared`, `fsync` e backup do editor) ficaram como estavam. No passo 4, `omsi_cfg::encode_utf16_bom` (em `text/`) trocou os quatro escritores UTF-16, `codepage::decode` ficou `pub(crate)` e os oito leitores de fora do crate passaram a `decode_text`, que tira a marca de ordem de bytes que antes ficava no começo do texto. No passo 5, `omsi_cfg::bytes::{le16, le32, le64}` (domínio próprio, com `get_le16` e `get_le32`, que devolvem `None` no fim do arquivo) trocou os leitores do `vfs.rs`, do `dds.rs`, do `gpu.rs`, do cabeçalho do `tga.rs` e dos helpers de BMP do `omsi-texture`.
- **Matemática pequena** (`smoothstep`, xorshift, sRGB, normalização de ângulo, rotação de heading para direção, limites) num `omsi-geometry::util`, e a psicrometria (pressão de saturação, umidade absoluta, relativa e ponto de orvalho) e a fórmula de Magnus, hoje em quatro versões com constantes diferentes, em `omsi-content::weather`. Unificar a fórmula de Magnus muda números: as constantes escolhidas saem num PR `fix/` à parte.

  Dividido por assunto em PRs curtos. No passo 6a, `omsi_geometry::util` (`ease` com `smoothstep` e `rng` com `xorshift64` e `unit_f32`) trocou as quatro cópias de `smoothstep` e os sete geradores xorshift64 iguais; o xorshift de 32 bits do teste de `omsi-net` fica como está. No passo 6b, `omsi_texture::srgb::{to_linear, from_linear}` trocou as seis cópias da curva sRGB em Rust (`bc.rs`, `api/assets.rs`, `passes/main.rs`, o relatório de céu de `atmosphere.rs` e `weather_setup.rs`); as cópias em WGSL ficam para os includes da C5, e a luminância WCAG do `omsi-ui`, com outro limiar, fica como está. No passo 6c, a psicrometria foi para `omsi_content::weather::psychro`, agrupada pelas constantes de Magnus que cada parte usa: `omsi` (base 10, a umidade dos arquivos de clima e o `Cabinair_relHum`), `bolton` (o clima personalizado e o modelo de clima) e `alduchov` (o embaçamento dos vidros). As contas não mudaram. Depois, num PR `fix/`, o Zane escolheu (17/10/2026) as constantes do OMSI original para tudo: `psychro` passou a ter uma curva só, `magnus` (base 10, 7.5/237.3 e 7.6/240.7 de água super-resfriada abaixo de 0 °C, não de gelo), e `humidity` com as grandezas que cada parte usa; o modelo de clima, o clima personalizado e o embaçamento mudam menos de 0,3%. No passo 6d, `omsi_geometry::util::angle::{wrap_deg, wrap_deg_f32}` trocou as 20 cópias de `(x + 540.0).rem_euclid(360.0) - 180.0`; as variantes com `+ 180.0` (o `wrap_deg` de `omsi-sim/src/traffic.rs`, `app.rs`, `input/mouse.rs`) e os `angle_diff` arredondam diferente e ficam como estão, para os traces não mudarem.
- **Um leitor little-endian** (`omsi_cfg::bytes::{le16, le32, le64}`) para `vfs.rs`, `dds.rs`, `gpu.rs`, `tga.rs` e os helpers de BMP.

### D6. Formatos OMSI só nos crates de formato (nova, 16/10/2026)

O guia de estilo do projeto diz que formatos OMSI são lidos só em `omsi-cfg`, `omsi-content`, `omsi-model`, `omsi-o3d` e `omsi-texture`. Hoje não é assim:

1. **`gamectrler.cfg`** (fix-17): `parse_cfg`, `cfg_text`, `DeviceCfg`, `Func` e `axis_shape` de `controllers.rs`, e `stock_keys.rs`, vão para `omsi_content::input`, que passa a ser o único leitor e escritor, com os blocos de extensão do Depot. O `omsi-app` fica só com o mapeamento de dispositivos. B4 já faz o launcher usar o escritor do jogo; este item move os dois.

   Feito no passo 3: `omsi_content::input` virou pasta, com `gamectrler` (`Func`, `DeviceCfg`, `read`, `parse`, `to_text`, `write` com a conferência de ida e volta, `axis_shape` e `AXIS_SHAPES`) e `stock_keys`. O antigo `load_game_controllers`, que só o `omsi-check` usava, deu lugar a `gamectrler::read`. O `omsi-app` reexporta o que usa e ficou com o caminho do arquivo, a regra do `[FFScale]` 0/0 herdado do OMSI e o mapeamento dos dispositivos.
2. **`[cloudtype]`** (`weather_setup.rs:346-351`) passa por `omsi_content::Envir::load`, com um `CloudKind` lido uma vez (Frente E3).

   Feito no passo 4: `cloud_texture` lê o `Weather/clouds.cfg` com `Envir::load` e acha o tipo com `Envir::cloud_type`, sem diferenciar maiúsculas. Um bloco desativado (`-<DISABLED>-`) agora fica de fora, como no OMSI. O `CloudKind` fica para a Frente E3.
3. **Fontes `.oft`** (`lan_mods.rs:336`) usam `Font::load_all`; o `[name]` do launcher e os textos de tile do editor (`editor.rs`) passam por `omsi-map` com `omsi_cfg::decode_text`.

   As fontes foram feitas no passo 5: o pacote de mods da LAN lê cada `.oft` com `Font::load_all` e leva o bitmap e o alfa das fontes em uso. O `[name]` do launcher e os textos de tile do editor ficam para um passo seguinte.

   No passo 6, um PR `fix/`, o leitor de `.dsc` do launcher foi para `omsi_content::Dsc` (`parse`, `from_bytes`, `candidates` e `find`), e o `[name]` das situações salvas para `Situation::read_name`, os dois com `omsi_cfg::decode_text`. Como `.dsc` e `.osn` não são formatos de mapa, ficaram em `omsi-content`, junto de `Situation`, e não em `omsi-map`. Antes o `.dsc` era lido como Latin-1, e agora sai na code page do autor, então descrições em russo, polonês ou tcheco, e as aspas do 1252, aparecem certas. O `decode_text` passou a aceitar também UTF-16 big endian com marca, que o launcher já lia. Os textos de tile do editor ficam para o passo 7.

   No passo 7, um PR `fix/`, o codec próprio do editor de objetos (`decode`, `encode` e `Encoding` de `editor.rs`) deu lugar a `omsi_cfg::text::{decode_with_encoding, encode_like}` e `TextEncoding`, que ficam no domínio de texto do `omsi-cfg`, junto de `decode_text`, porque não são específicos de tile. Antes um tile que não era UTF-8 nem UTF-16 era lido como Latin-1, e o nome de um objeto novo (V) em cirílico saía truncado byte a byte. Agora o tile é lido na code page do autor e escrito de volta nela. Se nenhuma code page devolve os mesmos bytes, o tile continua em Latin-1, então o que o editor não mudou sai igual.
4. **`handle_keyword` de `omsi-model`** (cerca de 620 linhas, `lib.rs:623`) se divide por família (malha e animação, `matl_*`, luzes, texturas dinâmicas, partículas), sem mudar a delegação do `.sco`.

   Feito no passo 8, só movendo código: o vocabulário do `model.cfg` foi para `omsi-model/src/keywords/`, com `mod.rs` (o `handle_keyword` público e as opções do modelo inteiro: `rendertype`, `surface`, `lod`, distâncias) e um módulo por família: `mesh` (malha, itens, variáveis, sombras, eventos de mouse, ossos e animações), `textures` (`ctc`, script, HTML, `texttexture` e `texchanges`), `lights`, `material` (`[matl]` e os comandos de material) e `particles`. Cada família devolve `false` para o que não é dela, e o `handle_keyword` tenta uma depois da outra. Os braços do `match` saíram iguais, linha por linha, e o `.sco` continua delegando do mesmo jeito. O `lib.rs` caiu de 1616 para 998 linhas.
5. **Um `DdsHeader::parse`** usado por `dds::decode` e `gpu::dds_blocks`, um `IMAGE_EXTENSIONS` só e o `CP437_HIGH` duplicado de `vfs.rs:97` trocado pelo de `codepage`.

   Feito no passo 9, sem mudar comportamento: `omsi_texture::dds::DdsHeader::parse` lê o cabeçalho uma vez (dimensões, mips, formato de pixel, fourcc, máscaras, `caps2`, o formato DXGI do cabeçalho DX10 e onde começam os dados), e `dds::decode` e `gpu::dds_blocks` usam ele. As listas de extensão viraram duas constantes, porque fazem coisas diferentes: `EXTENSIONS` é a ordem em que uma textura é procurada, e `IMAGE_EXTENSIONS` (as mesmas mais `jpeg`) é tudo que é lido como imagem. O índice de texturas de `Splines`/`Sceneryobjects` continua sem `.jpeg`, como antes; se isso deve mudar fica para um PR `fix/`. O `vfs.rs` passou a usar o `codepage::CP437_HIGH`, que era a mesma tabela.
6. **Testes de `.o3d`** (cabeçalhos v1, v3 e v4, índices longos, vértices embaralhados, tag desconhecida, arquivo truncado), que hoje não existem, e os nomes de textura `str8` decodificados por `codepage` em vez de 1252 fixo. Os testes entram antes de qualquer mudança no leitor.

   No passo 1, `crates/omsi-o3d/tests/o3d.rs` passou a cobrir o leitor como ele é hoje, com arquivos montados no teste: cabeçalhos v1, v3 e v4+, índices de 16 e 32 bits, ossos, a matriz, triângulos com vértice inexistente, as chaves de embaralhamento (um caso calculado à mão e a volta completa para várias chaves e flags), bytes desconhecidos entre seções, magic errado e arquivos truncados. O leitor não mudou. No passo 2, um PR `fix/`, os nomes `str8` (texturas e ossos) passaram a ser lidos por `omsi_cfg::decode_text`, que detecta a code page (1252, 1250, 1251 ou UTF-8) em vez de supor 1252; o leitor de `.x` continua em 1252.

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
| `omsi-launcher` | `launcher/` | \~13.000 | D1 e D3 ajudam, não são obrigatórios. **Substituída em 23/10/2026:** o launcher já roda como processo próprio e sai do app para `omsi-launcher-ui` e `omsi-launcher-core` (Trilha App, AP6), sem crate novo |
| `omsi-gameplay` | `traffic.rs`, `schedule.rs`, `schedule_paper.rs`, `humans.rs`, `humans_pax.rs`, `career.rs`, `driver.rs`, `duty_start.rs`, `journey.rs`, `money.rs`, `bus_service.rs` | \~29.000 | B4 e E1 |
| `omsi-world` (opcional) | `scene.rs`, `tiles.rs`, `world_load.rs`, `placing.rs` | \~18.000 | E1; só se `omsi-gameplay` precisar de `World` sem o resto do app |

O código de LAN do app (`lan.rs`, `lan_world.rs`, `lan_mods.rs`, `voice.rs`, `admin.rs`, `server.rs`) não entra nesta divisão: ele fica no app atrás da feature `lan` (A1). Quando o multiplayer for readaptado, o lugar natural dele é um crate opcional `omsi-lan-game`, que depende de `omsi-gameplay` e é ligado pela mesma feature. O `input_script.rs` sai bem menor da Frente C3, e o que sobrar de mapeamento de teclas vai para `omsi-input`.

**`omsi-input`, revisão de 16/10/2026** (fix-18). A extração ganha um desenho: `trait ControllerBackend` e `trait ForceOutput`, com gilrs, DirectInput, evdev e o HID do macOS atrás deles, e `Devices` como uma lista de backends mais a regra de deduplicação. Antes de mover, cinco costuras com o resto do app são cortadas: a curva dos pedais, a pasta de conteúdo, `omsi_render::catch`, `RigidBody` e a janela do winit. Também entram um helper único de evdev para Linux (listar nós e um tipo `Bitmap`), constantes com nome para os usos HID do macOS e um builder de efeitos do DirectInput. Os cerca de 40 `unsafe` do segmento ganham comentário `SAFETY` (fix-25). O caminho quente de `Controllers::poll` deixa de alocar a cada frame, e as opções dos controles são aplicadas quando mudam, não a cada frame (`app_events.rs:734-748`).

Dentro de `omsi-render` não é preciso crate novo: a Frente C1 resolve com módulos. O alvo de tamanho para o `omsi-app` depois de A1 e E2 é ficar abaixo de 50 mil linhas, com janela, laço de eventos, UI, menus e a cola entre os crates.

### E3. Clima num módulo só (nova, 16/10/2026)

O estado do clima está espalhado em 11 lugares, inclusive globais e um canal lateral de texto (`Weather.path`). Proposta (fix-19):

- `weather_setup.rs`, `weather_time.rs`, `weather_cycle.rs` e `weather_model.rs` formam um módulo `weather/` com um `WeatherState` e um único `publish()` que escreve os globais (neve, ambiente, vento das partículas, cone).
- `WeatherChoice { Map, Natural, Cycle, Metar(String), Custom(..), Preset(arquivo) }` substitui os prefixos de texto de `args.weather`, que hoje são interpretados em `drive.rs`, `phone.rs` e `showroom.rs`.
- As partes puras (interpolação, escolha, o codec do clima personalizado, o formato do METAR, precipitação derivada e estado da rua) vão para `omsi-content::weather`.
- `CloudKind` (nenhuma, cúmulos 1 a 3, encoberto) é lido uma vez do `clouds.cfg` e usado no lugar das comparações de texto.
- O METAR de `change_weather` passa para a thread de METAR que já existe (B6).
- `tiles.rs` se divide em `tiles/index.rs`, `tiles/rows.rs` e `tiles/stream.rs`; o streaming usa o pool de loaders no lugar de um `thread::spawn` por lote.

B2 e B3 são corrigidos antes, nos seus PRs `fix/`; esta frente só move o que eles deixarem.

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

Em vez de fases em sequência, trilhas com portão próprio. Todas as branches de refatoração são `feat/<nome>`; mudanças de comportamento, inclusive a retirada de compensações, são `fix/<nome>`.

Dois padrões valem para toda trilha que reorganiza um crate (decisões do mantenedor, 23/10/2026):

- **Testes fora de `src/`.** Nenhum `#[cfg(test)]`, `#[test]` ou `tests.rs` em `src/`: um binário de teste por crate, com um módulo por domínio, os auxílios em `tests/support/` e o que é interno alcançado por um módulo `testing` atrás da feature `test-support`, como a Trilha S fez no `omsi-sim`. Os crates já reorganizados levam os testes para fora quando forem mexidos de novo.
- **Pacotes por domínio em três partes**, no modelo de `omsi-sim/src/physics/`: `domain/` com os objetos, `interface/` com a entrada (`input.rs`, o que o resto do jogo chama) e a saída (`output.rs`, o que o domínio devolve e o que ele precisa de fora) e `implementation/` com os procedimentos. A trilha que cria a pasta de um domínio já a reorganiza assim. O detalhe está em [`OMSI_APP_REFACTOR.md`](OMSI_APP_REFACTOR.md#princípios-aplicados-a-este-crate).

Situação em 23/10/2026:

| Trilha | Situação |
| --- | --- |
| 0, caracterização | Concluída em 12/10/2026 (PRs #62 a #64). Pendente: tornar `offscreen-golden` bloqueante depois de três execuções verdes seguidas no main (em 23/10/2026 ainda é informativo). O `offscreen-traces` passou a bloquear no F0 |
| 1, crates frios | Concluída em 12/10/2026 (PRs #65 a #80), mais a divisão do `omsi-render` em domínios e camadas (T1.6, PRs #79 a #111), com a checagem de camadas bloqueante. Ficou de fora o `FrameContext`, que passa para a Frente C5 |
| 2, grupos mornos | Concluída em 13/10/2026 (PRs #82 a #128): A1, C3, D2, imports explícitos e `vehicle.rs` em submódulos |
| F, física de veículo, segunda rodada | Concluída em 17/10/2026 (F0 a F3, PRs #144 a #178). A equalização com o OMSI 2 continua em PRs `fix/`. Detalhe abaixo |
| 3, frios e mornos da auditoria | Começou em 17/10/2026. D5, D6, C5 e o passo 3 da C6 concluídos; faltam o fix-25 e o passo 1 da C6, reescrito em 23/10/2026 |
| 4, áreas quentes | A antiga trilha 3, ampliada com os itens da auditoria. Não começou; desde 23/10/2026 é a fase 2 da Trilha App |
| S, `omsi-sim` por domínio | Concluída em 23/10/2026 (S1 a S5, PRs #311 a #386). Detalhe em [`OMSI_SIM_REFACTOR.md`](OMSI_SIM_REFACTOR.md) |
| App, `omsi-app` por domínio | Nova (23/10/2026). A fase 1 (formatação, testes, nova interface, saída gráfica, HUD, launcher e pastas por domínio) pode correr em paralelo; a fase 2 é a Trilha 4. Detalhe em [`OMSI_APP_REFACTOR.md`](OMSI_APP_REFACTOR.md) |

Os bugs B1 a B7 não esperam nenhuma trilha.

As trilhas 0, 1 e 2 tinham este desenho:

1. **Trilha 0: caracterização, que já pode começar.** Adaptador gráfico por software (lavapipe) no CI, B0 (traço de estado do offscreen) e teste de ida e volta do `settings.cfg` entre jogo e launcher. São arquivos novos de teste e de CI e não conflitam com nada. Portão: golden e B0 rodando no CI em todo PR e verdes nos PRs de paridade e triagem.
2. **Trilha 1: crates frios, em paralelo à trilha 0.** `cargo fmt -p` por crate, com `fmt --check` bloqueante nos crates já formatados; limpeza do clippy nos crates de formato, `omsi-audio`, `omsi-ui` e `omsi-geometry`, com o clippy ainda informativo no CI; C1 (`omsi-render/src/lib.rs` em módulos), o maior ganho de legibilidade num crate com 10 commits em todo o histórico; e a extração de `omsi-html` de `omsi-sim/src/htmlengine`. Portão: crates de formato formatados e sem avisos do clippy, nenhum arquivo de `omsi-render` acima de 3.000 linhas, golden igual no lavapipe, e traços da física idênticos depois da extração do `omsi-html`.
3. **Trilha 2: grupos mornos do `omsi-app`, depois do portão 0.** A1 (features `lan`, `server` e `plugins`), que toca muitos arquivos mas com trechos pequenos; C3 (`input_script.rs` por assunto); D2 (registro de `OMSI_*`), primeiro nos outros crates e depois por módulo; imports explícitos no lugar dos globs; e a divisão do restante de `vehicle.rs` em `omsi-sim`. Portão: `omsi-net` e `omsi-plugin` fora do `cargo tree` do jogo, job `--all-features` rodando, `input_script.rs` abaixo de 500 linhas e toda `OMSI_*` declarada.
4. **Trilha 3: áreas quentes, por janelas.** Frente B (`Session::step`), C2 (`App` em sub-structs), D1 e D3 (um só `Settings` e o launcher-core tipado), E1 e E2. Cada uma entra numa janela curta e anunciada: a unidade é reservada, os PRs de paridade e triagem naqueles arquivos são mesclados ou portados antes, e a janela termina com o main integrável. O perfil por escopo (`enum Stage`) sai primeiro, porque a fase de performance do mapa depende dele. Portão: traço B0 idêntico em cada PR, `run_offscreen` abaixo de 400 linhas, `omsi-gameplay` compilando e testando sem `wgpu`, e `omsi-app` abaixo de 50 mil linhas na configuração padrão. Desde 16/10/2026 esta é a Trilha 4, descrita abaixo.

### Trilha F: física de veículo, segunda rodada

A refatoração da física de veículo vem antes da macro-refatoração geral. As Fases 0 a 4 do plano de física terminaram em 10/10/2026, mas a auditoria achou trabalho que pertence a ela. Enquanto esta trilha não termina, os itens das trilhas 3 e 4 esperam.

**A física do tráfego IA entra nesta trilha** (decisão do mantenedor, 16/10/2026), só como refatoração: a estrutura do código da IA muda, o comportamento dela no jogo não. A escolha de um modelo novo para a IA (o estudo da Fase 5) continua pausada. O código coberto é o corpo que segue a faixa (`AiBody` em `omsi-sim/src/ai_motion.rs`: rodovia, trilho e ar), o modelo cinemático (`KinematicModel` em `physics.rs`), o frame da IA (`vehicle/ai.rs`), a altura de repouso da IA (`ai_lift` em `vehicle.rs` e `ai_rest_offset` em `visuals.rs`) e as chamadas a esse código no tráfego do app (`omsi-app/src/traffic.rs`).

A trilha cabe em quatro PRs. Os estruturais provam que nada mudou com os traços idênticos bit a bit; os de comportamento são `fix/` com o rótulo `physics-delta`, um commit por item e o delta previsto antes do código, conferido quadro a quadro.

| Item | Branch | O que entra | Tipo | Pronto quando |
| --- | --- | --- | --- | --- |
| F0 | `feat/fisica-ia-tracos` | Testes de caracterização da IA, antes de qualquer mudança no código dela (detalhe abaixo) | só testes | Os traços novos rodam no job `physics-traces`, e o `offscreen-traces` passa a bloquear |
| F1 | `feat/fisica-estrutura` | Tudo o que é estrutural, em commits separados: `rigid/spec.rs` com `PhysicsSpec::from(&omsi_vehicle::Vehicle)` (massa, inércia, CG e, por eixo, mola, amortecedor, força máxima, raio e bitola em unidades SI), lido por `RigidBody::from_definition`, `wheel_rest_loads*`, `TrailerPart::new_ex`, `VehicleType::hub_heights`, `KinematicModel::from_definition`, `coupled_parts` e `AiBody::new`, com as heurísticas de hoje copiadas como estão (fix-5); as constantes do passo (0,25 s, fatias de 50 ms, 4 a 16 substeps, passes, Baumgarte) e as da IA (taxas de esterço de 25 e 40 °/s, limite de ±1,2 rad, limite de dt de 0,1 s do modelo cinemático) com nome em `params.rs`; um `SolverScratch` reaproveitado no lugar dos `Vec` alocados a cada substep (fix-7, parte estrutural); campos de `RigidBody`/`RigidWheel` atrás de acessores, `VehicleInstance::teleport()` no lugar das três cópias e `offscreen.rs:2346` lendo `CLIMB`, `HUB_REACH` e `WALL_HEIGHT` de `rigid::params` (fix-8); `ai_motion.rs` dividido em `ai/` (rodovia, trilho, ar e o que o planejador usa para sair da vaga e manobrar), `vehicle/ai.rs` renomeado para `vehicle/ai_frame.rs`; `DriveGround`, `drive_probe` e a montagem de colisão de `scene.rs:2549-2965` para `scene/ground_query.rs` (fix-9) | estrutural | Traços da física (jogador e IA), B0 e golden idênticos; nenhuma conversão do `.bus` para física fora de `spec.rs` |
| F2 | `fix/fisica-compensacoes` | Um commit por valor inventado ou compensação de modelo do jogador: os padrões dados a mola, amortecedor e força máxima zerados, o rolamento ausente, a altura do CG ausente, as heurísticas de unidade e os mínimos de massa, inércia, raio e bitola, o piso 0 de `motion.rs:308`, `[rot_pnt_long] 0` respeitado (`trailer.rs:339-356`, `articulation.rs:30`), sem acoplamento padrão (`trailer.rs:191`), `[boundingbox]` como o `.bus` declara (`vehicle_type.rs:376-397`), altura do cubo da física pelo `.bus` (`vehicle_type.rs:473-497`) e o piso `MIN_LOAD_SHARE` (`suspension.rs:30`). Cada um passa a seguir o OMSI, decidido uma vez em `spec.rs`. O código que deixar de ser usado fica atrás de uma feature, sem ser apagado (fix-5 e fix-6, parte de comportamento) | comportamento | Delta previsto confere em cada commit; traços da IA idênticos |
| F3 | `fix/fisica-passo-fixo` | Antes de tudo, os harnesses de traços passam a usar `omsi_sim::physics` e o apelido `rigid` sai. Substep de tamanho fixo com acumulador, para o resultado não depender da taxa de quadros, e `RigidBody::collide` dentro da fatia, com o dt dela, em vez de uma vez por frame com o dt sem limite (`motion.rs:398-403`) (fix-7, parte de comportamento) | comportamento | Delta previsto confere; invariantes I1 e I4 continuam valendo; traços da IA idênticos |

F0 entra antes de tudo, porque o script de comparação roda os mesmos testes na base e no PR. F1 vem antes de F2 e F3; F2 e F3 não dependem entre si. F1 é grande, mas só move e renomeia; se a revisão pedir, ele se divide nos mesmos commits.

**O que ficou para depois do merge de F1 (concluído).** O script de comparação copia os testes de traços do PR para a base e os compila lá, então F1 não podia mudar nada que esses testes usam. Os campos de `RigidBody` e `RigidWheel` foram fechados em dois passos: os acessores e setters que faltavam entraram primeiro (#148), e depois os testes de traços passaram a usá-los, os campos viraram `pub(crate)` e o apelido `omsi_sim::ai_motion` saiu.

**Como F2 ficou.** O veículo do jogador lê o `.bus` como o OMSI: massa em toneladas, inércia em t·m², molas, amortecedores e força máxima, rolamento, altura do CG, raio, bitola e esterçamento como o arquivo dá, sem mínimos nem valores inventados (só os padrões do próprio OMSI: 1 t, 100 t·m² por eixo e `achse_inertia_inv` 0,002). Os cubos saem só do `.bus`, nenhuma roda tem carga estática mínima, cada seção acoplada gira em torno do próprio `[rot_pnt_long]`, um engate não declarado fica na origem do modelo e o `[boundingbox]` vale como declarado. O modelo cinemático (as cópias da IA) guarda as leituras antigas em `kinematic_*` (`spec.rs`); o `load_ai` dá às cópias da IA o engate de 4 m e o corte do `[boundingbox]` de antes. As três linhas do `[momentofintertia]` são lidas no referencial de y para cima do OMSI (arfagem, guinada, rolagem; a verificação no jogo está na issue #49); em F2 elas chegaram a ser lidas na ordem do comentário do SDK (arfagem, rolagem, guinada), o que um `fix/` posterior desfez. Os padrões que o jogador deixou de usar ficam na feature `model-fallbacks` de `omsi-sim` só até a física ser validada no jogo; depois são excluídos.

**Como o pacote ficou antes de F3.** Entre F2 e F3, um PR só de movimento juntou `rigid/` e `physics.rs` num pacote `omsi-sim/src/physics/` em três partes. `domain/` tem os objetos e as leituras dos campos, sem cálculo: corpo, roda, chão, impacto, juntas do trem, os valores do `.bus` em SI (`PhysicsSpec`, `AxleSpec`, `AiSpec`), controles, `VehicleMotion`, o modelo cinemático e as constantes (`domain/params.rs`). `interface.rs` lista o que o resto do jogo usa, reexportado pelo pacote (`physics::RigidBody`). `implementation/` tem os procedimentos, um módulo por domínio, na ordem do passo: `spec`, `ground`, `suspension`, `wheel`, `tyre`, `body`, `collide`, `train` e `kinematic`. Os 72 traços, o traço do offscreen e o golden ficaram idênticos. O caminho `omsi_sim::rigid` continua como apelido de `physics` só para os dois harnesses de traços, que o script compila também no commit base; o primeiro commit de F3 os passa para `physics` e remove o apelido.

**Como F3 ficou.** Todo substep do corpo rígido tem 1/240 s, em qualquer taxa de quadros: um quadro roda quantos cabem no tempo dele e no que os quadros anteriores deixaram, e guarda o resto (`RigidBody::unstepped`). O esterço, o amortecimento de arfagem e rolagem do OMSI, a parada do corpo freado e a rede contra tombamento passaram a rodar a cada substep, e o mesmo comando dá o mesmo movimento, bit a bit, a 30, 60, 120 ou 240 fps. A colisão com obstáculos roda depois de cada fatia do passo (até 50 ms de substeps), com o tempo da fatia, por `step_train_with`; a 20 fps ou mais um quadro é uma fatia só. O corpo é mostrado entre os dois últimos substeps (`shown_origin`, `shown_heading_pitch_bank`), para o movimento na tela ficar liso acima de 60 fps, no máximo um substep atrás da física. Os traços da IA ficaram idênticos; os do corpo rígido mudaram pouco a 60 fps (o substep já tinha 1/240 s) e muito no traço de quadros variáveis (t13).

**Onde o jogador e a IA compartilham um valor.** F2 e F3 mudam só a física do jogador. Quando um valor de `spec.rs` também alimenta a IA, a IA continua lendo o valor antigo, num campo próprio com nome explícito, até a física da IA ser retomada. Os traços da IA provam isso em cada commit. As compensações que só existem na IA (`ai_lift`, `ai_rest_offset`, a troca de `rot_pnt_long` pelo eixo traseiro em `ai_motion::rotation_point`, o limite de ±1,2 rad e o ângulo zerado para braço até 0,5 m em `vehicle/ai.rs:90-94`, o limite de dt de 0,1 s do modelo cinemático) mudam de lugar em F1 e ficam como estão, anotadas para a retomada.

**Testes da IA (F0).** Hoje a IA tem dois traços na física (B5, um carro de duas seções numa faixa com curva e rampa; B6, o modelo cinemático contra uma parede) e as posições dos carros no traço de estado do offscreen. F0 acrescenta um arquivo `omsi-sim/tests/ai_traces.rs`, no mesmo formato e com o mesmo script de comparação (`scripts/physics-trace-diff.sh`), que grava por quadro a pose, a velocidade, o esterço e as variáveis que a IA passa aos scripts, nestes cenários:

- rodovia: reta, curva fechada, curva em S e troca de faixa, em plano e em rampa, com um veículo rígido de dois eixos, um de três eixos e um articulado;
- saída de vaga e entrada de ré (`pull_out_ramps`, `straight_pull_out`, `back_in_ramp`, `pull_out_room`), com e sem obstáculo ao lado;
- parada e retomada: frenagem até a linha de parada e saída do repouso;
- `sweep_clearance` contra obstáculos parados e em movimento;
- trilho: dois eixos externos numa curva e numa troca de via;
- ar: trajetória com curva e inclinação lateral;
- `rotation_point` com `[rot_pnt_long]` ausente, zero, no eixo traseiro e à frente do eixo dianteiro;
- altura de repouso (`ai_lift`, `ai_rest_offset`) com e sem `suspension_axles`;
- modelo cinemático com dt grande (acima de 0,1 s) e com um quadro de dt zero;
- frame da IA (`update_ai`) escrevendo as variáveis dos scripts numa parada com pisca e freio.

Os veículos são sintéticos, montados no teste como os de `physics_traces.rs`, para rodar no CI sem conteúdo do jogo. Junto, o job `offscreen-traces` passa a bloquear, o que estava pendente desde a trilha 0, porque é ele que confere os carros de IA dentro do tráfego de verdade (planejador, sinais e cruzamentos). Por fim, antes do merge de F1, um teste no jogo nas máquinas do mantenedor, com um mapa e os ônibus de IA de série, comparando o main e a branch.

Portão: nenhuma conversão do `.bus` para física fora de `spec.rs`, nenhuma compensação de modelo na física do jogador, os traços da IA idênticos desde F0 e os traços da física verdes no CI.

### Trilha 3: frios e mornos da auditoria

Depois da Trilha F. São crates com poucos commits ou trechos pequenos, que andam com as regras de trabalho em paralelo de sempre.

- **D5**, utilitários do `omsi-cfg` (fix-16), um PR por utilitário.
- **D6**, formatos só nos crates de formato (fix-17), com os testes de `.o3d` primeiro.
- **C5**, pendências do renderer e dos shaders (fix-20, fix-21).
- **C6** passos 1 e 3, kit de widgets em `omsi-ui` (fix-22) e `navigator.rs` em `nav/` (fix-23). O passo 2 depende da tabela de D1 e vai para a Trilha 4.
- **fix-25**, comentários `SAFETY`, `bytemuck` e testes, em qualquer crate que outro item não esteja mexendo.

Situação em 23/10/2026: D5 (#184 a #207), D6 (#208 a #303), C5 (#304 a #395) e o passo 3 da C6 (#182, #183) concluídos. No fix-25 só o `omsi-render` está feito (#398); faltam os `unsafe` do `omsi-app` (124, com 22 `SAFETY`), do `omsi-plugin` (18 e 3), do `omsi-launcher-core` (12 e 3) e os 5 do `omsi-render-d3d9`, e o `bytemuck`. A parte do `omsi-app` vem depois do rustfmt dele (AP1 da Trilha App), para não conflitar. O passo 1 da C6 foi reescrito (ver a C6).

Portão: nenhum caminho `~/.depot` ou gravação atômica fora do `omsi-cfg`, `gamectrler.cfg` só em `omsi-content`, nenhum Camera ou vertex de tela cheia copiado em WGSL, `api/` só com métodos curtos e o menu do jogo desenhado pelo kit de `omsi-ui` (cumprido pela nova interface; o resto do HUD fica com a C6 reescrita).

### Trilha 4: áreas quentes, por janelas

A antiga trilha 3, com o mesmo método de janelas curtas e anunciadas, agora com esta ordem. Desde 23/10/2026 ela é a fase 2 da [Trilha App](#trilha-app-omsi-app-por-domínio): o conteúdo não muda, e a linha `omsi-launcher` da E2 dá lugar ao passo AP6 de lá.

1. **Perfil por escopo** (`enum Stage` e a guarda única de C2), porque a fase de performance do mapa depende dele.
2. **Frente B** (fix-11), com a montagem e o passo compartilhados e as três divergências resolvidas em B6. Ela sobe para o começo porque, até terminar, o golden e o B0 não cobrem a janela, e as outras frentes desta trilha dependem deles como prova.
3. **C2** (fix-12, fix-13) e o passo 2 de C6.
4. **C4** (fix-14, e fix-10 como PRs `fix/`).
5. **D1** (fix-15) e **D3**.
6. **E1**, **E2** (com o desenho de `omsi-input` de fix-18) e **E3** (fix-19).

As alocações por frame (fix-24) entram item a item na fase de performance do mapa, medidas com o perfil por escopo, e não nesta trilha.

Portão: o da antiga trilha 3 (traço B0 idêntico em cada PR, `run_offscreen` abaixo de 400 linhas, `omsi-gameplay` compilando e testando sem `wgpu`, `omsi-app` abaixo de 50 mil linhas na configuração padrão), mais `scene.rs` sem arquivo acima de 3.000 linhas e nenhuma divergência conhecida no teste de ida e volta do `settings.cfg`.

### Trilha S: `omsi-sim` por domínio

Pedida pelo mantenedor em 18/10/2026, com três objetivos: testes e auxílios de teste fora do código funcional, o crate dividido por domínio e subdomínio, e sem código duplicado. O plano completo, com o diagnóstico, a árvore alvo e a sequência de PRs (S1 a S5), está em [`OMSI_SIM_REFACTOR.md`](OMSI_SIM_REFACTOR.md).

Em resumo, cinco PRs: rustfmt do crate, as quatro funções sem uso removidas e as cópias de `angle_diff`, do xorshift e do smoothstep trocadas (S1); todos os testes de `src/` para `tests/`, um apoio de testes só (`tests/support/`), o que é interno alcançado pela feature `test-support` e traços novos do motorista da IA e da pose das pessoas (S2); `traffic/` (S3); `people/` (S4); e `host/`, `automation/`, `collision/`, `textures/` e `vehicle/coupled/` (S5), cada um já com os imports do app nos caminhos novos. Ela não tem arquivo em comum com a Trilha 3 e prepara o `omsi-gameplay` da Trilha 4.

Portão: nenhum teste em `src/`, nenhum auxílio de teste copiado, nenhuma API só de teste no jogo, nenhum arquivo funcional do `omsi-sim` acima de 800 linhas, e os traços da física, da IA, do tráfego e das pessoas idênticos em todos os PRs.

### Trilha App: `omsi-app` por domínio

Pedida pelo mantenedor em 23/10/2026: uma trilha só para o `omsi-app`, que cubra o código que entrou depois da revisão de 16/10/2026 e inclua a formatação do crate, que ainda é informativa. O plano completo, com o diagnóstico por domínio, a árvore alvo e a sequência de PRs, está em [`OMSI_APP_REFACTOR.md`](OMSI_APP_REFACTOR.md).

O crate tem hoje 144,3 mil linhas em 155 arquivos, 109 deles na raiz de `src/`, cerca de 19,6 mil linhas de testes dentro do código, e três partes que nenhuma frente deste plano cobria: a nova interface (`new_ui*.rs`, 6,8 mil linhas), o launcher (`launcher/`, 19,2 mil, que já roda como processo próprio) e a saída pelo Direct3D 9Ex (`d3d9_output.rs`, com a escolha do backend espalhada por uma dúzia de arquivos).

Em duas fases:

1. **Estrutura, já pode começar.** AP1, `cargo fmt -p omsi-app` com o `scripts/fmt-merge.sh`, o crate e os novos `omsi-launcher-ui`, `omsi-launcher-core` e `omsi-render-d3d9` na lista bloqueante; AP2, os testes para `tests/`, como na Trilha S; AP3, a nova interface em `interface/`, com os dados e os retângulos de clique do HUD separados do desenho em CPU; AP4, a saída gráfica em `output/`, com o backend escolhido num ponto só; AP5, o HUD e as listas do menu por assunto; AP6, o launcher para `omsi-launcher-ui` e `omsi-launcher-core`; AP7, as outras pastas por domínio, movendo arquivos inteiros.
2. **Áreas quentes.** A Trilha 4, com o mesmo conteúdo e as mesmas janelas.

Cada pasta de domínio criada na fase 1 já sai como pacote com `domain/`, `interface/` (entrada e saída) e `implementation/`. Decidido pelo mantenedor em 23/10/2026: os testes saem de `src/` (padrão do projeto), a trilha começa já pela AP1, o launcher vai para `omsi-launcher-ui` e `omsi-launcher-core`, e a pasta da nova interface se chama `interface/`.

Aposentar o desenho em CPU do HUD (o passo 1 da C6, reescrito, porque o menu do jogo já é desenhado pelo `omsi-ui`) muda o que aparece na tela e fica com a trilha da nova interface, fora desta.

Portão da fase 1: `fmt --check` bloqueante no `omsi-app` e nos três crates, nenhum arquivo de código na raiz além de `lib.rs` e `main.rs`, a nova interface sem dependência do desenho em CPU do HUD, o backend gráfico escolhido num ponto e o launcher fora do app, a não ser a ligação com o processo do jogo. O portão da fase 2 é o da Trilha 4.

## Preparação para implementação: trilhas 0 e 1

Decidido pelo mantenedor em 11/10/2026: as trilhas 0 e 1 vêm primeiro, e a implementação é feita por agentes, um PR por vez na fila normal. A reserva das unidades fica na [issue #61](https://github.com/OmsiModsBR/depot/issues/61).

### Como cada PR é feito

1. Reservar a unidade na issue #61 (arquivos e branch) e conferir os PRs abertos que tocam esses arquivos. Se houver um, esperar ou deixar o arquivo de fora.
2. Abrir a branch `feat/<nome>` a partir do main e fazer só o que o item pede, sem mudar comportamento.
3. Provar a equivalência com o que o item indica: traços da física, golden no lavapipe, B0 (depois de T0.2) e os testes do workspace.
4. Abrir o PR, levar o CI ao verde, e depois do merge atualizar a issue e as branches abertas que tocavam os arquivos movidos.

### Trilha 0

| Item | Branch | O que entra | Pronto quando |
| --- | --- | --- | --- |
| T0.1 | `feat/ci-lavapipe` | Job novo em `depot-ci.yml` que instala o Vulkan por software (`mesa-vulkan-drivers`) no runner e roda `OMSI_GOLDEN_RENDER=1 cargo test -p omsi-app --test offscreen_golden`. O golden passa a ser abençoado também nesse adaptador, com o nome dele no arquivo de referência, para não brigar com o das máquinas com GPU. Começa informativo | O job passa em três execuções seguidas no mesmo commit; depois disso vira bloqueante |
| T0.2 | `feat/offscreen-traco-estado` | Opção de teste que grava, por frame, posição e velocidade do jogador, contagem e posições dos carros de IA, aspectos dos sinais e o estado do duty; um teste `offscreen_state_trace.rs` sobre a raiz sintética; `scripts/offscreen-trace-diff.sh [base]` no mesmo formato do `physics-trace-diff.sh`; job `offscreen-traces` que compara base e PR, com um rótulo para mudanças de comportamento intencionais | Dois PRs de paridade seguidos passam pelo job sem falso alarme |
| T0.3 | `feat/settings-ida-volta` | Teste que escreve um `settings.cfg` pelo jogo e o lê pelo launcher-core, e o contrário. As diferenças de padrão que existem hoje entram numa lista de divergências conhecidas no próprio teste, sem corrigi-las aqui (cada uma vira um PR `fix/` ou espera a D1) | O teste roda no CI e a lista de divergências está documentada |

### Trilha 1

| Item | Branch | O que entra | Pronto quando |
| --- | --- | --- | --- |
| T1.1 | `feat/fmt-formatos` | `cargo fmt -p` nos crates de formato (`omsi-cfg`, `-o3d`, `-model`, `-map`, `-vehicle`, `-scenery`, `-texture`, `-timetable`, `-content`, `-script`), em até três PRs de no máximo quatro crates cada. O passo de fmt do CI ganha uma lista de crates formatados com `--check` bloqueante; o resto continua informativo | Lista no CI cobrindo esses crates |
| T1.2 | `feat/fmt-runtime-frio` | O mesmo para `omsi-audio`, `omsi-ui`, `omsi-geometry` e `tools/omsi-check` | Idem |
| T1.3 | `feat/clippy-formatos` | Correção dos avisos do clippy nesses crates, um PR por crate ou grupo pequeno. O CI continua com o clippy informativo, por decisão do mantenedor; o `-D warnings` fica para depois | Zero avisos nesses crates no relatório do CI |
| T1.4 | `feat/render-modulos-*` | C1 em cinco PRs, nesta ordem: `cargo fmt -p omsi-render`; tipos sem lógica (`camera`, `targets`, `material`); `gpu` e `lighting`; `pipelines`; `passes` e as etapas de `new_on` e `render_inner`. Cada PR só move código | Golden idêntico no lavapipe a cada PR; nenhum arquivo acima de 3.000 linhas no fim |
| T1.5 | `feat/omsi-html-*` | Em três PRs. Primeiro, os tipos que o motor HTML usa de fora (`ApiValue` de `vehicle_api.rs`, o trait `HtmlRenderer`, `PointerKind` e `rebase_css_urls` de `htmltex.rs`) vão para dentro de `htmlengine`, com reexportação nos lugares antigos. Depois, o crate `omsi-html` recebe os arquivos, e `omsi-sim` passa a depender dele e a reexportá-lo como `htmlengine`. Por último, os testes que montam um snapshot de veículo (`snapshot`, `environment`) ficam em `omsi-sim` como testes de integração, e o resto vai com o crate | `omsi-html` compila sem `omsi-sim`; traços da física e testes iguais |

T0.1 vem antes de T1.4, porque o golden no lavapipe é a prova de cada PR do renderer. Os outros itens não dependem entre si e podem ir na ordem da tabela.

O que se espera no fim, em números medidos hoje: o `omsi-app` compilado no jogo padrão cai de 124,8 mil para menos de 50 mil linhas, com cerca de 10,7 mil atrás das features da Frente A1 e 50 mil movidas para crates próprios. O código de LAN continua no repositório e conta à parte até virar o crate opcional `omsi-lan-game`. O maior arquivo cai de 15.830 para menos de 3.000 linhas, e as 40 funções acima de 300 linhas ficam reduzidas às que são parsers ou tabelas por natureza (`tokenize`, `handle_keyword`, `parse_objects`).

## Decisões pendentes

Nenhuma destas decisões bloqueia a paridade nem a triagem. A recomendação vem primeiro em cada linha.

| Decisão | Recomendação | Alternativa | Por quê |
| --- | --- | --- | --- |
| Ordem de implementação | **Decidido (11/10/2026):** trilhas 0 e 1 primeiro | — | — |
| Quem implementa | **Decidido (11/10/2026):** agentes, um PR por vez | — | — |
| Placa de vídeo no CI | **Decidido (11/10/2026):** adaptador por software (lavapipe) no runner | — | Golden e B0 rodam em todo PR |
| Largura do rustfmt (A2) | **Decidido (11/10/2026):** 100 colunas | — | É o que o CONTRIBUTING promete |
| Clippy nos crates limpos | **Decidido (11/10/2026):** continua informativo por enquanto | — | — |
| `omsi-html` na trilha 1 | **Decidido (11/10/2026):** sim | — | — |
| Reserva das unidades | **Decidido (11/10/2026):** issue no GitHub, criada como a #61 | — | Todos os colaboradores acompanham o GitHub |
| Multiplayer, servidor e plugins (A1) | **Decidido (10/10/2026):** ficam no repositório, fora da compilação do jogo | — | Serão reaproveitados e readaptados mais tarde |
| Unidade de divisão da refatoração | Crate nos crates frios, grupo de módulos no `omsi-app` | Só por crate | O `omsi-app` tem 55% do código e metade dos commits |
| Prioridade entre refatoração e correções | A refatoração cede | Janelas de congelamento longas | Paridade e triagem são as fases ativas do mapa |
| Formatação (A2) | Por crate, com `scripts/fmt-merge.sh` para os quentes | Um commit único | O commit único quebra toda branch aberta |
| Quem porta branches abertas depois de uma movimentação | Quem moveu, ou a rotina diária | O dono de cada branch | Quem moveu sabe para onde cada trecho foi |
| Granularidade das features (A1) | Três: `lan`, `server` (implica `lan`) e `plugins` | Uma só, `online` | Plugins não dependem de rede |
| CI do código guardado (A1) | Job `cargo check --workspace --all-features` | Não compilar o código guardado | Sem o job, o código guardado quebra a cada refatoração do app |
| `signal_aspects` no offscreen (B6) | Passar o trilho do jogador, igual à janela | Manter `None` | O offscreen deve reproduzir o jogo |
| Onde mora o `Settings` compartilhado (D1) | Em `omsi-launcher-core` | Crate novo `omsi-settings` | Um crate a menos para manter |
| Crate `omsi-world` (E2) | Decidir na trilha 3 | Criar junto com `omsi-gameplay` | Depende do que o gameplay ainda precisar depois de E1 |
| Renomear `omsi-*` para `depot-*` | Não por enquanto | Renomear crates e identificadores | Custa um PR grande e não melhora a legibilidade |
| Trilha 3 durante a Trilha F (16/10/2026) | Esperar a Trilha F terminar | Deixar D5, D6 e C5 andarem junto, já que não tocam a física | É a regra do projeto; a alternativa só vale com decisão do mantenedor |
| Testes da IA no jogo antes do merge de F1 | Um mapa e os ônibus de IA de série nas máquinas do mantenedor, main contra a branch | Só os traços sintéticos e o offscreen | O conteúdo de série não pode ir para o CI |
| Valor quando o `.bus` não traz o parâmetro (F2) | O do OMSI, decidido uma vez em `spec.rs` | Manter os padrões atuais | Os padrões atuais compensam modelos mal configurados |
| Feature das heurísticas de material de veículo (C4) | Uma feature desligada, `vehicle-heuristics`, no lugar da variável `OMSI_REPAIR_BODY_DEPTH` | Manter a variável | O código fica no repositório, mas fora do jogo padrão |
| Onde mora a matemática pequena (D5) | `omsi-geometry::util`, com a psicrometria em `omsi-content::weather` | Um `omsi-cfg::math` | `omsi-app`, `omsi-render` e `omsi-sim`, onde as cópias estão, já dependem dele |
| Onde mora a tabela `SettingSpec` (D1) | Em `omsi-launcher-core`, junto com o `Settings` | Em `omsi-cfg` | Mantém a recomendação de D1 e um crate a menos |
| A pilha de UI em CPU de `ui.rs` (C6) | Parar de usá-la no menu e deixá-la atrás de uma feature quando não sobrar uso | Mantê-la ativa ao lado de `omsi-ui` | Duas pilhas desenham o mesmo menu de dois jeitos |
| Passo 1 da C6 depois da nova interface (23/10/2026) | Reescrever: separar dados e desenho do HUD na Trilha App e aposentar o desenho em CPU na trilha da nova interface | Dar o passo 1 por encerrado | O menu já é do `omsi-ui`, mas o resto do HUD ainda tem dois caminhos |
| D1 e o #151 (23/10/2026) | Um registro só de configurações | Dois trabalhos separados | A nova interface já consome o registro do #151 |
| PRs `fix/launcher-*` abertos e a D1 (23/10/2026) | Mesclar antes da D1 | Fechá-los e deixar a D1 resolver | Correção não espera refatoração |
| Contrato do Direct3D 9 (C7) | Manter o `omsi-render` contabilizando e entregando o plano, documentado e testado | Um trait de backend comum | Só há duas implementações, e a segunda já funciona assim |

## Riscos e critério de sucesso

- **Regressão silenciosa de comportamento.** Muitos testes pulam sem `OMSI_ROOT`, e as imagens do golden ainda não rodam no CI. Mitigação: o traço de estado B0 no CI, os traços da física (bloqueantes) e `omsi-check` antes e depois de cada fase, como o CONTRIBUTING já pede.
- **A rede de segurança não cobre a janela** (auditoria de 16/10/2026). O golden e o B0 rodam o offscreen, que hoje tem a sua própria cópia da montagem e do passo. Uma refatoração que quebre só o caminho da janela passa no CI. Mitigação: a Frente B no começo da Trilha 4, e até lá testar manualmente no jogo cada PR que mexa no frame da janela.
- **O Direct3D 9 fora da rede de segurança** (23/10/2026). O golden e o B0 rodam sobre o wgpu, e os testes de dispositivo do `omsi-render-d3d9` só no Windows com driver. Mitigação: a C7 e o job do Windows verde em todo PR que toque a saída gráfica.
- **Golden dependente de máquina.** As imagens só batem no mesmo adaptador gráfico. Mitigação: o B0 compara estado, não pixels, e roda em qualquer máquina; o golden fica para as Frentes C1 e o render.
- **Conflito com paridade e triagem.** As áreas quentes da refatoração são as mesmas onde os bugs são corrigidos. Mitigação: as regras de trabalho em paralelo (a refatoração cede, PRs curtos, reserva visível, quem move porta) e as áreas quentes só por janelas anunciadas.
- **Branches paralelas.** Sem upstream, o conflito que importa é entre branches do próprio depot. Cada movimentação grande (A2, C1, E2) é anunciada e mergeada rápido, num momento sem branches longas abertas; o `CHANGELOG.md`, que já conflita em quase todo merge, pode passar a usar fragmentos por PR.
- **PRs grandes demais para revisar.** Cada extração é um PR de movimentação pura, sem mudar lógica, seguido de PRs de ajuste.
- **Redação.** Comentários e documentos criados pela refatoração descrevem comportamento observável, sem identificadores do executável original, como fizeram os PRs #15 a #24.

Critério de sucesso: uma mudança típica de gameplay, como uma nova regra de tráfego, toca um crate e um ou dois arquivos abaixo de 2.000 linhas, roda igual no jogo e no offscreen, e pode ser testada sem GPU.
