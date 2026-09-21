#!/usr/bin/env python3
"""Fetch only desktop members of the official Godot template archive using HTTP ranges."""

import argparse
import hashlib
import io
import json
import urllib.request
import zipfile
from pathlib import Path


class RemoteZip(io.RawIOBase):
    def __init__(self, url, size):
        self.url, self.size, self.pos = url, size, 0

    def seekable(self):
        return True

    def readable(self):
        return True

    def tell(self):
        return self.pos

    def seek(self, offset, whence=0):
        self.pos = offset if whence == 0 else (self.pos if whence == 1 else self.size) + offset
        return self.pos

    def read(self, n=-1):
        n = min(n if n >= 0 else self.size - self.pos, self.size - self.pos)
        if n <= 0:
            return b""
        chunks = []
        while n:
            count = min(n, 4 * 1024 * 1024)
            req = urllib.request.Request(
                self.url, headers={"Range": f"bytes={self.pos}-{self.pos + count - 1}"}
            )
            with urllib.request.urlopen(req, timeout=60) as r:
                if r.status != 206:
                    raise RuntimeError(
                        "Server did not honor Range; refusing an unexpected full download"
                    )
                data = r.read(count + 1)
            if len(data) != count:
                raise RuntimeError("Short or oversized range response")
            chunks.append(data)
            self.pos += count
            n -= count
        return b"".join(chunks)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="4.7.2")
    ap.add_argument("--output", default=".cache/godot-templates")
    a = ap.parse_args()
    release = json.load(
        urllib.request.urlopen(
            f"https://api.github.com/repos/godotengine/godot-builds/releases/tags/{a.version}-stable",
            timeout=30,
        )
    )
    asset = next(
        x
        for x in release["assets"]
        if x["name"] == f"Godot_v{a.version}-stable_export_templates.tpz"
    )
    target = Path(a.output).resolve()
    target.mkdir(parents=True, exist_ok=True)
    record = {
        "version": a.version,
        "upstream": asset["browser_download_url"],
        "upstream_archive_digest": asset.get("digest"),
        "files": {},
    }
    with zipfile.ZipFile(RemoteZip(asset["browser_download_url"], asset["size"])) as z:
        wanted = [
            "templates/macos.zip",
            "templates/windows_release_x86_64.exe",
            "templates/linux_release.x86_64",
            "templates/version.txt",
        ]
        for name in wanted:
            if name not in z.namelist():
                raise RuntimeError(f"Missing template {name}; available: {z.namelist()}")
            out = target / Path(name).name
            if not out.exists():
                print(
                    "Downloading",
                    name,
                    z.getinfo(name).compress_size,
                    "compressed bytes",
                    flush=True,
                )
                data = z.read(name)  # zipfile checks each selected member CRC.
                temp = out.with_suffix(out.suffix + ".part")
                temp.write_bytes(data)
                temp.replace(out)
            record["files"][out.name] = {
                "sha256": hashlib.sha256(out.read_bytes()).hexdigest(),
                "bytes": out.stat().st_size,
            }
            print("Ready", out.name, flush=True)
    (target / "provenance.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
