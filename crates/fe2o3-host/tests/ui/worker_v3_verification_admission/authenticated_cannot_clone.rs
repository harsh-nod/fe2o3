use fe2o3_host::AuthenticatedWorkerV3ExecutableV1;

fn duplicate<K>(value: AuthenticatedWorkerV3ExecutableV1<K>) {
    let _duplicate = value.clone();
}

fn main() {}
