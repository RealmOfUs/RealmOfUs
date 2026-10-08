"""Thin ctypes transport. Domain rules and versioned JSON encoding live in rou-core."""

import ctypes
import json
import os
import platform
import sys
from pathlib import Path

ABI_VERSION = 3


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
LIMITS = CONFIG["limits"]["safety"]


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


def externalise_glb(path, texture_directory=None, texture_prefix=None):
    """Apply the native relocation plan; this host implements only bounded file IO."""
    path = Path(path)
    texture_directory = Path(texture_directory or path.parent)
    texture_directory.mkdir(parents=True, exist_ok=True)
    texture_prefix = texture_prefix or path.parent.name + "/"
    raw = read_bytes(path, LIMITS["glb_bytes"])
    plan = call("glb.externalise", raw)
    textures = []
    for image in plan["textures"]:
        destination = texture_directory / image["uri"]
        require(not destination.is_symlink(), "Texture symlink forbidden")
        data = memoryview(raw)[image["offset"] : image["offset"] + image["descriptor"]["bytes"]]
        if destination.exists():
            require(
                call("file.descriptor", read_bytes(destination, LIMITS["image_bytes"]))
                == image["descriptor"],
                "Existing texture bytes differ from their content address",
            )
        else:
            destination.write_bytes(data)
        textures.append(
            {
                "path": texture_prefix + image["uri"],
                **image["descriptor"],
                "metrics": image["metrics"],
            }
        )
    temporary = path.with_suffix(".external.glb")
    try:
        with temporary.open("wb") as target:
            target.write(bytes(plan["prefix"]))
            for segment in plan["segments"]:
                target.write(
                    memoryview(raw)[segment["offset"] : segment["offset"] + segment["bytes"]]
                )
                target.write(bytes(segment["padding"]))
        del raw
        require(
            call("file.descriptor", read_bytes(temporary, LIMITS["glb_bytes"]))
            == plan["descriptor"],
            "Relocated GLB differs from the core plan",
        )
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
    return {"descriptor": plan["descriptor"], "metrics": plan["metrics"], "textures": textures}


def inspect_glb(path, texture_prefix):
    """Inspect an external-image GLB and all referenced images through the core."""
    path = Path(path)
    require(not path.is_symlink() and not path.parent.is_symlink(), "Asset symlink forbidden")
    inspected = call("glb.inspect", read_bytes(path, LIMITS["glb_bytes"]))
    textures, images = [], {}
    for name in inspected["metrics"]["images"]:
        require(not (path.parent / name).is_symlink(), "Texture symlink forbidden")
        image = call("image.inspect", read_bytes(path.parent / name, LIMITS["image_bytes"]))
        texture = {"path": texture_prefix + name, **image["descriptor"]}
        call(
            "texture.match",
            {"texture": texture, "actual": image["descriptor"], "metrics": image["metrics"]},
        )
        textures.append({**texture, "metrics": image["metrics"]})
        images[name] = image["metrics"]
    inspected["metrics"] = call(
        "glb.textures",
        {
            "metrics": inspected["metrics"],
            "textures": [
                {key: value for key, value in texture.items() if key != "metrics"}
                for texture in textures
            ],
            "images": images,
        },
    )
    return {**inspected, "textures": textures}


def write_bundle(directory, bundle):
    """Write native-generated root and exact page bytes into a prepared bundle directory."""
    directory = Path(directory)
    require(not (directory / "chunks").is_symlink(), "Page directory symlink forbidden")
    (directory / "chunks").mkdir(parents=True, exist_ok=True)
    for name, text in bundle["pages"].items():
        path = directory / name
        require(not path.is_symlink(), "Page symlink forbidden")
        path.write_text(text)
    write_json(directory / PATHS["realm"], bundle["root"])


def read_bundle(path):
    """Read independently bounded, root-bound pages for development and export tools."""
    path = Path(path)
    require(
        not path.is_symlink() and not (path.parent / "chunks").is_symlink(),
        "Catalog symlink forbidden",
    )
    root = read_json(path, "manifest.validate")
    pages = {}
    for descriptor in root["chunks"].values():
        page_path = path.parent / descriptor["page"]
        require(not page_path.is_symlink(), "Page symlink forbidden")
        pages[descriptor["page"]] = read_bytes(page_path, LIMITS["json_bytes"]).decode("utf-8")
    return call("bundle.validate", {"root": root, "pages": pages})
