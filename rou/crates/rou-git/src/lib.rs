//! Native Git/LFS and filesystem orchestration. Domain rules live in rou_core.
//!
//! Mutating repository methods require the caller to hold [`lock::WriteLock`].
//! Read-only validation and view generation use immutable Git object IDs; local
//! caches are disposable atomic hints. See [`error::code`] for stable categories.
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
pub mod build;
pub mod error;
pub mod init;
pub mod io;
pub mod lock;
pub mod payload;
pub mod release;
pub mod repository;
pub mod snapshot;
pub mod sparse;
pub mod validate;
pub mod view;
pub mod workflow;

pub use anyhow::{Context, Result};
pub use repository::Repository;
