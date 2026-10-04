use fe2o3_device::{Bf16, Bf16x2, DEVICE_MATH_CONTRACT_VERSION_V1, DeviceMath, F16, LdsElement};

fn assert_lds_element<T: LdsElement>() {}

#[test]
fn public_half_types_are_lds_eligible_and_layout_stable() {
    assert_lds_element::<F16>();
    assert_lds_element::<Bf16>();
    assert_lds_element::<Bf16x2>();

    assert_eq!(core::mem::size_of::<F16>(), 2);
    assert_eq!(core::mem::size_of::<Bf16>(), 2);
    assert_eq!(core::mem::size_of::<Bf16x2>(), 4);
}

#[test]
fn public_conversion_and_packing_api_is_bit_exact() {
    let low = Bf16::from_f32(1.0);
    let high = Bf16::from_f32(-1.0);
    let packed = Bf16x2::new(low, high);

    assert_eq!(F16::from_f32(1.0).to_bits(), 0x3c00);
    assert_eq!(low.to_bits(), 0x3f80);
    assert_eq!(high.to_bits(), 0xbf80);
    assert_eq!(packed.to_bits(), 0xbf80_3f80);
    assert_eq!(packed.to_array(), [low, high]);
}

#[test]
fn device_math_api_is_versioned_and_requires_a_capability() {
    assert_eq!(DEVICE_MATH_CONTRACT_VERSION_V1, 1);

    let _: fn(&DeviceMath, f32) -> f32 = DeviceMath::sqrt_f32;
    let _: fn(&DeviceMath, f32, f32, f32) -> f32 = DeviceMath::mul_add_f32;
    let _: fn(&DeviceMath, Bf16x2, Bf16x2, Bf16x2) -> Bf16x2 = DeviceMath::mul_add_bf16x2;
}
