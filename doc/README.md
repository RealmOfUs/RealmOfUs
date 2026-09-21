# Realm of Us · Documentation

Realm of Us is Git for a multi-user, distributed, open 3D world. Native Git and Git LFS own the complete Blender projects and the sole history; `rou` adds spatial selection, builds, validation and fixed-revision browse bundles; a Blender add-on and a Godot viewer use the same Rust core.

| Document | What it covers |
| --- | --- |
| [tutorial.md](tutorial.md) | Hands-on walkthrough of the core features with the calibration sample: create a realm, build, commit, clone, select a region, collaborate, merge, view |
| [design.md](design.md) | Product and system design: goals, world and space, repository layout, commands, authoring and build, collaboration, viewing, S2 boundaries |
| [architecture.md](architecture.md) | Code architecture: crates and hosts, single authority, Git/LFS and sparse selection, build and integrity, merge and push, Blender and viewer, operation table and FFI |
| [protocol/](protocol/README.md) | Data contracts: [repository.md](protocol/repository.md) for the realm repository and [viewer-bundle.md](protocol/viewer-bundle.md) for the spatial profile and browse bundle |
| [status.md](status.md) | What is verified, how, and the known limits |

Code-level guides live next to the code: [rou/README.md](../rou/README.md) for the Rust workspace and CLI help, [toolchain/README.md](../toolchain/README.md) for the Blender add-on, acceptance harnesses and packaging, [examples/README.md](../examples/README.md) for the samples.
