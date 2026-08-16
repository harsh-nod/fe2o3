//! Independent raw-HSACO inspection of sealed Worker V2 first-build evidence.
//!
//! This boundary consumes and retains the inert first-build evidence. It is deliberately not
//! canonical descriptor finalization and grants no publication, loading, or launch authority.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_artifact_transaction::{BuildAttempt, CompilerModuleHandoffIdentityV1};
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiEnvelopeIdentityV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_hsaco::{
    CodeObjectVersion as InspectedCodeObjectVersion, KernelBindingError,
    inspect_and_bind_kernel_descriptors,
};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DeviceTargetV1, ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE,
    ROW_SOFTMAX_V1_WORKGROUP_SIZE, TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE,
    TILED_GEMM_V1_WORKGROUP_SIZE,
};
use object::{Object, ObjectSection, ObjectSymbol};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME, FirstBuildWorkerV2IdentityV1,
    InertFirstBuildWorkerV2EvidenceV1, MAX_WORKER_SYMBOLS, WorkerCompilerFfiEnvelopeIdentityV2,
    WorkerMeasurementV1, request_construction::decode_link_options,
};

const EXPECTED_PROCESSOR: &str = "gfx942";
const REQUIRED_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];
const REQUIRED_MAX_FLAT_WORKGROUP_SIZE: u32 = 256;
const REQUIRED_WAVEFRONT_SIZE: u32 = 64;
const POLICY_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-RAW-HSACO-POLICY/V1\0";
const RESPONSE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-SEALED-RESPONSE/V1\0";
const INSPECTION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-RAW-HSACO-INSPECTION/V1\0";

/// One exact kernel entry/descriptor pair independently observed in AMDHSA metadata and ELF.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservedWorkerV2KernelSymbolsV1 {
    entry: String,
    descriptor: String,
}

impl ObservedWorkerV2KernelSymbolsV1 {
    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }
}

/// Exact fixed launch properties required from every inspected gfx942 kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2RawLaunchContractV1 {
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2RawLaunchDiagnosticProfileV1 {
    LegacyGfx942G1,
    TiledGemmV1,
    RowSoftmaxV1,
    FlashAttentionV1,
    Wave64CollectivesV1,
    WorkgroupSyncV1,
    MoeTop2V1,
}

impl WorkerV2RawLaunchContractV1 {
    const GFX942_G1: Self = Self {
        required_workgroup_size: REQUIRED_WORKGROUP_SIZE,
        max_flat_workgroup_size: REQUIRED_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const TILED_GEMM_V1: Self = Self {
        required_workgroup_size: TILED_GEMM_V1_WORKGROUP_SIZE,
        max_flat_workgroup_size: TILED_GEMM_V1_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const ROW_SOFTMAX_V1: Self = Self {
        required_workgroup_size: ROW_SOFTMAX_V1_WORKGROUP_SIZE,
        max_flat_workgroup_size: ROW_SOFTMAX_V1_MAX_FLAT_WORKGROUP_SIZE,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const FLASH_ATTENTION_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const WAVE64_COLLECTIVES_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const WORKGROUP_SYNC_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub(crate) const MOE_TOP2_V1: Self = Self {
        required_workgroup_size: [64, 1, 1],
        max_flat_workgroup_size: 64,
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    };

    pub const fn required_workgroup_size(self) -> [u32; 3] {
        self.required_workgroup_size
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }
}

/// Stable identity of policy facts derived from retained evidence and raw-HSACO observations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2RawHsacoPolicyIdentityV1([u8; 32]);

impl WorkerV2RawHsacoPolicyIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Policy reconstructed without accepting caller-supplied target, contract, or symbol claims.
///
/// Symbol roles come from the manifest retained by sealed first-build evidence. Kernel pairings
/// come from independent raw-HSACO inspection. The manifest and cooperative handoff record claims;
/// neither authenticates compiler authorship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2RawHsacoPolicyV1 {
    identity: WorkerV2RawHsacoPolicyIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    observed_kernels: Vec<ObservedWorkerV2KernelSymbolsV1>,
    expected_defined_symbols: Vec<String>,
    launch: WorkerV2RawLaunchContractV1,
}

impl WorkerV2RawHsacoPolicyV1 {
    pub const fn identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.identity
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.compiler_envelope
    }

    pub const fn symbol_manifest(&self) -> &CompilerModuleSymbolManifestV1 {
        &self.symbol_manifest
    }

    pub fn observed_kernels(&self) -> &[ObservedWorkerV2KernelSymbolsV1] {
        &self.observed_kernels
    }

    pub fn expected_defined_symbols(&self) -> &[String] {
        &self.expected_defined_symbols
    }

    pub const fn launch(&self) -> WorkerV2RawLaunchContractV1 {
        self.launch
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
}

/// Observation of the canonical fe2o3 descriptor-table section in raw Worker output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalDescriptorSectionObservationV1 {
    Missing,
    PresentButNotFinalizedByThisInspection,
}

/// SHA-256 identity of the exact canonical sealed Worker V2 response bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SealedWorkerV2ResponseIdentityV1([u8; 32]);

impl SealedWorkerV2ResponseIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity binding complete first-build lineage, raw bytes, and reconstructed policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedRawWorkerV2HsacoIdentityV1([u8; 32]);

impl InspectedRawWorkerV2HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence that exact sealed Worker V2 output passed independent raw-HSACO inspection.
///
/// This retains the consumed first-build evidence. It is not a finalized HSACO, does not
/// authenticate compiler origin, and cannot publish, load, or launch an artifact.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedRawWorkerV2HsacoV1 {
    identity: InspectedRawWorkerV2HsacoIdentityV1,
    response_identity: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    policy: WorkerV2RawHsacoPolicyV1,
    source: InertFirstBuildWorkerV2EvidenceV1,
}

impl InspectedRawWorkerV2HsacoV1 {
    pub const fn identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.identity
    }

    pub const fn source_evidence_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.source.identity()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.source.attempt()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.source.handoff_identity()
    }

    /// Returns the identity of the complete link plan retained by sealed first-build evidence.
    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.source.link_plan_identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.source.worker_measurement()
    }

    /// Identity of the complete compiler FFI contract envelope retained from the V2 handoff.
    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.source.compiler_envelope_identity()
    }

    /// The matching envelope identity sealed into the independently checked Worker V2 response.
    pub const fn sealed_compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.source
            .authorized()
            .response()
            .compiler_envelope_identity()
    }

    /// Returns the sealed request id echoed by the already request-verified V2 response.
    pub fn sealed_request_id(&self) -> &[u8; 32] {
        self.source.authorized().response().request_id()
    }

    /// Returns the sealed request identity echoed by the already request-verified V2 response.
    pub fn sealed_request_identity(&self) -> &[u8; 32] {
        self.source.authorized().response().request_identity()
    }

    pub const fn response_identity(&self) -> SealedWorkerV2ResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.source.output_identity()
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.source.output_bytes()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.policy.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.policy.code_object_version()
    }

    pub const fn policy(&self) -> &WorkerV2RawHsacoPolicyV1 {
        &self.policy
    }

    pub const fn canonical_descriptor_section(&self) -> CanonicalDescriptorSectionObservationV1 {
        self.descriptor_section
    }

    pub const fn canonical_descriptor_finalization_ran(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Why sealed Worker V2 evidence failed independent raw-HSACO inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV2RawHsacoInspectionError {
    LineageMismatch(&'static str),
    UnsupportedTarget(String),
    LinkPolicy,
    HsacoBinding(KernelBindingError),
    TargetMismatch {
        expected: String,
        actual: String,
    },
    CodeObjectVersionMismatch {
        expected: CodeObjectVersion,
        actual: CodeObjectVersion,
    },
    MissingKernel,
    KernelEntryRoleMismatch,
    KernelDescriptorRoleMismatch,
    CompilerEnvelopeImportRoleMismatch,
    CompilerEnvelopeExportRoleMismatch,
    DefinedSymbolInspection,
    DefinedSymbolClosureMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
    },
    MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
    },
    MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
    },
    DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
    },
    TiledGemmV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    TiledGemmV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    TiledGemmV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    TiledGemmV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    RowSoftmaxV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    FlashAttentionV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
}

impl fmt::Display for WorkerV2RawHsacoInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineageMismatch(field) => {
                write!(formatter, "Worker V2 lineage mismatch: {field}")
            }
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "raw Worker V2 inspection supports only gfx942, found {target}"
                )
            }
            Self::LinkPolicy => formatter.write_str("retained link policy is invalid"),
            Self::HsacoBinding(error) => write!(formatter, "raw HSACO binding failed: {error}"),
            Self::TargetMismatch { expected, actual } => {
                write!(
                    formatter,
                    "raw HSACO target mismatch: expected {expected}, found {actual}"
                )
            }
            Self::CodeObjectVersionMismatch { expected, actual } => write!(
                formatter,
                "raw HSACO code-object version mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::MissingKernel => formatter.write_str("raw HSACO contains no kernel metadata"),
            Self::KernelEntryRoleMismatch => {
                formatter.write_str("raw HSACO kernel entries differ from retained manifest roles")
            }
            Self::KernelDescriptorRoleMismatch => formatter
                .write_str("raw HSACO kernel descriptors differ from retained manifest roles"),
            Self::CompilerEnvelopeImportRoleMismatch => formatter.write_str(
                "retained compiler envelope imports differ from retained manifest roles",
            ),
            Self::CompilerEnvelopeExportRoleMismatch => formatter.write_str(
                "retained compiler envelope exports differ from retained manifest roles",
            ),
            Self::DefinedSymbolInspection => {
                formatter.write_str("failed to inspect raw HSACO static symbols")
            }
            Self::DefinedSymbolClosureMismatch { expected, actual } => write!(
                formatter,
                "defined raw HSACO symbols mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::RequiredWorkgroupSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} requires {actual:?}, expected {REQUIRED_WORKGROUP_SIZE:?}"
            ),
            Self::MaxFlatWorkgroupSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} max flat workgroup is {actual}, expected {REQUIRED_MAX_FLAT_WORKGROUP_SIZE}"
            ),
            Self::MetadataWavefrontSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} metadata wavefront is {actual}, expected {REQUIRED_WAVEFRONT_SIZE}"
            ),
            Self::DescriptorWavefrontSizeMismatch { kernel, actual } => write!(
                formatter,
                "kernel {kernel} descriptor wavefront is {actual}, expected {REQUIRED_WAVEFRONT_SIZE}"
            ),
            Self::TiledGemmV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::TiledGemmV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::TiledGemmV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "tiled GEMM V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::RowSoftmaxV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "row-softmax V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::FlashAttentionV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "FlashAttention V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "Wave64 collectives V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "workgroup-sync V1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
        }
    }
}

impl Error for WorkerV2RawHsacoInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HsacoBinding(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes sealed first-build evidence and independently inspects its exact raw HSACO output.
///
/// No target, symbol, compiler-contract, or launch policy is accepted from the caller. Available
/// symbol roles are recovered from the exact manifest retained in the first-build evidence.
pub fn inspect_worker_v2_raw_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    inspect_worker_v2_raw_hsaco_with_launch_v1(
        source,
        WorkerV2RawLaunchContractV1::GFX942_G1,
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1,
    )
}

pub(crate) fn inspect_worker_v2_raw_hsaco_with_launch_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
) -> Result<InspectedRawWorkerV2HsacoV1, WorkerV2RawHsacoInspectionError> {
    validate_lineage(&source)?;
    let target = source.plan().target();
    if target.as_amd_target_id().processor() != EXPECTED_PROCESSOR {
        return Err(WorkerV2RawHsacoInspectionError::UnsupportedTarget(
            target.to_string(),
        ));
    }
    let (code_object_version, _) = decode_link_options(source.plan().options())
        .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?;
    let symbol_manifest = source.symbol_manifest().clone();
    let exact_bytes = source.output_bytes();
    if !source.output_identity().matches(exact_bytes) {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "linked output identity",
        ));
    }

    let inspected = inspect_and_bind_kernel_descriptors(exact_bytes)
        .map_err(WorkerV2RawHsacoInspectionError::HsacoBinding)?;
    let metadata = inspected.inspection();
    if metadata.target() != target.as_amd_target_id() {
        return Err(WorkerV2RawHsacoInspectionError::TargetMismatch {
            expected: target.to_string(),
            actual: metadata.target().to_string(),
        });
    }
    let actual_code_object_version = map_code_object_version(metadata.code_object_version());
    if actual_code_object_version != code_object_version {
        return Err(WorkerV2RawHsacoInspectionError::CodeObjectVersionMismatch {
            expected: code_object_version,
            actual: actual_code_object_version,
        });
    }
    if metadata.kernels().is_empty() {
        return Err(WorkerV2RawHsacoInspectionError::MissingKernel);
    }

    let mut observed_kernels: Vec<_> = metadata
        .kernels()
        .iter()
        .map(|kernel| ObservedWorkerV2KernelSymbolsV1 {
            entry: kernel.name().to_owned(),
            descriptor: kernel.symbol().to_owned(),
        })
        .collect();
    observed_kernels.sort();
    let observed_entries: Vec<&str> = observed_kernels
        .iter()
        .map(|kernel| kernel.entry())
        .collect();
    let manifest_entries: Vec<&str> = symbol_manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
        .collect();
    if observed_entries != manifest_entries {
        return Err(WorkerV2RawHsacoInspectionError::KernelEntryRoleMismatch);
    }
    let mut observed_descriptors: Vec<&str> = observed_kernels
        .iter()
        .map(|kernel| kernel.descriptor())
        .collect();
    observed_descriptors.sort_unstable();
    let manifest_descriptors: Vec<&str> = symbol_manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
        .collect();
    if observed_descriptors != manifest_descriptors {
        return Err(WorkerV2RawHsacoInspectionError::KernelDescriptorRoleMismatch);
    }

    let expected_symbols = expected_defined_symbols(&symbol_manifest);
    let actual_defined_symbols = inspect_defined_global_symbols(exact_bytes)?;
    if actual_defined_symbols != expected_symbols {
        return Err(
            WorkerV2RawHsacoInspectionError::DefinedSymbolClosureMismatch {
                expected: expected_symbols,
                actual: actual_defined_symbols,
            },
        );
    }
    for (kernel, binding) in metadata.kernels().iter().zip(inspected.bindings()) {
        if kernel.required_workgroup_size() != Some(launch.required_workgroup_size()) {
            return Err(required_workgroup_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.required_workgroup_size(),
            ));
        }
        if kernel.max_flat_workgroup_size() != launch.max_flat_workgroup_size() {
            return Err(max_flat_workgroup_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.max_flat_workgroup_size(),
            ));
        }
        if kernel.wavefront_size() != launch.wavefront_size() {
            return Err(metadata_wavefront_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                kernel.wavefront_size(),
            ));
        }
        let descriptor_wavefront = binding.descriptor().wavefront_size();
        if descriptor_wavefront != launch.wavefront_size() {
            return Err(descriptor_wavefront_size_mismatch(
                launch,
                diagnostic_profile,
                kernel.name(),
                descriptor_wavefront,
            ));
        }
    }

    let descriptor_section = inspect_descriptor_section(exact_bytes)?;
    let compiler_envelope = source.compiler_envelope_identity();
    let expected_defined_symbols = expected_defined_symbols(&symbol_manifest);
    let policy = WorkerV2RawHsacoPolicyV1 {
        identity: calculate_policy_identity(
            target,
            code_object_version,
            compiler_envelope,
            &symbol_manifest,
            &observed_kernels,
            &expected_defined_symbols,
            launch,
        ),
        target,
        code_object_version,
        compiler_envelope,
        symbol_manifest,
        observed_kernels,
        expected_defined_symbols,
        launch,
    };
    let response_identity =
        calculate_response_identity(source.authorized().response().canonical_bytes());
    let identity =
        calculate_inspection_identity(&source, &policy, response_identity, descriptor_section);
    Ok(InspectedRawWorkerV2HsacoV1 {
        identity,
        response_identity,
        descriptor_section,
        policy,
        source,
    })
}

fn required_workgroup_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: Option<[u32; 3]>,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.required_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn max_flat_workgroup_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.max_flat_workgroup_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn metadata_wavefront_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn descriptor_wavefront_size_mismatch(
    launch: WorkerV2RawLaunchContractV1,
    diagnostic_profile: WorkerV2RawLaunchDiagnosticProfileV1,
    kernel: &str,
    actual: u32,
) -> WorkerV2RawHsacoInspectionError {
    match diagnostic_profile {
        WorkerV2RawLaunchDiagnosticProfileV1::LegacyGfx942G1 => {
            WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::TiledGemmV1 => {
            WorkerV2RawHsacoInspectionError::TiledGemmV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::RowSoftmaxV1 => {
            WorkerV2RawHsacoInspectionError::RowSoftmaxV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::FlashAttentionV1 => {
            WorkerV2RawHsacoInspectionError::FlashAttentionV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::Wave64CollectivesV1 => {
            WorkerV2RawHsacoInspectionError::Wave64CollectivesV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::WorkgroupSyncV1 => {
            WorkerV2RawHsacoInspectionError::WorkgroupSyncV1DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
                expected: launch.wavefront_size(),
            }
        }
        WorkerV2RawLaunchDiagnosticProfileV1::MoeTop2V1 => {
            WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch {
                kernel: kernel.to_owned(),
                actual,
            }
        }
    }
}

fn validate_lineage(
    source: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<(), WorkerV2RawHsacoInspectionError> {
    let authorized = source.authorized();
    let response = authorized.response();
    if source.attempt() != authorized.attempt() {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "build attempt",
        ));
    }
    if source.handoff_identity() != authorized.handoff_identity() {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "handoff identity",
        ));
    }
    if source.compiler_envelope().target().to_string() != source.plan().target().to_string()
        || map_compiler_code_object_version(source.compiler_envelope().code_object_version())
            != decode_link_options(source.plan().options())
                .map_err(|_| WorkerV2RawHsacoInspectionError::LinkPolicy)?
                .0
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "compiler envelope target/code-object version",
        ));
    }
    if response.compiler_envelope_identity().as_bytes()
        != source.compiler_envelope_identity().as_bytes()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "compiler envelope identity",
        ));
    }
    let directional = source.compiler_envelope().directional_symbols();
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .eq(directional.imports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeImportRoleMismatch);
    }
    if !source
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .eq(directional.exports())
    {
        return Err(WorkerV2RawHsacoInspectionError::CompilerEnvelopeExportRoleMismatch);
    }
    if source.worker_measurement().executable() != authorized.worker_executable()
        || source.worker_measurement().worker_build_identity() != response.worker_build_identity()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "worker measurement",
        ));
    }
    let output = response
        .output()
        .ok_or(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "missing linked output",
        ))?;
    if output.identity() != source.output_identity()
        || output.request_identity() != response.request_identity()
        || output.compiler_envelope_identity() != response.compiler_envelope_identity()
    {
        return Err(WorkerV2RawHsacoInspectionError::LineageMismatch(
            "sealed request/response/output identity",
        ));
    }
    Ok(())
}

fn expected_defined_symbols(manifest: &CompilerModuleSymbolManifestV1) -> Vec<String> {
    let mut symbols = Vec::new();
    for role in [
        CompilerModuleSymbolRoleV1::KernelEntry,
        CompilerModuleSymbolRoleV1::KernelDescriptor,
        CompilerModuleSymbolRoleV1::DeviceFfiExport,
        CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
    ] {
        symbols.extend(manifest.symbols(role).map(str::to_owned));
    }
    symbols.sort();
    symbols
}

fn inspect_defined_global_symbols(
    bytes: &[u8],
) -> Result<Vec<String>, WorkerV2RawHsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
    let mut symbols = BTreeSet::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || (!symbol.is_global() && !symbol.is_weak()) {
            continue;
        }
        let name = symbol
            .name()
            .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
        if name.is_empty() || !symbols.insert(name.to_owned()) {
            return Err(WorkerV2RawHsacoInspectionError::DefinedSymbolInspection);
        }
    }
    if symbols.len() > MAX_WORKER_SYMBOLS {
        return Err(WorkerV2RawHsacoInspectionError::DefinedSymbolInspection);
    }
    Ok(symbols.into_iter().collect())
}

fn inspect_descriptor_section(
    bytes: &[u8],
) -> Result<CanonicalDescriptorSectionObservationV1, WorkerV2RawHsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?;
    for section in file.sections() {
        if section
            .name()
            .map_err(|_| WorkerV2RawHsacoInspectionError::DefinedSymbolInspection)?
            == DEVICE_DESCRIPTOR_SECTION_NAME
        {
            return Ok(
                CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
            );
        }
    }
    Ok(CanonicalDescriptorSectionObservationV1::Missing)
}

fn calculate_policy_identity(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    manifest: &CompilerModuleSymbolManifestV1,
    observed_kernels: &[ObservedWorkerV2KernelSymbolsV1],
    expected_defined_symbols: &[String],
    launch: WorkerV2RawLaunchContractV1,
) -> WorkerV2RawHsacoPolicyIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_IDENTITY_DOMAIN_V1);
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_tag(code_object_version)]);
    hasher.update(compiler_envelope.as_bytes());
    hasher.update(manifest.identity().sha256());
    hasher.update(manifest.identity().byte_len().to_le_bytes());
    hasher.update((observed_kernels.len() as u64).to_le_bytes());
    for kernel in observed_kernels {
        hash_text(&mut hasher, &kernel.entry);
        hash_text(&mut hasher, &kernel.descriptor);
    }
    hash_strings(&mut hasher, expected_defined_symbols);
    for dimension in launch.required_workgroup_size() {
        hasher.update(dimension.to_le_bytes());
    }
    hasher.update(launch.max_flat_workgroup_size().to_le_bytes());
    hasher.update(launch.wavefront_size().to_le_bytes());
    WorkerV2RawHsacoPolicyIdentityV1(hasher.finalize().into())
}

fn calculate_response_identity(bytes: &[u8]) -> SealedWorkerV2ResponseIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_IDENTITY_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    SealedWorkerV2ResponseIdentityV1(hasher.finalize().into())
}

fn calculate_inspection_identity(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    policy: &WorkerV2RawHsacoPolicyV1,
    response: SealedWorkerV2ResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
) -> InspectedRawWorkerV2HsacoIdentityV1 {
    let worker = source.worker_measurement();
    let sealed_response = source.authorized().response();
    let mut hasher = Sha256::new();
    hasher.update(INSPECTION_IDENTITY_DOMAIN_V1);
    hasher.update(source.identity().as_bytes());
    hash_attempt(&mut hasher, source.attempt());
    hasher.update(source.handoff_identity().as_bytes());
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hasher.update(sealed_response.request_id());
    hasher.update(sealed_response.request_identity());
    hasher.update(response.0);
    hash_content(&mut hasher, source.output_identity());
    hash_text(&mut hasher, &policy.target.to_string());
    hasher.update([code_object_version_tag(policy.code_object_version)]);
    hasher.update(policy.identity.0);
    hasher.update([match descriptor_section {
        CanonicalDescriptorSectionObservationV1::Missing => 0,
        CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection => 1,
    }]);
    InspectedRawWorkerV2HsacoIdentityV1(hasher.finalize().into())
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_strings(hasher: &mut Sha256, strings: &[String]) {
    hasher.update((strings.len() as u64).to_le_bytes());
    for value in strings {
        hash_text(hasher, value);
    }
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

const fn map_code_object_version(version: InspectedCodeObjectVersion) -> CodeObjectVersion {
    match version {
        InspectedCodeObjectVersion::V4 => CodeObjectVersion::V4,
        InspectedCodeObjectVersion::V5 => CodeObjectVersion::V5,
        InspectedCodeObjectVersion::V6 => CodeObjectVersion::V6,
    }
}

const fn map_compiler_code_object_version(version: CompilerCodeObjectVersion) -> CodeObjectVersion {
    match version {
        CompilerCodeObjectVersion::V4 => CodeObjectVersion::V4,
        CompilerCodeObjectVersion::V5 => CodeObjectVersion::V5,
        CompilerCodeObjectVersion::V6 => CodeObjectVersion::V6,
    }
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}
