use fe2o3_device::GpuSimd;

fn main() {
    let _ = GpuSimd::<bool, 4>::splat(true);
}
