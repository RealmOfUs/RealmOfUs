"""Build/load native adapters for developer tools; all domain rules stay in Rust."""

import importlib.util
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def library(name: str) -> str:
    if sys.platform == "win32":
        return name + ".dll"
    return "lib" + name + (".dylib" if sys.platform == "darwin" else ".so")


def artifact(path: Path, target: str) -> Path:
    """Return a build product or explain which Makefile target creates it."""
    if not path.is_file():
        raise SystemExit(f"missing {path.relative_to(ROOT)}; run `make {target}` first")
    return path


def cli() -> Path:
    return artifact(ROOT / "build/bin/rou", "rou")


def godot_extension() -> Path:
    return artifact(ROOT / "client/app/bin" / library("rou_godot"), "godot-ext")


def ffi_library() -> Path:
    path = artifact(ROOT / "rou/target/release" / library("rou"), "ffi")
    os.environ.setdefault("ROU_LIB", str(path))
    return path


def load_core():
    spec = importlib.util.spec_from_file_location(
        "rou_tool_core", ROOT / "toolchain/blender/rou_blender/core.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_bundle(path: Path, core=None) -> dict:
    core = core or load_core()
    manifest = core.read_json(path, "manifest.validate")
    checked = set()
    for entity in manifest["entities"]:
        for asset in [entity["asset"], entity["lod1"]]:
            if asset["sha256"] not in checked:
                raw = core.read_bytes(path.parent / asset["path"], core.LIMITS["glb_bytes"])
                actual = core.call("glb.inspect", raw)
                core.call("asset.match", {"asset": asset, "actual": actual["descriptor"]})
                checked.add(asset["sha256"])
    version = path.parent / "version.json"
    if version.exists():
        core.call(
            "version.validate", {"version": core.read_json(version), "catalog": path.read_text()}
        )
    return manifest
