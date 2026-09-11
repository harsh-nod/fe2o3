use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn main() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<gpu_host::GeneratedWorkerV3RuntimeInvocationV1<Marker>>();
}
