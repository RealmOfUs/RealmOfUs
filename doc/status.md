# Current status and known limits

Baseline 2.1 is implemented in Rust and accepted on the development machine (macOS arm64, Blender 5.2.1 LTS, Godot 4.7.2, Rust 1.94.0). This page records what is verified, how it is verified, and what is known not to be covered. It is updated whenever acceptance changes; historical rounds are not kept here.

## Verified

| Check | Command | Result on the current tree |
| --- | --- | --- |
| Rust tests | `make test-rust` | 79 tests including doc tests; 27 CLI Git/LFS regressions |
| Format, lint and doc checks | `make check` | cargo fmt/clippy/doc, Ruff and gdformat clean |
| Spatial contract | `make test-spatial` | 77 reviewed spatial cases, 113 Godot ABI assertions, 9 Rust viewer tests, 46 headless streaming/LOD/recovery assertions, 12 grid-comparison observations and an isolated real worker-error recovery probe |
| Two-author Git/LFS flow with real Blender and Godot | `make test-git` | 33 flow checks, 62 commands, 154 Godot checks; offline default export, explicit subsets, hard links and missing models covered |
| Blender extension: local preview, authoring session, real install | `make test-addon` | 10 + 42 + 62 checks, including linked libraries, shared instances, rename, undo and reload |
| Standalone viewer | `make viewer`, then a dense-GLB run through the packaged app | macOS arm64 export succeeds; packaged headless runtime reads/validates a dense external GLB on a worker, then parses/generates 8 mesh LODs and creates the scene on the main thread; the displayed development pipeline also passes 46 assertions |
| Calibration import and lifecycle | `verify_s1.py` and three clean 50-model lifecycle runs | 404 geometry checks; 9 lifecycle assertions on each run; no RID or other engine errors |
| Tutorial command replay | [Tutorial](tutorial.md), replayed through Make in `/tmp` | Six exact text blocks and three build summaries checked; two real-window opens and the JSON diff passed |
| M2 displayed streaming | `client/tests/streaming_rendered.gd` with the 800-entity sample | 657 assertions; metadata overview, mixed full meshes/boxes, replacement without gaps, unload/restore, missing and one-byte-tampered assets, Settings and diagnostics; all six inspected screenshots have window focus |
| Godot local workflow | `client/tests/local_workflow.gd` via the preview harness | 34 checks passed |
| Hosted platform over HTTP (local Forgejo 11 in Docker) | `make forgejo-up` then `make test-forgejo` | 20 checks, 38 commands: authenticated push with LFS, cold clone, region hydration over HTTP, two pull requests merged on the platform without a local rebuild, `pull` fast-forward and validation of the platform merge, anonymous clone and anonymous LFS batch download of a public repository, non-collaborator push rejected |
| C ABI | `rou/crates/rou-ffi/tests/c_smoke.c` | Previously compiled and run; this change reran Rust FFI tests and Blender native calls |
| Fuzzing | `cargo +nightly fuzz run` on `json_strict`, `glb_precheck`, `lfs_pointer` | Millions of iterations without a crash; not run on every change |

Deliverables are local products under `build/` and `dist/` and are not versioned: `make rou` builds the release CLI `build/bin/rou`, `make addon` the extension `dist/rou_blender.zip` with this machine's native library, and `make viewer` an independent viewer app under `dist/viewer/`. The viewer is a platform release artifact that opens any browse bundle; it embeds no world and needs no authoring tools at runtime. This checkout produces local artifacts; publication and distribution signing are separate.

## Viewer performance

M2 performance acceptance uses **balanced** rendering (SDFGI disabled); the normal viewer default is unchanged. The current 30-second foreground run meets the 16.7 ms steady-frame P95 and 33 ms mount-frame P95 targets. It uses a synthetic 800-entity preview (three shared calibration GLBs, 64 occupied chunks), Apple M3, macOS arm64, Godot 4.7.2, Metal Forward+, 1920 x 1080 rendering and 48 / 200 / 4000 m radii. The route shows the metadata overview, crosses the world, then releases all full scenes. It mounts and unloads 800 instances, prepares three unique GLBs on workers, generates three scenes on the main thread and ends with zero cached prototypes. There are no engine/script errors or rejected assets.

| Current run | Mount/unload frames / P95 | Steady frames / P95 | Peak / final sampled RSS |
| --- | --- | --- | --- |
| `make bench-viewer` (balanced) | 809 / 12.830 ms | 2382 / 11.630 ms | 453.8 / 453.8 MiB |

All 3191 frames are focused. The harness waits for 500 ms of continuous focus before opening the world; frame sampling starts at the second process tick, without warm-up exclusion, and includes startup and the released overview. Main-thread scene generation belongs to the mount group; its P95 is 11.152 ms across three scenes. Every recorded frame counts even if focus is later lost. RSS sampling excludes process teardown and zero values at exit.

A separate 90-second same-process run repeats the route three times: 2400 mounts and unloads, all 9628 frames focused, and zero resident scenes and cached prototypes at every cycle end. RSS near the three cycle ends is 453.0, 453.9 and 454.2 MiB (about 1.1 MiB growth from the first to third), approaching a plateau rather than repeating the initial allocation increase. Memory acceptance now requires a plateau across repeated entry and exit, without continuing per-cycle accumulation; immediate RSS decline is not required. This finite observation alone neither proves the absence of leaks nor establishes indefinite boundedness. The shared low-poly calibration assets generate no useful LODs; a separate dense-GLB test verifies real LOD generation, including the exported app.

Normal Godot acceptance commands reject every `ERROR:` line, including engine RID errors; the isolated fault-injection probe expects exactly its one deliberate script error and verifies that the slot is recovered. These measurements do not establish a product-wide performance budget. No result is extrapolated to many unique assets, larger worlds, other hardware or other operating systems.

## Hosting

A plain Git host with LFS is sufficient for authoring collaboration; no rou changes were needed. Observed on Forgejo 11.0 with a public repository:

- Anonymous `rou clone` and `rou sparse set` work: history and pointers come over smart HTTP, payloads over the LFS batch API without credentials. Pushing requires a collaborator token; a non-collaborator's push is rejected by the platform.
- Independent contributions on separate branches are mergeable through the platform's pull-request merge; the merge commit validates locally without any rebuild. Pull-request diffs show the multi-line model, entity and build-record JSON line by line, and LFS-tracked files as pointer changes.
- The acceptance run embeds tokens in the remote URL; a credential helper works equally. GitHub was not exercised; its LFS storage and bandwidth quotas are the main difference and are documented by GitHub.

## Known limits

- Only macOS arm64 has been tested on real hardware. Linux and Windows builds are expected to work but are unverified; Godot Web export and mobile are out of scope.
- M2 meets its balanced foreground frame-time targets and approaches an RSS plateau over three shared-asset cycles. Long-run accumulation and large-world performance still need measurement. Default-quality steady-frame P95 previously exceeded 16.7 ms and remains follow-up work; balanced acceptance does not change the normal rendering default.
- Godot 4.7's headless dummy backend requires main-thread glTF parsing and LOD generation. Display backends prepare GLBs in workers; scene generation runs on the main thread in both modes. A timed-out native call cannot be forcibly cancelled and exit still joins it; permanent native blocking requires a process-level boundary.
- The Villa Anahid sample logs a leaked-texture warning on exit under real-window tests; functionality passes, the cause is not located.
- The add-on's native library is ad-hoc signed. It loads after a normal extension install; a quarantined download does not load until the quarantine attribute is removed. Public distribution needs Developer ID signing and notarisation.
- "Save As" in Blender copies the add-on's ownership property into the new project; if the target project has an entity with the same ID, the shared model may bind to the wrong author. Cross-project bindings must be re-established by hand.
- The add-on's identity scan covers all Scenes and linked objects; a full Scene copy or a linked library carrying the same entity ID makes the session refuse to load until the duplicate is removed.
- Middle detail still uses the full GLB with earlier mesh LOD switching and reduced shadow/GI participation. Radii are viewer settings; separate middle-detail assets and profile radius policy are not implemented.
- S2 (public P2P, signatures, nodes) is designed but not implemented.
