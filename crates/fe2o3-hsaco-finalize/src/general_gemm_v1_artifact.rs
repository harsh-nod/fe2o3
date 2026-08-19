//! Exact, inert Worker V2 and post-link observations for general GEMM V1.
//!
//! This module consumes structural compiler output. It never authenticates the
//! frontend correspondence, proof, publication, loading, or launch boundaries.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{CompilerDescriptorSourceIdentityV1, CompilerModuleHandoffIdentityV2};
use fe2o3_general_gemm_compiler::{
    GeneralGemmMachineBindingIdentityV1, GeneralGemmPlironProjectionIdentityV1,
    GeneralGemmScheduleIdentityV1, GeneralGemmScheduleV1, GeneralGemmStructuralMachineV1,
    GeneralGemmSymbolicArtifactIdentityV1, GeneralGemmSymbolicCompilationIdentityV1,
    GeneralGemmSymbolicStructuralMachineV1,
};
use fe2o3_llvm_handoff::{
    CallTargetV2, FunctionAttributeV2, HandoffIdentityV2, InstructionKindV2, IntrinsicV2,
};
use fe2o3_llvm_text::LlvmAssemblySha256V2;
use object::{Object as _, ObjectSection as _, ObjectSymbol as _};
use sha2::{Digest, Sha256};

use crate::worker_v2_hsaco_finalization::finalize_allocated_general_gemm_worker_v2_hsaco_v1;
use crate::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, FirstBuildWorkerV2Error,
    FirstBuildWorkerV2IdentityV1, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoIdentityV1, LinkOptionV1, PinnedWorkerV1,
    PreparedFinalizedWorkerV2HsacoV1, WorkerExecutionLimitsV1, WorkerOutputConstraintsV1,
    WorkerV2HsacoFinalizationError, WorkerV2RawHsacoInspectionError,
    WorkerV2RawHsacoPolicyIdentityV1, execute_reproducible_first_build_worker_v2,
    inspect_general_gemm_worker_v2_raw_hsaco_v1,
};

const GENERAL_GEMM_WORKER_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM/WORKER-V2/INERT-EVIDENCE/V1\0";
const GENERAL_GEMM_POST_LINK_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM/SYMBOLIC-POST-LINK-MACHINE/V1\0";
const GENERAL_GEMM_MFMA_NUMERICAL_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM/GFX942-MFMA-NUMERICAL-REFINEMENT/V1\0";
const GENERAL_GEMM_BINDING_SECTION_V1: &str = ".fe2o3.general-gemm.binding.v1";
const GENERAL_GEMM_DESCRIPTOR_SECTION_V1: &str = ".fe2o3.kd.v1";
const GENERAL_GEMM_KERNEL_SYMBOL_V1: &str = "tiled_gemm_general_v1";
const GENERAL_GEMM_DESCRIPTOR_SOURCE_SYMBOL_V1: &str = "general_gemm_descriptor_source";
const GENERAL_GEMM_POST_LINK_SUCCESS_DIAGNOSTIC_V1: &str = "post_link.check=general_gemm_v1_profile status=ok workgroup=[64,1,1] \
explicit_kernarg_size=80 kernarg_size=336 kernarg_align=8 group_size=1024 \
private_size=0 wavefront_size=64 calls=0 atomics=0 spills=0 dynamic_stack=false \
mfma=1 lds_writes=8 lds_reads=8 barriers_ir=2 barriers_isa=0 \
barrier_refinement=single_wave_elision publish_order=vmcnt0-before-lds-write \
reuse_order=lgkmcnt0-after-lds-read-before-mfma descriptor_binding=byte_exact \
compilation_binding=byte_exact";
const MFMA_F32_16X16X16BF16_1K_OPCODE_V1: u32 = 0xd3e1_0002;
const S_BARRIER_OPCODE_V1: u32 = 0xbf8a_0000;
const S_WAITCNT_VMCNT_ZERO_V1: u32 = 0xbf8c_0f70;
const S_WAITCNT_LGKMCNT_ZERO_V1: u32 = 0xbf8c_c07f;
const REFERENCE_MACHINE_SHA256_V1: [u8; 32] = [
    0x26, 0x16, 0x9b, 0x77, 0x6e, 0x8a, 0x35, 0xeb, 0x36, 0x06, 0xcb, 0x9f, 0x2a, 0xec, 0x5e, 0x52,
    0xf1, 0xac, 0x71, 0xa1, 0xb6, 0x49, 0x03, 0xa8, 0x65, 0x6d, 0xfd, 0xbd, 0x68, 0x53, 0x1e, 0x40,
];
const VECTOR_A_MACHINE_SHA256_V1: [u8; 32] = [
    0x31, 0x4f, 0x07, 0x61, 0xe1, 0xfd, 0xc2, 0x06, 0x7c, 0xc8, 0xb0, 0xc7, 0x4c, 0xb7, 0xa0, 0xba,
    0xf9, 0x5d, 0x49, 0x94, 0xa2, 0xec, 0xa1, 0x5b, 0xeb, 0x2a, 0x99, 0x66, 0x46, 0x6a, 0xfe, 0x5b,
];

/// Identity binding the exact structural machine to one measured Worker V2 execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmWorkerV2IdentityV1([u8; 32]);

impl GeneralGemmWorkerV2IdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Identity of the exact post-link machine observation retained for the final join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmPostLinkMachineIdentityV1([u8; 32]);

impl GeneralGemmPostLinkMachineIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact reason that two authenticated IR barriers may become zero ISA barriers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmBarrierRefinementV1 {
    /// One wave owns the complete 64-thread workgroup and exact wait/order checks passed.
    SingleWaveElision,
}

/// Identity of the isolated gfx942 MFMA numerical-refinement observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmMfmaNumericalRefinementIdentityV1([u8; 32]);

impl GeneralGemmMfmaNumericalRefinementIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Typed observation for the exact gfx942 BF16 MFMA numerical boundary.
///
/// This record closes only the emitted-MFMA portion of numerical refinement.
/// It grants no source, proof, artifact, publication, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmMfmaNumericalRefinementV1 {
    identity: GeneralGemmMfmaNumericalRefinementIdentityV1,
    llvm_assembly: LlvmAssemblySha256V2,
    kernel_symbol_sha256: [u8; 32],
    opcode: u32,
    count: u32,
}

impl GeneralGemmMfmaNumericalRefinementV1 {
    pub const fn identity(&self) -> GeneralGemmMfmaNumericalRefinementIdentityV1 {
        self.identity
    }

    pub const fn llvm_assembly_identity(&self) -> LlvmAssemblySha256V2 {
        self.llvm_assembly
    }

    pub const fn kernel_symbol_sha256(&self) -> &[u8; 32] {
        &self.kernel_symbol_sha256
    }

    pub const fn opcode(&self) -> u32 {
        self.opcode
    }

    pub const fn count(&self) -> u32 {
        self.count
    }

    pub const fn fp_contract_is_off(&self) -> bool {
        true
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Owning, inert record for exact general-GEMM Worker V2 execution.
///
/// This type is intentionally not `Clone`. It retains the structural compiler
/// machine so post-link inspection can compare the exact descriptor and
/// compilation-binding sections rather than accepting caller-supplied claims.
#[derive(Debug)]
pub struct InertGeneralGemmWorkerV2EvidenceV1 {
    identity: GeneralGemmWorkerV2IdentityV1,
    consumed_handoff_identity: CompilerModuleHandoffIdentityV1,
    machine: GeneralGemmStructuralMachineV1,
    worker: InertFirstBuildWorkerV2EvidenceV1,
}

/// Owning inert Worker V2 execution of a production symbolic GEMM machine.
#[derive(Debug)]
pub struct InertSymbolicGeneralGemmWorkerV2EvidenceV1 {
    identity: GeneralGemmWorkerV2IdentityV1,
    consumed_handoff_identity: CompilerModuleHandoffIdentityV1,
    machine: GeneralGemmSymbolicStructuralMachineV1,
    worker: InertFirstBuildWorkerV2EvidenceV1,
}

/// Opaque, owning observation of an exact symbolic general-GEMM post-link machine.
///
/// This value is intentionally not `Clone`. It retains canonical finalization
/// lineage and all identities needed by a later rustc-owned three-party join.
/// It is never independently authoritative.
#[derive(Debug)]
pub struct OpaqueGeneralGemmPostLinkMachineObservationV1 {
    identity: GeneralGemmPostLinkMachineIdentityV1,
    schedule: GeneralGemmScheduleV1,
    schedule_identity: GeneralGemmScheduleIdentityV1,
    symbolic_compilation: GeneralGemmSymbolicCompilationIdentityV1,
    symbolic_artifact: GeneralGemmSymbolicArtifactIdentityV1,
    projection: GeneralGemmPlironProjectionIdentityV1,
    handoff_v2: HandoffIdentityV2,
    typed_worker_admission: [u8; 32],
    llvm_assembly: LlvmAssemblySha256V2,
    compiler_handoff: CompilerModuleHandoffIdentityV2,
    consumed_handoff: CompilerModuleHandoffIdentityV1,
    machine_binding: GeneralGemmMachineBindingIdentityV1,
    descriptor_source: CompilerDescriptorSourceIdentityV1,
    symbolic_worker: GeneralGemmWorkerV2IdentityV1,
    worker_execution: FirstBuildWorkerV2IdentityV1,
    measured_worker: ContentIdentityV1,
    worker_build_identity: String,
    llvm_build_identity: String,
    worker_request: [u8; 32],
    worker_response: [u8; 32],
    raw_output: ContentIdentityV1,
    raw_inspection: InspectedRawWorkerV2HsacoIdentityV1,
    raw_policy: WorkerV2RawHsacoPolicyIdentityV1,
    finalized: FinalizedWorkerV2HsacoIdentityV1,
    finalized_output: ContentIdentityV1,
    kernel_symbol_sha256: [u8; 32],
    vector_global_loads: u32,
    barriers_ir: u32,
    barriers_isa: u32,
    barrier_refinement: GeneralGemmBarrierRefinementV1,
    mfma_numerical: GeneralGemmMfmaNumericalRefinementV1,
    prepared: PreparedFinalizedWorkerV2HsacoV1,
}

impl OpaqueGeneralGemmPostLinkMachineObservationV1 {
    pub const fn identity(&self) -> GeneralGemmPostLinkMachineIdentityV1 {
        self.identity
    }

    pub const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    pub const fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule_identity
    }

    pub const fn symbolic_compilation_identity(&self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.symbolic_compilation
    }

    pub const fn symbolic_artifact_identity(&self) -> GeneralGemmSymbolicArtifactIdentityV1 {
        self.symbolic_artifact
    }

    pub const fn projection_identity(&self) -> GeneralGemmPlironProjectionIdentityV1 {
        self.projection
    }

    pub const fn handoff_v2_identity(&self) -> HandoffIdentityV2 {
        self.handoff_v2
    }

    pub const fn typed_worker_admission_identity(&self) -> &[u8; 32] {
        &self.typed_worker_admission
    }

    pub const fn llvm_assembly_identity(&self) -> LlvmAssemblySha256V2 {
        self.llvm_assembly
    }

    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff
    }

    pub const fn consumed_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.consumed_handoff
    }

    pub const fn machine_binding_identity(&self) -> GeneralGemmMachineBindingIdentityV1 {
        self.machine_binding
    }

    pub const fn descriptor_source_identity(&self) -> CompilerDescriptorSourceIdentityV1 {
        self.descriptor_source
    }

    pub const fn symbolic_worker_identity(&self) -> GeneralGemmWorkerV2IdentityV1 {
        self.symbolic_worker
    }

    pub const fn worker_execution_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.worker_execution
    }

    pub const fn measured_worker_identity(&self) -> ContentIdentityV1 {
        self.measured_worker
    }

    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    pub fn llvm_build_identity(&self) -> &str {
        &self.llvm_build_identity
    }

    pub const fn worker_request_identity(&self) -> &[u8; 32] {
        &self.worker_request
    }

    pub const fn worker_response_identity(&self) -> &[u8; 32] {
        &self.worker_response
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.raw_output
    }

    pub const fn raw_inspection_identity(&self) -> InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw_inspection
    }

    pub const fn raw_policy_identity(&self) -> WorkerV2RawHsacoPolicyIdentityV1 {
        self.raw_policy
    }

    pub const fn finalized_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output
    }

    pub const fn kernel_symbol_sha256(&self) -> &[u8; 32] {
        &self.kernel_symbol_sha256
    }

    pub const fn vector_global_load_count(&self) -> u32 {
        self.vector_global_loads
    }

    pub const fn barriers_ir(&self) -> u32 {
        self.barriers_ir
    }

    pub const fn barriers_isa(&self) -> u32 {
        self.barriers_isa
    }

    pub const fn barrier_refinement(&self) -> GeneralGemmBarrierRefinementV1 {
        self.barrier_refinement
    }

    pub const fn mfma_numerical_refinement(&self) -> &GeneralGemmMfmaNumericalRefinementV1 {
        &self.mfma_numerical
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.prepared.exact_finalized_bytes()
    }

    pub const fn grants_artifact_authority(&self) -> bool {
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

impl InertSymbolicGeneralGemmWorkerV2EvidenceV1 {
    pub const fn identity(&self) -> GeneralGemmWorkerV2IdentityV1 {
        self.identity
    }

    pub const fn consumed_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.consumed_handoff_identity
    }

    pub const fn symbolic_compilation_identity(&self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.machine.projection().compilation_identity()
    }

    pub const fn symbolic_artifact_identity(&self) -> GeneralGemmSymbolicArtifactIdentityV1 {
        self.machine.artifact_identity()
    }

    pub const fn machine(&self) -> &GeneralGemmSymbolicStructuralMachineV1 {
        &self.machine
    }

    pub const fn worker_evidence(&self) -> &InertFirstBuildWorkerV2EvidenceV1 {
        &self.worker
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

impl InertGeneralGemmWorkerV2EvidenceV1 {
    pub const fn identity(&self) -> GeneralGemmWorkerV2IdentityV1 {
        self.identity
    }

    pub const fn consumed_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.consumed_handoff_identity
    }

    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.machine.compiler_handoff().identity()
    }

    pub fn handoff_v2_identity(&self) -> HandoffIdentityV2 {
        self.machine.handoff().identity()
    }

    pub const fn llvm_assembly_identity(&self) -> LlvmAssemblySha256V2 {
        self.machine.assembly().sha256()
    }

    pub const fn projection_identity(&self) -> GeneralGemmPlironProjectionIdentityV1 {
        self.machine.projection().identity()
    }

    pub const fn machine_binding_identity(&self) -> GeneralGemmMachineBindingIdentityV1 {
        self.machine.binding_section().identity()
    }

    pub const fn worker_execution_identity(&self) -> FirstBuildWorkerV2IdentityV1 {
        self.worker.identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.worker.output_identity()
    }

    pub const fn worker_evidence(&self) -> &InertFirstBuildWorkerV2EvidenceV1 {
        &self.worker
    }

    pub const fn grants_artifact_authority(&self) -> bool {
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

/// Failure before exact general-GEMM post-link inspection.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneralGemmWorkerV2ErrorV1 {
    HandoffSubstitution,
    TypedAdmissionSubstitution,
    WorkerBuildPolicySubstitution,
    FixedLinkOption,
    OutputBound,
    Worker(FirstBuildWorkerV2Error),
}

impl fmt::Display for GeneralGemmWorkerV2ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandoffSubstitution => formatter.write_str(
                "consumed compiler handoff differs from the exact general-GEMM machine handoff",
            ),
            Self::TypedAdmissionSubstitution => formatter
                .write_str("typed pre-LLVM admission differs from the exact general-GEMM handoff"),
            Self::WorkerBuildPolicySubstitution => formatter.write_str(
                "measured Worker LLVM build differs from typed pre-LLVM admission policy",
            ),
            Self::FixedLinkOption => {
                formatter.write_str("fixed general-GEMM Worker V2 option is invalid")
            }
            Self::OutputBound => {
                formatter.write_str("fixed general-GEMM Worker V2 output bound is invalid")
            }
            Self::Worker(error) => write!(formatter, "general-GEMM Worker V2 failed: {error}"),
        }
    }
}

impl Error for GeneralGemmWorkerV2ErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            Self::HandoffSubstitution
            | Self::TypedAdmissionSubstitution
            | Self::WorkerBuildPolicySubstitution
            | Self::FixedLinkOption
            | Self::OutputBound => None,
        }
    }
}

/// Failure while deriving the opaque exact post-link machine observation.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneralGemmPostLinkMachineErrorV1 {
    ScheduleSubstitution,
    WorkerProfileDiagnostic,
    Object,
    SectionCardinality(&'static str),
    SectionIdentity(&'static str),
    KernelSymbol,
    MachineCodeIdentity,
    MachineProfile(&'static str),
    NumericalRefinement,
    RawInspection(WorkerV2RawHsacoInspectionError),
    Finalization(WorkerV2HsacoFinalizationError),
}

impl fmt::Display for GeneralGemmPostLinkMachineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScheduleSubstitution => {
                formatter.write_str("symbolic machine carries an unknown general-GEMM schedule")
            }
            Self::WorkerProfileDiagnostic => formatter.write_str(
                "measured worker did not emit the exact general-GEMM post-link diagnostic",
            ),
            Self::Object => formatter.write_str("general-GEMM HSACO object inspection failed"),
            Self::SectionCardinality(section) => {
                write!(formatter, "general-GEMM section {section} is not unique")
            }
            Self::SectionIdentity(section) => {
                write!(
                    formatter,
                    "general-GEMM section {section} bytes were substituted"
                )
            }
            Self::KernelSymbol => {
                formatter.write_str("exact general-GEMM kernel symbol is missing or ambiguous")
            }
            Self::MachineCodeIdentity => formatter.write_str(
                "general-GEMM kernel machine bytes differ from the reviewed schedule identity",
            ),
            Self::MachineProfile(reason) => {
                write!(formatter, "general-GEMM machine profile rejected: {reason}")
            }
            Self::NumericalRefinement => formatter.write_str(
                "general-GEMM gfx942 MFMA numerical-refinement observation is incomplete",
            ),
            Self::RawInspection(error) => {
                write!(
                    formatter,
                    "general-GEMM raw HSACO inspection failed: {error}"
                )
            }
            Self::Finalization(error) => {
                write!(formatter, "general-GEMM HSACO finalization failed: {error}")
            }
        }
    }
}

impl Error for GeneralGemmPostLinkMachineErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RawInspection(error) => Some(error),
            Self::Finalization(error) => Some(error),
            _ => None,
        }
    }
}

/// Executes the exact structural general-GEMM handoff with the closed Worker V2 policy.
///
/// The caller supplies only the already consumed transactional handoff, measured
/// worker, and bounded process limits. Link inputs, options, and the output
/// ceiling cannot be substituted. The result remains inert.
pub fn execute_general_gemm_worker_v2_v1(
    machine: GeneralGemmStructuralMachineV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertGeneralGemmWorkerV2EvidenceV1, GeneralGemmWorkerV2ErrorV1> {
    if machine.compiler_boundary().graph_export().source_handoff() != machine.handoff()
        || machine.compiler_boundary().graph_export().source_identity()
            != machine.handoff().identity()
        || machine.worker_admission().handoff() != machine.handoff()
        || machine.worker_admission().handoff_identity() != machine.handoff().identity()
    {
        return Err(GeneralGemmWorkerV2ErrorV1::TypedAdmissionSubstitution);
    }
    if machine
        .worker_admission()
        .build_identity()
        .llvm_build_identity()
        != worker.measurement().llvm_build_identity()
    {
        return Err(GeneralGemmWorkerV2ErrorV1::WorkerBuildPolicySubstitution);
    }
    if consumed.bytes() != machine.compiler_handoff().canonical_bytes() {
        return Err(GeneralGemmWorkerV2ErrorV1::HandoffSubstitution);
    }
    let consumed_handoff_identity = consumed.identity();
    let worker = execute_reproducible_first_build_worker_v2(
        consumed,
        worker,
        Vec::new(),
        fixed_link_options()?,
        WorkerOutputConstraintsV1::new(fe2o3_hsaco::MAX_HSACO_BYTES as u64)
            .map_err(|_| GeneralGemmWorkerV2ErrorV1::OutputBound)?,
        limits,
    )
    .map_err(GeneralGemmWorkerV2ErrorV1::Worker)?;
    let identity = calculate_worker_identity(&machine, consumed_handoff_identity, &worker);
    Ok(InertGeneralGemmWorkerV2EvidenceV1 {
        identity,
        consumed_handoff_identity,
        machine,
        worker,
    })
}

/// Synchronously executes an exact symbolic GEMM handoff with the closed Worker V2 policy.
pub fn execute_symbolic_general_gemm_worker_v2_v1(
    machine: GeneralGemmSymbolicStructuralMachineV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertSymbolicGeneralGemmWorkerV2EvidenceV1, GeneralGemmWorkerV2ErrorV1> {
    if machine.compiler_boundary().graph_export().source_handoff() != machine.handoff()
        || machine.compiler_boundary().graph_export().source_identity()
            != machine.handoff().identity()
        || machine.worker_admission().handoff() != machine.handoff()
        || machine.worker_admission().handoff_identity() != machine.handoff().identity()
    {
        return Err(GeneralGemmWorkerV2ErrorV1::TypedAdmissionSubstitution);
    }
    if machine
        .worker_admission()
        .build_identity()
        .llvm_build_identity()
        != worker.measurement().llvm_build_identity()
    {
        return Err(GeneralGemmWorkerV2ErrorV1::WorkerBuildPolicySubstitution);
    }
    if consumed.bytes() != machine.compiler_handoff().canonical_bytes() {
        return Err(GeneralGemmWorkerV2ErrorV1::HandoffSubstitution);
    }
    let consumed_handoff_identity = consumed.identity();
    let worker = execute_reproducible_first_build_worker_v2(
        consumed,
        worker,
        Vec::new(),
        fixed_link_options()?,
        WorkerOutputConstraintsV1::new(fe2o3_hsaco::MAX_HSACO_BYTES as u64)
            .map_err(|_| GeneralGemmWorkerV2ErrorV1::OutputBound)?,
        limits,
    )
    .map_err(GeneralGemmWorkerV2ErrorV1::Worker)?;
    let identity = calculate_symbolic_worker_identity(&machine, consumed_handoff_identity, &worker);
    Ok(InertSymbolicGeneralGemmWorkerV2EvidenceV1 {
        identity,
        consumed_handoff_identity,
        machine,
        worker,
    })
}

/// Consumes exact symbolic Worker V2 evidence into an opaque post-link observation.
///
/// Inspection derives the selected schedule from the retained symbolic machine,
/// compares both retained sections byte-for-byte, pins the schedule-specific
/// kernel symbol bytes, checks the single-wave barrier refinement, and then
/// consumes the independently inspected raw object through canonical descriptor
/// finalization. The result remains non-admitting by itself.
pub fn finalize_symbolic_general_gemm_worker_v2_v1(
    evidence: InertSymbolicGeneralGemmWorkerV2EvidenceV1,
) -> Result<OpaqueGeneralGemmPostLinkMachineObservationV1, GeneralGemmPostLinkMachineErrorV1> {
    let InertSymbolicGeneralGemmWorkerV2EvidenceV1 {
        identity: symbolic_worker,
        consumed_handoff_identity,
        machine,
        worker,
    } = evidence;
    let schedule_identity = machine.projection().schedule_identity();
    let schedule = schedule_from_identity(schedule_identity)?;
    validate_typed_machine_refinement(&machine)?;

    let diagnostics = worker.authorized().response().diagnostics();
    if diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.as_str() == GENERAL_GEMM_POST_LINK_SUCCESS_DIAGNOSTIC_V1)
        .count()
        != 1
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::WorkerProfileDiagnostic);
    }
    let worker_request = *worker.authorized().response().request_identity();
    let worker_measurement = worker.worker_measurement();
    let measured_worker = worker_measurement.executable();
    let worker_build_identity = worker_measurement.worker_build_identity().to_owned();
    let llvm_build_identity = worker_measurement.llvm_build_identity().to_owned();
    let worker_execution = worker.identity();
    let raw_output = worker.output_identity();

    let raw_machine = inspect_exact_machine_object(
        worker.output_bytes(),
        machine.binding_section().canonical_bytes(),
        Some(machine.descriptor_source().canonical_bytes()),
        schedule,
    )?;
    let mfma_numerical = derive_mfma_numerical_refinement(&machine, raw_machine)?;

    let raw = inspect_general_gemm_worker_v2_raw_hsaco_v1(worker)
        .map_err(GeneralGemmPostLinkMachineErrorV1::RawInspection)?;
    let worker_response = *raw.response_identity().as_bytes();
    let raw_inspection = raw.identity();
    let raw_policy = raw.policy().identity();
    let prepared = finalize_allocated_general_gemm_worker_v2_hsaco_v1(raw)
        .map_err(GeneralGemmPostLinkMachineErrorV1::Finalization)?;
    let finalized = prepared.identity();
    let finalized_output = prepared.finalized_output_identity();
    let finalized_machine = inspect_exact_machine_object(
        prepared.exact_finalized_bytes(),
        machine.binding_section().canonical_bytes(),
        None,
        schedule,
    )?;
    if finalized_machine != raw_machine {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineCodeIdentity);
    }

    let identity = calculate_post_link_identity(
        &machine,
        symbolic_worker,
        consumed_handoff_identity,
        worker_execution,
        measured_worker,
        &worker_build_identity,
        &llvm_build_identity,
        worker_request,
        worker_response,
        raw_output,
        raw_inspection,
        raw_policy,
        finalized,
        finalized_output,
        raw_machine,
        mfma_numerical,
    );
    Ok(OpaqueGeneralGemmPostLinkMachineObservationV1 {
        identity,
        schedule,
        schedule_identity,
        symbolic_compilation: machine.projection().compilation_identity(),
        symbolic_artifact: machine.artifact_identity(),
        projection: machine.projection().identity(),
        handoff_v2: machine.handoff().identity(),
        typed_worker_admission: *machine.worker_admission().admission_identity().as_bytes(),
        llvm_assembly: machine.assembly().sha256(),
        compiler_handoff: machine.compiler_handoff().identity(),
        consumed_handoff: consumed_handoff_identity,
        machine_binding: machine.binding_section().identity(),
        descriptor_source: machine.descriptor_source().identity(),
        symbolic_worker,
        worker_execution,
        measured_worker,
        worker_build_identity,
        llvm_build_identity,
        worker_request,
        worker_response,
        raw_output,
        raw_inspection,
        raw_policy,
        finalized,
        finalized_output,
        kernel_symbol_sha256: raw_machine.kernel_symbol_sha256,
        vector_global_loads: raw_machine.vector_global_loads,
        barriers_ir: 2,
        barriers_isa: raw_machine.barriers_isa,
        barrier_refinement: GeneralGemmBarrierRefinementV1::SingleWaveElision,
        mfma_numerical,
        prepared,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactMachineObservationV1 {
    kernel_symbol_sha256: [u8; 32],
    vector_global_loads: u32,
    barriers_isa: u32,
    mfma_count: u32,
}

fn schedule_from_identity(
    identity: GeneralGemmScheduleIdentityV1,
) -> Result<GeneralGemmScheduleV1, GeneralGemmPostLinkMachineErrorV1> {
    for schedule in [
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        if schedule.identity() == identity {
            return Ok(schedule);
        }
    }
    Err(GeneralGemmPostLinkMachineErrorV1::ScheduleSubstitution)
}

fn validate_typed_machine_refinement(
    machine: &GeneralGemmSymbolicStructuralMachineV1,
) -> Result<(), GeneralGemmPostLinkMachineErrorV1> {
    let functions = machine.handoff().module().functions();
    if functions.len() != 1 || functions[0].symbol() != GENERAL_GEMM_KERNEL_SYMBOL_V1 {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "typed kernel closure",
        ));
    }
    let function = &functions[0];
    if function
        .attributes()
        .iter()
        .filter(|attribute| **attribute == FunctionAttributeV2::FpContractOff)
        .count()
        != 1
        || machine
            .assembly()
            .as_str()
            .matches("\"fp-contract\"=\"off\"")
            .count()
            != 1
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::NumericalRefinement);
    }
    let mut barriers = 0_u32;
    let mut mfmas = 0_u32;
    for instruction in function
        .blocks()
        .iter()
        .flat_map(|block| block.instructions())
    {
        if let InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(intrinsic),
            ..
        } = instruction.kind()
        {
            match intrinsic {
                IntrinsicV2::AmdGpuBarrier => barriers += 1,
                IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => mfmas += 1,
                _ => {}
            }
        }
    }
    if barriers != 2 {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "typed IR barrier count",
        ));
    }
    if mfmas != 1 {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "typed IR MFMA count",
        ));
    }
    Ok(())
}

fn inspect_exact_machine_object(
    bytes: &[u8],
    expected_binding: &[u8],
    expected_descriptor: Option<&[u8]>,
    schedule: GeneralGemmScheduleV1,
) -> Result<ExactMachineObservationV1, GeneralGemmPostLinkMachineErrorV1> {
    let file = object::File::parse(bytes).map_err(|_| GeneralGemmPostLinkMachineErrorV1::Object)?;
    require_exact_section(&file, GENERAL_GEMM_BINDING_SECTION_V1, expected_binding)?;
    if let Some(descriptor) = expected_descriptor {
        require_exact_section(&file, GENERAL_GEMM_DESCRIPTOR_SECTION_V1, descriptor)?;
        require_unreferenced_descriptor_source(&file)?;
    }
    let kernel = unique_kernel_symbol_bytes(&file)?;
    inspect_exact_kernel_bytes(kernel, schedule)
}

fn require_unreferenced_descriptor_source(
    file: &object::File<'_>,
) -> Result<(), GeneralGemmPostLinkMachineErrorV1> {
    let mut symbols = file
        .symbols()
        .filter(|symbol| symbol.name().ok() == Some(GENERAL_GEMM_DESCRIPTOR_SOURCE_SYMBOL_V1));
    let symbol = symbols
        .next()
        .ok_or(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "descriptor source symbol closure",
        ))?;
    if symbols.next().is_some()
        || !symbol.is_definition()
        || symbol.is_global()
        || symbol.is_weak()
        || file
            .dynamic_symbols()
            .any(|dynamic| dynamic.name().ok() == Some(GENERAL_GEMM_DESCRIPTOR_SOURCE_SYMBOL_V1))
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "descriptor source symbol closure",
        ));
    }
    let symbol_index = symbol.index();
    for section in file.sections() {
        for (_, relocation) in section.relocations() {
            if relocation.target() == object::RelocationTarget::Symbol(symbol_index) {
                return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                    "descriptor source relocation",
                ));
            }
        }
    }
    Ok(())
}

fn require_exact_section<'data>(
    file: &object::File<'data>,
    name: &'static str,
    expected: &[u8],
) -> Result<(), GeneralGemmPostLinkMachineErrorV1> {
    let mut matches = file
        .sections()
        .filter(|section| section.name().ok() == Some(name));
    let section = matches
        .next()
        .ok_or(GeneralGemmPostLinkMachineErrorV1::SectionCardinality(name))?;
    if matches.next().is_some() {
        return Err(GeneralGemmPostLinkMachineErrorV1::SectionCardinality(name));
    }
    let actual = section
        .data()
        .map_err(|_| GeneralGemmPostLinkMachineErrorV1::Object)?;
    if actual != expected {
        return Err(GeneralGemmPostLinkMachineErrorV1::SectionIdentity(name));
    }
    Ok(())
}

fn unique_kernel_symbol_bytes<'data>(
    file: &object::File<'data>,
) -> Result<&'data [u8], GeneralGemmPostLinkMachineErrorV1> {
    let mut symbols = file
        .symbols()
        .filter(|symbol| symbol.name().ok() == Some(GENERAL_GEMM_KERNEL_SYMBOL_V1));
    let symbol = symbols
        .next()
        .ok_or(GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    if symbols.next().is_some()
        || !symbol.is_definition()
        || symbol.kind() != object::SymbolKind::Text
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::KernelSymbol);
    }
    let section_index = symbol
        .section_index()
        .ok_or(GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    let section = file
        .section_by_index(section_index)
        .map_err(|_| GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    let offset = symbol
        .address()
        .checked_sub(section.address())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    let size = usize::try_from(symbol.size())
        .map_err(|_| GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    let end = offset
        .checked_add(size)
        .ok_or(GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?;
    section
        .data()
        .map_err(|_| GeneralGemmPostLinkMachineErrorV1::KernelSymbol)?
        .get(offset..end)
        .ok_or(GeneralGemmPostLinkMachineErrorV1::KernelSymbol)
}

fn inspect_exact_kernel_bytes(
    bytes: &[u8],
    schedule: GeneralGemmScheduleV1,
) -> Result<ExactMachineObservationV1, GeneralGemmPostLinkMachineErrorV1> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "kernel instruction encoding",
        ));
    }
    let kernel_symbol_sha256: [u8; 32] = Sha256::digest(bytes).into();
    let expected_sha256 = match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => REFERENCE_MACHINE_SHA256_V1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => VECTOR_A_MACHINE_SHA256_V1,
    };
    if kernel_symbol_sha256 != expected_sha256 {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineCodeIdentity);
    }

    let words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("four-byte chunk")))
        .collect::<Vec<_>>();
    let (vector_global_loads, barriers_isa, mfma_count) =
        inspect_machine_word_profile(&words, schedule)?;
    Ok(ExactMachineObservationV1 {
        kernel_symbol_sha256,
        vector_global_loads,
        barriers_isa,
        mfma_count,
    })
}

fn inspect_machine_word_profile(
    words: &[u32],
    schedule: GeneralGemmScheduleV1,
) -> Result<(u32, u32, u32), GeneralGemmPostLinkMachineErrorV1> {
    let expected_vector_loads = match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => 0,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => 1,
    };
    let positions = |predicate: fn(u32) -> bool| {
        words
            .iter()
            .enumerate()
            .filter_map(|(index, word)| predicate(*word).then_some(index))
            .collect::<Vec<_>>()
    };
    let vector_loads = positions(|word| word >> 16 == 0xdc54);
    let barriers = positions(|word| word == S_BARRIER_OPCODE_V1);
    let writes = positions(|word| matches!(word >> 16, 0xd83e | 0xd8aa));
    let reads = positions(|word| word >> 16 == 0xd878);
    let mfmas = positions(|word| word == MFMA_F32_16X16X16BF16_1K_OPCODE_V1);
    if vector_loads.len() != expected_vector_loads as usize {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "schedule-specific vector A load count",
        ));
    }
    if !barriers.is_empty() || writes.len() != 8 || reads.len() != 8 || mfmas.len() != 1 {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "LDS, barrier, or MFMA count",
        ));
    }
    let publish_wait = writes[0].checked_sub(1);
    let reuse_wait = reads[7].checked_add(2);
    if publish_wait.and_then(|index| words.get(index)) != Some(&S_WAITCNT_VMCNT_ZERO_V1)
        || reads[0] != writes[7] + 2
        || reuse_wait.and_then(|index| words.get(index)) != Some(&S_WAITCNT_LGKMCNT_ZERO_V1)
        || reuse_wait.is_none_or(|index| mfmas[0] <= index)
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
            "single-wave publish/reuse wait order",
        ));
    }
    Ok((
        vector_loads.len() as u32,
        barriers.len() as u32,
        mfmas.len() as u32,
    ))
}

fn derive_mfma_numerical_refinement(
    machine: &GeneralGemmSymbolicStructuralMachineV1,
    observed: ExactMachineObservationV1,
) -> Result<GeneralGemmMfmaNumericalRefinementV1, GeneralGemmPostLinkMachineErrorV1> {
    if observed.mfma_count != 1
        || !machine
            .assembly()
            .as_str()
            .contains("\"fp-contract\"=\"off\"")
    {
        return Err(GeneralGemmPostLinkMachineErrorV1::NumericalRefinement);
    }
    let llvm_assembly = machine.assembly().sha256();
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_GEMM_MFMA_NUMERICAL_IDENTITY_DOMAIN_V1);
    hasher.update(llvm_assembly.as_bytes());
    hasher.update(observed.kernel_symbol_sha256);
    hasher.update(MFMA_F32_16X16X16BF16_1K_OPCODE_V1.to_le_bytes());
    hasher.update(observed.mfma_count.to_le_bytes());
    hasher.update([1]);
    Ok(GeneralGemmMfmaNumericalRefinementV1 {
        identity: GeneralGemmMfmaNumericalRefinementIdentityV1(hasher.finalize().into()),
        llvm_assembly,
        kernel_symbol_sha256: observed.kernel_symbol_sha256,
        opcode: MFMA_F32_16X16X16BF16_1K_OPCODE_V1,
        count: observed.mfma_count,
    })
}

#[allow(clippy::too_many_arguments)]
fn calculate_post_link_identity(
    machine: &GeneralGemmSymbolicStructuralMachineV1,
    symbolic_worker: GeneralGemmWorkerV2IdentityV1,
    consumed_handoff: CompilerModuleHandoffIdentityV1,
    worker_execution: FirstBuildWorkerV2IdentityV1,
    measured_worker: ContentIdentityV1,
    worker_build_identity: &str,
    llvm_build_identity: &str,
    worker_request: [u8; 32],
    worker_response: [u8; 32],
    raw_output: ContentIdentityV1,
    raw_inspection: InspectedRawWorkerV2HsacoIdentityV1,
    raw_policy: WorkerV2RawHsacoPolicyIdentityV1,
    finalized: FinalizedWorkerV2HsacoIdentityV1,
    finalized_output: ContentIdentityV1,
    machine_observation: ExactMachineObservationV1,
    mfma_numerical: GeneralGemmMfmaNumericalRefinementV1,
) -> GeneralGemmPostLinkMachineIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_GEMM_POST_LINK_IDENTITY_DOMAIN_V1);
    hasher.update(machine.projection().compilation_identity().as_bytes());
    hasher.update(machine.artifact_identity().as_bytes());
    hasher.update(machine.projection().identity().as_bytes());
    hasher.update(machine.projection().schedule_identity().as_bytes());
    hasher.update(machine.projection().symbolic_plan_identity().as_bytes());
    hasher.update(machine.projection().symbolic_kir_identity().as_bytes());
    hasher.update(machine.handoff().identity().as_bytes());
    hasher.update(machine.worker_admission().admission_identity().as_bytes());
    hasher.update(machine.assembly().sha256().as_bytes());
    hasher.update(machine.compiler_handoff().identity().sha256());
    hasher.update(consumed_handoff.as_bytes());
    hasher.update(machine.binding_section().identity().as_bytes());
    hasher.update(machine.descriptor_source().identity().sha256());
    hasher.update(
        machine
            .descriptor_source()
            .identity()
            .byte_len()
            .to_le_bytes(),
    );
    hasher.update(symbolic_worker.as_bytes());
    hasher.update(worker_execution.as_bytes());
    push_content_identity(&mut hasher, measured_worker);
    push_text(&mut hasher, worker_build_identity);
    push_text(&mut hasher, llvm_build_identity);
    hasher.update(worker_request);
    hasher.update(worker_response);
    push_content_identity(&mut hasher, raw_output);
    hasher.update(raw_inspection.as_bytes());
    hasher.update(raw_policy.as_bytes());
    hasher.update(finalized.as_bytes());
    push_content_identity(&mut hasher, finalized_output);
    hasher.update(machine_observation.kernel_symbol_sha256);
    hasher.update(machine_observation.vector_global_loads.to_le_bytes());
    hasher.update(2_u32.to_le_bytes());
    hasher.update(machine_observation.barriers_isa.to_le_bytes());
    hasher.update([1]);
    hasher.update(mfma_numerical.identity().as_bytes());
    GeneralGemmPostLinkMachineIdentityV1(hasher.finalize().into())
}

fn push_content_identity(hasher: &mut Sha256, identity: ContentIdentityV1) {
    hasher.update(identity.sha256());
    hasher.update(identity.byte_len().to_le_bytes());
}

fn fixed_link_options() -> Result<Vec<LinkOptionV1>, GeneralGemmWorkerV2ErrorV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| {
        LinkOptionV1::new(name, value).map_err(|_| GeneralGemmWorkerV2ErrorV1::FixedLinkOption)
    })
    .collect()
}

fn calculate_worker_identity(
    machine: &GeneralGemmStructuralMachineV1,
    consumed: CompilerModuleHandoffIdentityV1,
    worker: &InertFirstBuildWorkerV2EvidenceV1,
) -> GeneralGemmWorkerV2IdentityV1 {
    let measurement = worker.worker_measurement();
    let executable = measurement.executable();
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_GEMM_WORKER_IDENTITY_DOMAIN_V1);
    hasher.update(machine.projection().identity().as_bytes());
    hasher.update(machine.handoff().identity().as_bytes());
    hasher.update(machine.compiler_boundary().identity().as_bytes());
    hasher.update(machine.worker_admission().admission_identity().as_bytes());
    hasher.update(machine.assembly().sha256().as_bytes());
    hasher.update(machine.compiler_handoff().identity().sha256());
    hasher.update(consumed.as_bytes());
    hasher.update(worker.identity().as_bytes());
    hasher.update(executable.sha256());
    hasher.update(executable.byte_len().to_le_bytes());
    push_text(&mut hasher, measurement.worker_build_identity());
    push_text(&mut hasher, measurement.llvm_build_identity());
    hasher.update(worker.output_identity().sha256());
    hasher.update(worker.output_identity().byte_len().to_le_bytes());
    GeneralGemmWorkerV2IdentityV1(hasher.finalize().into())
}

fn calculate_symbolic_worker_identity(
    machine: &GeneralGemmSymbolicStructuralMachineV1,
    consumed: CompilerModuleHandoffIdentityV1,
    worker: &InertFirstBuildWorkerV2EvidenceV1,
) -> GeneralGemmWorkerV2IdentityV1 {
    let measurement = worker.worker_measurement();
    let executable = measurement.executable();
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_GEMM_WORKER_IDENTITY_DOMAIN_V1);
    hasher.update(machine.artifact_identity().as_bytes());
    hasher.update(machine.projection().identity().as_bytes());
    hasher.update(machine.handoff().identity().as_bytes());
    hasher.update(machine.compiler_boundary().identity().as_bytes());
    hasher.update(machine.worker_admission().admission_identity().as_bytes());
    hasher.update(machine.assembly().sha256().as_bytes());
    hasher.update(machine.compiler_handoff().identity().sha256());
    hasher.update(consumed.as_bytes());
    hasher.update(worker.identity().as_bytes());
    hasher.update(executable.sha256());
    hasher.update(executable.byte_len().to_le_bytes());
    push_text(&mut hasher, measurement.worker_build_identity());
    push_text(&mut hasher, measurement.llvm_build_identity());
    hasher.update(worker.output_identity().sha256());
    hasher.update(worker.output_identity().byte_len().to_le_bytes());
    GeneralGemmWorkerV2IdentityV1(hasher.finalize().into())
}

fn push_text(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_profile_words(vector_a: bool) -> Vec<u32> {
        let mut words = vec![0xbe80_0000];
        if vector_a {
            words.extend([0xdc54_8000, 0x0000_0000]);
        }
        words.push(S_WAITCNT_VMCNT_ZERO_V1);
        for index in 0..8 {
            words.extend([
                if index % 2 == 0 {
                    0xd83e_0000
                } else {
                    0xd8aa_0000
                },
                index,
            ]);
        }
        for index in 0..8 {
            words.extend([0xd878_0000, index]);
        }
        words.extend([
            S_WAITCNT_LGKMCNT_ZERO_V1,
            0xbe80_0000,
            MFMA_F32_16X16X16BF16_1K_OPCODE_V1,
            0x0000_0000,
        ]);
        words
    }

    #[test]
    fn machine_profile_distinguishes_reference_and_vector_a_schedules() {
        let reference = exact_profile_words(false);
        let vector_a = exact_profile_words(true);
        assert_eq!(
            inspect_machine_word_profile(&reference, GeneralGemmScheduleV1::ReferenceWave64Xor4V1)
                .unwrap(),
            (0, 0, 1)
        );
        assert_eq!(
            inspect_machine_word_profile(
                &vector_a,
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
            )
            .unwrap(),
            (1, 0, 1)
        );
        assert!(matches!(
            inspect_machine_word_profile(&vector_a, GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                "schedule-specific vector A load count"
            ))
        ));
        assert!(matches!(
            inspect_machine_word_profile(
                &reference,
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
            ),
            Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                "schedule-specific vector A load count"
            ))
        ));
    }

    #[test]
    fn machine_profile_rejects_barrier_and_wait_order_mutations() {
        let schedule = GeneralGemmScheduleV1::ReferenceWave64Xor4V1;
        let mut barrier = exact_profile_words(false);
        barrier.push(S_BARRIER_OPCODE_V1);
        assert!(matches!(
            inspect_machine_word_profile(&barrier, schedule),
            Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                "LDS, barrier, or MFMA count"
            ))
        ));

        let mut missing_publish = exact_profile_words(false);
        let publish = missing_publish
            .iter()
            .position(|word| *word == S_WAITCNT_VMCNT_ZERO_V1)
            .unwrap();
        missing_publish[publish] = 0xbe80_0000;
        assert!(matches!(
            inspect_machine_word_profile(&missing_publish, schedule),
            Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                "single-wave publish/reuse wait order"
            ))
        ));

        let mut early_reuse = exact_profile_words(false);
        let reuse = early_reuse
            .iter()
            .position(|word| *word == S_WAITCNT_LGKMCNT_ZERO_V1)
            .unwrap();
        early_reuse.swap(reuse, reuse - 2);
        assert!(matches!(
            inspect_machine_word_profile(&early_reuse, schedule),
            Err(GeneralGemmPostLinkMachineErrorV1::MachineProfile(
                "single-wave publish/reuse wait order"
            ))
        ));
    }
}
