use fe2o3_host::{
    AuthenticatedWorkerV3RosterEntryV1, AuthenticatedWorkerV3RosterV1,
    CompilerGeneratedKernelExpectationRosterV1, CompilerGeneratedKernelExpectationV1,
};

fn escape<R, K>(
    roster: &AuthenticatedWorkerV3RosterV1<R>,
) -> AuthenticatedWorkerV3RosterEntryV1<'static, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
{
    roster.entry::<K>().unwrap()
}

fn main() {}
