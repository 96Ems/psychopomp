"""Draw the High Council's 1996 CD-ROM screen as image sequences.

Reads output/high-council/frames.json (written by the Scene Program) and the
advisor portraits, and writes every frame the council segments cut between:
the chrome, gold-lit bezels, Sprite Sheets, agenda pages, speech-box pages,
shout banners, and a looping grain. Run from the repository root:

    python3 scenes/high-council/scripts/frames.py
"""

import json
import os
import random
import shutil
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "output/high-council"
SPRITES = Path(os.environ.get("HIGH_COUNCIL_SPRITES", "/Users/kit/code/home/artifacts/2026-10-04-high-council/sprites"))
FONTS = Path("/System/Library/Fonts/Supplemental")
SPEC = json.loads((OUT / "frames.json").read_text())

MOUTHS = ["rest", "closed", "mbp", "open", "wide", "round", "fv"]
STATIC_FRAMES = 3


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size)


SERIF = "Times New Roman Bold.ttf"
SERIF_ITALIC = "Times New Roman Bold Italic.ttf"
SANS = "Tahoma Bold.ttf"
MONO = "Courier New Bold.ttf"

FACE = (192, 192, 192)
LIGHT = (255, 255, 255)
SHADOW = (128, 128, 128)
DARK = (0, 0, 0)
GOLD = (255, 214, 92)
PARCHMENT = (232, 220, 178)
STONE = (58, 52, 46)


def crisp(draw):
    # Aliased text: no smoothing, like a 1996 GDI font.
    draw.fontmode = "1"
    return draw


def bevel(draw, box, raised=True, width=3, light=LIGHT, shadow=SHADOW, outer=DARK):
    x0, y0, x1, y1 = box
    top, bottom = (light, shadow) if raised else (shadow, light)
    draw.rectangle(box, outline=outer)
    for i in range(1, width + 1):
        draw.line([(x0 + i, y1 - i), (x0 + i, y0 + i), (x1 - i, y0 + i)], fill=top)
        draw.line([(x0 + i, y1 - i), (x1 - i, y1 - i), (x1 - i, y0 + i)], fill=bottom)


def button(draw, box, text, fill=FACE, color=DARK, size=18):
    draw.rectangle(box, fill=fill)
    bevel(draw, box, width=2)
    f = font(SANS, size)
    w = draw.textlength(text, font=f)
    x0, y0, x1, y1 = box
    draw.text(((x0 + x1 - w) / 2, (y0 + y1) / 2), text, font=f, fill=color, anchor="lm")


def stone_texture(size, seed=1, base=STONE):
    rng = random.Random(seed)
    w, h = size
    small = Image.new("RGB", (w // 4, h // 4))
    px = small.load()
    for y in range(h // 4):
        for x in range(w // 4):
            v = rng.randint(-10, 10)
            dot = 14 if (x + y) % 2 == 0 else 0
            px[x, y] = tuple(max(0, min(255, c + v - dot)) for c in base)
    return small.resize(size, Image.NEAREST)


def gradient(size, left, right):
    w, h = size
    img = Image.new("RGB", size)
    d = ImageDraw.Draw(img)
    steps = 32
    for i in range(steps):
        t = i / (steps - 1)
        c = tuple(int(a + (b - a) * t) for a, b in zip(left, right))
        d.rectangle([w * i // steps, 0, w * (i + 1) // steps, h], fill=c)
    return img


def box_of(center, size):
    (cx, cy), (w, h) = center, size
    return [int(cx - w / 2), int(cy - h / 2), int(cx + w / 2) - 1, int(cy + h / 2) - 1]


def chrome():
    img = stone_texture((1920, 1080), seed=3, base=(34, 32, 30))
    d = crisp(ImageDraw.Draw(img))
    window = [10, 10, 1909, 1069]
    d.rectangle(window, fill=(46, 42, 38))
    bevel(d, window, width=4, light=(120, 110, 96), shadow=(18, 16, 14))
    # Title bar.
    bar = [22, 22, 1897, 78]
    img.paste(gradient((bar[2] - bar[0], bar[3] - bar[1]), (92, 16, 12), (176, 120, 30)), (bar[0], bar[1]))
    bevel(d, bar, raised=False, width=2, light=(210, 170, 90), shadow=(40, 10, 8))
    d.text((40, 50), "HIGH COUNCIL", font=font(SERIF, 34), fill=GOLD, anchor="lm")
    chip = [330, 36, 790, 64]
    d.rectangle(chip, fill=(0, 20, 0))
    bevel(d, chip, raised=False, width=1)
    d.text((342, 50), "anomalyco/opencode · v2 · 320x240 CINEPAK", font=font(MONO, 17), fill=(80, 255, 120), anchor="lm")
    x = 1884
    for text, fill, color in [("Sound: ON", FACE, DARK), ("Sprites", FACE, DARK), ("Anarchy!", (178, 34, 34), LIGHT), ("Consult Council", FACE, DARK)]:
        w = int(d.textlength(text, font=font(SANS, 18))) + 36
        button(d, [x - w, 32, x, 68], text, fill=fill, color=color)
        x -= w + 10
    # Nameplates beneath each well.
    for panel in SPEC["panels"]:
        cx, cy = panel["center"]
        bw = SPEC["bezel"][0]
        top = int(cy + SPEC["bezel"][1] / 2) + 2
        plate = [int(cx - bw / 2), top, int(cx + bw / 2) - 1, top + 30]
        d.rectangle(plate, fill=(28, 24, 20))
        bevel(d, plate, width=2, light=(110, 96, 70), shadow=(10, 8, 6))
        d.text((plate[0] + 14, top + 16), panel["label"], font=font(SERIF, 24), fill=GOLD, anchor="lm")
        d.text((plate[2] - 12, top + 16), "ADVISOR", font=font(MONO, 15), fill=(170, 160, 130), anchor="rm")
    # Agenda well and speech well.
    for spec in (SPEC["agenda"], SPEC["speech"]):
        b = box_of(spec["center"], spec["size"])
        bevel(d, [b[0] - 5, b[1] - 5, b[2] + 5, b[3] + 5], raised=False, width=3, light=(120, 110, 96), shadow=(14, 12, 10))
    img.save(OUT / "chrome-0.png")


def bezels():
    w, h = map(int, SPEC["bezel"])
    for panel in SPEC["panels"]:
        for lit in (0, 1):
            img = stone_texture((w, h), seed=7, base=(150, 120, 60) if lit else (88, 80, 70))
            d = ImageDraw.Draw(img)
            bevel(d, [0, 0, w - 1, h - 1], width=6, light=(255, 230, 140) if lit else (150, 140, 120), shadow=(70, 40, 0) if lit else (24, 20, 16))
            inner = [8, 8, w - 9, h - 9]
            bevel(d, inner, raised=False, width=3, light=(255, 240, 180) if lit else (120, 110, 96), shadow=DARK)
            img.save(OUT / f"bezel-{panel['slug']}-{lit}.png")


def sprites():
    for panel in SPEC["panels"]:
        slug = panel["slug"]
        src = SPRITES / slug / "lofi-4x3"
        out = OUT / "sprites" / slug
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        patch = {}
        for mouth in MOUTHS[1:]:
            name = "closed" if mouth == "mbp" and not (src / "patch-mouth-mbp.png").exists() else mouth
            patch[mouth] = Image.open(src / f"patch-mouth-{name}.png").convert("RGBA")
        blink = Image.open(src / "patch-blink.png").convert("RGBA")
        index = 0
        for expression in SPEC["expressions"]:
            face = Image.open(src / f"{expression}.png").convert("RGBA")
            frames = [face] + [Image.alpha_composite(face, patch[m]) for m in MOUTHS[1:]] + [Image.alpha_composite(face, blink)]
            for frame in frames:
                lofi(frame).save(out / f"{index:02d}.png")
                index += 1
        rng = random.Random(slug)
        for _ in range(STATIC_FRAMES):
            noise = Image.new("L", (160, 120))
            noise.putdata([rng.randint(0, 255) for _ in range(160 * 120)])
            noise = noise.resize((640, 480), Image.NEAREST).convert("RGB")
            noise = Image.blend(noise, Image.new("RGB", noise.size, (40, 60, 70)), 0.3)
            noise.save(out / f"{index:02d}.png")
            index += 1


def lofi(frame):
    # Digitized video: 15-bit color and chunky 2x pixels.
    rgb = frame.convert("RGB")
    rgb = rgb.point(lambda v: (v >> 3) << 3)
    return rgb.resize((640, 480), Image.NEAREST)


AGENDA = [
    ("TODAY'S AGENDA", [
        ("Simplifier", "Delete the vendored Copilot SDK fork"),
        ("Performance", "47 SQLite transactions per step"),
        ("Bug Hunter", "Three bugs in compaction"),
        ("Contrarian", "Objects to all of the above"),
    ], None),
    ("SIMPLIFIER", [("Pitch", "Route Copilot through the native @opencode/ai protocols")], [
        "packages/core/src/",
        " github-copilot/",
        "25 files · 4,335 lines",
        "from @ai-sdk/openai-",
        " compatible + openai",
        "",
        "CAVEAT reasoning_opaque",
        " round-trips (chat/",
        " ...language-model:467)",
    ]),
    ("PERFORMANCE", [("Pitch", "Keep the hot assistant message in memory during a step")], [
        "bus.ts:314",
        " 1 transaction/event",
        "projector.ts:230",
        " read+decode+rewrite",
        "",
        "7 + 4 x 10 tools",
        " = 47 events/step",
        "",
        "#33356 db 13GB+",
    ]),
    ("BUG HUNTER", [("Pitch", "Disarm the three-bug compaction trap")], [
        "compaction.ts:586",
        " tools, no toolChoice",
        " -> #53242",
        "compaction.ts:716",
        " skips synthetic",
        " -> #43250",
        "compaction.ts:780",
        " keeps reasoning",
        " -> #51818",
    ]),
    ("ROYAL DECREES", [
        ("[ ]", "Banish the vendored Copilot fork"),
        ("[ ]", "Keep the hot message in memory"),
        ("[ ]", "Crush the three compaction beetles"),
    ], None),
]


def wrap(draw, text, f, width):
    lines, line = [], ""
    for word in text.split():
        trial = f"{line} {word}".strip()
        if draw.textlength(trial, font=f) > width and line:
            lines.append(line)
            line = word
        else:
            line = trial
    return lines + ([line] if line else [])


def agenda():
    w, h = map(int, SPEC["agenda"]["size"])
    for index, (title, items, evidence) in enumerate(AGENDA):
        img = Image.new("RGB", (w, h), FACE)
        d = crisp(ImageDraw.Draw(img))
        bevel(d, [0, 0, w - 1, h - 1], width=3)
        img.paste(gradient((w - 16, 40), (0, 0, 128), (16, 132, 208)), (8, 8))
        d.text((18, 28), title, font=font(SANS, 22), fill=LIGHT, anchor="lm")
        y = 70
        for head, body in items:
            if evidence is None and head not in ("[ ]",):
                d.text((20, y), head, font=font(SERIF, 24), fill=(120, 0, 0))
                y += 30
                for line in wrap(d, body, font(SERIF, 22), w - 50):
                    d.text((34, y), line, font=font(SERIF, 22), fill=DARK)
                    y += 26
                y += 18
            elif head == "[ ]":
                b = [20, y + 2, 44, y + 26]
                d.rectangle(b, fill=LIGHT)
                bevel(d, b, raised=False, width=2)
                for line in wrap(d, body, font(SERIF, 23), w - 80):
                    d.text((56, y), line, font=font(SERIF, 23), fill=DARK)
                    y += 28
                y += 26
            else:
                for line in wrap(d, body, font(SERIF, 26), w - 40):
                    d.text((20, y), line, font=font(SERIF, 26), fill=DARK)
                    y += 31
                y += 16
        if evidence:
            d.text((20, y), "EVIDENCE INSPECTOR", font=font(SANS, 16), fill=(60, 60, 60))
            y += 24
            box = [16, y, w - 17, h - 70]
            d.rectangle(box, fill=(0, 12, 0))
            bevel(d, box, raised=False, width=2)
            ty = y + 14
            for line in evidence:
                d.text((28, ty), line, font=font(MONO, 20), fill=(90, 255, 120))
                ty += 27
        if index == 4:
            button(d, [20, h - 120, 170, h - 76], "Ship It", fill=GOLD, size=20)
            button(d, [190, h - 120, 360, h - 76], "Anarchy!", fill=(178, 34, 34), color=LIGHT, size=20)
        button(d, [20, h - 58, w - 21, h - 18], "Dig Deeper", size=18)
        img.save(OUT / f"agenda-{index}.png")


def speech():
    w, h = map(int, SPEC["speech"]["size"])
    out = OUT / "speech"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    base = Image.new("RGB", (w, h), PARCHMENT)
    rng = random.Random(5)
    px = base.load()
    for _ in range(9000):
        x, y = rng.randrange(w), rng.randrange(h)
        c = px[x, y]
        px[x, y] = tuple(max(0, v - rng.randint(8, 30)) for v in c)
    d = ImageDraw.Draw(base)
    bevel(d, [0, 0, w - 1, h - 1], width=4, light=(255, 248, 220), shadow=(150, 130, 90))
    base.save(out / "0000.png")
    label_font = font(SERIF_ITALIC, 30)
    for index, page in enumerate(SPEC["speech"]["pages"], start=1):
        img = base.copy()
        d = crisp(ImageDraw.Draw(img))
        label = page["label"] + ":"
        d.text((28, 24), label, font=label_font, fill=(120, 0, 0))
        size = 38
        while True:
            f = font(SERIF, size)
            lines = layout_words(d, page["words"], f, w - 60)
            if len(lines) <= 2 or size <= 28:
                break
            size -= 2
        y = 66 if len(lines) <= 2 else 58
        line_height = size + 10
        for line in lines:
            x = 30
            for i, word in line:
                ww = d.textlength(word, font=f)
                if i == page["highlight"]:
                    d.rectangle([x - 5, y - 2, x + ww + 5, y + size + 4], fill=(110, 10, 10))
                    d.text((x, y), word, font=f, fill=GOLD)
                else:
                    d.text((x, y), word, font=f, fill=(20, 14, 8))
                x += ww + d.textlength(" ", font=f)
            y += line_height
        img.save(out / f"{index:04d}.png")


def layout_words(d, words, f, width):
    lines, line, x = [], [], 0
    space = d.textlength(" ", font=f)
    for i, word in enumerate(words):
        ww = d.textlength(word, font=f)
        if line and x + ww > width:
            lines.append(line)
            line, x = [], 0
        line.append((i, word))
        x += ww + space
    return lines + ([line] if line else [])


def banners():
    w, h = map(int, SPEC["banner"]["size"])
    out = OUT / "banners"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    Image.new("RGBA", (w, h), (0, 0, 0, 0)).save(out / "00.png")
    for index, text in enumerate(SPEC["banner"]["texts"], start=1):
        img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        d = crisp(ImageDraw.Draw(img))
        size = 44
        while d.textlength(text, font=font(SERIF, size)) > w - 60:
            size -= 2
        f = font(SERIF, size)
        tw = d.textlength(text, font=f)
        bw = int(tw) + 60
        box = [(w - bw) // 2, 6, (w + bw) // 2, h - 7]
        d.rectangle(box, fill=(170, 18, 18))
        bevel(d, box, width=4, light=(255, 120, 90), shadow=(70, 0, 0))
        for dx, dy in [(-2, 0), (2, 0), (0, -2), (0, 2), (2, 2)]:
            d.text((w / 2 + dx, h / 2 + dy), text, font=f, fill=DARK, anchor="mm")
        d.text((w / 2, h / 2), text, font=f, fill=(255, 236, 80), anchor="mm")
        img.save(out / f"{index:02d}.png")


def grain():
    out = OUT / "grain"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    rng = random.Random(9)
    w, h = 960, 540
    for index in range(6):
        alpha = Image.new("L", (w // 2, h // 2))
        alpha.putdata([rng.randint(0, 52) for _ in range(w * h // 4)])
        alpha = alpha.resize((w, h), Image.NEAREST)
        lines = Image.new("L", (w, h), 0)
        ld = ImageDraw.Draw(lines)
        for y in range(index % 2, h, 2):
            ld.line([(0, y), (w, y)], fill=34)
        a = Image.new("L", (w, h))
        a.putdata([min(255, p + q) for p, q in zip(alpha.getdata(), lines.getdata())])
        tint = rng.choice([(0, 0, 0), (20, 20, 20), (0, 0, 0)])
        img = Image.new("RGBA", (w, h), tint + (0,))
        img.putalpha(a)
        img.save(out / f"{index}.png")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    chrome()
    bezels()
    sprites()
    agenda()
    speech()
    banners()
    grain()
    print(f"speech pages: {len(SPEC['speech']['pages'])}, banners: {len(SPEC['banner']['texts'])}")
