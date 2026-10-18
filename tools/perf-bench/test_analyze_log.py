"""`python3 -I tools/perf-bench/test_analyze_log.py` (from the repository root)."""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_log  # noqa: E402

LOG = """\
[2026-10-17T10:00:00Z INFO  omsi_app::settings] settings from /h/.depot/settings.cfg: msaa 8 af 8 ssao true shadows true (2048) navigator true graphics vanilla_plus post aa fxaa vsync false render scale auto boarding auto min object size 0.013 max object distance auto max fps 0
[2026-10-17T10:00:20Z INFO  omsi_app] stutter: frame 1000 took 1020 ms (render 15, tiles 900, render.encode 3; outside the stages 40 ms)
[2026-10-17T10:00:25Z INFO  omsi_app] stutter: frame 1300 took 1010 ms (render 15, tiles 880; outside the stages 30 ms)
[2026-10-17T10:00:30Z INFO  omsi_app] stutter: frame 1600 took 990 ms (render 20, tiles 870; outside the stages 25 ms)
[2026-10-17T10:00:35Z INFO  omsi_app] stutter: frame 1900 took 700 ms (render 20, gpu 650; outside the stages 5 ms)
[2026-10-17T10:00:40Z INFO  omsi_app] stutter: frame 2200 took 1000 ms (render 20, tiles 900; outside the stages 5 ms)
[2026-10-17T10:00:45Z INFO  omsi_app] stutter: frame 2500 took 1000 ms (render 20, tiles 900; outside the stages 5 ms)
[2026-10-17T10:01:40Z INFO  omsi_app] exit after 100 s: 3000 frames total (0 with the window hidden, not drawn), 30.0 fps average, 6 frames over 50 ms, worst 1020 ms
[2026-10-17T10:01:40Z INFO  omsi_app] profile render    : 8.5 ms/frame
[2026-10-17T10:01:40Z INFO  omsi_app] profile count blended_layers: 7 a frame
[2026-10-17T10:01:40Z INFO  omsi_app] profile gpu pass main        : 12.34 ms (500 frames measured)
[2026-10-17T10:01:40Z INFO  omsi_app] profile: since 15 s 33.0 ms wall and 41.5 ms CPU (all threads) per frame, 1.3 cores busy
[2026-10-17T10:01:40Z INFO  omsi_app] profile: window 1920x1080, scene drawn at 1920x1080, 8x MSAA
"""


class Analyze(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, "game.log")
        with open(self.path, "w") as f:
            f.write(LOG)

    def tearDown(self):
        self.dir.cleanup()

    def test_summary(self):
        r = analyze_log.parse(self.path)
        self.assertEqual((r["msaa"], r["vsync"], r["scene"]), (8, "false", "1920x1080"))
        self.assertEqual((r["over50"], r["worst_ms"], r["fps"]), (6, 1020, 30.0))
        self.assertEqual(len(r["stutters"]), 6)
        self.assertEqual(r["gpu_pass"], {"main": 12.34})
        self.assertEqual(r["counts"], {"blended_layers": 7.0})
        self.assertEqual(r["cpu_ms"], 41.5)

    def test_regular_stutters_and_dominant_stage(self):
        r = analyze_log.parse(self.path)
        self.assertEqual(r["interval_frames"], 300)
        self.assertEqual(r["interval_s"], 5.0)
        self.assertTrue(r["regular"])
        # sub-stages ("render.encode") never win over their parent
        self.assertEqual(r["dominant"], {"tiles": 5, "gpu": 1})

    def test_adjacent_slow_frames_are_one_freeze(self):
        frames = [1000, 1001, 1002, 1300, 1301, 1600, 1601, 1900, 1901, 2200, 2201, 2500, 2501]
        st = [{"frame": f, "ms": 100, "outside": 0, "stages": [("tiles", 90)], "at": None} for f in frames]
        out = analyze_log.summarise_stutters(st)
        self.assertEqual((out["events"], out["interval_frames"], out["regular"]), (6, 300, True))

    def test_table_has_one_column_per_log(self):
        out = analyze_log.table([analyze_log.parse(self.path)] * 2)
        self.assertIn("| MSAA | 8 | 8 |", out)
        self.assertIn("count blended_layers per frame", out)


if __name__ == "__main__":
    unittest.main()
