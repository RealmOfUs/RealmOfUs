> 2.0 源码删除前的历史说明，仅供旧报告引用；当前实现见[根 README](../../README.md)。

> 2.1（2026-09-16）：本目录属于 2.0 Go/Python 参考实现，将由 Rust crates 替换并在迁移完成后删除；目标结构与迁移顺序见 [架构文档](../Realm_of_Us_Architecture.zh-CN.md) §1 与 §10。迁移期间以下说明仍描述可运行的当前代码。

# Native Git / Git LFS authoring backend

Git owns the complete authoring tree, index, commits, branches, merges, and reflog. This package coordinates the native `git` and `git lfs` executables; it does not implement another version-control database. The ignored `.rou/` directory contains spatial selections, build/view caches, and editor session state.

The Python domain implementation is `toolchain/git_realm`. It resolves chunk/entity selections, builds the Realm, verifies staged inputs and fixed commits, and prepares standalone viewing catalogs. Authoritative lightweight records live under `realm/`; original projects remain at author-selected paths such as `sources/house/house.blend`.

| File | Responsibility |
| --- | --- |
| `git.go` | Native subprocesses, repository discovery, caller-relative paths, Python delegation, writer lock |
| `init.go` | Git/LFS installation, metadata-only clone, ignore/attribute templates |
| `selection.go` | Persistent spatial selection, domain dependency resolution, Git cone preparation, shrink protection |
| `lfs.go` | LFS pointer parsing, explicit selected hydration, materialized SHA-256/size verification |
| `workflow.go` | Status, staged pre/post commit validation, branch/revision switching, merge, fetch/pull/push |
| `git_integration_test.go` | Real Git/LFS local-remote tests, binary conflicts, sparse behavior, validation races, historical fork availability |

The common user interface is `toolchain/cmd/rou`. Run `rou help` for commands. Git operations on user paths (`add`, `rm`, `mv`, `restore`, `diff`, and similar commands) preserve the invocation directory. Domain operations always use the discovered repository root. Git config and credential helpers retain their usual roles; use `rou config user.name ...` and `rou config user.email ...` for author identity.

```bash
go build -o build/bin/rou ./toolchain/cmd/rou
export ROU_TOOLCHAIN_ROOT="$PWD"
export PATH="$PWD/build/bin:$PATH"

mkdir build/example-realm
pushd build/example-realm
rou init
rou build
rou add .
rou commit -m "Initialize Realm"
rou switch -c feature/house
popd
```

`ROU_PYTHON` optionally selects the Python executable. A development binary inside this project can discover the toolchain from its executable location; separately installed binaries need `ROU_TOOLCHAIN_ROOT` or an installed `toolchain` Python package. Blender is needed only when building changed geometric inputs, not for ordinary README edits.

Inherited Git repository-routing variables such as `GIT_DIR` and `GIT_INDEX_FILE` are cleared consistently in Go and Python, so validation and commit cannot accidentally use different indexes. Author identity, credential helpers, and ordinary Git configuration remain available.

`rou clone` starts without heavy payloads. `rou sparse set/add` resolves the selected entities and their source dependencies, prepares Git cone directories, and explicitly fetches/materializes only those LFS paths. `rou sparse set --all` materializes the current Realm's authoring closure. It does not retrieve every historical object. Cone presentation can expose adjacent pointer files; a file's extension is not evidence that its payload is available. Dirty authoring files survive selection expansion, and shrink rejects dirty work and ignored backups that could be removed.

Build outputs enter the working tree without implicit staging. `rou add` stages the normal Git tree and LFS pointers. `rou commit` checks the staged logical Realm, detects an index/HEAD race, creates a normal Git commit, then verifies the actual resulting commit. Unstaged later edits remain in the working tree. A post-commit failure explicitly reports the existing commit; the tool does not claim to have rolled it back.

Only per-binding build records under `realm/derived/builds/` and runtime assets are written as versioned build outputs. Complete view and chunk indexes are derived from the selected snapshot in memory. `view_root_sha256` hashes canonical view JSON plus a newline. Existing Git commits containing shared indexes remain readable; `rou build` removes tracked legacy index files from the working tree and reports those deletions without staging them or deleting untracked files.

`rou checkout BRANCH` stays attached; `rou checkout COMMIT` inspects a detached revision; `rou switch -c BRANCH` creates a feature branch. A version change marks the previous editor session stale and prepares the persistent selection. Native Git refuses conflicting worktree overwrites. If subsequent LFS preparation fails, the completed Git switch is retained and the error identifies the missing preparation; this is not an atomic Git+network transaction.

Default `rou diff` retains native Git's positional revision and index/worktree semantics. `rou diff --realm [--staged] [A [B]]` adds a read-only domain summary of entities, placement, bindings, and source/dependency versions. It does not hydrate assets or run Blender. `rou view [REVISION] [--output PATH] [--chunk RANGE]` prepares a fixed committed view bundle; `--worktree` explicitly selects built working files instead. View output paths are relative to the caller, and the standalone Godot app consumes the resulting catalog without Git or Blender.

`rou merge` fast-forwards when possible. A divergent merge stays pending for review and commit. Independent entity/binding contributions, including different entities in the same chunk, merge without rebuilding a shared index. Source or per-binding output conflicts still require resolution, `rou build`, and staging before commit. Same-file binary edits are not automatically reconciled. `rou merge --abort` delegates native Git safeguards.

`rou fetch` updates Git metadata without implicit LFS downloads. `rou pull` uses a fixed fetched target and fast-forward-only integration. `rou push` validates an immutable commit, explicitly provides all reachable LFS history needed at the destination, and updates the remote using that exact commit OID. A new fork may require downloading missing old objects from the source remote first. Git/LFS publication is not an atomic distributed release. `rou lfs` exposes standard LFS diagnostics and explicit recovery commands.

Writer locks coordinate rou processes only. Native Git/IDE operations remain possible, so validation and publication use immutable targets and repeat state checks. A terminated process can leave `.rou/write.lock`; check its recorded PID before removing a stale file. Status distinguishes Git changes, local selection, and LFS presence; it does not silently perform integrity validation or download payloads.

There is no old CAS reader, compatibility converter, or dual backend. Formal release signatures, public P2P providers, authorization, network pin/availability, and hostile remote execution boundaries belong to S2.

```bash
go test ./repository ./toolchain/cmd/rou
go vet ./repository ./toolchain/cmd/rou
```
