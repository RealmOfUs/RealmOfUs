# Tutorial: a realm from scratch to a two-author merge

This walkthrough exercises every core feature with the CC0 calibration sample: creating a realm, building and committing, exporting a browse bundle, cloning with a region selection, two authors working in parallel, merging without rebuilding, and inspecting the result. Every command below was run as written on the current tree; the scripted "edit" steps stand in for interactive Blender sessions, which are covered at the end.

## 0. Prerequisites

Git, Git LFS, Blender 4.2+ and Godot 4.7 on your PATH, plus the CLI and the viewer extension built through the Makefile:

```bash
cd /path/to/RealmOfUs
make rou godot-ext                    # build/bin/rou and client/app/bin/librou_godot.*
export ROU_TOOLCHAIN_ROOT="$PWD"      # lets rou find the Blender export helper
export PATH="$PWD/build/bin:$PATH"
mkdir -p /tmp/realm-tutorial && cd /tmp/realm-tutorial
```

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
realm/entities/bench.json      realm/models/bench.json      sources/houses/courtyard.blend
realm/entities/far_tree.json   realm/models/far_tree.json   sources/nature/tree.blend
realm/entities/house.json      realm/models/house.json      sources/props/bench.blend
realm/entities/plaque.json     realm/models/plaque.json     sources/shared/checker.png
                                                            sources/shared/ornament.blend
```

`realm/models/house.json` says which `.blend` and which object to export and what it depends on; `realm/entities/house.json` says where the model stands, in integer millimetres. `house` and `plaque` live in the same `courtyard.blend`; all three props link `ornament.blend`.

## 2. Build, stage, commit

`rou build` freezes the saved sources, runs headless Blender once per source file, and writes `runtime/<model>.glb` plus a build record `runtime/<model>.json`. It never stages anything.

```bash
rou build          # "generated": ["bench", "far_tree", "house", "plaque"]
rou status         # everything still untracked
rou add .
rou commit -m "Create the shared garden"
```

`rou commit` validates the index (shapes, references, build freshness, LFS pointers, spatial closure), creates the native commit, then validates the actual commit and prints its OID. A README-only commit later never starts Blender.

## 3. Export a browse bundle and look at it

```bash
rou view HEAD --output .rou/view/first
ls .rou/view/first        # objects  realm.json  version.json
godot --path "$ROU_TOOLCHAIN_ROOT/client/app" -- --realm="$PWD/.rou/view/first/realm.json"
```

`realm.json` is the runtime catalog (profile, entities with world bounds, complete chunk index); `version.json` binds it to the commit; `objects/` holds the GLBs named by SHA-256. The viewer starts above the centre of the world; pick a place from the chunk map, the entity list or typed coordinates, or pass `--at=12,3,5`.

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

`push` validates the exact commit it publishes and pushes the LFS objects reachable from it.

## 5. Alice clones and selects one chunk

A clone downloads history and LFS pointers only. The selection decides which sources are materialised.

```bash
rou clone remote.git alice
cd alice
rou config user.name Alice && rou config user.email alice@example.invalid
rou switch -c feature/house
rou sparse set --chunk 0,0,0
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
rou build                           # "generated": ["house", "plaque"]  (same source file)
rou add sources/houses runtime
rou commit -m "Add the courtyard ornament"
rou push -u origin feature/house
cd ..
```

Only the two models that depend on the changed file were rebuilt; `bench` and `far_tree` kept their cold pointers and were neither downloaded nor rebuilt.

## 6. Bob clones and selects one entity

```bash
rou clone remote.git bob
cd bob
rou config user.name Bob && rou config user.email bob@example.invalid
rou switch -c feature/bench
rou sparse set --entity bench       # paths: props/bench.blend, shared/checker.png, shared/ornament.blend
blender --background --factory-startup --python-exit-code 1 \
  --python "$ROU_TOOLCHAIN_ROOT/examples/calibration/create_git_scene.py" -- --repo "$PWD" --edit bench
rou build                           # "generated": ["bench"]
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

```json
{
  "changed_entities": [
    { "id": "bench", "changes": ["geometry"], "before": {...}, "after": {...} },
    { "id": "house", "changes": ["geometry"], ... },
    { "id": "plaque", "changes": ["geometry"], ... }
  ],
  "added_entities": [], "removed_entities": [], ...
}
```

Then view the merged revision:

```bash
rou view HEAD --output .rou/view/merged
godot --path "$ROU_TOOLCHAIN_ROOT/client/app" -- --realm="$PWD/.rou/view/merged/realm.json"
```

If two authors had edited the same `.blend`, `merge` would leave Git's base/ours/theirs for that file; you resolve it in Blender, run `rou build`, add the result and commit. Rou never merges geometry itself.

## 8. Grow or shrink the selection

```bash
rou sparse set --all                        # every current entity and all their sources
rou sparse set --chunk '1,0,1..-1,0,-1'     # inclusive range, reversed corners allowed
rou sparse add --entity far_tree            # extend instead of replace
rou sparse list
```

Shrinking never deletes history. Local edits in directories that would leave the checkout are refused; ignored backups such as `*.blend1` are moved under `.rou/backups/` first.

## 9. Working interactively in Blender

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
