//! Paths and git output are bytes internally, because git's are. They become strings only
//! here, at the serialization boundary, where invalid UTF-8 renders lossily rather than
//! failing or being dropped.

use bstr::{BString, ByteSlice};
use serde::Serializer;

/// # Errors
/// Propagates the serializer's own failure.
pub fn as_str<S: Serializer>(b: &BString, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&b.to_str_lossy())
}

/// # Errors
/// Propagates the serializer's own failure.
// serde's serialize_with always passes the field by reference.
#[allow(clippy::ref_option)]
pub fn as_str_opt<S: Serializer>(b: &Option<BString>, s: S) -> Result<S::Ok, S::Error> {
    match b {
        Some(b) => s.serialize_str(&b.to_str_lossy()),
        None => s.serialize_none(),
    }
}
