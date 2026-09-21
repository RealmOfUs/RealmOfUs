"""Villa Anahid v3: floor-program correction and original detailed interiors.

The listing establishes room counts and floors, not a measured interior plan.
Our room positions/dimensions remain an explicitly approximate reconstruction.
Run: blender --background --python examples/villa_anahid/create_scene.py
"""

import argparse
import json
import math
import sys
from pathlib import Path

import bpy
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.common.blender_primitives import location, root
from examples.villa_anahid import appearance as v
from examples.villa_anahid import architecture as h
from examples.villa_anahid import furniture as f

ROOMS = []
ENTITIES = []
DOORS = []
BEDS = []
SOURCE_AUDIT = {}


def entity(key, label):
    o = root("walsh_" + key, label, (0, 0, 0))
    ENTITIES.append(o)
    return o


def room(key, name, floor, rect, kind="living", station=None):
    r = {"id": key, "name": name, "floor": floor, "rect": rect, "kind": kind}
    if station is not None:
        r["station"] = station
    ROOMS.append(r)


def slab(o, name, x0, x1, z0, z1, y, key="stone", depth=0.22):
    return h.box(
        o, name, ((x0 + x1) / 2, y - depth / 2, (z0 + z1) / 2), (x1 - x0, depth, z1 - z0), key
    )


def wall(o, x0, z0, x1, z1, y, height=3.6, key="ivory", thickness=0.16):
    return h.box(
        o,
        "Interior wall",
        ((x0 + x1) / 2, y + height / 2, (z0 + z1) / 2),
        (max(thickness, abs(x1 - x0)), height, max(thickness, abs(z1 - z0))),
        key,
    )


def doorwall(o, x0, x1, z, y, door_x, width=1.05, height=3.0):
    # A real clear opening with header. Door leaf held open against its jamb.
    wall(o, x0, z, door_x - width / 2, z, y, height)
    wall(o, door_x + width / 2, z, x1, z, y, height)
    h.box(
        o,
        "Door lintel",
        (door_x, y + (height + 2.32) / 2, z),
        (width, height - 2.32, 0.16),
        "ivory",
    )
    for dx in [-width / 2, width / 2]:
        h.box(o, "Oak door lining", (door_x + dx, y + 1.16, z), (0.035, 2.32, 0.18), "oak")
    h.box(
        o,
        "Open oak door",
        (door_x - width / 2 + 0.02, y + 1.14, z + 0.50),
        (0.045, 2.28, width - 0.06),
        "oak",
        0.008,
    )
    h.box(
        o,
        "Door lever",
        (door_x - width / 2 + 0.066, y + 1.04, z + 0.86),
        (0.07, 0.026, 0.13),
        "bronze",
        0.008,
    )
    DOORS.append({"center": [door_x, y, z], "clear_width_m": width, "height_m": 2.32})


def ceiling(o, x0, x1, z0, z1, y, height=3.8, woodfloor=True):
    slab(o, "Ceiling", x0, x1, z0, z1, y + height + 0.18, "ivory", 0.18)
    # Fine linear lighting tracks, paired downlights and concealed perimeter coves.
    for z in [z0 + 0.23, z1 - 0.23]:
        h.box(
            o,
            "Cove shadow line",
            ((x0 + x1) / 2, y + height - 0.025, z),
            (x1 - x0 - 0.35, 0.035, 0.09),
            "seam",
        )
        h.box(
            o,
            "Concealed cove",
            ((x0 + x1) / 2, y + height - 0.035, z),
            (x1 - x0 - 0.45, 0.014, 0.025),
            "light",
        )
    for x in np.arange(x0 + 1.3, x1 - 0.5, 3.5):
        for z in np.arange(z0 + 1.4, z1 - 0.7, 3.5):
            h.box(
                o,
                "Recessed lighting track",
                (float(x), y + height - 0.006, float(z)),
                (0.07, 0.016, 1.65),
                "dark",
            )
            for dz in [-0.60, 0.60]:
                v.cylinder(
                    o,
                    "Warm track optic",
                    (float(x), y + height - 0.021, float(z) + dz),
                    0.025,
                    0.01,
                    h.M["light"],
                    12,
                )
    if woodfloor:
        for x in np.arange(x0 + 0.23, x1, 0.23):
            h.box(
                o,
                "Oak floor plank joint",
                (float(x), y + 0.002, (z0 + z1) / 2),
                (0.0008, 0.001, z1 - z0),
                "oak",
            )


def fireplace(o, x, y, z, w=3.4, height=3.8, key="onyx"):
    # Black firebox is a genuine recess assembled from surrounding stone pieces.
    slab_y = 0.82
    opening = 0.42
    h.box(o, "Fireplace lower slab", (x, y + slab_y / 2, z), (w, slab_y, 0.35), key)
    h.box(
        o,
        "Fireplace upper slab",
        (x, y + (slab_y + opening + height) / 2, z),
        (w, height - slab_y - opening, 0.35),
        key,
    )
    for dx in [-1, 1]:
        h.box(
            o,
            "Fireplace jamb",
            (x + dx * (w / 2 - 0.20), y + slab_y + opening / 2, z),
            (0.40, opening, 0.35),
            key,
        )
    h.box(
        o,
        "Blackened firebox",
        (x, y + slab_y + opening / 2, z - 0.055),
        (w - 0.76, opening, 0.20),
        "ebony",
    )
    vs = []
    fs = []
    for i in range(120):
        xx = x - w / 2 + 0.45 + i * (w - 0.9) / 119
        hh = 0.020 + 0.045 * (0.5 + 0.5 * math.sin(i * 2.7)) + 0.016 * math.sin(i * 0.75) ** 2
        vs.extend([(xx, y + slab_y + 0.005, z + 0.14), (xx, y + slab_y + hh, z + 0.14)])
        if i:
            fs.append((2 * i - 2, 2 * i, 2 * i + 1, 2 * i - 1))
    f.mesh(o, "Irregular gas flame ribbon", vs, fs, "light", False)


def shelving(o, x, y, z, w=3.0, hh=3.2):
    h.box(o, "Oak display backing", (x, y + hh / 2, z), (w, hh, 0.08), "oak")
    for dx in [-w / 2, w / 2]:
        h.box(o, "Cabinet stile", (x + dx, y + hh / 2, z + 0.20), (0.06, hh, 0.47), "oak")
    for i in range(5):
        yy = y + 0.48 + i * 0.58
        h.box(o, "Floating oak shelf", (x, yy, z + 0.20), (w, 0.045, 0.47), "oak", 0.008)
        h.box(o, "Shelf light", (x, yy - 0.025, z + 0.10), (w - 0.1, 0.012, 0.018), "light")
        if i % 2:
            f.vase(o, x + 0.35, yy + 0.025, z + 0.22, 0.50)
        else:
            f.books(o, x - 0.25, yy + 0.06, z + 0.24)


def bathroom(o, x, y, z, w=3.2, primary=False):
    g = f.group(o, "Primary bathroom" if primary else "Shower bathroom", (x, y, z))
    h.box(g, "Vanity cabinet", (0, 0.57, -0.85), (w - 0.3, 0.50, 0.58), "oak", 0.012)
    h.box(
        g,
        "Vanity marble",
        (0, 0.845, -0.85),
        (w - 0.2, 0.05, 0.65),
        "onyx" if primary else "marble",
        0.012,
    )
    for xx in [-0.72, 0.72] if primary else [0]:
        f.soft(g, "Vessel basin", (xx, 0.93, -0.85), (0.50, 0.14, 0.34), "ceramic", 0.55)
        f.faucet(g, xx - 0.10, 0.88, -1.02)
        h.box(g, "Mirror bronze surround", (xx, 1.75, -1.145), (0.68, 1.03, 0.03), "bronze", 0.07)
        h.box(g, "Mirror", (xx, 1.75, -1.12), (0.62, 0.97, 0.025), "steel", 0.06)
        h.box(g, "Mirror side light", (xx - 0.34, 1.75, -1.10), (0.018, 0.90, 0.016), "light")
    # Shower enclosure at the left end, rainfall fitting and bench.
    h.glass(g, -w / 2 + 0.25, 0, 0.30, 1.8, 2.4, True)
    h.box(g, "Shower tray", (-w / 2 + 0.82, 0.035, 0.35), (1.18, 0.07, 1.85), "marble", 0.025)
    h.rod(
        g, "Shower riser", (-w / 2 + 0.90, 0.9, -0.49), (-w / 2 + 0.90, 2.25, -0.49), 0.016, "steel"
    )
    h.rod(
        g,
        "Rain shower arm",
        (-w / 2 + 0.90, 2.25, -0.49),
        (-w / 2 + 0.90, 2.25, 0.03),
        0.014,
        "steel",
    )
    v.cylinder(g, "Rain shower head", (-w / 2 + 0.90, 2.24, 0.03), 0.16, 0.024, h.M["steel"], 32)
    f.soft(g, "Wall hung toilet", (w / 2 - 0.55, 0.40, 0.50), (0.42, 0.45, 0.67), "ceramic", 0.5)
    h.box(g, "Flush plate", (w / 2 - 0.55, 0.97, -0.02), (0.19, 0.13, 0.012), "steel", 0.025)
    if primary:
        # Open oval tub, with a deep inset instead of a solid white block.
        vs = []
        fs = []
        n = 64
        for yy, rx, rz in [
            (0.04, 0.48, 1.02),
            (0.48, 0.57, 1.10),
            (0.63, 0.59, 1.12),
            (0.63, 0.52, 1.05),
            (0.15, 0.39, 0.86),
        ]:
            for i in range(n):
                a = i * math.tau / n
                vs.append((0.50 + rx * math.cos(a), yy, 0.40 + rz * 0.78 * math.sin(a)))
        for j in range(4):
            for i in range(n):
                fs.append(
                    (j * n + i, j * n + (i + 1) % n, (j + 1) * n + (i + 1) % n, (j + 1) * n + i)
                )
        fs.append(tuple(range(4 * n, 5 * n)))
        f.mesh(g, "Freestanding oval bath", vs, fs, "ceramic")
        f.faucet(g, 1.14, 0.61, 0.37)
    return g


def new_entry(o):
    for floor in [1, 2, 3]:
        room(
            "atrium_" + str(floor),
            "Atrium / stair",
            floor,
            [-6.30, 4.65, 7.5, 8.35],
            "circulation",
            1,
        )
    slab(o, "Arrival floor", -16, 5, -4, 14, 0)
    for x, w in [(-8, 3), (3, 3.5)]:
        h.box(o, "Travertine entry pier", (x, 4.15, 13), (w, 8.3, 0.36), "stone")
    wall(o, -15, -4, -15, 13, 0, 8.4, "stone", 0.36)
    # Door opening kept clear; glass only beside/above it.
    h.glass(o, -4.25, 0, 13, 3.50, 8.35)
    h.glass(o, 1.25, 0, 13, 1.0, 8.35)
    h.glass(o, -0.65, 3.20, 13, 2.1, 5.15)
    h.box(o, "Pivot door open", (-1.62, 1.54, 13.78), (0.11, 3.08, 1.80), "oak", 0.015)
    h.box(o, "Pivot door pull", (-1.53, 1.56, 14.40), (0.045, 0.85, 0.05), "bronze", 0.01)
    for y in [4.2, 8.4]:
        # Three-sided gallery with an unobstructed 6.6m stair void.
        slab(o, "Atrium front landing", -7.0, 1.2, 11.05, 13, y)
        slab(o, "Atrium west gallery", -7.0, -6.30, 4.65, 11.05, y)
        slab(o, "Atrium east gallery", 0.30, 1.20, 4.65, 11.05, y)
        h.rail(o, -6.30, y, 7.7, 6.1, True)
        h.rail(o, 0.30, y, 7.7, 6.1, True)
        # Continuous full-turn stair, 28 risers per storey, radial wedge treads.
        base = y - 4.2
        steps = 28
        vs = []
        fs = []
        for i in range(steps):
            a0 = math.pi / 2 + i * math.tau / steps
            a1 = a0 + math.tau / steps * 0.98
            yy = base + (i + 1) * 0.15
            verts = []
            for ty in [yy - 0.10, yy]:
                for r, a in [(1.15, a0), (3.05, a0), (3.05, a1), (1.15, a1)]:
                    verts.append((-3 + r * math.cos(a), ty, 8 + r * math.sin(a)))
            f.mesh(
                o,
                "Oak radial tread",
                verts,
                [
                    (0, 3, 2, 1),
                    (4, 5, 6, 7),
                    (0, 1, 5, 4),
                    (1, 2, 6, 5),
                    (2, 3, 7, 6),
                    (3, 0, 4, 7),
                ],
                "oak",
                False,
            )
        for side, radius in [("outer", 3.07), ("inner", 1.09)]:
            vs = []
            fs = []
            n = 160
            for i in range(n + 1):
                a = math.pi / 2 + i * math.tau / n
                yy = base + i * 4.2 / n
                for dr, dy in [(-0.045, 0), (0.045, 0), (0.045, 0.95), (-0.045, 0.95)]:
                    vs.append(
                        (-3 + (radius + dr) * math.cos(a), yy + dy, 8 + (radius + dr) * math.sin(a))
                    )
                if i:
                    for j in range(4):
                        fs.append(
                            (
                                (i - 1) * 4 + j,
                                (i - 1) * 4 + (j + 1) % 4,
                                i * 4 + (j + 1) % 4,
                                i * 4 + j,
                            )
                        )
            fs.extend([(3, 2, 1, 0), (4 * n, 4 * n + 1, 4 * n + 2, 4 * n + 3)])
            f.mesh(o, "Sculpted white " + side + " stair ribbon", vs, fs, "ivory")
    h.box(o, "Atrium skylight", (-3, 11.95, 8), (6.6, 0.055, 6.6), "glass")
    for x in [-8.9, -7.1, 2, 4]:
        h.box(o, "Bronze facade sconce", (x, 2.85, 13.25), (0.10, 0.65, 0.12), "bronze", 0.03)
        h.box(o, "Sconce aperture", (x, 2.50, 13.28), (0.05, 0.02, 0.09), "light")


def new_main(o):
    # Main level: continuous living/dining/kitchen/family space, with a rear suite.
    for yy, key in [(4.2, "oak"), (8.4, "ivory")]:
        slab(o, "Main floor" if yy == 4.2 else "Main roof", -15, 9, -15, 4.65, yy, key)
        slab(o, "Front west slab", -15, -6.3, 4.65, 6, yy, key)
        slab(o, "Front east slab", 0.30, 9, 4.65, 6, yy, key)
    wall(o, -15, -15, 9, -15, 4.2, 4.0)
    wall(o, -15, -15, -15, -7.2, 4.2, 4.0, "stone", 0.28)
    wall(o, -15, -5.8, -15, 6, 4.2, 4.0, "stone", 0.28)
    h.box(o, "Gym doorway header", (-15, 7.46, -6.5), (0.28, 1.48, 1.4), "stone")
    # Open pocketing glazing: no mullions across the main pool view.
    h.glass(o, 9, 4.2, -10.5, 9, 3.9, True)
    for z in [-5.7, -5.45, -5.2]:
        h.glass(o, 9, 4.2, z, 1.25, 3.9, True)
    for x in [-14.7, -14.5, -14.3]:
        h.glass(o, x, 4.2, 5.8, 1.6, 3.9)
    ceiling(o, -14.8, 8.8, -14.8, 4.5, 4.2, 3.95)
    # Clear route from the atrium into dining, then kitchen and family room.
    room("living", "Living", 2, [0.4, -2.4, 8.4, 8.1], station=2)
    room("dining", "Dining", 2, [-14.7, -3.2, 14.6, 7.5], station=2)
    room("kitchen", "Double-island kitchen", 2, [-14.7, -14.7, 10.0, 10.2], "kitchen", 4)
    room("family", "Family room", 2, [-4.4, -8.8, 13.2, 6.2], station=5)
    room("chef", "Chef kitchen", 2, [-4.4, -14.7, 5.2, 5.6], "kitchen", 6)
    room("suite_l2", "Suite / office", 2, [1.0, -14.7, 7.8, 5.6], "bedroom", 12)
    wall(o, -4.5, -15, -4.5, -10.1, 4.2, 3.95)
    doorwall(o, -4.5, 1, -9.0, 4.2, -1.8, 1.2, 3.95)
    wall(o, 0.85, -15, 0.85, -9, 4.2, 3.95)
    doorwall(o, 1, 9, -9, 4.2, 7.65, 1.1, 3.95)
    # Lighting accents and the shared onyx fireplace near the pool-side room.
    fire = f.group(o, "Pool-side onyx fireplace", (8.72, 4.2, 1.5), -math.pi / 2)
    fireplace(fire, 0, 0, 0, 3.6, 3.95)
    living = entity("living_furniture", "L2 / crescent sofas and dining")
    f.rug(living, 4.2, 4.2, 1.45, 6.7, 5.6)
    f.sofa(living, 4.2, 4.2, 3.20, 3.75, (4.2, 1.35))
    f.sofa(living, 4.2, 4.2, -0.25, 3.75, (4.2, 1.35))
    f.coffee(living, 4.2, 4.2, 1.4, 2.30, 0.95)
    # Dining chairs all face the table, including both end chairs.
    f.rug(living, -9, 4.2, 0.6, 7.2, 4.2)
    f.soft(living, "Oval walnut dining top", (-9, 4.98, 0.6), (4.8, 0.09, 1.28), "walnut", 0.7)
    for x in [-10.5, -7.5]:
        v.cylinder(living, "Fluted dining pedestal", (x, 4.59, 0.6), 0.32, 0.76, h.M["walnut"], 40)
    for x in [-10.65, -9.55, -8.45, -7.35]:
        for z in [-0.55, 1.75]:
            f.chair(living, x, 4.2, z, (-9, 0.6))
    for x in [-11.95, -6.05]:
        f.chair(living, x, 4.2, 0.6, (-9, 0.6))
    f.vase(living, -9, 5.03, 0.6, 1.2)
    kitchen = entity("kitchen", "L2 / two waterfall islands and appliances")
    f.kitchen(kitchen, -10, 4.2, -10.5)
    chef = entity("chef_kitchen", "L2 / separate personal chef kitchen")
    f.kitchen(chef, -1.9, 4.2, -12.85, False)
    family = entity("family", "L2 / family room open to kitchen")
    f.rug(family, 5.6, 4.2, -6.0, 5.8, 4.6)
    f.sofa(family, 3.65, 4.2, -6.0, 3.4, (7.8, -6.0))
    f.coffee(family, 5.5, 4.2, -6.0, 1.9, 0.8)
    tv = f.group(family, "Family media facing kitchen", (8.75, 4.2, -6.1), -math.pi / 2)
    wall(tv, -2.05, 0, 2.05, 0, 0, 3.95, "stone", 0.20)
    h.box(tv, "Television", (0, 2.0, 0.155), (2.25, 1.25, 0.065), "ebony", 0.02)
    h.box(tv, "Linear family hearth", (0, 0.77, 0.165), (2.25, 0.25, 0.075), "ebony")
    h.box(tv, "Family fire ribbon", (0, 0.68, 0.21), (2.1, 0.024, 0.02), "light")
    shelving(tv, -2.7, 0, -0.05, 1.7, 3.5)
    shelving(tv, 2.7, 0, -0.05, 1.7, 3.5)
    suite = entity("suite_l2", "L2 / one bedroom suite or office")
    f.bed(suite, 4.6, 4.2, -11.3, 1.8)
    BEDS.append(
        {"room": "suite_l2", "floor": 2, "residence": "main", "position": [4.6, 4.2, -11.3]}
    )
    f.rug(suite, 4.6, 4.2, -11.0, 4.3, 3.5)
    h.box(suite, "Suite wardrobe", (1.6, 5.55, -11.5), (0.64, 2.7, 3.8), "oak")
    # Compact ensuite, separated behind the headboard.
    doorwall(suite, 2.0, 8.8, -12.9, 4.2, 7.3, 1.0, 3.95)
    bathroom(suite, 5.4, 4.2, -13.55, 3.1)


def lower_rooms(o):
    slab(o, "Welcome lounge floor", -15, 4, -4.6, 4.65, 0)
    wall(o, -15, -4.65, 4, -4.65, 0, 4.2)
    ceiling(o, -14.8, 3.8, -4.55, 4.6, 0, 4.0, False)
    h.greenwall(o, -12.2, 0, -4.4, 5.3, 3.7)
    # Backlit onyx wine display on the correct lower floor.
    wine = entity("wine", "L1 / onyx-backed wine display")
    h.box(wine, "Onyx wine wall", (-5.5, 1.9, -4.40), (5.5, 3.8, 0.18), "onyx")
    for i in range(5):
        yy = 0.40 + i * 0.64
        h.box(wine, "Wine shelf", (-5.5, yy, -4.05), (5.35, 0.032, 0.56), "bronze")
        h.box(wine, "Wine shelf light", (-5.5, yy + 0.57, -4.25), (5.25, 0.012, 0.02), "light")
        for j in range(12):
            xx = -7.9 + j * 0.44
            v.cylinder(wine, "Wine bottle", (xx, yy + 0.20, -3.98), 0.060, 0.33, h.M["wine"], 12)
            v.cylinder(
                wine, "Bottle shoulder", (xx, yy + 0.37, -3.98), 0.043, 0.045, h.M["wine"], 12
            )
            v.cylinder(wine, "Bottle neck", (xx, yy + 0.435, -3.98), 0.025, 0.09, h.M["wine"], 10)
            v.cylinder(
                wine, "Cream wine label", (xx, yy + 0.20, -3.98), 0.061, 0.11, h.M["paper"], 12
            )
    fireplace(o, 0.4, 0, -4.36, 3.9, 4.0, "stone")
    h.box(o, "Welcome television", (0.4, 2.2, -4.14), (2.3, 1.25, 0.05), "ebony", 0.02)
    seating = entity("welcome_furniture", "L1 / welcome lounge seating")
    f.rug(seating, -4.8, 0, 0.05, 7.4, 5.5)
    f.sofa(seating, -4.8, 0, 1.65, 4.2, (-4.8, -0.30))
    f.sofa(seating, -0.9, 0, -0.3, 2.55, (-4.8, -0.30))
    f.coffee(seating, -4.5, 0, -0.2, 2.5, 1.2)
    # Photo reference also includes the billiards area by the living wall.
    h.box(seating, "Billiard oak frame", (-11.9, 0.70, 0.5), (2.7, 1.2, 1.55), "walnut", 0.06)
    h.box(seating, "Billiard cloth", (-11.9, 1.32, 0.5), (2.5, 0.045, 1.34), "leather")
    h.rod(seating, "Pool cue", (-13.0, 1.37, 0.12), (-10.9, 1.37, 0.87), 0.01, "oak")
    room("welcome", "Welcome + wine", 1, [-15, -4.65, 19, 9.3], station=7)
    theater = entity("cinema", "L1 / nine-seat cinema")
    slab(theater, "Cinema floor", -14.8, -5, -14.8, -5.0, 0, "rug")
    for x in [-14.8, -5]:
        wall(theater, x, -14.8, x, -5, 0, 3.8, "leather")
    wall(theater, -14.8, -14.8, -5, -14.8, 0, 3.8, "leather")
    doorwall(theater, -14.8, -5, -5, 0, -5.95, 1.15, 3.8)
    ceiling(theater, -14.8, -5, -14.8, -5, 0, 3.8, False)
    h.box(theater, "Cinema screen surround", (-9.9, 2.10, -14.63), (6.8, 2.9, 0.14), "ebony", 0.02)
    h.box(theater, "Projection screen", (-9.9, 2.12, -14.53), (6.35, 2.48, 0.02), "screen")
    for row, z in enumerate([-11.55, -9.45, -7.35]):
        slab(
            theater,
            "Tiered cinema platform",
            -14.55,
            -6.15,
            z - 0.95,
            z + 1.0,
            row * 0.16,
            "rug",
            0.16,
        )
        for x in [-12.15, -10.35, -8.55]:
            seat = f.facing(theater, "Cinema recliner", x, row * 0.16, z, (-9.9, -14.6))
            f.soft(seat, "Leather seat", (0, 0.39, 0), (1.00, 0.4, 0.94), "leather", 0.5)
            f.soft(seat, "Recliner back", (0, 0.88, -0.43), (0.99, 0.85, 0.23), "leather", 0.5)
            f.soft(seat, "Recliner headrest", (0, 1.24, -0.4), (0.76, 0.20, 0.22), "leather", 0.5)
            for dx in [-0.59, 0.59]:
                f.soft(seat, "Leather arm", (dx, 0.51, 0.02), (0.20, 0.54, 1.10), "leather", 0.45)
            for dx in [-0.40, 0.40]:
                for dz in [-0.32, 0.32]:
                    h.box(
                        seat,
                        "Recliner recessed foot",
                        (dx, 0.10, dz),
                        (0.12, 0.20, 0.12),
                        "ebony",
                        0.02,
                    )
        h.box(
            theater,
            "Cinema riser light",
            (-10.25, row * 0.16 + 0.015, z + 1.01),
            (8.0, 0.015, 0.018),
            "light",
        )
    room("cinema", "Cinema / 9 seats", 1, [-14.8, -14.8, 9.8, 9.8], station=8)


def upper_rooms(o, rear):
    y = 8.4
    # Primary is west of the atrium void, with bathroom and closet behind it.
    slab(o, "Primary floor", -15, -6.35, 1, 12.2, y, "oak")
    ceiling(o, -15, -6.35, 1, 12.2, y, 3.2)
    wall(o, -15, 1, -15, 12.2, y, 3.2, "stone", 0.25)
    wall(o, -15, 1, -6.35, 1, y, 3.2)
    h.glass(o, -10.65, y, 12.2, 8.7, 3.2)
    wall(o, -6.35, 1, -6.35, 6.2, y, 3.2)
    wall(o, -6.35, 7.45, -6.35, 12.2, y, 3.2)
    doorwall(o, -15, -6.35, 4.65, y, -7.25, 1.10, 3.2)
    wall(o, -10.60, 1, -10.60, 4.65, y, 3.2)
    # Open bathroom entrance beside the bed, separate from closet entry.
    # Replace a short segment with a second clear opening.
    # The west bathroom is reached from the closet through a 1.1m opening.
    for ob in list(o.children):
        if (
            ob.type == "MESH"
            and ob.name.startswith("Interior wall")
            and abs(ob.location.x + 10.60) < 0.01
        ):
            bpy.data.objects.remove(ob, do_unlink=True)
    wall(o, -10.6, 1, -10.6, 2.8, y, 3.2)
    wall(o, -10.6, 3.9, -10.6, 4.65, y, 3.2)
    bed = entity("primary_furniture", "L3 / primary bed, dressing and bath")
    f.rug(bed, -11, 8.4, 8.1, 6.3, 5.9)
    f.bed(bed, -11, y, 7.05, 2.10)
    BEDS.append({"room": "primary", "floor": 3, "residence": "main", "position": [-11, y, 7.05]})
    f.soft(bed, "Upholstered bed-end bench", (-11, y + 0.38, 9.2), (2.3, 0.38, 0.58), "throw", 0.55)
    for x in [-11.85, -10.15]:
        h.box(bed, "Bench oak support", (x, y + 0.16, 9.2), (0.09, 0.32, 0.40), "walnut", 0.02)
    # Headboard accent and a sculpted wall mirror, as in the room photographs.
    h.box(bed, "Primary headboard oak rail", (-11, y + 1.75, 4.79), (5.9, 0.12, 0.13), "oak", 0.025)
    f.soft(bed, "Primary sculpted mirror", (-11, y + 2.38, 4.80), (2.85, 1.02, 0.05), "steel", 0.85)
    for x in [-13.8, -8.2]:
        h.box(bed, "Low bedside joinery", (x, y + 0.36, 5.1), (0.9, 0.72, 0.56), "oak", 0.024)
        f.vase(bed, x, y + 0.74, 5.1, 0.7)
    h.box(bed, "Primary fireplace wall", (-14.82, y + 1.6, 8.4), (0.12, 3.2, 3.0), "stone")
    h.box(bed, "Primary linear firebox", (-14.73, y + 0.82, 8.4), (0.03, 0.36, 2.15), "ebony")
    bath = entity("primary_bath", "L3 / onyx primary bath and dressing")
    slab(bath, "Onyx bathroom floor", -14.9, -10.7, 1.1, 4.5, y + 0.015, "onyx", 0.02)
    h.box(bath, "Onyx bathroom wall", (-12.8, y + 1.60, 1.12), (4.25, 3.2, 0.08), "onyx")
    bathroom(bath, -12.7, y, 2.38, 4.1, True)
    for x in [-10.25, -6.75]:
        cab = f.group(
            bath, "Walk in dressing joinery", (x, y, 2.7), math.pi / 2 if x < -8 else -math.pi / 2
        )
        shelving(cab, 0, 0, 0, 2.65, 3.0)
    room("primary", "Primary bedroom", 3, [-15, 4.65, 8.65, 7.55], "bedroom", 9)
    room("primary_bath", "Primary bath", 3, [-15, 1, 4.4, 3.65], "bath", 10)
    room("closet", "Dressing", 3, [-10.6, 1, 4.25, 3.65], "service", 10)
    # Front terrace joins the landing without filling the helical stair opening.
    slab(o, "Primary terrace", -15, 3, 12.2, 15.2, y)
    h.rail(o, -6, y, 15.15, 18)
    for z in np.arange(1, 12.2, 0.13):
        h.box(
            o,
            "Fine cedar facade fin",
            (-15.18, y + 1.62, float(z)),
            (0.13, 3.24, 0.036),
            "oak",
            0.008,
        )
    # Rear gallery: exactly three additional suites + a shared upper lounge.
    slab(rear, "Rear upper floor", -15, 11, -15, -4.3, y, "oak")
    ceiling(rear, -15, 11, -15, -6, y, 3.2)
    wall(rear, -15, -15, 11, -15, y, 3.2)
    for x in [-15, -9, -3, 3, 11]:
        wall(rear, x, -15, x, -6, y, 3.2)
    for i, x in enumerate([-12, -6, 0]):
        name = "suite_l3_" + str(i + 1)
        r = entity(name, "L3 / bedroom suite " + str(i + 2))
        # Openable front glass with a clear door at the right jamb, not a bed showroom.
        h.glass(rear, x - 0.65, y, -6, 4.55, 3.2)
        h.box(rear, "Open suite entrance", (x + 2.4, y + 1.16, -6.55), (0.05, 2.32, 1.03), "oak")
        doorwall(rear, x - 3, x + 3, -12.25, y, x + 2, 1.0, 3.2)
        f.bed(r, x - 0.35, y, -10.3, 1.75)
        f.rug(r, x - 0.35, y, -9.3, 4.1, 4.8)
        bathroom(r, x, y, -13.72, 3.5)
        h.box(r, "Wardrobe", (x - 2.55, y + 1.4, -8.5), (0.60, 2.8, 3.2), "oak", 0.015)
        BEDS.append(
            {"room": name, "floor": 3, "residence": "main", "position": [x - 0.35, y, -10.3]}
        )
        room(name, "Suite " + str(i + 2), 3, [x - 3, -15, 6, 9], "bedroom", 12)
    lounge = entity("upper_lounge", "L3 / lounge, coffee bar and three workstations")
    h.glass(rear, 11, y, -10.5, 9, 3.2, True)
    f.sofa(lounge, 7, y, -7.1, 3.0, (7, -10.0))
    f.coffee(lounge, 7, y, -9.1, 1.7, 0.8)
    for x in [4.3, 6.5, 8.7]:
        h.box(
            lounge, "Built in workstation", (x, y + 0.75, -13.1), (1.8, 0.065, 0.70), "oak", 0.015
        )
        h.box(
            lounge,
            "Workstation pedestal",
            (x + 0.66, y + 0.36, -13.1),
            (0.40, 0.72, 0.65),
            "oak",
            0.008,
        )
        f.chair(lounge, x, y, -12.2, (x, -13.1))
        h.box(lounge, "Desk monitor", (x, y + 1.13, -13.28), (0.62, 0.37, 0.045), "ebony", 0.02)
        h.box(lounge, "Monitor stand", (x, y + 0.84, -13.2), (0.05, 0.17, 0.09), "metal")
    room("upper_lounge", "Lounge + 3 desks", 3, [3, -15, 8, 9], station=11)
    h.rail(rear, -2, y, -4.3, 26)
    slab(rear, "West gallery bridge", -7, -5.2, -4.3, 4.65, y)
    h.rail(rear, -5.2, y, 0.15, 8.9, True)


def adu(o):
    y = 8.4
    slab(o, "ADU floor", 3, 18, -29, -19, y, "oak")
    ceiling(o, 3, 18, -29, -19, y, 3.0)
    for x in [3, 18]:
        wall(o, x, -29, x, -19, y, 3.0)
    wall(o, 3, -29, 18, -29, y, 3.0)
    h.glass(o, 10.5, y, -19, 15, 3.0)
    wall(o, 10.5, -29, 10.5, -24, y, 3.0)
    for i, x in enumerate([6.75, 14.25]):
        doorwall(o, x - 3.75, x + 3.75, -24, y, x + 2.35, 1.05, 3.0)
        r = entity("adu_bed_" + str(i + 1), "ADU / bedroom " + str(i + 1))
        f.bed(r, x - 0.4, y, -26.0, 1.75)
        f.rug(r, x - 0.4, y, -25.5, 4.2, 3.8)
        # Each ADU bedroom has its own enclosed shower bathroom to the side.
        wall(r, x + 1.35, -29, x + 1.35, -25.15, y, 3.0)
        doorwall(r, x + 1.35, x + 3.7, -25.15, y, x + 2.65, 1.0, 3.0)
        h.box(r, "ADU shower tray", (x + 2.5, y + 0.05, -28.2), (1.8, 0.10, 1.3), "marble", 0.025)
        h.glass(r, x + 2.5, y, -27.52, 1.8, 2.3)
        h.rod(
            r,
            "ADU rainfall arm",
            (x + 2.5, y + 2.25, -28.8),
            (x + 2.5, y + 2.25, -28.2),
            0.015,
            "steel",
        )
        v.cylinder(r, "ADU rain shower", (x + 2.5, y + 2.25, -28.2), 0.14, 0.024, h.M["steel"], 24)
        h.box(r, "ADU vanity", (x + 1.75, y + 0.62, -26.4), (0.65, 0.45, 0.72), "oak", 0.02)
        f.soft(r, "ADU basin", (x + 1.75, y + 0.91, -26.4), (0.48, 0.12, 0.44), "ceramic", 0.6)
        f.soft(r, "ADU toilet", (x + 3.10, y + 0.37, -26.4), (0.46, 0.45, 0.68), "ceramic", 0.6)
        BEDS.append(
            {
                "room": "adu_bed_" + str(i + 1),
                "floor": "ADU",
                "residence": "adu",
                "position": [x - 0.4, y, -26],
            }
        )
        room(
            "adu_bed_" + str(i + 1),
            "ADU bedroom " + str(i + 1),
            "ADU",
            [x - 3.75, -29, 7.5, 5],
            "bedroom",
            13,
        )
    living = entity("adu_living", "ADU / independent kitchen and sitting room")
    f.kitchen(living, 5.55, y, -22.2, False)
    f.sofa(living, 13.6, y, -20.5, 2.8, (13.6, -22.4))
    f.coffee(living, 13.6, y, -22.2, 1.8, 0.80)
    room("adu_living", "ADU kitchen + living", "ADU", [3, -24, 15, 5], station=13)
    slab(o, "ADU terrace", 3, 18, -19, -16.2, y)
    h.rail(o, 10.5, y, -16.2, 15)
    slab(o, "ADU connecting bridge", 8.0, 9.5, -16.2, -14.8, y)
    # Separate exterior stair from the upper garden, not a floating pavilion.
    h.stairs(o, 19.8, 4.2, -17, 2.0, 4.2, 8.0, 28)


def fitness():
    o = entity("fitness", "L2 / gym and sauna")
    y = 4.2
    slab(o, "Fitness floor", -22, -15, -14, -4, y, "oak")
    ceiling(o, -22, -15, -14, -4, y, 3.95)
    wall(o, -22, -14, -22, -4, y, 3.95)
    wall(o, -22, -14, -15, -14, y, 3.95)
    h.glass(o, -18.5, y, -4, 7, 3.95)
    # Sauna is an enclosed cedar room at the back.
    wall(o, -22, -10.5, -15, -10.5, y, 3.2, "glass", 0.03)
    for z in [-13.3, -12.7]:
        for i in range(8):
            h.box(
                o,
                "Sauna bench slat",
                (-18.6, y + 0.50 + (z + 13.3) * 0.6, z + i * 0.067),
                (5.5, 0.045, 0.043),
                "oak",
                0.005,
            )
    h.box(o, "Backlit salt wall", (-18.5, y + 1.85, -13.83), (5.8, 2.45, 0.10), "onyx")
    for x in [-20.6, -17.9]:
        h.box(o, "Treadmill deck", (x, y + 0.18, -7.3), (1.15, 0.25, 2.2), "ebony", 0.055)
        for dx in [-0.46, 0.46]:
            h.rod(
                o,
                "Treadmill upright",
                (x + dx, y + 0.25, -8.1),
                (x + dx, y + 1.45, -8.4),
                0.045,
                "metal",
            )
        h.box(o, "Treadmill console", (x, y + 1.42, -8.4), (1.05, 0.12, 0.36), "ebony", 0.035)
    room("fitness", "Gym + sauna", 2, [-22, -14, 7, 10], station=14)


def decorate(entities, mats):
    v.decorate(entities, mats)
    ENTITIES.extend(entities)
    f.finish_materials()
    mats["screen"] = h.material("Cinema projection", (0.11, 0.16, 0.19), 0.95)
    screen = mats["screen"].node_tree.nodes.get("Principled BSDF")
    screen.inputs["Emission Color"].default_value = (0.07, 0.12, 0.17, 1)
    screen.inputs["Emission Strength"].default_value = 1
    by_id = {o["rou_entity_id"]: o for o in entities}
    site = by_id["walsh_site"]
    for ob in list(site.children_recursive):
        if ob.name.startswith("Upper garden"):
            bpy.data.objects.remove(ob, do_unlink=True)
    slab(site, "Rear raised terrain", -24, 24, -30, -15.15, 4.15, "grass", 1.0)
    slab(site, "ADU hillside foundation", 3, 18, -29, -19, 8.18, "stone", 4.03)
    for key in ["entry", "main", "lounge", "upper", "rear", "adu"]:
        for ob in list(by_id["walsh_" + key].children_recursive):
            bpy.data.objects.remove(ob, do_unlink=True)
    new_entry(by_id["walsh_entry"])
    new_main(by_id["walsh_main"])
    lower_rooms(by_id["walsh_lounge"])
    upper_rooms(by_id["walsh_upper"], by_id["walsh_rear"])
    adu(by_id["walsh_adu"])
    fitness()
    # Replace outdoor furniture and the obstructive lawn planting under the seats.
    garden = by_id["walsh_garden"]
    for name in [ob.name for ob in garden.children_recursive]:
        ob = bpy.data.objects.get(name)
        if ob is None:
            continue
        if ob.type == "EMPTY" or ob.name.startswith(
            ("Coffee", "Table", "Ceramic bowl", "Planting bed", "Ornamental meadow")
        ):
            if ob.name in bpy.data.objects:
                for ch in list(ob.children_recursive):
                    bpy.data.objects.remove(ch, do_unlink=True)
                bpy.data.objects.remove(ob, do_unlink=True)
    bpy.context.view_layer.update()
    for ob in list(garden.children_recursive):
        if (
            ob.type == "MESH"
            and min((ob.matrix_world @ __import__("mathutils").Vector(p)).z for p in ob.bound_box)
            > 8.3
        ):
            bpy.data.objects.remove(ob, do_unlink=True)
    outdoor = entity("outdoor_seating", "Garden / seating facing the central table")
    f.outdoor_sofa(outdoor, 3.6, 4.2, 7.3, 2.8, (3.6, 9.5))
    f.outdoor_sofa(outdoor, 3.6, 4.2, 11.6, 2.8, (3.6, 9.5))
    f.coffee(outdoor, 3.6, 4.2, 9.5, 1.8, 1.0)
    f.physical_uvs()
    # Inspect actual assemblies before joining, independently of the room schedule.
    # Seat/back geometry centroids establish facing, not merely the supplied yaw.
    from mathutils import Vector

    def xyz(p):
        return [float(p.x), float(p.z), float(-p.y)]

    source_beds = []
    source_seating = []
    for ob in list(bpy.data.objects):
        if ob.get("bed"):
            source_beds.append(
                {"position": xyz(ob.matrix_world.translation), "entity": ob.parent["rou_entity_id"]}
            )
        if ob.get("furniture_kind") != "Crescent sofa":
            continue
        seat = next(c for c in ob.children if c.name.startswith("Single tailored crescent seat"))
        back = next(c for c in ob.children if c.name.startswith("Padded wraparound back"))

        def centroid(mesh):
            return sum(
                (mesh.matrix_world @ vert.co for vert in mesh.data.vertices), Vector()
            ) / len(mesh.data.vertices)

        front = centroid(seat) - centroid(back)
        front.z = 0
        front.normalize()
        t = ob["view_target_xz"]
        target = ob.parent.matrix_world @ Vector(location((t[0], ob.location.z, t[1])))
        toward = target - ob.matrix_world.translation
        toward.z = 0
        toward.normalize()
        source_seating.append(
            {
                "position": xyz(ob.matrix_world.translation),
                "actual_front_dot_target": float(front.dot(toward)),
            }
        )
    SOURCE_AUDIT.update(beds=source_beds, sofas=source_seating)
    # Keep each semantic assembly as its own budgeted entity.
    entities[:] = ENTITIES
    print(
        "V3_INTERIORS",
        len(entities),
        "entities",
        len(BEDS),
        "beds",
        len(f.AUDIT),
        "oriented assemblies",
        flush=True,
    )


def configure(scene):
    listing = {
        "address": "349 Walsh Road",
        "location": "Atherton, California",
        "stats": "MAIN HOUSE  5 BEDROOMS   /   DETACHED ADU  2 BEDROOMS",
        "note": "Photo-informed study \u00b7 Room positions and dimensions approximate",
        "source": "https://villaanahid.com/details-main-residence/",
        "as_of": "2026-09-09",
        "visual_version": 3,
        "rooms": ROOMS,
    }
    scene["villa_anahid_listing_json"] = json.dumps(listing)
    entries = [
        (
            "The arrival",
            "Grounds / Terraced stone, oak and a three-storey stair.",
            [27, 10.5, 29],
            [-1, 5.7, 3],
            0,
        ),
        (
            "The atrium",
            "L1 / A continuous stair with open landings on each floor.",
            [0.10, 1.8, 14.1],
            [-3.4, 4.1, 8],
            1,
        ),
        (
            "Living & dining",
            "L2 / Two crescent sofas around an oval table; dining beyond.",
            [0.55, 5.8, 4.8],
            [5.25, 5.15, 0.5],
            2,
        ),
        (
            "Pool pavilion",
            "Grounds / A 56-foot pool and garden seating.",
            [21, 5.9, 13.5],
            [6, 5.7, 0.5],
            0,
        ),
        (
            "The main kitchen",
            "L2 / Two marble waterfall islands, gas range and integrated appliances.",
            [-5.1, 5.85, -5.4],
            [-10.3, 5.35, -11.5],
            2,
        ),
        (
            "The family room",
            "L2 / The family lounge opens directly to the main kitchen.",
            [0.85, 5.85, -3.1],
            [6.2, 5.2, -6.5],
            2,
        ),
        (
            "The chef kitchen",
            "L2 / A separate working kitchen behind the family room.",
            [-0.25, 5.8, -10.0],
            [-2, 5.35, -13.2],
            2,
        ),
        (
            "Welcome & wine",
            "L1 / Living wall, travertine fireplace and onyx wine display.",
            [1.8, 1.7, 3.5],
            [-5.7, 1.55, -2.2],
            1,
        ),
        (
            "The cinema",
            "L1 / Nine recliners on three stepped rows.",
            [-6.1, 2.25, -6.2],
            [-10.0, 1.9, -12],
            1,
        ),
        (
            "Primary retreat",
            "L3 / One primary bedroom with a separate bath and dressing room.",
            [-7.1, 10.1, 11.3],
            [-11, 9.4, 7.0],
            3,
        ),
        (
            "Bath & dressing",
            "L3 / Onyx, an open oval bath and bespoke oak joinery.",
            [-10.85, 10.0, 4.25],
            [-13.0, 9.6, 2.4],
            3,
        ),
        (
            "Upper lounge",
            "L3 / Three workstations and a shared lounge beside three bedroom suites.",
            [9.95, 10.05, -6.55],
            [6.4, 9.65, -11.5],
            3,
        ),
        (
            "Bedroom suite",
            "L2 / The fifth main-house bedroom can also serve as an office.",
            [7.7, 5.85, -9.85],
            [4.2, 5.1, -11.3],
            2,
        ),
        (
            "The guest residence",
            "ADU / A distinct two-bedroom home with its own kitchen and sitting area.",
            [16.8, 10.1, -19.7],
            [8.0, 9.6, -23.2],
            4,
        ),
        (
            "Fitness & sauna",
            "L2 / Exercise room with a glass-fronted sauna.",
            [-15.8, 5.9, -4.8],
            [-19.0, 5.5, -11.0],
            2,
        ),
        (
            "The architecture",
            "Grounds / Schematic reconstruction, not a measured as-built survey.",
            [37, 31, 28],
            [-1, 4, -5],
            0,
        ),
    ]
    scene["villa_anahid_waypoints_json"] = json.dumps(
        [
            {"name": n, "description": d, "position": p, "look_at": t, "floor": fl}
            for n, d, p, t, fl in entries
        ]
    )
    scene["rou_visual_pass"] = (
        "v3 / corrected floor program, detailed kitchens and upholstered furniture"
    )
    scene["rou_room_audit_json"] = json.dumps(
        {
            "rooms": ROOMS,
            "beds": BEDS,
            "doors": DOORS,
            "furniture": f.AUDIT,
            "source_geometry": SOURCE_AUDIT,
        }
    )


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--output", default=str(ROOT / "build/walsh-v3-2.1"))
    args = p.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    out = Path(args.output).resolve()
    h.cube = v.cube
    h.cylinder = v.cylinder
    h.texmat = v.texmat
    h.ball = v.ball
    h.sofa = v.sofa
    old_combine = h.combine

    def combine(o):
        old_combine(o)
        # Bake child transforms into the joined geometry. This avoids inflated
        # rotated AABBs and keeps entity-local GLBs straightforward to inspect.
        from mathutils import Matrix

        for ob in o.children:
            if ob.type != "MESH":
                continue
            transform = o.matrix_world.inverted() @ ob.matrix_world
            ob.data.transform(transform)
            ob.matrix_parent_inverse = Matrix.Identity(4)
            ob.matrix_basis = Matrix.Identity(4)

    h.combine = combine
    h.build(out, decorate=decorate, configure=configure)
    (out / "room-audit.json").write_text(bpy.context.scene["rou_room_audit_json"] + "\n")


if __name__ == "__main__":
    main()
