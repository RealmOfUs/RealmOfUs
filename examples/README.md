# Reproducible samples

| Directory | Contents | Role |
| --- | --- | --- |
| `common/` | Blender geometry helpers shared by the samples | Internal reuse, not part of the world protocol |
| [calibration/](calibration/README.md) | Original CC0 street scene, coordinate probes, and the small Git realm used by the tutorial and acceptance harness | Measurement source and integration sample |
| [villa_anahid/](villa_anahid/README.md) | Villa Anahid house and the offline living-room study | Standalone showcase |

Directories use underscores so they can be imported as Python packages. The framework add-on can be used by any author; samples only call its public export interface, and add-on tests do not depend on a specific house.

Modelling scripts and the necessary original textures are reproducible source. Generated `.blend`, GLB, apps and video go to the root `build/` and `dist/`; downloaded material goes to `.cache/`. Each sample declares its own provenance and content licence.
