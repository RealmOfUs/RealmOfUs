"""Persist author entry ownership in the .blend and resolve objects by entity ID."""

import json

import bpy

from . import core, scene

OWNERS_PROPERTY = scene.MODEL_OWNERS_PROPERTY


def saved_owners():
    """Read owner entity IDs from every Scene, including inactive authoring Scenes."""
    owners = scene.saved_model_owners()
    for value in owners:
        core.call("id.validate", value)
    return owners


def remember(owners):
    """Save IDs, never RNA references or names; object duplication cannot copy ownership."""
    encoded = json.dumps(sorted(saved_owners() | set(owners)))
    if bpy.context.scene.get(OWNERS_PROPERTY) != encoded:
        bpy.context.scene[OWNERS_PROPERTY] = encoded


def entity_objects():
    """Index current live objects once, also after undo or duplicate-and-delete."""
    return scene.entity_objects()


def capture(models, entities):
    """Map models to persistent owner entity IDs without conflating their namespaces.

    Existing files bootstrap from the sole entity ID or a named, tagged entry.
    Once saved, ownership never falls back to a name or another shared instance.
    Scene metadata survives object deletion and is saved by source-only saves too.
    """
    saved = saved_owners()
    objects = entity_objects()
    owners = {}
    for model_id, model in models.items():
        if model["entry"]["kind"] != "object":
            continue
        candidates = {key for key, entity in entities.items() if entity["model"] == model_id}
        known = candidates & saved
        core.require(len(known) <= 1, "Multiple author entities own model: " + model_id)
        if known:
            owners[model_id] = next(iter(known))
        elif len(candidates) == 1:
            entity_id = next(iter(candidates))
            owners[model_id] = entity_id
            if entity_id not in objects:
                entry = bpy.data.objects.get(model["entry"]["name"])
                if entry is not None and not entry.get("rou_entity_id"):
                    entry["rou_entity_id"] = entity_id
        elif candidates:
            entry = bpy.data.objects.get(model["entry"]["name"])
            entity_id = entry.get("rou_entity_id") if entry is not None else None
            core.require(
                entity_id in candidates,
                "Cannot identify the author entity for shared model: " + model_id,
            )
            owners[model_id] = entity_id
    if owners:
        remember(owners.values())
    return owners


def roots(owners):
    """Require every unique surviving owner before synchronizing export entry names."""
    objects = entity_objects()
    for entity_id in owners.values():
        core.require(
            entity_id in objects,
            "Source model root missing; restore the entity object: " + entity_id,
        )
    return {model_id: objects[entity_id] for model_id, entity_id in owners.items()}
