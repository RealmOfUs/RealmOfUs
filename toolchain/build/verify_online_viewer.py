"""Verify online bundle materialization, cache reuse, and isolated transport failures."""

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import urllib.parse
from contextlib import contextmanager
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def viewer_binary() -> Path:
    """Return the packaged viewer executable for the current host."""
    if sys.platform == "darwin":
        path = ROOT / "dist/viewer/macos/Realm of Us.app/Contents/MacOS/Realm of Us"
    elif sys.platform == "win32":
        path = ROOT / "dist/viewer/windows/Realm of Us.exe"
    else:
        path = ROOT / "dist/viewer/linux/Realm of Us.x86_64"
    if not path.is_file():
        raise FileNotFoundError(f"missing packaged viewer: {path}; run `make viewer`")
    return path


def read_bundle(bundle: Path) -> dict:
    """Read root-bound pages and return expected version and tier payload sets."""
    root = json.loads((bundle / "realm.json").read_text())
    version = json.loads((bundle / "version.json").read_text())
    entities = []
    pages = []
    for descriptor in root["chunks"].values():
        page = bundle / descriptor["page"]
        pages.append(page)
        entities.extend(json.loads(page.read_text())["entities"])
    lod_payloads = {}
    full_payloads = {}
    tiers = {}
    for entity in entities:
        tiers[entity["id"]] = {entity["lod1"]["sha256"], entity["asset"]["sha256"]}
        for tier, target in (("lod1", lod_payloads), ("asset", full_payloads)):
            asset = entity[tier]
            target[Path(asset["path"]).name] = asset["bytes"]
            for texture in entity["textures"][tier]:
                target[Path(texture["path"]).name] = texture["bytes"]
    return {
        "root": root,
        "version": version,
        "entities": entities,
        "pages": pages,
        "lod_payloads": lod_payloads,
        "full_payloads": full_payloads,
        "tiers": tiers,
    }


def run_viewer(
    binary: Path,
    address: str,
    cache: Path,
    output: Path,
    *,
    expected: int = 0,
    seconds: float = 0.5,
    at: str = "8,2,8",
    exit_on_error: bool = False,
) -> dict:
    """Run the standalone app with no authoring tools on PATH and return its report."""
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    log = output.with_suffix(".log")
    command = [
        str(binary),
        "--headless",
        "--",
        f"--benchmark={seconds}",
        "--balanced",
        "--no-recent",
        f"--url={address}",
        f"--cache={cache}",
        f"--at={at}",
        f"--report={output}",
    ]
    if exit_on_error:
        command.append("--exit-on-error")
    environment = os.environ.copy()
    with tempfile.TemporaryDirectory(prefix="rou-online-empty-path-") as empty_path:
        environment["PATH"] = empty_path
        result = subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            capture_output=True,
            text=True,
            timeout=120,
        )
    log.write_text(result.stdout + "\n" + result.stderr)
    if result.returncode != expected:
        raise RuntimeError(
            f"online viewer exit {result.returncode}, expected {expected}; see {log}"
        )
    if "ERROR:" in log.read_text():
        raise RuntimeError(f"online viewer logged an engine error; see {log}")
    if not output.is_file():
        raise RuntimeError(f"online viewer wrote no report; see {log}")
    return json.loads(output.read_text())


def check_cache_layout(manifest: Path) -> None:
    """Mirror the bundle host boundary after all on-demand writes have completed."""
    root = manifest.parent
    if any(path.is_symlink() for path in [root, *root.rglob("*")]):
        raise AssertionError("online cache contains a symlink")
    if {path.name for path in root.iterdir()} - {"realm.json", "version.json", "chunks", "objects"}:
        raise AssertionError("online cache contains an unrelated root path")
    if not (root / "chunks").is_dir() or not (root / "objects").is_dir():
        raise AssertionError("online cache is missing required directories")
    for path in (root / "objects").iterdir():
        suffix = path.suffix.removeprefix(".")
        if not path.is_file() or suffix not in {"glb", "png", "jpg"} or len(path.stem) != 64:
            raise AssertionError(f"invalid cached object path: {path}")


class FaultHandler(SimpleHTTPRequestHandler):
    """Serve a bundle with one optional missing page or truncated object."""

    missing_path = ""
    truncated_path = ""

    def log_message(self, _format, *_args):
        pass

    def do_GET(self):
        relative = urllib.parse.urlsplit(self.path).path.lstrip("/")
        if relative == self.missing_path:
            self.send_error(404)
            return
        if relative == self.truncated_path:
            path = Path(self.translate_path(self.path))
            data = path.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data[: max(1, len(data) // 3)])
            self.wfile.flush()
            try:
                self.connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            self.connection.close()
            return
        super().do_GET()


@contextmanager
def serve(bundle: Path, *, missing: str = "", truncated: str = ""):
    """Serve a bundle on an ephemeral loopback port."""

    class Handler(FaultHandler):
        missing_path = missing
        truncated_path = truncated

    server = ThreadingHTTPServer(
        ("127.0.0.1", 0), partial(Handler, directory=str(bundle.resolve()))
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/realm.json"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=10)


def verify(address: str, bundle: Path, output: Path, binary: Path | None = None) -> dict:
    """Run the complete hosted and static online-viewer contract."""
    binary = binary or viewer_binary()
    bundle = bundle.resolve()
    output = output.resolve()
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    expected = read_bundle(bundle)
    version = expected["version"]

    hosted_cache = output / "hosted-cache"
    first = run_viewer(binary, address, hosted_cache, output / "hosted-first.json", seconds=1)
    assert first["commit_oid"] == version["commit_oid"]
    assert first["view_root_sha256"] == version["view_root_sha256"]
    assert first["view_validation"] == "local-trusted"
    assert not first["rejected"] and not first["asset_failures"] and not first["page_errors"]
    assert first["network"]["downloaded_bytes"] > 0
    assert first["first_ready_object_downloaded_bytes"] <= sum(expected["lod_payloads"].values())
    assert first["cache_layout_error"] == ""
    manifest = Path(first["realm"])
    check_cache_layout(manifest)

    order = first["network"]["download_order"]
    lod_names = set(expected["lod_payloads"])
    full_only = set(expected["full_payloads"]) - lod_names
    if full_only and any(name in full_only for name in order):
        first_full = next(index for index, name in enumerate(order) if name in full_only)
        missing_lod = {
            name for name in lod_names if name.endswith(".glb") and name not in order[:first_full]
        }
        assert not missing_lod, f"full tier started before lod1 set: {sorted(missing_lod)}"

    second = run_viewer(binary, address, hosted_cache, output / "hosted-second.json")
    assert second["network"]["downloaded_bytes"] == 0
    assert second["network"]["requests"] == 0
    # Both runs mount the same entities from the cache. With large full tiers the tier
    # reached inside the benchmark window may differ, so compare entity sets and require
    # every mounted SHA to be a catalogue tier of its entity.
    assert set(second["loaded_assets"]) == set(first["loaded_assets"])
    assert all(sha in expected["tiers"][entity] for entity, sha in second["loaded_assets"].items())

    latest = run_viewer(
        binary,
        address.rsplit("@", 1)[0],
        hosted_cache,
        output / "hosted-latest.json",
    )
    assert latest["commit_oid"] == first["commit_oid"]
    assert latest["view_root_sha256"] == first["view_root_sha256"]
    assert latest["network"]["tag"] == address.rsplit("@", 1)[1]

    target = next(path for path in sorted(manifest.parent.joinpath("objects").glob("*.glb")))
    damaged = bytearray(target.read_bytes())
    damaged[min(20, len(damaged) - 1)] ^= 0xFF
    target.write_bytes(damaged)
    shared = hosted_cache / version["realm_id"] / "_objects" / target.name
    if shared.is_file():
        shared.write_bytes(damaged)
    repaired = run_viewer(binary, address, hosted_cache, output / "hosted-repaired.json")
    assert target.name in repaired["network"]["download_order"]
    assert repaired["network"]["object_downloaded_bytes"] >= target.stat().st_size
    assert not repaired["rejected"] and not repaired["asset_failures"]

    missing = run_viewer(
        binary,
        address.rsplit("@", 1)[0] + "@does-not-exist",
        output / "missing-tag-cache",
        output / "missing-tag.json",
        expected=2,
        exit_on_error=True,
    )
    assert "does not exist" in missing["error"]
    credentials = run_viewer(
        binary,
        "rou://user:secret@localhost:3000/realm/garden@v1",
        output / "credential-cache",
        output / "credential-address.json",
        expected=2,
        exit_on_error=True,
    )
    assert "credentials" in credentials["error"]

    with serve(bundle) as static_address:
        static = run_viewer(
            binary, static_address, output / "static-cache", output / "static.json", seconds=1
        )
    assert static["commit_oid"] == first["commit_oid"]
    assert static["view_root_sha256"] == first["view_root_sha256"]
    assert set(static["loaded_assets"]) == set(first["loaded_assets"])
    assert all(sha in expected["tiers"][entity] for entity, sha in static["loaded_assets"].items())
    assert not static["rejected"] and not static["asset_failures"]

    missing_page = expected["pages"][0].relative_to(bundle).as_posix()
    with serve(bundle, missing=missing_page) as static_address:
        partial_world = run_viewer(
            binary,
            static_address,
            output / "missing-page-cache",
            output / "missing-page.json",
        )
    assert partial_world["page_errors"]
    assert partial_world["loaded_assets"], "one missing page blocked every other page"

    interrupted_name = next(name for name in expected["lod_payloads"] if name.endswith(".glb"))
    with serve(bundle, truncated="objects/" + interrupted_name) as static_address:
        interrupted = run_viewer(
            binary,
            static_address,
            output / "interrupted-cache",
            output / "interrupted.json",
            expected=1,
        )
    assert interrupted["asset_failures"]
    interrupted_manifest = Path(interrupted["realm"])
    assert not (interrupted_manifest.parent / "objects" / interrupted_name).exists()
    assert interrupted["loaded_assets"], "one interrupted object blocked every other object"

    summary = {
        "format": "rou-online-viewer-acceptance/0.1",
        "address": address,
        "commit_oid": first["commit_oid"],
        "view_root_sha256": first["view_root_sha256"],
        "first_downloaded_bytes": first["network"]["downloaded_bytes"],
        "first_ready_object_downloaded_bytes": first["first_ready_object_downloaded_bytes"],
        "second_downloaded_bytes": second["network"]["downloaded_bytes"],
        "tampered_object": target.name,
        "checks": 19,
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("ONLINE_VIEWER_ACCEPTANCE_PASS", json.dumps(summary), flush=True)
    return summary


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--viewer", type=Path)
    args = parser.parse_args()
    verify(args.address, args.bundle, args.output, args.viewer)


if __name__ == "__main__":
    main()
