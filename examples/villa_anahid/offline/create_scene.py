"""Photo-informed living room study, authored for Cycles rather than the S1 GLB budget.

Blender coordinates: metres, Z up. This is an approximate, isolated art study.
Run fetch_assets.py first, then this script with Blender headless.
"""

import argparse
import json
import math
import random
import sys
import time
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "build/walsh-offline"
ASSETS = ROOT / ".cache/walsh-offline/assets"
R = random.Random(349)
M = {}


def mat(name, color, rough=0.5, metal=0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes.get("Principled BSDF")
    p.inputs["Base Color"].default_value = (*color, 1)
    p.inputs["Roughness"].default_value = rough
    p.inputs["Metallic"].default_value = metal
    return m


def tex(m, path, noncolor=False):
    n = m.node_tree.nodes.new("ShaderNodeTexImage")
    n.image = bpy.data.images.load(str(path), check_existing=True)
    if noncolor:
        n.image.colorspace_settings.name = "Non-Color"
    return n


def pbr(name, asset, physical_size, tint=None):
    m = mat(name, (0.6, 0.5, 0.4))
    n = m.node_tree.nodes
    links = m.node_tree.links
    p = n.get("Principled BSDF")
    coord = n.new("ShaderNodeTexCoord")
    scale = n.new("ShaderNodeVectorMath")
    scale.operation = "SCALE"
    scale.inputs[3].default_value = 1 / physical_size
    links.new(coord.outputs["UV"], scale.inputs[0])
    for channel, suffix in [("Base Color", "diff"), ("Roughness", "rough"), ("Normal", "nor_gl")]:
        node = tex(m, ASSETS / asset / (asset + "_" + suffix + "_4k.jpg"), channel != "Base Color")
        links.new(scale.outputs["Vector"], node.inputs["Vector"])
        output = node.outputs["Color"]
        if channel == "Normal":
            normal = n.new("ShaderNodeNormalMap")
            normal.inputs["Strength"].default_value = 0.55
            links.new(output, normal.inputs["Color"])
            output = normal.outputs["Normal"]
        elif channel == "Base Color" and tint and ("cloth" in name or "wool" in name):
            # The scanned textile is patterned/blue; retain its measured weave
            # normals and roughness but dye the yarn to the listing's plain linen.
            noise = n.new("ShaderNodeTexNoise")
            noise.inputs["Scale"].default_value = 120
            links.new(coord.outputs["Object"], noise.inputs["Vector"])
            ramp = n.new("ShaderNodeValToRGB")
            for e, factor in zip(ramp.color_ramp.elements, [0.66, 0.72]):
                e.color = (*(v * factor for v in tint), 1)
            links.new(noise.outputs["Fac"], ramp.inputs[0])
            output = ramp.outputs[0]
        elif channel == "Base Color" and tint:
            mix = n.new("ShaderNodeMixRGB")
            mix.blend_type = "MULTIPLY"
            mix.inputs[0].default_value = 1
            mix.inputs[2].default_value = (*tint, 1)
            links.new(output, mix.inputs[1])
            output = mix.outputs[0]
        links.new(output, p.inputs[channel])
    if "cloth" in name or "wool" in name:
        p.inputs["Sheen Weight"].default_value = 0.32
        p.inputs["Sheen Roughness"].default_value = 0.7
        # Fine looped yarn relief remains visible after recolouring the scan.
        noise = n.new("ShaderNodeTexNoise")
        noise.inputs["Scale"].default_value = 310
        noise.inputs["Detail"].default_value = 2
        links.new(coord.outputs["Object"], noise.inputs["Vector"])
        micro = n.new("ShaderNodeBump")
        micro.inputs["Distance"].default_value = 0.0014
        micro.inputs["Strength"].default_value = 0.42
        existing = p.inputs["Normal"].links[0].from_socket
        links.new(existing, micro.inputs["Normal"])
        links.new(noise.outputs["Fac"], micro.inputs["Height"])
        links.new(micro.outputs[0], p.inputs["Normal"])
    return m


def noise_mat(name, base, rough, scale, bump, color_variation=0.1):
    m = mat(name, base, rough)
    n = m.node_tree.nodes
    links = m.node_tree.links
    p = n.get("Principled BSDF")
    coord = n.new("ShaderNodeTexCoord")
    noise = n.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = scale
    noise.inputs["Detail"].default_value = 4
    links.new(coord.outputs["Object"], noise.inputs["Vector"])
    ramp = n.new("ShaderNodeValToRGB")
    for e, factor in zip(ramp.color_ramp.elements, [1 - color_variation, 1 + color_variation]):
        e.color = (*(c * factor for c in base), 1)
    links.new(noise.outputs["Fac"], ramp.inputs[0])
    links.new(ramp.outputs[0], p.inputs["Base Color"])
    bn = n.new("ShaderNodeBump")
    bn.inputs["Strength"].default_value = 0.25
    bn.inputs["Distance"].default_value = bump
    links.new(noise.outputs["Fac"], bn.inputs["Height"])
    links.new(bn.outputs[0], p.inputs["Normal"])
    return m


def mesh(name, vs, fs, material, smooth=True, parent=None):
    me = bpy.data.meshes.new(name)
    me.from_pydata(vs, [], fs)
    me.update()
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(o)
    o.parent = parent
    me.materials.append(material)
    for p in me.polygons:
        p.use_smooth = smooth
    # Local metre UVs. Curved upholstery receives explicit continuous UVs below.
    uv = me.uv_layers.new(name="Metres")
    for p in me.polygons:
        axis = max(range(3), key=lambda i: abs(p.normal[i]))
        axes = [i for i in range(3) if i != axis]
        for li in p.loop_indices:
            v = me.vertices[me.loops[li].vertex_index].co
            uv.data[li].uv = (v[axes[0]], v[axes[1]])
    return o


def box(name, p, size, material, bevel=0.005, parent=None):
    a, b, c = [s / 2 for s in size]
    vs = [
        (-a, -b, -c),
        (a, -b, -c),
        (a, b, -c),
        (-a, b, -c),
        (-a, -b, c),
        (a, -b, c),
        (a, b, c),
        (-a, b, c),
    ]
    fs = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    o = mesh(name, vs, fs, material, False, parent)
    o.location = p
    if bevel:
        mod = o.modifiers.new("Soft manufactured edges", "BEVEL")
        mod.width = bevel
        mod.segments = 4
        mod = o.modifiers.new("Weighted corner normals", "WEIGHTED_NORMAL")
        mod.keep_sharp = True
    return o


def group(name, p=(0, 0, 0), angle=0):
    o = bpy.data.objects.new(name, None)
    bpy.context.collection.objects.link(o)
    o.location = p
    o.rotation_euler.z = angle
    return o


def spow(x, p):
    return math.copysign(abs(x) ** p, x)


def soft(name, p, size, material, exp=0.4, bend=0, parent=None, wrinkles=0):
    vs = []
    fs = []
    nu = 96
    nv = 48
    a, b, c = [s / 2 for s in size]
    for i in range(nv + 1):
        lat = -math.pi / 2 + math.pi * i / nv
        cl = spow(math.cos(lat), exp)
        for j in range(nu):
            lon = j * math.tau / nu
            x = a * cl * spow(math.cos(lon), exp)
            y = b * cl * spow(math.sin(lon), exp)
            z = c * spow(math.sin(lat), exp)
            z += (
                wrinkles
                * (math.sin(31 * x + 5 * y) * math.sin(19 * y) + 0.5 * math.sin(54 * x - 9 * y))
                * cl**4
            )
            vs.append((x, y + bend * (x / a) ** 2, z))
            if i:
                k = (i - 1) * nu + j
                kk = (i - 1) * nu + (j + 1) % nu
                fs.append((k, kk, i * nu + (j + 1) % nu, i * nu + j))
    o = mesh(name, vs, fs, material, True, parent)
    o.location = p
    return o


def curve(name, points, radius, material, parent=None):
    cu = bpy.data.curves.new(name, "CURVE")
    cu.dimensions = "3D"
    cu.resolution_u = 2
    cu.bevel_depth = radius
    cu.bevel_resolution = 3
    s = cu.splines.new("POLY")
    s.points.add(len(points) - 1)
    for pt, co in zip(s.points, points):
        pt.co = (*co, 1)
    o = bpy.data.objects.new(name, cu)
    bpy.context.collection.objects.link(o)
    o.parent = parent
    cu.materials.append(material)
    return o


def lathe(name, profile, material, p=(0, 0, 0), parent=None, n=96):
    vs = []
    fs = []
    for i, (z, r) in enumerate(profile):
        for j in range(n):
            a = j * math.tau / n
            vs.append((r * math.cos(a), r * math.sin(a), z))
            if i:
                fs.append(
                    ((i - 1) * n + j, (i - 1) * n + (j + 1) % n, i * n + (j + 1) % n, i * n + j)
                )
    o = mesh(name, vs, fs, material, True, parent)
    o.location = p
    return o


def light(name, p, target, energy, color, size, size_y=None):
    d = bpy.data.lights.new(name, "AREA")
    d.energy = energy
    d.color = color
    d.specular_factor = 0
    d.transmission_factor = 0
    d.shape = "RECTANGLE"
    d.size = size
    d.size_y = size_y if size_y else size
    o = bpy.data.objects.new(name, d)
    bpy.context.collection.objects.link(o)
    o.location = p
    o.visible_glossy = False  # Invisible photographic fill; fixtures themselves still reflect.
    o.rotation_euler = (Vector(target) - o.location).to_track_quat("-Z", "Y").to_euler()
    return o


def onyx():
    # Original AI-generated flat material swatch; geometry/lighting/video are 3-D.
    im = bpy.data.images.load(
        str(ROOT / "examples/villa_anahid/offline/assets/champagne-onyx-v1.png")
    )
    m = mat("Champagne onyx / polished and backlit", (0.6, 0.4, 0.2), 0.25)
    nodes = m.node_tree.nodes
    links = m.node_tree.links
    p = nodes.get("Principled BSDF")
    ref = group("Onyx continuous texture space", (0.425, 0, 0))
    tc = nodes.new("ShaderNodeTexCoord")
    tc.object = ref
    sep = nodes.new("ShaderNodeSeparateXYZ")
    links.new(tc.outputs["Object"], sep.inputs[0])
    comb = nodes.new("ShaderNodeCombineXYZ")
    links.new(sep.outputs["X"], comb.inputs["X"])
    links.new(sep.outputs["Z"], comb.inputs["Y"])
    scale = nodes.new("ShaderNodeVectorMath")
    scale.operation = "SCALE"
    scale.inputs[3].default_value = 0.245
    links.new(comb.outputs[0], scale.inputs[0])
    image = nodes.new("ShaderNodeTexImage")
    image.image = im
    links.new(scale.outputs[0], image.inputs[0])
    links.new(image.outputs[0], p.inputs["Base Color"])
    links.new(image.outputs[0], p.inputs["Emission Color"])
    p.inputs["Emission Strength"].default_value = 0.14
    p.inputs["Subsurface Weight"].default_value = 0.09
    p.inputs["Subsurface Radius"].default_value = (0.035, 0.02, 0.008)
    p.inputs["Coat Weight"].default_value = 0.22
    p.inputs["Coat Roughness"].default_value = 0.2
    return m


def materials():
    M["wool"] = pbr("Ivory boucl\u00e9 wool cloth", "wool_boucle", 0.55, (1.0, 0.95, 0.83))
    M["linen"] = pbr("Sand linen cloth", "rough_linen", 0.7, (0.76, 0.66, 0.53))
    M["rug"] = pbr("Warm grey wool cloth rug", "rough_linen", 0.65, (0.43, 0.37, 0.29))
    M["oak"] = pbr("Natural pale oak", "white_oak_veneer", 1.8, (0.65, 0.47, 0.29))
    M["plaster"] = noise_mat(
        "Warm white lime plaster", (0.73, 0.72, 0.66), 0.85, 140, 0.0006, 0.025
    )
    M["stone"] = noise_mat("Honed limestone", (0.53, 0.50, 0.43), 0.39, 2.6, 0.001, 0.15)
    M["ebony"] = noise_mat("Ebonised walnut", (0.012, 0.009, 0.007), 0.27, 8, 0.00035, 0.4)
    M["black"] = mat("Black anodised aluminium", (0.009, 0.011, 0.012), 0.27, 0.65)
    M["bronze"] = mat("Brushed dark bronze", (0.17, 0.095, 0.044), 0.26, 0.82)
    M["seam"] = mat("Ivory thread", (0.53, 0.48, 0.38), 0.95)
    M["paper"] = noise_mat("Warm uncoated paper", (0.67, 0.64, 0.56), 0.82, 130, 0.0003, 0.04)
    M["onyx"] = onyx()
    M["light"] = mat("2700K concealed diffuser", (1, 0.8, 0.5), 0.4)
    p = M["light"].node_tree.nodes.get("Principled BSDF")
    p.inputs["Emission Color"].default_value = (1, 0.61, 0.27, 1)
    p.inputs["Emission Strength"].default_value = 5
    M["leaf"] = noise_mat("Olive leaves", (0.07, 0.11, 0.024), 0.48, 35, 0.0003, 0.3)
    p = M["leaf"].node_tree.nodes.get("Principled BSDF")
    p.inputs["Subsurface Weight"].default_value = 0.15
    M["bark"] = noise_mat("Olive twigs", (0.085, 0.054, 0.025), 0.9, 75, 0.001, 0.4)
    M["soil"] = noise_mat("Deep planting substrate", (0.014, 0.023, 0.007), 1, 20, 0.02, 0.5)
    M["ceramic"] = noise_mat("Chalk glazed stoneware", (0.58, 0.55, 0.47), 0.25, 90, 0.0007, 0.08)
    M["water"] = mat("Pool water", (0.25, 0.64, 0.7), 0.07)
    p = M["water"].node_tree.nodes.get("Principled BSDF")
    p.inputs["Transmission Weight"].default_value = 1
    p.inputs["IOR"].default_value = 1.333
    n = M["water"].node_tree.nodes
    links = M["water"].node_tree.links
    noise = n.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = 8
    noise.inputs["Detail"].default_value = 2
    coord = n.new("ShaderNodeTexCoord")
    links.new(coord.outputs["Object"], noise.inputs["Vector"])
    b = n.new("ShaderNodeBump")
    b.inputs["Strength"].default_value = 0.24
    b.inputs["Distance"].default_value = 0.038
    links.new(noise.outputs["Fac"], b.inputs["Height"])
    links.new(b.outputs[0], p.inputs["Normal"])
    M["pooltile"] = noise_mat("Pool pale blue mosaic", (0.18, 0.4, 0.42), 0.28, 90, 0.001, 0.2)
    M["glass"] = mat("Architectural clear glazing", (0.98, 0.99, 1), 0.035)
    p = M["glass"].node_tree.nodes.get("Principled BSDF")
    p.inputs["Transmission Weight"].default_value = 1
    p.inputs["IOR"].default_value = 1.46


def sofa(p, angle, name):
    o = group(name, p, angle)
    o["faces_local"] = "-Y"
    soft(
        "Recessed shadow plinth", (0, -0.05, 0.075), (3.20, 1.04, 0.12), M["black"], 0.24, -0.22, o
    )
    soft(
        "Sewn upholstered foundation",
        (0, -0.07, 0.235),
        (3.50, 1.18, 0.35),
        M["wool"],
        0.25,
        -0.23,
        o,
    )
    soft(
        "Single sprung crescent seat",
        (0, -0.14, 0.455),
        (3.05, 0.93, 0.235),
        M["wool"],
        0.37,
        -0.22,
        o,
        0.002,
    )
    # One continuous full-height tailored shell, with softly rounded ends.
    vs = []
    fs = []
    steps = 160
    ring = 40
    for i in range(steps + 1):
        t = -1.66 + 3.32 * i / steps
        x = 1.54 * math.sin(t)
        y = 0.47 * math.cos(t)
        top = 0.97 - 0.15 * abs(math.sin(t)) ** 8
        bottom = 0.075
        mid = (top + bottom) / 2
        for j in range(ring):
            a = j * math.tau / ring
            d = 0.165 * spow(math.cos(a), 0.34)
            vs.append(
                (
                    x + math.sin(t) * d,
                    y + math.cos(t) * d,
                    mid + (top - bottom) / 2 * spow(math.sin(a), 0.30),
                )
            )
        if i:
            for j in range(ring):
                fs.append(
                    (
                        (i - 1) * ring + j,
                        (i - 1) * ring + (j + 1) % ring,
                        i * ring + (j + 1) % ring,
                        i * ring + j,
                    )
                )
    for side, idx in [(-1, 0), (1, steps * ring)]:
        previous = idx
        t = side * 1.66
        cx = 1.54 * math.sin(t)
        cy = 0.47 * math.cos(t)
        top = 0.82
        mid = (top + 0.075) / 2
        for k in range(1, 13):
            a = k * math.pi / 24
            start = len(vs)
            for j in range(ring):
                q = j * math.tau / ring
                d = 0.165 * spow(math.cos(q), 0.34) * math.cos(a)
                vs.append(
                    (
                        cx + math.sin(t) * d,
                        cy + math.cos(t) * d - 0.22 * math.sin(a),
                        mid + (top - 0.075) / 2 * spow(math.sin(q), 0.30) * math.cos(a),
                    )
                )
            for j in range(ring):
                fs.append(
                    (previous + j, previous + (j + 1) % ring, start + (j + 1) % ring, start + j)
                )
            previous = start
    shell = mesh("Continuous upholstered back / rounded arms", vs, fs, M["wool"], True, o)
    # Tangential UVs in metres prevent fabric scale from changing around the shell.
    for poly in shell.data.polygons:
        for li in poly.loop_indices:
            vi = shell.data.loops[li].vertex_index
            v = shell.data.vertices[vi].co
            shell.data.uv_layers[0].data[li].uv = (math.atan2(v.x / 1.54, v.y / 0.47) * 1.25, v.z)
    for i, x in enumerate([-0.96, -0.34, 0.31, 0.94]):
        y = 0.23 - 0.14 * (x / 1.5) ** 2
        cushion = soft(
            "Round feather-filled back cushion",
            (x, y, 0.747 + R.uniform(-0.022, 0.02)),
            (0.48, 0.19, 0.48),
            M["wool"],
            R.uniform(0.72, 0.93),
            0,
            o,
            0.002,
        )
        for vertex in cushion.data.vertices:
            v = vertex.co
            r = math.hypot(v.x / 0.24, v.z / 0.24)
            a = math.atan2(v.z, v.x)
            v.y += 0.006 * math.sin(a * 19 + i * 0.8) * math.exp(-(((r - 0.82) / 0.14) ** 2))
            v.z = max(v.z, -0.219) + 0.003 * math.sin(a * 5 + i)
        cushion.rotation_euler = (
            math.radians(-10),
            math.radians(R.uniform(-8, 8)),
            math.radians(R.uniform(-12, 12)),
        )
        # Fine circular seam lies in the cushion's own local plane.
        points = [
            (0.229 * math.cos(a), -0.044, 0.229 * math.sin(a))
            for a in np.linspace(0, math.tau, 145)
        ]
        curve("Cushion hand-stitched perimeter", points, 0.0016, M["seam"], cushion)
    points = []
    for t in np.linspace(0, math.tau, 193):
        x = 1.505 * spow(math.cos(t), 0.37)
        y = 0.453 * spow(math.sin(t), 0.37) - 0.14 - 0.22 * (x / 1.525) ** 2
        points.append((x, y, 0.445))
    curve("Fine seat edge stitching", points, 0.0023, M["seam"], o)
    return o


def append_model(name):
    path = ASSETS / name / (name + "_2k.blend")
    with bpy.data.libraries.load(str(path), link=False) as (src, dst):
        dst.objects = src.objects
    result = []
    for o in dst.objects:
        if o is not None:
            bpy.context.collection.objects.link(o)
            result.append(o)
    # Blender resolves appended image paths; make them portable before packing.
    for im in bpy.data.images:
        if im.source == "FILE" and im.filepath:
            if Path(im.filepath).name.startswith(name):
                im.filepath = str(ASSETS / name / "textures" / Path(im.filepath).name)
    return result


def props():
    soft("Bound handwoven wool rug", (0, -0.15, 0.009), (6.8, 5.25, 0.018), M["rug"], 0.14)
    # Dense short wool pile; geometry stays below the scale of individual stitches.
    vs = []
    fs = []
    for i in range(38000):
        x = R.uniform(-3.37, 3.37)
        y = R.uniform(-2.76, 2.46)
        z = 0.018
        h = R.uniform(0.0008, 0.002)
        w = 0.00025
        k = len(vs)
        vs.extend(
            [
                (x - w, y, z),
                (x + w, y, z),
                (x + 0.001, y + 0.001, z + h),
                (x, y + 0.002, z + h * 0.7),
            ]
        )
        fs.append((k, k + 1, k + 2, k + 3))
    mesh("Short irregular wool rug pile", vs, fs, M["rug"], False)
    # Individually cut fringes, only along the short edges.
    for side in [-1, 1]:
        for x in np.linspace(-3.32, 3.32, 360):
            y = side * 2.62 - 0.15
            curve(
                "Rug cotton fringe",
                [
                    (x, y, 0.026),
                    (x + R.uniform(-0.01, 0.01), y + side * R.uniform(0.035, 0.065), 0.019),
                ],
                0.0009,
                M["seam"],
            )
    sofa((-0.5, 1.35, 0.048), -0.10, "Courtyard-facing crescent sofa")
    sofa((0.55, -1.67, 0.048), math.pi - 0.06, "Fireplace-facing crescent sofa")
    table = group("Sculptural oval walnut coffee table", (0, -0.10, 0.055), 0.08)
    soft(
        "Thin oval walnut tabletop", (0, 0, 0.365), (2.35, 0.95, 0.055), M["ebony"], 0.96, 0, table
    )
    for x in [-0.73, 0.72]:
        lathe(
            "Turned dark walnut pedestal",
            [(0.008, 0.13), (0.035, 0.14), (0.10, 0.115), (0.32, 0.105), (0.355, 0.15)],
            M["ebony"],
            (x, 0, 0),
            table,
        )
    for i, angle in enumerate([-0.11, 0.06]):
        g = group("Linen-bound architecture book")
        g.parent = table
        g.location = (-0.53, -0.06, 0.409 + i * 0.052)
        g.rotation_euler.z = angle
        box("Linen book cover", (0, 0, -0.019), (0.39, 0.28, 0.006), M["linen"], 0.003, g)
        box("Visible paper signatures", (0, 0, 0), (0.378, 0.265, 0.033), M["paper"], 0.002, g)
        box("Linen book cover", (0, 0, 0.019), (0.39, 0.28, 0.006), M["linen"], 0.003, g)
        lettering = bpy.data.curves.new("Book cover typography", "FONT")
        lettering.body = "SPACES" if i else "LIGHT / FORM"
        lettering.size = 0.023
        lettering.align_x = "CENTER"
        lettering.space_character = 1.2
        lettering.extrude = 0.00012
        lettering.materials.append(M["ebony"])
        title = bpy.data.objects.new("Embossed architecture book title", lettering)
        bpy.context.collection.objects.link(title)
        title.parent = g
        title.location = (0, 0.058, 0.0222)
        box("Cover architectural rule", (0, 0.037, 0.0225), (0.22, 0.001, 0.0004), M["ebony"], 0, g)
        for z in np.linspace(-0.011, 0.01, 7):
            box("Book page deckle", (0, -0.134, z), (0.36, 0.001, 0.0005), M["seam"], 0, g)
    lathe(
        "Hand-thrown shallow bowl",
        [
            (0, 0.025),
            (0.025, 0.095),
            (0.055, 0.145),
            (0.09, 0.16),
            (0.10, 0.157),
            (0.094, 0.146),
            (0.046, 0.105),
            (0.035, 0.01),
        ],
        M["ceramic"],
        (0.13, -0.23, 0.447),
    )
    imported = append_model("ceramic_vase_03")
    for ob in imported:
        ob.location = (0.52, 0.035, 0.445)
        ob.scale = (1.85, 1.85, 1.65)
    # Branches, connected secondary stems, and individually shaped olive leaves.
    for i in range(7):
        start = Vector((0.52, 0.035, 0.95))
        end = Vector(
            (0.52 + R.uniform(-0.45, 0.45), 0.035 + R.uniform(-0.23, 0.23), R.uniform(1.48, 1.83))
        )
        mid = start.lerp(end, 0.52) + Vector((R.uniform(-0.08, 0.08), 0.025, 0.03))
        curve("Olive arrangement main branch", [start, mid, end], 0.003, M["bark"])
        for t in [0.35, 0.50, 0.66, 0.82, 0.95]:
            center = start.lerp(end, t)
            for side in [-1, 1]:
                e = center + Vector(
                    (side * R.uniform(0.06, 0.17), R.uniform(-0.10, 0.10), R.uniform(0.015, 0.075))
                )
                curve("Fine connected olive twig", [center, e], 0.0014, M["bark"])
                for q in [0.55, 1]:
                    c = center.lerp(e, q)
                    ang = R.uniform(0, math.tau)
                    a = Vector((math.cos(ang), math.sin(ang), 0.25)) * 0.055
                    b = Vector((-math.sin(ang), math.cos(ang), 0.1)) * 0.012
                    mesh(
                        "Folded olive leaf",
                        [c - a, c + b, c + a, c - b, c + Vector((0, 0, 0.005))],
                        [(0, 1, 4), (1, 2, 4), (2, 3, 4), (3, 0, 4)],
                        M["leaf"],
                    )
    # A restrained reading vignette against the side wall.
    box("Floating oak sideboard", (-5.91, -0.5, 0.48), (0.57, 3.5, 0.63), M["oak"], 0.012)
    for y in np.linspace(-1.80, 0.8, 5):
        box("Sideboard door gap", (-5.614, y, 0.5), (0.004, 0.009, 0.53), M["black"], 0)
    for ob in append_model("potted_plant_02"):
        ob.location = (4.9, 3.7, 0)
        ob.scale = (1.8, 1.8, 1.8)
        if "pot" in ob.name and "leaves" not in ob.name and "dirt" not in ob.name:
            ob.data.materials.clear()
            ob.data.materials.append(M["ceramic"])


def architecture():
    box("Limestone structural slab", (0, 1, -0.17), (14, 16, 0.30), M["stone"], 0.008)
    for ix in range(-5, 6):
        for iy in range(-4, 9):
            o = box(
                "Honed limestone / cut tile",
                (ix * 1.2, iy * 1.2, -0.006),
                (1.197, 1.197, 0.025),
                M["stone"],
                0.0015,
            )
            # Offset the procedural texture origin without affecting tile alignment.
            for poly in o.data.polygons:
                for li in poly.loop_indices:
                    o.data.uv_layers[0].data[li].uv += Vector((R.uniform(0, 8), R.uniform(0, 8)))
    box("West plaster wall", (-6.45, 0, 1.95), (0.20, 10, 3.9), M["plaster"], 0.008)
    box("Ceiling", (0, 0, 3.95), (13.1, 10, 0.18), M["plaster"], 0.004)
    # Pocketing glass is retracted: a real open corner towards the pool terrace.
    for x in [-6.2, -6.09, -5.98]:
        box("Retracted glass pocket", (x, 3.8, 1.6), (0.02, 1.5, 3.15), M["glass"], 0.001)
    for z in [0.02, 3.18]:
        box("Dark glazing track", (0, 4.35, z), (13.0, 0.075, 0.045), M["black"], 0.004)
    box("Clerestory fascia", (0, 4.35, 3.38), (13.0, 0.20, 0.36), M["plaster"], 0.004)
    box("Clerestory glazing", (0, 4.35, 3.73), (13.0, 0.012, 0.34), M["glass"], 0.001)
    box("Clerestory top rail", (0, 4.35, 3.89), (13.0, 0.045, 0.024), M["black"], 0.003)
    for x in [-6.4, 6.4]:
        box("Minimal corner mullion", (x, 4.35, 1.94), (0.05, 0.055, 3.9), M["black"], 0.003)
    # The right return is a glazed wall, opening to a planted side courtyard.
    # Main panels fully retracted, like the photographed open indoor/outdoor corner.
    for y in [-4.80, -4.70, -4.60]:
        box("Right retracted glass pocket", (6.43, y, 1.60), (1.5, 0.012, 3.15), M["glass"], 0.001)
    for z in [0.03, 3.2, 3.89]:
        box(
            "Right glazing horizontal frame", (6.43, -0.3, z), (0.05, 9.2, 0.032), M["black"], 0.002
        )
    box("Right clerestory fascia", (6.43, -0.3, 3.4), (0.18, 9.2, 0.37), M["plaster"], 0.004)
    box("Right clerestory glass", (6.43, -0.3, 3.74), (0.012, 9.2, 0.31), M["glass"], 0.001)
    for y in [-4.9, 4.3]:
        box("Right glazing vertical frame", (6.43, y, 1.6), (0.047, 0.047, 3.2), M["black"], 0.002)
    # Stone pier with a real recessed firebox and continuous texture across slabs.
    x = 2.1
    y = 3.91
    w = 3.35
    box("Onyx lower plinth", (x, y, 0.305), (w, 0.36, 0.61), M["onyx"], 0.006)
    box("Onyx overmantel", (x, y, 2.49), (w, 0.36, 2.82), M["onyx"], 0.006)
    for dx in [-1, 1]:
        box(
            "Onyx fireplace jamb",
            (x + dx * (w / 2 - 0.16), y, 0.845),
            (0.32, 0.36, 0.47),
            M["onyx"],
            0.004,
        )
    box("Firebox dark interior", (x, y + 0.09, 0.845), (w - 0.62, 0.07, 0.47), M["black"], 0.005)
    box("Dark hearth bed", (x, y - 0.04, 0.625), (w - 0.60, 0.28, 0.025), M["black"], 0.005)
    # Small mineral stones and flame tongues, modelled in 3-D, static for camera study.
    for i in range(90):
        xx = x + R.uniform(-1.27, 1.27)
        yy = y + R.uniform(-0.135, 0.065)
        soft(
            "Fireplace black river stone",
            (xx, yy, 0.654),
            (R.uniform(0.025, 0.055), 0.028, 0.019),
            M["ebony"],
            0.8,
        )
    flame = mat("Warm gas flame", (1, 0.32, 0.015), 0.5)
    p = flame.node_tree.nodes.get("Principled BSDF")
    p.inputs["Emission Color"].default_value = (1, 0.36, 0.025, 1)
    p.inputs["Emission Strength"].default_value = 5
    for i in range(60):
        xx = x - 1.24 + i * 2.48 / 59
        hh = R.uniform(0.025, 0.13)
        vs = []
        fs = []
        for j in range(7):
            t = j / 6
            cx = xx + 0.017 * math.sin(t * 4 + i)
            rr = 0.009 * (1 - t) ** 0.55
            for k in range(5):
                a = k * math.tau / 5
                vs.append((cx + rr * math.cos(a), y - 0.105 + rr * math.sin(a), 0.67 + t * hh))
            if j:
                for k in range(5):
                    fs.append(
                        ((j - 1) * 5 + k, (j - 1) * 5 + (k + 1) % 5, j * 5 + (k + 1) % 5, j * 5 + k)
                    )
        mesh("Fine natural gas flame", vs, fs, flame)
    light("Hearth warm bounce", (x, y - 0.25, 0.79), (x, 1, 0.4), 35, (1, 0.48, 0.15), 2.5, 0.16)
    # Recessed trim, lights and AC slots have geometric depth.
    for y in [-3.3, 0.25, 2.7]:
        for x in [-4.4, 0, 4.5]:
            box("Twin downlight recess", (x, y, 3.848), (0.34, 0.16, 0.026), M["black"], 0.012)
            for dx in [-0.095, 0.095]:
                lathe(
                    "Recessed optic", [(0, 0.038), (0.014, 0.038)], M["light"], (x + dx, y, 3.825)
                )
            light("Warm ceiling pool", (x, y, 3.82), (x, y, 0), 34, (1, 0.73, 0.46), 0.28, 0.13)
    for y in [-2.5, 1.5]:
        box("Linear air supply", (-5.4, y, 3.853), (0.024, 2.2, 0.012), M["black"], 0.002)
    for x in [-6.24, 6.24]:
        box("Cove shadow channel", (x, 0, 3.835), (0.10, 9.5, 0.075), M["black"], 0.003)
        box("Concealed warm cove", (x, 0, 3.812), (0.016, 9.35, 0.01), M["light"], 0.002)


def garden():
    box("Garden background wall", (0, 13, 2.65), (19, 0.22, 5.3), M["soil"], 0.01)
    box("Planted side courtyard enclosure", (9.5, 3.7, 2.7), (0.22, 20, 5.4), M["soil"], 0.01)
    box("Side courtyard paving", (8.1, 2.0, -0.02), (3.4, 17, 0.12), M["stone"], 0.012)
    ferns = append_model("fern_02")
    # Independent orientation / scale; linked mesh data keeps this dense planting modest.
    for row in range(8):
        for col in range(28):
            source = ferns[(row * 7 + col) % len(ferns)]
            o = source.copy()
            o.data = source.data
            bpy.context.collection.objects.link(o)
            o.location = (
                -9.2 + col * 0.68 + R.uniform(-0.17, 0.17),
                12.8 - R.uniform(0, 0.25),
                0.25 + row * 0.65 + R.uniform(-0.20, 0.20),
            )
            from mathutils import Quaternion

            o.rotation_mode = "QUATERNION"
            o.rotation_quaternion = Vector((0, -1, 0.05)).to_track_quat("Z", "Y") @ Quaternion(
                (0, 0, 1), R.random() * math.tau
            )
            s = R.uniform(1.15, 1.95)
            o.scale = (s, s, s)
    for row in range(7):
        for col in range(27):
            source = ferns[(row + col) % len(ferns)]
            o = source.copy()
            o.data = source.data
            bpy.context.collection.objects.link(o)
            o.location = (
                9.32 - R.uniform(0, 0.15),
                -6 + col * 0.72 + R.uniform(-0.12, 0.12),
                0.35 + row * 0.77 + R.uniform(-0.17, 0.17),
            )
            o.rotation_mode = "QUATERNION"
            o.rotation_quaternion = Vector((-1, 0, 0.08)).to_track_quat("Z", "Y") @ Quaternion(
                (0, 0, 1), R.random() * math.tau
            )
            s = R.uniform(1.4, 2.0)
            o.scale = (s, s, s)
    for o in ferns:
        bpy.data.objects.remove(o, do_unlink=True)
    # Pool with depth and independently modelled coping.
    box("Pool blue basin", (3.7, 8.7, -0.62), (6.4, 7, 0.10), M["pooltile"], 0.02)
    for x in [0.45, 6.95]:
        box("Pool coping edge", (x, 8.7, 0.02), (0.22, 7.2, 0.18), M["stone"], 0.015)
    for y in [5.15, 12.25]:
        box("Pool coping edge", (3.7, y, 0.02), (6.7, 0.22, 0.18), M["stone"], 0.015)
    # Remove the floor tile coverage beneath water so its depth remains visible.
    for o in list(bpy.data.objects):
        if (
            o.name.startswith("Honed limestone / cut tile")
            and 0.4 < o.location.x < 7
            and 5.15 < o.location.y < 12.25
        ):
            bpy.data.objects.remove(o, do_unlink=True)
    box("Pool water body", (3.7, 8.7, -0.23), (6.2, 6.9, 0.56), M["water"], 0)
    # Pool occupies an opening in the deck; slab pieces replace the old solid slab.
    old = bpy.data.objects.get("Limestone structural slab")
    bpy.data.objects.remove(old, do_unlink=True)
    for p, s in [
        ((0, 0.04, -0.17), (14, 10.2, 0.30)),
        ((-3.4, 8.7, -0.17), (7.5, 7.2, 0.30)),
        ((7.45, 8.7, -0.17), (1, 7.2, 0.30)),
    ]:
        box("Deck substructure", p, s, M["stone"], 0.003)
    # Teak terrace bench with real slats and cushions.
    for x in [-5.6, -3.0]:
        for y in [7.1, 8.05]:
            box("Teak bench leg", (x, y, 0.29), (0.065, 0.065, 0.58), M["oak"], 0.01)
        box("Teak bench arm", (x, 7.57, 0.65), (0.10, 1.12, 0.09), M["oak"], 0.014)
    for y in np.arange(7.1, 8.11, 0.10):
        box("Teak seat slat", (-4.3, float(y), 0.37), (2.75, 0.067, 0.045), M["oak"], 0.008)
    for z in [0.54, 0.69, 0.84]:
        box("Teak back rail", (-4.3, 8.08, z), (2.76, 0.035, 0.095), M["oak"], 0.012)
    for x in [-5.0, -3.6]:
        soft("Terrace seat cushion", (x, 7.58, 0.48), (1.28, 0.87, 0.19), M["linen"], 0.4)
        soft("Terrace back cushion", (x, 7.92, 0.79), (1.27, 0.20, 0.53), M["wool"], 0.44)
    # Small leaves soften the garden wall edges in front of the dense fern carpet.
    for ob in append_model("potted_plant_02"):
        ob.location = (-6.0, 10.6, 0)
        ob.scale = (3.5, 3.5, 3.5)


SHOTS = [
    {
        "name": "01 / living room dolly",
        "first": 1,
        "last": 120,
        "start": [-5.3, -5.15, 1.82],
        "end": [-4.45, -4.8, 1.74],
        "target_start": [0.30, 0.45, 1.48],
        "target_end": [0.38, 0.65, 1.43],
        "lens": 28,
        "fstop": 8,
    },
    {
        "name": "02 / upholstery and objects",
        "first": 121,
        "last": 192,
        "start": [-2.8, -0.83, 1.15],
        "end": [-2.4, -0.66, 1.17],
        "target_start": [0.18, 0.25, 0.78],
        "target_end": [0.27, 0.28, 0.8],
        "lens": 50,
        "fstop": 4.5,
    },
    {
        "name": "03 / courtyard and hearth",
        "first": 193,
        "last": 288,
        "start": [-4.9, -2.8, 2.15],
        "end": [-4.35, -2.35, 2.10],
        "target_start": [1.8, 3.15, 1.72],
        "target_end": [2.0, 3.5, 1.75],
        "lens": 30,
        "fstop": 8,
    },
]


def camera_at(frame):
    shot = next(s for s in SHOTS if s["first"] <= frame <= s["last"])
    t = (frame - shot["first"]) / max(1, shot["last"] - shot["first"])
    t = t * t * (3 - 2 * t)
    p = Vector(shot["start"]).lerp(Vector(shot["end"]), t)
    target = Vector(shot["target_start"]).lerp(Vector(shot["target_end"]), t)
    camera = bpy.context.scene.camera
    camera.location = p
    camera.rotation_euler = (target - p).to_track_quat("-Z", "Y").to_euler()
    camera.data.lens = shot["lens"]
    camera.data.dof.aperture_fstop = shot["fstop"]
    camera.data.dof.focus_distance = (target - p).length


def setup():
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    prefs = bpy.context.preferences.addons["cycles"].preferences
    prefs.compute_device_type = "METAL"
    prefs.refresh_devices()
    devices = []
    for d in prefs.devices:
        d.use = d.type == "METAL"
        devices.append({"name": d.name, "type": d.type, "enabled": d.use})
    scene.cycles.device = "GPU"
    scene.cycles.samples = 128
    scene.cycles.use_denoising = True
    scene.cycles.denoiser = "OPENIMAGEDENOISE"
    scene.cycles.use_adaptive_sampling = True
    scene.cycles.adaptive_threshold = 0.018
    scene.cycles.adaptive_min_samples = 32
    scene.cycles.max_bounces = 12
    scene.cycles.diffuse_bounces = 5
    scene.cycles.glossy_bounces = 6
    scene.cycles.transmission_bounces = 8
    scene.cycles.transparent_max_bounces = 16
    scene.cycles.sample_clamp_indirect = 4
    scene.cycles.use_light_tree = True
    scene.render.use_persistent_data = True
    scene.cycles.seed = 349
    scene.cycles.use_animated_seed = False
    scene.render.resolution_x = 1920
    scene.render.resolution_y = 1080
    scene.render.resolution_percentage = 100
    scene.render.fps = 24
    scene.frame_start = 1
    scene.frame_end = 288
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.image_settings.color_depth = "8"
    scene.view_settings.view_transform = "AgX"
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.view_settings.exposure = -0.5
    world = bpy.data.worlds.new("Soft afternoon sky")
    world.use_nodes = True
    scene.world = world
    n = world.node_tree.nodes
    links = world.node_tree.links
    hdr = n.new("ShaderNodeTexEnvironment")
    hdr.image = bpy.data.images.load(str(ROOT / "client/app/assets/kloppenheim_06_puresky_2k.hdr"))
    tc = n.new("ShaderNodeTexCoord")
    mapping = n.new("ShaderNodeMapping")
    mapping.inputs["Rotation"].default_value[2] = 0.8
    links.new(tc.outputs["Generated"], mapping.inputs["Vector"])
    links.new(mapping.outputs["Vector"], hdr.inputs["Vector"])
    links.new(hdr.outputs[0], n.get("Background").inputs["Color"])
    n.get("Background").inputs["Strength"].default_value = 0.38
    # Open-corner daylight comes from the real sky dome and sun. No studio softboxes.
    n.get("Background").inputs["Strength"].default_value = 0.8
    d = bpy.data.lights.new("Late afternoon sun", "SUN")
    d.energy = 1.25
    d.angle = 0.12
    d.color = (1, 0.86, 0.65)
    sun = bpy.data.objects.new("Late afternoon sun", d)
    bpy.context.collection.objects.link(sun)
    sun.rotation_euler = (math.radians(48), math.radians(-25), math.radians(-38))
    cam = bpy.data.objects.new(
        "Architectural cinema camera", bpy.data.cameras.new("Architectural cinema camera")
    )
    bpy.context.collection.objects.link(cam)
    scene.camera = cam
    cam.data.sensor_width = 36
    cam.data.clip_start = 0.03
    cam.data.clip_end = 150
    cam.data.dof.use_dof = True
    cam.data.dof.aperture_blades = 9
    # Dense camera keyframes make the saved .blend fully playable without this script.
    for frame in range(1, 289):
        camera_at(frame)
        cam.keyframe_insert("location", frame=frame)
        cam.keyframe_insert("rotation_euler", frame=frame)
        cam.data.keyframe_insert("lens", frame=frame)
        cam.data.dof.keyframe_insert("focus_distance", frame=frame)
        cam.data.dof.keyframe_insert("aperture_fstop", frame=frame)
    for shot in SHOTS:
        scene.timeline_markers.new(shot["name"], frame=shot["first"])
    scene.frame_set(1)
    return devices


def main():
    global OUT
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=OUT)
    parser.add_argument("--preview", type=int)
    parser.add_argument("--samples", type=int, default=64)
    parser.add_argument("--width", type=int, default=960)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])
    OUT = args.output.resolve()
    OUT.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    materials()
    architecture()
    props()
    garden()
    devices = setup()
    bpy.ops.file.pack_all()
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "living-room-cycles.blend"), compress=True)
    metadata = {
        "renderer": "Cycles / Metal",
        "devices": devices,
        "shots": SHOTS,
        "fps": 24,
        "frames": 288,
        "scene": "living-room-cycles.blend",
        "scope": "Isolated, photo-informed living room art study; approximate dimensions.",
        "objects": len(bpy.context.scene.objects),
        "vertices": sum(
            len(o.data.vertices) for o in bpy.context.scene.objects if o.type == "MESH"
        ),
        "assets": "Powered by Poly Haven / CC0. See asset-provenance.json.",
    }
    (OUT / "scene-report.json").write_text(json.dumps(metadata, indent=2) + "\n")
    if args.preview:
        s = bpy.context.scene
        s.frame_set(args.preview)
        s.cycles.samples = args.samples
        s.cycles.adaptive_min_samples = min(32, args.samples)
        s.render.resolution_x = args.width
        s.render.resolution_y = round(args.width * 9 / 16)
        s.render.filepath = str(OUT / f"preview-{args.preview:04d}.png")
        start = time.monotonic()
        bpy.ops.render.render(write_still=True)
        print("RENDER_SECONDS", time.monotonic() - start, flush=True)


if __name__ == "__main__":
    main()
