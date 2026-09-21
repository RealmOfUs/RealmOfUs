# Authoring, acceptance and packaging

Production path: Rust `rou` → native Git/LFS → headless Blender → fixed-revision browse bundle → Godot. Python handles only host IO, bpy and acceptance orchestration; every rule is called through `librou`. Data and command semantics are defined in the [repository contract](../doc/protocol/repository.md).

| Module | Purpose |
| --- | --- |
| `../rou/` | Core, Git/CLI, C ABI and GDExtension |
| `blender/rou_blender/` | Complete original projects, placement, session guard, local preview |
| `blender/export_git_realm.py` | Export helper for object/collection entries of a frozen project |
| `build/native.py` | Builds and loads the native library for development scripts; no domain rules |
| `build/`, `tests/blender/` | Black-box acceptance and packaging |

## Development checks

The root `Makefile` is the only build entry point; scripts under `build/` consume its products and stop with the target to run when one is missing. The Rust version is pinned by `rou/rust-toolchain.toml`; `make check` creates `.venv` from `requirements-dev.txt` (Ruff and gdtoolkit) on first use.

```bash
make check                 # cargo fmt/clippy/doc, Ruff, gdformat
make test-rust             # cargo test --workspace
make test-spatial          # reviewed spatial cases through the Godot extension
make test-git              # two-author Git/LFS flow with real Blender and headless Godot
make test                  # all three
```

`verify_git_realm.py` performs every Git operation inside a fresh acceptance directory and records each command, result and log; it never touches the source repository's index. Run it directly to watch Godot in a window: `python3 toolchain/build/verify_git_realm.py --windowed`.

On macOS the generated header can be exercised from C to check the ABI, success and error envelopes and release:

```bash
mkdir -p build/rust-validation
clang -std=c11 -Wall -Wextra -Werror -I rou/include \
  rou/crates/rou-ffi/tests/c_smoke.c -L rou/target/debug -lrou \
  -Wl,-rpath,"$PWD/rou/target/debug" -o build/rust-validation/ffi-c-smoke
build/rust-validation/ffi-c-smoke
```

## Blender extension

```bash
make addon                 # dist/rou_blender.zip with the native library for this machine
make test-addon            # local preview, authoring session and real install checks
```

The ZIP uses the Blender 4.2+ extension manifest: Python modules and `blender_manifest.toml` at the root, and `lib/<platform>/` carrying the release library for the current machine architecture, so the package never depends on the development `target/` directory. The display name is Realm of Us, the extension ID `rou_blender`, and operators stay `rou.*`. Install through Preferences → Install from Disk, then use the Realm tab in the 3D viewport's N panel.

After enabling or re-enabling the extension, reopen the author project to establish a session. `rou.register_entities` registers models and entities; `rou.write_placement` writes placement; "save source only" keeps the external JSON untouched, and "re-apply placement" adopts the current scene positions after an explicit action. The session baseline covers only the records of the current source; complete metadata is still checked for references and budgets. Linked libraries, external textures and `//` relative paths have real Blender regressions.

Author roots are located by the object's `rou_entity_id`; the Scene property `rou_model_owners_json` records which entity owns each model definition, so objects are found by ID after reopening, re-applying placement and undo. Model IDs come from the entity records and need not equal the entity ID. When an existing project is bound for the first time, a single instance adopts its entity ID; a shared model requires the existing entry object to carry a matching entity ID, otherwise registration reports that the author cannot be identified. Registration synchronises the author root's new name into the model JSON; instance names never change the entry. After Shift+D two live roots cannot share an ID; after deleting the original, the copy with the same ID can continue as the author root. Bindings are saved with the complete source and with "save source only", add no fields to realm JSON and never enter a bundle.

After renaming a registered author root, run "register entities" before saving and building. Complete save and "save source only" both check that entry names match the current author roots and write nothing on failure. A native Blender save bypasses the add-on buttons; the next `rou build` checks the persisted author identity against the frozen snapshot's entity associations and refuses to export when a name drifted or an author root is missing, asking you to register and save again.

The real Preferences install and enable/disable test in `make test-addon` runs with `BLENDER_USER_*` pointed at `build/addon-install/`, so it never writes your user configuration.

The add-on's local preview writes only `realm.json` and `objects/`; without a version sidecar the viewer shows the bundle as unbound. Git staging and commits stay explicit author actions; the session guard cannot intercept external programs or native Blender saves.

On macOS, packaging ad-hoc signs the dylib inside the ZIP and verifies it with `codesign --verify --strict`. This is neither Developer ID signing nor notarisation. A delivery probe records quarantine attributes, `spctl` and the real Blender load result on isolated copies without changing system policy:

```bash
python toolchain/build/verify_addon_macos.py --addon-zip dist/rou_blender.zip --output build/macos-delivery-probe
```

A normal install, or loading after removing the quarantine attribute by hand, passing does not mean a public download is admitted automatically; formal distribution still needs signing, notarisation and a real download-path acceptance.

## Standalone viewer

Run `rou view` to produce a fixed-revision directory, then package it with the native extension for the current platform:

```bash
make viewer REALM=/path/to/view/realm.json          # fetches export templates, writes dist/viewer/
python3 toolchain/build/verify_git_viewer.py \
  --app "dist/viewer/macos/Realm of Us.app" \
  --output build/viewer-acceptance --seconds 12 --activate --at=12,3,5
```

`--at` picks a metre position in the actual world. The app has no Git, LFS or Blender runtime dependency and contains no source projects. Mac packaging extracts the arm64 binary from the official universal template, leaves the template untouched and ad-hoc signs the app. Only macOS arm64 is accepted; exporting for another platform needs that platform's native library and template, and an export succeeding is not a real-hardware pass.

The viewer's default start, map/entity/coordinate picking and local recent positions follow the [bundle contract](../doc/protocol/viewer-bundle.md). Recent positions are stored per realm ID in Godot's user directory; `--at` takes precedence and `--no-recent` makes tests repeatable. The map can select a Y layer. The loading radius is always 48 m; a high default overview may show no nearby entities until a place is picked.

The `godot` crate is pinned to 0.5.5 with API 4.7. The generic entry point is `Rou.invoke`, which no longer shadows `Object.call`; `load_catalog` and `catalog_query` reuse the Rust-validated catalog index.

## Other tools

- `verify_s1.py`: Rust spatial tests, real GLB import and AABB checks, viewer lifecycle.
- `compare_grids.py`: 16/32/64 m calibration comparison computed through the native library; the observed positions are test data.
- `benchmark_viewer.py`: real-window frame times, RSS and start-up time, with focus validity recorded separately; supports `--at` and `--sample`.
- `client/tests/rendered_workflow.gd`: scripted input in a real window to check navigation, file open/reload and the sample room map.
- `examples/villa_anahid/`: modelling and local tour sample that never writes sample waypoints into a generic bundle.
