#!/usr/bin/env python3
"""Writes a tiny synthetic map package (about 2 KB) to test the Lines panel.

Everything in it is invented by this script: no OpenStreetMap data, nothing from OMSI 2. It has a
`surface.oms` (a lake, a river, a railway), a `manifest.cfg` and a `lines.lin` with one bus line,
one tram line, one train line and a bus line with two eras, so the tabs per kind of vehicle and
the eras can be checked. Format: docs/MAP_PACKAGE.md.

    python3 make_synthetic_package.py OUT_DIR [--bus-line NUMBER]

Copy OUT_DIR to <content folder>/Maps/<Map>/openomsi-realmap/. The package carries no timetable: a
line shows departures when the map has them in its own TTData for a line of the same number, so
give --bus-line the number of a bus line of the map you test with.

For a real package use the generator in https://github.com/lmonteirotech/openomsi-map-packages
(tools/mapgen), the supported way to build packages.
"""
import argparse
import struct
from pathlib import Path

CHUNK = 300.0
WATER, RIVER, RAIL = 0, 0, 1


def chunk_body(fills, lines, labels):
    b = struct.pack("<I", len(fills))
    for cls, tris in fills:
        flat = [v for t in tris for p in t for v in p]
        b += struct.pack("<BI", cls, len(tris)) + struct.pack("<%df" % len(flat), *flat)
    b += struct.pack("<I", len(lines))
    for cls, width, pts in lines:
        flat = [v for p in pts for v in p]
        b += struct.pack("<BBfII", cls, 0, width, 0xFFFFFFFF, len(pts)) + struct.pack("<%df" % len(flat), *flat)
    b += struct.pack("<I", len(labels))
    for kind, rank, name, x, y in labels:
        b += struct.pack("<BBIff", kind, rank, name, x, y)
    return b


def surface():
    names = ["Test Lake"]
    lake = [((40, 40), (140, 40), (140, 120)), ((40, 40), (140, 120), (40, 120))]
    river = (RIVER, 6.0, [(140, 80), (200, 120), (299, 130)])
    rail = (RAIL, 0.0, [(0, 250), (150, 240), (299, 250)])
    body = chunk_body([(WATER, lake)], [river, rail], [(4, 120, 0, 90.0, 80.0)])
    head = b"OMSF" + struct.pack("<I", 1) + struct.pack("<dd", 0.0, 0.0)
    head += struct.pack("<ffff", 0.0, CHUNK, 0.0, CHUNK) + struct.pack("<f", CHUNK)
    head += struct.pack("<I", len(names))
    for n in names:
        e = n.encode()
        head += struct.pack("<H", len(e)) + e
    head += struct.pack("<I", 1)
    index = struct.pack("<iiQI", 0, 0, len(head) + 20, len(body))
    return head + index + body


def lines(bus):
    def route(km, stops, mins, itin, way, stop_pts):
        r = "R\t%s\t%d\t%d\t0\tEvery day\t\t%s\n" % (km, stops, mins, itin)
        r += "P\t" + " ".join("%g %g" % p for p in way) + "\n"
        return r + "S\t" + " ".join("%g %g" % p for p in stop_pts) + "\n"

    east = [(20, 200), (120, 205), (220, 200), (280, 205)]
    ends = [east[0], east[2], east[3]]
    t = "RLIN\t1\n# invented by make_synthetic_package.py\nORIGEM\t0.0\t0.0\n"
    t += "L\t%s\tALI\tLake - Hill\tbus\t0\t0\n" % bus + route(0.3, 3, 6, "Lake - Hill", east, ends)
    t += route(0.3, 3, 6, "Hill - Lake", east[::-1], ends[::-1])
    rail = [(10, 250), (150, 240), (290, 250)]
    t += "L\tT1\tALI\tShore tram\ttram\t0\t0\n" + route(0.3, 2, 5, "Shore - Mill", rail, [rail[0], rail[2]])
    t += "L\tR1\tALI\tRegional train\trail\t0\t0\n" + route(0.3, 2, 7, "Shore - Mill", rail, [rail[0], rail[2]])
    # one line in two eras: the form in force on the game's date is the one shown
    old = [(30, 100), (100, 150), (180, 160)]
    t += "L\t7\tALI\tOld route\tbus\t0\t19991231\n" + route(0.2, 2, 4, "A - B", old, [old[0], old[2]])
    t += "L\t7\tALI\tNew route\tbus\t20000101\t0\n" + route(0.2, 2, 4, "A - C", old[::-1], [old[2], old[0]])
    return t


MANIFEST = """# synthetic test package, invented data (see make_synthetic_package.py)
format = 1
name   = Synthetic test
grid   = 300
credit = Synthetic data made by a script, not a real place
tram_vehicles = vehicles/tram/
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--bus-line", default="1")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "surface.oms").write_bytes(surface())
    (out / "manifest.cfg").write_text(MANIFEST)
    (out / "lines.lin").write_text(lines(a.bus_line))
    print("wrote", out)


if __name__ == "__main__":
    main()
