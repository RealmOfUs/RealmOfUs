"""Host IO for source-scoped editing baselines; fingerprints come from Rust."""

import os
import subprocess
from pathlib import Path

from . import core


class SessionError(ValueError):
    """Reopen or explicitly reapply after the worktree changes."""


def git(root, *args, optional=False):
    environment = dict(os.environ)
    for key in (
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ):
        environment.pop(key, None)
    environment.update(GIT_NO_REPLACE_OBJECTS="1", GIT_TERMINAL_PROMPT="0", LC_ALL="C")
    result = subprocess.run(["git", *args], cwd=root, env=environment, capture_output=True)
    if result.returncode and not optional:
        raise SessionError(os.fsdecode(result.stderr).strip() or "Git command failed")
    return os.fsdecode(result.stdout).rstrip("\n")


def repository(path):
    path = Path(path)
    root = Path(
        git(path if path.is_dir() else path.parent, "rev-parse", "--show-toplevel")
    ).resolve()
    core.read_json(root / core.PATHS["realm"], "realm.validate")
    return root


def relative_file(root, name):
    core.call("path.safe", name)
    path = root / name
    for parent in [path, *path.parents]:
        if parent == root.parent:
            break
        core.require(not parent.is_symlink(), f"Authoring path contains a symlink: {path}")
    return path


def records(root, kind):
    result = {}
    directory = core.PATHS[{"model": "models", "entity": "entities"}[kind]]
    for path in sorted((root / directory).glob("*.json")):
        value = core.read_json(path, kind + ".validate")
        core.call(
            "record.validate",
            {"path": path.relative_to(root).as_posix(), "kind": kind, "id": value["id"]},
        )
        result[value["id"]] = value
    return result


def descriptor(path):
    data = core.read_bytes(path, core.LIMITS["file_bytes"])
    pointer = core.call("lfs.pointer", data[: core.LIMITS["pointer_bytes"] + 1])
    return pointer or core.call("file.descriptor", data)


class FileCache:
    """Cache host reads by identity, size and nanosecond modification/change stamps."""

    def __init__(self):
        self.values = {}

    def read(self, path, loader):
        path = Path(path)
        stat = path.stat()
        stamp = (stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns)
        key = (path, loader)
        previous = self.values.get(key)
        if previous is not None and previous[0] == stamp:
            return previous[1]
        value = loader(path)
        after = path.stat()
        if stamp != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        ):
            raise SessionError(f"Authoring file changed while reading: {path}")
        self.values[key] = (stamp, value)
        return value


def scoped_records(root, source, cache, required):
    result = {"model": {}, "entity": {}}
    for kind in ("model", "entity"):
        directory = core.PATHS[{"model": "models", "entity": "entities"}[kind]]
        for path in sorted((root / directory).glob("*.json")):
            try:
                value = cache.read(
                    relative_file(root, path.relative_to(root).as_posix()), core.read_json
                )
            except (ValueError, OSError):
                if path in required:
                    raise
                continue
            relevant = isinstance(value, dict) and (
                value.get("source") == source
                if kind == "model"
                else value.get("model") in result["model"]
            )
            if not relevant:
                if path in required:
                    # A former local record changed scope; the fingerprint must change.
                    core.call(kind + ".validate", value)
                continue
            core.call(kind + ".validate", value)
            core.call(
                "record.validate",
                {"path": path.relative_to(root).as_posix(), "kind": kind, "id": value["id"]},
            )
            result[kind][value["id"]] = value
            required.add(path)
    return result["model"], result["entity"]


def baseline(root, source, cache, required):
    relative = source.relative_to(root).as_posix()
    models, entities = scoped_records(root, relative, cache, required)
    inputs = {}
    shared = {relative: cache.read(source, descriptor)}
    for identity, model in models.items():
        if model["source"] != relative:
            continue
        inputs[identity] = {}
        for name in [model["source"], *model["dependencies"]]:
            if name not in shared:
                shared[name] = cache.read(relative_file(root, name), descriptor)
            inputs[identity][name] = shared[name]
    world = {
        "realm": core.read_json(root / core.PATHS["realm"], "realm.validate"),
        "models": models,
        "entities": entities,
        "inputs": inputs,
    }
    return {
        "source": shared[relative],
        "records": core.call("session.fingerprint", {"world": world, "source": relative}),
    }


def head(root):
    return (
        git(root, "rev-parse", "--verify", "HEAD", optional=True),
        git(root, "symbolic-ref", "--quiet", "HEAD", optional=True),
    )


class Session:
    def __init__(self, source):
        self.source = Path(source).resolve()
        self.root = repository(self.source)
        self.base_head = head(self.root)
        self.cache = FileCache()
        self.required = set()
        self.base_inputs = baseline(self.root, self.source, self.cache, self.required)

    def check(self):
        if head(self.root) != self.base_head:
            raise SessionError("Git version or branch changed; reopen the source")
        if baseline(self.root, self.source, self.cache, self.required) != self.base_inputs:
            raise SessionError(
                "This source, its model, placement or dependencies changed; reopen, save source only, or reapply placement"
            )

    def check_source(self):
        if (
            head(self.root) != self.base_head
            or self.cache.read(self.source, descriptor) != self.base_inputs["source"]
        ):
            raise SessionError("Source or Git version changed; reopen before saving")

    def accept_own_write(self):
        if head(self.root) != self.base_head:
            raise SessionError("Git version changed during write; reopen the source")
        self.base_inputs = baseline(self.root, self.source, self.cache, self.required)
