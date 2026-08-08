use fe2o3_host::{
    AuthenticatedWorkerV2ExecutableV1 as Authenticated,
    CompilerGeneratedKernelExpectationV1, RecoveredWorkerV2PinnedDescriptorV1 as Recovered,
    WorkerV2PrerequisiteAuthenticatorV1,
};

fn authenticate<K, A>(recovered: Recovered, authenticator: &mut A)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: WorkerV2PrerequisiteAuthenticatorV1<K>,
{
    let _ = Authenticated::<K>::authenticate(recovered, authenticator);
}

fn main() {}
