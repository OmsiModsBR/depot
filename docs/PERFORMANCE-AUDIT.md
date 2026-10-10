# Auditoria de performance: oportunidades de ganho

Escopo: ganhos reais de FPS, tempo de frame, hitching, tempo de carregamento e memória.
Falhas, segurança e correção ficam de fora; já foram auditadas.

## Como ler este documento

- **Método:** leitura estática do código, em quatro frentes (renderer, loop de frame e streaming,
  simulação/IA/scripts, carregamento de assets e build). Nada foi compilado, executado ou medido.
- **Os ganhos são estimativas** a partir do formato do código, não medições. Itens marcados
  *(especulativo)* não foram verificados nem na leitura. Antes de implementar qualquer item,
  meça com `OMSI_PROFILE` (veja `docs/performance-th-wald.md` para o método usado no #284).
- **Já feito e não repetido aqui:** batching de splines, cache de esferas de instância, encoders
  paralelos, culling por blocos/LOD/fog, shadows em cascata com redesenho alternado, PCF com
  early-out, AO em meia resolução, BC1/BC3 + mips na GPU, VM de scripts em bytecode, grid de
  lanes de 50 m, LOD/paralelismo dos humanos, carros dormentes, `hashbrown` na simulação.
- **Esforço/risco:** B = baixo, M = médio, A = alto.

## Resumo: os 12 maiores ganhos

| # | Item | Onde | Ganho esperado | Esforço/risco |
|---|------|------|----------------|---------------|
| 1 | Criar recursos de GPU do streaming de tiles fora da thread principal | `scene.rs` `upload_step` | Alto (hitching) | M-A / M |
| 2 | Eliminar um dos 3 passes de geometria no Enhanced + MSAA | `omsi-render` prepasses | Alto (GPU) | M / M |
| 3 | Cache em disco de texturas GPU-ready (BC já codificado) | `omsi-texture/src/gpu.rs` | Alto (carga) | M / B |
| 4 | Não reconstruir listas/colisão do mundo todo a cada tile | `scene.rs` `refresh_tile_lists` | Médio-alto (hitching) | M / B-M |
| 5 | `body_in_way` O(carros × footprints) | `traffic.rs:4942` | Médio-alto (CPU serial) | M / M |
| 6 | Sombras dos postes redesenhadas todo frame | `omsi-render` ~10112 | Médio-alto à noite | B |
| 7 | Cascata de sombra próxima redesenha tudo | `omsi-render` ~8897 | Médio-alto (GPU) | A |
| 8 | Perfil de release: overflow-checks por pacote, perfil `dist` | `Cargo.toml` | 3-12% | trivial |
| 9 | Substeps mínimos de 4 na física do ônibus em FPS alto | `rigid/train.rs:415` | Médio-alto a 150+ fps | M-A / M |
| 10 | Allocator (mimalloc) no `omsi-app` | `omsi-app` | Médio (carga, Windows) | B |
| 11 | `tree_sway` recalculado por vértice em até 6 passes | `shader.wgsl:562` | Médio (florestas) | B-M |
| 12 | `update_scripted`: parse/lookup por nome por frame | `scene.rs:9719` | Médio (mapas grandes) | M |

---

## 1. Renderer (`crates/omsi-render`)

### 1.1 Três passes de geometria para o mesmo conjunto visível (Enhanced + MSAA) — alto
`lib.rs` ~10140-10180 (prepass single-sample para SSAO) e ~10338-10390 (prepass MSAA) desenham os
mesmos batches, mais o passe de cor. O `vs_main` completo (sway, specular, sphere map) roda duas
vezes antes de qualquer cor.
**Correção:** com MSAA ligado, desenhar só o prepass MSAA e resolver a profundidade com um
shader pequeno para a textura que o SSAO lê; remover o prepass single-sample. Manter a
invariância de profundidade (`shader.wgsl:463`).
**Ganho:** 1/3 a 1/2 do custo de prepass e do encoding associado. **Lacuna relacionada:** o prepass
MSAA é só `enhanced` (~10345); o modo vanilla com MSAA em GPUs não-Apple paga overdraw completo.

### 1.2 Sombras de postes limpas e redesenhadas todo frame — médio-alto à noite, esforço baixo
`lib.rs` ~10112-10135: até 4 tiles (`LAMP_SHADOWS`, 472) fazem clear + redesenho por frame, e a CPU
reconstrói os casters (`casters()` ~9150). Poste e entorno são quase estáticos.
**Correção:** cache por tile; só redesenhar quando posição do poste (`lamp_shadow_last`),
`render_origin` ou casters dinâmicos mudarem, ou a cada 2-4 frames. É o mesmo padrão já usado nas
cascatas near/far (~8930).

### 1.3 Cascata próxima redesenha estáticos e dinâmicos juntos — médio-alto, esforço alto
`lib.rs` ~8897-8925. O comentário em ~8890 diz que é o passe de sombra mais caro (1/3 só das
folhas de árvore). Ainda por cima a cascata "close" é desenhada todo frame no mesmo alvo, forçando
load/store da metade near (`keep_near`, ~10093).
**Opção A (barata):** separar a cascata close em textura/passe próprio.
**Opção B:** mapa de casters estáticos refeito só quando sol/origem/janela mudam + só dinâmicos
(veículos, pessoas) por cima.
**Extra:** LOD mais leve ou mip bias no alpha-test das folhas no passe de sombra.

### 1.4 `tree_sway` recalculado por vértice — médio em florestas, esforço baixo-médio
`shader.wgsl:562-615`, chamado de `vs_main` e de todos os `vs_shadow*`: até 6 passes por frame.
Pé da árvore, `world_pattern_xy`, `hash_cell`, três `sin`, dois `pow(…,1.5)` dependem só da
instância. **Correção:** pré-calcular por instância (CPU ou compute pequeno) em `inst_params`;
deixar no VS só o termo por vértice (`h*h`, seno dos galhos).

### 1.5 Vertex shader largo em passes só de profundidade — baixo-médio, esforço baixo
`shader.wgsl:460-475`: `VsOut` tem ~24 floats, incluindo `params`/`params2` (8 floats constantes
por instância, sem `@interpolate(flat)`). Prepass e `vs_shadow*` reusam a estrutura. Specular,
sphere map e normal podem rodar à toa *(especulativo: depende do driver eliminar saídas mortas)*.
**Correção:** `vs_prepass` enxuto, varyings por instância `flat` (ou lidos de `inst_params` no
fragment), `fragment: None` nos pipelines de profundidade opacos. Mais relevante em GPUs tilers.

### 1.6 Texturas PBR sempre RGBA8 sem compressão — médio em memória/banda (Enhanced)
`add_pbr_maps` (~5164-5176) passa normal e ORM por `Rgba8UnormSrgb` após um passe de LUT sRGB por
byte na CPU. 4 B/texel contra 0,5-1 B com BC. 3 dos 9 slots do bind group são PBR.
**Correção:** `Rgba8Unorm` + BC5 (normal) / BC7 ou BC1 (ORM) — exige encoder em `omsi_texture::bc`.
**Mobile:** não há caminho ASTC/ETC2; Android/iOS caem em RGBA8 não comprimido. ASTC 6x6/8x8
cortaria 4-8x de memória/banda lá.

### 1.7 Pequenos
- **Store de depth MSAA** (~10546): `msaa_prepass || ao_on` força write-back que talvez ninguém leia
  (a chuva usa a depth single-sample). Verificar glass/puddle antes. Ganho de banda, sobretudo Apple.
- **SipHash no sort de blended:** `nearest_by_origin` (~12638, `HashMap<[u64;3], f32>` std) e
  `holders.contains()` linear (~9755/9795). Trocar por hashbrown+hasher rápido ou sort por chave.
- **Bookkeeping de culling:** `sizes: Vec<([u64;4], f32)>` reinserido em `object_sizes` por
  instância por frame (~9500-9540); `cull_blocks()` aloca Vec e roda 2x/frame; cada passe cria
  `Vec<DrawItem>/Vec<Batch>`. Reaproveitar buffers de scratch no `Renderer`.
- **Formato de vértice:** `Vertex` 32 B e índices sempre `u32`. Normais snorm/oct (24 B) e `u16`
  para malhas < 65 536 vértices: ~25% e ~50% menos fetch/VRAM. Só ajuda onde há gargalo de banda.
- **Sombra com `cull_mode: None`** (~3156): front-face culling em malhas fechadas opacas reduz
  raster; muda o bias, exige checagem visual.
- **PCF:** até 16 `textureSampleCompareLevel`; `textureGatherCompare` dá 4 taps por fetch.
- **`omsi_cfg::env::var_os` ~25x por frame** em `render_inner`; cachear uma vez.
- **Luzes pontuais:** loop de até `CELL_CAP`=32 luzes por célula 2D por fragmento
  (`shader.wgsl:1100`); grade clusterizada ajudaria cidades à noite.
- **Espelhos:** cada espelho é um `render_inner` completo (cull + batches + bundles); compartilhar a
  lista de batches entre espelhos do mesmo frame não foi examinado.

### 1.8 Longo prazo: desenho GPU-driven / oclusão — alto (CPU), esforço muito alto
Só existe `draw_indexed` (~12821). Culling, sort, gravação de bundles e re-encode por passe
(prepass, prepass MSAA, 3 cascatas, main) continuam O(draws) por frame. Caminho: compute culling +
`multi_draw_indexed_indirect` + Hi-Z do frame anterior; primeiro passo mais barato é reaproveitar a
lista de batches entre prepass/prepass MSAA/main e cachear bundles de cascatas não redesenhadas.
Risco alto (fallback GL não tem indirect).

---

## 2. Loop de frame e streaming (`crates/omsi-app`)

### 2.1 Recursos de GPU criados na thread principal durante o streaming — alto (hitching)
`scene.rs:6665-6735` (`upload_step`), `6764+` (`place_step`), chamado de `tiles.rs:1041`. A thread
loader só prepara dados de CPU; a principal faz `add_texture_data`/`add_mesh`/`add_instance` (cada
um `create_*` + `write_*`) com `gpu.lock()` preso. O orçamento de 6 ms só é checado *entre* itens
(`scene.rs:6728`), então uma textura 2048² estoura.
**Prova de viabilidade:** `omsi_render::prepare_texture`/`prepare_meshes` (~5118) já criam recursos
em worker, usados em `scene.rs:11621-11666`, `12137`, `12248`, `12293` — mas só para veículos.
**Correção:** usá-los em `prepare_tiles_impl`/`cut_terrain` (malha de terreno, texturas de corte e
pintura, novos tipos de objeto); a principal só faz `add_prepared_*`. Manter o fallback GL de
`scene.rs:11603`.

### 2.2 Listas do mundo inteiro reconstruídas a cada tile — médio-alto (hitching)
`scene.rs:8440-8490` (`refresh_tile_lists`), chamado de `tiles.rs:1116` a cada tile carregado ou
descarregado; depois `app.rs:1050-1075`. Reconstrói `CollisionWorld`, caixas estacionadas, paradas,
luzes, coronas etc. de TODOS os tiles carregados, com `.cloned()` em structs com `String`.
`CollisionWorld::add` usa `HashMap<(i32,i32), Vec<usize>>` com SipHash (`omsi-sim/src/collision.rs:512-544`).
**Correção:** `Arc<CollisionTile>` por tile construído no loader, consulta no 3x3; trocar hasher;
chamar `refresh_tile_lists` uma vez quando a fila esvazia, não por tile.

### 2.3 Pipeline do streamer serial e orçamento fixo — médio, esforço baixo-médio
`tiles.rs:1187-1225`, `993+`: um único lote em voo (3 tiles) que termina com o tile mais lento; uma
thread de SO nova por lote (`tiles.rs:1205`); o próximo lote só começa depois do loop de upload; o
orçamento é 6 ms fixos (`app.rs:1043`). `missing()` roda todo frame e aloca `Vec`+`HashSet`
(`tiles.rs:976-987, 1095`).
**Correção:** iniciar o próximo lote *antes* do upload; worker persistente com canal, ou 2 lotes em
voo; orçamento adaptativo `min(6 ms, frame_alvo − trabalho_do_frame)`; `missing()` guiado por
evento (célula da câmera mudou / fila vazia).

### 2.4 Sem caches persistentes — alto no tempo de carga, esforço alto
- **Índice do mapa:** `MapIndex::build` (`tiles.rs:165`, via `scene.rs:3754`) parseia todos os tiles
  no início (345 mil objetos em Ahlheim) e `stage_tile` (`scene.rs:4102`) parseia de novo.
  Serializar o índice e o registro parseado por tile em `~/.openomsi/cache`, chave mtime+tamanho.
- **Compressão de textura em runtime:** ver §4.1 (mesma causa; também refeita a cada re-stream).
- **`trim_staged`** (`scene.rs:4075`) descarta dados assim que nenhum tile os usa; voltar a um tile
  reparseia terreno e splines. LRU pequeno (tiles em `load_radius`+2 ou N MB).
- **Decodificação dupla:** textura decodificada como RGBA rápido e lida/decodificada de novo para o
  upgrade comprimido (`load_texture_key`, `scene.rs:8747`). Passar a imagem já decodificada ao job.

### 2.5 `edit_objects` para todo objeto colocado — médio em memória, esforço baixo
`scene.rs:5437` (`editable` verdadeiro para todo objeto de mapa), `7820-7827`, `HashMap<i64,
EditObject>` (`2192`) com `Vec<usize>` e `PathBuf` clonado por entrada, mesmo sem abrir o editor; cada
unload faz `retain` em tudo (`6623`, `8287`; idem `parked_objects`/`departed_objects`).
**Correção:** criar entradas sob demanda ao abrir o editor, ou por tile em `TileState`; `Arc<Path>`.

### 2.6 `update_scripted` por frame — médio em mapas grandes, esforço médio
`scene.rs:9719-10010`, chamado de `app_events.rs:2176`:
- loop de `variants` (~9885): `parse::<f32>()` + `o.inst.var(var)` (lowercase + hash) por variante de
  cada objeto a ≤800 m, todo frame. Resolver a `VarId` uma vez na colocação.
- displays de partida reconstroem `host.arrivals` todo frame (~9780), clonando 2 `String` por
  chegada. Contador de geração em `boards.by_stop`.
- objetos com `dynamic_textures`: montam Vec + HashMap, clonam `o.instances` e pegam o lock `gpu`
  (~9960) todo frame. Cachear a seleção anterior.
- `phase_of` chamado 2x por objeto; scripts de objetos a >~200 m poderiam rodar a 15-30 Hz com dt
  escalado.

### 2.7 Menores
- `update_night_modes` (`app_events.rs:1978`, `scene.rs:9435`): a cada 60 *frames* (não tempo), com
  `vec![v; m.slots]` por modo e `set_slot_night` mesmo sem mudança. Disparar por mudança de minuto/
  tiles/limiar e só escrever quando o estado virar.
- `update_texture_budget` (`scene.rs:8833`): passada completa 1x/s sob locks. Manter total corrente.
- `app_events.rs:690`: `ctl.disabled = ctrl_off.split('|').collect()` aloca todo frame; cachear.
- `lights.rs:483-502`: varre todos os `light_objects`/`particle_objects` com `length()`; usar
  distância ao quadrado ou filtro por tile.
- `set_signals` (`scene.rs:9488`, mapas ferroviários): 2 HashMaps + `var("Signal")` por nome, todo
  frame; `set_switches` (`9649`) com `find` linear. Índice por id.
- `env::var_os` por frame em `lights::collect`, `OMSI_NO_SPRAY`, `OMSI_WATCH_VARS` etc.: `OnceLock<bool>`.
- Contenção: `wanted_textures` (`scene.rs:5984-5998`) pega `gpu.lock()` no loader enquanto a principal
  o segura no upload; o item 2.1 resolve a maior parte.
- Rede de tráfego só cresce: `traffic.rs:7442` (`add_tiles`) estende `net.lanes`/`lane_tiles` sem
  remoção vista no unload *(especulativo; confirmar se há poda em outro lugar)*.
- Limitador de FPS (`app_events.rs:2970`): `thread::sleep` puro pode estourar 1-2 ms; dormir até
  ~1,5 ms antes do alvo e fazer spin no resto.

---

## 3. Simulação, IA e scripts

A VM de scripts (`omsi-script/src/vm.rs`) já é bom bytecode; só sobram micro-custos
(`String` clonada em `LoadStr`/`StoreStr`/`PushStr`, linhas ~381-444, relevante para matrix
displays; `Stacks::push/pop` com `copy_within`, não vale mexer).

### 3.1 `body_in_way` O(carros × footprints) — médio-alto, serial
`traffic.rs:4942-5040`, chamado em `5501` no loop de planejamento serial (`5456+`). Para cada carro
filtra TODOS os `feet` (reconstruídos a cada tick, `4842`/`5458`) com 3 closures (com `sqrt`),
aloca `Vec<&Footprint>` e amostra `way_point()` a cada 0,75/1,5 m até ~100 m (`way_point` = Hermite
com 4 sin/cos, `omsi-sim/src/traffic.rs:196-213`).
**Correção:** bin dos `feet` no grid de 50 m existente (ou hash de 25 m) uma vez por tick; consultar
só as células ao longo do alcance; distância ao quadrado; Vec de scratch em `Traffic`; cache das
amostras de `way_point` por carro/tick. Cuidado com determinismo (testes de tráfego existentes).
`people_stop` (`4208-4275`) e `may_cross` (`humans.rs:4581-4602`) têm o mesmo padrão
(carros × pessoas): binar pessoas no grid; reavaliar pedestres em espera a cada 0,1-0,2 s.

### 3.2 HashMaps/Vecs refeitos a cada tick no planejamento — médio, esforço baixo-médio
`traffic.rs:5236-5320`: `by_lane`, `coming`, `reservations`, `walkers` como `HashMap<usize, Vec<..>>`
e `index_of` (`5232`) reconstruídos por tick; `way_lanes()` (`4281`) devolve `Vec` e roda 2x por
carro (`5300`, `5339`); `obstacle_ahead` faz `upcoming().collect()` (`3163`). Ids de lane são densos.
**Correção:** `Vec<SmallVec<[_;4]>>` indexado por lane, limpo (não liberado) a cada tick; `index_of`
como Vec; `way_lanes(reach)` uma vez por carro em buffer de scratch.

### 3.3 Passo da multidão (`omsi-sim/src/crowd.rs`, `humans.rs`) — médio com paradas cheias
- `crowd.rs:158-260`: grid `HashMap<(u64,i32,i32), Vec<usize>>` por passo, passo single-thread,
  `walkers.clone()` (`humans.rs:4251`), `t.powf(2.0)` constante (~153) → `t*t`.
- `crowd.rs:~225`: loop de `blocks` por walker (O(W×B)); `humans.rs:4250` filtra blocos com
  `walkers.iter().any` (O(B×W)); blocos incluem todo carro estacionado a 320 m.
- `humans.rs:4190-4206`: O(pessoas × blocos) com `length()` → `length_squared() < 0.25`.
- `humans.rs:4170`: `remove.contains(&i)` em loop → `Vec<bool>`.
**Correção:** grid espacial único de blocos compartilhado pelos 3 consumidores; grid de células como
array plano ordenado; passe de forças com `par_chunks` (só lê `walkers`, escreve `new_vel`; a ordem
da soma por walker não muda).

### 3.4 Variáveis por nome, todo frame, para todo carro de IA — baixo-médio, mecânico
`omsi-sim/src/vehicle.rs:2378-2496`: `set_engine_var("schedule_active")`, ~10 `set_var(nome, ..)` e
`var("AI_Scheduled_AtStation")` (lowercase + hash) por carro por frame, dentro da região paralela;
`self.v_wheels.clone()` (~2411) aloca por frame; `ai_motion::rotation_point` recalculado por frame.
**Correção:** resolver os ~12 nomes a `VarId` na construção (como `v_velocity`); iterar `v_wheels` por
índice; cachear `rotation_point` no tipo.

### 3.5 Substeps mínimos de 4 na física do ônibus — médio-alto a FPS alto
`omsi-sim/src/rigid/train.rs:415`: `substeps = ceil(dt/0.0042).clamp(4,16)`. A 150 fps são 4 substeps
de 1,7 ms (~2,5x mais trabalho que o necessário; ~5x a 300 fps), cada um com `tyre_line`
(`rigid.rs:1310+`, `LATTICE=18` sondas de solo por roda: ~108 por substep num ônibus de 6 rodas).
`train.rs:420-422` e `348` ainda alocam `Vec`s por substep/chamada.
**Correção:** acumulador fixo de 4,2 ms (ou `clamp(1,16)`, já que 4,2 ms é o limite estável
declarado); Vecs viram arrays de pilha/scratch; memoizar sondas do lattice por roda *(especulativo,
depende do custo da sonda)*. **Risco:** muda a numérica da suspensão: rodar testes de física e
checar a "sensação".

### 3.6 Menores
- `omsi_cfg::env::var_os` (`omsi-cfg/src/lib.rs:1220`: SipHash + clone de `OsString`) em ~41 pontos
  de `traffic.rs`, 22 de `vehicle.rs`, 15 de `humans.rs`, todo tick/carro. `OnceLock<bool>`/campos
  em `Traffic` (como `debug_pax()`). O mais barato da lista.
- `schedule::tick` (`schedule.rs:1990-1998`) varre todas as `departures` por frame apesar de já
  ordenadas (`partition_point` é usado logo abaixo); `traffic.cars.iter().position(..)` por viagem
  (`2170+`) deveria usar `Traffic::index_of`; `HashSet` de ids vivos refeito por tick (~2100).
- LAN `sync_vars` (`lan.rs:3026-3045`) monta `Vec<f32>`/`Vec<String>` clonando strings *todo frame*
  antes do rate limit em `var_sender.tick` (`omsi-net/src/lib.rs:2753`). Só em sessões LAN: testar
  "devido?" antes e reusar buffers.
- Sound sets (`omsi-audio/src/soundset.rs:337,344,350,540-549`, `lan.rs:3100+`): `var(&nome)` por
  condição/curva/pitch a cada frame (50-150 entradas por ônibus). Resolver a `VarId` na criação do
  `SoundSet` (muda a API entre crates).
- Callbacks do host (`omsi-sim/src/host.rs:354-375`): `match` em string em minúsculas por chamada,
  ignorando o `NameId` recebido; `getfontindex` com varredura linear. Mapear `NameId → enum` uma vez.
- Caminhos: `Lane::nearest_point` (`omsi-sim/src/traffic.rs:147-160`) sem AABB de rejeição e padrões
  `seen: Vec` + `.contains` O(n²); `shortest_path` (`1089+`) Dijkstra com `vec![INF; n]` por consulta;
  `way_back` (`navigator.rs:2243`) com HashMaps até 6 km. É correção de *hitch* em spawn/reroute, não
  custo sustentado: AABB, array com carimbo de geração, A*.
- **Skinning de humanos na CPU** (`humans.rs:4930-4990`) + upload de malha por pessoa: LOD e rayon já
  existem, mas o custo escala com vértices × pessoas posadas. Skinning na GPU (paleta de ossos em
  storage buffer) removeria skin e upload; potencialmente alto em multidões *(especulativo)*, esforço
  alto (toca shaders e o caminho de instâncias).

---

## 4. Carregamento de assets, áudio e build

O caminho de assets já é bom: `find_texture` memoizado, listagens de diretório em cache, BC1/BC3 e
mips paralelos, gate de PSNR, mips na GPU, DDS existentes preservados, tiles em pool rayon.

### 4.1 Cache em disco de texturas prontas para GPU — alto na carga, esforço médio, risco baixo
`omsi-texture/src/gpu.rs:182-210` (`prepare_image_with`) e `lib.rs:567-590` (`get_gpu_path`): todo
lançamento decodifica BMP/TGA/PNG/JPG → RGBA8, reamostra (Catmull-Rom) se não for múltiplo de 4,
codifica BC1/BC3 do nível 0, reduz e codifica cada mip, e roda o gate de PSNR. Uma BC3 2048² com
mips custa 10-100+ ms multi-core; a frota de repaints repete isso a cada execução. O cache é só em
memória (única exceção em disco: o PNG do papel de horário, `schedule_paper.rs:46`).
**Correção:** gravar os blocos em `<cache>/tex/<hash(caminho, tamanho, mtime, GpuOptions, versão do
encoder)>.bin` (cabeçalho: w, h, formato, níveis, has_alpha + bytes) e consultar antes de decodificar;
cachear também o resultado "rejeitado" (para não recodificar só para descobrir que falha).
**Ganho:** 2º lançamento provavelmente 3-10x mais rápido em mapas com muitas texturas
*(especulativo)*. Também evita o intermediário RGBA. `release` (`lib.rs:621-625`) descarta a cópia
decodificada, então reuso entre cenas redecodifica: manter os blocos BC residentes (4-8x menores).

### 4.2 Perfil de release (`Cargo.toml`)
| Item | Ganho típico | Observação |
|------|--------------|------------|
| `overflow-checks = true` no release | ~2-8% em código aritmético (sim, geometria, build de malha, mixer) | Manter nos parsers de conteúdo não confiável (`omsi-o3d`, `-cfg`, `-scenery`, `-map`, `-vehicle`) e desligar por pacote (`[profile.release.package.<crate>]`) em `omsi-sim`, `omsi-geometry`, `omsi-render`, `omsi-audio`. Os parsers já usam `need()`/`checked_add`. Decisão de política |
| `codegen-units = 4` + thin LTO | 3-10% (fat LTO +1-5%) | Criar `[profile.dist]` herdando release com `codegen-units = 1`, `lto = "fat"`, `debug = 0`; usar só em artefatos de release. CI de testes não muda |
| `panic = "abort"` | 1-5% e binário 5-10% menor | **Bloqueio:** há `catch_unwind` em `omsi-render/src/lib.rs:13056,15827`, `tiles.rs:1209`, `launcher/state.rs:367`, `install.rs:226`, e rayon propaga panics. Só no `dist`, se isolamento de crash for aceitável |
| Allocator global | 5-15% do tempo de carga *(especulativo)*, menos contenção entre workers rayon; mais relevante no Windows | `mimalloc` só no `omsi-app` (testar antes em cdylib/Android). Churn alto: `CfgFile` com `Vec<String>`, `str8`, mixer por callback, loaders por tile |
| `debug = 1` | Nenhum em runtime; menos tempo de link/disco | `line-tables-only` ou `split-debuginfo` + `strip` no `dist` |
| `target-cpu` / PGO | `x86-64-v3` 5-20% nos kernels (BC, downsample, mixer, glam) *(especulativo)*; PGO 5-15% | `x86-64-v3` (AVX2/FMA/BMI2) não roda em Intel pré-Haswell (2013), em Pentium/Celeron/Atom de várias gerações, nem em AMD FX/Bulldozer–Steamroller; o projeto declara suporte a hardware assim (Intel HD 2500, #770, `startup.rs:258`). Preferir `x86-64-v2` ou dispatch em runtime (`#[target_feature]`) só nos kernels quentes. PGO: `scripts/profile-th-wald.ps1` serve de treino; trabalho de CI |

### 4.3 Parsing e VFS
- **`CfgFile`** (`omsi-cfg/src/lib.rs:55-79`): `decode_text` devolve `String`, `split_lines` copia char a
  char para `Vec<String>` (1 alocação por linha), e `decode` (`codepage.rs:206-213`) faz
  `from_utf8_lossy(..).into_owned()` mesmo em ASCII puro: três cópias. Um buffer + `Vec<Range<u32>>`,
  `memchr` no split, `String::from_utf8` sem cópia. ~20-40% menos tempo de parse de `.map/.sco/.hof/.cfg`
  grandes *(especulativo)*. Esforço médio: `lines` é campo público usado em vários crates.
- **ZIP VFS** (`omsi-cfg/src/vfs.rs:287-305`): `metadata()` (syscall) por leitura em `check_range`
  (~309) → guardar o tamanho na abertura; buffer comprimido novo + buffer inflado → scratch reusado.
  Conferir se `flate2` usa o backend `zlib-rs` (o `Cargo.lock` lista `miniz_oxide` e `zlib-rs`);
  `zlib-rs` descomprime ~1,5-2x mais rápido. `locate()` (`413`) aloca `Vec<String>` + `to_lowercase` por
  componente a cada chamada (barato quando nenhum zip está montado).
- **Lookup case-insensitive** (`name_key`, `omsi-cfg/src/lib.rs:474-516`): bom desenho (listagem
  preguiçosa por diretório), mas um único `Mutex` em `LISTINGS`, travado 2x por lookup, serializa os
  loaders rayon → `RwLock` (como `MOUNTS`) ou sharding.
- **Decodificação de imagem** (`omsi-texture/src/lib.rs:108-165`): `image` → `DynamicImage` →
  `into_rgba8()` (cópia + expansão) → `bmp32_alpha` reparseia o cabeçalho. Um caminho rápido para
  BMP 24/32 bits sem compressão (o formato típico do jogo base; já existe `bmp24_bitfields` de
  modelo) deve ser ~2-3x mais rápido *(especulativo)* e evita o decode-falha-retry (`:150`).
- **`omsi-o3d`** (`lib.rs:75-80, 128-235`): parse já é linear; para arquivos sem scramble
  (chave `0xFFFFFFFF`) o bloco de vértices poderia ir direto por `bytemuck::cast_slice` (32 B por
  vértice). Ganho baixo; o custo real está em `drop_bad_triangles` e na tesselação posterior. Cache em
  disco de malhas só se um profile mostrar necessidade.

### 4.4 Áudio (`omsi-audio/src/mixer.rs`, `wav.rs`)
- `index: HashMap<VoiceId, usize>` reconstruído quase todo callback (o jogo seta parâmetros de toda
  voz todo frame, comentário em `mixer.rs:100`) → sort + busca binária ou índice de slot persistente.
- Loop interno (`131-330`): closures `sample(c)` com `c.min(cch-1)`, bounds checks em
  `clip.samples[i0*cch+c]`, divisão `/32768.0` por tap e branch de `lowpass_hz` dentro do loop;
  içar mono/estéreo e lowpass para fora, multiplicar por `1.0/32768.0` pré-calculado (ou clips em
  f32 na carga, dobrando a memória), remover `% nframes` do loop. Acima de 200 vozes, `ranked`/`keep`
  alocam por callback.
- `wav.rs:46-50` monta `Vec<i16>` com iterador por amostra; `read_clip` (`mixer.rs:393`) mantém dois
  buffers cheios por clip durante a carga. Parsear in-place; mmap para arquivos grandes.

### 4.5 Texturas rejeitadas pelo gate e Android
Abaixo de 33 dB de PSNR (`MIN_PSNR_COLOUR`), de 64×64 texels ou com lado < 128 não múltiplo de 4,
a textura fica RGBA8. BC7 melhoraria a qualidade das rejeitadas (encode mais lento); ASTC no mobile
(ver §1.6). Ganho de VRAM só para as rejeitadas; esforço alto.

### 4.6 CI
`depot-ci.yml:47-52` roda `cargo test --release` e depois `cargo build --release`. Conferir cache
(`Swatinem/rust-cache`/`sccache`) e usar `--profile dist` apenas em tags. Ganho: tempo de
desenvolvedor, não de runtime.

---

## 5. Launcher (UI): redesenho planejado

Marcado como prioridade. Leitura de código, **sem medição**. O preview 3D do ônibus é mantido (é um
diferencial); o resto da UI é o problema.

**Diagnóstico**
1. `make_surface` (`launcher/mod.rs:316-339`) abre o `Renderer` 3D completo do jogo, com MSAA 4x e
   `showroom_options` (`mod.rs:1297`), mesmo em páginas sem ônibus (Settings, Controls, Mods...).
   *A verificar:* se `Renderer::new` compila todos os pipelines na criação (provável maior custo de abertura).
2. A UI é desenhada em modo imediato sobre esse device, com **MSAA 4x em tela cheia** (`mod.rs:331`).
3. Sem dirty tracking: todo frame refaz layout, re-tessela e reenvia o buffer de vértices inteiro
   (`mod.rs:~870`). `about_to_wait` (`mod.rs:562-606`) usa `Poll` por 3 s após input e depois redesenha
   tudo a 20 fps parado.
4. Custo por texto, todo frame: `i18n::tr` (RwLock + tabela + tradução automática), chave `String` do
   atlas (`omsi-ui/src/atlas.rs`), `fit` com busca binária alocando `String`s e `cap_height` com
   `outline_glyph` (`omsi-ui/src/text.rs`); `rounded_outline` aloca `Vec` com sin/cos por forma.
5. Listas só pulam o desenho via `rect_visible` (`drive.rs:310,400,907`); o layout de todas as linhas roda.
6. Acoplamento launcher/jogo na GPU gera remendos (devolver o device ao abrir o jogo, recuperação de
   device perdido #274, ordem de backends #1058/#1044/#746/#770).

**Plano (por risco crescente)**
1. Redesenhar só quando algo mudou (`ControlFlow::Wait`, `request_redraw` por input/animação/dados) em
   vez de 20 fps contínuos. Independente dos demais.
2. UI sem MSAA (1x); cantos arredondados continuam suaves pelo shader/tesselação.
3. Cache de texto por frame: tradução e `fit`/`cap_height` memoizados por (texto, px, peso, largura);
   chave do atlas sem alocar; buffers de `rounded_outline` reaproveitados.
4. `Renderer` 3D criado só ao abrir uma página com preview e liberado ao sair; UI com device/pipeline
   próprios e leves.
5. Layout virtualizado nas listas longas (calcular só as linhas visíveis).

**Medir antes e depois:** tempo até a primeira janela útil, CPU/GPU em idle por 60 s, frame time ao
rolar a lista de ônibus, memória residente.

## 6. Ordem de ataque sugerida

**Fase 0: medir (antes de tudo).** Capturar `OMSI_PROFILE` (inclui `tick_split` plan/par/post) em 2-3
cenários fixos (cidade pesada com tráfego; Enhanced+MSAA à noite; viagem rápida com streaming de
tiles; carga fria vs. quente). Sem isso, os números acima continuam sendo estimativas.

**Fase 1: horas de trabalho, risco baixo.**
`overflow-checks` por pacote e perfil `dist` (§4.2) · `var_os` → `OnceLock` (§3.6, §1.7, §2.7) ·
`schedule::tick` com `partition_point` · `powf(2.0)`/`length_squared`/`remove` como `Vec<bool>`
(§3.3) · sombras dos postes em cache (§1.2) · `flate2` com `zlib-rs` · `ctl.disabled` e
`missing()` sem alocar por frame · `VarId` nas variáveis de IA (§3.4).

**Fase 2: dias, maior retorno em frame time.**
Recursos de GPU do streaming fora da thread principal (§2.1) · prepass duplicado do MSAA (§1.1) ·
listas/colisão incrementais (§2.2) · pipeline do streamer (§2.3) · binning espacial em
`body_in_way`/`people_stop` + Vecs por lane (§3.1, §3.2) · `tree_sway` por instância (§1.4) ·
mimalloc (§4.2) · `update_scripted` (§2.6).

**Fase 3: projetos maiores.**
Cache em disco de texturas BC (§4.1) e de índice de mapa (§2.4) · cascata de sombra próxima
estática+dinâmica (§1.3) · substeps da física (§3.5) · `CfgFile` em buffer único (§4.3) · PBR
comprimido/ASTC (§1.6) · PGO · skinning na GPU (§3.6) · desenho GPU-driven/Hi-Z (§1.8).

## 7. Limitações desta auditoria

- Nenhum benchmark foi executado; todas as estimativas de ganho vêm do formato do código.
- Não foram lidos em profundidade: `omsi-scenery`, `omsi-map`, `omsi-vehicle`, `omsi-timetable`,
  `omsi-net`, `omsi-model`, `omsi-content` e `omsi-ui`. Pode haver mais alocações por token nos
  parsers.
- O renderer foi auditado em `lib.rs` e `shader.wgsl`; os demais `.wgsl` e o caminho de OpenXR/VR
  não foram examinados.
- Itens em *(especulativo)* e a retenção de lanes no unload (§2.7) pedem confirmação antes de agir.
