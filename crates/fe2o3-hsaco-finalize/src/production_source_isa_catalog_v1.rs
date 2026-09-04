//! Durable, name-independent production Source-to-sparse-ISA observations.
//!
//! The catalog is a bounded canonical projection of an already admitted production correlation.
//! It retains every admitted record needed for exact source, KIR, compiler-handoff LLVM, and
//! sparse final-HSACO lookups. Decoding proves canonical catalog structure and identity only; it
//! does not reacquire compiler custody or artifact, publication, runtime, debugger, or profiler
//! authority.

use std::{error::Error, fmt};

use dialect_amdgcn::{ProductionReplayKernelIrVersionV1, ProductionTargetStructuralBindingV1};
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    DebugSourceMapSpanV1, MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1, MAX_SEMANTIC_DEBUG_NODES_V1,
    SemanticDebugLocationV1,
};
use sha2::{Digest, Sha256};

use crate::{
    AdmittedProductionSourceIsaCorrelationV1, AdmittedProductionSourceIsaRecordV1,
    ContentIdentityV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    ProductionSemanticAnchorTransformationV1, ProductionSourceIsaCorrelationErrorV1,
    ProductionSourceIsaCorrelationUnavailableV1, ProductionSourceIsaRecordKindV1,
    production_source_isa_correlation_v1::UnboxedProductionSourceIsaCorrelationAdmissionV1,
};

pub const PRODUCTION_SOURCE_ISA_CATALOG_MAGIC_V1: [u8; 8] = *b"F2SICAT1";
pub const PRODUCTION_SOURCE_ISA_CATALOG_VERSION_V1: u16 = 1;
pub const MAX_PRODUCTION_SOURCE_ISA_CATALOG_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1: usize =
    MAX_SEMANTIC_DEBUG_NODES_V1 + dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHORS_V1;
pub const MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1: usize =
    MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1;

const CATALOG_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-SOURCE-ISA-CATALOG/V1\0";
const CATALOG_HEADER_BYTES_V1: usize = 336;
const CATALOG_IDENTITY_BYTES_V1: usize = 32;
const RECORD_HEADER_BYTES_V1: usize = 8;
const SOURCE_SPAN_BYTES_V1: usize = 56;
const COORDINATE_BYTES_V1: usize = 24;
const ISA_INTERVAL_BYTES_V1: usize = 24;
const NO_SOURCE_RECORD_FIXED_FIELDS_BYTES_V1: usize =
    COORDINATE_BYTES_V1 + 32 + COORDINATE_BYTES_V1;
const MIN_VALID_RECORD_BYTES_V1: usize =
    RECORD_HEADER_BYTES_V1 + NO_SOURCE_RECORD_FIXED_FIELDS_BYTES_V1;

const SOURCE_NODE_FLAG_V1: u16 = 1 << 0;
const SOURCE_SPAN_FLAG_V1: u16 = 1 << 1;
const MIR_NODE_FLAG_V1: u16 = 1 << 2;
const MIR_FLAG_V1: u16 = 1 << 3;
const NEUTRAL_KIR_NODE_FLAG_V1: u16 = 1 << 4;
const NEUTRAL_KIR_FLAG_V1: u16 = 1 << 5;
const TARGET_KIR_FLAG_V1: u16 = 1 << 6;
const SEMANTIC_OPERATION_FLAG_V1: u16 = 1 << 7;
const COMPILER_HANDOFF_LLVM_FLAG_V1: u16 = 1 << 8;
const ALL_RECORD_FLAGS_V1: u16 = (1 << 9) - 1;
const ELIMINATED_RECORD_FLAGS_V1: u16 =
    SOURCE_NODE_FLAG_V1 | SOURCE_SPAN_FLAG_V1 | MIR_NODE_FLAG_V1 | MIR_FLAG_V1;
const NO_SOURCE_RECORD_FLAGS_V1: u16 =
    TARGET_KIR_FLAG_V1 | SEMANTIC_OPERATION_FLAG_V1 | COMPILER_HANDOFF_LLVM_FLAG_V1;

/// Typed admission result. Unavailability preserves the exact correlation gap.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProductionSourceIsaCatalogAdmissionV1 {
    Admitted(ProductionSourceIsaCatalogV1),
    Unavailable(ProductionSourceIsaCorrelationUnavailableV1),
}

#[derive(Debug)]
pub enum ProductionSourceIsaCatalogErrorV1 {
    Correlation(ProductionSourceIsaCorrelationErrorV1),
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeader,
    InvalidIdentity,
    InvalidStructuralBinding,
    InvalidRecord,
    NonCanonicalRecordOrder,
    ExactProjectionMismatch,
    SourceProjectionForKirV9Unavailable,
    SourceProjectionForKirV11Unavailable,
    ResourceLimit,
    AllocationFailure,
}

impl fmt::Display for ProductionSourceIsaCatalogErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid production Source/ISA catalog: {self:?}")
    }
}

impl Error for ProductionSourceIsaCatalogErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Correlation(error) => Some(error),
            _ => None,
        }
    }
}

/// Stable content identity retained without the named bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProductionSourceIsaCatalogContentIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn validate(self) -> bool {
        self.sha256 != [0; 32] && self.byte_len != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCatalogTargetV1 {
    Gfx942,
    Gfx950,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCatalogKirVersionV1 {
    V8,
    V9,
    V11,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogStructuralCountsV1 {
    functions: u64,
    defined_bodies: u64,
    blocks: u64,
    operations: u64,
}

impl ProductionSourceIsaCatalogStructuralCountsV1 {
    pub(crate) const fn new_for_bridge_v1(
        functions: u64,
        defined_bodies: u64,
        blocks: u64,
        operations: u64,
    ) -> Self {
        Self {
            functions,
            defined_bodies,
            blocks,
            operations,
        }
    }

    pub const fn functions(self) -> u64 {
        self.functions
    }

    pub const fn defined_bodies(self) -> u64 {
        self.defined_bodies
    }

    pub const fn blocks(self) -> u64 {
        self.blocks
    }

    pub const fn operations(self) -> u64 {
        self.operations
    }
}

/// Exact structural identity snapshot retained by the catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogStructuralBindingV1 {
    identity: [u8; 32],
    target: ProductionSourceIsaCatalogTargetV1,
    kir_version: ProductionSourceIsaCatalogKirVersionV1,
    neutral_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1,
    target_bound_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1,
    counts: ProductionSourceIsaCatalogStructuralCountsV1,
}

impl ProductionSourceIsaCatalogStructuralBindingV1 {
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn target(self) -> ProductionSourceIsaCatalogTargetV1 {
        self.target
    }

    pub const fn kir_version(self) -> ProductionSourceIsaCatalogKirVersionV1 {
        self.kir_version
    }

    pub const fn neutral_kernel_ir(self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.neutral_kernel_ir
    }

    pub const fn target_bound_kernel_ir(self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.target_bound_kernel_ir
    }

    pub const fn counts(self) -> ProductionSourceIsaCatalogStructuralCountsV1 {
        self.counts
    }

    pub const fn proves_semantic_refinement(self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCatalogRecordKindV1 {
    EliminatedBeforeKir,
    SourceAnchored,
    NoSourceProvenance,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSourceIsaCatalogTransformationV1 {
    Preserved,
    Duplicated,
    Coalesced,
    DuplicatedAndCoalesced,
    Eliminated,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaMirCoordinateV1 {
    body_ordinal: u64,
    block_ordinal: u64,
    statement_ordinal: u64,
}

impl ProductionSourceIsaMirCoordinateV1 {
    pub fn new(
        body_ordinal: u64,
        block_ordinal: u64,
        statement_ordinal: u64,
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        let coordinate = Self {
            body_ordinal,
            block_ordinal,
            statement_ordinal,
        };
        if !coordinate.is_valid() {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
        }
        Ok(coordinate)
    }

    pub const fn body_ordinal(self) -> u64 {
        self.body_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn statement_ordinal(self) -> u64 {
        self.statement_ordinal
    }

    fn is_valid(self) -> bool {
        valid_coordinate(
            self.body_ordinal,
            self.block_ordinal,
            self.statement_ordinal,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaKirCoordinateV1 {
    function_ordinal: u64,
    block_ordinal: u64,
    operation_ordinal: u64,
}

impl ProductionSourceIsaKirCoordinateV1 {
    pub fn new(
        function_ordinal: u64,
        block_ordinal: u64,
        operation_ordinal: u64,
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        let coordinate = Self {
            function_ordinal,
            block_ordinal,
            operation_ordinal,
        };
        if !coordinate.is_valid() {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
        }
        Ok(coordinate)
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn operation_ordinal(self) -> u64 {
        self.operation_ordinal
    }

    fn is_valid(self) -> bool {
        valid_coordinate(
            self.function_ordinal,
            self.block_ordinal,
            self.operation_ordinal,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaLlvmCoordinateV1 {
    function_ordinal: u64,
    block_ordinal: u64,
    instruction_ordinal: u64,
}

impl ProductionSourceIsaLlvmCoordinateV1 {
    pub fn new(
        function_ordinal: u64,
        block_ordinal: u64,
        instruction_ordinal: u64,
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        let coordinate = Self {
            function_ordinal,
            block_ordinal,
            instruction_ordinal,
        };
        if !coordinate.is_valid() {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
        }
        Ok(coordinate)
    }

    pub const fn function_ordinal(self) -> u64 {
        self.function_ordinal
    }

    pub const fn block_ordinal(self) -> u64 {
        self.block_ordinal
    }

    pub const fn instruction_ordinal(self) -> u64 {
        self.instruction_ordinal
    }

    fn is_valid(self) -> bool {
        valid_coordinate(
            self.function_ordinal,
            self.block_ordinal,
            self.instruction_ordinal,
        )
    }
}

/// Exact four-byte half-open interval relative to the AMDHSA metadata kernel symbol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogIntervalV1 {
    kernel_ordinal: u64,
    byte_start: u64,
    byte_end: u64,
}

impl ProductionSourceIsaCatalogIntervalV1 {
    pub fn new(
        kernel_ordinal: u64,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        let interval = Self {
            kernel_ordinal,
            byte_start,
            byte_end,
        };
        if !interval.is_valid() {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
        }
        Ok(interval)
    }

    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    pub const fn byte_start(self) -> u64 {
        self.byte_start
    }

    pub const fn byte_end(self) -> u64 {
        self.byte_end
    }

    fn is_valid(self) -> bool {
        self.kernel_ordinal == 0
            && self.byte_start.is_multiple_of(4)
            && self.byte_start.checked_add(4) == Some(self.byte_end)
    }
}

/// One canonical name-free catalog record.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogRecordV1 {
    kind: ProductionSourceIsaCatalogRecordKindV1,
    source_node_identity: Option<[u8; 32]>,
    source_span: Option<DebugSourceMapSpanV1>,
    mir_node_identity: Option<[u8; 32]>,
    mir: Option<ProductionSourceIsaMirCoordinateV1>,
    neutral_kir_node_identity: Option<[u8; 32]>,
    neutral_kir: Option<ProductionSourceIsaKirCoordinateV1>,
    target_kir: Option<ProductionSourceIsaKirCoordinateV1>,
    semantic_operation_id: Option<[u8; 32]>,
    compiler_handoff_llvm: Option<ProductionSourceIsaLlvmCoordinateV1>,
    isa: Vec<ProductionSourceIsaCatalogIntervalV1>,
    transformation: Option<ProductionSourceIsaCatalogTransformationV1>,
}

impl ProductionSourceIsaCatalogRecordV1 {
    pub const fn kind(&self) -> ProductionSourceIsaCatalogRecordKindV1 {
        self.kind
    }

    pub const fn source_node_identity(&self) -> Option<[u8; 32]> {
        self.source_node_identity
    }

    pub const fn source_span(&self) -> Option<DebugSourceMapSpanV1> {
        self.source_span
    }

    pub const fn mir_node_identity(&self) -> Option<[u8; 32]> {
        self.mir_node_identity
    }

    pub const fn mir(&self) -> Option<ProductionSourceIsaMirCoordinateV1> {
        self.mir
    }

    pub const fn neutral_kir_node_identity(&self) -> Option<[u8; 32]> {
        self.neutral_kir_node_identity
    }

    pub const fn neutral_kir(&self) -> Option<ProductionSourceIsaKirCoordinateV1> {
        self.neutral_kir
    }

    pub const fn target_kir(&self) -> Option<ProductionSourceIsaKirCoordinateV1> {
        self.target_kir
    }

    pub const fn semantic_operation_id(&self) -> Option<[u8; 32]> {
        self.semantic_operation_id
    }

    pub const fn compiler_handoff_llvm(&self) -> Option<ProductionSourceIsaLlvmCoordinateV1> {
        self.compiler_handoff_llvm
    }

    pub fn isa(&self) -> &[ProductionSourceIsaCatalogIntervalV1] {
        &self.isa
    }

    pub const fn transformation(&self) -> Option<ProductionSourceIsaCatalogTransformationV1> {
        self.transformation
    }

    pub(crate) fn try_clone_bounded(&self) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        let mut isa = Vec::new();
        isa.try_reserve_exact(self.isa.len())
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
        isa.extend_from_slice(&self.isa);
        Ok(Self {
            kind: self.kind,
            source_node_identity: self.source_node_identity,
            source_span: self.source_span,
            mir_node_identity: self.mir_node_identity,
            mir: self.mir,
            neutral_kir_node_identity: self.neutral_kir_node_identity,
            neutral_kir: self.neutral_kir,
            target_kir: self.target_kir,
            semantic_operation_id: self.semantic_operation_id,
            compiler_handoff_llvm: self.compiler_handoff_llvm,
            isa,
            transformation: self.transformation,
        })
    }

    #[cfg(test)]
    pub(crate) fn characteristic_fixture_v1(
        source_node_identity: Option<[u8; 32]>,
        target_kir: Option<ProductionSourceIsaKirCoordinateV1>,
        transformation: Option<ProductionSourceIsaCatalogTransformationV1>,
    ) -> Self {
        let operation = target_kir.map_or(0, ProductionSourceIsaKirCoordinateV1::operation_ordinal);
        let source_span = source_node_identity.map(|identity| {
            DebugSourceMapSpanV1::new(identity, operation * 4, operation * 4 + 4, 1, 1)
                .expect("bounded characteristic fixture span")
        });
        let has_source = source_node_identity.is_some();
        let neutral_shape = has_source && target_kir.is_some();
        let isa = if target_kir.is_some()
            && !matches!(
                transformation,
                Some(ProductionSourceIsaCatalogTransformationV1::Eliminated)
            ) {
            vec![
                ProductionSourceIsaCatalogIntervalV1::new(0, operation * 4, operation * 4 + 4)
                    .expect("bounded characteristic fixture interval"),
            ]
        } else {
            Vec::new()
        };
        Self {
            kind: if target_kir.is_none() {
                ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
            } else if source_node_identity.is_some() {
                ProductionSourceIsaCatalogRecordKindV1::SourceAnchored
            } else {
                ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance
            },
            source_node_identity,
            source_span,
            mir_node_identity: has_source.then_some([0x31; 32]),
            mir: has_source
                .then(|| ProductionSourceIsaMirCoordinateV1::new(0, 0, operation).unwrap()),
            neutral_kir_node_identity: neutral_shape.then_some([0x32; 32]),
            neutral_kir: neutral_shape.then_some(target_kir).flatten(),
            target_kir,
            semantic_operation_id: target_kir.map(|_| [0x33; 32]),
            compiler_handoff_llvm: target_kir
                .map(|_| ProductionSourceIsaLlvmCoordinateV1::new(0, 0, operation).unwrap()),
            isa,
            transformation,
        }
    }

    #[cfg(test)]
    pub(crate) fn characteristic_fixture_with_duplicate_isa_v1(
        source_node_identity: [u8; 32],
        target_kir: ProductionSourceIsaKirCoordinateV1,
        transformation: ProductionSourceIsaCatalogTransformationV1,
    ) -> Self {
        let mut record = Self::characteristic_fixture_v1(
            Some(source_node_identity),
            Some(target_kir),
            Some(transformation),
        );
        let interval = record.isa[0];
        record.isa.push(interval);
        record
    }

    #[cfg(test)]
    pub(crate) fn characteristic_pre_kir_empty_span_fixture_v1(
        source_node_identity: [u8; 32],
    ) -> Self {
        let mut record = Self::characteristic_fixture_v1(Some(source_node_identity), None, None);
        record.source_span = Some(
            DebugSourceMapSpanV1::new_eliminated(source_node_identity, 0, 0, 1, 1)
                .expect("bounded eliminated characteristic fixture span"),
        );
        record
    }
}

#[cfg(test)]
pub(crate) fn characteristic_catalog_fixture_v1(
    target_kir_sha256: [u8; 32],
    target_kir_byte_len: u64,
    counts: ProductionSourceIsaCatalogStructuralCountsV1,
    mut records: Vec<ProductionSourceIsaCatalogRecordV1>,
) -> ProductionSourceIsaCatalogV1 {
    records.sort_unstable();
    validate_record_collection(&records).expect("valid characteristic catalog fixture");
    let indices = build_catalog_indices(&records).expect("bounded characteristic indices");
    let mut catalog = ProductionSourceIsaCatalogV1 {
        identity: [0; 32],
        correlation_identity: [0x41; 32],
        semantic_map_identity: [0x42; 32],
        source_map_v2_identity: ProductionSourceIsaCatalogContentIdentityV1 {
            sha256: [0x43; 32],
            byte_len: 101,
        },
        artifact_identity: ContentIdentityV1::from_parts([0x44; 32], 103),
        structural_binding: ProductionSourceIsaCatalogStructuralBindingV1 {
            identity: [0x45; 32],
            target: ProductionSourceIsaCatalogTargetV1::Gfx942,
            kir_version: ProductionSourceIsaCatalogKirVersionV1::V8,
            neutral_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: [0x46; 32],
                byte_len: 107,
            },
            target_bound_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: target_kir_sha256,
                byte_len: target_kir_byte_len,
            },
            counts,
        },
        records,
        indices,
    };
    catalog.identity = catalog_identity(
        &catalog
            .canonical_preimage()
            .expect("bounded characteristic catalog preimage"),
    );
    catalog
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSourceIsaCatalogPointV1 {
    kernel_ordinal: u64,
    symbol_relative_pc: u64,
}

impl ProductionSourceIsaCatalogPointV1 {
    pub const fn new(kernel_ordinal: u64, symbol_relative_pc: u64) -> Self {
        Self {
            kernel_ordinal,
            symbol_relative_pc,
        }
    }

    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    pub const fn symbol_relative_pc(self) -> u64 {
        self.symbol_relative_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceIsaCatalogQueryUnavailableV1 {
    UnknownSourceNode,
    UnknownSourceSpan,
    UnknownMirNode,
    UnknownMirCoordinate,
    UnknownNeutralKirNode,
    UnknownNeutralKirCoordinate,
    UnknownTargetKirCoordinate,
    UnknownSemanticOperation,
    UnknownCompilerHandoffLlvmCoordinate,
    UnalignedProgramCounter,
    UnknownMetadataKernelOrdinal,
    ProgramCounterIsNotAnAdmittedAnchor,
}

#[derive(Debug)]
enum CatalogMatchIndexSliceV1<'a> {
    Identity(&'a [([u8; 32], usize)]),
    Span(&'a [(DebugSourceMapSpanV1, usize)]),
    Mir(&'a [(ProductionSourceIsaMirCoordinateV1, usize)]),
    Kir(&'a [(ProductionSourceIsaKirCoordinateV1, usize)]),
    Llvm(&'a [(ProductionSourceIsaLlvmCoordinateV1, usize)]),
    Isa(&'a [(ProductionSourceIsaCatalogPointV1, usize)]),
}

/// Allocation-free exact query results over canonical records.
#[derive(Debug)]
pub struct ProductionSourceIsaCatalogMatchesV1<'a> {
    records: &'a [ProductionSourceIsaCatalogRecordV1],
    index: CatalogMatchIndexSliceV1<'a>,
    next: usize,
}

impl<'a> Iterator for ProductionSourceIsaCatalogMatchesV1<'a> {
    type Item = &'a ProductionSourceIsaCatalogRecordV1;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_with_ordinal().map(|(_, record)| record)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = match &self.index {
            CatalogMatchIndexSliceV1::Identity(entries) => entries.len(),
            CatalogMatchIndexSliceV1::Span(entries) => entries.len(),
            CatalogMatchIndexSliceV1::Mir(entries) => entries.len(),
            CatalogMatchIndexSliceV1::Kir(entries) => entries.len(),
            CatalogMatchIndexSliceV1::Llvm(entries) => entries.len(),
            CatalogMatchIndexSliceV1::Isa(entries) => entries.len(),
        };
        let remaining = length.saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl<'a> ProductionSourceIsaCatalogMatchesV1<'a> {
    pub(crate) fn next_with_ordinal(
        &mut self,
    ) -> Option<(usize, &'a ProductionSourceIsaCatalogRecordV1)> {
        let index = match &self.index {
            CatalogMatchIndexSliceV1::Identity(entries) => {
                entries.get(self.next).map(|entry| entry.1)
            }
            CatalogMatchIndexSliceV1::Span(entries) => entries.get(self.next).map(|entry| entry.1),
            CatalogMatchIndexSliceV1::Mir(entries) => entries.get(self.next).map(|entry| entry.1),
            CatalogMatchIndexSliceV1::Kir(entries) => entries.get(self.next).map(|entry| entry.1),
            CatalogMatchIndexSliceV1::Llvm(entries) => entries.get(self.next).map(|entry| entry.1),
            CatalogMatchIndexSliceV1::Isa(entries) => entries.get(self.next).map(|entry| entry.1),
        }?;
        self.next += 1;
        self.records.get(index).map(|record| (index, record))
    }
}

impl ExactSizeIterator for ProductionSourceIsaCatalogMatchesV1<'_> {}

type IdentityIndexV1 = Vec<([u8; 32], usize)>;
type SpanIndexV1 = Vec<(DebugSourceMapSpanV1, usize)>;
type MirIndexV1 = Vec<(ProductionSourceIsaMirCoordinateV1, usize)>;
type KirIndexV1 = Vec<(ProductionSourceIsaKirCoordinateV1, usize)>;
type LlvmIndexV1 = Vec<(ProductionSourceIsaLlvmCoordinateV1, usize)>;
type IsaIndexV1 = Vec<(ProductionSourceIsaCatalogPointV1, usize)>;

#[derive(Debug)]
struct CatalogIndicesV1 {
    source_node: IdentityIndexV1,
    source_span: SpanIndexV1,
    mir_node: IdentityIndexV1,
    mir: MirIndexV1,
    neutral_kir_node: IdentityIndexV1,
    neutral_kir: KirIndexV1,
    target_kir: KirIndexV1,
    semantic_operation: IdentityIndexV1,
    compiler_handoff_llvm: LlvmIndexV1,
    isa: IsaIndexV1,
}

/// Canonically decoded catalog claims which have not been re-admitted against their correlation.
///
/// This type deliberately exposes no records or query methods. Its identities remain wire claims
/// until [`Self::admit_exact_projection_v1`] reconstructs and compares the complete projection.
pub struct InertProductionSourceIsaCatalogV1 {
    claimed_catalog: ProductionSourceIsaCatalogV1,
}

impl fmt::Debug for InertProductionSourceIsaCatalogV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionSourceIsaCatalogV1")
            .field("claimed_identity", &self.claimed_identity())
            .field(
                "claimed_correlation_identity",
                &self.claimed_correlation_identity(),
            )
            .finish_non_exhaustive()
    }
}

impl InertProductionSourceIsaCatalogV1 {
    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        Ok(Self {
            claimed_catalog: ProductionSourceIsaCatalogV1::decode_claimed_canonical_bytes(encoded)?,
        })
    }

    pub const fn claimed_identity(&self) -> &[u8; 32] {
        self.claimed_catalog.identity()
    }

    pub const fn claimed_correlation_identity(&self) -> &[u8; 32] {
        self.claimed_catalog.correlation_identity()
    }

    /// Reconstructs every record from the admitted correlation and requires canonical equality.
    pub fn admit_exact_projection_v1(
        self,
        correlation: &AdmittedProductionSourceIsaCorrelationV1,
    ) -> Result<ProductionSourceIsaCatalogV1, ProductionSourceIsaCatalogErrorV1> {
        let exact = ProductionSourceIsaCatalogV1::from_admitted_correlation_v1(correlation)?;
        if !same_exact_catalog_projection(&self.claimed_catalog, &exact) {
            return Err(ProductionSourceIsaCatalogErrorV1::ExactProjectionMismatch);
        }
        Ok(exact)
    }

    pub const fn grants_debugger_authority(&self) -> bool {
        false
    }

    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_live_program_counter_ownership(&self) -> bool {
        false
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }

    pub const fn grants_profiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

fn same_exact_catalog_projection(
    claimed: &ProductionSourceIsaCatalogV1,
    exact: &ProductionSourceIsaCatalogV1,
) -> bool {
    claimed.identity == exact.identity
        && claimed.correlation_identity == exact.correlation_identity
        && claimed.semantic_map_identity == exact.semantic_map_identity
        && claimed.source_map_v2_identity == exact.source_map_v2_identity
        && claimed.artifact_identity == exact.artifact_identity
        && claimed.structural_binding == exact.structural_binding
        && claimed.records == exact.records
}

/// Canonical observation catalog. It owns no compiler, artifact, debugger, or runtime handle.
#[derive(Debug)]
pub struct ProductionSourceIsaCatalogV1 {
    identity: [u8; 32],
    correlation_identity: [u8; 32],
    semantic_map_identity: [u8; 32],
    source_map_v2_identity: ProductionSourceIsaCatalogContentIdentityV1,
    artifact_identity: ContentIdentityV1,
    structural_binding: ProductionSourceIsaCatalogStructuralBindingV1,
    records: Vec<ProductionSourceIsaCatalogRecordV1>,
    indices: CatalogIndicesV1,
}

impl ProductionSourceIsaCatalogV1 {
    /// Projects a previously admitted exact production correlation into durable observation data.
    pub fn from_admitted_correlation_v1(
        correlation: &AdmittedProductionSourceIsaCorrelationV1,
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        validate_top_level_identities(correlation)?;
        if correlation.records().len() > MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(correlation.records().len())
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
        for record in correlation.records() {
            records.push(project_record(record)?);
        }
        records.sort_unstable();
        validate_record_collection(&records)?;
        let indices = build_catalog_indices(&records)?;
        let source_map_v2 = correlation.source_map_v2_identity();
        let structural_binding = structural_snapshot(correlation.structural_binding());
        let mut catalog = Self {
            identity: [0; 32],
            correlation_identity: *correlation.identity(),
            semantic_map_identity: *correlation.semantic_map_identity(),
            source_map_v2_identity: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: source_map_v2.sha256(),
                byte_len: source_map_v2.byte_len(),
            },
            artifact_identity: correlation.artifact_identity(),
            structural_binding,
            records,
            indices,
        };
        let preimage = catalog.canonical_preimage()?;
        catalog.identity = catalog_identity(&preimage);
        Ok(catalog)
    }

    pub const fn format_version(&self) -> u16 {
        PRODUCTION_SOURCE_ISA_CATALOG_VERSION_V1
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn correlation_identity(&self) -> &[u8; 32] {
        &self.correlation_identity
    }

    pub const fn semantic_map_identity(&self) -> &[u8; 32] {
        &self.semantic_map_identity
    }

    pub const fn source_map_v2_identity(&self) -> ProductionSourceIsaCatalogContentIdentityV1 {
        self.source_map_v2_identity
    }

    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.artifact_identity
    }

    pub const fn structural_binding(&self) -> ProductionSourceIsaCatalogStructuralBindingV1 {
        self.structural_binding
    }

    pub fn records(&self) -> &[ProductionSourceIsaCatalogRecordV1] {
        &self.records
    }

    pub fn canonical_byte_len(&self) -> Result<u64, ProductionSourceIsaCatalogErrorV1> {
        u64::try_from(catalog_encoded_len(&self.records)?)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)
    }

    pub fn query_source_node(
        &self,
        identity: [u8; 32],
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_identity_index(
            &self.indices.source_node,
            identity,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSourceNode,
        )
    }

    pub fn query_source_span(
        &self,
        span: DebugSourceMapSpanV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        let range = equal_range_by_key(&self.indices.source_span, &span, |entry| &entry.0);
        if range.is_empty() {
            return Err(ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSourceSpan);
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Span(&self.indices.source_span[range]),
            next: 0,
        })
    }

    pub fn query_mir_node(
        &self,
        identity: [u8; 32],
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_identity_index(
            &self.indices.mir_node,
            identity,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMirNode,
        )
    }

    pub fn query_mir(
        &self,
        coordinate: ProductionSourceIsaMirCoordinateV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        let range = equal_range_by_key(&self.indices.mir, &coordinate, |entry| &entry.0);
        if range.is_empty() {
            return Err(ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMirCoordinate);
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Mir(&self.indices.mir[range]),
            next: 0,
        })
    }

    pub fn query_neutral_kir_node(
        &self,
        identity: [u8; 32],
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_identity_index(
            &self.indices.neutral_kir_node,
            identity,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownNeutralKirNode,
        )
    }

    pub fn query_neutral_kir(
        &self,
        coordinate: ProductionSourceIsaKirCoordinateV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_kir_index(
            &self.indices.neutral_kir,
            coordinate,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownNeutralKirCoordinate,
        )
    }

    pub fn query_target_kir(
        &self,
        coordinate: ProductionSourceIsaKirCoordinateV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_kir_index(
            &self.indices.target_kir,
            coordinate,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownTargetKirCoordinate,
        )
    }

    pub fn query_semantic_operation(
        &self,
        identity: [u8; 32],
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        self.query_identity_index(
            &self.indices.semantic_operation,
            identity,
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSemanticOperation,
        )
    }

    pub fn query_compiler_handoff_llvm(
        &self,
        coordinate: ProductionSourceIsaLlvmCoordinateV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        let range = equal_range_by_key(&self.indices.compiler_handoff_llvm, &coordinate, |entry| {
            &entry.0
        });
        if range.is_empty() {
            return Err(
                ProductionSourceIsaCatalogQueryUnavailableV1::UnknownCompilerHandoffLlvmCoordinate,
            );
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Llvm(&self.indices.compiler_handoff_llvm[range]),
            next: 0,
        })
    }

    pub fn query_isa_pc(
        &self,
        point: ProductionSourceIsaCatalogPointV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'_>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        if !point.symbol_relative_pc.is_multiple_of(4) {
            return Err(ProductionSourceIsaCatalogQueryUnavailableV1::UnalignedProgramCounter);
        }
        if point.kernel_ordinal != 0 {
            return Err(ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMetadataKernelOrdinal);
        }
        let range = equal_range_by_key(&self.indices.isa, &point, |entry| &entry.0);
        if range.is_empty() {
            return Err(
                ProductionSourceIsaCatalogQueryUnavailableV1::ProgramCounterIsNotAnAdmittedAnchor,
            );
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Isa(&self.indices.isa[range]),
            next: 0,
        })
    }

    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, ProductionSourceIsaCatalogErrorV1> {
        let mut encoded = self.canonical_preimage()?;
        encoded.extend_from_slice(&self.identity);
        Ok(encoded)
    }

    #[allow(clippy::too_many_lines)]
    fn decode_claimed_canonical_bytes(
        encoded: &[u8],
    ) -> Result<Self, ProductionSourceIsaCatalogErrorV1> {
        if encoded.len() < CATALOG_HEADER_BYTES_V1 + CATALOG_IDENTITY_BYTES_V1
            || encoded.len() > MAX_PRODUCTION_SOURCE_ISA_CATALOG_BYTES_V1
        {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
        }
        let mut decoder = CatalogDecoderV1::new(encoded);
        if decoder.take(8)? != PRODUCTION_SOURCE_ISA_CATALOG_MAGIC_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidMagic);
        }
        if decoder.u16()? != PRODUCTION_SOURCE_ISA_CATALOG_VERSION_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::UnsupportedVersion);
        }
        if usize::from(decoder.u16()?) != CATALOG_HEADER_BYTES_V1 || decoder.u32()? != 0 {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidHeader);
        }
        let declared_len = usize::try_from(decoder.u64()?)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        if declared_len != encoded.len() {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
        }
        let record_count = usize::try_from(decoder.u64()?)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        let declared_isa_count = usize::try_from(decoder.u64()?)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        if record_count > MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1
            || declared_isa_count > MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1
        {
            return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
        }
        let payload_bytes = encoded
            .len()
            .checked_sub(CATALOG_HEADER_BYTES_V1 + CATALOG_IDENTITY_BYTES_V1)
            .ok_or(ProductionSourceIsaCatalogErrorV1::InvalidLength)?;
        let minimum_payload_bytes = record_count
            .checked_mul(MIN_VALID_RECORD_BYTES_V1)
            .and_then(|bytes| {
                declared_isa_count
                    .checked_mul(ISA_INTERVAL_BYTES_V1)
                    .and_then(|isa_bytes| bytes.checked_add(isa_bytes))
            })
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        if minimum_payload_bytes > payload_bytes {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
        }
        let correlation_identity = decoder.identity()?;
        let semantic_map_identity = decoder.identity()?;
        let source_map_v2_identity = decoder.content_identity()?;
        let artifact_identity = decoder.content_identity()?;
        let structural_identity = decoder.identity()?;
        let target = decode_target(decoder.u8()?)?;
        let kir_version = decode_kir_version(decoder.u8()?)?;
        if kir_version == ProductionSourceIsaCatalogKirVersionV1::V9 {
            return Err(ProductionSourceIsaCatalogErrorV1::SourceProjectionForKirV9Unavailable);
        }
        if kir_version == ProductionSourceIsaCatalogKirVersionV1::V11 {
            return Err(ProductionSourceIsaCatalogErrorV1::SourceProjectionForKirV11Unavailable);
        }
        if decoder.take(6)?.iter().any(|byte| *byte != 0) {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidHeader);
        }
        let neutral_kernel_ir = decoder.content_identity()?;
        let target_bound_kernel_ir = decoder.content_identity()?;
        let counts = ProductionSourceIsaCatalogStructuralCountsV1 {
            functions: decoder.u64()?,
            defined_bodies: decoder.u64()?,
            blocks: decoder.u64()?,
            operations: decoder.u64()?,
        };
        if decoder.cursor != CATALOG_HEADER_BYTES_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidHeader);
        }
        let source_map_v2_identity = ProductionSourceIsaCatalogContentIdentityV1 {
            sha256: *source_map_v2_identity.sha256(),
            byte_len: source_map_v2_identity.byte_len(),
        };
        let structural_binding = ProductionSourceIsaCatalogStructuralBindingV1 {
            identity: structural_identity,
            target,
            kir_version,
            neutral_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: *neutral_kernel_ir.sha256(),
                byte_len: neutral_kernel_ir.byte_len(),
            },
            target_bound_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: *target_bound_kernel_ir.sha256(),
                byte_len: target_bound_kernel_ir.byte_len(),
            },
            counts,
        };
        validate_decoded_header(
            correlation_identity,
            semantic_map_identity,
            source_map_v2_identity,
            artifact_identity,
            structural_binding,
        )?;

        let mut records = Vec::new();
        records
            .try_reserve_exact(record_count)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
        let mut actual_isa_count = 0_usize;
        for _ in 0..record_count {
            let remaining_isa_budget = declared_isa_count
                .checked_sub(actual_isa_count)
                .ok_or(ProductionSourceIsaCatalogErrorV1::InvalidRecord)?;
            let record = decode_record(&mut decoder, remaining_isa_budget)?;
            actual_isa_count = actual_isa_count
                .checked_add(record.isa.len())
                .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
            if actual_isa_count > declared_isa_count {
                return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
            }
            if records.last().is_some_and(|previous| previous > &record) {
                return Err(ProductionSourceIsaCatalogErrorV1::NonCanonicalRecordOrder);
            }
            records.push(record);
        }
        if actual_isa_count != declared_isa_count
            || decoder.cursor.checked_add(CATALOG_IDENTITY_BYTES_V1) != Some(encoded.len())
        {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
        }
        let identity = decoder.identity()?;
        if decoder.cursor != encoded.len()
            || identity != catalog_identity(&encoded[..encoded.len() - CATALOG_IDENTITY_BYTES_V1])
        {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidIdentity);
        }
        validate_record_collection(&records)?;
        let indices = build_catalog_indices(&records)?;
        Ok(Self {
            identity,
            correlation_identity,
            semantic_map_identity,
            source_map_v2_identity,
            artifact_identity,
            structural_binding,
            records,
            indices,
        })
    }

    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_live_program_counter_ownership(&self) -> bool {
        false
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }

    pub const fn grants_debugger_authority(&self) -> bool {
        false
    }

    pub const fn grants_profiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    fn query_identity_index<'a>(
        &'a self,
        index: &'a [([u8; 32], usize)],
        identity: [u8; 32],
        unavailable: ProductionSourceIsaCatalogQueryUnavailableV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'a>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        let range = equal_range_by_key(index, &identity, |entry| &entry.0);
        if range.is_empty() {
            return Err(unavailable);
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Identity(&index[range]),
            next: 0,
        })
    }

    fn query_kir_index<'a>(
        &'a self,
        index: &'a [(ProductionSourceIsaKirCoordinateV1, usize)],
        coordinate: ProductionSourceIsaKirCoordinateV1,
        unavailable: ProductionSourceIsaCatalogQueryUnavailableV1,
    ) -> Result<ProductionSourceIsaCatalogMatchesV1<'a>, ProductionSourceIsaCatalogQueryUnavailableV1>
    {
        let range = equal_range_by_key(index, &coordinate, |entry| &entry.0);
        if range.is_empty() {
            return Err(unavailable);
        }
        Ok(ProductionSourceIsaCatalogMatchesV1 {
            records: &self.records,
            index: CatalogMatchIndexSliceV1::Kir(&index[range]),
            next: 0,
        })
    }

    fn canonical_preimage(&self) -> Result<Vec<u8>, ProductionSourceIsaCatalogErrorV1> {
        validate_record_collection(&self.records)?;
        validate_decoded_header(
            self.correlation_identity,
            self.semantic_map_identity,
            self.source_map_v2_identity,
            self.artifact_identity,
            self.structural_binding,
        )?;
        let total_len = catalog_encoded_len(&self.records)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(total_len)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
        encoded.extend_from_slice(&PRODUCTION_SOURCE_ISA_CATALOG_MAGIC_V1);
        push_u16(&mut encoded, PRODUCTION_SOURCE_ISA_CATALOG_VERSION_V1);
        push_u16(
            &mut encoded,
            u16::try_from(CATALOG_HEADER_BYTES_V1)
                .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        );
        push_u32(&mut encoded, 0);
        push_u64(
            &mut encoded,
            u64::try_from(total_len)
                .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        );
        push_u64(
            &mut encoded,
            u64::try_from(self.records.len())
                .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        );
        let isa_count = total_isa_count(&self.records)?;
        push_u64(
            &mut encoded,
            u64::try_from(isa_count)
                .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        );
        encoded.extend_from_slice(&self.correlation_identity);
        encoded.extend_from_slice(&self.semantic_map_identity);
        encode_catalog_content_identity(&mut encoded, self.source_map_v2_identity);
        encode_content_identity(&mut encoded, self.artifact_identity);
        encode_structural_binding(&mut encoded, self.structural_binding);
        if encoded.len() != CATALOG_HEADER_BYTES_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidHeader);
        }
        for record in &self.records {
            encode_record(&mut encoded, record)?;
        }
        if encoded.len().checked_add(CATALOG_IDENTITY_BYTES_V1) != Some(total_len) {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
        }
        Ok(encoded)
    }
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Admits the existing exact correlation and projects all of its records into a catalog.
    pub fn admit_production_source_isa_catalog_v1(
        &self,
    ) -> Result<ProductionSourceIsaCatalogAdmissionV1, ProductionSourceIsaCatalogErrorV1> {
        match self
            .admit_unboxed_production_source_isa_correlation_v1()
            .map_err(ProductionSourceIsaCatalogErrorV1::Correlation)?
        {
            UnboxedProductionSourceIsaCorrelationAdmissionV1::Admitted(correlation) => {
                Ok(ProductionSourceIsaCatalogAdmissionV1::Admitted(
                    ProductionSourceIsaCatalogV1::from_admitted_correlation_v1(&correlation)?,
                ))
            }
            UnboxedProductionSourceIsaCorrelationAdmissionV1::Unavailable(reason) => {
                Ok(ProductionSourceIsaCatalogAdmissionV1::Unavailable(reason))
            }
        }
    }
}

fn validate_top_level_identities(
    correlation: &AdmittedProductionSourceIsaCorrelationV1,
) -> Result<(), ProductionSourceIsaCatalogErrorV1> {
    if correlation.identity() == &[0; 32]
        || correlation.semantic_map_identity() == &[0; 32]
        || correlation.artifact_identity().sha256() == &[0; 32]
        || correlation.artifact_identity().byte_len() == 0
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidIdentity);
    }
    Ok(())
}

fn project_record(
    record: &AdmittedProductionSourceIsaRecordV1,
) -> Result<ProductionSourceIsaCatalogRecordV1, ProductionSourceIsaCatalogErrorV1> {
    let kind = match record.kind() {
        ProductionSourceIsaRecordKindV1::EliminatedBeforeKir => {
            ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
        }
        ProductionSourceIsaRecordKindV1::SourceAnchored => {
            ProductionSourceIsaCatalogRecordKindV1::SourceAnchored
        }
        ProductionSourceIsaRecordKindV1::NoSourceProvenance => {
            ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance
        }
    };
    let mut isa = Vec::new();
    isa.try_reserve_exact(record.isa().len())
        .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
    for location in record.isa() {
        let SemanticDebugLocationV1::Isa {
            kernel_ordinal,
            byte_start,
            byte_end,
        } = *location
        else {
            return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
        };
        isa.push(ProductionSourceIsaCatalogIntervalV1 {
            kernel_ordinal,
            byte_start,
            byte_end,
        });
    }
    isa.sort_unstable();
    let projected = ProductionSourceIsaCatalogRecordV1 {
        kind,
        source_node_identity: record.source_node_identity(),
        source_span: record.source_span(),
        mir_node_identity: record.mir_node_identity(),
        mir: decode_mir_location(record.mir())?,
        neutral_kir_node_identity: record.neutral_kir_node_identity(),
        neutral_kir: decode_kir_location(record.neutral_kir())?,
        target_kir: decode_kir_location(record.target_kir())?,
        semantic_operation_id: record.semantic_operation_id(),
        compiler_handoff_llvm: decode_llvm_location(record.compiler_handoff_llvm())?,
        isa,
        transformation: record.anchor_transformation().map(map_transformation),
    };
    validate_record(&projected)?;
    Ok(projected)
}

fn decode_mir_location(
    location: Option<SemanticDebugLocationV1>,
) -> Result<Option<ProductionSourceIsaMirCoordinateV1>, ProductionSourceIsaCatalogErrorV1> {
    match location {
        Some(SemanticDebugLocationV1::Mir {
            body_ordinal,
            block_ordinal,
            statement_ordinal,
        }) => Ok(Some(ProductionSourceIsaMirCoordinateV1 {
            body_ordinal,
            block_ordinal,
            statement_ordinal,
        })),
        None => Ok(None),
        _ => Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord),
    }
}

fn decode_kir_location(
    location: Option<SemanticDebugLocationV1>,
) -> Result<Option<ProductionSourceIsaKirCoordinateV1>, ProductionSourceIsaCatalogErrorV1> {
    match location {
        Some(SemanticDebugLocationV1::Kir {
            function_ordinal,
            block_ordinal,
            operation_ordinal,
        }) => Ok(Some(ProductionSourceIsaKirCoordinateV1 {
            function_ordinal,
            block_ordinal,
            operation_ordinal,
        })),
        None => Ok(None),
        _ => Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord),
    }
}

fn decode_llvm_location(
    location: Option<SemanticDebugLocationV1>,
) -> Result<Option<ProductionSourceIsaLlvmCoordinateV1>, ProductionSourceIsaCatalogErrorV1> {
    match location {
        Some(SemanticDebugLocationV1::Llvm {
            function_ordinal,
            block_ordinal,
            instruction_ordinal,
        }) => Ok(Some(ProductionSourceIsaLlvmCoordinateV1 {
            function_ordinal,
            block_ordinal,
            instruction_ordinal,
        })),
        None => Ok(None),
        _ => Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord),
    }
}

fn map_transformation(
    transformation: ProductionSemanticAnchorTransformationV1,
) -> ProductionSourceIsaCatalogTransformationV1 {
    match transformation {
        ProductionSemanticAnchorTransformationV1::Preserved => {
            ProductionSourceIsaCatalogTransformationV1::Preserved
        }
        ProductionSemanticAnchorTransformationV1::Duplicated => {
            ProductionSourceIsaCatalogTransformationV1::Duplicated
        }
        ProductionSemanticAnchorTransformationV1::Coalesced => {
            ProductionSourceIsaCatalogTransformationV1::Coalesced
        }
        ProductionSemanticAnchorTransformationV1::DuplicatedAndCoalesced => {
            ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced
        }
        ProductionSemanticAnchorTransformationV1::Eliminated => {
            ProductionSourceIsaCatalogTransformationV1::Eliminated
        }
    }
}

fn structural_snapshot(
    binding: ProductionTargetStructuralBindingV1,
) -> ProductionSourceIsaCatalogStructuralBindingV1 {
    let target = match binding.profile() {
        ProductionAmdTargetProfileV1::Gfx942 => ProductionSourceIsaCatalogTargetV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950 => ProductionSourceIsaCatalogTargetV1::Gfx950,
    };
    let kir_version = match binding.version() {
        ProductionReplayKernelIrVersionV1::V8 => ProductionSourceIsaCatalogKirVersionV1::V8,
        ProductionReplayKernelIrVersionV1::V9 => ProductionSourceIsaCatalogKirVersionV1::V9,
        ProductionReplayKernelIrVersionV1::V11 => ProductionSourceIsaCatalogKirVersionV1::V11,
    };
    let neutral = binding.neutral_kernel_ir();
    let target_bound = binding.target_bound_kernel_ir();
    let counts = binding.counts();
    ProductionSourceIsaCatalogStructuralBindingV1 {
        identity: binding.identity(),
        target,
        kir_version,
        neutral_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
            sha256: neutral.sha256(),
            byte_len: neutral.byte_len(),
        },
        target_bound_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
            sha256: target_bound.sha256(),
            byte_len: target_bound.byte_len(),
        },
        counts: ProductionSourceIsaCatalogStructuralCountsV1 {
            functions: counts.functions(),
            defined_bodies: counts.defined_bodies(),
            blocks: counts.blocks(),
            operations: counts.operations(),
        },
    }
}

fn validate_decoded_header(
    correlation_identity: [u8; 32],
    semantic_map_identity: [u8; 32],
    source_map_v2_identity: ProductionSourceIsaCatalogContentIdentityV1,
    artifact_identity: ContentIdentityV1,
    structural_binding: ProductionSourceIsaCatalogStructuralBindingV1,
) -> Result<(), ProductionSourceIsaCatalogErrorV1> {
    if correlation_identity == [0; 32]
        || semantic_map_identity == [0; 32]
        || !source_map_v2_identity.validate()
        || artifact_identity.sha256() == &[0; 32]
        || artifact_identity.byte_len() == 0
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidIdentity);
    }
    if structural_binding.identity == [0; 32]
        || !structural_binding.neutral_kernel_ir.validate()
        || !structural_binding.target_bound_kernel_ir.validate()
        || structural_binding.counts.defined_bodies > structural_binding.counts.functions
        || structural_binding.counts.blocks < structural_binding.counts.defined_bodies
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidStructuralBinding);
    }
    Ok(())
}

fn valid_coordinate(first: u64, second: u64, third: u64) -> bool {
    [first, second, third]
        .into_iter()
        .all(|ordinal| ordinal <= u64::from(u32::MAX))
}

fn validate_record(
    record: &ProductionSourceIsaCatalogRecordV1,
) -> Result<(), ProductionSourceIsaCatalogErrorV1> {
    if record
        .source_node_identity
        .is_some_and(|identity| identity == [0; 32])
        || record
            .mir_node_identity
            .is_some_and(|identity| identity == [0; 32])
        || record
            .neutral_kir_node_identity
            .is_some_and(|identity| identity == [0; 32])
        || record
            .semantic_operation_id
            .is_some_and(|identity| identity == [0; 32])
        || record.mir.is_some_and(|coordinate| !coordinate.is_valid())
        || record
            .neutral_kir
            .is_some_and(|coordinate| !coordinate.is_valid())
        || record
            .target_kir
            .is_some_and(|coordinate| !coordinate.is_valid())
        || record
            .compiler_handoff_llvm
            .is_some_and(|coordinate| !coordinate.is_valid())
        || record.isa.iter().any(|interval| !interval.is_valid())
        || record.isa.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
    }
    let source_shape = record.source_node_identity.is_some()
        && record.source_span.is_some()
        && record.mir_node_identity.is_some()
        && record.mir.is_some();
    let neutral_shape = record.neutral_kir_node_identity.is_some() && record.neutral_kir.is_some();
    let target_shape = record.target_kir.is_some()
        && record.semantic_operation_id.is_some()
        && record.compiler_handoff_llvm.is_some()
        && record.transformation.is_some();
    let exact_anchor_outcome = record.transformation.is_some_and(|transformation| {
        record.isa.is_empty()
            == (transformation == ProductionSourceIsaCatalogTransformationV1::Eliminated)
    });
    let valid_shape = match record.kind {
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => {
            source_shape
                && !neutral_shape
                && record.neutral_kir_node_identity.is_none()
                && record.neutral_kir.is_none()
                && record.target_kir.is_none()
                && record.semantic_operation_id.is_none()
                && record.compiler_handoff_llvm.is_none()
                && record.isa.is_empty()
                && record.transformation.is_none()
        }
        ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => {
            source_shape && neutral_shape && target_shape && exact_anchor_outcome
        }
        ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => {
            record.source_node_identity.is_none()
                && record.source_span.is_none()
                && record.mir_node_identity.is_none()
                && record.mir.is_none()
                && record.neutral_kir_node_identity.is_none()
                && record.neutral_kir.is_none()
                && target_shape
                && exact_anchor_outcome
        }
    };
    if !valid_shape {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
    }
    Ok(())
}

fn validate_record_collection(
    records: &[ProductionSourceIsaCatalogRecordV1],
) -> Result<(), ProductionSourceIsaCatalogErrorV1> {
    if records.len() > MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1 {
        return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
    }
    if records.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(ProductionSourceIsaCatalogErrorV1::NonCanonicalRecordOrder);
    }
    for record in records {
        validate_record(record)?;
    }
    if total_isa_count(records)? > MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1 {
        return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
    }
    Ok(())
}

fn total_isa_count(
    records: &[ProductionSourceIsaCatalogRecordV1],
) -> Result<usize, ProductionSourceIsaCatalogErrorV1> {
    records.iter().try_fold(0_usize, |count, record| {
        count
            .checked_add(record.isa.len())
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)
    })
}

fn build_catalog_indices(
    records: &[ProductionSourceIsaCatalogRecordV1],
) -> Result<CatalogIndicesV1, ProductionSourceIsaCatalogErrorV1> {
    let mut source_nodes = 0_usize;
    let mut source_spans = 0_usize;
    let mut mir_nodes = 0_usize;
    let mut mir = 0_usize;
    let mut neutral_kir_nodes = 0_usize;
    let mut neutral_kir = 0_usize;
    let mut target_kir = 0_usize;
    let mut semantic_operations = 0_usize;
    let mut compiler_handoff_llvm = 0_usize;
    let mut isa = 0_usize;
    for record in records {
        source_nodes = source_nodes
            .checked_add(usize::from(record.source_node_identity.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        source_spans = source_spans
            .checked_add(usize::from(record.source_span.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        mir_nodes = mir_nodes
            .checked_add(usize::from(record.mir_node_identity.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        mir = mir
            .checked_add(usize::from(record.mir.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        neutral_kir_nodes = neutral_kir_nodes
            .checked_add(usize::from(record.neutral_kir_node_identity.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        neutral_kir = neutral_kir
            .checked_add(usize::from(record.neutral_kir.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        target_kir = target_kir
            .checked_add(usize::from(record.target_kir.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        semantic_operations = semantic_operations
            .checked_add(usize::from(record.semantic_operation_id.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        compiler_handoff_llvm = compiler_handoff_llvm
            .checked_add(usize::from(record.compiler_handoff_llvm.is_some()))
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        isa = isa
            .checked_add(record.isa.len())
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
    }
    let mut indices = CatalogIndicesV1 {
        source_node: try_vec_capacity(source_nodes)?,
        source_span: try_vec_capacity(source_spans)?,
        mir_node: try_vec_capacity(mir_nodes)?,
        mir: try_vec_capacity(mir)?,
        neutral_kir_node: try_vec_capacity(neutral_kir_nodes)?,
        neutral_kir: try_vec_capacity(neutral_kir)?,
        target_kir: try_vec_capacity(target_kir)?,
        semantic_operation: try_vec_capacity(semantic_operations)?,
        compiler_handoff_llvm: try_vec_capacity(compiler_handoff_llvm)?,
        isa: try_vec_capacity(isa)?,
    };
    for (index, record) in records.iter().enumerate() {
        if let Some(identity) = record.source_node_identity {
            indices.source_node.push((identity, index));
        }
        if let Some(span) = record.source_span {
            indices.source_span.push((span, index));
        }
        if let Some(identity) = record.mir_node_identity {
            indices.mir_node.push((identity, index));
        }
        if let Some(coordinate) = record.mir {
            indices.mir.push((coordinate, index));
        }
        if let Some(identity) = record.neutral_kir_node_identity {
            indices.neutral_kir_node.push((identity, index));
        }
        if let Some(coordinate) = record.neutral_kir {
            indices.neutral_kir.push((coordinate, index));
        }
        if let Some(coordinate) = record.target_kir {
            indices.target_kir.push((coordinate, index));
        }
        if let Some(identity) = record.semantic_operation_id {
            indices.semantic_operation.push((identity, index));
        }
        if let Some(coordinate) = record.compiler_handoff_llvm {
            indices.compiler_handoff_llvm.push((coordinate, index));
        }
        for interval in &record.isa {
            indices.isa.push((
                ProductionSourceIsaCatalogPointV1::new(
                    interval.kernel_ordinal,
                    interval.byte_start,
                ),
                index,
            ));
        }
    }
    indices.source_node.sort_unstable();
    indices.source_span.sort_unstable();
    indices.mir_node.sort_unstable();
    indices.mir.sort_unstable();
    indices.neutral_kir_node.sort_unstable();
    indices.neutral_kir.sort_unstable();
    indices.target_kir.sort_unstable();
    indices.semantic_operation.sort_unstable();
    indices.compiler_handoff_llvm.sort_unstable();
    indices.isa.sort_unstable();
    Ok(indices)
}

fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, ProductionSourceIsaCatalogErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
    Ok(values)
}

fn equal_range_by_key<T, K: Ord, F: Fn(&T) -> &K>(
    values: &[T],
    key: &K,
    field: F,
) -> std::ops::Range<usize> {
    let start = values.partition_point(|value| field(value) < key);
    let end = values.partition_point(|value| field(value) <= key);
    start..end
}

fn catalog_encoded_len(
    records: &[ProductionSourceIsaCatalogRecordV1],
) -> Result<usize, ProductionSourceIsaCatalogErrorV1> {
    let mut length = CATALOG_HEADER_BYTES_V1
        .checked_add(CATALOG_IDENTITY_BYTES_V1)
        .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
    for record in records {
        length = length
            .checked_add(record_encoded_len(record)?)
            .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        if length > MAX_PRODUCTION_SOURCE_ISA_CATALOG_BYTES_V1 {
            return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
        }
    }
    Ok(length)
}

fn record_encoded_len(
    record: &ProductionSourceIsaCatalogRecordV1,
) -> Result<usize, ProductionSourceIsaCatalogErrorV1> {
    let mut length = RECORD_HEADER_BYTES_V1;
    for (present, bytes) in [
        (record.source_node_identity.is_some(), 32),
        (record.source_span.is_some(), SOURCE_SPAN_BYTES_V1),
        (record.mir_node_identity.is_some(), 32),
        (record.mir.is_some(), COORDINATE_BYTES_V1),
        (record.neutral_kir_node_identity.is_some(), 32),
        (record.neutral_kir.is_some(), COORDINATE_BYTES_V1),
        (record.target_kir.is_some(), COORDINATE_BYTES_V1),
        (record.semantic_operation_id.is_some(), 32),
        (record.compiler_handoff_llvm.is_some(), COORDINATE_BYTES_V1),
    ] {
        if present {
            length = length
                .checked_add(bytes)
                .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
        }
    }
    length
        .checked_add(
            record
                .isa
                .len()
                .checked_mul(ISA_INTERVAL_BYTES_V1)
                .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        )
        .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)
}

fn catalog_identity(preimage: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((CATALOG_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(CATALOG_IDENTITY_DOMAIN_V1);
    digest.update(preimage);
    digest.finalize().into()
}

fn encode_structural_binding(
    output: &mut Vec<u8>,
    binding: ProductionSourceIsaCatalogStructuralBindingV1,
) {
    output.extend_from_slice(&binding.identity);
    output.push(match binding.target {
        ProductionSourceIsaCatalogTargetV1::Gfx942 => 1,
        ProductionSourceIsaCatalogTargetV1::Gfx950 => 2,
    });
    output.push(match binding.kir_version {
        ProductionSourceIsaCatalogKirVersionV1::V8 => 8,
        ProductionSourceIsaCatalogKirVersionV1::V9 => 9,
        ProductionSourceIsaCatalogKirVersionV1::V11 => 11,
    });
    output.extend_from_slice(&[0; 6]);
    encode_catalog_content_identity(output, binding.neutral_kernel_ir);
    encode_catalog_content_identity(output, binding.target_bound_kernel_ir);
    push_u64(output, binding.counts.functions);
    push_u64(output, binding.counts.defined_bodies);
    push_u64(output, binding.counts.blocks);
    push_u64(output, binding.counts.operations);
}

fn encode_catalog_content_identity(
    output: &mut Vec<u8>,
    identity: ProductionSourceIsaCatalogContentIdentityV1,
) {
    output.extend_from_slice(&identity.sha256);
    push_u64(output, identity.byte_len);
}

fn encode_content_identity(output: &mut Vec<u8>, identity: ContentIdentityV1) {
    output.extend_from_slice(identity.sha256());
    push_u64(output, identity.byte_len());
}

fn encode_record(
    output: &mut Vec<u8>,
    record: &ProductionSourceIsaCatalogRecordV1,
) -> Result<(), ProductionSourceIsaCatalogErrorV1> {
    validate_record(record)?;
    output.push(match record.kind {
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => 1,
        ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => 2,
        ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => 3,
    });
    output.push(match record.transformation {
        None => 0,
        Some(ProductionSourceIsaCatalogTransformationV1::Preserved) => 1,
        Some(ProductionSourceIsaCatalogTransformationV1::Duplicated) => 2,
        Some(ProductionSourceIsaCatalogTransformationV1::Coalesced) => 3,
        Some(ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced) => 4,
        Some(ProductionSourceIsaCatalogTransformationV1::Eliminated) => 5,
    });
    push_u16(output, record_flags(record));
    push_u32(
        output,
        u32::try_from(record.isa.len())
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
    );
    if let Some(identity) = record.source_node_identity {
        output.extend_from_slice(&identity);
    }
    if let Some(span) = record.source_span {
        encode_span(output, span);
    }
    if let Some(identity) = record.mir_node_identity {
        output.extend_from_slice(&identity);
    }
    if let Some(coordinate) = record.mir {
        encode_coordinate(
            output,
            coordinate.body_ordinal,
            coordinate.block_ordinal,
            coordinate.statement_ordinal,
        );
    }
    if let Some(identity) = record.neutral_kir_node_identity {
        output.extend_from_slice(&identity);
    }
    if let Some(coordinate) = record.neutral_kir {
        encode_coordinate(
            output,
            coordinate.function_ordinal,
            coordinate.block_ordinal,
            coordinate.operation_ordinal,
        );
    }
    if let Some(coordinate) = record.target_kir {
        encode_coordinate(
            output,
            coordinate.function_ordinal,
            coordinate.block_ordinal,
            coordinate.operation_ordinal,
        );
    }
    if let Some(identity) = record.semantic_operation_id {
        output.extend_from_slice(&identity);
    }
    if let Some(coordinate) = record.compiler_handoff_llvm {
        encode_coordinate(
            output,
            coordinate.function_ordinal,
            coordinate.block_ordinal,
            coordinate.instruction_ordinal,
        );
    }
    for interval in &record.isa {
        encode_coordinate(
            output,
            interval.kernel_ordinal,
            interval.byte_start,
            interval.byte_end,
        );
    }
    Ok(())
}

fn record_flags(record: &ProductionSourceIsaCatalogRecordV1) -> u16 {
    let mut flags = 0;
    for (present, flag) in [
        (record.source_node_identity.is_some(), SOURCE_NODE_FLAG_V1),
        (record.source_span.is_some(), SOURCE_SPAN_FLAG_V1),
        (record.mir_node_identity.is_some(), MIR_NODE_FLAG_V1),
        (record.mir.is_some(), MIR_FLAG_V1),
        (
            record.neutral_kir_node_identity.is_some(),
            NEUTRAL_KIR_NODE_FLAG_V1,
        ),
        (record.neutral_kir.is_some(), NEUTRAL_KIR_FLAG_V1),
        (record.target_kir.is_some(), TARGET_KIR_FLAG_V1),
        (
            record.semantic_operation_id.is_some(),
            SEMANTIC_OPERATION_FLAG_V1,
        ),
        (
            record.compiler_handoff_llvm.is_some(),
            COMPILER_HANDOFF_LLVM_FLAG_V1,
        ),
    ] {
        if present {
            flags |= flag;
        }
    }
    flags
}

fn encode_span(output: &mut Vec<u8>, span: DebugSourceMapSpanV1) {
    output.extend_from_slice(&span.file_identity());
    push_u64(output, span.byte_start());
    push_u64(output, span.byte_end());
    push_u32(output, span.line());
    push_u32(output, span.column());
}

fn encode_coordinate(output: &mut Vec<u8>, first: u64, second: u64, third: u64) {
    push_u64(output, first);
    push_u64(output, second);
    push_u64(output, third);
}

fn decode_record(
    decoder: &mut CatalogDecoderV1<'_>,
    remaining_isa_budget: usize,
) -> Result<ProductionSourceIsaCatalogRecordV1, ProductionSourceIsaCatalogErrorV1> {
    let kind = match decoder.u8()? {
        1 => ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir,
        2 => ProductionSourceIsaCatalogRecordKindV1::SourceAnchored,
        3 => ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance,
        _ => return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord),
    };
    let transformation = match decoder.u8()? {
        0 => None,
        1 => Some(ProductionSourceIsaCatalogTransformationV1::Preserved),
        2 => Some(ProductionSourceIsaCatalogTransformationV1::Duplicated),
        3 => Some(ProductionSourceIsaCatalogTransformationV1::Coalesced),
        4 => Some(ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced),
        5 => Some(ProductionSourceIsaCatalogTransformationV1::Eliminated),
        _ => return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord),
    };
    let flags = decoder.u16()?;
    if flags & !ALL_RECORD_FLAGS_V1 != 0 {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
    }
    let expected_flags = match kind {
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => ELIMINATED_RECORD_FLAGS_V1,
        ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => ALL_RECORD_FLAGS_V1,
        ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => NO_SOURCE_RECORD_FLAGS_V1,
    };
    if flags != expected_flags
        || matches!(
            kind,
            ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
        ) != transformation.is_none()
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
    }
    let isa_count = usize::try_from(decoder.u32()?)
        .map_err(|_| ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
    if isa_count > MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1
        || isa_count > remaining_isa_budget
    {
        return Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit);
    }
    let transformation_requires_empty =
        transformation == Some(ProductionSourceIsaCatalogTransformationV1::Eliminated);
    let eliminated_before_kir = matches!(
        kind,
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir
    );
    if (eliminated_before_kir && isa_count != 0)
        || (!eliminated_before_kir && (isa_count == 0) != transformation_requires_empty)
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord);
    }
    let has = |flag| flags & flag != 0;
    let fixed_field_bytes = [
        (SOURCE_NODE_FLAG_V1, 32),
        (SOURCE_SPAN_FLAG_V1, SOURCE_SPAN_BYTES_V1),
        (MIR_NODE_FLAG_V1, 32),
        (MIR_FLAG_V1, COORDINATE_BYTES_V1),
        (NEUTRAL_KIR_NODE_FLAG_V1, 32),
        (NEUTRAL_KIR_FLAG_V1, COORDINATE_BYTES_V1),
        (TARGET_KIR_FLAG_V1, COORDINATE_BYTES_V1),
        (SEMANTIC_OPERATION_FLAG_V1, 32),
        (COMPILER_HANDOFF_LLVM_FLAG_V1, COORDINATE_BYTES_V1),
    ]
    .into_iter()
    .try_fold(0_usize, |bytes, (flag, field_bytes)| {
        if has(flag) {
            bytes.checked_add(field_bytes)
        } else {
            Some(bytes)
        }
    })
    .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
    let required_record_bytes = fixed_field_bytes
        .checked_add(
            isa_count
                .checked_mul(ISA_INTERVAL_BYTES_V1)
                .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?,
        )
        .ok_or(ProductionSourceIsaCatalogErrorV1::ResourceLimit)?;
    if required_record_bytes
        .checked_add(CATALOG_IDENTITY_BYTES_V1)
        .is_none_or(|required| required > decoder.remaining())
    {
        return Err(ProductionSourceIsaCatalogErrorV1::InvalidLength);
    }
    let source_node_identity = has(SOURCE_NODE_FLAG_V1)
        .then(|| decoder.identity())
        .transpose()?;
    let source_span = has(SOURCE_SPAN_FLAG_V1)
        .then(|| decoder.span())
        .transpose()?;
    let mir_node_identity = has(MIR_NODE_FLAG_V1)
        .then(|| decoder.identity())
        .transpose()?;
    let mir = has(MIR_FLAG_V1)
        .then(|| decoder.mir_coordinate())
        .transpose()?;
    let neutral_kir_node_identity = has(NEUTRAL_KIR_NODE_FLAG_V1)
        .then(|| decoder.identity())
        .transpose()?;
    let neutral_kir = has(NEUTRAL_KIR_FLAG_V1)
        .then(|| decoder.kir_coordinate())
        .transpose()?;
    let target_kir = has(TARGET_KIR_FLAG_V1)
        .then(|| decoder.kir_coordinate())
        .transpose()?;
    let semantic_operation_id = has(SEMANTIC_OPERATION_FLAG_V1)
        .then(|| decoder.identity())
        .transpose()?;
    let compiler_handoff_llvm = has(COMPILER_HANDOFF_LLVM_FLAG_V1)
        .then(|| decoder.llvm_coordinate())
        .transpose()?;
    let mut isa = Vec::new();
    isa.try_reserve_exact(isa_count)
        .map_err(|_| ProductionSourceIsaCatalogErrorV1::AllocationFailure)?;
    for _ in 0..isa_count {
        isa.push(decoder.isa_interval()?);
    }
    let record = ProductionSourceIsaCatalogRecordV1 {
        kind,
        source_node_identity,
        source_span,
        mir_node_identity,
        mir,
        neutral_kir_node_identity,
        neutral_kir,
        target_kir,
        semantic_operation_id,
        compiler_handoff_llvm,
        isa,
        transformation,
    };
    validate_record(&record)?;
    Ok(record)
}

fn decode_target(
    value: u8,
) -> Result<ProductionSourceIsaCatalogTargetV1, ProductionSourceIsaCatalogErrorV1> {
    match value {
        1 => Ok(ProductionSourceIsaCatalogTargetV1::Gfx942),
        2 => Ok(ProductionSourceIsaCatalogTargetV1::Gfx950),
        _ => Err(ProductionSourceIsaCatalogErrorV1::InvalidStructuralBinding),
    }
}

fn decode_kir_version(
    value: u8,
) -> Result<ProductionSourceIsaCatalogKirVersionV1, ProductionSourceIsaCatalogErrorV1> {
    match value {
        8 => Ok(ProductionSourceIsaCatalogKirVersionV1::V8),
        9 => Ok(ProductionSourceIsaCatalogKirVersionV1::V9),
        11 => Ok(ProductionSourceIsaCatalogKirVersionV1::V11),
        _ => Err(ProductionSourceIsaCatalogErrorV1::InvalidStructuralBinding),
    }
}

struct CatalogDecoderV1<'a> {
    encoded: &'a [u8],
    cursor: usize,
}

impl<'a> CatalogDecoderV1<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, cursor: 0 }
    }

    fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.cursor)
    }

    fn take(&mut self, bytes: usize) -> Result<&'a [u8], ProductionSourceIsaCatalogErrorV1> {
        let end = self
            .cursor
            .checked_add(bytes)
            .ok_or(ProductionSourceIsaCatalogErrorV1::InvalidLength)?;
        let value = self
            .encoded
            .get(self.cursor..end)
            .ok_or(ProductionSourceIsaCatalogErrorV1::InvalidLength)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProductionSourceIsaCatalogErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionSourceIsaCatalogErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(
            |_| ProductionSourceIsaCatalogErrorV1::InvalidLength,
        )?))
    }

    fn u32(&mut self) -> Result<u32, ProductionSourceIsaCatalogErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| ProductionSourceIsaCatalogErrorV1::InvalidLength,
        )?))
    }

    fn u64(&mut self) -> Result<u64, ProductionSourceIsaCatalogErrorV1> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| ProductionSourceIsaCatalogErrorV1::InvalidLength,
        )?))
    }

    fn identity(&mut self) -> Result<[u8; 32], ProductionSourceIsaCatalogErrorV1> {
        self.take(32)?
            .try_into()
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::InvalidLength)
    }

    fn content_identity(&mut self) -> Result<ContentIdentityV1, ProductionSourceIsaCatalogErrorV1> {
        Ok(ContentIdentityV1::from_parts(self.identity()?, self.u64()?))
    }

    fn span(&mut self) -> Result<DebugSourceMapSpanV1, ProductionSourceIsaCatalogErrorV1> {
        let file_identity = self.identity()?;
        let byte_start = self.u64()?;
        let byte_end = self.u64()?;
        let line = self.u32()?;
        let column = self.u32()?;
        DebugSourceMapSpanV1::new_eliminated(file_identity, byte_start, byte_end, line, column)
            .map_err(|_| ProductionSourceIsaCatalogErrorV1::InvalidRecord)
    }

    fn coordinate(&mut self) -> Result<(u64, u64, u64), ProductionSourceIsaCatalogErrorV1> {
        Ok((self.u64()?, self.u64()?, self.u64()?))
    }

    fn mir_coordinate(
        &mut self,
    ) -> Result<ProductionSourceIsaMirCoordinateV1, ProductionSourceIsaCatalogErrorV1> {
        let (body_ordinal, block_ordinal, statement_ordinal) = self.coordinate()?;
        Ok(ProductionSourceIsaMirCoordinateV1 {
            body_ordinal,
            block_ordinal,
            statement_ordinal,
        })
    }

    fn kir_coordinate(
        &mut self,
    ) -> Result<ProductionSourceIsaKirCoordinateV1, ProductionSourceIsaCatalogErrorV1> {
        let (function_ordinal, block_ordinal, operation_ordinal) = self.coordinate()?;
        Ok(ProductionSourceIsaKirCoordinateV1 {
            function_ordinal,
            block_ordinal,
            operation_ordinal,
        })
    }

    fn llvm_coordinate(
        &mut self,
    ) -> Result<ProductionSourceIsaLlvmCoordinateV1, ProductionSourceIsaCatalogErrorV1> {
        let (function_ordinal, block_ordinal, instruction_ordinal) = self.coordinate()?;
        Ok(ProductionSourceIsaLlvmCoordinateV1 {
            function_ordinal,
            block_ordinal,
            instruction_ordinal,
        })
    }

    fn isa_interval(
        &mut self,
    ) -> Result<ProductionSourceIsaCatalogIntervalV1, ProductionSourceIsaCatalogErrorV1> {
        let (kernel_ordinal, byte_start, byte_end) = self.coordinate()?;
        Ok(ProductionSourceIsaCatalogIntervalV1 {
            kernel_ordinal,
            byte_start,
            byte_end,
        })
    }
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(seed: u8, start: u64) -> DebugSourceMapSpanV1 {
        DebugSourceMapSpanV1::new([seed; 32], start, start + 4, 1, 1).unwrap()
    }

    fn source_record(
        source: u8,
        operation: u64,
        pcs: &[u64],
    ) -> ProductionSourceIsaCatalogRecordV1 {
        let transformation = if pcs.is_empty() {
            ProductionSourceIsaCatalogTransformationV1::Eliminated
        } else {
            ProductionSourceIsaCatalogTransformationV1::Preserved
        };
        ProductionSourceIsaCatalogRecordV1 {
            kind: ProductionSourceIsaCatalogRecordKindV1::SourceAnchored,
            source_node_identity: Some([source; 32]),
            source_span: Some(span(source, operation * 4)),
            mir_node_identity: Some([source.wrapping_add(10); 32]),
            mir: Some(ProductionSourceIsaMirCoordinateV1::new(0, 0, operation).unwrap()),
            neutral_kir_node_identity: Some([source.wrapping_add(20); 32]),
            neutral_kir: Some(ProductionSourceIsaKirCoordinateV1::new(0, 0, operation).unwrap()),
            target_kir: Some(ProductionSourceIsaKirCoordinateV1::new(0, 0, operation).unwrap()),
            semantic_operation_id: Some([source.wrapping_add(30); 32]),
            compiler_handoff_llvm: Some(
                ProductionSourceIsaLlvmCoordinateV1::new(0, 0, operation).unwrap(),
            ),
            isa: pcs
                .iter()
                .map(|pc| ProductionSourceIsaCatalogIntervalV1::new(0, *pc, *pc + 4).unwrap())
                .collect(),
            transformation: Some(transformation),
        }
    }

    fn eliminated_record(source: u8) -> ProductionSourceIsaCatalogRecordV1 {
        ProductionSourceIsaCatalogRecordV1 {
            kind: ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir,
            source_node_identity: Some([source; 32]),
            source_span: Some(span(source, 16)),
            mir_node_identity: Some([source.wrapping_add(10); 32]),
            mir: Some(ProductionSourceIsaMirCoordinateV1::new(0, 0, 4).unwrap()),
            neutral_kir_node_identity: None,
            neutral_kir: None,
            target_kir: None,
            semantic_operation_id: None,
            compiler_handoff_llvm: None,
            isa: Vec::new(),
            transformation: None,
        }
    }

    fn no_source_record(operation: u64, pc: u64) -> ProductionSourceIsaCatalogRecordV1 {
        ProductionSourceIsaCatalogRecordV1 {
            kind: ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance,
            source_node_identity: None,
            source_span: None,
            mir_node_identity: None,
            mir: None,
            neutral_kir_node_identity: None,
            neutral_kir: None,
            target_kir: Some(ProductionSourceIsaKirCoordinateV1::new(0, 0, operation).unwrap()),
            semantic_operation_id: Some([90; 32]),
            compiler_handoff_llvm: Some(
                ProductionSourceIsaLlvmCoordinateV1::new(0, 0, operation).unwrap(),
            ),
            isa: vec![ProductionSourceIsaCatalogIntervalV1::new(0, pc, pc + 4).unwrap()],
            transformation: Some(ProductionSourceIsaCatalogTransformationV1::Preserved),
        }
    }

    fn catalog_from_records(
        mut records: Vec<ProductionSourceIsaCatalogRecordV1>,
    ) -> ProductionSourceIsaCatalogV1 {
        for record in &mut records {
            record.isa.sort_unstable();
        }
        records.sort_unstable();
        validate_record_collection(&records).unwrap();
        let indices = build_catalog_indices(&records).unwrap();
        let mut catalog = ProductionSourceIsaCatalogV1 {
            identity: [0; 32],
            correlation_identity: [1; 32],
            semantic_map_identity: [2; 32],
            source_map_v2_identity: ProductionSourceIsaCatalogContentIdentityV1 {
                sha256: [3; 32],
                byte_len: 100,
            },
            artifact_identity: ContentIdentityV1::from_parts([4; 32], 200),
            structural_binding: ProductionSourceIsaCatalogStructuralBindingV1 {
                identity: [5; 32],
                target: ProductionSourceIsaCatalogTargetV1::Gfx942,
                kir_version: ProductionSourceIsaCatalogKirVersionV1::V8,
                neutral_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                    sha256: [6; 32],
                    byte_len: 300,
                },
                target_bound_kernel_ir: ProductionSourceIsaCatalogContentIdentityV1 {
                    sha256: [7; 32],
                    byte_len: 400,
                },
                counts: ProductionSourceIsaCatalogStructuralCountsV1 {
                    functions: 1,
                    defined_bodies: 1,
                    blocks: 1,
                    operations: 8,
                },
            },
            records,
            indices,
        };
        let preimage = catalog.canonical_preimage().unwrap();
        catalog.identity = catalog_identity(&preimage);
        catalog
    }

    fn fixture_catalog() -> ProductionSourceIsaCatalogV1 {
        catalog_from_records(vec![
            source_record(1, 0, &[8, 0]),
            source_record(1, 1, &[]),
            source_record(2, 2, &[0]),
            eliminated_record(3),
            no_source_record(3, 12),
        ])
    }

    fn rehash(encoded: &mut [u8]) {
        let identity_offset = encoded.len() - CATALOG_IDENTITY_BYTES_V1;
        let identity = catalog_identity(&encoded[..identity_offset]);
        encoded[identity_offset..].copy_from_slice(&identity);
    }

    #[test]
    fn coordinate_constructors_are_exact_and_bounded() {
        assert_eq!(
            ProductionSourceIsaKirCoordinateV1::new(1, 2, 3)
                .unwrap()
                .operation_ordinal(),
            3
        );
        assert!(matches!(
            ProductionSourceIsaMirCoordinateV1::new(u64::from(u32::MAX) + 1, 0, 0),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
        assert!(matches!(
            ProductionSourceIsaCatalogIntervalV1::new(0, 1, 5),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
        assert!(matches!(
            ProductionSourceIsaCatalogIntervalV1::new(1, 0, 4),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
    }

    #[test]
    fn canonical_length_and_query_ordinals_are_exact() {
        let catalog = fixture_catalog();
        let encoded = catalog.to_canonical_bytes().unwrap();
        assert_eq!(catalog.canonical_byte_len().unwrap(), encoded.len() as u64);

        let mut matches = catalog.query_source_node([1; 32]).unwrap();
        assert_eq!(matches.len(), 2);
        let (first_ordinal, first) = matches.next_with_ordinal().unwrap();
        assert_eq!(first, &catalog.records()[first_ordinal]);
        assert_eq!(matches.len(), 1);
        let second = matches.next().unwrap();
        let second_ordinal = catalog
            .records()
            .iter()
            .position(|record| std::ptr::eq(record, second))
            .unwrap();
        assert_ne!(first_ordinal, second_ordinal);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn canonical_round_trip_retains_every_record_axis_and_identity() {
        let catalog = fixture_catalog();
        let encoded = catalog.to_canonical_bytes().unwrap();
        let decoded = InertProductionSourceIsaCatalogV1::from_canonical_bytes(&encoded).unwrap();
        let claims = &decoded.claimed_catalog;
        assert_eq!(claims.format_version(), 1);
        assert_eq!(decoded.claimed_identity(), catalog.identity());
        assert_eq!(
            decoded.claimed_correlation_identity(),
            catalog.correlation_identity()
        );
        assert_eq!(
            claims.semantic_map_identity(),
            catalog.semantic_map_identity()
        );
        assert_eq!(
            claims.source_map_v2_identity(),
            catalog.source_map_v2_identity()
        );
        assert_eq!(claims.artifact_identity(), catalog.artifact_identity());
        assert_eq!(claims.structural_binding(), catalog.structural_binding());
        assert_eq!(claims.records(), catalog.records());
        assert_eq!(claims.to_canonical_bytes().unwrap(), encoded);
        assert!(!decoded.proves_complete_machine_instruction_coverage());
        assert!(!decoded.proves_a_schedule());
        assert!(!decoded.proves_live_program_counter_ownership());
        assert!(!decoded.proves_semantic_refinement());
        assert!(!decoded.proves_optimized_or_final_llvm_custody());
        assert!(!decoded.grants_debugger_authority());
        assert!(!decoded.grants_profiler_authority());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_runtime_authority());
    }

    #[test]
    fn exact_queries_round_trip_source_kir_operation_and_sparse_isa() {
        let catalog = fixture_catalog();
        assert_eq!(catalog.query_source_node([1; 32]).unwrap().len(), 2);
        assert_eq!(catalog.query_source_span(span(1, 0)).unwrap().len(), 1);
        assert_eq!(catalog.query_mir_node([11; 32]).unwrap().len(), 2);
        assert_eq!(
            catalog
                .query_mir(ProductionSourceIsaMirCoordinateV1::new(0, 0, 0).unwrap())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(catalog.query_neutral_kir_node([21; 32]).unwrap().len(), 2);
        assert_eq!(
            catalog
                .query_neutral_kir(ProductionSourceIsaKirCoordinateV1::new(0, 0, 0).unwrap())
                .unwrap()
                .len(),
            1
        );
        let target = catalog
            .query_target_kir(ProductionSourceIsaKirCoordinateV1::new(0, 0, 2).unwrap())
            .unwrap()
            .next()
            .unwrap();
        assert_eq!(target.source_node_identity(), Some([2; 32]));
        assert_eq!(catalog.query_semantic_operation([31; 32]).unwrap().len(), 2);
        assert_eq!(
            catalog
                .query_compiler_handoff_llvm(
                    ProductionSourceIsaLlvmCoordinateV1::new(0, 0, 2).unwrap(),
                )
                .unwrap()
                .len(),
            1
        );
        let reverse = catalog
            .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(0, 0))
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(reverse.len(), 2);
        assert!(reverse.iter().all(|record| record.source_span().is_some()));
        assert_eq!(
            catalog
                .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(0, 1))
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnalignedProgramCounter
        );
        assert_eq!(
            catalog
                .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(1, 0))
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMetadataKernelOrdinal
        );
        assert_eq!(
            catalog
                .query_target_kir(ProductionSourceIsaKirCoordinateV1::new(0, 0, 99).unwrap())
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownTargetKirCoordinate
        );
        assert_eq!(
            catalog.query_source_node([99; 32]).unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSourceNode
        );
        assert_eq!(
            catalog.query_source_span(span(99, 0)).unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSourceSpan
        );
        assert_eq!(
            catalog.query_mir_node([99; 32]).unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMirNode
        );
        assert_eq!(
            catalog
                .query_mir(ProductionSourceIsaMirCoordinateV1::new(0, 0, 99).unwrap())
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownMirCoordinate
        );
        assert_eq!(
            catalog.query_neutral_kir_node([99; 32]).unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownNeutralKirNode
        );
        assert_eq!(
            catalog
                .query_neutral_kir(ProductionSourceIsaKirCoordinateV1::new(0, 0, 99).unwrap())
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownNeutralKirCoordinate
        );
        assert_eq!(
            catalog.query_semantic_operation([99; 32]).unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownSemanticOperation
        );
        assert_eq!(
            catalog
                .query_compiler_handoff_llvm(
                    ProductionSourceIsaLlvmCoordinateV1::new(0, 0, 99).unwrap(),
                )
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::UnknownCompilerHandoffLlvmCoordinate
        );
        assert_eq!(
            catalog
                .query_isa_pc(ProductionSourceIsaCatalogPointV1::new(0, 100))
                .unwrap_err(),
            ProductionSourceIsaCatalogQueryUnavailableV1::ProgramCounterIsNotAnAdmittedAnchor
        );
    }

    #[test]
    fn all_backend_transformation_statuses_round_trip_without_inference() {
        let transformations = [
            ProductionSourceIsaCatalogTransformationV1::Preserved,
            ProductionSourceIsaCatalogTransformationV1::Duplicated,
            ProductionSourceIsaCatalogTransformationV1::Coalesced,
            ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced,
            ProductionSourceIsaCatalogTransformationV1::Eliminated,
        ];
        let records = transformations
            .into_iter()
            .enumerate()
            .map(|(index, transformation)| {
                let pcs =
                    if transformation == ProductionSourceIsaCatalogTransformationV1::Eliminated {
                        Vec::new()
                    } else {
                        vec![u64::try_from(index).unwrap() * 4]
                    };
                let mut record = source_record(
                    u8::try_from(index + 1).unwrap(),
                    u64::try_from(index).unwrap(),
                    &pcs,
                );
                record.transformation = Some(transformation);
                record
            })
            .collect();
        let catalog = catalog_from_records(records);
        let decoded = InertProductionSourceIsaCatalogV1::from_canonical_bytes(
            &catalog.to_canonical_bytes().unwrap(),
        )
        .unwrap();
        let observed = decoded
            .claimed_catalog
            .records()
            .iter()
            .map(ProductionSourceIsaCatalogRecordV1::transformation)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(observed, transformations.into_iter().map(Some).collect());

        let mut invalid_empty_preserved = source_record(20, 20, &[]);
        invalid_empty_preserved.transformation =
            Some(ProductionSourceIsaCatalogTransformationV1::Preserved);
        assert!(matches!(
            validate_record(&invalid_empty_preserved),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
        let mut invalid_nonempty_eliminated = source_record(21, 21, &[84]);
        invalid_nonempty_eliminated.transformation =
            Some(ProductionSourceIsaCatalogTransformationV1::Eliminated);
        assert!(matches!(
            validate_record(&invalid_nonempty_eliminated),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
    }

    #[test]
    fn canonical_order_is_independent_of_input_and_interval_order() {
        let canonical = catalog_from_records(vec![
            source_record(1, 0, &[0, 8]),
            source_record(2, 1, &[4]),
            eliminated_record(3),
        ]);
        let reordered = catalog_from_records(vec![
            eliminated_record(3),
            source_record(2, 1, &[4]),
            source_record(1, 0, &[8, 0]),
        ]);
        assert_eq!(canonical.identity(), reordered.identity());
        assert_eq!(
            canonical.to_canonical_bytes().unwrap(),
            reordered.to_canonical_bytes().unwrap()
        );
    }

    #[test]
    fn hostile_headers_records_and_identity_fail_closed() {
        let base = fixture_catalog().to_canonical_bytes().unwrap();

        let mut wrong_magic = base.clone();
        wrong_magic[0] ^= 1;
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&wrong_magic),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidMagic)
        ));

        let mut reserved_header = base.clone();
        reserved_header[12] = 1;
        rehash(&mut reserved_header);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&reserved_header),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidHeader)
        ));

        let mut zero_source_map = base.clone();
        zero_source_map[104..136].fill(0);
        rehash(&mut zero_source_map);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&zero_source_map),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidIdentity)
        ));

        let mut invalid_structural_counts = base.clone();
        invalid_structural_counts[304..312].copy_from_slice(&1_u64.to_le_bytes());
        invalid_structural_counts[312..320].copy_from_slice(&2_u64.to_le_bytes());
        rehash(&mut invalid_structural_counts);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&invalid_structural_counts),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidStructuralBinding)
        ));

        let mut unavailable_v9 = base.clone();
        unavailable_v9[217] = 9;
        rehash(&mut unavailable_v9);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&unavailable_v9),
            Err(ProductionSourceIsaCatalogErrorV1::SourceProjectionForKirV9Unavailable)
        ));

        let mut unavailable_v11 = base.clone();
        unavailable_v11[217] = 11;
        rehash(&mut unavailable_v11);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&unavailable_v11),
            Err(ProductionSourceIsaCatalogErrorV1::SourceProjectionForKirV11Unavailable)
        ));

        let mut invalid_kind = base.clone();
        invalid_kind[CATALOG_HEADER_BYTES_V1] = 0;
        rehash(&mut invalid_kind);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&invalid_kind),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));

        let mut invalid_flags = base.clone();
        invalid_flags[CATALOG_HEADER_BYTES_V1 + 2..CATALOG_HEADER_BYTES_V1 + 4].fill(0);
        rehash(&mut invalid_flags);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&invalid_flags),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));

        let mut invalid_source_identity = base.clone();
        invalid_source_identity
            [CATALOG_HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1..CATALOG_HEADER_BYTES_V1 + 40]
            .fill(0);
        rehash(&mut invalid_source_identity);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&invalid_source_identity),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));

        let mut wrong_identity = base.clone();
        *wrong_identity.last_mut().unwrap() ^= 1;
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&wrong_identity),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidIdentity)
        ));

        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&base[..base.len() - 1]),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidLength)
        ));
    }

    #[test]
    fn hostile_record_order_and_sparse_interval_mutations_fail_closed() {
        let two_records =
            catalog_from_records(vec![source_record(1, 0, &[0]), source_record(2, 1, &[4])]);
        let mut reordered = two_records.to_canonical_bytes().unwrap();
        let record_bytes = record_encoded_len(&two_records.records[0]).unwrap();
        let first = CATALOG_HEADER_BYTES_V1;
        let second = first + record_bytes;
        let left = reordered[first..second].to_vec();
        let right = reordered[second..second + record_bytes].to_vec();
        reordered[first..second].copy_from_slice(&right);
        reordered[second..second + record_bytes].copy_from_slice(&left);
        rehash(&mut reordered);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&reordered),
            Err(ProductionSourceIsaCatalogErrorV1::NonCanonicalRecordOrder)
        ));

        let catalog = fixture_catalog();
        let mut invalid_interval = catalog.to_canonical_bytes().unwrap();
        let source_index = catalog
            .records
            .iter()
            .position(|record| !record.isa.is_empty())
            .unwrap();
        let record_offset = CATALOG_HEADER_BYTES_V1
            + catalog.records[..source_index]
                .iter()
                .map(|record| record_encoded_len(record).unwrap())
                .sum::<usize>();
        let interval_offset = record_offset
            + record_encoded_len(&catalog.records[source_index]).unwrap()
            - catalog.records[source_index].isa.len() * ISA_INTERVAL_BYTES_V1;
        invalid_interval[interval_offset + 8..interval_offset + 16]
            .copy_from_slice(&1_u64.to_le_bytes());
        invalid_interval[interval_offset + 16..interval_offset + 24]
            .copy_from_slice(&5_u64.to_le_bytes());
        rehash(&mut invalid_interval);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&invalid_interval),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
    }

    #[test]
    fn hostile_declared_resources_are_rejected_before_allocation() {
        assert_eq!(MIN_VALID_RECORD_BYTES_V1, 88);

        let base = fixture_catalog().to_canonical_bytes().unwrap();
        let mut too_many_records = base.clone();
        too_many_records[24..32].copy_from_slice(
            &u64::try_from(MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        rehash(&mut too_many_records);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&too_many_records),
            Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit)
        ));

        const PADDED_HOSTILE_BYTES: usize = 4 * 1024 * 1024;
        let mut padded_max_records = fixture_catalog().to_canonical_bytes().unwrap();
        padded_max_records.resize(PADDED_HOSTILE_BYTES, 0);
        padded_max_records[16..24]
            .copy_from_slice(&u64::try_from(PADDED_HOSTILE_BYTES).unwrap().to_le_bytes());
        padded_max_records[24..32].copy_from_slice(
            &u64::try_from(MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1)
                .unwrap()
                .to_le_bytes(),
        );
        padded_max_records[32..40].copy_from_slice(&0_u64.to_le_bytes());
        rehash(&mut padded_max_records);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&padded_max_records),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidLength)
        ));

        let mut too_many_isa = base;
        too_many_isa[32..40].copy_from_slice(
            &u64::try_from(MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        rehash(&mut too_many_isa);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&too_many_isa),
            Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit)
        ));

        let source_only = catalog_from_records(vec![source_record(1, 0, &[0])]);
        let mut per_record_exceeds_global = source_only.to_canonical_bytes().unwrap();
        per_record_exceeds_global[32..40].copy_from_slice(&1_u64.to_le_bytes());
        per_record_exceeds_global[CATALOG_HEADER_BYTES_V1 + 4..CATALOG_HEADER_BYTES_V1 + 8]
            .copy_from_slice(&2_u32.to_le_bytes());
        rehash(&mut per_record_exceeds_global);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&per_record_exceeds_global),
            Err(ProductionSourceIsaCatalogErrorV1::ResourceLimit)
        ));

        let mut per_record_bytes_missing = source_only.to_canonical_bytes().unwrap();
        per_record_bytes_missing[32..40].copy_from_slice(&2_u64.to_le_bytes());
        per_record_bytes_missing[CATALOG_HEADER_BYTES_V1 + 4..CATALOG_HEADER_BYTES_V1 + 8]
            .copy_from_slice(&2_u32.to_le_bytes());
        rehash(&mut per_record_bytes_missing);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&per_record_bytes_missing),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidLength)
        ));

        let mut inconsistent_outcome = source_only.to_canonical_bytes().unwrap();
        inconsistent_outcome[CATALOG_HEADER_BYTES_V1 + 1] = 5;
        rehash(&mut inconsistent_outcome);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&inconsistent_outcome),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));

        let eliminated_anchor = catalog_from_records(vec![source_record(1, 0, &[])]);
        let mut inconsistent_empty_outcome = eliminated_anchor.to_canonical_bytes().unwrap();
        inconsistent_empty_outcome[CATALOG_HEADER_BYTES_V1 + 1] = 1;
        rehash(&mut inconsistent_empty_outcome);
        assert!(matches!(
            InertProductionSourceIsaCatalogV1::from_canonical_bytes(&inconsistent_empty_outcome),
            Err(ProductionSourceIsaCatalogErrorV1::InvalidRecord)
        ));
    }
}
