"""Thin ctypes transport. Domain rules and versioned JSON encoding live in rou-core."""

import ctypes
import json
import os
import platform
import sys
from pathlib import Path

ABI_VERSION = 1


class RouError(ValueError):
    def __init__(self, code, message):
        self.code = code
        super().__init__(message)


class Buffer(ctypes.Structure):
    _fields_ = [("ptr", ctypes.c_void_p), ("len", ctypes.c_size_t)]


def library_path():
    if os.environ.get("ROU_LIB"):
        return Path(os.environ["ROU_LIB"])
    system = {"darwin": "macos", "win32": "windows"}.get(sys.platform, "linux")
    machine = "arm64" if platform.machine().lower() in {"arm64", "aarch64"} else "x64"
    filename = {"macos": "librou.dylib", "windows": "rou.dll", "linux": "librou.so"}[system]
    bundled = Path(__file__).parent / "lib" / f"{system}-{machine}" / filename
    if bundled.is_file():
        return bundled
    # Source checkout only; installed archives always carry their own library.
    for parent in Path(__file__).resolve().parents:
        if (parent / "rou/crates/rou-ffi/Cargo.toml").is_file():
            for profile in ("release", "debug"):
                candidate = parent / "rou/target" / profile / filename
                if candidate.is_file():
                    return candidate
            return parent / "rou/target/release" / filename
    raise RouError("library_missing", f"Realm library missing for {system}-{machine}")


LIB = ctypes.CDLL(str(library_path()))
LIB.rou_abi_version.argtypes = []
LIB.rou_abi_version.restype = ctypes.c_uint32
LIB.rou_call.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(Buffer)]
LIB.rou_call.restype = ctypes.c_int32
LIB.rou_free.argtypes = [Buffer]
LIB.rou_free.restype = None
if LIB.rou_abi_version() != ABI_VERSION:
    raise RouError("abi_mismatch", "Realm library and Blender extension ABI versions differ")


def call(operation, value=b""):
    raw = (
        value
        if isinstance(value, bytes)
        else json.dumps(value, ensure_ascii=False, allow_nan=False).encode()
    )
    output = Buffer()
    # c_char_p references immutable bytes without copying; raw stays alive until return.
    status = LIB.rou_call(
        operation.encode(),
        ctypes.cast(ctypes.c_char_p(raw), ctypes.c_void_p),
        len(raw),
        ctypes.byref(output),
    )
    try:
        if not output.ptr:
            raise RouError("ffi_failure", f"Realm ABI returned no response ({status})")
        envelope = json.loads(ctypes.string_at(output.ptr, output.len))
    finally:
        LIB.rou_free(output)
    if status or not envelope["ok"]:
        raise RouError(**envelope["error"])
    return envelope["value"]


CONFIG = call("config")
FORMATS = CONFIG["formats"]
PATHS = CONFIG["paths"]
LIMITS = CONFIG["limits"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_bytes(path, budget):
    with Path(path).open("rb") as stream:
        data = stream.read(budget + 1)
    require(len(data) <= budget, f"File exceeds byte budget: {path}")
    return data


def read_json(path, operation="json.strict"):
    return call(operation, read_bytes(path, LIMITS["json_bytes"]))


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    with temporary.open("xb") as stream:
        stream.write(call("json.pretty", value).encode())
    try:
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
