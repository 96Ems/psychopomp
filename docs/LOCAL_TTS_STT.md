# Local TTS and word timings on Linux (no API, no macOS)

Upstream `scripts/narrate.ts` voices a scene with macOS `say` or with ElevenLabs /
Fish Audio, and gets its word timings from `mlx-whisper` — Apple Silicon only.
On a Linux box that means: no draft voice, a paid API for the final one, and no
way to time the visuals. This fork adds a fully local path.

## What is added

| piece | what it is | licence |
|---|---|---|
| `engine: "kokoro"` in `scripts/narrate.ts` | shells out to `scripts/kokoro-tts.py` | MIT |
| `scripts/kokoro-tts.py` | [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) through `kokoro-onnx` / onnxruntime, CPU | wrapper MIT, model Apache-2.0 |
| `scripts/stt-words.py` | word timings from [NVIDIA Parakeet-TDT 0.6B v2](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) through [`onnx-asr`](https://github.com/istupakov/onnx-asr) | MIT, model CC-BY-4.0 |
| `WHISPER_CMD` in `scripts/narrate.ts` | any transcriber that accepts the whisper CLI flags, instead of the hard-coded MLX one | — |
| one line in `crates/psychopomp-render/src/encode.rs` | `alimiter=...:latency=1` only exists from FFmpeg 5; on FFmpeg 4.4 every audio render aborted. Removed (harmless on newer FFmpeg) | — |

`stt-words.py` deliberately accepts the exact flags `narrate.ts` passes
(`--model --word-timestamps --output-format --output-dir --output-name AUDIO`) and
writes the same `<id>.json` shape, so it is a drop-in for the whisper step. It also
transcodes to 16 kHz WAV with ffmpeg first, because `onnx-asr` only reads RIFF WAV.

Parakeet emits **token-level** timestamps; the script merges sub-word pieces back
into words (a piece starting with the SentencePiece marker starts a new word). In
practice this tracks speech more tightly than Whisper's cross-attention estimate,
which matters because every visual beat in a Psychopomp scene hangs on an anchor
phrase: a 200 ms drift over a two-minute film is a visible desync.

## Install (Linux)

```sh
sudo apt-get install -y espeak-ng ffmpeg

python3 -m venv ~/.venvs/tts
~/.venvs/tts/bin/pip install kokoro-onnx soundfile onnx-asr
# optional, better English phonemisation (it pulls in torch):
# ~/.venvs/tts/bin/pip install "misaki[en]"

# Kokoro weights (~190 MB, download once)
mkdir -p ~/.local/share/kokoro && cd ~/.local/share/kokoro
curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.1/kokoro-v1.0.fp16.onnx
curl -LO https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.1/voices-v1.0.bin
# Parakeet is fetched from Hugging Face on first use (~600 MB, cached by onnx-asr)
```

## Use it

`scenes/<scene>/narration/script.json`:

```json
{
  "engine": "kokoro",
  "voice": "am_michael",
  "model": "kokoro-v1.0.fp16",
  "clips": [{ "id": "intro", "text": "..." }]
}
```

```sh
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
WHISPER_CMD="$HOME/.venvs/tts/bin/python $PWD/scripts/stt-words.py" \
STT_MODEL=nemo-parakeet-tdt-0.6b-v2 \
bun scripts/narrate.ts scenes/<scene>/narration/script.json
```

Then build the scene program and render as usual:

```sh
cargo run -p <scene-package>
cargo run --release -- plan validate scenes/<scene>/<scene>.reel.json
bun scripts/sheet.ts scenes/<scene>/<scene>.reel.json 4,24,45 --theme opencode --shutter --out output/sheet.png
cargo run --release -- plan render scenes/<scene>/<scene>.reel.json output/<scene>.mp4 --theme opencode
```

Only clips whose text/voice/engine changed are regenerated, so editing one line of
narration costs one Kokoro run and one Parakeet run.

## Environment variables

| var | default | meaning |
|---|---|---|
| `KOKORO_PYTHON` | `~/.venvs/tts/bin/python` | interpreter that has `kokoro_onnx` |
| `KOKORO_HELPER` | `scripts/kokoro-tts.py` | the helper script |
| `KOKORO_DIR` | `~/.local/share/kokoro` | weights directory |
| `KOKORO_MODEL` / `KOKORO_VOICES` | `kokoro-v1.0.fp16.onnx` / `voices-v1.0.bin` | files inside it |
| `WHISPER_CMD` | `uvx --from mlx-whisper mlx_whisper` | transcriber command (set it on Linux) |
| `STT_MODEL` | `nemo-parakeet-tdt-0.6b-v2` | onnx-asr model name |

## Verified on

Linux (Ubuntu 22.04, GTX 960M), Rust 1.98, bun 1.3.11, FFmpeg 4.4.2, Python 3.10,
onnxruntime CPU. Produced a 118 s narrated 1080p60 reel end to end, with every
anchor phrase checked against the Parakeet transcript before the scene was written.

Two things worth knowing when you pick anchors: the recogniser writes the product
name however it hears it (never anchor on it) and it can glue digits to words
(`carries107,000`), so never anchor on a phrase that touches a number.
