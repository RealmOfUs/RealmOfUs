# Realm of Us · Code Architecture

Baseline 2.1. Scope is defined in [design.md](design.md); data contracts in [protocol/](protocol/README.md); the walkthrough in [tutorial.md](tutorial.md); the current acceptance state in [status.md](status.md).

## 1. Modules and processes

All Rust lives in the Cargo workspace under `rou/`. The root `Cargo.toml` only lists members and shared settings; `app/` is the CLI package `rou`, and `crates/` holds `rou-core`, `rou-git`, `rou-ffi` and `rou-godot`. Every package owns its manifest and sources; core and CLI tests live with their packages, shared test helpers in `rou/tests/common/`. Fuzz targets, the generated C header and the shared `target/` also live under `rou/`; see the [workspace guide](../rou/README.md).

Every rule is implemented once in `rou_core`, and every host reaches it through one operation table:

| Module | Artifact | Responsibility | Dependencies |
| --- | --- | --- | --- |
| `rou/crates/rou-core` | `rou_core` | Spatial rules, strict JSON and RFC 8785 canonical encoding, SHA-256, GLB precheck and budgets, catalog and version-sidecar validation, ID/path/LFS-pointer rules, projections, build-record freshness, domain diff; the `ops` module exposes them as `dispatch(op, input) -> Result<Vec<u8>>`; one serde type per document (unknown fields rejected) is the shape layer, no JSON Schema is committed or loaded | No IO, no global state; `serde`, `serde_json`, `sha2`, `thiserror` |
| `rou/crates/rou-git` | `rou_git` | git/git-lfs subprocess wrappers, snapshots (commit, index, worktree), sparse selection and payload hydration, write lock with signal cleanup, lineage and source-hash caches, build orchestration and headless Blender invocation | `rou_core`, `std::process`, `bstr`, `ctrlc` |
| `rou/app` | binary `rou` | All commands, help and human-readable reports, using the typed API directly | `clap`, `rou_git` |
| `rou/crates/rou-ffi` | cdylib/staticlib `librou` | C ABI `rou_call`, `rou_free`, `rou_abi_version`, forwarding to `rou_core::ops`; `cbindgen` generates `include/rou.h` | `rou_core` only |
| `rou/crates/rou-godot` | cdylib + `.gdextension` | GDExtension class `Rou` with `invoke(op, bytes) -> Dictionary` and `abi_version()`, forwarding to `rou_core::ops` | `godot` (godot-rust), `rou_core` |

Hosts and the core:

| Host | Language | Binding |
| --- | --- | --- |
| `rou` CLI | Rust | Links `rou_git`/`rou_core` directly, no FFI |
| Blender add-on `toolchain/blender/rou_blender/` | Python, bpy logic only | `ctypes` loads the platform `librou` shipped inside the add-on; every rule goes through `rou_call` |
| Export helper `toolchain/blender/export_git_realm.py` | Python | Run by `rou build` inside headless Blender; exports full/lod1 GLBs and uses librou for metrics; rou rechecks both outputs, metrics and dependencies afterwards |
| Godot viewer `client/app/` | GDScript for camera, scheduling and UI | Every validation and spatial conversion goes through `Rou.invoke`; no rule is duplicated |
| Acceptance drivers `toolchain/build/*.py` | Python | Drive rou, Blender and Godot as black boxes; contain no domain rules |

rou invokes native git and git-lfs, and headless Blender only for geometry builds. Commands use argument arrays and explicit working directories, never shell strings. A realm need not live inside the toolchain checkout. Git output is read with `-z` and handled as bytes; paths are decoded only for display.

## 2. Data and the single authority

Git tree/index/commit/refs/reflog are the only version state; LFS pointers pin payload SHA-256 and size. Both `.git` directories and gitfiles are supported; packs are never written directly. The public CLI discovers the repository from the current directory and has no `--repo`; internal components may take an explicit root.

A realm has three layers: `realm/models/<id>.json` defines a model (source `.blend`, entry, dependencies), `realm/entities/<id>.json` defines placement (model ID, millimetre position), `sources/` holds the authors' complete projects; `realm.json` holds only `format`, `id` and `profile` and is immutable after init; `runtime/<model>.glb` and `runtime/<model>.lod1.glb` plus the record `runtime/<model>.json` are the complete build output. Authors write the first three; rou writes only `runtime/`. The root `.gitignore` excludes `/.rou/`, which holds only the selection, session, build scratch, browse bundles, backups and hash caches. The complete view and chunk index are recomputed in memory by `projections` and never written to the working tree; the view root is SHA-256 over JCS canonical JSON. Every document `format` identifier and repository path constant is defined once in the `formats` and `paths` modules of `rou_core`. Versioned JSON is written with one fixed multi-line layout (sorted keys, two-space indent, one array element per line) so Git can merge different fields of one file; hashes use JCS and are independent of layout, and `rou_core` provides the only writer.

Git OIDs and asset SHA-256 are distinct and carry their algorithm. Inputs never hash their own outputs, and a view root never contains the commit that carries it. Safe relative paths keep real directories, reject escape and symbolic links, and never depend on absolute paths of an author's machine.

Space is millimetres, Y-up, right-handed, half-open AABBs; Blender coordinates are converted once. A GLB is local geometry; JSON supplies the world translation. Spanning entities are deduplicated by ID.

Parsing safety ceilings and advisory build references are separate groups in `rou_core::limits` and `config`; GLB metrics and warnings are computed by the core, while validate only reports correctness. No quality limit is a hard rejection and no budget field is added to Realm metadata. All strict-JSON budgets are defined once in `rou_core`: duplicate keys, non-finite or overflowing numbers, non-UTF-8 encodings, depth and size limits. GLB top-level arrays, image dimensions and decoded-pixel budgets are likewise defined once. All hosts share the implementation, so nothing is committable yet unviewable.

## 3. Git, LFS and sparse selection

`init` creates a standard Git repository with local skip-smudge, templates and an empty domain, without committing. `clone` uses no-checkout, installs the local LFS settings, configures the lightweight cone, then checks out. The cone always contains `realm/`, `runtime/` and `docs/`; unselected GLBs under `runtime/` stay pointers. The selection decides which `sources/` directories join the cone and which source files and GLBs are hydrated; build never has to widen the cone, and a matching cold pointer is reused without download. Shrinking only protects directories that contain tracked files and are leaving the cone; unrelated cache directories stay where they are.

`resolve` pins a commit, reads entities, models and build records, derives the effective spatial index and expands the selection into directories and exact dependency paths. Git sparse-checkout manages the working paths; LFS fetch/checkout prepares payloads, and bytes are verified after materialisation because a successful checkout does not prove a missing payload was supplied.

`--all` means the current world, not all historical LFS objects. Shrinking never garbage-collects. Switching and shrinking preflight local changes and ignored author files; failures keep the current state, and destructive reset is never used as hidden rollback.

Bulk object reads use a persistent `git cat-file --batch` rather than one subprocess per file. Materialised payload hashes are cached under `.rou` keyed by path, size, mtime, ctime and inode and recomputed only when the stat changes; the cache holds at most 100,000 entries with least-recently-used eviction and an independent 128 MiB limit, and a corrupt or unwritable cache never blocks an operation. Frozen files and LFS objects are hashed only within one snapshot. Lineage results are cached per immutable commit OID. The LFS storage directory is read once per snapshot.

No transaction spans Git, the network and working-tree updates. The rou write lock records its PID, is removed on Ctrl+C/SIGTERM, and stale locks are detected by checking process liveness; it cannot lock out external Git or IDEs, so actual state is reread every time and editor baselines are checked separately. `validate`, `view` and read-only LFS queries do not take the author write lock. A real deletion or rewrite of an unselected `runtime/` file requires selecting the entity before repair. Materialisation resets the old pointer stat, verifies the payload and refreshes its stat without staging content; temporary indexes are cleaned up on success, failure and interruption.

## 4. Build and integrity

Snapshots read a fixed commit, the index or the working tree. The working tree prefers real files; only explicitly skip-worktree paths fall back to the index; a merge reads the resolved index and never HEAD. New author files may build but must be added before committing.

`build` freezes sources and dependencies under `.rou/build`, opens each source `.blend` once in headless Blender for every model it hosts, exports the full GLB and a lod1 with simplified geometry and textures up to 1024 pixels, computes full-model local bounds and writes the records. One build loads the world snapshot once. On success only GLBs and records are written back to `runtime/`, never staged; frozen run directories are pruned according to a command-line option. The input digest binds profile, model definition, entry and dependency bytes and exporter settings; a changed shared dependency invalidates dependent outputs, while entity placement only affects the spatial projection and never invalidates geometry.

`validate` checks document shapes, immutable lineage, unique IDs, references, source/output digests, LFS pointers, GLB bytes and the spatial closure, deriving the complete spatial result from current inputs. Staged v1 is not disturbed by unstaged v2; commit validates the index first and the actual immutable commit afterwards, and a hook that changes the result is reported as an existing commit needing review, never silently rolled back.

README-only commits reuse unchanged build records. `validate`, `view` and `resolve` share one projection function. Digests prove version consistency, not cryptographic proof that an arbitrary exporter ran honestly.

## 5. Merge and push

Fast-forward validates the pinned target; a divergent `merge --no-commit --no-ff` keeps the state for inspection and build, and `--no-commit` alone never prevents a fast-forward.

Independent entity and model contributions merge without rebuilding, including different new entities in one chunk, because the complete spatial result is derived at validation and view time. Author conflicts are resolved first; a conflicting record or GLB of one model is rebuilt from resolved inputs. There is no `write-tree` on a fully unmerged index and no bulk "ours" instead of a rebuild; binary conflicts keep base/ours/theirs for manual resolution.

`push` pins the validated OID in its refspec so validating A can never publish B. LFS pre-push covers newly reachable history rather than the current selection. Git/LFS upload is not an atomic release; long-term availability belongs to S2.

## 6. Blender and the viewer

Blender saves the complete `.blend`; readable paths, shared libraries and textures are restored with each revision. Object and Collection entries define the export scope, but selecting one entry still needs the whole source file. Placement JSON is the exchange authority; the editing session pins revision, source, model and placement digests, and explicit placement writes check the baseline first.

The add-on keeps only bpy logic: dependency closure, packing, export and the session UI. Coordinate conversion, bounds rounding, canonical JSON and digests, model/entity record validation and safe-path rules all go through `rou_call`; a Blender `//` prefix is treated as a relative path in the core. The session baseline covers only the models, entities and dependencies of the current source, with two recovery paths when stale ("save source only" and "re-apply the current scene placement to the latest JSON"); a failed session start records its reason and shows it in the panel. Author roots are located by the object's persistent `rou_entity_id`; the Scene property `rou_model_owners_json` records which entity owns each model definition, so reopening, re-applying placement and undo rebind by ID rather than by name. Registration synchronises the author root's current name into the model entry; shared instances never take over an entry. Complete save and source-only save check that entry names match the author roots before writing, and the build exporter checks the persisted ownership against the frozen snapshot, so a native Blender save with a renamed root is rejected instead of exporting an impostor object. Build and preview share the hidden-mesh inclusion and visibility restore logic; the helper reports `bounds_blender_m` in Z-up metres and Rust `blender::bounds_native` converts it.

`rou view` validates a fixed commit and exports the complete catalog with locally available GLBs from the saved selection, without downloads. Explicit `--chunk`/`--entity` selectors may fetch that subset; `--all` fetches every referenced model. The same rules apply to built worktree drafts. Materialized runtime GLBs are hard-linked when possible, falling back to a copy; other verified LFS objects are copied. Bundle assets are read-only. Godot never calls a shell or Git, never reads `.git` or LFS internals, and the bundle contains no `.blend`.

The viewer app is a separate platform release that opens any browse bundle; it embeds no world. Users can download the precompiled app, or developers can export it with `make viewer` into `dist/viewer/`. Without `--realm`, it opens the world chooser.

The `rou-godot` extension supplies catalog, version-sidecar, GLB byte and budget checks and millimetre/chunk conversion; GDScript uses them through `Rou.invoke` and handles only camera, scheduling and UI. Start locations do not come from realm data: the default is above the centre of the union of entity bounds at a distance equal to its diagonal, looking at the centre, or 2 m above the origin for an empty world; the player picks a place from the chunk map, the entity list or typed coordinates; recent positions are stored per realm ID in the local user directory; `--at=x,y,z` (metres) sets a start directly. Opening a bundle first checks the three-entry layout, object names and symbolic links at the bundle root or inside it; ancestors outside the bundle may be links, and only an ordinary `.DS_Store` at the root or in `objects/` is ignored; failures keep the current world. A bundle without a version sidecar is shown as unbound, never silently treated as fixed. Sample tours live under `client/app/samples/` and are loaded on demand.

`rou_core::viewer::Catalog` owns distance tiers, nearest-first ordering, the default 1.15 middle-radius hysteresis and far-box visibility. Its initialization layout includes all world-bound box transforms. `main.gd` schedules requests and deferred attachment, `loader.gd` runs file reads and Rust checks through WorkerThreadPool, and `impostors.gd` renders the complete catalog in one MultiMesh. Display backends also parse glTF and generate mesh LODs on workers, transferring document/state ownership through a mutex-protected completion queue. Scene generation and at most one instance mount per frame run on the main thread; the first mount generates the prototype. Godot 4.7's headless dummy backend additionally parses glTF and generates LODs on the main thread to avoid concurrent RID creation. Two unique hashes may prepare concurrently; shared hashes have one outstanding task. A box is hidden only after attachment.

World switches retire old loaders without waiting; exit joins every task. Finished workers without a published result are reaped as failures. A 30-second timeout quarantines the task, rejects pending work and stops further submissions to that loader; late results are discarded without blocking the main thread. Native calls cannot be forcibly cancelled. Unrequested successful preparations share the prototype cache's five-second expiry and four-idle-entry limit. Failures retain a structured code and reason until reopen, even after camera cancellation. Missing file discovery also runs in the background, including far entities. Godot Rust bindings enable `experimental-threads`; the core itself remains stateless and reentrant.

Near and middle tiers select full and lod1 GLBs respectively; lod1 combines reduced geometry with textures up to a 1024-pixel edge. They use mesh LOD biases of 1 and 0.25. Import, caching and failures are keyed independently by SHA; identical hashes still deduplicate. Missing lod1 displays a box without importing a full asset. Tier swaps retain the previous scene only until successful replacement or a target-tier failure. Middle-tier geometry omits shadow casting and GI participation; both return in the near tier. Radii default to 48 / 200 / 4000 m in core configuration, adjustable in the viewer. The mandatory lod1 descriptor upgrades build and browse formats to 0.2; the Realm and spatial profile remain unchanged. Benchmark reports separate scene-generation/mount/unload frames from other frames, starting at the second process tick without warm-up exclusion. They track worker preparation, main-thread scene generation and total mount durations. M2's reference performance preset is balanced, with SDFGI disabled.

The version descriptor is explicitly local-trusted; untrusted S2 sources need signatures, authorisation or complete verification material, and a `verified` flag in JSON is not public trust. A Git tree path proves membership, not build freshness.

## 7. Operation table and FFI contract

`rou_core::ops` implements `dispatch(op: &str, input: &[u8]) -> Result<Vec<u8>, Error>` once; input is JSON or raw bytes (GLB), output is a JSON value. It is the only normative surface for hosts and is tested with one set of JSON fixtures.

`librou` exposes three C functions, all forwarding to the table:

```c
uint32_t rou_abi_version(void);
int32_t  rou_call(const char *op, const uint8_t *input, size_t input_len, RouBuf *out);
void     rou_free(RouBuf buf);
typedef struct { uint8_t *ptr; size_t len; } RouBuf;
```

- `op` is a NUL-terminated operation name; `input` is JSON or raw bytes; `out` is allocated by the library and must be released with `rou_free`. Return 0 is a success envelope, 1 an error envelope; 2 means invalid arguments and 3 a caught panic, and in those two cases `out` is empty.
- The envelope is `{"ok":true,"value":...}` or `{"ok":false,"error":{"code":"...","message":"..."}}`; error codes are stable strings that hosts map to messages.
- Panics are caught inside the library and never cross the boundary. There is no global mutable state, so calls may run concurrently. The library performs no file or network IO; hosts supply bytes.
- New operations add an op name without changing the signature; incompatible changes bump `rou_abi_version`, which hosts check at start-up.

`rou-godot` does not go through the C ABI: it links `rou_core` and exposes the GDExtension class `Rou` with `invoke(op: String, input: PackedByteArray) -> Dictionary` and `abi_version() -> int`. Rust converts the JSON value into a Godot Dictionary so GDScript never parses JSON, and byte arrays are passed as zero-copy slices. When a bundle is opened, `load_catalog(bytes)` builds a read-only Rust `viewer::Catalog`, and the periodic `catalog_query(request_bytes)` reuses it instead of re-serialising and re-validating the whole catalog; the request is `{position, radii, loaded, hysteresis?}` and the rules are shared with the `viewer.query` operation. `core.gd` caches the adapter and the ABI check.

The operation table is documented in [`rou/crates/rou-core/src/ops.rs`](../rou/crates/rou-core/src/ops.rs), including input shapes, results and executable examples. Python binds with `ctypes.CDLL`, passing `bytes` by pointer; a small wrapper turns error envelopes into Python exceptions. A PyO3 `abi3` module can be added later without touching `rou_core`.

## 8. S2 interfaces

| Component | Boundary |
| --- | --- |
| rou-node | Object serving, regional pins, quotas, replication, recovery, one local Web administration page |
| LFS transfer adapter | Takes OID and size, obtains bytes through peers, verifies whole-file SHA-256 |
| Metadata provider | Git or a bridge; a regional asset node need not promise all history |
| Release record | Created after the commit; binds realm/fork identity, Git algorithm/OID and the SHA-256 view root |
| Signaling/TURN | Replaceable connectivity that grants no maintenance authority |
| RealmHub | Optional business and catalog service, never the version authority |

Assets are not encrypted. Contributor, maintainer, peer and administration credentials are separate; public-key propagation is not initial trust. Signature vectors, transport session binding, replay rejection and cross-host vectors are S2 work. A hash does not identify an author and peer identity grants no maintenance authority.

Pins fix a commit plus a `runtime/` and `sources/` scope; preparing/complete/failed status is independent of the sharing switch. Drafts and caches are never auto-public; all-sources-offline is reported as unavailable. Local Web administration still needs authentication and Origin/CSRF checks; initial remote access is an SSH tunnel; Godot only connects and opens pages.

Chunking, backpressure, resume, forced relay and multi-peer coverage require real tests; Git/LFS cache retention is not long-term pinning. S2 node and transport code is also Rust.

## 9. Optional services

The local core has no server dependency. A separate signaling service and coturn can be deployed later. RealmHub prefers independent API and worker services reusing Supabase Auth/PostgreSQL/Storage and on-demand notifications; Edge Functions fit lightweight endpoints.

Platform authentication only handles platform sessions; public-key control is verified separately. Private keys and service-role secrets never reach clients. Storage defaults to avatars, screenshots and attachments; heavy assets have their own providers, retention and cost. A platform outage does not affect known reachable core paths.
