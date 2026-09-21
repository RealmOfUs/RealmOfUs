# Offline architectural study assets

`champagne-onyx-v1.png` is an original flat material swatch generated using the
built-in imagegen tool. The exact prompt is in `onyx-prompt.txt`; dimensions and
SHA-256 are in `provenance.json`. It is used only as the onyx shader's base colour.
The film's objects, camera motion, shading and reflections are rendered in Blender.

The remaining external assets are downloaded by
`examples/villa_anahid/offline/fetch_assets.py` into `.cache/walsh-offline/assets`.
**Powered by [Poly Haven](https://polyhaven.com/)**; its downloaded materials and
models are [CC0](https://polyhaven.com/license). Each exact URL and digest is saved
in `.cache/walsh-offline/asset-provenance.json` and copied into the build directory.

- wool_boucle: measured yarn normals and roughness; recoloured plain ivory.
- rough_linen: measured weave normals and roughness; recoloured sand / grey.
- white_oak_veneer: oak colour, roughness and normals.
- fern_02: four fern meshes and their surface maps, used in the garden walls.
- potted_plant_02: plant, pot and soil; interior pot recoloured stoneware.
- ceramic_vase_03: scanned ceramic vase and its surface maps.
- kloppenheim_06_puresky: existing project's CC0 Poly Haven sky HDRI, reused.

Listing photographs and the reference film are visual research only. They are
not included in the `.blend`, film, or material files. The isolated room has
inferred dimensions and is not a measured reconstruction of the complete house.
