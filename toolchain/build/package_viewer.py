"""Export real Godot templates; keep the runtime GLB closure outside the PCK."""

import argparse
import importlib.util
import json
import os
import platform as host_platform
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))


def arm64_template(source: Path) -> Path:
    """Official templates contain universal binaries; export needs named arm64 binaries."""
    target = ROOT / ".cache/rou-godot-templates/macos-arm64.zip"
    target.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rou-template-") as directory:
        directory = Path(directory)
        with zipfile.ZipFile(source) as archive:
            archive.extractall(directory)
        for binary in directory.rglob("*.universal"):
            subprocess.run(
                [
                    "lipo",
                    str(binary),
                    "-thin",
                    "arm64",
                    "-output",
                    str(binary.with_suffix(".arm64")),
                ],
                check=True,
            )
            binary.unlink()
        with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(directory.rglob("*")):
                archive.write(path, path.relative_to(directory))
    return target


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--realm", type=Path, default=ROOT / "build/s1-2.1/preview/realm.json")
    p.add_argument("--output-dir", type=Path, default=ROOT / "dist/viewer-2.1")
    p.add_argument("--name", default="Realm of Us", help="Application filename")
    p.add_argument("--platform", choices=["macos", "windows", "linux", "all"], default="macos")
    a = p.parse_args()
    project = ROOT / "client/app"
    if not a.name.strip() or any((char in a.name for char in '/\\\n\r"')) or a.name in {".", ".."}:
        p.error("Choose a simple application name")
    a.realm = a.realm.resolve()
    system = {"darwin": "macos", "win32": "windows"}.get(sys.platform, "linux")
    if a.platform not in [system]:
        p.error(
            "This build packages the current native target; cross-build its extension before exporting another platform"
        )
    suffix = {"macos": ".dylib", "windows": ".dll", "linux": ".so"}[system]
    prefix = "" if system == "windows" else "lib"
    extension = prefix + "rou_godot" + suffix
    for artifact, target in (
        (project / "bin" / extension, "godot-ext"),
        (ROOT / "rou/target/release" / (prefix + "rou" + suffix), "ffi"),
    ):
        if not artifact.is_file():
            p.error(f"missing {artifact.relative_to(ROOT)}; run `make {target}` first")
    os.environ["ROU_LIB"] = str(ROOT / "rou/target/release" / (prefix + "rou" + suffix))
    spec = importlib.util.spec_from_file_location(
        "rou_package_core", ROOT / "toolchain/blender/rou_blender/core.py"
    )
    core = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(core)
    manifest = core.read_json(a.realm, "manifest.validate")
    verified = set()
    for entity in manifest["entities"]:
        asset = entity["asset"]
        if asset["sha256"] not in verified:
            data = core.read_bytes(a.realm.parent / asset["path"], core.LIMITS["glb_bytes"])
            inspected = core.call("glb.inspect", data)
            core.call("asset.match", {"asset": asset, "actual": inspected["descriptor"]})
            verified.add(asset["sha256"])
    version_path = a.realm.parent / "version.json"
    if version_path.exists():
        version = core.read_json(version_path)
        core.call(
            "version.validate",
            {"version": version, "catalog": a.realm.read_bytes().decode("utf-8")},
        )
    templates = ROOT / ".cache/godot-templates"
    if not (templates / "macos.zip").exists():
        raise SystemExit("Run python3 toolchain/build/fetch_templates.py first")
    mac_template = templates / "macos.zip"
    if system == "macos" and host_platform.machine() == "arm64":
        mac_template = arm64_template(mac_template)
    configs = [
        ("macos", "macOS", str(mac_template), a.name + ".app"),
        ("windows", "Windows Desktop", "windows_release_x86_64.exe", a.name + ".exe"),
        ("linux", "Linux", "linux_release.x86_64", a.name + ".x86_64"),
    ]
    lines = []
    for i, (key, platform, template, name) in enumerate(configs):
        lines.extend(
            [
                f"[preset.{i}]",
                f'name="{key}"',
                f'platform="{platform}"',
                "runnable=true",
                'export_filter="all_resources"',
                'include_filter="*.json"',
                'exclude_filter=""',
                'export_path=""',
                "script_export_mode=2",
                "",
                f"[preset.{i}.options]",
                f'custom_template/debug="{templates / template}"',
                f'custom_template/release="{templates / template}"',
                "",
            ]
        )
        if key == "macos":
            lines.extend(
                [
                    'application/bundle_identifier="org.realmofus.viewer"',
                    'application/short_version="0.1"',
                    'application/version="0.1"',
                    'application/min_macos_version="13.0"',
                    "codesign/codesign=0",
                    f'''binary_format/architecture="{("arm64" if host_platform.machine() == "arm64" else "universal")}"''',
                    "notarization/notarization=0",
                    "",
                ]
            )
        else:
            lines.extend(
                ['binary_format/architecture="x86_64"', "binary_format/embed_pck=true", ""]
            )
    presets = project / "export_presets.cfg"
    presets.write_text("\n".join(lines))
    notices_path = a.realm.resolve().parent.parent / "THIRD-PARTY-NOTICES.txt"
    subprocess.run(
        [
            "godot",
            "--headless",
            "--path",
            str(project),
            "--script",
            "../tests/licenses.gd",
            "--",
            "--output=" + str(notices_path),
        ],
        check=True,
        capture_output=True,
        timeout=30,
    )
    subprocess.run(
        ["godot", "--headless", "--editor", "--path", str(project), "--quit"],
        check=True,
        capture_output=True,
        timeout=120,
    )
    reports = []
    for key, platform, template, name in configs:
        if a.platform not in ["all", key]:
            continue
        destination = a.output_dir.resolve() / key
        destination.mkdir(parents=True, exist_ok=True)
        output = destination / name
        log = a.realm.resolve().parent.parent / ("export-" + key + ".log")
        with log.open("w") as f:
            proc = subprocess.run(
                [
                    "godot",
                    "--headless",
                    "--path",
                    str(project),
                    "--export-release",
                    key,
                    str(output),
                ],
                stdout=f,
                stderr=subprocess.STDOUT,
                timeout=180,
            )
        if proc.returncode or not output.exists():
            raise SystemExit("Export failed: " + str(log))
        data = output / "Contents/Resources/preview" if key == "macos" else destination / "preview"
        if data.exists():
            shutil.rmtree(data)
        data.mkdir(parents=True, exist_ok=True)
        shutil.copy2(a.realm, data / "realm.json")
        if version_path.exists():
            shutil.copy2(version_path, data / "version.json")
        for entity in manifest["entities"]:
            src = a.realm.parent / entity["asset"]["path"]
            dst = data / entity["asset"]["path"]
            dst.parent.mkdir(exist_ok=True)
            shutil.copy2(src, dst)
        notice_destination = output / "Contents/Resources" if key == "macos" else destination
        notices = (
            notices_path.read_text() + "\n\n" + (project / "assets/ATTRIBUTION.md").read_text()
        )
        (notice_destination / "THIRD-PARTY-NOTICES.txt").write_text(notices)
        if key == "macos":
            subprocess.run(
                ["codesign", "--force", "--deep", "--sign", "-", str(output)],
                check=True,
                capture_output=True,
            )
        elif key == "linux":
            output.chmod(493)
        reports.append(
            {
                "platform": key,
                "output": str(output.relative_to(ROOT))
                if output.is_relative_to(ROOT)
                else str(output),
                "exported": True,
                "runtime_tested": False,
            }
        )
    (a.realm.resolve().parent.parent / "package-report.json").write_text(
        json.dumps(reports, indent=2) + "\n"
    )
    print(json.dumps(reports, indent=2))


if __name__ == "__main__":
    main()
