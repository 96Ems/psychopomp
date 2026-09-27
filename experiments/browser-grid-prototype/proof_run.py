"""Validate current-run membership/provenance, independently of pixel tolerance."""
import hashlib
import json
from pathlib import Path

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def load_run(folder, group, variants):
    folder = Path(folder)
    run = json.loads((folder / "capture-run.json").read_text())
    inventory = folder / "proof-cases.json"
    manifest = json.loads(inventory.read_text())
    if (run.get("version") != 1 or manifest.get("version") != 1 or not run.get("runId")
            or run.get("status") != "complete" or run.get("group") != group
            or run.get("variants") != list(variants) or run.get("inventorySha256") != sha(inventory)):
        raise ValueError("Incomplete, stale or incompatible proof receipt")
    cases = [case for case in manifest["cases"] if case["group"] == group]
    if not cases or len({case["id"] for case in cases}) != len(cases):
        raise ValueError("Empty or duplicate case inventory")
    expected = {f'{prefix}{case["id"]}.png' for case in cases for prefix in variants}
    if any(Path(name).name != name for name in expected) or set(run["files"]) != expected:
        raise ValueError("Missing or unexpected captures in this run")
    for name, digest in run["files"].items():
        if sha(folder / name) != digest:
            raise ValueError(f"Changed/stale capture: {name}")
    if manifest["size"] != [1920, 1080]:
        raise ValueError("Proof requires the authored 1920x1080 source")
    return cases, run
