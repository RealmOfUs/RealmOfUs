//! Pure rules shared by the CLI, Blender and Godot.
//!
//! This module neither reads files nor starts processes and has no global mutable state.
//! Hosts provide raw bytes, snapshots and paths; they are responsible for file existence,
//! symlink checks, Git history and loading actual assets.
//!
//! # Entry points and validation order
//!
//! - Rust callers use the typed APIs in [`documents`], [`spatial`] and [`world`].
//! - FFI/GDExtension adapters access the same rules through [`ops::dispatch`].
//! - External JSON goes through [`json::parse`]: strict byte parsing, then document shape checks.
//! - `validate_shape` performs only the local checks declared by that type. Cross-field and
//!   cross-document constraints require its `validate` method or [`world::World::validate`].
//!   Constructing a Rust struct directly does not validate it.
//!
//! World placements and bounds use integer millimetres in a right-handed Y-up system.
//! Bounding boxes and world bounds are half-open: the minimum is included, the maximum excluded.
//! Viewer cameras use metres; Blender Z-up conversion is centralized in [`blender`].
//! Errors are classified by [`Error::code`], with display text in [`Error::message`].
//!
//! # Examples
//!
//! ```rust
//! use rou_core::{documents::Realm, json, spatial::Profile};
//!
//! let bytes = serde_json::to_vec(&serde_json::json!({
//!     "format": "rou-realm/0.1", "id": "garden", "profile": Profile::default()
//! }))?;
//! let realm: Realm = json::parse(&bytes)?;
//! realm.validate()?;
//! assert_eq!(realm.id, "garden");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod blender;
pub mod documents;
pub mod error;
pub mod formats;
pub mod glb;
pub mod json;
pub mod lfs;
pub mod limits;
pub mod ops;
pub mod paths;
pub mod selection;
pub mod spatial;
pub mod world;

pub use error::{Error, Result};

pub mod viewer;
