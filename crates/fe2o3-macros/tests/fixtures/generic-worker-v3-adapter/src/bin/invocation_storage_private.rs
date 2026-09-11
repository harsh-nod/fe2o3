use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn extract(invocation: gpu_host::GeneratedWorkerV3RuntimeInvocationV1<Marker>) {
    let _ = invocation.storage;
}

fn main() {}
