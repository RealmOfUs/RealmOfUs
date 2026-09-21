"""Build/load native adapters for developer tools; all domain rules stay in Rust."""

import importlib.util
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def library(name: str) -> str:
    if sys.platform == "win32":
        return name + ".dll"
    return "lib" + name + (".dylib" if sys.platform == "darwin" else ".so")


def build_native(*crates: str, release: bool = False) -> None:
    command = ["cargo", "build"]
    for crate in crates:
        command += ["-p", crate]
    if release:
        command.append("--release")
    subprocess.run(command, cwd=ROOT / "rou", check=True)
    target = ROOT / "rou/target" / ("release" if release else "debug")
    if "rou-godot" in crates:
        destination = ROOT / "client/app/bin"
        destination.mkdir(exist_ok=True)
        shutil.copy2(target / library("rou_godot"), destination / library("rou_godot"))
    if "rou-ffi" in crates:
        os.environ["ROU_LIB"] = str(target / library("rou"))


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
        asset = entity["asset"]
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
