# Plano de desempenho: raio do cursor e picos de trânsito

Plano de 24/10/2026, revisto em 25/10/2026, a partir das medições da issue #43. Só leitura de código e de logs; nenhuma linha de código mudou ainda. As medições seguem o [protocolo de medição](performance-measurement.md).

## Resumo

O raio do cursor é quase todo o problema das travadas: 5,8 s de 6,2 s medidos nos 147 quadros acima de 50 ms da sessão de 24/10 (19:25), com picos de 40 a 82 ms. O trânsito trava quando o jogo cria muitos carros de uma vez: 46 ms na primeira povoação daquela sessão e quadros de 250 a 500 ms numa sessão de 25/10, ao entrar numa região de muitas faixas.

O plano tem dois PRs para o cursor e dois para o trânsito, nessa ordem. Nenhum muda o que o jogador vê ou qual carro aparece: só quando e como o cálculo é feito.

| etapa | quadros lentos com ela | total (ms) | pico (ms) |
| --- | --- | --- | --- |
| `player.hover` (cursor) | 143 de 147 | 5802 | 82 |
| `traffic` | 72 de 147 | 223 | 55 |
| `traffic.populate` | 4 de 147 | 54 | 46 |

Fonte: `game.log` de 24/10/2026, 19:25–19:33, build `3d3143ad`, `OMSI_PROFILE=1`, MSAA 8x, sombras 4096, SSAO, 3440x1440, TopbusPB articulado, trânsito 120.

## Raio do cursor

O custo vem de testar o raio contra todos os triângulos das malhas grandes da cabine, várias vezes por consulta e mais de uma consulta por quadro. A câmera fica dentro do ônibus, então a esfera de cada malha grande (carroceria, interior) contém o olho e o filtro por esfera (`ray_may_hit`) não descarta nada.

### O que uma consulta faz hoje

`update_hover` (`crates/omsi-app/src/input/mouse.rs`) chama `hovered_part` (`crates/omsi-app/src/player.rs`), que faz em sequência:

1. `pick_in`: até 25 raios (o central e dois anéis de 8 e 16) contra as malhas com `[mouseevent]`.
2. Para cada raio com acerto, `nearest_blocker`: um raio contra todas as malhas sem evento do ônibus e dos reboques (368 malhas no TopbusPB), triângulo por triângulo. Se o interruptor está tapado, tenta o próximo raio e repete.
3. Sem interruptor: `pick_trailer_in` repete o passo 1 nos reboques.
4. Ainda sem nada: `hovered_body_part` passa outra vez por todas as malhas visíveis para dar o nome da peça.

Em cada teste, `ray_mesh` (`crates/omsi-geometry/src/lib.rs`) recalcula e inverte a matriz da malha. Os passos 2 e 4 repetem o mesmo raio central sobre as mesmas malhas.

### Consultas por quadro

- Cada evento de movimento do mouse chama `update_hover` na hora (`on_cursor`), fora das etapas do profiler. Um mouse de 1000 Hz gera vários eventos por quadro.
- O quadro chama de novo na etapa `player.hover`, porque o evento não atualiza `hover_key`. É a parte que aparece no log; a parte dos eventos pode explicar parte do tempo "fora das etapas".

### Plano

1. **Medir as partes** (no mesmo PR da etapa 2): subetapas `player.hover.pick`, `.blocker`, `.trailer`, `.body` e contagem de triângulos testados, no log de travadas. Mais um teste de desempenho com uma malha sintética de cabine.
2. **Uma consulta por quadro** (`fix/cursor-uma-consulta`): o evento só marca que o cursor mudou e o quadro faz a consulta. Toque e scripts `OMSI_INPUT`, que leem `hover` logo após o evento, continuam consultando na hora. Calcular a matriz e a inversa de cada malha uma vez por consulta e reaproveitar o raio central do passo 2 no passo 4.
3. **Árvore de caixas por malha** (`fix/cursor-arvore-malha`): uma BVH no espaço da malha em `omsi-geometry`, montada uma vez por tipo de veículo (na primeira consulta, para não pesar no carregamento) e compartilhada pelas cópias. `ray_mesh`, `ray_mesh_hit`, `cone_triangles` e `ray_triangles` passam a usá-la. O teste cai de todos os triângulos para algumas dezenas. As malhas com esqueleto (sanfona, cortina, para-sol) se deformam: ficam no teste completo.

Resultado esperado (estimativa, a confirmar com a medição): picos de 40–82 ms abaixo de 2 ms, com a mesma peça escolhida. O mesmo ganho vale para o clique, a roda do mouse e o ponteiro de VR, que usam as mesmas funções.

### Riscos

- A escolha do interruptor não pode mudar (ibox do Aachen, painéis VDV). Os testes atuais de `best_mouseevent_hit` ficam, e entra um teste que compara a BVH com o teste completo em malhas reais.
- Memória da BVH: cerca de duas vezes o índice de triângulos, só para os tipos que recebem consulta (o ônibus do jogador).

## Picos de trânsito

O jogo cria de uma vez todos os carros que faltam para o alvo, e criar carros é o que pesa, não tê-los.

### O que os logs mostram

- **24/10, 19:25 (com profiler):** um único pico, no quadro 81 (71 ms), com `traffic.populate` 46 ms, `traffic.schedule` 5 ms e `traffic.sync` 3 ms. Foi a primeira povoação, logo depois do carregamento. No resto da sessão o trânsito ficou entre 2 e 8 ms nos quadros lentos.
- **25/10, 02:51–03:25 (sem profiler, só a linha de status por minuto):** entre 03:19 e 03:21 o número de carros passou de 122 para 262 e depois 303, e o pior quadro de cada minuto caiu para 2–4 fps (quadros de 250–500 ms), antecedido de travadas menores. Com 200 carros parados às 03:02 o jogo ficou em 63 fps.

### Causa provável

O alvo de carros (`street_target` em `populate_seen`, `crates/omsi-app/src/traffic.rs`) é o número pedido vezes a densidade da hora vezes `road_scale`, que cresce com a quantidade e a densidade das faixas por perto (até 8 vezes). Ao chegar numa região densa o alvo sobe, e `populate_kind` e `wake_dormant` criam todo o déficit numa chamada só. As travadas menores de antes devem ser o alvo subindo aos poucos durante a aproximação.

O limite `[AIMaxCountRandom]` (`max_random`, padrão 1000) entrou depois dessas sessões e corta o alvo, mas não muda o fato de que o déficit é criado de uma vez.

A causa da sessão de 25/10 ainda não está medida: falta repetir o mesmo trecho com `OMSI_PROFILE=1` e `OMSI_DEBUG_POPULATION=1`.

### Plano

1. **Medir as partes** (`feat/profile-povoacao`, pequeno): subetapas `traffic.populate.despawn`, `.wake`, `.parked`, `.spawn`, mais a contagem de carros criados e removidos na chamada, no log de travadas. Uma rodada no mesmo trecho mostra onde está o tempo.
2. **Orçamento por chamada** (`fix/transito-orcamento-povoacao`): criar no máximo alguns carros por chamada (por tempo, por exemplo 3 ms, ou por contagem) em `populate_kind` e `wake_dormant`, e voltar em 0,1 s enquanto faltar carro, como o código já faz com as partidas da tabela. O alvo é atingido em um ou dois segundos sem travar um quadro.
3. **Primeira povoação na tela de carregamento**: complemento do item 2 só para o início. Fazer a primeira chamada antes do primeiro quadro, onde o custo não aparece.

### Riscos

- O orçamento muda quando cada carro aparece, não onde nem qual. Os carros continuam nascendo só fora da vista, então nada aparece "do nada".
- Os testes do trânsito que contam carros logo após `populate` precisam chamar até o alvo ser atingido.

## Ordem dos PRs e como medir

Um PR por vez, cada um com a medição antes e depois na mesma situação salva (TopbusPB, MSAA 8x, trânsito 120, `OMSI_PROFILE=1`).

| ordem | branch | o que muda | como provar |
| --- | --- | --- | --- |
| 1 | `fix/cursor-uma-consulta` | uma consulta por quadro, matrizes reaproveitadas, subetapas do cursor | teste de que toque e `OMSI_INPUT` ainda leem `hover` na hora; log com as subetapas |
| 2 | `fix/cursor-arvore-malha` | BVH por malha em `omsi-geometry` | teste BVH = teste completo em malhas reais; teste de desempenho; `player.hover` no log |
| 3 | `feat/profile-povoacao` | subetapas da povoação | uma rodada no trecho denso de 25/10 |
| 4 | `fix/transito-orcamento-povoacao` | orçamento por chamada e primeira povoação no carregamento | testes do trânsito; nenhum quadro acima de 50 ms com `traffic.populate` |

Critério de pronto: nenhum quadro acima de 50 ms causado por `player.hover` ou `traffic.populate` numa sessão de 5 minutos dirigindo, mexendo nos interruptores e passando por uma região de muitas faixas.
