# Realm of Us · Product and System Design

Baseline 2.1 · 2026-09-16. Baseline 2.0 chose native Git plus standard Git LFS and delivered a Go/Python reference implementation; 2.1 decides that all core tooling is implemented in Rust and wired into Blender and Godot, reorganizes the realm directory layout, and keeps no compatibility layer. See the [Chinese design](Realm_of_Us_Design.zh-CN.md) for detailed rationale, the [data contract](protocol/spec/s1-repository.zh-CN.md), the [architecture and migration](Realm_of_Us_Architecture.en.md), and the [reference-implementation acceptance](experiments/S1-Git-results.zh-CN.md).

## Purpose and delivery

A realm is a collaborative 3D authoring project with immutable spatial rules. Contributors clone its catalog, branch, select regions, model in Blender, commit and merge. An independent Godot application explores a fixed revision.

Native Git owns the complete authoring tree and sole history. Standard Git LFS stores source and runtime binaries. The daily interface is rou, which adds spatial selection, builds, domain validation and viewer handoff.

Core tooling is Rust with exactly one implementation of every rule: rou is a single binary; the same core library is exposed through a C ABI to the Blender add-on via ctypes and through GDExtension to the Godot viewer, and both hosts use the same operation table. Python remains only for bpy-dependent add-on logic; GDScript remains only for viewer UI and load scheduling.

| Stage | Scope |
| --- | --- |
| S1 (2.0 reference implementation, G1–G4) | Spatial rules, full original projects, Git/LFS, branches and standard remotes, spatial source hydration, builds, validation, merge, independent viewing, current Mac acceptance |
| 2.1 Rust migration | Rust core and CLI, Blender ctypes binding, Godot Rust extension, new directory layout, fixes for review-confirmed defects, removal of the Go/Python reference implementation |
| S2 | Public P2P, signed releases and maintenance authority, peer identity, signaling/TURN, regional pins, resume and failover |
| Extensions | Godot Editor placement, richer model/instance reuse, visual diff, RealmHub/Supabase, Steam, other OS runtime tests, mobile/Web |

Git branches, forks and three-way file merges belong to S1. S2 must not invent another commit DAG. Pre-research means no compatibility: 2.1 does not read the 2.0 layout or its shared index files, does not preserve old view-root hashes, and existing realms are regenerated from scripts.

## Realm and authoring tree

The root realm.json fixes realm identity and the bounded Cartesian profile: right-handed, Y-up, integer millimeter placement, default 32 m chunks. Chunk boxes are half-open; negative coordinates use floor. A spanning entity is defined once and indexed in every intersecting chunk. Moving preserves its stable ID. S1 instance roots support translation; rotation/scale can be applied inside model geometry.

```text
realm-project/
├── .git/                    # Git history, sole index, LFS cache
├── .rou/                    # ignored selection/session/build/view/backup caches
├── .gitignore
├── .gitattributes
├── README.md
├── realm.json               # world rules, immutable after init
├── realm/
│   ├── models/<id>.json     # model definition: which .blend entry to export, which dependencies
│   └── entities/<id>.json   # placement: which model, at which millimeter position
├── sources/                 # author's complete .blend/library/texture tree, LFS
├── runtime/
│   ├── <model>.glb          # exported model written by rou build, LFS
│   └── <model>.json         # build record for that GLB: input hashes, GLB hash, local bounds
└── docs/
```

Three rules: authors write only realm/ and sources/; rou build writes only runtime/; every model has exactly one GLB plus one record of the same name under runtime/. One file per ID and an immutable realm.json mean no single file is edited by many contributors. Sources hold geometry and authoring scenes, models say what to export from where, entities say where it goes. Only the Git index stages changes. The original project does not depend on its exported GLB.

The complete spatial catalog and view root are derived on read from versioned inputs and per-model build records; they are never committed as shared files. The view root SHA-256 is computed over RFC 8785 canonical JSON and remains the value S2 release records bind to.

## Daily operations

Use rou init/clone, switch/checkout, sparse, build, add, diff, commit, fetch/pull, merge and push. Native Git and IDEs remain usable; rou rereads their state and owns no second HEAD.

Clone obtains lightweight Git history and LFS pointers without payloads. realm/, runtime/ and docs/ are always checked out; unselected GLBs under runtime/ are pointers only. The selection decides which sources directories join the Git cone and which source files and GLBs are hydrated. Shrinking refuses to overwrite local work and relocates ignored author backups under .rou first.

## Build and commit

Blender saves the full original project. Placement writes are explicit and guarded by version/source/placement session baselines; an old Scene cannot silently replace newer JSON. Default build freezes saved working-tree inputs under .rou/build, exports per-model GLBs, writes runtime/ without staging. Records bind input bytes, entry, profile, exporter settings and output hashes. Commit validates the index, creates a native commit, then validates the actual result.

## Collaboration and viewing

Git merges independent source/entity/model files; rou checks identities, references and spatial results derived at read time. Different entities in one chunk merge without rebuilding a shared index, so hosted platforms can merge natively. Duplicate IDs are invalid even if text merges cleanly. Binary source conflicts and same-model record/GLB conflicts are resolved by authors, then rebuilt.

The viewer is read-only. Neither realm data nor the bundle contains viewer entry points: the start location is derived from content or chosen by the player. A trusted local rou bridge validates a fixed commit and exports catalog/GLBs; Godot neither runs Git/LFS/Blender nor reads .git. Catalog, version sidecar, GLB budget checks and spatial conversion come from the Rust extension, the same implementation rou uses; GDScript no longer duplicates rules. Bundles without a version sidecar are shown as unbound, never silently downgraded.

## S2 and optional services

Public assets have no application encryption. Hashes validate bytes; signatures identify publishers; transport uses standard secure channels. Release records bind realm/fork identity, Git object algorithm/commit and the SHA-256 view root after commit creation. P2P attaches behind LFS transfers and viewer providers; signaling and TURN are replaceable infrastructure. Node administration uses one local Web UI. RealmHub may reuse Supabase with independent API/worker services; core CLI, Blender, viewer and nodes do not depend on it.

## Acceptance

G1–G4 were completed by the 2.0 reference implementation. 2.1 passes the same black-box harnesses with the Rust implementation and the new layout: two-author Git/LFS flow, reviewed spatial test cases, fixed-version Godot workflow, Blender add-on registration and placement through ctypes including linked-library projects, regression cases for every review-confirmed defect, and all checks still passing after the Go/Python directories are deleted. Only the current Mac is required; other OS and mobile/Web are validated separately.
