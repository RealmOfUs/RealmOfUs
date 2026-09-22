# Realm of Us

A collaboration framework for a multi-user, distributed, open 3D world: Git for 3D scenes. Native Git owns the complete authoring projects and the sole history, standard Git LFS stores source and runtime assets, and `rou` adds spatial selection, builds, validation and fixed-revision viewing. The current scope is the local loop; public P2P, signatures and node management are the next stage.

The core is Rust. Blender calls `librou` through `ctypes`; Godot calls the same `rou_core::ops` table through a GDExtension. There is no second implementation of any rule.

[Documentation](doc/README.md) · [Tutorial](doc/tutorial.md) · [Design](doc/design.md) · [Architecture](doc/architecture.md) · [Data contracts](doc/protocol/README.md) · [Status](doc/status.md)

| Directory | Responsibility |
| --- | --- |
| `rou/` | Cargo workspace: Rust sources, tests, fuzz targets and build output; see [rou/README.md](rou/README.md) |
| `rou/crates/rou-core` | Pure rules: document types, strict JSON and JCS, space, GLB, hashes, projections, diff |
| `rou/crates/rou-git`, `rou/app` | Git/LFS, snapshots, sparse selection, build orchestration and the `rou` CLI |
| `rou/crates/rou-ffi` | C ABI and the generated `include/rou.h` |
| `rou/crates/rou-godot` | Godot native adapter |
| `toolchain/blender/rou_blender` | Blender add-on: bpy operations, complete-source sessions, add-on UI |
| `client/app` | Generic viewer; `samples/` holds the house demo data separately |
| `toolchain/build`, `toolchain/tests/blender`, `client/tests` | Acceptance harnesses, packaging and development tools |
| `examples` | Original CC0 calibration scenes and the Villa Anahid art experiments |

A realm's `realm.json` holds only `format`, `id` and `profile` and is immutable after init. `realm/models/` defines source files, model entries and dependencies; `realm/entities/` defines model references and millimetre positions; complete projects live in `sources/`; build output is `runtime/<model>.glb` plus a record of the same name. Browse bundles contain no viewing entry points; recent positions are kept in the local user directory.

## Build and run

Requires Git, Git LFS, Blender 4.2+ and Godot 4.7. Acceptance uses Blender 5.2.1, Godot 4.7.2 and macOS arm64; `rou/rust-toolchain.toml` pins Rust 1.94.0. Other platforms are untested on real hardware.

All build and verification entry points are Makefile targets; `make help` lists them.

```bash
make rou godot-ext          # build/bin/rou and the Godot extension in client/app/bin/
export ROU_TOOLCHAIN_ROOT="$PWD"
export PATH="$PWD/build/bin:$PATH"

mkdir -p build/my-realm
pushd build/my-realm
rou init
# Save complete .blend files in Blender, register models and entities, then build, stage and commit explicitly.
rou build
rou add .
rou commit -m "Create a realm"
rou view HEAD --all --output .rou/view/current
popd

godot --path client/app -- --realm="$PWD/build/my-realm/.rou/view/current/realm.json"
```

Git identity comes from your local configuration; tests use isolated identities. `init` stages nothing and `build` never runs `add`. Select regions with `rou sparse set --chunk '1,0,1..-1,0,-1'`, extend with `rou sparse add --entity house`, or take every authoring dependency with `rou sparse set --all`. Built-in commands print human-readable summaries; add `--json` before or after a built-in command for its machine report. Any other command is forwarded unchanged to Git. Built-in commands print a short usage with `-h` and full rules and examples with `--help`, for example `rou sparse set --help`, `rou build --help`, `rou diff --help`; see [CLI help](rou/README.md#cli-help). The [tutorial](doc/tutorial.md) walks through a complete two-author flow.

The viewer is a separate platform application distributed as a precompiled release. Download it and open any browse bundle; a source checkout and Godot are only needed to build it yourself with `make viewer`. The app opens a world chooser when started without `--realm`. `rou view` defaults to the saved sparse selection without downloads; `--all` explicitly fetches every model. Models omitted from a bundle remain visible in its catalog as missing.

The viewer starts above the centre of all entity bounds at a distance equal to their diagonal, or 2 m above the origin for an empty world. Pick a place from the map, the entity list or typed metre coordinates, or pass `--at=12,3,5`. The loading radius is 48 m.

## Verification

```bash
make check          # cargo fmt/clippy/doc, Ruff, gdformat (creates .venv on first use)
make test           # Rust tests, spatial cases, two-author Git/LFS flow with Blender and Godot
make test-addon     # Blender extension: preview, authoring session, real install
make forgejo-up && make test-forgejo   # two-author flow over HTTP against a local Forgejo in Docker
```

`make test-git` creates a real Blender scene and an isolated bare Git/LFS remote in a fresh build directory and checks two-author collaboration, selection, reopening original projects, parallel edits, merge and fixed-revision viewing. Packaging (`make addon`, `make viewer`) and the standalone-app acceptance run are described in the [toolchain guide](toolchain/README.md). Current results and known limits are in [status.md](doc/status.md).

## Samples

[examples/](examples/README.md) keeps the original CC0 calibration scenes and the Villa Anahid reconstruction. The house tour is enabled explicitly with `--sample=villa-anahid`; its waypoints are local sample data and never enter a browse bundle.
