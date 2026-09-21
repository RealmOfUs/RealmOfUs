"""Second, reproducible visual pass over the S1 architectural study.
All geometry / surface maps are generated here. HDRI attribution lives in client/assets.
Run: blender --background --python examples/villa_anahid/appearance.py
"""

import argparse
import json
import math
import random
import sys
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from examples.common.blender_primitives import location, material
from examples.villa_anahid import architecture as h

R = random.Random(20260909)


def texmat(name, base, kind):
    # Periodic multi-scale noise avoids directional striping and visible wrap seams.
    n = 512
    y, x = np.mgrid[0:n, 0:n] / n
    rng = np.random.default_rng(349)

    def noise(stretch=1):
        v = np.zeros((n, n))
        for frequency, amplitude in [
            (1, 0.5),
            (2, 0.25),
            (5, 0.13),
            (11, 0.07),
            (29, 0.035),
            (73, 0.012),
        ]:
            for _ in range(4):
                kx = int(rng.integers(1, frequency + 2))
                ky = int(rng.integers(1, frequency + 2))
                v += (
                    amplitude
                    * 0.25
                    * np.sin(2 * np.pi * (kx * x * stretch + ky * y) + rng.uniform(0, 6.28))
                )
        return v

    f = noise()
    if kind == "stone":
        tone = 0.96 + 0.055 * f + 0.012 * rng.normal(size=(n, n))
        vein = np.exp(-(((f - 0.04) * 70) ** 2)) * 0.024
        rgb = np.stack([tone * 0.88 - vein, tone * 0.85 - vein, tone * 0.79 - vein], axis=-1)
    elif kind == "onyx":
        bands = np.sin(y * math.tau * 4 + f * 1.0 + np.sin(x * math.tau) * 0.12)
        vein = np.exp(-((bands * 11) ** 2))
        tone = 0.86 + 0.08 * bands + 0.08 * f
        rgb = (
            np.stack([tone + 0.09, tone * 0.86 + 0.05, tone * 0.66 + 0.12], axis=-1)
            - vein[:, :, None] * 0.04
        )
    else:
        grain = noise(18)
        tone = 0.88 + 0.23 * grain + 0.035 * f
        rgb = np.stack([tone * 0.45, tone * 0.30, tone * 0.18], axis=-1)
    m = material(name, (1, 1, 1), 0.46 if kind == "onyx" else 0.7)
    a = np.ones((n, n, 4), dtype=np.float32)
    a[:, :, :3] = np.clip(rgb, 0, 1)
    im = bpy.data.images.new(name + " / original seamless surface", width=n, height=n)
    im.pixels.foreach_set(a.ravel())
    im.pack()
    node = m.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = im
    m.node_tree.links.new(
        node.outputs["Color"], m.node_tree.nodes.get("Principled BSDF").inputs["Base Color"]
    )
    return m


def mesh(parent, name, verts, faces, key, smooth=False):
    me = bpy.data.meshes.new(name)
    me.from_pydata([location(v) for v in verts], [], faces)
    me.update()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.parent = parent
    me.materials.append(h.M[key])
    for poly in me.polygons:
        poly.use_smooth = smooth
    return ob


def cube(parent, name, p, size, mat, bevel=0):
    # Direct mesh construction avoids scene-wide operator updates for thousands of details.
    sx, sy, sz = (s / 2 for s in size)
    vs = [
        (-sx, -sy, -sz),
        (sx, -sy, -sz),
        (sx, sy, -sz),
        (-sx, sy, -sz),
        (-sx, -sy, sz),
        (sx, -sy, sz),
        (sx, sy, sz),
        (-sx, sy, sz),
    ]
    me = bpy.data.meshes.new(name)
    me.from_pydata(
        [location(v) for v in vs],
        [],
        [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (3, 7, 6, 2), (0, 4, 7, 3), (1, 2, 6, 5)],
    )
    me.update()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.parent = parent
    ob.location = location(p)
    me.materials.append(mat)
    bm = bmesh.new()
    bm.from_mesh(me)
    if bevel:
        bmesh.ops.bevel(
            bm,
            geom=list(bm.edges),
            offset=min(bevel, min(size) * 0.44),
            segments=4,
            profile=0.5,
            affect="EDGES",
            clamp_overlap=True,
        )
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    if bevel:
        for poly in me.polygons:
            poly.use_smooth = True
        wn = ob.modifiers.new("Area weighted normals", "WEIGHTED_NORMAL")
        wn.keep_sharp = True
    me.uv_layers.new(name="Surface metres")
    return ob


def cylinder(parent, name, p, radius, depth, mat, vertices=12):
    vs = []
    fs = []
    for yy in [-depth / 2, depth / 2]:
        for j in range(vertices):
            a = j * math.tau / vertices
            vs.append(location((radius * math.cos(a), yy, radius * math.sin(a))))
    for j in range(vertices):
        fs.append((j, (j + 1) % vertices, (j + 1) % vertices + vertices, j + vertices))
    fs.extend([tuple(reversed(range(vertices))), tuple(range(vertices, vertices * 2))])
    me = bpy.data.meshes.new(name)
    me.from_pydata(vs, [], fs)
    me.update()
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(ob)
    ob.parent = parent
    ob.location = location(p)
    me.materials.append(mat)
    for f in me.polygons:
        f.use_smooth = len(f.vertices) == 4
    return ob


def leaf(vs, fs, p, size, angle, tilt=0.4):
    # Folded, tapered leaf, real geometry (no alpha cards / external textures).
    i = len(vs)
    p = Vector(p)
    along = Vector((math.cos(angle), tilt, math.sin(angle))).normalized() * size
    across = Vector((-math.sin(angle), 0.06, math.cos(angle))) * size * 0.21
    vs.extend(
        [
            tuple(p - along * 0.2),
            tuple(p + along * 0.35 + across),
            tuple(p + along),
            tuple(p + along * 0.35 - across),
            tuple(p + along * 0.38 + Vector((0, size * 0.1, 0))),
        ]
    )
    fs.extend([(i, i + 1, i + 4), (i + 1, i + 2, i + 4), (i + 2, i + 3, i + 4), (i + 3, i, i + 4)])


old_ball = h.ball


def ball(o, n, p, s, key="leaf"):
    if n in ["Living foliage", "Botanical leaf"]:
        vs = []
        fs = []
        for _ in range(12):
            q = [
                p[0] + R.uniform(-s[0], s[0]),
                p[1] + R.uniform(-s[1], s[1]),
                p[2] + R.uniform(-s[2], s[2]),
            ]
            leaf(
                vs,
                fs,
                q,
                R.uniform(0.12, 0.28) if n == "Living foliage" else R.uniform(0.22, 0.48),
                R.random() * math.tau,
                R.uniform(-0.5, 0.7),
            )
        return mesh(o, n, vs, fs, key, True)
    if key in ["fabric", "pillow"]:
        ob = cube(o, n, p, [k * 2 for k in s], h.M[key], min(s) * 0.85)
        ob.rotation_euler.y = R.uniform(-0.14, 0.14)
        return ob
    return old_ball(o, n, p, s, key)


def sofa(o, x, y, z, w=3, angle=0):
    g = bpy.data.objects.new("Curved linen sofa", None)
    bpy.context.collection.objects.link(g)
    g.parent = o
    g.location = location((x, y, z))
    g.rotation_euler.z = angle
    h.box(g, "Recessed walnut feet", (0, 0.11, 0), (w - 0.4, 0.18, 0.8), "wood", 0.045)
    h.box(g, "Soft upholstered base", (0, 0.33, 0), (w, 0.45, 1.12), "fabric", 0.20)
    # Curved continuous back wraps into the arms instead of a rectangular box.
    vs = []
    fs = []
    for i in range(41):
        t = i / 40 * math.pi
        xx = -math.cos(t) * (w / 2 - 0.16)
        zz = -math.sin(t) * 0.46
        for dy, dr in [(0.44, -0.12), (0.95, -0.12), (0.95, 0.12), (0.44, 0.12)]:
            vs.append((xx + math.cos(t) * dr, dy, zz - math.sin(t) * dr))
        if i:
            for k in range(4):
                fs.append(
                    ((i - 1) * 4 + k, (i - 1) * 4 + (k + 1) % 4, i * 4 + (k + 1) % 4, i * 4 + k)
                )
    fs.extend([(3, 2, 1, 0), (160, 161, 162, 163)])
    back = mesh(g, "Wraparound boucl\u00e9 back", vs, fs, "fabric", True)
    bevel = back.modifiers.new("Rounded upholstery seam", "BEVEL")
    bevel.width = 0.09
    bevel.segments = 3
    count = max(2, round(w / 1.2))
    for i in range(count):
        dx = (i - (count - 1) / 2) * (w - 0.5) / count
        h.box(
            g,
            "Deep seat cushion",
            (dx, 0.59, 0.12),
            ((w - 0.54) / count, 0.25, 0.76),
            "fabric",
            0.11,
        )
        if i in [0, count - 1]:
            ball(g, "Linen scatter cushion", (dx, 0.81, -0.17), (0.29, 0.24, 0.12), "pillow")


def tree(o, index):
    for ob in list(o.children_recursive):
        bpy.data.objects.remove(ob, do_unlink=True)
    height = R.uniform(15, 23)
    o.name = "Coastal conifer %02d" % index
    vs = []
    fs = []
    for yy, r in [(0, 0.27), (height * 0.55, 0.11), (height, 0.006)]:
        for j in range(12):
            a = j * math.tau / 12
            vs.append((math.cos(a) * r, yy, math.sin(a) * r))
    for band in range(2):
        for j in range(12):
            fs.append(
                (
                    band * 12 + j,
                    band * 12 + (j + 1) % 12,
                    (band + 1) * 12 + (j + 1) % 12,
                    (band + 1) * 12 + j,
                )
            )
    mesh(o, "Tapered conifer trunk", vs, fs, "bark", True)
    groups = {k: ([], []) for k in ["leaf", "leaf2", "leaf3"]}
    for tier in range(24):
        yy = 2.5 + tier * (height - 3) / 24
        extent = (1 - yy / height) ** 0.8 * R.uniform(4.5, 6.2)
        for j in range(5):
            a = tier * 2.399 + j * math.tau / 5 + R.uniform(-0.3, 0.3)
            end = (math.cos(a) * extent, yy - R.uniform(0.1, 0.7), math.sin(a) * extent)
            h.rod(
                o,
                "Sweeping bough",
                (0, yy + 0.2, 0),
                end,
                0.025 * (1 - yy / height) + 0.008,
                "bark",
            )
            for step in range(4):
                t = (step + 0.65) / 4
                for side in [-1, 1]:
                    spread = extent * (1 - t) * 0.45
                    q = (
                        end[0] * t + math.cos(a + side * 1.0) * spread,
                        end[1] + R.uniform(-0.12, 0.30),
                        end[2] * t + math.sin(a + side * 1.0) * spread,
                    )
                    vs, fs = groups[R.choices(["leaf", "leaf2", "leaf3"], [6, 3, 1])[0]]
                    for needle in range(5):
                        leaf(
                            vs,
                            fs,
                            (
                                q[0] + R.uniform(-0.12, 0.12),
                                q[1] + R.uniform(-0.18, 0.18),
                                q[2] + R.uniform(-0.12, 0.12),
                            ),
                            R.uniform(0.42, 0.85) * (1 - 0.2 * yy / height),
                            a + side * (0.35 + needle * 0.5),
                            R.uniform(-0.2, 0.4),
                        )
    for key, (vs, fs) in groups.items():
        mesh(o, "Fine evergreen sprays / " + key, vs, fs, key, True)


def decorate(entities, mats):
    print("VISUAL_DETAILS", flush=True)
    # More restrained palette; small geometric details catch the practical lights.
    for key, color in {
        "ivory": (0.79, 0.78, 0.73),
        "fabric": (0.73, 0.69, 0.60),
        "pillow": (0.31, 0.23, 0.15),
        "leaf": (0.022, 0.064, 0.033),
        "leaf2": (0.055, 0.105, 0.047),
        "leaf3": (0.11, 0.16, 0.065),
        "grass": (0.07, 0.105, 0.046),
    }.items():
        mats[key].node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (
            *color,
            1,
        )
    mats["grout"] = material("Warm limestone joints", (0.34, 0.32, 0.28), 0.95)
    mats["bronze"] = material("Satin champagne bronze", (0.4, 0.25, 0.105), 0.3, 0.7)
    mats["wine"] = material("Dark green wine glass", (0.016, 0.031, 0.018), 0.22, 0.12)
    mats["paper"] = material("Book paper", (0.66, 0.59, 0.46), 0.95)
    mats["pooltile"] = material("Sea green pool tile", (0.07, 0.22, 0.22), 0.34)
    mats["soil"] = material("Fine dark mulch", (0.033, 0.027, 0.02), 1)
    light = mats["light"].node_tree.nodes.get("Principled BSDF")
    light.inputs["Emission Color"].default_value = (1, 0.53, 0.18, 1)
    light.inputs["Emission Strength"].default_value = 1
    by_id = {o["rou_entity_id"]: o for o in entities}
    for i, o in enumerate(entities):
        if "tree_" in o["rou_entity_id"]:
            tree(o, i)
            print("VISUAL_TREE", i, flush=True)
    # Stone slab joints, floor projection, built-in coves and recessed luminaires.
    rooms = [
        ("main", -3, 4.2, 0, 24, 12, 3.9),
        ("lounge", -6, 0, 1, 20, 10, 3.85),
        ("upper", -9, 8.5, 7, 11, 10, 3.1),
        ("rear", -1, 8.5, -11, 25, 7, 3.1),
        ("adu", 10, 8.5, -24, 14, 7, 3),
    ]
    for key, x, y, z, w, d, hh in rooms:
        o = by_id["walsh_" + key]
        for xx in np.arange(x - w / 2 + 1.5, x + w / 2, 1.5):
            h.box(o, "Limestone slab joint", (float(xx), y + 0.006, z), (0.008, 0.009, d), "grout")
        for zz in np.arange(z - d / 2 + 1.5, z + d / 2, 1.5):
            h.box(o, "Limestone slab joint", (x, y + 0.006, float(zz)), (w, 0.009, 0.008), "grout")
        for zz in [z - d / 2 + 0.25, z + d / 2 - 0.3]:
            h.box(o, "Walnut ceiling cove", (x, y + hh - 0.07, zz), (w - 0.2, 0.11, 0.14), "wood")
            h.box(o, "Concealed warm cove", (x, y + hh - 0.14, zz), (w - 0.3, 0.025, 0.05), "light")
        for xx in np.arange(x - w / 2 + 2, x + w / 2, 3.8):
            for zz in [z - d / 2 + 1.6, z + d / 2 - 1.6]:
                h.cylinder(
                    o,
                    "Recessed black bezel",
                    (float(xx), y + hh - 0.06, zz),
                    0.1,
                    0.028,
                    mats["dark"],
                    16,
                )
                h.cylinder(
                    o,
                    "Warm recessed lens",
                    (float(xx), y + hh - 0.079, zz),
                    0.064,
                    0.012,
                    mats["light"],
                    16,
                )
    main = by_id["walsh_main"]
    for ob in list(main.children_recursive):
        if ob.name.startswith("Chair leg"):
            bpy.data.objects.remove(ob, do_unlink=True)
    for xx in [-7.2, -6, -4.8]:
        for zz in [0, 2]:
            for dx in [-0.21, 0.21]:
                for dz in [-0.21, 0.21]:
                    h.box(
                        main,
                        "Slender chair leg",
                        (xx + dx, 4.45, zz + dz),
                        (0.043, 0.45, 0.043),
                        "wood",
                        0.01,
                    )
    for xx in [-7.1, -4.9]:
        h.box(main, "Dining pedestal", (xx, 4.61, 1), (0.18, 0.78, 0.8), "wood", 0.06)
    # A built-in wine display, with real shelves, bottles, labels and bronze dividers.
    h.box(main, "Wine display recess", (-2, 6, -5.78), (5.1, 3.2, 0.3), "dark", 0.02)
    for x in [-4.5, -2, 0.5]:
        h.box(main, "Wine cabinet divider", (x, 6, -5.4), (0.04, 3.2, 0.6), "bronze")
    for row in range(5):
        yy = 4.53 + row * 0.59
        h.box(main, "Walnut wine shelf", (-2, yy, -5.42), (5, 0.05, 0.58), "wood", 0.015)
        h.box(main, "Shelf concealed light", (-2, yy + 0.49, -5.7), (4.9, 0.017, 0.02), "light")
        for col in range(13):
            xx = -4.3 + col * 0.38
            h.cylinder(main, "Wine bottle", (xx, yy + 0.22, -5.34), 0.065, 0.36, mats["wine"], 10)
            h.cylinder(main, "Bottle neck", (xx, yy + 0.44, -5.34), 0.028, 0.12, mats["wine"], 8)
            h.cylinder(main, "Bottle label", (xx, yy + 0.23, -5.34), 0.066, 0.12, mats["paper"], 10)
    for i, x in enumerate([-7.2, -6, -4.8]):
        h.rod(main, "Pendant cord", (x, 8, 1), (x, 6.5, 1), 0.008, "metal")
        h.cylinder(main, "Bronze pendant", (x, 6.45, 1), 0.18, 0.10, mats["bronze"], 24)
        h.cylinder(main, "Opal pendant lens", (x, 6.39, 1), 0.14, 0.025, mats["light"], 24)
    for key, x, y, z in [
        ("main", 1, 4.2, 1.3),
        ("lounge", -3, 0, -0.8),
        ("upper", -0.6, 8.5, 11.6),
    ]:
        o = by_id["walsh_" + key]
        for j in range(3):
            ob = h.box(
                o,
                "Architecture monograph",
                (x - 0.42, y + 0.56 + j * 0.055, z),
                (0.5, 0.05, 0.34),
                "paper" if j == 1 else "wood",
                0.01,
            )
            ob.rotation_euler.z = j * 0.12
        h.cylinder(o, "Stoneware vase", (x + 0.4, y + 0.75, z), 0.12, 0.42, mats["ivory"], 32)
        for j in range(3):
            h.rod(
                o,
                "Dried branch",
                (x + 0.4, y + 0.9, z),
                (
                    x + 0.4 + R.uniform(-0.2, 0.2),
                    y + 1.35 + R.random() * 0.2,
                    z + R.uniform(-0.1, 0.1),
                ),
                0.006,
                "bark",
            )
    entry = by_id["walsh_entry"]
    for x in [-8.9, -7.1, 2, 4]:
        h.box(entry, "Bronze facade sconce", (x, 2.85, 13.28), (0.1, 0.65, 0.14), "bronze", 0.035)
        h.box(entry, "Sconce warm aperture", (x, 2.49, 13.3), (0.06, 0.018, 0.1), "light")
    for xx in np.arange(-1.1, 0.55, 0.09):
        h.box(
            entry,
            "Fine fluted pivot door",
            (float(xx), 1.55, 13.52),
            (0.038, 3.04, 0.025),
            "wood",
            0.01,
        )
    pool = by_id["walsh_pool"]
    for ob in pool.children_recursive:
        if ob.name.startswith("Pool basin"):
            ob.data.materials[0] = mats["pooltile"]
    for xx in [9, 20.2]:
        h.box(
            pool, "Pavilion concealed linear light", (xx, 8.385, 5), (0.035, 0.028, 19.5), "light"
        )
    h.box(pool, "Front pavilion light", (14.7, 8.386, 14.9), (11.4, 0.027, 0.035), "light")
    for i in range(0, 24, 2):
        h.box(
            pool,
            "Warm step nosing",
            (7, (i + 1) * 4.2 / 24 + 0.004, 18 - i * 8 / 24 + 0.16),
            (2.9, 0.012, 0.022),
            "light",
        )
    for z in [1.5, 5, 8.5]:
        h.box(
            pool, "Lounger folded towel", (21.8, 4.79, z + 0.55), (0.88, 0.09, 0.52), "ivory", 0.04
        )
        h.cylinder(pool, "Side table", (20.95, 4.48, z + 0.9), 0.25, 0.42, mats["bronze"], 24)
    # Low, irregular planting in beds softens the motor-court edge, without obscuring the house.
    garden = by_id["walsh_garden"]
    for x, z, w, d, y in [
        (-19, 22, 3.8, 17, 0),
        (17, 24, 10, 4, 0),
        (24, 0, 2, 22, 4.15),
        (4, 9, 5.5, 5.5, 4.2),
    ]:
        h.box(garden, "Planting bed", (x, y + 0.03, z), (w, 0.08, d), "soil")
        vs = []
        fs = []
        for _ in range(int(w * d * 10)):
            px = x + R.uniform(-w / 2, w / 2)
            pz = z + R.uniform(-d / 2, d / 2)
            height = R.uniform(0.2, 0.6)
            for j in range(3):
                leaf(
                    vs, fs, (px, y + 0.06, pz), height, j * 2.399 + R.random(), R.uniform(1.5, 3.5)
                )
        mesh(garden, "Ornamental meadow grasses", vs, fs, "leaf2")
    # Correct physical UVs for every surface, including horizontal floors.
    bpy.context.view_layer.update()
    for ob in bpy.data.objects:
        if ob.type != "MESH" or not ob.data.materials:
            continue
        name = ob.data.materials[0].name
        if name not in ["Honed travertine", "Honey onyx", "Fluted cedar"]:
            continue
        uv = ob.data.uv_layers.active or ob.data.uv_layers.new()
        for poly in ob.data.polygons:
            normal = ob.matrix_world.to_3x3() @ poly.normal
            drop = max(range(3), key=lambda i: abs(normal[i]))
            axes = [i for i in range(3) if i != drop]
            for li in poly.loop_indices:
                co = ob.matrix_world @ ob.data.vertices[ob.data.loops[li].vertex_index].co
                uv.data[li].uv = (
                    co[axes[0]] * (0.24 if name == "Fluted cedar" else 0.16),
                    co[axes[1]] * 0.16,
                )


def configure(scene):
    entries = json.loads(scene["villa_anahid_waypoints_json"])
    for i, pos, look in [
        (0, [26, 10.5, 29], [-1, 5.6, 3]),
        (1, [1, 3.5, 14], [-3, 5.3, 8]),
        (2, [6.9, 5.8, 3.6], [-4, 5.4, -1.5]),
        (3, [21, 5.8, 13.5], [9, 5.7, 0]),
        (4, [0.5, 1.65, 3.5], [-7, 1.65, -2.5]),
        (5, [-5.9, 10.15, 10.8], [-10, 9.7, 6.5]),
    ]:
        entries[i]["position"] = pos
        entries[i]["look_at"] = look
    scene["villa_anahid_waypoints_json"] = json.dumps(entries)
    listing = json.loads(scene["villa_anahid_listing_json"])
    listing["as_of"] = "2026-09-09"
    scene["villa_anahid_listing_json"] = json.dumps(listing)
    scene["rou_visual_pass"] = (
        "v2 / twilight, detailed planting, curved upholstery, physical surface UVs"
    )


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--output", default=str(ROOT / "build/walsh-v2-2.1"))
    args = p.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    h.cube = cube
    h.cylinder = cylinder
    h.texmat = texmat
    h.ball = ball
    h.sofa = sofa
    h.build(Path(args.output).resolve(), decorate=decorate, configure=configure)
