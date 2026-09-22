"""Exercise the native rou workflow with real Blender files on the current Mac.

Every run creates independent repositories under build/. The synthetic remote is
a local bare Git/LFS provider, so no account, hosted service or P2P is implied.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build import native  # noqa: E402


class Acceptance:
    def __init__(self, directory: Path):
        self.directory = directory
        self.directory.mkdir(parents=True)
        self.binary = directory / "rou"
        self.environment = {
            key: value for key, value in os.environ.items() if not key.startswith("GIT_")
        }
        self.environment.update(
            {
                "GIT_CONFIG_GLOBAL": os.devnull,
                "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_AUTHOR_NAME": "Realm acceptance",
                "GIT_AUTHOR_EMAIL": "acceptance@example.invalid",
                "GIT_COMMITTER_NAME": "Realm acceptance",
                "GIT_COMMITTER_EMAIL": "acceptance@example.invalid",
                "GIT_LFS_SKIP_SMUDGE": "1",
                "ROU_TOOLCHAIN_ROOT": str(ROOT),
                "ROU_PYTHON": sys.executable,
                "PYTHONPATH": str(ROOT),
            }
        )
        self.checks = []
        self.commands = []

    def run(
        self, command: list[str], cwd: Path = ROOT, expected: int | None = 0, timeout: int = 300
    ) -> str:
        number = len(self.commands) + 1
        result = subprocess.run(
            list(map(str, command)),
            cwd=cwd,
            env=self.environment,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        log = self.directory / f"{number:03d}-{Path(command[0]).name}.log"
        log.write_text(result.stdout + "\n" + result.stderr)
        self.commands.append(
            {
                "argv": list(map(str, command)),
                "cwd": str(cwd),
                "exit_code": result.returncode,
                "log": str(log.relative_to(self.directory)),
            }
        )
        self.save()
        if expected is not None and result.returncode != expected:
            raise RuntimeError(
                f"Command failed ({result.returncode}): {command}; see {log}\n{result.stderr[-2000:]}"
            )
        return result.stdout

    def rou(self, cwd: Path, *args: str, expected: int | None = 0):
        text = self.run([str(self.binary), "--json", *args], cwd, expected)
        try:
            return json.loads(text) if text.strip() else None
        except json.JSONDecodeError:
            return text

    def git(self, cwd: Path, *args: str) -> str:
        return self.run(["git", *args], cwd).strip()

    def blender(self, cwd: Path, *args: str) -> None:
        self.run(
            [
                "blender",
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
                str(ROOT / "examples/calibration" / "create_git_scene.py"),
                "--",
                "--repo",
                str(cwd),
                *args,
            ]
        )

    def check(self, name: str, condition: bool) -> None:
        if not condition:
            raise AssertionError(name)
        self.checks.append({"name": name, "passed": True})
        self.save()
        print("GIT_REALM_CHECK_PASS", name, flush=True)

    def save(self) -> None:
        (self.directory / "report.json").write_text(
            json.dumps(
                {
                    "format": "rou-git-acceptance/0.1",
                    "checks": self.checks,
                    "commands": self.commands,
                    "scope": "current Mac; local bare Git/LFS; no P2P",
                },
                indent=2,
            )
            + "\n"
        )

    def payloads(self, cwd: Path) -> list[Path]:
        media = self.git(cwd, "rev-parse", "--git-path", "lfs/objects")
        path = Path(media)
        if not path.is_absolute():
            path = cwd / path
        return [item for item in path.rglob("*") if item.is_file()]

    def execute(self, windowed: bool, git_only: bool = False) -> None:
        shutil.copy2(native.cli(), self.binary)
        official, remote, alice, bob = [
            self.directory / name for name in ("official", "remote.git", "alice", "bob")
        ]
        official.mkdir()
        self.rou(official, "init")
        self.blender(official)
        self.rou(official, "build")
        self.check(
            "build does not stage", not self.git(official, "diff", "--cached", "--name-only")
        )
        self.rou(official, "add", ".")
        first = self.rou(official, "commit", "-m", "Initial shared garden")["commit"]
        self.check(
            "realm contains only model and entity records",
            all(
                path.startswith(("realm/models/", "realm/entities/")) and path.endswith(".json")
                for path in self.git(official, "ls-files", "realm").splitlines()
            )
            and bool(self.git(official, "ls-files", "realm")),
        )
        self.check(
            "only Git owns history",
            not (official / ".rou/HEAD").exists() and (not (official / ".rou/objects").exists()),
        )
        self.check("local caches ignored", not self.git(official, "ls-files", ".rou"))
        self.git(ROOT, "init", "--bare", "--initial-branch=main", str(remote))
        self.rou(official, "remote", "add", "origin", str(remote))
        self.rou(official, "push", "-u", "origin", "main")
        for author in (alice, bob):
            self.rou(self.directory, "clone", str(remote), str(author))
            self.check(f"{author.name} clone has no LFS payloads", not self.payloads(author))
            self.check(
                f"{author.name} clone has metadata only",
                (author / "realm/entities/house.json").is_file()
                and (not (author / "sources").exists()),
            )
        self.rou(alice, "switch", "-c", "feature/house")
        self.rou(bob, "switch", "-c", "feature/bench")
        self.check(
            "branch creation does not hydrate",
            not self.payloads(alice) and (not self.payloads(bob)),
        )
        empty_view = self.directory / "empty-view"
        self.rou(alice, "view", "--output", str(empty_view))
        self.check(
            "empty selection exports metadata without downloads",
            not self.payloads(alice) and not list((empty_view / "objects").iterdir()),
        )
        self.rou(alice, "sparse", "set", "--chunk", "0,0,0")
        self.rou(bob, "sparse-checkout", "set", "--entity", "bench")
        selected_view = self.directory / "selected-view"
        before = set(self.payloads(alice))
        selected = self.rou(alice, "view", "HEAD", "--output", str(selected_view))
        self.check(
            "default view includes only house and plaque without downloads",
            selected["downloaded_entities"] == ["house", "plaque"]
            and len(list((selected_view / "objects").iterdir())) == 2
            and set(self.payloads(alice)) == before,
        )
        catalog = json.loads((selected_view / "realm.json").read_text())
        self.check("selected bundle keeps all four entities", len(catalog["entities"]) == 4)
        self.check(
            "materialized runtime assets are hard-linked",
            all(
                (selected_view / entity["asset"]["path"]).samefile(
                    alice / "runtime" / (entity["id"] + ".glb")
                )
                for entity in catalog["entities"]
                if entity["id"] in {"house", "plaque"}
            ),
        )
        chunk_view = self.directory / "chunk-view"
        chunk = self.rou(alice, "view", "--chunk", "1,0,0", "--output", str(chunk_view))
        self.check(
            "explicit chunk exports bench without fetching sources",
            chunk["downloaded_entities"] == ["bench"] and not (alice / "sources/props").exists(),
        )

        self.check(
            "selection hydrates source and shared dependencies",
            all(
                (
                    (alice / path).is_file()
                    for path in (
                        "sources/houses/courtyard.blend",
                        "sources/shared/ornament.blend",
                        "sources/shared/checker.png",
                    )
                )
            ),
        )
        self.check(
            "selection leaves far model cold", not (alice / "sources/nature/tree.blend").exists()
        )
        self.check(
            "hydration remains Git clean",
            not self.git(alice, "status", "--porcelain")
            and (not self.git(bob, "status", "--porcelain")),
        )
        self.blender(alice, "--reopen", "house")
        self.blender(alice, "--reopen", "plaque")
        self.blender(bob, "--reopen", "bench")
        self.check("fresh clone restores original scenes and shared dependencies", True)
        (alice / "README.md").write_text("# Common garden\n\nAlice's authoring notes.\n")
        self.rou(alice, "add", "README.md")
        self.blender(alice, "--edit", "house")
        text_commit = self.rou(alice, "commit", "-m", "Notes before saved model edit")["commit"]
        self.check(
            "README-only commit keeps unstaged model edits",
            "courtyard.blend" in self.git(alice, "diff", "--name-only"),
        )
        self.rou(alice, "build")
        self.rou(alice, "add", "sources/houses", "runtime")
        house = self.rou(alice, "commit", "-m", "Add house ornament")["commit"]
        self.blender(bob, "--edit", "bench")
        self.rou(bob, "build")
        self.rou(bob, "add", "sources/props", "runtime")
        bench = self.rou(bob, "commit", "-m", "Extend the bench")["commit"]
        self.rou(bob, "push", "-u", "origin", "feature/bench")
        self.rou(alice, "fetch", "origin")
        self.check("fetch preserves author HEAD", self.git(alice, "rev-parse", "HEAD") == house)
        self.rou(alice, "merge", "origin/feature/bench")
        merge_path = Path(self.git(alice, "rev-parse", "--git-path", "MERGE_HEAD"))
        if not merge_path.is_absolute():
            merge_path = alice / merge_path
        self.check("divergent merge stays pending", merge_path.is_file())
        self.check(
            "independent contributions have no merge conflicts",
            not self.git(alice, "ls-files", "-u"),
        )
        merged = self.rou(alice, "commit", "-m", "Merge independent contributions")["commit"]
        self.check("independent merge commits without rebuilding", bool(merged))
        parents = self.git(alice, "rev-list", "--parents", "-n", "1", merged).split()[1:]
        self.check("merge preserves both Git parents", set(parents) == {house, bench})
        self.rou(alice, "validate", merged)
        difference = self.rou(alice, "diff", "--realm", first, merged)
        changed = {entity["id"] for entity in difference["changed_entities"]}
        self.check(
            "semantic diff identifies both model contributions", {"house", "bench"} <= changed
        )
        self.check(
            "merge retains sparse-away entity", (alice / "realm/entities/far_tree.json").exists()
        )
        self.rou(alice, "push", "-u", "origin", "feature/house")
        initial_view, merged_view = (
            self.directory / "initial-view",
            self.directory / "merged-view",
        )
        self.rou(official, "view", first, "--all", "--output", str(initial_view))
        prepared = self.rou(alice, "view", "HEAD", "--all", "--output", str(merged_view))
        self.check(
            "explicit all includes four models", len(list((merged_view / "objects").iterdir())) == 4
        )
        self.check(
            "view resolves symbolic revision to immutable OID",
            prepared["version"]["commit_oid"] == merged,
        )
        self.check(
            "view contains no authoring or Git dependency",
            not list(merged_view.rglob("*.blend")) and (not (merged_view / ".git").exists()),
        )
        self.check(
            "view preserves exact versions",
            first != merged
            and (initial_view / "realm.json").is_file()
            and (merged_view / "realm.json").is_file(),
        )
        self.rou(alice, "sparse", "set", "--all")
        self.blender(alice, "--reopen", "far_tree")
        self.check(
            "whole-world selection restores remaining source",
            (alice / "sources/nature/tree.blend").is_file(),
        )
        resolved = self.rou(alice, "sparse", "set", "--chunk", "1,0,1..-1,0,-1")
        self.rou(alice, "sparse", "list")
        self.check("reversed range covers nine chunks", len(resolved["resolution"]["chunks"]) == 9)
        if not git_only:
            native.godot_extension()
            godot = ["godot"] + ([] if windowed else ["--headless"])
            self.run(
                godot
                + [
                    "--path",
                    str(ROOT / "client/app"),
                    "--script",
                    "../tests/git_view_workflow.gd",
                    "--",
                    "--initial=" + str(initial_view),
                    "--selected=" + str(selected_view),
                    "--updated=" + str(merged_view),
                    "--report=" + str(self.directory / "godot-report.json"),
                ]
            )
            self.check("Godot fixed-version workflow", True)
        (self.directory / "versions.json").write_text(
            json.dumps(
                {
                    "initial": first,
                    "notes": text_commit,
                    "house": house,
                    "bench": bench,
                    "merged": merged,
                    "initial_view": str(initial_view),
                    "merged_view": str(merged_view),
                },
                indent=2,
            )
            + "\n"
        )
        print(
            "GIT_REALM_GIT_ONLY_PASS" if git_only else "GIT_REALM_ACCEPTANCE_PASS",
            self.directory,
            flush=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/s1-git/verification")
    parser.add_argument("--windowed", action="store_true")
    parser.add_argument(
        "--git-only", action="store_true", help="Run Git/Blender checks without opening Godot"
    )
    args = parser.parse_args()
    run = args.output.resolve() / datetime.now(timezone.utc).strftime("run-%Y%m%dT%H%M%S%fZ")
    Acceptance(run).execute(args.windowed, args.git_only)


if __name__ == "__main__":
    main()
