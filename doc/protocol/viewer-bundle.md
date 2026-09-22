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

Chunks are queried by AABB within 48 m of the camera; entities are deduplicated per chunk and one GLB is imported per frame. Every 0.35 s the required set is recomputed, stale queue items not yet imported are cancelled, and entities that left the radius are released. A GLB with the same hash is read, verified and imported once. There is no LOD, disk-cache eviction, background decoding or fade-in yet; synchronous import can cause frame spikes. `F1` shows diagnostics and `F2` the ground chunk grid. Bundles are regenerable; copy the whole directory to keep a particular one.

## 6. Test cases and verification

`rou/crates/rou-core/tests/fixtures/spatial.json` holds reviewed spatial and catalog cases and is read-only; expected values are never regenerated by the implementation under test. In `profiles`, `shape_accepted` means the case parses into the profile type and `accepted` means both layers pass; `positions` cover owner and offset; `bounds` cover half-open AABBs; `catalogs` cover spanning entities, moves, shared assets, empty catalogs and rejection of unknown catalogs. Catalog cases check metadata only; placeholder asset descriptions are not real GLBs. The default profile and its `profile_hash` are unchanged under RFC 8785 because the profile contains only integers and ASCII strings.

```sh
make test-spatial
```

This builds the native extension, runs the Rust spatial tests and the headless Godot ABI checks, and writes `build/s1-2.1/spatial-checks/report.json`.
