//! Bounded PNG/JPEG header inspection without image decoding or filesystem access.
use crate::{Error, Result, error::require, limits};
use serde::{Deserialize, Serialize};

fn check(condition: bool, message: &str) -> Result<()> {
    require(condition, "invalid_image", message)
}

fn pixel_count(width: u32, height: u32) -> Result<u64> {
    check(width > 0 && height > 0, "image dimensions must be nonzero")?;
    check(
        width <= limits::IMAGE_DIMENSION && height <= limits::IMAGE_DIMENSION,
        "image safety limit exceeded: IMAGE_DIMENSION (16384 pixels)",
    )?;
    Ok(u64::from(width) * u64::from(height))
}

/// Header-derived resource statistics for one external image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Canonical extension derived from the signature, either png or jpg.
    pub extension: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Width times height, without multiplying by instances or materials.
    pub pixels: u64,
    /// Conservative four-byte-per-pixel decoded size.
    pub decoded_bytes: u64,
}
impl Report {
    /// Validate declared dimensions and recompute all derived statistics.
    pub fn validate(&self) -> Result<()> {
        check(
            matches!(self.extension.as_str(), "png" | "jpg"),
            "unsupported image extension",
        )?;
        let pixels = pixel_count(self.width, self.height)?;
        check(
            self.pixels == pixels && self.decoded_bytes == pixels * 4,
            "invalid image statistics",
        )
    }
}

/// Identify PNG/JPEG by signature and check dimensions and the encoded byte budget.
/// Does not decode pixels or verify complete PNG/JPEG integrity.
pub fn precheck(data: &[u8]) -> Result<Report> {
    check(
        data.len() <= limits::GLB_BYTES,
        "image safety limit exceeded: IMAGE_BYTES (256 MiB)",
    )?;
    let (extension, mime) = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("png", "image/png")
    } else if data.starts_with(b"\xff\xd8") {
        ("jpg", "image/jpeg")
    } else {
        return Err(Error::new(
            "invalid_image",
            "unsupported PNG/JPEG signature",
        ));
    };
    let (width, height) = dimensions(data, mime)?;
    let pixels = pixel_count(width, height)?;
    let result = Report {
        extension: extension.into(),
        width,
        height,
        pixels,
        decoded_bytes: pixels * 4,
    };
    result.validate()?;
    Ok(result)
}

/// Read pixel `(width, height)` from a PNG IHDR or JPEG SOF0/1/2 header.
///
/// Does not decode images, verify PNG CRC/JPEG integrity or check dimension budgets.
/// [`precheck`] requires positive dimensions and limits total decoded pixels.
///
/// # Errors
///
/// Return `invalid_image` for a MIME type other than `image/png`/`image/jpeg`, a damaged header or no supported JPEG frame.
pub fn dimensions(data: &[u8], mime: &str) -> Result<(u32, u32)> {
    if mime == "image/png" {
        check(
            data.len() >= 24
                && data.starts_with(b"\x89PNG\r\n\x1a\n")
                && &data[12..16] == b"IHDR"
                && data[8..12] == 13_u32.to_be_bytes(),
            "invalid PNG header",
        )?;
        return Ok((
            u32::from_be_bytes(data[16..20].try_into().expect("four bytes")),
            u32::from_be_bytes(data[20..24].try_into().expect("four bytes")),
        ));
    }
    check(
        mime == "image/jpeg" && data.starts_with(b"\xff\xd8"),
        "unsupported image",
    )?;
    let mut cursor = 2;
    while cursor + 4 <= data.len() {
        check(data[cursor] == 255, "invalid JPEG marker")?;
        while cursor < data.len() && data[cursor] == 255 {
            cursor += 1;
        }
        check(cursor < data.len(), "truncated JPEG marker")?;
        let marker = data[cursor];
        cursor += 1;
        if marker == 0xd8 || marker == 0xd9 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        check(cursor + 2 <= data.len(), "truncated JPEG segment")?;
        let size = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
        check(
            size >= 2 && cursor + size <= data.len(),
            "JPEG segment overrun",
        )?;
        if (0xc0..=0xc2).contains(&marker) {
            check(size >= 8, "truncated JPEG frame")?;
            return Ok((
                u16::from_be_bytes([data[cursor + 5], data[cursor + 6]]) as u32,
                u16::from_be_bytes([data[cursor + 3], data[cursor + 4]]) as u32,
            ));
        }
        cursor += size;
    }
    Err(Error::new("invalid_image", "JPEG has no supported frame"))
}
