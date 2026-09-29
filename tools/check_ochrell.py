"""Run the applicable existing surface checkpoints with Ochrell, preserving gates.

The Mixbox-specific T2 and T8 harness are deliberately not relabeled as Ochrell
tests. Integration/API checks are in tests/test_ochrell.py.
"""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT), str(ROOT / "tests")]
from oilpaint import mix
import checkpoints


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--mixer", choices=["ochrell", "rgb"], default="ochrell")
    p.add_argument("--out", type=Path, default=Path("out/ochrell-checkpoints"))
    a = p.parse_args()
    mix.set_backend(a.mixer)
    a.out.mkdir(parents=True, exist_ok=True)
    checkpoints.OUT = str(a.out)
    checkpoints.main(["t1", "t3", "t4", "t5", "t7"])
    (a.out / "conditions.json").write_text(json.dumps(dict(mixer=a.mixer, checkpoints=["t1","t3","t4","t5","t7"]), indent=2))
    return 0 if all(v["pass"] for v in checkpoints.RESULTS.values()) else 1


if __name__ == "__main__": raise SystemExit(main())
