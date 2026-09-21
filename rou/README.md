# rou workspace

Rust code, tests, fuzz targets and native build artifacts live in this Cargo workspace.
The root manifest is virtual: executable packages live in `app/`, and libraries live
in `crates/`. Each package owns its manifest and source tree.

```text
rou/
|-- Cargo.toml                    # Members, shared dependencies and lints
|-- Cargo.lock                    # One lockfile for the main workspace
|-- rust-toolchain.toml           # Pinned toolchain
|-- app/
|   |-- Cargo.toml                # Package and executable: rou
|   |-- src/main.rs               # CLI parsing and dispatch
|   |-- src/help.rs               # Detailed command help
|   `-- tests/git_workflow.rs     # Real Git/LFS CLI integration tests
|-- crates/
|   |-- rou-core/
|   |   |-- Cargo.toml
|   |   |-- src/lib.rs            # Pure domain rules: rou_core
|   |   `-- tests/                # Contracts and reviewed spatial fixtures
|   |-- rou-git/
|   |   |-- Cargo.toml
|   |   `-- src/lib.rs            # Git/LFS and filesystem orchestration: rou_git
|   |-- rou-ffi/
|   |   |-- Cargo.toml
|   |   |-- build.rs              # Generate include/rou.h with cbindgen
|   |   |-- src/lib.rs            # C ABI: librou / rou.dll
|   |   `-- tests/c_smoke.c       # C ABI smoke test
|   `-- rou-godot/
|       |-- Cargo.toml
|       `-- src/lib.rs            # GDExtension: librou_godot / rou_godot.dll
|-- tests/common/                 # Shared test helpers, not an application crate
|-- include/rou.h                 # Generated public C header
|-- fuzz/                         # Separate nightly cargo-fuzz workspace
`-- target/                       # Shared Cargo build output, not versioned
```

Dependencies follow the actual boundaries:

```text
rou (app) ------> rou-git ------> rou-core
    `--------------------------> rou-core
rou-ffi -----------------------> rou-core
rou-godot ---------------------> rou-core
rou-fuzz ----------------------> rou-core
```

The core has no CLI/Git feature switches or host dependencies. The C ABI and Godot
packages depend directly on it. They keep distinct library names, build settings
and native-boundary lint rules. The CLI binary remains `rou`; C symbols, ABI version,
Godot class names and native library filenames are unchanged.

`tests/common/` supplies fixture constructors to core and CLI tests through explicit
module paths. Test cases remain with their owning packages. The reviewed spatial
fixture is `crates/rou-core/tests/fixtures/spatial.json`; Godot reads that same file.

## Build and run

The root `Makefile` (`make rou`, `make godot-ext`, `make ffi`, `make addon`) is the entry point that places products where the tools expect them. For direct Cargo use, run it from `rou/` so rustup selects the pinned toolchain. From the project root:

```bash
pushd rou
cargo build --workspace
cargo run -p rou -- --help
cargo build -p rou-core
cargo build -p rou-ffi -p rou-godot --release
popd
```

Default workspace members are the app, core and Git libraries. `cargo build` builds
the CLI and its libraries; `--workspace` also builds the native adapters. All members
share `rou/target/debug/` or `rou/target/release/`. Build scripts invoke Cargo from
this workspace directory. The C header stays at `rou/include/rou.h`.

## CLI help

Built-in commands use `-h` for a summary and `--help` or `rou help <command>` for
full behavior, parameters, defaults and examples. Help works outside a repository:

```bash
rou --help
rou sparse set --help
rou help sparse add
rou build --help
rou diff --help
rou view --help
```

Every `--chunk` help explains inclusive endpoints, independent axis min/max,
reversed and negative coordinates, deduplication, out-of-world rejection, and the
difference from Git revision ranges and half-open spatial bounds. `diff --help`
lists every supported domain-diff input form. Other commands document index versus
worktree behavior, LFS downloads, build output, merges and browse bundles.

Native Git commands retain the installed Git manual: inside a repository use
`rou add -h` for usage or `rou add --help` for the manual; outside use `git help add`.
Replace `add` with the desired native command. Git LFS uses `git lfs help <command>`.
`rou help <command>` applies to built-ins; native arguments are parsed by Git.

## Core API and validation

`rou_core` implements the 2.1 rules without filesystem, network, process or global
mutable state. `rou_git` and other hosts provide actual bytes. Data contracts live
under [doc/protocol](../doc/protocol/README.md). Read external JSON with
`rou_core::json::parse`, retaining strict encoding, duplicate-key and budget checks.

Serde document types enforce shape constraints. Cross-field and cross-document
checks use `validate` and `World::projections`; a valid profile shape alone does not
prove valid world bounds. Write versioned JSON with `json::pretty`, compute semantic
digests with `json::canonical`/`json::hash`, and hash actual file bytes with
`json::digest`.

Public core and Git modules, types, fields, constants and functions have English Rustdoc
covering units, preconditions, validation scope, errors and host responsibilities.
Code documentation and comments are English; source is ASCII where practical, with
Unicode escapes preserving required string values. See [code conventions](../AGENTS.md).

```bash
pushd rou
cargo doc -p rou-core -p rou-git --no-deps --open
cargo test -p rou-core --doc
popd
```

Generated documentation starts at `rou/target/doc/rou_core/index.html`.
`deny(missing_docs)` and `deny(rustdoc::broken_intra_doc_links)` protect both core and Git API
coverage. `check_code.py` builds their documentation, and `cargo test` runs the examples.

## Host adapters

[The operation table](crates/rou-core/src/ops.rs) documents all host operations,
input shapes, results and executable examples. `rou_core::ops::dispatch(op, input)`
returns a JSON-encoded value; `ops::envelope` produces `ok/value` or `ok/error`.
The FFI and GDExtension adapters own panic containment and boundary memory handling.

Operations cover strict/canonical/readable JSON, raw descriptors, ID/path checks,
LFS pointers, GLB prechecks, profile/document/build validation, spatial ownership,
intersections and selections, Blender conversion, world projection/diffs, session
fingerprints and viewer starts/queries/maps. Hosts do not duplicate these rules.

A `World` contains `realm`, `models` and `entities` keyed by ID, and `inputs` keyed
by model ID. Build records are also keyed by model ID. Browse catalogs contain no
viewing entries. There is no 2.0 reader or shared versioned chunk-index file.

## Tests and fuzzing

From the project root:

```bash
pushd rou
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p rou-core
cargo test -p rou --test git_workflow
popd
```

Core tests preserve reviewed expectations in `crates/rou-core/tests/fixtures/spatial.json`;
the original catalog fixtures only lost their retired `entries` field. GLB and domain
regressions include previously confirmed defects. RFC 8785 numeric and UTF-16 key-order
expectations are specified independently rather than generated by the implementation.

`fuzz/` uses libFuzzer to generate and mutate inputs for strict JSON, GLB prechecks
and LFS pointers. JSON and LFS targets also check encoding round trips. It is a
separate workspace with its own lockfile, nightly toolchain invocation and runtime;
it does not participate in normal application builds or `cargo test`.

Install `cargo-fuzz` and a nightly toolchain before running from the project root:

```bash
pushd rou
cargo install cargo-fuzz --locked
cargo +nightly fuzz run json_strict -- -max_total_time=60
cargo +nightly fuzz run glb_precheck -- -max_total_time=60
cargo +nightly fuzz run lfs_pointer -- -max_total_time=60
popd
```

Native C, Blender and Godot verification and packaging commands are in the
[toolchain guide](../toolchain/README.md).

Host failures retain anyhow context and expose stable categories through
`rou_git::error::code`: core domain codes, `git_failed`, `missing_payload`,
`io_error` and `invalid_json`. Host preconditions declare explicit codes including
`snapshot_changed`, `unresolved_conflict`, `stale_runtime`, `local_changes`,
`lock_busy`, `unsafe_path`, `missing_tool`, `invalid_selection`, `lfs_required`,
`payload_mismatch` and `realm_lineage`; `host_error` is reserved for unclassified
external failures. CLI diagnostics include
this code; native Git passthrough retains its original exit status.

Local source/lineage caches use a separate 128 MiB budget and fail open to
recomputation. LFS media paths are cached per snapshot, never across repository
configuration changes in a new operation. Frozen files and LFS object hashes
remain in snapshot-local memory. The persistent source hash cache retains at
most 100,000 entries and evicts the least recently used entries to make room.
Worktree payload resolution shares this cache. Cache hits update recency in memory;
disk persistence happens only when descriptors change, saving recency at that time.
A successful build with `--keep-build-runs 0` reports `run: null`. Build never
stages outputs and rejects index changes; no `index_changed` flag is returned.
Its baseline uses the same logical index bytes that were parsed for the snapshot.
Runtime symlinks and unselected tracked record deletions are rejected before fetching
or exporting. Chunk-based record repairs require fresh captured bounds; otherwise
select the entity explicitly before repairing its missing record.
Sparse hydration refreshes only matching payload stat entries, using a temporary
index that is removed on success, failure, SIGINT or SIGTERM.

The Godot adapter exposes `invoke`, `load_catalog` and `catalog_query`. The latter
two retain an immutable core loading index per opened bundle. Blender build
responses use explicit Z-up metre `bounds_blender_m`; both hosts use the core
`blender::bounds_native` conversion.
