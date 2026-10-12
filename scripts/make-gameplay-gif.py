#!/usr/bin/env python3
"""Encode UNO_GAMEPLAY_DIR frames. Requires Pillow (pip install Pillow)."""

import argparse
from pathlib import Path

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frames", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    paths = [args.frames / f"frame-{i:03}.png" for i in range(180)]

    # One shared palette prevents color flicker between animation frames.
    samples = Image.new("RGB", (240 * 6, 150 * 3))
    for i, path in enumerate(paths[::10]):
        with Image.open(path) as image:
            samples.paste(image.convert("RGB").resize((240, 150)),
                          ((i % 6) * 240, (i // 6) * 150))
    palette = samples.quantize(colors=256)
    frames = []
    for path in paths:
        with Image.open(path) as image:
            frame = image.convert("RGB").resize((960, 600), Image.Resampling.LANCZOS)
            frames.append(frame.quantize(palette=palette, dither=Image.Dither.NONE))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(
        args.output,
        save_all=True,
        append_images=frames[1:],
        # GIF delays use 10 ms units: alternate to average exactly 15 fps.
        duration=[70, 60, 70] * 60,
        loop=0,
        optimize=True,
        disposal=1,
    )
    print(f"{args.output}: {args.output.stat().st_size / 1024 / 1024:.2f} MiB")


if __name__ == "__main__":
    main()
