# Realm of Us · Product and System Design

Baseline 2.1. Realm of Us is "Git for a multi-user, distributed, open 3D world": a realm is a collaborative authoring project with immutable spatial rules. Contributors clone it, branch, select a region, model in Blender, commit and merge; anyone can explore a fixed revision in an independent Godot viewer.

The data contract is in [protocol/](protocol/README.md), the implementation structure in [architecture.md](architecture.md), an end-to-end walkthrough in [tutorial.md](tutorial.md), and the current acceptance state and known limits in [status.md](status.md).

## 1. Goal and scope

Native Git owns the complete authoring tree and the sole history. Standard Git LFS stores the heavy files. The daily interface is `rou`, which adds spatial selection, builds, domain validation and the hand-off to the viewer. Platform accounts and commercial distribution are not prerequisites for local authoring.

Every rule has exactly one implementation, in Rust: `rou` is a single binary; the same core library is exposed through a C ABI to the Blender add-on (via `ctypes`) and through GDExtension to the Godot viewer, and both hosts use the same operation table. Python remains only for bpy-dependent add-on logic; GDScript remains only for viewer UI and load scheduling.

| Stage | Scope |
| --- | --- |
| S1 (current) | Spatial rules, complete original projects, Git/LFS, branches and standard remotes, region-based source hydration, builds and validation, merge, independent viewing |
| S2 (planned) | Public P2P, signed releases and maintenance authority, peer identity, signaling/TURN, regional pins, resume and failover |
| Extensions | Godot Editor placement, model/instance reuse, visual diff, RealmHub/Supabase, Steam, other OS runtime tests, mobile/Web |

Ordinary Git branches, forks and three-way merges belong to S1. S2 will not invent another commit DAG. This is pre-research: there is no compatibility layer for earlier layouts, and any incompatible change bumps a format version and regenerates data.

## 2. World and space

The root `realm.json` fixes the realm identity and the bounded Cartesian profile: right-handed, Y-up, metres for display, integer millimetres for placement, 32 m chunks by default. The same lineage cannot change its rules; changing rules means creating a new realm.

A chunk is a cuboid index cell, not a model file. Chunk bounds are half-open and negative coordinates use floor. An entity carries a stable ID, the model it uses and a world position; a spanning entity is defined once and indexed in every chunk it intersects, and moving it preserves its ID. Entity roots support translation only; rotation and scale live inside the model's child objects.

## 3. Repository layout

```text
my-realm/
├── .git/                    # Git history, sole index, LFS cache
├── .rou/                    # ignored: selection, session, build scratch, browse bundles, backups
├── .gitignore
├── .gitattributes
├── README.md
├── realm.json               # world rules, immutable after init
├── realm/
│   ├── models/<id>.json     # model definition: which .blend entry to export, which dependencies
│   └── entities/<id>.json   # placement: which model, at which millimetre position
├── sources/                 # the authors' complete .blend files, libraries and textures (LFS)
├── runtime/
│   ├── <model>.glb          # full model written by rou build (LFS)
│   ├── <model>.lod1.glb     # simplified middle-detail model (LFS)
│   └── <model>.json         # build record: input hashes, GLB hash, local bounds
└── docs/                    # ordinary text
```

Three rules: authors write only `realm/` and `sources/`; `rou build` writes only `runtime/`; every model has two GLBs (full and lod1) plus one record. One file per ID and an immutable `realm.json` mean no single file is edited by many contributors, so independent work merges with plain Git. Sources hold geometry and the authoring environment, models say what to export from where, entities say where it goes; there is never a second copy of a model. Author directories are free-form and unrelated to chunk coordinates. Only the Git index stages changes; there is no extra staging layer.

The complete spatial catalog and the view root are derived on read from the versioned inputs and per-model build records; they are never committed as shared files. The view root SHA-256 is computed over RFC 8785 canonical JSON and is the value future release records bind to.

## 4. Commands and selection

The daily commands are `rou init`/`clone`, `switch`/`checkout`, `sparse`, `build`, `add`, `diff`, `commit`, `fetch`/`pull`, `merge` and `push`. Native Git and IDEs keep working on the same repository; rou rereads their state and owns no second HEAD.

`clone` obtains the lightweight Git history and LFS pointers without payloads. `sparse set` replaces the selection, `add` extends it, `list` shows it; `--chunk` and `--entity` combine, `--all` selects the whole current realm. An inclusive range such as `1,-1,0..-1,1,0` normalises each axis independently. Spanning entities and shared dependencies are deduplicated.

`realm/`, `runtime/` and `docs/` are always checked out; unselected GLBs under `runtime/` are pointers only. The selection decides which `sources/` directories join the Git cone and which source files and GLBs are hydrated. A pointer that appears in the cone is not a downloaded model; selecting one entity of a large project still downloads the whole project. Shrinking a selection never deletes history or prunes caches; overwriting local edits is refused, and ignored author backups are moved under `.rou/backups/` first.

Creating a branch downloads nothing; switching revisions recomputes dependencies. Git cleanliness, the current selection, payload presence and world validity are reported separately. `fetch` leaves working files untouched; `pull` is fast-forward only, divergent history needs an explicit `merge`.

## 5. Authoring, build and commit

Blender saves the complete original project. Placement is written to entity JSON by explicit operations; a stale Scene can never silently overwrite newer JSON. An editing session is pinned to a revision plus source, model and placement baselines; after an external switch the author reloads, or explicitly re-applies the current scene placement to the latest JSON.

`rou build` freezes the saved working-tree inputs under `.rou/build`, exports through headless Blender and writes back to `runtime/` without staging. Only paths that are explicitly absent because of sparse selection fall back to the index; a merge builds from the merged index; a real deletion stays a deletion. `--staged` builds from the index but still writes to the working tree.

`runtime/` is machine-produced, versioned content, not a disposable cache: re-exporting the same inputs need not reproduce the same bytes or LFS object IDs. An upstream repository publishes regenerated content after format upgrades; consumers pull those outputs.

The same frozen Blender session exports full geometry and a quarter-ratio Decimate lod1 from temporary evaluated copies; small meshes retain their geometry. LOD textures use a maximum 1024-pixel edge with aspect ratio preserved; full textures and live author images remain unchanged. Geometry and image policy come from core configuration, and source files are never overwritten. Build reports sizes and advisory warnings. Hard rejection is reserved for parsing safety limits; the immutable Realm root gains no budget or quality fields.

A build record binds source and dependency bytes, entry, model definition, profile, exporter settings and output hash/length. A changed shared dependency invalidates every model that references it. `rou add` stages explicitly; `rou commit` validates the index, creates a native commit, then validates the actual commit. Staged v1 with unstaged v2 is valid; staged v2 with stale v1 output is rejected. Artistically unfinished but internally consistent revisions are allowed, and README-only commits never start Blender.

Git/LFS deduplicates identical whole files; there is no promise of semantic deduplication across `.blend`/GLB or delta storage for similar binaries.

## 6. Collaboration

| Parallel change | Handling |
| --- | --- |
| Independent entities or models | Git merges files, rou validates the domain result |
| Different entities in one chunk | Git merges files; the index is derived on read, no extra build |
| Same entity, different fields | Text merge, then identity/placement/reference checks |
| Duplicate ID in different files | Rejected even when Git reports no conflict |
| Same `.blend` or texture | Git keeps base/ours/theirs; authors resolve in the modelling tool |
| Same model's record or GLB | Resolve the author inputs, then build; never hand-pick an old hash |

A divergent merge stays pending for inspection, build, add and commit; fast-forward is handled separately. GLB conflicts are resolved through sources and builds; geometry is never merged automatically. Independent contributions need no post-merge rebuild of a shared index, so hosted platforms can merge natively; domain validation still checks duplicate IDs, references and build freshness.

The standard remote handles accounts and write access; a Git author configuration is not an authenticated signature. `push` follows newly reachable history, not the current selection, and a new fork may need older payloads first. An ordinary push is neither a formal release nor a retention guarantee.

## 7. Independent viewing

The Godot viewer offers free flight, region and entity information and fixed-revision identity. Neither realm data nor the browse bundle contains viewing entry points: the default start is derived from the union of entity bounds, the player picks a place from the chunk map, the entity list or typed coordinates, recent positions are stored in the local user directory, and `--at=x,y,z` sets a start directly. House tours are an optional sample with their own local waypoint data. The viewer has no placement editing; that belongs to a future Godot Editor plugin.

`rou view` validates a fixed commit and exports a complete catalog with the saved selection's locally available models by default; explicit selectors or `--all` may fetch more models. The independent viewer app is distributed per platform and opens any browse directory; Godot reads only that directory and its GLBs, never runs Git/LFS/Blender and never reads `.git`. The bundle carries version identity and hashes; the local bridge is trusted locally and is not a public signature. Catalog, version sidecar, GLB budgets and spatial conversion come from the Rust extension, the same implementation rou uses.

Rust selects distance tiers by chunk AABB distance, deduplicates spanning entities, and applies residency hysteresis. The viewer shows complete GLBs with automatic mesh LODs nearby, loads build-produced lod1 assets with simplified geometry, textures up to 1024 pixels and reduced shadow/GI cost in the middle tier, and uses catalog-bound boxes at a distance. Default radii are 48 / 200 / 4000 m, adjustable through `--radius` or Settings; they are viewer settings rather than profile fields. Boxes exist for every catalog entity and remain until a validated scene is mounted. Missing and rejected assets retain marked boxes and explicit reasons; unknown catalog, verified empty region and missing model remain distinct states.

Reads and hash/budget validation run in background workers, deduplicated by asset SHA. Display backends also parse glTF and generate mesh LODs there; the main thread creates scenes and mounts at most one instance per frame. Godot 4.7's headless dummy backend keeps glTF parsing and LOD generation on the main thread as well because of its RID concurrency limits. Completed unrequested assets are cached briefly; failed or timed-out tasks release active slots and expose errors, and retired loaders cannot attach old results. Unused prepared assets and prototypes expire. Missing lod1 stays a marked box without loading the full model; full and lod1 share imports only when their SHA matches. Radius policy remains in viewer settings. M2 performance acceptance uses balanced rendering; the default viewer preset is unchanged.

## 8. S2 and optional platform

Only public realms: no asset encryption, world passwords or content keys. Hashes validate bytes, signatures identify publishers, transport uses standard secure channels. Public reading needs no platform login; write access, maintenance authority, peer identity and local administration are verified separately.

Git OIDs record their algorithm explicitly; LFS and view roots use SHA-256. A release record is created after the commit and binds realm identity, fork governance identity, commit and view root, avoiding cycles. A clone grants no upstream write authority; signing a view root does not change Git's object algorithm.

Metadata travels through Git or a bridge; heavy assets attach P2P behind the LFS transfer or node interface. Signaling exchanges connection information and TURN relays when direct connectivity fails. Source discovery, session identity binding, resume, quotas, pins and source switching are accepted separately. A node only serves ranges explicitly enabled for sharing; drafts and caches are never public by default.

Administration is one local Web page per node; Godot only opens it. RealmHub may reuse Supabase Auth/PostgreSQL/Storage/notifications with independent API and worker services; nodes, CLI, Blender and the viewer do not depend on it, and a platform outage must not break local, cached or known-node paths.

## 9. Acceptance

S1 is accepted on the current Mac through black-box harnesses: the two-author Git/LFS flow with real Blender and Godot, the reviewed spatial cases, the fixed-revision Godot workflow, add-on registration and placement through `ctypes` including linked-library projects, and regression cases for every confirmed defect. S2 requires shared signature/authority vectors, two-node direct and relayed transfer with recovery, regional replica management and a demonstration without any platform. The current state and known limits are tracked in [status.md](status.md).
