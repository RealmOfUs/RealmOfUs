"""Record signing, quarantine and real Blender loading of isolated add-on copies.

This is a local delivery probe, not proof of Developer ID signing, notarization,
Finder download behavior or Gatekeeper acceptance on other machines. Never
modify system policy or remove quarantine from the supplied archive.
"""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
import uuid
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--addon-zip", type=Path, default=ROOT / "dist/rou_blender.zip")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("This probe requires macOS")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {
        "macos": platform.mac_ver()[0],
        "machine": platform.machine(),
        "archive_sha256": hashlib.sha256(args.addon_zip.read_bytes()).hexdigest(),
        "scope": "isolated copies; local Blender ctypes load; no security policy changes",
        "commands": [],
        "cases": {},
        "rou_lib_override_removed": "ROU_LIB" in os.environ,
    }
    environment = os.environ.copy()
    environment.pop("ROU_LIB", None)

    def run(command):
        try:
            result = subprocess.run(
                list(map(str, command)),
                capture_output=True,
                text=True,
                timeout=30,
                env=environment,
            )
            record = {
                "argv": list(map(str, command)),
                "exit_code": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "timed_out": False,
            }
        except subprocess.TimeoutExpired as error:
            record = {
                "argv": list(map(str, command)),
                "exit_code": 124,
                "stdout": (error.stdout or b"").decode(errors="replace"),
                "stderr": (error.stderr or b"").decode(errors="replace"),
                "timed_out": True,
            }
        report["commands"].append(record)
        (output / "gatekeeper-report.json").write_text(json.dumps(report, indent=2) + "\n")
        return record

    run(["spctl", "--status"])
    archive = output / "quarantined.zip"
    shutil.copyfile(args.addon_zip, archive)
    quarantine = f"0083;{int(time.time()):x};RealmDeliveryProbe;{uuid.uuid4()}"
    assert run(["xattr", "-w", "com.apple.quarantine", quarantine, archive])["exit_code"] == 0
    extracted = output / "extracted"
    assert run(["ditto", "-x", "-k", archive, extracted])["exit_code"] == 0
    library = next(extracted.glob("lib/*/librou.dylib"))
    run(["xattr", "-l", library])
    for name in ("packaged", "quarantined", "unsigned_quarantined"):
        case = output / name
        with zipfile.ZipFile(archive) as package:
            package.extractall(case / "rou_blender")
        dylib = next((case / "rou_blender/lib").glob("*/librou.dylib"))
        if name == "unsigned_quarantined":
            assert run(["codesign", "--remove-signature", dylib])["exit_code"] == 0
        if name != "packaged":
            assert run(["xattr", "-w", "com.apple.quarantine", quarantine, dylib])["exit_code"] == 0
        details = run(["codesign", "-d", "--verbose=4", dylib])
        verified = run(["codesign", "--verify", "--strict", "--verbose=4", dylib])
        assessment = run(["spctl", "--assess", "--type", "execute", "--verbose=4", dylib])
        attributes = run(["xattr", "-l", dylib])
        expression = (
            f"import sys; sys.path.insert(0, {str(case)!r}); print('ROU_LOAD_BEGIN', flush=True); import rou_blender; "
            f"from pathlib import Path; assert rou_blender.core.library_path().resolve() == Path({str(dylib)!r}).resolve(); "
            "print('ROU_LOADED_LIBRARY', rou_blender.core.library_path(), flush=True); "
            "rou_blender.register(); print('ROU_QUARANTINE_LOAD_OK', "
            "rou_blender.core.call('config')['abi_version']); rou_blender.unregister()"
        )
        loaded = run(
            [
                "blender",
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python-expr",
                expression,
            ]
        )
        report["cases"][name] = {
            "signature_details": details["stderr"],
            "signature_valid": verified["exit_code"] == 0,
            "spctl_exit_code": assessment["exit_code"],
            "quarantine": attributes["stdout"],
            "blender_exit_code": loaded["exit_code"],
            "blender_timed_out": loaded["timed_out"],
            "blender_abi_loaded": "ROU_QUARANTINE_LOAD_OK 1" in loaded["stdout"],
        }
        (output / "gatekeeper-report.json").write_text(json.dumps(report, indent=2) + "\n")
    assert report["cases"]["packaged"]["signature_valid"]
    assert report["cases"]["packaged"]["blender_abi_loaded"]
    print("MACOS_DELIVERY_PROBE_RECORDED", output / "gatekeeper-report.json")


if __name__ == "__main__":
    main()
