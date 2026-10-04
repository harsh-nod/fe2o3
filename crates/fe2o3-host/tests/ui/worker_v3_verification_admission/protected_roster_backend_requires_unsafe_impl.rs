use fe2o3_host::{
    CompilerGeneratedKernelExpectationRosterV1, WorkerV3ProtectedRosterVerificationEvidenceV1,
    WorkerV3ProtectedRosterVerifierBackendV1, WorkerV3RosterVerificationRequestV1,
};

struct ExternalBackend;

impl<R: CompilerGeneratedKernelExpectationRosterV1> WorkerV3ProtectedRosterVerifierBackendV1<R>
    for ExternalBackend
{
    type Error = ();

    unsafe fn verify_protected_roster(
        &mut self,
        _request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    ) -> Result<WorkerV3ProtectedRosterVerificationEvidenceV1, Self::Error> {
        unreachable!()
    }
}

fn main() {}
