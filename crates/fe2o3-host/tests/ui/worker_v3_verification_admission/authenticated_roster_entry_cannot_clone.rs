use fe2o3_host::AuthenticatedWorkerV3RosterEntryV1;

fn duplicate<R, K>(value: AuthenticatedWorkerV3RosterEntryV1<'_, R, K>) {
    let _duplicate = value.clone();
}

fn main() {}
