use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn execute(invocation: gpu_host::GeneratedWorkerV3ContextInvocationV1<Marker>) {
    let _ = invocation.execute();
}

fn main() {}
