#!/usr/bin/env python3
"""Kokoro narration for a Scene Program clip: local TTS, no API.

Used by scripts/narrate.ts as `engine: "kokoro"`. Writes a raw WAV that narrate.ts
then loudness-normalizes and transcribes for word timings.

  kokoro-tts.py --text-file clip.txt --out raw.wav --voice am_michael [--speed 1.0]
"""
import argparse
import os
import sys

import soundfile as sf


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--text-file", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--voice", default="am_michael")
    ap.add_argument("--speed", type=float, default=1.0)
    ap.add_argument("--lang", default="en-us")
    args = ap.parse_args()

    model_dir = os.environ.get("KOKORO_DIR", os.path.expanduser("~/.local/share/kokoro"))
    model = os.path.join(model_dir, os.environ.get("KOKORO_MODEL", "kokoro-v1.0.fp16.onnx"))
    voices = os.path.join(model_dir, os.environ.get("KOKORO_VOICES", "voices-v1.0.bin"))

    from kokoro_onnx import Kokoro

    text = open(args.text_file, encoding="utf-8").read().strip()
    kokoro = Kokoro(model, voices)
    samples, rate = kokoro.create(text, voice=args.voice, speed=args.speed, lang=args.lang)
    sf.write(args.out, samples, rate)
    print(f"{os.path.basename(args.out)}: {len(samples) / rate:.2f}s voice={args.voice}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
