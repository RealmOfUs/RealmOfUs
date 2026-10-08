# Synthetic viewer scale sample

`make bench-viewer` builds the native adapters, generates the calibration source if
needed, creates `build/scale/bundle-800/`, and runs a 30-second displayed streaming
flight. Measurements, including RSS samples and separate mount/steady percentiles,
are written to `/tmp/rou-m2-viewer.json`. Check `foreground_measurement_valid` before
using the numbers as foreground acceptance. On macOS the target requests activation
of the measured process; the desktop session still has to permit foreground focus.

To generate only the preview, after `make ffi` and the calibration export:

```sh
python3 examples/scale/make_bundle.py --count 800 --seed 20260922
```

The generator uses Python and the core's `preview.create` and `viewer.catalog`
operations. It places 800 entities across the default world's X/Z chunks, sharing
three verified GLBs from the calibration preview (axis probe, house, tree). Each
entity carries both full and lod1 descriptors; these small meshes have identical
bytes in both roles, so the three assets remain shared. It
creates a complete catalog and no `version.json`; no Blender invocation is needed
by the generator itself. Positions are deterministic but can overlap. This sample
measures repeated instances and streaming, not a city with 800 unique assets.

Use `--output` for a separate destination and `--source` to select another export
of the same calibration scene. An existing output is reused only if its validated
manifest equals the requested synthetic preview and it has no version sidecar.

The development flight starts in the metadata overview, crosses the world, then
moves beyond the middle radius for the last third of the run. To measure the
existing balanced rendering option (SDFGI disabled):

```sh
python3 toolchain/build/benchmark_viewer.py \
  --realm build/scale/bundle-800/realm.json --seconds 30 --flight \
  --balanced --activate --report /tmp/rou-m2-balanced.json --json
```

Omit `--activate` on platforms other than macOS. Neither the seed nor the radii
change the browse-bundle format; radius defaults remain viewer settings.

## Detailed furniture sample

After `make all`, generate a separate Git realm with both detail assets:

```sh
python3 examples/scale/make_detailed_realm.py
python3 toolchain/build/benchmark_viewer.py \
  --realm build/scale/detailed/.rou/view/worktree/realm.json \
  --seconds 30 --flight --balanced --activate \
  --report /tmp/rou-m3-detailed.json --json
```

The output directory must be empty; use `--output` for another location. The script
downloads the CC0 Poly Haven `ArmChair_01`, `WoodenChair_01` and `wool_boucle` 4K
assets, verifies their provider checksums and records source URLs and SHA-256 hashes.
Downloads are cached under `.cache/walsh-offline/`. Missing downloads fail the run.
The provider's 4K wool images are rectangular (4096 by 5076 pixels); the sample
preserves them in full detail and emits the corresponding image-dimension reference
warning. LOD images are resized proportionally to a maximum 1024-pixel edge.

The two scanned furniture models use SIMPLE subdivision to increase tessellation
without claiming additional scanned detail. A third, original woven panel has
procedural geometry and the wool textures. The generator saves packed Blender
sources, runs `rou build`, `rou add .`, `rou validate --staged` and
`rou view --worktree --all`, checks the browse bundle, verifies source files stayed
unchanged and requires substantial lod1 triangle and texture reduction. It creates no commit.
Commands and metrics are retained under the generated realm's `.rou/` directory,
including `detailed-report.json`.

LOD decimates geometry and resamples oversized textures on copies. This three-model
sample measures large individual assets and tier replacement; the 800-entity
sample measures shared instances. Neither establishes a city-scale budget.

## Scale bundles

`make_bundle.py --count N` places N instances of the three calibration models and is what
`make bench-viewer` uses (N = 800). Two options build the M4 scale bundles:

- `--unique U` derives U byte-distinct GLB variants from the source models by tagging
  `asset.generator`; geometry and textures are unchanged, so streaming sees U different SHAs.
- `--lod1-only` uses each source lod1 for both tiers, which keeps textured bundles small.

```bash
python3 examples/scale/make_bundle.py --count 4096 --output build/scale/bundle-4096
python3 examples/scale/make_bundle.py --source build/scale/detailed/.rou/view/worktree/realm.json \
  --lod1-only --unique 64 --count 64 --output build/scale/textured-64
godot --headless --path client/app --script ../tests/refresh_cost.gd -- \
  --realm="$PWD/build/scale/bundle-4096/realm.json" --rounds=30
```

The textured source is the detailed sample (`make_detailed_realm.py`); the refresh probe prints
one `REFRESH_COST` JSON line with the headless `refresh_assets()` cost. Results are recorded in
`doc/status.md` under Scale measurements.

