# Current status and known limits

M3 implements separate parsing safety ceilings and advisory build references, full and lod1 build assets, and tier-specific streaming. LOD geometry and texture policy comes from `config.lod1`; its 1024-pixel texture limit is an export policy, not an input rejection or Realm field. The Realm profile and its hash are unchanged. The development-machine checks use macOS arm64, Apple M3, Blender 5.2.1 LTS, Godot 4.7.2 and Rust 1.94.0. The viewer-tier changes were validated on 2026-10-07 local date, including the displayed shared-asset benchmark and the current detailed sample. Native ABI 2 carries mounted SHAs into core tier hysteresis, replacement ordering and box visibility. Viewer packaging and detailed-flight performance observations remain from 2026-10-01 unless dated otherwise. These are observations on this machine, not product-wide guarantees.

## Verified

| Check | Command | Result | Local date |
| --- | --- | --- | --- |
| Format, lint and documentation | `make check` | cargo fmt/clippy/doc, Ruff and gdformat pass | 2026-10-07 |
| Rust | `make test` | 91 tests including doc tests, of which 29 are CLI Git/LFS regressions; distinct full/lod1 SHA publication and retired-format diagnostics covered | 2026-10-07 |
| Spatial, ABI and streaming | `make test` | 77 spatial vectors, 113 Godot ABI assertions, 12 Rust viewer tests, 97 headless streaming assertions, 12 grid observations and the expected isolated worker-error recovery probe | 2026-10-07 |
| C ABI | C smoke command in `toolchain/README.md`, linked to `rou/target/release` | ABI 2, success/error envelopes and buffer release pass | 2026-10-07 |
| Displayed shared-asset performance | `make bench-viewer` | Two mains-powered runs of the committed code, 3143 and 3167 focused frames: mount P95 11.824 and 12.201 ms, steady P95 11.480 and 10.952 ms; peak/final RSS 388.97/281.42 and 400.61/291.23 MiB | 2026-10-07 |
| Displayed streaming | `client/tests/streaming_contract.gd` with Metal Forward+ | 69 assertions; distinct full/lod1 hashes, replacement, shared-hash reuse, cancellation, expiry and independent missing-tier failures | 2026-10-01 |
| Git/LFS with real Blender and Godot | `make test` | 41 flow checks, 62 commands, 154 Godot checks; all four calibration models retain identical full/lod1 hashes, unchanged from the preceding round | 2026-10-07 |
| Blender extension | `make test-addon` | 46 instancing checks, 46 geometry/material/UV/texture/source-preservation LOD checks, plus 10 + 42 + 62 checks for local preview, authoring session and actual installation | 2026-10-07 |
| HTTP hosting | `make test-forgejo` | 20 checks, 38 commands against local Forgejo 11; LFS, cold clone, sparse hydration and platform merges | 2026-10-07 |
| Historical revision diagnostics | `rou validate`, `rou view HEAD` in a pre-M3 test repository | Both reject with the current build format and regeneration instructions; malformed and duplicate-key JSON retain their original errors in contract tests | 2026-10-07 |
| Tutorial command replay | Exact section 2 bash block in an isolated generated realm | Four models build with eight statistics lines; add and commit succeed; working tree ends clean | 2026-10-06 |
| Generated examples | `make examples` | Calibration and Villa previews regenerated with mandatory full/lod1 descriptors; two consecutive runs produced identical bytes for all 109 distinct Villa assets | 2026-10-07 |
| Standalone viewer | `make viewer`, then packaged headless runtime with empty authoring PATH | macOS arm64 export under ABI 2 succeeds; fixed external Git version opens and imports four assets, with no rejected SHA or engine errors | 2026-10-07 |
| High-detail realm rebuild | `rou build`, `rou validate --staged`, `rou view --worktree --all` in the existing sample | All three models regenerated and validated; lod1 image edges at most 1024 pixels, full textures unchanged; matching material names and unchanged source hashes; a subsequent build reuses all three models | 2026-10-07 |
| High-detail display | `make test-detailed-display` | 64 assertions, six focused near/middle screenshots, six distinct SHAs; middle images at most 1024 pixels; zero released scenes/prototypes, rejected assets or engine errors | 2026-10-07 |
| Catalog-size refresh cost | `client/tests/refresh_cost.gd` on `examples/scale/make_bundle.py --count N` bundles | Headless `refresh_assets()` P95 grows linearly from 4.3 ms at 800 entities to 30.7 ms at the 4096-entity catalog budget; see Scale measurements | 2026-10-07 |
| Unique textured assets | `make_bundle.py --unique 16/64/128 --lod1-only` from the detailed sample, then `benchmark_viewer.py --flight --balanced` | Peak RSS 625 / 898 / 882 MiB; the 128-asset flight imports 56 of 128 in 30 s with two workers; see Scale measurements | 2026-10-07 |
| GLB fuzzing | `cargo +nightly fuzz run glb_precheck -- -max_total_time=60` | 12,417,341 iterations in 61 seconds; no crash or timeout. The default 4096-byte input limit does not exercise large JSON chunks | 2026-10-06 |
| GLB JSON chunk boundary and memory | Rust contract tests and separate-process FFI probes | Valid 8 MiB JSON chunk accepted; 8 MiB + 4 bytes, 32, 64 and 128 MiB rejected before parsing with `GLB_JSON_BYTES`; rejected cases add 32 KiB to the pre-call peak RSS | 2026-10-06 |

Coverage not repeated in this viewer-tier round includes tutorial replay (last replayed on 2026-10-06), `client/tests/local_workflow.gd` (previously 34 checks), the displayed `client/tests/streaming_contract.gd` (previously 69 assertions), `client/tests/streaming_rendered.gd` (previously 657 displayed assertions), `toolchain/build/verify_s1.py` (previously 404 geometry checks and three lifecycle cycles), and all three fuzz targets. The 2026-10-06 JSON memory probes, 2026-10-01 large-seed GLB fuzz run (558 iterations over six detailed assets and a rejected 300 MiB input), earlier generated-example rebuilds, standalone viewer packaging, detailed-flight benchmark and three-cycle memory probe were also not repeated. These historical counts are not part of the current test total. The named detailed-display harness and the shared-asset benchmark below were rerun.

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

The three-model sample uses CC0 [ArmChair_01](https://polyhaven.com/a/ArmChair_01), [WoodenChair_01](https://polyhaven.com/a/WoodenChair_01) and [wool_boucle](https://polyhaven.com/a/wool_boucle) from Poly Haven. SIMPLE subdivision increases furniture tessellation while preserving the scanned shape; it does not create new scanned detail. The woven panel is original procedural geometry. The provider's 4K wool textures are 4096 by 5076 pixels and remain at that size in full detail; lod1 uses 826 by 1024 pixels. Downloads carry verified provider checksums and recorded SHA-256 provenance. The following asset statistics are from the 2026-10-07 rebuild with texture resizing. Vertex and byte counts are measurements of these exports, not deterministic properties of the sources.

| Model / tier | GLB MiB | Vertices | Triangles | Maximum image edge | Decoded-image estimate MiB |
| --- | --- | --- | --- | --- | --- |
| Armchair / full | 96.61 | 300,702 | 540,096 | 4096 px | 192.00 |
| Armchair / lod1 | 8.94 | 71,283 | 135,023 | 1024 px | 12.00 |
| Wooden chair / full | 65.30 | 210,763 | 344,960 | 4096 px | 192.00 |
| Wooden chair / lod1 | 6.35 | 48,608 | 86,240 | 1024 px | 12.00 |
| Woven panel / full | 85.27 | 525,825 | 1,048,576 | 5076 px | 237.94 |
| Woven panel / lod1 | 8.41 | 131,663 | 262,143 | 1024 px | 9.68 |

LOD triangles are approximately 25% of full triangles. The six GLBs total 270.89 MiB, of which lod1 accounts for 23.70 MiB. Only the three full-detail assets warn above the 32 MiB reference; the full panel also warns above 500,000 vertices and a 4096-pixel image edge. No lod1 reference warning remains. Validation succeeds without advisory warnings. Decoded-image estimates are four bytes per pixel, not measured process or GPU residency. Normal-map resampling is lossy; no visual-quality or frame-time claim follows from the file-size reduction.

On 2026-10-07, `make test-detailed-display` passed 64 assertions and captured one near and one middle image for each model under Metal Forward+ at 1920 x 1080. Every capture followed at least 500 ms of continuous window focus. Near assets had maximum image edges of 4096, 4096 and 5076 pixels; all three middle assets had 1024-pixel edges and distinct SHAs. Release left zero resident scenes and cached prototypes, with no rejected assets or engine errors. The harness writes `build/scale/detailed-display/report.json` and six PNGs; its exact command and regeneration procedure are in `toolchain/README.md`.

The images were visually inspected: furniture silhouettes, materials and the panel pattern remain recognizable, with visibly softer fabric/wood detail in the middle tier and no obvious missing materials or broken geometry. Middle screenshots use a narrow FOV at 128 m from each entity center to expose detail at comparable screen size; poses and FOVs are recorded. This is a local visual observation, not a general quality guarantee or performance sample. The 1024-pixel policy is unchanged. The initial harness run rejected a stale browse bundle containing 4096/5076-pixel lod1 textures; `rou view --worktree --all` republished the already rebuilt runtime assets, and the rerun passed without changing sources.

The detailed-flight performance numbers below remain from 2026-10-01 and use earlier exports. They were not remeasured by the functional display harness.

The final detailed flight uses the same balanced preset, viewport and default radii as the shared-asset flight. All 3028 frames retain focus. It records 10 mount/unload frames (P95 **71.554 ms**) and 3018 steady frames (P95 **13.072 ms**), with peak/final RSS **1374.59 / 402.78 MiB**. Eight worker preparations and eight scene generations cover six distinct SHAs as tiers change and expired prototypes are reloaded; nine instances mount and unload, with zero final scenes, cached prototypes or asset failures. Worker preparation P95 is 2234.442 ms and main-thread scene generation P95 is 40.866 ms. The small mount-frame count makes its P95 sensitive to individual stalls. An initial flight lost focus and is excluded from foreground acceptance.

This is a measured M4 baseline, **not a 33 ms mount-budget pass for large assets**. Main-thread materialisation and texture upload still cause visible stalls despite background preparation. No renderer quality, reference warning or safety ceiling was lowered to hide them.

## Scale measurements

These are the first M4 numbers: synthetic bundles, one machine, mains power, the committed viewer-tier code. They describe where the current design stops being comfortable; they do not set limits, and nothing was tuned to improve them.

**Catalog size.** `examples/scale/make_bundle.py --count N` places N instances of the three calibration models (three shared GLBs, so this isolates catalog bookkeeping from asset cost). `client/tests/refresh_cost.gd` measures one `refresh_assets()` on the main thread, P95 over 30 rounds, at the world center (every entity inside the middle radius) and far from everything (nothing to load; the response is 174 bytes).

| Entities | Query at center | Refresh at center | Response | Query far | Refresh far |
| --- | --- | --- | --- | --- | --- |
| 800 | 3.34 ms | 4.33 ms | 31 KB | 2.83 ms | 3.09 ms |
| 1600 | 7.70 ms | 9.81 ms | 63 KB | 6.86 ms | 7.38 ms |
| 2400 | 12.27 ms | 16.59 ms | 94 KB | 11.47 ms | 12.18 ms |
| 3200 | 18.55 ms | 24.84 ms | 125 KB | 16.80 ms | 17.92 ms |
| 4096 | 23.59 ms | 30.70 ms | 160 KB | 21.87 ms | 23.33 ms |

Cost is linear in catalog size, about 7.5 ms per thousand entities at the center and 5.7 ms far away, and most of it is the core's per-entity distance pass rather than response conversion: the far case returns 174 bytes and still costs 21.9 ms at 4096 entities. Every 0.35 s refresh exceeds one 60 Hz frame above roughly 2400 entities at the center and 3000 entities far away. `WORLD_RECORDS` therefore marks where a single catalog already stalls once per refresh, not a comfortable per-realm size. The query walks all entities although the core holds a chunk index; restricting the pass to chunks within the far radius is the obvious follow-up and was not attempted here.

**Unique textured assets.** `make_bundle.py --unique U --lod1-only` derives U byte-distinct GLBs from the detailed sample's lod1 assets (identical geometry and 1024-pixel textures, a tagged `asset.generator`, 6.35-8.94 MiB and 9.68-12.00 MiB decoded each) and places each once. The standard 30-second balanced flight then mounts as many as it reaches.

| Unique assets | Imports in 30 s | Cancelled | Worker P95 | Mount P95 | Steady P95 | Peak / final RSS |
| --- | --- | --- | --- | --- | --- | --- |
| 3 (800 shared instances, reference) | 800 | 0 | - | 12.2 ms | 11.0 ms | 401 / 291 MiB |
| 16 | 16 | 0 | 285 ms | 39.0 ms | 9.6 ms | 625 / 297 MiB |
| 64 | 64 | 0 | 393 ms | 19.7 ms | 10.6 ms | 898 / 352 MiB |
| 128 | 56 | 72 | 1065 ms | 28.3 ms | 14.9 ms | 882 / 414 MiB |

Two walls appear. Resident memory grows by roughly 8-15 MiB per unique textured lod1 asset, so peak RSS reaches 0.9 GiB with about 60 such assets resident, and a 4K-textured full asset costs an order of magnitude more (192-238 MiB decoded each, see the high-detail sample). Import throughput is bounded by two worker tasks at about one second per textured GLB: the 128-asset flight imports fewer than half of them in 30 seconds, the rest stay as boxes and are cancelled when the camera moves on, and worker P95 passes one second. Mount-frame P95 with few mount frames (19 and 63) is dominated by individual stalls and is not comparable with the 800-frame shared-asset runs. The design question M4 raises is density, not totals: how many unique textured assets fall inside the 200 m middle radius at once, which no per-realm constant controls today.

**Storage.** One revision of the three-model detailed sample puts 390 MiB into LFS (sources 119.5 MiB, full assets 247.2 MiB, lod1 23.7 MiB), that is 91-158 MiB per model per revision, against 400 KiB for the whole four-model calibration garden. Because re-exports are not byte-reproducible (Known limits), every rebuild of an unchanged model adds another object: after the rebuilds of this validation session the sample's local `.git/lfs` holds 941 MiB, including four distinct 65.30 MiB objects for one unchanged full asset. Texture bytes dominate; lod1 is 9 percent of the full asset after resampling.

## Hosting

A plain Git host with LFS is sufficient for authoring collaboration; no rou changes were needed. Observed on Forgejo 11.0 with a public repository:

- Anonymous `rou clone` and `rou sparse set` work: history and pointers come over smart HTTP, payloads over the LFS batch API without credentials. Pushing requires a collaborator token; a non-collaborator's push is rejected by the platform.
- Independent contributions on separate branches are mergeable through the platform's pull-request merge; the merge commit validates locally without any rebuild. Pull-request diffs show the multi-line model, entity and build-record JSON line by line, and LFS-tracked files as pointer changes.
- The acceptance run embeds tokens in the remote URL; a credential helper works equally. GitHub was not exercised; its LFS storage and bandwidth quotas are the main difference and are documented by GitHub.

## Known limits

- Re-exporting a frozen source does not guarantee identical GLB bytes or SHA-256. Four exports of the wooden chair with Blender build `9e2066aef7ef` kept the same topology and 1,034,880 full-detail indices, but seam vertex deduplication produced 210,763 / 210,763 / 210,768 / 210,769 vertices. All three embedded images were byte-identical. Before texture resizing, lod1 kept 48,608 vertices and 258,720 indices, with only 79 of 59,662,952 bytes differing, all in UV data. The four simple calibration models were byte-stable over 16 independent acceptance runs. Normal builds reuse unchanged inputs without invoking Blender (observed: three models reused in 1.7 seconds); input changes or exporter-format upgrades trigger re-export and can create new LFS objects. Upstream CI must validate recorded digests and semantics, not compare fresh exports byte-for-byte with published assets. Geometry topology was reproducible in these observations; binary reproducibility is not a contract.
- M3 functional checks pass in the 2026-10-07 closing round. Balanced shared-asset frame targets passed again on 2026-10-07; the current detailed sample passed the named visual harness. The detailed sample exceeded the 33 ms mount target and establishes a baseline for follow-up; it does not establish a general large-asset performance guarantee. The three-cycle shared-asset memory observation showed no per-cycle accumulation, with focus limitations described above.
- Only macOS arm64 has been tested on real hardware. Linux and Windows are unverified; Godot Web export and mobile remain out of scope.
- The safety ceilings bound accepted input; they do not promise that every combination of maximum-sized assets fits available CPU/GPU memory. Many unique assets, larger worlds and long-run memory accumulation require measurement before raising `WORLD_RECORDS` or related scale ceilings.
- LOD reduces geometry and resizes oversized textures while retaining material slots and UV data. Full and lod1 images may coexist during replacement and caching. Texture compression and aggregate resident-memory policy still need separate measurement; the decoded-image safety ceiling is 4 GiB per GLB.
- Godot scene generation remains on the main thread. Headless dummy rendering also requires glTF parsing and runtime LOD generation on that thread; display backends perform preparation on workers. A native call cannot be forcibly cancelled, and exit joins outstanding tasks; permanent native blocking needs a process-level boundary.
- Default-quality steady-frame P95 exceeding 16.7 ms remains follow-up work. Balanced acceptance does not change the default or establish a default-quality pass.
- The earlier Villa Anahid displayed exit warning about leaked textures has not been re-investigated in M3; it remains open independently of these tests.
- The add-on native library is ad-hoc signed. Normal installation works; public quarantined distribution requires Developer ID signing and notarisation.
- Blender Save As copies ownership metadata. Cross-project bindings must be re-established when entity IDs overlap. Identity scans cover all Scenes and linked objects; duplicate entity IDs are rejected.
- M4 needs scale measurements with many unique models, city organisation and evidence before increasing entity/chunk limits. No Realm budget or radius-policy fields were introduced.
- Performance acceptance is measured on mains power. On battery, the same build measured mount-frame P95 3-5 ms higher on this machine, enough to fail the 14.8 ms regression reference; `make bench-viewer` records `power_source` in its receipt.
- S2 public P2P, signatures and nodes are designed but not implemented.
