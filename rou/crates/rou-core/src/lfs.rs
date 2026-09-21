//! Parsing and encoding of three-line Git LFS pointers, with a unified pointer/payload descriptor.
//!
//! Malformed input with a recognized pointer prefix is an error, not an ordinary asset.
//! This module does not download payloads. A pointer describes its target payload,
//! not the pointer text itself.
//!
//! ```rust
//! use rou_core::{documents::Descriptor, lfs};
//!
//! let payload = b"BLENDER payload";
//! let expected = Descriptor::of(payload);
//! let pointer = lfs::encode(&expected)?;
//! assert_eq!(lfs::pointer(&pointer)?, Some(expected.clone()));
//! assert_eq!(lfs::descriptor(payload)?, expected);
//! # Ok::<(), rou_core::Error>(())
//! ```
use crate::{Result, documents::Descriptor, error::require, limits};
/// The first line of a standard Git LFS v1 pointer, without a newline.
pub const VERSION: &str = "version https://git-lfs.github.com/spec/v1";

/// Recognize and parse a three-line `version`, `oid sha256:`, `size` pointer.
///
/// Return `Ok(None)` without an LFS version prefix. Standard pointers may have one
/// trailing LF, but no extra lines or extension fields. The descriptor refers to the actual payload.
///
/// # Errors
///
/// Return `invalid_pointer` for a recognized prefix with malformed encoding, line count,
/// version or size text. [`Descriptor::validate_shape`] reports digest-shape and file-budget errors.
pub fn pointer(data: &[u8]) -> Result<Option<Descriptor>> {
    if !data.starts_with(b"version https://git-lfs.github.com/spec/") {
        return Ok(None);
    }
    require(
        data.len() <= limits::POINTER_BYTES,
        "invalid_pointer",
        "LFS pointer byte budget exceeded",
    )?;
    let text = std::str::from_utf8(data)
        .map_err(|_| crate::Error::new("invalid_pointer", "LFS pointer must be UTF-8"))?;
    let lines: Vec<_> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect();
    require(
        lines.len() == 3 && lines[0] == VERSION,
        "invalid_pointer",
        "unsupported or malformed LFS pointer",
    )?;
    let hash = lines[1]
        .strip_prefix("oid sha256:")
        .ok_or_else(|| crate::Error::new("invalid_pointer", "missing LFS SHA-256"))?;
    let length = lines[2]
        .strip_prefix("size ")
        .ok_or_else(|| crate::Error::new("invalid_pointer", "missing LFS size"))?;
    require(
        !length.is_empty() && length.bytes().all(|c| c.is_ascii_digit()),
        "invalid_pointer",
        "invalid LFS size",
    )?;
    let bytes = length
        .parse()
        .map_err(|_| crate::Error::new("invalid_pointer", "LFS size overflow"))?;
    let result = Descriptor {
        sha256: hash.to_owned(),
        bytes,
    };
    result.validate_shape()?;
    Ok(Some(result))
}

/// Validate a payload descriptor and encode it as a standard three-line pointer with a trailing LF.
///
/// # Errors
///
/// Return validation errors for an invalid descriptor digest or file byte budget.
pub fn encode(descriptor: &Descriptor) -> Result<Vec<u8>> {
    descriptor.validate_shape()?;
    Ok(format!(
        "{VERSION}\noid sha256:{}\nsize {}\n",
        descriptor.sha256, descriptor.bytes
    )
    .into_bytes())
}

/// Return the descriptor declared in a pointer, or compute one from ordinary payload bytes.
///
/// # Errors
///
/// Return `file_budget` for input exceeding the file budget. Propagate [`pointer()`]
/// errors for a malformed recognized pointer without falling back to ordinary payload handling.
pub fn descriptor(data: &[u8]) -> Result<Descriptor> {
    require(
        data.len() as u64 <= limits::FILE_BYTES,
        "file_budget",
        "file byte budget exceeded",
    )?;
    Ok(pointer(data)?.unwrap_or_else(|| Descriptor::of(data)))
}
