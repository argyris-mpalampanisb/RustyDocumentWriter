"""Render RustyDocumentWriter's built-in 3D shape library with Blender (headless).

    blender -b --factory-startup --python tools/shapes/make_shapes.py -- <out_dir> [shape ...]

Every shape is modelled procedurally here, so the library has no third-party assets.
Each one is rendered in neutral grey on a transparent background, with a soft contact
shadow. The app tints the grey to the colour the user picks when inserting it,
so one image per shape covers every colour. Run tools/shapes/finish.py afterwards to crop
and downscale the renders into assets/shapes/.
"""

import math
import sys

import bmesh
import bpy
from mathutils import Vector

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "shapes-out"
ONLY = sys.argv[sys.argv.index("--") + 2:] if "--" in sys.argv else []
RES = 768

# ---------------------------------------------------------------------------
# Scene


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    try:
        sc.render.engine = "CYCLES"
    except TypeError as e:
        print("Cycles unavailable:", e)
    sc.render.resolution_x = sc.render.resolution_y = RES
    sc.render.film_transparent = True
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGBA"
    # Predictable tones: the app maps mid-grey to the chosen colour.
    try:
        sc.view_settings.view_transform = "Standard"
    except TypeError:
        pass
    sc.cycles.samples = 96
    sc.cycles.use_denoising = True
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for kind in ("OPTIX", "CUDA"):
            try:
                prefs.compute_device_type = kind
                break
            except TypeError:
                continue
        prefs.refresh_devices()
        for d in prefs.devices:
            d.use = d.type != "CPU"
        sc.cycles.device = "GPU"
    except Exception as e:  # noqa: BLE001 - fall back to CPU rendering
        print("GPU setup failed, using CPU:", e)

    world = bpy.data.worlds.new("World")
    sc.world = world
    world.use_nodes = True
    bg = next(n for n in world.node_tree.nodes if n.type == "BACKGROUND")
    bg.inputs[0].default_value = (0.35, 0.35, 0.35, 1.0)
    bg.inputs[1].default_value = 0.35

    def area(name, loc, energy, size):
        light = bpy.data.lights.new(name, "AREA")
        light.energy = energy
        light.size = size
        ob = bpy.data.objects.new(name, light)
        sc.collection.objects.link(ob)
        ob.location = loc
        ob.rotation_euler = (Vector((0, 0, 0)) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()

    area("Key", (-1.6, -2.2, 6.0), 260, 1.6)
    area("Fill", (4.5, -3.0, 2.0), 70, 4.0)
    area("Rim", (1.0, 4.0, 4.0), 160, 2.0)

    cam_data = bpy.data.cameras.new("Camera")
    cam_data.lens = 70
    cam = bpy.data.objects.new("Camera", cam_data)
    sc.collection.objects.link(cam)
    cam.location = (0.0, -7.2, 3.6)
    cam.rotation_euler = (Vector((0, 0, 0.1)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    sc.camera = cam

    bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, 0))
    ground = bpy.context.object
    ground.is_shadow_catcher = True


def material():
    m = bpy.data.materials.new("Shape")
    m.use_nodes = True
    p = next(n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    p.inputs["Base Color"].default_value = (0.42, 0.42, 0.42, 1.0)
    p.inputs["Roughness"].default_value = 0.32
    for name in ("Coat Weight", "Clearcoat"):
        if name in p.inputs:
            p.inputs[name].default_value = 0.35
            break
    return m


def finish(ob, bevel=0.0, smooth=True, spin=0.0, tilt=0.0):
    """Material, bevel, smoothing, then scale to a unit bounding sphere resting on the ground."""
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    if bevel > 0:
        mod = ob.modifiers.new("Bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 4
        mod.limit_method = "ANGLE"
        mod.harden_normals = True
    if smooth:
        try:
            bpy.ops.object.shade_auto_smooth(angle=math.radians(35))
        except Exception:  # noqa: BLE001 - older/newer operator signatures
            bpy.ops.object.shade_smooth()
    ob.data.materials.append(material())
    ob.rotation_euler = (math.radians(tilt), 0, math.radians(spin))
    bpy.context.view_layer.update()
    pts = [ob.matrix_world @ Vector(c) for c in ob.bound_box]
    centre = sum(pts, Vector()) / 8
    radius = max((p - centre).length for p in pts)
    s = 1.2 / radius
    ob.scale = (s, s, s)
    bpy.context.view_layer.update()
    pts = [ob.matrix_world @ Vector(c) for c in ob.bound_box]
    centre = sum(pts, Vector()) / 8
    ob.location -= Vector((centre.x, centre.y, min(p.z for p in pts)))


def prism(name, outline, depth=0.5):
    """Extrude a 2D outline (counter-clockwise points in the XZ plane) into a standing slab."""
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    verts = [bm.verts.new((x, 0.0, z)) for x, z in outline]
    face = bm.faces.new(verts)
    bmesh.ops.recalc_face_normals(bm, faces=[face])
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    moved = [v for v in ext["geom"] if isinstance(v, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=(0.0, depth, 0.0))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    # Triangulate the caps so concave outlines (stars, hearts) bevel and shade cleanly.
    caps = [f for f in bm.faces if len(f.verts) > 4]
    bmesh.ops.triangulate(bm, faces=caps, quad_method="BEAUTY", ngon_method="BEAUTY")
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    ob.location.y = -depth / 2
    return ob


def circle_pts(n, r=1.0, start=math.pi / 2):
    return [(r * math.cos(start + i * math.tau / n), r * math.sin(start + i * math.tau / n)) for i in range(n)]


# ---------------------------------------------------------------------------
# Shapes


def cube():
    bpy.ops.mesh.primitive_cube_add()
    finish(bpy.context.object, bevel=0.12, spin=30)


def sphere():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=96, ring_count=48)
    finish(bpy.context.object)


def cylinder():
    bpy.ops.mesh.primitive_cylinder_add(vertices=96, depth=2.0)
    finish(bpy.context.object, bevel=0.08)


def cone():
    bpy.ops.mesh.primitive_cone_add(vertices=96, depth=2.0)
    finish(bpy.context.object, bevel=0.04)


def pyramid():
    bpy.ops.mesh.primitive_cone_add(vertices=4, depth=1.6, radius1=1.3)
    finish(bpy.context.object, bevel=0.03, smooth=False, spin=20)


def torus():
    bpy.ops.mesh.primitive_torus_add(major_segments=96, minor_segments=48, major_radius=1.0, minor_radius=0.38)
    finish(bpy.context.object, tilt=55)


def capsule():
    # A sphere split at the equator, the halves pulled apart and bridged by its middle ring.
    bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.6)
    ob = bpy.context.object
    for v in ob.data.vertices:
        v.co.z += 0.55 if v.co.z > 1e-4 else (-0.55 if v.co.z < -1e-4 else 0.0)
    finish(ob, tilt=90, spin=25)


def hexagon():
    bpy.ops.mesh.primitive_cylinder_add(vertices=6, depth=0.8)
    finish(bpy.context.object, bevel=0.06, smooth=False, spin=15)


def gem():
    me = bpy.data.meshes.new("Gem")
    bm = bmesh.new()
    n = 10
    crown = [bm.verts.new((math.cos(i * math.tau / n), math.sin(i * math.tau / n), 0.0)) for i in range(n)]
    table = [bm.verts.new((0.55 * math.cos((i + 0.5) * math.tau / n), 0.55 * math.sin((i + 0.5) * math.tau / n), 0.45)) for i in range(n)]
    tip = bm.verts.new((0, 0, -1.1))
    bm.faces.new(table)
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((crown[i], crown[j], table[i]))
        bm.faces.new((table[i], crown[j], table[j]))
        bm.faces.new((crown[j], crown[i], tip))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new("Gem", me)
    bpy.context.scene.collection.objects.link(ob)
    finish(ob, smooth=False, tilt=-12)


def star():
    pts = [(r * math.cos(math.pi / 2 + i * math.pi / 5), r * math.sin(math.pi / 2 + i * math.pi / 5)) for i, r in ((i, 1.0 if i % 2 == 0 else 0.45) for i in range(10))]
    finish(prism("Star", pts, 0.45), bevel=0.07, spin=-18)


def heart():
    pts = []
    for i in range(96):
        t = i * math.tau / 96
        pts.append((16 * math.sin(t) ** 3 / 16, (13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)) / 16))
    finish(prism("Heart", pts[::-1], 0.5), bevel=0.12, spin=-18)


def arrow():
    pts = [(-1.2, -0.3), (0.3, -0.3), (0.3, -0.75), (1.25, 0.0), (0.3, 0.75), (0.3, 0.3), (-1.2, 0.3)]
    finish(prism("Arrow", pts, 0.4), bevel=0.06, spin=-25)


def lightning():
    pts = [(0.25, 1.3), (-0.65, -0.05), (-0.05, -0.05), (-0.35, -1.3), (0.7, 0.25), (0.08, 0.25), (0.45, 1.3)]
    finish(prism("Lightning", pts[::-1], 0.35), bevel=0.05, spin=-20)


def plus():
    a, b = 0.32, 1.0
    pts = [(-a, -b), (a, -b), (a, -a), (b, -a), (b, a), (a, a), (a, b), (-a, b), (-a, a), (-b, a), (-b, -a), (-a, -a)]
    finish(prism("Plus", pts, 0.45), bevel=0.08, spin=-22)


def check():
    pts = [(-1.0, 0.1), (-0.65, 0.45), (-0.25, 0.05), (0.75, 1.05), (1.1, 0.7), (-0.25, -0.65)]
    finish(prism("Check", pts, 0.4), bevel=0.07, spin=-20)


def bubble():
    pts = []
    for i in range(72):
        t = i * math.tau / 72
        pts.append((1.2 * math.cos(t), 0.2 + 0.8 * math.sin(t)))
    # Splice a tail in at the lower left of the ellipse.
    k = int(72 * 0.62)
    tail = [(-0.7, -1.05)]
    pts = pts[:k] + tail + pts[k + 4:]
    finish(prism("Bubble", pts, 0.4), bevel=0.08, spin=-18)


def ring():
    bpy.ops.mesh.primitive_torus_add(major_segments=96, minor_segments=32, major_radius=1.0, minor_radius=0.16)
    finish(bpy.context.object, tilt=70, spin=10)


def crown():
    # A band whose top edge zigzags into points, with a ball on each point.
    n = 12
    me = bpy.data.meshes.new("Crown")
    bm = bmesh.new()
    bottom = [bm.verts.new((math.cos(i * math.tau / n), math.sin(i * math.tau / n), 0.0)) for i in range(n)]
    top = [bm.verts.new((math.cos(i * math.tau / n), math.sin(i * math.tau / n), 1.15 if i % 2 == 0 else 0.6)) for i in range(n)]
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((bottom[i], bottom[j], top[j], top[i]))
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new("Crown", me)
    bpy.context.scene.collection.objects.link(ob)
    sol = ob.modifiers.new("Thickness", "SOLIDIFY")
    sol.thickness = 0.1
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.modifier_apply(modifier=sol.name)
    parts = [ob]
    for i in range(0, n, 2):
        a = i * math.tau / n
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.11, location=(math.cos(a), math.sin(a), 1.2))
        parts.append(bpy.context.object)
    bpy.ops.object.select_all(action="DESELECT")
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.join()
    finish(ob, bevel=0.02, tilt=18)


SHAPES = {
    "cube": cube,
    "sphere": sphere,
    "cylinder": cylinder,
    "cone": cone,
    "pyramid": pyramid,
    "torus": torus,
    "capsule": capsule,
    "hexagon": hexagon,
    "gem": gem,
    "star": star,
    "heart": heart,
    "arrow": arrow,
    "lightning": lightning,
    "plus": plus,
    "check": check,
    "bubble": bubble,
    "ring": ring,
    "crown": crown,
}

for name, build in SHAPES.items():
    if ONLY and name not in ONLY:
        continue
    reset()
    build()
    bpy.context.scene.render.filepath = f"{OUT}/{name}.png"
    bpy.ops.render.render(write_still=True)
    print("RENDERED", name)
