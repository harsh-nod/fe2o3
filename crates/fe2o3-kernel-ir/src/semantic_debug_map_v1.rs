//! Canonical cross-layer semantic debug map bound to one finalized artifact.
//!
//! This format is separate from the frozen simulation Source Map V1/V2 formats. It records
//! identity-keyed, name-independent locations and explicit adjacent-layer transformation
//! outcomes. The record is descriptive evidence only and grants no compiler, proof, artifact,
//! load, launch, or hardware authority.

use std::{error::Error, fmt, io, io::Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    DebugSourceMapDocumentV2, DebugSourceMapSpanV1, MAX_MODULE_BYTES_V1,
    VerifiedCanonicalKernelIrV7,
};

pub const SEMANTIC_DEBUG_MAP_SCHEMA_V1: &str = "fe2o3-semantic-debug-map-v1";
pub const MAX_SEMANTIC_DEBUG_MAP_BYTES_V1: usize = 64 * 1024 * 1024;
// Conservative lower bounds for one compact JSON record/reference. They are deliberately below
// the shortest currently emitted representation so every derived count is a permissive ceiling,
// while the encoded byte ceiling remains the final aggregate bound.
pub const MIN_SEMANTIC_DEBUG_NODE_WIRE_BYTES_V1: usize = 128;
pub const MIN_SEMANTIC_DEBUG_MAPPING_WIRE_BYTES_V1: usize = 192;
pub const MIN_SEMANTIC_DEBUG_REFERENCE_WIRE_BYTES_V1: usize = 66;
pub const MIN_SEMANTIC_DEBUG_BOUNDARY_WIRE_BYTES_V1: usize = 128;
pub const MAX_SEMANTIC_DEBUG_NODES_V1: usize =
    MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 / MIN_SEMANTIC_DEBUG_NODE_WIRE_BYTES_V1;
pub const MAX_SEMANTIC_DEBUG_MAPPINGS_V1: usize =
    MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 / MIN_SEMANTIC_DEBUG_MAPPING_WIRE_BYTES_V1;
pub const MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1: usize =
    MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 / MIN_SEMANTIC_DEBUG_REFERENCE_WIRE_BYTES_V1;
pub const MAX_SEMANTIC_DEBUG_BOUNDARIES_V1: usize =
    MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 / MIN_SEMANTIC_DEBUG_BOUNDARY_WIRE_BYTES_V1;

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

/// Stable meaning of every `kernel_ordinal` used by an ISA location.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugKernelOrdinalBasisV1 {
    AmdhsaMetadataDeclarationOrder,
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
    /// Half-open byte interval relative to the kernel entry symbol selected in AMDHSA metadata
    /// declaration order, never ELF symbol-table or lexical-name order.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugMapStatusV1 {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugBoundaryDirectionV1 {
    PredecessorUnavailable,
    SuccessorUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDebugBoundaryReasonV1 {
    ProducerBoundary,
    MissingDebugInformation,
    NotRepresented,
    UnsupportedLayer,
}

/// An explicit partial-map endpoint. Interior nodes missing either adjacent transformation must
/// carry the corresponding boundary; complete maps permit no boundaries.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDebugBoundaryV1 {
    #[serde(with = "hex_identity_v1")]
    node: [u8; 32],
    direction: SemanticDebugBoundaryDirectionV1,
    reason: SemanticDebugBoundaryReasonV1,
}

impl SemanticDebugBoundaryV1 {
    pub fn new(
        node: [u8; 32],
        direction: SemanticDebugBoundaryDirectionV1,
        reason: SemanticDebugBoundaryReasonV1,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        if node == [0; 32] {
            return Err(SemanticDebugMapErrorV1::InvalidBoundary);
        }
        Ok(Self {
            node,
            direction,
            reason,
        })
    }

    pub const fn node(self) -> [u8; 32] {
        self.node
    }

    pub const fn direction(self) -> SemanticDebugBoundaryDirectionV1 {
        self.direction
    }

    pub const fn reason(self) -> SemanticDebugBoundaryReasonV1 {
        self.reason
    }
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
        let output_count = self.output.nodes().len();
        let reference_count = self
            .inputs
            .len()
            .checked_add(output_count)
            .ok_or(SemanticDebugMapErrorV1::ResourceLimit)?;
        if reference_count > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
            return Err(SemanticDebugMapErrorV1::ResourceLimit);
        }
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
    kernel_ordinal_basis: SemanticDebugKernelOrdinalBasisV1,
    status: SemanticDebugMapStatusV1,
    #[serde(deserialize_with = "bounded_nodes_v1::deserialize")]
    nodes: Vec<SemanticDebugNodeV1>,
    #[serde(deserialize_with = "bounded_mappings_v1::deserialize")]
    mappings: Vec<SemanticDebugMappingV1>,
    #[serde(deserialize_with = "bounded_boundaries_v1::deserialize")]
    boundaries: Vec<SemanticDebugBoundaryV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum SemanticDebugMapSchemaV1 {
    #[serde(rename = "fe2o3-semantic-debug-map-v1")]
    V1,
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticDebugMapInputsV1<'a> {
    pub source_map_v2: &'a [u8],
    pub semantic_mir: &'a [u8],
    pub canonical_kir: &'a [u8],
    pub schedule: &'a [u8],
    pub llvm_module: &'a [u8],
    pub finalized_artifact: &'a [u8],
}

impl SemanticDebugMapDocumentV1 {
    /// Rebinds only the pre-finalization artifact axis. All compiler input axes and graph
    /// records are retained and revalidated.
    pub fn with_finalized_artifact_identity(
        self,
        finalized_artifact: SemanticDebugContentIdentityV1,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let binding = SemanticDebugMapBindingV1::new(
            self.binding.source_map_v2(),
            self.binding.semantic_mir(),
            self.binding.canonical_kir(),
            self.binding.schedule(),
            self.binding.llvm_module(),
            finalized_artifact,
        )?;
        Self::new_with_status(
            binding,
            self.status,
            self.nodes,
            self.mappings,
            self.boundaries,
        )
    }

    /// Constructs a complete map. Every non-source node must have a recorded predecessor outcome
    /// and every non-ISA node must have a recorded successor outcome.
    pub fn new(
        binding: SemanticDebugMapBindingV1,
        nodes: Vec<SemanticDebugNodeV1>,
        mappings: Vec<SemanticDebugMappingV1>,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        Self::new_with_status(
            binding,
            SemanticDebugMapStatusV1::Complete,
            nodes,
            mappings,
            Vec::new(),
        )
    }

    /// Constructs a partial map whose every missing adjacent transformation is explicit.
    pub fn new_partial(
        binding: SemanticDebugMapBindingV1,
        nodes: Vec<SemanticDebugNodeV1>,
        mappings: Vec<SemanticDebugMappingV1>,
        boundaries: Vec<SemanticDebugBoundaryV1>,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        Self::new_with_status(
            binding,
            SemanticDebugMapStatusV1::Partial,
            nodes,
            mappings,
            boundaries,
        )
    }

    fn new_with_status(
        binding: SemanticDebugMapBindingV1,
        status: SemanticDebugMapStatusV1,
        nodes: Vec<SemanticDebugNodeV1>,
        mappings: Vec<SemanticDebugMappingV1>,
        boundaries: Vec<SemanticDebugBoundaryV1>,
    ) -> Result<Self, SemanticDebugMapErrorV1> {
        let mut document = Self {
            schema: SemanticDebugMapSchemaV1::V1,
            binding,
            kernel_ordinal_basis: SemanticDebugKernelOrdinalBasisV1::AmdhsaMetadataDeclarationOrder,
            status,
            nodes,
            mappings,
            boundaries,
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
        if document.to_canonical_json_bytes()? != bytes {
            return Err(SemanticDebugMapErrorV1::NonCanonicalEncoding);
        }
        Ok(document)
    }

    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, SemanticDebugMapErrorV1> {
        let mut writer = BoundedSemanticMapWriterV1::new(MAX_SEMANTIC_DEBUG_MAP_BYTES_V1);
        serde_json::to_writer(&mut writer, self).map_err(|_| writer.error())?;
        let bytes = writer.finish()?;
        if bytes.is_empty() || bytes.len() > MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 {
            return Err(SemanticDebugMapErrorV1::InvalidLength);
        }
        Ok(bytes)
    }

    pub const fn binding(&self) -> SemanticDebugMapBindingV1 {
        self.binding
    }
    pub const fn kernel_ordinal_basis(&self) -> SemanticDebugKernelOrdinalBasisV1 {
        self.kernel_ordinal_basis
    }
    pub const fn status(&self) -> SemanticDebugMapStatusV1 {
        self.status
    }
    pub fn nodes(&self) -> &[SemanticDebugNodeV1] {
        &self.nodes
    }
    pub fn mappings(&self) -> &[SemanticDebugMappingV1] {
        &self.mappings
    }
    pub fn boundaries(&self) -> &[SemanticDebugBoundaryV1] {
        &self.boundaries
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
        if inputs.canonical_kir.len() > MAX_MODULE_BYTES_V1 {
            return Err(SemanticDebugMapErrorV1::InvalidBoundCanonicalKir);
        }
        let mut canonical_kir = Vec::new();
        canonical_kir
            .try_reserve_exact(inputs.canonical_kir.len())
            .map_err(|_| SemanticDebugMapErrorV1::AllocationFailure)?;
        canonical_kir.extend_from_slice(inputs.canonical_kir);
        let admitted_kir = VerifiedCanonicalKernelIrV7::from_canonical_bytes(canonical_kir)
            .map_err(|_| SemanticDebugMapErrorV1::InvalidBoundCanonicalKir)?;
        let decoded_kir = crate::decode_module_v7(admitted_kir.canonical_bytes())
            .map_err(|_| SemanticDebugMapErrorV1::InvalidBoundCanonicalKir)?;
        let source_map_kir = source_map.binding().canonical_kir();
        if source_map_kir.digest() != *admitted_kir.identity().digest()
            || source_map_kir.canonical_bytes() != admitted_kir.identity().canonical_length()
        {
            return Err(SemanticDebugMapErrorV1::SourceMapKirBindingMismatch);
        }
        self.validate_source_nodes(&source_map)?;
        self.validate_kir_nodes(&decoded_kir)
    }

    /// Revalidates the artifact binding and every symbol-relative ISA interval. Entry sizes must
    /// be in AMDHSA metadata declaration order, matching `kernel_ordinal_basis`.
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
            if !source_map
                .sites()
                .iter()
                .any(|site| site.spans().contains(&span))
            {
                return Err(SemanticDebugMapErrorV1::InvalidSourceLocation);
            }
        }
        Ok(())
    }

    fn validate_kir_nodes(&self, module: &crate::Module) -> Result<(), SemanticDebugMapErrorV1> {
        for node in &self.nodes {
            let SemanticDebugLocationV1::Kir {
                function_ordinal,
                block_ordinal,
                operation_ordinal,
            } = node.location
            else {
                continue;
            };
            let exists = usize::try_from(function_ordinal)
                .ok()
                .and_then(|ordinal| module.functions.get(ordinal))
                .and_then(|function| function.body.as_ref())
                .and_then(|body| {
                    usize::try_from(block_ordinal)
                        .ok()
                        .and_then(|ordinal| body.blocks.get(ordinal))
                })
                .is_some_and(|block| {
                    usize::try_from(operation_ordinal)
                        .ok()
                        .is_some_and(|ordinal| ordinal < block.operations.len())
                });
            if !exists {
                return Err(SemanticDebugMapErrorV1::InvalidKirLocation);
            }
        }
        Ok(())
    }

    fn normalize_and_validate(&mut self) -> Result<(), SemanticDebugMapErrorV1> {
        self.binding.validate()?;
        if self.kernel_ordinal_basis
            != SemanticDebugKernelOrdinalBasisV1::AmdhsaMetadataDeclarationOrder
        {
            return Err(SemanticDebugMapErrorV1::InvalidKernelOrdinalBasis);
        }
        if self.nodes.is_empty()
            || self.mappings.is_empty()
            || self.nodes.len() > MAX_SEMANTIC_DEBUG_NODES_V1
            || self.mappings.len() > MAX_SEMANTIC_DEBUG_MAPPINGS_V1
            || self.boundaries.len() > MAX_SEMANTIC_DEBUG_BOUNDARIES_V1
        {
            return Err(SemanticDebugMapErrorV1::ResourceLimit);
        }
        match self.status {
            SemanticDebugMapStatusV1::Complete if !self.boundaries.is_empty() => {
                return Err(SemanticDebugMapErrorV1::InvalidBoundary);
            }
            SemanticDebugMapStatusV1::Partial if self.boundaries.is_empty() => {
                return Err(SemanticDebugMapErrorV1::UntypedBoundary);
            }
            _ => {}
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
        if self
            .boundaries
            .iter()
            .any(|boundary| boundary.node == [0; 32])
        {
            return Err(SemanticDebugMapErrorV1::InvalidBoundary);
        }
        self.boundaries.sort_unstable();
        if self
            .boundaries
            .windows(2)
            .any(|pair| pair[0].node == pair[1].node && pair[0].direction == pair[1].direction)
        {
            return Err(SemanticDebugMapErrorV1::InvalidBoundary);
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
        let expected_capacity = self
            .nodes
            .len()
            .checked_mul(2)
            .ok_or(SemanticDebugMapErrorV1::ResourceLimit)?;
        let mut missing = Vec::new();
        missing
            .try_reserve_exact(expected_capacity)
            .map_err(|_| SemanticDebugMapErrorV1::AllocationFailure)?;
        for node in &self.nodes {
            let is_consumed = consumed.binary_search(&node.identity).is_ok();
            let is_produced = produced.binary_search(&node.identity).is_ok();
            if node.layer() != SemanticDebugLayerV1::Source && !is_produced {
                missing.push((
                    node.identity,
                    SemanticDebugBoundaryDirectionV1::PredecessorUnavailable,
                ));
            }
            if node.layer() != SemanticDebugLayerV1::Isa && !is_consumed {
                missing.push((
                    node.identity,
                    SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                ));
            }
        }
        missing.sort_unstable();
        match self.status {
            SemanticDebugMapStatusV1::Complete if !missing.is_empty() => {
                return Err(SemanticDebugMapErrorV1::UntypedBoundary);
            }
            SemanticDebugMapStatusV1::Partial => {
                let mut declared = Vec::new();
                declared
                    .try_reserve_exact(self.boundaries.len())
                    .map_err(|_| SemanticDebugMapErrorV1::AllocationFailure)?;
                for boundary in &self.boundaries {
                    self.node(boundary.node)
                        .ok_or(SemanticDebugMapErrorV1::InvalidBoundary)?;
                    declared.push((boundary.node, boundary.direction));
                }
                if declared != missing {
                    return Err(SemanticDebugMapErrorV1::UntypedBoundary);
                }
            }
            SemanticDebugMapStatusV1::Complete => {}
        }
        Ok(())
    }
}

struct BoundedSemanticMapWriterV1 {
    bytes: Vec<u8>,
    max: usize,
    exceeded: bool,
    allocation_failed: bool,
}

impl BoundedSemanticMapWriterV1 {
    fn new(max: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max,
            exceeded: false,
            allocation_failed: false,
        }
    }

    fn error(&self) -> SemanticDebugMapErrorV1 {
        if self.exceeded {
            SemanticDebugMapErrorV1::InvalidLength
        } else if self.allocation_failed {
            SemanticDebugMapErrorV1::AllocationFailure
        } else {
            SemanticDebugMapErrorV1::Encoding
        }
    }

    fn finish(self) -> Result<Vec<u8>, SemanticDebugMapErrorV1> {
        if self.exceeded {
            Err(SemanticDebugMapErrorV1::InvalidLength)
        } else if self.allocation_failed {
            Err(SemanticDebugMapErrorV1::AllocationFailure)
        } else {
            Ok(self.bytes)
        }
    }
}

impl Write for BoundedSemanticMapWriterV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let Some(new_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("semantic debug map length overflow"));
        };
        if new_len > self.max {
            self.exceeded = true;
            return Err(io::Error::other("semantic debug map exceeds wire limit"));
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(io::Error::other("semantic debug map allocation failed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
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
    InvalidKernelOrdinalBasis,
    InvalidNode,
    InvalidMapping,
    DuplicateNode,
    DuplicateMapping,
    DuplicateReference,
    UnknownNode,
    LayerMismatch,
    ContradictoryMapping,
    OrphanNode,
    InvalidBoundary,
    UntypedBoundary,
    ResourceLimit,
    AllocationFailure,
    ContentBindingMismatch,
    ArtifactBindingMismatch,
    InvalidBoundSourceMap,
    InvalidBoundCanonicalKir,
    SourceMapKirBindingMismatch,
    InvalidSourceLocation,
    InvalidMirLocation,
    InvalidKirLocation,
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
    use std::fmt;

    use serde::{
        Deserializer, Serializer,
        de::{self, SeqAccess, Visitor},
        ser::SerializeSeq,
    };

    use super::{HexIdentityV1, MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1};

    pub fn serialize<S>(values: &[[u8; 32]], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            sequence.serialize_element(&HexIdentityV1(*value))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<[u8; 32]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdentityVisitorV1;

        impl<'de> Visitor<'de> for IdentityVisitorV1 {
            type Value = Vec<[u8; 32]>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of semantic node identities")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(identity) = sequence.next_element::<HexIdentityV1>()? {
                    if values.len() == MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
                        return Err(de::Error::custom(
                            "semantic mapping reference count exceeds wire-derived limit",
                        ));
                    }
                    if values.len() == values.capacity() {
                        let remaining = MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 - values.len();
                        values.try_reserve_exact(remaining.min(1024)).map_err(|_| {
                            de::Error::custom("semantic identity allocation failed")
                        })?;
                    }
                    values.push(identity.0);
                }
                Ok(values)
            }
        }

        deserializer.deserialize_seq(IdentityVisitorV1)
    }
}

fn deserialize_bounded_vec_v1<'de, D, T>(
    deserializer: D,
    max: usize,
    description: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    use std::{fmt, marker::PhantomData};

    use serde::de::{self, SeqAccess, Visitor};

    struct BoundedVecVisitorV1<T> {
        max: usize,
        description: &'static str,
        marker: PhantomData<T>,
    }

    impl<'de, T> Visitor<'de> for BoundedVecVisitorV1<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a bounded sequence of {}", self.description)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if values.len() == self.max {
                    return Err(de::Error::custom(
                        "semantic map sequence exceeds wire limit",
                    ));
                }
                if values.len() == values.capacity() {
                    let remaining = self.max - values.len();
                    values.try_reserve_exact(remaining.min(1024)).map_err(|_| {
                        de::Error::custom("semantic map sequence allocation failed")
                    })?;
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitorV1 {
        max,
        description,
        marker: PhantomData,
    })
}

mod bounded_nodes_v1 {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<SemanticDebugNodeV1>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v1(deserializer, MAX_SEMANTIC_DEBUG_NODES_V1, "semantic nodes")
    }
}

mod bounded_mappings_v1 {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<SemanticDebugMappingV1>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v1(
            deserializer,
            MAX_SEMANTIC_DEBUG_MAPPINGS_V1,
            "semantic mappings",
        )
    }
}

mod bounded_boundaries_v1 {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<SemanticDebugBoundaryV1>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bounded_vec_v1(
            deserializer,
            MAX_SEMANTIC_DEBUG_BOUNDARIES_V1,
            "semantic boundaries",
        )
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(transparent)]
struct HexIdentityV1(#[serde(with = "hex_identity_v1")] [u8; 32]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BasicBlock, BlockId, Constant, DebugSourceMapBindingV1, DebugSourceMapFileV1,
        DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, Function, Module, Operation, OperationKind,
        ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
        VerifiedCanonicalKernelIrIdentityV7,
    };

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn canonical_kir(module_id: &str) -> (Vec<u8>, VerifiedCanonicalKernelIrIdentityV7) {
        let mut module = Module::new(module_id);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(1)),
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        module.functions.push(Function::definition(
            "mapped",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        ));
        let owner = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
        let identity = *owner.identity();
        (owner.into_canonical_bytes(), identity)
    }

    fn source_map(kir_identity: VerifiedCanonicalKernelIrIdentityV7) -> Vec<u8> {
        source_map_with_sites(kir_identity, true)
    }

    fn source_map_with_sites(
        kir_identity: VerifiedCanonicalKernelIrIdentityV7,
        include_site: bool,
    ) -> Vec<u8> {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                id(80),
                *kir_identity.digest(),
                kir_identity.canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new(id(1), 128, "/src/kernel.rs".into()).unwrap()],
            include_site
                .then(|| {
                    DebugSourceMapSiteV1::new(
                        DebugSourceMapKirSiteV1::operation(0, 0, 0),
                        vec![DebugSourceMapSpanV1::new(id(1), 4, 8, 1, 5).unwrap()],
                    )
                    .unwrap()
                })
                .into_iter()
                .collect(),
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
            let (kir, kir_identity) = canonical_kir("semantic-debug-map-test");
            Self {
                source_map: source_map(kir_identity),
                mir: b"semantic-mir".to_vec(),
                kir,
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
                    operation_ordinal: 0,
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

    fn artifact_tail_map(
        inputs: &Inputs,
        kernel_ordinal: u64,
        byte_end: u64,
    ) -> SemanticDebugMapDocumentV1 {
        let nodes = vec![
            node(
                30,
                SemanticDebugLocationV1::Llvm {
                    function_ordinal: 0,
                    block_ordinal: 0,
                    instruction_ordinal: 0,
                },
            ),
            node(
                31,
                SemanticDebugLocationV1::Isa {
                    kernel_ordinal,
                    byte_start: 0,
                    byte_end,
                },
            ),
        ];
        let mappings = vec![mapping(
            32,
            SemanticDebugLayerV1::Llvm,
            SemanticDebugLayerV1::Isa,
            SemanticDebugTransformationV1::Preserved,
            &[30],
            &[31],
        )];
        SemanticDebugMapDocumentV1::new_partial(
            inputs.binding(),
            nodes,
            mappings,
            vec![
                SemanticDebugBoundaryV1::new(
                    id(30),
                    SemanticDebugBoundaryDirectionV1::PredecessorUnavailable,
                    SemanticDebugBoundaryReasonV1::ProducerBoundary,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn full_cross_layer_map_round_trips_and_queries_both_directions() {
        let inputs = Inputs::new();
        let map = full_map(&inputs);
        assert_eq!(map.status(), SemanticDebugMapStatusV1::Complete);
        assert!(map.boundaries().is_empty());
        assert_eq!(
            map.kernel_ordinal_basis(),
            SemanticDebugKernelOrdinalBasisV1::AmdhsaMetadataDeclarationOrder
        );
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
    fn partial_maps_require_exact_boundaries_for_every_interior_gap() {
        let inputs = Inputs::new();
        let partial = artifact_tail_map(&inputs, 0, 8);
        assert_eq!(partial.status(), SemanticDebugMapStatusV1::Partial);
        assert_eq!(partial.boundaries().len(), 1);
        assert_eq!(
            SemanticDebugMapDocumentV1::new(
                inputs.binding(),
                partial.nodes().to_vec(),
                partial.mappings().to_vec(),
            ),
            Err(SemanticDebugMapErrorV1::UntypedBoundary)
        );
        assert_eq!(
            SemanticDebugMapDocumentV1::new_partial(
                inputs.binding(),
                partial.nodes().to_vec(),
                partial.mappings().to_vec(),
                vec![
                    SemanticDebugBoundaryV1::new(
                        id(31),
                        SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                        SemanticDebugBoundaryReasonV1::NotRepresented,
                    )
                    .unwrap()
                ],
            ),
            Err(SemanticDebugMapErrorV1::UntypedBoundary)
        );
    }

    #[test]
    fn isa_kernel_ordinals_use_amdhsa_metadata_declaration_order() {
        let inputs = Inputs::new();
        let second_kernel = artifact_tail_map(&inputs, 1, 12);
        second_kernel
            .validate_finalized_artifact(&inputs.artifact, &[8, 16])
            .unwrap();
        assert_eq!(
            second_kernel.validate_finalized_artifact(&inputs.artifact, &[16, 8]),
            Err(SemanticDebugMapErrorV1::InvalidIsaInterval)
        );
        assert_eq!(
            second_kernel.validate_finalized_artifact(&inputs.artifact, &[8, 11]),
            Err(SemanticDebugMapErrorV1::InvalidIsaInterval)
        );
        assert_eq!(
            second_kernel.validate_finalized_artifact(&inputs.artifact, &[16]),
            Err(SemanticDebugMapErrorV1::InvalidIsaInterval)
        );
    }

    #[test]
    fn wire_derived_reference_limit_fails_before_sorting_hostile_inputs() {
        assert_eq!(
            MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1,
            MAX_SEMANTIC_DEBUG_MAP_BYTES_V1 / MIN_SEMANTIC_DEBUG_REFERENCE_WIRE_BYTES_V1
        );
        let derived_references = std::hint::black_box(MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1);
        assert!(
            derived_references * MIN_SEMANTIC_DEBUG_REFERENCE_WIRE_BYTES_V1
                <= MAX_SEMANTIC_DEBUG_MAP_BYTES_V1
        );
        let oversized = vec![id(1); MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 + 1];
        assert_eq!(
            SemanticDebugMappingV1::new(
                id(90),
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Preserved,
                oversized,
                SemanticDebugMappingOutputV1::available(vec![id(2)]),
            ),
            Err(SemanticDebugMapErrorV1::ResourceLimit)
        );

        let mut writer = BoundedSemanticMapWriterV1::new(8);
        writer.write_all(&[0; 8]).unwrap();
        assert!(writer.write_all(&[0]).is_err());
        assert_eq!(writer.error(), SemanticDebugMapErrorV1::InvalidLength);
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

        let (_, other_kir_identity) = canonical_kir("substituted-semantic-debug-map-test");
        let substituted_source_map = source_map(other_kir_identity);
        let substituted_inputs = Inputs {
            source_map: substituted_source_map,
            mir: inputs.mir.clone(),
            kir: inputs.kir.clone(),
            schedule: inputs.schedule.clone(),
            llvm: inputs.llvm.clone(),
            artifact: inputs.artifact.clone(),
        };
        let substituted_map = SemanticDebugMapDocumentV1::new(
            substituted_inputs.binding(),
            map.nodes().to_vec(),
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            substituted_map.validate_exact_inputs(substituted_inputs.borrowed()),
            Err(SemanticDebugMapErrorV1::SourceMapKirBindingMismatch)
        );

        let invalid_kir = b"not-canonical-kir-v7";
        let invalid_inputs = Inputs {
            source_map: inputs.source_map.clone(),
            mir: inputs.mir.clone(),
            kir: invalid_kir.to_vec(),
            schedule: inputs.schedule.clone(),
            llvm: inputs.llvm.clone(),
            artifact: inputs.artifact.clone(),
        };
        let invalid_map = SemanticDebugMapDocumentV1::new(
            invalid_inputs.binding(),
            map.nodes().to_vec(),
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            invalid_map.validate_exact_inputs(invalid_inputs.borrowed()),
            Err(SemanticDebugMapErrorV1::InvalidBoundCanonicalKir)
        );

        let mut invalid_source_nodes = map.nodes().to_vec();
        let source_index = invalid_source_nodes
            .iter()
            .position(|node| node.layer() == SemanticDebugLayerV1::Source)
            .unwrap();
        invalid_source_nodes[source_index] = node(
            10,
            SemanticDebugLocationV1::Source {
                span: DebugSourceMapSpanV1::new(id(1), 8, 12, 2, 1).unwrap(),
            },
        );
        let invalid_source = SemanticDebugMapDocumentV1::new(
            inputs.binding(),
            invalid_source_nodes,
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            invalid_source.validate_exact_inputs(inputs.borrowed()),
            Err(SemanticDebugMapErrorV1::InvalidSourceLocation)
        );

        let (_, kir_identity) = canonical_kir("semantic-debug-map-test");
        let empty_source_sites = Inputs {
            source_map: source_map_with_sites(kir_identity, false),
            mir: inputs.mir.clone(),
            kir: inputs.kir.clone(),
            schedule: inputs.schedule.clone(),
            llvm: inputs.llvm.clone(),
            artifact: inputs.artifact.clone(),
        };
        let empty_source_map = SemanticDebugMapDocumentV1::new(
            empty_source_sites.binding(),
            map.nodes().to_vec(),
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            empty_source_map.validate_exact_inputs(empty_source_sites.borrowed()),
            Err(SemanticDebugMapErrorV1::InvalidSourceLocation)
        );

        let mut invalid_kir_nodes = map.nodes().to_vec();
        let kir_index = invalid_kir_nodes
            .iter()
            .position(|node| node.layer() == SemanticDebugLayerV1::Kir)
            .unwrap();
        invalid_kir_nodes[kir_index] = node(
            12,
            SemanticDebugLocationV1::Kir {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 1,
            },
        );
        let invalid_kir = SemanticDebugMapDocumentV1::new(
            inputs.binding(),
            invalid_kir_nodes,
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            invalid_kir.validate_exact_inputs(inputs.borrowed()),
            Err(SemanticDebugMapErrorV1::InvalidKirLocation)
        );

        let empty_kir = VerifiedCanonicalKernelIrV7::from_module(Module::new("empty")).unwrap();
        let empty_kir_identity = *empty_kir.identity();
        let empty_kir_inputs = Inputs {
            source_map: source_map(empty_kir_identity),
            mir: inputs.mir.clone(),
            kir: empty_kir.into_canonical_bytes(),
            schedule: inputs.schedule.clone(),
            llvm: inputs.llvm.clone(),
            artifact: inputs.artifact.clone(),
        };
        let empty_kir_map = SemanticDebugMapDocumentV1::new(
            empty_kir_inputs.binding(),
            map.nodes().to_vec(),
            map.mappings().to_vec(),
        )
        .unwrap();
        assert_eq!(
            empty_kir_map.validate_exact_inputs(empty_kir_inputs.borrowed()),
            Err(SemanticDebugMapErrorV1::InvalidKirLocation)
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
