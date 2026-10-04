use std::{error::Error, fmt};

use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, GeneratedWorkerV3KfdDifferentialAvailabilityV1,
    GeneratedWorkerV3KfdDifferentialObservationV1, GeneratedWorkerV3KfdExecutionError,
    GeneratedWorkerV3KfdInvocation,
};
use fe2o3_hsaco_finalize::ProductionKirV7StructuralBridgeV1;
use fe2o3_kernel_ir::VerifiedSimulationBundleV4;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, SimulationFailureReductionReportV1, SimulationLimitsV1,
    SimulationRequestV1, SimulationScheduleRequestV1, SimulationTargetV1,
};

use crate::{
    PhysicalDifferentialErrorV1, PhysicalDifferentialLimitsV1, PhysicalDifferentialReportV1,
    PreparedPhysicalDifferentialV1, prepare_physical_differential_v1,
};

/// Exact authority-free simulator inputs paired with one already-authenticated invocation.
///
/// The inputs borrow canonical owners and cannot carry verifier, KFD, publication, or parity
/// authority. The generated invocation remains the sole owner of the direct-KFD authority.
#[derive(Clone, Copy)]
pub struct PhysicalDifferentialSimulationInputsV1<'input> {
    pub bundle: &'input VerifiedSimulationBundleV4,
    pub bridge: &'input ProductionKirV7StructuralBridgeV1,
    pub module: &'input AdmittedSimulationModuleV1,
    pub request: &'input SimulationRequestV1,
    pub target: SimulationTargetV1,
    pub simulation_limits: SimulationLimitsV1,
    pub schedule: SimulationScheduleRequestV1<'input>,
    pub reduction: Option<&'input SimulationFailureReductionReportV1>,
    pub differential_limits: PhysicalDifferentialLimitsV1,
}

/// Single-use composition of an authenticated generated invocation and its exact CPU comparison.
///
/// This type cannot be constructed from a verifier backend, HSACO bytes, digests, raw pointers, or
/// a synthetic observation. It owns the only invocation authority and the prepared simulator
/// result, and exposes only one terminal execute-and-compare transition.
#[must_use = "a protected physical differential retains direct-KFD authority and output borrows"]
pub struct PreparedWorkerV3PhysicalDifferentialV1<'allocation, K> {
    invocation: GeneratedWorkerV3KfdInvocation<'allocation, K>,
    differential: PreparedPhysicalDifferentialV1,
}

impl<K> fmt::Debug for PreparedWorkerV3PhysicalDifferentialV1<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedWorkerV3PhysicalDifferentialV1")
            .field("invocation", &self.invocation)
            .field("authority", &"retained-private")
            .finish_non_exhaustive()
    }
}

/// Binds an already-authenticated generated Worker V3 invocation to exact simulator state.
///
/// Authentication and KFD preparation must have completed through `fe2o3-host`'s production
/// application interface before this function can be called. In particular, this API accepts no
/// verifier implementation and cannot turn the synthetic test verifier feature into production
/// evidence. An invocation without protected compiler/proof evidence fails before execution.
///
/// ```compile_fail
/// use fe2o3_host::WorkerV3ProtectedVerifierAdapterV1;
/// use fe2o3_sim_physical_differential::{
///     PhysicalDifferentialSimulationInputsV1,
///     prepare_generated_worker_v3_physical_differential_v1,
/// };
///
/// fn verifier_is_not_an_invocation<B>(
///     verifier: WorkerV3ProtectedVerifierAdapterV1<B>,
///     inputs: PhysicalDifferentialSimulationInputsV1<'_>,
/// ) {
///     let _ = prepare_generated_worker_v3_physical_differential_v1(verifier, inputs);
/// }
/// ```
///
/// The synthetic verifier is not exported by the default production dependency at all:
///
/// ```compile_fail
/// use fe2o3_host::WorkerV3SyntheticVerifierAdapterV1;
/// # fn main() {}
/// ```
pub fn prepare_generated_worker_v3_physical_differential_v1<'allocation, K>(
    invocation: GeneratedWorkerV3KfdInvocation<'allocation, K>,
    inputs: PhysicalDifferentialSimulationInputsV1<'_>,
) -> Result<
    PreparedWorkerV3PhysicalDifferentialV1<'allocation, K>,
    PhysicalApplicationPreparationErrorV1,
>
where
    K: CompilerGeneratedKernelExpectationV1,
{
    require_protected_differential_evidence(invocation.differential_availability())?;
    let binding = invocation
        .differential_binding()
        .ok_or(PhysicalApplicationPreparationErrorV1::ProtectedProductionEvidenceUnavailable)?;
    let differential = prepare_physical_differential_v1(
        inputs.bundle,
        inputs.bridge,
        inputs.module,
        inputs.request,
        inputs.target,
        inputs.simulation_limits,
        inputs.schedule,
        binding,
        inputs.reduction,
        inputs.differential_limits,
    )
    .map_err(PhysicalApplicationPreparationErrorV1::Differential)?;
    Ok(PreparedWorkerV3PhysicalDifferentialV1 {
        invocation,
        differential,
    })
}

impl<K> PreparedWorkerV3PhysicalDifferentialV1<'_, K>
where
    K: CompilerGeneratedKernelExpectationV1,
{
    /// Executes the exact retained direct-KFD invocation and compares its completed buffers.
    ///
    /// Runtime failure, stale publication, stale device state, ambiguous native completion, and
    /// generated writeback failure cannot produce a report. The direct-KFD runtime terminates the
    /// process for failures after native mutation where returning would expose ambiguous state.
    pub fn execute_and_compare(
        self,
    ) -> Result<PhysicalDifferentialReportV1, PhysicalApplicationExecutionErrorV1> {
        let Self {
            invocation,
            differential,
        } = self;
        let observation = invocation
            .execute_for_differential()
            .map_err(PhysicalApplicationExecutionErrorV1::Execution)?;
        complete_after_execution(differential, observation)
    }
}

fn require_protected_differential_evidence(
    availability: GeneratedWorkerV3KfdDifferentialAvailabilityV1,
) -> Result<(), PhysicalApplicationPreparationErrorV1> {
    match availability {
        GeneratedWorkerV3KfdDifferentialAvailabilityV1::SealedObservationAvailable => Ok(()),
        GeneratedWorkerV3KfdDifferentialAvailabilityV1::ProtectedProductionEvidenceUnavailable => {
            Err(PhysicalApplicationPreparationErrorV1::ProtectedProductionEvidenceUnavailable)
        }
    }
}

fn complete_after_execution(
    differential: PreparedPhysicalDifferentialV1,
    observation: GeneratedWorkerV3KfdDifferentialObservationV1,
) -> Result<PhysicalDifferentialReportV1, PhysicalApplicationExecutionErrorV1> {
    differential
        .complete(observation)
        .map_err(PhysicalApplicationExecutionErrorV1::Comparison)
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PhysicalApplicationPreparationErrorV1 {
    ProtectedProductionEvidenceUnavailable,
    Differential(PhysicalDifferentialErrorV1),
}

impl fmt::Display for PhysicalApplicationPreparationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProtectedProductionEvidenceUnavailable => formatter.write_str(
                "protected Worker V3 compiler/proof evidence is unavailable for this invocation",
            ),
            Self::Differential(error) => write!(formatter, "simulator preparation failed: {error}"),
        }
    }
}

impl Error for PhysicalApplicationPreparationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Differential(error) => Some(error),
            Self::ProtectedProductionEvidenceUnavailable => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PhysicalApplicationExecutionErrorV1 {
    Execution(GeneratedWorkerV3KfdExecutionError),
    Comparison(PhysicalDifferentialErrorV1),
}

impl fmt::Display for PhysicalApplicationExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(error) => write!(formatter, "direct-KFD execution failed: {error}"),
            Self::Comparison(error) => write!(formatter, "physical comparison failed: {error}"),
        }
    }
}

impl Error for PhysicalApplicationExecutionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(error) => Some(error),
            Self::Comparison(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_evidence_gate_rejects_unavailable_invocations() {
        assert!(matches!(
            require_protected_differential_evidence(
                GeneratedWorkerV3KfdDifferentialAvailabilityV1::ProtectedProductionEvidenceUnavailable,
            ),
            Err(PhysicalApplicationPreparationErrorV1::ProtectedProductionEvidenceUnavailable)
        ));
        assert!(
            require_protected_differential_evidence(
                GeneratedWorkerV3KfdDifferentialAvailabilityV1::SealedObservationAvailable,
            )
            .is_ok()
        );
    }
}
