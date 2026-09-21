"""Package the Blender add-on with its dependency-free preview validation library."""

import argparse
import platform
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[2]


def package_addon(destination: Path) -> Path:
    subprocess.run(["cargo", "build", "-p", "rou-ffi", "--release"], cwd=ROOT / "rou", check=True)
    system = {"darwin": "macos", "win32": "windows"}.get(sys.platform, "linux")
    machine = "arm64" if platform.machine().lower() in {"arm64", "aarch64"} else "x64"
    target = f"{system}-{machine}"
    filename = {"macos": "librou.dylib", "windows": "rou.dll", "linux": "librou.so"}[system]
    library = ROOT / "rou/target/release" / filename
    source = ROOT / "toolchain/blender/rou_blender"
    destination.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix="rou-addon-") as temporary:
        packaged_library = Path(temporary) / filename
        shutil.copyfile(library, packaged_library)
        if system == "macos":
            # A reproducible local signature, not Developer ID signing or notarization.
            subprocess.run(
                ["codesign", "--force", "--sign", "-", "--timestamp=none", str(packaged_library)],
                check=True,
            )
            subprocess.run(["codesign", "--verify", "--strict", str(packaged_library)], check=True)
        with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(source.glob("*.py")):
                archive.write(path, path.name)
            manifest = (source / "blender_manifest.toml").read_text()
            archive.writestr(
                "blender_manifest.toml",
                manifest.replace('platforms = ["macos-arm64"]', f'platforms = ["{target}"]'),
            )
            archive.write(packaged_library, f"lib/{target}/{filename}")
    return destination


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    function = package_addon
    destination = args.output or ROOT / "dist/rou_blender.zip"
    print("ROU_ADDON_PACKAGED", function(destination))


if __name__ == "__main__":
    main()
