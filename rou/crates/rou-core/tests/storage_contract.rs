#[path = "../../../tests/common/mod.rs"]
mod common;
use rou_core::{documents::*, glb, image, json, limits, ops, spatial, viewer};
use serde_json::{Value, json as value};
use std::collections::BTreeMap;

fn png(width: u32, height: u32) -> Vec<u8> {
    [
        b"\x89PNG\r\n\x1a\n".as_slice(),
        &13_u32.to_be_bytes(),
        b"IHDR",
        &width.to_be_bytes(),
        &height.to_be_bytes(),
    ]
    .concat()
}
fn query(position: [f64; 3]) -> viewer::Residency {
    viewer::Residency {
        position,
        radii: [1., 2., 3.],
        loaded: BTreeMap::new(),
        hysteresis: 1.3,
    }
}
fn preview() -> Preview {
    let (world, records) = common::world();
    world.projections(&records).unwrap().manifest().unwrap()
}
fn rebind(bundle: &mut rou_core::paging::Bundle, key: &str, page: &Page) {
    let descriptor = bundle.root.chunks.get_mut(key).unwrap();
    let bytes = json::pretty(page).unwrap();
    descriptor.sha256 = json::digest(&bytes);
    descriptor.bytes = bytes.len() as u64;
    descriptor.entities = page.entities.len();
    bundle
        .pages
        .insert(descriptor.page.clone(), String::from_utf8(bytes).unwrap());
}

#[test]
fn externalisation_preserves_geometry_images_and_remaps_views() {
    let (mut document, geometry) = common::triangle();
    let pixels = png(16, 8);
    let binary = [pixels.as_slice(), geometry.as_slice()].concat();
    document["buffers"][0]["byteLength"] = value!(binary.len());
    document["bufferViews"] = value!([
        {"buffer":0,"byteLength":pixels.len()},
        {"buffer":0,"byteOffset":pixels.len(),"byteLength":geometry.len()}
    ]);
    document["accessors"][0]["bufferView"] = value!(1);
    document["images"] = value!([
        {"bufferView":0,"mimeType":"image/png","name":"first"},
        {"bufferView":0,"mimeType":"image/png","name":"shared"}
    ]);
    let original = common::glb(&document, &binary);
    assert!(glb::precheck(&original).is_err());
    let plan = glb::externalise(&original).unwrap();
    assert_eq!(plan.textures.len(), 1);
    assert_eq!(plan.metrics.image_decoded_bytes, 16 * 8 * 4);
    let extracted = &plan.textures[0];
    assert_eq!(
        &original[extracted.offset..extracted.offset + extracted.descriptor.bytes as usize],
        &pixels
    );
    let result = plan.materialize(&original).unwrap();
    plan.descriptor.verify(&result).unwrap();
    let length = u32::from_le_bytes(result[12..16].try_into().unwrap()) as usize;
    let rewritten: Value = serde_json::from_slice(&result[20..20 + length]).unwrap();
    assert_eq!(rewritten["accessors"][0]["bufferView"], 0);
    assert_eq!(rewritten["bufferViews"].as_array().unwrap().len(), 1);
    assert_eq!(rewritten["images"][0]["mimeType"], "image/png");
    assert_eq!(rewritten["images"][1]["mimeType"], "image/png");
    assert_eq!(&result[28 + length..], &geometry);
    assert_eq!(
        glb::precheck(&result).unwrap().images,
        vec![format!("{}.png", json::digest(&pixels))]
    );
    // The converter accepts only its embedded source form, not already published files.
    assert!(glb::externalise(&result).is_err());
}

#[test]
fn external_jpeg_preserves_its_verified_decoder_hint() {
    let jpeg = b"\xff\xd8\xff\xc0\x00\x0b\x08\x00\x02\x00\x03\x01\x01\x11\x00";
    let (mut document, geometry) = common::triangle();
    let binary = [geometry.as_slice(), jpeg.as_slice()].concat();
    document["buffers"][0]["byteLength"] = value!(binary.len());
    document["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(value!({
            "buffer":0,"byteOffset":geometry.len(),"byteLength":jpeg.len()
        }));
    document["images"] = value!([{"bufferView":1,"mimeType":"image/jpeg"}]);
    let source = common::glb(&document, &binary);
    let plan = glb::externalise(&source).unwrap();
    let result = plan.materialize(&source).unwrap();
    let length = u32::from_le_bytes(result[12..16].try_into().unwrap()) as usize;
    let rewritten: Value = serde_json::from_slice(&result[20..20 + length]).unwrap();
    assert_eq!(rewritten["images"][0]["mimeType"], "image/jpeg");
    assert_eq!(
        rewritten["images"][0]["uri"],
        format!("{}.jpg", json::digest(jpeg))
    );
    let extracted = &plan.textures[0];
    assert_eq!(
        &source[extracted.offset..extracted.offset + extracted.descriptor.bytes as usize],
        jpeg
    );
    assert_eq!(plan.metrics.image_pixels, 6);
}

#[test]
fn uri_set_signature_and_declarations_are_exact() {
    let pixels = png(1, 2);
    let sha = json::digest(&pixels);
    let texture = Texture {
        path: format!("objects/{sha}.png"),
        sha256: sha.clone(),
        bytes: pixels.len() as u64,
    };
    let (mut document, binary) = common::triangle();
    let uri = format!("{sha}.png");
    document["images"] = value!([{"uri":uri}]);
    let metrics = glb::precheck(&common::glb(&document, &binary)).unwrap();
    metrics
        .match_textures(std::slice::from_ref(&texture))
        .unwrap();
    assert!(metrics.match_textures(&[]).is_err());
    assert!(
        metrics
            .match_textures(&[texture.clone(), texture.clone()])
            .is_err()
    );
    texture.verify(&pixels).unwrap();
    let mut wrong = texture.clone();
    wrong.path = format!("objects/{sha}.jpg");
    assert!(wrong.verify(&pixels).is_err());
    for invalid in [
        "data:image/png;base64,aaaa".into(),
        format!("/{uri}"),
        format!("../{uri}"),
        format!("sub/{uri}"),
        format!("{uri}?x"),
        format!("{}.PNG", sha.to_uppercase()),
        format!("{sha}.jpeg"),
    ] {
        document["images"][0]["uri"] = value!(invalid);
        assert!(
            glb::precheck(&common::glb(&document, &binary)).is_err(),
            "{invalid}"
        );
    }
    let jpeg = b"\xff\xd8\xff\xc0\x00\x0b\x08\x00\x02\x00\x03\x01\x01\x11\x00";
    assert_eq!(image::precheck(jpeg).unwrap().extension, "jpg");
    assert!(image::precheck(b"not an image").is_err());
    assert!(image::precheck(&png(0, 10)).is_err());
    assert!(image::precheck(&png(limits::IMAGE_DIMENSION + 1, 1)).is_err());
}

#[test]
fn coverage_pages_discover_owners_and_tampering_does_not_change_state() {
    let mut world = preview();
    world.entities.retain(|e| e.id == "house");
    world.entities[0].bounds_mm.max[0] = 33_000;
    world.chunks = spatial::build_index(&world.profile, &world.entities).unwrap();
    let bundle = world.bundle().unwrap();
    assert_eq!(bundle.root.chunks["1,0,0"].entities, 0);
    let mut catalog = viewer::Catalog::load_root(&bundle.root).unwrap();
    let request = query([40., 1., 1.]);
    assert_eq!(
        catalog.query(&request).unwrap()["missing_pages"],
        value!(["1,0,0"])
    );
    let foreign = &bundle.pages["chunks/1,0,0.json"];
    assert!(
        catalog
            .load_page("1,0,0", format!("{foreign} ").as_bytes())
            .is_err()
    );
    assert_eq!(
        catalog.query(&request).unwrap()["missing_pages"],
        value!(["1,0,0"])
    );
    catalog.load_page("1,0,0", foreign.as_bytes()).unwrap();
    assert_eq!(
        catalog.query(&request).unwrap()["missing_pages"],
        value!(["0,0,0"])
    );
    catalog
        .load_page("0,0,0", bundle.pages["chunks/0,0,0.json"].as_bytes())
        .unwrap();
    let result = catalog.query(&request).unwrap();
    assert_eq!(result["missing_pages"], value!([]));
    assert_eq!(result["load"], value!(["house"]));
    assert_eq!(
        catalog.query(&query([3000.; 3])).unwrap()["load"],
        value!([])
    );
    assert_eq!(
        catalog.query(&query([-100.; 3])).unwrap()["missing_pages"],
        value!([])
    );
}

#[test]
fn late_owner_rejects_false_foreign_reference_and_payload_conflicts() {
    let mut world = preview();
    world.entities[1].bounds_mm.max[0] = 33_000;
    world.chunks = spatial::build_index(&world.profile, &world.entities).unwrap();
    let mut bundle = world.bundle().unwrap();
    let mut page: Page = json::parse(bundle.pages["chunks/1,0,0.json"].as_bytes()).unwrap();
    page.foreign[0].id = "absent".into();
    rebind(&mut bundle, "1,0,0", &page);
    let mut catalog = viewer::Catalog::load_root(&bundle.root).unwrap();
    catalog
        .load_page("1,0,0", bundle.pages["chunks/1,0,0.json"].as_bytes())
        .unwrap();
    assert!(
        catalog
            .load_page("0,0,0", bundle.pages["chunks/0,0,0.json"].as_bytes())
            .is_err()
    );
    let mut bundle = preview().bundle().unwrap();
    let mut page: Page = json::parse(bundle.pages["chunks/1,0,0.json"].as_bytes()).unwrap();
    page.entities[0].asset.bytes += 4;
    rebind(&mut bundle, "1,0,0", &page);
    assert!(ops::dispatch("bundle.validate", &serde_json::to_vec(&bundle).unwrap()).is_err());
}

#[test]
fn world_count_exceeds_old_cap_but_each_page_remains_bounded() {
    let original = preview();
    let mut world = original.clone();
    world.entities = (0..limits::PAGE_RECORDS + 1)
        .map(|index| {
            let mut entity = original.entities[index % 2].clone();
            entity.id = format!("entity_{index:05}");
            entity
        })
        .collect();
    world.chunks = spatial::build_index(&world.profile, &world.entities).unwrap();
    let bundle = world.bundle().unwrap();
    assert_eq!(bundle.root.entity_count, limits::PAGE_RECORDS + 1);
    ops::dispatch("bundle.validate", &serde_json::to_vec(&bundle).unwrap()).unwrap();
    for entity in &mut world.entities {
        entity.position_mm = [0; 3];
        entity.owner_chunk = [0; 3];
        entity.offset_mm = [0; 3];
        entity.bounds_mm = original.entities[1].bounds_mm.clone();
    }
    world.chunks = spatial::build_index(&world.profile, &world.entities).unwrap();
    assert!(world.bundle().is_err());
}

#[test]
fn prior_storage_formats_are_diagnosed_without_compatibility_readers() {
    for format in ["rou-build", "rou-preview", "rou-blender-export"] {
        let bytes = serde_json::to_vec(&value!({"format":format!("{format}/0.2")})).unwrap();
        let error = match format {
            "rou-build" => json::parse::<Build>(&bytes).unwrap_err(),
            "rou-preview" => json::parse::<Manifest>(&bytes).unwrap_err(),
            _ => json::parse::<BlenderExport>(&bytes).unwrap_err(),
        };
        assert!(
            error
                .message
                .contains(&format!("document predates {format}/0.3"))
        );
    }
}

#[test]
fn image_dimensions_are_bounded_before_decoded_byte_arithmetic() {
    // Regression for the first external-image fuzz run's IHDR multiplication overflow.
    for dimension in [0x96969696, u32::MAX] {
        assert_eq!(
            image::precheck(&png(dimension, dimension))
                .unwrap_err()
                .code,
            "invalid_image"
        );
    }
}
