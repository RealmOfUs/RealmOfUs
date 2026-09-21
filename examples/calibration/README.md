# Original calibration scenes

`create_scene.py` builds a 50-entity street with roads, houses, trees, a cross-chunk gallery and unequal coloured axes. The original content is declared CC0-1.0 and is used to compare chunk catalogs and to verify size, orientation and bounds through Blender → GLB → Godot.

```bash
blender --background --factory-startup --python-exit-code 1 --python examples/calibration/create_scene.py
```

`make examples` runs this and the Villa Anahid generator. The default output is `build/s1-2.1/`; pass `-- --output build/my-calibration` to write elsewhere. The complete project is `neighborhood.blend`, the preview directory `preview/`, and the same geometry is indexed at 16/32/64 m chunks. The native library must exist (`make ffi`).

`create_git_scene.py` generates the small Git realm used by the [tutorial](../../doc/tutorial.md) and the acceptance harness: four entities (house, plaque, bench, far_tree) with a shared linked library and an external texture, written directly into an initialised `rou` repository:

```bash
blender --background --factory-startup --python-exit-code 1 \
  --python examples/calibration/create_git_scene.py -- --repo /path/to/realm
```

`--edit house|bench` applies a scripted modelling change to the saved source, standing in for an interactive Blender session; `--reopen <entity>` opens the source and verifies that its dependencies are present.

Standalone statistics use the reviewed metadata in `toolchain/fixtures/calibration.json`; regenerating the Blender scene produces new source files and hashes and does not replace the reviewed inputs.
