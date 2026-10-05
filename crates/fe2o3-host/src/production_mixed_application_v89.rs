//! Concrete predicated or CFG publication preparation, not native proof authority.
use crate::{
    MixedWorkerV89PreparationError as PreparationError,
    PreparedMixedWorkerV89Invocation as PreparedInvocation,
    consume_inherited_mixed_worker_v89_application_handoff as consume_handoff,
};
include!("production_mixed_application_family.rs");
fn audit<R: CompilerGeneratedKernelExpectationRosterV1>(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    request: &crate::MixedWorkerV89VerificationRequest<'_, R>,
) -> Result<WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordAuditErrorV1> {
    auditor.audit_mixed_v89(request)
}
pub use ProductionMixedApplicationError as ProductionMixedWorkerV89ApplicationError;
pub use prepare_application as prepare_inherited_mixed_worker_v89_application;
