"""Two-author acceptance over HTTP against the local Forgejo started by `make forgejo-up`.

Reuses the calibration realm and the command helpers of verify_git_realm.py. Verifies what a
local bare remote cannot: HTTP transport with credentials, anonymous clone and LFS download of a
public repository, platform-side pull-request merges without any local rebuild, and write
permissions. Requires .cache/forgejo/credentials.json written by toolchain/forgejo/bootstrap.py.
"""

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from toolchain.build import native  # noqa: E402
from toolchain.build.verify_git_realm import Acceptance  # noqa: E402
from toolchain.build.verify_online_viewer import verify as verify_online_viewer  # noqa: E402

CREDENTIALS = ROOT / ".cache/forgejo/credentials.json"


class Forgejo:
    def __init__(self, credentials: dict):
        self.url = credentials["url"].rstrip("/")
        self.repo = credentials["repo"]
        self.tokens = credentials["tokens"]

    def api(self, path: str, method: str = "GET", body: dict | None = None, user: str = "realm"):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(self.url + "/api/v1" + path, data=data, method=method)
        request.add_header("Content-Type", "application/json")
        request.add_header("Authorization", "token " + self.tokens[user])
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                text = response.read().decode()
                return response.status, (json.loads(text) if text else None)
        except urllib.error.HTTPError as error:
            text = error.read().decode(errors="replace")
            try:
                return error.code, json.loads(text)
            except json.JSONDecodeError:
                return error.code, {"message": text}

    def remote(self, user: str | None) -> str:
        parsed = urllib.parse.urlsplit(self.url)
        netloc = parsed.netloc if user is None else f"{user}:{self.tokens[user]}@{parsed.netloc}"
        return urllib.parse.urlunsplit((parsed.scheme, netloc, f"/{self.repo}.git", "", ""))

    def lfs_batch(self, oid: str, size: int, user: str | None = None) -> tuple[int, dict]:
        request = urllib.request.Request(
            f"{self.url}/{self.repo}.git/info/lfs/objects/batch",
            data=json.dumps(
                {"operation": "download", "objects": [{"oid": oid, "size": size}]}
            ).encode(),
            method="POST",
        )
        request.add_header("Content-Type", "application/vnd.git-lfs+json")
        request.add_header("Accept", "application/vnd.git-lfs+json")
        if user:
            request.add_header("Authorization", "token " + self.tokens[user])
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, json.loads(response.read().decode())
        except urllib.error.HTTPError as error:
            return error.code, {}


def pointer(acceptance: Acceptance, repo: Path, path: str) -> tuple[str, int]:
    """Read the committed LFS pointer; the working tree holds the materialised bytes."""
    lines = acceptance.git(repo, "show", f"HEAD:{path}").splitlines()
    oid = next(line.split("sha256:")[1] for line in lines if line.startswith("oid "))
    size = int(next(line.split(" ")[1] for line in lines if line.startswith("size ")))
    return oid, size


def merge_pull_request(
    forgejo: Forgejo, head: str, title: str, author: str, acceptance: Acceptance
) -> dict:
    status, pull = forgejo.api(
        f"/repos/{forgejo.repo}/pulls",
        "POST",
        {"head": head, "base": "main", "title": title},
        user=author,
    )
    acceptance.check(f"pull request opened by {author}: {title}", status == 201)
    index = pull["number"]
    for _ in range(30):
        status, current = forgejo.api(f"/repos/{forgejo.repo}/pulls/{index}")
        if current.get("mergeable"):
            break
        time.sleep(1)
    acceptance.check(
        f"platform reports the pull request mergeable: {title}", bool(current.get("mergeable"))
    )
    status, body = forgejo.api(
        f"/repos/{forgejo.repo}/pulls/{index}/merge",
        "POST",
        {"Do": "merge", "merge_message_field": f"Merge {head}"},
    )
    detail = "" if status == 200 else f" (HTTP {status}: {str(body)[:200]})"
    acceptance.check(f"platform merged without local rebuild: {title}{detail}", status == 200)
    return current


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/forgejo/verification")
    args = parser.parse_args()
    if not CREDENTIALS.exists():
        print("FORGEJO_SKIPPED no .cache/forgejo/credentials.json; run `make forgejo-up` first")
        return
    forgejo = Forgejo(json.loads(CREDENTIALS.read_text()))
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    acceptance = Acceptance(args.output / f"run-{stamp}")
    acceptance.commands_scope = "local Forgejo over HTTP"
    import shutil

    shutil.copy2(native.cli(), acceptance.binary)
    base = acceptance.directory
    # Fresh repository state for every run: delete and recreate through the API.
    forgejo.api(f"/repos/{forgejo.repo}", "DELETE")
    owner, name = forgejo.repo.split("/")
    status, _ = forgejo.api(
        "/user/repos",
        "POST",
        {"name": name, "private": False, "auto_init": False, "default_branch": "main"},
    )
    acceptance.check("public repository created through the API", status == 201)
    for user in ("alice", "bob"):
        status, _ = forgejo.api(
            f"/repos/{forgejo.repo}/collaborators/{user}", "PUT", {"permission": "write"}
        )
        acceptance.check(f"{user} granted write access", status in (201, 204))

    official = base / "official"
    official.mkdir()
    acceptance.rou(official, "init")
    acceptance.blender(official)
    acceptance.rou(official, "build")
    acceptance.rou(official, "add", ".")
    first = acceptance.rou(official, "commit", "-m", "Initial shared garden")["commit"]
    acceptance.rou(official, "remote", "add", "origin", forgejo.remote("realm"))
    acceptance.rou(official, "push", "-u", "origin", "main")
    house_oid, house_size = pointer(acceptance, official, "runtime/house.glb")
    status, batch = forgejo.lfs_batch(house_oid, house_size, user="realm")
    acceptance.check(
        "LFS object stored on the platform after push",
        status == 200 and "download" in batch["objects"][0].get("actions", {}),
    )

    alice, bob = base / "alice", base / "bob"
    acceptance.rou(base, "clone", forgejo.remote("alice"), str(alice))
    acceptance.rou(base, "clone", forgejo.remote("bob"), str(bob))
    acceptance.check(
        "clone over HTTP keeps payloads cold", (alice / "runtime/house.glb").stat().st_size < 200
    )
    acceptance.rou(alice, "switch", "-c", "feature/house")
    acceptance.rou(alice, "sparse", "set", "--chunk", "0,0,0")
    acceptance.check(
        "chunk selection downloaded the house source over HTTP",
        (alice / "sources/houses/courtyard.blend").stat().st_size > 1000,
    )
    acceptance.blender(alice, "--edit", "house")
    acceptance.rou(alice, "build")
    acceptance.rou(alice, "add", "sources/houses", "runtime")
    house = acceptance.rou(alice, "commit", "-m", "Add house ornament")["commit"]
    acceptance.rou(alice, "push", "-u", "origin", "feature/house")

    acceptance.rou(bob, "switch", "-c", "feature/bench")
    acceptance.rou(bob, "sparse", "set", "--entity", "bench")
    acceptance.blender(bob, "--edit", "bench")
    acceptance.rou(bob, "build")
    acceptance.rou(bob, "add", "sources/props", "runtime")
    bench = acceptance.rou(bob, "commit", "-m", "Extend the bench")["commit"]
    acceptance.rou(bob, "push", "-u", "origin", "feature/bench")

    merge_pull_request(forgejo, "feature/bench", "Extend the bench", "bob", acceptance)
    merge_pull_request(forgejo, "feature/house", "Add house ornament", "alice", acceptance)

    acceptance.rou(alice, "switch", "main")
    acceptance.rou(alice, "pull")
    merged = acceptance.git(alice, "rev-parse", "HEAD")
    acceptance.check(
        "pull fast-forwarded to the platform merge", merged not in (first, house, bench)
    )
    parents = acceptance.git(alice, "rev-list", "--parents", "-n", "1", merged).split()[1:]
    acceptance.check("platform merge has two parents", len(parents) == 2)
    report = acceptance.rou(alice, "validate", merged)
    acceptance.check(
        "platform-merged revision validates without a local build",
        report["ok"] and report["entity_count"] == 4,
    )
    difference = acceptance.rou(alice, "diff", "--realm", first, merged)
    changed = {item["id"] for item in difference["changed_entities"]}
    acceptance.check(
        "both contributions present after platform merge", {"house", "plaque", "bench"} <= changed
    )
    view = acceptance.rou(alice, "view", merged, "--all", "--output", str(base / "merged-view"))
    acceptance.check(
        "view of the platform merge includes every model",
        view["included"] == 4 if "included" in view else view["entity_count"] == 4,
    )

    release = acceptance.rou(alice, "release", "create", "v1")
    expected_address = f"rou://{urllib.parse.urlsplit(forgejo.url).netloc}/{forgejo.repo}@v1"
    acceptance.check(
        "browse tag binds the merged commit and derives the viewer address without pushing",
        release["ok"]
        and release["commit_oid"] == merged
        and release["address"] == expected_address
        and acceptance.git(alice, "ls-remote", "--tags", "origin", "v1") == "",
    )
    retried = acceptance.rou(alice, "release", "create", "v1")
    acceptance.check(
        "browse tag creation is idempotent for a matching tag",
        retried["commit_oid"] == merged
        and retried["view_root_sha256"] == release["view_root_sha256"],
    )
    listed = acceptance.rou(alice, "release", "list")
    acceptance.check(
        "release list reports the view-root annotation",
        listed
        == [
            {
                "tag": "v1",
                "commit_oid": merged,
                "view_root_sha256": release["view_root_sha256"],
            }
        ],
    )
    # Publishing is explicit: the validated branch, its LFS objects and reachable annotated tags.
    acceptance.rou(alice, "push", "--follow-tags")
    # An annotated tag advertises its tag object under refs/tags/<tag>; the commit is
    # only on the peeled refs/tags/<tag>^{} line.
    acceptance.check(
        "tag published explicitly with git push",
        merged in acceptance.git(alice, "ls-remote", "--tags", "origin", "v1^{}"),
    )
    acceptance.git(alice, "tag", "-a", "ordinary", "-m", "No browse annotation", merged)
    acceptance.git(alice, "push", "origin", "ordinary")
    acceptance.check(
        "newer ordinary annotated tag exists for browse discovery filtering",
        merged in acceptance.git(alice, "ls-remote", "--tags", "origin", "ordinary^{}"),
    )
    online = verify_online_viewer(release["address"], base / "merged-view", base / "online-viewer")
    acceptance.check(
        "standalone viewer opens hosted and static sources with verified cache recovery",
        online["second_downloaded_bytes"] == 0
        and online["commit_oid"] == merged
        and online["view_root_sha256"] == release["view_root_sha256"],
    )

    anonymous = base / "anonymous"
    acceptance.rou(base, "clone", forgejo.remote(None), str(anonymous))
    acceptance.rou(anonymous, "sparse", "set", "--entity", "bench")
    acceptance.check(
        "anonymous clone downloaded LFS payloads of a public repository",
        (anonymous / "sources/props/bench.blend").stat().st_size > 1000
        and (anonymous / "runtime/bench.glb").stat().st_size > 1000,
    )
    status, batch = forgejo.lfs_batch(house_oid, house_size, user=None)
    acceptance.check(
        "anonymous LFS batch download is offered for a public repository",
        status == 200 and "download" in batch["objects"][0].get("actions", {}),
    )

    mallory = base / "mallory"
    acceptance.rou(base, "clone", forgejo.remote("mallory"), str(mallory))
    acceptance.rou(mallory, "switch", "-c", "feature/intruder")
    (mallory / "docs").mkdir(exist_ok=True)
    (mallory / "docs/note.md").write_text("unauthorised\n")
    acceptance.rou(mallory, "add", "docs/note.md")
    acceptance.rou(mallory, "commit", "-m", "Unauthorised note")
    acceptance.rou(mallory, "push", "-u", "origin", "feature/intruder", expected=None)
    status, branches = forgejo.api(f"/repos/{forgejo.repo}/branches")
    acceptance.check(
        "non-collaborator push rejected by the platform",
        all(branch["name"] != "feature/intruder" for branch in branches),
    )

    (base / "summary.json").write_text(
        json.dumps(
            {
                "format": "rou-forgejo-acceptance/0.1",
                "url": forgejo.url,
                "repo": forgejo.repo,
                "checks": len(acceptance.checks),
                "commands": len(acceptance.commands),
                "initial": first,
                "merged": merged,
                "online": online,
            },
            indent=2,
        )
        + "\n"
    )
    print(
        "FORGEJO_ACCEPTANCE_PASS",
        base,
        len(acceptance.checks),
        "checks",
        len(acceptance.commands),
        "commands",
    )


if __name__ == "__main__":
    main()
