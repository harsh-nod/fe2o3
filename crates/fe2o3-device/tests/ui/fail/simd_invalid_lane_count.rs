use fe2o3_device::GpuSimd;

fn main() {
    let _ = GpuSimd::<u32, 3>::splat(0);
}
