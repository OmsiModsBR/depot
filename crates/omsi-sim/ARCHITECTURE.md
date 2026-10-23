# omsi-sim: arquitetura

O crate simula o que se move no jogo: veículos, pessoas, tráfego, o terreno de colisão e os
textos e imagens que os scripts desenham. Este arquivo descreve a divisão por domínio depois da
trilha de refatoração (`docs/OMSI_SIM_REFACTOR.md`). Cada domínio é um diretório; `mod.rs` diz o
que o domínio expõe, e os arquivos dentro dele separam as etapas de um procedimento.

## Regras de organização

- **Código de teste fora de `src/`.** Os testes ficam em `tests/`, e o que um teste precisa do
  interior do crate chega por `omsi_sim::testing`, que só existe com a feature `test-support`.
- **Um domínio por diretório.** Um arquivo novo entra no domínio que ele serve, não no que o
  chamou.
- **Módulo como fachada.** `mod.rs` reexporta com `pub use` o que os outros crates usam. Os
  caminhos públicos (`omsi_sim::{ai, collision, physics, vehicle, traffic, people, ...}`) não
  mudam quando um arquivo é dividido.
- **Visibilidade mínima.** Item compartilhado entre irmãos é `pub(super)`; entre subárvores,
  `pub(crate)` ou `pub(in caminho)`.
- **Limites de tamanho.** Nenhum arquivo acima de 800 linhas; nenhuma função acima de 150 linhas,
  salvo tabelas de despacho (como `Host::callback` e os `cb_*` de `host/callbacks/`).
- **Traços como prova.** `scripts/physics-trace-diff.sh` compara os traços da física, da IA,
  do tráfego e das pessoas com a base. Uma mudança de estrutura não pode alterar nenhum deles.

## Árvore

```
src/
  lib.rs              raiz: reexporta os caminhos públicos
  clock.rs            tempo da simulação: data, hora do dia, Timegap
  daylight.rs         posição do sol e as três luzes de envir.cfg
  input.rs            ações de entrada (Inputs/keyboard.cfg)
  anim.rs             avaliação de [newanim]: variáveis viram transformações de malha
  particles.rs        sistemas de partículas ([smoke], [particle_emitter])
  scenery.rs          objetos de cenário animados por script
  testing.rs          o que os testes em tests/ alcançam (feature test-support)

  host/               o host de script do veículo: variáveis de sistema, callbacks, sons
    mod.rs            VehicleHost e o despacho de callback
    callbacks/        um arquivo por família: fonts, script_tex, depot, timetable,
                      passengers, scenery

  textures/           texturas que scripts e páginas desenham
    htmltex.rs        páginas HTML como textura
    texttex.rs        [texttexture]: texto gerado com fontes .oft
    scripttex.rs      [scripttexture]: imagens RGBA desenhadas pelos callbacks ST*

  collision/          mundo de colisão
    world.rs          as caixas da cena e as consultas sobre elas
    obb.rs            caixas orientadas: teste, contato, altura do impacto
    box3.rs           caixa alinhada ao plano do chão
    mesh.rs           obstáculos a partir de malhas (triângulos em caixas, grade)
    polygon.rs        recorte de polígonos convexos

  automation/         o motorista que a própria simulação faz
    startup/          colocar o ônibus em serviço (Shift+U, --autostart)
      probe.rs        o que os scripts dizem: chaves ligadas, motor rodando
      displays.rs     mostradores apagados e botões que os acendem
    ibis/             digitar um serviço no IBIS como um motorista faz
      unit.rs         o que o IBIS é, aprendido dos scripts compilados
      program.rs      leitura dos scripts: que variáveis cada gatilho lê e grava
      sim.rs          cópia do estado dos scripts sobre a qual o planejador pressiona teclas
      typist.rs       digitação de um serviço, tecla a tecla
      plan.rs         busca da sequência de teclas que chega ao serviço

  vehicle/            veículo rodoviário
    vehicle_type.rs   o tipo: definição, scripts compilados, modelo, malhas
    vehicle_type/
      paint_schemes.rs  esquemas de pintura dos arquivos .cti
    winding.rs        orientação dos triângulos, julgada pacote a pacote
    builtins.rs       variáveis que todo veículo tem antes de declarar as suas
    scripts.rs        o lado dos scripts: variáveis, gatilhos, texto, pintura, revisão
    motion.rs         controles do motorista, passo da física e do corpo rígido
    physics_bridge.rs a interface de scripts para qualquer forma de movimento
    ai_frame.rs       movimento recebido do tráfego a cada quadro
    pose.rs           posição do corpo no mundo e câmeras
    visuals.rs        o que o veículo mostra a cada quadro
    props.rs          propriedades de exibição por malha ([visible], mapas de luz, alfa)
    skin.rs           malhas com ossos: os ossos que as animações movem
    radio.rs          o texto do rádio no painel
    weather.rs        efeito do clima: umidade do ar e aderência da pista
    coupled/          partes acopladas atrás do veículo
      articulation.rs seções traseiras de ônibus articulado
      trailer.rs      reboques de caminhão
      mod.rs          VehicleInstance::new e a ordem de atualização das partes

  vehicle_api.rs      o veículo como a página o vê (window.omsi.vehicle)
  vehicle_api/
    snapshot.rs       o instantâneo: movimento, motor, portas, luzes, freios, condição

  physics/            física do veículo, em duas camadas
    domain/           o que o resto do jogo entrega à física e lê dela
                      (corpo, roda, solo, impacto, parâmetros, especificação, trem)
    implementation/   os procedimentos, um módulo por domínio na ordem de um passo:
                      spec, body, ground, suspension, wheel, tyre, collide, kinematic, train
    interface.rs      a única entrada usada pelo resto do jogo

  traffic/            rede de faixas e veículos de IA rodoviários
    lane.rs           uma faixa: tipo, identidade no mapa, geometria
    geometry.rs       faixas a partir de arcos, curvas e polilinhas
    lights.rs         semáforos: programa de fases e aspecto de cada luz
    network/          a rede inteira
      topology.rs     como as faixas se unem
      spatial.rs      busca de faixas por lugar (grade de 50 m)
      routing.rs      menores caminhos entre faixas
      conflicts.rs    cruzamentos e quem dá a preferência
    driver/           o motorista de IA
      plan.rs         escolha de rota no fim de uma faixa
      drive.rs        para onde a direção aponta e o indicador
      lane_change.rs  troca de faixa
      speed.rs        velocidade desejada: perfil nas curvas e IDM
    mod.rs            o tráfego: reexportações e os motoristas de IA

  ai/                 como o corpo de uma IA segue o caminho do planejador
    road.rs           veículo rodoviário: modelo de bicicleta e suspensão
    rail.rs           veículo sobre trilhos
    air.rs            aeronave na trajetória
    planner.rs        o que o planejador pede ao corpo antes de andar
    mod.rs            o corpo seguindo o caminho

  people/             pessoas a pé e a bordo
    mod.rs            humanos (.hum): modelos com ossos posados por procedimento
    rig/              o esqueleto: ossos do motor, slots, articulações [links]
      load.rs         carga do tipo de humano a partir do .hum
    pose/             a pose de uma pessoa a cada quadro
      advance.rs      andar, sentar, pagar e segurar, um quadro por vez
      bones.rs        transformações dos ossos da pose
      gait.rs         passada: quanto de cada passo o pé está no chão
      math.rs         ajudantes de vetor, rotação e IK de dois ossos
    skin.rs           pele: malha deformada pelos ossos, mãos no apoio
    omsi_anim.rs      a animação das pessoas do OMSI
    crowd/            multidão
      steering.rs     caminhantes, evitação antecipada, um passo da multidão
      corridor.rs     manter o caminhante no corredor
      paths.rs        rotas pela rede de caminhos da cabine (paths.cfg)
```

## Testes

Os testes que cobrem o crate ficam em `tests/`, com um módulo por domínio em `tests/sim/`,
espelhando `src/`. Os traços (`physics_traces`, `ai_traces`, `traffic_traces`, `people_traces`)
são os testes de regressão de comportamento; `htmlengine_game.rs` cobre a página HTML com o resto
do jogo (instantâneo do veículo, relógio e folhas de estilo).

## Como verificar uma mudança de estrutura

```
cargo test -p omsi-sim
bash scripts/physics-trace-diff.sh origin/main      # todos os traços idênticos
cargo check -p omsi-sim --all-targets --features test-support
cargo check --workspace --all-targets --all-features  # sem avisos do omsi-sim
cargo fmt -p omsi-sim -- --check
```
