"""Check local Markdown link destinations in source documentation."""

import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]


def main():
    files = subprocess.check_output(
        ["rg", "--files", "-g", "*.md"], cwd=ROOT, text=True
    ).splitlines()
    broken = []
    for name in files:
        path = ROOT / name
        for raw in re.findall(r"!?\[[^\]]*\]\(([^\n)]*)\)", path.read_text()):
            target = raw.strip()
            if target.startswith("<"):
                target = target[1 : target.find(">")]
            else:
                target = target.split(' "', 1)[0].split(" '", 1)[0]
            parsed = urlsplit(target)
            if parsed.scheme or not parsed.path or target.startswith("/"):
                continue
            if not (path.parent / unquote(parsed.path)).exists():
                broken.append(f"{name}: {target}")
    if broken:
        raise SystemExit("\n".join(broken))
    print(f"MARKDOWN_LINKS_PASS {len(files)} files")


if __name__ == "__main__":
    main()
