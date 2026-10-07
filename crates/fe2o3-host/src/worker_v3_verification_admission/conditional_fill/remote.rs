//! Remote proof custody joined to one original publication and production compiler audit.
use super::super::{
    DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3PinnedDescriptorV1,
    RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3AuditorV1,
    WorkerV3CompilerExecutionVerificationV1, WorkerV3VerificationRequestPreparationErrorV1,
    prepare_request,
};
use super::{
    ConditionalFillArtifactView, InertWorkerV3ConditionalFillSubjectV1,
    WorkerV3ConditionalFillPendingErrorV1, derive_worker_v3_conditional_fill_host_contract_v1,
};
use crate::{
    CompilerGeneratedKernelExpectationV1, InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    RegisteredWorkerV3CustodianApplicationV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
};
use fe2o3_verifier::{
    CompilerTargetLineageValidationErrorV1, ConditionalCompilerProofInputValidationErrorV1,
    ValidatedCompilerTargetLineageV1, ValidatedConditionalCompilerProofInputsV1,
    check_conditional_fill_program_v1, validate_conditional_compiler_proof_inputs_v1,
    validate_conditional_compiler_target_lineage_v1,
};
use std::{
    error::Error,
    fmt,
    marker::PhantomData,
    time::{Duration, Instant},
};

mod invocation;
pub use invocation::WorkerV3ConditionalFillInvocationErrorV1;

/// Retains the application controller, original startup publication token and production
/// FD195 audit. Proof ownership stays in the independent controller.
///
/// This is not an unconditional executable or native launch permit. The conditional
/// invocation method separately checks actual argument coverage and binds per-device
/// storage/completion. Neither bytes nor a copied subject can construct this owner. Drop sends no
/// Release and establishes no GPU settlement.
///
/// ```
/// use fe2o3_host::{CompilerGeneratedKernelExpectationV1, RemoteConditionalFillArtifactV1};
/// fn traits<K: CompilerGeneratedKernelExpectationV1>() {
///     fn send_sync<T: Send + Sync>() {}
///     send_sync::<RemoteConditionalFillArtifactV1<K>>();
/// }
/// ```
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// fn no_clone<K>() { cloneable::<fe2o3_host::RemoteConditionalFillArtifactV1<K>>(); }
/// ```
/// ```compile_fail
/// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
///     RemoteConditionalFillArtifactV1};
/// fn promote<K: CompilerGeneratedKernelExpectationV1>(proof: RemoteConditionalFillArtifactV1<K>)
///     -> AuthenticatedWorkerV3ExecutableV1<K> { proof }
/// ```
#[must_use]
pub struct RemoteConditionalFillArtifactV1<K> {
    admission: RecoveredWorkerV3PinnedDescriptorV1,
    current: DurableCurrentLinkPublicationTokenV1,
    finalizer: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    inputs: ValidatedConditionalCompilerProofInputsV1,
    lineage: ValidatedCompilerTargetLineageV1,
    subject: InertWorkerV3ConditionalFillSubjectV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for RemoteConditionalFillArtifactV1<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteConditionalFillArtifactV1")
            .field("lineage", &self.admission.lineage_identity())
            .field("subject", &self.subject)
            .field("launch_authority", &false)
            .finish_non_exhaustive()
    }
}

impl RegisteredWorkerV3CustodianApplicationV1 {
    /// Consumes this registered application and its original currentness token.
    ///
    /// Local marker/program/finalizer failures occur before FD195 admission. Its one-use
    /// production audit runs immediately before the potentially long proof execution.
    /// The deadline can shorten, but cannot refresh, the client's original execution
    /// budget. Late results reject; CPU preparation and cleanup are not hard-preempted.
    ///
    /// # Safety
    /// Transfer exclusive ownership of inherited FD 195 with no existing Rust
    /// owner, outstanding borrow, or concurrent descriptor-table mutation. The
    /// registered application owns its proof channel, not this separate service
    /// slot. The slot may be consumed on failure and must not be reused.
    pub unsafe fn into_remote_conditional_fill<K: CompilerGeneratedKernelExpectationV1>(
        self,
        deadline: Instant,
    ) -> Result<RemoteConditionalFillArtifactV1<K>, WorkerV3RemoteConditionalFillErrorV1> {
        use WorkerV3ConditionalFillPendingErrorV1 as P;
        use WorkerV3RemoteConditionalFillErrorV1 as E;
        remaining(deadline)?;
        let (admission, current) = self.into_parts();
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(P::CurrentPublication)?;
        let request = prepare_request::<K>(&admission, &current).map_err(|error| match error {
            WorkerV3VerificationRequestPreparationErrorV1::Marker(field) => P::Marker(field),
            WorkerV3VerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile => {
                P::UnsupportedGeneratedProfile
            }
        })?;
        let capsule = request.semantic_compiler_handoff().capsule();
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
        let lineage = validate_conditional_compiler_target_lineage_v1(capsule, &inputs)
            .map_err(E::TargetLineage)?;
        let program = check_conditional_fill_program_v1(&inputs, &lineage).map_err(P::Program)?;
        let source = ConditionalFillArtifactView::from_request(&request)
            .check_program(&program)
            .map_err(P::Association)?;
        let contract = derive_worker_v3_conditional_fill_host_contract_v1(&program, &source)
            .map_err(P::Association)?;
        if contract != request.generated_host_contract_identity() {
            return Err(P::Marker("generated host contract").into());
        }
        let finalizer = request
            .independently_revalidate_finalizer_derivation()
            .map_err(P::Finalizer)?;
        if &finalizer != request.finalizer_derivation() {
            return Err(P::FinalizerMismatch.into());
        }
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(P::CurrentPublication)?;
        // SAFETY: the caller transfers the separate inherited service slot.
        let mut auditor = unsafe {
            InheritedWorkerV3CompilerCurrentRecordAuditorV1::admit_production_application_service_with_timeout(
                remaining(deadline)?)
        }.map_err(P::CurrentRecord)?;
        let audit =
            <InheritedWorkerV3CompilerCurrentRecordAuditorV1 as WorkerV3AuditorV1<K>>::audit(
                &mut auditor,
                &request,
            );
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(P::CurrentPublication)?;
        let compiler_execution = audit
            .map_err(P::CurrentRecord)?
            .bind_exact_compiler_execution_v1(
                request.compiler_execution_subject(),
                request.compiler_execution_receipt_carriage(),
            )
            .map_err(P::CompilerExecution)?;
        compiler_execution
            .revalidate_production_deployment()
            .map_err(P::CurrentRecord)?;
        remaining(deadline)?;
        let bytes = admission
            .request_application_custodian_proof(&current, deadline)
            .map_err(E::ProofChannel)?;
        let subject = InertWorkerV3ConditionalFillSubjectV1::match_remote(
            &bytes,
            *request.lineage_identity().as_bytes(),
            *request.challenge_identity().as_bytes(),
        )
        .ok_or(E::Subject)?;
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(P::CurrentPublication)?;
        compiler_execution
            .revalidate_production_deployment()
            .map_err(P::CurrentRecord)?;
        remaining(deadline)?;
        Ok(RemoteConditionalFillArtifactV1 {
            admission,
            current,
            finalizer,
            compiler_execution,
            inputs,
            lineage,
            subject,
            _marker: PhantomData,
        })
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> RemoteConditionalFillArtifactV1<K> {
    /// Brackets one serialized controller probe with original publication/deployment checks.
    /// Concurrent use rejects contention without waiting; no callback runs under its mutex.
    pub fn revalidate(
        &self,
        deadline: Instant,
    ) -> Result<(), WorkerV3RemoteConditionalFillErrorV1> {
        use WorkerV3ConditionalFillPendingErrorV1 as P;
        remaining(deadline)?;
        self.admission
            .revalidate_retained_currentness_token(&self.current)
            .map_err(P::CurrentPublication)?;
        self.compiler_execution
            .revalidate_production_deployment()
            .map_err(P::CurrentRecord)?;
        self.admission
            .probe_application_custodian_proof(&self.current, deadline)
            .map_err(WorkerV3RemoteConditionalFillErrorV1::ProofChannel)?;
        self.compiler_execution
            .revalidate_production_deployment()
            .map_err(P::CurrentRecord)?;
        remaining(deadline)?;
        Ok(())
    }
    pub fn descriptor(&self) -> &fe2o3_kernel_descriptor::KernelDescriptorV1 {
        self.admission.descriptor()
    }
    pub const fn subject(&self) -> &InertWorkerV3ConditionalFillSubjectV1 {
        &self.subject
    }
    pub const fn compiler_inputs(&self) -> &ValidatedConditionalCompilerProofInputsV1 {
        &self.inputs
    }
    pub const fn target_lineage(&self) -> &ValidatedCompilerTargetLineageV1 {
        &self.lineage
    }
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.finalizer
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn remaining(deadline: Instant) -> Result<Duration, WorkerV3RemoteConditionalFillErrorV1> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(WorkerV3RemoteConditionalFillErrorV1::Deadline)
}

#[derive(Debug)]
pub enum WorkerV3RemoteConditionalFillErrorV1 {
    Pending(WorkerV3ConditionalFillPendingErrorV1),
    CompilerInputs(ConditionalCompilerProofInputValidationErrorV1),
    TargetLineage(CompilerTargetLineageValidationErrorV1),
    ProofChannel(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Subject,
    Deadline,
}
impl From<WorkerV3ConditionalFillPendingErrorV1> for WorkerV3RemoteConditionalFillErrorV1 {
    fn from(error: WorkerV3ConditionalFillPendingErrorV1) -> Self {
        Self::Pending(error)
    }
}
impl fmt::Display for WorkerV3RemoteConditionalFillErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "remote conditional fill rejected: {self:?}")
    }
}
impl Error for WorkerV3RemoteConditionalFillErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pending(error) => Some(error),
            Self::CompilerInputs(error) => Some(error),
            Self::TargetLineage(error) => Some(error),
            Self::ProofChannel(error) => Some(error),
            Self::Subject | Self::Deadline => None,
        }
    }
}
