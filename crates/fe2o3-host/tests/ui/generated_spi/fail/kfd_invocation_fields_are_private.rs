use fe2o3_host::GeneratedWorkerV3KfdInvocation;

fn expose<K>(invocation: GeneratedWorkerV3KfdInvocation<'static, K>) {
    let GeneratedWorkerV3KfdInvocation {
        authority,
        device,
        prepared,
        completion,
        differential,
    } = invocation;
    let _ = (authority, device, prepared, completion, differential);
}

fn main() {}
