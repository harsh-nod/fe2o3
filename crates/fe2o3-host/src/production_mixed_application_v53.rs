//! Concrete predicated or CFG publication preparation, not native proof authority.
use crate::{
    MixedWorkerV53PreparationError as PreparationError,
    PreparedMixedWorkerV53Invocation as PreparedInvocation,
    consume_inherited_mixed_worker_v53_application_handoff as consume_handoff,
};
include!("production_mixed_application_family.rs");
fn audit<R: CompilerGeneratedKernelExpectationRosterV1>(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    request: &crate::MixedWorkerV53VerificationRequest<'_, R>,
) -> Result<WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordAuditErrorV1> {
    auditor.audit_mixed_v53(request)
}
pub use ProductionMixedApplicationError as ProductionMixedWorkerV53ApplicationError;
pub use prepare_application as prepare_inherited_mixed_worker_v53_application;
