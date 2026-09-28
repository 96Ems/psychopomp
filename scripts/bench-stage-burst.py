#!/usr/bin/env python3
"""Pixel continuity benchmark. Requires Pillow and numpy; build release first."""
import argparse
import json
import pathlib
import statistics
import subprocess

import numpy as np
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument("label")
parser.add_argument("--runs", type=int, default=7)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parent.parent
out = root / "output" / "burst-research" / args.label
out.mkdir(parents=True, exist_ok=True)
reel = json.loads((root / "scenes/pr-walkthrough/pr-50825.reel.json").read_text())
stage = reel["segments"][0]["plan"]["actors"][0]
stage["data"]["elements"] = [e for e in stage["data"]["elements"]
                              if e["id"] in ("service", "client", "link")]
plan = dict(version=2, id="burst-continuity", durationNanos=4_000_000_000,
            actors=[stage], semanticTargets=[], continuousChannels=[],
            stateChannels=[], cues=[], media=[])


def frame(age, name):
    plan["continuousChannels"] = [dict(id="stage.service.burst", actorId="stage",
                                     property="service.burst", initial=age, events=[])]
    source = out / f"{name}.json"
    source.write_text(json.dumps(plan))
    image = out / f"{name}.png"
    subprocess.run([str(root / "target/release/kinograph"), "plan", "frame",
                    str(source), "2", str(image), "--theme", "opencode"],
                   cwd=root, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    return np.asarray(Image.open(image).convert("RGB"), dtype=np.float32)


measurements = []
near_lifts = []
far_lifts = []
edge_steps = []
for run in range(args.runs + 1):
    intact = frame(-1, "intact")
    entered = frame(0, "entered")
    hot = frame(0.45, "hot")
    error = np.abs(intact - entered)[300:645, 880:1240]
    if run:
        measurements.append(float(error.mean()))
        near_lifts.append(float((hot - intact)[380:400, 581:584, 0].mean()))
        far_lifts.append(float((hot - intact)[380:400, 229:232, 0].mean()))
        edge_steps.append(float(np.percentile(np.abs(np.diff(hot[429:486, 750:790], axis=1)), 99)))

median = statistics.median(measurements)
result = dict(label=args.label, metric="burst_entry_mae_rgb8", runs=args.runs,
              median=median, mad=statistics.median(abs(x - median) for x in measurements),
              measurements=measurements,
              near_rim_red_lift=statistics.median(near_lifts),
              far_rim_red_lift=statistics.median(far_lifts),
              volume_edge_step_p99=statistics.median(edge_steps),
              changed_pixel_fraction=float((error.max(axis=2) > 1).mean()))
(out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
print(f"METRIC burst_entry_mae_rgb8={median:.6f}")
