use fe2o3_device::{
    Bf16, Bf16x2, DeviceMath, F16, Fp8E4M3Fnuz, Fp8E4M3Fnuzx4, Fp8E5M2Fnuz,
    Fp8E5M2Fnuzx4,
};

fn type_check_device_intrinsics(math: &DeviceMath) {
    let _: f32 = math.sqrt_f32(4.0);
    let _: f32 = math.mul_add_f32(2.0, 3.0, 4.0);
    let lanes = Bf16x2::new(Bf16::ONE, Bf16::ONE);
    let _: Bf16x2 = math.mul_add_bf16x2(lanes, lanes, lanes);
}

fn main() {
    let half = F16::from_f32(1.5);
    let brain = Bf16::from_f32(-2.25);
    let pair = Bf16x2::new(brain, Bf16::ONE);

    let _: u16 = half.to_bits();
    let _: f32 = half.to_f32();
    let _: u32 = pair.to_bits();
    let _: [Bf16; 2] = pair.to_array();
    let fp8 = Fp8E4M3Fnuz::from_f32(1.5);
    let bf8 = Fp8E5M2Fnuz::from_f32(-2.0);
    let fp8x4 = Fp8E4M3Fnuzx4::from_array([fp8; 4]);
    let bf8x4 = Fp8E5M2Fnuzx4::new(bf8, bf8, bf8, bf8);
    let _: u8 = fp8.to_bits();
    let _: f32 = bf8.to_f32();
    let _: u32 = fp8x4.to_bits();
    let _: [Fp8E5M2Fnuz; 4] = bf8x4.to_array();
    let _: fn(&DeviceMath) = type_check_device_intrinsics;
}
