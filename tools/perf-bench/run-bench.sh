#!/bin/sh
# Linux/macOS twin of run-bench.ps1: one run per MSAA level with OMSI_PROFILE and a private
# HOME, then the summary. Other exported OMSI_* variables pass through. Protocol:
# docs/performance-measurement.md (#36, #43).
#
#   ROOT=~/omsi2 MAP=maps/X/global.cfg BUS=Vehicles/X/X.bus SPAWN=1000,2000,90,0 \
#   MSAA="1 4 8" SECONDS_RUN=90 REPEAT=3 tools/perf-bench/run-bench.sh
set -eu
: "${ROOT:?ROOT}" "${MAP:?MAP}" "${BUS:?BUS}" "${SPAWN:?SPAWN}"
EXE=${EXE:-target/release/depot}
MSAA=${MSAA:-4}
SECONDS_RUN=${SECONDS_RUN:-90}
REPEAT=${REPEAT:-1}
VIEW=${VIEW:-driver}
TRAFFIC=${TRAFFIC:-20}
TIME=${TIME:-09:00}
VSYNC=${VSYNC:-0}
SEED=${SEED:-30794024}
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out="${OUT:-$repo/target/perf-bench}/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
logs=""
for level in $MSAA; do
  run=1
  while [ "$run" -le "$REPEAT" ]; do
    name="msaa$level-run$run"
    h="$out/home-$name"
    mkdir -p "$h/.depot"
    { [ -f "$HOME/.depot/settings.cfg" ] && grep -v -E '^(msaa|vsync)=' "$HOME/.depot/settings.cfg" || true; } > "$h/.depot/settings.cfg"
    printf 'msaa=%s\nvsync=%s\n' "$level" "$VSYNC" >> "$h/.depot/settings.cfg"
    echo "$name"
    HOME="$h" RUST_LOG=info OMSI_PROFILE=1 OMSI_SEED="$SEED" OMSI_MAX_FPS=1000 \
      "$repo/$EXE" --root "$ROOT" --no-menu --map "$MAP" --bus "$BUS" --time "$TIME" \
        --traffic "$TRAFFIC" --passengers --schedule --spawn "$SPAWN" --view "$VIEW" \
        --exit-after "$SECONDS_RUN" > "$out/$name.stdout.log" 2> "$out/$name.stderr.log" || echo "$name exited non-zero"
    if [ -f "$h/.depot/game.log" ]; then
      cp "$h/.depot/game.log" "$out/$name.log"
      logs="$logs $out/$name.log"
    else
      echo "$name wrote no game.log"
    fi
    run=$((run + 1))
  done
done
# shellcheck disable=SC2086
[ -n "$logs" ] && python3 -I "$here/analyze_log.py" $logs | tee "$out/summary.md"
echo "Logs and summary: $out"
