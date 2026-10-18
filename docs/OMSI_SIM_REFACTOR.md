# Refatoração do `omsi-sim` por domínio (plano)

Plano de 18/10/2026 para o crate `omsi-sim`, medido no `main` em `7cc8474` (PR #303). Só leitura de código; contagens de linha e de função são aproximadas (casamento de chaves). É a **Trilha S** do [plano macro](MACRO_REFACTOR_DRAFT.md#trilha-s-omsi-sim-por-domínio) e segue as regras de lá: um PR por vez, reservado na issue #61, branches `feat/` para estrutura e `fix/` para qualquer mudança de comportamento.

Três objetivos, pedidos pelo mantenedor:

1. **Separar testes e auxílios de teste do código funcional.**
2. **Quebrar o código por domínio e subdomínio**, seguindo SOLID e Clean Code.
3. **Tirar o código duplicado**, de produção e de teste.

## Regras herdadas

- **Refatoração não muda comportamento.** Todo PR deste plano prova isso: traços da física e da IA idênticos bit a bit (`scripts/physics-trace-diff.sh`), `offscreen-traces` e golden iguais, e a mesma lista de testes (`cargo test -p omsi-sim -- --list`, comparando os nomes sem o caminho do módulo). Qualquer diferença achada no caminho vira um PR `fix/` separado.
- **A física do tráfego IA só muda de lugar**, com testes que provam que ela se comporta igual no jogo. As compensações que só existem na IA (`ai_lift`, `ai_rest_offset`, `rotation_point`, o limite de ±1,2 rad) ficam como estão.
- **O jogo não compensa modelos mal configurados.** Nenhuma etapa deste plano acrescenta compensação; as que existem só na IA não são tocadas.
- **Nada se apaga**, com duas exceções decididas pelo mantenedor: as quatro funções sem uso listadas abaixo, que saem neste plano, e a feature `model-fallbacks`, que sai depois da validação da física no jogo, fora deste plano. O resto do código sem uso fica no repositório; no máximo sai da compilação do jogo atrás de uma feature.
- **Os caminhos que os harnesses de traços usam não mudam.** O script de comparação compila `tests/physics_traces.rs` e `tests/ai_traces.rs` do PR também no commit base. Eles usam `omsi_sim::{ai, collision, physics, vehicle}` e as reexportações da raiz (`Controls`, `KinematicModel`, `SimClock`, `VehicleHost`, `VehicleInstance`, `VehicleType`); esses caminhos continuam valendo do começo ao fim.

## Diagnóstico

O crate tem 82 arquivos `.rs` e 28.439 linhas em `src/`, além de 2.610 em `tests/` e 802 em `examples/`. A física (`physics/`, com `domain/`, `interface.rs` e `implementation/`) e o veículo (`vehicle/`, 15 submódulos desde o #128) já estão divididos e servem de modelo. O resto ainda está em arquivos planos na raiz.

### Testes misturados com o código

- **7.314 das 28.439 linhas de `src/` (26%) são testes.** Só cinco arquivos de teste já são separados (`physics/tests.rs`, `physics/implementation/train/tests.rs`, `vehicle/tests.rs`, `ai/tests.rs` e `startup/tests.rs`). Os outros 26 arquivos têm módulos `#[cfg(test)]` dentro do código.
- Os maiores: `traffic.rs` (791 linhas de teste em quatro módulos: `extend_tests`, `tests`, `light_tests`, `aurora_pool_tests`), `human.rs` (522), `input.rs` (377, 60% do arquivo), `collision.rs` (233 em quatro módulos), `physics/implementation/collide.rs` (219, 63%), `host.rs` (218), `vehicle/articulation.rs` (154).
- Em `human.rs` o módulo de testes fica no meio do arquivo (linhas 2222 a 2749) e o código funcional continua depois dele (`curl_hands`, `grip_centres`, `hand_slot`, `skin_from`).
- `startup.rs` tem dois módulos de teste: um dentro do arquivo (linhas 88 a 213) e `regressions`, que aponta para `startup/tests.rs` com `#[path]`.

### Auxílios de teste no código funcional

- **API pública usada só por testes**: `Pose::sit_amount` (`human.rs`) e `VehicleType::mesh_dropped` (`vehicle/vehicle_type.rs`), só pelos testes unitários; `RigidBody::{wheels_mut, set_velocity, set_friction}` e `RigidWheel::{set_spin, set_spring_factor}` (`physics/domain/`), só pelos harnesses de traços; `attach_trailer` (`vehicle/trailer.rs`), pelos testes unitários e pelos harnesses (o jogo usa `attach_trailer_ex`).
- `vehicle.rs:39` importa `physics_bridge::scripts_acceleration` com `#[cfg(test)]` para os testes de `vehicle/tests.rs`.

### Duplicação

Em testes:

- **Pacote de veículo temporário montado à mão em 12 lugares** (`std::env::temp_dir()` mais `.bus`, `model.cfg`, lista de variáveis e `.osc` escritos um a um): `host.rs` (4), `startup.rs` (3), `startup/tests.rs`, `scenery.rs`, `vehicle/tests.rs`, `tests/automated_gearbox_script.rs` e `tests/htmlengine_game.rs`. Cada um tem seu contador de nomes; alguns não apagam a pasta no fim.
- **O articulado GN92 está copiado três vezes**, igual linha por linha (`physics/implementation/train/tests.rs`, `tests/physics_invariants.rs`, `tests/physics_traces.rs`), e há outros veículos e pisos sintéticos repetidos entre os três (`bus`, `axle`, `flat`, `humps`).
- **O gravador de traço está copiado** entre `physics_traces.rs` e `ai_traces.rs` (`n32`, `n64`, `dv3`, `frame`, `body`, `check`).

Em produção:

- `angle_diff` em `human.rs:901` e `crowd.rs:335`, com a mesma conta.
- O gerador de `ParticleSet::rand` (`particles.rs:132`) é o `omsi_geometry::util::unit_f32` (xorshift64 13/7/17, 24 bits de cima) escrito à mão, vezes 2 menos 1. A D5 trocou as outras cópias e deixou esta.
- `smooth01` (`traffic.rs:1961`) é `omsi_geometry::util::smoothstep(0.0, 1.0, t)` com a mesma aritmética.
- O `wrap_deg` de `traffic.rs:346` (forma `+ 180`) arredonda diferente do de `omsi_geometry` e fica como está, como a D5 decidiu; ele só muda de arquivo.

### Arquivos que misturam domínios

| Arquivo | Linhas | O que tem dentro |
| --- | --- | --- |
| `traffic.rs` | 3.343 | Faixa e geometria de faixa, a rede (ligação, alcance, grade, conflitos, projeção, busca de caminho), semáforos e o motorista da IA (`AiState`: plano, troca de faixa, velocidade, condução) |
| `human.rs` | 2.898 | Ossos e juntas, carga do `.hum`, pose procedural (`Pose::advance` com 441 linhas e `Pose::bones` com 437) e pele |
| `ibis.rs` | 1.105 | Aprender a unidade do IBIS pelo script (`Unit::learn`, 230 linhas), simular, buscar o plano de teclas |
| `startup.rs` | 976 | Ler o estado do veículo pelos scripts, achar gatilhos e mostradores, e a máquina de passos (`StartUp::tick`, 237 linhas) |
| `host.rs` | 969 | O host dos scripts: variáveis de sistema e cerca de 45 callbacks num `match` só de 349 linhas (fontes e texto, script texture, rota e paradas, horário, mundo, bilhetes) |
| `collision.rs` | 958 | OBB, caixa 3D, recorte de polígono, malha de colisão e o mundo de colisão |
| `crowd.rs` | 768 | Desvio entre pedestres, corredor e o grafo de caminhos da cabine |

Outras funções acima de 150 linhas: `VehicleInstance::new` (237), `VehicleType::load_with` (218), `TrailerPart::follow` (194, `vehicle/articulation.rs`), `OmsiAnim::advance` (194, `human_omsi.rs`), `vehicle_api::snapshot` (180), `step_rigid` (174) e `step_physics` (157) em `vehicle/motion.rs`, `prepare` (176, `physics/implementation/tyre.rs`).

### Nomes que enganam

- `VehicleHost` (`host.rs`) também é o host dos scripts dos objetos de cenário (`scenery.rs` usa ele).
- `vehicle/trailer.rs` e `vehicle/articulation.rs` começam com a mesma frase ("as partes acopladas atrás de um veículo") e ambos têm `impl TrailerPart`: um é o engate e o desenho, o outro o corpo próprio de cada seção.
- `ai/` é o corpo do veículo da IA (física), e `traffic::AiState` é o motorista da IA (decisão). O nome não diz qual é qual.

### Estado global

Seis variáveis estáticas que o app escreve e a simulação lê, escondidas das assinaturas: `particles::{EYE, WIND}`, `host::AMBIENT` (`set_ambient_weather`), `daylight::PLACE`, `vehicle_api::LOCALE` e `htmltex::BACKEND`.

### Sem uso (busca de texto no workspace)

`TrafficLightController::phase_index`, `PathGraph::distances_from`, `StartUp::nothing_to_do` e `htmltex::set_backend` não têm chamador. O mantenedor decidiu (18/10/2026) que estas quatro saem do código, como exceção à regra de não apagar.

### Formatação

O `omsi-sim` é um dos três crates ainda fora do `fmt --check` bloqueante: `cargo fmt --check -p omsi-sim` acusa 1.235 trechos.

## Princípios, aplicados a este crate

- **Uma responsabilidade por arquivo (SRP).** Um arquivo fala de um assunto e tem o nome dele. Função longa vira etapas com nome, na mesma ordem, sem mudar a lógica.
- **Aberto para extensão (OCP).** Callbacks do host e famílias de dados entram por módulo de família, como o `handle_keyword` do `omsi-model` (D6, passo 8): cada família devolve "não é meu" e o despachante tenta a próxima. Um callback novo mexe num arquivo só.
- **Interfaces pequenas (ISP).** Cada domínio grande tem um `mod.rs` que reexporta só o que o resto do jogo usa, como o `physics/interface.rs`. Os submódulos são privados.
- **Dependências explícitas (DIP).** O estado global vira parâmetro, mas só junto da Frente B (ver [Decisões pendentes](#decisões-pendentes)).
- **Sem abstração especulativa.** Trait só onde houver duas implementações de verdade. Hoje nenhum domínio deste plano tem; a divisão é por módulos e funções.
- **Clean Code:** nomes que dizem o domínio, sem comentário que conta a história de uma mudança, testes descritos pelo comportamento (o padrão que o crate já segue).

## Convenções novas

1. **Todo teste em `tests/`, nenhum em `src/`** (decisão do mantenedor, 18/10/2026). `src/` fica sem `#[cfg(test)]`, sem `#[test]` e sem `tests.rs`. Os testes que hoje são unitários vão para um binário de teste só, `tests/sim/main.rs`, com um módulo por domínio (`tests/sim/traffic/…`, `tests/sim/people/…`), para não compilar e linkar um binário por arquivo. Os harnesses de traços e os outros testes de integração continuam binários próprios, porque o CI e o script de comparação os chamam pelo nome (`--test physics_traces --test ai_traces`).
2. **Auxílios de teste num lugar só:** `tests/support/` (incluído por `mod support;` em cada binário de teste), com `ScratchPack`, que monta um pacote de veículo ou objeto numa pasta temporária com nome único e a apaga no `Drop`, o gravador de traço e os veículos e pisos sintéticos. O GN92, o gravador e os pacotes temporários passam a existir uma vez.
3. **O que é interno chega aos testes pela feature `test-support`.** Um teste em `tests/` só enxerga a API pública, e cerca de 30 funções e métodos privados (e alguns campos) que os testes de hoje usam não são (`smooth01`, `choose_after`, `bone_id_by_name`, `measure`, `arg_idx`, `starter_engaged`, `winding_pack`, `step_physics` e `update_visuals` do veículo, `inv_mass_at`, entre outros; contagem por busca de texto, a ser conferida no PR). Esses itens passam a `pub` dentro de submódulos privados, o que não os põe na API, e um módulo `omsi_sim::testing`, compilado só com `#[cfg(feature = "test-support")]`, os reexporta. Campos privados ganham acessores no mesmo módulo. O `Cargo.toml` liga a feature para os testes e os exemplos por uma dev-dependency do próprio crate (`omsi-sim = { path = ".", features = ["test-support"] }`); o jogo compila sem ela. Os setters que só os harnesses usam (`RigidBody::set_velocity` e os outros) também passam para trás da feature; no commit base eles continuam sem condição, então os harnesses compilam dos dois lados.
4. **O script de comparação copia `tests/support/` junto com os harnesses**, e `tests/support/` usa só API que já existe no `main` do momento, para compilar também no commit base.
5. **Sem reexportação de caminho antigo.** Cada PR de domínio já troca os imports do `omsi-app` e das ferramentas para os caminhos novos. Só ficam os caminhos que os harnesses usam (ver as regras herdadas).

## Arquitetura alvo

```
src/
  lib.rs              reexportações da raiz (as de hoje) e `testing` atrás da feature
  physics/            como está (domain/, interface.rs, implementation/), sem tests.rs
  ai/                 corpo do veículo da IA, como está (rodovia, trilho, ar, planner)
  vehicle/            como está, mais:
    coupled/          mod.rs (TrailerPart), coupling.rs (engates e posição),
                      kinematic.rs (seção pendurada no engate), dynamic.rs (PartBody, follow),
                      visuals.rs
  traffic/
    mod.rs            o que o app usa
    lane.rs           LaneKind, LaneKey, Lane, prioridade e densidade por pool
    geometry.rs       LaneBuilder, arcos, wrap_deg (+180), heading_dir
    lights.rs         LightStop, TrafficLightController, Aspect
    network/
      mod.rs          Network, find, parallel, beside*
      topology.rs     link, extend, alcance (reach), DEAD_END
      conflicts.rs    Crossing, compute_conflicts, must_yield, opposite, upstream
      spatial.rs      grade, nearest_lane*, lane_along, projeção na rota
      routing.rs      shortest_path, shortest_path_to_any (o route_search.rs de hoje)
    driver/           o motorista da IA (AiState)
      mod.rs          AiState, Lead, constantes
      plan.rs         escolha de saída, plano, rota
      lane_change.rs  LaneChange, LaneSeq, desvio
      speed.rs        arrival_time, curve_speed, desired_accel, rampa de saída
      drive.rs        advance, drive, way_point, pisca
  people/
    mod.rs            o que o app usa
    rig/              ossos e slots, Joints, Rig, HumanType (carga do .hum)
    pose/             PoseInput, Foot, Pose e as etapas de advance e bones; gait.rs; math.rs
    skin.rs           skin, skin_from, mãos (curl_hands, grip_centres)
    omsi_anim.rs      o human_omsi.rs de hoje
    crowd/            steering.rs (Walker, Block, step), corridor.rs, paths.rs (PathGraph)
  host/
    mod.rs            VehicleHost, sys_var e o despachante de callbacks
    callbacks/        text.rs, script_texture.rs, route.rs, timetable.rs, world.rs, tickets.rs
  automation/         o motorista automático do jogador
    startup/          mod.rs (StartUp e as etapas de tick), state.rs (energia, motor),
                      triggers.rs, displays.rs
    ibis/             mod.rs (Typist), unit.rs (Unit e as etapas de learn), sim.rs, search.rs
  collision/          obb.rs, box3.rs, polygon.rs, mesh.rs, world.rs
  textures/           script.rs (scripttex), text.rs (texttex), html.rs (htmltex),
                      page_api.rs (vehicle_api)
  testing.rs          só com `test-support`: reexporta o que é interno para os testes
  anim.rs, particles.rs, scenery.rs, input.rs, clock.rs, daylight.rs
                      ficam como estão, sem os testes

tests/
  support/            ScratchPack, gravador de traço, veículos e pisos sintéticos
  sim/                um binário: main.rs e um módulo por domínio, espelhando src/
  physics_traces.rs, ai_traces.rs, traffic_traces.rs, people_traces.rs
  physics_invariants.rs, automated_gearbox_script.rs, htmlengine_game.rs
```

Alvo de tamanho: nenhum arquivo de `src/` acima de 800 linhas (o padrão que a física já cumpre) e nenhuma função acima de 150, a não ser tabelas de despacho.

## Sequência de PRs

Cinco PRs, só de estrutura. Cada um tem um commit por assunto, para a revisão ler por partes, e por isso passa do limite de um dia útil das regras de trabalho em paralelo, como o F1 passou. Antes de cada um, o PR reserva a unidade na issue #61 e confere se há PR `fix/` aberto nos arquivos; se houver, espera ou deixa o arquivo de fora.

| Item | Branch | O que entra, em commits | Pronto quando |
| --- | --- | --- | --- |
| S1 | `feat/sim-limpeza` | (1) `cargo fmt -p omsi-sim` sozinho, e o crate na lista bloqueante do `fmt --check`; (2) as quatro funções sem uso removidas; (3) as cópias trocadas: um `angle_diff` em `omsi_geometry::util::angle`, `ParticleSet::rand` sobre `unit_f32`, `smooth01` como `smoothstep(0.0, 1.0, t)`, só onde a aritmética é a mesma | Traços idênticos; `fmt --check -p omsi-sim` verde |
| S2 | `feat/sim-testes` | (1) feature `test-support`, `omsi_sim::testing` e os setters só de teste atrás dela; (2) `tests/support/` com `ScratchPack`, gravador e veículos, e o script copiando a pasta; (3) por domínio, os testes de `src/` para `tests/sim/`, inclusive os cinco `tests.rs` de hoje; (4) `tests/traffic_traces.rs` (o `AiState` numa rede sintética: cruzamento com preferência, semáforo, troca de faixa, seguidor que freia, beco sem saída) e `tests/people_traces.rs` (os ossos de `Pose` e de `OmsiAnim` em andar, correr, parar, sentar e levantar), os dois no job `physics-traces` | Nenhum teste em `src/`; a mesma lista de testes, pelos nomes; traços idênticos; `cargo build -p omsi-sim` sem a feature não tem `testing` |
| S3 | `feat/sim-trafego` | `traffic/` (faixa, geometria, semáforos, `network/` e `driver/`), `AiState::drive` em etapas, imports do app | Traços da IA, `traffic_traces` e `offscreen-traces` idênticos |
| S4 | `feat/sim-pessoas` | `people/` (rig, pose, pele, animação do OMSI, multidão), `Pose::advance` e `Pose::bones` em etapas com nome, imports do app | `people_traces` idêntico; golden igual |
| S5 | `feat/sim-dominios` | `host/` com os callbacks por família; `automation/` com `StartUp::tick` e `Unit::learn` em etapas; `collision/`; `textures/`; `vehicle/coupled/`, com `VehicleInstance::new`, `VehicleType::load_with`, `TrailerPart::follow`, `step_rigid` e `step_physics` em etapas; imports do app; `crates/omsi-sim/ARCHITECTURE.md` com a árvore final | Traços da física e da IA idênticos; app compila com `--all-features` |

S1 vem primeiro porque o rustfmt deixa os PRs de movimento legíveis. S2 vem antes da divisão por domínio porque, com os testes fora de `src/`, mover um arquivo deixa de levar os testes junto, e porque os traços novos do tráfego e das pessoas, que hoje não têm nenhum, são a prova de S3 e S4. S3 a S5 não dependem entre si e podem trocar de ordem conforme o que estiver aberto no repositório. Se a revisão pedir, qualquer um deles se divide nos próprios commits.

Depois de S5, o crate entra na limpeza do clippy (A3), que não faz parte deste plano.

## Onde esta trilha encosta nas outras

- **Trilha 3.** C5 é só do renderer e dos shaders, e o fix-25 não tem o que fazer aqui (o `omsi-sim` não tem `unsafe`). Não há arquivo em comum; as duas podem correr juntas.
- **Trilha 4.** A Frente E move o tráfego e os humanos do **app** (`omsi-app/src/traffic.rs`, `humans.rs`) para um `omsi-gameplay`. Com esta trilha feita antes, o `omsi-gameplay` passa a depender de `omsi_sim::traffic` e `omsi_sim::people` já divididos. Os trechos da cena que a auditoria sugere levar para cá (faixas de `scene.rs` e a criação dos semáforos) entram na C4, não aqui. Os imports que S3 a S5 trocam no app são linhas de `use`; se houver PR aberto nesses arquivos, quem move porta a branch, como as regras pedem.
- **Física.** A trilha F terminou; S5 só move código da física do veículo e prova com os mesmos traços. A retirada de `model-fallbacks` continua esperando a validação no jogo.

## Decisões pendentes

| Pergunta | Recomendação | Alternativa |
| --- | --- | --- |
| Quando começa | Em paralelo à Trilha 3, já que não há arquivo em comum | Depois da Trilha 3 |
| Como os testes em `tests/` alcançam o que é interno | `omsi_sim::testing` atrás da feature `test-support` | Reescrever esses testes só pela API pública, o que muda o que eles verificam |
| Estado global (`EYE`, `WIND`, `AMBIENT`, `PLACE`, `LOCALE`, `BACKEND`) | Trocar por parâmetros junto da Frente B, quando o `Session::step` passar a ser dono desse contexto | Um PR próprio nesta trilha, depois de S5 |
| Nome do `VehicleHost` | Manter o nome e documentar que serve também ao cenário; renomear exigiria um apelido para os harnesses | `ScriptHost` com `pub use ScriptHost as VehicleHost` |

## Critério de sucesso

- Nenhum teste em `src/`: nenhum `#[cfg(test)]`, `#[test]` ou `tests.rs`.
- Nenhum pacote temporário, veículo sintético ou gravador de traço copiado.
- Nenhuma API só de teste compilada no jogo.
- Nenhum arquivo de `src/` acima de 800 linhas; nenhuma função acima de 150, a não ser tabelas de despacho.
- Traços da física, da IA, do tráfego e das pessoas idênticos desde S2; `offscreen-traces` e golden iguais em todos os PRs.
- `fmt --check` bloqueante no `omsi-sim`.
