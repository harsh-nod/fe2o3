//! Strict source-map V2 wire model for exact source-variable locations.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV1, DebugSourceMapFileV1, DebugSourceMapSiteV1,
    DebugSourceMapSpanV1, MAX_SIMULATION_DEBUG_MAP_BYTES_V1,
};

pub const DEBUG_SOURCE_MAP_SCHEMA_V2: &str = "fe2o3-debug-source-map-v2";
pub const MAX_DEBUG_SOURCE_SCOPES_V2: usize = 65_536;
pub const MAX_DEBUG_SOURCE_VARIABLES_V2: usize = 65_536;
pub const MAX_DEBUG_SOURCE_VARIABLE_LOCATIONS_V2: usize = 1_000_000;
pub const MAX_DEBUG_SOURCE_VARIABLE_NAME_BYTES_V2: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceMapDocumentV2 {
    schema: DebugSourceMapSchemaV2,
    binding: DebugSourceMapBindingV1,
    files: Vec<DebugSourceMapFileV1>,
    sites: Vec<DebugSourceMapSiteV1>,
    eliminated: Vec<DebugSourceMapSpanV1>,
    scopes: Vec<DebugSourceScopeV2>,
    variables: Vec<DebugSourceVariableV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum DebugSourceMapSchemaV2 {
    #[serde(rename = "fe2o3-debug-source-map-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceScopeV2 {
    #[serde(with = "hex_identity_v2")]
    identity: [u8; 32],
    function_ordinal: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_identity: Option<HexIdentityV2>,
    depth: u32,
    span: DebugSourceMapSpanV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
struct HexIdentityV2(#[serde(with = "hex_identity_v2")] [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSourceVariableFallbackV2 {
    NotInScope,
    OptimizedOut,
    Unrepresented,
    NotCaptured,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceVariableV2 {
    #[serde(with = "hex_identity_v2")]
    identity: [u8; 32],
    name: String,
    function_ordinal: u64,
    #[serde(with = "hex_identity_v2")]
    scope_identity: [u8; 32],
    fallback: DebugSourceVariableFallbackV2,
    locations: Vec<DebugSourceVariableLocationV2>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSourceVariableLocationV2 {
    block_ordinal: u64,
    next_operation: u64,
    generation: u64,
    binding: DebugSourceVariableBindingV2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugSourceVariableBindingV2 {
    NotInScope,
    Uninitialized,
    Ambiguous,
    NotCaptured,
    OptimizedOut,
    Unrepresented,
    Captured { value_ordinal: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSourceMapErrorV2 {
    InvalidLength,
    InvalidJson,
    InvalidV1Content,
    InvalidIdentity,
    InvalidScope,
    InvalidVariable,
    InvalidLocation,
    DuplicateScope,
    DuplicateVariable,
    DuplicateLocation,
    ResourceLimit,
    AllocationFailure,
    Encoding,
    NonCanonicalEncoding,
}

impl fmt::Display for DebugSourceMapErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 debug source map V2: {self:?}")
    }
}

impl Error for DebugSourceMapErrorV2 {}

impl DebugSourceScopeV2 {
    pub fn new(
        identity: [u8; 32],
        function_ordinal: u64,
        parent_identity: Option<[u8; 32]>,
        depth: u32,
        span: DebugSourceMapSpanV1,
    ) -> Result<Self, DebugSourceMapErrorV2> {
        let scope = Self {
            identity,
            function_ordinal,
            parent_identity: parent_identity.map(HexIdentityV2),
            depth,
            span,
        };
        scope.validate_shallow()?;
        Ok(scope)
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn parent_identity(self) -> Option<[u8; 32]> {
        match self.parent_identity {
            Some(identity) => Some(identity.0),
            None => None,
        }
    }

    pub const fn depth(self) -> u32 {
        self.depth
    }

    pub const fn span(self) -> DebugSourceMapSpanV1 {
        self.span
    }

    fn validate_shallow(self) -> Result<(), DebugSourceMapErrorV2> {
        if self.identity == [0; 32]
            || self.function_ordinal > u64::from(u32::MAX)
            || self
                .parent_identity
                .is_some_and(|parent| parent.0 == [0; 32])
            || (self.parent_identity.is_none() != (self.depth == 0))
        {
            return Err(DebugSourceMapErrorV2::InvalidScope);
        }
        Ok(())
    }
}

impl DebugSourceVariableLocationV2 {
    pub fn new(
        block_ordinal: u64,
        next_operation: u64,
        generation: u64,
        binding: DebugSourceVariableBindingV2,
    ) -> Result<Self, DebugSourceMapErrorV2> {
        let location = Self {
            block_ordinal,
            next_operation,
            generation,
            binding,
        };
        location.validate()?;
        Ok(location)
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn next_operation(self) -> u64 {
        self.next_operation
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn binding(self) -> DebugSourceVariableBindingV2 {
        self.binding
    }

    fn validate(self) -> Result<(), DebugSourceMapErrorV2> {
        if self.block_ordinal > u64::from(u32::MAX)
            || self.next_operation > u64::from(u32::MAX)
            || (self.generation == 0
                && !matches!(self.binding, DebugSourceVariableBindingV2::NotInScope))
        {
            return Err(DebugSourceMapErrorV2::InvalidLocation);
        }
        Ok(())
    }
}

impl DebugSourceVariableV2 {
    pub fn new(
        identity: [u8; 32],
        name: String,
        function_ordinal: u64,
        scope_identity: [u8; 32],
        fallback: DebugSourceVariableFallbackV2,
        locations: Vec<DebugSourceVariableLocationV2>,
    ) -> Result<Self, DebugSourceMapErrorV2> {
        let mut variable = Self {
            identity,
            name,
            function_ordinal,
            scope_identity,
            fallback,
            locations,
        };
        variable.normalize_and_validate()?;
        Ok(variable)
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn function_ordinal(&self) -> u64 {
        self.function_ordinal
    }

    pub const fn scope_identity(&self) -> [u8; 32] {
        self.scope_identity
    }

    pub const fn fallback(&self) -> DebugSourceVariableFallbackV2 {
        self.fallback
    }

    pub fn locations(&self) -> &[DebugSourceVariableLocationV2] {
        &self.locations
    }

    fn normalize_and_validate(&mut self) -> Result<(), DebugSourceMapErrorV2> {
        if self.identity == [0; 32]
            || self.scope_identity == [0; 32]
            || self.function_ordinal > u64::from(u32::MAX)
            || self.name.is_empty()
            || self.name.len() > MAX_DEBUG_SOURCE_VARIABLE_NAME_BYTES_V2
            || self.name.chars().any(char::is_control)
        {
            return Err(DebugSourceMapErrorV2::InvalidVariable);
        }
        for location in &self.locations {
            location.validate()?;
        }
        self.locations
            .sort_unstable_by_key(|location| (location.block_ordinal, location.next_operation));
        if self.locations.windows(2).any(|pair| {
            pair[0].block_ordinal == pair[1].block_ordinal
                && pair[0].next_operation == pair[1].next_operation
        }) {
            return Err(DebugSourceMapErrorV2::DuplicateLocation);
        }
        if self.locations.iter().enumerate().any(|(index, location)| {
            (index == 0 || self.locations[index - 1].block_ordinal != location.block_ordinal)
                && location.next_operation != 0
        }) {
            return Err(DebugSourceMapErrorV2::InvalidLocation);
        }
        Ok(())
    }
}

impl DebugSourceMapDocumentV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: DebugSourceMapBindingV1,
        files: Vec<DebugSourceMapFileV1>,
        sites: Vec<DebugSourceMapSiteV1>,
        eliminated: Vec<DebugSourceMapSpanV1>,
        scopes: Vec<DebugSourceScopeV2>,
        variables: Vec<DebugSourceVariableV2>,
    ) -> Result<Self, DebugSourceMapErrorV2> {
        let mut document = Self {
            schema: DebugSourceMapSchemaV2::V2,
            binding,
            files,
            sites,
            eliminated,
            scopes,
            variables,
        };
        document.normalize_and_validate()?;
        Ok(document)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DebugSourceMapErrorV2> {
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_DEBUG_MAP_BYTES_V1 {
            return Err(DebugSourceMapErrorV2::InvalidLength);
        }
        let mut document: Self =
            serde_json::from_slice(bytes).map_err(|_| DebugSourceMapErrorV2::InvalidJson)?;
        document.normalize_and_validate()?;
        Ok(document)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, DebugSourceMapErrorV2> {
        let document = Self::from_json_bytes(bytes)?;
        let canonical =
            serde_json::to_vec(&document).map_err(|_| DebugSourceMapErrorV2::Encoding)?;
        if canonical != bytes {
            return Err(DebugSourceMapErrorV2::NonCanonicalEncoding);
        }
        Ok(document)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, DebugSourceMapErrorV2> {
        let mut document = self.clone();
        document.normalize_and_validate()?;
        let bytes = serde_json::to_vec(&document).map_err(|_| DebugSourceMapErrorV2::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SIMULATION_DEBUG_MAP_BYTES_V1 {
            return Err(DebugSourceMapErrorV2::InvalidLength);
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

    pub fn scopes(&self) -> &[DebugSourceScopeV2] {
        &self.scopes
    }

    pub fn variables(&self) -> &[DebugSourceVariableV2] {
        &self.variables
    }

    fn normalize_and_validate(&mut self) -> Result<(), DebugSourceMapErrorV2> {
        let v1 = DebugSourceMapDocumentV1::new(
            self.binding,
            self.files.clone(),
            self.sites.clone(),
            self.eliminated.clone(),
        )
        .map_err(|_| DebugSourceMapErrorV2::InvalidV1Content)?;
        self.files = v1.files().to_vec();
        self.sites = v1.sites().to_vec();
        self.eliminated = v1.eliminated().to_vec();
        if self.scopes.len() > MAX_DEBUG_SOURCE_SCOPES_V2
            || self.variables.len() > MAX_DEBUG_SOURCE_VARIABLES_V2
        {
            return Err(DebugSourceMapErrorV2::ResourceLimit);
        }
        for scope in &self.scopes {
            scope.validate_shallow()?;
            let span = scope.span();
            DebugSourceMapSpanV1::new(
                span.file_identity(),
                span.byte_start(),
                span.byte_end(),
                span.line(),
                span.column(),
            )
            .map_err(|_| DebugSourceMapErrorV2::InvalidScope)?;
            let file = self
                .files
                .binary_search_by_key(&span.file_identity(), DebugSourceMapFileV1::identity)
                .ok()
                .map(|index| &self.files[index])
                .ok_or(DebugSourceMapErrorV2::InvalidScope)?;
            if span.byte_end() > file.byte_len() {
                return Err(DebugSourceMapErrorV2::InvalidScope);
            }
        }
        self.scopes.sort_unstable_by_key(|scope| scope.identity);
        if self
            .scopes
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(DebugSourceMapErrorV2::DuplicateScope);
        }
        for index in 0..self.scopes.len() {
            let scope = self.scopes[index];
            if let Some(parent) = scope.parent_identity() {
                let parent = self
                    .scopes
                    .binary_search_by_key(&parent, |candidate| candidate.identity)
                    .ok()
                    .map(|parent| self.scopes[parent])
                    .ok_or(DebugSourceMapErrorV2::InvalidScope)?;
                if parent.function_ordinal != scope.function_ordinal
                    || parent.depth.checked_add(1) != Some(scope.depth)
                {
                    return Err(DebugSourceMapErrorV2::InvalidScope);
                }
            }
        }
        for variable in &mut self.variables {
            variable.normalize_and_validate()?;
        }
        self.variables
            .sort_unstable_by_key(|variable| variable.identity);
        if self
            .variables
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(DebugSourceMapErrorV2::DuplicateVariable);
        }
        let location_count = self
            .variables
            .iter()
            .try_fold(0_usize, |count, variable| {
                count.checked_add(variable.locations.len())
            })
            .ok_or(DebugSourceMapErrorV2::ResourceLimit)?;
        if location_count > MAX_DEBUG_SOURCE_VARIABLE_LOCATIONS_V2 {
            return Err(DebugSourceMapErrorV2::ResourceLimit);
        }
        for variable in &self.variables {
            let scope = self
                .scopes
                .binary_search_by_key(&variable.scope_identity, |candidate| candidate.identity)
                .ok()
                .map(|scope| self.scopes[scope])
                .ok_or(DebugSourceMapErrorV2::InvalidVariable)?;
            if scope.function_ordinal != variable.function_ordinal {
                return Err(DebugSourceMapErrorV2::InvalidVariable);
            }
        }
        Ok(())
    }
}

mod hex_identity_v2 {
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
        let mut decoded = [0_u8; 32];
        for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            let high = nibble(pair[0])
                .ok_or_else(|| de::Error::custom("identity must use lowercase hex"))?;
            let low = nibble(pair[1])
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
    use crate::{DebugSourceMapKirSiteV1, DebugSourceMapSiteV1};

    fn document() -> DebugSourceMapDocumentV2 {
        let span = DebugSourceMapSpanV1::new([3; 32], 4, 12, 2, 3).unwrap();
        let root = DebugSourceScopeV2::new([4; 32], 0, None, 0, span).unwrap();
        let nested = DebugSourceScopeV2::new([5; 32], 0, Some([4; 32]), 1, span).unwrap();
        let variable = DebugSourceVariableV2::new(
            [6; 32],
            "item".into(),
            0,
            [5; 32],
            DebugSourceVariableFallbackV2::NotInScope,
            vec![
                DebugSourceVariableLocationV2::new(
                    0,
                    0,
                    1,
                    DebugSourceVariableBindingV2::Uninitialized,
                )
                .unwrap(),
                DebugSourceVariableLocationV2::new(
                    0,
                    2,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 7 },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new([1; 32], [2; 32], 17).unwrap(),
            vec![DebugSourceMapFileV1::new([3; 32], 64, "/src/kernel.rs".into()).unwrap()],
            vec![
                DebugSourceMapSiteV1::new(DebugSourceMapKirSiteV1::operation(0, 0, 1), vec![span])
                    .unwrap(),
            ],
            Vec::new(),
            vec![nested, root],
            vec![variable],
        )
        .unwrap()
    }

    #[test]
    fn canonical_v2_round_trip_preserves_v1_rejection() {
        let document = document();
        let bytes = document.to_canonical_json_bytes().unwrap();
        assert_eq!(
            DebugSourceMapDocumentV2::from_canonical_json_bytes(&bytes).unwrap(),
            document
        );
        assert!(DebugSourceMapDocumentV1::from_json_bytes(&bytes).is_err());
        let mut whitespace = bytes.clone();
        whitespace.insert(0, b' ');
        assert_eq!(
            DebugSourceMapDocumentV2::from_canonical_json_bytes(&whitespace),
            Err(DebugSourceMapErrorV2::NonCanonicalEncoding)
        );
    }

    #[test]
    fn rejects_scope_cycles_and_nonzero_first_checkpoints() {
        let bytes = document().to_canonical_json_bytes().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let cyclic = text.replace(
            "\"parent_identity\":\"0404040404040404040404040404040404040404040404040404040404040404\",\"depth\":1",
            "\"parent_identity\":\"0505050505050505050505050505050505050505050505050505050505050505\",\"depth\":1",
        );
        assert!(DebugSourceMapDocumentV2::from_json_bytes(cyclic.as_bytes()).is_err());
        let nonzero_first = text.replace(
            "\"block_ordinal\":0,\"next_operation\":0",
            "\"block_ordinal\":0,\"next_operation\":1",
        );
        assert_eq!(
            DebugSourceMapDocumentV2::from_json_bytes(nonzero_first.as_bytes()),
            Err(DebugSourceMapErrorV2::InvalidLocation)
        );
    }

    #[test]
    fn hostile_fields_and_generation_zero_fail_closed() {
        let bytes = document().to_canonical_json_bytes().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let unknown = text.replacen("\"variables\":[", "\"unknown\":0,\"variables\":[", 1);
        assert_eq!(
            DebugSourceMapDocumentV2::from_json_bytes(unknown.as_bytes()),
            Err(DebugSourceMapErrorV2::InvalidJson)
        );
        let generation = text.replacen("\"generation\":1", "\"generation\":0", 1);
        assert_eq!(
            DebugSourceMapDocumentV2::from_json_bytes(generation.as_bytes()),
            Err(DebugSourceMapErrorV2::InvalidLocation)
        );
        let unknown_scope_file = text.replace(
            "\"scopes\":[{\"identity\":\"0404040404040404040404040404040404040404040404040404040404040404\",\"function_ordinal\":0,\"depth\":0,\"span\":{\"file_identity\":\"0303030303030303030303030303030303030303030303030303030303030303\"",
            "\"scopes\":[{\"identity\":\"0404040404040404040404040404040404040404040404040404040404040404\",\"function_ordinal\":0,\"depth\":0,\"span\":{\"file_identity\":\"0909090909090909090909090909090909090909090909090909090909090909\"",
        );
        assert_eq!(
            DebugSourceMapDocumentV2::from_json_bytes(unknown_scope_file.as_bytes()),
            Err(DebugSourceMapErrorV2::InvalidScope)
        );
    }
}
