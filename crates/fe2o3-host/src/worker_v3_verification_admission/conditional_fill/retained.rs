//! Executed proof custody for the closed fill profile, independent of application admission.

use super::{
    ConditionalFillArtifactView, InertWorkerV3ConditionalFillSubjectV1,
    WorkerV3ConditionalFillAssociationErrorV1, WorkerV3ConditionalFillPendingErrorV1,
};
use crate::{RecoveredWorkerV3AdmissionErrorV1, check_worker_v3_compiler_closure_v1};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectErrorV1, AuthenticatedPhysicalMachineEffectLimitsV1,
    AuthenticatedPhysicalMachineEffectWorkerV1, Gfx942FillAnalysisErrorV1,
    MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectRequestErrorV1,
    check_gfx942_fill_analysis_v1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_runtime_protocol::MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2;
use fe2o3_verifier::{
    CompilerTargetLineageValidationErrorV1, ConditionalCompilerProofInputValidationErrorV1,
    ConditionalFillProgramErrorV1, ConditionalFillRefinementErrorV1,
    FunctionalRefinementVerusRuntimeLeaseV1, OwnedConditionalFillRefinementExecutionV1,
    check_conditional_fill_program_v1, execute_owned_conditional_fill_refinement_v1,
    validate_conditional_compiler_proof_inputs_v1, validate_conditional_compiler_target_lineage_v1,
};
use std::{
    collections::TryReserveError,
    error::Error,
    fmt,
    time::{Duration, Instant},
};

const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(60);
const PROOF_TIMEOUT_SECONDS: u64 = 180;

/// Retains exact immutable compiler inputs and the original executed fill proof.
///
/// There is no receipt-import constructor or application marker. The supplied analyzer
/// and protected runtime must be independently approved by the caller's deployment.
/// This owner establishes neither publication currentness nor compiler origin and is
/// not an executable. A custodian must separately authenticate application occurrence
/// and retain this owner through every associated invocation's settlement.
///
/// ```
/// use fe2o3_host::RetainedWorkerV3ConditionalFillProofV1;
/// fn owned<T: Send + Sync + 'static>() {}
/// owned::<RetainedWorkerV3ConditionalFillProofV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_host::RetainedWorkerV3ConditionalFillProofV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<RetainedWorkerV3ConditionalFillProofV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1,
///     CompilerGeneratedKernelExpectationV1, RetainedWorkerV3ConditionalFillProofV1};
/// fn promote<K: CompilerGeneratedKernelExpectationV1>(
///     proof: RetainedWorkerV3ConditionalFillProofV1,
/// ) -> AuthenticatedWorkerV3ExecutableV1<K> { proof }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::RetainedWorkerV3ConditionalFillProofV1;
/// fn change(proof: &mut RetainedWorkerV3ConditionalFillProofV1) {
///     proof.finalized_hsaco_bytes()[0] ^= 1;
/// }
/// ```
#[must_use]
pub struct RetainedWorkerV3ConditionalFillProofV1 {
    envelope: Box<[u8]>,
    finalized_hsaco: Box<[u8]>,
    kernel_id: KernelId,
    refinement: OwnedConditionalFillRefinementExecutionV1,
    subject: InertWorkerV3ConditionalFillSubjectV1,
}

impl fmt::Debug for RetainedWorkerV3ConditionalFillProofV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetainedWorkerV3ConditionalFillProofV1")
            .field("subject", &self.subject)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl RetainedWorkerV3ConditionalFillProofV1 {
    pub const fn exact_canonical_envelope_bytes(&self) -> &[u8] {
        &self.envelope
    }
    pub const fn finalized_hsaco_bytes(&self) -> &[u8] {
        &self.finalized_hsaco
    }
    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }
    pub const fn refinement(&self) -> &OwnedConditionalFillRefinementExecutionV1 {
        &self.refinement
    }
    pub const fn subject(&self) -> &InertWorkerV3ConditionalFillSubjectV1 {
        &self.subject
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Moves the original proof into a local admission path, discarding copied inputs.
    ///
    /// `PendingWorkerV3ConditionalFillArtifactV1::check` must still independently
    /// associate it with the retained publication and consume the current-record audit.
    pub fn into_refinement(self) -> OwnedConditionalFillRefinementExecutionV1 {
        self.refinement
    }
}

/// Executes and retains the closed fill's authenticated analysis and protected proof.
///
/// One absolute deadline covers all stages. Analysis is capped at 60 seconds, proof
/// at 180 seconds (floored to remaining whole seconds); late success is rejected.
/// Preparation and subprocess cleanup can overrun these budgets, so this is not a
/// hard wall-clock termination guarantee. A service must additionally supervise its
/// worker against the external session deadline. No FD195 audit is consumed here.
pub fn execute_retained_worker_v3_conditional_fill_v1(
    envelope: Box<[u8]>,
    finalized_hsaco: Box<[u8]>,
    kernel_id: KernelId,
    analyzer: &AuthenticatedPhysicalMachineEffectWorkerV1,
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    deadline: Instant,
) -> Result<RetainedWorkerV3ConditionalFillProofV1, WorkerV3ConditionalFillRetainedErrorV1> {
    use WorkerV3ConditionalFillRetainedErrorV1 as E;
    remaining(deadline, Instant::now())?;
    check_input_sizes(envelope.len(), finalized_hsaco.len())?;
    let closure = check_worker_v3_compiler_closure_v1(&envelope, &finalized_hsaco, kernel_id)
        .map_err(E::Closure)?;
    remaining(deadline, Instant::now())?;
    let capsule = closure.semantic_compiler_handoff().capsule();
    let receipts = capsule.receipts();
    let inputs = validate_conditional_compiler_proof_inputs_v1(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .map_err(E::CompilerInputs)?;
    remaining(deadline, Instant::now())?;
    let lineage = validate_conditional_compiler_target_lineage_v1(capsule, &inputs)
        .map_err(E::TargetLineage)?;
    remaining(deadline, Instant::now())?;
    let program = check_conditional_fill_program_v1(&inputs, &lineage).map_err(E::Program)?;
    let entry = PhysicalMachineEffectEntryRequestV1::new(
        program.function_symbol(),
        PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
    )
    .map_err(E::AnalysisRequest)?;
    // The analyzer owns its request; retain a separate immutable payload for the closure.
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(finalized_hsaco.len())
        .map_err(E::Allocation)?;
    payload.extend_from_slice(&finalized_hsaco);
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        remaining(deadline, Instant::now())?.min(ANALYSIS_TIMEOUT),
        1024 * 1024,
        16384,
    )
    .map_err(E::Analysis)?;
    let analysis = analyzer
        .analyze(payload, vec![entry], limits)
        .map_err(E::Analysis)?;
    remaining(deadline, Instant::now())?;
    let machine =
        check_gfx942_fill_analysis_v1(&analysis, program.function_symbol()).map_err(E::Machine)?;
    ConditionalFillArtifactView::from_closure(&closure)
        .check(&program, &machine)
        .map_err(E::Association)?;
    let timeout = proof_timeout(deadline, Instant::now())?;
    let refinement =
        execute_owned_conditional_fill_refinement_v1(runtime, inputs, lineage, analysis, timeout)
            .map_err(E::Refinement)?;
    remaining(deadline, Instant::now())?;
    let subject = closure
        .check_conditional_fill_refinement_v1(&refinement)
        .map_err(E::Subject)?;
    remaining(deadline, Instant::now())?;
    Ok(RetainedWorkerV3ConditionalFillProofV1 {
        envelope,
        finalized_hsaco,
        kernel_id,
        refinement,
        subject,
    })
}

fn remaining(
    deadline: Instant,
    now: Instant,
) -> Result<Duration, WorkerV3ConditionalFillRetainedErrorV1> {
    deadline
        .checked_duration_since(now)
        .filter(|value| !value.is_zero())
        .ok_or(WorkerV3ConditionalFillRetainedErrorV1::Deadline)
}

fn proof_timeout(
    deadline: Instant,
    now: Instant,
) -> Result<u32, WorkerV3ConditionalFillRetainedErrorV1> {
    let seconds = remaining(deadline, now)?
        .as_secs()
        .min(PROOF_TIMEOUT_SECONDS) as u32;
    if seconds == 0 {
        return Err(WorkerV3ConditionalFillRetainedErrorV1::Deadline);
    }
    Ok(seconds)
}

fn check_input_sizes(
    envelope: usize,
    payload: usize,
) -> Result<(), WorkerV3ConditionalFillRetainedErrorV1> {
    for (field, actual, maximum) in [
        ("envelope", envelope, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2),
        (
            "finalized HSACO",
            payload,
            MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
        ),
    ] {
        if actual == 0 || actual > maximum {
            return Err(WorkerV3ConditionalFillRetainedErrorV1::InputSize {
                field,
                actual,
                maximum,
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum WorkerV3ConditionalFillRetainedErrorV1 {
    Deadline,
    InputSize {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    Allocation(TryReserveError),
    Closure(RecoveredWorkerV3AdmissionErrorV1),
    CompilerInputs(ConditionalCompilerProofInputValidationErrorV1),
    TargetLineage(CompilerTargetLineageValidationErrorV1),
    Program(ConditionalFillProgramErrorV1),
    AnalysisRequest(PhysicalMachineEffectRequestErrorV1),
    Analysis(AuthenticatedPhysicalMachineEffectErrorV1),
    Machine(Gfx942FillAnalysisErrorV1),
    Association(WorkerV3ConditionalFillAssociationErrorV1),
    Refinement(ConditionalFillRefinementErrorV1),
    Subject(WorkerV3ConditionalFillPendingErrorV1),
}

impl fmt::Display for WorkerV3ConditionalFillRetainedErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "retained conditional fill proof rejected: {self:?}")
    }
}

impl Error for WorkerV3ConditionalFillRetainedErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Deadline | Self::InputSize { .. } => None,
            Self::Allocation(error) => Some(error),
            Self::Closure(error) => Some(error),
            Self::CompilerInputs(error) => Some(error),
            Self::TargetLineage(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::AnalysisRequest(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::Machine(error) => Some(error),
            Self::Association(error) => Some(error),
            Self::Refinement(error) => Some(error),
            Self::Subject(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_fill_deadline_never_rounds_up_or_restarts() {
        let now = Instant::now();
        assert!(remaining(now, now).is_err());
        assert!(remaining(now, now + Duration::from_nanos(1)).is_err());
        assert!(proof_timeout(now + Duration::from_millis(999), now).is_err());
        assert_eq!(proof_timeout(now + Duration::from_secs(1), now).unwrap(), 1);
        assert_eq!(
            proof_timeout(now + Duration::from_millis(1999), now).unwrap(),
            1
        );
        let deadline = now + Duration::from_secs(300);
        assert_eq!(proof_timeout(deadline, now).unwrap(), 180);
        assert_eq!(
            proof_timeout(deadline, now + Duration::from_secs(250)).unwrap(),
            50
        );
        assert_eq!(
            remaining(deadline, now).unwrap().min(ANALYSIS_TIMEOUT),
            ANALYSIS_TIMEOUT
        );
        assert_eq!(
            remaining(deadline, now + Duration::from_secs(299))
                .unwrap()
                .min(ANALYSIS_TIMEOUT),
            Duration::from_secs(1)
        );
    }

    #[test]
    fn retained_fill_bounds_inputs_before_replay_or_payload_copy() {
        let envelope = MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2;
        let payload = MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1;
        assert!(check_input_sizes(1, 1).is_ok());
        assert!(check_input_sizes(envelope, payload).is_ok());
        for (e, p, expected) in [
            (0, 1, "envelope"),
            (envelope + 1, 1, "envelope"),
            (1, 0, "finalized HSACO"),
            (1, payload + 1, "finalized HSACO"),
        ] {
            assert!(
                matches!(check_input_sizes(e, p), Err(WorkerV3ConditionalFillRetainedErrorV1::InputSize { field, .. }) if field == expected)
            );
        }
    }
}
