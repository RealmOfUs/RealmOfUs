# Realm repository contract

Version 2.1. Git is the only history and staging authority; there is no converter, compatibility reader or second backend. Public signatures, P2P and node authorisation are defined by S2.

## 1. Files and formats

| Path | Format / purpose |
| --- | --- |
| `realm.json` | `rou-realm/0.1`; realm ID and spatial profile, immutable after init |
| `realm/models/<id>.json` | `rou-model/0.1`; model definition: source `.blend`, entry object or collection, dependencies |
| `realm/entities/<id>.json` | `rou-entity/0.1`; placement: stable ID, model, integer millimetre position |
| `runtime/<model>.glb` | Exported model, tracked by standard LFS |
| `runtime/<model>.json` | `rou-build/0.1`; build record of that GLB: input hashes, input digest, GLB hash and length, local bounds, tool versions |
| `sources/` or any safe author directory | Complete original projects, shared libraries, textures; readable real paths |
| `.git/` | Native tree/index/commit/refs/reflog and the LFS cache |
| `.rou/` | Ignored local state: selection, session, build scratch, browse bundles, backups, caches; no authoritative history |

`sources/` are the authors' complete projects; `realm/models/` says what to export from which file; `realm/entities/` says where it goes; `runtime/` is the complete build output, exactly one GLB plus one record of the same name per model. Authors write only `realm/` and `sources/`; rou writes only `runtime/`. One file per ID is what lets parallel contributions merge with plain Git; `realm.json` never changes after init, so no single file is edited by many contributors.

Versioned JSON is written with one fixed on-disk layout: sorted keys, two-space indent, one array element per line, UTF-8 without BOM, one trailing newline. rou and the add-on both use it so that changes to different fields of one file merge line by line and diffs stay readable. Hashes and digests are computed over the RFC 8785 encoding of the parsed value and are independent of layout.

The data directory is `realm/`; there is no root `work/`, `staging/` or `profile.json`. Ordinary text, READMEs and scripts are managed by the normal Git file tree. Original projects and exported models use standard LFS pointers, identifying payloads by SHA-256 and actual length; Git OIDs record their own algorithm and are not limited to 64 characters.

The complete view root and chunk index are derived deterministically from the versioned records above and are never written or committed as shared index files. Browse bundles that need to exist on disk live under `.rou/view/` or an explicit output directory.

### Format identifiers

The `format` field of every JSON document is its format identifier, `name/major.minor`. A reader accepts only identifiers it knows and rejects anything else without guessing; during pre-research any incompatible change bumps the version and regenerates data. All identifiers are defined once in the core's `formats` module.

| Identifier | Document | Writer | Reader |
| --- | --- | --- | --- |
| `rou-realm/0.1` | `realm.json` | `rou init`, author | rou, add-on |
| `rou-model/0.1` | `realm/models/<id>.json` | author, add-on | rou |
| `rou-entity/0.1` | `realm/entities/<id>.json` | author, add-on | rou |
| `rou-build/0.1` | `runtime/<model>.json` | `rou build` | rou |
| `rou-blender-export/0.1` | `response.json` of the Blender export helper under `.rou/build/`; also the value of `tool_versions.exporter` in build records and part of the input digest | export helper | `rou build` |
| `rou-view/0.1` | The complete in-memory view produced by `projections`, the object the view root hash is computed over; never written to disk | rou | rou |
| `rou-preview/0.1` | Browse bundle `realm.json` | `rou view`, add-on local export | Godot extension |
| `rou-git-view/0.1` | Browse bundle `version.json` | `rou view` | Godot extension |

Local files under `.rou/` (selection, session, caches) are not versioned documents and carry no format identifier.

## 2. Author inputs

```json
{
  "format": "rou-model/0.1",
  "id": "house",
  "source": "sources/houses/courtyard.blend",
  "entry": {"kind": "object", "name": "house"},
  "dependencies": ["sources/shared/ornament.blend", "sources/shared/checker.png"],
  "settings": {}
}
```

```json
{
  "format": "rou-entity/0.1",
  "id": "house",
  "model": "house",
  "position_mm": [8000, 0, 8000],
  "label": "Courtyard house"
}
```

Model definition fields; no other fields are allowed:

| Field | Constraint |
| --- | --- |
| `format` | Constant `rou-model/0.1` |
| `id` | Stable ID equal to the file name, see below |
| `source` | Safe repository-relative path ending in `.blend`, not under `runtime/` |
| `entry.kind` | `object` or `collection` |
| `entry.name` | 1–256 characters |
| `dependencies` | Array of strings, at most 4096, no duplicates, each a safe relative path, not the `source` itself, not under `runtime/` |
| `settings` | Must be an empty object |

Entity fields; no other fields are allowed:

| Field | Constraint |
| --- | --- |
| `format` | Constant `rou-entity/0.1` |
| `id` | Stable ID equal to the file name |
| `model` | Model ID; the model must exist |
| `position_mm` | Three integers in millimetres, inside the world bounds |
| `label` | Optional, at most 256 characters |

### Identifiers and file names

A stable ID matches `[a-z0-9_-]{1,80}`: lower-case ASCII letters, digits, underscore and hyphen only, avoiding conflicts on case-insensitive file systems. Three independent namespaces exist:

| Identifier | Where it appears | Rule |
| --- | --- | --- |
| Realm ID | `id` in `realm.json` | Generated by `rou init` as `realm_` plus 32 hex digits; unchanged by clone and fork; part of the rule lineage |
| Model ID | File name and `id` of `realm/models/<id>.json`; the `model` field of entities; file names of `runtime/<id>.glb` and `runtime/<id>.json` and the `model` field of the record | Unique within models; exactly one GLB and one record per model, named after the model ID |
| Entity ID | File name and `id` of `realm/entities/<id>.json`; `id` of entities in the view and the bundle | Unique within entities; may coincide with a model ID, for example model `house` placed by entity `house` |

The file name must equal the `id` inside; a mismatch is rejected. Two files with the same ID in one namespace are rejected even when Git merges them without conflict. IDs are immutable: changing an ID means deleting the old record and creating a new one, and moving an entity changes only `position_mm`. An entity's `model` must exist; delete or repoint referencing entities before deleting a model, since validation rejects dangling references. `label` is an optional display name of at most 256 characters and takes part in no reference.

### Entries and dependencies

The entry kind is `object` or `collection`. An object entry is a root with no parent and identity rotation and scale; its translation is zeroed on export. A collection has no transform of its own; its internal coordinates define the model frame, and no pivot is invented.

The model definition is the sole authority for source path, entry and dependencies. Within one source, `(entry.kind, entry.name)` must be unique; instancing is done by several entities referencing one model, never by duplicate definitions of the same entry. The add-on locates the author root by the persistent author entity ID and synchronises its current name into the entry; an instance object's own name never changes a shared entry; the binding is stored in the source `.blend`, so session rebuilds and undo do not rebind by name. `dependencies` list the complete static export closure (including indirect libraries and images) as repository-relative paths; missing, escaping, symbolic-link and undeclared external dependencies are rejected. A Blender path starting with `//` is relative to its `.blend`, not absolute. `settings` accepts only an empty object and never silently ignores unknown settings. Unsupported animation, caches or dynamic dependencies cannot count as valid static assets; the check covers every object under the entry, not just the root.

`realm.json` has only `format`, `id` and `profile` and never changes after init; profile rules are in [viewer-bundle.md](viewer-bundle.md). A realm's `format`, `id` and `profile` must match every reachable realm ancestor; committing an invalid parent and an unchanged child does not bypass the rule. Complete Git metadata history is required: shallow history is rejected and local replace objects are ignored. Entities support translation only. At most 4096 entities and 4096 models; one model may be referenced by several entities, and the add-on locates placement targets by entity ID, not object name.

A registered author root must keep the name recorded in the model entry; after a rename, register first, then save. Complete save and source-only save in the add-on check this before writing; the build exporter also checks the persisted author identity in the source against the entity associations of the frozen snapshot, and rejects name drift or a missing author root left behind by a native save instead of exporting another object under the old name.

## 3. Build records and spatial results

Build defaults to the saved working tree; `--staged` takes the index as input. The logical tree reads the index only for paths that are explicitly skip-worktree; a real deletion is reported or treated as a deletion. A merge uses the resolved index and never restores old entities from HEAD.

A build freezes the relevant bytes in a fresh directory under `.rou/build` and runs headless Blender on that copy; models sharing one `.blend` are exported in one pass. Each model gets `runtime/<model>.glb` and `runtime/<model>.json`:

```json
{
  "format": "rou-build/0.1",
  "model": "house",
  "inputs": {
    "sources/houses/courtyard.blend": {"sha256": "fa71…4a88", "bytes": 91180},
    "sources/shared/checker.png": {"sha256": "ce30…401d", "bytes": 300},
    "sources/shared/ornament.blend": {"sha256": "e5e8…591d", "bytes": 88969}
  },
  "input_digest": "31e6…ba56",
  "asset": {"path": "runtime/house.glb", "sha256": "ce42…2ec7", "bytes": 5620},
  "local_bounds_mm": {"min": [-4500, 0, -3500], "max": [4500, 4400, 3500]},
  "tool_versions": {"blender": "5.2.1 LTS", "exporter": "rou-blender-export/0.1"}
}
```

Build record fields; no other fields are allowed:

| Field | Constraint |
| --- | --- |
| `format` | Constant `rou-build/0.1` |
| `model` | Model ID equal to the file name |
| `inputs` | Object keyed by the relative paths of `source` and every dependency; the key set must equal exactly `source` plus `dependencies` of the model; each value is `{sha256, bytes}` with a 64-digit lower-case hex hash and a non-negative byte count |
| `input_digest` | 64 hex digits: SHA-256 of the RFC 8785 encoding of `{exporter, profile, model, inputs}`, where `exporter` is `rou-blender-export/0.1` and `model` is the complete model definition |
| `asset.path` | Fixed `runtime/<model>.glb` |
| `asset.sha256`, `asset.bytes` | SHA-256 and length of the actual GLB bytes; at least 20 bytes and at most the 8 MiB single-GLB budget of the bundle specification |
| `local_bounds_mm` | `min` and `max` of three integers each, `min < max` on every axis |
| `tool_versions` | Optional map of strings to strings, including `exporter` and the Blender version; not part of `input_digest` |

The digest covers the profile, the complete model definition and the input byte descriptions; world position enters only the derived entities and the complete index and never feeds back into the geometry build.

A GLB is local geometry. A derived entity adds its `position_mm` to the local AABB to obtain the world AABB, then computes owner chunk, offset and every intersecting chunk. `projections` produces the complete in-memory view (`rou-view/0.1`): world position, bounds, owner and offset and asset description of every entity, plus the complete map from chunk key to entity IDs; there is no separate chunk page object. An empty chunk is one that is absent from the complete index; a download failure never masquerades as an empty chunk. A local AABB may be degenerate on an axis; export expands such an axis by 1 mm so every axis has positive thickness.

The view does not contain the current Git commit. `view_root_sha256 = SHA-256(JCS(view))` with JCS being RFC 8785. The `catalog_sha256` of an exported bundle and every GLB SHA-256 are computed over actual file bytes. The internal `input_digest` is likewise computed over the JCS encoding, and build inputs can never include their own output digest.

Build writes only `runtime/` and never stages. New outputs are untracked until added; changed tracked outputs show as modified. When a model definition is deleted, build deletes the obsolete GLB and record from `runtime/` and reports them, tracked or not. A conflicting record or GLB of one model is rebuilt from resolved inputs without a `write-tree` of the unmerged index; source conflicts must be resolved first. A failed build never advances HEAD.

In a sparse working tree, build reports `stale_runtime` when an unselected GLB has been altered or a tracked record has been deleted; select the entity, then repair. A matching cold GLB pointer is reused without download. Missing records and symbolic links under `runtime/` are checked before any payload fetch or Blender call; repairing a missing record by chunk needs fresh bounds in the snapshot, otherwise the entity must be selected explicitly. The index baseline is taken from the same data the build snapshot parsed; an index change during capture or publication is reported as `snapshot_changed`.

## 4. Validate and commit

`validate --staged` checks the index; `validate <commit>` checks a fixed native commit. Both check rules, identities and references, dependency pointers, per-model build freshness, GLB hash/length/precheck budgets, and derive the complete spatial result from the current inputs. Missing, stale or extra records and GLBs under `runtime/` are rejected. Errors are never filled in silently from old outputs.

Strict JSON rejects duplicate keys, non-finite or overflowing numbers, non-UTF-8 encodings, and documents beyond the depth and size budgets; the budgets, including GLB top-level arrays and images, are defined once in the core and shared by every host.

New or changed GLB bytes must be obtained and verified. Unchanged GLBs of a parent revision may remain undownloaded; the result then reports `missing_assets` and `byte_validation_complete: false` and never conflates metadata validity with fully checked assets. Fixed-revision viewing still verifies the GLB bytes actually supplied.

Commit pins the index tree and HEAD, validates, calls Git commit, then reads and validates the actual commit. If a hook or external write changed the result, the created commit ID and the validation error are reported; the commit is neither denied nor rolled back automatically. A valid staged v1 is not disturbed by unstaged v2, and README-only commits need no Blender. Native Git commands may create unvalidated drafts; the existence of a commit does not mean it is valid for publication.

## 5. Selection and transfer

`.rou/selection.json` is `{all: boolean, chunks: [string], entities: [string]}` with `[]` rather than `null`. `set` replaces, `add` extends, `list` shows; `sparse-checkout` is an alias of `sparse`. `--all` excludes explicit selectors, and `set` without any selector is an error.

A chunk selector is `x,y,z` or two opposite corners `x,y,z..x,y,z`. Each axis takes its own min and max, both ends inclusive, negative and reversed coordinates allowed, overlaps deduplicated; at most 32768 chunks per command, all inside the realm, never clipped. Range parsing has one implementation and the CLI passes selector strings through unchanged. Selecting a chunk includes every intersecting entity; spanning entities and shared paths are deduplicated. Directories go to the Git cone; LFS downloads follow the actual dependencies; a pointer in the cone does not mean its payload is present.

The Git cone always contains `realm/`, `runtime/` and `docs/`; GLBs under `runtime/` remain pointers until selected. The selection decides which `sources/` directories join the cone and which source files and GLB payloads are materialised. `clone` is no-checkout, then local skip-smudge, then the lightweight cone, then checkout. `fetch` only updates Git objects and remote-tracking refs. LFS fetch only caches; checkout materialises from the cache; materialised bytes are verified by hash, and verified payloads are cached by path, size, modification time and inode. Selecting the whole world is not downloading all history.

Switching revisions follows native Git's protection of local changes and preflights author files and ignored backups in a shrinking selection: ignored backups that the cone would delete are moved under `.rou/backups/` and reported, author changes are refused. Multi-step operations are not one transaction; failures show the real state and never hide a reset or deletion. External Git can change state at any time; the `.rou` lock does not replace Git's locks.

## 6. Merge and push

`switch -c` and `checkout -b` create branches; `checkout <branch>` stays attached, `checkout <commit>` is detached. Ordinary text is left to Git; `rou mv` and `rou rm` keep native rename and delete semantics. Default `diff` is Git's file diff; `diff --realm [--staged] [A [B]]` adds an entity, placement and model summary that needs no payload download. Parallel edits across entities are validated after the merge; duplicate IDs, inconsistent entity records, delete/modify and binary conflicts are handled explicitly.

Independent entity and model contributions, including different new entities in one chunk, merge with native Git and need no post-merge build of a shared index; hosted platforms can merge natively without a local merge driver. A successful Git merge is still validated as a domain result.

A divergent `rou merge` uses `--no-commit --no-ff` and keeps the state; fast-forwards are validated and applied separately. A conflicting record or GLB of one model is rebuilt from resolved inputs, never taken as "ours". `merge --abort` delegates to Git and does not promise to restore extra untracked files.

`push` pins the validated OID and refspec. Standard LFS push covers newly reachable history regardless of the current selection; missing historical payloads are recovered from an explicit source when needed. A Git ref update plus LFS upload is not an atomic distributed release; formal authority, signatures and pins belong to S2.

## 7. Browse bundle export

`rou view [REVISION]` defaults to HEAD; the revision is resolved to a full commit OID; `--worktree` explicitly exports a built draft. The output contains `realm.json` (`rou-preview/0.1`), `version.json` (`rou-git-view/0.1`) and `objects/<sha256>.glb`. Without explicit selectors, only locally available GLBs from `.rou/selection.json` are included, and no network download is attempted. An empty selection produces a catalog-only bundle; unavailable selected bytes at an older revision remain missing. `--chunk` and `--entity` use the sparse selection parser and may fetch that explicit subset; `--all` fetches every referenced model and conflicts with those selectors. These rules also apply to `--worktree`. Every bundle keeps the complete metadata; models whose payloads are absent are shown as missing, never as empty. The bundle contains no viewing entry points; the viewer derives or lets the player choose the start.

Verified materialized `runtime/<model>.glb` files are hard-linked into local bundles; cross-filesystem or unsupported links fall back to copies. Other verified LFS payloads are copied. Bundle GLBs must be treated as read-only: an in-place write to a hard-linked file changes both paths; builds replace runtime files atomically. Symbolic links remain forbidden.

`version.json` contains `realm_id`, `git_object_format`, `commit_oid`, `view_root_sha256`, `catalog_sha256` and `validation: local-trusted`; a draft has an empty commit. Godot verifies through the Rust extension that the catalog bytes match the sidecar and that every GLB matches its hash and budget, without reading `.git` or running Git, LFS or Blender. A bundle without `version.json` is shown as unbound, never silently treated as a fixed revision. A local JSON is not a public identity signature; verification of untrusted providers is defined by S2.

The viewer is read-only; Godot Editor placement is separate work.

## 8. Implementation and acceptance

The implementation structure, operation table and FFI contract are in [architecture.md](../architecture.md). Validation has two layers: the shape layer is implemented by one core type per document with the field tables above as its human-readable definition (no JSON Schema); the semantic layer (cross-file references, ID uniqueness and file-name agreement, lineage, hashes, AABBs, input/output relations) is implemented in core code and defined by this text. Both must pass. Reviewed spatial and catalog cases live in `rou/crates/rou-core/tests/fixtures/spatial.json`. S2 public network formats are not frozen.
