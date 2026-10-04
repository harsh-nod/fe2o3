use fe2o3_host::AuthenticatedWorkerV3RosterV1;

fn authorize<R>(value: AuthenticatedWorkerV3RosterV1<R>) {
    value.load();
    value.launch();
}

fn main() {}
