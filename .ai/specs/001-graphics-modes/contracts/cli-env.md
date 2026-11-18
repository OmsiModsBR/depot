# Contract: command-line arguments and environment variables of the graphics mode

Documented today in `docs/USER_GUIDE.md` and `docs/ENVIRONMENT.md` (made from
`omsi-cfg/src/flags.rs`).

## Command-line arguments (`omsi-app/src/core/domain/cli.rs`)

| Argument | After |
|---|---|
| `--enhanced` | stays (Enhanced for this run) |
| `--enhanced-plus` | **removed** from the help and the USER_GUIDE; still accepted and ignored with a log line (spec, Edge Cases: never an error) |

## Variables

| Variable | Kind | After |
|---|---|---|
| `OMSI_GRAPHICS=vanilla\|vanilla_plus\|enhanced` | user | stays; `enhanced_plus` read as `enhanced` |
| `OMSI_BACKEND=d3d9` | user | **removed**: the backend follows the mode (`OMSI_GRAPHICS=vanilla`) |
| `OMSI_ENHANCED` | user | **removed** (covered by `OMSI_GRAPHICS=enhanced`) |
| `OMSI_ENHANCED_PLUS` | user | **removed** |
| `OMSI_NO_ENHANCED` | user | **removed** (covered by `OMSI_GRAPHICS=vanilla_plus`); no longer read in four places of the frame |
| `OMSI_NO_RT`, `OMSI_NO_RT_GRADE` | user | **removed** |
| `OMSI_NO_RT_FRAME`, `OMSI_RT_REFL_HALF`, `OMSI_DEBUG_RT` | diagnostic | **removed** |
| `OMSI_DEBUG_ENHANCED` | diagnostic | stays; its views of the traced shadow go |
| `OMSI_MIRROR_ENHANCED` | diagnostic | stays |

A removed variable still present in a player's `env.txt` is ignored with a log line, never an error
(spec, Edge Cases); the task of phase A1 checks whether `omsi-cfg` already warns about unknown keys
and adds the warning if not.

## Launcher and the game's menu

See [graphics-options.md](graphics-options.md).
