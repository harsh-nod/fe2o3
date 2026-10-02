use generic_worker_v3_adapter_fixture::context_map_gpu;
use gpu_host::{GeneratedContextReadSlice, GeneratedContextReadWriteSlice};
fn wrong(source: GeneratedContextReadSlice<f32>, output: GeneratedContextReadWriteSlice<f32>) {
    let _ = context_map_gpu::ContextArguments::new(1.0, source, output);
}
fn main() {}
