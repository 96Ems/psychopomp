"""Compare isolated-browser captures against native bakes (Pillow + NumPy)."""
import json
import os
from pathlib import Path

import numpy as np
from PIL import Image
from proof_run import load_run, sha

root = Path(__file__).resolve().parents[2]
assets = Path(os.environ.get("DIAGRAM_ASSETS", root / "target/browser-diagram-site"))
output = Path(os.environ.get("DIAGRAM_OUTPUT", root / "output/diagram-gpu/browser"))
results = []
cases, run = load_run(output, "diagram", ("",))
assert run["wasmSha256"] == sha(assets / "pkg/kinograph_browser_grid_prototype_bg.wasm"), "proof belongs to a different build"
for case in cases:
    path = output / f'{case["id"]}.png'
    assert Path(case["native"]).name == case["native"]
    reference = assets / case["native"]
    assert run["native"].get(case["native"]) == sha(reference), "native reference changed after capture"
    browser = np.array(Image.open(path).convert("RGBA")).astype(int)
    native = np.array(Image.open(reference).convert("RGBA")).astype(int)
    assert browser.shape == native.shape == (1080, 1920, 4)
    delta = np.abs(browser - native)
    results.append({"frame": path.name, "components": int(np.count_nonzero(delta)),
                    "max": int(delta.max()), "mean": float(delta.mean())})
(output / "pixels.json").write_text(json.dumps(results, indent=2))
# Same shader/assets, different Metal compiler stacks. This tight local gate is
# not a universal cross-GPU guarantee. Dense blur produces one reviewed 84-pixel
# +/-1 color strip at a quantization boundary on a fading edge (no alpha change).
# Keep both the amplitude and extent bounded; shifted/reshaped ink must fail.
assert all(r["max"] <= 1 and r["components"] <= 128 for r in results), results
print(f'{len(results)} passed; {sum(r["max"] == 0 for r in results)} exact; '
      f'max component difference {max(r["max"] for r in results)}')
