# PBR materials

Project Depot can draw OMSI content with physically based materials: a normal map for small
relief, and roughness, metalness and ambient occlusion maps for how a surface reflects
light. OMSI 2 itself knows nothing of this, so these maps are purely an addition: a bus, a
map object or a spline without them looks exactly as it always did, and content with them
still works in the original OMSI 2 (which ignores the extra files).

PBR maps are drawn with the **Enhanced** graphics (launcher → Settings → Graphics:
`graphics=enhanced`), on the computer and on Android alike.

## How maps are found

No config file is changed. The maps lie **next to the diffuse texture** and share its name,
with a suffix, as the common PBR tools (Substance, Blender, Materialize ...) export them. For
`Texture/bus.dds`:

| Map | File names | Notes |
|---|---|---|
| Normal | `bus_nn`, `bus_normal`, `bus_nrm` | tangent space; Direct3D style (green down) by default, add `_gl` for OpenGL style (green up): `bus_nn_gl`, `bus_normal_gl` |
| Roughness | `bus_rr`, `bus_rough`, `bus_roughness` | white = rough |
| Glossiness | `bus_gg`, `bus_gloss`, `bus_glossiness` | the inverse of roughness |
| Metalness | `bus_mm`, `bus_metal`, `bus_metallic`, `bus_metalness` | white = metal |
| Ambient occlusion | `bus_aa`, `bus_ao`, `bus_occlusion` | white = open |
| Packed | `bus_orm`, `bus_arm` | occlusion, roughness, metalness in red, green, blue |
| Packed | `bus_mra` | metalness, roughness, occlusion in red, green, blue |

Each can be `.png`, `.tga`, `.dds`, `.bmp` or `.jpg`; case does not matter. The short
suffixes are **doubled letters** (`_nn`, `_rr`, `_mm`, `_aa`, `_gg`) on purpose: OMSI mods
already use single letters for other things (`_n` is a night map, `_r` and `_m` mean
anything), so a single letter is never taken for a PBR map. A normal map must also look like
one (bluish), or it is ignored.

The separate maps are packed into one occlusion/roughness/metalness picture for the GPU.
PBR maps are kept at up to 2048 px and uncompressed (a normal map does not survive DXT1
compression), so use them where they are seen up close.

## How they are drawn

- The normal map bends the surface normal (a tangent frame is derived per pixel, so no
  tangents are needed in the `.o3d` model).
- Roughness sharpens or blurs the reflections of the environment and the sun's highlight;
  metalness tints the reflection with the diffuse colour and darkens the diffuse part.
- Occlusion darkens the ambient light only.
- An OMSI `[matl_envmap]` strength still scales the reflection, and the diffuse texture's
  alpha still works as OMSI's reflection mask where no roughness map is given.

## Tips for modders

- Export at the diffuse texture's resolution or half of it.
- Prefer one packed `_orm` file over three single maps: one file read instead of three.
- Check the green channel of a normal map: dents that look like bumps mean it needs `_gl`
  (or the other way round).
- Source: `crates/omsi-texture/src/pbr.rs`, `crates/omsi-render/src/enhanced.wgsl`.
