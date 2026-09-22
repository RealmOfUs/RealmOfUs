"""Prepare the local Forgejo instance: users, API tokens and the shared realm repository.

Runs after `docker compose up`. Idempotent: existing users, tokens and repositories are
reused. Credentials go to .cache/forgejo/credentials.json, which is ignored by Git.
"""

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from base64 import b64encode
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COMPOSE = [
    "docker",
    "compose",
    "-p",
    "rou-forgejo",
    "-f",
    str(ROOT / "toolchain/forgejo/compose.yaml"),
]
CREDENTIALS = ROOT / ".cache/forgejo/credentials.json"
USERS = {
    "realm": {"admin": True, "collaborator": False},
    "alice": {"admin": False, "collaborator": True},
    "bob": {"admin": False, "collaborator": True},
    "mallory": {"admin": False, "collaborator": False},
}
SCOPES = ["write:repository", "write:issue", "write:user", "read:user", "write:organization"]


def api(
    url: str, method: str = "GET", body: dict | None = None, auth: str | None = None
) -> tuple[int, dict | list | None]:
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(url, data=data, method=method)
    request.add_header("Content-Type", "application/json")
    if auth:
        request.add_header("Authorization", auth)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            text = response.read().decode()
            return response.status, (json.loads(text) if text else None)
    except urllib.error.HTTPError as error:
        text = error.read().decode(errors="replace")
        try:
            return error.code, json.loads(text)
        except json.JSONDecodeError:
            return error.code, {"message": text}


def basic(user: str, password: str) -> str:
    return "Basic " + b64encode(f"{user}:{password}".encode()).decode()


def wait_ready(base: str, seconds: int = 120) -> None:
    deadline = time.time() + seconds
    while time.time() < deadline:
        try:
            status, _ = api(base + "/api/v1/version")
            if status == 200:
                return
        except (urllib.error.URLError, ConnectionError, TimeoutError):
            pass
        time.sleep(2)
    raise SystemExit(f"Forgejo did not answer at {base} within {seconds} s")


def ensure_user(name: str, password: str, admin: bool) -> None:
    command = COMPOSE + [
        "exec",
        "-T",
        "-u",
        "git",
        "forgejo",
        "forgejo",
        "admin",
        "user",
        "create",
        "--username",
        name,
        "--password",
        password,
        "--email",
        f"{name}@example.invalid",
        "--must-change-password=false",
    ]
    if admin:
        command.append("--admin")
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0 and "already exists" not in result.stdout + result.stderr:
        raise SystemExit(f"creating user {name} failed:\n{result.stdout}\n{result.stderr}")


def ensure_token(base: str, name: str, password: str) -> str:
    auth = basic(name, password)
    status, tokens = api(f"{base}/api/v1/users/{name}/tokens", auth=auth)
    if status == 200:
        for token in tokens:
            if token["name"] == "rou-acceptance":
                api(f"{base}/api/v1/users/{name}/tokens/{token['id']}", "DELETE", auth=auth)
    status, created = api(
        f"{base}/api/v1/users/{name}/tokens",
        "POST",
        {"name": "rou-acceptance", "scopes": SCOPES},
        auth=auth,
    )
    if status != 201:
        raise SystemExit(f"token for {name} failed: {status} {created}")
    return created["sha1"]


def ensure_repo(base: str, owner: str, token: str, name: str) -> None:
    auth = "token " + token
    status, _ = api(f"{base}/api/v1/repos/{owner}/{name}", auth=auth)
    if status == 404:
        status, body = api(
            f"{base}/api/v1/user/repos",
            "POST",
            {"name": name, "private": False, "auto_init": False, "default_branch": "main"},
            auth=auth,
        )
        if status != 201:
            raise SystemExit(f"repository {name} failed: {status} {body}")
    for user, spec in USERS.items():
        if spec["collaborator"]:
            status, body = api(
                f"{base}/api/v1/repos/{owner}/{name}/collaborators/{user}",
                "PUT",
                {"permission": "write"},
                auth=auth,
            )
            if status not in (201, 204):
                raise SystemExit(f"collaborator {user} failed: {status} {body}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://localhost:3000")
    parser.add_argument("--repo", default="garden")
    args = parser.parse_args()
    wait_ready(args.url)
    CREDENTIALS.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(CREDENTIALS.read_text()) if CREDENTIALS.exists() else {}
    passwords = existing.get("passwords") or {name: f"{name}-local-password-1" for name in USERS}
    for name, spec in USERS.items():
        ensure_user(name, passwords[name], spec["admin"])
    tokens = {name: ensure_token(args.url, name, passwords[name]) for name in USERS}
    ensure_repo(args.url, "realm", tokens["realm"], args.repo)
    CREDENTIALS.write_text(
        json.dumps(
            {
                "url": args.url,
                "repo": f"realm/{args.repo}",
                "passwords": passwords,
                "tokens": tokens,
            },
            indent=2,
        )
        + "\n"
    )
    CREDENTIALS.chmod(0o600)
    print("FORGEJO_READY", args.url, f"realm/{args.repo}", ",".join(USERS))


if __name__ == "__main__":
    sys.exit(main())
