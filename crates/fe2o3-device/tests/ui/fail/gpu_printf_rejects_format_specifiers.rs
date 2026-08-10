use fe2o3_device::gpu_printf;

fn main() {
    gpu_printf!("value={:x}", 7u32);
}
