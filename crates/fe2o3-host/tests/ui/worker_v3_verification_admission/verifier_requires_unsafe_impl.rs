use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, WorkerV3VerificationDecisionV1,
    WorkerV3VerificationRequestV1, WorkerV3VerifierV1,
};

struct SafeVerifier;

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3VerifierV1<K> for SafeVerifier {
    type Error = ();

    unsafe fn verify(
        &mut self,
        _request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        unimplemented!()
    }
}

fn main() {}
