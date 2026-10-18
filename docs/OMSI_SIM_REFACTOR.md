# Refatoração do `omsi-sim` por domínio (plano)

Plano de 18/10/2026 para o crate `omsi-sim`, medido no `main` em `7cc8474` (PR #303). Só leitura de código; contagens de linha e de função são aproximadas (casamento de chaves). É a **Trilha S** do [plano macro](MACRO_REFACTOR_DRAFT.md#trilha-s-omsi-sim-por-domínio) e segue as regras de lá: PRs curtos, um por vez, reservados na issue #61, branches `feat/` para estrutura e `fix/` para qualquer mudança de comportamento.

Três objetivos, pedidos pelo mantenedor:

1. **Separar testes e auxílios de teste do código funcional.**
2. **Quebrar o código por domínio e subdomínio**, seguindo SOLID e Clean Code.
3. **Tirar o código duplicado**, de produção e de teste.

## Regras herdadas

- **Refatoração não muda comportamento.** Todo PR deste plano prova isso: traços da física e da IA idênticos bit a bit (`scripts/physics-trace-diff.sh`), `offscreen-traces` e golden iguais, e a mesma lista de testes (`cargo test -p omsi-sim -- --list`, comparando os nomes sem o caminho do módulo). Qualquer diferença achada no caminho vira um PR `fix/` separado.
- **A física do tráfego IA só muda de lugar**, com testes que provam que ela se comporta igual no jogo. As compensações que só existem na IA (`ai_lift`, `ai_rest_offset`, `rotation_point`, o limite de ±1,2 rad) ficam como estão.
- **O jogo não compensa modelos mal configurados.** Nenhuma etapa deste plano acrescenta compensação; as que existem só na IA não são tocadas.
- **Nada se apaga.** Código sem uso fica no repositório; no máximo sai da compilação do jogo atrás de uma feature. A exceção já decidida é a feature `model-fallbacks`, que sai depois da validação da física no jogo, fora deste plano.
- **Os caminhos que os harnesses de traços usam não mudam.** O script de comparação compila `tests/physics_traces.rs` e `tests/ai_traces.rs` do PR também no commit base. Eles usam `omsi_sim::{ai, collision, physics, vehicle}` e as reexportações da raiz (`Controls`, `KinematicModel`, `SimClock`, `VehicleHost`, `VehicleInstance`, `VehicleType`); esses caminhos continuam valendo do começo ao fim.

## Diagnóstico

O crate tem 82 arquivos `.rs` e 28.439 linhas em `src/`, além de 2.610 em `tests/` e 802 em `examples/`. A física (`physics/`, com `domain/`, `interface.rs` e `implementation/`) e o veículo (`vehicle/`, 15 submódulos desde o #128) já estão divididos e servem de modelo. O resto ainda está em arquivos planos na raiz.

### Testes misturados com o código

- **7.314 das 28.439 linhas de `src/` (26%) são testes.** Só cinco arquivos de teste já são separados (`physics/tests.rs`, `physics/implementation/train/tests.rs`, `vehicle/tests.rs`, `ai/tests.rs` e `startup/tests.rs`). Os outros 26 arquivos têm módulos `#[cfg(test)]` dentro do código.
- Os maiores: `traffic.rs` (791 linhas de teste em quatro módulos: `extend_tests`, `tests`, `light_tests`, `aurora_pool_tests`), `human.rs` (522), `input.rs` (377, 60% do arquivo), `collision.rs` (233 em quatro módulos), `physics/implementation/collide.rs` (219, 63%), `host.rs` (218), `vehicle/articulation.rs` (154).
- Em `human.rs` o módulo de testes fica no meio do arquivo (linhas 2222 a 2749) e o código funcional continua depois dele (`curl_hands`, `grip_centres`, `hand_slot`, `skin_from`).
- `startup.rs` tem dois módulos de teste: um dentro do arquivo (linhas 88 a 213) e `regressions`, que aponta para `startup/tests.rs` com `#[path]`.
- O job `physics-coverage` já ignora arquivos chamados `tests.rs`; com os testes dentro de `collide.rs`, `ground.rs` e `body.rs`, as linhas de teste entram na conta da cobertura da física.

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

`TrafficLightController::phase_index`, `PathGraph::distances_from`, `StartUp::nothing_to_do` e `htmltex::set_backend` não têm chamador. Pela regra do projeto, ficam.

### Formatação

O `omsi-sim` é um dos três crates ainda fora do `fmt --check` bloqueante: `cargo fmt --check -p omsi-sim` acusa 1.235 trechos.

## Princípios, aplicados a este crate

- **Uma responsabilidade por arquivo (SRP).** Um arquivo fala de um assunto e tem o nome dele. Função longa vira etapas com nome, na mesma ordem, sem mudar a lógica.
- **Aberto para extensão (OCP).** Callbacks do host e famílias de dados entram por módulo de família, como o `handle_keyword` do `omsi-model` (D6, passo 8): cada família devolve "não é meu" e o despachante tenta a próxima. Um callback novo mexe num arquivo só.
- **Interfaces pequenas (ISP).** Cada domínio grande ganha um `mod.rs` que reexporta só o que o resto do jogo usa, como o `physics/interface.rs`. O que é interno fica `pub(crate)` ou `pub(super)`.
- **Dependências explícitas (DIP).** O estado global vira parâmetro, mas só junto da Frente B (ver [Decisões pendentes](#decisões-pendentes)).
- **Sem abstração especulativa.** Trait só onde houver duas implementações de verdade. Hoje nenhum domínio deste plano tem; a divisão é por módulos e funções.
- **Clean Code:** nomes que dizem o domínio, sem comentário que conta a história de uma mudança, testes descritos pelo comportamento (o padrão que o crate já segue).

## Convenções novas

1. **Testes unitários num arquivo irmão.** Todo módulo com testes declara `#[cfg(test)] mod tests;` e os testes ficam em `<módulo>/tests.rs` (ou `<módulo>/tests/` com um arquivo por assunto, quando passam de uns 400 linhas). Nenhum `mod … { #[test] … }` dentro de arquivo funcional, nenhum `#[path]`.
2. **Auxílios de teste num lugar só, por lado:**
   - `src/testing/` (`#[cfg(test)] pub(crate) mod testing`): `ScratchPack`, que monta um pacote de veículo ou objeto numa pasta temporária com nome único e a apaga no `Drop`, e os veículos e pisos sintéticos que os testes unitários usam.
   - `tests/support/` (incluído por `mod support;` em cada teste de integração): o gravador de traço e os veículos sintéticos dos harnesses. Ele usa só API pública que já existe no `main`, porque o script de comparação também o copia para o commit base.
3. **API só de teste atrás da feature `test-support`.** Os setters e acessores que só testes e exemplos usam ficam num `impl` próprio com `#[cfg(any(test, feature = "test-support"))]`. O `Cargo.toml` liga a feature para os testes de integração e os exemplos com uma dev-dependency do próprio crate (`omsi-sim = { path = ".", features = ["test-support"] }`). O jogo compila sem ela. No commit base os harnesses continuam compilando, porque lá esses métodos ainda não têm a condição.
4. **Caminhos antigos como reexportação.** Cada movimento deixa `pub use` no lugar antigo (`pub use people::human;` no `lib.rs`), para o `omsi-app` não mudar no mesmo PR. Um PR no fim troca os imports do app e tira as reexportações, menos as que os harnesses usam.

## Arquitetura alvo

```
src/
  lib.rs              reexportações da raiz (as de hoje)
  testing/            só em teste: ScratchPack, veículos e pisos sintéticos
  physics/            como está (domain/, interface.rs, implementation/)
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
    mod.rs            reexporta human, human_omsi e crowd nos caminhos de hoje
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
  anim.rs, particles.rs, scenery.rs, input.rs, clock.rs, daylight.rs
                      ficam como estão, só com os testes fora
```

Alvo de tamanho: nenhum arquivo funcional acima de 800 linhas (o padrão que a física já cumpre) e nenhuma função acima de 150, a não ser tabelas de despacho.

## Sequência de PRs

Cada PR é só estrutura, a não ser onde a tabela diz "testes". Antes de cada um, o PR reserva a unidade na issue #61 e confere se há PR `fix/` aberto nos arquivos; se houver, espera ou deixa o arquivo de fora.

| Item | Branch | O que entra | Pronto quando |
| --- | --- | --- | --- |
| S0 | `feat/sim-fmt` | `cargo fmt -p omsi-sim`, sem outra mudança, e o crate na lista bloqueante do `fmt --check` no CI | Traços idênticos; `fmt --check -p omsi-sim` verde |
| S1a | `feat/sim-testes-fisica-veiculo` | Testes inline de `physics/`, `vehicle/`, `ai/`, `anim`, `clock`, `daylight`, `particles`, `scenery`, `scripttex`, `texttex`, `input` e `vehicle_api` para arquivos `tests.rs` irmãos | Mesma lista de testes; traços idênticos; a cobertura da física passa a contar só código |
| S1b | `feat/sim-testes-dominios` | O mesmo em `traffic` (os quatro módulos viram `traffic/tests/{extend,network,lights,pools}.rs`), `human`, `human_omsi`, `crowd`, `collision`, `host`, `ibis` e `startup` (os dois módulos num só, sem `#[path]`) | Mesma lista de testes; nenhum `#[cfg(test)] mod … {` em arquivo funcional |
| S2 | `feat/sim-apoio-testes` | `src/testing/` com `ScratchPack` no lugar dos 12 pacotes temporários; `tests/support/` com o gravador de traço e os veículos sintéticos (GN92 de três cópias para uma por lado); `scripts/physics-trace-diff.sh` copia `tests/support/` junto com os harnesses | Traços idênticos byte a byte; nenhuma pasta temporária sobra depois de `cargo test` |
| S3 | `feat/sim-api-de-teste` | Feature `test-support` com os setters e acessores só de teste; `sit_amount`, `mesh_dropped` e o import de `scripts_acceleration` para dentro dos testes | `cargo build -p omsi-sim` sem a feature não tem esses métodos; app compila com `--all-features`; traços idênticos |
| S4 | `feat/sim-tracos-trafego-pessoas` | Testes: `tests/traffic_traces.rs` (o `AiState` numa rede sintética: cruzamento com preferência, semáforo, troca de faixa, seguidor que freia, beco sem saída) e `tests/people_traces.rs` (matrizes dos ossos de `Pose` e de `OmsiAnim` em andar, correr, parar, sentar e levantar), os dois no script de comparação | Os traços novos rodam no job `physics-traces` |
| S5 | `feat/sim-duplicados` | Um `angle_diff` (em `omsi_geometry::util::angle`, junto do `wrap_deg`); `ParticleSet::rand` sobre `unit_f32`; `smooth01` vira `smoothstep(0.0, 1.0, t)`. Só trocas com a mesma aritmética | Traços, inclusive os de S4, idênticos |
| S6a | `feat/sim-trafego-faixas` | `traffic/` como pasta: `lane.rs`, `geometry.rs`, `lights.rs` | Traços e `offscreen-traces` idênticos |
| S6b | `feat/sim-trafego-rede` | `traffic/network/` | Idem |
| S6c | `feat/sim-trafego-motorista` | `traffic/driver/`, e `AiState::drive` (126 linhas) em etapas | Idem; `traffic_traces` idêntico |
| S7a | `feat/sim-pessoas` | `people/` só movendo: `rig/`, `pose/`, `skin.rs`, `omsi_anim.rs`, `crowd/` | `people_traces` idêntico |
| S7b | `feat/sim-pessoas-pose` | `Pose::advance` e `Pose::bones` em etapas com nome (pés, quadril, tronco, braços, mãos, olhar) | `people_traces` idêntico |
| S8 | `feat/sim-host-callbacks` | `host/` com os callbacks por família, como o D6 passo 8 | Traços idênticos; os testes do host verdes |
| S9 | `feat/sim-automacao` | `automation/startup` e `automation/ibis`, com `StartUp::tick` e `Unit::learn` em etapas | Testes de regressão de partida e do IBIS iguais |
| S10 | `feat/sim-colisao` | `collision/` em submódulos | Traços idênticos (os cenários de batida usam `CollisionWorld`) |
| S11 | `feat/sim-texturas` | `textures/` (script, texto, HTML e a API da página) | `tests/htmlengine_game.rs` e golden iguais |
| S12 | `feat/sim-veiculo-acoplados` | `vehicle/coupled/` no lugar de `trailer.rs` e `articulation.rs`; `VehicleInstance::new`, `VehicleType::load_with`, `TrailerPart::follow`, `step_rigid` e `step_physics` em etapas | Traços da física e da IA idênticos |
| S13 | `feat/sim-imports-app` | Imports do `omsi-app` e das ferramentas nos caminhos novos; saem as reexportações, menos as que os harnesses usam; `crates/omsi-sim/ARCHITECTURE.md` com a árvore final | App compila com `--all-features`; traços idênticos |

S0 vem antes de tudo porque deixa os PRs de movimento legíveis e não conflita com nada aberto hoje. S1 a S3 cumprem o primeiro objetivo; S4 é a rede de segurança dos domínios que ainda não têm traço; S5 cumpre o terceiro; S6 a S13 cumprem o segundo. Os itens S6 a S12 não dependem entre si e podem trocar de ordem conforme o que estiver aberto no repositório; S6 primeiro porque `traffic.rs` é o maior arquivo do crate.

Depois de S13, o crate entra na limpeza do clippy (A3), que não faz parte deste plano.

## Onde esta trilha encosta nas outras

- **Trilha 3.** C5 é só do renderer e dos shaders, e o fix-25 não tem o que fazer aqui (o `omsi-sim` não tem `unsafe`). Não há arquivo em comum; as duas podem correr juntas.
- **Trilha 4.** A Frente E move o tráfego e os humanos do **app** (`omsi-app/src/traffic.rs`, `humans.rs`) para um `omsi-gameplay`. Com esta trilha feita antes, o `omsi-gameplay` passa a depender de `omsi_sim::traffic` e `omsi_sim::people` já divididos. Os trechos da cena que a auditoria sugere levar para cá (faixas de `scene.rs` e a criação dos semáforos) entram na C4, não aqui.
- **Física.** A trilha F terminou; S12 só move código da física do veículo e prova com os mesmos traços. A retirada de `model-fallbacks` continua esperando a validação no jogo.

## Decisões pendentes

| Pergunta | Recomendação | Alternativa |
| --- | --- | --- |
| Quando começa | Em paralelo à Trilha 3, já que não há arquivo em comum | Depois da Trilha 3 |
| API só de teste | Feature `test-support` ligada pelas dev-dependencies | Deixar pública, num `impl` separado com documentação |
| Estado global (`EYE`, `WIND`, `AMBIENT`, `PLACE`, `LOCALE`, `BACKEND`) | Trocar por parâmetros junto da Frente B, quando o `Session::step` passar a ser dono desse contexto | Um PR próprio nesta trilha, depois de S13 |
| Nome do `VehicleHost` | Manter o nome e documentar que serve também ao cenário; renomear exigiria um apelido para os harnesses | `ScriptHost` com `pub use ScriptHost as VehicleHost` |
| As quatro funções sem uso | Ficam como estão | Atrás de uma feature desligada |

## Critério de sucesso

- Nenhum módulo de teste dentro de arquivo funcional, nenhum `#[path]` de teste.
- Nenhum pacote temporário, veículo sintético ou gravador de traço copiado.
- Nenhuma API só de teste compilada no jogo.
- Nenhum arquivo funcional acima de 800 linhas; nenhuma função acima de 150, a não ser tabelas de despacho.
- Traços da física, da IA, do tráfego e das pessoas idênticos desde S4; `offscreen-traces` e golden iguais em todos os PRs.
- `fmt --check` bloqueante no `omsi-sim`.
