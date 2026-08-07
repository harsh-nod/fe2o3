use fe2o3_host::{
    CompilerGeneratedKernelContractV1, WorkerV2PrerequisiteAuthenticatorV1,
    WorkerV2PrerequisiteDecisionV1, WorkerV2PrerequisiteRequestV1,
};

struct Authenticator;

impl<K: CompilerGeneratedKernelContractV1> WorkerV2PrerequisiteAuthenticatorV1<K>
    for Authenticator
{
    type Error = ();

    unsafe fn authenticate(
        &mut self,
        _request: &WorkerV2PrerequisiteRequestV1<'_, K>,
    ) -> Result<WorkerV2PrerequisiteDecisionV1, Self::Error> {
        todo!()
    }
}

fn main() {}
