use fe2o3_host::AuthenticatedWorkerV3ExecutableV1;

fn steal_currentness<K>(value: &AuthenticatedWorkerV3ExecutableV1<K>) {
    let _ = value.current_publication_token();
}

fn main() {}
