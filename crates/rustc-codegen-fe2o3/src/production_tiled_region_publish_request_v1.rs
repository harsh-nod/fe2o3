//! Bounded inert request; borrowed text is not HIR or compiler custody.
use super::*;
use serde::Deserialize;
const REQUEST_CAP: usize = 8192;
const SCHEMA: &str = "fe2o3-bf16-tile-source-promotion-request-v1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire<'a> {
    schema: &'a str,
    semantic_sha256: &'a str,
    canonical_sha256: &'a str,
    mir_sha256: &'a str,
    original_sha256: &'a str,
    original_path: &'a str,
    candidate_path: &'a str,
    helper_name: &'a str,
    return_order: Bf16TileReturnOrderV1,
}
pub(crate) struct Bf16TileSourcePublishRequestV1<'a> {
    pub(crate) semantic_sha256: [u8; 32],
    pub(crate) canonical_sha256: [u8; 32],
    pub(crate) mir_sha256: [u8; 32],
    pub(crate) original_sha256: [u8; 32],
    pub(crate) original_path: &'a str,
    pub(crate) candidate_path: &'a str,
    pub(crate) helper_name: &'a str,
    pub(crate) return_order: Bf16TileReturnOrderV1,
}
fn digest(text: &str) -> Result<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(Error::refused(
            "BF16 digest requires 64 lowercase hexadecimal bytes",
        ));
    }
    let mut bytes = [0; 32];
    for (slot, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let n = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
        *slot = n(pair[0]) * 16 + n(pair[1]);
    }
    if bytes == [0; 32] {
        return Err(Error::refused("BF16 zero digest is not a selection"));
    }
    Ok(bytes)
}
impl<'a> Bf16TileSourcePublishRequestV1<'a> {
    // Borrowed strings deliberately reject JSON escapes requiring owned decoding.
    // The separate input domain owns these <=8192 bytes while action runs.
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > REQUEST_CAP {
            return Err(Error::refused("BF16 request exceeds bounded input profile"));
        }
        let wire: Wire<'a> = serde_json::from_slice(bytes)
            .map_err(|_| Error::refused("BF16 request is not closed borrowed JSON"))?;
        if wire.schema != SCHEMA {
            return Err(Error::refused("BF16 request schema differs"));
        }
        let value = Self {
            semantic_sha256: digest(wire.semantic_sha256)?,
            canonical_sha256: digest(wire.canonical_sha256)?,
            mir_sha256: digest(wire.mir_sha256)?,
            original_sha256: digest(wire.original_sha256)?,
            original_path: wire.original_path,
            candidate_path: wire.candidate_path,
            helper_name: wire.helper_name,
            return_order: wire.return_order,
        };
        value.validate()?;
        Ok(value)
    }
    pub(super) fn validate(&self) -> Result<()> {
        if [
            self.semantic_sha256,
            self.canonical_sha256,
            self.mir_sha256,
            self.original_sha256,
        ]
        .contains(&[0; 32])
        {
            return Err(Error::refused("BF16 zero digest is not a selection"));
        }
        for path in [self.original_path, self.candidate_path] {
            fe2o3_source_isa_observation::source_edit_v1::validate_source_edit_path_v1(path)
                .map_err(|_| {
                    Error::refused("BF16 source paths require bounded relative Rust files")
                })?;
        }
        if self.original_path == self.candidate_path {
            return Err(Error::refused(
                "BF16 candidate must be create-new and distinct",
            ));
        }
        text::helper_name(self.helper_name)
    }
}
