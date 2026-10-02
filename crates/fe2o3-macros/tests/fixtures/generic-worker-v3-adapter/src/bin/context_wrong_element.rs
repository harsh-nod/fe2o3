use generic_worker_v3_adapter_fixture::context_map_gpu;
use gpu_host::{GeneratedContextReadSlice, GeneratedContextWriteSlice};
fn wrong(source: GeneratedContextReadSlice<u32>, output: GeneratedContextWriteSlice<f32>) {
    let _ = context_map_gpu::ContextArguments::new(1.0, source, output);
}
fn main() {}
