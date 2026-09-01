//! Authority-free, lossless characteristic projections of admitted source/ISA catalogs.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub const SOURCE_ISA_CHARACTERISTIC_MAGIC_V1: &[u8; 8] = b"F2SICH1\0";
pub const SOURCE_ISA_CHARACTERISTIC_VERSION_V1: u16 = 1;
pub const SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1: usize = 512;
pub const SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1: usize = 32;
pub const SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1: usize = 152;
// The producer may simultaneously retain every bounded target correlation, pre-KIR fact,
// structural target header, and sparse ISA anchor. The observer's fixed-width optional-axis
// slots make that projection larger than the producer's compact 64 MiB catalog.
pub const MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1: usize = 128 * 1024 * 1024;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1: usize = 528_384;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1: usize = 65_536;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1: usize = 262_144;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1: usize = 4_096;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1: usize = 65_536;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1: usize = 1_016_800;
pub const MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1: u16 = 64;

const TARGET_HEADER_BYTES_V1: usize = 80;
const TARGET_CORRELATION_PREFIX_BYTES_V1: usize = 312;
const PRE_KIR_FACT_BYTES_V1: usize = 184;
const ISA_INTERVAL_BYTES_V1: usize = 24;
const COLLECTION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC/V1\0";
const QUERY_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-QUERY/V1\0";
const TARGET_QUERY_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-TARGET-QUERY/V1\0";
const TARGET_MATCH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-TARGET-MATCH/V1\0";
const MATCH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-MATCH/V1\0";
const TARGET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-TARGET/V1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-ISA-CHARACTERISTIC-CURSOR/V1\0";
const CURSOR_MAGIC_V1: &[u8; 8] = b"F2SICU1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIsaCharacteristicErrorV1 {
    Malformed,
    InvalidTag,
    InvalidClaim,
    ResourceLimit,
    AllocationFailure,
    NonCanonical,
    IdentityMismatch,
    InvalidCursor,
}

impl fmt::Display for SourceIsaCharacteristicErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "malformed source/ISA characteristic evidence",
            Self::InvalidTag => "source/ISA characteristic evidence has an invalid tag",
            Self::InvalidClaim => "source/ISA characteristic evidence has an invalid claim",
            Self::ResourceLimit => "source/ISA characteristic evidence exceeds a resource limit",
            Self::AllocationFailure => "cannot allocate bounded source/ISA characteristic evidence",
            Self::NonCanonical => "source/ISA characteristic evidence is not canonical",
            Self::IdentityMismatch => {
                "source/ISA characteristic identity differs from its canonical bytes"
            }
            Self::InvalidCursor => "source/ISA characteristic query cursor is invalid",
        })
    }
}

impl Error for SourceIsaCharacteristicErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl SourceIsaCharacteristicContentIdentityV1 {
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self { sha256, byte_len })
    }
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceIsaCharacteristicTargetProfileV1 {
    Gfx942 = 1,
    Gfx950 = 2,
}

impl SourceIsaCharacteristicTargetProfileV1 {
    pub const fn code(self) -> u8 {
        self as u8
    }
    fn decode(code: u8) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        match code {
            1 => Ok(Self::Gfx942),
            2 => Ok(Self::Gfx950),
            _ => Err(SourceIsaCharacteristicErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceIsaCharacteristicKirVersionV1 {
    V8 = 8,
    V9 = 9,
}

impl SourceIsaCharacteristicKirVersionV1 {
    pub const fn code(self) -> u8 {
        self as u8
    }
    fn decode(code: u8) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        match code {
            8 => Ok(Self::V8),
            9 => Ok(Self::V9),
            _ => Err(SourceIsaCharacteristicErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicStructuralCountsV1 {
    pub functions: u64,
    pub defined_bodies: u64,
    pub blocks: u64,
    pub operations: u64,
}

impl SourceIsaCharacteristicStructuralCountsV1 {
    fn validate(self) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if self.defined_bodies > self.functions || self.blocks < self.defined_bodies {
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicBindingV1 {
    target_profile: SourceIsaCharacteristicTargetProfileV1,
    kir_version: SourceIsaCharacteristicKirVersionV1,
    structural_identity: [u8; 32],
    structural_counts: SourceIsaCharacteristicStructuralCountsV1,
    source_map_v2: SourceIsaCharacteristicContentIdentityV1,
    neutral_kir: SourceIsaCharacteristicContentIdentityV1,
    target_kir: SourceIsaCharacteristicContentIdentityV1,
    artifact: SourceIsaCharacteristicContentIdentityV1,
    catalog: SourceIsaCharacteristicContentIdentityV1,
    structural_bridge: SourceIsaCharacteristicContentIdentityV1,
    correlation_identity: [u8; 32],
    semantic_map_identity: [u8; 32],
}

impl SourceIsaCharacteristicBindingV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_profile: SourceIsaCharacteristicTargetProfileV1,
        kir_version: SourceIsaCharacteristicKirVersionV1,
        structural_identity: [u8; 32],
        structural_counts: SourceIsaCharacteristicStructuralCountsV1,
        source_map_v2: SourceIsaCharacteristicContentIdentityV1,
        neutral_kir: SourceIsaCharacteristicContentIdentityV1,
        target_kir: SourceIsaCharacteristicContentIdentityV1,
        artifact: SourceIsaCharacteristicContentIdentityV1,
        catalog: SourceIsaCharacteristicContentIdentityV1,
        structural_bridge: SourceIsaCharacteristicContentIdentityV1,
        correlation_identity: [u8; 32],
        semantic_map_identity: [u8; 32],
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        structural_counts.validate()?;
        if structural_identity == [0; 32]
            || correlation_identity == [0; 32]
            || semantic_map_identity == [0; 32]
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            target_profile,
            kir_version,
            structural_identity,
            structural_counts,
            source_map_v2,
            neutral_kir,
            target_kir,
            artifact,
            catalog,
            structural_bridge,
            correlation_identity,
            semantic_map_identity,
        })
    }
    pub const fn target_profile(self) -> SourceIsaCharacteristicTargetProfileV1 {
        self.target_profile
    }
    pub const fn kir_version(self) -> SourceIsaCharacteristicKirVersionV1 {
        self.kir_version
    }
    pub const fn structural_identity(self) -> [u8; 32] {
        self.structural_identity
    }
    pub const fn structural_counts(self) -> SourceIsaCharacteristicStructuralCountsV1 {
        self.structural_counts
    }
    pub const fn source_map_v2(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.source_map_v2
    }
    pub const fn neutral_kir(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.neutral_kir
    }
    pub const fn target_kir(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.target_kir
    }
    pub const fn artifact(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.artifact
    }
    pub const fn catalog(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.catalog
    }
    pub const fn structural_bridge(self) -> SourceIsaCharacteristicContentIdentityV1 {
        self.structural_bridge
    }
    pub const fn correlation_identity(self) -> [u8; 32] {
        self.correlation_identity
    }
    pub const fn semantic_map_identity(self) -> [u8; 32] {
        self.semantic_map_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SourceIsaCharacteristicCategoryV1 {
    TargetKirGlobalStore = 1,
    TargetKirWorkgroupLdsRead = 2,
    TargetKirWorkgroupLdsWrite = 3,
    TargetKirWorkgroupBarrier = 4,
    TargetKirBf16MfmaExact = 5,
}

impl SourceIsaCharacteristicCategoryV1 {
    pub const fn code(self) -> u16 {
        self as u16
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceIsaCharacteristicMemoryFormV1 {
    Plain = 1,
    Guarded = 2,
    MatrixTile = 3,
}

impl SourceIsaCharacteristicMemoryFormV1 {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceIsaCharacteristicKindV1 {
    GlobalStore {
        form: SourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupLoad {
        form: SourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupStore {
        form: SourceIsaCharacteristicMemoryFormV1,
    },
    WorkgroupBarrier,
    Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
}

impl SourceIsaCharacteristicKindV1 {
    pub const fn code(self) -> u16 {
        match self {
            Self::GlobalStore {
                form: SourceIsaCharacteristicMemoryFormV1::Plain,
            } => 1,
            Self::GlobalStore {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded,
            } => 2,
            Self::GlobalStore {
                form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
            } => 3,
            Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::Plain,
            } => 4,
            Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded,
            } => 5,
            Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
            } => 6,
            Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::Plain,
            } => 7,
            Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded,
            } => 8,
            Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
            } => 9,
            Self::WorkgroupBarrier => 10,
            Self::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => 11,
        }
    }

    pub const fn category(self) -> SourceIsaCharacteristicCategoryV1 {
        match self {
            Self::GlobalStore { .. } => SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore,
            Self::WorkgroupLoad { .. } => {
                SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsRead
            }
            Self::WorkgroupStore { .. } => {
                SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsWrite
            }
            Self::WorkgroupBarrier => SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupBarrier,
            Self::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
                SourceIsaCharacteristicCategoryV1::TargetKirBf16MfmaExact
            }
        }
    }

    fn decode(code: u16) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        let form = |form| Ok(Self::GlobalStore { form });
        match code {
            1 => form(SourceIsaCharacteristicMemoryFormV1::Plain),
            2 => form(SourceIsaCharacteristicMemoryFormV1::Guarded),
            3 => form(SourceIsaCharacteristicMemoryFormV1::MatrixTile),
            4 => Ok(Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::Plain,
            }),
            5 => Ok(Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded,
            }),
            6 => Ok(Self::WorkgroupLoad {
                form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
            }),
            7 => Ok(Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::Plain,
            }),
            8 => Ok(Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded,
            }),
            9 => Ok(Self::WorkgroupStore {
                form: SourceIsaCharacteristicMemoryFormV1::MatrixTile,
            }),
            10 => Ok(Self::WorkgroupBarrier),
            11 => Ok(Self::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate),
            _ => Err(SourceIsaCharacteristicErrorV1::InvalidTag),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SourceIsaCharacteristicMissingReasonV1 {
    NoQualifyingTargetOperation = 1,
    NoAdmittedCorrelation = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SourceIsaCharacteristicUnavailableReasonV1 {
    CatalogUnavailable = 1,
    StructuralBridgeUnavailable = 2,
    ClassifierUnavailable = 3,
    SourceProjectionUnavailable = 4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SourceIsaCharacteristicObservationErrorV1 {
    InvalidCatalog = 1,
    InvalidStructuralBridge = 2,
    InvalidClassification = 3,
    ConflictingEvidence = 4,
    ResourceLimit = 5,
    AllocationFailure = 6,
}

macro_rules! tagged_reason {
    ($ty:ty, $($code:literal => $variant:path),+ $(,)?) => {
        impl $ty {
            pub const fn code(self) -> u16 { self as u16 }
            fn decode(code: u16) -> Result<Self, SourceIsaCharacteristicErrorV1> {
                match code { $($code => Ok($variant),)+ _ => Err(SourceIsaCharacteristicErrorV1::InvalidTag) }
            }
        }
    };
}

tagged_reason!(SourceIsaCharacteristicMissingReasonV1,
    1 => SourceIsaCharacteristicMissingReasonV1::NoQualifyingTargetOperation,
    2 => SourceIsaCharacteristicMissingReasonV1::NoAdmittedCorrelation);
tagged_reason!(SourceIsaCharacteristicUnavailableReasonV1,
    1 => SourceIsaCharacteristicUnavailableReasonV1::CatalogUnavailable,
    2 => SourceIsaCharacteristicUnavailableReasonV1::StructuralBridgeUnavailable,
    3 => SourceIsaCharacteristicUnavailableReasonV1::ClassifierUnavailable,
    4 => SourceIsaCharacteristicUnavailableReasonV1::SourceProjectionUnavailable);
tagged_reason!(SourceIsaCharacteristicObservationErrorV1,
    1 => SourceIsaCharacteristicObservationErrorV1::InvalidCatalog,
    2 => SourceIsaCharacteristicObservationErrorV1::InvalidStructuralBridge,
    3 => SourceIsaCharacteristicObservationErrorV1::InvalidClassification,
    4 => SourceIsaCharacteristicObservationErrorV1::ConflictingEvidence,
    5 => SourceIsaCharacteristicObservationErrorV1::ResourceLimit,
    6 => SourceIsaCharacteristicObservationErrorV1::AllocationFailure);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceIsaCharacteristicScanStateV1 {
    Complete,
    Missing(SourceIsaCharacteristicMissingReasonV1),
    Unavailable(SourceIsaCharacteristicUnavailableReasonV1),
    Error(SourceIsaCharacteristicObservationErrorV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicScanSummaryV1 {
    catalog_record_count: u64,
    catalog_records_scanned: u64,
    target_operation_count: u64,
    target_operations_scanned: u64,
    classified_target_count: u64,
    retained_target_correlation_count: u64,
    pre_kir_elimination_count: u64,
    correlation_count: u64,
    state: SourceIsaCharacteristicScanStateV1,
}

impl SourceIsaCharacteristicScanSummaryV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        catalog_record_count: u64,
        catalog_records_scanned: u64,
        target_operation_count: u64,
        target_operations_scanned: u64,
        classified_target_count: u64,
        retained_target_correlation_count: u64,
        pre_kir_elimination_count: u64,
        correlation_count: u64,
        state: SourceIsaCharacteristicScanStateV1,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if !usize::try_from(catalog_record_count)
            .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1)
            || !usize::try_from(catalog_records_scanned)
                .is_ok_and(|count| count <= MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1)
        {
            return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
        }
        let retained = retained_target_correlation_count
            .checked_add(pre_kir_elimination_count)
            .ok_or(SourceIsaCharacteristicErrorV1::InvalidClaim)?;
        if catalog_records_scanned > catalog_record_count
            || target_operations_scanned > target_operation_count
            || correlation_count != retained
            || correlation_count > catalog_records_scanned
            || matches!(state, SourceIsaCharacteristicScanStateV1::Complete)
                && (catalog_records_scanned != catalog_record_count
                    || target_operations_scanned != target_operation_count)
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            catalog_record_count,
            catalog_records_scanned,
            target_operation_count,
            target_operations_scanned,
            classified_target_count,
            retained_target_correlation_count,
            pre_kir_elimination_count,
            correlation_count,
            state,
        })
    }
    pub const fn catalog_record_count(self) -> u64 {
        self.catalog_record_count
    }
    pub const fn catalog_records_scanned(self) -> u64 {
        self.catalog_records_scanned
    }
    pub const fn target_operation_count(self) -> u64 {
        self.target_operation_count
    }
    pub const fn target_operations_scanned(self) -> u64 {
        self.target_operations_scanned
    }
    pub const fn classified_target_count(self) -> u64 {
        self.classified_target_count
    }
    pub const fn retained_target_correlation_count(self) -> u64 {
        self.retained_target_correlation_count
    }
    pub const fn pre_kir_elimination_count(self) -> u64 {
        self.pre_kir_elimination_count
    }
    pub const fn correlation_count(self) -> u64 {
        self.correlation_count
    }
    pub const fn state(self) -> SourceIsaCharacteristicScanStateV1 {
        self.state
    }
    pub const fn is_complete(self) -> bool {
        matches!(self.state, SourceIsaCharacteristicScanStateV1::Complete)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicSourceSpanV1 {
    file_identity: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
}

impl SourceIsaCharacteristicSourceSpanV1 {
    pub fn new(
        file_identity: [u8; 32],
        byte_start: u64,
        byte_end: u64,
        line: u32,
        column: u32,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if file_identity == [0; 32] || byte_start > byte_end || line == 0 || column == 0 {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            file_identity,
            byte_start,
            byte_end,
            line,
            column,
        })
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
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicSourceCoordinateV1 {
    node_identity: [u8; 32],
    span: SourceIsaCharacteristicSourceSpanV1,
}

impl SourceIsaCharacteristicSourceCoordinateV1 {
    pub fn new(
        node_identity: [u8; 32],
        span: SourceIsaCharacteristicSourceSpanV1,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if node_identity == [0; 32] {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            node_identity,
            span,
        })
    }
    pub const fn node_identity(self) -> [u8; 32] {
        self.node_identity
    }
    pub const fn span(self) -> SourceIsaCharacteristicSourceSpanV1 {
        self.span
    }
}

macro_rules! ordinal_coordinate {
    ($name:ident, $first:ident, $second:ident, $third:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            $first: u64,
            $second: u64,
            $third: u64,
        }
        impl $name {
            pub fn new(
                $first: u64,
                $second: u64,
                $third: u64,
            ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
                if [$first, $second, $third]
                    .into_iter()
                    .any(|value| value > u64::from(u32::MAX))
                {
                    return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
                }
                Ok(Self {
                    $first,
                    $second,
                    $third,
                })
            }
            pub const fn $first(self) -> u64 {
                self.$first
            }
            pub const fn $second(self) -> u64 {
                self.$second
            }
            pub const fn $third(self) -> u64 {
                self.$third
            }
        }
    };
}

ordinal_coordinate!(
    SourceIsaCharacteristicMirCoordinateV1,
    body_ordinal,
    block_ordinal,
    statement_ordinal
);
ordinal_coordinate!(
    SourceIsaCharacteristicKirCoordinateV1,
    function_ordinal,
    block_ordinal,
    operation_ordinal
);
ordinal_coordinate!(
    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    function_ordinal,
    block_ordinal,
    instruction_ordinal
);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicIsaIntervalV1 {
    kernel_ordinal: u64,
    symbol_relative_start: u64,
    symbol_relative_end: u64,
}

impl SourceIsaCharacteristicIsaIntervalV1 {
    pub fn new(
        kernel_ordinal: u64,
        symbol_relative_start: u64,
        symbol_relative_end: u64,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if kernel_ordinal != 0
            || !symbol_relative_start.is_multiple_of(4)
            || symbol_relative_start.checked_add(4) != Some(symbol_relative_end)
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            kernel_ordinal,
            symbol_relative_start,
            symbol_relative_end,
        })
    }
    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }
    pub const fn symbol_relative_start(self) -> u64 {
        self.symbol_relative_start
    }
    pub const fn symbol_relative_end(self) -> u64 {
        self.symbol_relative_end
    }
    pub const fn contains(self, kernel_ordinal: u64, pc: u64) -> bool {
        self.kernel_ordinal == kernel_ordinal
            && self.symbol_relative_start <= pc
            && pc < self.symbol_relative_end
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceIsaCharacteristicRecordKindV1 {
    EliminatedBeforeKir = 1,
    SourceAnchored = 2,
    NoSourceProvenance = 3,
}

impl SourceIsaCharacteristicRecordKindV1 {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceIsaCharacteristicTransformationV1 {
    Preserved = 1,
    Duplicated = 2,
    Coalesced = 3,
    DuplicatedAndCoalesced = 4,
    Eliminated = 5,
}

impl SourceIsaCharacteristicTransformationV1 {
    pub const fn code(self) -> u8 {
        self as u8
    }
    fn decode(code: u8) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        match code {
            1 => Ok(Self::Preserved),
            2 => Ok(Self::Duplicated),
            3 => Ok(Self::Coalesced),
            4 => Ok(Self::DuplicatedAndCoalesced),
            5 => Ok(Self::Eliminated),
            _ => Err(SourceIsaCharacteristicErrorV1::InvalidTag),
        }
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicTargetCorrelationV1 {
    catalog_record_ordinal: u64,
    kind: SourceIsaCharacteristicRecordKindV1,
    source: Option<SourceIsaCharacteristicSourceCoordinateV1>,
    mir_node_identity: Option<[u8; 32]>,
    mir: Option<SourceIsaCharacteristicMirCoordinateV1>,
    neutral_kir_node_identity: Option<[u8; 32]>,
    neutral_kir: Option<SourceIsaCharacteristicKirCoordinateV1>,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    semantic_operation_identity: [u8; 32],
    compiler_handoff_llvm: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    isa_intervals: Vec<SourceIsaCharacteristicIsaIntervalV1>,
    transformation: SourceIsaCharacteristicTransformationV1,
}

impl SourceIsaCharacteristicTargetCorrelationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        catalog_record_ordinal: u64,
        kind: SourceIsaCharacteristicRecordKindV1,
        source: Option<SourceIsaCharacteristicSourceCoordinateV1>,
        mir_node_identity: Option<[u8; 32]>,
        mir: Option<SourceIsaCharacteristicMirCoordinateV1>,
        neutral_kir_node_identity: Option<[u8; 32]>,
        neutral_kir: Option<SourceIsaCharacteristicKirCoordinateV1>,
        target_kir: SourceIsaCharacteristicKirCoordinateV1,
        semantic_operation_identity: [u8; 32],
        compiler_handoff_llvm: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
        mut isa_intervals: Vec<SourceIsaCharacteristicIsaIntervalV1>,
        transformation: SourceIsaCharacteristicTransformationV1,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if semantic_operation_identity == [0; 32]
            || mir_node_identity.is_some_and(|identity| identity == [0; 32])
            || neutral_kir_node_identity.is_some_and(|identity| identity == [0; 32])
            || isa_intervals.len() > MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1
            || matches!(
                transformation,
                SourceIsaCharacteristicTransformationV1::Eliminated
            ) != isa_intervals.is_empty()
            || match kind {
                SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => true,
                SourceIsaCharacteristicRecordKindV1::SourceAnchored => {
                    source.is_none()
                        || source
                            .is_some_and(|source| source.span.byte_start == source.span.byte_end)
                        || mir_node_identity.is_none()
                        || mir.is_none()
                        || neutral_kir_node_identity.is_none()
                        || neutral_kir.is_none()
                }
                SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => {
                    source.is_some()
                        || mir_node_identity.is_some()
                        || mir.is_some()
                        || neutral_kir_node_identity.is_some()
                        || neutral_kir.is_some()
                }
            }
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        isa_intervals.sort_unstable();
        if isa_intervals.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            catalog_record_ordinal,
            kind,
            source,
            mir_node_identity,
            mir,
            neutral_kir_node_identity,
            neutral_kir,
            target_kir,
            semantic_operation_identity,
            compiler_handoff_llvm,
            isa_intervals,
            transformation,
        })
    }
    pub const fn catalog_record_ordinal(&self) -> u64 {
        self.catalog_record_ordinal
    }
    pub const fn kind(&self) -> SourceIsaCharacteristicRecordKindV1 {
        self.kind
    }
    pub const fn source(&self) -> Option<SourceIsaCharacteristicSourceCoordinateV1> {
        self.source
    }
    pub const fn mir_node_identity(&self) -> Option<[u8; 32]> {
        self.mir_node_identity
    }
    pub const fn mir(&self) -> Option<SourceIsaCharacteristicMirCoordinateV1> {
        self.mir
    }
    pub const fn neutral_kir_node_identity(&self) -> Option<[u8; 32]> {
        self.neutral_kir_node_identity
    }
    pub const fn neutral_kir(&self) -> Option<SourceIsaCharacteristicKirCoordinateV1> {
        self.neutral_kir
    }
    pub const fn target_kir(&self) -> SourceIsaCharacteristicKirCoordinateV1 {
        self.target_kir
    }
    pub const fn semantic_operation_identity(&self) -> [u8; 32] {
        self.semantic_operation_identity
    }
    pub const fn compiler_handoff_llvm(
        &self,
    ) -> SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1 {
        self.compiler_handoff_llvm
    }
    pub fn isa_intervals(&self) -> &[SourceIsaCharacteristicIsaIntervalV1] {
        &self.isa_intervals
    }
    pub const fn transformation(&self) -> SourceIsaCharacteristicTransformationV1 {
        self.transformation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicPreKirEliminationV1 {
    catalog_record_ordinal: u64,
    source: SourceIsaCharacteristicSourceCoordinateV1,
    mir_node_identity: [u8; 32],
    mir: SourceIsaCharacteristicMirCoordinateV1,
}

impl SourceIsaCharacteristicPreKirEliminationV1 {
    pub fn new(
        catalog_record_ordinal: u64,
        source: SourceIsaCharacteristicSourceCoordinateV1,
        mir_node_identity: [u8; 32],
        mir: SourceIsaCharacteristicMirCoordinateV1,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if mir_node_identity == [0; 32] {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            catalog_record_ordinal,
            source,
            mir_node_identity,
            mir,
        })
    }
    pub const fn catalog_record_ordinal(self) -> u64 {
        self.catalog_record_ordinal
    }
    pub const fn source(self) -> SourceIsaCharacteristicSourceCoordinateV1 {
        self.source
    }
    pub const fn mir_node_identity(self) -> [u8; 32] {
        self.mir_node_identity
    }
    pub const fn mir(self) -> SourceIsaCharacteristicMirCoordinateV1 {
        self.mir
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceIsaCharacteristicTargetV1 {
    identity: [u8; 32],
    kind: SourceIsaCharacteristicKindV1,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    correlations: Vec<SourceIsaCharacteristicTargetCorrelationV1>,
}

impl SourceIsaCharacteristicTargetV1 {
    pub fn new(
        kind: SourceIsaCharacteristicKindV1,
        target_kir: SourceIsaCharacteristicKirCoordinateV1,
        mut correlations: Vec<SourceIsaCharacteristicTargetCorrelationV1>,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if correlations.len() > MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1
            || correlations
                .iter()
                .any(|correlation| correlation.target_kir != target_kir)
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        correlations.sort_unstable_by_key(|value| value.catalog_record_ordinal);
        if correlations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        let identity = target_identity(kind, target_kir, &correlations)?;
        Ok(Self {
            identity,
            kind,
            target_kir,
            correlations,
        })
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn category(&self) -> SourceIsaCharacteristicCategoryV1 {
        self.kind.category()
    }
    pub const fn kind(&self) -> SourceIsaCharacteristicKindV1 {
        self.kind
    }
    pub const fn target_kir(&self) -> SourceIsaCharacteristicKirCoordinateV1 {
        self.target_kir
    }
    pub fn correlations(&self) -> &[SourceIsaCharacteristicTargetCorrelationV1] {
        &self.correlations
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicCollectionV1 {
    binding: SourceIsaCharacteristicBindingV1,
    scan: SourceIsaCharacteristicScanSummaryV1,
    targets: Vec<SourceIsaCharacteristicTargetV1>,
    pre_kir_eliminations: Vec<SourceIsaCharacteristicPreKirEliminationV1>,
    identity: [u8; 32],
}

impl SourceIsaCharacteristicCollectionV1 {
    pub fn new(
        binding: SourceIsaCharacteristicBindingV1,
        scan: SourceIsaCharacteristicScanSummaryV1,
        mut targets: Vec<SourceIsaCharacteristicTargetV1>,
        mut pre_kir_eliminations: Vec<SourceIsaCharacteristicPreKirEliminationV1>,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        targets.sort_unstable_by_key(|target| target.target_kir);
        pre_kir_eliminations.sort_unstable();
        validate_collection_shape(binding, scan, &targets, &pre_kir_eliminations)?;
        let mut collection = Self {
            binding,
            scan,
            targets,
            pre_kir_eliminations,
            identity: [0; 32],
        };
        let prefix = collection.encode_prefix()?;
        collection.identity = collection_identity(&prefix);
        Ok(collection)
    }
    pub const fn binding(&self) -> SourceIsaCharacteristicBindingV1 {
        self.binding
    }
    pub const fn scan(&self) -> SourceIsaCharacteristicScanSummaryV1 {
        self.scan
    }
    pub fn targets(&self) -> &[SourceIsaCharacteristicTargetV1] {
        &self.targets
    }
    pub fn pre_kir_eliminations(&self) -> &[SourceIsaCharacteristicPreKirEliminationV1] {
        &self.pre_kir_eliminations
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn canonical_byte_len(&self) -> Result<u64, SourceIsaCharacteristicErrorV1> {
        let payload = payload_length(&self.targets, self.pre_kir_eliminations.len())?;
        let bytes = SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1
            .checked_add(payload)
            .and_then(|value| value.checked_add(SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1))
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        u64::try_from(bytes).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
    pub const fn grants_hardware_observation_authority(&self) -> bool {
        false
    }
    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }
    pub const fn proves_a_schedule(&self) -> bool {
        false
    }
    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }
    pub const fn proves_final_llvm_classification(&self) -> bool {
        false
    }
    pub const fn proves_final_isa_opcode_classification(&self) -> bool {
        false
    }
    pub const fn contains_decoded_isa(&self) -> bool {
        false
    }
    pub fn has_sparse_final_hsaco_anchors(&self) -> bool {
        self.targets.iter().any(|target| {
            target
                .correlations
                .iter()
                .any(|correlation| !correlation.isa_intervals.is_empty())
        })
    }
    pub const fn schema_supports_sparse_final_hsaco_anchors(&self) -> bool {
        true
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, SourceIsaCharacteristicErrorV1> {
        let mut encoded = self.encode_prefix()?;
        encoded
            .try_reserve_exact(SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        encoded.extend_from_slice(&self.identity);
        Ok(encoded)
    }

    fn encode_prefix(&self) -> Result<Vec<u8>, SourceIsaCharacteristicErrorV1> {
        validate_collection_shape(
            self.binding,
            self.scan,
            &self.targets,
            &self.pre_kir_eliminations,
        )?;
        let target_correlations = target_correlation_count(&self.targets)?;
        let payload_len = payload_length(&self.targets, self.pre_kir_eliminations.len())?;
        let total = SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1
            .checked_add(payload_len)
            .and_then(|value| value.checked_add(SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1))
            .filter(|value| *value <= MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1)
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(total - SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        encoded.extend_from_slice(SOURCE_ISA_CHARACTERISTIC_MAGIC_V1);
        push_u16(&mut encoded, SOURCE_ISA_CHARACTERISTIC_VERSION_V1);
        push_u16(
            &mut encoded,
            u16::try_from(SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1)
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        );
        push_u32(
            &mut encoded,
            u32::try_from(total).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        );
        push_u32(
            &mut encoded,
            u32::try_from(self.targets.len())
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        );
        push_u32(
            &mut encoded,
            u32::try_from(target_correlations)
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        );
        push_u32(
            &mut encoded,
            u32::try_from(self.pre_kir_eliminations.len())
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        );
        let (scan_tag, scan_reason) = match self.scan.state {
            SourceIsaCharacteristicScanStateV1::Complete => (1, 0),
            SourceIsaCharacteristicScanStateV1::Missing(reason) => (2, reason.code()),
            SourceIsaCharacteristicScanStateV1::Unavailable(reason) => (3, reason.code()),
            SourceIsaCharacteristicScanStateV1::Error(error) => (4, error.code()),
        };
        push_u8(&mut encoded, scan_tag);
        push_u8(&mut encoded, 0);
        push_u16(&mut encoded, scan_reason);
        for count in [
            self.scan.catalog_record_count,
            self.scan.catalog_records_scanned,
            self.scan.target_operation_count,
            self.scan.target_operations_scanned,
            self.scan.classified_target_count,
            self.scan.retained_target_correlation_count,
            self.scan.pre_kir_elimination_count,
            self.scan.correlation_count,
        ] {
            push_u64(&mut encoded, count);
        }
        push_u8(&mut encoded, self.binding.target_profile.code());
        push_u8(&mut encoded, self.binding.kir_version.code());
        encoded.extend_from_slice(&[0; 6]);
        encoded.extend_from_slice(&self.binding.structural_identity);
        for count in [
            self.binding.structural_counts.functions,
            self.binding.structural_counts.defined_bodies,
            self.binding.structural_counts.blocks,
            self.binding.structural_counts.operations,
        ] {
            push_u64(&mut encoded, count);
        }
        for content in [
            self.binding.source_map_v2,
            self.binding.neutral_kir,
            self.binding.target_kir,
            self.binding.artifact,
            self.binding.catalog,
            self.binding.structural_bridge,
        ] {
            encode_content(&mut encoded, content);
        }
        encoded.extend_from_slice(&self.binding.correlation_identity);
        encoded.extend_from_slice(&self.binding.semantic_map_identity);
        encoded.extend_from_slice(&[0; 40]);
        if encoded.len() != SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1 {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        for target in &self.targets {
            encode_target(&mut encoded, target)?;
        }
        for fact in &self.pre_kir_eliminations {
            encode_pre_kir_fact(&mut encoded, *fact);
        }
        if encoded.len() + SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1 != total {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        Ok(encoded)
    }
}

fn decode_target(
    decoder: &mut DecoderV1<'_>,
) -> Result<SourceIsaCharacteristicTargetV1, SourceIsaCharacteristicErrorV1> {
    let identity = decoder.identity()?;
    let kind = SourceIsaCharacteristicKindV1::decode(decoder.u16()?)?;
    let tag = decoder.u8()?;
    decoder.zeros(1)?;
    let reason = decoder.u16()?;
    decoder.zeros(2)?;
    let correlation_count = usize::try_from(decoder.u32()?)
        .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    if correlation_count > MAX_SOURCE_ISA_CHARACTERISTIC_CORRELATIONS_PER_TARGET_V1 {
        return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
    }
    let target_kir = decoder.kir()?;
    decoder.zeros(12)?;
    if tag != 1 || reason != 0 {
        return Err(SourceIsaCharacteristicErrorV1::InvalidTag);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(correlation_count)
        .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
    for _ in 0..correlation_count {
        values.push(decode_target_correlation(decoder)?);
    }
    if values.iter().any(|value| value.target_kir != target_kir)
        || values
            .windows(2)
            .any(|pair| pair[0].catalog_record_ordinal >= pair[1].catalog_record_ordinal)
    {
        return Err(SourceIsaCharacteristicErrorV1::NonCanonical);
    }
    let target = SourceIsaCharacteristicTargetV1::new(kind, target_kir, values)?;
    if target.identity != identity {
        return Err(SourceIsaCharacteristicErrorV1::IdentityMismatch);
    }
    Ok(target)
}

fn decode_target_correlation(
    decoder: &mut DecoderV1<'_>,
) -> Result<SourceIsaCharacteristicTargetCorrelationV1, SourceIsaCharacteristicErrorV1> {
    let catalog_record_ordinal = decoder.u64()?;
    let kind = match decoder.u8()? {
        2 => SourceIsaCharacteristicRecordKindV1::SourceAnchored,
        3 => SourceIsaCharacteristicRecordKindV1::NoSourceProvenance,
        _ => return Err(SourceIsaCharacteristicErrorV1::InvalidTag),
    };
    let transformation = SourceIsaCharacteristicTransformationV1::decode(decoder.u8()?)?;
    let flags = decoder.u16()?;
    decoder.zeros(4)?;
    let expected_flags = match kind {
        SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => {
            return Err(SourceIsaCharacteristicErrorV1::InvalidTag);
        }
        SourceIsaCharacteristicRecordKindV1::SourceAnchored => 0x3f,
        SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => 0x38,
    };
    if flags != expected_flags {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    let source = decode_optional_source(decoder, flags & 1 != 0)?;
    let (mir_node_identity, mir) = decode_optional_node_mir(decoder, flags & 2 != 0)?;
    let (neutral_kir_node_identity, neutral_kir) =
        decode_optional_node_kir(decoder, flags & 4 != 0)?;
    let target_kir = decoder.kir()?;
    let semantic_operation_identity = decoder.identity()?;
    let compiler_handoff_llvm = decoder.llvm()?;
    let interval_count = usize::try_from(decoder.u32()?)
        .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    decoder.zeros(12)?;
    if interval_count > MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1 {
        return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
    }
    let mut intervals = Vec::new();
    intervals
        .try_reserve_exact(interval_count)
        .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
    for _ in 0..interval_count {
        intervals.push(SourceIsaCharacteristicIsaIntervalV1::new(
            decoder.u64()?,
            decoder.u64()?,
            decoder.u64()?,
        )?);
    }
    if intervals.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(SourceIsaCharacteristicErrorV1::NonCanonical);
    }
    SourceIsaCharacteristicTargetCorrelationV1::new(
        catalog_record_ordinal,
        kind,
        source,
        mir_node_identity,
        mir,
        neutral_kir_node_identity,
        neutral_kir,
        target_kir,
        semantic_operation_identity,
        compiler_handoff_llvm,
        intervals,
        transformation,
    )
}

fn decode_pre_kir_fact(
    decoder: &mut DecoderV1<'_>,
) -> Result<SourceIsaCharacteristicPreKirEliminationV1, SourceIsaCharacteristicErrorV1> {
    let ordinal = decoder.u64()?;
    let source = decoder.source()?;
    let mir_node = decoder.identity()?;
    let mir = decoder.mir()?;
    decoder.zeros(32)?;
    SourceIsaCharacteristicPreKirEliminationV1::new(ordinal, source, mir_node, mir)
}

fn decode_optional_source(
    decoder: &mut DecoderV1<'_>,
    present: bool,
) -> Result<Option<SourceIsaCharacteristicSourceCoordinateV1>, SourceIsaCharacteristicErrorV1> {
    if present {
        Ok(Some(decoder.source()?))
    } else {
        decoder.zeros(88)?;
        Ok(None)
    }
}
fn decode_optional_node_mir(
    decoder: &mut DecoderV1<'_>,
    present: bool,
) -> Result<
    (
        Option<[u8; 32]>,
        Option<SourceIsaCharacteristicMirCoordinateV1>,
    ),
    SourceIsaCharacteristicErrorV1,
> {
    if present {
        Ok((Some(decoder.identity()?), Some(decoder.mir()?)))
    } else {
        decoder.zeros(56)?;
        Ok((None, None))
    }
}
fn decode_optional_node_kir(
    decoder: &mut DecoderV1<'_>,
    present: bool,
) -> Result<
    (
        Option<[u8; 32]>,
        Option<SourceIsaCharacteristicKirCoordinateV1>,
    ),
    SourceIsaCharacteristicErrorV1,
> {
    if present {
        Ok((Some(decoder.identity()?), Some(decoder.kir()?)))
    } else {
        decoder.zeros(56)?;
        Ok((None, None))
    }
}

fn collection_identity(prefix: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COLLECTION_IDENTITY_DOMAIN_V1);
    digest.update(prefix);
    digest.finalize().into()
}

fn target_identity(
    kind: SourceIsaCharacteristicKindV1,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    correlations: &[SourceIsaCharacteristicTargetCorrelationV1],
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    let mut digest = Sha256::new();
    digest.update(TARGET_IDENTITY_DOMAIN_V1);
    digest.update(kind.code().to_le_bytes());
    hash_kir(&mut digest, target_kir);
    digest.update(
        u64::try_from(correlations.len())
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    for value in correlations {
        digest.update(value.catalog_record_ordinal.to_le_bytes());
        digest.update([value.kind as u8]);
        match value.source {
            Some(source) => {
                digest.update([1]);
                digest.update(source.node_identity);
                hash_span(&mut digest, source.span);
            }
            None => digest.update([0]),
        }
        hash_optional_identity(&mut digest, value.mir_node_identity);
        match value.mir {
            Some(coordinate) => {
                digest.update([1]);
                hash_mir(&mut digest, coordinate);
            }
            None => digest.update([0]),
        }
        hash_optional_identity(&mut digest, value.neutral_kir_node_identity);
        match value.neutral_kir {
            Some(coordinate) => {
                digest.update([1]);
                hash_kir(&mut digest, coordinate);
            }
            None => digest.update([0]),
        }
        hash_kir(&mut digest, value.target_kir);
        digest.update(value.semantic_operation_identity);
        hash_llvm(&mut digest, value.compiler_handoff_llvm);
        digest.update([value.transformation as u8]);
        digest.update(
            u64::try_from(value.isa_intervals.len())
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?
                .to_le_bytes(),
        );
        for interval in &value.isa_intervals {
            digest.update(interval.kernel_ordinal.to_le_bytes());
            digest.update(interval.symbol_relative_start.to_le_bytes());
            digest.update(interval.symbol_relative_end.to_le_bytes());
        }
    }
    Ok(digest.finalize().into())
}

fn hash_optional_identity(digest: &mut Sha256, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update(value);
        }
        None => digest.update([0]),
    }
}
fn hash_span(digest: &mut Sha256, value: SourceIsaCharacteristicSourceSpanV1) {
    digest.update(value.file_identity);
    digest.update(value.byte_start.to_le_bytes());
    digest.update(value.byte_end.to_le_bytes());
    digest.update(value.line.to_le_bytes());
    digest.update(value.column.to_le_bytes());
}
fn hash_mir(digest: &mut Sha256, value: SourceIsaCharacteristicMirCoordinateV1) {
    digest.update(value.body_ordinal.to_le_bytes());
    digest.update(value.block_ordinal.to_le_bytes());
    digest.update(value.statement_ordinal.to_le_bytes());
}
fn hash_kir(digest: &mut Sha256, value: SourceIsaCharacteristicKirCoordinateV1) {
    digest.update(value.function_ordinal.to_le_bytes());
    digest.update(value.block_ordinal.to_le_bytes());
    digest.update(value.operation_ordinal.to_le_bytes());
}
fn hash_llvm(digest: &mut Sha256, value: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1) {
    digest.update(value.function_ordinal.to_le_bytes());
    digest.update(value.block_ordinal.to_le_bytes());
    digest.update(value.instruction_ordinal.to_le_bytes());
}
fn push_u8(output: &mut Vec<u8>, value: u8) {
    output.push(value);
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

struct DecoderV1<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    const fn position(&self) -> usize {
        self.position
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], SourceIsaCharacteristicErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(SourceIsaCharacteristicErrorV1::Malformed)?;
        let value = &self.bytes[self.position..end];
        self.position = end;
        Ok(value)
    }
    fn expect(&mut self, value: &[u8]) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if self.take(value.len())? == value {
            Ok(())
        } else {
            Err(SourceIsaCharacteristicErrorV1::Malformed)
        }
    }
    fn zeros(&mut self, count: usize) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if self.take(count)?.iter().all(|byte| *byte == 0) {
            Ok(())
        } else {
            Err(SourceIsaCharacteristicErrorV1::NonCanonical)
        }
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], SourceIsaCharacteristicErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)
    }
    fn u8(&mut self) -> Result<u8, SourceIsaCharacteristicErrorV1> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SourceIsaCharacteristicErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, SourceIsaCharacteristicErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, SourceIsaCharacteristicErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn identity(&mut self) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
        let value = self.array()?;
        if value == [0; 32] {
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        } else {
            Ok(value)
        }
    }
    fn content(
        &mut self,
    ) -> Result<SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicErrorV1> {
        SourceIsaCharacteristicContentIdentityV1::new(self.identity()?, self.u64()?)
    }
    fn span(
        &mut self,
    ) -> Result<SourceIsaCharacteristicSourceSpanV1, SourceIsaCharacteristicErrorV1> {
        SourceIsaCharacteristicSourceSpanV1::new(
            self.identity()?,
            self.u64()?,
            self.u64()?,
            self.u32()?,
            self.u32()?,
        )
    }
    fn source(
        &mut self,
    ) -> Result<SourceIsaCharacteristicSourceCoordinateV1, SourceIsaCharacteristicErrorV1> {
        SourceIsaCharacteristicSourceCoordinateV1::new(self.identity()?, self.span()?)
    }
    fn mir(
        &mut self,
    ) -> Result<SourceIsaCharacteristicMirCoordinateV1, SourceIsaCharacteristicErrorV1> {
        SourceIsaCharacteristicMirCoordinateV1::new(self.u64()?, self.u64()?, self.u64()?)
    }
    fn kir(
        &mut self,
    ) -> Result<SourceIsaCharacteristicKirCoordinateV1, SourceIsaCharacteristicErrorV1> {
        SourceIsaCharacteristicKirCoordinateV1::new(self.u64()?, self.u64()?, self.u64()?)
    }
    fn llvm(
        &mut self,
    ) -> Result<
        SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
        SourceIsaCharacteristicErrorV1,
    > {
        SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(
            self.u64()?,
            self.u64()?,
            self.u64()?,
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceIsaCharacteristicQueryV1 {
    All,
    CharacteristicIdentity([u8; 32]),
    Category(SourceIsaCharacteristicCategoryV1),
    Kind(SourceIsaCharacteristicKindV1),
    SourceNode([u8; 32]),
    SourceSpan(SourceIsaCharacteristicSourceSpanV1),
    RecordKind(SourceIsaCharacteristicRecordKindV1),
    MirNode([u8; 32]),
    Mir(SourceIsaCharacteristicMirCoordinateV1),
    NeutralKirNode([u8; 32]),
    NeutralKir(SourceIsaCharacteristicKirCoordinateV1),
    TargetKir(SourceIsaCharacteristicKirCoordinateV1),
    SemanticOperation([u8; 32]),
    CompilerHandoffLlvm(SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1),
    Transformation(SourceIsaCharacteristicTransformationV1),
    ExactPc {
        kernel_ordinal: u64,
        symbol_relative_pc: u64,
    },
    PreKirOnly,
}

impl SourceIsaCharacteristicQueryV1 {
    fn validate(&self) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if matches!(self, Self::CharacteristicIdentity(identity) | Self::SourceNode(identity)
            | Self::MirNode(identity) | Self::NeutralKirNode(identity)
            | Self::SemanticOperation(identity) if *identity == [0; 32])
            || matches!(self, Self::ExactPc { kernel_ordinal, symbol_relative_pc }
                if *kernel_ordinal != 0 || *symbol_relative_pc % 4 != 0)
        {
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        } else {
            Ok(())
        }
    }
    pub fn identity(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(QUERY_IDENTITY_DOMAIN_V1);
        match self {
            Self::All => digest.update([1]),
            Self::CharacteristicIdentity(value) => {
                digest.update([2]);
                digest.update(value);
            }
            Self::Category(value) => {
                digest.update([3]);
                digest.update(value.code().to_le_bytes());
            }
            Self::Kind(value) => {
                digest.update([17]);
                digest.update(value.code().to_le_bytes());
            }
            Self::SourceNode(value) => {
                digest.update([4]);
                digest.update(value);
            }
            Self::SourceSpan(value) => {
                digest.update([5]);
                hash_span(&mut digest, *value);
            }
            Self::RecordKind(value) => {
                digest.update([6]);
                digest.update([*value as u8]);
            }
            Self::MirNode(value) => {
                digest.update([7]);
                digest.update(value);
            }
            Self::Mir(value) => {
                digest.update([8]);
                hash_mir(&mut digest, *value);
            }
            Self::NeutralKirNode(value) => {
                digest.update([9]);
                digest.update(value);
            }
            Self::NeutralKir(value) => {
                digest.update([10]);
                hash_kir(&mut digest, *value);
            }
            Self::TargetKir(value) => {
                digest.update([11]);
                hash_kir(&mut digest, *value);
            }
            Self::SemanticOperation(value) => {
                digest.update([12]);
                digest.update(value);
            }
            Self::CompilerHandoffLlvm(value) => {
                digest.update([13]);
                hash_llvm(&mut digest, *value);
            }
            Self::Transformation(value) => {
                digest.update([14]);
                digest.update([*value as u8]);
            }
            Self::ExactPc {
                kernel_ordinal,
                symbol_relative_pc,
            } => {
                digest.update([15]);
                digest.update(kernel_ordinal.to_le_bytes());
                digest.update(symbol_relative_pc.to_le_bytes());
            }
            Self::PreKirOnly => digest.update([16]),
        }
        digest.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceIsaCharacteristicTargetQueryV1 {
    All,
    CharacteristicIdentity([u8; 32]),
    Category(SourceIsaCharacteristicCategoryV1),
    Kind(SourceIsaCharacteristicKindV1),
    TargetKir(SourceIsaCharacteristicKirCoordinateV1),
}

impl SourceIsaCharacteristicTargetQueryV1 {
    fn validate(&self) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if matches!(self, Self::CharacteristicIdentity(identity) if *identity == [0; 32]) {
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        } else {
            Ok(())
        }
    }

    pub fn identity(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(TARGET_QUERY_IDENTITY_DOMAIN_V1);
        match self {
            Self::All => digest.update([1]),
            Self::CharacteristicIdentity(identity) => {
                digest.update([2]);
                digest.update(identity);
            }
            Self::Category(category) => {
                digest.update([3]);
                digest.update(category.code().to_le_bytes());
            }
            Self::Kind(kind) => {
                digest.update([4]);
                digest.update(kind.code().to_le_bytes());
            }
            Self::TargetKir(coordinate) => {
                digest.update([5]);
                hash_kir(&mut digest, *coordinate);
            }
        }
        digest.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicTargetMatchV1 {
    identity: [u8; 32],
    characteristic_identity: [u8; 32],
    category: SourceIsaCharacteristicCategoryV1,
    kind: SourceIsaCharacteristicKindV1,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    correlation_count: u64,
}

impl SourceIsaCharacteristicTargetMatchV1 {
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub const fn characteristic_identity(self) -> [u8; 32] {
        self.characteristic_identity
    }
    pub const fn category(self) -> SourceIsaCharacteristicCategoryV1 {
        self.category
    }
    pub const fn kind(self) -> SourceIsaCharacteristicKindV1 {
        self.kind
    }
    pub const fn target_kir(self) -> SourceIsaCharacteristicKirCoordinateV1 {
        self.target_kir
    }
    pub const fn correlation_count(self) -> u64 {
        self.correlation_count
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicTargetPageV1 {
    collection_identity: [u8; 32],
    query_identity: [u8; 32],
    total_matches: u64,
    page_exhausted: bool,
    next_cursor: Option<SourceIsaCharacteristicCursorV1>,
    targets: Vec<SourceIsaCharacteristicTargetMatchV1>,
}

impl SourceIsaCharacteristicTargetPageV1 {
    pub const fn collection_identity(&self) -> [u8; 32] {
        self.collection_identity
    }
    pub const fn query_identity(&self) -> [u8; 32] {
        self.query_identity
    }
    pub const fn total_matches(&self) -> u64 {
        self.total_matches
    }
    pub const fn page_exhausted(&self) -> bool {
        self.page_exhausted
    }
    pub const fn next_cursor(&self) -> Option<SourceIsaCharacteristicCursorV1> {
        self.next_cursor
    }
    pub fn targets(&self) -> &[SourceIsaCharacteristicTargetMatchV1] {
        &self.targets
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicTargetCorrelationSummaryV1 {
    pub catalog_record_ordinal: u64,
    pub kind: SourceIsaCharacteristicRecordKindV1,
    pub source: Option<SourceIsaCharacteristicSourceCoordinateV1>,
    pub mir_node_identity: Option<[u8; 32]>,
    pub mir: Option<SourceIsaCharacteristicMirCoordinateV1>,
    pub neutral_kir_node_identity: Option<[u8; 32]>,
    pub neutral_kir: Option<SourceIsaCharacteristicKirCoordinateV1>,
    pub target_kir: SourceIsaCharacteristicKirCoordinateV1,
    pub semantic_operation_identity: [u8; 32],
    pub compiler_handoff_llvm: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    pub interval_count: u64,
    pub transformation: SourceIsaCharacteristicTransformationV1,
}

impl SourceIsaCharacteristicTargetCorrelationSummaryV1 {
    fn from_correlation(
        value: &SourceIsaCharacteristicTargetCorrelationV1,
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        Ok(Self {
            catalog_record_ordinal: value.catalog_record_ordinal,
            kind: value.kind,
            source: value.source,
            mir_node_identity: value.mir_node_identity,
            mir: value.mir,
            neutral_kir_node_identity: value.neutral_kir_node_identity,
            neutral_kir: value.neutral_kir,
            target_kir: value.target_kir,
            semantic_operation_identity: value.semantic_operation_identity,
            compiler_handoff_llvm: value.compiler_handoff_llvm,
            interval_count: u64::try_from(value.isa_intervals.len())
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
            transformation: value.transformation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceIsaCharacteristicMatchOutcomeV1 {
    TargetCorrelation(SourceIsaCharacteristicTargetCorrelationSummaryV1),
    PreKirElimination(SourceIsaCharacteristicPreKirEliminationV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicMatchV1 {
    identity: [u8; 32],
    characteristic_identity: Option<[u8; 32]>,
    category: Option<SourceIsaCharacteristicCategoryV1>,
    kind: Option<SourceIsaCharacteristicKindV1>,
    target_kir: Option<SourceIsaCharacteristicKirCoordinateV1>,
    outcome: SourceIsaCharacteristicMatchOutcomeV1,
}

impl SourceIsaCharacteristicMatchV1 {
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub const fn characteristic_identity(self) -> Option<[u8; 32]> {
        self.characteristic_identity
    }
    pub const fn category(self) -> Option<SourceIsaCharacteristicCategoryV1> {
        self.category
    }
    pub const fn kind(self) -> Option<SourceIsaCharacteristicKindV1> {
        self.kind
    }
    pub const fn target_kir(self) -> Option<SourceIsaCharacteristicKirCoordinateV1> {
        self.target_kir
    }
    pub const fn outcome(self) -> SourceIsaCharacteristicMatchOutcomeV1 {
        self.outcome
    }
}

enum RawMatchV1<'a> {
    TargetCorrelation(
        &'a SourceIsaCharacteristicTargetV1,
        &'a SourceIsaCharacteristicTargetCorrelationV1,
    ),
    PreKir(&'a SourceIsaCharacteristicPreKirEliminationV1),
}

impl RawMatchV1<'_> {
    fn identity(&self, collection: [u8; 32]) -> [u8; 32] {
        match self {
            Self::TargetCorrelation(target, value) => {
                source_isa_characteristic_target_correlation_match_identity_v1(
                    collection,
                    target.identity,
                    value.catalog_record_ordinal,
                )
            }
            Self::PreKir(value) => source_isa_characteristic_pre_kir_match_identity_v1(
                collection,
                value.catalog_record_ordinal,
            ),
        }
    }
    fn project(
        &self,
        collection: [u8; 32],
    ) -> Result<SourceIsaCharacteristicMatchV1, SourceIsaCharacteristicErrorV1> {
        match self {
            Self::TargetCorrelation(target, value) => Ok(SourceIsaCharacteristicMatchV1 {
                identity: self.identity(collection),
                characteristic_identity: Some(target.identity),
                category: Some(target.category()),
                kind: Some(target.kind),
                target_kir: Some(target.target_kir),
                outcome: SourceIsaCharacteristicMatchOutcomeV1::TargetCorrelation(
                    SourceIsaCharacteristicTargetCorrelationSummaryV1::from_correlation(value)?,
                ),
            }),
            Self::PreKir(value) => Ok(SourceIsaCharacteristicMatchV1 {
                identity: self.identity(collection),
                characteristic_identity: None,
                category: None,
                kind: None,
                target_kir: None,
                outcome: SourceIsaCharacteristicMatchOutcomeV1::PreKirElimination(**value),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicCursorV1 {
    collection_identity: [u8; 32],
    query_identity: [u8; 32],
    next_ordinal: u64,
    preceding_item_identity: [u8; 32],
    identity: [u8; 32],
}

impl SourceIsaCharacteristicCursorV1 {
    fn new(
        collection_identity: [u8; 32],
        query_identity: [u8; 32],
        next_ordinal: u64,
        preceding_item_identity: [u8; 32],
    ) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if collection_identity == [0; 32]
            || query_identity == [0; 32]
            || next_ordinal == 0
            || preceding_item_identity == [0; 32]
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        let identity = cursor_identity(
            collection_identity,
            query_identity,
            next_ordinal,
            preceding_item_identity,
        );
        Ok(Self {
            collection_identity,
            query_identity,
            next_ordinal,
            preceding_item_identity,
            identity,
        })
    }
    fn validate(
        &self,
        collection: [u8; 32],
        query: [u8; 32],
    ) -> Result<(), SourceIsaCharacteristicErrorV1> {
        if self.collection_identity != collection
            || self.query_identity != query
            || self.next_ordinal == 0
            || self.preceding_item_identity == [0; 32]
            || self.identity
                != cursor_identity(
                    self.collection_identity,
                    self.query_identity,
                    self.next_ordinal,
                    self.preceding_item_identity,
                )
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        Ok(())
    }
    pub const fn collection_identity(self) -> [u8; 32] {
        self.collection_identity
    }
    pub const fn query_identity(self) -> [u8; 32] {
        self.query_identity
    }
    pub const fn next_ordinal(self) -> u64 {
        self.next_ordinal
    }
    pub const fn preceding_item_identity(self) -> [u8; 32] {
        self.preceding_item_identity
    }
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub fn encode_canonical(self) -> [u8; SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1] {
        let mut bytes = [0; SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1];
        bytes[..8].copy_from_slice(CURSOR_MAGIC_V1);
        bytes[8..10].copy_from_slice(&SOURCE_ISA_CHARACTERISTIC_VERSION_V1.to_le_bytes());
        bytes[10..12].copy_from_slice(&16u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&152u32.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.collection_identity);
        bytes[48..80].copy_from_slice(&self.query_identity);
        bytes[80..88].copy_from_slice(&self.next_ordinal.to_le_bytes());
        bytes[88..120].copy_from_slice(&self.preceding_item_identity);
        bytes[120..].copy_from_slice(&self.identity);
        bytes
    }
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        if bytes.len() != SOURCE_ISA_CHARACTERISTIC_CURSOR_BYTES_V1
            || bytes.get(..8) != Some(CURSOR_MAGIC_V1)
            || u16::from_le_bytes(
                bytes[8..10]
                    .try_into()
                    .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            ) != 1
            || u16::from_le_bytes(
                bytes[10..12]
                    .try_into()
                    .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            ) != 16
            || u32::from_le_bytes(
                bytes[12..16]
                    .try_into()
                    .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            ) != 152
        {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        let cursor = Self {
            collection_identity: bytes[16..48]
                .try_into()
                .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            query_identity: bytes[48..80]
                .try_into()
                .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            next_ordinal: u64::from_le_bytes(
                bytes[80..88]
                    .try_into()
                    .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            ),
            preceding_item_identity: bytes[88..120]
                .try_into()
                .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
            identity: bytes[120..]
                .try_into()
                .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?,
        };
        cursor.validate(cursor.collection_identity, cursor.query_identity)?;
        Ok(cursor)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicPageV1 {
    collection_identity: [u8; 32],
    query_identity: [u8; 32],
    total_matches: u64,
    page_exhausted: bool,
    next_cursor: Option<SourceIsaCharacteristicCursorV1>,
    matches: Vec<SourceIsaCharacteristicMatchV1>,
}

impl SourceIsaCharacteristicPageV1 {
    pub const fn collection_identity(&self) -> [u8; 32] {
        self.collection_identity
    }
    pub const fn query_identity(&self) -> [u8; 32] {
        self.query_identity
    }
    pub const fn total_matches(&self) -> u64 {
        self.total_matches
    }
    pub const fn page_exhausted(&self) -> bool {
        self.page_exhausted
    }
    pub const fn next_cursor(&self) -> Option<SourceIsaCharacteristicCursorV1> {
        self.next_cursor
    }
    pub fn matches(&self) -> &[SourceIsaCharacteristicMatchV1] {
        &self.matches
    }
}

impl SourceIsaCharacteristicCollectionV1 {
    pub fn query_targets_page(
        &self,
        query: &SourceIsaCharacteristicTargetQueryV1,
        cursor: Option<&SourceIsaCharacteristicCursorV1>,
        limit: u16,
    ) -> Result<SourceIsaCharacteristicTargetPageV1, SourceIsaCharacteristicErrorV1> {
        query.validate()?;
        validate_page_limit(limit)?;
        let query_identity = query.identity();
        let start = cursor.map_or(Ok(0usize), |cursor| {
            cursor.validate(self.identity, query_identity)?;
            usize::try_from(cursor.next_ordinal)
                .map_err(|_| SourceIsaCharacteristicErrorV1::InvalidCursor)
        })?;
        let total = self
            .targets
            .iter()
            .filter(|target| target_matches_query(target, query))
            .count();
        if cursor.is_some() && (start == 0 || start == total) || start > total {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        if let Some(cursor) = cursor {
            let preceding = self
                .targets
                .iter()
                .filter(|target| target_matches_query(target, query))
                .nth(start - 1)
                .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?;
            if target_match_identity(
                self.identity,
                preceding.identity,
                preceding.kind,
                preceding.target_kir,
                preceding.correlations.len(),
            )? != cursor.preceding_item_identity
            {
                return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
            }
        }
        let take = usize::from(limit).min(total - start);
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(take)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        for target in self
            .targets
            .iter()
            .filter(|target| target_matches_query(target, query))
            .skip(start)
            .take(take)
        {
            let correlation_count = u64::try_from(target.correlations.len())
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
            targets.push(SourceIsaCharacteristicTargetMatchV1 {
                identity: target_match_identity(
                    self.identity,
                    target.identity,
                    target.kind,
                    target.target_kir,
                    target.correlations.len(),
                )?,
                characteristic_identity: target.identity,
                category: target.category(),
                kind: target.kind,
                target_kir: target.target_kir,
                correlation_count,
            });
        }
        let next = start
            .checked_add(take)
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        let next_cursor = if next == total {
            None
        } else {
            Some(SourceIsaCharacteristicCursorV1::new(
                self.identity,
                query_identity,
                u64::try_from(next).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
                targets
                    .last()
                    .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?
                    .identity,
            )?)
        };
        Ok(SourceIsaCharacteristicTargetPageV1 {
            collection_identity: self.identity,
            query_identity,
            total_matches: u64::try_from(total)
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
            page_exhausted: next == total,
            next_cursor,
            targets,
        })
    }
}

fn target_matches_query(
    target: &SourceIsaCharacteristicTargetV1,
    query: &SourceIsaCharacteristicTargetQueryV1,
) -> bool {
    match query {
        SourceIsaCharacteristicTargetQueryV1::All => true,
        SourceIsaCharacteristicTargetQueryV1::CharacteristicIdentity(identity) => {
            target.identity == *identity
        }
        SourceIsaCharacteristicTargetQueryV1::Category(category) => target.category() == *category,
        SourceIsaCharacteristicTargetQueryV1::Kind(kind) => target.kind == *kind,
        SourceIsaCharacteristicTargetQueryV1::TargetKir(coordinate) => {
            target.target_kir == *coordinate
        }
    }
}

fn target_match_identity(
    collection: [u8; 32],
    target: [u8; 32],
    kind: SourceIsaCharacteristicKindV1,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    correlation_count: usize,
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    let mut digest = Sha256::new();
    digest.update(TARGET_MATCH_IDENTITY_DOMAIN_V1);
    digest.update(collection);
    digest.update(target);
    digest.update(kind.code().to_le_bytes());
    hash_kir(&mut digest, target_kir);
    digest.update(
        u64::try_from(correlation_count)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(digest.finalize().into())
}

pub fn source_isa_characteristic_target_match_identity_v1(
    collection: [u8; 32],
    target: [u8; 32],
    kind: SourceIsaCharacteristicKindV1,
    target_kir: SourceIsaCharacteristicKirCoordinateV1,
    correlation_count: u64,
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    if collection == [0; 32] || target == [0; 32] {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    target_match_identity(
        collection,
        target,
        kind,
        target_kir,
        usize::try_from(correlation_count)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
    )
}

pub fn source_isa_characteristic_target_correlation_match_identity_v1(
    collection: [u8; 32],
    target: [u8; 32],
    catalog_record_ordinal: u64,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(MATCH_IDENTITY_DOMAIN_V1);
    digest.update(collection);
    digest.update([1]);
    digest.update(target);
    digest.update(catalog_record_ordinal.to_le_bytes());
    digest.finalize().into()
}

pub fn source_isa_characteristic_pre_kir_match_identity_v1(
    collection: [u8; 32],
    catalog_record_ordinal: u64,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(MATCH_IDENTITY_DOMAIN_V1);
    digest.update(collection);
    digest.update([2]);
    digest.update(catalog_record_ordinal.to_le_bytes());
    digest.finalize().into()
}

impl SourceIsaCharacteristicCollectionV1 {
    pub fn query_page(
        &self,
        query: &SourceIsaCharacteristicQueryV1,
        cursor: Option<&SourceIsaCharacteristicCursorV1>,
        limit: u16,
    ) -> Result<SourceIsaCharacteristicPageV1, SourceIsaCharacteristicErrorV1> {
        query.validate()?;
        validate_page_limit(limit)?;
        let query_identity = query.identity();
        let start = cursor.map_or(Ok(0usize), |cursor| {
            cursor.validate(self.identity, query_identity)?;
            usize::try_from(cursor.next_ordinal)
                .map_err(|_| SourceIsaCharacteristicErrorV1::InvalidCursor)
        })?;
        let total = self.query_match_count(query)?;
        if cursor.is_some() && (start == 0 || start == total) {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        if start > total {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        if let Some(cursor) = cursor {
            let predecessor = self
                .query_match_identity_at(query, start - 1)?
                .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?;
            if predecessor != cursor.preceding_item_identity {
                return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
            }
        }
        let take = usize::from(limit).min(total - start);
        let mut matches = Vec::new();
        matches
            .try_reserve_exact(take)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        let end = start
            .checked_add(take)
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        self.visit_matches(query, |ordinal, raw| {
            if ordinal >= start && ordinal < end {
                matches.push(raw.project(self.identity)?);
            }
            Ok(ordinal
                .checked_add(1)
                .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?
                < end)
        })?;
        if matches.len() != take {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        let next = start + take;
        let next_cursor = if next == total {
            None
        } else {
            Some(SourceIsaCharacteristicCursorV1::new(
                self.identity,
                query_identity,
                u64::try_from(next).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
                matches
                    .last()
                    .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?
                    .identity,
            )?)
        };
        Ok(SourceIsaCharacteristicPageV1 {
            collection_identity: self.identity,
            query_identity,
            total_matches: u64::try_from(total)
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
            page_exhausted: next == total,
            next_cursor,
            matches,
        })
    }

    fn query_match_count(
        &self,
        query: &SourceIsaCharacteristicQueryV1,
    ) -> Result<usize, SourceIsaCharacteristicErrorV1> {
        let mut count = 0usize;
        self.visit_matches(query, |_, _| {
            count = count
                .checked_add(1)
                .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
            Ok(true)
        })?;
        Ok(count)
    }

    fn query_match_identity_at(
        &self,
        query: &SourceIsaCharacteristicQueryV1,
        target: usize,
    ) -> Result<Option<[u8; 32]>, SourceIsaCharacteristicErrorV1> {
        let mut found = None;
        self.visit_matches(query, |ordinal, raw| {
            if ordinal == target {
                found = Some(raw.identity(self.identity));
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        Ok(found)
    }

    fn visit_matches<F>(
        &self,
        query: &SourceIsaCharacteristicQueryV1,
        mut visitor: F,
    ) -> Result<(), SourceIsaCharacteristicErrorV1>
    where
        F: FnMut(usize, RawMatchV1<'_>) -> Result<bool, SourceIsaCharacteristicErrorV1>,
    {
        let mut ordinal = 0usize;
        for target in &self.targets {
            for correlation in &target.correlations {
                let raw = RawMatchV1::TargetCorrelation(target, correlation);
                if raw_matches_query(&raw, query) {
                    if !visitor(ordinal, raw)? {
                        return Ok(());
                    }
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
                }
            }
        }
        for fact in &self.pre_kir_eliminations {
            let raw = RawMatchV1::PreKir(fact);
            if raw_matches_query(&raw, query) {
                if !visitor(ordinal, raw)? {
                    return Ok(());
                }
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
            }
        }
        Ok(())
    }
}

fn raw_matches_query(raw: &RawMatchV1<'_>, query: &SourceIsaCharacteristicQueryV1) -> bool {
    match raw {
        RawMatchV1::TargetCorrelation(target, value) => match query {
            SourceIsaCharacteristicQueryV1::All => true,
            SourceIsaCharacteristicQueryV1::CharacteristicIdentity(identity) => {
                target.identity == *identity
            }
            SourceIsaCharacteristicQueryV1::Category(category) => target.category() == *category,
            SourceIsaCharacteristicQueryV1::Kind(kind) => target.kind == *kind,
            SourceIsaCharacteristicQueryV1::SourceNode(identity) => value
                .source
                .is_some_and(|source| source.node_identity == *identity),
            SourceIsaCharacteristicQueryV1::SourceSpan(span) => {
                value.source.is_some_and(|source| source.span == *span)
            }
            SourceIsaCharacteristicQueryV1::RecordKind(kind) => value.kind == *kind,
            SourceIsaCharacteristicQueryV1::MirNode(identity) => {
                value.mir_node_identity == Some(*identity)
            }
            SourceIsaCharacteristicQueryV1::Mir(coordinate) => value.mir == Some(*coordinate),
            SourceIsaCharacteristicQueryV1::NeutralKirNode(identity) => {
                value.neutral_kir_node_identity == Some(*identity)
            }
            SourceIsaCharacteristicQueryV1::NeutralKir(coordinate) => {
                value.neutral_kir == Some(*coordinate)
            }
            SourceIsaCharacteristicQueryV1::TargetKir(coordinate) => {
                value.target_kir == *coordinate
            }
            SourceIsaCharacteristicQueryV1::SemanticOperation(identity) => {
                value.semantic_operation_identity == *identity
            }
            SourceIsaCharacteristicQueryV1::CompilerHandoffLlvm(coordinate) => {
                value.compiler_handoff_llvm == *coordinate
            }
            SourceIsaCharacteristicQueryV1::Transformation(transformation) => {
                value.transformation == *transformation
            }
            SourceIsaCharacteristicQueryV1::ExactPc {
                kernel_ordinal,
                symbol_relative_pc,
            } => value
                .isa_intervals
                .iter()
                .any(|interval| interval.contains(*kernel_ordinal, *symbol_relative_pc)),
            SourceIsaCharacteristicQueryV1::PreKirOnly => false,
        },
        RawMatchV1::PreKir(value) => match query {
            SourceIsaCharacteristicQueryV1::All => true,
            SourceIsaCharacteristicQueryV1::SourceNode(identity) => {
                value.source.node_identity == *identity
            }
            SourceIsaCharacteristicQueryV1::SourceSpan(span) => value.source.span == *span,
            SourceIsaCharacteristicQueryV1::MirNode(identity) => {
                value.mir_node_identity == *identity
            }
            SourceIsaCharacteristicQueryV1::Mir(coordinate) => value.mir == *coordinate,
            SourceIsaCharacteristicQueryV1::RecordKind(kind) => {
                *kind == SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir
            }
            SourceIsaCharacteristicQueryV1::PreKirOnly => true,
            _ => false,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicIntervalItemV1 {
    identity: [u8; 32],
    ordinal: u64,
    interval: SourceIsaCharacteristicIsaIntervalV1,
}
impl SourceIsaCharacteristicIntervalItemV1 {
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub const fn ordinal(self) -> u64 {
        self.ordinal
    }
    pub const fn interval(self) -> SourceIsaCharacteristicIsaIntervalV1 {
        self.interval
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SourceIsaCharacteristicIntervalPageV1 {
    occurrence_identity: [u8; 32],
    total_intervals: u64,
    page_exhausted: bool,
    next_cursor: Option<SourceIsaCharacteristicCursorV1>,
    intervals: Vec<SourceIsaCharacteristicIntervalItemV1>,
}
impl SourceIsaCharacteristicIntervalPageV1 {
    pub const fn occurrence_identity(&self) -> [u8; 32] {
        self.occurrence_identity
    }
    pub const fn total_intervals(&self) -> u64 {
        self.total_intervals
    }
    pub const fn page_exhausted(&self) -> bool {
        self.page_exhausted
    }
    pub const fn next_cursor(&self) -> Option<SourceIsaCharacteristicCursorV1> {
        self.next_cursor
    }
    pub fn intervals(&self) -> &[SourceIsaCharacteristicIntervalItemV1] {
        &self.intervals
    }
}

impl SourceIsaCharacteristicCollectionV1 {
    pub fn interval_page(
        &self,
        occurrence_identity: [u8; 32],
        cursor: Option<&SourceIsaCharacteristicCursorV1>,
        limit: u16,
    ) -> Result<SourceIsaCharacteristicIntervalPageV1, SourceIsaCharacteristicErrorV1> {
        validate_page_limit(limit)?;
        if occurrence_identity == [0; 32] {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        let correlation = self
            .find_correlation(occurrence_identity)
            .ok_or(SourceIsaCharacteristicErrorV1::InvalidClaim)?;
        let query_identity = interval_query_identity(occurrence_identity);
        let start = cursor.map_or(Ok(0usize), |cursor| {
            cursor.validate(self.identity, query_identity)?;
            usize::try_from(cursor.next_ordinal)
                .map_err(|_| SourceIsaCharacteristicErrorV1::InvalidCursor)
        })?;
        if cursor.is_some() && (start == 0 || start == correlation.isa_intervals.len())
            || start > correlation.isa_intervals.len()
        {
            return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
        }
        if let Some(cursor) = cursor {
            let interval = correlation
                .isa_intervals
                .get(start - 1)
                .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?;
            if interval_item_identity(self.identity, occurrence_identity, start - 1, *interval)?
                != cursor.preceding_item_identity
            {
                return Err(SourceIsaCharacteristicErrorV1::InvalidCursor);
            }
        }
        let take = usize::from(limit).min(correlation.isa_intervals.len() - start);
        let mut intervals = Vec::new();
        intervals
            .try_reserve_exact(take)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        for (ordinal, interval) in correlation
            .isa_intervals
            .iter()
            .enumerate()
            .skip(start)
            .take(take)
        {
            intervals.push(SourceIsaCharacteristicIntervalItemV1 {
                identity: interval_item_identity(
                    self.identity,
                    occurrence_identity,
                    ordinal,
                    *interval,
                )?,
                ordinal: u64::try_from(ordinal)
                    .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
                interval: *interval,
            });
        }
        let next = start + take;
        let total = correlation.isa_intervals.len();
        let next_cursor = if next == total {
            None
        } else {
            Some(SourceIsaCharacteristicCursorV1::new(
                self.identity,
                query_identity,
                u64::try_from(next).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
                intervals
                    .last()
                    .ok_or(SourceIsaCharacteristicErrorV1::InvalidCursor)?
                    .identity,
            )?)
        };
        Ok(SourceIsaCharacteristicIntervalPageV1 {
            occurrence_identity,
            total_intervals: u64::try_from(total)
                .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
            page_exhausted: next == total,
            next_cursor,
            intervals,
        })
    }

    fn find_correlation(
        &self,
        occurrence_identity: [u8; 32],
    ) -> Option<&SourceIsaCharacteristicTargetCorrelationV1> {
        for target in &self.targets {
            for correlation in &target.correlations {
                let raw = RawMatchV1::TargetCorrelation(target, correlation);
                if raw.identity(self.identity) == occurrence_identity {
                    return Some(correlation);
                }
            }
        }
        None
    }
}

fn validate_page_limit(limit: u16) -> Result<(), SourceIsaCharacteristicErrorV1> {
    if limit == 0 || limit > MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1 {
        Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
    } else {
        Ok(())
    }
}
fn cursor_identity(
    collection: [u8; 32],
    query: [u8; 32],
    next: u64,
    preceding: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CURSOR_IDENTITY_DOMAIN_V1);
    digest.update(collection);
    digest.update(query);
    digest.update(next.to_le_bytes());
    digest.update(preceding);
    digest.finalize().into()
}
fn interval_query_identity(occurrence: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(QUERY_IDENTITY_DOMAIN_V1);
    digest.update([0x80]);
    digest.update(occurrence);
    digest.finalize().into()
}
pub fn source_isa_characteristic_interval_query_identity_v1(
    occurrence: [u8; 32],
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    if occurrence == [0; 32] {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    Ok(interval_query_identity(occurrence))
}
fn interval_item_identity(
    collection: [u8; 32],
    occurrence: [u8; 32],
    ordinal: usize,
    interval: SourceIsaCharacteristicIsaIntervalV1,
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    let mut digest = Sha256::new();
    digest.update(MATCH_IDENTITY_DOMAIN_V1);
    digest.update([0x80]);
    digest.update(collection);
    digest.update(occurrence);
    digest.update(
        u64::try_from(ordinal)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    digest.update(interval.kernel_ordinal.to_le_bytes());
    digest.update(interval.symbol_relative_start.to_le_bytes());
    digest.update(interval.symbol_relative_end.to_le_bytes());
    Ok(digest.finalize().into())
}

pub fn source_isa_characteristic_interval_item_identity_v1(
    collection: [u8; 32],
    occurrence: [u8; 32],
    ordinal: u64,
    interval: SourceIsaCharacteristicIsaIntervalV1,
) -> Result<[u8; 32], SourceIsaCharacteristicErrorV1> {
    if collection == [0; 32] || occurrence == [0; 32] {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    interval_item_identity(
        collection,
        occurrence,
        usize::try_from(ordinal).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
        interval,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }
    fn content(byte: u8, len: u64) -> SourceIsaCharacteristicContentIdentityV1 {
        SourceIsaCharacteristicContentIdentityV1::new(id(byte), len).unwrap()
    }
    fn binding(operations: u64) -> SourceIsaCharacteristicBindingV1 {
        SourceIsaCharacteristicBindingV1::new(
            SourceIsaCharacteristicTargetProfileV1::Gfx942,
            SourceIsaCharacteristicKirVersionV1::V8,
            id(1),
            SourceIsaCharacteristicStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 1,
                operations,
            },
            content(2, 20),
            content(3, 30),
            content(4, 40),
            content(5, 50),
            content(6, 60),
            content(7, 70),
            id(8),
            id(9),
        )
        .unwrap()
    }
    fn span(node: u8, empty: bool) -> SourceIsaCharacteristicSourceSpanV1 {
        let start = u64::from(node);
        SourceIsaCharacteristicSourceSpanV1::new(
            id(node + 1),
            start,
            if empty { start } else { start + 4 },
            u32::from(node),
            1,
        )
        .unwrap()
    }
    fn source(node: u8) -> SourceIsaCharacteristicSourceCoordinateV1 {
        SourceIsaCharacteristicSourceCoordinateV1::new(id(node), span(node, false)).unwrap()
    }
    fn target_kir() -> SourceIsaCharacteristicKirCoordinateV1 {
        SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap()
    }
    fn global_store_kind() -> SourceIsaCharacteristicKindV1 {
        SourceIsaCharacteristicKindV1::GlobalStore {
            form: SourceIsaCharacteristicMemoryFormV1::Plain,
        }
    }
    fn anchored_at(catalog_record_ordinal: u64) -> SourceIsaCharacteristicTargetCorrelationV1 {
        let duplicate = SourceIsaCharacteristicIsaIntervalV1::new(0, 0x40, 0x44).unwrap();
        SourceIsaCharacteristicTargetCorrelationV1::new(
            catalog_record_ordinal,
            SourceIsaCharacteristicRecordKindV1::SourceAnchored,
            Some(source(20)),
            Some(id(22)),
            Some(SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 2).unwrap()),
            Some(id(23)),
            Some(SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap()),
            target_kir(),
            id(24),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 3).unwrap(),
            vec![duplicate, duplicate],
            SourceIsaCharacteristicTransformationV1::Duplicated,
        )
        .unwrap()
    }
    fn anchored() -> SourceIsaCharacteristicTargetCorrelationV1 {
        anchored_at(0)
    }
    fn no_source_eliminated() -> SourceIsaCharacteristicTargetCorrelationV1 {
        SourceIsaCharacteristicTargetCorrelationV1::new(
            1,
            SourceIsaCharacteristicRecordKindV1::NoSourceProvenance,
            None,
            None,
            None,
            None,
            None,
            target_kir(),
            id(25),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 4).unwrap(),
            Vec::new(),
            SourceIsaCharacteristicTransformationV1::Eliminated,
        )
        .unwrap()
    }
    fn pre_kir() -> SourceIsaCharacteristicPreKirEliminationV1 {
        SourceIsaCharacteristicPreKirEliminationV1::new(
            2,
            SourceIsaCharacteristicSourceCoordinateV1::new(id(30), span(30, true)).unwrap(),
            id(32),
            SourceIsaCharacteristicMirCoordinateV1::new(0, 3, 4).unwrap(),
        )
        .unwrap()
    }
    fn fixture() -> SourceIsaCharacteristicCollectionV1 {
        SourceIsaCharacteristicCollectionV1::new(
            binding(2),
            SourceIsaCharacteristicScanSummaryV1::new(
                3,
                3,
                2,
                2,
                1,
                2,
                1,
                3,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            vec![
                SourceIsaCharacteristicTargetV1::new(
                    global_store_kind(),
                    target_kir(),
                    vec![anchored(), no_source_eliminated()],
                )
                .unwrap(),
            ],
            vec![pre_kir()],
        )
        .unwrap()
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn canonical_known_answer_round_trip_requires_exact_projection() {
        let exact = fixture();
        let encoded = exact.encode_canonical().unwrap();
        assert_eq!(encoded.len(), 1_480);
        assert_eq!(
            hex(&Sha256::digest(&encoded)),
            "d2abdf6e408a56db774d48463c1c064afb596a7818d1c435bc59848ef1dfd935"
        );
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded).unwrap();
        assert_eq!(inert.claimed_identity(), exact.identity());
        assert_eq!(inert.admit_exact_projection_v1(&exact).unwrap(), exact);
    }

    #[test]
    fn complete_empty_scan_is_canonical() {
        let exact = SourceIsaCharacteristicCollectionV1::new(
            binding(0),
            SourceIsaCharacteristicScanSummaryV1::new(
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let encoded = exact.encode_canonical().unwrap();
        assert_eq!(
            encoded.len(),
            SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1 + 32
        );
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded).unwrap();
        assert_eq!(inert.admit_exact_projection_v1(&exact).unwrap(), exact);
        assert!(exact.scan().is_complete());
        assert!(!exact.has_sparse_final_hsaco_anchors());
    }

    #[test]
    fn zero_structural_counts_are_losslessly_bound() {
        let binding = SourceIsaCharacteristicBindingV1::new(
            SourceIsaCharacteristicTargetProfileV1::Gfx942,
            SourceIsaCharacteristicKirVersionV1::V8,
            id(1),
            SourceIsaCharacteristicStructuralCountsV1 {
                functions: 0,
                defined_bodies: 0,
                blocks: 0,
                operations: 0,
            },
            content(2, 20),
            content(3, 30),
            content(4, 40),
            content(5, 50),
            content(6, 60),
            content(7, 70),
            id(8),
            id(9),
        )
        .unwrap();
        let collection = SourceIsaCharacteristicCollectionV1::new(
            binding,
            SourceIsaCharacteristicScanSummaryV1::new(
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(collection.binding().structural_counts().functions, 0);
        assert_eq!(collection.canonical_byte_len().unwrap(), 544);
    }

    #[test]
    fn independent_producer_maxima_fit_the_128_mib_wire_bound() {
        let worst_case = SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1
            .checked_add(
                MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1
                    .checked_mul(TARGET_HEADER_BYTES_V1)
                    .unwrap(),
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1
                        .checked_mul(TARGET_CORRELATION_PREFIX_BYTES_V1)?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1
                        .checked_mul(ISA_INTERVAL_BYTES_V1)?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1
                        .checked_mul(PRE_KIR_FACT_BYTES_V1)?,
                )
            })
            .and_then(|bytes| bytes.checked_add(SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1))
            .unwrap();
        assert_eq!(worst_case, 123_494_176);
        assert!(worst_case <= MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1);
        assert!(payload_length(&[], MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1).is_ok());
    }

    #[test]
    fn catalog_record_bound_is_explicit_and_fails_closed() {
        let maximum = u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_CATALOG_RECORDS_V1).unwrap();
        assert!(
            SourceIsaCharacteristicScanSummaryV1::new(
                maximum,
                maximum,
                0,
                0,
                0,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .is_ok()
        );
        let over = maximum.checked_add(1).unwrap();
        assert_eq!(
            SourceIsaCharacteristicScanSummaryV1::new(
                over,
                over,
                0,
                0,
                0,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            ),
            Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn complete_scan_may_omit_non_characteristic_catalog_records() {
        let exact = SourceIsaCharacteristicCollectionV1::new(
            binding(1),
            SourceIsaCharacteristicScanSummaryV1::new(
                1,
                1,
                1,
                1,
                0,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        assert!(exact.scan().is_complete());
        assert!(exact.targets().is_empty());
        assert!(exact.pre_kir_eliminations().is_empty());
    }

    #[test]
    fn partial_axes_duplicate_anchors_and_zero_width_pre_kir_span_are_lossless() {
        let exact = fixture();
        let correlations = exact.targets()[0].correlations();
        assert_eq!(correlations[0].isa_intervals().len(), 2);
        assert_eq!(
            correlations[0].isa_intervals()[0],
            correlations[0].isa_intervals()[1]
        );
        assert!(correlations[1].source().is_none());
        assert!(correlations[1].isa_intervals().is_empty());
        assert_eq!(
            correlations[1].transformation(),
            SourceIsaCharacteristicTransformationV1::Eliminated
        );
        let pre = exact.pre_kir_eliminations()[0];
        assert_eq!(
            pre.source().span().byte_start(),
            pre.source().span().byte_end()
        );
    }

    #[test]
    fn fact_queries_cover_every_axis_without_inlining_intervals() {
        let exact = fixture();
        for (query, expected) in [
            (SourceIsaCharacteristicQueryV1::All, 3),
            (
                SourceIsaCharacteristicQueryV1::CharacteristicIdentity(
                    exact.targets()[0].identity(),
                ),
                2,
            ),
            (
                SourceIsaCharacteristicQueryV1::Category(
                    SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore,
                ),
                2,
            ),
            (SourceIsaCharacteristicQueryV1::Kind(global_store_kind()), 2),
            (SourceIsaCharacteristicQueryV1::SourceNode(id(20)), 1),
            (SourceIsaCharacteristicQueryV1::SourceNode(id(30)), 1),
            (
                SourceIsaCharacteristicQueryV1::SourceSpan(span(20, false)),
                1,
            ),
            (SourceIsaCharacteristicQueryV1::MirNode(id(22)), 1),
            (
                SourceIsaCharacteristicQueryV1::Mir(
                    SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 2).unwrap(),
                ),
                1,
            ),
            (SourceIsaCharacteristicQueryV1::NeutralKirNode(id(23)), 1),
            (SourceIsaCharacteristicQueryV1::NeutralKir(target_kir()), 1),
            (SourceIsaCharacteristicQueryV1::TargetKir(target_kir()), 2),
            (SourceIsaCharacteristicQueryV1::SemanticOperation(id(25)), 1),
            (
                SourceIsaCharacteristicQueryV1::CompilerHandoffLlvm(
                    SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 3).unwrap(),
                ),
                1,
            ),
            (
                SourceIsaCharacteristicQueryV1::RecordKind(
                    SourceIsaCharacteristicRecordKindV1::NoSourceProvenance,
                ),
                1,
            ),
            (
                SourceIsaCharacteristicQueryV1::RecordKind(
                    SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir,
                ),
                1,
            ),
            (
                SourceIsaCharacteristicQueryV1::Transformation(
                    SourceIsaCharacteristicTransformationV1::Eliminated,
                ),
                1,
            ),
            (SourceIsaCharacteristicQueryV1::PreKirOnly, 1),
            (
                SourceIsaCharacteristicQueryV1::ExactPc {
                    kernel_ordinal: 0,
                    symbol_relative_pc: 0x40,
                },
                1,
            ),
        ] {
            assert_eq!(
                exact.query_page(&query, None, 64).unwrap().total_matches(),
                expected
            );
        }
        assert_eq!(
            exact.query_page(
                &SourceIsaCharacteristicQueryV1::ExactPc {
                    kernel_ordinal: 0,
                    symbol_relative_pc: 0x41
                },
                None,
                64
            ),
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        );
        let page = exact
            .query_page(&SourceIsaCharacteristicQueryV1::All, None, 1)
            .unwrap();
        let SourceIsaCharacteristicMatchOutcomeV1::TargetCorrelation(summary) =
            page.matches()[0].outcome()
        else {
            panic!("first all-fact match must be a target correlation");
        };
        assert_eq!(summary.interval_count, 2);
    }

    #[test]
    fn structural_target_without_catalog_correlation_is_query_visible() {
        let exact = SourceIsaCharacteristicCollectionV1::new(
            binding(1),
            SourceIsaCharacteristicScanSummaryV1::new(
                0,
                0,
                1,
                1,
                1,
                0,
                0,
                0,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            vec![
                SourceIsaCharacteristicTargetV1::new(
                    SourceIsaCharacteristicKindV1::GlobalStore {
                        form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                    },
                    target_kir(),
                    Vec::new(),
                )
                .unwrap(),
            ],
            Vec::new(),
        )
        .unwrap();
        let fact_page = exact
            .query_page(&SourceIsaCharacteristicQueryV1::All, None, 64)
            .unwrap();
        assert_eq!(fact_page.total_matches(), 0);
        let page = exact
            .query_targets_page(&SourceIsaCharacteristicTargetQueryV1::All, None, 64)
            .unwrap();
        assert_eq!(page.total_matches(), 1);
        assert_eq!(page.targets()[0].correlation_count(), 0);
        assert_eq!(
            page.targets()[0].kind(),
            SourceIsaCharacteristicKindV1::GlobalStore {
                form: SourceIsaCharacteristicMemoryFormV1::Guarded
            }
        );
        assert_eq!(
            exact.interval_page(page.targets()[0].identity(), None, 1),
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        );
    }

    #[test]
    fn memory_form_is_committed_and_exactly_selectable() {
        let make = |form| {
            SourceIsaCharacteristicCollectionV1::new(
                binding(1),
                SourceIsaCharacteristicScanSummaryV1::new(
                    0,
                    0,
                    1,
                    1,
                    1,
                    0,
                    0,
                    0,
                    SourceIsaCharacteristicScanStateV1::Complete,
                )
                .unwrap(),
                vec![
                    SourceIsaCharacteristicTargetV1::new(
                        SourceIsaCharacteristicKindV1::GlobalStore { form },
                        target_kir(),
                        Vec::new(),
                    )
                    .unwrap(),
                ],
                Vec::new(),
            )
            .unwrap()
        };
        let plain = make(SourceIsaCharacteristicMemoryFormV1::Plain);
        let guarded = make(SourceIsaCharacteristicMemoryFormV1::Guarded);
        assert_ne!(plain.identity(), guarded.identity());
        assert_eq!(
            guarded
                .query_targets_page(
                    &SourceIsaCharacteristicTargetQueryV1::Kind(
                        SourceIsaCharacteristicKindV1::GlobalStore {
                            form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                        },
                    ),
                    None,
                    64,
                )
                .unwrap()
                .total_matches(),
            1
        );
    }

    #[test]
    fn exact_duplicate_catalog_payloads_retain_distinct_record_occurrences() {
        let exact = SourceIsaCharacteristicCollectionV1::new(
            binding(1),
            SourceIsaCharacteristicScanSummaryV1::new(
                2,
                2,
                1,
                1,
                1,
                2,
                0,
                2,
                SourceIsaCharacteristicScanStateV1::Complete,
            )
            .unwrap(),
            vec![
                SourceIsaCharacteristicTargetV1::new(
                    global_store_kind(),
                    target_kir(),
                    vec![anchored_at(1), anchored_at(0)],
                )
                .unwrap(),
            ],
            Vec::new(),
        )
        .unwrap();
        let page = exact
            .query_page(
                &SourceIsaCharacteristicQueryV1::RecordKind(
                    SourceIsaCharacteristicRecordKindV1::SourceAnchored,
                ),
                None,
                64,
            )
            .unwrap();
        assert_eq!(page.total_matches(), 2);
        assert_ne!(page.matches()[0].identity(), page.matches()[1].identity());
        let ordinals = page
            .matches()
            .iter()
            .map(|item| match item.outcome() {
                SourceIsaCharacteristicMatchOutcomeV1::TargetCorrelation(summary) => {
                    summary.catalog_record_ordinal
                }
                SourceIsaCharacteristicMatchOutcomeV1::PreKirElimination(_) => u64::MAX,
            })
            .collect::<Vec<_>>();
        assert_eq!(ordinals, [0, 1]);
    }

    #[test]
    fn target_shape_rejects_pre_kir_kind_and_empty_source_span() {
        let duplicate = SourceIsaCharacteristicIsaIntervalV1::new(0, 0x40, 0x44).unwrap();
        assert_eq!(
            SourceIsaCharacteristicTargetCorrelationV1::new(
                0,
                SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir,
                Some(source(20)),
                Some(id(22)),
                Some(SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 2).unwrap()),
                Some(id(23)),
                Some(SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap()),
                target_kir(),
                id(24),
                SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 3).unwrap(),
                vec![duplicate],
                SourceIsaCharacteristicTransformationV1::Preserved,
            ),
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        );
        let empty_source =
            SourceIsaCharacteristicSourceCoordinateV1::new(id(20), span(20, true)).unwrap();
        assert_eq!(
            SourceIsaCharacteristicTargetCorrelationV1::new(
                0,
                SourceIsaCharacteristicRecordKindV1::SourceAnchored,
                Some(empty_source),
                Some(id(22)),
                Some(SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 2).unwrap()),
                Some(id(23)),
                Some(SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap()),
                target_kir(),
                id(24),
                SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 3).unwrap(),
                vec![duplicate],
                SourceIsaCharacteristicTransformationV1::Preserved,
            ),
            Err(SourceIsaCharacteristicErrorV1::InvalidClaim)
        );
    }

    #[test]
    fn duplicate_intervals_have_distinct_occurrences_and_fact_bound_cursors() {
        let exact = fixture();
        let fact = exact
            .query_page(
                &SourceIsaCharacteristicQueryV1::ExactPc {
                    kernel_ordinal: 0,
                    symbol_relative_pc: 0x40,
                },
                None,
                1,
            )
            .unwrap()
            .matches()[0];
        let first = exact.interval_page(fact.identity(), None, 1).unwrap();
        assert_eq!(first.total_intervals(), 2);
        let cursor = first.next_cursor().unwrap();
        let second = exact
            .interval_page(fact.identity(), Some(&cursor), 1)
            .unwrap();
        assert_eq!(
            first.intervals()[0].interval(),
            second.intervals()[0].interval()
        );
        assert_ne!(
            first.intervals()[0].identity(),
            second.intervals()[0].identity()
        );
        let other_fact = exact
            .query_page(
                &SourceIsaCharacteristicQueryV1::Transformation(
                    SourceIsaCharacteristicTransformationV1::Eliminated,
                ),
                None,
                1,
            )
            .unwrap()
            .matches()[0];
        assert_eq!(
            exact.interval_page(other_fact.identity(), Some(&cursor), 1),
            Err(SourceIsaCharacteristicErrorV1::InvalidCursor)
        );

        let terminal_interval_cursor = SourceIsaCharacteristicCursorV1::new(
            exact.identity(),
            interval_query_identity(fact.identity()),
            2,
            second.intervals()[0].identity(),
        )
        .unwrap();
        assert_eq!(
            exact.interval_page(fact.identity(), Some(&terminal_interval_cursor), 1),
            Err(SourceIsaCharacteristicErrorV1::InvalidCursor)
        );
    }

    #[test]
    fn hostile_limits_and_terminal_fact_cursor_are_rejected() {
        let exact = fixture();
        assert_eq!(
            exact.query_page(&SourceIsaCharacteristicQueryV1::All, None, 0),
            Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
        );
        assert_eq!(
            exact.query_page(
                &SourceIsaCharacteristicQueryV1::All,
                None,
                MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1 + 1,
            ),
            Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
        );
        let all = exact
            .query_page(&SourceIsaCharacteristicQueryV1::All, None, 64)
            .unwrap();
        let terminal = SourceIsaCharacteristicCursorV1::new(
            exact.identity(),
            SourceIsaCharacteristicQueryV1::All.identity(),
            all.total_matches(),
            all.matches().last().unwrap().identity(),
        )
        .unwrap();
        assert_eq!(
            exact.query_page(&SourceIsaCharacteristicQueryV1::All, Some(&terminal), 1),
            Err(SourceIsaCharacteristicErrorV1::InvalidCursor)
        );

        let mut encoded = exact.encode_canonical().unwrap();
        encoded[16..20].copy_from_slice(
            &u32::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        let identity_start = encoded.len() - SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1;
        let resealed = collection_identity(&encoded[..identity_start]);
        encoded[identity_start..].copy_from_slice(&resealed);
        assert!(matches!(
            InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded),
            Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
        ));
    }

    #[test]
    fn resealed_substitution_is_inert_but_fails_exact_admission() {
        let exact = fixture();
        let mut encoded = exact.encode_canonical().unwrap();
        let binding_offset = 104;
        encoded[binding_offset] ^= 1;
        let identity_start = encoded.len() - 32;
        let resealed = collection_identity(&encoded[..identity_start]);
        encoded[identity_start..].copy_from_slice(&resealed);
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(&encoded).unwrap();
        assert_eq!(
            inert.admit_exact_projection_v1(&exact),
            Err(SourceIsaCharacteristicErrorV1::IdentityMismatch)
        );
    }

    #[test]
    fn authority_and_final_semantic_nonclaims_are_explicit() {
        let exact = fixture();
        assert!(!exact.grants_compiler_authority());
        assert!(!exact.grants_runtime_authority());
        assert!(!exact.grants_hardware_observation_authority());
        assert!(!exact.proves_a_schedule());
        assert!(!exact.proves_final_llvm_classification());
        assert!(!exact.proves_final_isa_opcode_classification());
        assert!(!exact.contains_decoded_isa());
        assert!(exact.has_sparse_final_hsaco_anchors());
    }
}

fn validate_collection_shape(
    binding: SourceIsaCharacteristicBindingV1,
    scan: SourceIsaCharacteristicScanSummaryV1,
    targets: &[SourceIsaCharacteristicTargetV1],
    pre_kir: &[SourceIsaCharacteristicPreKirEliminationV1],
) -> Result<(), SourceIsaCharacteristicErrorV1> {
    if targets.len() > MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1
        || targets
            .windows(2)
            .any(|pair| pair[0].target_kir >= pair[1].target_kir)
        || pre_kir.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(SourceIsaCharacteristicErrorV1::NonCanonical);
    }
    let target_count =
        u64::try_from(targets.len()).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    let target_correlations = target_correlation_count(targets)?;
    let target_correlations_u64 = u64::try_from(target_correlations)
        .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    let pre_count =
        u64::try_from(pre_kir.len()).map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    if scan.target_operation_count != binding.structural_counts.operations
        || scan.classified_target_count != target_count
        || scan.classified_target_count > scan.target_operations_scanned
        || scan.retained_target_correlation_count != target_correlations_u64
        || scan.pre_kir_elimination_count != pre_count
        || scan.correlation_count
            != target_correlations_u64
                .checked_add(pre_count)
                .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?
    {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    if pre_kir.len() > MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1 {
        return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
    }
    let interval_count = targets.iter().try_fold(0usize, |total, target| {
        target
            .correlations
            .iter()
            .try_fold(total, |total, correlation| {
                total
                    .checked_add(correlation.isa_intervals.len())
                    .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)
            })
    })?;
    if interval_count > MAX_SOURCE_ISA_CHARACTERISTIC_INTERVALS_V1 {
        return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
    }
    let mut ordinals = Vec::new();
    let ordinal_count = target_correlations
        .checked_add(pre_kir.len())
        .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    ordinals
        .try_reserve_exact(ordinal_count)
        .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
    for target in targets {
        {
            for correlation in &target.correlations {
                if correlation.target_kir != target.target_kir
                    || correlation.catalog_record_ordinal >= scan.catalog_record_count
                {
                    return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
                }
                ordinals.push(correlation.catalog_record_ordinal);
            }
        }
    }
    for fact in pre_kir {
        if fact.catalog_record_ordinal >= scan.catalog_record_count {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        ordinals.push(fact.catalog_record_ordinal);
    }
    ordinals.sort_unstable();
    if ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
    }
    payload_length(targets, pre_kir.len())?;
    Ok(())
}

fn target_correlation_count(
    targets: &[SourceIsaCharacteristicTargetV1],
) -> Result<usize, SourceIsaCharacteristicErrorV1> {
    let count = targets.iter().try_fold(0usize, |count, target| {
        let additional = target.correlations.len();
        count
            .checked_add(additional)
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)
    })?;
    if count > MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1 {
        Err(SourceIsaCharacteristicErrorV1::ResourceLimit)
    } else {
        Ok(count)
    }
}

fn payload_length(
    targets: &[SourceIsaCharacteristicTargetV1],
    pre_kir_count: usize,
) -> Result<usize, SourceIsaCharacteristicErrorV1> {
    let mut length = pre_kir_count
        .checked_mul(PRE_KIR_FACT_BYTES_V1)
        .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    for target in targets {
        length = length
            .checked_add(TARGET_HEADER_BYTES_V1)
            .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        {
            for correlation in &target.correlations {
                let intervals = correlation
                    .isa_intervals
                    .len()
                    .checked_mul(ISA_INTERVAL_BYTES_V1)
                    .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
                length = length
                    .checked_add(TARGET_CORRELATION_PREFIX_BYTES_V1)
                    .and_then(|value| value.checked_add(intervals))
                    .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
            }
        }
    }
    SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1
        .checked_add(length)
        .and_then(|value| value.checked_add(SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1))
        .filter(|value| *value <= MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1)
        .ok_or(SourceIsaCharacteristicErrorV1::ResourceLimit)?;
    Ok(length)
}

fn encode_target(
    output: &mut Vec<u8>,
    target: &SourceIsaCharacteristicTargetV1,
) -> Result<(), SourceIsaCharacteristicErrorV1> {
    output.extend_from_slice(&target.identity);
    push_u16(output, target.kind.code());
    push_u8(output, 1);
    push_u8(output, 0);
    push_u16(output, 0);
    push_u16(output, 0);
    push_u32(
        output,
        u32::try_from(target.correlations.len())
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
    );
    encode_kir(output, target.target_kir);
    output.extend_from_slice(&[0; 12]);
    for correlation in &target.correlations {
        encode_target_correlation(output, correlation)?;
    }
    Ok(())
}

fn encode_target_correlation(
    output: &mut Vec<u8>,
    value: &SourceIsaCharacteristicTargetCorrelationV1,
) -> Result<(), SourceIsaCharacteristicErrorV1> {
    push_u64(output, value.catalog_record_ordinal);
    push_u8(output, value.kind as u8);
    push_u8(output, value.transformation as u8);
    let flags = match value.kind {
        SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        SourceIsaCharacteristicRecordKindV1::SourceAnchored => 0x3f_u16,
        SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => 0x38_u16,
    };
    push_u16(output, flags);
    output.extend_from_slice(&[0; 4]);
    encode_optional_source(output, value.source);
    encode_optional_node_coordinate(output, value.mir_node_identity, value.mir, encode_mir);
    encode_optional_node_coordinate(
        output,
        value.neutral_kir_node_identity,
        value.neutral_kir,
        encode_kir,
    );
    encode_kir(output, value.target_kir);
    output.extend_from_slice(&value.semantic_operation_identity);
    encode_llvm(output, value.compiler_handoff_llvm);
    push_u32(
        output,
        u32::try_from(value.isa_intervals.len())
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?,
    );
    output.extend_from_slice(&[0; 12]);
    for interval in &value.isa_intervals {
        push_u64(output, interval.kernel_ordinal);
        push_u64(output, interval.symbol_relative_start);
        push_u64(output, interval.symbol_relative_end);
    }
    Ok(())
}

fn encode_pre_kir_fact(output: &mut Vec<u8>, value: SourceIsaCharacteristicPreKirEliminationV1) {
    push_u64(output, value.catalog_record_ordinal);
    encode_source(output, value.source);
    output.extend_from_slice(&value.mir_node_identity);
    encode_mir(output, value.mir);
    output.extend_from_slice(&[0; 32]);
}

fn encode_content(output: &mut Vec<u8>, value: SourceIsaCharacteristicContentIdentityV1) {
    output.extend_from_slice(&value.sha256);
    push_u64(output, value.byte_len);
}
fn encode_source(output: &mut Vec<u8>, source: SourceIsaCharacteristicSourceCoordinateV1) {
    output.extend_from_slice(&source.node_identity);
    encode_span(output, source.span);
}
fn encode_optional_source(
    output: &mut Vec<u8>,
    source: Option<SourceIsaCharacteristicSourceCoordinateV1>,
) {
    match source {
        Some(source) => encode_source(output, source),
        None => output.extend_from_slice(&[0; 88]),
    }
}
fn encode_span(output: &mut Vec<u8>, span: SourceIsaCharacteristicSourceSpanV1) {
    output.extend_from_slice(&span.file_identity);
    push_u64(output, span.byte_start);
    push_u64(output, span.byte_end);
    push_u32(output, span.line);
    push_u32(output, span.column);
}
fn encode_optional_node_coordinate<T: Copy>(
    output: &mut Vec<u8>,
    node: Option<[u8; 32]>,
    coordinate: Option<T>,
    encode: fn(&mut Vec<u8>, T),
) {
    match (node, coordinate) {
        (Some(node), Some(coordinate)) => {
            output.extend_from_slice(&node);
            encode(output, coordinate);
        }
        _ => output.extend_from_slice(&[0; 56]),
    }
}
fn encode_mir(output: &mut Vec<u8>, value: SourceIsaCharacteristicMirCoordinateV1) {
    push_u64(output, value.body_ordinal);
    push_u64(output, value.block_ordinal);
    push_u64(output, value.statement_ordinal);
}
fn encode_kir(output: &mut Vec<u8>, value: SourceIsaCharacteristicKirCoordinateV1) {
    push_u64(output, value.function_ordinal);
    push_u64(output, value.block_ordinal);
    push_u64(output, value.operation_ordinal);
}
fn encode_llvm(
    output: &mut Vec<u8>,
    value: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
) {
    push_u64(output, value.function_ordinal);
    push_u64(output, value.block_ordinal);
    push_u64(output, value.instruction_ordinal);
}

impl SourceIsaCharacteristicCollectionV1 {
    fn decode_claimed_canonical(encoded: &[u8]) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        let minimum =
            SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1 + SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1;
        if encoded.len() < minimum
            || encoded.len() > MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1
            || encoded.get(..8) != Some(SOURCE_ISA_CHARACTERISTIC_MAGIC_V1)
        {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        let identity_start = encoded.len() - SOURCE_ISA_CHARACTERISTIC_IDENTITY_BYTES_V1;
        let retained_identity: [u8; 32] = encoded[identity_start..]
            .try_into()
            .map_err(|_| SourceIsaCharacteristicErrorV1::Malformed)?;
        if retained_identity == [0; 32]
            || retained_identity != collection_identity(&encoded[..identity_start])
        {
            return Err(SourceIsaCharacteristicErrorV1::IdentityMismatch);
        }
        let mut decoder = DecoderV1::new(&encoded[..identity_start]);
        decoder.expect(SOURCE_ISA_CHARACTERISTIC_MAGIC_V1)?;
        if decoder.u16()? != SOURCE_ISA_CHARACTERISTIC_VERSION_V1
            || usize::from(decoder.u16()?) != SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?).ok() != Some(encoded.len())
        {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        let target_count = usize::try_from(decoder.u32()?)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        let retained_target_correlations = usize::try_from(decoder.u32()?)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        let pre_count = usize::try_from(decoder.u32()?)
            .map_err(|_| SourceIsaCharacteristicErrorV1::ResourceLimit)?;
        if target_count > MAX_SOURCE_ISA_CHARACTERISTIC_TARGETS_V1
            || retained_target_correlations > MAX_SOURCE_ISA_CHARACTERISTIC_TARGET_CORRELATIONS_V1
            || pre_count > MAX_SOURCE_ISA_CHARACTERISTIC_PRE_KIR_ELIMINATIONS_V1
        {
            return Err(SourceIsaCharacteristicErrorV1::ResourceLimit);
        }
        let scan_tag = decoder.u8()?;
        decoder.zeros(1)?;
        let scan_reason = decoder.u16()?;
        let catalog_record_count = decoder.u64()?;
        let catalog_records_scanned = decoder.u64()?;
        let target_operation_count = decoder.u64()?;
        let target_operations_scanned = decoder.u64()?;
        let classified_target_count = decoder.u64()?;
        let retained_target_correlation_count = decoder.u64()?;
        let pre_kir_elimination_count = decoder.u64()?;
        let correlation_count = decoder.u64()?;
        let state = match scan_tag {
            1 if scan_reason == 0 => SourceIsaCharacteristicScanStateV1::Complete,
            2 => SourceIsaCharacteristicScanStateV1::Missing(
                SourceIsaCharacteristicMissingReasonV1::decode(scan_reason)?,
            ),
            3 => SourceIsaCharacteristicScanStateV1::Unavailable(
                SourceIsaCharacteristicUnavailableReasonV1::decode(scan_reason)?,
            ),
            4 => SourceIsaCharacteristicScanStateV1::Error(
                SourceIsaCharacteristicObservationErrorV1::decode(scan_reason)?,
            ),
            _ => return Err(SourceIsaCharacteristicErrorV1::InvalidTag),
        };
        let scan = SourceIsaCharacteristicScanSummaryV1::new(
            catalog_record_count,
            catalog_records_scanned,
            target_operation_count,
            target_operations_scanned,
            classified_target_count,
            retained_target_correlation_count,
            pre_kir_elimination_count,
            correlation_count,
            state,
        )?;
        let target_profile = SourceIsaCharacteristicTargetProfileV1::decode(decoder.u8()?)?;
        let kir_version = SourceIsaCharacteristicKirVersionV1::decode(decoder.u8()?)?;
        decoder.zeros(6)?;
        let structural_identity = decoder.identity()?;
        let structural_counts = SourceIsaCharacteristicStructuralCountsV1 {
            functions: decoder.u64()?,
            defined_bodies: decoder.u64()?,
            blocks: decoder.u64()?,
            operations: decoder.u64()?,
        };
        let binding = SourceIsaCharacteristicBindingV1::new(
            target_profile,
            kir_version,
            structural_identity,
            structural_counts,
            decoder.content()?,
            decoder.content()?,
            decoder.content()?,
            decoder.content()?,
            decoder.content()?,
            decoder.content()?,
            decoder.identity()?,
            decoder.identity()?,
        )?;
        decoder.zeros(40)?;
        if decoder.position() != SOURCE_ISA_CHARACTERISTIC_HEADER_BYTES_V1 {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(target_count)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        for _ in 0..target_count {
            targets.push(decode_target(&mut decoder)?);
        }
        let mut pre = Vec::new();
        pre.try_reserve_exact(pre_count)
            .map_err(|_| SourceIsaCharacteristicErrorV1::AllocationFailure)?;
        for _ in 0..pre_count {
            pre.push(decode_pre_kir_fact(&mut decoder)?);
        }
        if decoder.position() != identity_start {
            return Err(SourceIsaCharacteristicErrorV1::Malformed);
        }
        validate_collection_shape(binding, scan, &targets, &pre)?;
        if target_correlation_count(&targets)? != retained_target_correlations {
            return Err(SourceIsaCharacteristicErrorV1::InvalidClaim);
        }
        Ok(Self {
            binding,
            scan,
            targets,
            pre_kir_eliminations: pre,
            identity: retained_identity,
        })
    }
}

#[derive(Debug)]
pub struct InertSourceIsaCharacteristicCollectionV1 {
    claimed: SourceIsaCharacteristicCollectionV1,
}

impl InertSourceIsaCharacteristicCollectionV1 {
    pub fn decode_canonical(encoded: &[u8]) -> Result<Self, SourceIsaCharacteristicErrorV1> {
        Ok(Self {
            claimed: SourceIsaCharacteristicCollectionV1::decode_claimed_canonical(encoded)?,
        })
    }
    pub const fn claimed_identity(&self) -> [u8; 32] {
        self.claimed.identity
    }
    pub const fn claimed_binding(&self) -> SourceIsaCharacteristicBindingV1 {
        self.claimed.binding
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
    pub const fn grants_hardware_observation_authority(&self) -> bool {
        false
    }
    pub(crate) fn into_self_claimed_archive_for_agent_inspection_v1(
        self,
    ) -> SourceIsaCharacteristicCollectionV1 {
        // Canonical decoding establishes the archive's internal structure and identity only.
        // Keeping this conversion crate-private prevents a decoded archive from becoming a
        // publicly admitted producer projection; the agent protocol separately emits explicit
        // authority and archive-authentication nonclaims for every response.
        self.claimed
    }
    pub fn admit_exact_projection_v1(
        self,
        exact: &SourceIsaCharacteristicCollectionV1,
    ) -> Result<SourceIsaCharacteristicCollectionV1, SourceIsaCharacteristicErrorV1> {
        if &self.claimed != exact {
            return Err(SourceIsaCharacteristicErrorV1::IdentityMismatch);
        }
        Ok(self.claimed)
    }
}
