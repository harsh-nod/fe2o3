use fe2o3_host::GeneratedWorkerV3KfdInvocation;

fn duplicate<K>(invocation: GeneratedWorkerV3KfdInvocation<'static, K>) {
    let _ = invocation.clone();
}

fn main() {}
