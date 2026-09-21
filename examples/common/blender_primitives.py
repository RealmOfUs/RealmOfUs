"""Small modeling primitives shared by the authored examples; realm coordinates are Y-up."""

import bmesh
import bpy


def location(p):
    return (p[0], -p[2], p[1])


def material(name, color, roughness=0.82, metal=0):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    m.use_nodes = True
    bs = m.node_tree.nodes.get("Principled BSDF")
    bs.inputs["Base Color"].default_value = (*color, 1)
    bs.inputs["Roughness"].default_value = roughness
    bs.inputs["Metallic"].default_value = metal
    return m


def root(eid, label, p):
    o = bpy.data.objects.new(label, None)
    bpy.context.collection.objects.link(o)
    o.location = location(p)
    o["rou_entity_id"] = eid
    return o


def cube(parent, name, p, size, mat, bevel=0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0))
    o = bpy.context.object
    o.name = name
    o.parent = parent
    o.location = location(p)
    o.scale = (size[0], size[2], size[1])
    o.data.materials.append(mat)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        m = o.modifiers.new("Soft edges", "BEVEL")
        m.width = bevel
        m.segments = 1
        bpy.ops.object.modifier_apply(modifier=m.name)
    return o


def cylinder(parent, name, p, radius, depth, mat, vertices=12):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth)
    o = bpy.context.object
    o.name = name
    o.parent = parent
    o.location = location(p)
    o.data.materials.append(mat)
    return o


def cone(parent, name, p, radius, depth, mat):
    bpy.ops.mesh.primitive_cone_add(vertices=9, radius1=radius, radius2=radius * 0.12, depth=depth)
    o = bpy.context.object
    o.name = name
    o.parent = parent
    o.location = location(p)
    o.data.materials.append(mat)
    return o


def roof(parent, w, d, h, base, mat):
    vs = [
        (-w / 2, base, -d / 2),
        (w / 2, base, -d / 2),
        (w / 2, base, d / 2),
        (-w / 2, base, d / 2),
        (0, base + h, -d / 2),
        (0, base + h, d / 2),
    ]
    mesh = bpy.data.meshes.new("Roof")
    mesh.from_pydata(
        [location(v) for v in vs],
        [],
        [(0, 1, 4), (3, 5, 2), (0, 4, 5, 3), (4, 1, 2, 5), (0, 3, 2, 1)],
    )
    mesh.update()
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    o = bpy.data.objects.new("Terracotta roof", mesh)
    bpy.context.collection.objects.link(o)
    o.parent = parent
    o.data.materials.append(mat)
