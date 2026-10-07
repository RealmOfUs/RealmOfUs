"""Fetch the small, explicitly selected CC0 art set used by the offline study.

Powered by Poly Haven: https://polyhaven.com/ . No accounts or paid assets.
Downloads are checked against the provider's MD5 and recorded with SHA-256.
"""

import concurrent.futures
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CACHE = ROOT / ".cache/walsh-offline"
ASSETS = CACHE / "assets"


def fetch(url, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["curl", "-L", "--fail", "--silent", "--show-error", "--retry", "2", url, "-o", str(path)],
        check=True,
    )


def download_assets(names, model_resolution="2k", provenance="asset-provenance.json"):
    """Fetch explicit assets into the shared cache; network or integrity failures are fatal."""
    jobs = []
    for name in names:
        manifest = CACHE / (name + ".json")
        if not manifest.exists():
            fetch("https://api.polyhaven.com/files/" + name, manifest)
        data = json.loads(manifest.read_text())
        if name in ["wool_boucle", "rough_linen", "white_oak_veneer"]:
            for channel in ["Diffuse", "Rough", "nor_gl"]:
                entry = data[channel]["4k"]["jpg"]
                jobs.append((name, ASSETS / name / Path(entry["url"]).name, entry))
        else:
            entry = data["blend"][model_resolution]["blend"]
            jobs.append((name, ASSETS / name / Path(entry["url"]).name, entry))
            for relative, dependency in entry.get("include", {}).items():
                jobs.append((name, ASSETS / name / relative, dependency))

    def download(job):
        name, path, entry = job
        if not path.exists() or hashlib.md5(path.read_bytes()).hexdigest() != entry["md5"]:
            fetch(entry["url"], path)
        payload = path.read_bytes()
        assert hashlib.md5(payload).hexdigest() == entry["md5"], path
        print("Verified", path.name, flush=True)
        return {
            "asset": name,
            "page": "https://polyhaven.com/a/" + name,
            "url": entry["url"],
            "path": str(path.relative_to(ROOT)),
            "license": "CC0-1.0",
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        }

    with concurrent.futures.ThreadPoolExecutor(5) as pool:
        records = list(pool.map(download, jobs))
    (CACHE / provenance).write_text(json.dumps(records, indent=2) + "\n")

    return records


def main():
    download_assets(
        [
            "wool_boucle",
            "rough_linen",
            "white_oak_veneer",
            "fern_02",
            "potted_plant_02",
            "ceramic_vase_03",
        ]
    )


if __name__ == "__main__":
    main()
