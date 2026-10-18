#!/usr/bin/env python3
"""Summarise `game.log` files written with OMSI_PROFILE=1 for the measurement protocol in
docs/performance-measurement.md (#36, #43). Standard library only.

    analyze_log.py run-a.log [run-b.log ...]          # one column per log, as a Markdown table
    analyze_log.py --stutters run-a.log               # every stutter line, with the stage list
    analyze_log.py --json run-a.log                   # the same numbers as JSON

It only reads lines the game already prints; it changes nothing about a run.
"""
import argparse
import json
import re
import statistics
import sys
from datetime import datetime

STUTTER = re.compile(r"stutter: frame (\d+) took (\d+) ms \((.*?)(?:; )?outside the stages (\d+) ms\)")
EXIT = re.compile(
    r"exit after ([\d.]+) s: (\d+) frames total .*?, ([\d.]+) fps average, (\d+) frames over 50 ms, worst (\d+) ms"
)
SETTINGS = re.compile(r"settings from .*?: msaa (\d+) .*? vsync (\w+) render scale (\S+)")
SCENE = re.compile(r"profile: window (\d+)x(\d+), scene drawn at (\d+)x(\d+), (\d+)x MSAA")
GPU_PASS = re.compile(r"profile gpu pass (\S+)\s*: ([\d.]+) ms")
COUNT = re.compile(r"profile count (\S+): ([\d.]+) a frame")
STAGE = re.compile(r"\bprofile ([\w.]+?)\s*: ([\d.]+) ms/frame")
CPU = re.compile(r"profile: since 15 s ([\d.]+) ms wall and ([\d.]+) ms CPU")
GROUP_FRAMES = 5
TS = re.compile(r"^\[(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)")


def stage_list(text):
    """`render 15, tiles 900` -> [("render", 15), ("tiles", 900)]; sub-stages (with a dot) are kept."""
    out = []
    for part in filter(None, (p.strip() for p in text.split(","))):
        name, _, ms = part.rpartition(" ")
        if name and ms.isdigit():
            out.append((name, int(ms)))
    return out


def parse(path):
    r = {"path": path, "stutters": [], "gpu_pass": {}, "counts": {}, "stages": {}}
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            ts = TS.match(line)
            when = datetime.fromisoformat(ts.group(1)) if ts else None
            if m := STUTTER.search(line):
                stages = stage_list(m.group(3))
                r["stutters"].append(
                    {"frame": int(m.group(1)), "ms": int(m.group(2)), "outside": int(m.group(4)), "stages": stages, "at": when}
                )
            elif m := EXIT.search(line):
                r["seconds"], r["frames"] = float(m.group(1)), int(m.group(2))
                r["fps"], r["over50"], r["worst_ms"] = float(m.group(3)), int(m.group(4)), int(m.group(5))
            elif m := SETTINGS.search(line):
                r["msaa"], r["vsync"], r["render_scale"] = int(m.group(1)), m.group(2), m.group(3)
            elif m := SCENE.search(line):
                r["scene"] = f"{m.group(3)}x{m.group(4)}"
                r["msaa"] = int(m.group(5))
            elif m := GPU_PASS.search(line):
                r["gpu_pass"][m.group(1)] = float(m.group(2))
            elif m := COUNT.search(line):
                r["counts"][m.group(1)] = float(m.group(2))
            elif m := CPU.search(line):
                r["wall_ms"], r["cpu_ms"] = float(m.group(1)), float(m.group(2))
            elif m := STAGE.search(line):
                r["stages"][m.group(1)] = float(m.group(2))
    r.update(summarise_stutters(r["stutters"]))
    return r


def summarise_stutters(stutters):
    """Dominant stage per stutter and how regular the spacing is (the question of #36)."""
    out = {"dominant": {}, "interval_frames": None, "interval_s": None, "regular": None}
    for s in stutters:
        top = [(n, v) for n, v in s["stages"] if "." not in n]
        top.append(("(outside the stages)", s["outside"]))
        name = max(top, key=lambda p: p[1])[0]
        out["dominant"][name] = out["dominant"].get(name, 0) + 1
    # frames over 50 ms within GROUP_FRAMES of each other are one freeze; spacing runs from the
    # first frame of one freeze to the first of the next
    firsts = []
    for s in stutters:
        if not firsts or s["frame"] - last_frame > GROUP_FRAMES:
            firsts.append(s)
        last_frame = s["frame"]
    out["events"] = len(firsts)
    if len(firsts) >= 3:
        gaps = [b["frame"] - a["frame"] for a, b in zip(firsts, firsts[1:])]
        out["interval_frames"] = statistics.median(gaps)
        if all(s["at"] for s in firsts):
            secs = [(b["at"] - a["at"]).total_seconds() for a, b in zip(firsts, firsts[1:])]
            out["interval_s"] = statistics.median(secs)
        if len(firsts) >= 6 and statistics.mean(gaps) > 0:
            out["regular"] = statistics.pstdev(gaps) / statistics.mean(gaps) < 0.25
    return out


def dominant_text(d):
    return ", ".join(f"{k} x{v}" for k, v in sorted(d.items(), key=lambda p: -p[1])) or "-"


def table(runs):
    rows = [
        ("log", lambda r: r["path"].replace("\\", "/").rsplit("/", 1)[-1]),
        ("MSAA", lambda r: r.get("msaa", "?")),
        ("scene size", lambda r: r.get("scene", "?")),
        ("average fps", lambda r: r.get("fps", "?")),
        ("frames over 50 ms", lambda r: r.get("over50", "?")),
        ("worst frame (ms)", lambda r: r.get("worst_ms", "?")),
        ("stutter lines (freezes)", lambda r: f"{len(r['stutters'])} ({r['events']})"),
        ("median gap between stutters (frames)", lambda r: r["interval_frames"] if r["interval_frames"] is not None else "-"),
        ("median gap between stutters (s)", lambda r: r["interval_s"] if r["interval_s"] is not None else "-"),
        ("regular spacing", lambda r: {None: "-", True: "yes", False: "no"}[r["regular"]]),
        ("dominant stage per stutter", lambda r: dominant_text(r["dominant"])),
        ("CPU ms/frame (all threads)", lambda r: r.get("cpu_ms", "-")),
    ]
    names = sorted({k for r in runs for k in r["gpu_pass"]})
    rows += [(f"GPU pass {n} (ms)", lambda r, n=n: r["gpu_pass"].get(n, "-")) for n in names]
    names = sorted({k for r in runs for k in r["counts"]})
    rows += [(f"count {n} per frame", lambda r, n=n: r["counts"].get(n, "-")) for n in names]
    out = ["| | " + " | ".join(str(f(r)) for f in [rows[0][1]] for r in runs) + " |"]
    out.append("|---|" + "---|" * len(runs))
    out += ["| " + label + " | " + " | ".join(str(f(r)) for r in runs) + " |" for label, f in rows[1:]]
    return "\n".join(out)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("logs", nargs="+")
    ap.add_argument("--stutters", action="store_true", help="list every stutter line")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    runs = [parse(p) for p in args.logs]
    if args.json:
        for r in runs:
            for s in r["stutters"]:
                s["at"] = s["at"].isoformat() if s["at"] else None
        json.dump(runs, sys.stdout, indent=1)
        print()
        return 0
    if args.stutters:
        for r in runs:
            print(f"# {r['path']}")
            for s in r["stutters"]:
                print(f"frame {s['frame']}: {s['ms']} ms; " + ", ".join(f"{n} {v}" for n, v in s["stages"]) + f"; outside {s['outside']}")
        return 0
    print(table(runs))
    return 0


if __name__ == "__main__":
    sys.exit(main())
