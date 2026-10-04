use super::super::{
    CompilerGeneratedKernelExpectationV1, DurableCurrentLinkPublicationTokenV1,
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3PinnedDescriptorV1,
    RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3AuditorV1,
    WorkerV3CompilerExecutionEvidenceErrorV1, WorkerV3CompilerExecutionVerificationV1,
    WorkerV3HsacoPublicationErrorV1, WorkerV3VerificationRequestPreparationErrorV1,
    prepare_request,
};
use super::{InertWorkerV3ConditionalFillSubjectV1, WorkerV3ConditionalFillAssociationErrorV1};
use crate::{
    InheritedWorkerV3CompilerCurrentRecordAuditorV1, WorkerV3CompilerCurrentRecordAuditErrorV1,
    WorkerV3HostLineageIdentityV1,
};
use fe2o3_kernel_analysis::Gfx942FillAnalysisErrorV1;
use fe2o3_verifier::{ConditionalFillProgramErrorV1, OwnedConditionalFillRefinementExecutionV1};
use std::{error::Error, fmt, marker::PhantomData};

/// Retains one current publication, executed refinement and fresh compiler-service audit.
///
/// This is pending evidence, not an unconditional executable. The signed compiler
/// audit does not establish protected key custody or independently administered
/// anchor deployment. Actual prepared storage, coverage, device, native entry and
/// completion remain separate invocation obligations.
///
/// ```
/// use fe2o3_host::{CompilerGeneratedKernelExpectationV1,
///     PendingWorkerV3ConditionalFillArtifactV1};
/// fn check_traits<K: CompilerGeneratedKernelExpectationV1>() {
///     fn requires_send_sync<T: Send + Sync>() {}
///     requires_send_sync::<PendingWorkerV3ConditionalFillArtifactV1<K>>();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::{CompilerGeneratedKernelExpectationV1,
///     PendingWorkerV3ConditionalFillArtifactV1};
/// fn requires_clone<T: Clone>() {}
/// fn cannot_clone<K: CompilerGeneratedKernelExpectationV1>() {
///     requires_clone::<PendingWorkerV3ConditionalFillArtifactV1<K>>();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1,
///     CompilerGeneratedKernelExpectationV1, PendingWorkerV3ConditionalFillArtifactV1};
/// fn cannot_promote<K: CompilerGeneratedKernelExpectationV1>(
///     pending: PendingWorkerV3ConditionalFillArtifactV1<K>,
/// ) -> AuthenticatedWorkerV3ExecutableV1<K> { pending }
/// ```
#[must_use]
pub struct PendingWorkerV3ConditionalFillArtifactV1<K> {
    admission: RecoveredWorkerV3PinnedDescriptorV1,
    current: DurableCurrentLinkPublicationTokenV1,
    finalizer: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    refinement: OwnedConditionalFillRefinementExecutionV1,
    subject: InertWorkerV3ConditionalFillSubjectV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for PendingWorkerV3ConditionalFillArtifactV1<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingWorkerV3ConditionalFillArtifactV1")
            .field("lineage", &self.admission.lineage_identity())
            .field("launch_authority", &false)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> PendingWorkerV3ConditionalFillArtifactV1<K> {
    /// Consumes the original artifact and refinement under one retained publication token.
    ///
    /// Local association failures do not consume the auditor's endpoint. Once the
    /// service transaction starts, success or failure consumes that endpoint. Its
    /// client generates the fresh challenge; the deterministic host-request identity
    /// is not used as a nonce. All failures consume the supplied artifact/refinement.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_host::{CompilerGeneratedKernelExpectationV1,
    ///     InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    ///     PendingWorkerV3ConditionalFillArtifactV1, RecoveredWorkerV3PinnedDescriptorV1};
    /// use fe2o3_verifier::OwnedConditionalFillRefinementExecutionV1;
    /// fn consumed<K: CompilerGeneratedKernelExpectationV1>(
    ///     admission: RecoveredWorkerV3PinnedDescriptorV1,
    ///     refinement: OwnedConditionalFillRefinementExecutionV1,
    ///     auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    /// ) {
    ///     let _ = PendingWorkerV3ConditionalFillArtifactV1::<K>::check(
    ///         admission, refinement, auditor);
    ///     let _ = (&admission, &refinement);
    /// }
    /// ```
    pub fn check(
        admission: RecoveredWorkerV3PinnedDescriptorV1,
        refinement: OwnedConditionalFillRefinementExecutionV1,
        auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    ) -> Result<Self, WorkerV3ConditionalFillPendingErrorV1> {
        use WorkerV3ConditionalFillPendingErrorV1 as E;
        let current = admission
            .acquire_retained_currentness_token()
            .map_err(E::CurrentPublication)?;
        let (finalizer, compiler_execution, subject) = {
            let request =
                prepare_request::<K>(&admission, &current).map_err(|error| match error {
                    WorkerV3VerificationRequestPreparationErrorV1::Marker(field) => {
                        E::Marker(field)
                    }
                    WorkerV3VerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile => {
                        E::UnsupportedGeneratedProfile
                    }
                })?;
            let subject = InertWorkerV3ConditionalFillSubjectV1::check(&request, &refinement)?;
            let finalizer = request
                .independently_revalidate_finalizer_derivation()
                .map_err(E::Finalizer)?;
            if &finalizer != request.finalizer_derivation() {
                return Err(E::FinalizerMismatch);
            }
            admission
                .revalidate_retained_currentness_token(&current)
                .map_err(E::CurrentPublication)?;
            let audit =
                <InheritedWorkerV3CompilerCurrentRecordAuditorV1 as WorkerV3AuditorV1<K>>::audit(
                    auditor, &request,
                );
            // Recheck even when the one-use service call failed, as in ordinary host admission.
            admission
                .revalidate_retained_currentness_token(&current)
                .map_err(E::CurrentPublication)?;
            let compiler_execution = audit
                .map_err(E::CurrentRecord)?
                .bind_exact_compiler_execution_v1(
                    request.compiler_execution_subject(),
                    request.compiler_execution_receipt_carriage(),
                )
                .map_err(E::CompilerExecution)?;
            (finalizer, compiler_execution, subject)
        };
        Ok(Self {
            admission,
            current,
            finalizer,
            compiler_execution,
            refinement,
            subject,
            _marker: PhantomData,
        })
    }

    /// Rechecks publication currentness only; production deployment is a separate fallible check.
    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.admission
            .revalidate_retained_currentness_token(&self.current)
    }
    /// Rechecks retained production configuration without consuming another service connection.
    #[cfg(target_arch = "x86_64")]
    pub fn revalidate_production_deployment(
        &self,
    ) -> Result<(), WorkerV3CompilerCurrentRecordAuditErrorV1> {
        self.compiler_execution.revalidate_production_deployment()
    }
    pub fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.admission.lineage_identity()
    }
    pub fn descriptor(&self) -> &fe2o3_kernel_descriptor::KernelDescriptorV1 {
        self.admission.descriptor()
    }
    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.admission.target()
    }
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.finalizer
    }
    pub const fn compiler_execution(&self) -> &WorkerV3CompilerExecutionVerificationV1 {
        &self.compiler_execution
    }
    pub const fn refinement(&self) -> &OwnedConditionalFillRefinementExecutionV1 {
        &self.refinement
    }
    /// Borrows matching data only; the original proof/publication owners remain here.
    pub const fn subject(&self) -> &InertWorkerV3ConditionalFillSubjectV1 {
        &self.subject
    }
    pub const fn authenticates_protected_compiler_origin(&self) -> bool {
        false
    }
    pub const fn authenticates_verification_authority(&self) -> bool {
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
pub enum WorkerV3ConditionalFillPendingErrorV1 {
    Marker(&'static str),
    UnsupportedGeneratedProfile,
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    Program(ConditionalFillProgramErrorV1),
    Machine(Gfx942FillAnalysisErrorV1),
    Association(WorkerV3ConditionalFillAssociationErrorV1),
    Finalizer(WorkerV3HsacoPublicationErrorV1),
    FinalizerMismatch,
    CurrentRecord(WorkerV3CompilerCurrentRecordAuditErrorV1),
    CompilerExecution(WorkerV3CompilerExecutionEvidenceErrorV1),
}

impl fmt::Display for WorkerV3ConditionalFillPendingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pending conditional fill rejected: {self:?}")
    }
}
impl Error for WorkerV3ConditionalFillPendingErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::Machine(error) => Some(error),
            Self::Association(error) => Some(error),
            Self::Finalizer(error) => Some(error),
            Self::CurrentRecord(error) => Some(error),
            Self::CompilerExecution(error) => Some(error),
            Self::Marker(_) | Self::UnsupportedGeneratedProfile | Self::FinalizerMismatch => None,
        }
    }
}
