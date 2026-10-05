#!/usr/bin/env python3
"""Cut each segment's shots from the simulator recordings into footage/<id>.mp4.

usage: cut.py <recordings dir>   (reads the shot list from `cargo run -- --shots`)
"""
import json, pathlib, subprocess, sys

scene = pathlib.Path(__file__).resolve().parent.parent
raw = pathlib.Path(sys.argv[1])
shots = json.loads(subprocess.check_output(
    ["cargo", "run", "-q", "-p", "psychopomp-ios-build-111", "--", "--shots"], cwd=scene))
out = scene / "footage"
out.mkdir(exist_ok=True)
for segment, parts in shots.items():
    args, filters = [], []
    for index, (recording, start, end) in enumerate(parts):
        # Simulator recordings only have frames when the screen changes, so
        # hold the last one and trim each shot to its exact length.
        length = end - start
        args += ["-ss", str(start), "-t", str(length + 1), "-i", str(raw / f"{recording}.mov")]
        filters.append(
            f"[{index}:v]fps=60:start_time=0,tpad=stop_mode=clone:stop_duration={length + 1},"
            f"trim=duration={length},setpts=PTS-STARTPTS,format=yuv420p[v{index}]")
    joined = "".join(f"[v{i}]" for i in range(len(parts)))
    graph = ";".join(filters) + f";{joined}concat=n={len(parts)}:v=1:a=0[out]"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *args, "-filter_complex", graph,
                    "-map", "[out]", "-c:v", "libx264", "-crf", "18", "-preset", "fast",
                    "-movflags", "+faststart", str(out / f"{segment}.mp4")], check=True)
    print(segment, len(parts), "shots")
