use fe2o3_device::{F16, GpuSimd};

fn main() {
    let mut lanes = GpuSimd::<u32, 4>::from_array([1, 2, 3, 4]);
    *lanes.lane_mut(1).unwrap() = 7;
    let _ = lanes + GpuSimd::splat(2);

    let half = GpuSimd::<F16, 16>::splat(F16::ONE);
    let _: [F16; 16] = half.into();
}
