#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{fmt, ops::Range};

use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, COV6_IMPLICIT_ARGUMENT_BYTES,
    CodeObjectVersion as InspectedCodeObjectVersion, ExplicitArgument, ExplicitValueKind,
    ExplicitValueType, InspectedHsaco, InspectedKernel, InspectedKernelBindings,
    KernelBindingError, KernelDescriptorBinding, MAX_ELF_SECTIONS, MAX_ELF_SEGMENTS,
    MAX_HSACO_BYTES, inspect_and_bind_kernel_descriptors,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CANONICAL_CODE_OBJECT_DIGEST_OFFSET,
    CanonicalCodeObjectDigest, CodeObjectVersion, DecodeError, DeviceDescriptorTableV1,
    KernelDescriptorV1, MAX_DESCRIPTOR_TABLE_BYTES, PhysicalAbiComponentKind,
    RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1, RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1,
    ScalarTypeV1, decode_device_descriptor_table_v1,
};

mod compiler_ffi_bridge;
mod compiler_ffi_observation;
mod first_build_worker_engine;
mod first_build_worker_v3;
mod link_plan;
mod request_construction;
mod semantic_debug_map_v1;
mod worker_executor;
mod worker_protocol;
mod worker_protocol_v2;
mod worker_v3_compact_finalizer_replay;
mod worker_v3_hsaco_admission;
mod worker_v3_hsaco_finalization;
mod worker_v3_hsaco_publication;

pub use compiler_ffi_bridge::{
    ExpectedFinalDefinedSymbolsClaimIdentityV1, ExpectedFinalDefinedSymbolsClaimV1,
    FfiClaimOriginV1, FfiPlanInputClaimV1, FfiPlanInputRoleClaimV1,
    FfiSymbolProviderBindingClaimV1, FinalSymbolEvidenceSourceClaimV1,
    G4DeclarationOwnerClaimIdentityV1, G4DeclarationOwnerClaimV1, G4DeclaredContractClaimsV1,
    G4FfiClaimEnvelopeAdapterV1, G4FfiClaimEnvelopeIdentityV1, G4FfiClaimEnvelopeV1,
    G4FfiContractIdV1, G4FfiDirectionClaimV1, G4FfiSymbolClaimFieldV1, G4FfiSymbolClaimV1,
    G4SymbolProviderClassClaimV1, InputSymbolEvidenceCoverageClaimV1,
    MAX_FFI_PRODUCER_NAME_BYTES_V1, MAX_FFI_PRODUCER_VERSION_BYTES_V1,
    MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1, MAX_G4_FFI_CRATE_LABEL_BYTES_V1,
    MAX_G4_FFI_ENVELOPE_BYTES_V1, MAX_G4_FFI_INSTANCE_SYMBOL_BYTES_V1,
    MAX_G4_FFI_ITEM_LABEL_BYTES_V1, MAX_G4_FFI_SYMBOL_CLAIMS_V1, MAX_G4_KERNEL_CLAIMS_V1,
    MAX_G4_RUST_DEFINITION_CLAIMS_V1, MAX_STAGED_FFI_LINK_PLAN_BYTES_V1,
    StagedFfiExecutionBlockerV1, StagedFfiLinkError, StagedFfiLinkPlanIdentityV1,
    StagedFfiLinkPlanInspectionV1, StagedFfiLinkPlanV1, UnauthenticatedProducerClaimIdentityV1,
    UnauthenticatedProducerClaimV1, stage_g4_ffi_link_plan_v1,
};
pub use compiler_ffi_observation::{
    StagedCompilerFfiEnvelopeBlockerV1, StagedCompilerFfiEnvelopeIdentityV1,
    StagedCompilerFfiEnvelopeInspectionV1, StagedCompilerFfiEnvelopeV1,
    stage_compiler_ffi_envelope_v1,
};
pub use fe2o3_build_authority::CompilerClosureV2;
pub use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerFfiCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEffectAbiIdentityV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeError,
    CompilerFfiEnvelopeIdentityV1, CompilerFfiEnvelopeInspectionV1, CompilerFfiEnvelopeV1,
    CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerIdentityV1, CompilerFfiSourceOwnerV1,
    CompilerFfiTextFieldV1, CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestIdentityV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    DeviceTargetV1 as CompilerFfiDeviceTargetV1, MAX_COMPILER_FFI_AGGREGATE_TEXT_BYTES_V1,
    MAX_COMPILER_FFI_CONTRACTS_V1, MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1,
    MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1,
};
pub use first_build_worker_v3::{
    InertProtectedCompilerHandoffExecutionV3, InertProtectedFirstBuildWorkerV3EvidenceV1,
    PreparedProtectedFirstBuildWorkerV3PreflightV1, ProtectedCompilerHandoffBindingErrorV3,
    ProtectedCompilerHandoffBindingIdentityV3, ProtectedCompilerHandoffBindingV3,
    ProtectedCompilerHandoffExpectationV3, ProtectedFirstBuildWorkerV3Error,
    ProtectedFirstBuildWorkerV3IdentityV1,
    execute_preflighted_protected_reproducible_first_build_worker_v3,
    execute_protected_reproducible_first_build_worker_v3,
    preflight_protected_reproducible_first_build_worker_v3,
};
pub use link_plan::{
    ContentIdentityV1, LinkInputV1, LinkOptionV1, LinkOutputV1, LinkPlanError, LinkPlanIdentityV1,
    MAX_LINK_INPUTS, MAX_LINK_OPTION_NAME_BYTES, MAX_LINK_OPTION_VALUE_BYTES, MAX_LINK_OPTIONS,
    MAX_LINK_PROVENANCE_EDGES, MAX_LINK_PROVENANCE_NODES, MultiInputLinkPlanV1, ProvenanceNodeV1,
};
pub use request_construction::{
    LinkInputKindClosureIdentityV1, LinkInputKindClosureV1, LinkSymbolClosureIdentityV1,
    LinkSymbolClosureV1, WorkerRequestConstructionError,
};
pub use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
    derive_device_ffi_contract_id_v1,
};
pub use semantic_debug_map_v1::{
    AdmittedFinalizedSemanticDebugMapV1, FinalizedSemanticDebugMapAdmissionStatusV1,
    FinalizedSemanticDebugMapErrorV1, FinalizedSemanticDebugMapIdentityV1,
    ProductionFinalizedSemanticDebugAdmissionV1, admit_finalized_semantic_debug_map_v1,
    admit_finalized_semantic_debug_map_with_inputs_v1,
};
pub use worker_executor::{
    DEFAULT_WORKER_STDERR_BYTES, DEFAULT_WORKER_TIMEOUT, MAX_WORKER_EXECUTABLE_BYTES,
    MAX_WORKER_STDERR_BYTES, MAX_WORKER_TIMEOUT, PinnedWorkerV1, WORKER_ENVIRONMENT_ALLOWLIST_V1,
    WorkerExecutionError, WorkerExecutionErrorKind, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerTerminationV1,
};
pub use worker_protocol::{
    MAX_WORKER_DIAGNOSTIC_BYTES, MAX_WORKER_DIAGNOSTICS, MAX_WORKER_OUTPUT_BYTES,
    MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES, MAX_WORKER_SYMBOL_BYTES,
    MAX_WORKER_SYMBOLS, MAX_WORKER_TARGET_BYTES, MAX_WORKER_TOOLCHAIN_ID_BYTES,
    MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES, MAX_WORKER_TOTAL_INPUT_BYTES, WorkerInputKindV1,
    WorkerInputV1, WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, WorkerStageV1,
};
pub use worker_protocol_v2::{
    InertDecodedWorkerExchangeV2, WORKER_REQUEST_MAGIC_V2, WORKER_RESPONSE_MAGIC_V2,
    WORKER_RESPONSE_MAGIC_V3, WORKER_RESPONSE_MAGIC_V4, WorkerCompilerFfiEnvelopeIdentityV2,
    WorkerDerivationEvidenceV1, WorkerDeviceLibraryProviderEvidenceV1,
    WorkerDeviceLibraryProviderFileEvidenceV1, WorkerEvidenceClassV2,
    WorkerNativeLinkInputEvidenceV1, WorkerNativeLinkInputSourceV1, WorkerOutputV2,
    WorkerRequestV2, WorkerResponseV2,
};
pub use worker_v3_compact_finalizer_replay::{
    MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1,
    PreparedProtectedWorkerV3CompactFinalizerReplayV1,
    PreparedProtectedWorkerV3CompactFinalizerReplayV2,
    ProtectedWorkerV3CompactFinalizerReplayErrorV1,
    ProtectedWorkerV3CompactFinalizerReplayIdentityV1,
    ProtectedWorkerV3CompactFinalizerReplayIdentityV2,
    ProtectedWorkerV3CompactFinalizerReplayPartsV1, ProtectedWorkerV3CompactFinalizerReplayPartsV2,
    ProtectedWorkerV3CompactFinalizerReplayV1, ProtectedWorkerV3CompactFinalizerReplayV2,
    prepare_protected_worker_v3_compact_finalizer_replay_v1,
    prepare_protected_worker_v3_compact_finalizer_replay_v2,
};
pub use worker_v3_hsaco_admission::{
    CanonicalDescriptorSectionObservationV1, InspectedProtectedWorkerV3HsacoIdentityV1,
    InspectedProtectedWorkerV3HsacoV1, ObservedWorkerKernelSymbolsV1,
    SealedWorkerResponseIdentityV1, WorkerV3HsacoInspectionError, WorkerV3HsacoPolicyIdentityV1,
    WorkerV3HsacoPolicyV1, inspect_protected_worker_v3_hsaco_v1,
};
pub use worker_v3_hsaco_finalization::{
    DescriptorSourceEvidenceRequirementV1, FinalizedProtectedWorkerV3HsacoIdentityV1,
    MissingAuthenticatedProtectedDescriptorSourceEvidenceV3,
    PreparedFinalizedProtectedWorkerV3HsacoV1, WorkerV3HsacoFinalizationError,
    finalize_protected_worker_v3_hsaco_v1,
};
pub use worker_v3_hsaco_publication::{
    PreparedProtectedWorkerV3HsacoPublicationV1, PublishedProtectedWorkerV3HsacoV1,
    PublishedProtectedWorkerV3LoadEnvelopePartsV1, RecoveredProtectedWorkerV3HsacoPublicationV1,
    RevalidatedProtectedWorkerV3FinalizerDerivationIdentityV1,
    RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    SealedProtectedWorkerV3HsacoPublicationIntentV1, WorkerV3HsacoPublicationErrorV1,
    persist_prepared_protected_worker_v3_hsaco_publication_v1,
    prepare_protected_worker_v3_hsaco_publication_v1,
    publish_recovered_protected_worker_v3_hsaco_v1,
    recover_protected_worker_v3_hsaco_publication_v1,
    revalidate_protected_worker_v3_finalizer_derivation_v1,
};

/// The only ELF section name recognized for a canonical V1 descriptor table.
pub const DEVICE_DESCRIPTOR_SECTION_NAME: &str = ".fe2o3.kd.v1";
/// The exact required alignment of the descriptor section and its file offset.
pub const DEVICE_DESCRIPTOR_SECTION_ALIGNMENT: u64 = 8;

const ELF64_HEADER_BYTES: usize = 64;
const ELF64_PROGRAM_HEADER_BYTES: usize = 56;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const ELF64_SECTION_NAME_OFFSET: usize = 0;
const ELF64_SECTION_TYPE_OFFSET: usize = 4;
const ELF64_SECTION_FLAGS_OFFSET: usize = 8;
const ELF64_SECTION_FILE_OFFSET: usize = 24;
const ELF64_SECTION_SIZE_OFFSET: usize = 32;
const ELF64_SECTION_ALIGNMENT_OFFSET: usize = 48;
const ELF64_PROGRAM_TYPE_OFFSET: usize = 0;
const ELF64_PROGRAM_FLAGS_OFFSET: usize = 4;
const ELF64_PROGRAM_FILE_OFFSET: usize = 8;
const ELF64_PROGRAM_FILE_SIZE_OFFSET: usize = 32;
const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;
const SHT_NOBITS: u32 = 8;
const PT_LOAD: u32 = 1;
const PF_R: u32 = 4;
const SHF_ALLOC: u64 = 2;
const GENERAL_V3_COV6_PRODUCER_VERSION_V1: &str = "typed-general-gfx942-cov6-v1";
const GENERAL_V3_COV6_DEVICE_TARGET_V1: &str = "gfx942:xnack-";

/// Failure while locating, checking, finalizing, or rechecking a descriptor table.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FinalizationError {
    InputTooLarge,
    AllocationFailed,
    HsacoBinding(KernelBindingError),
    InvalidElf(&'static str),
    MissingDescriptorSection,
    DuplicateDescriptorSection,
    InvalidDescriptorSectionType,
    InvalidDescriptorSectionFlags(u64),
    InvalidDescriptorSectionAlignment,
    DescriptorSectionOutOfBounds,
    DescriptorSectionOverlaps(&'static str),
    DescriptorTableTooLarge,
    DescriptorDecode(DecodeError),
    ExpectedZeroDigest,
    ExpectedFinalizedDigest,
    CodeObjectVersionMismatch,
    DeviceTargetMismatch,
    DescriptorKernelMissingInMetadata {
        entry_name: String,
    },
    MetadataKernelMissingInDescriptor {
        entry_name: String,
    },
    KernelDescriptorSymbolMismatch {
        entry_name: String,
    },
    KernelBindingClosureMismatch {
        entry_name: String,
    },
    KernargSegmentSizeMismatch {
        entry_name: String,
        descriptor: u32,
        metadata: u64,
    },
    KernargSegmentAlignmentMismatch {
        entry_name: String,
        descriptor: u32,
        metadata: u64,
    },
    ExplicitArgumentCountMismatch {
        entry_name: String,
        descriptor: usize,
        metadata: usize,
    },
    PhysicalArgumentMismatch {
        entry_name: String,
        index: usize,
        field: &'static str,
    },
    ImplicitArgumentBoundaryMismatch {
        entry_name: String,
    },
    StaticGroupSegmentSizeMismatch {
        entry_name: String,
        descriptor: u32,
        metadata: u64,
    },
    MaxFlatWorkgroupSizeMismatch {
        entry_name: String,
        descriptor: u32,
        metadata: u32,
    },
    RequiredWorkgroupSizeMismatch {
        entry_name: String,
    },
    MaxWorkgroupsMismatch {
        entry_name: String,
        axis: usize,
        descriptor: u32,
        metadata: u32,
    },
    CanonicalDigestMismatch {
        declared: CanonicalCodeObjectDigest,
        calculated: CanonicalCodeObjectDigest,
    },
    OutputVerification(&'static str),
}

impl fmt::Display for FinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => formatter.write_str("HSACO exceeds the input size limit"),
            Self::AllocationFailed => formatter.write_str("HSACO replay buffer allocation failed"),
            Self::HsacoBinding(error) => write!(formatter, "HSACO binding failed: {error}"),
            Self::InvalidElf(reason) => write!(formatter, "invalid ELF section layout: {reason}"),
            Self::MissingDescriptorSection => write!(
                formatter,
                "ELF section {DEVICE_DESCRIPTOR_SECTION_NAME} is missing"
            ),
            Self::DuplicateDescriptorSection => write!(
                formatter,
                "ELF contains multiple {DEVICE_DESCRIPTOR_SECTION_NAME} sections"
            ),
            Self::InvalidDescriptorSectionType => {
                formatter.write_str("descriptor section must be SHT_PROGBITS")
            }
            Self::InvalidDescriptorSectionFlags(flags) => write!(
                formatter,
                "descriptor section must have no ELF flags, found {flags:#x}"
            ),
            Self::InvalidDescriptorSectionAlignment => write!(
                formatter,
                "descriptor section must have alignment {DEVICE_DESCRIPTOR_SECTION_ALIGNMENT}"
            ),
            Self::DescriptorSectionOutOfBounds => {
                formatter.write_str("descriptor section file range is invalid")
            }
            Self::DescriptorSectionOverlaps(region) => {
                write!(formatter, "descriptor section overlaps {region}")
            }
            Self::DescriptorTableTooLarge => write!(
                formatter,
                "descriptor table exceeds {MAX_DESCRIPTOR_TABLE_BYTES} bytes"
            ),
            Self::DescriptorDecode(error) => {
                write!(formatter, "descriptor table is invalid: {error}")
            }
            Self::ExpectedZeroDigest => {
                formatter.write_str("unfinalized descriptor table has a nonzero code-object digest")
            }
            Self::ExpectedFinalizedDigest => {
                formatter.write_str("finalized descriptor table has a zero code-object digest")
            }
            Self::CodeObjectVersionMismatch => {
                formatter.write_str("descriptor and HSACO code-object versions do not match")
            }
            Self::DeviceTargetMismatch => {
                formatter.write_str("descriptor and HSACO targets do not match")
            }
            Self::DescriptorKernelMissingInMetadata { entry_name } => write!(
                formatter,
                "descriptor kernel {entry_name} is missing from HSACO metadata"
            ),
            Self::MetadataKernelMissingInDescriptor { entry_name } => write!(
                formatter,
                "metadata kernel {entry_name} is missing from the descriptor table"
            ),
            Self::KernelDescriptorSymbolMismatch { entry_name } => write!(
                formatter,
                "descriptor symbol for kernel {entry_name} does not match metadata"
            ),
            Self::KernelBindingClosureMismatch { entry_name } => write!(
                formatter,
                "kernel {entry_name} has no exact metadata-to-ELF descriptor binding"
            ),
            Self::KernargSegmentSizeMismatch {
                entry_name,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor kernarg size {descriptor} for {entry_name} does not match metadata size {metadata}"
            ),
            Self::KernargSegmentAlignmentMismatch {
                entry_name,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor kernarg alignment {descriptor} for {entry_name} does not match metadata alignment {metadata}"
            ),
            Self::ExplicitArgumentCountMismatch {
                entry_name,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor physical argument count {descriptor} for {entry_name} does not match metadata count {metadata}"
            ),
            Self::PhysicalArgumentMismatch {
                entry_name,
                index,
                field,
            } => write!(
                formatter,
                "descriptor physical argument {index} for {entry_name} disagrees with metadata field {field}"
            ),
            Self::ImplicitArgumentBoundaryMismatch { entry_name } => write!(
                formatter,
                "explicit/implicit argument boundary for {entry_name} is inconsistent"
            ),
            Self::StaticGroupSegmentSizeMismatch {
                entry_name,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor static group size {descriptor} for {entry_name} does not match metadata size {metadata}"
            ),
            Self::MaxFlatWorkgroupSizeMismatch {
                entry_name,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor maximum flat workgroup size {descriptor} for {entry_name} does not match metadata value {metadata}"
            ),
            Self::RequiredWorkgroupSizeMismatch { entry_name } => write!(
                formatter,
                "descriptor block-size constraint for {entry_name} does not match metadata"
            ),
            Self::MaxWorkgroupsMismatch {
                entry_name,
                axis,
                descriptor,
                metadata,
            } => write!(
                formatter,
                "descriptor maximum workgroups axis {axis} value {descriptor} for {entry_name} does not match metadata value {metadata}"
            ),
            Self::CanonicalDigestMismatch {
                declared,
                calculated,
            } => write!(
                formatter,
                "declared canonical digest {:02x?} does not match calculated digest {:02x?}",
                declared.as_bytes(),
                calculated.as_bytes()
            ),
            Self::OutputVerification(reason) => {
                write!(
                    formatter,
                    "finalized output failed independent reinspection: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for FinalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HsacoBinding(error) => Some(error),
            Self::DescriptorDecode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<KernelBindingError> for FinalizationError {
    fn from(value: KernelBindingError) -> Self {
        if value == KernelBindingError::Inspection(fe2o3_hsaco::InspectionError::InputTooLarge) {
            Self::InputTooLarge
        } else {
            Self::HsacoBinding(value)
        }
    }
}

impl From<DecodeError> for FinalizationError {
    fn from(value: DecodeError) -> Self {
        Self::DescriptorDecode(value)
    }
}

/// File location of one canonical descriptor table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorSectionLocation {
    offset: usize,
    size: usize,
    digest_offset: usize,
}

impl DescriptorSectionLocation {
    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn size(self) -> usize {
        self.size
    }

    pub const fn digest_offset(self) -> usize {
        self.digest_offset
    }
}

/// A bounded description of an unfinalized embedded table.
///
/// This value carries no module-loading, launch, compiler-evidence, or Verus authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnfinalizedDescriptorInspection {
    bindings: InspectedKernelBindings,
    table: DeviceDescriptorTableV1,
    location: DescriptorSectionLocation,
}

impl UnfinalizedDescriptorInspection {
    pub fn hsaco(&self) -> &InspectedHsaco {
        self.bindings.inspection()
    }

    pub const fn kernel_bindings(&self) -> &InspectedKernelBindings {
        &self.bindings
    }

    pub const fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        &self.table
    }

    pub const fn location(&self) -> DescriptorSectionLocation {
        self.location
    }

    /// Finalization is descriptive integrity checking, never launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// A bounded description of a finalized embedded table and its matching digest.
///
/// This value carries no module-loading, launch, compiler-evidence, or Verus authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedDescriptorInspection {
    bindings: InspectedKernelBindings,
    table: DeviceDescriptorTableV1,
    location: DescriptorSectionLocation,
    digest: CanonicalCodeObjectDigest,
}

impl FinalizedDescriptorInspection {
    pub fn hsaco(&self) -> &InspectedHsaco {
        self.bindings.inspection()
    }

    pub const fn kernel_bindings(&self) -> &InspectedKernelBindings {
        &self.bindings
    }

    pub const fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        &self.table
    }

    pub const fn location(&self) -> DescriptorSectionLocation {
        self.location
    }

    pub const fn digest(&self) -> CanonicalCodeObjectDigest {
        self.digest
    }

    /// Digest agreement is descriptive integrity checking, never launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Owned finalized bytes plus their independently reinspected description.
///
/// The bounded full-file clone is deliberate in V1. This value is not a load or launch token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedHsaco {
    bytes: Vec<u8>,
    inspection: FinalizedDescriptorInspection,
}

impl FinalizedHsaco {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub const fn inspection(&self) -> &FinalizedDescriptorInspection {
        &self.inspection
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Locates and validates an embedded canonical table whose digest slot is all zero.
pub fn inspect_unfinalized(
    bytes: &[u8],
) -> Result<UnfinalizedDescriptorInspection, FinalizationError> {
    inspect_unfinalized_with_placement(bytes, DescriptorPlacementV1::Detached)
}

fn inspect_unfinalized_with_placement(
    bytes: &[u8],
    placement: DescriptorPlacementV1,
) -> Result<UnfinalizedDescriptorInspection, FinalizationError> {
    let parsed = inspect_embedded_table_with_placement(bytes, placement)?;
    if parsed.table.canonical_code_object_digest().as_bytes() != &[0; 32] {
        return Err(FinalizationError::ExpectedZeroDigest);
    }
    Ok(UnfinalizedDescriptorInspection {
        bindings: parsed.bindings,
        table: parsed.table,
        location: parsed.location,
    })
}

/// Finalizes one already-embedded table by patching only its fixed digest field.
///
/// The output is independently reparsed, reinspected, decoded, and rehashed before return.
pub fn finalize_unfinalized(bytes: &[u8]) -> Result<FinalizedHsaco, FinalizationError> {
    finalize_unfinalized_with_placement(bytes, DescriptorPlacementV1::Detached)
}

pub(crate) fn finalize_allocated_read_only_unfinalized(
    bytes: &[u8],
) -> Result<FinalizedHsaco, FinalizationError> {
    finalize_unfinalized_with_placement(bytes, DescriptorPlacementV1::AllocatedReadOnly)
}

fn finalize_unfinalized_with_placement(
    bytes: &[u8],
    placement: DescriptorPlacementV1,
) -> Result<FinalizedHsaco, FinalizationError> {
    let unfinalized = inspect_unfinalized_with_placement(bytes, placement)?;
    let digest = CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(bytes);
    let digest_start = unfinalized.location.digest_offset;
    let digest_end = digest_start
        .checked_add(digest.as_bytes().len())
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?;

    let mut output = bytes.to_vec();
    output
        .get_mut(digest_start..digest_end)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?
        .copy_from_slice(digest.as_bytes());

    for (index, (before, after)) in bytes.iter().zip(&output).enumerate() {
        if before != after && !(digest_start..digest_end).contains(&index) {
            return Err(FinalizationError::OutputVerification(
                "a byte outside the canonical digest field changed",
            ));
        }
    }

    let inspection = inspect_finalized_with_placement(&output, placement)?;
    if inspection.digest != digest {
        return Err(FinalizationError::OutputVerification(
            "reinspected digest differs from the patched digest",
        ));
    }
    Ok(FinalizedHsaco {
        bytes: output,
        inspection,
    })
}

/// Reparses and verifies one finalized table against the canonical whole-HSACO digest.
///
/// Verification here establishes byte integrity only. It grants no load or launch authority.
pub fn inspect_finalized(bytes: &[u8]) -> Result<FinalizedDescriptorInspection, FinalizationError> {
    inspect_finalized_with_placement(bytes, DescriptorPlacementV1::Detached)
}

fn inspect_finalized_with_placement(
    bytes: &[u8],
    placement: DescriptorPlacementV1,
) -> Result<FinalizedDescriptorInspection, FinalizationError> {
    let parsed = inspect_embedded_table_with_placement(bytes, placement)?;
    let declared = parsed.table.canonical_code_object_digest();
    if declared.as_bytes() == &[0; 32] {
        return Err(FinalizationError::ExpectedFinalizedDigest);
    }

    let mut canonicalized = bytes.to_vec();
    let digest_start = parsed.location.digest_offset;
    let digest_end = digest_start
        .checked_add(32)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?;
    canonicalized
        .get_mut(digest_start..digest_end)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?
        .fill(0);
    let calculated = CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(&canonicalized);
    if declared != calculated {
        return Err(FinalizationError::CanonicalDigestMismatch {
            declared,
            calculated,
        });
    }

    Ok(FinalizedDescriptorInspection {
        bindings: parsed.bindings,
        table: parsed.table,
        location: parsed.location,
        digest: calculated,
    })
}

/// Verifies a finalized table and returns the same bounded descriptive result as inspection.
///
/// This spelling emphasizes an integrity check; it still grants no load or launch authority.
pub fn verify_finalized(bytes: &[u8]) -> Result<FinalizedDescriptorInspection, FinalizationError> {
    inspect_finalized(bytes)
}

/// Reconstructs the exact pre-finalization HSACO from one canonical finalized HSACO.
///
/// Canonical finalization changes only the fixed 32-byte digest field. This operation first
/// verifies the finalized artifact, fallibly allocates one output buffer, clears that field, and
/// independently reinspects the result as unfinalized HSACO. The returned bytes are inert and
/// grant no publication, loading, or launch authority.
pub fn derive_unfinalized_hsaco_from_finalized_v1(
    finalized_bytes: &[u8],
) -> Result<Vec<u8>, FinalizationError> {
    let finalized = verify_finalized(finalized_bytes)?;
    let digest_start = finalized.location().digest_offset();
    let digest_end = digest_start
        .checked_add(32)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?;

    let mut raw = Vec::new();
    raw.try_reserve_exact(finalized_bytes.len())
        .map_err(|_| FinalizationError::AllocationFailed)?;
    raw.extend_from_slice(finalized_bytes);
    raw.get_mut(digest_start..digest_end)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?
        .fill(0);

    let unfinalized = inspect_unfinalized(&raw)?;
    if unfinalized.location() != finalized.location()
        || CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(&raw) != finalized.digest()
    {
        return Err(FinalizationError::OutputVerification(
            "derived raw HSACO does not reproduce the finalized digest",
        ));
    }
    Ok(raw)
}

pub(crate) fn verify_allocated_read_only_finalized(
    bytes: &[u8],
) -> Result<FinalizedDescriptorInspection, FinalizationError> {
    inspect_finalized_with_placement(bytes, DescriptorPlacementV1::AllocatedReadOnly)
}

struct ParsedEmbeddedTable {
    bindings: InspectedKernelBindings,
    table: DeviceDescriptorTableV1,
    location: DescriptorSectionLocation,
}

fn inspect_embedded_table_with_placement(
    bytes: &[u8],
    placement: DescriptorPlacementV1,
) -> Result<ParsedEmbeddedTable, FinalizationError> {
    if bytes.len() > MAX_HSACO_BYTES {
        return Err(FinalizationError::InputTooLarge);
    }
    let bindings = inspect_and_bind_kernel_descriptors(bytes)?;
    let section = locate_descriptor_section_with_placement(bytes, placement)?;
    if section.range.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(FinalizationError::DescriptorTableTooLarge);
    }
    let table_bytes = bytes
        .get(section.range.clone())
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?;
    let table = decode_device_descriptor_table_v1(table_bytes)?;
    cross_check(&bindings, &table)?;
    let digest_offset = section
        .range
        .start
        .checked_add(CANONICAL_CODE_OBJECT_DIGEST_OFFSET)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)?;
    Ok(ParsedEmbeddedTable {
        bindings,
        table,
        location: DescriptorSectionLocation {
            offset: section.range.start,
            size: section.range.len(),
            digest_offset,
        },
    })
}

fn cross_check(
    bindings: &InspectedKernelBindings,
    table: &DeviceDescriptorTableV1,
) -> Result<(), FinalizationError> {
    let hsaco = bindings.inspection();
    let versions_match = matches!(
        (table.code_object_version(), hsaco.code_object_version()),
        (CodeObjectVersion::V4, InspectedCodeObjectVersion::V4)
            | (CodeObjectVersion::V5, InspectedCodeObjectVersion::V5)
            | (CodeObjectVersion::V6, InspectedCodeObjectVersion::V6)
    );
    if !versions_match {
        return Err(FinalizationError::CodeObjectVersionMismatch);
    }
    if table.device_target().as_amd_target_id() != hsaco.target() {
        return Err(FinalizationError::DeviceTargetMismatch);
    }
    for descriptor in table.kernels() {
        let entry_name = descriptor.entry_name().as_str();
        let Some(metadata) = hsaco
            .kernels()
            .iter()
            .find(|kernel| kernel.name() == entry_name)
        else {
            return Err(FinalizationError::DescriptorKernelMissingInMetadata {
                entry_name: entry_name.to_owned(),
            });
        };
        if descriptor.descriptor_symbol().as_str() != metadata.symbol() {
            return Err(FinalizationError::KernelDescriptorSymbolMismatch {
                entry_name: entry_name.to_owned(),
            });
        }
        let metadata_index = hsaco
            .kernels()
            .iter()
            .position(|kernel| kernel.name() == entry_name)
            .ok_or_else(|| FinalizationError::DescriptorKernelMissingInMetadata {
                entry_name: entry_name.to_owned(),
            })?;
        let binding = bindings
            .bindings()
            .iter()
            .find(|binding| binding.kernel_index() == metadata_index)
            .ok_or_else(|| FinalizationError::KernelBindingClosureMismatch {
                entry_name: entry_name.to_owned(),
            })?;
        validate_kernel_physical_abi(table, descriptor, metadata, *binding)?;
    }
    for metadata in hsaco.kernels() {
        if !table
            .kernels()
            .iter()
            .any(|descriptor| descriptor.entry_name().as_str() == metadata.name())
        {
            return Err(FinalizationError::MetadataKernelMissingInDescriptor {
                entry_name: metadata.name().to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_kernel_physical_abi(
    table: &DeviceDescriptorTableV1,
    descriptor: &KernelDescriptorV1,
    metadata: &InspectedKernel,
    binding: KernelDescriptorBinding,
) -> Result<(), FinalizationError> {
    let entry_name = descriptor.entry_name().as_str();
    let layout = descriptor.abi_layout();
    let descriptor_size = layout.kernarg_segment_size();
    let bound_descriptor = binding.descriptor();
    if !kernarg_segment_sizes_match_v1(table, descriptor, metadata, bound_descriptor.kernarg_size())
    {
        return Err(FinalizationError::KernargSegmentSizeMismatch {
            entry_name: entry_name.to_owned(),
            descriptor: descriptor_size,
            metadata: metadata.kernarg_segment_size(),
        });
    }
    if u64::from(layout.kernarg_segment_alignment()) != metadata.kernarg_segment_alignment() {
        return Err(FinalizationError::KernargSegmentAlignmentMismatch {
            entry_name: entry_name.to_owned(),
            descriptor: layout.kernarg_segment_alignment(),
            metadata: metadata.kernarg_segment_alignment(),
        });
    }

    let physical_count = descriptor
        .arguments()
        .iter()
        .map(|argument| argument.physical_components().len())
        .sum::<usize>();
    if physical_count != metadata.explicit_arguments().len() {
        return Err(FinalizationError::ExplicitArgumentCountMismatch {
            entry_name: entry_name.to_owned(),
            descriptor: physical_count,
            metadata: metadata.explicit_arguments().len(),
        });
    }
    let mut physical_index = 0usize;
    for logical in descriptor.arguments() {
        let element_scalar = table
            .type_records()
            .iter()
            .find(|record| record.identity() == logical.source_type())
            .map(|record| record.descriptor().scalar_type())
            .ok_or_else(|| FinalizationError::PhysicalArgumentMismatch {
                entry_name: entry_name.to_owned(),
                index: physical_index,
                field: "source type identity",
            })?;
        for (kind, offset, size, alignment) in logical.physical_components() {
            let argument = &metadata.explicit_arguments()[physical_index];
            validate_physical_argument(
                entry_name,
                physical_index,
                kind,
                logical.access(),
                logical.alias(),
                offset,
                size,
                alignment,
                element_scalar,
                argument,
            )?;
            physical_index += 1;
        }
    }

    let explicit_size = u64::from(layout.explicit_argument_size());
    let expected_implicit_offset = align_up(explicit_size, 8)?;
    if let Some(implicit_offset) = metadata.implicit_argument_offset() {
        let expected_implicit_size = metadata
            .kernarg_segment_size()
            .checked_sub(expected_implicit_offset)
            .ok_or_else(|| FinalizationError::ImplicitArgumentBoundaryMismatch {
                entry_name: entry_name.to_owned(),
            })?;
        if implicit_offset != expected_implicit_offset
            || metadata.implicit_argument_size() != expected_implicit_size
        {
            return Err(FinalizationError::ImplicitArgumentBoundaryMismatch {
                entry_name: entry_name.to_owned(),
            });
        }
    } else if !metadata.hidden_arguments().is_empty()
        || metadata.implicit_argument_size() != 0
        || metadata.kernarg_segment_size() != align_up(explicit_size, 4)?
    {
        return Err(FinalizationError::ImplicitArgumentBoundaryMismatch {
            entry_name: entry_name.to_owned(),
        });
    }

    let static_group = descriptor.launch().static_shared_memory_bytes();
    if u64::from(static_group) != metadata.group_segment_fixed_size()
        || static_group != bound_descriptor.group_segment_fixed_size()
    {
        return Err(FinalizationError::StaticGroupSegmentSizeMismatch {
            entry_name: entry_name.to_owned(),
            descriptor: static_group,
            metadata: metadata.group_segment_fixed_size(),
        });
    }
    let max_flat = descriptor.launch().max_flat_workgroup_size();
    if max_flat != metadata.max_flat_workgroup_size() {
        return Err(FinalizationError::MaxFlatWorkgroupSizeMismatch {
            entry_name: entry_name.to_owned(),
            descriptor: max_flat,
            metadata: metadata.max_flat_workgroup_size(),
        });
    }
    let expected_required = match descriptor.launch().block_size() {
        BlockSizeV1::Any | BlockSizeV1::AtMost(_) => None,
        BlockSizeV1::Exact(dimensions) => Some([dimensions.x(), dimensions.y(), dimensions.z()]),
    };
    if expected_required != metadata.required_workgroup_size() {
        return Err(FinalizationError::RequiredWorkgroupSizeMismatch {
            entry_name: entry_name.to_owned(),
        });
    }
    let max_grid = descriptor.launch().max_grid();
    let descriptor_max = [max_grid.x(), max_grid.y(), max_grid.z()];
    for (axis, (declared, observed)) in descriptor_max
        .into_iter()
        .zip(metadata.max_workgroups())
        .enumerate()
    {
        if let Some(observed) = observed
            && declared != observed
        {
            return Err(FinalizationError::MaxWorkgroupsMismatch {
                entry_name: entry_name.to_owned(),
                axis,
                descriptor: declared,
                metadata: observed,
            });
        }
    }
    Ok(())
}

fn kernarg_segment_sizes_match_v1(
    table: &DeviceDescriptorTableV1,
    descriptor: &KernelDescriptorV1,
    metadata: &InspectedKernel,
    bound_descriptor_size: u32,
) -> bool {
    let layout = descriptor.abi_layout();
    let descriptor_total = u64::from(layout.kernarg_segment_size());
    let metadata_size = metadata.kernarg_segment_size();
    let bound_descriptor_size = u64::from(bound_descriptor_size);

    if is_general_v3_cov6_profile_v1(table) {
        if metadata.hidden_arguments().is_empty() {
            return general_v3_cov6_implicit_span_is_canonical_v1(
                metadata.implicit_argument_size(),
                false,
            ) && metadata.implicit_argument_offset().is_none()
                && metadata_size == u64::from(layout.explicit_argument_size())
                && bound_descriptor_size == metadata_size
                && general_v3_cov6_total_kernarg_size_v1(metadata_size) == Some(descriptor_total);
        }
        return general_v3_cov6_implicit_span_is_canonical_v1(
            metadata.implicit_argument_size(),
            true,
        ) && descriptor_total == metadata_size
            && descriptor_total == bound_descriptor_size;
    }

    descriptor_total == metadata_size && descriptor_total == bound_descriptor_size
}

fn is_general_v3_cov6_profile_v1(table: &DeviceDescriptorTableV1) -> bool {
    table.code_object_version() == CodeObjectVersion::V6
        && table.device_target().to_string() == GENERAL_V3_COV6_DEVICE_TARGET_V1
        && table.compiler().name().as_str() == RUSTC_CODEGEN_FE2O3_COMPILER_NAME_V1
        && table.producer().name().as_str() == RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1
        && table.producer().version().as_str() == GENERAL_V3_COV6_PRODUCER_VERSION_V1
}

fn general_v3_cov6_total_kernarg_size_v1(explicit_size: u64) -> Option<u64> {
    explicit_size.checked_add(COV6_IMPLICIT_ARGUMENT_BYTES)
}

const fn general_v3_cov6_implicit_span_is_canonical_v1(
    implicit_size: u64,
    has_hidden_records: bool,
) -> bool {
    if has_hidden_records {
        implicit_size == COV6_IMPLICIT_ARGUMENT_BYTES
    } else {
        implicit_size == 0
    }
}

#[cfg(test)]
mod kernarg_reconciliation_tests {
    use super::{
        general_v3_cov6_implicit_span_is_canonical_v1, general_v3_cov6_total_kernarg_size_v1,
    };

    #[test]
    fn general_v3_cov6_total_size_rejects_overflow() {
        assert_eq!(general_v3_cov6_total_kernarg_size_v1(u64::MAX), None);
        assert_eq!(general_v3_cov6_total_kernarg_size_v1(u64::MAX - 255), None);
        assert_eq!(
            general_v3_cov6_total_kernarg_size_v1(u64::MAX - 256),
            Some(u64::MAX)
        );
    }

    #[test]
    fn general_v3_cov6_span_is_exact_or_explicit_only_legacy() {
        assert!(general_v3_cov6_implicit_span_is_canonical_v1(0, false));
        assert!(!general_v3_cov6_implicit_span_is_canonical_v1(0, true));
        for size in [68, 255, 257, u64::MAX] {
            assert!(!general_v3_cov6_implicit_span_is_canonical_v1(size, true));
        }
        assert!(general_v3_cov6_implicit_span_is_canonical_v1(256, true));
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_physical_argument(
    entry_name: &str,
    index: usize,
    kind: PhysicalAbiComponentKind,
    access: AccessMode,
    alias: AliasSemantics,
    offset: u32,
    size: u16,
    alignment: u16,
    element_scalar: ScalarTypeV1,
    metadata: &ExplicitArgument,
) -> Result<(), FinalizationError> {
    let mismatch = |field| FinalizationError::PhysicalArgumentMismatch {
        entry_name: entry_name.to_owned(),
        index,
        field,
    };
    if u64::from(offset) != metadata.offset() {
        return Err(mismatch(".offset"));
    }
    if u64::from(size) != metadata.size() {
        return Err(mismatch(".size"));
    }
    if metadata
        .alignment()
        .is_some_and(|value| value != u64::from(alignment))
    {
        return Err(mismatch(".align"));
    }
    let expected_value_type = match kind {
        PhysicalAbiComponentKind::ScalarByValue(scalar) => physical_value_type(scalar),
        PhysicalAbiComponentKind::GlobalPointer => physical_value_type(element_scalar),
        PhysicalAbiComponentKind::SliceLengthU64 => ExplicitValueType::U64,
    };
    if metadata
        .value_type()
        .is_some_and(|value| value != expected_value_type)
    {
        return Err(mismatch(".value_type"));
    }
    match kind {
        PhysicalAbiComponentKind::GlobalPointer => {
            if metadata.value_kind() != ExplicitValueKind::GlobalBuffer {
                return Err(mismatch(".value_kind"));
            }
            if metadata.address_space() != Some(ArgumentAddressSpace::Global) {
                return Err(mismatch(".address_space"));
            }
            if metadata
                .pointee_alignment()
                .is_some_and(|value| value != u64::from(element_scalar.alignment_bytes()))
            {
                return Err(mismatch(".pointee_align"));
            }
            let expected_access = match access {
                AccessMode::ReadOnly => ArgumentAccess::ReadOnly,
                AccessMode::WriteOnly => ArgumentAccess::WriteOnly,
                AccessMode::ReadWrite => ArgumentAccess::ReadWrite,
                AccessMode::ByValue => return Err(mismatch("descriptor access")),
            };
            if metadata
                .access()
                .is_some_and(|value| value != expected_access)
            {
                return Err(mismatch(".access"));
            }
            if metadata
                .actual_access()
                .is_some_and(|value| !actual_access_is_subset(expected_access, value))
            {
                return Err(mismatch(".actual_access"));
            }
            if metadata
                .is_restrict()
                .is_some_and(|value| value != (alias == AliasSemantics::Exclusive))
            {
                return Err(mismatch(".is_restrict"));
            }
            if metadata
                .is_const()
                .is_some_and(|value| value != (access == AccessMode::ReadOnly))
            {
                return Err(mismatch(".is_const"));
            }
        }
        PhysicalAbiComponentKind::ScalarByValue(_) | PhysicalAbiComponentKind::SliceLengthU64 => {
            if metadata.value_kind() != ExplicitValueKind::ByValue {
                return Err(mismatch(".value_kind"));
            }
            if metadata.address_space().is_some() {
                return Err(mismatch(".address_space"));
            }
            if metadata.pointee_alignment().is_some() {
                return Err(mismatch(".pointee_align"));
            }
            if metadata.access().is_some() || metadata.actual_access().is_some() {
                return Err(mismatch(".access"));
            }
            if metadata.is_restrict() == Some(true) || metadata.is_const() == Some(true) {
                return Err(mismatch("argument qualifiers"));
            }
        }
    }
    if metadata.is_volatile() == Some(true) {
        return Err(mismatch(".is_volatile"));
    }
    if metadata.is_pipe() == Some(true) {
        return Err(mismatch(".is_pipe"));
    }
    Ok(())
}

const fn physical_value_type(value: ScalarTypeV1) -> ExplicitValueType {
    match value {
        ScalarTypeV1::I8 => ExplicitValueType::I8,
        ScalarTypeV1::U8 => ExplicitValueType::U8,
        ScalarTypeV1::I16 => ExplicitValueType::I16,
        ScalarTypeV1::U16 => ExplicitValueType::U16,
        ScalarTypeV1::I32 => ExplicitValueType::I32,
        ScalarTypeV1::U32 => ExplicitValueType::U32,
        ScalarTypeV1::I64 => ExplicitValueType::I64,
        ScalarTypeV1::U64 => ExplicitValueType::U64,
        ScalarTypeV1::F16 => ExplicitValueType::F16,
        ScalarTypeV1::F32 => ExplicitValueType::F32,
        ScalarTypeV1::F64 => ExplicitValueType::F64,
    }
}

fn actual_access_is_subset(contract: ArgumentAccess, actual: ArgumentAccess) -> bool {
    matches!(
        (contract, actual),
        (ArgumentAccess::ReadOnly, ArgumentAccess::ReadOnly)
            | (ArgumentAccess::WriteOnly, ArgumentAccess::WriteOnly)
            | (
                ArgumentAccess::ReadWrite,
                ArgumentAccess::ReadOnly | ArgumentAccess::WriteOnly | ArgumentAccess::ReadWrite
            )
    )
}

fn align_up(value: u64, alignment: u64) -> Result<u64, FinalizationError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|sum| sum & !mask)
        .ok_or(FinalizationError::DescriptorSectionOutOfBounds)
}

#[derive(Clone, Debug)]
struct ElfSection {
    index: usize,
    is_descriptor: bool,
    section_type: u32,
    flags: u64,
    range: Range<usize>,
    alignment: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DescriptorPlacementV1 {
    Detached,
    AllocatedReadOnly,
}

fn locate_descriptor_section_with_placement(
    bytes: &[u8],
    placement: DescriptorPlacementV1,
) -> Result<ElfSection, FinalizationError> {
    if bytes.len() < ELF64_HEADER_BYTES {
        return Err(FinalizationError::InvalidElf("ELF header is truncated"));
    }
    if read_u16(bytes, 52)? != ELF64_HEADER_BYTES as u16 {
        return Err(FinalizationError::InvalidElf(
            "ELF header size is not canonical",
        ));
    }

    let program_table = parse_table_range(
        bytes,
        32,
        54,
        56,
        ELF64_PROGRAM_HEADER_BYTES,
        MAX_ELF_SEGMENTS,
        true,
        "program header table",
    )?;
    let section_table = parse_table_range(
        bytes,
        40,
        58,
        60,
        ELF64_SECTION_HEADER_BYTES,
        MAX_ELF_SECTIONS,
        false,
        "section header table",
    )?
    .ok_or(FinalizationError::InvalidElf(
        "section header table is missing",
    ))?;
    let section_count = section_table.len() / ELF64_SECTION_HEADER_BYTES;
    let section_zero = bytes
        .get(section_table.start..section_table.start + ELF64_SECTION_HEADER_BYTES)
        .ok_or(FinalizationError::InvalidElf(
            "section header zero is truncated",
        ))?;
    if section_zero.iter().any(|byte| *byte != 0) {
        return Err(FinalizationError::InvalidElf(
            "section header zero is not an all-zero SHT_NULL entry",
        ));
    }

    let shstr_index = usize::from(read_u16(bytes, 62)?);
    if shstr_index == 0 || shstr_index >= section_count {
        return Err(FinalizationError::InvalidElf(
            "section-name string-table index is invalid",
        ));
    }
    let shstr_header = section_header_offset(&section_table, shstr_index)?;
    if read_u32(bytes, shstr_header + ELF64_SECTION_TYPE_OFFSET)? != SHT_STRTAB {
        return Err(FinalizationError::InvalidElf(
            "section-name table is not SHT_STRTAB",
        ));
    }
    let shstr_range = section_file_range(bytes, shstr_header)?;
    let shstr = bytes
        .get(shstr_range.clone())
        .ok_or(FinalizationError::InvalidElf(
            "section-name table is out of bounds",
        ))?;
    if shstr.first() != Some(&0) || shstr.last() != Some(&0) {
        return Err(FinalizationError::InvalidElf(
            "section-name table is not NUL delimited",
        ));
    }

    let mut sections = Vec::with_capacity(section_count);
    for index in 1..section_count {
        let header = section_header_offset(&section_table, index)?;
        let name_offset = usize::try_from(read_u32(bytes, header + ELF64_SECTION_NAME_OFFSET)?)
            .map_err(|_| FinalizationError::InvalidElf("section name offset overflows usize"))?;
        let is_descriptor = fixed_section_name_matches(shstr, name_offset)?;
        let section_type = read_u32(bytes, header + ELF64_SECTION_TYPE_OFFSET)?;
        let range = section_file_range(bytes, header)?;
        sections.push(ElfSection {
            index,
            is_descriptor,
            section_type,
            flags: read_u64(bytes, header + ELF64_SECTION_FLAGS_OFFSET)?,
            range,
            alignment: read_u64(bytes, header + ELF64_SECTION_ALIGNMENT_OFFSET)?,
        });
    }

    let mut candidates = sections.iter().filter(|section| section.is_descriptor);
    let candidate = candidates
        .next()
        .ok_or(FinalizationError::MissingDescriptorSection)?;
    if candidates.next().is_some() {
        return Err(FinalizationError::DuplicateDescriptorSection);
    }
    if candidate.section_type != SHT_PROGBITS {
        return Err(FinalizationError::InvalidDescriptorSectionType);
    }
    let expected_flags = match placement {
        DescriptorPlacementV1::Detached => 0,
        DescriptorPlacementV1::AllocatedReadOnly => SHF_ALLOC,
    };
    if candidate.flags != expected_flags {
        return Err(FinalizationError::InvalidDescriptorSectionFlags(
            candidate.flags,
        ));
    }
    if candidate.alignment != DEVICE_DESCRIPTOR_SECTION_ALIGNMENT
        || !candidate
            .range
            .start
            .is_multiple_of(DEVICE_DESCRIPTOR_SECTION_ALIGNMENT as usize)
    {
        return Err(FinalizationError::InvalidDescriptorSectionAlignment);
    }
    if candidate.range.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(FinalizationError::DescriptorTableTooLarge);
    }
    if ranges_overlap(&candidate.range, &(0..ELF64_HEADER_BYTES)) {
        return Err(FinalizationError::DescriptorSectionOverlaps("ELF header"));
    }
    if program_table
        .as_ref()
        .is_some_and(|range| ranges_overlap(&candidate.range, range))
    {
        return Err(FinalizationError::DescriptorSectionOverlaps(
            "program header table",
        ));
    }
    if ranges_overlap(&candidate.range, &section_table) {
        return Err(FinalizationError::DescriptorSectionOverlaps(
            "section header table",
        ));
    }
    for section in &sections {
        if section.index != candidate.index
            && !section.range.is_empty()
            && ranges_overlap(&candidate.range, &section.range)
        {
            return Err(FinalizationError::DescriptorSectionOverlaps(
                "another file-backed section",
            ));
        }
    }
    let mut containing_read_only_loads = 0_usize;
    if let Some(program_table) = program_table {
        let count = program_table.len() / ELF64_PROGRAM_HEADER_BYTES;
        for index in 0..count {
            let header = program_table.start + index * ELF64_PROGRAM_HEADER_BYTES;
            let segment_range = checked_range(
                bytes.len(),
                read_u64(bytes, header + ELF64_PROGRAM_FILE_OFFSET)?,
                read_u64(bytes, header + ELF64_PROGRAM_FILE_SIZE_OFFSET)?,
                "program segment file range is invalid",
            )?;
            if !segment_range.is_empty() && ranges_overlap(&candidate.range, &segment_range) {
                if placement == DescriptorPlacementV1::AllocatedReadOnly
                    && read_u32(bytes, header + ELF64_PROGRAM_TYPE_OFFSET)? == PT_LOAD
                    && read_u32(bytes, header + ELF64_PROGRAM_FLAGS_OFFSET)? == PF_R
                    && segment_range.start <= candidate.range.start
                    && candidate.range.end <= segment_range.end
                {
                    containing_read_only_loads += 1;
                } else {
                    return Err(FinalizationError::DescriptorSectionOverlaps(
                        "a program segment",
                    ));
                }
            }
        }
    }
    if placement == DescriptorPlacementV1::AllocatedReadOnly && containing_read_only_loads != 1 {
        return Err(FinalizationError::DescriptorSectionOverlaps(
            "exactly one read-only PT_LOAD is required",
        ));
    }
    Ok(candidate.clone())
}

#[allow(clippy::too_many_arguments)]
fn parse_table_range(
    bytes: &[u8],
    offset_field: usize,
    entry_size_field: usize,
    count_field: usize,
    expected_entry_size: usize,
    max_count: usize,
    allow_empty: bool,
    name: &'static str,
) -> Result<Option<Range<usize>>, FinalizationError> {
    let count = usize::from(read_u16(bytes, count_field)?);
    let entry_size = usize::from(read_u16(bytes, entry_size_field)?);
    let offset = read_u64(bytes, offset_field)?;
    if count == 0 {
        if allow_empty && offset == 0 {
            return Ok(None);
        }
        return Err(FinalizationError::InvalidElf(if allow_empty {
            "empty table has a nonzero offset"
        } else {
            "required ELF table is empty"
        }));
    }
    if count > max_count {
        return Err(FinalizationError::InvalidElf(
            "ELF table count exceeds limit",
        ));
    }
    if entry_size != expected_entry_size {
        return Err(FinalizationError::InvalidElf(
            "ELF table entry size is invalid",
        ));
    }
    let byte_len = count
        .checked_mul(entry_size)
        .ok_or(FinalizationError::InvalidElf("ELF table size overflows"))?;
    let range = checked_range(
        bytes.len(),
        offset,
        u64::try_from(byte_len)
            .map_err(|_| FinalizationError::InvalidElf("ELF table size overflows u64"))?,
        name,
    )?;
    Ok(Some(range))
}

fn section_header_offset(
    section_table: &Range<usize>,
    index: usize,
) -> Result<usize, FinalizationError> {
    section_table
        .start
        .checked_add(index.checked_mul(ELF64_SECTION_HEADER_BYTES).ok_or(
            FinalizationError::InvalidElf("section header index overflows"),
        )?)
        .ok_or(FinalizationError::InvalidElf(
            "section header offset overflows",
        ))
}

fn section_file_range(bytes: &[u8], header: usize) -> Result<Range<usize>, FinalizationError> {
    let section_type = read_u32(bytes, header + ELF64_SECTION_TYPE_OFFSET)?;
    let offset = read_u64(bytes, header + ELF64_SECTION_FILE_OFFSET)?;
    let size = read_u64(bytes, header + ELF64_SECTION_SIZE_OFFSET)?;
    if section_type == SHT_NOBITS {
        let offset = usize::try_from(offset)
            .map_err(|_| FinalizationError::InvalidElf("NOBITS section offset overflows usize"))?;
        return Ok(offset..offset);
    }
    checked_range(bytes.len(), offset, size, "section file range is invalid")
}

fn fixed_section_name_matches(bytes: &[u8], offset: usize) -> Result<bool, FinalizationError> {
    let suffix = bytes.get(offset..).ok_or(FinalizationError::InvalidElf(
        "section name offset is out of bounds",
    ))?;
    let name = DEVICE_DESCRIPTOR_SECTION_NAME.as_bytes();
    let Some(candidate) = suffix.get(..name.len() + 1) else {
        return Ok(false);
    };
    Ok(&candidate[..name.len()] == name && candidate[name.len()] == 0)
}

fn checked_range(
    file_len: usize,
    offset: u64,
    size: u64,
    reason: &'static str,
) -> Result<Range<usize>, FinalizationError> {
    let offset = usize::try_from(offset).map_err(|_| FinalizationError::InvalidElf(reason))?;
    let size = usize::try_from(size).map_err(|_| FinalizationError::InvalidElf(reason))?;
    let end = offset
        .checked_add(size)
        .ok_or(FinalizationError::InvalidElf(reason))?;
    if end > file_len {
        return Err(FinalizationError::InvalidElf(reason));
    }
    Ok(offset..end)
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, FinalizationError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(FinalizationError::InvalidElf("u16 field is truncated"))?;
    Ok(u16::from_le_bytes(value.try_into().map_err(|_| {
        FinalizationError::InvalidElf("u16 field is malformed")
    })?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, FinalizationError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(FinalizationError::InvalidElf("u32 field is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| {
        FinalizationError::InvalidElf("u32 field is malformed")
    })?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, FinalizationError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(FinalizationError::InvalidElf("u64 field is truncated"))?;
    Ok(u64::from_le_bytes(value.try_into().map_err(|_| {
        FinalizationError::InvalidElf("u64 field is malformed")
    })?))
}

#[cfg(test)]
mod allocated_descriptor_placement_tests {
    use super::*;

    const DESCRIPTOR_OFFSET: usize = 0x100;
    const DESCRIPTOR_SIZE: usize = 64;
    const SHSTRTAB_OFFSET: usize = 0x180;
    const SECTION_TABLE_OFFSET: usize = 0x200;

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn allocated_elf(program_count: u16) -> Vec<u8> {
        let mut bytes = vec![0; SECTION_TABLE_OFFSET + 3 * ELF64_SECTION_HEADER_BYTES];
        write_u64(&mut bytes, 32, ELF64_HEADER_BYTES as u64);
        write_u64(&mut bytes, 40, SECTION_TABLE_OFFSET as u64);
        write_u16(&mut bytes, 52, ELF64_HEADER_BYTES as u16);
        write_u16(&mut bytes, 54, ELF64_PROGRAM_HEADER_BYTES as u16);
        write_u16(&mut bytes, 56, program_count);
        write_u16(&mut bytes, 58, ELF64_SECTION_HEADER_BYTES as u16);
        write_u16(&mut bytes, 60, 3);
        write_u16(&mut bytes, 62, 2);

        let first_program = ELF64_HEADER_BYTES;
        write_u32(
            &mut bytes,
            first_program + ELF64_PROGRAM_TYPE_OFFSET,
            PT_LOAD,
        );
        write_u32(&mut bytes, first_program + ELF64_PROGRAM_FLAGS_OFFSET, PF_R);
        write_u64(&mut bytes, first_program + ELF64_PROGRAM_FILE_OFFSET, 0);
        write_u64(
            &mut bytes,
            first_program + ELF64_PROGRAM_FILE_SIZE_OFFSET,
            (DESCRIPTOR_OFFSET + DESCRIPTOR_SIZE) as u64,
        );

        let names = b"\0.fe2o3.kd.v1\0.shstrtab\0";
        bytes[SHSTRTAB_OFFSET..SHSTRTAB_OFFSET + names.len()].copy_from_slice(names);
        let descriptor = SECTION_TABLE_OFFSET + ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, descriptor + ELF64_SECTION_NAME_OFFSET, 1);
        write_u32(
            &mut bytes,
            descriptor + ELF64_SECTION_TYPE_OFFSET,
            SHT_PROGBITS,
        );
        write_u64(
            &mut bytes,
            descriptor + ELF64_SECTION_FLAGS_OFFSET,
            SHF_ALLOC,
        );
        write_u64(
            &mut bytes,
            descriptor + ELF64_SECTION_FILE_OFFSET,
            DESCRIPTOR_OFFSET as u64,
        );
        write_u64(
            &mut bytes,
            descriptor + ELF64_SECTION_SIZE_OFFSET,
            DESCRIPTOR_SIZE as u64,
        );
        write_u64(
            &mut bytes,
            descriptor + ELF64_SECTION_ALIGNMENT_OFFSET,
            DEVICE_DESCRIPTOR_SECTION_ALIGNMENT,
        );
        let shstr = descriptor + ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, shstr + ELF64_SECTION_NAME_OFFSET, 15);
        write_u32(&mut bytes, shstr + ELF64_SECTION_TYPE_OFFSET, SHT_STRTAB);
        write_u64(
            &mut bytes,
            shstr + ELF64_SECTION_FILE_OFFSET,
            SHSTRTAB_OFFSET as u64,
        );
        write_u64(
            &mut bytes,
            shstr + ELF64_SECTION_SIZE_OFFSET,
            names.len() as u64,
        );
        write_u64(&mut bytes, shstr + ELF64_SECTION_ALIGNMENT_OFFSET, 1);
        bytes
    }

    #[test]
    fn allocated_placement_is_distinct_from_detached_and_requires_read_only_load() {
        let bytes = allocated_elf(1);
        assert_eq!(
            locate_descriptor_section_with_placement(&bytes, DescriptorPlacementV1::Detached)
                .unwrap_err(),
            FinalizationError::InvalidDescriptorSectionFlags(SHF_ALLOC)
        );
        let section = locate_descriptor_section_with_placement(
            &bytes,
            DescriptorPlacementV1::AllocatedReadOnly,
        )
        .unwrap();
        assert_eq!(
            section.range,
            DESCRIPTOR_OFFSET..DESCRIPTOR_OFFSET + DESCRIPTOR_SIZE
        );

        for flags in [PF_R | 2, PF_R | 1] {
            let mut hostile = bytes.clone();
            write_u32(
                &mut hostile,
                ELF64_HEADER_BYTES + ELF64_PROGRAM_FLAGS_OFFSET,
                flags,
            );
            assert!(matches!(
                locate_descriptor_section_with_placement(
                    &hostile,
                    DescriptorPlacementV1::AllocatedReadOnly
                ),
                Err(FinalizationError::DescriptorSectionOverlaps(
                    "a program segment"
                ))
            ));
        }
    }

    #[test]
    fn allocated_placement_rejects_multiple_read_only_load_containment() {
        let mut bytes = allocated_elf(2);
        let second = ELF64_HEADER_BYTES + ELF64_PROGRAM_HEADER_BYTES;
        write_u32(&mut bytes, second + ELF64_PROGRAM_TYPE_OFFSET, PT_LOAD);
        write_u32(&mut bytes, second + ELF64_PROGRAM_FLAGS_OFFSET, PF_R);
        write_u64(&mut bytes, second + ELF64_PROGRAM_FILE_OFFSET, 0);
        write_u64(
            &mut bytes,
            second + ELF64_PROGRAM_FILE_SIZE_OFFSET,
            (DESCRIPTOR_OFFSET + DESCRIPTOR_SIZE) as u64,
        );
        assert!(matches!(
            locate_descriptor_section_with_placement(
                &bytes,
                DescriptorPlacementV1::AllocatedReadOnly
            ),
            Err(FinalizationError::DescriptorSectionOverlaps(
                "exactly one read-only PT_LOAD is required"
            ))
        ));
    }
}
