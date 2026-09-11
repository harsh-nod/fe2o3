use generic_worker_v3_adapter_fixture::transform_gpu::Marker;

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<gpu_host::GeneratedWorkerV3ContextInvocationV1<Marker>>();
}
