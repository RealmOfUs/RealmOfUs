//! Strict JSON parsing, RFC 8785 canonical encoding, SHA-256 and readable on-disk formatting.
//!
//! External bytes must pass through [`strict`] or [`parse`] to retain duplicate-key,
//! encoding and budget checks. Use [`canonical`] for stable digests and [`pretty`]
//! for text intended for Git merging; these encodings serve different purposes.
//!
//! ```rust
//! use rou_core::json;
//!
//! let value = json::strict(br#"{"b":1.0,"a":-0.0}"#)?;
//! assert_eq!(json::canonical(&value)?, br#"{"a":0,"b":1}"#);
//! assert_eq!(json::hash(&value)?, json::digest(&json::canonical(&value)?));
//! assert!(json::strict(br#"{"a":1,"a":2}"#).is_err());
//! # Ok::<(), rou_core::Error>(())
//! ```
use crate::{Result, error::require, limits};
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::fmt;

struct Strict(usize);

impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        if self.0 > limits::JSON_DEPTH {
            return Err(de::Error::custom("JSON depth budget exceeded"));
        }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Strict {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("strict JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite JSON number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Value, E> {
        Ok(value.into())
    }
    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Strict(self.0 + 1))? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON key: {key}")));
            }
            values.insert(key, map.next_value_seed(Strict(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}

/// Parse one complete JSON value with the byte limit in [`limits::JSON_BYTES`].
///
/// # Errors
///
/// Return `json_budget` when the byte budget is exceeded. Invalid UTF-8, a BOM,
/// duplicate keys, overflowing numbers, excessive nesting, syntax errors or trailing
/// non-whitespace content return `invalid_json`.
pub fn strict(data: &[u8]) -> Result<Value> {
    strict_with_limit(data, limits::JSON_BYTES)
}

/// Parse JSON with a custom byte budget and the other rules of [`strict`].
///
/// `max_bytes` replaces only the byte budget; nesting remains bounded by [`limits::JSON_DEPTH`].
/// The GLB precheck uses this to parse larger embedded JSON chunks.
///
/// # Errors
///
/// Return `json_budget` for excessive bytes, or `invalid_json` for invalid encoding, structure or syntax.
pub fn strict_with_limit(data: &[u8], max_bytes: usize) -> Result<Value> {
    require(
        data.len() <= max_bytes,
        "json_budget",
        "JSON byte budget exceeded",
    )?;
    require(
        !data.starts_with(b"\xef\xbb\xbf") && std::str::from_utf8(data).is_ok(),
        "invalid_json",
        "JSON must be UTF-8 without BOM",
    )?;
    let mut reader = serde_json::Deserializer::from_slice(data);
    let result = Strict(0).deserialize(&mut reader)?;
    reader.end()?;
    Ok(result)
}

/// Strictly parse raw bytes, then deserialize them into the target type.
///
/// Document deserialization checks shape; full semantics still require the corresponding `validate` method.
///
/// # Errors
///
/// Propagate [`strict`] errors; type mismatches, unknown fields or invalid document shapes return `invalid_json`.
pub fn parse<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
    from_value(strict(data)?)
}
/// Convert a parsed JSON value into a target type using its deserialization rules.
///
/// Does not recheck raw encoding, duplicate keys, byte count or nesting depth:
/// use [`strict`] before constructing the `Value` to check that information.
/// Callers remain responsible for full document semantic validation.
///
/// # Errors
///
/// Return `invalid_json` if the target type or its shape constraints do not match.
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}
/// Encode a serializable value under RFC 8785 as UTF-8 bytes without a trailing newline.
///
/// Normalize key order and numeric representation without document semantic checks or input byte budgets.
///
/// # Errors
///
/// Return `invalid_json` if serialization or canonical encoding fails.
pub fn canonical<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    Ok(serde_json_canonicalizer::to_vec(&value)?)
}
/// Compute the SHA-256 of raw bytes as 64 lowercase hexadecimal digits.
///
/// Does not parse or normalize input; use [`hash`] for a digest of a JSON value.
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
/// Apply [`canonical`] encoding, then compute [`digest`].
///
/// # Errors
///
/// Propagate canonical encoding errors as `invalid_json`; does not validate domain semantics.
pub fn hash<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    Ok(digest(&canonical(value)?))
}

/// Format JSON with sorted keys, two-space indentation, one array element per line and one final newline.
///
/// Normalize integral floats and negative zero through JCS before formatting readable text.
/// This output is intended for storage and Git merging. Use [`hash`] for digests;
/// do not treat the formatted bytes as canonical encoding.
///
/// # Errors
///
/// Propagate serialization errors and the byte/depth budget errors from [`strict`] after canonical encoding.
pub fn pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    // Round-trip through JCS also normalizes integral floats and negative zero.
    let value = strict(&canonical(value)?)?;
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Read an integer in the closed interval `[min, max]` by numeric value, not JSON spelling.
///
/// `1`, `1.0` and `1e0` are equivalent; booleans and strings are not numbers.
/// Comparisons use `f64` internally and target exactly representable integers within
/// this module's budgets, not lossless parsing of arbitrarily large integers.
///
/// # Errors
///
/// Return `invalid_integer` for a nonnumeric, nonfinite, fractional or out-of-range value.
pub fn integer(value: &Value, min: i64, max: i64) -> Result<i64> {
    let number = value
        .as_f64()
        .ok_or_else(|| crate::Error::new("invalid_integer", "expected a number"))?;
    require(
        number.is_finite() && number.fract() == 0.0 && number >= min as f64 && number <= max as f64,
        "invalid_integer",
        "expected a bounded integer",
    )?;
    Ok(number as i64)
}

/// Serde field adapter that reads exactly three bounded integer components.
///
/// Check each component with [`integer`], bounded in absolute value by [`limits::COORDINATE_MM`].
/// This function does not define units: placement vectors use millimetres and chunk vectors use indices.
///
/// # Errors
///
/// Return a deserializer error for an invalid element count, type or range.
pub fn vector<'de, D: de::Deserializer<'de>>(reader: D) -> std::result::Result<[i64; 3], D::Error> {
    let values = <[Value; 3]>::deserialize(reader)?;
    let mut result = [0; 3];
    for (out, value) in result.iter_mut().zip(values.iter()) {
        *out = integer(value, -limits::COORDINATE_MM, limits::COORDINATE_MM)
            .map_err(de::Error::custom)?;
    }
    Ok(result)
}

/// Serde field adapter that reads an integer length in `0..=FILE_BYTES`.
///
/// Accept numerically equivalent floating-point spellings; this is not a parser for the entire `u64` range.
///
/// # Errors
///
/// Return a deserializer error for a fractional/negative value or one exceeding [`limits::FILE_BYTES`].
pub fn unsigned<'de, D: de::Deserializer<'de>>(reader: D) -> std::result::Result<u64, D::Error> {
    integer(&Value::deserialize(reader)?, 0, limits::FILE_BYTES as i64)
        .map(|v| v as u64)
        .map_err(de::Error::custom)
}
