use fe2o3_host::AuthenticatedWorkerV3CapabilityRosterV1;

fn duplicate<R>(value: AuthenticatedWorkerV3CapabilityRosterV1<R>) {
    let _duplicate = value.clone();
}

fn main() {}
