//! Independent raw-HSACO inspection of sealed Worker V3 first-build evidence.
//!
//! This boundary consumes and retains the inert first-build evidence. It is deliberately not
//! canonical descriptor finalization and grants no publication, loading, or launch authority.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffSlotV3, CompilerModuleHandoffTransactionIdentityV3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiEnvelopeIdentityV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffIdentityV3, InertSemanticCompilerModuleHandoffV3,
    ProductionGfx942CompilerFfiEnvelopeKindV1, ProductionGfx950CompilerFfiEnvelopeKindV1,
    inspect_production_gfx942_compiler_ffi_envelope_v1,
    inspect_production_gfx950_compiler_ffi_envelope_v1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion,
    ExplicitArgument, ExplicitValueKind, ExplicitValueType, HiddenArgument, HiddenValueKind,
    InspectedKernelBindings, KernelBindingError, inspect_and_bind_kernel_descriptors,
};
use fe2o3_kernel_descriptor::{BlockSizeV1, CodeObjectVersion, DeviceTargetV1, KernelDescriptorV1};
use object::{Object, ObjectSection, ObjectSymbol};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, DEVICE_DESCRIPTOR_SECTION_NAME, FinalizationError,
    InertDecodedWorkerExchangeV2, InertProtectedFirstBuildWorkerV3EvidenceV1, MAX_WORKER_SYMBOLS,
    MultiInputLinkPlanV1, ProtectedCompilerHandoffBindingIdentityV3,
    ProtectedCompilerHandoffExpectationV3, ProtectedFirstBuildWorkerV3IdentityV1,
    WorkerCompilerFfiEnvelopeIdentityV2, WorkerMeasurementV1,
    request_construction::decode_link_options,
};

const PRODUCTION_GFX942_TARGET: &str = "gfx942:xnack-";
const PRODUCTION_GFX950_TARGET: &str = "gfx950:xnack-";
const REQUIRED_WAVEFRONT_SIZE: u32 = 64;
// These private codec domains retain historical V2 bytes so existing identities remain stable.
// The embedded version is a serialization label and grants no retired V2 admission authority.
const FROZEN_POLICY_IDENTITY_CODEC_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-RAW-HSACO-POLICY/V1\0";
const FROZEN_RESPONSE_IDENTITY_CODEC_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-SEALED-RESPONSE/V1\0";
const PROTECTED_V3_INSPECTION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/STRICT-V3-PROTECTED-WORKER-RAW-HSACO-INSPECTION/V1\0";
const PROTECTED_DESCRIPTOR_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/AMDHSA-DESCRIPTORS/V1\0";
const PROTECTED_ABI_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/KERNEL-ABI/V1\0";
const PROTECTED_RESOURCES_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-RAW-HSACO/KERNEL-RESOURCES/V1\0";

/// One exact kernel entry/descriptor pair independently observed in AMDHSA metadata and ELF.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservedWorkerKernelSymbolsV1 {
    entry: String,
    descriptor: String,
}

impl ObservedWorkerKernelSymbolsV1 {
    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }
}

/// Exact fixed launch properties required from every inspected production kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3LaunchContractV1 {
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
}

impl WorkerV3LaunchContractV1 {
    const PRODUCTION_V1: Self = Self {
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
pub struct WorkerV3HsacoPolicyIdentityV1([u8; 32]);

impl WorkerV3HsacoPolicyIdentityV1 {
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
pub struct WorkerV3HsacoPolicyV1 {
    identity: WorkerV3HsacoPolicyIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    observed_kernels: Vec<ObservedWorkerKernelSymbolsV1>,
    expected_defined_symbols: Vec<String>,
    launch: WorkerV3LaunchContractV1,
}

impl WorkerV3HsacoPolicyV1 {
    pub const fn identity(&self) -> WorkerV3HsacoPolicyIdentityV1 {
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

    pub fn observed_kernels(&self) -> &[ObservedWorkerKernelSymbolsV1] {
        &self.observed_kernels
    }

    pub fn expected_defined_symbols(&self) -> &[String] {
        &self.expected_defined_symbols
    }

    pub const fn launch(&self) -> WorkerV3LaunchContractV1 {
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

/// SHA-256 identity of the exact canonical sealed response bytes from the versioned worker codec.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SealedWorkerResponseIdentityV1([u8; 32]);

impl SealedWorkerResponseIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of one strict-V3 protected raw-HSACO inspection.
///
/// It retains the complete V3 transaction, semantic handoff, compiler closure, worker exchange,
/// and HSACO observations without projecting them into a legacy admission route.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedProtectedWorkerV3HsacoIdentityV1([u8; 32]);

impl InspectedProtectedWorkerV3HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert strict-V3 evidence that exact direct-LLVM Worker output passed raw-HSACO inspection.
///
/// The complete V3 first-build evidence remains owned by this value. Structural inspection does
/// not authenticate the compiler, prove semantic correctness, or grant publication, load, or
/// launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedProtectedWorkerV3HsacoV1 {
    identity: InspectedProtectedWorkerV3HsacoIdentityV1,
    response_identity: SealedWorkerResponseIdentityV1,
    descriptor_section: CanonicalDescriptorSectionObservationV1,
    descriptor_observation_preimage: Vec<u8>,
    abi_observation_preimage: Vec<u8>,
    resource_observation_preimage: Vec<u8>,
    policy: WorkerV3HsacoPolicyV1,
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
}

impl InspectedProtectedWorkerV3HsacoV1 {
    pub const fn identity(&self) -> InspectedProtectedWorkerV3HsacoIdentityV1 {
        self.identity
    }

    pub const fn source_evidence_identity(&self) -> ProtectedFirstBuildWorkerV3IdentityV1 {
        self.source.identity()
    }

    pub const fn binding_identity(&self) -> ProtectedCompilerHandoffBindingIdentityV3 {
        self.source.binding().identity()
    }

    pub const fn binding_expectation(&self) -> ProtectedCompilerHandoffExpectationV3 {
        self.source.binding().expectation()
    }

    pub const fn attempt(&self) -> BuildAttempt {
        self.binding_expectation().attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.binding_expectation().slot()
    }

    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.binding_expectation().transaction_identity()
    }

    pub const fn outer_handoff_identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.binding_expectation().outer_handoff_identity()
    }

    pub const fn outer_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.source.handoff()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.binding_expectation().compiler_closure()
    }

    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        self.source.plan()
    }

    pub const fn link_plan_identity(&self) -> crate::LinkPlanIdentityV1 {
        self.source.plan().identity()
    }

    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        self.source.worker_measurement()
    }

    pub const fn compiler_envelope_identity(&self) -> CompilerFfiEnvelopeIdentityV1 {
        self.source.handoff().module_handoff().envelope().identity()
    }

    pub const fn sealed_compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.source
            .exact_replay()
            .response()
            .compiler_envelope_identity()
    }

    pub fn sealed_request_id(&self) -> &[u8; 32] {
        self.source.exact_replay().response().request_id()
    }

    pub fn sealed_request_identity(&self) -> &[u8; 32] {
        self.source.exact_replay().response().request_identity()
    }

    pub const fn response_identity(&self) -> SealedWorkerResponseIdentityV1 {
        self.response_identity
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.source.output_identity()
    }

    pub const fn raw_hsaco_identity(&self) -> ContentIdentityV1 {
        self.linked_output_identity()
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

    pub const fn policy(&self) -> &WorkerV3HsacoPolicyV1 {
        &self.policy
    }

    /// Borrows the complete native V3 first-build evidence retained by this inspection.
    pub const fn source_evidence(&self) -> &InertProtectedFirstBuildWorkerV3EvidenceV1 {
        &self.source
    }

    pub(crate) fn into_source_evidence(self) -> InertProtectedFirstBuildWorkerV3EvidenceV1 {
        self.source
    }

    /// Returns the exact canonical descriptor observation transcript.
    pub fn descriptor_observation_preimage(&self) -> &[u8] {
        &self.descriptor_observation_preimage
    }

    /// Returns the exact canonical kernel-ABI observation transcript.
    pub fn abi_observation_preimage(&self) -> &[u8] {
        &self.abi_observation_preimage
    }

    /// Returns the exact canonical kernel-resource observation transcript.
    pub fn resource_observation_preimage(&self) -> &[u8] {
        &self.resource_observation_preimage
    }

    /// Returns descriptor, ABI, and resource identities in that order.
    pub fn observation_identities(&self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            calculate_observation_identity(
                PROTECTED_DESCRIPTOR_DOMAIN_V1,
                &self.descriptor_observation_preimage,
            ),
            calculate_observation_identity(PROTECTED_ABI_DOMAIN_V1, &self.abi_observation_preimage),
            calculate_observation_identity(
                PROTECTED_RESOURCES_DOMAIN_V1,
                &self.resource_observation_preimage,
            ),
        )
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

    pub const fn proves_semantic_correctness(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
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

/// Why sealed production Worker V3 evidence failed independent raw-HSACO inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3HsacoInspectionError {
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
    StrictV3Gfx942DeviceFfiPolicy,
    StrictV3Gfx942OcmlProviderClosureMismatch,
    StrictV3Gfx950DeviceFfiPolicy,
    StrictV3Gfx950OcmlProviderClosureUnmeasured,
    StrictV3Gfx950OcmlProviderClosureMismatch,
    DefinedSymbolInspection,
    DefinedSymbolClosureMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    ProductionV1RequiredWorkgroupSizeMismatch {
        kernel: String,
        actual: Option<[u32; 3]>,
        expected: [u32; 3],
    },
    ProductionV1MaxFlatWorkgroupSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    ProductionV1MetadataWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    ProductionV1DescriptorWavefrontSizeMismatch {
        kernel: String,
        actual: u32,
        expected: u32,
    },
    StrictV3DescriptorLaunchContract(&'static str),
}

impl fmt::Display for WorkerV3HsacoInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineageMismatch(field) => {
                write!(formatter, "Worker V3 lineage mismatch: {field}")
            }
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "raw Worker V3 inspection profile does not support {target}; gfx950 is admitted only as exact {PRODUCTION_GFX950_TARGET} by the production profile"
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
            Self::StrictV3Gfx942DeviceFfiPolicy => formatter.write_str(
                "strict V3 gfx942 compiler handoff has a noncanonical device-FFI or LLVM import closure",
            ),
            Self::StrictV3Gfx942OcmlProviderClosureMismatch => formatter.write_str(
                "strict V3 gfx942 OCML exp finalization observed a missing, substituted, or non-reproducible measured provider closure",
            ),
            Self::StrictV3Gfx950DeviceFfiPolicy => formatter.write_str(
                "strict V3 gfx950 compiler handoff has a noncanonical device-FFI or LLVM import closure",
            ),
            Self::StrictV3Gfx950OcmlProviderClosureUnmeasured => formatter.write_str(
                "strict V3 gfx950 OCML exp finalization requires a measured, identity-pinned ROCm gfx950 OCML provider/link closure",
            ),
            Self::StrictV3Gfx950OcmlProviderClosureMismatch => formatter.write_str(
                "strict V3 gfx950 OCML exp finalization observed a missing, substituted, or non-reproducible ROCm 7.2.1 provider closure",
            ),
            Self::DefinedSymbolInspection => {
                formatter.write_str("failed to inspect raw HSACO static symbols")
            }
            Self::DefinedSymbolClosureMismatch { expected, actual } => write!(
                formatter,
                "defined raw HSACO symbols mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::ProductionV1RequiredWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} requires {actual:?}, expected {expected:?}"
            ),
            Self::ProductionV1MaxFlatWorkgroupSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} max flat workgroup is {actual}, expected {expected}"
            ),
            Self::ProductionV1MetadataWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} metadata wavefront is {actual}, expected {expected}"
            ),
            Self::ProductionV1DescriptorWavefrontSizeMismatch {
                kernel,
                actual,
                expected,
            } => write!(
                formatter,
                "production-v1 kernel {kernel} descriptor wavefront is {actual}, expected {expected}"
            ),
            Self::StrictV3DescriptorLaunchContract(field) => {
                write!(
                    formatter,
                    "strict V3 descriptor launch contract rejected {field}"
                )
            }
        }
    }
}

impl Error for WorkerV3HsacoInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HsacoBinding(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes native strict-V3 first-build evidence under its descriptor-bound production contract.
///
/// The complete V3 transaction and outer semantic handoff remain owned by the result. This route
/// has no legacy admission fallback. When the artifact has a canonical descriptor table, its exact
/// physically cross-checked launch facts define the inspection policy; callers cannot inject or
/// weaken those facts.
pub fn inspect_protected_worker_v3_hsaco_v1(
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<InspectedProtectedWorkerV3HsacoV1, WorkerV3HsacoInspectionError> {
    validate_protected_v3_lineage(&source)?;
    let launch = strict_v3_launch_contract(&source)?;
    let raw = inspect_worker_v3_hsaco_v1(&source, launch)?;
    let response_identity =
        calculate_response_identity(source.exact_replay().response().canonical_bytes());
    let identity = calculate_protected_v3_inspection_identity(&source, &raw, response_identity);
    Ok(InspectedProtectedWorkerV3HsacoV1 {
        identity,
        response_identity,
        descriptor_section: raw.descriptor_section,
        descriptor_observation_preimage: raw.descriptor_observation_preimage,
        abi_observation_preimage: raw.abi_observation_preimage,
        resource_observation_preimage: raw.resource_observation_preimage,
        policy: raw.policy,
        source,
    })
}

fn strict_v3_launch_contract(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<WorkerV3LaunchContractV1, WorkerV3HsacoInspectionError> {
    let inspection = match crate::inspect_unfinalized(source.output_bytes()) {
        Ok(inspection) => inspection,
        // Preserve the existing descriptive inspection stage for legacy tests that deliberately
        // omit compiler descriptor-source evidence. Finalization still rejects those artifacts.
        Err(FinalizationError::MissingDescriptorSection) => {
            return Ok(WorkerV3LaunchContractV1::PRODUCTION_V1);
        }
        Err(_) => {
            return Err(
                WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "embedded descriptor inspection",
                ),
            );
        }
    };
    let mut kernels = inspection.descriptor_table().kernels().iter();
    let first = kernels
        .next()
        .ok_or(WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract("kernel set"))?;
    let expected = strict_v3_kernel_launch_contract(first)?;
    for kernel in kernels {
        if strict_v3_kernel_launch_contract(kernel)? != expected {
            return Err(
                WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "heterogeneous per-kernel launch policy",
                ),
            );
        }
    }
    Ok(expected)
}

fn strict_v3_kernel_launch_contract(
    kernel: &KernelDescriptorV1,
) -> Result<WorkerV3LaunchContractV1, WorkerV3HsacoInspectionError> {
    let block = match kernel.launch().block_size() {
        BlockSizeV1::Exact(block) => block,
        BlockSizeV1::Any | BlockSizeV1::AtMost(_) => {
            return Err(
                WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "non-exact block size",
                ),
            );
        }
    };
    Ok(WorkerV3LaunchContractV1 {
        required_workgroup_size: [block.x(), block.y(), block.z()],
        max_flat_workgroup_size: kernel.launch().max_flat_workgroup_size(),
        wavefront_size: REQUIRED_WAVEFRONT_SIZE,
    })
}

pub(crate) struct SharedWorkerV3HsacoInspectionV1 {
    pub(crate) descriptor_section: CanonicalDescriptorSectionObservationV1,
    pub(crate) policy: WorkerV3HsacoPolicyV1,
    pub(crate) descriptor_identity: [u8; 32],
    pub(crate) abi_identity: [u8; 32],
    pub(crate) resource_identity: [u8; 32],
    pub(crate) descriptor_observation_preimage: Vec<u8>,
    pub(crate) abi_observation_preimage: Vec<u8>,
    pub(crate) resource_observation_preimage: Vec<u8>,
}

fn inspect_worker_v3_hsaco_v1(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
    launch: WorkerV3LaunchContractV1,
) -> Result<SharedWorkerV3HsacoInspectionV1, WorkerV3HsacoInspectionError> {
    let target = source.plan().target();
    if !target_is_supported(target) {
        return Err(WorkerV3HsacoInspectionError::UnsupportedTarget(
            target.to_string(),
        ));
    }
    let (code_object_version, _) = decode_link_options(source.plan().options())
        .map_err(|_| WorkerV3HsacoInspectionError::LinkPolicy)?;
    inspect_worker_v3_hsaco_preimage_v1(
        target,
        code_object_version,
        source.handoff().module_handoff().symbol_manifest().clone(),
        source.handoff().module_handoff().envelope().identity(),
        source.output_identity(),
        source.output_bytes(),
        launch,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn inspect_worker_v3_hsaco_preimage_v1(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    symbol_manifest: CompilerModuleSymbolManifestV1,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    output_identity: ContentIdentityV1,
    exact_bytes: &[u8],
    launch: WorkerV3LaunchContractV1,
) -> Result<SharedWorkerV3HsacoInspectionV1, WorkerV3HsacoInspectionError> {
    if !target_is_supported(target) {
        return Err(WorkerV3HsacoInspectionError::UnsupportedTarget(
            target.to_string(),
        ));
    }
    if !output_identity.matches(exact_bytes) {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "linked output identity",
        ));
    }

    let inspected = inspect_and_bind_kernel_descriptors(exact_bytes)
        .map_err(WorkerV3HsacoInspectionError::HsacoBinding)?;
    let metadata = inspected.inspection();
    if metadata.target() != target.as_amd_target_id() {
        return Err(WorkerV3HsacoInspectionError::TargetMismatch {
            expected: target.to_string(),
            actual: metadata.target().to_string(),
        });
    }
    let actual_code_object_version = map_code_object_version(metadata.code_object_version());
    if actual_code_object_version != code_object_version {
        return Err(WorkerV3HsacoInspectionError::CodeObjectVersionMismatch {
            expected: code_object_version,
            actual: actual_code_object_version,
        });
    }
    if metadata.kernels().is_empty() {
        return Err(WorkerV3HsacoInspectionError::MissingKernel);
    }

    let mut observed_kernels: Vec<_> = metadata
        .kernels()
        .iter()
        .map(|kernel| ObservedWorkerKernelSymbolsV1 {
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
        return Err(WorkerV3HsacoInspectionError::KernelEntryRoleMismatch);
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
        return Err(WorkerV3HsacoInspectionError::KernelDescriptorRoleMismatch);
    }

    let expected_symbols = expected_defined_symbols(&symbol_manifest);
    let actual_defined_symbols = inspect_defined_global_symbols(exact_bytes)?;
    if actual_defined_symbols != expected_symbols {
        return Err(WorkerV3HsacoInspectionError::DefinedSymbolClosureMismatch {
            expected: expected_symbols,
            actual: actual_defined_symbols,
        });
    }
    for (kernel, binding) in metadata.kernels().iter().zip(inspected.bindings()) {
        if kernel.required_workgroup_size() != Some(launch.required_workgroup_size()) {
            return Err(required_workgroup_size_mismatch(
                launch,
                kernel.name(),
                kernel.required_workgroup_size(),
            ));
        }
        if kernel.max_flat_workgroup_size() != launch.max_flat_workgroup_size() {
            return Err(max_flat_workgroup_size_mismatch(
                launch,
                kernel.name(),
                kernel.max_flat_workgroup_size(),
            ));
        }
        if kernel.wavefront_size() != launch.wavefront_size() {
            return Err(metadata_wavefront_size_mismatch(
                launch,
                kernel.name(),
                kernel.wavefront_size(),
            ));
        }
        let descriptor_wavefront = binding.descriptor().wavefront_size();
        if descriptor_wavefront != launch.wavefront_size() {
            return Err(descriptor_wavefront_size_mismatch(
                launch,
                kernel.name(),
                descriptor_wavefront,
            ));
        }
    }

    let descriptor_section = inspect_descriptor_section(exact_bytes)?;
    let descriptor_observation_preimage = encode_descriptor_observation_preimage(&inspected);
    let abi_observation_preimage = encode_abi_observation_preimage(&inspected);
    let resource_observation_preimage = encode_resource_observation_preimage(&inspected);
    let descriptor_identity = calculate_observation_identity(
        PROTECTED_DESCRIPTOR_DOMAIN_V1,
        &descriptor_observation_preimage,
    );
    let abi_identity =
        calculate_observation_identity(PROTECTED_ABI_DOMAIN_V1, &abi_observation_preimage);
    let resource_identity = calculate_observation_identity(
        PROTECTED_RESOURCES_DOMAIN_V1,
        &resource_observation_preimage,
    );
    let expected_defined_symbols = expected_defined_symbols(&symbol_manifest);
    let policy = WorkerV3HsacoPolicyV1 {
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
    Ok(SharedWorkerV3HsacoInspectionV1 {
        descriptor_section,
        policy,
        descriptor_identity,
        abi_identity,
        resource_identity,
        descriptor_observation_preimage,
        abi_observation_preimage,
        resource_observation_preimage,
    })
}

fn target_is_supported(target: DeviceTargetV1) -> bool {
    let target = target.to_string();
    matches!(
        target.as_str(),
        PRODUCTION_GFX942_TARGET | PRODUCTION_GFX950_TARGET
    )
}

fn required_workgroup_size_mismatch(
    launch: WorkerV3LaunchContractV1,
    kernel: &str,
    actual: Option<[u32; 3]>,
) -> WorkerV3HsacoInspectionError {
    WorkerV3HsacoInspectionError::ProductionV1RequiredWorkgroupSizeMismatch {
        kernel: kernel.to_owned(),
        actual,
        expected: launch.required_workgroup_size(),
    }
}

fn max_flat_workgroup_size_mismatch(
    launch: WorkerV3LaunchContractV1,
    kernel: &str,
    actual: u32,
) -> WorkerV3HsacoInspectionError {
    WorkerV3HsacoInspectionError::ProductionV1MaxFlatWorkgroupSizeMismatch {
        kernel: kernel.to_owned(),
        actual,
        expected: launch.max_flat_workgroup_size(),
    }
}

fn metadata_wavefront_size_mismatch(
    launch: WorkerV3LaunchContractV1,
    kernel: &str,
    actual: u32,
) -> WorkerV3HsacoInspectionError {
    WorkerV3HsacoInspectionError::ProductionV1MetadataWavefrontSizeMismatch {
        kernel: kernel.to_owned(),
        actual,
        expected: launch.wavefront_size(),
    }
}

fn descriptor_wavefront_size_mismatch(
    launch: WorkerV3LaunchContractV1,
    kernel: &str,
    actual: u32,
) -> WorkerV3HsacoInspectionError {
    WorkerV3HsacoInspectionError::ProductionV1DescriptorWavefrontSizeMismatch {
        kernel: kernel.to_owned(),
        actual,
        expected: launch.wavefront_size(),
    }
}

fn validate_protected_v3_lineage(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
) -> Result<(), WorkerV3HsacoInspectionError> {
    let binding = source.binding();
    let expected = binding.expectation();
    let handoff = source.handoff();
    let capsule = handoff.capsule();
    let nested = handoff.module_handoff();
    let pair = handoff.pair_binding();

    if source.bootstrap().binding() != binding || source.exact_replay().binding() != binding {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 worker binding",
        ));
    }
    if expected.outer_handoff_identity() != handoff.identity()
        || !expected
            .outer_handoff_identity()
            .matches_canonical_bytes(handoff.canonical_bytes())
    {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 outer handoff",
        ));
    }

    let capsule_identity = capsule.identity();
    let pair_identity = pair.identity();
    if expected.capsule_sha256() != *capsule_identity.sha256()
        || expected.capsule_byte_len() != capsule_identity.byte_len()
        || expected.invocation_digest() != *capsule.invocation_digest().as_bytes()
        || expected.pair_binding_sha256() != *pair_identity.sha256()
        || expected.pair_binding_byte_len() != pair_identity.byte_len()
        || expected.nested_handoff_identity() != nested.identity()
        || pair.capsule_identity() != capsule_identity
        || pair.module_handoff_identity() != nested.identity()
        || expected.compiler_closure() != *capsule.compiler_closure()
        || expected.compiler_closure() != *capsule.invocation().compiler_closure()
    {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 semantic handoff association",
        ));
    }

    let final_receipt = capsule.receipts().final_compiler_module_commitment();
    let final_receipt_identity = final_receipt.identity();
    let final_commitment = InertFinalCompilerModuleCommitmentV3::decode(
        final_receipt.canonical_preimage(),
    )
    .map_err(|_| {
        WorkerV3HsacoInspectionError::LineageMismatch("strict V3 final compiler-module commitment")
    })?;
    let final_identity = final_commitment.identity();
    if expected.final_commitment_receipt_sha256() != *final_receipt_identity.sha256()
        || expected.final_commitment_receipt_byte_len() != final_receipt_identity.byte_len()
        || expected.final_commitment_sha256() != *final_identity.sha256()
        || expected.final_commitment_byte_len() != final_identity.byte_len()
        || !final_commitment.matches_handoff(nested)
    {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 final compiler-module commitment association",
        ));
    }

    if nested.target().to_string() != source.plan().target().to_string()
        || map_compiler_code_object_version(nested.code_object_version())
            != decode_link_options(source.plan().options())
                .map_err(|_| WorkerV3HsacoInspectionError::LinkPolicy)?
                .0
    {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 compiler envelope target/code-object version",
        ));
    }
    let gfx942_ffi = validate_strict_v3_gfx942_device_ffi(nested)?;
    let gfx950_ffi = validate_strict_v3_gfx950_device_ffi(nested)?;
    let directional = nested.envelope().directional_symbols();
    if !nested
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
        .eq(directional.imports())
    {
        return Err(WorkerV3HsacoInspectionError::CompilerEnvelopeImportRoleMismatch);
    }
    if !nested
        .symbol_manifest()
        .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
        .eq(directional.exports())
    {
        return Err(WorkerV3HsacoInspectionError::CompilerEnvelopeExportRoleMismatch);
    }

    let expected_envelope = nested.envelope().identity();
    let bootstrap = source.bootstrap();
    let replay = source.exact_replay();
    validate_strict_v3_gfx942_provider_exchanges(source, gfx942_ffi, bootstrap, replay)?;
    validate_strict_v3_gfx950_provider_exchanges(source, gfx950_ffi, bootstrap, replay)?;
    for execution in [bootstrap, replay] {
        let response = execution.response();
        if source.worker_measurement().executable() != execution.worker_executable()
            || source.worker_measurement().worker_build_identity()
                != response.worker_build_identity()
        {
            return Err(WorkerV3HsacoInspectionError::LineageMismatch(
                "strict V3 worker measurement",
            ));
        }
        if response.compiler_envelope_identity().as_bytes() != expected_envelope.as_bytes() {
            return Err(WorkerV3HsacoInspectionError::LineageMismatch(
                "strict V3 compiler envelope identity",
            ));
        }
        let output = response
            .output()
            .ok_or(WorkerV3HsacoInspectionError::LineageMismatch(
                "missing strict V3 linked output",
            ))?;
        if output.identity() != source.output_identity()
            || output.request_identity() != response.request_identity()
            || output.compiler_envelope_identity() != response.compiler_envelope_identity()
        {
            return Err(WorkerV3HsacoInspectionError::LineageMismatch(
                "strict V3 sealed request/response/output identity",
            ));
        }
    }

    let bootstrap_output = bootstrap
        .response()
        .output()
        .expect("strict V3 bootstrap output checked above");
    let replay_output = replay
        .response()
        .output()
        .expect("strict V3 replay output checked above");
    if bootstrap_output.bytes() != replay_output.bytes()
        || replay_output.bytes() != source.output_bytes()
    {
        return Err(WorkerV3HsacoInspectionError::LineageMismatch(
            "strict V3 reproducible output bytes",
        ));
    }
    Ok(())
}

fn validate_strict_v3_gfx942_device_ffi(
    nested: &CompilerModuleHandoffV2,
) -> Result<ProductionGfx942CompilerFfiEnvelopeKindV1, WorkerV3HsacoInspectionError> {
    if nested.target().to_string() != PRODUCTION_GFX942_TARGET {
        return Ok(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi);
    }
    let kind = inspect_production_gfx942_compiler_ffi_envelope_v1(nested.envelope())
        .ok_or(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)?;
    if nested.kind() != CompilerModuleKindV1::LlvmTextIr {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy);
    }
    let llvm = std::str::from_utf8(nested.module_bytes())
        .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)?;
    match kind {
        ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi => {
            if llvm.contains("@__ocml_") {
                return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy);
            }
            Ok(kind)
        }
        ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            let exact_llvm = llvm.matches("declare float @__ocml_exp_f32(float)").count() == 1
                && llvm.matches("call float @__ocml_exp_f32(float ").count() >= 1
                && llvm
                    .split("@__ocml_")
                    .skip(1)
                    .all(|suffix| suffix.starts_with("exp_f32"));
            if !exact_llvm {
                return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy);
            }
            Ok(kind)
        }
    }
}

const GFX942_OCML_PROVIDER_IDENTITY_V1: &str = "gfx942-ocml-v1";
const GFX942_OCML_PROVIDER_DIAGNOSTIC_V1: &str = "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4";
// Reviewed ROCm 7.2.4 gfx942 provider closure. Worker configure-time measurements
// must match these independent policy pins; self-consistent worker evidence is not authority.
const GFX942_OCML_PROVIDER_FILES_V1: [(&str, &str); 4] = [
    (
        "ocml.bc",
        "cfe97fe9ee29379f522e5f20ae55aae1cdb96eb41d6aa250ea11c4941c54e019",
    ),
    (
        "oclc_isa_version_942.bc",
        "580d540cc738c0f9554c8710575bbc9b51ebacdcbc29aa0074ed05d3691dea1d",
    ),
    (
        "oclc_unsafe_math_off.bc",
        "22c799b9154389f050f8f3368762636b9954a2ea25622199c359366bbd84657f",
    ),
    (
        "oclc_finite_only_off.bc",
        "f3138eeee65c1d83234260728d124f635f021abb37c495f4ed027dfe92bcb1dd",
    ),
];

fn validate_strict_v3_gfx942_provider_exchanges(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
    kind: ProductionGfx942CompilerFfiEnvelopeKindV1,
    bootstrap: &crate::InertProtectedCompilerHandoffExecutionV3,
    replay: &crate::InertProtectedCompilerHandoffExecutionV3,
) -> Result<(), WorkerV3HsacoInspectionError> {
    if source.handoff().module_handoff().target().to_string() != PRODUCTION_GFX942_TARGET {
        return Ok(());
    }
    let bootstrap_exchange = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        bootstrap.response().canonical_bytes(),
    )
    .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)?;
    let replay_exchange = InertDecodedWorkerExchangeV2::decode(
        source.exact_replay_request_bytes(),
        replay.response().canonical_bytes(),
    )
    .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)?;

    for exchange in [&bootstrap_exchange, &replay_exchange] {
        validate_strict_v3_gfx942_provider_exchange(kind, exchange)?;
    }
    if matches!(
        kind,
        ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. }
    ) && bootstrap_exchange.response().device_library_provider()
        != replay_exchange.response().device_library_provider()
    {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch);
    }
    Ok(())
}

fn validate_strict_v3_gfx942_provider_exchange(
    kind: ProductionGfx942CompilerFfiEnvelopeKindV1,
    exchange: &InertDecodedWorkerExchangeV2,
) -> Result<(), WorkerV3HsacoInspectionError> {
    let request = exchange.request();
    if !request.external_providers().is_empty()
        || request.target().to_string() != PRODUCTION_GFX942_TARGET
        || request.code_object_version() != CodeObjectVersion::V6
    {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch);
    }
    match kind {
        ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi => {
            if !request.import_symbols().is_empty()
                || exchange.response().device_library_provider().is_some()
            {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch,
                );
            }
        }
        ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            if request.import_symbols() != ["__ocml_exp_f32"]
                || !exchange
                    .response()
                    .diagnostics()
                    .iter()
                    .any(|value| value == GFX942_OCML_PROVIDER_DIAGNOSTIC_V1)
            {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch,
                );
            }
            validate_gfx942_ocml_provider_evidence(
                exchange.response().device_library_provider().ok_or(
                    WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch,
                )?,
            )?;
        }
    }
    Ok(())
}

fn validate_gfx942_ocml_provider_evidence(
    evidence: &crate::WorkerDeviceLibraryProviderEvidenceV1,
) -> Result<(), WorkerV3HsacoInspectionError> {
    if evidence.provider_identity() != GFX942_OCML_PROVIDER_IDENTITY_V1
        || evidence.target().to_string() != PRODUCTION_GFX942_TARGET
        || evidence.code_object_version() != CodeObjectVersion::V6
        || evidence.import_symbols() != ["__ocml_exp_f32"]
        || evidence.files().len() != GFX942_OCML_PROVIDER_FILES_V1.len()
    {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch);
    }
    for (actual, (basename, sha256)) in evidence.files().iter().zip(GFX942_OCML_PROVIDER_FILES_V1) {
        if actual.basename() != basename
            || decode_sha256_hex(sha256).as_ref() != Some(actual.sha256())
        {
            return Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch);
        }
    }
    Ok(())
}

fn validate_strict_v3_gfx950_device_ffi(
    nested: &CompilerModuleHandoffV2,
) -> Result<ProductionGfx950CompilerFfiEnvelopeKindV1, WorkerV3HsacoInspectionError> {
    if nested.target().to_string() != PRODUCTION_GFX950_TARGET {
        return Ok(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi);
    }
    let kind = inspect_production_gfx950_compiler_ffi_envelope_v1(nested.envelope())
        .ok_or(WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy)?;
    let llvm = std::str::from_utf8(nested.module_bytes())
        .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy)?;
    match kind {
        ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi => {
            if llvm.contains("@__ocml_") {
                return Err(WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy);
            }
            Ok(kind)
        }
        ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            let exact_llvm = llvm.matches("declare float @__ocml_exp_f32(float)").count() == 1
                && llvm.matches("call float @__ocml_exp_f32(float ").count() >= 1
                && llvm
                    .split("@__ocml_")
                    .skip(1)
                    .all(|suffix| suffix.starts_with("exp_f32"));
            if !exact_llvm {
                return Err(WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy);
            }
            Ok(kind)
        }
    }
}

const GFX950_OCML_PROVIDER_IDENTITY_V1: &str = "gfx950-ocml-rocm-7.2.1-v1";
const GFX950_OCML_PROVIDER_DIAGNOSTIC_V1: &str = "device_library.check=identity status=ok provider=gfx950-ocml-rocm-7.2.1-v1 roots=[__ocml_exp_f32] files=9";
const GFX950_OCML_PROVIDER_FILES_V1: [(&str, &str); 9] = [
    (
        "ocml.bc",
        "2e3451857fcf47b931c5c5a29e9c42a6ddc3099c8359079441a9a06a217ead7e",
    ),
    (
        "ockl.bc",
        "8320aec59c4dc87cb28fdb374a44a55088a6258b59dffae4a85e8eacec8be456",
    ),
    (
        "oclc_daz_opt_off.bc",
        "3b2344acba86e174b87961e8a5e4a164ab61addf8c8a035e9b6dcd03ddab23fa",
    ),
    (
        "oclc_unsafe_math_off.bc",
        "a500bc03fd046bcd7806938ea323758e5c9ba8d56cfd767cef71612b3bd87d37",
    ),
    (
        "oclc_finite_only_off.bc",
        "e1d1fddf85577b078d02a07212f670324e1e157d1b6608a8c765ad3c171a7b29",
    ),
    (
        "oclc_correctly_rounded_sqrt_on.bc",
        "3b2344acba86e174b87961e8a5e4a164ab61addf8c8a035e9b6dcd03ddab23fa",
    ),
    (
        "oclc_wavefrontsize64_on.bc",
        "9560b0d120b9e7c6b28a56a87eeed4ae155b60dec54152700ff9f60b69de1259",
    ),
    (
        "oclc_isa_version_950.bc",
        "9ea1498966ac0b4d0a54677501a847cb1ee932768e78576613d42985bf394d34",
    ),
    (
        "oclc_abi_version_600.bc",
        "79d3d09404f5df01c484dc15cc64583c7c1803234463eee6505226f0186a71b1",
    ),
];

fn validate_strict_v3_gfx950_provider_exchanges(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
    kind: ProductionGfx950CompilerFfiEnvelopeKindV1,
    bootstrap: &crate::InertProtectedCompilerHandoffExecutionV3,
    replay: &crate::InertProtectedCompilerHandoffExecutionV3,
) -> Result<(), WorkerV3HsacoInspectionError> {
    if source.handoff().module_handoff().target().to_string() != PRODUCTION_GFX950_TARGET {
        return Ok(());
    }
    let bootstrap_exchange = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        bootstrap.response().canonical_bytes(),
    )
    .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch)?;
    let replay_exchange = InertDecodedWorkerExchangeV2::decode(
        source.exact_replay_request_bytes(),
        replay.response().canonical_bytes(),
    )
    .map_err(|_| WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch)?;

    match kind {
        ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi => {
            for exchange in [&bootstrap_exchange, &replay_exchange] {
                validate_strict_v3_gfx950_provider_exchange(kind, exchange)?;
            }
            Ok(())
        }
        ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            for exchange in [&bootstrap_exchange, &replay_exchange] {
                validate_strict_v3_gfx950_provider_exchange(kind, exchange)?;
            }
            if bootstrap_exchange.response().device_library_provider()
                != replay_exchange.response().device_library_provider()
            {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch,
                );
            }
            Ok(())
        }
    }
}

fn validate_strict_v3_gfx950_provider_exchange(
    kind: ProductionGfx950CompilerFfiEnvelopeKindV1,
    exchange: &InertDecodedWorkerExchangeV2,
) -> Result<(), WorkerV3HsacoInspectionError> {
    let request = exchange.request();
    if !request.external_providers().is_empty()
        || request.target().to_string() != PRODUCTION_GFX950_TARGET
        || request.code_object_version() != CodeObjectVersion::V6
    {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch);
    }
    match kind {
        ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi => {
            if !request.import_symbols().is_empty()
                || exchange.response().device_library_provider().is_some()
            {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch,
                );
            }
        }
        ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            if request.import_symbols() != ["__ocml_exp_f32"]
                || !exchange
                    .response()
                    .diagnostics()
                    .iter()
                    .any(|value| value == GFX950_OCML_PROVIDER_DIAGNOSTIC_V1)
            {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch,
                );
            }
            validate_gfx950_ocml_provider_evidence(
                exchange.response().device_library_provider().ok_or(
                    WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch,
                )?,
            )?;
        }
    }
    Ok(())
}

fn validate_gfx950_ocml_provider_evidence(
    evidence: &crate::WorkerDeviceLibraryProviderEvidenceV1,
) -> Result<(), WorkerV3HsacoInspectionError> {
    if evidence.provider_identity() != GFX950_OCML_PROVIDER_IDENTITY_V1
        || evidence.target().to_string() != PRODUCTION_GFX950_TARGET
        || evidence.code_object_version() != CodeObjectVersion::V6
        || evidence.import_symbols() != ["__ocml_exp_f32"]
        || evidence.files().len() != GFX950_OCML_PROVIDER_FILES_V1.len()
    {
        return Err(WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch);
    }
    for (actual, (basename, sha256)) in evidence.files().iter().zip(GFX950_OCML_PROVIDER_FILES_V1) {
        if actual.basename() != basename
            || decode_sha256_hex(sha256).as_ref() != Some(actual.sha256())
        {
            return Err(WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch);
        }
    }
    Ok(())
}

fn decode_sha256_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut result = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (decode_hex_nibble(pair[0])? << 4) | decode_hex_nibble(pair[1])?;
    }
    Some(result)
}

const fn decode_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
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
) -> Result<Vec<String>, WorkerV3HsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV3HsacoInspectionError::DefinedSymbolInspection)?;
    let mut symbols = BTreeSet::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || (!symbol.is_global() && !symbol.is_weak()) {
            continue;
        }
        let name = symbol
            .name()
            .map_err(|_| WorkerV3HsacoInspectionError::DefinedSymbolInspection)?;
        if name.is_empty() || !symbols.insert(name.to_owned()) {
            return Err(WorkerV3HsacoInspectionError::DefinedSymbolInspection);
        }
    }
    if symbols.len() > MAX_WORKER_SYMBOLS {
        return Err(WorkerV3HsacoInspectionError::DefinedSymbolInspection);
    }
    Ok(symbols.into_iter().collect())
}

fn inspect_descriptor_section(
    bytes: &[u8],
) -> Result<CanonicalDescriptorSectionObservationV1, WorkerV3HsacoInspectionError> {
    let file = object::File::parse(bytes)
        .map_err(|_| WorkerV3HsacoInspectionError::DefinedSymbolInspection)?;
    for section in file.sections() {
        if section
            .name()
            .map_err(|_| WorkerV3HsacoInspectionError::DefinedSymbolInspection)?
            == DEVICE_DESCRIPTOR_SECTION_NAME
        {
            return Ok(
                CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
            );
        }
    }
    Ok(CanonicalDescriptorSectionObservationV1::Missing)
}

fn encode_descriptor_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u64(&mut bytes, inspected.bindings().len() as u64);
    for binding in inspected.bindings() {
        let descriptor = binding.descriptor();
        push_u64(&mut bytes, binding.kernel_index() as u64);
        push_u64(&mut bytes, binding.descriptor_address());
        push_u64(&mut bytes, binding.descriptor_file_offset());
        push_u64(&mut bytes, binding.entry_address());
        push_u64(&mut bytes, binding.entry_file_offset());
        push_u64(&mut bytes, binding.entry_size());
        push_u32(&mut bytes, descriptor.group_segment_fixed_size());
        push_u32(&mut bytes, descriptor.private_segment_fixed_size());
        push_u32(&mut bytes, descriptor.kernarg_size());
        push_i64(&mut bytes, descriptor.kernel_code_entry_byte_offset());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc3());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc1());
        push_u32(&mut bytes, descriptor.compute_pgm_rsrc2());
        push_u16(&mut bytes, descriptor.kernel_code_properties());
        push_u16(&mut bytes, descriptor.kernarg_preload());
    }
    bytes
}

fn encode_abi_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    let metadata = inspected.inspection();
    push_u32(&mut bytes, metadata.metadata_version().major());
    push_u32(&mut bytes, metadata.metadata_version().minor());
    push_u64(&mut bytes, metadata.kernels().len() as u64);
    for kernel in metadata.kernels() {
        push_text(&mut bytes, kernel.name());
        push_text(&mut bytes, kernel.symbol());
        push_u64(&mut bytes, kernel.kernarg_segment_size());
        push_u64(&mut bytes, kernel.kernarg_segment_alignment());
        push_optional_u64(&mut bytes, kernel.implicit_argument_offset());
        push_u64(&mut bytes, kernel.implicit_argument_size());
        push_u64(&mut bytes, kernel.explicit_arguments().len() as u64);
        for argument in kernel.explicit_arguments() {
            push_explicit_argument(&mut bytes, argument);
        }
        push_u64(&mut bytes, kernel.hidden_arguments().len() as u64);
        for argument in kernel.hidden_arguments() {
            push_hidden_argument(&mut bytes, *argument);
        }
    }
    bytes
}

fn encode_resource_observation_preimage(inspected: &InspectedKernelBindings) -> Vec<u8> {
    let mut bytes = Vec::new();
    let kernels = inspected.inspection().kernels();
    push_u64(&mut bytes, kernels.len() as u64);
    for kernel in kernels {
        push_text(&mut bytes, kernel.name());
        push_u64(&mut bytes, kernel.group_segment_fixed_size());
        push_u64(&mut bytes, kernel.private_segment_fixed_size());
        push_u32(&mut bytes, kernel.wavefront_size());
        push_u16(&mut bytes, kernel.sgpr_count());
        push_u16(&mut bytes, kernel.vgpr_count());
        push_optional_u32(&mut bytes, kernel.agpr_count());
        push_optional_u32(&mut bytes, kernel.sgpr_spill_count());
        push_optional_u32(&mut bytes, kernel.vgpr_spill_count());
        push_u32(&mut bytes, kernel.max_flat_workgroup_size());
        push_optional_dimensions(&mut bytes, kernel.required_workgroup_size());
        for limit in kernel.max_workgroups() {
            push_optional_u32(&mut bytes, limit);
        }
        push_optional_dimensions(&mut bytes, kernel.cluster_dims());
        push_optional_bool(&mut bytes, kernel.uniform_work_group_size_declaration());
        push_optional_bool(&mut bytes, kernel.uses_dynamic_stack_declaration());
        push_optional_bool(&mut bytes, kernel.workgroup_processor_mode());
    }
    bytes
}

fn calculate_observation_identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(preimage);
    hasher.finalize().into()
}

fn push_explicit_argument(bytes: &mut Vec<u8>, argument: &ExplicitArgument) {
    push_optional_text(bytes, argument.name());
    push_optional_text(bytes, argument.type_name());
    push_u64(bytes, argument.offset());
    push_u64(bytes, argument.size());
    push_optional_u64(bytes, argument.alignment());
    bytes.push(explicit_value_kind_tag(argument.value_kind()));
    push_optional_tag(bytes, argument.value_type().map(explicit_value_type_tag));
    push_optional_tag(bytes, argument.address_space().map(address_space_tag));
    push_optional_tag(bytes, argument.access().map(argument_access_tag));
    push_optional_tag(bytes, argument.actual_access().map(argument_access_tag));
    push_optional_u64(bytes, argument.pointee_alignment());
    push_optional_bool(bytes, argument.is_const());
    push_optional_bool(bytes, argument.is_restrict());
    push_optional_bool(bytes, argument.is_volatile());
    push_optional_bool(bytes, argument.is_pipe());
}

fn push_hidden_argument(bytes: &mut Vec<u8>, argument: HiddenArgument) {
    push_u64(bytes, argument.offset());
    push_u64(bytes, argument.size());
    bytes.push(hidden_value_kind_tag(argument.value_kind()));
}

fn push_optional_text(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_text(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_u64(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_u32(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_optional_bool(bytes: &mut Vec<u8>, value: Option<bool>) {
    match value {
        Some(value) => bytes.extend_from_slice(&[1, u8::from(value)]),
        None => bytes.push(0),
    }
}

fn push_optional_tag(bytes: &mut Vec<u8>, value: Option<u8>) {
    match value {
        Some(value) => bytes.extend_from_slice(&[1, value]),
        None => bytes.push(0),
    }
}

fn push_optional_dimensions(bytes: &mut Vec<u8>, value: Option<[u32; 3]>) {
    match value {
        Some(value) => {
            bytes.push(1);
            for dimension in value {
                push_u32(bytes, dimension);
            }
        }
        None => bytes.push(0),
    }
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    push_u64(bytes, value.len() as u64);
    bytes.extend_from_slice(value.as_bytes());
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

const fn explicit_value_kind_tag(kind: ExplicitValueKind) -> u8 {
    match kind {
        ExplicitValueKind::ByValue => 0,
        ExplicitValueKind::GlobalBuffer => 1,
        ExplicitValueKind::DynamicSharedPointer => 2,
        ExplicitValueKind::Sampler => 3,
        ExplicitValueKind::Image => 4,
        ExplicitValueKind::Pipe => 5,
        ExplicitValueKind::Queue => 6,
    }
}

const fn explicit_value_type_tag(value_type: ExplicitValueType) -> u8 {
    match value_type {
        ExplicitValueType::Struct => 0,
        ExplicitValueType::I8 => 1,
        ExplicitValueType::U8 => 2,
        ExplicitValueType::I16 => 3,
        ExplicitValueType::U16 => 4,
        ExplicitValueType::F16 => 5,
        ExplicitValueType::I32 => 6,
        ExplicitValueType::U32 => 7,
        ExplicitValueType::F32 => 8,
        ExplicitValueType::I64 => 9,
        ExplicitValueType::U64 => 10,
        ExplicitValueType::F64 => 11,
    }
}

const fn address_space_tag(address_space: ArgumentAddressSpace) -> u8 {
    match address_space {
        ArgumentAddressSpace::Private => 0,
        ArgumentAddressSpace::Global => 1,
        ArgumentAddressSpace::Constant => 2,
        ArgumentAddressSpace::Local => 3,
        ArgumentAddressSpace::Generic => 4,
        ArgumentAddressSpace::Region => 5,
    }
}

const fn argument_access_tag(access: ArgumentAccess) -> u8 {
    match access {
        ArgumentAccess::ReadOnly => 0,
        ArgumentAccess::WriteOnly => 1,
        ArgumentAccess::ReadWrite => 2,
    }
}

const fn hidden_value_kind_tag(kind: HiddenValueKind) -> u8 {
    match kind {
        HiddenValueKind::BlockCountX => 0,
        HiddenValueKind::BlockCountY => 1,
        HiddenValueKind::BlockCountZ => 2,
        HiddenValueKind::GroupSizeX => 3,
        HiddenValueKind::GroupSizeY => 4,
        HiddenValueKind::GroupSizeZ => 5,
        HiddenValueKind::RemainderX => 6,
        HiddenValueKind::RemainderY => 7,
        HiddenValueKind::RemainderZ => 8,
        HiddenValueKind::GlobalOffsetX => 9,
        HiddenValueKind::GlobalOffsetY => 10,
        HiddenValueKind::GlobalOffsetZ => 11,
        HiddenValueKind::GridDimensions => 12,
        HiddenValueKind::None => 13,
        HiddenValueKind::PrintfBuffer => 14,
        HiddenValueKind::HostcallBuffer => 15,
        HiddenValueKind::HeapV1 => 16,
        HiddenValueKind::DefaultQueue => 17,
        HiddenValueKind::CompletionAction => 18,
        HiddenValueKind::MultigridSyncArgument => 19,
        HiddenValueKind::DynamicLdsSize => 20,
        HiddenValueKind::PrivateBase => 21,
        HiddenValueKind::SharedBase => 22,
        HiddenValueKind::QueuePointer => 23,
    }
}

fn calculate_policy_identity(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_envelope: CompilerFfiEnvelopeIdentityV1,
    manifest: &CompilerModuleSymbolManifestV1,
    observed_kernels: &[ObservedWorkerKernelSymbolsV1],
    expected_defined_symbols: &[String],
    launch: WorkerV3LaunchContractV1,
) -> WorkerV3HsacoPolicyIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(FROZEN_POLICY_IDENTITY_CODEC_DOMAIN_V1);
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
    WorkerV3HsacoPolicyIdentityV1(hasher.finalize().into())
}

fn calculate_response_identity(bytes: &[u8]) -> SealedWorkerResponseIdentityV1 {
    SealedWorkerResponseIdentityV1(calculate_response_identity_bytes_v1(bytes))
}

pub(crate) fn calculate_response_identity_bytes_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FROZEN_RESPONSE_IDENTITY_CODEC_DOMAIN_V1);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn calculate_protected_v3_inspection_identity(
    source: &InertProtectedFirstBuildWorkerV3EvidenceV1,
    raw: &SharedWorkerV3HsacoInspectionV1,
    response: SealedWorkerResponseIdentityV1,
) -> InspectedProtectedWorkerV3HsacoIdentityV1 {
    let binding = source.binding();
    let expected = binding.expectation();
    let worker = source.worker_measurement();
    let outer_identity = expected.outer_handoff_identity();
    let nested_identity = expected.nested_handoff_identity();
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_V3_INSPECTION_IDENTITY_DOMAIN_V1);
    hasher.update(source.identity().as_bytes());
    hasher.update(binding.identity().as_bytes());
    hash_attempt(&mut hasher, expected.attempt());
    hasher.update([expected.slot() as u8]);
    hasher.update(expected.transaction_identity().as_bytes());
    hasher.update(outer_identity.sha256());
    hasher.update(outer_identity.byte_len().to_le_bytes());
    hasher.update(expected.capsule_sha256());
    hasher.update(expected.capsule_byte_len().to_le_bytes());
    hasher.update(expected.invocation_digest());
    hasher.update(expected.pair_binding_sha256());
    hasher.update(expected.pair_binding_byte_len().to_le_bytes());
    hasher.update(nested_identity.sha256());
    hasher.update(nested_identity.byte_len().to_le_bytes());
    hasher.update(expected.final_commitment_receipt_sha256());
    hasher.update(expected.final_commitment_receipt_byte_len().to_le_bytes());
    hasher.update(expected.final_commitment_sha256());
    hasher.update(expected.final_commitment_byte_len().to_le_bytes());
    hash_compiler_closure_v2(&mut hasher, expected.compiler_closure());
    hash_bytes(&mut hasher, source.handoff().canonical_bytes());
    hash_content(&mut hasher, worker.executable());
    hash_text(&mut hasher, worker.worker_build_identity());
    hash_text(&mut hasher, worker.llvm_build_identity());
    hash_bytes(&mut hasher, &source.plan().canonical_bytes());
    hash_bytes(&mut hasher, source.bootstrap_request_bytes());
    hash_bytes(&mut hasher, source.bootstrap().response().canonical_bytes());
    hash_bytes(&mut hasher, source.exact_replay_request_bytes());
    hash_bytes(
        &mut hasher,
        source.exact_replay().response().canonical_bytes(),
    );
    hasher.update(response.0);
    hash_content(&mut hasher, source.output_identity());
    hash_bytes(&mut hasher, source.output_bytes());
    hash_text(&mut hasher, &raw.policy.target.to_string());
    hasher.update([code_object_version_tag(raw.policy.code_object_version)]);
    hasher.update(raw.policy.identity.0);
    hasher.update([descriptor_section_tag(raw.descriptor_section)]);
    hasher.update(raw.descriptor_identity);
    hasher.update(raw.abi_identity);
    hasher.update(raw.resource_identity);
    InspectedProtectedWorkerV3HsacoIdentityV1(hasher.finalize().into())
}

fn hash_attempt(hasher: &mut Sha256, attempt: BuildAttempt) {
    hasher.update(attempt.generation().to_le_bytes());
    hasher.update(attempt.session().as_bytes());
    hasher.update(attempt.invocation().as_bytes());
}

fn hash_compiler_closure_v2(hasher: &mut Sha256, closure: CompilerClosureV2) {
    hasher.update(closure.cargo_executable_sha256());
    hasher.update(closure.cargo_binding_trampoline_sha256());
    hasher.update(closure.cargo_fe2o3_binding_wrapper_sha256());
    hasher.update(closure.rustc_executable_sha256());
    hasher.update(closure.rustc_runtime_tree_sha256());
    hasher.update(closure.codegen_backend_sha256());
    hasher.update(
        closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    hasher.update(closure.identity_sha256());
}

fn hash_content(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
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

const fn descriptor_section_tag(section: CanonicalDescriptorSectionObservationV1) -> u8 {
    match section {
        CanonicalDescriptorSectionObservationV1::Missing => 0,
        CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection => 1,
    }
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

#[cfg(test)]
mod exact_production_target_tests {
    use super::*;
    use crate::{
        WorkerCompilerFfiEnvelopeIdentityV2, WorkerInputKindV1, WorkerInputV1,
        WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1,
        worker_protocol_v2::{
            SealedWorkerRequestV2Parts, WorkerRequestV2, WorkerResponseReplayMetadataV1,
            reconstruct_complete_worker_response_v2,
        },
    };
    use fe2o3_compiler_ffi::{
        CompilerFfiEnvelopeV1, CompilerModuleKindV1, CompilerModuleSymbolRoleV1,
        construct_production_gfx942_ocml_exp_envelope_v1,
        construct_production_gfx950_ocml_exp_envelope_v1,
    };

    #[test]
    fn production_raw_hsaco_admission_accepts_only_canonical_production_targets() {
        for accepted in [PRODUCTION_GFX942_TARGET, PRODUCTION_GFX950_TARGET] {
            assert!(target_is_supported(
                DeviceTargetV1::parse(accepted).unwrap()
            ));
        }

        for rejected in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx950",
            "gfx950:xnack+",
            "gfx950:sramecc+:xnack-",
            "gfx90a:xnack-",
        ] {
            assert!(!target_is_supported(
                DeviceTargetV1::parse(rejected).unwrap()
            ));
        }
    }

    fn production_handoff(
        target: &str,
        kind: CompilerModuleKindV1,
        envelope: CompilerFfiEnvelopeV1,
        llvm: &[u8],
        import: bool,
    ) -> CompilerModuleHandoffV2 {
        let target = DeviceTargetV1::parse(target).unwrap();
        let mut symbols = vec![
            (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        ];
        if import {
            symbols.push((
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                "__ocml_exp_f32",
            ));
        }
        CompilerModuleHandoffV2::new(
            kind,
            target,
            CompilerCodeObjectVersion::V6,
            envelope,
            CompilerModuleSymbolManifestV1::new(symbols).unwrap(),
            llvm,
        )
        .unwrap()
    }

    fn gfx942_handoff(
        envelope: CompilerFfiEnvelopeV1,
        llvm: &[u8],
        import: bool,
    ) -> CompilerModuleHandoffV2 {
        production_handoff(
            PRODUCTION_GFX942_TARGET,
            CompilerModuleKindV1::LlvmTextIr,
            envelope,
            llvm,
            import,
        )
    }

    fn gfx950_handoff(
        envelope: CompilerFfiEnvelopeV1,
        llvm: &[u8],
        import: bool,
    ) -> CompilerModuleHandoffV2 {
        production_handoff(
            PRODUCTION_GFX950_TARGET,
            CompilerModuleKindV1::LlvmTextIr,
            envelope,
            llvm,
            import,
        )
    }

    #[test]
    fn strict_v3_gfx942_no_ffi_and_ocml_shapes_are_exact() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET).unwrap();
        let no_ffi = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target,
            CompilerCodeObjectVersion::V6,
        )
        .unwrap();
        let no_ffi_handoff = gfx942_handoff(
            no_ffi,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            false,
        );
        assert_eq!(
            validate_strict_v3_gfx942_device_ffi(&no_ffi_handoff),
            Ok(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi)
        );

        let envelope = construct_production_gfx942_ocml_exp_envelope_v1([0x37; 32]).unwrap();
        let exp_handoff = gfx942_handoff(
            envelope,
            b"target triple = \"amdgcn-amd-amdhsa\"\n\
              declare float @__ocml_exp_f32(float)\n\
              define amdgpu_kernel void @kernel() {\n\
                %value = call float @__ocml_exp_f32(float 0.000000e+00)\n\
                ret void\n\
              }\n",
            true,
        );
        assert!(matches!(
            validate_strict_v3_gfx942_device_ffi(&exp_handoff),
            Ok(ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. })
        ));
    }

    #[test]
    fn strict_v3_gfx942_rejects_bitcode_for_every_ffi_shape() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET).unwrap();
        let no_ffi = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target,
            CompilerCodeObjectVersion::V6,
        )
        .unwrap();
        let no_ffi_bitcode = production_handoff(
            PRODUCTION_GFX942_TARGET,
            CompilerModuleKindV1::LlvmBitcode,
            no_ffi,
            b"bitcode-no-ffi",
            false,
        );
        assert_eq!(
            validate_strict_v3_gfx942_device_ffi(&no_ffi_bitcode),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)
        );

        let exp = construct_production_gfx942_ocml_exp_envelope_v1([0x37; 32]).unwrap();
        let exp_bitcode = production_handoff(
            PRODUCTION_GFX942_TARGET,
            CompilerModuleKindV1::LlvmBitcode,
            exp,
            b"bitcode-exp",
            true,
        );
        assert_eq!(
            validate_strict_v3_gfx942_device_ffi(&exp_bitcode),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)
        );
    }

    #[test]
    fn strict_v3_gfx942_rejects_hidden_or_substituted_ocml_llvm() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET).unwrap();
        let no_ffi = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target,
            CompilerCodeObjectVersion::V6,
        )
        .unwrap();
        let hidden = gfx942_handoff(
            no_ffi,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndeclare float @__ocml_exp_f32(float)\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            false,
        );
        assert_eq!(
            validate_strict_v3_gfx942_device_ffi(&hidden),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)
        );

        let envelope = construct_production_gfx942_ocml_exp_envelope_v1([0x37; 32]).unwrap();
        let missing_call = gfx942_handoff(
            envelope,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndeclare float @__ocml_exp_f32(float)\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            true,
        );
        assert_eq!(
            validate_strict_v3_gfx942_device_ffi(&missing_call),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942DeviceFfiPolicy)
        );
    }

    fn push_test_u32(output: &mut Vec<u8>, value: usize) {
        output.extend_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
    }

    #[derive(Clone, Copy)]
    struct Gfx942ProviderFixtureV1 {
        provider_identity: &'static str,
        provider_target: &'static str,
        provider_cov: u8,
        provider_import: &'static str,
        diagnostic: &'static str,
        basenames: [&'static str; 4],
        file_digests: [[u8; 32]; 4],
    }

    impl Gfx942ProviderFixtureV1 {
        fn exact() -> Self {
            Self {
                provider_identity: GFX942_OCML_PROVIDER_IDENTITY_V1,
                provider_target: PRODUCTION_GFX942_TARGET,
                provider_cov: 6,
                provider_import: "__ocml_exp_f32",
                diagnostic: GFX942_OCML_PROVIDER_DIAGNOSTIC_V1,
                basenames: [
                    GFX942_OCML_PROVIDER_FILES_V1[0].0,
                    GFX942_OCML_PROVIDER_FILES_V1[1].0,
                    GFX942_OCML_PROVIDER_FILES_V1[2].0,
                    GFX942_OCML_PROVIDER_FILES_V1[3].0,
                ],
                file_digests: [
                    decode_sha256_hex(GFX942_OCML_PROVIDER_FILES_V1[0].1).unwrap(),
                    decode_sha256_hex(GFX942_OCML_PROVIDER_FILES_V1[1].1).unwrap(),
                    decode_sha256_hex(GFX942_OCML_PROVIDER_FILES_V1[2].1).unwrap(),
                    decode_sha256_hex(GFX942_OCML_PROVIDER_FILES_V1[3].1).unwrap(),
                ],
            }
        }
    }

    fn gfx942_provider_body(fixture: Gfx942ProviderFixtureV1) -> Vec<u8> {
        let mut provider = Vec::new();
        for value in [fixture.provider_identity, fixture.provider_target] {
            push_test_u32(&mut provider, value.len());
            provider.extend_from_slice(value.as_bytes());
        }
        provider.push(fixture.provider_cov);
        push_test_u32(&mut provider, 1);
        push_test_u32(&mut provider, fixture.provider_import.len());
        provider.extend_from_slice(fixture.provider_import.as_bytes());
        push_test_u32(&mut provider, fixture.basenames.len());
        for (basename, digest) in fixture.basenames.into_iter().zip(fixture.file_digests) {
            push_test_u32(&mut provider, basename.len());
            provider.extend_from_slice(basename.as_bytes());
            provider.extend_from_slice(&digest);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"FE2O3/DEVICE-LIBRARY-PROVIDER-MANIFEST/V1\0");
        hasher.update((provider.len() as u64).to_le_bytes());
        hasher.update(&provider);
        provider.extend_from_slice(&hasher.finalize());
        provider
    }

    fn gfx942_provider_exchange(
        fixture: Gfx942ProviderFixtureV1,
    ) -> Result<InertDecodedWorkerExchangeV2, crate::WorkerProtocolError> {
        let request = WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x41; 32],
            llvm_build_identity: "llvm-gfx942-test".to_owned(),
            worker_build_identity: "worker-gfx942-test".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x42; 32], 4096),
            target: DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET).unwrap(),
            code_object_version: CodeObjectVersion::V6,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_test_bytes([0x43; 32]),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmTextIr,
                b"compiler module".to_vec(),
            )
            .unwrap(),
            external_providers: Vec::new(),
            import_symbols: vec!["__ocml_exp_f32".to_owned()],
            export_symbols: vec!["kernel".to_owned()],
            final_symbols: vec!["__ocml_exp_f32".to_owned(), "kernel".to_owned()],
            output: WorkerOutputConstraintsV1::new(4096).unwrap(),
        })
        .unwrap();
        let mut diagnostics_body = Vec::new();
        push_test_u32(&mut diagnostics_body, 1);
        push_test_u32(&mut diagnostics_body, fixture.diagnostic.len());
        diagnostics_body.extend_from_slice(fixture.diagnostic.as_bytes());
        let provider_body = gfx942_provider_body(fixture);
        let response = reconstruct_complete_worker_response_v2(
            &request,
            b"inert hsaco",
            WorkerResponseReplayMetadataV1::from_test_bodies(
                &diagnostics_body,
                Some(&provider_body),
            ),
        )?;
        InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), response.canonical_bytes())
    }

    #[test]
    fn strict_v3_gfx942_requires_the_ordered_authorized_provider_files() {
        let kind = ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
            canonical_kernel_ir_identity: [0x37; 32],
        };
        let exact = gfx942_provider_exchange(Gfx942ProviderFixtureV1::exact()).unwrap();
        assert_eq!(
            validate_strict_v3_gfx942_provider_exchange(kind, &exact),
            Ok(())
        );

        let zero_digest = gfx942_provider_exchange(Gfx942ProviderFixtureV1 {
            file_digests: [[0x51; 32], [0; 32], [0x53; 32], [0x54; 32]],
            ..Gfx942ProviderFixtureV1::exact()
        })
        .unwrap();
        assert_eq!(
            validate_strict_v3_gfx942_provider_exchange(kind, &zero_digest),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)
        );

        let mut substituted_digests = Gfx942ProviderFixtureV1::exact().file_digests;
        substituted_digests[0] = [0x51; 32];
        let substituted_digest = gfx942_provider_exchange(Gfx942ProviderFixtureV1 {
            file_digests: substituted_digests,
            ..Gfx942ProviderFixtureV1::exact()
        })
        .unwrap();
        assert_eq!(
            validate_strict_v3_gfx942_provider_exchange(kind, &substituted_digest),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)
        );

        let reordered = gfx942_provider_exchange(Gfx942ProviderFixtureV1 {
            basenames: [
                "ocml.bc",
                "oclc_unsafe_math_off.bc",
                "oclc_isa_version_942.bc",
                "oclc_finite_only_off.bc",
            ],
            file_digests: [[0x51; 32], [0x53; 32], [0x52; 32], [0x54; 32]],
            ..Gfx942ProviderFixtureV1::exact()
        })
        .unwrap();
        assert_eq!(
            validate_strict_v3_gfx942_provider_exchange(kind, &reordered),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)
        );

        for substituted in [
            Gfx942ProviderFixtureV1 {
                provider_identity: "gfx942-ocml-substituted-v1",
                ..Gfx942ProviderFixtureV1::exact()
            },
            Gfx942ProviderFixtureV1 {
                diagnostic: "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_sin_f32] files=4",
                ..Gfx942ProviderFixtureV1::exact()
            },
        ] {
            let exchange = gfx942_provider_exchange(substituted).unwrap();
            assert_eq!(
                validate_strict_v3_gfx942_provider_exchange(kind, &exchange),
                Err(WorkerV3HsacoInspectionError::StrictV3Gfx942OcmlProviderClosureMismatch)
            );
        }

        for protocol_mismatch in [
            Gfx942ProviderFixtureV1 {
                provider_target: PRODUCTION_GFX950_TARGET,
                ..Gfx942ProviderFixtureV1::exact()
            },
            Gfx942ProviderFixtureV1 {
                provider_cov: 5,
                ..Gfx942ProviderFixtureV1::exact()
            },
            Gfx942ProviderFixtureV1 {
                provider_import: "__ocml_sin_f32",
                ..Gfx942ProviderFixtureV1::exact()
            },
        ] {
            assert!(gfx942_provider_exchange(protocol_mismatch).is_err());
        }
    }

    #[test]
    fn strict_v3_gfx950_gemm_retains_no_device_ffi() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX950_TARGET).unwrap();
        let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target,
            CompilerCodeObjectVersion::V6,
        )
        .unwrap();
        let handoff = gfx950_handoff(
            envelope,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            false,
        );
        assert_eq!(
            validate_strict_v3_gfx950_device_ffi(&handoff),
            Ok(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi)
        );
    }

    #[test]
    fn strict_v3_gfx950_no_ffi_rejects_external_provider_injection() {
        let request = WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x11; 32],
            llvm_build_identity: "llvm-gfx950-test".to_owned(),
            worker_build_identity: "worker-gfx950-test".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 4096),
            target: DeviceTargetV1::parse(PRODUCTION_GFX950_TARGET).unwrap(),
            code_object_version: CodeObjectVersion::V6,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_test_bytes([0x33; 32]),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmTextIr,
                b"compiler module".to_vec(),
            )
            .unwrap(),
            external_providers: vec![
                WorkerInputV1::new(
                    WorkerInputKindV1::LlvmBitcode,
                    b"injected provider".to_vec(),
                )
                .unwrap(),
            ],
            import_symbols: Vec::new(),
            export_symbols: vec!["kernel".to_owned()],
            final_symbols: vec!["kernel".to_owned()],
            output: WorkerOutputConstraintsV1::new(4096).unwrap(),
        })
        .unwrap();
        let diagnostics_body = 0_u32.to_le_bytes();
        let response = reconstruct_complete_worker_response_v2(
            &request,
            b"inert hsaco",
            WorkerResponseReplayMetadataV1::from_test_bodies(&diagnostics_body, None),
        )
        .unwrap();
        let exchange = InertDecodedWorkerExchangeV2::decode(
            request.canonical_bytes(),
            response.canonical_bytes(),
        )
        .unwrap();

        assert_eq!(
            validate_strict_v3_gfx950_provider_exchange(
                ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi,
                &exchange,
            ),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx950OcmlProviderClosureMismatch)
        );
    }

    #[test]
    fn strict_v3_gfx950_ocml_requires_the_measured_provider_exchange() {
        let envelope = construct_production_gfx950_ocml_exp_envelope_v1([0x38; 32]).unwrap();
        let handoff = gfx950_handoff(
            envelope,
            b"target triple = \"amdgcn-amd-amdhsa\"\n\
              declare float @__ocml_exp_f32(float)\n\
              define amdgpu_kernel void @kernel() {\n\
                %value = call float @__ocml_exp_f32(float 0.000000e+00)\n\
                ret void\n\
              }\n",
            true,
        );
        assert!(matches!(
            validate_strict_v3_gfx950_device_ffi(&handoff),
            Ok(ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. })
        ));
        assert_eq!(
            decode_sha256_hex(GFX950_OCML_PROVIDER_FILES_V1[0].1)
                .unwrap()
                .len(),
            32
        );
    }

    #[test]
    fn strict_v3_gfx950_rejects_hidden_or_substituted_ocml_llvm() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX950_TARGET).unwrap();
        let no_ffi = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target,
            CompilerCodeObjectVersion::V6,
        )
        .unwrap();
        let hidden = gfx950_handoff(
            no_ffi,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndeclare float @__ocml_exp_f32(float)\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            false,
        );
        assert_eq!(
            validate_strict_v3_gfx950_device_ffi(&hidden),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy)
        );

        let envelope = construct_production_gfx950_ocml_exp_envelope_v1([0x38; 32]).unwrap();
        let missing_call = gfx950_handoff(
            envelope,
            b"target triple = \"amdgcn-amd-amdhsa\"\ndeclare float @__ocml_exp_f32(float)\ndefine amdgpu_kernel void @kernel() { ret void }\n",
            true,
        );
        assert_eq!(
            validate_strict_v3_gfx950_device_ffi(&missing_call),
            Err(WorkerV3HsacoInspectionError::StrictV3Gfx950DeviceFfiPolicy)
        );
    }
}
