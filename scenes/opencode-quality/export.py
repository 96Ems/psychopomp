"""Package Psychopomp-rendered final reveal frames, never a second slide renderer.

Run after `cargo run -p psychopomp-opencode-quality` and `cargo build --release`.
Requires Pillow. Generated output stays under output/opencode-quality.
"""
import json
from pathlib import Path
import shutil
import subprocess

from PIL import Image, ImageDraw


def main():
    root = Path(__file__).resolve().parents[2]
    plans = root / "target/opencode-quality"
    output = root / "output/opencode-quality"
    slides = output / "slides"
    slides.mkdir(parents=True, exist_ok=True)
    deck = json.loads((plans / "deck.json").read_text())
    renderer = root / "target/release/psychopomp"
    images = []
    for index, slide in enumerate(deck["slides"], 1):
        plan = slide["plan"]
        # A two-second final hold is beyond the authored zero-bounce reveals.
        seconds = plan["presentationSteps"][-1]["holdNanos"] / 1e9
        destination = slides / f"{index:02}.png"
        subprocess.run([
            str(renderer), "plan", "frame", str(plans / f'{plan["id"]}.json'),
            str(seconds), str(destination), "--theme", "neutral",
        ], cwd=root, check=True)
        with Image.open(destination) as image:
            if image.size != (1920, 1080):
                raise ValueError(f"Unexpected slide size: {image.size}")
            images.append(image.convert("RGB"))
    # 144 dpi makes 1920x1080 source pixels a 13 1/3 x 7 1/2-inch PDF page.
    images[0].save(output / "opencode-quality.pdf", save_all=True,
                   append_images=images[1:], resolution=144.,
                   title="OpenCode — Spend tokens once. Keep the protection.",
                   author="Kit", subject="Two-week quality loops proposal")
    sheet = Image.new("RGB", (3 * 640, ((len(images) + 2) // 3) * 388), "#151515")
    draw = ImageDraw.Draw(sheet)
    for index, image in enumerate(images):
        x, y = (index % 3) * 640, (index // 3) * 388
        sheet.paste(image.resize((640, 360), Image.Resampling.LANCZOS), (x, y))
        draw.text((x + 15, y + 364), f'{index + 1:02} / {deck["slides"][index]["title"]}', fill="#c8c8c8")
    sheet.save(output / "contact-sheet.jpg", quality=95)
    shutil.copyfile(root / "scenes/opencode-quality/PRESENTER.md", output / "PRESENTER.md")
    print(f"Exported {len(images)} Psychopomp slides, PDF, contact sheet, and presenter notes to {output}")


if __name__ == "__main__":
    main()
