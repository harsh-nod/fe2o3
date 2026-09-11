use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<gpu_host::GeneratedWorkerV3ContextInvocationV1<Marker>>();
}
