use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, WorkerV3ProtectedVerificationEvidenceV1,
    WorkerV3ProtectedVerifierBackendV1, WorkerV3VerificationRequestV1,
};

struct ExternalBackend;

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3ProtectedVerifierBackendV1<K>
    for ExternalBackend
{
    type Error = ();

    unsafe fn verify_protected(
        &mut self,
        _request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, Self::Error> {
        unreachable!()
    }
}

fn main() {}
