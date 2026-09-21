"""Resolve Blender's complete project dependency graph and portable references."""

from pathlib import Path

import bpy

from . import core, scene

closure = scene.dependency_closure


def validate_static_data(roots):
    root = Path(bpy.data.filepath).resolve()
    # Static checks and dependency existence checks share the same traversal as build.
    from . import git_session

    repository = git_session.repository(root)
    return scene.source_dependencies(roots, repository, root.relative_to(repository).as_posix())


def project_files(root, *, make_relative=False):
    root = Path(root).resolve()
    source = Path(bpy.data.filepath).resolve()
    result = set()
    for block, references in bpy.data.file_path_map(include_libraries=True).items():
        if isinstance(block, bpy.types.Image) and block.packed_file:
            continue
        owner = (
            block.parent if isinstance(block, bpy.types.Library) and block.parent else block.library
        )
        for reference in references:
            path = Path(bpy.path.abspath(reference, library=owner)).resolve()
            core.require(path.is_file(), f"Missing authoring dependency: {path}")
            core.require(
                path.is_relative_to(root), f"Move this dependency inside the realm: {path}"
            )
            owner_path = (
                Path(bpy.path.abspath(owner.filepath, library=owner.parent)).resolve()
                if owner
                else source
            )
            if reference.startswith("//"):
                relative = core.call(
                    "blender.relative_path",
                    {"source": owner_path.relative_to(root).as_posix(), "reference": reference},
                )
            elif Path(reference).is_absolute():
                core.require(
                    make_relative and owner is None and hasattr(block, "filepath"),
                    f"Save the linked project with relative paths first: {path}",
                )
                block.filepath = bpy.path.relpath(str(path))
                relative = path.relative_to(root).as_posix()
            else:
                relative = core.call(
                    "blender.relative_path",
                    {"source": owner_path.relative_to(root).as_posix(), "reference": reference},
                )
            core.call("path.safe", relative)
            result.add(relative)
    return sorted(result)
