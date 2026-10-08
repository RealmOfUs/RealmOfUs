# Current status and known limits

The 2026-10-08 storage round implements external content-addressed textures and chunk-paged catalogs in `rou-build/0.3`, `rou-preview/0.3` and `rou-blender-export/0.3`, with native ABI 3. Both 0.1 and 0.2 storage formats require regeneration. The Realm profile and hash remain unchanged. LOD policy still comes from `config.lod1`, including its 1024-pixel image edge. The current machine is macOS arm64, Apple M3, Blender 5.2.1 LTS, Godot 4.7.2 and Rust 1.98.1. Results are local observations, not platform-wide guarantees.

The 0.3 displayed streaming and six-image detailed checks pass, including external JPEG/PNG decoding. Two idle, mains-powered shared-asset benchmarks remain; an unlocked run switched from AC to battery and was excluded; historical 0.2 frame measurements do not substitute for that acceptance.

## Verified

| Check | Command | Result | Local date |
| --- | --- | --- | --- |
| Format, lint and documentation | `make check` | cargo fmt/clippy/doc, Ruff and gdformat pass | 2026-10-08 |
| Rust | `make test` | 100 tests including doc tests, with 30 CLI Git/LFS regressions and eight new storage/page contracts; shared-texture deduplication, deletion, LFS repair and retired 0.2 diagnostics covered | 2026-10-08 |
| Spatial, ABI and streaming | `make test`, final `make test-spatial` | 77 spatial vectors, 113 Godot ABI assertions, 12 Rust viewer tests, 126 headless streaming assertions including page/texture failures, 12 grid observations and the isolated worker-error recovery probe | 2026-10-08 |
| C ABI | C smoke command in `toolchain/README.md`, linked to `rou/target/release` | ABI 3, success/error envelopes and buffer release | 2026-10-08 |
| Displayed shared-asset performance | `make bench-viewer` | Two mains-powered runs of the committed code, 3143 and 3167 focused frames: mount P95 11.824 and 12.201 ms, steady P95 11.480 and 10.952 ms; peak/final RSS 388.97/281.42 and 400.61/291.23 MiB | 2026-10-07 |
| Displayed streaming | `client/tests/streaming_contract.gd` with Metal Forward+ | 126 assertions, including actual external JPEG/PNG pixels, independent image failures, paging and worker parsing; no engine errors | 2026-10-08 |
| Git/LFS with real Blender and Godot | `make test` | 41 flow checks, 62 commands, 154 Godot checks; paged publication, shared-image hydration and exact GLB/image validation | 2026-10-08 |
| Blender extension | `make test-addon` | 46 instancing checks, 52 geometry/material/UV/external-image/source-preservation LOD checks, plus local preview, authoring session and actual installation checks | 2026-10-08 |
| HTTP hosting | `make test-forgejo` | 20 checks, 38 commands against local Forgejo 11; external-image LFS, cold clone, sparse hydration and platform merges | 2026-10-08 |
| Historical revision diagnostics | `rou validate`, `rou view HEAD` in a pre-M3 test repository | Both reject with the current build format and regeneration instructions; malformed and duplicate-key JSON retain their original errors in contract tests | 2026-10-07 |
| Tutorial command replay | Exact section 2 bash block in an isolated generated realm | Four models build with eight statistics lines; add and commit succeed; working tree ends clean | 2026-10-06 |
| Generated examples | `make examples` | Calibration and Villa regenerated with paged catalogs and external images; detailed and scale bundles rebuilt separately | 2026-10-08 |
| Standalone viewer | `make viewer`, then packaged headless runtime with empty authoring PATH | macOS arm64 export under ABI 3 succeeds; a fixed external 0.3 Git bundle opens and imports four assets without authoring tools, rejected SHAs or engine errors | 2026-10-08 |
| High-detail realm rebuild | `rou build`, `rou validate --staged`, `rou view --worktree --all` in the existing sample | All three models regenerated as 0.3 and validated; both tiers use external images, lod1 edges at most 1024 pixels, source files preserved; a subsequent build reuses all three models | 2026-10-08 |
| High-detail display | `make test-detailed-display` | 64 assertions, six focused near/middle screenshots visually inspected, six distinct SHAs; middle images at most 1024 pixels; zero released scenes/prototypes, rejected assets or engine errors | 2026-10-08 |
| Catalog-size refresh cost | `client/tests/refresh_cost.gd` on `examples/scale/make_bundle.py --count N` bundles | With pages cached, far refresh P95 is 0.012 and 0.013 ms in two runs at 4096 entities; an 8192-entity bundle passes complete page validation | 2026-10-08 |
| Unique textured assets | `make_bundle.py --unique 16/64/128 --lod1-only` from the detailed sample, then `benchmark_viewer.py --flight --balanced` | Peak RSS 625 / 898 / 882 MiB; the 128-asset flight imports 56 of 128 in 30 s with two workers; see Scale measurements | 2026-10-07 |
| Collaboration bookkeeping | `toolchain/build/scale_collab.py generate/timing/scenarios` with the guarded fake exporter | 256- and 4096-model placeholder realms; validate/commit/build timing; five two-author scenarios with exact conflict and rebuild assertions; reproduced in two independent runs | 2026-10-08 |
| GLB/image fuzzing | `cargo +nightly fuzz run glb_precheck -- -max_total_time=60` | Precheck, externalisation and image headers: 6,834,010 runs after fixing a discovered PNG dimension arithmetic overflow, then 5,663,125 runs in 61 seconds after retaining image MIME types, with regression tests. No further crash; the default 4096-byte input limit does not exercise large JSON chunks | 2026-10-08 |
| GLB JSON chunk boundary and memory | Rust contract tests and separate-process FFI probes | Valid 8 MiB JSON chunk accepted; 8 MiB + 4 bytes, 32, 64 and 128 MiB rejected before parsing with `GLB_JSON_BYTES`; rejected cases add 32 KiB to the pre-call peak RSS | 2026-10-06 |

This round repeats Rust, spatial/headless streaming, real Blender/Git, add-on installation and local Forgejo checks. The packaged ABI 3 viewer also passed a headless fixed-version smoke test with four imports and no authoring tools on PATH. Tutorial replay, the detailed flight, repeated-cycle memory and older unique-asset display measurements remain historical. Current foreground streaming and detailed display checks pass; the two performance runs still await an unlocked display. No old screenshot or benchmark result is counted as a current pass.

The JSON chunk ceiling is inclusive and independent of the 256 MiB container ceiling. An adversarial 8 MiB JSON tree still reached 166.80 MiB peak RSS (129.91 MiB above the pre-call peak); larger chunks are rejected before building that tree. Input buffers themselves still consume memory proportional to their byte size. Shared linked meshes and collection instances now retain one simplified mesh per compatible evaluated mesh and effective material assignment. In the 20-linked-instance regression, full/lod1 sizes are 71,220 / 26,744 bytes, indices are 2,880 / 720, and both tiers contain one mesh.

The unchanged spatial fixture SHA-256 is `866cd547b12356335c2c317a0b0d695f8a512fa10ec0ba9e047ac49c79bf0d79`; the default `profile_hash` remains `c5ea37e2ce31d6894d505e45475b8791c530767b8bd7d81637bc16e5df4a1f97`. Historical spatial fixtures are adapted only inside test harnesses; production readers reject old build/preview formats.

Deliverables are local products under `build/` and `dist/`, not versioned: `make rou` builds `build/bin/rou`, `make addon` produces `dist/rou_blender.zip`, and `make viewer` exports `dist/viewer/macos/Realm of Us.app`. The standalone app embeds no world or authoring tools. Publication, signing and notarisation remain separate.

## Viewer performance

Acceptance uses **balanced** rendering (SDFGI disabled), with the normal rendering default unchanged. The table retains the 2026-10-01 baseline and adds the 2026-10-07 viewer-tier run. The 30-second, 800-entity flight uses three shared calibration GLBs, 64 occupied chunks, Metal Forward+, a 1920 x 1080 viewport and 48 / 200 / 4000 m radii. All four valid measurements include every frame from the second process tick, with no warm-up exclusion. Each mounts and unloads 800 instances, prepares three unique GLBs, and ends with no scenes or cached prototypes, rejected assets or engine errors.

| M3 run | Focused / total frames | Mount/unload frames / P95 | Steady frames / P95 | Peak / final sampled RSS |
| --- | --- | --- | --- | --- |
| 2026-10-01 before refresh optimisation: first | 2551 / 2551 | 808 / 14.978 ms | 1743 / 13.821 ms | 393.53 / 300.52 MiB |
| 2026-10-01 before refresh optimisation: repeat | 2522 / 2522 | 808 / 16.114 ms | 1714 / 14.097 ms | 389.88 / 306.81 MiB |
| 2026-10-01 after refresh optimisation | 2588 / 2588 | 809 / 14.380 ms | 1779 / 13.636 ms | 397.84 / 313.58 MiB |
| 2026-10-07 core tier scheduling (ABI 2), before dropping the `desired` map | 3255 / 3255 | 808 / 11.193 ms | 2447 / 13.760 ms | 451.89 / 276.50 MiB |
| 2026-10-07 committed code, mains power, run 1 | 3143 / 3143 | 808 / 11.824 ms | 2335 / 11.480 ms | 388.97 / 281.42 MiB |
| 2026-10-07 committed code, mains power, run 2 | 3167 / 3167 | 808 / 12.201 ms | 2359 / 10.952 ms | 400.61 / 291.23 MiB |

All three 2026-10-07 runs pass the 33 ms mount and 16.7 ms steady targets and the tighter M3 mount regression reference of 14.8 ms. The two runs of the committed code were taken back to back on an idle machine on mains power; same-session alternation against the previous commit gave 11.300 and 11.697 ms mount P95 for that build, so the viewer-tier round changes mount-frame cost by less than the 2 ms local-variation allowance. On battery the same build measured 16.4-17.0 ms mount P95 across five runs and fails the 14.8 ms reference, which is why acceptance is defined on mains power. Removing the per-entity `desired` map returned query responses to 174 bytes when nothing is in range and 31 KB at the world center for 800 entities, matching the previous build. Peak RSS is 13-25 MiB above the rounded 376 MiB handoff reference; final RSS is 19-29 MiB below its 310 MiB reference. Runs end with zero scenes and prototypes, but do not repeat the multi-cycle memory experiment. Main-thread scene-generation P95 was 24.130 and 13.131 ms across three scenes. Peak RSS is 75.89 MiB above the rounded 376 MiB handoff reference; final RSS is 33.50 MiB below its 310 MiB reference. It ends with zero scenes and prototypes, but does not repeat the multi-cycle memory experiment. Main-thread scene-generation P95 is 20.566 ms across three scenes.

The 2026-10-01 post-optimisation run also passed those frame targets; its preceding two runs exceeded the tighter reference and are retained above. A same-session headless CPU probe identified extra refresh work: loaded refresh P95 was 6.643 ms for the committed M2 control, 7.754 ms before the M3 optimisation and 6.038 ms after it. The fix skips failure reconciliation when no asset has failed and avoids adding every retained scene to the visible-box set only to remove it again. The ABI 2 query preserves those properties; only a failure-driven detach requests a second core visibility calculation. This probe isolates bookkeeping; it is not a rendering benchmark or proof of the entire frame-time difference. The 2026-10-01 final main-thread scene-generation P95 was 19.024 ms across three scenes. An earlier unfocused flight is excluded from acceptance; a displayed M2 control failed to obtain focus and supplies no frame-time comparison.

Memory acceptance means approaching a plateau over repeated entry/exit in one process, without continuing per-cycle accumulation; an immediate RSS drop is not required. A separate 90-second M3 run repeats the flight three times and completes 2400 mounts/unloads. Every cycle ends with zero scenes and cached prototypes. RSS in the final two seconds of each cycle is approximately 301.1, 295.1 and 293.8 MiB, with no per-cycle accumulation; engine static memory is 142.26, 142.39 and 142.58 MiB. The run has 3231 focused and 3529 unfocused frames, so it is used only for residency/RSS observation, not foreground frame-time acceptance. RSS cycle alignment uses the observed ready marker and has polling uncertainty of roughly 0.2 seconds. This finite observation does not prove the absence of leaks or indefinite boundedness.

Normal Godot acceptance commands reject every `ERROR:` line, including engine RID errors. The fault-injection probe expects exactly its deliberate worker error and verifies recovery. Reports retain failures by asset SHA even after the selected tier becomes usable, preventing a failed full asset from disappearing from acceptance when the camera moves to lod1.

## High-detail sample

The three-model sample uses CC0 [ArmChair_01](https://polyhaven.com/a/ArmChair_01), [WoodenChair_01](https://polyhaven.com/a/WoodenChair_01) and [wool_boucle](https://polyhaven.com/a/wool_boucle) from Poly Haven. SIMPLE subdivision increases furniture tessellation while preserving the scanned shape; it does not create new scanned detail. The woven panel is original procedural geometry. The provider's 4K wool textures are 4096 by 5076 pixels and remain at that size in full detail; lod1 uses 826 by 1024 pixels. Downloads carry verified provider checksums and recorded SHA-256 provenance. The following asset statistics are from the 2026-10-08 rebuild with external textures. Vertex and byte counts are measurements of these exports, not deterministic properties of the sources.

| Model / tier | GLB MiB | External images MiB | Vertices | Triangles | Max image edge | Decoded images MiB |
| --- | --- | --- | --- | --- | --- | --- |
| Armchair / full | 15.36 | 81.25 | 300,702 | 540,096 | 4096 px | 192.00 |
| Armchair / lod1 | 3.72 | 5.22 | 71,283 | 135,023 | 1024 px | 12.00 |
| Wooden chair / full | 10.38 | 54.92 | 210,769 | 344,960 | 4096 px | 192.00 |
| Wooden chair / lod1 | 1.98 | 4.37 | 48,608 | 86,240 | 1024 px | 12.00 |
| Woven panel / full | 28.05 | 57.22 | 525,825 | 1,048,576 | 5076 px | 237.94 |
| Woven panel / lod1 | 7.02 | 1.39 | 131,663 | 262,143 | 1024 px | 9.68 |

The 0.3 rebuild leaves 66.51 MiB of GLB geometry and 204.38 MiB of encoded images across both tiers. LOD triangle counts remain approximately 25% of full detail. The full panel warns for vertices and image edge; externalisation removes the previous GLB-byte reference warnings. No source-image dimensions or encoded image bytes are changed by externalisation. Decoded-image estimates are four bytes per pixel, not measured process/GPU residency. The current displayed verification passes after retaining each image's verified MIME type; omitting it caused Godot to probe JPEG bytes as PNG and emit errors before its successful fallback.

On 2026-10-08, `make test-detailed-display` passed 64 assertions and captured one near and one middle image for each model under Metal Forward+ at 1920 x 1080. Every capture followed at least 500 ms of continuous window focus. Near assets had maximum image edges of 4096, 4096 and 5076 pixels; all three middle assets had 1024-pixel edges and distinct SHAs. Release left zero resident scenes and cached prototypes, with no rejected assets or engine errors. The harness writes `build/scale/detailed-display/report.json` and six PNGs; its exact command and regeneration procedure are in `toolchain/README.md`.

The images were visually inspected: furniture silhouettes, materials and the panel pattern remain recognizable, with visibly softer fabric/wood detail in the middle tier and no obvious missing materials or broken geometry. Middle screenshots use a narrow FOV at 128 m from each entity center to expose detail at comparable screen size; poses and FOVs are recorded. This is a local visual observation, not a general quality guarantee or performance sample. The 1024-pixel policy is unchanged. The first unlocked 0.3 run was rejected for decoder errors caused by missing external-image MIME hints. The converter now retains the signature-verified MIME, JPEG/PNG pixel regressions pass, and the regenerated bundle captures without engine errors. All external image descriptors remain unchanged.

The detailed-flight performance numbers below remain from 2026-10-01 and use earlier exports. They were not remeasured by the functional display harness.

The final detailed flight uses the same balanced preset, viewport and default radii as the shared-asset flight. All 3028 frames retain focus. It records 10 mount/unload frames (P95 **71.554 ms**) and 3018 steady frames (P95 **13.072 ms**), with peak/final RSS **1374.59 / 402.78 MiB**. Eight worker preparations and eight scene generations cover six distinct SHAs as tiers change and expired prototypes are reloaded; nine instances mount and unload, with zero final scenes, cached prototypes or asset failures. Worker preparation P95 is 2234.442 ms and main-thread scene generation P95 is 40.866 ms. The small mount-frame count makes its P95 sensitive to individual stalls. An initial flight lost focus and is excluded from foreground acceptance.

This is a measured M4 baseline, **not a 33 ms mount-budget pass for large assets**. Main-thread materialisation and texture upload still cause visible stalls despite background preparation. No renderer quality, reference warning or safety ceiling was lowered to hide them.

## Scale measurements

The catalog and storage sections below were remeasured for the 2026-10-08 storage implementation. The unique-asset display table remains historical. These synthetic/local observations do not define production capacity.

**Catalog size.** `examples/scale/make_bundle.py --count N` places N instances of the three calibration models (three shared GLBs, so this isolates catalog bookkeeping from asset cost). `client/tests/refresh_cost.gd` measures one `refresh_assets()` on the main thread, P95 over 30 rounds, at the world center (every entity inside the middle radius) and far from everything (nothing to load; the response is 193 bytes).

| Entities | Query at center | Refresh at center | Response | Query far | Refresh far |
| --- | --- | --- | --- | --- | --- |
| 800 | 0.721 ms | 2.607 ms | 31,386 B | 0.009 ms | 0.014 ms |
| 1600 | 1.475 ms | 5.584 ms | 62,586 B | 0.008 ms | 0.014 ms |
| 2400 | 2.121 ms | 7.556 ms | 93,786 B | 0.007 ms | 0.012 ms |
| 3200 | 2.899 ms | 10.334 ms | 124,986 B | 0.007 ms | 0.012 ms |
| 4096 | 3.834 ms | 14.588 ms | 159,930 B | 0.007 ms | 0.012 ms |

The 2026-10-08 headless probe uses 30 rounds with all pages retained. Far refresh is now governed by 64 page/chunk entries and the visible set, not every entity; the 4096 result falls from 23.33 ms to 0.012 ms, with a 193-byte response including `missing_pages`. Center cost still grows with the candidates returned to Godot; the sort computes each candidate's center distance once. The 3200-point outlier is retained rather than smoothed. A second complete curve, after acceptance jobs finished, measured center refresh at 1.958 / 4.197 / 7.779 / 10.543 / 15.617 ms for the same five counts; far refresh stayed at 0.012 / 0.014 / 0.012 / 0.012 / 0.013 ms. These CPU probes are not frame benchmarks and do not establish foreground performance.

**Unique textured assets (historical 2026-10-07 display measurements).** `make_bundle.py --unique U --lod1-only` derives U byte-distinct GLBs from the detailed sample's lod1 assets (identical geometry and 1024-pixel textures, a tagged `asset.generator`, 6.35-8.94 MiB and 9.68-12.00 MiB decoded each) and places each once. The standard 30-second balanced flight then mounts as many as it reaches.

| Unique assets | Imports in 30 s | Cancelled | Worker P95 | Mount P95 | Steady P95 | Peak / final RSS |
| --- | --- | --- | --- | --- | --- | --- |
| 3 (800 shared instances, reference) | 800 | 0 | - | 12.2 ms | 11.0 ms | 401 / 291 MiB |
| 16 | 16 | 0 | 285 ms | 39.0 ms | 9.6 ms | 625 / 297 MiB |
| 64 | 64 | 0 | 393 ms | 19.7 ms | 10.6 ms | 898 / 352 MiB |
| 128 | 56 | 72 | 1065 ms | 28.3 ms | 14.9 ms | 882 / 414 MiB |

Two walls appear. Resident memory grows by roughly 8-15 MiB per unique textured lod1 asset, so peak RSS reaches 0.9 GiB with about 60 such assets resident, and a 4K-textured full asset costs an order of magnitude more (192-238 MiB decoded each, see the high-detail sample). Import throughput is bounded by two worker tasks at about one second per textured GLB: the 128-asset flight imports fewer than half of them in 30 seconds, the rest stay as boxes and are cancelled when the camera moves on, and worker P95 passes one second. Mount-frame P95 with few mount frames (19 and 63) is dominated by individual stalls and is not comparable with the 800-frame shared-asset runs. The design question M4 raises is density, not totals: how many unique textured assets fall inside the 200 m middle radius at once, which no per-realm constant controls today.

**Storage (2026-10-08).** In an isolated copy of the three-model source, each geometry edit changes one vertex by 1 mm and each texture edit changes one packed image pixel. Counts below are new runtime LFS object bytes, excluding `.blend` input revisions. LFS deduplicates unchanged image bytes globally; three models sharing one small image across both tiers create exactly one runtime image and one LFS object. Deleting one model preserves it; a simulated merged deletion is repaired from verified LFS history; deleting the last reference removes it.

| Model | Geometry-only edit: new GLBs / new images | One image-pixel edit: new GLBs / new images |
| --- | --- | --- |
| Armchair | 19.082 / 0 MiB | 15.359 / 2.416 MiB |
| Wooden chair | 12.361 / 0 MiB | 12.361 / 1.679 MiB |
| Woven panel | 35.068 / 0 MiB | 35.068 / 3.699 MiB |

A full initial runtime revision still holds about 270.89 MiB; externalisation principally saves repeated image storage on geometry revisions and across shared models/tiers. The former 390 MiB initial LFS observation also counted 119.5 MiB of source inputs and is not comparable to runtime-only deltas. Texture edits can still replace a whole GLB because its image URI contains the new SHA. One full-resolution pixel edit did not change the armchair's downsampled image, so that tier was reused by LFS. The pixel edit also changes encoding/compression; these small probes are examples, not a universal texture-edit ratio.

Two forced re-exports with the final converter added 12.36 and 10.38 MiB of wooden-chair geometry and zero image bytes; armchair and panel reused all runtime objects. Normal unchanged builds reuse their records without exporting. Geometry canonicalisation/quantisation is deferred: the observed residual is confined to seams in one model, the externalisation step must preserve geometry bytes, and adding lossy precision rules needs separate quality and fuzz evidence. No binary-reproducibility contract is claimed.

## Collaboration scale

The 2026-10-07 local M4 1a run measures Git/LFS bookkeeping on this Apple M3, on mains power, with a guarded deterministic fake exporter and 3,414-byte placeholder sources. Each source serves 64 models; eight of them share a dependency. Full/lod1 calibration templates carry model IDs, giving 512 distinct asset SHAs at 256 models and 8192 at 4096 models. Both generated realms pass full byte validation. Input revisions change build records but leave fake GLB bytes unchanged, so these results exclude real Blender work, binary conflict rates, historical LFS growth and network transfer.

The following subprocess wall times use five samples per operation after one validation warm-up. Staging is outside commit timing; saving the revision is outside build timing. P50 is the median and nearest-rank P95 is the maximum of five samples, not a stable tail estimate. Measurements ran sequentially, without concurrent acceptance jobs. `rou --version` took 14.9 ms at 256 and 12.9 ms at 4096.

| Operation | 256 models/entities P50 / P95 | 4096 models/entities P50 / P95 |
| --- | --- | --- |
| Validate HEAD | 1.018 / 1.656 s | 3.195 / 4.512 s |
| Commit after one placement edit | 2.853 / 5.069 s | 8.993 / 11.142 s |
| Commit after 64 placement edits | 2.633 / 3.657 s | 7.911 / 10.878 s |
| Build after one source edit | 2.508 / 2.767 s | 28.079 / 32.470 s |
| Build with nothing to generate | not measured | 34.7 s, single observation (24.3 s in the second run) |

Every source-edit build generates 64 models and reuses the other 192 or 4032. A second independent run on the same machine the next day, with the regenerated realms, measured medians of 0.418 / 1.818 s for validate, 0.950 / 4.269 s for the one-placement commit, 1.098 / 4.812 s for the 64-placement commit and 2.182 / 23.233 s for the source-edit build (256 / 4096 models), 1.2-2.7 times faster than the table with identical outcomes; the run-to-run spread is larger than the P50-P95 spread within a run. The 4096-model placement commit therefore corresponds to **455-748 proposals/hour for the commit step alone** (`3600 / 7.911` and `3600 / 4.812`), excluding staging, building, merging, review and transfer. A build with nothing to generate still costs 24-35 s at 4096 models: the build's fixed bookkeeping is roughly 6-8 ms per model and does not depend on how many models changed. Validation still reads and prechecks every available GLB despite cached payload hashes; commit validates both the index and the resulting commit. On the three-model detailed sample with 270.9 MiB of hydrated real assets, a warm `rou validate --staged` takes 1.40-1.46 s, about 190 MiB/s, so validation time is bounded by the hydrated selection rather than by the world: the 46 MB of placeholder GLBs account for about a quarter of a second of the 4096-model figure, and a sparse checkout of real assets validates at that throughput. Two points and five samples establish a local constant, not a fitted complexity law or sustained service capacity.

Two authors start from the same 4096-model seed through a disposable local bare repository. Each hydrates only the first entity's source/dependency and two GLBs; inherited cold assets remain explicitly reported as byte validation incomplete. Conflict paths and rebuilt IDs are asserted exactly.

| Scenario | Conflicting files | Validation after taking Alice's conflicting files | Models rebuilt |
| --- | --- | --- | --- |
| Different sets of 16 placements | 0 | Pass | 0 |
| Same placement and shared source | 66: entity, source, 64 build records | Pass | 0 |
| Shared dependency versus shared source | 8 build records | Reject stale records | 8 |
| Deliberately stale bare Git commit | 0 | Reject, also in a fresh clone | 8 |
| Add a dependency while upstream changes it | 0 | Reject, also in a fresh clone | 1 |

The same-source resolution takes 42.447 s and the shared-dependency resolution 27.646 s, each one automated observation including resolution, staging, validation, build and commit, excluding proposal preparation, the initial merge and human review; in the second run every case resolved in 26.7-30.4 s, including the disjoint case (59.7 s in the first run), so resolution cost is dominated by the fixed build bookkeeping rather than by the conflicts. Proposal commits in the sparse clones took about 3.0 s, less than in the fully hydrated realm, because few GLBs are available to precheck. Taking all of Alice's source and records leaves a consistent same-source result; taking her records alongside Bob's changed source leaves eight stale records in the cross-dependency case. Fake GLB pointers never conflict because their bytes do not change.

**A clean Git merge can be invalid even when both proposals passed `rou commit`.** In the last case, Alice edits a dependency and rebuilds its eight existing users. Bob adds that dependency to `m0008` and builds against its old contents. Their changed records do not overlap, so Git merges cleanly, but `m0008` now has stale inputs. Validation rejects `stale_build`, including in a fresh clone; rebuilding only `m0008` repairs it. The deliberately bare Git variant is also rejected, but bypassing rou is not necessary for this failure. Keep validation of the actual merged snapshot, as required by the repository contract; branch validation cannot replace it.

The 0.3 root and each chunk page have independent 4 MiB budgets. `PAGE_RECORDS` (renamed from `WORLD_RECORDS` because it no longer bounds the world) limits the sum of owned and foreign records on one page to 4096, with no independent world model/entity cap. The 4096-entity bundle has a 12,452-byte root, 4,960,235 page bytes total and a 101,308-byte largest page. The generated 8192-entity bundle also passes full cross-page validation: root 12,576 bytes, pages 9,919,719 bytes total, largest page 195,598 bytes. Pages remain cached until reopen; arbitrary world scale and long-run metadata memory are still unmeasured. The profile hash remains `c5ea37e2ce31d6894d505e45475b8791c530767b8bd7d81637bc16e5df4a1f97`.

The reproduction commands and measurement boundaries are in [the toolchain guide](../toolchain/README.md#collaboration-bookkeeping-measurements). Each run saves raw samples, conflict paths, rebuilt IDs, command output and a power-source observation under `build/scale-collab/reports/`. The storage/page implementation now uses 0.3, with geometry canonicalisation deferred after measuring its residual. A 64-model fake-exporter smoke realm also validates on the new format; the 256/4096 collaboration timings above remain pre-storage history. After completing foreground acceptance, follow-up work is real pull/LFS curves on the new layout, then M5.

## Hosting

A plain Git host with LFS is sufficient for authoring collaboration; no rou changes were needed. Observed on Forgejo 11.0 with a public repository:

- Anonymous `rou clone` and `rou sparse set` work: history and pointers come over smart HTTP, payloads over the LFS batch API without credentials. Pushing requires a collaborator token; a non-collaborator's push is rejected by the platform.
- Independent contributions on separate branches are mergeable through the platform's pull-request merge; the merge commit validates locally without any rebuild. Pull-request diffs show the multi-line model, entity and build-record JSON line by line, and LFS-tracked files as pointer changes.
- The acceptance run embeds tokens in the remote URL; a credential helper works equally. GitHub was not exercised; its LFS storage and bandwidth quotas are the main difference and are documented by GitHub.

## Known limits

- Frozen-source byte reproducibility remains unresolved for seam vertex/UV deduplication. In two final 0.3 re-export probes, wooden-chair geometry added 12.36 and 10.38 MiB, while all images and both other models were byte-stable. The earlier 79-byte UV difference and varying full vertex count remain relevant evidence of the cause. Externalisation preserves image/geometry bytes and reduces the repeated storage bill; vertex canonicalisation or quantisation is deferred. Upstream CI must validate recorded digests and semantics, not require fresh exports to match published GLB bytes.
- Current displayed streaming and high-detail visual acceptance pass after the MIME fix. Two mains-powered `make bench-viewer` runs remain. One fully focused run switched from AC to battery and is excluded (mount P95 12.391 ms, steady P95 18.309 ms). Historical 0.2 performance results do not close this requirement.
- Pages load synchronously on the main thread and remain cached until reopen. Steady far refresh is cheap, but page-arrival stalls and larger long-running worlds still need measurement. The root and page limits bound documents rather than resident CPU/GPU memory.
- Only macOS arm64 has been tested on real hardware. Linux and Windows are unverified; Godot Web export and mobile remain out of scope.
- The safety ceilings bound accepted input; they do not promise that every combination of maximum-sized assets fits available CPU/GPU memory. Many unique assets, larger worlds and long-run memory accumulation require measurement; 4096 now constrains a single page rather than world size.
- LOD reduces geometry and resizes oversized textures while retaining material slots and UV data. Full and lod1 images may coexist during replacement and caching. Texture compression and aggregate resident-memory policy still need separate measurement; the decoded-image safety ceiling is 4 GiB per GLB.
- Godot scene generation remains on the main thread. Headless dummy rendering also requires glTF parsing and runtime LOD generation on that thread; display backends perform preparation on workers. A native call cannot be forcibly cancelled, and exit joins outstanding tasks; permanent native blocking needs a process-level boundary.
- Default-quality steady-frame P95 exceeding 16.7 ms remains follow-up work. Balanced acceptance does not change the default or establish a default-quality pass.
- The earlier Villa Anahid displayed exit warning about leaked textures has not been re-investigated in M3; it remains open independently of these tests.
- The add-on native library is ad-hoc signed. Normal installation works; public quarantined distribution requires Developer ID signing and notarisation.
- Blender Save As copies ownership metadata. Cross-project bindings must be re-established when entity IDs overlap. Identity scans cover all Scenes and linked objects; duplicate entity IDs are rejected.
- Next work is the planned 1b upstream-node/network measurement, then M5. This round adds no CDC backend, P2P, real-time layer or Realm budget/radius-policy fields.
- Performance acceptance is measured on mains power. On battery, the same build measured mount-frame P95 3-5 ms higher on this machine, enough to fail the 14.8 ms regression reference; `make bench-viewer` records `power_source` in its receipt.
- S2 public P2P, signatures and nodes are designed but not implemented.
