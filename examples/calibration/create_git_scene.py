"""Create a small CC0 authoring tree with original scenes and shared dependencies.

This generates new Git-format inputs directly. It does not convert the earlier
experimental repository or copy its per-entity source libraries.
"""

import argparse
import json
import sys
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from examples.common.blender_primitives import cube, cylinder, material, root


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n")


def shared_files(repository: Path) -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    directory = repository / "sources/shared"
    directory.mkdir(parents=True, exist_ok=True)
    image = bpy.data.images.new("Original checker", width=16, height=16)
    pixels = []
    for y in range(16):
        for x in range(16):
            shade = 0.8 if (x // 4 + y // 4) % 2 else 0.4
            pixels.extend((shade, shade * 0.8, shade * 0.6, 1.0))
    image.pixels = pixels
    image.filepath_raw = str(directory / "checker.png")
    image.file_format = "PNG"
    image.save()
    image.filepath = "//checker.png"
    wood = material("Shared checker material", (0.55, 0.35, 0.2))
    node = wood.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = image
    wood.node_tree.links.new(
        node.outputs["Color"], wood.node_tree.nodes.get("Principled BSDF").inputs["Base Color"]
    )
    bpy.ops.mesh.primitive_cube_add(size=0.5)
    obj = bpy.context.object
    obj.name = "Shared ornament"
    obj.data.name = "SharedOrnamentMesh"
    obj.data.materials.append(wood)
    bpy.ops.wm.save_as_mainfile(filepath=str(directory / "ornament.blend"))


def ornament(repository: Path, parent) -> None:
    path = repository / "sources/shared/ornament.blend"
    with bpy.data.libraries.load(str(path), link=True, relative=True) as (available, requested):
        requested.meshes = ["SharedOrnamentMesh"]
    mesh = requested.meshes[0]
    obj = bpy.data.objects.new("Linked ornament", mesh)
    bpy.context.collection.objects.link(obj)
    obj.parent = parent
    obj.location = (0, 0, 2.0)


def record(repository: Path, name: str, source: str, position: list[int], shared: bool) -> None:
    write_json(
        repository / f"realm/models/{name}.json",
        {
            "format": "rou-model/0.1",
            "id": name,
            "source": source,
            "entry": {"kind": "object", "name": name},
            "dependencies": ["sources/shared/ornament.blend", "sources/shared/checker.png"]
            if shared
            else [],
            "settings": {},
        },
    )
    write_json(
        repository / f"realm/entities/{name}.json",
        {
            "format": "rou-entity/0.1",
            "id": name,
            "model": name,
            "position_mm": position,
            "label": name.replace("_", " ").title(),
        },
    )


def create(repository: Path) -> None:
    if not (repository / "realm.json").is_file():
        raise ValueError("Run rou init in the destination first")
    shared_files(repository)
    scenes = [
        ("sources/houses/courtyard.blend", ["house", "plaque"]),
        ("sources/props/bench.blend", ["bench"]),
        ("sources/nature/tree.blend", ["far_tree"]),
    ]
    positions = {
        "house": [8000, 0, 8000],
        "plaque": [15000, 0, 8000],
        "bench": [40000, 0, 8000],
        "far_tree": [104000, 0, 8000],
    }
    for source, names in scenes:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        stone = material("Limestone", (0.75, 0.68, 0.5))
        green = material("Sage", (0.25, 0.48, 0.3))
        for name in names:
            position = [value / 1000 for value in positions[name]]
            entity = root(name, name, position)
            entity.name = name
            if name == "house":
                cube(entity, "Walls", (0, 2, 0), (8, 4, 6), stone)
                cube(entity, "Roof", (0, 4.2, 0), (9, 0.4, 7), green)
            elif name == "bench":
                cube(entity, "Seat", (0, 0.6, 0), (3, 0.3, 1), stone)
                cube(entity, "Back", (0, 1.3, -0.4), (3, 1.2, 0.2), green)
            elif name == "plaque":
                cube(entity, "Sign", (0, 1, 0), (1.4, 2, 0.3), stone)
            else:
                cylinder(entity, "Trunk", (0, 2, 0), 0.4, 4, stone)
                cube(entity, "Canopy", (0, 4.5, 0), (4, 3, 4), green)
            if name != "far_tree":
                ornament(repository, entity)
            record(repository, name, source, positions[name], name != "far_tree")
        bpy.context.scene["rou_license"] = "CC0-1.0"
        bpy.context.scene["fixture"] = "Original Git realm calibration"
        bpy.context.view_layer.update()
        path = repository / source
        path.parent.mkdir(parents=True, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=str(path), relative_remap=True)
    (repository / "README.md").write_text(
        "# Common garden\n\nOriginal CC0-1.0 calibration scene.\n"
        "house and plaque share an original multi-entity scene.\n"
        "house, plaque and bench use a shared linked mesh and external checker.\n"
    )
    print("GIT_SCENE_CREATED", repository, flush=True)


def edit(repository: Path, name: str) -> None:
    model = json.loads((repository / f"realm/models/{name}.json").read_text())
    path = repository / model["source"]
    bpy.ops.wm.open_mainfile(filepath=str(path))
    entity = bpy.data.objects[model["entry"]["name"]]
    stone = material("Contribution brass", (0.7, 0.44, 0.18))
    cube(entity, "New contribution", (0, 5 if name == "house" else 2.5, 0), (1, 1, 1), stone)
    bpy.context.view_layer.update()
    bpy.ops.wm.save_as_mainfile(filepath=str(path), relative_remap=True)
    print("GIT_SCENE_EDITED", name, flush=True)


def reopen(repository: Path, name: str) -> None:
    model = json.loads((repository / f"realm/models/{name}.json").read_text())
    bpy.ops.wm.open_mainfile(filepath=str(repository / model["source"]))
    assert bpy.data.scenes and bpy.context.scene.objects
    assert bpy.data.objects.get(model["entry"]["name"])
    for library in bpy.data.libraries:
        assert Path(bpy.path.abspath(library.filepath)).is_file(), library.filepath
    for image in bpy.data.images:
        if image.source == "FILE" and not image.packed_file:
            assert Path(bpy.path.abspath(image.filepath, library=image.library)).is_file(), (
                image.filepath
            )
    print("GIT_SCENE_REOPENED", name, flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--edit", choices=["house", "bench"])
    parser.add_argument("--reopen", choices=["house", "bench", "plaque", "far_tree"])
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    repository = args.repo.resolve()
    if args.edit:
        edit(repository, args.edit)
    elif args.reopen:
        reopen(repository, args.reopen)
    else:
        create(repository)


if __name__ == "__main__":
    main()
