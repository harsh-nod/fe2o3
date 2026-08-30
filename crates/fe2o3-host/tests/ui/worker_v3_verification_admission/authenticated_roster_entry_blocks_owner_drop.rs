use fe2o3_host::{
    AuthenticatedWorkerV3RosterV1, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV1,
};

fn drop_owner_while_borrowed<R, K>(roster: AuthenticatedWorkerV3RosterV1<R>)
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
{
    let entry = roster.entry::<K>().unwrap();
    drop(roster);
    let _ = entry.ordinal();
}

fn main() {}
