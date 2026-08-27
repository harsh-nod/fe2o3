use fe2o3_host::GeneratedWorkerV3KfdInvocation;

fn extract<K>(invocation: GeneratedWorkerV3KfdInvocation<'static, K>) {
    let _ = invocation.into_unchecked_kfd_request();
}

fn main() {}
