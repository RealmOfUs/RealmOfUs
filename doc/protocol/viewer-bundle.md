# Spatial profile and browse bundle

Version 2.1. This document defines the **spatial profile** shared by every realm and the **browse bundle** produced by `rou view` or the Blender add-on's local export and consumed by the Godot viewer (`rou-preview/0.1` and `rou-git-view/0.1`). The spatial rules have been frozen since 2026-09-09; the identifier `bounded-cartesian-v1-experiment` and its integer parameters stay as they are, the `experiment` suffix being historical naming only. Repository layout, models, entities, build records and Git semantics are in [repository.md](repository.md). This document defines no S2 signatures, public keys, network authentication or P2P; assets are not encrypted; a local SHA-256 check is not author authentication.

## 1. Spatial conventions

The default profile `bounded-cartesian-v1-experiment` is a bounded 256 × 128 × 256 m cuboid centred on the origin, right-handed, Y-up, displayed in metres, placed in integer millimetres, with 32 m chunks. Parameters cannot change within one realm; different parameters yield a different `profile_hash` and are not an ordinary update of the same world. What is frozen is the defaults and the allowed ranges, not one size for every realm:

| Parameter | Default / allowed range |
| --- | --- |
| `id`, `units`, `up`, `handedness` | Fixed to `bounded-cartesian-v1-experiment`, `meter`, `Y`, `right` |
| `placement_unit_mm` | Fixed to `1`; constrains placement only, model vertices need not snap to millimetres |
| `origin_mm` | Default `[0,0,0]`; three integers, each within `[-1000000000, 1000000000]` mm |
| `chunk_size_mm` | Default `[32000,32000,32000]`; each axis an integer `1..64000` mm, fixed within a realm; chunks are axis-aligned and may be cuboids |
| `min_chunk` / `max_chunk` | Default `[-4,-2,-4]` / `[4,2,4]`; each component within `[-1000000000, 1000000000]`, `min < max` per axis |
| Total chunks | Product of `(max_chunk - min_chunk)` over the three axes at most `32768`, empty chunks included |
| World extent | `origin_mm + bound_chunk * chunk_size_mm` at both ends of every axis within `[-1000000000, 1000000000]` mm |

The shape layer checks fields, types, constants and single-field ranges through the core's serde types; the semantic layer must additionally check `min < max`, the total chunk count and the computed world extent. Parsing into the type alone does not make a profile valid. JSON integers are judged by value: `1`, `1.0` and `1e0` are the same integer; booleans, numeric strings, non-integers and non-finite values are not integer parameters; canonical encoding writes decimal integers and normalises `-0.0` to `0`.

Per axis, `chunk = floor((position_mm - origin_mm) / chunk_size_mm)` and `offset = position_mm - origin_mm - chunk * chunk_size_mm`. Chunks and the world are `[min, max)`: X = -1 mm belongs to chunk -1 with offset 31999 mm, X = 32000 mm belongs to chunk 1, and the world's maximum boundary is outside the world. The maximum of an AABB is exclusive, so covered chunks are computed with `max_mm - 1`. Bounds are integer millimetres; export expands a degenerate axis by 1 mm so every axis has positive thickness.

An entity is a scene object with a stable ID and one placement that references the exported GLB of a model. Its owner chunk comes from the placement origin; its covered chunks come from the world AABB, a conservative box query rather than a per-triangle test. A spanning AABB records the same ID in every intersecting chunk and the viewer deduplicates by ID. Moving keeps the ID and recomputes position, owner, offset, AABB and index. Chunking never clips models and is not a P2P byte block. Placing one model several times uses distinct entity IDs referencing the same GLB hash; one entity record cannot express several positions. Entity roots translate only; rotation and scale live in child objects.

The first profile does not support spherical, toroidal or wrapping worlds, unbounded extension, irregular or per-chunk grids, runtime parameter changes or coverage beyond budget. An allowed numeric range is not a promise that rendering quality has been validated at extreme coordinates. Changing a frozen constraint updates the specification, the test cases and the core together, never one side.

## 2. Complete index and empty chunks

`index_complete` must be the boolean `true`. After the complete index is validated, any chunk inside the world with no index entry is a known empty chunk; a whole world may have `entities: []` and `chunks: {}`. Covered chunks of existing entities must all be recorded, and the index must agree exactly with the entity AABBs; deleting an index entry cannot turn a populated chunk into an empty one. `index_complete: false`, a missing field or a non-boolean is rejected, so a missing index is never displayed as known-empty.

A bundle always carries the complete metadata; `rou view` includes the saved selection by default without downloads; explicit `--chunk`/`--entity` or `--all` may download the requested GLBs, and models not included are shown as missing, never as empty.

`profile_hash` is the SHA-256 of the RFC 8785 encoding of the profile and binds the parameters in the catalog; changed parameters with a stale hash are rejected. It is not world identity; lineage is enforced by the ancestor check in [repository.md § 2](repository.md#2-author-inputs).

## 3. Browse bundle

A bundle contains exactly the three entries below. An ordinary `.DS_Store` file at the root or inside `objects/` is ignored; it is not bundle content and takes part in no check or digest. Any other file or directory, and any symbolic link at the bundle root or inside it, is rejected; ancestors outside the bundle may be symbolic links, such as `/tmp` and `/var` on macOS.

| Path | Format | Contents |
| --- | --- | --- |
| `realm.json` | `rou-preview/0.1` | Runtime catalog: profile and hash, world position, bounds and asset description of every entity, complete chunk index |
| `version.json` | `rou-git-view/0.1` | Fixed-revision sidecar: realm ID, Git object algorithm and commit, view root hash, catalog byte hash, validation level |
| `objects/<sha256>.glb` | glTF 2.0 binary | Immutable viewing assets named after the SHA-256 of their actual bytes |

`realm.json` is produced by the core's `projections`, sorted by entity ID and chunk key, and deterministic:

```json
{
  "format": "rou-preview/0.1",
  "profile": {"id": "bounded-cartesian-v1-experiment", "units": "meter", "up": "Y", "handedness": "right",
              "placement_unit_mm": 1, "origin_mm": [0, 0, 0], "chunk_size_mm": [32000, 32000, 32000],
              "min_chunk": [-4, -2, -4], "max_chunk": [4, 2, 4]},
  "profile_hash": "c5ea37e2ce31d6894d505e45475b8791c530767b8bd7d81637bc16e5df4a1f97",
  "index_complete": true,
  "chunks": {"0,0,0": ["house", "plaque"], "1,0,0": ["bench"]},
  "entities": [
    {
      "id": "house",
      "label": "Courtyard house",
      "position_mm": [8000, 0, 8000],
      "owner_chunk": [0, 0, 0],
      "offset_mm": [8000, 0, 8000],
      "bounds_mm": {"min": [3500, 0, 4500], "max": [12500, 4400, 11500]},
      "asset": {"path": "objects/ce42…2ec7.glb", "sha256": "ce42…2ec7", "bytes": 5620}
    }
  ]
}
```

The bundle contains no viewing entry points. `asset.path` is fixed to `objects/<sha256>.glb`, `sha256` equals the file name and `bytes` is the actual length. A GLB stores model-local coordinates and `position_mm` supplies the world translation; Blender sources are Z-up and map to Godot as `(x, z, -y)`, converted once at export.

`version.json` is written by `rou view`:

```json
{
  "format": "rou-git-view/0.1",
  "realm_id": "realm_45cde0e62fc8431ca8d627d9dc73bce6",
  "git_object_format": "sha1",
  "commit_oid": "bd52c15f3ceb0ae73a5b9bb260b0b7d2c5759547",
  "view_root_sha256": "054f40ae945efde072983368431293b37eb6e3608692a63e6bc0cac5115fa061",
  "catalog_sha256": "…",
  "validation": "local-trusted"
}
```

`view_root_sha256` is the SHA-256 of the RFC 8785 encoding of the in-memory view (`rou-view/0.1`), the value future release records bind to; `catalog_sha256` is the SHA-256 of the actual bytes of `realm.json`, which the viewer uses to confirm the catalog was not rewritten; a draft exported with `--worktree` has an empty `commit_oid`; `validation` has the single value `local-trusted`, meaning a locally trusted bridge rather than a public signature. The add-on's local export writes only `realm.json` and `objects/` without `version.json`, and the viewer must show such a bundle as unbound rather than silently treating it as a fixed revision.

The viewer performs every check through the Rust extension: catalog shape and budgets, `profile_hash`, the binding between `version.json` and the catalog bytes, length and SHA-256 of every GLB, and the GLB precheck. It never reads `.git` or runs Git, LFS or Blender. Canonical JSON is RFC 8785 throughout, with the core as the only implementation.

## 4. Budgets

| Object | Limit |
| --- | --- |
| `realm.json` | ≤ 4 MiB, ≤ 4096 entities, ≤ 32768 chunks; one entity covers ≤ 2048 chunks |
| One GLB | ≤ 8 MiB, ≤ 100000 vertices, ≤ 300000 indices, ≤ 512 nodes, top-level arrays ≤ 8192 items; static triangle meshes, glTF 2.0; embedded PNG/JPEG ≤ 2048², decoded-pixel budget 64 MiB |
| Rejected in GLB | External or data URIs, extensions, animation, skinning, cameras, sparse accessors |
| Strict JSON | Duplicate keys, non-finite or overflowing numbers, non-UTF-8 encoding or BOM, nesting deeper than 40, size beyond budget |

All budgets are defined once in the core; `rou validate` and the viewer run the same implementation, so nothing can be committed yet fail to display. Loading checks the catalog and index first, then GLB length and SHA-256, JSON and buffer bounds, geometry and image budgets, and only then hands the file to Godot's native glTF importer. The precheck is neither a complete glTF conformance check nor a sandbox around the native parser; untrusted network assets in S2 need isolation and fuzzing first.

## 5. Viewer start and loading behaviour

The start location does not come from realm data. The default is above the centre of the union of all entity world bounds, at a distance equal to the union's diagonal, looking at the centre; an empty world starts 2 m above the origin looking at it. The player can pick a place on entry or at any time through the chunk map, the entity list or typed coordinates; recent positions are stored per realm ID in the local user directory, never in the bundle; `--at=x,y,z` (metres) sets the start directly.

The viewer has three distance tiers, measured by the Rust core against occupied chunk AABBs. A spanning entity uses its nearest covered chunk and appears once. Default radii are **48 / 200 / 4000 m**; `--radius=16,64,4000` or the Settings tab changes them in metres. Radii must be finite, positive and nondecreasing, with a maximum of 4,000,000 m supplied by core configuration. Equal adjacent radii are allowed and the nearer tier wins their shared boundary. Defaults are viewer settings, not profile fields. The near tier renders the complete GLB with generated mesh LODs selected by screen size. The middle tier retains that same GLB, with a lower mesh LOD bias so available simplified meshes are selected earlier, and disables shadow casting and GI participation for that tier. Near-tier shadows and GI participation are restored on approach. Low-complexity meshes may have no useful simplification. A separate middle-detail GLB is not part of this format.

On open, all catalog world bounds become translucent boxes in one MultiMesh, without reading GLBs. Rust supplies each box's metre center and size, the visible far set, and the boxes hidden by resident scenes. A box stays visible during loading and is hidden only after its scene mounts; detaching a scene restores its box in the same frame. Boxes outside the far radius are hidden. A background file-availability scan marks missing assets even in the far tier. Missing boxes are orange; assets rejected by validation are red, and the entity details retain the reason. Missing, invalid, known-empty and unknown-metadata states remain distinct. Reopening the bundle retries rejected assets.

Every 0.35 s `Rou.catalog_query` receives `{position, radii, loaded}` and an optional `hysteresis` (default `1.15`). It returns distance-ordered `load`, `keep`, `unload`, `impostor_hidden`, `impostor_visible`, near/mid/far ID lists, and current chunk information. Boundaries are inclusive; loaded entities stay resident through middle-radius times hysteresis, even beyond the far-box radius. Hysteresis is finite and at least one, and its product with the middle radius cannot exceed the independent 4,600,000 m retention budget. Duplicate loaded IDs are deduplicated. Ordering uses entity world-AABB center distance, with ID as the tie-breaker. Pending requests outside the new set are cancelled. Completed successes enter the bounded cache even if no longer requested; failures retain a structured code and reason for every instance of that SHA. Retired loaders from a previous world discard results and cannot attach scenes to the new world.

At most two unique GLBs import concurrently through WorkerThreadPool. File reads and Rust GLB/hash/budget checks always run in workers. With a display renderer, workers also parse glTF and generate mesh LODs, then transfer the document and state through a mutex-protected completion queue. The main thread generates a detached scene on the first requested mount, duplicates it and attaches at most one instance per frame through deferred processing. For Godot 4.7's headless dummy renderer, glTF parsing and LOD generation also run on the main thread because its rendering-resource owners are not safe for concurrent creation. There is no user-facing thread-disable switch; reads and validation remain asynchronous in both modes. Entities sharing a SHA wait on one task and share a prototype's meshes and textures. Prepared assets and scene prototypes share an idle-cache limit of four entries and expire after five seconds; active entries remain while referenced by loaded or requested entities.

Completion is checked against WorkerThreadPool task state, not only the publication queue: a finished task without a result produces a visible worker failure and releases its slot. A task exceeding 30 seconds is reported as timed out; the loader stops accepting new imports, rejects remaining requests and quarantines the running task until it can safely discard its late result. Native calls cannot be forcibly cancelled; world switching stays nonblocking, while application exit joins outstanding tasks. A permanently blocked native call therefore still requires process-level isolation or termination.

`F1` shows radii, tier populations, the pending queue and worker count; `F2` shows the chunk grid. Frame reports start at the second process tick after opening a world, with no warm-up frames removed. Frames containing main-thread scene generation, mounts or unloads are grouped as `mount_frames`; the remaining frames are `steady_frames`, including startup and the released overview. Both groups include count and P50/P95/P99. Worker preparation, main-thread scene generation and total mount durations are also reported separately. The 30-second flight begins its route at first readiness, after the harness has acquired focus; all recorded frames count even if focus is later lost. M2 performance acceptance uses the balanced preset (SDFGI disabled); the normal viewer default remains unchanged. Bundles are regenerable; copy the whole directory to keep a particular one.

## 6. Test cases and verification

`rou/crates/rou-core/tests/fixtures/spatial.json` holds reviewed spatial and catalog cases and is read-only; expected values are never regenerated by the implementation under test. In `profiles`, `shape_accepted` means the case parses into the profile type and `accepted` means both layers pass; `positions` cover owner and offset; `bounds` cover half-open AABBs; `catalogs` cover spanning entities, moves, shared assets, empty catalogs and rejection of unknown catalogs. Catalog cases check metadata only; placeholder asset descriptions are not real GLBs. The default profile and its `profile_hash` are unchanged under RFC 8785 because the profile contains only integers and ASCII strings.

```sh
make test-spatial
```

This builds the native extension, runs the Rust spatial tests and the headless Godot ABI checks, and writes `build/s1-2.1/spatial-checks/report.json`.
