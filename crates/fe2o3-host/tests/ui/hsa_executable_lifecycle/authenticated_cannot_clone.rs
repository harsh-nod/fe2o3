use fe2o3_host::AuthenticatedWorkerV2ExecutableV1;

fn duplicate<K>(authenticated: AuthenticatedWorkerV2ExecutableV1<K>) {
    let _duplicate = authenticated.clone();
}

fn main() {}
