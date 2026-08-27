//! Strict shared wire model for compiler-owned simulation source maps.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::MAX_SIMULATION_DEBUG_MAP_BYTES_V1;

pub const DEBUG_SOURCE_MAP_SCHEMA_V1: &str = "fe2o3-debug-source-map-v1";
pub const MAX_DEBUG_SOURCE_FILES_V1: usize = 65_536;
pub const MAX_DEBUG_SOURCE_SITES_V1: usize = 1_000_000;
pub const MAX_DEBUG_SOURCE_SPANS_V1: usize = 4_000_000;
pub const MAX_DEBUG_SOURCE_PATH_BYTES_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapDocumentV1 {
    schema: DebugSourceMapSchemaV1,
    binding: DebugSourceMapBindingV1,
    files: Vec<DebugSourceMapFileV1>,
    sites: Vec<DebugSourceMapSiteV1>,
    eliminated: Vec<DebugSourceMapSpanV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum DebugSourceMapSchemaV1 {
    #[serde(rename = "fe2o3-debug-source-map-v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapBindingV1 {
    #[serde(with = "hex_identity")]
    bundle_subject_identity: [u8; 32],
    canonical_kir: DebugSourceMapKirIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapKirIdentityV1 {
    #[serde(with = "hex_identity")]
    digest: [u8; 32],
    canonical_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapFileV1 {
    #[serde(with = "hex_identity")]
    identity: [u8; 32],
    byte_len: u64,
    display_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapSiteV1 {
    site: DebugSourceMapKirSiteV1,
    spans: Vec<DebugSourceMapSpanV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapKirSiteV1 {
    function_ordinal: u64,
    block_ordinal: u64,
    point: DebugSourceMapKirPointV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugSourceMapKirPointV1 {
    Operation { operation_ordinal: u64 },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapSpanV1 {
    #[serde(with = "hex_identity")]
    file_identity: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSourceMapErrorV1 {
    InvalidLength,
    InvalidJson,
    InvalidIdentity,
    InvalidKirIdentity,
    InvalidFile,
    InvalidSite,
    InvalidSpan,
    DuplicateFile,
    DuplicateSite,
    DuplicateSpan,
    ResourceLimit,
    AllocationFailure,
    Encoding,
    NonCanonicalEncoding,
}

impl fmt::Display for DebugSourceMapErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 debug source map: {self:?}")
    }
}

impl Error for DebugSourceMapErrorV1 {}

impl DebugSourceMapBindingV1 {
    pub fn new(
        bundle_subject_identity: [u8; 32],
        canonical_kir_digest: [u8; 32],
        canonical_kir_bytes: u64,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        if bundle_subject_identity == [0; 32] {
            return Err(DebugSourceMapErrorV1::InvalidIdentity);
        }
        let canonical_kir =
            DebugSourceMapKirIdentityV1::new(canonical_kir_digest, canonical_kir_bytes)?;
        Ok(Self {
            bundle_subject_identity,
            canonical_kir,
        })
    }

    pub const fn bundle_subject_identity(self) -> [u8; 32] {
        self.bundle_subject_identity
    }

    pub const fn canonical_kir(self) -> DebugSourceMapKirIdentityV1 {
        self.canonical_kir
    }
}

impl DebugSourceMapKirIdentityV1 {
    pub fn new(digest: [u8; 32], canonical_bytes: u64) -> Result<Self, DebugSourceMapErrorV1> {
        if digest == [0; 32] || canonical_bytes == 0 {
            return Err(DebugSourceMapErrorV1::InvalidKirIdentity);
        }
        Ok(Self {
            digest,
            canonical_bytes,
        })
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
}

impl DebugSourceMapFileV1 {
    pub fn new(
        identity: [u8; 32],
        byte_len: u64,
        display_path: String,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        let file = Self {
            identity,
            byte_len,
            display_path,
        };
        file.validate()?;
        Ok(file)
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub fn display_path(&self) -> &str {
        &self.display_path
    }

    fn validate(&self) -> Result<(), DebugSourceMapErrorV1> {
        if self.identity == [0; 32]
            || self.byte_len == 0
            || self.display_path.is_empty()
            || self.display_path.len() > MAX_DEBUG_SOURCE_PATH_BYTES_V1
            || self.display_path.contains('\0')
        {
            return Err(DebugSourceMapErrorV1::InvalidFile);
        }
        Ok(())
    }
}

impl DebugSourceMapKirSiteV1 {
    pub const fn operation(
        function_ordinal: u64,
        block_ordinal: u64,
        operation_ordinal: u64,
    ) -> Self {
        Self {
            function_ordinal,
            block_ordinal,
            point: DebugSourceMapKirPointV1::Operation { operation_ordinal },
        }
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn point(self) -> DebugSourceMapKirPointV1 {
        self.point
    }

    pub const fn operation_ordinal(self) -> u64 {
        match self.point {
            DebugSourceMapKirPointV1::Operation { operation_ordinal } => operation_ordinal,
        }
    }

    fn validate(self) -> Result<(), DebugSourceMapErrorV1> {
        if self.function_ordinal > u64::from(u32::MAX)
            || self.block_ordinal > u64::from(u32::MAX)
            || self.operation_ordinal() > u64::from(u32::MAX)
        {
            return Err(DebugSourceMapErrorV1::InvalidSite);
        }
        Ok(())
    }
}

impl DebugSourceMapSpanV1 {
    pub fn new(
        file_identity: [u8; 32],
        byte_start: u64,
        byte_end: u64,
        line: u32,
        column: u32,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        let span = Self {
            file_identity,
            byte_start,
            byte_end,
            line,
            column,
        };
        span.validate(false)?;
        Ok(span)
    }

    /// Constructs an exact span for a source construct that emitted no KIR.
    /// rustc may represent such a construct as an empty call-site range.
    pub fn new_eliminated(
        file_identity: [u8; 32],
        byte_start: u64,
        byte_end: u64,
        line: u32,
        column: u32,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        let span = Self {
            file_identity,
            byte_start,
            byte_end,
            line,
            column,
        };
        span.validate(true)?;
        Ok(span)
    }

    pub const fn file_identity(self) -> [u8; 32] {
        self.file_identity
    }

    pub const fn byte_start(self) -> u64 {
        self.byte_start
    }

    pub const fn byte_end(self) -> u64 {
        self.byte_end
    }

    pub const fn line(self) -> u32 {
        self.line
    }

    pub const fn column(self) -> u32 {
        self.column
    }

    fn validate(self, allow_empty: bool) -> Result<(), DebugSourceMapErrorV1> {
        if self.file_identity == [0; 32]
            || self.byte_start > self.byte_end
            || (!allow_empty && self.byte_start == self.byte_end)
            || self.line == 0
            || self.column == 0
        {
            return Err(DebugSourceMapErrorV1::InvalidSpan);
        }
        Ok(())
    }
}

impl DebugSourceMapSiteV1 {
    pub fn new(
        site: DebugSourceMapKirSiteV1,
        spans: Vec<DebugSourceMapSpanV1>,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        let mut value = Self { site, spans };
        value.normalize_and_validate()?;
        Ok(value)
    }

    pub const fn site(&self) -> DebugSourceMapKirSiteV1 {
        self.site
    }

    pub fn spans(&self) -> &[DebugSourceMapSpanV1] {
        &self.spans
    }

    fn normalize_and_validate(&mut self) -> Result<(), DebugSourceMapErrorV1> {
        self.site.validate()?;
        if self.spans.is_empty() {
            return Err(DebugSourceMapErrorV1::InvalidSite);
        }
        for span in &self.spans {
            span.validate(false)?;
        }
        self.spans.sort_unstable();
        if self.spans.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DebugSourceMapErrorV1::DuplicateSpan);
        }
        Ok(())
    }
}

impl DebugSourceMapDocumentV1 {
    pub fn new(
        binding: DebugSourceMapBindingV1,
        files: Vec<DebugSourceMapFileV1>,
        sites: Vec<DebugSourceMapSiteV1>,
        eliminated: Vec<DebugSourceMapSpanV1>,
    ) -> Result<Self, DebugSourceMapErrorV1> {
        let mut document = Self {
            schema: DebugSourceMapSchemaV1::V1,
            binding,
            files,
            sites,
            eliminated,
        };
        document.normalize_and_validate()?;
        Ok(document)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DebugSourceMapErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_DEBUG_MAP_BYTES_V1 {
            return Err(DebugSourceMapErrorV1::InvalidLength);
        }
        let mut document: Self =
            serde_json::from_slice(bytes).map_err(|_| DebugSourceMapErrorV1::InvalidJson)?;
        document.normalize_and_validate()?;
        Ok(document)
    }

    /// Decodes the unique compact JSON representation used inside `.fe2sim`.
    /// Caller-bound sidecars may use any strict JSON layout accepted by
    /// [`Self::from_json_bytes`]; bundle bytes have one committed encoding.
    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, DebugSourceMapErrorV1> {
        let document = Self::from_json_bytes(bytes)?;
        let canonical =
            serde_json::to_vec(&document).map_err(|_| DebugSourceMapErrorV1::Encoding)?;
        if canonical != bytes {
            return Err(DebugSourceMapErrorV1::NonCanonicalEncoding);
        }
        Ok(document)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, DebugSourceMapErrorV1> {
        let mut document = self.clone();
        document.normalize_and_validate()?;
        let bytes = serde_json::to_vec(&document).map_err(|_| DebugSourceMapErrorV1::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_DEBUG_MAP_BYTES_V1 {
            return Err(DebugSourceMapErrorV1::InvalidLength);
        }
        Ok(bytes)
    }

    pub const fn binding(&self) -> DebugSourceMapBindingV1 {
        self.binding
    }

    pub fn files(&self) -> &[DebugSourceMapFileV1] {
        &self.files
    }

    pub fn sites(&self) -> &[DebugSourceMapSiteV1] {
        &self.sites
    }

    pub fn eliminated(&self) -> &[DebugSourceMapSpanV1] {
        &self.eliminated
    }

    fn normalize_and_validate(&mut self) -> Result<(), DebugSourceMapErrorV1> {
        DebugSourceMapBindingV1::new(
            self.binding.bundle_subject_identity,
            self.binding.canonical_kir.digest,
            self.binding.canonical_kir.canonical_bytes,
        )?;
        if self.files.is_empty()
            || self.files.len() > MAX_DEBUG_SOURCE_FILES_V1
            || self.sites.len() > MAX_DEBUG_SOURCE_SITES_V1
        {
            return Err(DebugSourceMapErrorV1::ResourceLimit);
        }
        for file in &self.files {
            file.validate()?;
        }
        self.files
            .sort_unstable_by_key(DebugSourceMapFileV1::identity);
        if self
            .files
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(DebugSourceMapErrorV1::DuplicateFile);
        }
        for site in &mut self.sites {
            site.normalize_and_validate()?;
        }
        self.sites.sort_unstable_by_key(DebugSourceMapSiteV1::site);
        if self
            .sites
            .windows(2)
            .any(|pair| pair[0].site == pair[1].site)
        {
            return Err(DebugSourceMapErrorV1::DuplicateSite);
        }
        for span in &self.eliminated {
            span.validate(true)?;
        }
        self.eliminated.sort_unstable();
        if self.eliminated.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DebugSourceMapErrorV1::DuplicateSpan);
        }
        let span_count = self
            .sites
            .iter()
            .try_fold(self.eliminated.len(), |count, site| {
                count.checked_add(site.spans.len())
            })
            .ok_or(DebugSourceMapErrorV1::ResourceLimit)?;
        if span_count > MAX_DEBUG_SOURCE_SPANS_V1 {
            return Err(DebugSourceMapErrorV1::ResourceLimit);
        }
        for span in self
            .sites
            .iter()
            .flat_map(|site| &site.spans)
            .chain(&self.eliminated)
        {
            let file = self
                .files
                .binary_search_by_key(&span.file_identity, |file| file.identity)
                .ok()
                .map(|index| &self.files[index])
                .ok_or(DebugSourceMapErrorV1::InvalidSpan)?;
            if span.byte_end > file.byte_len {
                return Err(DebugSourceMapErrorV1::InvalidSpan);
            }
        }
        Ok(())
    }
}

mod hex_identity {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut encoded = [0_u8; 64];
        for (index, byte) in value.iter().copied().enumerate() {
            encoded[index * 2] = hex(byte >> 4);
            encoded[index * 2 + 1] = hex(byte & 0x0f);
        }
        serializer.serialize_str(std::str::from_utf8(&encoded).expect("hex is ASCII"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        if text.len() != 64 || !text.is_ascii() {
            return Err(de::Error::custom(
                "identity must be exactly 64 lowercase hex bytes",
            ));
        }
        let bytes = text.as_bytes();
        let mut decoded = [0_u8; 32];
        for index in 0..32 {
            let high = nibble(bytes[index * 2])
                .ok_or_else(|| de::Error::custom("identity must use lowercase hex"))?;
            let low = nibble(bytes[index * 2 + 1])
                .ok_or_else(|| de::Error::custom("identity must use lowercase hex"))?;
            decoded[index] = (high << 4) | low;
        }
        Ok(decoded)
    }

    const fn hex(value: u8) -> u8 {
        if value < 10 {
            b'0' + value
        } else {
            b'a' + value - 10
        }
    }

    const fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> DebugSourceMapDocumentV1 {
        DebugSourceMapDocumentV1::new(
            DebugSourceMapBindingV1::new([1; 32], [2; 32], 17).unwrap(),
            vec![DebugSourceMapFileV1::new([3; 32], 64, "/src/kernel.rs".into()).unwrap()],
            vec![
                DebugSourceMapSiteV1::new(
                    DebugSourceMapKirSiteV1::operation(0, 0, 1),
                    vec![DebugSourceMapSpanV1::new([3; 32], 4, 12, 2, 3).unwrap()],
                )
                .unwrap(),
            ],
            vec![DebugSourceMapSpanV1::new([3; 32], 20, 24, 3, 1).unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn canonical_codec_round_trips_and_rejects_schema_substitution() {
        let document = document();
        let bytes = document.to_canonical_json_bytes().unwrap();
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(&bytes).unwrap(),
            document
        );
        let substituted = String::from_utf8(bytes)
            .unwrap()
            .replace(DEBUG_SOURCE_MAP_SCHEMA_V1, "fe2o3-debug-source-map-v2");
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(substituted.as_bytes()),
            Err(DebugSourceMapErrorV1::InvalidJson)
        );
    }

    #[test]
    fn strict_codec_rejects_unknown_duplicate_null_and_bounds() {
        let bytes = document().to_canonical_json_bytes().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let unknown = text.replacen("\"files\":", "\"unknown\":1,\"files\":", 1);
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(unknown.as_bytes()),
            Err(DebugSourceMapErrorV1::InvalidJson)
        );
        let duplicate = text.replacen("\"files\":", "\"files\":[],\"files\":", 1);
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(duplicate.as_bytes()),
            Err(DebugSourceMapErrorV1::InvalidJson)
        );
        let null = text.replacen("\"sites\":[", "\"sites\":null,\"ignored\":[", 1);
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(null.as_bytes()),
            Err(DebugSourceMapErrorV1::InvalidJson)
        );
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(&vec![
                b' ';
                MAX_SIMULATION_DEBUG_MAP_BYTES_V1 + 1
            ]),
            Err(DebugSourceMapErrorV1::InvalidLength)
        );
    }

    #[test]
    fn strict_codec_rejects_noncanonical_whitespace_and_field_order() {
        let bytes = document().to_canonical_json_bytes().unwrap();
        let mut whitespace = bytes.clone();
        whitespace.push(b'\n');
        assert_eq!(
            DebugSourceMapDocumentV1::from_canonical_json_bytes(&whitespace),
            Err(DebugSourceMapErrorV1::NonCanonicalEncoding)
        );

        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let reordered = serde_json::to_vec(&value).unwrap();
        assert_ne!(reordered, bytes);
        assert_eq!(
            DebugSourceMapDocumentV1::from_canonical_json_bytes(&reordered),
            Err(DebugSourceMapErrorV1::NonCanonicalEncoding)
        );
        assert_eq!(
            DebugSourceMapDocumentV1::from_json_bytes(&reordered).unwrap(),
            document()
        );
    }
}
