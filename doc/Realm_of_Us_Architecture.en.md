# Realm of Us · Code Architecture

Baseline 2.1 · 2026-09-16. See [product scope](Realm_of_Us_Design.en.md), the [detailed Chinese architecture](Realm_of_Us_Architecture.zh-CN.md), the [repository contract](protocol/spec/s1-repository.zh-CN.md) and the [viewer bundle format](protocol/spec/s1-preview.zh-CN.md). The 2.0 Go/Python reference implementation and its [acceptance](experiments/S1-Git-results.zh-CN.md) were removed after Rust black-box acceptance; see the [migration record](experiments/rust-migration-status.zh-CN.md).

## Components

All Rust code lives in the Cargo workspace under `rou/`. The root manifest only defines members and shared configuration. `app/` is the `rou` CLI package; `crates/` contains `rou-core`, `rou-git`, `rou-ffi` and `rou-godot`, each with its own manifest and `src/`. Core and CLI tests live with their packages, sharing helpers under `rou/tests/common/`. Fuzz targets, generated headers and the shared target directory remain under `rou/`; see the [module guide](../rou/README.md).

One Cargo workspace; every rule is implemented once in `rou_core`, and every host reaches it through one operation table.

| Module | Artifact | Responsibility |
| --- | --- | --- |
| `rou/crates/rou-core/src` | `rou_core` | Spatial rules, strict JSON and RFC 8785 canonical encoding, SHA-256, GLB precheck and budgets, catalog/version validation, ID/path/LFS-pointer rules, projections, build-record freshness, domain diff; the `ops` module exposes them as `dispatch(op, input) -> Result<Vec<u8>>`; one serde type per document (unknown fields rejected) is the shape layer, and no JSON Schema is committed or loaded. No IO, no global state |
| `rou/crates/rou-git/src` | `rou_git` | git/git-lfs subprocess wrappers, snapshots (commit/index/worktree), sparse and payload hydration, write lock with signal cleanup, lineage and source-hash caches, build orchestration and headless Blender invocation |
| `rou/app/src/main.rs` | `rou` binary | All commands, using the typed API directly |
| `rou/crates/rou-ffi/src` | cdylib/staticlib `librou` | C ABI `rou_call`/`rou_free`/`rou_abi_version`, forwarding to `rou_core::ops`; header via cbindgen |
| `rou/crates/rou-godot/src/lib.rs` | cdylib + `.gdextension` | GDExtension class `Rou` with `invoke(op, bytes) -> Dictionary` and `abi_version()`, forwarding to `rou_core::ops` |

Hosts: the CLI uses the library modules directly. The Blender add-on (`toolchain/blender/rou_blender/`, packaged as `rou_blender.zip`) keeps only bpy logic and loads the platform-specific `librou` shipped inside the add-on through `ctypes`. The headless export helper reports explicit Z-up metre `bounds_blender_m`, converted by Rust `blender::bounds_native`. Build and preview share static export settings, include hidden meshes and restore scene visibility. Source sessions validate only related records and cache file reads by identity, size and nanosecond modification/change timestamps. The helper only exports GLBs and reports bounds/dependencies; rou validates the GLB afterwards. The Godot viewer keeps GDScript for camera, scheduling and UI and calls `Rou.invoke` for every rule. Start locations do not come from realm data: the default is derived from the union of entity bounds, the player can pick a place from the chunk map, the entity list or typed coordinates, and recent positions live in the local user directory. Python acceptance drivers under `toolchain/build/` drive rou, Blender and Godot as black boxes.

rou invokes native git/git-lfs and, only for geometry builds, headless Blender. Commands use argument arrays and explicit cwd. Git output is read with `-z` and handled as bytes.

## Authority and files

Git owns the only tree/index/history/refs/reflog. LFS pointers name raw SHA-256 payloads and sizes. A realm has three layers: `realm/models/<id>.json` defines a model (source .blend, entry, dependencies), `realm/entities/<id>.json` defines placement (model ID, millimeter position), `sources/` holds the author's complete projects; `realm.json` holds only format/id/profile and is immutable after init; `runtime/<model>.glb` plus the record `runtime/<model>.json` are the complete build output. Authors write only the first three; rou writes only runtime/. The complete view/chunk index is recomputed in memory by `projections`; the view root is SHA-256 over JCS canonical JSON. Every document `format` identifier and repository path constant is defined once in the `formats` and `paths` modules of `rou_core`; the full list is in the repository contract. Versioned JSON is written with one fixed multi-line layout (sorted keys, two-space indent, one array element per line) so Git can merge different fields of one file; hashes use JCS and are independent of layout. Inputs cannot hash their own outputs and view roots cannot contain the carrying commit.

All strict-JSON and GLB budgets are defined once in `rou_core`: duplicate keys, non-finite or overflowing numbers, non-UTF-8 encodings, depth/size limits, top-level array and image budgets. Hosts share the implementation, so nothing is committable yet unviewable.

## Version operations and assets

The Git cone always contains realm/, runtime/ and docs/; unselected GLBs under runtime/ are pointers. The selection decides which sources directories join the cone and which source files and GLBs are hydrated, so build never has to widen the cone. No-checkout clone with local skip-smudge, byte verification after materialization, shrink preflight for local changes and ignored author backups, and no destructive reset as hidden rollback keep the 2.0 semantics.

Bulk object reads use `git cat-file --batch` or `gix` rather than one subprocess per file. Worktree source hashes are cached under .rou keyed by path, size, mtime, ctime and inode. Frozen files and LFS object hashes remain in snapshot-local memory. Lineage results are cached per commit OID. The write lock records its PID, is removed on Ctrl+C/SIGTERM, and stale locks are detected by checking process liveness.

## Build and validation

Build freezes inputs under .rou/build, opens each source .blend once for all models it hosts, exports per-model GLBs, writes GLBs and records to runtime/ without staging, and loads the world snapshot once. Validate checks schema, immutable lineage, unique IDs, references, freshness, LFS pointers, GLB bytes and the complete spatial derivation. Commit validates the index and then the actual created commit.

## Merge and push

Independent entity/model contributions, including different entities in the same chunk, merge without a shared index rebuild. Same-model record/GLB conflicts and binary source conflicts are resolved by authors, then rebuilt. Push pins the validated OID in its refspec; LFS requirements follow newly reachable history.

## Operation table and FFI contract

`rou_core::ops` implements `dispatch(op: &str, input: &[u8]) -> Result<Vec<u8>, Error>` once; input is JSON or raw bytes (GLB), output is a JSON value. It is the only normative surface for hosts and is tested with one set of JSON fixtures.

```c
uint32_t rou_abi_version(void);
int32_t  rou_call(const char *op, const uint8_t *input, size_t input_len, RouBuf *out);
void     rou_free(RouBuf buf);
typedef struct { uint8_t *ptr; size_t len; } RouBuf;
```

JSON envelope out: `{"ok":true,"value":...}` or `{"ok":false,"error":{"code","message"}}`. Output is allocated by the library and released with `rou_free`. Status 0 carries a success envelope and 1 an error envelope; 2 means invalid arguments and 3 a caught panic, with empty output in both cases (an invalid output pointer cannot be written). Panics are caught inside the library and never cross the boundary. No global mutable state; no file or network IO inside the library. New operations add an op name without changing the signature; incompatible changes bump the ABI version. Python binds with `ctypes`; a PyO3 `abi3` module can be added later without touching `rou_core`.

`rou-godot` does not go through the C ABI: it links `rou_core` and exposes `Rou.invoke(op: String, input: PackedByteArray) -> Dictionary` plus `abi_version()`. Rust converts the JSON value into a Godot Dictionary, so GDScript never parses JSON, and byte arrays are passed as zero-copy slices. Python and Godot therefore share one set of op names and semantics; `load_catalog(bytes)` builds an immutable Rust `viewer::Catalog`, and `catalog_query(position_bytes)` reuses it for streaming queries without serializing or revalidating the whole manifest. The same query implementation backs `viewer.query`. The generic adapter and ABI check are cached.

Local Git caches have a separate 128 MiB limit and are disposable on corruption, oversize or write failure. LFS storage resolution is cached once per snapshot and refreshed for a new snapshot. Source hashes invalidate on inode/size/mtime/ctime and retain at most 100,000 entries with least-recently-used eviction; lineage entries use immutable commit OIDs. Sparse shrink protects only tracked directories actually leaving the cone. Reusing a matching runtime pointer never downloads or materializes it. A deleted or modified unselected runtime file is rejected with instructions to select its entity before repair. Hydration resets stale pointer stats, then refreshes verified payload stats without staging; its temporary index is cleaned up on success, failure or signals. `rou-git` public APIs have English Rustdoc enforced by `deny(missing_docs)`; `error::code` exposes stable categories while retaining diagnostic context. Validation, browse export and read-only LFS commands do not acquire the author writer lock.

The viewer checks bundle layout and symlinks at the bundle root and inside it before committing a new catalog. Symlink ancestors outside the bundle are allowed; ordinary `.DS_Store` files at the root and in `objects/` are ignored. Failed opens preserve the previous world.

## S2 and platform boundaries

rou-node, the LFS transfer adapter, release records, signaling/TURN and optional RealmHub keep the 2.0 boundaries. No asset encryption; separate author, maintainer, peer and administration credentials; pins fix commit and asset scope; drafts and caches are never auto-public. S2 node and transport code is also Rust.

## Migration

Terminology and path mapping from 2.0: `realm/authoring/<id>.json` → `realm/models/<id>.json` (`rou-model/0.1`); entity field `binding` → `model`; `realm/derived/builds/<binding>.json` → `runtime/<model>.json` with field `model`; `realm/derived/` disappears. Paths and format strings live in one constants module of `rou_core`.

1. `rou_core` ported from `toolchain/realm_format` and the pure parts of `toolchain/git_realm` using the new layout, with the `ops` table, tested with the reviewed cases from `toolchain/fixtures/spatial.json` (copied into the crate, `catalogs` rewritten for the 2.1 bundle shape) and ported Python cases, fuzzed on JSON and GLB entry points; `shape_accepted` in the profile cases means “parses into the profile type”, `accepted` means both layers pass.
2. `rou_git` and `rou/app/src/main.rs` ported from `repository/` and the orchestration in `toolchain/git_realm`; the calibration generator emits the new layout; verified with `toolchain/build/verify_git_realm.py`.
3. `rou-ffi` and the add-on binding: rename the package to `rou_blender`, replace its `realm_format` calls with `rou_call`, add a linked-library authoring test and per-platform packaging.
4. `rou-godot` and viewer wiring, including the Godot test scripts and viewer packaging.
5. Delete `repository/`, `toolchain/cmd/rou/`, `go.mod`, `toolchain/git_realm/`, `toolchain/realm_format/`, their unit tests, `toolchain/fixtures/spatial.json` (already copied into the crate) and the Go parts of `check_code.py`.

No compatibility with the 2.0 layout, shared index files, old view-root hashes or the earlier custom `.rou/repository.json`. Review-confirmed defects are fixed during the migration with regression cases. Existing black-box harnesses remain the acceptance baseline; only the current Mac is required.
