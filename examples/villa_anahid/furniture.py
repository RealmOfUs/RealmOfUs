"""Original, photo-informed furniture. Coordinates are Godot metres, forward +Z.

Assemblies face a target, so callers never guess a sofa/chair yaw. All soft forms
are closed meshes; curved backs do not rely on unapplied bevel modifiers.
"""

import math

import bmesh
import bpy
import numpy as np

from examples.common.blender_primitives import location, material
from examples.villa_anahid import appearance as v
from examples.villa_anahid import architecture as h

AUDIT = []


def group(parent, name, p=(0, 0, 0), angle=0):
    ob = bpy.data.objects.new(name, None)
    bpy.context.collection.objects.link(ob)
    ob.parent = parent
    ob.location = location(p)
    ob.rotation_euler.z = angle
    return ob


def facing(parent, name, x, y, z, target):
    angle = math.atan2(target[0] - x, target[1] - z)
    ob = group(parent, name, (x, y, z), angle)
    ob["view_target_xz"] = list(target)
    ob["furniture_kind"] = name
    AUDIT.append({"name": name, "position": [x, y, z], "target": list(target), "yaw": angle})
    return ob


def mesh(parent, name, vs, fs, key, smooth=True):
    ob = v.mesh(parent, name, vs, fs, key, smooth)
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(ob.data)
    bm.free()
    return ob


def spow(t, e):
    return math.copysign(abs(t) ** e, t)


def soft(parent, name, p, size, key="fabric", exponent=0.42, bend=0, wrinkle=0):
    # Rounded cuboid / pillow with a subtly irregular sewn silhouette.
    sx, sy, sz = [s / 2 for s in size]
    vs = []
    fs = []
    nu = 40
    nv = 20
    for i in range(nv + 1):
        lat = -math.pi / 2 + math.pi * i / nv
        c = spow(math.cos(lat), exponent)
        for j in range(nu):
            lon = j * math.tau / nu
            xx = sx * c * spow(math.cos(lon), exponent)
            zz = sz * c * spow(math.sin(lon), exponent)
            yy = sy * spow(math.sin(lat), exponent)
            yy += wrinkle * math.sin(xx * 22 + zz * 9) * math.sin(zz * 17) * c * c
            vs.append((p[0] + xx, p[1] + yy, p[2] + zz + bend * (xx / sx) ** 2))
            if i:
                a = (i - 1) * nu + j
                b = (i - 1) * nu + (j + 1) % nu
                d = i * nu + j
                cidx = i * nu + (j + 1) % nu
                fs.append((a, b, cidx, d))
    return mesh(parent, name, vs, fs, key)


def tube(parent, name, points, radius, key="bronze", sides=8):
    # Parallel frame sufficient for the horizontal/vertical planar detail curves used here.
    from mathutils import Vector

    vs = []
    fs = []
    for i, p in enumerate(points):
        p = Vector(p)
        t = (
            Vector(points[min(i + 1, len(points) - 1)]) - Vector(points[max(i - 1, 0)])
        ).normalized()
        ref = Vector((0, 1, 0)) if abs(t.y) < 0.9 else Vector((0, 0, 1))
        a = t.cross(ref).normalized()
        b = t.cross(a).normalized()
        for j in range(sides):
            vs.append(
                tuple(
                    p
                    + radius
                    * (math.cos(j * math.tau / sides) * a + math.sin(j * math.tau / sides) * b)
                )
            )
        if i:
            for j in range(sides):
                fs.append(
                    (
                        (i - 1) * sides + j,
                        (i - 1) * sides + (j + 1) % sides,
                        i * sides + (j + 1) % sides,
                        i * sides + j,
                    )
                )
    fs.extend(
        [
            tuple(reversed(range(sides))),
            tuple(range((len(points) - 1) * sides, len(points) * sides)),
        ]
    )
    return mesh(parent, name, vs, fs, key)


def sofa(parent, x, y, z, w=3.5, target=(0, 0), key="fabric"):
    o = facing(parent, "Crescent sofa", x, y, z, target)
    soft(o, "Recessed dark foot", (0, 0.055, 0), (w - 0.3, 0.10, 1.05), "dark", 0.3, bend=0.27)
    soft(o, "Continuous upholstered plinth", (0, 0.255, 0), (w, 0.40, 1.24), key, 0.38, bend=0.28)
    soft(
        o,
        "Single tailored crescent seat",
        (0, 0.51, 0.12),
        (w - 0.48, 0.26, 1.01),
        key,
        0.43,
        bend=0.28,
        wrinkle=0.006,
    )
    # A thick, rounded U shaped upholstered back, capped at both arm ends.
    vs = []
    fs = []
    steps = 64
    ring = 20
    for i in range(steps + 1):
        t = -math.pi / 2 + math.pi * i / steps
        xx = math.sin(t) * (w / 2 - 0.21)
        zz = -math.cos(t) * 0.61 + 0.10
        top = 0.97 - 0.13 * abs(math.sin(t)) ** 5
        mid = (top + 0.32) / 2
        hh = (top - 0.32) / 2
        for j in range(ring):
            a = j * math.tau / ring
            out = 0.21 * spow(math.cos(a), 0.65)
            vs.append(
                (xx + math.sin(t) * out, mid + hh * spow(math.sin(a), 0.62), zz - math.cos(t) * out)
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
    # Continue the back surface into rounded arm ends. A separate overlapping
    # ellipsoid creates flickering seams, so the end caps share the boundary ring.
    for side, start in [(-1, 0), (1, steps * ring)]:
        previous = start
        cx = side * (w / 2 - 0.21)
        for k in range(1, 9):
            phi = k * math.pi / 16
            current = len(vs)
            for j in range(ring):
                a = j * math.tau / ring
                vs.append(
                    (
                        cx + side * 0.21 * spow(math.cos(a), 0.65) * math.cos(phi),
                        0.58 + 0.26 * spow(math.sin(a), 0.62) * math.cos(phi),
                        0.10 + 0.42 * math.sin(phi),
                    )
                )
            for j in range(ring):
                fs.append(
                    (previous + j, previous + (j + 1) % ring, current + (j + 1) % ring, current + j)
                )
            previous = current
    mesh(o, "Padded wraparound back and arms", vs, fs, key)
    for i, dx in enumerate([-0.30 * w, -0.1 * w, 0.1 * w, 0.30 * w]):
        pillow = soft(
            o,
            "Round boucl\u00e9 scatter cushion",
            (dx, 0.75, -0.22 + 0.24 * (dx / (w / 2)) ** 2),
            (0.43, 0.44, 0.18),
            key,
            0.85,
        )
        pillow.rotation_euler.x = 0.12 * (-1 if i % 2 else 1)
    # Fine seam catches a small highlight at the seat edge.
    pts = []
    for j in range(81):
        a = j * math.tau / 80
        xx = (w - 0.5) / 2 * spow(math.cos(a), 0.43)
        zz = 0.51 * spow(math.sin(a), 0.43)
        pts.append((xx, 0.49, zz + 0.12 + 0.28 * (xx / (w / 2)) ** 2))
    tube(o, "Seat piping", pts, 0.006, "seam", 6)
    return o


def chair(parent, x, y, z, target, high=False, key="fabric"):
    o = facing(parent, "Counter stool" if high else "Dining chair", x, y, z, target)
    seat = 0.71 if high else 0.46
    soft(o, "Upholstered seat", (0, seat, 0), (0.52, 0.14, 0.51), key, 0.5)
    for dx in [-0.20, 0.20]:
        for dz in [-0.20, 0.20]:
            h.rod(
                o,
                "Tapered oak chair leg",
                (dx * 1.14, 0.025, dz * 1.14),
                (dx, seat - 0.035, dz),
                0.025,
                "walnut",
            )
    pts = [
        (0.255 * math.sin(a), seat + 0.32, -0.23 * math.cos(a)) for a in np.linspace(-1.2, 1.2, 28)
    ]
    tube(o, "Curved solid oak back rail", pts, 0.045, "walnut", 10)
    for dx in [-0.23, 0.23]:
        h.rod(
            o, "Back upright", (dx, seat - 0.04, -0.17), (dx, seat + 0.33, -0.12), 0.022, "walnut"
        )
    if high:
        for dz in [-0.20, 0.20]:
            h.rod(o, "Footrest", (-0.20, 0.29, dz), (0.20, 0.29, dz), 0.014, "bronze")
    return o


def vase(o, x, y, z, scale=1):
    # Lathed ceramic profile, including lip and inset mouth.
    profile = [
        (0.001, 0.065),
        (0.02, 0.105),
        (0.12, 0.16),
        (0.28, 0.15),
        (0.40, 0.10),
        (0.48, 0.072),
        (0.50, 0.074),
        (0.50, 0.053),
        (0.45, 0.052),
    ]
    vs = []
    fs = []
    n = 40
    for i, (yy, rr) in enumerate(profile):
        for j in range(n):
            a = j * math.tau / n
            vs.append((x + rr * scale * math.cos(a), y + yy * scale, z + rr * scale * math.sin(a)))
            if i:
                fs.append(
                    ((i - 1) * n + j, (i - 1) * n + (j + 1) % n, i * n + (j + 1) % n, i * n + j)
                )
    mesh(o, "Hand thrown ceramic vase", vs, fs, "ceramic")
    for i in range(4):
        a = i * 2.4
        end = (
            x + math.cos(a) * 0.25 * scale,
            y + (1.0 + i * 0.06) * scale,
            z + math.sin(a) * 0.16 * scale,
        )
        h.rod(o, "Olive branch", (x, y + 0.42 * scale, z), end, 0.005 * scale, "bark")
        for t in [0.45, 0.65, 0.82]:
            p = (
                x + (end[0] - x) * t,
                y + 0.4 * scale + (end[1] - y - 0.4 * scale) * t,
                z + (end[2] - z) * t,
            )
            vv = []
            ff = []
            v.leaf(vv, ff, p, 0.13 * scale, a + i, 0.1)
            v.mesh(o, "Olive leaf", vv, ff, "leaf2", True)


def books(o, x, y, z):
    for i in range(3):
        h.box(
            o,
            "Book cover",
            (x, y + i * 0.055, z),
            (0.42, 0.016, 0.30),
            "walnut" if i == 1 else "ivory",
            0.006,
        )
        h.box(o, "Book pages", (x, y + 0.025 + i * 0.055, z), (0.407, 0.035, 0.287), "paper", 0.004)


def coffee(parent, x, y, z, w=2.0, d=0.85):
    o = group(parent, "Oval coffee table", (x, y, z))
    soft(o, "Ebonised oval top", (0, 0.39, 0), (w, 0.055, d), "ebony", 0.9)
    for dx in [-w * 0.30, w * 0.30]:
        v.cylinder(o, "Sculpted pedestal", (dx, 0.20, 0), 0.11, 0.37, h.M["ebony"], 32)
    books(o, -w * 0.24, 0.43, 0)
    vase(o, w * 0.21, 0.42, 0, 0.85)


def rug(o, x, y, z, w, d):
    soft(o, "Woven wool rug", (x, y + 0.017, z), (w, 0.034, d), "rug", 0.20)
    for side in [-1, 1]:
        h.box(
            o,
            "Bound rug edge",
            (x, y + 0.037, z + side * (d / 2 - 0.035)),
            (w - 0.12, 0.006, 0.018),
            "seam",
        )


def outdoor_sofa(parent, x, y, z, w, target):
    o = facing(parent, "Teak garden sofa", x, y, z, target)
    for dx in [-w / 2 + 0.08, w / 2 - 0.08]:
        for dz in [-0.41, 0.41]:
            h.box(o, "Teak frame foot", (dx, 0.28, dz), (0.09, 0.56, 0.09), "walnut", 0.015)
        h.box(o, "Teak arm rail", (dx, 0.66, 0), (0.11, 0.10, 1.06), "walnut", 0.022)
    for zz in np.arange(-0.42, 0.46, 0.12):
        h.box(o, "Teak seat slat", (0, 0.36, float(zz)), (w, 0.06, 0.09), "walnut", 0.012)
    for yy in [0.53, 0.68, 0.83]:
        h.box(o, "Teak back slat", (0, yy, -0.46), (w, 0.10, 0.045), "walnut", 0.012)
    for dx in [-w * 0.25, w * 0.25]:
        soft(o, "Garden seat cushion", (dx, 0.48, 0.025), (w * 0.48, 0.19, 0.84), "linen", 0.43)
        soft(o, "Garden back cushion", (dx, 0.76, -0.29), (w * 0.47, 0.52, 0.20), "linen", 0.50)
        soft(o, "Garden scatter pillow", (dx, 0.78, -0.08), (0.40, 0.38, 0.16), "throw", 0.60)
    return o


def bed(parent, x, y, z, w=1.95, angle=0):
    o = group(parent, "Bedroom / one bed", (x, y, z), angle)
    o["bed"] = True
    soft(o, "Upholstered bed base", (0, 0.19, 0), (w + 0.13, 0.35, 2.26), "fabric", 0.35)
    soft(o, "Padded headboard", (0, 0.77, -1.16), (w + 0.42, 1.39, 0.21), "fabric", 0.40)
    soft(o, "Pillow top mattress", (0, 0.43, 0), (w, 0.27, 2.12), "linen", 0.35)
    # Duvet is a continuous draped grid with irregular hem and rounded shoulder.
    vs = []
    fs = []
    nx = 48
    nz = 40
    for i in range(nx + 1):
        xx = (i / nx - 0.5) * (w + 0.26)
        side = max(0, abs(xx) - (w / 2 - 0.13)) / 0.26
        for j in range(nz + 1):
            zz = -0.65 + j / nz * 1.92
            foot = max(0, zz - 0.94) / 0.33
            yy = (
                0.635
                - 0.27 * side**1.4
                - 0.28 * foot**1.5
                + 0.015 * math.sin(xx * 29 + zz * 3) * (0.25 + side)
                + 0.009 * math.sin(zz * 27 + xx * 9)
            )
            vs.append((xx, yy, zz + 0.012 * math.sin(xx * 22) * foot))
            if i and j:
                a = (i - 1) * (nz + 1) + j - 1
                b = a + nz + 1
                fs.append((a, a + 1, b + 1, b))
    ob = mesh(o, "Draped linen duvet", vs, fs, "linen")
    ob.data.materials[0].use_nodes = True
    for dx in [-w * 0.25, w * 0.25]:
        soft(
            o,
            "Sleeping pillow",
            (dx, 0.71, -0.78),
            (w * 0.43, 0.21, 0.43),
            "linen",
            0.6,
            wrinkle=0.005,
        )
        p = soft(
            o, "Small decorative cushion", (dx, 0.77, -0.48), (0.44, 0.34, 0.18), "pillow", 0.6
        )
        p.rotation_euler.x = 0.22
    soft(
        o,
        "Folded woven bed runner",
        (0, 0.662, 0.53),
        (w + 0.13, 0.07, 0.47),
        "throw",
        0.27,
        wrinkle=0.012,
    )
    for dx in [-w / 2 - 0.48, w / 2 + 0.48]:
        h.box(o, "Oak bedside drawer", (dx, 0.28, -0.92), (0.66, 0.54, 0.55), "oak", 0.032)
        h.box(o, "Drawer shadow gap", (dx, 0.31, -0.637), (0.60, 0.012, 0.008), "seam")
        v.cylinder(o, "Lamp foot", (dx, 0.58, -0.92), 0.14, 0.04, h.M["bronze"], 32)
        h.rod(o, "Lamp stem", (dx, 0.59, -0.92), (dx, 0.86, -0.92), 0.014, "bronze")
        soft(o, "Linen lamp shade", (dx, 0.97, -0.92), (0.35, 0.29, 0.35), "linen", 0.9)
        v.cylinder(o, "Warm lamp diffuser", (dx, 0.825, -0.92), 0.135, 0.012, h.M["light"], 24)
    return o


def faucet(o, x, y, z):
    pts = [(x, y, z), (x, y + 0.22, z)]
    for a in np.linspace(math.pi, 0, 20):
        pts.append((x + 0.115 + 0.115 * math.cos(a), y + 0.22 + 0.115 * math.sin(a), z))
    pts.append((x + 0.23, y + 0.16, z))
    tube(o, "Swan neck mixer tap", pts, 0.017, "steel", 10)
    h.rod(o, "Tap lever", (x - 0.055, y + 0.02, z), (x - 0.055, y + 0.13, z), 0.012, "steel")


def sinktop(o, x, y, z, w=3.5, d=1.35, key="marble"):
    # Real open sink: four slab pieces, basin below the counter, no black decal.
    sw = 0.68
    sd = 0.42
    for xx in [-1, 1]:
        h.box(
            o,
            "Stone worktop beside sink",
            (x + xx * (w + sw) / 4, y, z),
            ((w - sw) / 2, 0.055, d),
            key,
            0.012,
        )
    for zz in [-1, 1]:
        h.box(
            o,
            "Stone worktop sink rim",
            (x, y, z + zz * (d + sd) / 4),
            (sw, 0.055, (d - sd) / 2),
            key,
            0.009,
        )
    h.box(o, "Sink bottom", (x, y - 0.18, z), (sw, 0.025, sd), "steel", 0.025)
    for xx in [-1, 1]:
        h.box(
            o,
            "Sink side",
            (x + xx * (sw / 2 - 0.012), y - 0.09, z),
            (0.025, 0.18, sd),
            "steel",
            0.01,
        )
    for zz in [-1, 1]:
        h.box(
            o,
            "Sink end",
            (x, y - 0.09, z + zz * (sd / 2 - 0.012)),
            (sw, 0.18, 0.025),
            "steel",
            0.01,
        )
    v.cylinder(o, "Sink drain", (x, y - 0.163, z), 0.032, 0.004, h.M["dark"], 20)
    faucet(o, x - 0.11, y + 0.026, z - 0.30)


def oven(o, x, y, z, w=0.72):
    h.box(o, "Brushed oven fascia", (x, y, z), (w, 0.64, 0.055), "steel", 0.016)
    h.box(
        o, "Smoked oven glazing", (x, y - 0.07, z + 0.035), (w - 0.095, 0.43, 0.027), "ebony", 0.018
    )
    h.box(o, "Oven handle", (x, y + 0.16, z + 0.085), (w - 0.12, 0.027, 0.035), "steel", 0.01)
    h.box(o, "Digital oven display", (x, y + 0.245, z + 0.034), (0.16, 0.035, 0.015), "dark")
    for dx in [-0.25, 0.25]:
        knob = v.cylinder(
            o, "Oven control", (x + dx, y + 0.24, z + 0.058), 0.026, 0.022, h.M["steel"], 16
        )
        knob.rotation_euler.x = math.pi / 2


def range_cooker(o, x, y, z):
    h.box(o, "Professional range body", (x, y + 0.46, z), (1.18, 0.88, 0.72), "steel", 0.022)
    oven(o, x, y + 0.39, z + 0.38, 1.1)
    h.box(o, "Black enamel cooking surface", (x, y + 0.92, z), (1.17, 0.036, 0.70), "ebony", 0.01)
    for dx in [-0.39, 0, 0.39]:
        for dz in [-0.19, 0.19]:
            v.cylinder(
                o, "Gas burner cap", (x + dx, y + 0.945, z + dz), 0.065, 0.025, h.M["metal"], 24
            )
            for a in [0, math.pi / 2]:
                ob = h.box(
                    o,
                    "Cast iron burner grate",
                    (x + dx, y + 0.985, z + dz),
                    (0.27, 0.025, 0.024),
                    "dark",
                    0.008,
                )
                ob.rotation_euler.z = a
    for dx in [-0.43, -0.26, -0.09, 0.09, 0.26, 0.43]:
        ob = v.cylinder(
            o, "Range control knob", (x + dx, y + 0.82, z + 0.397), 0.032, 0.038, h.M["steel"], 20
        )
        ob.rotation_euler.x = math.pi / 2
    h.rod(o, "Pot filler riser", (x, y + 1.38, z - 0.32), (x, y + 1.67, z - 0.32), 0.018, "steel")
    h.rod(
        o,
        "Pot filler folding arm",
        (x, y + 1.64, z - 0.32),
        (x + 0.29, y + 1.64, z - 0.15),
        0.016,
        "steel",
    )
    h.rod(
        o,
        "Pot filler spout",
        (x + 0.29, y + 1.64, z - 0.15),
        (x + 0.29, y + 1.51, z - 0.15),
        0.014,
        "steel",
    )


def cabinet(o, x, y, z, w=0.6, height=0.9, key="oak"):
    h.box(o, "Recessed cabinet carcass", (x, y + height / 2, z), (w, height, 0.62), "seam")
    h.box(
        o,
        "Handle free oak front",
        (x, y + height / 2 + 0.03, z + 0.325),
        (w - 0.016, height - 0.09, 0.035),
        key,
        0.006,
    )
    h.box(o, "Inset toe kick", (x, y + 0.05, z + 0.24), (w - 0.04, 0.10, 0.03), "dark")


def kitchen(parent, x, y, z, full=True):
    """Main kitchen, local width 8.4m; back is -Z. Two islands run east-west."""
    o = group(parent, "Main double-island kitchen" if full else "Chef kitchen", (x, y, z))
    if not full:
        for xx in [-1.7, -1.1, 0.6, 1.2]:
            cabinet(o, xx, 0, -0.8)
        h.box(o, "Chef counter", (-1.4, 0.94, -0.8), (1.25, 0.06, 0.73), "marble", 0.015)
        h.box(o, "Chef counter", (0.9, 0.94, -0.8), (1.2, 0.06, 0.73), "marble", 0.015)
        range_cooker(o, -0.24, 0, -0.8)
        h.box(o, "Chef backsplash", (0, 1.46, -1.14), (4.1, 0.98, 0.06), "marble")
        for xx in [-1.65, -0.95, -0.25, 0.45, 1.15]:
            cabinet(o, xx, 2.0, -0.92, 0.69, 1.0)
        h.box(o, "Chef refrigerator", (2.13, 1.22, -0.8), (0.8, 2.44, 0.75), "steel", 0.024)
        h.box(o, "Fridge vertical handle", (1.86, 1.1, -0.39), (0.027, 0.74, 0.04), "steel", 0.01)
        sinktop(o, 1.00, 0.95, -0.79, 1.15, 0.69)
        return o
    # Full-height light oak cabinetry with a marble cooking niche.
    for xx in [-3.85, -3.1, -2.35, -1.6, -0.85, -0.1, 0.65, 1.4, 2.15, 2.9, 3.65]:
        cabinet(o, xx, 0, -3.3, 0.74, 0.88)
        cabinet(o, xx, 2.05, -3.3, 0.74, 1.40)
    h.box(o, "Carrara backsplash", (0, 1.49, -3.63), (8.2, 1.13, 0.045), "marble")
    h.box(o, "Rear counter left", (-2.42, 0.93, -3.3), (3.30, 0.06, 0.75), "marble", 0.01)
    h.box(o, "Rear counter right", (2.20, 0.93, -3.3), (3.74, 0.06, 0.75), "marble", 0.01)
    range_cooker(o, -0.12, 0, -3.3)
    h.box(o, "Concealed hood grille", (-0.12, 2.037, -3.26), (1.30, 0.018, 0.52), "steel")
    h.box(o, "Backsplash task light", (0, 2.025, -3.60), (8.1, 0.018, 0.026), "light")
    # Appliance tower at left, panelled integrated refrigerator at right.
    for xx in [-3.72, -2.88]:
        h.box(o, "Oven tower", (xx, 1.70, -3.10), (0.80, 3.40, 0.68), "oak", 0.008)
        oven(o, xx, 0.98, -2.72)
        oven(o, xx, 1.68, -2.72)
    for xx in [2.93, 3.70]:
        h.box(o, "Panelled refrigerator", (xx, 1.54, -3.10), (0.745, 3.08, 0.69), "oak", 0.008)
        h.box(
            o, "Refrigerator pull", (xx - 0.24, 1.55, -2.71), (0.021, 0.78, 0.037), "bronze", 0.008
        )
    for island_z in [-0.90, 1.92]:
        # Seating overhang on the front island; drawers on the working one.
        h.box(o, "Island oak carcass", (0, 0.47, island_z), (3.50, 0.88, 0.94), "oak", 0.022)
        for xx in [-1.83, 1.83]:
            h.box(
                o,
                "Waterfall marble return",
                (xx, 0.50, island_z),
                (0.08, 1.00, 1.43),
                "marble",
                0.015,
            )
        if island_z < 0:
            sinktop(o, 0, 0.985, island_z, 3.75, 1.43)
            for xx in [-1.25, 1.25]:
                h.box(
                    o,
                    "Wine cooler smoked door",
                    (xx, 0.50, island_z + 0.485),
                    (0.68, 0.76, 0.037),
                    "ebony",
                    0.014,
                )
                for yy in [0.26, 0.40, 0.54, 0.68]:
                    h.box(
                        o,
                        "Wine cooler rack",
                        (xx, yy, island_z + 0.51),
                        (0.60, 0.012, 0.014),
                        "steel",
                    )
                h.box(
                    o,
                    "Wine cooler handle",
                    (xx, 0.84, island_z + 0.54),
                    (0.56, 0.025, 0.025),
                    "steel",
                    0.008,
                )
        else:
            h.box(
                o,
                "Waterfall island top",
                (0, 0.985, island_z),
                (3.75, 0.065, 1.43),
                "marble",
                0.015,
            )
            for xx in [-1.35, -0.45, 0.45, 1.35]:
                chair(o, xx, 0, island_z + 1.04, (xx, island_z), True)
    side = group(o, "Return wall kitchen joinery", (-4.45, 0, 0.9), math.pi / 2)
    for xx in [-1.95, -1.30, -0.65, 0, 0.65, 1.30, 1.95]:
        cabinet(side, xx, 0, 0, 0.64, 0.88)
        cabinet(side, xx, 2.05, 0, 0.64, 1.40)
    h.box(side, "Return wall marble niche", (0, 1.49, -0.33), (4.52, 1.13, 0.045), "marble")
    sinktop(side, 0, 0.94, 0, 4.52, 0.72)
    h.box(side, "Return wall task light", (0, 2.025, -0.25), (4.40, 0.014, 0.022), "light")
    vase(o, 0.94, 1.02, -0.95, 1.55)
    v.cylinder(o, "Bread board", (-1.10, 1.026, -0.95), 0.27, 0.025, h.M["walnut"], 40)
    for j in range(3):
        soft(
            o, "Ceramic prep bowl", (2.0 + j * 0.17, 1.0, -3.2), (0.14, 0.08, 0.14), "ceramic", 0.85
        )
    return o


def finish_materials():
    for key, name, col, rough, metal in [
        ("oak", "Pale oak cabinetry", (0.68, 0.59, 0.46), 0.62, 0),
        ("walnut", "Solid walnut", (0.20, 0.115, 0.062), 0.48, 0),
        ("ebony", "Ebonised oak", (0.016, 0.019, 0.019), 0.36, 0),
        ("steel", "Brushed stainless steel", (0.48, 0.50, 0.50), 0.27, 0.85),
        ("ceramic", "Chalk ceramic", (0.83, 0.82, 0.76), 0.48, 0),
        ("linen", "Ivory bed linen", (0.83, 0.82, 0.77), 0.94, 0),
        ("rug", "Woven greige wool", (0.50, 0.455, 0.39), 1, 0),
        ("seam", "Tailored seams", (0.40, 0.35, 0.28), 0.95, 0),
        ("throw", "Taupe woven throw", (0.25, 0.21, 0.17), 1, 0),
        ("leather", "Warm grey cinema leather", (0.31, 0.29, 0.26), 0.55, 0),
    ]:
        h.M[key] = material(name, col, rough, metal)
    h.M["fabric"].node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (
        0.80,
        0.77,
        0.69,
        1,
    )
    # Packed, original images: restrained grain, non-repeating-looking veining and
    # fabric micro-normal detail. No procedural shader is expected to survive glTF.
    n = 512
    yy, xx = np.mgrid[0:n, 0:n] / n
    rng = np.random.default_rng(3493)
    for key in ["oak", "walnut", "fabric", "linen", "rug", "marble", "onyx"]:
        if key == "marble":
            h.M[key] = material("White Carrara marble", (0.85, 0.85, 0.82), 0.31)
        m = h.M[key]
        bs = m.node_tree.nodes.get("Principled BSDF")
        base = np.array(bs.inputs["Base Color"].default_value[:3])
        if key in ["oak", "walnut"]:
            field = (
                0.012 * np.sin(xx * math.tau * 91 + np.sin(yy * math.tau * 2) * 0.3)
                + 0.022 * np.sin(xx * math.tau * 23 + np.sin(yy * math.tau) * 0.8)
                + rng.normal(0, 0.006, (n, n))
            )
            rgb = base[None, None, :] * (1 + field[:, :, None])
        elif key in ["marble", "onyx"]:
            warp = (
                0.011 * np.sin(xx * math.tau * 2)
                + 0.006 * np.sin(xx * math.tau * 7 + yy * math.tau)
                + 0.002 * np.sin(xx * math.tau * 19)
            )
            if key == "onyx":
                bands = (
                    0.045 * np.sin((yy + warp) * math.tau * 17)
                    + 0.035 * np.sin((yy + warp) * math.tau * 39)
                    + 0.017 * np.sin((yy + warp) * math.tau * 83)
                )
                grey = 0.018 * np.sin((yy + warp) * math.tau * 11) ** 5
                rgb = np.stack([0.80 + bands, 0.755 + bands, 0.655 + bands], -1) - grey[:, :, None]
            else:
                vein = np.zeros((n, n))
                for k, amp in [(3, 0.048), (7, 0.025), (17, 0.014)]:
                    flow = (
                        xx * 0.71
                        + yy
                        + 0.06 * np.sin(xx * math.tau * 3)
                        + 0.012 * np.sin(yy * math.tau * 11)
                    )
                    vein += np.exp(-((np.sin(flow * math.tau * k) * 14) ** 2)) * amp
                rgb = (
                    np.ones((n, n, 3)) * [0.87, 0.87, 0.84]
                    - vein[:, :, None]
                    + rng.normal(0, 0.003, (n, n, 1))
                )
        else:
            field = rng.normal(0, 0.018, (n, n)) + 0.009 * np.sin(xx * math.tau * 128) * np.sin(
                yy * math.tau * 128
            )
            rgb = base[None, None, :] * (1 + field[:, :, None])
            normal = np.ones((n, n, 4), dtype=np.float32)
            normal[:, :, :3] = [0.5, 0.5, 1]
            normal[:, :, 0] += 0.055 * np.sin(xx * math.tau * 128)
            normal[:, :, 1] += 0.055 * np.sin(yy * math.tau * 128)
            im = bpy.data.images.new(m.name + " / woven normal", width=n, height=n)
            im.colorspace_settings.name = "Non-Color"
            im.pixels.foreach_set(normal.ravel())
            im.pack()
            node = m.node_tree.nodes.new("ShaderNodeTexImage")
            node.image = im
            nm = m.node_tree.nodes.new("ShaderNodeNormalMap")
            nm.inputs["Strength"].default_value = 0.22
            m.node_tree.links.new(node.outputs["Color"], nm.inputs["Color"])
            m.node_tree.links.new(nm.outputs["Normal"], bs.inputs["Normal"])
        rgba = np.ones((n, n, 4), dtype=np.float32)
        rgba[:, :, :3] = np.clip(rgb, 0, 1)
        im = bpy.data.images.new(m.name + " / original surface", width=n, height=n)
        im.pixels.foreach_set(rgba.ravel())
        im.pack()
        tex = m.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = im
        m.node_tree.links.new(tex.outputs["Color"], bs.inputs["Base Color"])


def physical_uvs():
    bpy.context.view_layer.update()
    for ob in bpy.data.objects:
        if ob.type != "MESH" or not ob.data.materials:
            continue
        uv = ob.data.uv_layers.active or ob.data.uv_layers.new(name="Metres")
        name = ob.data.materials[0].name
        for poly in ob.data.polygons:
            normal = ob.matrix_world.to_3x3() @ poly.normal
            drop = max(range(3), key=lambda i: abs(normal[i]))
            axes = [i for i in range(3) if i != drop]
            for li in poly.loop_indices:
                co = ob.matrix_world @ ob.data.vertices[ob.data.loops[li].vertex_index].co
                scale = 1.4 if any(s in name.lower() for s in ["fabric", "linen", "wool"]) else 0.18
                uv.data[li].uv = (co[axes[0]] * scale, co[axes[1]] * scale)
