"""Exact same-session candidate/control pixel gate; requires Pillow."""
import json
import sys
from pathlib import Path
from PIL import Image, ImageChops
from proof_run import load_run

folder = Path(sys.argv[1])
results = []
cases, run = load_run(folder, "grid", ("", "control-"))
controls = [folder / f'control-{case["id"]}.png' for case in cases]
for control in controls:
    candidate = folder / control.name.removeprefix("control-")
    a, b = Image.open(control).convert("RGBA"), Image.open(candidate).convert("RGBA")
    if a.size != b.size or a.size != (1920, 1080):
        raise SystemExit(f"Size mismatch: {candidate}")
    diff = ImageChops.difference(a, b)
    histogram = diff.histogram()
    differing = a.width * a.height * 4 - sum(histogram[channel * 256] for channel in range(4))
    results.append({"frame": candidate.name, "differing_components": differing,
                    "maximum_byte_difference": max(high for _, high in diff.getextrema())})
(folder / "pixels.json").write_text(json.dumps(results, indent=2))
failures = [r for r in results if r["differing_components"]]
print(json.dumps({"frames": len(results), "identical": not failures, "failures": failures}))
if failures:
    raise SystemExit(1)
