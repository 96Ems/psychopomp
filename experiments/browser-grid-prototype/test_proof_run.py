import json
import tempfile
import unittest
from pathlib import Path

from proof_run import load_run, sha


class ProofRunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        manifest = {"version": 1, "size": [1920, 1080], "cases": [
            {"id": "base", "group": "grid"}, {"id": "theme", "group": "grid"}]}
        (self.folder / "proof-cases.json").write_text(json.dumps(manifest))
        files = {}
        for name in ["base.png", "control-base.png", "theme.png", "control-theme.png"]:
            (self.folder / name).write_bytes(b"fixture bytes; pixel validation is separate")
            files[name] = sha(self.folder / name)
        self.run = {"version": 1, "runId": "fresh-test-run", "status": "complete", "group": "grid",
                    "variants": ["", "control-"], "files": files,
                    "inventorySha256": sha(self.folder / "proof-cases.json")}
        self.save()

    def save(self):
        (self.folder / "capture-run.json").write_text(json.dumps(self.run))

    def check(self):
        return load_run(self.folder, "grid", ("", "control-"))

    def test_complete_receipt(self):
        self.assertEqual(len(self.check()[0]), 2)

    def test_stale_png_cannot_replace_a_missing_current_capture(self):
        del self.run["files"]["control-theme.png"]
        self.save()
        with self.assertRaises(ValueError):
            self.check()

    def test_missing_file(self):
        (self.folder / "theme.png").unlink()
        with self.assertRaises(FileNotFoundError):
            self.check()

    def test_changed_bytes(self):
        (self.folder / "theme.png").write_bytes(b"old capture")
        with self.assertRaises(ValueError):
            self.check()

    def test_incomplete_run(self):
        self.run["status"] = "running"
        self.save()
        with self.assertRaises(ValueError):
            self.check()

    def test_changed_inventory(self):
        with (self.folder / "proof-cases.json").open("a") as file:
            file.write("\n")
        with self.assertRaises(ValueError):
            self.check()


if __name__ == "__main__":
    unittest.main()
