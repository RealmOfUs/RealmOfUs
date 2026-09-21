"""Headless, photo-informed Villa Anahid study. Dimensions beyond pool are estimates.
Reference photography is never copied into the model or exported application.
"""

import argparse
import json
import math
import random
import sys
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.common.blender_primitives import cube, cylinder, location, material, root
from toolchain.blender.rou_blender import core, exporter

R = random.Random(349)
M = {}


def texmat(name, base, kind):
    import numpy as np

    m = material(name, base, 0.62)
    n = 128 if kind == "wood" else 256
    y, x = np.mgrid[0:n, 0:n] / n
    rng = np.random.default_rng(349)
    if kind == "stone":
        v = 0.92 + 0.05 * np.sin(y * 110 + np.sin(x * 9) * 2) + 0.028 * rng.random((n, n))
    elif kind == "onyx":
        v = (
            0.70
            + 0.20 * np.sin(y * 49 + np.sin(x * 8) * 1.2)
            + 0.075 * np.sin(y * 270 + np.sin(x * 9) * 3)
        )
    else:
        v = 0.7 + 0.22 * np.sin(x * 190 + np.sin(y * 8)) + 0.06 * rng.random((n, n))
    im = bpy.data.images.new(name + " / original procedural texture", width=n, height=n)
    a = np.ones((n, n, 4), dtype=np.float32)
    for k in range(3):
        a[:, :, k] = np.clip(v * base[k], 0, 1)
    im.pixels.foreach_set(a.ravel())
    im.pack()
    node = m.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = im
    m.node_tree.links.new(
        node.outputs["Color"], m.node_tree.nodes.get("Principled BSDF").inputs["Base Color"]
    )
    return m


def box(o, n, p, s, key="stone", b=0):
    return cube(o, n, p, s, M[key], b)


def ball(o, n, p, s, key="leaf"):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
    a = bpy.context.object
    a.name = n
    a.parent = o
    a.location = location(p)
    a.scale = (s[0], s[2], s[1])
    a.data.materials.append(M[key])
    for f in a.data.polygons:
        f.use_smooth = True
    return a


def rod(o, n, a, b, r, key):
    av = Vector(location(a))
    bv = Vector(location(b))
    v = bv - av
    ob = cylinder(o, n, (0, 0, 0), r, v.length, M[key], 10)
    ob.location = (av + bv) / 2
    ob.rotation_euler = v.to_track_quat("Z", "Y").to_euler()
    return ob


def glass(o, x, y, z, w, h, side=False):
    box(o, "Low iron glazing", (x, y + h / 2, z), (0.025, h, w) if side else (w, h, 0.025), "glass")
    for k in range(int(w / 2.5) + 1):
        t = -w / 2 + k * w / max(1, int(w / 2.5))
        box(
            o,
            "Bronze mullion",
            (x, y + h / 2, z + t) if side else (x + t, y + h / 2, z),
            (0.055, h, 0.055),
            "metal",
        )
    for v in [y, y + h]:
        box(o, "Slim frame", (x, v, z), (0.06, 0.06, w) if side else (w, 0.06, 0.06), "metal")


def rail(o, x, y, z, w, side=False):
    box(
        o,
        "Glass balustrade",
        (x, y + 0.53, z),
        (0.025, 1.06, w) if side else (w, 1.06, 0.025),
        "glass",
    )
    box(
        o,
        "Balustrade cap",
        (x, y + 1.08, z),
        (0.035, 0.035, w) if side else (w, 0.035, 0.035),
        "metal",
    )


def sofa(o, x, y, z, w=3, angle=0):
    g = bpy.data.objects.new("Sofa assembly", None)
    bpy.context.collection.objects.link(g)
    g.parent = o
    g.location = location((x, y, z))
    g.rotation_euler.z = angle
    box(g, "Upholstered plinth", (0, 0.25, 0), (w, 0.5, 1.05), "fabric", 0.16)
    box(g, "Rounded back", (0, 0.73, -0.44), (w, 0.7, 0.26), "fabric", 0.12)
    for side in [-1, 1]:
        box(g, "Soft arm", (side * (w / 2 - 0.13), 0.62, 0), (0.26, 0.6, 1.05), "fabric", 0.12)
    for i in range(3):
        box(
            g,
            "Seat cushion",
            ((i - 1) * (w - 0.55) / 3, 0.57, 0.02),
            ((w - 0.6) / 3, 0.18, 0.78),
            "fabric",
            0.08,
        )
        ball(
            g, "Linen pillow", ((i - 1) * (w - 0.6) / 3, 0.86, -0.24), (0.31, 0.29, 0.13), "pillow"
        )


def table(o, x, y, z, w=1.6, d=0.8):
    box(o, "Coffee table", (x, y + 0.48, z), (w, 0.09, d), "wood", 0.045)
    for a in [-1, 1]:
        box(o, "Table base", (x + a * w * 0.32, y + 0.23, z), (0.08, 0.46, d * 0.6), "metal")
    cylinder(o, "Ceramic bowl", (x + 0.2, y + 0.58, z), 0.17, 0.12, M["ivory"], 24)


def planter(o, x, y, z, s=1):
    cylinder(o, "Stone planter", (x, y + 0.35 * s, z), 0.42 * s, 0.7 * s, M["ivory"], 20)
    for k in range(7):
        a = k * 2.4
        ball(
            o,
            "Botanical leaf",
            (
                x + math.cos(a) * 0.35 * s,
                y + (1 + R.random() * 0.5) * s,
                z + math.sin(a) * 0.35 * s,
            ),
            (0.35 * s, 0.20 * s, 0.14 * s),
            "leaf2",
        )


def greenwall(o, x, y, z, w, h):
    box(o, "Living wall", (x, y + h / 2, z), (w, h, 0.22), "leaf")
    for i in range(int(w * 3)):
        for j in range(int(h * 2.5)):
            ball(
                o,
                "Living foliage",
                (
                    x - w / 2 + (i + 0.5) / 3 + R.uniform(-0.1, 0.1),
                    y + (j + 0.5) / 2.5 + R.uniform(-0.1, 0.1),
                    z + R.uniform(0.12, 0.28),
                ),
                (R.uniform(0.15, 0.28), R.uniform(0.16, 0.30), 0.16),
                R.choice(["leaf", "leaf2", "leaf3"]),
            )


def stairs(o, x, y, z, width, rise, run, n):
    for i in range(n):
        box(
            o,
            "Floating limestone tread",
            (x, y + (i + 0.5) * rise / n, z - i * run / n),
            (width, rise / n, run / n + 0.06),
            "stone",
            0.025,
        )
    for side in [-1, 1]:
        rod(
            o,
            "Stair handrail",
            (x + side * width / 2, y + 1, z + 0.2),
            (x + side * width / 2, y + rise + 1, z - run),
            0.025,
            "metal",
        )
        for i in [0, n // 2, n - 1]:
            rod(
                o,
                "Railing post",
                (x + side * width / 2, y + i * rise / n, z - i * run / n),
                (x + side * width / 2, y + i * rise / n + 1, z - i * run / n),
                0.018,
                "metal",
            )


def room_shell(o, x, y, z, w, d, h=3.5, open_front=False):
    box(o, "Limestone floor", (x, y - 0.15, z), (w, 0.3, d))
    box(o, "Floating roof", (x, y + h + 0.16, z), (w + 0.7, 0.32, d + 0.7), "ivory", 0.04)
    box(o, "Rear wall", (x, y + h / 2, z - d / 2), (w, h, 0.24), "ivory")
    box(o, "Stone end wall", (x - w / 2, y + h / 2, z), (0.3, h, d))
    glass(o, x + w / 2, y, z, d, h, True)
    if not open_front:
        glass(o, x, y, z + d / 2, w, h)
    box(o, "Warm recessed reveal", (x, y + h - 0.05, z + d / 2 - 0.15), (w, 0.035, 0.05), "light")


def bed(o, x, y, z):
    box(o, "Walnut bed", (x, y + 0.2, z), (2.1, 0.4, 2.4), "wood", 0.1)
    box(o, "Linen mattress", (x, y + 0.5, z), (2, 0.3, 2.3), "fabric", 0.12)
    box(o, "Headboard", (x, y + 0.85, z - 1.2), (2.8, 1.6, 0.14), "wood", 0.05)
    box(o, "Folded throw", (x, y + 0.69, z + 0.6), (2.03, 0.08, 0.8), "pillow", 0.04)
    for dx in [-0.5, 0.5]:
        ball(o, "Pillow", (x + dx, y + 0.76, z - 0.73), (0.44, 0.14, 0.27), "fabric")
    for dx in [-1.5, 1.5]:
        box(o, "Nightstand", (x + dx, y + 0.3, z - 0.95), (0.65, 0.6, 0.65), "wood", 0.04)
        cylinder(o, "Lamp shade", (x + dx, y + 1, z - 0.95), 0.22, 0.3, M["ivory"], 24)


def combine(o):
    # One mesh per material per entity bounds draw calls without flattening entity identity.
    groups = {}
    for a in o.children_recursive:
        if a.type == "MESH":
            groups.setdefault(a.data.materials[0].name, []).append(a)
    for name, objs in groups.items():
        bpy.ops.object.select_all(action="DESELECT")
        for a in objs:
            a.select_set(True)
        bpy.context.view_layer.objects.active = objs[0]
        bpy.ops.object.join()
        a = bpy.context.object
        world = a.matrix_world.copy()
        a.parent = o
        a.matrix_world = world
        a.name = name
    for a in list(o.children_recursive):
        if a.type == "EMPTY" and not a.children:
            bpy.data.objects.remove(a, do_unlink=True)


def build(out, decorate=None, configure=None):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    M.update(
        stone=texmat("Honed travertine", (0.84, 0.79, 0.67), "stone"),
        wood=texmat("Fluted cedar", (0.43, 0.29, 0.16), "wood"),
        onyx=texmat("Honey onyx", (0.95, 0.73, 0.37), "onyx"),
    )
    for name, color, rough, metal in [
        ("ivory", (0.88, 0.87, 0.80), 0.75, 0),
        ("metal", (0.12, 0.13, 0.12), 0.3, 0.75),
        ("fabric", (0.82, 0.78, 0.66), 0.93, 0),
        ("pillow", (0.36, 0.32, 0.25), 0.95, 0),
        ("leaf", (0.065, 0.15, 0.07), 0.88, 0),
        ("leaf2", (0.17, 0.25, 0.09), 0.85, 0),
        ("leaf3", (0.24, 0.32, 0.10), 0.9, 0),
        ("grass", (0.13, 0.22, 0.11), 1, 0),
        ("water", (0.035, 0.32, 0.38), 0.18, 0.4),
        ("bark", (0.18, 0.13, 0.085), 1, 0),
        ("dark", (0.025, 0.029, 0.026), 0.4, 0),
        ("glass", (0.65, 0.83, 0.84), 0.08, 0.12),
        ("light", (1, 0.77, 0.38), 0.4, 0),
    ]:
        M[name] = material(name, color, rough, metal)
    bs = M["glass"].node_tree.nodes.get("Principled BSDF")
    bs.inputs["Alpha"].default_value = 0.12
    M["glass"].diffuse_color = (0.65, 0.83, 0.84, 0.12)
    M["glass"].surface_render_method = "DITHERED"
    bs = M["light"].node_tree.nodes.get("Principled BSDF")
    bs.inputs["Emission Color"].default_value = (1, 0.62, 0.22, 1)
    bs.inputs["Emission Strength"].default_value = 1
    entities = []

    def entity(eid, label):
        o = root("walsh_" + eid, label, (0, 0, 0))
        entities.append(o)
        return o

    o = entity("site", "Terraced garden & motor court")
    box(o, "Site plinth", (0, -0.65, 0), (66, 1, 68), "grass")
    box(o, "Arrival court", (-6, -0.09, 23), (35, 0.2, 15), "stone")
    for x in range(-22, 14, 3):
        box(o, "Grass paving joint", (x, 0.018, 23), (0.13, 0.025, 14), "grass")
    box(o, "Upper garden", (0, 3.6, -19), (48, 1.1, 15), "grass")
    for x in [-25, 25]:
        box(o, "Retaining wall", (x, 1.8, 0), (0.4, 4.4, 48))
    for i in range(16):
        box(o, "Driveway grass seam", (-25 + i * 0.7, -0.1, 33 - i * 0.4), (0.1, 0.03, 6), "grass")
    box(o, "Water mirror", (-4, 0.01, 15), (11, 0.07, 1.1), "water")
    o = entity("entry", "Double height atrium & sculptural stair")
    box(o, "Ground floor", (-5, -0.12, 5), (24, 0.25, 18))
    box(o, "West stone wing", (-15, 4, 5), (2, 8, 18))
    box(o, "Entry pier", (-8, 4, 13), (3, 8, 0.4))
    box(o, "East entry pier", (3, 4, 13), (3.5, 8, 0.4))
    glass(o, -2.2, 0, 13, 7.2, 8)
    box(o, "Portal surround", (-0.3, 1.7, 13.2), (2.4, 3.4, 0.4), "ivory")
    box(o, "Pivot door", (-0.3, 1.55, 13.44), (1.8, 3.1, 0.12), "wood")
    box(o, "Brass handle", (0.32, 1.65, 13.55), (0.035, 0.8, 0.06), "light")
    # Open helical stair: actual treads and a continuous sculpted outer ribbon.
    for floor in [0, 4.2]:
        verts = []
        faces = []
        for i in range(45):
            a = -math.pi * 0.35 + i / 44 * math.pi * 1.55
            y = floor + i / 44 * 4.2
            x = -3 + math.cos(a) * 2.2
            z = 8 + math.sin(a) * 2.2
            ob = box(o, "Helical tread", (x, y + 0.04, z), (1.65, 0.09, 0.43), "wood")
            ob.rotation_euler.z = -a
            for rad, h in [(3.04, 0), (3.04, 1.0), (3.14, 1.0), (3.14, 0)]:
                verts.append(location((-3 + math.cos(a) * rad, y + h, 8 + math.sin(a) * rad)))
            if i:
                for k in range(4):
                    faces.append(
                        ((i - 1) * 4 + k, (i - 1) * 4 + (k + 1) % 4, i * 4 + (k + 1) % 4, i * 4 + k)
                    )
        me = bpy.data.meshes.new("Sculptural ribbon")
        me.from_pydata(verts, [], faces)
        me.update()
        ob = bpy.data.objects.new("White helical balustrade", me)
        bpy.context.collection.objects.link(ob)
        ob.parent = o
        me.materials.append(M["ivory"])
    box(o, "Atrium skylight", (-3, 11.9, 8), (6, 0.06, 5), "glass")
    o = entity("lounge", "Garden lounge / lower level")
    box(o, "Lounge back wall", (-6, 1.85, -4), (20, 3.7, 0.3))
    box(o, "Lounge ceiling", (-6, 4.02, 1), (20, 0.25, 10), "ivory")
    greenwall(o, -12, 0, -3.77, 6, 3.5)
    box(o, "Media wall", (-3, 1.85, -3.7), (5, 3.7, 0.2), "onyx")
    box(o, "Screen", (-3, 2.25, -3.53), (2.7, 1.5, 0.07), "dark")
    box(o, "Linear fire", (-3, 0.72, -3.52), (2.4, 0.17, 0.08), "light")
    sofa(o, -4, 0, 1, 4.6)
    sofa(o, 0, 0, -0.5, 2.3, math.pi / 2)
    table(o, -3, 0, -0.8, 2.5, 1.1)
    box(o, "Woven rug", (-3, 0.025, -0.6), (7, 0.025, 5), "pillow")
    box(o, "Billiard walnut base", (-11, 0.55, 1), (2.8, 1.1, 1.6), "wood", 0.06)
    box(o, "Billiard felt", (-11, 1.13, 1), (2.6, 0.06, 1.4), "dark")
    for i in range(6):
        ball(
            o,
            "Billiard ball",
            (-11 + i * 0.065, 1.22, 1 + (i % 2) * 0.12),
            (0.045, 0.045, 0.045),
            "light",
        )
    o = entity("main", "Main living / dining / kitchen")
    room_shell(o, -3, 4.2, 0, 24, 12, 3.9, True)
    box(o, "Onyx fireplace", (5.3, 6.2, 4.9), (2.6, 4, 0.65), "onyx")
    box(o, "Hearth opening", (5.3, 5.08, 5.25), (2.15, 0.48, 0.025), "dark")
    box(o, "Fire ribbon", (5.3, 4.92, 5.28), (1.9, 0.05, 0.04), "light")
    for i in range(25):
        ball(
            o,
            "Flame",
            (4.45 + i * 0.065, 4.98 + R.random() * 0.1, 5.29),
            (0.022, 0.07, 0.018),
            "light",
        )
    sofa(o, 1, 4.2, 3, 3.5)
    sofa(o, 1, 4.2, -0.5, 3.5, math.pi)
    table(o, 1, 4.2, 1.3, 1.9, 1)
    box(o, "Lounge rug", (1, 4.22, 1.2), (5, 0.025, 4.5), "pillow")
    box(o, "Dining table", (-6, 5, 1), (3.6, 0.13, 1.15), "wood", 0.06)
    for x in [-7.2, -6, -4.8]:
        for z in [-0.0, 2]:
            box(o, "Dining seat", (x, 4.7, z), (0.57, 0.12, 0.57), "fabric", 0.07)
            box(
                o,
                "Chair back",
                (x, 5.05, z + (-0.25 if z < 1 else 0.25)),
                (0.55, 0.65, 0.10),
                "wood",
                0.04,
            )
            for dx in [-0.2, 0.2]:
                box(o, "Chair leg", (x + dx, 4.42, z), (0.045, 0.42, 0.45), "metal")
    box(o, "Kitchen cabinetry", (-10, 5.55, -5.5), (7, 2.7, 0.7), "wood")
    for x in [-12.5, -11.2, -9.9, -8.6]:
        box(o, "Cabinet reveal", (x, 5.55, -5.11), (0.018, 2.5, 0.02), "metal")
    box(o, "Kitchen island", (-10, 4.75, -2.6), (4.2, 1.1, 1.25), "stone", 0.04)
    box(o, "Marble worktop", (-10, 5.34, -2.6), (4.35, 0.13, 1.4), "ivory", 0.03)
    box(o, "Induction hob", (-11, 5.42, -2.6), (1, 0.02, 0.65), "dark")
    planter(o, 7.8, 4.2, 3.3)
    o = entity("pool", "56 foot pool / spa / waterfall pavilion")
    box(o, "Pool terrace west", (9.6, 3.9, 5.5), (2.6, 0.5, 22))
    box(o, "Pool terrace east", (21, 3.9, 5.5), (3.6, 0.5, 22))
    for edge in [-4.1, 16.2]:
        box(o, "Pool terrace end", (15, 3.9, edge), (8.2, 0.5, 2.4))
    box(o, "Pool basin", (15, 3.85, 6), (7.92, 0.3, 17.02), "water")
    box(o, "Pool water", (15, 4.10, 6), (7.65, 0.025, 16.7), "water")
    for x in [10.9, 19.1]:
        box(o, "Limestone coping", (x, 4.2, 6), (0.35, 0.25, 17.5))
    for z in [-2.65, 14.65]:
        box(o, "Limestone coping", (15, 4.2, z), (8.5, 0.25, 0.35))
    glass(o, 15, 0.5, 14.45, 7.9, 3.6)
    box(o, "Spa rim", (15, 4.22, -0.4), (4.0, 0.32, 3.1), "ivory")
    box(o, "Spa water", (15, 4.41, -0.4), (3.5, 0.025, 2.6), "water")
    for x in [9.1, 20.5]:
        box(o, "Pavilion slender pier", (x, 6.3, 15), (0.32, 4.5, 0.36), "ivory")
    box(o, "Cantilever frame", (10.7, 8.63, 15), (20, 0.44, 0.65), "ivory")
    box(o, "Cantilever frame", (20.5, 8.63, 5), (0.65, 0.44, 20), "ivory")
    box(o, "Cantilever frame", (14.7, 8.63, -5), (12.3, 0.44, 0.65), "ivory")
    greenwall(o, 15, 4.2, -5, 10.8, 4)
    box(o, "Sheer descent lip", (16.8, 7.8, -4.65), (3.8, 0.12, 0.4), "stone")
    box(o, "Waterfall veil", (16.8, 6, -4.35), (3.75, 3.5, 0.035), "glass")
    for i in range(24):
        rod(
            o,
            "Waterfall streak",
            (15 + i * 0.15, 7.75, -4.33),
            (15 + i * 0.15, 4.2, -4.33),
            0.007,
            "water",
        )
    stairs(o, 7, 0, 18, 3.2, 4.2, 8, 24)
    for z in [1.5, 5, 8.5]:
        box(o, "Teak sunlounger", (21.8, 4.48, z), (1.1, 0.22, 2.3), "wood", 0.07)
        box(o, "Lounger cushion", (21.8, 4.65, z), (1, 0.17, 2.1), "fabric", 0.06)
    o = entity("upper", "Primary suite & private terrace")
    room_shell(o, -9, 8.5, 7, 11, 10, 3.1)
    box(o, "Cedar cantilever fascia", (-14.6, 10.13, 7), (0.26, 3.35, 10.6), "wood")
    for z in range(52):
        box(o, "Cedar fins", (-14.76, 10.13, 1.9 + z * 0.2), (0.1, 3.35, 0.065), "wood")
    bed(o, -10, 8.5, 7)
    sofa(o, -5.3, 8.5, 9.7, 2, math.pi / 2)
    box(o, "Private terrace", (-0.4, 8.35, 10), (6.3, 0.3, 5.5), "stone")
    rail(o, -0.4, 8.5, 12.7, 6.3)
    rail(o, 2.8, 8.5, 10, 5.5, True)
    sofa(o, -0.6, 8.5, 10, 2.4)
    table(o, -0.6, 8.5, 11.6)
    planter(o, 2, 8.5, 11.9, 0.8)
    o = entity("rear", "Upper bedroom gallery")
    room_shell(o, -1, 8.5, -11, 25, 7, 3.1)
    for x in [-9, -3, 3, 9]:
        bed(o, x, 8.5, -11)
        if x < 9:
            box(o, "Suite partition", (x + 3, 10.05, -11), (0.16, 3.1, 7), "ivory")
    box(o, "Gallery terrace", (-1, 8.35, -5.8), (25, 0.3, 3.4), "stone")
    rail(o, -1, 8.5, -4.2, 25)
    o = entity("adu", "Detached guest residence")
    room_shell(o, 10, 8.5, -24, 14, 7, 3.0)
    bed(o, 6, 8.5, -24)
    bed(o, 14, 8.5, -24)
    box(o, "Guest partition", (10, 10, -24), (0.2, 3, 7), "ivory")
    box(o, "Guest terrace", (10, 8.3, -18.8), (15, 0.3, 3.2), "stone")
    sofa(o, 11, 8.5, -19, 2.6)
    table(o, 11, 8.5, -17.8)
    rail(o, 10, 8.5, -17.2, 15)
    o = entity("garden", "Courtyard garden & outdoor living")
    box(o, "Raised lawn", (4, 4.12, 9), (6, 0.15, 6), "grass")
    sofa(o, 3.6, 4.2, 9, 2.7)
    table(o, 3.6, 4.2, 10.7)
    for x, z, y in [(-12, 16, 0), (1.7, 15, 0), (4, 12, 4.2), (8, -7, 8.5), (-12, -6, 8.5)]:
        planter(o, x, y, z, 1.15)
    for i in range(27):
        x, z = R.choice([-1, 1]) * R.uniform(27, 31), R.uniform(-29, 29)
        if i > 17:
            x, z = R.uniform(-27, 27), R.choice([-30, 30])
        y = 0 if z > 15 else max(0, min(4, (15 - z) * 0.14))
        o = entity("tree_%02d" % i, "Woodland oak %02d" % i)
        # Individual roots are spatially owned at their own locations.
        o.location = location((x, y, z))
        cylinder(o, "Oak trunk", (0, 3, 0), 0.28, 6, M["bark"], 10)
        for k in range(11):
            a = R.random() * math.tau
            rr = R.uniform(0.5, 3)
            p = (math.cos(a) * rr, 5 + R.uniform(-0.3, 3), math.sin(a) * rr)
            rod(o, "Branch", (0, 2.8, 0), p, 0.07, "bark")
            ball(
                o,
                "Oak canopy",
                p,
                (R.uniform(1.2, 2.2), 1.2, R.uniform(1, 1.8)),
                R.choice(["leaf", "leaf2", "leaf3"]),
            )
    bpy.context.view_layer.update()
    for ob in list(bpy.data.objects):
        if (
            ob.type == "MESH"
            and ob.data.uv_layers
            and ob.data.materials
            and ob.data.materials[0].name in ["Honed travertine", "Honey onyx"]
        ):
            uv = ob.data.uv_layers.active.data
            for poly in ob.data.polygons:
                for li in poly.loop_indices:
                    co = ob.matrix_world @ ob.data.vertices[ob.data.loops[li].vertex_index].co
                    uv[li].uv = ((co.x if abs(poly.normal.y) > 0.5 else co.y) * 0.15, co.z * 0.18)
    if decorate:
        decorate(entities, M)
    for o in entities:
        combine(o)
    scene = bpy.context.scene
    scene["rou_profile_json"] = json.dumps(core.CONFIG["profile"])
    scene["rou_title"] = "VILLA ANAHID"
    scene["rou_license"] = "NOASSERTION"
    scene["villa_anahid_listing_json"] = json.dumps(
        {
            "address": "349 Walsh Road",
            "location": "Atherton, California",
            "stats": "5 BEDROOMS    /    8 BATHROOMS    /    11,968 SQ FT",
            "note": "Independent architectural study \u00b7 Approximate reconstruction",
            "source": "https://villaanahid.com/",
            "as_of": "2026-09-08",
        }
    )
    scene["villa_anahid_waypoints_json"] = json.dumps(
        [
            {
                "name": "The arrival",
                "description": "A sculptural home, set into the Atherton hillside.",
                "position": [26, 12.5, 27],
                "look_at": [0, 5, 2],
            },
            {
                "name": "The atrium",
                "description": "A floating stair connects three levels of living.",
                "position": [-1, 1.7, 11.5],
                "look_at": [-4, 4.5, 7],
            },
            {
                "name": "Living & dining",
                "description": "Warm stone, honey onyx and open garden rooms.",
                "position": [-2, 5.9, 6.3],
                "look_at": [4, 5.4, 3.5],
            },
            {
                "name": "Pool pavilion",
                "description": "A 56-foot pool beneath a cantilevered frame.",
                "position": [21.8, 6, 12],
                "look_at": [7, 5.7, 0],
            },
            {
                "name": "Garden lounge",
                "description": "A quieter level framed by a living green wall.",
                "position": [-8, 1.7, 4],
                "look_at": [-3, 1.6, -2],
            },
            {
                "name": "Primary retreat",
                "description": "Private living above the treetops.",
                "position": [-6, 10.2, 10.7],
                "look_at": [-10, 9.6, 6],
            },
            {
                "name": "Guest residence",
                "description": "An independent pavilion at the upper garden.",
                "position": [19, 12, -13],
                "look_at": [10, 9.8, -23],
            },
            {
                "name": "The architecture",
                "description": "Terraced volumes, courtyards and long horizontal lines.",
                "position": [37, 31, 28],
                "look_at": [0, 4, -4],
            },
        ]
    )
    if configure:
        configure(scene)
    bpy.context.view_layer.update()
    out.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(out / "villa-anahid.blend"))
    (out / "sample.json").write_text(
        json.dumps(
            {
                "listing": json.loads(scene["villa_anahid_listing_json"]),
                "waypoints": json.loads(scene["villa_anahid_waypoints_json"]),
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n"
    )
    exporter.export_scene(out / "preview")
    print("WALSH_COMPLETE", str(out))


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--output", default=str(ROOT / "build/walsh-2.1"))
    args = a.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    build(Path(args.output).resolve())
