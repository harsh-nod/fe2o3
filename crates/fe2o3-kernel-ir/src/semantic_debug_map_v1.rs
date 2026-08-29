//! Canonical cross-layer semantic debug map bound to one finalized artifact.
//!
//! This format is separate from the frozen simulation Source Map V1/V2 formats. It records
//! identity-keyed, name-independent locations and explicit adjacent-layer transformation
//! outcomes. The record is descriptive evidence only and grants no compiler, proof, artifact,
//! load, launch, or hardware authority.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{DebugSourceMapDocumentV2, DebugSourceMapSpanV1};

pub const SEMANTIC_DEBUG_MAP_SCHEMA_V1: &str = "fe2o3-semantic-debug-map-v1";
pub const MAX_SEMANTIC_DEBUG_MAP_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_SEMANTIC_DEBUG_NODES_V1: usize = 1_000_000;
pub const MAX_SEMANTIC_DEBUG_MAPPINGS_V1: usize = 2_000_000;
pub const MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1: usize = 8_000_000;

const SEMANTIC_DEBUG_MAP_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SEMANTIC-DEBUG-MAP/V1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugContentIdentityV1 {
    #[serde(with = "hex_identity_v1")]
    sha256: [u8; 32],
    byte_len: u64,
}

impl SemanticDebugContentIdentityV1 {
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, SemanticDebugMapErrorV1> {
        let identity = Self { sha256, byte_len };
        identity.validate()?;
        Ok(identity)
    }

    pub fn calculate(bytes: &[u8]) -> Result<Self, SemanticDebugMapErrorV1> {
        Self::new(Sha256::digest(bytes).into(), bytes.len() as u64)
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }

    fn validate(self) -> Result<(), SemanticDebugMapErrorV1> {
        if self.sha256 == [0; 32] || self.byte_len == 0 {
            Err(SemanticDebugMapErrorV1::InvalidBinding)
        } else {
            Ok(())
        }
    }
}

/// Exact content axes needed to interpret one semantic map.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugMapBindingV1 {
    source_map_v2: SemanticDebugContentIdentityV1,
    semantic_mir: SemanticDebugContentIdentityV1,
    canonical_kir: SemanticDebugContentIdentityV1,
    schedule: SemanticDebugContentIdentityV1,
    llvm_module: SemanticDebugContentIdentityV1,
    finalized_artifact: SemanticDebugContentIdentityV1,
}

impl SemanticDebugMapBindingV1 {
    pub fn new(
        source_map_v2: SemanticDebugContentIdentityV1,
        semantic_mir: SemanticDebugContentIdentityV1,
        canonical_kir: SemanticDebugContentIdentityV1,
        schedule: SemanticDebugContentIdentityV1,
        llvm_module: SemanticDebugContentIdentityV1,
        finalized_artifact: SemanticDebugContentIdentityV1,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let binding = Self {
            source_map_v2,
            semantic_mir,
            canonical_kir,
            schedule,
            llvm_module,
            finalized_artifact,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub const fn source_map_v2(self) -> SemanticDebugContentIdentityV1 {
        self.source_map_v2
    }
    pub const fn semantic_mir(self) -> SemanticDebugContentIdentityV1 {
        self.semantic_mir
    }
    pub const fn canonical_kir(self) -> SemanticDebugContentIdentityV1 {
        self.canonical_kir
    }
    pub const fn schedule(self) -> SemanticDebugContentIdentityV1 {
        self.schedule
    }
    pub const fn llvm_module(self) -> SemanticDebugContentIdentityV1 {
        self.llvm_module
    }
    pub const fn finalized_artifact(self) -> SemanticDebugContentIdentityV1 {
        self.finalized_artifact
    }

    fn validate(self) -> Result<(), SemanticDebugMapErrorV1> {
        for identity in [
            self.source_map_v2,
            self.semantic_mir,
            self.canonical_kir,
            self.schedule,
            self.llvm_module,
            self.finalized_artifact,
        ] {
            identity.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugLayerV1 {
    Source,
    Mir,
    Kir,
    Schedule,
    Llvm,
    Isa,
}

impl SemanticDebugLayerV1 {
    const fn successor(self) -> Option<Self> {
        match self {
            Self::Source => Some(Self::Mir),
            Self::Mir => Some(Self::Kir),
            Self::Kir => Some(Self::Schedule),
            Self::Schedule => Some(Self::Llvm),
            Self::Llvm => Some(Self::Isa),
            Self::Isa => None,
        }
    }
}

/// A name-independent location in one compiler or artifact layer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(tag = "layer", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticDebugLocationV1 {
    Source {
        span: DebugSourceMapSpanV1,
    },
    Mir {
        body_ordinal: u64,
        block_ordinal: u64,
        statement_ordinal: u64,
    },
    Kir {
        function_ordinal: u64,
        block_ordinal: u64,
        operation_ordinal: u64,
    },
    Schedule {
        function_ordinal: u64,
        region_ordinal: u64,
        operation_ordinal: u64,
    },
    Llvm {
        function_ordinal: u64,
        block_ordinal: u64,
        instruction_ordinal: u64,
    },
    /// Half-open byte interval relative to the ordinal-selected kernel entry symbol.
    Isa {
        kernel_ordinal: u64,
        byte_start: u64,
        byte_end: u64,
    },
}

impl SemanticDebugLocationV1 {
    pub const fn layer(self) -> SemanticDebugLayerV1 {
        match self {
            Self::Source { .. } => SemanticDebugLayerV1::Source,
            Self::Mir { .. } => SemanticDebugLayerV1::Mir,
            Self::Kir { .. } => SemanticDebugLayerV1::Kir,
            Self::Schedule { .. } => SemanticDebugLayerV1::Schedule,
            Self::Llvm { .. } => SemanticDebugLayerV1::Llvm,
            Self::Isa { .. } => SemanticDebugLayerV1::Isa,
        }
    }

    fn validate(self) -> Result<(), SemanticDebugMapErrorV1> {
        let ordinal_is_invalid = |ordinal: u64| ordinal > u64::from(u32::MAX);
        match self {
            Self::Source { span } => DebugSourceMapSpanV1::new(
                span.file_identity(),
                span.byte_start(),
                span.byte_end(),
                span.line(),
                span.column(),
            )
            .map(|_| ())
            .map_err(|_| SemanticDebugMapErrorV1::InvalidNode),
            Self::Mir {
                body_ordinal,
                block_ordinal,
                statement_ordinal,
            } => validate_ordinals(&[body_ordinal, block_ordinal, statement_ordinal]),
            Self::Kir {
                function_ordinal,
                block_ordinal,
                operation_ordinal,
            } => validate_ordinals(&[function_ordinal, block_ordinal, operation_ordinal]),
            Self::Schedule {
                function_ordinal,
                region_ordinal,
                operation_ordinal,
            } => validate_ordinals(&[function_ordinal, region_ordinal, operation_ordinal]),
            Self::Llvm {
                function_ordinal,
                block_ordinal,
                instruction_ordinal,
            } => validate_ordinals(&[function_ordinal, block_ordinal, instruction_ordinal]),
            Self::Isa {
                kernel_ordinal,
                byte_start,
                byte_end,
            } => {
                if ordinal_is_invalid(kernel_ordinal)
                    || byte_start >= byte_end
                    || !byte_start.is_multiple_of(4)
                    || !byte_end.is_multiple_of(4)
                {
                    Err(SemanticDebugMapErrorV1::InvalidNode)
                } else {
                    Ok(())
                }
            }
        }
    }
}

fn validate_ordinals(ordinals: &[u64]) -> Result<(), SemanticDebugMapErrorV1> {
    if ordinals
        .iter()
        .any(|ordinal| *ordinal > u64::from(u32::MAX))
    {
        Err(SemanticDebugMapErrorV1::InvalidNode)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugNodeV1 {
    #[serde(with = "hex_identity_v1")]
    identity: [u8; 32],
    location: SemanticDebugLocationV1,
}

impl SemanticDebugNodeV1 {
    pub fn new(
        identity: [u8; 32],
        location: SemanticDebugLocationV1,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let node = Self { identity, location };
        node.validate()?;
        Ok(node)
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn location(self) -> SemanticDebugLocationV1 {
        self.location
    }
    pub const fn layer(self) -> SemanticDebugLayerV1 {
        self.location.layer()
    }

    fn validate(self) -> Result<(), SemanticDebugMapErrorV1> {
        if self.identity == [0; 32] {
            return Err(SemanticDebugMapErrorV1::InvalidNode);
        }
        self.location.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugTransformationV1 {
    Preserved,
    Duplicated,
    Fused,
    Inlined,
    Moved,
    Eliminated,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugUnavailableReasonV1 {
    Eliminated,
    OptimizedOut,
    NotEmitted,
    MissingDebugInformation,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticDebugMappingOutputV1 {
    Available {
        #[serde(with = "hex_identities_v1")]
        nodes: Vec<[u8; 32]>,
    },
    Unavailable {
        reason: SemanticDebugUnavailableReasonV1,
    },
}

impl SemanticDebugMappingOutputV1 {
    pub fn available(nodes: Vec<[u8; 32]>) -> Self {
        Self::Available { nodes }
    }
    pub const fn unavailable(reason: SemanticDebugUnavailableReasonV1) -> Self {
        Self::Unavailable { reason }
    }
    pub fn nodes(&self) -> &[[u8; 32]] {
        match self {
            Self::Available { nodes } => nodes,
            Self::Unavailable { .. } => &[],
        }
    }
    pub const fn reason(&self) -> Option<SemanticDebugUnavailableReasonV1> {
        match self {
            Self::Available { .. } => None,
            Self::Unavailable { reason } => Some(*reason),
        }
    }
}

/// One explicit adjacent-layer transformation. Node references are stable identities, never
/// function or symbol names.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugMappingV1 {
    #[serde(with = "hex_identity_v1")]
    identity: [u8; 32],
    input_layer: SemanticDebugLayerV1,
    output_layer: SemanticDebugLayerV1,
    transformation: SemanticDebugTransformationV1,
    #[serde(with = "hex_identities_v1")]
    inputs: Vec<[u8; 32]>,
    output: SemanticDebugMappingOutputV1,
}

impl SemanticDebugMappingV1 {
    pub fn new(
        identity: [u8; 32],
        input_layer: SemanticDebugLayerV1,
        output_layer: SemanticDebugLayerV1,
        transformation: SemanticDebugTransformationV1,
        inputs: Vec<[u8; 32]>,
        output: SemanticDebugMappingOutputV1,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let mut mapping = Self {
            identity,
            input_layer,
            output_layer,
            transformation,
            inputs,
            output,
        };
        mapping.normalize_and_validate_shape()?;
        Ok(mapping)
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn input_layer(&self) -> SemanticDebugLayerV1 {
        self.input_layer
    }
    pub const fn output_layer(&self) -> SemanticDebugLayerV1 {
        self.output_layer
    }
    pub const fn transformation(&self) -> SemanticDebugTransformationV1 {
        self.transformation
    }
    pub fn inputs(&self) -> &[[u8; 32]] {
        &self.inputs
    }
    pub const fn output(&self) -> &SemanticDebugMappingOutputV1 {
        &self.output
    }

    fn normalize_and_validate_shape(&mut self) -> Result<(), SemanticDebugMapErrorV1> {
        if self.identity == [0; 32]
            || self.input_layer.successor() != Some(self.output_layer)
            || self.inputs.is_empty()
        {
            return Err(SemanticDebugMapErrorV1::InvalidMapping);
        }
        self.inputs.sort_unstable();
        if self.inputs.windows(2).any(|pair| pair[0] == pair[1]) || self.inputs.contains(&[0; 32]) {
            return Err(SemanticDebugMapErrorV1::DuplicateReference);
        }
        if let SemanticDebugMappingOutputV1::Available { nodes } = &mut self.output {
            nodes.sort_unstable();
            if nodes.is_empty()
                || nodes.windows(2).any(|pair| pair[0] == pair[1])
                || nodes.contains(&[0; 32])
            {
                return Err(SemanticDebugMapErrorV1::DuplicateReference);
            }
        }
        let input_count = self.inputs.len();
        let output_count = self.output.nodes().len();
        let available = self.output.reason().is_none();
        let valid = match self.transformation {
            SemanticDebugTransformationV1::Preserved | SemanticDebugTransformationV1::Moved => {
                available && input_count == 1 && output_count == 1
            }
            SemanticDebugTransformationV1::Duplicated => {
                available && input_count == 1 && output_count >= 2
            }
            SemanticDebugTransformationV1::Fused => {
                available && input_count >= 2 && output_count >= 1
            }
            SemanticDebugTransformationV1::Inlined => available,
            SemanticDebugTransformationV1::Eliminated => {
                self.output.reason() == Some(SemanticDebugUnavailableReasonV1::Eliminated)
            }
            SemanticDebugTransformationV1::Unavailable => self
                .output
                .reason()
                .is_some_and(|reason| reason != SemanticDebugUnavailableReasonV1::Eliminated),
        };
        if valid {
            Ok(())
        } else {
            Err(SemanticDebugMapErrorV1::InvalidMapping)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugMapDocumentV1 {
    schema: SemanticDebugMapSchemaV1,
    binding: SemanticDebugMapBindingV1,
    nodes: Vec<SemanticDebugNodeV1>,
    mappings: Vec<SemanticDebugMappingV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum SemanticDebugMapSchemaV1 {
    #[serde(rename = "fe2o3-semantic-debug-map-v1")]
    V1,
}

pub struct SemanticDebugMapInputsV1<'a> {
    pub source_map_v2: &'a [u8],
    pub semantic_mir: &'a [u8],
    pub canonical_kir: &'a [u8],
    pub schedule: &'a [u8],
    pub llvm_module: &'a [u8],
    pub finalized_artifact: &'a [u8],
}

impl SemanticDebugMapDocumentV1 {
    pub fn new(
        binding: SemanticDebugMapBindingV1,
        nodes: Vec<SemanticDebugNodeV1>,
        mappings: Vec<SemanticDebugMappingV1>,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let mut document = Self {
            schema: SemanticDebugMapSchemaV1::V1,
            binding,
            nodes,
            mappings,
        };
        document.normalize_and_validate()?;
        Ok(document)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, SemanticDebugMapErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 {
            return Err(SemanticDebugMapErrorV1::InvalidLength);
        }
        let mut document = serde_json::from_slice::<Self>(bytes)
            .map_err(|_| SemanticDebugMapErrorV1::InvalidJson)?;
        document.normalize_and_validate()?;
        Ok(document)
    }

    pub fn from_canonical_json_bytes(bytes: &[u8]) -> Result<Self, SemanticDebugMapErrorV1> {
        let document = Self::from_json_bytes(bytes)?;
        if serde_json::to_vec(&document).map_err(|_| SemanticDebugMapErrorV1::Encoding)? != bytes {
            return Err(SemanticDebugMapErrorV1::NonCanonicalEncoding);
        }
        Ok(document)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SemanticDebugMapErrorV1> {
        let mut document = self.clone();
        document.normalize_and_validate()?;
        let bytes = serde_json::to_vec(&document).map_err(|_| SemanticDebugMapErrorV1::Encoding)?;
        if bytes.is_empty() || bytes.len() > MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 {
            return Err(SemanticDebugMapErrorV1::InvalidLength);
        }
        Ok(bytes)
    }

    pub const fn binding(&self) -> SemanticDebugMapBindingV1 {
        self.binding
    }
    pub fn nodes(&self) -> &[SemanticDebugNodeV1] {
        &self.nodes
    }
    pub fn mappings(&self) -> &[SemanticDebugMappingV1] {
        &self.mappings
    }
    pub fn node(&self, identity: [u8; 32]) -> Option<&SemanticDebugNodeV1> {
        self.nodes
            .binary_search_by_key(&identity, SemanticDebugNodeV1::identity)
            .ok()
            .map(|index| &self.nodes[index])
    }

    /// Exact location lookup. The result is deterministic and may contain multiple stable nodes.
    pub fn nodes_at(
        &self,
        location: SemanticDebugLocationV1,
    ) -> impl Iterator<Item = &SemanticDebugNodeV1> {
        self.nodes
            .iter()
            .filter(move |node| node.location() == location)
    }

    /// Returns the unique next-layer transformation consuming this node, if one was recorded.
    pub fn mapping_from(&self, identity: [u8; 32]) -> Option<&SemanticDebugMappingV1> {
        self.mappings
            .iter()
            .find(|mapping| mapping.inputs.binary_search(&identity).is_ok())
    }

    /// Returns the unique previous-layer transformation producing this node, if one was recorded.
    pub fn mapping_to(&self, identity: [u8; 32]) -> Option<&SemanticDebugMappingV1> {
        self.mappings
            .iter()
            .find(|mapping| mapping.output.nodes().binary_search(&identity).is_ok())
    }

    /// Revalidates every exact content axis and the Source Map V2 file/span relationship.
    pub fn validate_exact_inputs(
        &self,
        inputs: SemanticDebugMapInputsV1<'_>,
    ) -> Result<(), SemanticDebugMapErrorV1> {
        let claims = [
            (self.binding.source_map_v2, inputs.source_map_v2),
            (self.binding.semantic_mir, inputs.semantic_mir),
            (self.binding.canonical_kir, inputs.canonical_kir),
            (self.binding.schedule, inputs.schedule),
            (self.binding.llvm_module, inputs.llvm_module),
            (self.binding.finalized_artifact, inputs.finalized_artifact),
        ];
        if claims
            .iter()
            .any(|(identity, bytes)| !identity.matches(bytes))
        {
            return Err(SemanticDebugMapErrorV1::ContentBindingMismatch);
        }
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(inputs.source_map_v2)
            .map_err(|_| SemanticDebugMapErrorV1::InvalidBoundSourceMap)?;
        self.validate_source_nodes(&source_map)
    }

    /// Revalidates the artifact binding and every symbol-relative ISA interval.
    pub fn validate_finalized_artifact(
        &self,
        artifact: &[u8],
        kernel_entry_sizes: &[u64],
    ) -> Result<(), SemanticDebugMapErrorV1> {
        if !self.binding.finalized_artifact.matches(artifact) {
            return Err(SemanticDebugMapErrorV1::ArtifactBindingMismatch);
        }
        for node in &self.nodes {
            let SemanticDebugLocationV1::Isa {
                kernel_ordinal,
                byte_start: _,
                byte_end,
            } = node.location
            else {
                continue;
            };
            let size = usize::try_from(kernel_ordinal)
                .ok()
                .and_then(|ordinal| kernel_entry_sizes.get(ordinal))
                .ok_or(SemanticDebugMapErrorV1::InvalidIsaInterval)?;
            if byte_end > *size {
                return Err(SemanticDebugMapErrorV1::InvalidIsaInterval);
            }
        }
        Ok(())
    }

    fn validate_source_nodes(
        &self,
        source_map: &DebugSourceMapDocumentV2,
    ) -> Result<(), SemanticDebugMapErrorV1> {
        for node in &self.nodes {
            let SemanticDebugLocationV1::Source { span } = node.location else {
                continue;
            };
            let file = source_map
                .files()
                .binary_search_by_key(&span.file_identity(), |file| file.identity())
                .ok()
                .map(|index| &source_map.files()[index])
                .ok_or(SemanticDebugMapErrorV1::InvalidBoundSourceMap)?;
            if span.byte_end() > file.byte_len() {
                return Err(SemanticDebugMapErrorV1::InvalidBoundSourceMap);
            }
        }
        Ok(())
    }

    fn normalize_and_validate(&mut self) -> Result<(), SemanticDebugMapErrorV1> {
        self.binding.validate()?;
        if self.nodes.is_empty()
            || self.mappings.is_empty()
            || self.nodes.len() > MAX_SEMANTIC_DEBUG_NODES_V1
            || self.mappings.len() > MAX_SEMANTIC_DEBUG_MAPPINGS_V1
        {
            return Err(SemanticDebugMapErrorV1::ResourceLimit);
        }
        for node in &self.nodes {
            node.validate()?;
        }
        self.nodes
            .sort_unstable_by_key(SemanticDebugNodeV1::identity);
        if self
            .nodes
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(SemanticDebugMapErrorV1::DuplicateNode);
        }
        for mapping in &mut self.mappings {
            mapping.normalize_and_validate_shape()?;
        }
        self.mappings
            .sort_unstable_by_key(SemanticDebugMappingV1::identity);
        if self
            .mappings
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err(SemanticDebugMapErrorV1::DuplicateMapping);
        }
        let references = self.mappings.iter().try_fold(0_usize, |count, mapping| {
            count
                .checked_add(mapping.inputs.len())
                .and_then(|count| count.checked_add(mapping.output.nodes().len()))
        });
        let Some(reference_count) = references else {
            return Err(SemanticDebugMapErrorV1::ResourceLimit);
        };
        if reference_count > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
            return Err(SemanticDebugMapErrorV1::ResourceLimit);
        }

        let mut consumed = Vec::new();
        let mut produced = Vec::new();
        consumed
            .try_reserve_exact(reference_count)
            .map_err(|_| SemanticDebugMapErrorV1::AllocationFailure)?;
        produced
            .try_reserve_exact(reference_count)
            .map_err(|_| SemanticDebugMapErrorV1::AllocationFailure)?;
        for mapping in &self.mappings {
            for identity in &mapping.inputs {
                let node = self
                    .node(*identity)
                    .ok_or(SemanticDebugMapErrorV1::UnknownNode)?;
                if node.layer() != mapping.input_layer {
                    return Err(SemanticDebugMapErrorV1::LayerMismatch);
                }
                consumed.push(*identity);
            }
            for identity in mapping.output.nodes() {
                let node = self
                    .node(*identity)
                    .ok_or(SemanticDebugMapErrorV1::UnknownNode)?;
                if node.layer() != mapping.output_layer {
                    return Err(SemanticDebugMapErrorV1::LayerMismatch);
                }
                produced.push(*identity);
            }
        }
        consumed.sort_unstable();
        produced.sort_unstable();
        if consumed.windows(2).any(|pair| pair[0] == pair[1])
            || produced.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(SemanticDebugMapErrorV1::ContradictoryMapping);
        }
        for node in &self.nodes {
            let appears = consumed.binary_search(&node.identity).is_ok()
                || produced.binary_search(&node.identity).is_ok();
            if !appears {
                return Err(SemanticDebugMapErrorV1::OrphanNode);
            }
        }
        Ok(())
    }
}

pub fn semantic_debug_map_identity_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SEMANTIC_DEBUG_MAP_IDENTITY_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticDebugMapErrorV1 {
    InvalidLength,
    InvalidJson,
    NonCanonicalEncoding,
    Encoding,
    InvalidBinding,
    InvalidNode,
    InvalidMapping,
    DuplicateNode,
    DuplicateMapping,
    DuplicateReference,
    UnknownNode,
    LayerMismatch,
    ContradictoryMapping,
    OrphanNode,
    ResourceLimit,
    AllocationFailure,
    ContentBindingMismatch,
    ArtifactBindingMismatch,
    InvalidBoundSourceMap,
    InvalidIsaInterval,
}

impl fmt::Display for SemanticDebugMapErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 semantic debug map: {self:?}")
    }
}

impl Error for SemanticDebugMapErrorV1 {}

mod hex_identity_v1 {
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

mod hex_identities_v1 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::HexIdentityV1;

    pub fn serialize<S>(values: &[[u8; 32]], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        values
            .iter()
            .copied()
            .map(HexIdentityV1)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<[u8; 32]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Vec::<HexIdentityV1>::deserialize(deserializer)?
            .into_iter()
            .map(|identity| identity.0)
            .collect())
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(transparent)]
struct HexIdentityV1(#[serde(with = "hex_identity_v1")] [u8; 32]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DebugSourceMapBindingV1, DebugSourceMapFileV1, DebugSourceMapSiteV1};

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn source_map() -> Vec<u8> {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(id(80), id(81), 3).unwrap(),
            vec![DebugSourceMapFileV1::new(id(1), 128, "/src/kernel.rs".into()).unwrap()],
            Vec::<DebugSourceMapSiteV1>::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap()
    }

    struct Inputs {
        source_map: Vec<u8>,
        mir: Vec<u8>,
        kir: Vec<u8>,
        schedule: Vec<u8>,
        llvm: Vec<u8>,
        artifact: Vec<u8>,
    }

    impl Inputs {
        fn new() -> Self {
            Self {
                source_map: source_map(),
                mir: b"semantic-mir".to_vec(),
                kir: b"canonical-kir".to_vec(),
                schedule: b"schedule".to_vec(),
                llvm: b"llvm-module".to_vec(),
                artifact: b"finalized-hsaco".to_vec(),
            }
        }
        fn binding(&self) -> SemanticDebugMapBindingV1 {
            SemanticDebugMapBindingV1::new(
                SemanticDebugContentIdentityV1::calculate(&self.source_map).unwrap(),
                SemanticDebugContentIdentityV1::calculate(&self.mir).unwrap(),
                SemanticDebugContentIdentityV1::calculate(&self.kir).unwrap(),
                SemanticDebugContentIdentityV1::calculate(&self.schedule).unwrap(),
                SemanticDebugContentIdentityV1::calculate(&self.llvm).unwrap(),
                SemanticDebugContentIdentityV1::calculate(&self.artifact).unwrap(),
            )
            .unwrap()
        }
        fn borrowed(&self) -> SemanticDebugMapInputsV1<'_> {
            SemanticDebugMapInputsV1 {
                source_map_v2: &self.source_map,
                semantic_mir: &self.mir,
                canonical_kir: &self.kir,
                schedule: &self.schedule,
                llvm_module: &self.llvm,
                finalized_artifact: &self.artifact,
            }
        }
    }

    fn node(identity: u8, location: SemanticDebugLocationV1) -> SemanticDebugNodeV1 {
        SemanticDebugNodeV1::new(id(identity), location).unwrap()
    }

    fn mapping(
        identity: u8,
        input_layer: SemanticDebugLayerV1,
        output_layer: SemanticDebugLayerV1,
        transformation: SemanticDebugTransformationV1,
        inputs: &[u8],
        outputs: &[u8],
    ) -> SemanticDebugMappingV1 {
        SemanticDebugMappingV1::new(
            id(identity),
            input_layer,
            output_layer,
            transformation,
            inputs.iter().copied().map(id).collect(),
            SemanticDebugMappingOutputV1::available(outputs.iter().copied().map(id).collect()),
        )
        .unwrap()
    }

    fn full_map(inputs: &Inputs) -> SemanticDebugMapDocumentV1 {
        let nodes = vec![
            node(
                10,
                SemanticDebugLocationV1::Source {
                    span: DebugSourceMapSpanV1::new(id(1), 4, 8, 1, 5).unwrap(),
                },
            ),
            node(
                11,
                SemanticDebugLocationV1::Mir {
                    body_ordinal: 0,
                    block_ordinal: 0,
                    statement_ordinal: 1,
                },
            ),
            node(
                12,
                SemanticDebugLocationV1::Kir {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    operation_ordinal: 2,
                },
            ),
            node(
                13,
                SemanticDebugLocationV1::Schedule {
                    function_ordinal: 0,
                    region_ordinal: 1,
                    operation_ordinal: 3,
                },
            ),
            node(
                14,
                SemanticDebugLocationV1::Llvm {
                    function_ordinal: 0,
                    block_ordinal: 1,
                    instruction_ordinal: 4,
                },
            ),
            node(
                15,
                SemanticDebugLocationV1::Isa {
                    kernel_ordinal: 0,
                    byte_start: 4,
                    byte_end: 12,
                },
            ),
        ];
        let mappings = vec![
            mapping(
                20,
                SemanticDebugLayerV1::Source,
                SemanticDebugLayerV1::Mir,
                SemanticDebugTransformationV1::Preserved,
                &[10],
                &[11],
            ),
            mapping(
                21,
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Moved,
                &[11],
                &[12],
            ),
            mapping(
                22,
                SemanticDebugLayerV1::Kir,
                SemanticDebugLayerV1::Schedule,
                SemanticDebugTransformationV1::Preserved,
                &[12],
                &[13],
            ),
            mapping(
                23,
                SemanticDebugLayerV1::Schedule,
                SemanticDebugLayerV1::Llvm,
                SemanticDebugTransformationV1::Inlined,
                &[13],
                &[14],
            ),
            mapping(
                24,
                SemanticDebugLayerV1::Llvm,
                SemanticDebugLayerV1::Isa,
                SemanticDebugTransformationV1::Preserved,
                &[14],
                &[15],
            ),
        ];
        SemanticDebugMapDocumentV1::new(inputs.binding(), nodes, mappings).unwrap()
    }

    #[test]
    fn full_cross_layer_map_round_trips_and_queries_both_directions() {
        let inputs = Inputs::new();
        let map = full_map(&inputs);
        map.validate_exact_inputs(inputs.borrowed()).unwrap();
        map.validate_finalized_artifact(&inputs.artifact, &[64])
            .unwrap();
        let bytes = map.to_canonical_json_bytes().unwrap();
        let decoded = SemanticDebugMapDocumentV1::from_canonical_json_bytes(&bytes).unwrap();
        assert_eq!(decoded, map);
        assert_eq!(decoded.mapping_from(id(10)).unwrap().identity(), id(20));
        assert_eq!(decoded.mapping_to(id(15)).unwrap().identity(), id(24));
        assert_eq!(
            decoded
                .nodes_at(SemanticDebugLocationV1::Isa {
                    kernel_ordinal: 0,
                    byte_start: 4,
                    byte_end: 12
                })
                .count(),
            1
        );
        assert_ne!(semantic_debug_map_identity_v1(&bytes), [0; 32]);
    }

    #[test]
    fn optimization_shapes_and_typed_absence_are_explicit() {
        let duplicated = SemanticDebugMappingV1::new(
            id(40),
            SemanticDebugLayerV1::Mir,
            SemanticDebugLayerV1::Kir,
            SemanticDebugTransformationV1::Duplicated,
            vec![id(1)],
            SemanticDebugMappingOutputV1::available(vec![id(2), id(3)]),
        )
        .unwrap();
        assert_eq!(duplicated.output().nodes().len(), 2);
        let fused = SemanticDebugMappingV1::new(
            id(41),
            SemanticDebugLayerV1::Kir,
            SemanticDebugLayerV1::Schedule,
            SemanticDebugTransformationV1::Fused,
            vec![id(1), id(2)],
            SemanticDebugMappingOutputV1::available(vec![id(3)]),
        )
        .unwrap();
        assert_eq!(fused.inputs().len(), 2);
        let eliminated = SemanticDebugMappingV1::new(
            id(42),
            SemanticDebugLayerV1::Schedule,
            SemanticDebugLayerV1::Llvm,
            SemanticDebugTransformationV1::Eliminated,
            vec![id(1)],
            SemanticDebugMappingOutputV1::unavailable(SemanticDebugUnavailableReasonV1::Eliminated),
        )
        .unwrap();
        assert_eq!(
            eliminated.output().reason(),
            Some(SemanticDebugUnavailableReasonV1::Eliminated)
        );
        assert_eq!(
            SemanticDebugMappingV1::new(
                id(43),
                SemanticDebugLayerV1::Llvm,
                SemanticDebugLayerV1::Isa,
                SemanticDebugTransformationV1::Eliminated,
                vec![id(1)],
                SemanticDebugMappingOutputV1::unavailable(
                    SemanticDebugUnavailableReasonV1::OptimizedOut
                )
            ),
            Err(SemanticDebugMapErrorV1::InvalidMapping)
        );
    }

    #[test]
    fn stale_or_substituted_content_and_isa_ranges_fail_closed() {
        let inputs = Inputs::new();
        let map = full_map(&inputs);
        assert_eq!(
            map.validate_exact_inputs(SemanticDebugMapInputsV1 {
                canonical_kir: b"substituted-kir",
                ..inputs.borrowed()
            }),
            Err(SemanticDebugMapErrorV1::ContentBindingMismatch)
        );
        let mut artifact = inputs.artifact.clone();
        artifact[0] ^= 1;
        assert_eq!(
            map.validate_finalized_artifact(&artifact, &[64]),
            Err(SemanticDebugMapErrorV1::ArtifactBindingMismatch)
        );
        assert_eq!(
            map.validate_finalized_artifact(&inputs.artifact, &[8]),
            Err(SemanticDebugMapErrorV1::InvalidIsaInterval)
        );
        assert_eq!(
            map.validate_finalized_artifact(&inputs.artifact, &[]),
            Err(SemanticDebugMapErrorV1::InvalidIsaInterval)
        );
    }

    #[test]
    fn hostile_graphs_and_noncanonical_json_fail_closed() {
        let inputs = Inputs::new();
        let bytes = full_map(&inputs).to_canonical_json_bytes().unwrap();
        let mut trailing = bytes.clone();
        trailing.push(b'\n');
        assert_eq!(
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(&trailing),
            Err(SemanticDebugMapErrorV1::NonCanonicalEncoding)
        );
        let unknown = String::from_utf8(bytes.clone()).unwrap().replacen(
            "\"nodes\":",
            "\"unknown\":1,\"nodes\":",
            1,
        );
        assert_eq!(
            SemanticDebugMapDocumentV1::from_json_bytes(unknown.as_bytes()),
            Err(SemanticDebugMapErrorV1::InvalidJson)
        );
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["binding"]["finalized_artifact"]["sha256"] =
            serde_json::Value::String("00".repeat(32));
        assert_eq!(
            SemanticDebugMapDocumentV1::from_json_bytes(&serde_json::to_vec(&value).unwrap()),
            Err(SemanticDebugMapErrorV1::InvalidBinding)
        );
    }
}
