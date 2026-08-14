//! Exact gfx942 machine-effect profile for the finalized Scalar GEMM V1 HSACO.
//!
//! The profile binds one finalized payload and one descriptor identity to the
//! measured worker's static-site evidence. It deliberately does not connect
//! machine addresses back to logical buffers; that remains a compiler
//! refinement and memory-safety obligation.

use crate::{
    AuthenticatedPhysicalMachineEffectErrorV1, AuthenticatedPhysicalMachineEffectExecutionV1,
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1, PhysicalMachineAnalyzerIdentityV1,
    PhysicalMachineDescriptorIdentityV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectKindV1, PhysicalMachineEffectRequestErrorV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1,
    PhysicalMachinePayloadIdentityV1, PhysicalMachineTargetV1, PhysicalMachineToolchainIdentityV1,
};
use std::{error::Error, fmt};

pub const SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL: &str = "scalar_gemm_v1";

const KERNARG_READ_SITES: u32 = 6;
const LOGICAL_GLOBAL_READ_SITES: u32 = 2;
const LOGICAL_GLOBAL_WRITE_SITES: u32 = 1;
const RETURN_SITES: u32 = 1;

/// Static-site budget for the exact Scalar GEMM V1 gfx942 lowering.
///
/// The six kernarg reads precede two logical f32 reads from A/B and one logical
/// f32 write to C. Every memory site also contributes one address effect.
pub const SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET: PhysicalMachineEffectBudgetV1 =
    PhysicalMachineEffectBudgetV1::new(
        KERNARG_READ_SITES + LOGICAL_GLOBAL_READ_SITES + LOGICAL_GLOBAL_WRITE_SITES,
        KERNARG_READ_SITES + LOGICAL_GLOBAL_READ_SITES,
        LOGICAL_GLOBAL_WRITE_SITES,
        RETURN_SITES,
        0,
    );

/// Exact payload and descriptor expectations for one Scalar GEMM V1 analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmV1PhysicalMachineEffectProfileV1 {
    finalized_hsaco_identity: PhysicalMachinePayloadIdentityV1,
    exact_finalized_hsaco: Vec<u8>,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
}

impl ScalarGemmV1PhysicalMachineEffectProfileV1 {
    pub fn new(
        finalized_hsaco_identity: PhysicalMachinePayloadIdentityV1,
        exact_finalized_hsaco: Vec<u8>,
        descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    ) -> Result<Self, ScalarGemmV1PhysicalMachineEffectErrorV1> {
        if exact_finalized_hsaco.is_empty()
            || exact_finalized_hsaco.len() > MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1
        {
            return Err(
                ScalarGemmV1PhysicalMachineEffectErrorV1::FinalizedHsacoSize {
                    actual: exact_finalized_hsaco.len(),
                    maximum: MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
                },
            );
        }
        let actual = PhysicalMachinePayloadIdentityV1::calculate(&exact_finalized_hsaco);
        if actual != finalized_hsaco_identity {
            return Err(
                ScalarGemmV1PhysicalMachineEffectErrorV1::FinalizedHsacoIdentityMismatch {
                    expected: finalized_hsaco_identity,
                    actual,
                },
            );
        }
        if descriptor_identity.as_bytes() == [0; 32] {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::ZeroDescriptorIdentity);
        }
        Ok(Self {
            finalized_hsaco_identity,
            exact_finalized_hsaco,
            descriptor_identity,
        })
    }

    pub const fn finalized_hsaco_identity(&self) -> PhysicalMachinePayloadIdentityV1 {
        self.finalized_hsaco_identity
    }

    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        &self.exact_finalized_hsaco
    }

    pub const fn descriptor_identity(&self) -> PhysicalMachineDescriptorIdentityV1 {
        self.descriptor_identity
    }

    pub const fn effect_budget(&self) -> PhysicalMachineEffectBudgetV1 {
        SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET
    }

    pub fn request_v1(
        &self,
        execution_challenge: PhysicalMachineExecutionChallengeV1,
        analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
        toolchain_identity: PhysicalMachineToolchainIdentityV1,
    ) -> Result<PhysicalMachineEffectRequestV1, ScalarGemmV1PhysicalMachineEffectErrorV1> {
        PhysicalMachineEffectRequestV1::new(
            execution_challenge,
            analyzer_identity,
            toolchain_identity,
            self.exact_finalized_hsaco.clone(),
            vec![PhysicalMachineEffectEntryRequestV1::new(
                SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
                SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET,
            )?],
        )
        .map_err(Into::into)
    }

    /// Runs the exact profile through a retained, policy-authenticated worker.
    #[cfg(target_os = "linux")]
    pub fn analyze(
        &self,
        worker: &AuthenticatedPhysicalMachineEffectWorkerV1,
        limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    ) -> Result<
        AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1,
        ScalarGemmV1PhysicalMachineEffectErrorV1,
    > {
        let entry = PhysicalMachineEffectEntryRequestV1::new(
            SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET,
        )?;
        let execution = worker
            .analyze(self.exact_finalized_hsaco.clone(), vec![entry], limits)
            .map_err(ScalarGemmV1PhysicalMachineEffectErrorV1::AuthenticatedWorker)?;
        self.validate_evidence(execution.evidence())?;
        Ok(AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1 {
            finalized_hsaco_identity: self.finalized_hsaco_identity,
            descriptor_identity: self.descriptor_identity,
            execution,
        })
    }

    /// Validates profile closure without upgrading descriptive evidence.
    pub fn validate_evidence(
        &self,
        evidence: &PhysicalMachineEffectEvidenceV1,
    ) -> Result<(), ScalarGemmV1PhysicalMachineEffectErrorV1> {
        if evidence.payload_identity() != self.finalized_hsaco_identity {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::PayloadSubstitution);
        }
        if evidence.target() != PhysicalMachineTargetV1::Gfx942XnackMinusCov6 {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::TargetSubstitution);
        }
        let [entry] = evidence.entry_points() else {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EntryPointSet);
        };
        if entry.symbol() != SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EntryPointSet);
        }
        if entry.descriptor_identity() != self.descriptor_identity {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::DescriptorSubstitution);
        }

        let [function] = evidence.functions() else {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::CallGraphNotClosed);
        };
        if function.symbol() != SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL
            || !function.direct_callees().is_empty()
        {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::CallGraphNotClosed);
        }

        let mut counts = [0_u32; 4];
        let mut four_byte_reads = 0_u32;
        let mut eight_byte_reads = 0_u32;
        for effect in evidence.effects() {
            if effect.entry_symbol() != SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL
                || effect.function_symbol() != SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL
            {
                return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EffectSet);
            }
            match effect.kind() {
                PhysicalMachineEffectKindV1::GlobalAddress if effect.byte_width() == 8 => {
                    counts[0] += 1;
                }
                PhysicalMachineEffectKindV1::GlobalRead if effect.byte_width() == 4 => {
                    counts[1] += 1;
                    four_byte_reads += 1;
                }
                PhysicalMachineEffectKindV1::GlobalRead if effect.byte_width() == 8 => {
                    counts[1] += 1;
                    eight_byte_reads += 1;
                }
                PhysicalMachineEffectKindV1::GlobalWrite if effect.byte_width() == 4 => {
                    counts[2] += 1;
                }
                PhysicalMachineEffectKindV1::Return if effect.byte_width() == 0 => {
                    counts[3] += 1;
                }
                _ => return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EffectSet),
            }
        }
        let expected = [
            SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_addresses(),
            SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_reads(),
            SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_writes(),
            SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_returns(),
        ];
        if counts != expected || four_byte_reads != 5 || eight_byte_reads != 3 {
            return Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EffectSet);
        }
        Ok(())
    }
}

/// Policy-authenticated analyzer execution restricted to the scalar profile.
///
/// This value is inert. Static sites do not establish source/compiler
/// refinement, concrete addresses, memory safety, bounds safety, or race
/// freedom and grant no publication, load, or launch authority.
#[derive(Debug)]
pub struct AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1 {
    finalized_hsaco_identity: PhysicalMachinePayloadIdentityV1,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    execution: AuthenticatedPhysicalMachineEffectExecutionV1,
}

impl AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1 {
    pub const fn finalized_hsaco_identity(&self) -> PhysicalMachinePayloadIdentityV1 {
        self.finalized_hsaco_identity
    }

    pub const fn descriptor_identity(&self) -> PhysicalMachineDescriptorIdentityV1 {
        self.descriptor_identity
    }

    pub const fn evidence(&self) -> &PhysicalMachineEffectEvidenceV1 {
        self.execution.evidence()
    }

    pub const fn authenticated_execution(&self) -> &AuthenticatedPhysicalMachineEffectExecutionV1 {
        &self.execution
    }

    pub const fn authenticates_analyzer_execution(&self) -> bool {
        self.execution.authenticates_analyzer_execution()
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn establishes_logical_buffer_address_refinement(&self) -> bool {
        false
    }

    pub const fn establishes_memory_safety(&self) -> bool {
        false
    }

    pub const fn establishes_out_of_bounds_absence(&self) -> bool {
        false
    }

    pub const fn establishes_race_freedom(&self) -> bool {
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

#[derive(Debug)]
pub enum ScalarGemmV1PhysicalMachineEffectErrorV1 {
    FinalizedHsacoSize {
        actual: usize,
        maximum: usize,
    },
    FinalizedHsacoIdentityMismatch {
        expected: PhysicalMachinePayloadIdentityV1,
        actual: PhysicalMachinePayloadIdentityV1,
    },
    ZeroDescriptorIdentity,
    Request(PhysicalMachineEffectRequestErrorV1),
    AuthenticatedWorker(AuthenticatedPhysicalMachineEffectErrorV1),
    PayloadSubstitution,
    TargetSubstitution,
    EntryPointSet,
    DescriptorSubstitution,
    CallGraphNotClosed,
    EffectSet,
}

impl From<PhysicalMachineEffectRequestErrorV1> for ScalarGemmV1PhysicalMachineEffectErrorV1 {
    fn from(error: PhysicalMachineEffectRequestErrorV1) -> Self {
        Self::Request(error)
    }
}

impl fmt::Display for ScalarGemmV1PhysicalMachineEffectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Scalar GEMM V1 physical machine-effect profile: {self:?}"
        )
    }
}

impl Error for ScalarGemmV1PhysicalMachineEffectErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::AuthenticatedWorker(error) => Some(error),
            _ => None,
        }
    }
}
