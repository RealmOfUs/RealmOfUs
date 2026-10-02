//! A pure operation table shared by CLI helpers, the C ABI and GDExtension.
//!
//! [`dispatch`] accepts an operation name and input bytes, returning one UTF-8-encoded
//! JSON value. [`envelope`] wraps it as `ok/value` or `ok/error`.
//! Does not read paths, files or global state.
//!
//! # Raw-byte operations
//!
//! | Name | Input | JSON result |
//! | --- | --- | --- |
//! | `json.strict` | Raw JSON | Parsed value |
//! | `json.canonical` / `json.pretty` | Raw JSON | A **JSON string** containing the encoded text |
//! | `digest.sha256` | Arbitrary bytes | Lowercase SHA-256 string |
//! | `file.descriptor` | Raw payload bytes | [`Descriptor`] |
//! | `glb.precheck` | Complete GLB | [`glb::Report`] |
//! | `glb.inspect` | Complete GLB | `{metrics, descriptor}` |
//! | `lfs.pointer` | Pointer or payload bytes | Pointer [`Descriptor`], or `null` for a non-pointer |
//! | `config` | Ignored; empty bytes allowed | ABI, profile, viewer defaults and limits, host read budgets, format/path constants |
//!
//! # JSON request operations
//!
//! Inputs below are UTF-8 bytes of the described JSON, not host-object memory layouts.
//! Request objects reject unknown fields; linked types and methods define their semantics.
//!
//! | Name | Input | Result / rules |
//! | --- | --- | --- |
//! | `id.validate` / `path.safe` | JSON string | Original string after validation |
//! | `asset.path` | SHA-256 JSON string | `objects/<sha256>.glb` |
//! | `asset.match` | `{asset: Asset, actual: Descriptor}` | `true` when descriptors match |
//! | `record.validate` | `{path, kind, id}`, kind is `model` or `entity` | `true` when the filename matches |
//! | `profile.validate` | [`Profile`] | Fully validated profile |
//! | `profile.calibration` | Integer `16`, `32` or `64` metres | Profile for grid comparison experiments |
//! | `realm.validate` / `model.validate` / `entity.validate` | [`Realm`] / [`Model`] / [`Entity`] | Validated document |
//! | `manifest.validate` | [`Manifest`] | Fully validated catalog |
//! | `world.metadata` | [`World`] | `true` after metadata validation, without requiring cold input descriptors |
//! | `preview.create` | `{profile, entities}`; entities have id, optional label, position_mm, bounds_mm, asset | [`Manifest`] with derived owners, offsets and index |
//! | `build.validate` | `{profile, model, inputs, record}` | Record after [`Build::validate`] |
//! | `build.input_digest` | `{profile, model, inputs}` | [`input_digest`] string |
//! | `version.validate` | `{version, catalog}`, catalog is the raw catalog text string | Sidecar after [`Version::validate`] |
//! | `spatial.owner` | `{profile, position_mm}` | `{chunk, offset}` |
//! | `spatial.key` | Three-integer chunk index | Canonical `x,y,z` key |
//! | `spatial.intersections` | `{profile, bounds_mm}` | Chunk list from [`Profile::intersections`] |
//! | `spatial.range` | `{profile, selectors}` | Sorted, deduplicated chunks from [`Profile::range`] |
//! | `spatial.grid` | [`Profile`] | `{vertices_mm, edges}`, grid endpoints and index pairs on the Y=0 plane |
//! | `spatial.project` | `{world, records}`, records keyed by model ID | In-memory view from [`World::projections`] |
//! | `blender.placement` | `{direction:"to_realm", position}` or `{direction:"to_blender", position_mm}` | Three-component conversion between Z-up metres and Y-up millimetres |
//! | `blender.bounds` | Exported Y-up metre point array | Positive-thickness integer millimetre [`Bounds`] |
//! | `blender.bounds_native` | Blender Z-up metre `[min,max]` | Axis conversion followed by millimetre [`Bounds`] |
//! | `blender.relative_path` | `{source, reference}` | Safe relative path from [`blender::relative_path`] |
//! | `world.diff` | `{before, after}`, either world may be null | Semantic change object from [`world::diff`] |
//! | `session.fingerprint` | `{world, source}` | Baseline digest from [`world::session_fingerprint`] |
//! | `viewer.catalog` / `viewer.start` | [`Manifest`] | Viewer layout including all impostor bounds / default overview pose |
//! | `viewer.radii` | Three metre radii | Validated ordered near/middle/far radii |
//! | `viewer.at` | `x,y,z` JSON string in metres | Three-component metre coordinates |
//! | `viewer.aim` | `{position, look_at}` in metres | Pose with coincident-target fallback |
//! | `viewer.pose` | [`super::viewer::Pose`] | Validated pose |
//! | `viewer.query` | [`super::viewer::Query`] | Streaming actions, tier IDs and current chunk information |
//! | `viewer.map` | [`super::viewer::MapRequest`] | In-world map grid layout |
//! | `viewer.chunk` | `{profile, selector}`, a canonical single-chunk key | `{position, chunk}`, position is the center in metres |
//! | `viewer.metres` | Three-integer millimetre coordinates | Three-component metre coordinates |
//!
//! # Errors and host boundaries
//!
//! Operations return domain [`Error`] values. Native boundaries must catch panics
//! rather than letting them cross the C/Godot ABI. Operations perform no host IO, so
//! a valid catalog does not prove that referenced files exist or asset bytes passed validation.
use crate::{
    Error, Result, blender,
    documents::*,
    formats, glb, json, lfs, limits, paths,
    spatial::{Bounds, Profile},
    world::{self, World},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    profile: Profile,
    #[serde(deserialize_with = "json::vector")]
    position_mm: [i64; 3],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intersections {
    profile: Profile,
    bounds_mm: Bounds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    profile: Profile,
    selectors: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    world: World,
    records: BTreeMap<String, Build>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildRequest {
    profile: Profile,
    model: Model,
    inputs: Inputs,
    record: Build,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRequest {
    profile: Profile,
    model: Model,
    inputs: Inputs,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionRequest {
    version: Version,
    catalog: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelativePath {
    source: String,
    reference: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordPath {
    path: String,
    kind: String,
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetMatch {
    asset: Asset,
    actual: Descriptor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkDestination {
    profile: Profile,
    selector: String,
}
#[derive(Deserialize)]
#[serde(transparent)]
struct Millimetres(#[serde(deserialize_with = "json::vector")] [i64; 3]);
#[derive(Deserialize)]
#[serde(tag = "direction", rename_all = "snake_case", deny_unknown_fields)]
enum Placement {
    ToRealm {
        position: [f64; 3],
    },
    ToBlender {
        #[serde(deserialize_with = "json::vector")]
        position_mm: [i64; 3],
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diff {
    before: Option<World>,
    after: Option<World>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    world: World,
    source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewRequest {
    profile: Profile,
    entities: Vec<PreviewDraft>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewDraft {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(deserialize_with = "json::vector")]
    position_mm: [i64; 3],
    bounds_mm: Bounds,
    asset: Asset,
}

/// Dispatch an operation and return its JSON-encoded result without a response envelope.
///
/// See the [module documentation](self) for operation names, input shapes and results.
/// JSON encoding operations also return a JSON string; hosts must decode one layer
/// to obtain the formatted text.
///
/// # Errors
///
/// Return `unknown_operation` for an unknown name; preserve core input parsing and
/// domain validation errors. Does not catch panics; native hosts must catch them
/// at their respective ABI boundaries.
///
/// # Examples
///
/// ```rust
/// use rou_core::ops;
///
/// let response = ops::dispatch("json.canonical", br#"{"b":1,"a":2}"#)?;
/// let text: String = serde_json::from_slice(&response)?;
/// assert_eq!(text, r#"{"a":2,"b":1}"#);
/// let (status, encoded) = ops::envelope(ops::dispatch("unknown", b""));
/// let error: serde_json::Value = serde_json::from_slice(&encoded)?;
/// assert_eq!(status, 1);
/// assert_eq!(error["error"]["code"], "unknown_operation");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn dispatch(op: &str, input: &[u8]) -> Result<Vec<u8>> {
    let value = match op {
        "json.strict" => json::strict(input)?,
        "json.canonical" => json!(
            String::from_utf8(json::canonical(&json::strict(input)?)?).expect("JSON is UTF-8")
        ),
        "json.pretty" => {
            json!(String::from_utf8(json::pretty(&json::strict(input)?)?).expect("JSON is UTF-8"))
        }
        "digest.sha256" => json!(json::digest(input)),
        "file.descriptor" => {
            let value = Descriptor::of(input);
            value.validate_shape()?;
            serde_json::to_value(value)?
        }
        "glb.precheck" => serde_json::to_value(glb::precheck(input)?)?,
        "glb.inspect" => {
            json!({"metrics":glb::precheck(input)?,"descriptor":Descriptor::of(input)})
        }
        "asset.match" => {
            let request: AssetMatch = json::parse(input)?;
            request.asset.validate_shape()?;
            request.actual.validate_shape()?;
            crate::error::require(
                request.asset.descriptor() == request.actual,
                "asset_mismatch",
                "asset hash or size mismatch",
            )?;
            json!(true)
        }
        "lfs.pointer" => serde_json::to_value(lfs::pointer(input)?)?,
        "id.validate" => {
            let value: String = json::parse(input)?;
            paths::identity(&value)?;
            json!(value)
        }
        "asset.path" => {
            let digest: String = json::parse(input)?;
            hash_shape(&digest)?;
            json!(paths::object(&digest))
        }
        "path.safe" => {
            let value: String = json::parse(input)?;
            paths::safe(&value)?;
            json!(value)
        }
        "record.validate" => {
            let request: RecordPath = json::parse(input)?;
            let directory = match request.kind.as_str() {
                "model" => paths::MODELS,
                "entity" => paths::ENTITIES,
                _ => return Err(Error::new("invalid_argument", "unknown record kind")),
            };
            world::record_id(&request.path, directory, &request.id)?;
            json!(true)
        }
        "profile.validate" => {
            let value: Profile = json::parse(input)?;
            value.validate()?;
            serde_json::to_value(value)?
        }
        "realm.validate" => {
            let value: Realm = json::parse(input)?;
            value.validate()?;
            serde_json::to_value(value)?
        }
        "world.metadata" => {
            let value: World = json::parse(input)?;
            value.validate_metadata()?;
            json!(true)
        }
        "profile.calibration" => {
            let size: i64 = json::parse(input)?;
            crate::error::require(
                [16, 32, 64].contains(&size),
                "invalid_profile",
                "calibration grid must be 16, 32 or 64 metres",
            )?;
            let profile = Profile {
                chunk_size_mm: [size * 1000; 3],
                min_chunk: [-128 / size, -64 / size, -128 / size],
                max_chunk: [128 / size, 64 / size, 128 / size],
                ..Profile::default()
            };
            profile.validate()?;
            serde_json::to_value(profile)?
        }
        "model.validate" => {
            let value: Model = json::parse(input)?;
            value.validate()?;
            serde_json::to_value(value)?
        }
        "entity.validate" => {
            let value: Entity = json::parse(input)?;
            value.validate_shape()?;
            serde_json::to_value(value)?
        }
        "manifest.validate" => {
            let value: Manifest = json::parse(input)?;
            value.validate()?;
            serde_json::to_value(value)?
        }
        "preview.create" => {
            let request: PreviewRequest = json::parse(input)?;
            let mut entities = Vec::new();
            for entity in request.entities {
                let (owner_chunk, offset_mm) = request.profile.owner(entity.position_mm)?;
                entities.push(PreviewEntity {
                    id: entity.id,
                    label: entity.label,
                    position_mm: entity.position_mm,
                    owner_chunk,
                    offset_mm,
                    bounds_mm: entity.bounds_mm,
                    asset: entity.asset,
                });
            }
            entities.sort_by(|a, b| a.id.cmp(&b.id));
            let manifest = Manifest {
                format: formats::PREVIEW.into(),
                profile_hash: json::hash(&request.profile)?,
                chunks: crate::spatial::build_index(&request.profile, &entities)?,
                profile: request.profile,
                index_complete: true,
                entities,
            };
            manifest.validate()?;
            serde_json::to_value(manifest)?
        }
        "build.validate" => {
            let request: BuildRequest = json::parse(input)?;
            request
                .record
                .validate(&request.profile, &request.model, &request.inputs)?;
            serde_json::to_value(request.record)?
        }
        "build.input_digest" => {
            let request: InputRequest = json::parse(input)?;
            json!(input_digest(
                &request.profile,
                &request.model,
                &request.inputs
            )?)
        }
        "version.validate" => {
            let request: VersionRequest = json::parse(input)?;
            request.version.validate(request.catalog.as_bytes())?;
            serde_json::to_value(request.version)?
        }
        "spatial.owner" => {
            let request: Position = json::parse(input)?;
            let (chunk, offset) = request.profile.owner(request.position_mm)?;
            json!({"chunk": chunk, "offset": offset})
        }
        "spatial.key" => {
            let Millimetres(chunk) = json::parse(input)?;
            crate::spatial::vector(&chunk)?;
            json!(crate::spatial::chunk_key(chunk))
        }
        "spatial.intersections" => {
            let request: Intersections = json::parse(input)?;
            json!(request.profile.intersections(&request.bounds_mm)?)
        }
        "spatial.range" => {
            let request: Range = json::parse(input)?;
            json!(request.profile.range(&request.selectors)?)
        }
        "spatial.grid" => {
            let profile: Profile = json::parse(input)?;
            profile.validate()?;
            let lower: [i64; 3] = std::array::from_fn(|a| {
                profile.origin_mm[a] + profile.min_chunk[a] * profile.chunk_size_mm[a]
            });
            let upper: [i64; 3] = std::array::from_fn(|a| {
                profile.origin_mm[a] + profile.max_chunk[a] * profile.chunk_size_mm[a]
            });
            let mut vertices = Vec::<[i64; 3]>::new();
            let mut edges = Vec::new();
            for axis in [0, 2] {
                for index in profile.min_chunk[axis]..=profile.max_chunk[axis] {
                    let value = profile.origin_mm[axis] + index * profile.chunk_size_mm[axis];
                    edges.push([vertices.len(), vertices.len() + 1]);
                    vertices.extend(if axis == 0 {
                        [[value, 0, lower[2]], [value, 0, upper[2]]]
                    } else {
                        [[lower[0], 0, value], [upper[0], 0, value]]
                    });
                }
            }
            json!({"vertices_mm":vertices,"edges":edges})
        }
        "spatial.project" => {
            let request: Projection = json::parse(input)?;
            serde_json::to_value(request.world.projections(&request.records)?)?
        }
        "blender.placement" => match json::parse::<Placement>(input)? {
            Placement::ToRealm { position } => json!(blender::placement(position)?),
            Placement::ToBlender { position_mm } => json!(blender::inverse(position_mm)?),
        },
        "blender.bounds" => {
            serde_json::to_value(blender::bounds(&json::parse::<Vec<[f64; 3]>>(input)?)?)?
        }
        "blender.bounds_native" => {
            serde_json::to_value(blender::bounds_native(json::parse(input)?)?)?
        }
        "blender.relative_path" => {
            let request: RelativePath = json::parse(input)?;
            json!(blender::relative_path(&request.source, &request.reference)?)
        }
        "world.diff" => {
            let request: Diff = json::parse(input)?;
            world::diff(request.before.as_ref(), request.after.as_ref())?
        }
        "session.fingerprint" => {
            let request: Session = json::parse(input)?;
            json!(world::session_fingerprint(&request.world, &request.source)?)
        }
        "viewer.catalog" => crate::viewer::catalog(&json::parse(input)?)?,
        "viewer.start" => serde_json::to_value(crate::viewer::default_pose(&json::parse(input)?)?)?,
        "viewer.at" => json!(crate::viewer::at(&json::parse::<String>(input)?)?),
        "viewer.aim" => serde_json::to_value(crate::viewer::aim(json::parse(input)?)?)?,
        "viewer.pose" => serde_json::to_value(crate::viewer::pose(json::parse(input)?)?)?,
        "viewer.radii" => json!(crate::viewer::radii(json::parse(input)?)?),
        "viewer.query" => crate::viewer::query(&json::parse(input)?)?,
        "viewer.map" => crate::viewer::map(&json::parse(input)?)?,
        "viewer.chunk" => {
            let request: ChunkDestination = json::parse(input)?;
            let chunk = crate::spatial::parse_chunk(&request.selector)?;
            json!({"position":crate::viewer::chunk_center(&request.profile,chunk)?,"chunk":chunk})
        }
        "viewer.metres" => {
            let Millimetres(position) = json::parse(input)?;
            crate::spatial::vector(&position)?;
            json!(crate::viewer::metres(position))
        }
        "config" => json!({
            "abi_version": formats::ABI_VERSION,
            "profile": Profile::default(),
            "viewer": {
                "radii": crate::viewer::default_radii(), "hysteresis": limits::VIEW_HYSTERESIS,
                "max_radius": limits::VIEW_COORDINATE_METRES,
                "max_retention": limits::VIEW_RETENTION_METRES
            },
            "limits": {
                "json_bytes": limits::JSON_BYTES,
                "glb_bytes": limits::GLB_BYTES,
                "file_bytes": limits::FILE_BYTES,
                "pointer_bytes": limits::POINTER_BYTES
            },
            "formats": {
                "realm": formats::REALM, "model": formats::MODEL,
                "entity": formats::ENTITY, "build": formats::BUILD,
                "blender_export": formats::BLENDER_EXPORT, "view": formats::VIEW,
                "preview": formats::PREVIEW, "git_view": formats::GIT_VIEW
            },
            "paths": {
                "realm": paths::REALM, "models": paths::MODELS,
                "entities": paths::ENTITIES, "runtime": paths::RUNTIME,
                "sources": paths::SOURCES, "objects": paths::OBJECTS,
                "version": paths::VERSION
            }
        }),
        _ => {
            return Err(Error::new(
                "unknown_operation",
                format!("unknown operation: {op}"),
            ));
        }
    };
    Ok(serde_json::to_vec(&value)?)
}

/// Wrap an operation result as `(status code, JSON envelope bytes)`.
///
/// Success returns code 0 and `{"ok":true,"value":...}`; failure returns code 1 and
/// `{"ok":false,"error":{"code":...,"message":...}}`. Successful bytes that are not
/// valid JSON also become an `invalid_json` error envelope.
///
/// Codes 0/1 here describe only domain results. Adapters separately handle boundary
/// errors, such as invalid C ABI output pointers. Does not catch panics.
pub fn envelope(result: Result<Vec<u8>>) -> (i32, Vec<u8>) {
    let (status, value) =
        match result.and_then(|bytes| Ok(serde_json::from_slice::<Value>(&bytes)?)) {
            Ok(value) => (0, json!({"ok": true, "value": value})),
            Err(error) => (1, json!({"ok": false, "error": error})),
        };
    (
        status,
        serde_json::to_vec(&value).expect("envelope contains only JSON values"),
    )
}
