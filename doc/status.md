# Current status and known limits

Baseline 2.1 is implemented in Rust and accepted on the development machine (macOS arm64, Blender 5.2.1 LTS, Godot 4.7.2, Rust 1.94.0). This page records what is verified, how it is verified, and what is known not to be covered. It is updated whenever acceptance changes; historical rounds are not kept here.

## Verified

| Check | Command | Result on the current tree |
| --- | --- | --- |
| Rust format, lint and tests | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` in `rou/` | 71 tests including doc tests |
| Python and GDScript lint | `toolchain/build/check_code.py` | Ruff and gdformat clean |
| Spatial contract | `toolchain/build/verify_spatial.py` | 77 reviewed cases, 110 headless Godot assertions |
| Two-author Git/LFS flow with real Blender and Godot | `toolchain/build/verify_git_realm.py` | 27 flow checks, 58 commands, 104 Godot checks |
| Blender authoring session | `toolchain/tests/blender/verify_git_authoring.py` | 42 checks including linked libraries, shared instances, rename, undo and reload |
| Blender local preview | `toolchain/tests/blender/verify_addon.py` | 10 checks |
| Real extension install and lifecycle | `toolchain/tests/blender/verify_addon_install.py` in an isolated user directory | 62 checks |
| Godot local workflow | `client/tests/local_workflow.gd` via the preview harness | 28 checks |
| C ABI | `rou/crates/rou-ffi/tests/c_smoke.c` | Compiles and runs |
| Fuzzing | `cargo +nightly fuzz run` on `json_strict`, `glb_precheck`, `lfs_pointer` | Millions of iterations without a crash; not run on every change |

Deliverables are local products under `build/` and `dist/` and are not versioned: the release CLI `build/bin/rou`, the add-on `dist/rou_blender.zip` with the macOS arm64 native library, and a standalone viewer app.

## Known limits

- Only macOS arm64 has been tested on real hardware. Linux and Windows builds are expected to work but are unverified; Godot Web export and mobile are out of scope.
- Foreground rendering performance has been measured only on the small calibration sample and not re-measured after the latest changes; no claim is made for large worlds.
- The Villa Anahid sample logs a leaked-texture warning on exit under real-window tests; functionality passes, the cause is not located.
- The add-on's native library is ad-hoc signed. It loads after a normal extension install; a quarantined download does not load until the quarantine attribute is removed. Public distribution needs Developer ID signing and notarisation.
- "Save As" in Blender copies the add-on's ownership property into the new project; if the target project has an entity with the same ID, the shared model may bind to the wrong author. Cross-project bindings must be re-established by hand.
- The add-on's identity scan covers all Scenes and linked objects; a full Scene copy or a linked library carrying the same entity ID makes the session refuse to load until the duplicate is removed.
- The loading radius is fixed at 48 m; a high default overview of a large world may show no nearby entities until the player picks a place.
- S2 (public P2P, signatures, nodes) is designed but not implemented.
