# Tutorial: a realm from scratch to a two-author merge

This walkthrough exercises the core authoring workflow with the CC0 calibration sample: creating a realm, building and committing, exporting a browse bundle, cloning with a region selection, two authors working in parallel, merging without rebuilding, and inspecting the result. Validation coverage and replay dates are recorded in [Current status](status.md); the scripted "edit" steps stand in for interactive Blender sessions, which are covered at the end.

## 0. Prerequisites

Authoring needs Git, Git LFS and Blender 4.2+ on your PATH. The viewer is a separate precompiled application: download the release for your platform and open any browse bundle. No source checkout or Godot installation is needed to use that app.

This source-based walkthrough also uses Godot 4.7 on your PATH and builds through the Makefile:

```bash
cd /path/to/RealmOfUs
make rou godot-ext                    # build/bin/rou and client/app/bin/librou_godot.*
export ROU_TOOLCHAIN_ROOT="$PWD"      # lets rou find the Blender export helper
export PATH="$PWD/build/bin:$PATH"
mkdir -p /tmp/realm-tutorial && cd /tmp/realm-tutorial
```

`make godot-ext` produces the viewer's internal native library for validation and coordinate conversion. It is loaded by the viewer, not a command-line tool that users operate. To export the independent application yourself, run `make viewer`; the app is written under `dist/viewer/` and embeds no world.

## 1. Create a realm

`rou init` creates a standard Git repository with LFS configured, a `.gitignore`, the fixed `realm.json` and the empty `realm/` and `runtime/` directories. Nothing is committed yet.

```bash
mkdir garden && cd garden
rou init
rou config user.name "Maintainer"
rou config user.email "maintainer@example.invalid"
```

Instead of modelling by hand, generate the sample: four entities (`house`, `plaque`, `bench`, `far_tree`) whose sources share a linked library and an external texture.

```bash
blender --background --factory-startup --python-exit-code 1 \
  --python "$ROU_TOOLCHAIN_ROOT/examples/calibration/create_git_scene.py" -- --repo "$PWD"
find realm sources -type f | sort
```

```text
realm/entities/bench.json
realm/entities/far_tree.json
realm/entities/house.json
realm/entities/plaque.json
realm/models/bench.json
realm/models/far_tree.json
realm/models/house.json
realm/models/plaque.json
sources/houses/courtyard.blend
sources/nature/tree.blend
sources/props/bench.blend
sources/shared/checker.png
sources/shared/ornament.blend
```

`realm/models/house.json` says which `.blend` and which object to export and what it depends on; `realm/entities/house.json` says where the model stands, in integer millimetres. `house` and `plaque` live in the same `courtyard.blend`; all three props link `ornament.blend`.

## 2. Build, stage, commit

`rou build` freezes the saved sources, runs headless Blender once per source file, and writes `runtime/<model>.glb`, `runtime/<model>.lod1.glb` and the build record `runtime/<model>.json`. It never stages anything.

```bash
rou build          # Built 4 models (bench, far_tree, house, plaque); reused 0; outputs written to runtime/, nothing staged
# Example statistics for house; the build prints both tiers for all four models:
#   house asset: 5620 bytes, 72 vertices, 108 indices, 4 nodes, max image 16 px, decoded 1024 bytes
#   house lod1: 5620 bytes, 72 vertices, 108 indices, 4 nodes, max image 16 px, decoded 1024 bytes
rou status         # everything still untracked
rou add .
rou commit -m "Create the shared garden"
```

`rou commit` validates the index (shapes, references, build freshness, LFS pointers, spatial closure), creates the native commit, then validates the actual commit. Its default output starts with Git's commit output, followed by `Realm: 4 entities, 4 models validated`. All built-ins default to readable summaries; add `--json` before or after a built-in command to get the machine report. Native Git commands retain Git's output and exit status. A README-only commit later never starts Blender.

## 3. Export a browse bundle and look at it

```bash
rou sparse set --all
rou view HEAD --output .rou/view/first
ls .rou/view/first        # objects  realm.json  version.json
godot --path "$ROU_TOOLCHAIN_ROOT/client/app" -- --realm="$PWD/.rou/view/first/realm.json"
```

```text
Bundle: /private/tmp/realm-tutorial/garden/.rou/view/first; 4 entities in catalog, 4 models included, 0 missing; commit 33047f0ec927954103dfab4dbc4c0d57a397dde4
```

`rou view HEAD` includes only locally available models in the saved sparse selection, without downloading anything. The explicit selection above includes the whole garden. With no selection, the bundle contains only its catalog. Use `rou view HEAD --all` to fetch every model directly, or `--chunk`/`--entity` to fetch an explicit subset. The same rules apply to `--worktree`.

A downloaded viewer app opens the **Open an exported world** chooser; select the bundle directory or its `realm.json`. The `godot` command above runs the same viewer from the source tree. Missing model payloads stay listed as **Asset missing**, never as an empty chunk.

`realm.json` is the small runtime root (profile, entity count and chunk-page descriptors); `version.json` binds it to the commit. `chunks/` holds root-bound entity pages and `objects/` holds GLBs and shared PNG/JPEG images named by SHA-256. Local GLBs are hard-linked to materialized runtime files when possible, with a copy fallback. Treat bundle assets as read-only. The viewer starts above the centre of the world; pick a place from the chunk map, the entity list or typed coordinates, or pass `--at=12,3,5`.

## 4. Publish to a remote

Any Git remote with LFS works. For the tutorial, a local bare repository:

```bash
cd ..
git init --bare --initial-branch=main remote.git
cd garden
rou remote add origin ../remote.git
rou push -u origin main
cd ..
```

`push` validates the exact commit it publishes and pushes the LFS objects reachable from it. It may restore missing historical payloads needed for publication, without widening the sparse checkout.

### Using a hosted platform instead

Any Git host with LFS works the same way. The repository ships a local Forgejo for this: `make forgejo-up` starts it in Docker (OrbStack on macOS), creates the users `realm`, `alice`, `bob` and the public repository `realm/garden`, and writes API tokens to `.cache/forgejo/credentials.json`. Replace the remote with the HTTP URL, embedding the token for authenticated users:

```bash
rou remote add origin http://realm:<token>@localhost:3000/realm/garden.git
rou push -u origin main
rou clone http://localhost:3000/realm/garden.git reader     # anonymous: history and pointers only
```

Anonymous clones of a public repository can select regions and download LFS payloads without an account; pushing needs write access on the platform. Pull requests opened on the platform for independent contributions are mergeable there without any local rebuild, because no shared index file is committed. `make test-forgejo` runs this whole flow, including two platform-side merges and a rejected push from a non-collaborator.

## 5. Alice clones and selects one chunk

A clone downloads history and LFS pointers only. The selection decides which sources are materialised.

```bash
rou clone remote.git alice
cd alice
rou config user.name Alice && rou config user.email alice@example.invalid
rou switch -c feature/house
rou sparse set --chunk 0,0,0
```

```text
Selected 2 entities (house, plaque): 3 source files, 2 runtime assets in sources/houses, sources/shared; materialized 5 payloads, 0 already present
```

The selection resolves to the entities intersecting chunk `0,0,0` (`house` and `plaque`), their source `courtyard.blend`, and the shared library and texture. Only those payloads are downloaded:

```bash
ls sources                          # houses  shared      (no props, no nature)
wc -c runtime/bench.glb             # 129 bytes: still an LFS pointer
```

Alice now edits `sources/houses/courtyard.blend` in Blender and saves it. The tutorial applies a scripted change instead:

```bash
blender --background --factory-startup --python-exit-code 1 \
  --python "$ROU_TOOLCHAIN_ROOT/examples/calibration/create_git_scene.py" -- --repo "$PWD" --edit house
rou status                          # M sources/houses/courtyard.blend
rou build                           # Built 2 models (house, plaque); reused 2; outputs written to runtime/, nothing staged
rou add sources/houses runtime
rou commit -m "Add the courtyard ornament"
rou push -u origin feature/house
cd ..
```

During the build, only the two models that depend on the changed file were rebuilt; `bench` and `far_tree` kept their cold pointers and were neither downloaded nor rebuilt by `build`.

## 6. Bob clones and selects one entity

```bash
rou clone remote.git bob
cd bob
rou config user.name Bob && rou config user.email bob@example.invalid
rou switch -c feature/bench
rou sparse set --entity bench       # paths: props/bench.blend, shared/checker.png, shared/ornament.blend
blender --background --factory-startup --python-exit-code 1 \
  --python "$ROU_TOOLCHAIN_ROOT/examples/calibration/create_git_scene.py" -- --repo "$PWD" --edit bench
rou build                           # Built 1 model (bench); reused 3; outputs written to runtime/, nothing staged
rou add sources/props runtime
rou commit -m "Extend the bench"
rou push -u origin feature/bench
cd ..
```

## 7. Alice merges Bob's work without rebuilding

Because every model and entity is its own file and no shared index is committed, independent contributions merge with plain Git:

```bash
cd alice
rou fetch origin
rou merge origin/feature/bench      # stays pending for inspection; no conflicts here
rou commit -m "Merge the bench work"
```

`commit` validates the merged index: both contributions, no duplicate IDs, every build record fresh against its inputs. No `rou build` was needed. Inspect what changed in domain terms rather than file terms:

```bash
rou diff --realm main HEAD
```

```text
Entities:
  bench: geometry
  house: geometry
  plaque: geometry
Models:
  bench: modified
  house: modified
  plaque: modified
```

The text report starts with the resolved comparison revisions, followed by the entity and model groups above. Use `rou diff --realm main HEAD --json` for the full machine report.

Then export the merged revision. Alice's saved selection still contains only house and plaque:

```bash
rou view HEAD --output .rou/view/merged
godot --path "$ROU_TOOLCHAIN_ROOT/client/app" -- --realm="$PWD/.rou/view/merged/realm.json"
```

```text
Bundle: /private/tmp/realm-tutorial/alice/.rou/view/merged; 4 entities in catalog, 2 models included, 2 missing; commit 4cdd4ec7139e18248d805daf65c13a1ee0d00e9e
```

The catalog still lists all four entities, but bench and far_tree are marked **Asset missing**. To include every model, run `rou view HEAD --all --output .rou/view/merged`; this explicitly permits runtime downloads.

If two authors had edited the same `.blend`, `merge` would leave Git's base/ours/theirs for that file; you resolve it in Blender, run `rou build`, add the result and commit. Rou never merges geometry itself.

## 8. Grow or shrink the selection

```bash
rou sparse set --all                        # every current entity and all their sources
rou sparse set --chunk '1,0,1..-1,0,-1'     # inclusive range, reversed corners allowed
rou sparse add --entity far_tree            # extend instead of replace
rou sparse list
```

```text
All: false
Chunks: 1,0,1..-1,0,-1
Entities: far_tree
```

Shrinking never deletes history. Local edits in directories that would leave the checkout are refused; ignored backups such as `*.blend1` are moved under `.rou/backups/` first.

## 9. Working interactively in Blender

Entity placement is currently edited with the Blender add-on's **Write placement changes** action or by directly editing `realm/entities/<id>.json`. The Godot viewer is read-only; a Godot Editor placement plugin is a future extension.

The scripted `--edit` steps above are what a real session does through the add-on:

1. Package and install it once: `make addon`, then Preferences → Add-ons → Install from Disk with `dist/rou_blender.zip`. Reopen the author project afterwards; the Realm tab appears in the 3D viewport's N panel.
2. In a clone with the relevant sources selected, open the `.blend` from `sources/`. The panel shows the session: revision, source and its models and entities.
3. Edit geometry inside an entity root, then use **Save complete source**. Move an entity root and use **Write placement changes** to update `realm/entities/<id>.json`; the JSON is the authority, a plain Ctrl+S never changes placement.
4. To add an entity, create a top-level object, use **Mark selected objects as entities**, then **Register marked entities** and save. Registration writes the model and entity JSON and records which object owns the model, so renaming the root later is handled by registering again before saving.
5. Back in the terminal: `rou build`, `rou add`, `rou commit`, `rou push`, exactly as above.

After `commit`, `switch` or `merge` the session is stale; reopen the project. Details and the acceptance harnesses are in the [toolchain guide](../toolchain/README.md).

## 10. Clean up

```bash
rm -rf /tmp/realm-tutorial
```

Build output also prints a statistics line for each generated full and lod1 GLB;
exact counts depend on the Blender version and source. A `warning:` line marks
an advisory reference exceedance, not a failed build. For machine-readable
measurements use `rou --json build` and its per-model `metrics` and `warnings`.
Each model now produces `runtime/<model>.glb`, `runtime/<model>.lod1.glb` and
`runtime/<model>.json`, plus shared `runtime/<sha256>.png|jpg` images. Stage the GLBs, images and record together. Existing 0.1 and 0.2 build records and browse catalogs need regeneration with `rou build` and `rou view`.
