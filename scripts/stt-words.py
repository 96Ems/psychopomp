#!/usr/bin/env python3
"""Parakeet word timings for narrate.ts: local STT, Whisper-CLI compatible.

narrate.ts calls its transcriber with mlx-whisper's flags; this drops in for Linux
(parakeet-tdt on onnxruntime, no torch, no ffmpeg) and writes the same JSON shape:

  python scripts/stt-words.py --model ignored --word-timestamps True \
      --output-format json --output-dir WORK --output-name ID AUDIO

  WORK/ID.json = {"segments":[{"words":[{"word","start","end"}]}]}

Parakeet emits token-level timestamps; sub-word pieces are merged back into words
(a piece starting with the SentencePiece marker starts a new word).
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile


def riff_ready(path: str) -> bool:
    """onnx-asr reads RIFF WAV only; mp3 and friends must be transcoded first."""
    try:
        with open(path, "rb") as fh:
            return fh.read(4) == b"RIFF"
    except OSError:
        return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="")          # accepted and ignored
    ap.add_argument("--word-timestamps", dest="word_timestamps", default="True")
    ap.add_argument("--output-format", dest="output_format", default="json")
    ap.add_argument("--output-dir", dest="output_dir", required=True)
    ap.add_argument("--output-name", dest="output_name", required=True)
    ap.add_argument("audio")
    a = ap.parse_args()

    import onnx_asr

    source = a.audio
    scratch: str | None = None
    if not riff_ready(source):
        scratch = tempfile.mktemp(suffix=".wav")
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", source, "-ar", "16000", "-ac", "1", scratch],
            check=True,
        )
        source = scratch

    name = os.environ.get("STT_MODEL", "nemo-parakeet-tdt-0.6b-v2")
    model = onnx_asr.load_model(name).with_timestamps()
    try:
        result = model.recognize(source)
    finally:
        if scratch:
            os.unlink(scratch)

    tokens = list(getattr(result, "tokens", None) or [])
    stamps = list(getattr(result, "timestamps", None) or [])
    if not tokens or len(stamps) != len(tokens):
        print(f"stt-words: unexpected result {type(result).__name__} fields={dir(result)}", file=sys.stderr)
        return 2

    words: list[dict] = []
    for token, stamp in zip(tokens, stamps):
        start, end = (stamp if isinstance(stamp, (list, tuple)) else (stamp, stamp))
        piece = str(token)
        clean = piece.replace("\u2581", "").strip()
        if not clean:
            continue
        starts_word = piece.startswith("\u2581") or piece.startswith(" ") or not words
        if starts_word:
            words.append({"word": clean, "start": float(start), "end": float(end)})
        else:
            words[-1]["word"] += clean
            words[-1]["end"] = float(end)

    os.makedirs(a.output_dir, exist_ok=True)
    out = os.path.join(a.output_dir, f"{a.output_name}.json")
    with open(out, "w", encoding="utf-8") as fh:
        json.dump({"segments": [{"words": words}]}, fh)
    print(f"stt-words: {os.path.basename(a.audio)} -> {len(words)} words", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
