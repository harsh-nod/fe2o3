#![cfg(target_os = "linux")]

use fe2o3_host::{
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
    InheritedWorkerV3CompilerCurrentRecordAuditorV1, WorkerV3CompilerCurrentRecordAuditErrorV1,
    WorkerV3CompilerCurrentRecordAuditV1, WorkerV3RosterVerificationRequestV1,
};

struct CompileSurfaceRoster;

impl CompilerGeneratedKernelExpectationRosterV1 for CompileSurfaceRoster {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] = &[];
}

#[allow(dead_code)]
fn audit_exact_roster_request<R>(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    request: &WorkerV3RosterVerificationRequestV1<'_, R>,
) -> Result<WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordAuditErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    auditor.audit_roster(request)
}

#[test]
fn roster_current_record_audit_is_exposed_only_through_the_request_type() {
    let _request_bound_method =
        InheritedWorkerV3CompilerCurrentRecordAuditorV1::audit_roster::<CompileSurfaceRoster>;
}
