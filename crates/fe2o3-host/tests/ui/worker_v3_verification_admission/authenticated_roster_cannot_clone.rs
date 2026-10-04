use fe2o3_host::AuthenticatedWorkerV3RosterV1;

fn duplicate<R>(value: AuthenticatedWorkerV3RosterV1<R>) {
    let _duplicate = value.clone();
}

fn main() {}
