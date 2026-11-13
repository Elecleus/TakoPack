#!/usr/bin/env python3
"""Generate the yazi spec skeleton and apply the manual patch."""

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OUT = ROOT / "target" / "yazi-example"
OUT.mkdir(parents=True, exist_ok=True)
SKELETON = OUT / "yazi.spec"

with SKELETON.open("w") as output:
    subprocess.run(
        [
            "cargo", "run", "--quiet", "-p", "takopack", "--", "binary",
            "--url", "https://github.com/sxyazi/yazi/archive/refs/tags/v26.5.6.tar.gz",
            "--package", "yazi-fm",
        ],
        cwd=ROOT,
        stdout=output,
        check=True,
    )

subprocess.run(
    ["patch", "-p0", "--fuzz=2", "-i", str(HERE / "yazi.diff")],
    cwd=OUT,
    check=True,
)

filled = OUT / "yazi-26.5.6.spec"
SKELETON.rename(filled)
print(filled)
