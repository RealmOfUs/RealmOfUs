# Current status and known limits

Baseline 2.1 is implemented in Rust and accepted on the development machine (macOS arm64, Blender 5.2.1 LTS, Godot 4.7.2, Rust 1.94.0). This page records what is verified, how it is verified, and what is known not to be covered. It is updated whenever acceptance changes; historical rounds are not kept here.

## Verified

| Check | Command | Result on the current tree |
| --- | --- | --- |
| Rust tests | `make test-rust` | 71 tests including doc tests |
| Format, lint and doc checks | `make check` | cargo fmt/clippy/doc, Ruff and gdformat clean |
| Spatial contract | `make test-spatial` | 77 reviewed cases, 110 headless Godot assertions |
| Two-author Git/LFS flow with real Blender and Godot | `make test-git` | 27 flow checks, 58 commands, 104 Godot checks |
| Blender extension: local preview, authoring session, real install | `make test-addon` | 10 + 42 + 62 checks, including linked libraries, shared instances, rename, undo and reload |
| Godot local workflow | `client/tests/local_workflow.gd` via the preview harness | 28 checks |
| C ABI | `rou/crates/rou-ffi/tests/c_smoke.c` | Compiles and runs |
| Fuzzing | `cargo +nightly fuzz run` on `json_strict`, `glb_precheck`, `lfs_pointer` | Millions of iterations without a crash; not run on every change |

Deliverables are local products under `build/` and `dist/` and are not versioned: `make rou` builds the release CLI `build/bin/rou`, `make addon` the extension `dist/rou_blender.zip` with this machine's native library, and `make viewer REALM=...` a standalone viewer app.

## Known limits

- Only macOS arm64 has been tested on real hardware. Linux and Windows builds are expected to work but are unverified; Godot Web export and mobile are out of scope.
- Foreground rendering performance has been measured only on the small calibration sample and not re-measured after the latest changes; no claim is made for large worlds.
- The Villa Anahid sample logs a leaked-texture warning on exit under real-window tests; functionality passes, the cause is not located.
- The add-on's native library is ad-hoc signed. It loads after a normal extension install; a quarantined download does not load until the quarantine attribute is removed. Public distribution needs Developer ID signing and notarisation.
- "Save As" in Blender copies the add-on's ownership property into the new project; if the target project has an entity with the same ID, the shared model may bind to the wrong author. Cross-project bindings must be re-established by hand.
- The add-on's identity scan covers all Scenes and linked objects; a full Scene copy or a linked library carrying the same entity ID makes the session refuse to load until the duplicate is removed.
- The loading radius is fixed at 48 m; a high default overview of a large world may show no nearby entities until the player picks a place.
- S2 (public P2P, signatures, nodes) is designed but not implemented.
