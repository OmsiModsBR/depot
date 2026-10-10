# Macro-refatoração do depot (rascunho)

Rascunho de 10/10/2026. Versão editável e comentável: [Claude Doc](https://claude.ai/code/artifact/50cfb0f6-9b77-45af-bb95-31469df26758).

## Resumo

O maior problema de manutenção do depot é a concentração: o crate `omsi-app` tem 124,7 mil das 224 mil linhas de Rust do workspace (56%), e três funções passam de 2.900 linhas cada. A proposta é atacar isso em fases pequenas, sem mudar comportamento, começando por guarda-corpos baratos (rustfmt, clippy, CI) e terminando com a separação entre simulação e apresentação.

Escopo: leitura inicial do `main` no commit `a521843` (base openOMSI v0.2.16), sem alterar código; revista em 10/10/2026 contra o `main` em `d99a219` (PRs #1 a #6). Números medidos com scripts sobre os fontes; contagens de função são aproximadas (casamento de chaves).

As cinco recomendações principais:

1. Tornar formatação e lint um portão de PR. O CI do Depot (`depot-ci.yml`, PR #1) já roda `cargo fmt --check`, `clippy`, `cargo audit` e `cargo deny`, mas só como informativos: o fmt ainda acusa cerca de 8.000 trechos e o clippy cerca de 550 avisos (`docs/QUALITY.md`).
2. Unificar o laço de simulação. O modo janela (`app_events.rs`) e o modo offscreen (`offscreen.rs`) orquestram o mesmo frame em cópias que já divergem.
3. Quebrar as funções gigantes do renderer e do app em etapas nomeadas (`Renderer::new_on` com cerca de 5.500 linhas, `window_event` com 3.000, `run_offscreen` com 3.000).
4. Tirar do `omsi-app` a lógica de domínio (tráfego, horários, passageiros) que não depende de janela, e fazer a simulação emitir pedidos em vez de chamar o renderer diretamente.
5. Centralizar as cerca de 300 variáveis de ambiente `OMSI_*` espalhadas por 77 arquivos num registro tipado e documentado.

Contexto que libera o plano: o depot segue caminho próprio e não vai mais integrar mudanças com o openOMSI. Isso tira o principal freio a movimentações grandes de arquivo e renomeações, e faz da fase 4 (divisão de crates) uma meta normal, não opcional. A "integração seletiva" descrita no `DEPOT_BASELINE.md` se refere ao próprio Depot (cada mudança revista e integrada individualmente), não ao openOMSI, então a política do arquivo continua válida. Renomear os identificadores internos `omsi-*` também passa a ser possível, mas é opcional: custa um PR grande e não melhora a legibilidade por si.

## Panorama atual

O workspace tem 21 crates e três camadas: formatos de arquivo embaixo, runtime no meio e o `omsi-app` em cima. As duas de baixo estão razoavelmente organizadas; o peso está no topo.

| Camada | Crate ou área | Linhas de Rust |
| --- | --- | --- |
| `omsi-app` (124.700, depende de 18 crates internos) | Gameplay: tráfego, horário, pessoas | ~29.000 |
| | Laço, eventos, offscreen, VR, resto | ~28.500 |
| | Mundo, cena e tiles | ~17.900 |
| | Input: controles, toque, FFB | ~13.400 |
| | Launcher (`launcher/`) | ~13.300 |
| | UI, HUD, navegador | ~8.200 |
| | LAN e servidor | ~6.800 |
| | Plataforma: Discord, Steam, updater | ~4.600 |
| | Clima e chuva | ~3.000 |
| Runtime (~74.000) | `omsi-sim` (inclui motor HTML/JS) | ~31.500 |
| | `omsi-render` (`lib.rs` com 15.800) | ~20.000 |
| | `omsi-net` | ~9.200 |
| | `omsi-launcher-core` | ~6.100 |
| | `omsi-audio` + `omsi-ui` | ~4.200 |
| | `omsi-plugin` | ~3.200 |
| Formatos (~23.400, grafo limpo e acíclico) | `omsi-cfg`, `-script`, `-o3d`, `-model`, `-scenery`, `-map`, `-vehicle`, `-timetable`, `-content`, `-texture`, `-geometry` | ~23.400 |

Os números são linhas de Rust medidas no `main`, arredondadas. Além do tamanho, a direção das dependências é saudável: nenhum crate de formato depende de runtime, e `omsi-sim` não depende de `wgpu`. O problema é que tráfego, horário e passageiros moram no `omsi-app` justamente porque falam direto com o renderer.

## Code smells encontrados

Os smells mais caros são estruturais (arquivos e funções enormes, lógica duplicada), não de estilo. A tabela vai do maior impacto ao menor.

| Smell | Evidência | Por que dói |
| --- | --- | --- |
| Crate monolítico | `omsi-app`: 110 arquivos, 124.713 linhas, depende de 18 crates internos | Toda mudança recompila o crate maior; o limite entre jogo, launcher, VR, LAN e simulação só existe por convenção |
| Arquivos gigantes | `omsi-render/src/lib.rs` 15.830 linhas; `omsi-app/src/scene.rs` 14.857; `traffic.rs` 8.311; `schedule.rs` 6.820; `humans.rs` 6.754; `omsi-sim/src/vehicle.rs` 6.077 | Revisão e navegação lentas; conflitos de merge frequentes entre branches paralelas |
| Funções gigantes | 40 funções acima de 300 linhas, 140 acima de 150. Maiores: `Renderer::new_on` (lib.rs:2490, \~5.500), `window_event` (app\_events.rs:67, \~3.000), `run_offscreen` (offscreen.rs:5, \~3.000), `render_inner` (lib.rs:8673, \~2.400), `build` (lib.rs:2845, \~2.000) | Impossível testar uma etapa isolada; variáveis locais vivem milhares de linhas |
| Laço de frame duplicado | O passo do tráfego aparece em `app_events.rs:625` e `offscreen.rs:636`; o papel de horário em `app.rs:850`, `app_events.rs:1554`, `offscreen.rs:114` e `:663`, `game_lists.rs:2924` | As cópias já divergem: a janela passa o trilho do jogador a `signal_aspects` (app\_events.rs:629), o offscreen passa `None` (offscreen.rs:638); testes offscreen podem não reproduzir o jogo |
| God objects | `App` com \~190 campos (49 `Option`), `Settings` \~137, `Renderer` \~129, `Humans` \~100, `Traffic` \~78, `World` \~76 | Qualquer método enxerga todo o estado; o empréstimo mutável de `self` bloqueia paralelizar etapas |
| Simulação acoplada ao renderer | `omsi-app/src/traffic.rs` cita `Renderer`/`Scene` 34 vezes e cria veículos via `world.add_vehicle_shared(renderer, scene, …)` (traffic.rs:1527); idem `humans.rs` (30) e `schedule.rs` (12) | A lógica de tráfego não pode ir para `omsi-sim` nem rodar sem GPU (servidor dedicado, testes) |
| Variáveis de ambiente como configuração | \~297 nomes `OMSI_*` distintos lidos em 77 arquivos; 129 deles são ganchos `DEBUG`/`FAKE`/`NO`/`TEST`; só \~108 aparecem em `docs/` | Comportamento invisível; o cache delas mora em `omsi-cfg` (lib.rs:1205), um crate de formatos de arquivo |
| Namespace achatado | `lib.rs` do app faz `use app::*`, `use offscreen::*` e mais 14 globs; 32 módulos começam com `use super::*` | Não dá para saber de onde vem um nome sem IDE; renomear quebra coisas longe |
| Instrumentação manual | 28 `let __tN = Instant::now()` e 29 `self.profile.entry("…")` só em `app_events.rs` | Ruído no meio da lógica; chaves de string sem verificação |
| Configuração com parser manual | `Settings::from_text` (settings.rs:432) lê `chave=valor` campo a campo para \~137 campos, com `serde` já no workspace | Cada opção nova mexe em quatro lugares (campo, default, parse, save) |
| APIs com JSON solto | `omsi-launcher-core` usa `serde_json::Value`/`json!` 223 vezes em `lib.rs`, herdado da IPC do Tauri que já saiu | Erros de chave só aparecem em tempo de execução |
| Nomes legados | Pacote `omsi-launcher-core` com lib `omsi_launcher_lib`, importada como `core` (launcher/mod.rs:27), sombreando o `core` da linguagem; `#[path]` em humans.rs:57 e scene.rs:14376 | Confunde quem chega; `core` sombreado é armadilha em macros |
| `unwrap`/`expect` fora de testes | \~1.350 `unwrap()` fora de arquivos de teste (1.701 no total), 255 `expect`, 33 `panic!` | Conteúdo de mod malformado pode derrubar o jogo em vez de virar aviso |
| Lints silenciados | 47 `#[allow(clippy::too_many_arguments)]`, 7 `dead_code` | Sintoma dos god objects: funções recebem `world, renderer, scene, …` soltos |

Pontos que não são problema: o grafo entre crates de formato (`omsi-cfg`, `-o3d`, `-model`, `-map`, …) é limpo e acíclico, as dependências externas estão centralizadas em `[workspace.dependencies]`, há só 6 TODO/FIXME e os comentários explicam o porquê, como o CONTRIBUTING pede.

## Propostas de refatoração

As propostas estão agrupadas por eixo e ordenadas dentro de cada eixo da mais barata para a mais cara. Nenhuma muda o comportamento do jogo; cada uma cabe em PRs pequenos.

### Guarda-corpos (antes de mexer em estrutura)

1. **Formatar uma vez e travar.** Um PR só com `cargo fmt --all`, registrado em `.git-blame-ignore-revs`, e depois `cargo fmt --check` no CI. Se a largura atual for intencional (há 2.515 linhas acima de 140 colunas), definir `max_width` num `rustfmt.toml` em vez de fingir o padrão.
2. **CI: de informativo a bloqueante.** O PR #1 criou o `depot-ci.yml`, com build e testes bloqueantes e um job `quality` informativo (fmt, clippy, audit, deny). Falta virar cada passo para bloqueante à medida que o backlog de `docs/QUALITY.md` zera, começando pelo fmt logo depois do commit de formatação.
3. **Clippy com baseline.** Ligar `[workspace.lints]` em `Cargo.toml`, começando por `clippy::unwrap_used` como aviso nos crates de formato (`omsi-cfg`, `-o3d`, `-model`, `-map`, `-vehicle`, `-scenery`), onde um arquivo de mod quebrado nunca deveria derrubar o processo.
4. **Testes de caracterização.** O modo `--offscreen` já renderiza sessões roteirizadas em PNG. Fixar 3 ou 4 cenários (mapa, ônibus, hora, tráfego) como testes de imagem e de estado (posições, variáveis de script) dá uma rede de segurança para as fases 2 a 4.

### Legibilidade

1. **Imports explícitos.** Trocar os 16 `use módulo::*` de `omsi-app/src/lib.rs` e os 32 `use super::*` por imports nomeados, módulo a módulo. Mecânico, revisável e sem risco.
2. **Funções gigantes viram etapas nomeadas.** Extrair sem mudar ordem nem lógica:
   - `Renderer::new_on` em `open_device`, `Pipelines::new`, `ShadowTargets::new`, `PostChain::new`, `SkyResources::new`, cada um devolvendo um struct próprio que o `Renderer` compõe.
   - `render_inner` em uma função por passada (sombras, principal, reflexos, pós, overlay), com um `FrameContext` carregando o que hoje são locais.
   - `window_event` num `match` curto que despacha para `on_keyboard`, `on_mouse`, `on_resize`, `on_redraw`.
3. **Perfil por escopo.** Substituir os pares `__tN`/`profile.entry` por um guard (`let _s = profile.scope(Stage::Traffic);`) com um `enum Stage`, ou pelo crate `tracing` se o custo for aceitável.
4. **Nomes.** Renomear a lib `omsi_launcher_lib` para `omsi_launcher_core` e parar de importá-la como `core`; trocar os `#[path]` por módulos em pasta (`humans/pax.rs`, `scene/tests/…`).

### Manutenibilidade

1. **Um único passo de simulação.** Criar um `Session` (ou `Simulation`) que possua relógio, clima, tráfego, horário, passageiros e jogador, com `fn step(&mut self, dt, &Inputs) -> StepOutput`. A janela e o offscreen passam a chamar o mesmo `step`, e a divergência do `signal_aspects` some por construção. Isso também encolhe o `App` de \~190 campos.
2. **Simulação emite pedidos, apresentação executa.** Em vez de `traffic` chamar `world.add_vehicle_shared(renderer, scene, …)`, ele empilha `SpawnVehicle`/`ReleaseVehicle` numa fila que um `RenderSync` aplica no fim do frame. É o passo que permite mover tráfego, horário e passageiros para fora do `omsi-app` e rodá-los sem GPU (testes e offscreen; o servidor LAN foi desativado no PR #1).
3. **Registro de flags.** Um módulo (ou crate `omsi-flags`) com uma macro que declara cada `OMSI_*` uma vez: nome, tipo, padrão e doc. O código lê `flags::DEBUG_TRAFFIC` tipado, e um teste gera a lista em `docs/` e falha se faltar documentação. Tirar o cache de variáveis de `omsi-cfg`.
4. **Settings declarativo.** Derivar a leitura e escrita de `Settings` (serde com um formato `chave=valor` próprio, ou uma macro de tabela) para que opção nova seja uma linha. Manter o formato do arquivo para não quebrar perfis existentes.
5. **Tipar o launcher-core.** Substituir `serde_json::Value` por structs com `Serialize`/`Deserialize` nas funções públicas; o CLI continua emitindo JSON pelo `serde`.
6. **Erros de conteúdo não derrubam.** Nos parsers, trocar `unwrap` por erro com contexto (`anyhow::Context` já é usado) e um coletor de avisos por carregamento, mostrado no log e no `omsi-check`.

### Estrutura (crates)

Depois que o passo de simulação estiver unificado e desacoplado do renderer, o `omsi-app` pode ser dividido por área. Ordem sugerida, das áreas com menos dependências para as com mais:

1. `omsi-html`: o motor HTML/JS de `omsi-sim/src/htmlengine` (\~5.000 linhas), que não é simulação.
2. `omsi-input`: controles, DirectInput, evdev, HID do macOS, toque, head tracking, force feedback (\~13.400 linhas no app).
3. `omsi-launcher`: a pasta `launcher/` (\~13.300 linhas), que já tem fronteira clara com `omsi-launcher-core`.
4. `omsi-lan-game`: `lan.rs`, `lan_world.rs`, `lan_mods.rs`, `server.rs` (\~6.800 linhas). Revisto em 10/10: o PR #1 desativou multiplayer, servidor dedicado e plugins no cliente e deixou esse código inalcançável (`docs/removed/MULTIPLAYER_DISABLED.md`). Em vez de extrair, decidir entre apagar o código morto (junto com a UI de chat, voz e name tags) ou isolá-lo atrás de uma feature desligada.
5. `omsi-gameplay`: tráfego, horário, passageiros, carreira, motorista (\~29.000 linhas), só depois do item 2 de manutenibilidade.
6. Dentro de `omsi-render`, quebrar `lib.rs` em módulos (`pipelines`, `passes/*`, `scene`, `lighting`, `materials`) sem criar crate novo.

O `omsi-app` restante fica com janela, laço de eventos, mundo/tiles e a cola entre os crates.

## A branch de articulação multicorpo

A branch `claude/multibody-articulation-ofd4nd` (PR #8) já segue a direção deste documento e deve entrar antes de qualquer refatoração em `omsi-sim`. O tipo de articulação Pusher entrou no `main` pelo PR #5; a branch agora soma +2.111/−825 linhas em 7 arquivos sobre o `main` e cria dois submódulos novos: `rigid/train.rs` (857 linhas, corpos ligados por juntas, incluindo o Pusher) e `vehicle/articulation.rs` (664 linhas, as seções acopladas atrás do veículo).

O que isso muda no plano:

- **Ordem.** `rigid.rs` (1.034 linhas tocadas) e `vehicle.rs` (379) são justamente dois dos maiores arquivos do `omsi-sim`. Refatorar esses arquivos antes do merge geraria conflitos grandes; então eles ficam de fora até a branch entrar.
- **Padrão a copiar.** A branch extrai uma responsabilidade inteira para um submódulo com doc de módulo explicando o modelo físico. É o formato que este plano propõe para `vehicle.rs` (6.077 linhas) e `rigid.rs` (2.399): `vehicle/{script, anim, input, doors, articulation}.rs` e `rigid/{body, tyres, ground, train}.rs`.
- **Smells que ela herda.** `articulation.rs` começa com `use super::*` e acrescenta mais uma variável, `OMSI_KINEMATIC_TRAILERS`. As duas coisas seguem o padrão atual e seriam absorvidas pelos itens de imports explícitos e registro de flags.
- **Extensões do formato.** A palavra-chave `[coupling_pitch_offset]` não existe no OMSI 2 original. Vale marcar as extensões do Depot num lugar só (por exemplo, uma tabela em `docs/FORMATS.md` e uma constante por palavra-chave em `omsi-vehicle`), para que a compatibilidade 1:1 continue verificável.

## Plano em fases

Cinco fases em sequência, cada uma liberando a seguinte. Sem integração com o openOMSI, não há custo de divergência a pesar: a ordem existe só para que cada fase conte com a rede de segurança da anterior.

1. **Fase 0: guarda-corpos.** rustfmt de uma vez, CI com testes e clippy, `[workspace.lints]`, testes de caracterização pelo offscreen.
   - Portão para seguir: CI verde no `main` e pelo menos 3 cenários offscreen fixados.
   - Junto: um `.git-blame-ignore-revs` com o commit de formatação, para o `git blame` continuar útil.
2. **Fase 1: legibilidade mecânica.** Imports explícitos, perfil por escopo, renomeações, fim dos `#[path]` e registro de flags.
   - Portão: nenhum `use …::*` fora de testes e preludes; toda `OMSI_*` declarada e documentada.
3. **Fase 2: merge da articulação multicorpo** e, em seguida, a divisão de `omsi-sim/src/vehicle.rs` e `rigid.rs` em submódulos, no mesmo formato que a branch introduziu.
   - Portão: nenhum arquivo de `omsi-sim` acima de 2.500 linhas; testes de física passando.
4. **Fase 3: passo de simulação único.** `Session::step` usado pela janela e pelo offscreen; quebra de `window_event`, `run_offscreen`, `Renderer::new_on` e `render_inner` em etapas.
   - Portão: os cenários offscreen produzem o mesmo estado antes e depois (com a divergência do `signal_aspects` corrigida de propósito e registrada); nenhuma função acima de 400 linhas nesses arquivos.
5. **Fase 4: desacoplar e dividir crates.** Fila de pedidos entre simulação e renderer, depois a extração de `omsi-html`, `omsi-input`, `omsi-launcher` e por último `omsi-gameplay`. O código de LAN, inalcançável desde o PR #1, sai antes (apagado ou atrás de uma feature desligada).
   - Portão: `omsi-gameplay` compila e testa sem `wgpu`; `omsi-app` abaixo de 40 mil linhas.

Riscos principais:

- **Regressão silenciosa de comportamento.** O jogo depende de compatibilidade fina com o OMSI 2 e muitos testes pulam sem `OMSI_ROOT`. Por isso os testes de caracterização vêm primeiro, e o `omsi-check` deve rodar antes e depois de cada fase, como o CONTRIBUTING já pede.
- **Branches paralelas.** Sem o upstream, o conflito que importa é entre branches do próprio depot, como a de articulação. Cada movimentação grande deve ser anunciada e mergeada rápido, num momento sem branches longas abertas.
- **PRs grandes demais para revisar.** Cada extração deve ser um PR de movimentação pura (sem mudar lógica), seguido de PRs de ajuste.

Critério de sucesso geral: uma mudança típica de gameplay (por exemplo, uma nova regra de tráfego) passa a tocar um crate e um ou dois arquivos abaixo de 2.000 linhas, e o mesmo código roda no jogo e no offscreen.
