# Performance measurement protocol

How to measure the two open performance reports, so that a fix is only written after data says
where the time goes. It changes nothing in the game: every number comes from switches and log
lines the game already has (`OMSI_PROFILE`, `OMSI_GPU_TIMERS`, `--exit-after`), read by
[`tools/perf-bench`](../tools/perf-bench).

* #36: periodic freezes of about one second on large maps (Munich), since the 0.2.16 baseline.
* #43: 2-10 FPS near buses with 8x MSAA, about 200 FPS without MSAA.

Neither report has a measurement yet, so neither has a cause. The ideas listed below are
candidates to rule in or out, not conclusions.

## Rules for every run

1. **One variable at a time.** Same executable, map, bus, spawn point, time, date, seed,
   traffic count and window size; change only the thing under test.
2. **Release build, vsync off, no frame cap** (`OMSI_MAX_FPS=0`; the scripts set vsync to 0
   unless told otherwise). A frame cap or vsync hides exactly the frame times being studied.
3. **At least 3 runs per configuration**, the first one discarded if the shader/pipeline
   caches were cold. Report all of them, not the best.
4. **Quiet machine.** Close the browser, overlays and capture tools. Note any that stay on.
5. **Write down the machine once** (see the template) and keep it with the numbers: GPU and
   driver version, CPU, RAM, OS, graphics API (`OMSI_BACKEND` if forced), display resolution.
6. **Attach the raw `game.log`** of each run to the issue. The summary table is for reading;
   the log is the evidence.

## The bench

```
tools/perf-bench/run-bench.ps1   (Windows)     tools/perf-bench/run-bench.sh   (Linux, macOS)
tools/perf-bench/analyze_log.py                 (Python 3, standard library only)
```

The runner starts the game once per MSAA level and repeat with `OMSI_PROFILE=1`,
`--exit-after N`, a fixed `OMSI_SEED` and a private home folder. The private folder holds a
copy of the player's own `settings.cfg` with only `msaa` and `vsync` replaced, so the player's
real `~/.depot` is not touched and the runs use the player's real graphics settings. Each run's
`game.log` is copied next to the others, then `analyze_log.py` prints one Markdown column per
log: average FPS, frames over 50 ms, worst frame, the number and spacing of `stutter:` lines,
the dominant stage of each stutter, CPU time per frame, the per-pass GPU times and the
`profile count` lines.

`analyze_log.py --stutters game.log` lists every stutter with its stage list. The analyzer's
tests run with `python3 -I tools/perf-bench/test_analyze_log.py`.

What `OMSI_PROFILE` already reports (all in `game.log`): a `stutter: frame N took X ms (stage
Y, ...; outside the stages Z ms)` line for every frame over 50 ms after the 60th, the per-stage
milliseconds per frame at exit, `profile gpu pass` lines with `OMSI_GPU_TIMERS=1`, and
`profile count` lines for what the pictures drew. "Outside the stages" is time no stage
accounts for: waiting for the window, the system, or another process.

## #36: periodic freezes (Munich and similar maps)

**Question:** what happens every few seconds that takes about one second?

**Capture.** One run per configuration, 120 s or longer, driving or standing at the same spot
(a standing run separates streaming from driving). `Seconds` must cover several periods.

```
.\tools\perf-bench\run-bench.ps1 -Root 'C:\OMSI 2' -Map maps/Munich/global.cfg `
    -Bus Vehicles/<bus>/<bus>.bus -Spawn '<x>,<y>,<heading>,0' -View driver `
    -Msaa 4 -Seconds 120 -Repeat 3
```

**Read.**

1. In the summary: are the stutters regularly spaced ("regular spacing: yes") and what is the
   median gap in seconds and frames? A fixed period points at a timer (audio, an AI or
   timetable tick, an autosave, a metar sync, the streamer's batch size); a spacing that
   follows distance driven points at tile streaming.
2. Dominant stage per stutter. A stage named in the stutter lines is the answer; "(outside
   the stages)" dominant means the time went somewhere `OMSI_PROFILE` does not time, and the
   next step is to add a stage there, not to guess.
3. In the exit lines, `tile streaming: ... slowest upload X ms, streaming over 16 ms in N
   frames (worst Y ms)` against the stutter count: if tiles dominate, compare a standing run
   with a driving run.

**Discriminating runs** (each one switch, everything else equal; compare the stutter count and
period against the base run):

| Candidate | Run |
|---|---|
| Audio | `OMSI_MUTE=1` |
| AI traffic / scripts | `-Traffic 0`, then `-Traffic 20` |
| Tile streaming | stand still vs. drive the same route; a smaller view distance in `settings.cfg` |
| GPU wait (`gpu`, `present`, `acquire` stages) | `OMSI_PROFILE_GPU=1` so GPU time shows as a stage |
| Pipeline/shader compile | the first stutters of a cold run vs. a second run on the same cache |
| Render worker pool | `OMSI_NO_RENDER_POOL=1` |
| Resolution governor | settings `render_scale` fixed to a number instead of `auto` |

The baseline claim in the issue ("since 0.2.16") is checked by running the same scenario on
the 0.2.16 build and on current `main`; build both from tagged revisions and record the
commit of each.

**Done for #36 when** the issue has: the machine record, the summary table for the baseline
and for `main`, the period, the dominant stage, and the discriminating runs that rule other
candidates out.

## #43: severe FPS drop with 8x MSAA near buses

**Question:** does cost grow with the sample count because of overdraw, and in which pass?

**Camera point.** Fix it so it is identical across runs: spawn on the same coordinates with the
same view, with scheduled buses in sight. Use `-View driver` for the reported case and repeat
once with `-View outside` as a control. `-Spawn` takes the same `x,y,heading,speed` as
`--spawn`.

```
.\tools\perf-bench\run-bench.ps1 -Root 'C:\OMSI 2' -Map <map> -Bus <bus> `
    -Spawn '<x>,<y>,<heading>,0' -View driver -Msaa 1,4,8 -Seconds 60 -Repeat 3 `
    -GpuTimers -DrawCounts
```

**Read.** From the summary, for each of 1x, 4x and 8x:

1. Average FPS and the `profile gpu pass` times. If the extra cost sits in one pass, the table
   shows which. If CPU stages grow with MSAA instead, the cause is not fill rate.
2. The `profile count` lines and the draw counts (`OMSI_DEBUG_DRAWS`, which logs every frame, so keep those runs short): how many blended
   full-screen layers (glass, rain film, dirt) are drawn at the camera point. The hypothesis
   in the issue predicts GPU time per pass growing with the sample count while the counts stay
   the same.
3. `profile: window WxH, scene drawn at WxH, Nx MSAA` to confirm the run used the level it
   was meant to (a count the GPU cannot do falls back to the next lower one).
4. Whether the drop is tied to the bus's presence: repeat the 8x run with the camera moved
   away (a second spawn point 100 m from any bus) and with no traffic.

**Discriminating runs** at 8x:

| Candidate | Run |
|---|---|
| Overdraw of the glass/rain film | `OMSI_NO_GLASS_PICTURE=1` |
| The multisampled depth prepass | `OMSI_NO_MSAA_PREPASS=1` |
| Shadows / shadow casters | `OMSI_NO_SHADOWS=1` |
| Resolve cost / target size | settings `render_scale` below 1 |
| Post anti-aliasing instead | `msaa=1` with `post_aa=fxaa` (what the player would pick instead) |

The reporter's #1710-family reports differ in version and cause; the issue itself notes that
one should not be assumed to be another. Measure on the machine of a reporter if one can
supply a log with `OMSI_PROFILE` output.

**Done for #43 when** the issue has: the machine record, the table for 1x/4x/8x with GPU pass
times and layer counts, the result of each discriminating run, and a statement confirming or
discarding the overdraw hypothesis. A fix or a documented limit or default comes after, with
a before/after on the same bench.

## Template for the issue comment

```
Machine: GPU/driver ..., CPU ..., RAM ..., OS ..., API ..., display ...
Build: <commit or tag>, release, vsync off
Scenario: map ..., bus ..., spawn x,y,heading,speed ..., view ..., time ..., traffic ..., seed ...
Runs: N per configuration (cold first run discarded: yes/no)

<paste summary.md>

Reading: <what the dominant stage is / what the period is / what was ruled out>
Logs: <attached game.log files>
```

## What this protocol does not do

It does not change game behaviour, add instrumentation or propose a fix. If the measurement
shows that a stage is missing from `OMSI_PROFILE` ("outside the stages" dominates), adding a
stage is the next small change, made before anything else. Hardware outside the supported
matrix is recorded, not supported by it.
