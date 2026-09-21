# Villa Anahid showcase

An approximate reconstruction based on public listing material for 349 Walsh Road. It exercises the modelling and viewing flow; dimensions, wall lines and furniture are not survey results. House content keeps the `NOASSERTION` licence; reference photographs and the architectural PDF never enter the delivered app; the generic sky and downloaded material keep their own provenance notes.

## Modelling modules

| Module | Responsibility |
| --- | --- |
| `architecture.py` | Building massing, base modelling and assembly; keeps the first reconstruction entry point |
| `appearance.py` | Materials, vegetation and the second visual style; keeps the second entry point |
| `furniture.py` | Furniture geometry, orientation and audit data |
| `create_scene.py` | Current third-version floor layout, fixtures, furniture configuration and scene entry point |
| `verify.py` | Checks beds, kitchen, sofa orientation and asset budgets against the actual meshes and room audit |
| `offline/` | Standalone Cycles living-room modelling, asset fetching, rendering, encoding and background monitoring |

These are procedural scene-building modules for a single Blender process; the version styles share base functions, and the current entry point does not import historical `v2`/`v3` scripts.

## House and viewer

```bash
blender --background --factory-startup --python-exit-code 1 --python examples/villa_anahid/create_scene.py
python examples/villa_anahid/verify.py
python toolchain/build/verify_s1.py --realm build/walsh-v3-2.1/preview/realm.json --viewer-realm build/walsh-v3-2.1/preview/realm.json
godot --path client/app -- --realm="$PWD/build/walsh-v3-2.1/preview/realm.json" \
  --sample=villa-anahid --sample-data="$PWD/build/walsh-v3-2.1/sample.json" --station=0
```

Modelling accepts `-- --output build/my-villa` and verification `--scene build/my-villa`. The default output is `build/walsh-v3-2.1/`; `make examples` runs both generators. `make ffi` and `make godot-ext` provide the native libraries; desktop export is `make viewer REALM=build/walsh-v3-2.1/preview/realm.json`.

The sample's `sample.json` sits next to the source project and holds room descriptions and tour waypoints; it is not part of the `preview/` bundle. The generic viewer keeps its bounds-based start; it only jumps to sample waypoints when they are selected explicitly.

## Offline living room

```bash
python examples/villa_anahid/offline/fetch_assets.py
blender --background --factory-startup --python-exit-code 1 --python examples/villa_anahid/offline/create_scene.py
blender --background build/walsh-offline/living-room-cycles.blend --python-exit-code 1 --python examples/villa_anahid/offline/render.py -- --frames 1-288 --stride 2 --width 1920 --samples 64 --threshold .035 --label frames
python examples/villa_anahid/offline/watch.py --renderer-pid PROCESS_ID
```

The last command applies when the render already runs in the background and needs its real PID; the monitor then renders a 4K still, encodes and checks the video. `package.py` can also encode an existing complete sequence on its own. Default products stay under `build/walsh-offline/` and `dist/walsh-offline/`.

`offline/create_scene.py -- --output build/my-offline --preview 1 --width 640 --samples 8` gives an isolated quick check. Original stone textures, generation prompts and provenance are kept in [offline/assets/](offline/assets/README.md); the film is rendered by Blender. `offline/render.py` accepts `--output-dir` and `offline/package.py` accepts `--build-dir` and `--output-dir` for isolated checks that do not overwrite existing deliveries.
