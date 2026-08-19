//! Exact, inert Worker V2 and post-link observations for general GEMM V1.
//!
//! This module consumes structural compiler output. It never authenticates the
//! frontend correspondence, proof, publication, loading, or launch boundaries.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::{
    CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffIdentityV2;
use fe2o3_general_gemm_compiler::{
    GeneralGemmMachineBindingIdentityV1, GeneralGemmPlironProjectionIdentityV1,
    GeneralGemmStructuralMachineV1,
};
use fe2o3_llvm_handoff::HandoffIdentityV2;
use fe2o3_llvm_text::LlvmAssemblySha256V2;
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FirstBuildWorkerV2Error, FirstBuildWorkerV2IdentityV1,
    InertFirstBuildWorkerV2EvidenceV1, LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerOutputConstraintsV1, execute_reproducible_first_build_worker_v2,
};

const GENERAL_GEMM_WORKER_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERAL-GEMM/WORKER-V2/INERT-EVIDENCE/V1\0";

/// Identity binding the exact structural machine to one measured Worker V2 execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmWorkerV2IdentityV1([u8; 32]);

impl GeneralGemmWorkerV2IdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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
            Self::HandoffSubstitution | Self::FixedLinkOption | Self::OutputBound => None,
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
