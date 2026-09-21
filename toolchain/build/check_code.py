"""Check Python lint/format and GDScript format using the pinned development tools."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main() -> None:
    scripts = [
        str(path.relative_to(ROOT))
        for path in sorted((ROOT / "client").rglob("*.gd"))
        if ".godot" not in path.parts
    ]
    commands = [
        ["cargo", "fmt", "--all", "--check"],
        ["cargo", "fmt", "--manifest-path", "fuzz/Cargo.toml", "--check"],
        ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        ["cargo", "doc", "-p", "rou-core", "-p", "rou-git", "--no-deps"],
        [sys.executable, "-m", "ruff", "check", "toolchain", "examples"],
        [sys.executable, "-m", "ruff", "format", "--check", "toolchain", "examples"],
        [sys.executable, "-m", "gdtoolkit.formatter", "--check", *scripts],
    ]
    for command in commands:
        subprocess.run(command, cwd=ROOT / "rou" if command[0] == "cargo" else ROOT, check=True)
    print("CODE_CHECKS_PASS")


if __name__ == "__main__":
    main()
