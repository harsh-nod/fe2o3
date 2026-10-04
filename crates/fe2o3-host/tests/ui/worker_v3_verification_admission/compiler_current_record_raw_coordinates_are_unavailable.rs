use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_host::InheritedWorkerV3CompilerCurrentRecordAuditorV1;
use fe2o3_runtime_protocol::CompilerExecutionReceiptCarriageV1;

fn bypass_request(
    auditor: &mut InheritedWorkerV3CompilerCurrentRecordAuditorV1,
    subject: &InertCompilerExecutionSubjectV1,
    carriage: &CompilerExecutionReceiptCarriageV1,
) {
    let _ = auditor.audit_exact(subject, carriage);
}

fn main() {}
