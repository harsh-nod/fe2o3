use fe2o3_device::gpu_printf;

fn main() {
    let format = "value={}";
    gpu_printf!(format, 7u32);
}
