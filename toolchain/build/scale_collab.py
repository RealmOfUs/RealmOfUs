"""Measure local collaboration bookkeeping with guarded, deterministic placeholder assets."""

import argparse
import hashlib
import json
import math
import os
import random
import re
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
AREA = ROOT / "build/scale-collab"
sys.path.insert(0, str(ROOT))
from toolchain.build import native
from toolchain.build.fake_exporter import templates


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def local_path(path):
    path = path.resolve()
    if not path.is_relative_to(AREA) or path == AREA:
        raise ValueError("Measurements may only write inside build/scale-collab/")
    return path


def model_id(index):
    return f"m{index:04d}"


def move(realm, indexes, delta):
    for index in indexes:
        path = realm / f"realm/entities/{model_id(index)}.json"
        entity = json.loads(path.read_text())
        entity["position_mm"][0] += delta
        write(path, entity)


def edit_source(realm, revision):
    path = realm / "sources/group000/source.blend"
    source = json.loads(path.read_text())
    source["revision"] = revision
    write(path, source)


class Experiment:
    def __init__(self, action, realm):
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        self.output = AREA / "reports" / f"{action}-{realm.name}-{stamp}"
        self.output.mkdir(parents=True)
        self.binary = native.cli()
        native.ffi_library()
        self.core = native.load_core()
        self.env = {k: v for k, v in os.environ.items() if not k.startswith(("GIT_", "ROU_"))}
        self.env.update(
            GIT_CONFIG_GLOBAL=os.devnull,
            GIT_CONFIG_NOSYSTEM="1",
            GIT_AUTHOR_NAME="Collaboration measurement",
            GIT_AUTHOR_EMAIL="measurement@example.invalid",
            GIT_COMMITTER_NAME="Collaboration measurement",
            GIT_COMMITTER_EMAIL="measurement@example.invalid",
            GIT_LFS_SKIP_SMUDGE="1",
            GIT_TERMINAL_PROMPT="0",
            ROU_TOOLCHAIN_ROOT=str(ROOT),
            ROU_BLENDER=str(ROOT / "toolchain/build/fake_exporter.py"),
            ROU_FAKE_EXPORTER="1",
            ROU_LIB=str(native.ffi_library()),
        )
        power = (
            subprocess.run(["pmset", "-g", "batt"], capture_output=True, text=True).stdout
            if sys.platform == "darwin"
            else "not measured"
        )
        self.report = {
            "action": action,
            "realm": str(realm),
            "conditions": "Local machine; deterministic fake exporter; placeholder sources; warm OS caches; no remote network",
            "power": power.strip(),
            "profile_hash": self.core.call(
                "digest.sha256",
                self.core.call("json.canonical", self.core.CONFIG["profile"]).encode(),
            ),
            "commands": [],
            "results": {},
        }
        self.save()

    def save(self):
        write(self.output / "report.json", self.report)

    def run(self, cwd, program, *args, expected=0):
        command = [str(program), *map(str, args)]
        started = time.perf_counter()
        proc = subprocess.run(
            command, cwd=cwd, env=self.env, capture_output=True, text=True, timeout=900
        )
        seconds = time.perf_counter() - started
        log = f"{len(self.report['commands']) + 1:03d}-{Path(program).name}.log"
        (self.output / log).write_text(proc.stdout + "\n" + proc.stderr)
        self.report["commands"].append(
            {
                "argv": command,
                "cwd": str(cwd),
                "seconds": seconds,
                "exit_code": proc.returncode,
                "log": log,
            }
        )
        self.save()
        if expected is not None and proc.returncode != expected:
            raise RuntimeError(f"Command failed: {command}; see {self.output / log}")
        return proc, seconds

    def rou(self, cwd, *args, expected=0):
        proc, seconds = self.run(cwd, self.binary, "--json", *args, expected=expected)
        if proc.returncode:
            match = re.search(r"rou \[([^\]]+)\]: (.*)", proc.stderr, re.DOTALL)
            return {
                "ok": False,
                "error": {
                    "code": match[1] if match else "command_failed",
                    "message": proc.stderr.strip(),
                },
            }, seconds
        try:
            value = json.loads(proc.stdout)
        except json.JSONDecodeError:
            value = proc.stdout.strip()
        return value, seconds

    def git(self, cwd, *args, expected=0):
        return self.run(cwd, "git", *args, expected=expected)[0].stdout.strip()

    def commit(self, realm, message):
        self.rou(realm, "add", ".")
        return self.rou(realm, "commit", "-m", message)

    def finish(self, result):
        self.report["results"] = result
        self.report["status"] = "passed"
        self.save()
        print(
            "COLLAB_SCALE_VERIFIED",
            json.dumps({"report": str(self.output / "report.json"), **result}),
            flush=True,
        )


def generate(exp, realm, count):
    if realm.exists() and any(realm.iterdir()):
        raise ValueError("Generate requires an empty output directory")
    if count < 64 or count % 64:
        raise ValueError("Use a positive multiple of 64 models")
    realm.mkdir(parents=True, exist_ok=True)
    exp.rou(realm, "init")
    with (realm / ".gitattributes").open("a") as stream:
        stream.write("*.bin filter=lfs diff=lfs merge=lfs -text\n")
    rng = random.Random(20261007)
    profile = exp.core.CONFIG["profile"]
    for group in range(count // 64):
        source = f"sources/group{group:03d}/source.blend"
        dependency = f"sources/shared/{group:03d}.bin"
        placeholder = {"format": "rou-collab-placeholder/1", "revision": "initial", "models": {}}
        path = realm / dependency
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"initial dependency\n" * 128)
        for offset in range(64):
            name = model_id(group * 64 + offset)
            deps = [dependency] if offset < 8 else []
            placeholder["models"][name] = {"dependencies": deps}
            write(
                realm / f"realm/models/{name}.json",
                {
                    "format": "rou-model/0.1",
                    "id": name,
                    "source": source,
                    "entry": {"kind": "object", "name": name},
                    "dependencies": deps,
                    "settings": {},
                },
            )
            position = [
                rng.randint(
                    profile["origin_mm"][a]
                    + profile["min_chunk"][a] * profile["chunk_size_mm"][a]
                    + 2000,
                    profile["origin_mm"][a]
                    + profile["max_chunk"][a] * profile["chunk_size_mm"][a]
                    - 2000,
                )
                for a in range(3)
            ]
            write(
                realm / f"realm/entities/{name}.json",
                {"format": "rou-entity/0.1", "id": name, "model": name, "position_mm": position},
            )
        write(realm / source, placeholder)
    built, build_seconds = exp.rou(realm, "build")
    assert len(built["generated"]) == count and not built["reused"]
    exp.rou(realm, "add", ".")
    validated, _ = exp.rou(realm, "validate", "--staged")
    assert validated["entity_count"] == count and validated["byte_validation_complete"]
    hashes = {
        json.loads(p.read_text())[tier]["sha256"]
        for p in (realm / "runtime").glob("*.json")
        for tier in ("asset", "lod1")
    }
    assert len(hashes) == count * 2
    commit, _ = exp.commit(realm, "Seed synthetic collaboration measurement")
    marker = {
        "entities": count,
        "seed_commit": commit["commit"],
        "source_models": 64,
        "dependency_models": 8,
    }
    write(realm / ".rou/scale-collab.json", marker)
    pair = templates()
    exp.finish(
        {
            **marker,
            "build_seconds": build_seconds,
            "unique_asset_shas": len(hashes),
            "source_bytes": [p.stat().st_size for p in (realm / "sources").glob("group*/*.blend")],
            "templates": [
                {"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in pair
            ],
        }
    )


def summary(samples):
    ordered = sorted(samples)
    return {
        "count": len(samples),
        "seconds": samples,
        "p50_seconds": statistics.median(samples),
        "p95_seconds": ordered[math.ceil(0.95 * len(ordered)) - 1],
        "percentile_method": "median P50; nearest-rank P95",
    }


def timing(exp, realm, repeat):
    marker = json.loads((realm / ".rou/scale-collab.json").read_text())
    if exp.git(realm, "status", "--porcelain"):
        raise ValueError("Timing requires a clean generated realm")
    _, startup = exp.run(realm, exp.binary, "--version")
    # Warm the descriptor cache before the five measured validation subprocesses.
    exp.rou(realm, "validate")
    results = {"entities": marker["entities"], "startup_seconds": startup}
    results["validate"] = summary([exp.rou(realm, "validate")[1] for _ in range(repeat)])
    for count in (1, 64):
        samples = []
        for n in range(repeat):
            move(realm, range(count), 1)
            _, seconds = exp.commit(realm, f"Move {count} entities for timing {n}")
            samples.append(seconds)
        results[f"commit_{count}"] = summary(samples)
    samples, builds = [], []
    for n in range(repeat):
        edit_source(realm, f"timing-{time.time_ns()}")
        built, seconds = exp.rou(realm, "build")
        assert len(built["generated"]) == 64 and len(built["reused"]) == marker["entities"] - 64
        samples.append(seconds)
        builds.append({"generated": built["generated"], "reused_count": len(built["reused"])})
        exp.commit(realm, f"Save synthetic source revision {n}")
    results["build_64"] = {**summary(samples), "builds": builds}
    results["commit_only_proposals_per_hour"] = 3600 / results["commit_64"]["p50_seconds"]
    exp.finish(results)


def scenarios(exp, realm):
    marker = json.loads((realm / ".rou/scale-collab.json").read_text())
    remote = exp.output / "remote.git"
    exp.git(exp.output, "init", "--bare", "--initial-branch=main", remote)
    remote_name = "measurement-" + exp.output.name
    exp.git(realm, "remote", "add", remote_name, remote)
    exp.git(realm, "push", remote_name, f"{marker['seed_commit']}:refs/heads/main")
    exp.git(realm, "remote", "remove", remote_name)
    results = {}
    source = "sources/group000/source.blend"
    dependency = "sources/shared/000.bin"
    for case in ("disjoint", "same_source", "shared_dependency", "bare_commit", "new_dependency"):
        authors = []
        for author in ("alice", "bob"):
            folder = exp.output / f"{case}-{author}"
            exp.rou(exp.output, "clone", remote, folder)
            exp.rou(folder, "sparse", "set", "--entity", "m0000")
            authors.append(folder)
        alice, bob = authors
        if case == "disjoint":
            move(alice, range(16), 10)
            move(bob, range(16, 32), 20)
        elif case == "same_source":
            for author, delta in ((alice, 10), (bob, 20)):
                move(author, [0], delta)
                edit_source(author, author.name)
                built, _ = exp.rou(author, "build")
                assert len(built["generated"]) == 64
        elif case in ("shared_dependency", "new_dependency"):
            (alice / dependency).write_bytes(b"alice dependency\n" * 128)
            built, _ = exp.rou(alice, "build")
            assert len(built["generated"]) == 8
            if case == "shared_dependency":
                edit_source(bob, "bob source revision")
                expected_generated = 64
            else:
                # New dependency edges are not covered by the proposed merge proof.
                path = bob / "realm/models/m0008.json"
                model = json.loads(path.read_text())
                model["dependencies"] = [dependency]
                write(path, model)
                expected_generated = 1
            built, _ = exp.rou(bob, "build")
            assert len(built["generated"]) == expected_generated
        else:
            move(alice, [marker["entities"] - 1], 10)
            (bob / dependency).write_bytes(b"unbuilt dependency\n" * 128)
        exp.commit(alice, f"Alice proposal for {case}")
        exp.rou(bob, "add", ".")
        if case == "bare_commit":
            rejected, _ = exp.rou(bob, "commit", "-m", "Reject stale proposal", expected=1)
            assert rejected["error"]["code"] == "stale_build"
            exp.git(bob, "commit", "-m", "Intentionally bypass validation")
        else:
            exp.rou(bob, "commit", "-m", f"Bob proposal for {case}")
        exp.git(alice, "push", "origin", f"HEAD:refs/heads/{case}-alice")
        exp.git(bob, "fetch", "origin", f"{case}-alice")
        merge, merge_seconds = exp.run(
            bob, "git", "merge", "--no-commit", "--no-ff", "FETCH_HEAD", expected=None
        )
        conflicts = exp.git(bob, "diff", "--name-only", "--diff-filter=U").splitlines()
        expected = []
        if case == "same_source":
            expected = ["realm/entities/m0000.json", source] + [
                f"runtime/{model_id(i)}.json" for i in range(64)
            ]
        elif case == "shared_dependency":
            expected = [f"runtime/{model_id(i)}.json" for i in range(8)]
        assert sorted(conflicts) == sorted(expected), (case, conflicts)
        assert merge.returncode == (1 if conflicts else 0)
        started = time.perf_counter()
        if conflicts:
            # Alice is upstream (theirs) because the merge runs in Bob's checkout.
            exp.git(bob, "checkout", "--theirs", "--", *conflicts)
            exp.git(bob, "add", "--", *conflicts)
        valid, _ = exp.rou(bob, "validate", "--staged", expected=None)
        stale = case in ("shared_dependency", "bare_commit", "new_dependency")
        assert bool(valid.get("ok")) != stale, (case, valid)
        if stale:
            assert valid["error"]["code"] == "stale_build", (case, valid)
        result = {
            "conflicts": conflicts,
            "git_merge_clean": not conflicts,
            "merge_seconds": merge_seconds,
            "validation_after_resolution": {
                k: v for k, v in valid.items() if k not in ("validated_assets", "missing_assets")
            },
        }
        if case in ("bare_commit", "new_dependency"):
            # Publish the bad merge only to the disposable local bare repository.
            exp.git(bob, "commit", "-m", "Capture clean Git merge with stale runtime")
            exp.git(bob, "push", "origin", f"HEAD:refs/heads/{case}-merged")
            clean = exp.output / f"{case}-clean"
            exp.git(exp.output, "clone", "--branch", f"{case}-merged", remote, clean)
            rejected, _ = exp.rou(clean, "validate", expected=1)
            assert rejected["error"]["code"] == "stale_build"
            result["clean_clone_validation"] = rejected
            assert not exp.git(clean, "status", "--porcelain")
        built, build_seconds = exp.rou(bob, "build")
        expected_ids = [model_id(i) for i in range(8)] if stale else []
        if case == "new_dependency":
            expected_ids = ["m0008"]
        assert sorted(built["generated"]) == expected_ids, (case, built["generated"])
        exp.commit(bob, f"Resolve and validate {case}")
        result.update(
            generated=built["generated"],
            reused_count=len(built["reused"]),
            rebuild_seconds=build_seconds,
            resolution_seconds=time.perf_counter() - started,
        )
        results[case] = result
        exp.report["results"] = results
        exp.save()
        print(
            "COLLAB_SCENARIO",
            case,
            len(conflicts),
            "conflicts;",
            len(built["generated"]),
            "rebuilt",
            flush=True,
        )
    exp.finish(results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    generate_parser = sub.add_parser("generate")
    generate_parser.add_argument("--entities", type=int, required=True)
    generate_parser.add_argument("--output", type=Path, required=True)
    for action in ("timing", "scenarios"):
        child = sub.add_parser(action)
        child.add_argument("--realm", type=Path, required=True)
        if action == "timing":
            child.add_argument("--repeat", type=int, default=5)
    args = parser.parse_args()
    realm = local_path(args.output if args.action == "generate" else args.realm)
    exp = Experiment(args.action, realm)
    if args.action == "generate":
        generate(exp, realm, args.entities)
    elif args.action == "timing":
        if args.repeat < 1:
            parser.error("--repeat must be positive")
        timing(exp, realm, args.repeat)
    else:
        scenarios(exp, realm)


if __name__ == "__main__":
    main()
