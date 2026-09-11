use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn main() {
    fn assert_send<T: Send>() {}
    assert_send::<gpu_host::GeneratedWorkerV3RuntimeInvocationV1<Marker>>();
}
