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
three verified GLBs from the calibration preview (axis probe, house, tree). It
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
