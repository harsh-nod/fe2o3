use core::mem::{align_of, size_of};

use fe2o3_device::{Bf16, F16, Fp8E4M3Fnuz, GpuSimd};

#[test]
fn transparent_layout_matches_the_backing_array() {
    assert_eq!(size_of::<GpuSimd<u8, 2>>(), size_of::<[u8; 2]>());
    assert_eq!(align_of::<GpuSimd<u8, 2>>(), align_of::<[u8; 2]>());
    assert_eq!(size_of::<GpuSimd<u32, 4>>(), size_of::<[u32; 4]>());
    assert_eq!(align_of::<GpuSimd<u32, 4>>(), align_of::<[u32; 4]>());
    assert_eq!(size_of::<GpuSimd<f64, 8>>(), size_of::<[f64; 8]>());
    assert_eq!(align_of::<GpuSimd<f64, 8>>(), align_of::<[f64; 8]>());
    assert_eq!(
        GpuSimd::<u16, 16>::__fe2o3_rust_layout_v1(),
        (size_of::<[u16; 16]>(), align_of::<[u16; 16]>())
    );
}

#[test]
fn construction_and_access_preserve_lane_order() {
    let mut value = GpuSimd::<u32, 4>::from_array([3, 5, 8, 13]);
    assert_eq!(GpuSimd::<u32, 4>::LANES, 4);
    assert_eq!(value.to_array(), [3, 5, 8, 13]);
    assert_eq!(value.lane(0), Some(&3));
    assert_eq!(value.lane(3), Some(&13));
    assert_eq!(value.lane(4), None);

    *value.lane_mut(2).unwrap() = 21;
    value[0] = 1;
    assert_eq!(value.as_array(), &[1, 5, 21, 13]);
    assert!(value.lane_mut(usize::MAX).is_none());
}

#[test]
fn arithmetic_is_lane_wise_and_does_not_cross_lanes() {
    let lhs = GpuSimd::<i32, 4>::from_array([2, -3, 5, -7]);
    let rhs = GpuSimd::<i32, 4>::from_array([11, 13, -17, -19]);

    assert_eq!((lhs + rhs).to_array(), [13, 10, -12, -26]);
    assert_eq!((lhs - rhs).to_array(), [-9, -16, 22, 12]);
    assert_eq!((lhs * rhs).to_array(), [22, -39, -85, 133]);
    assert_eq!((-lhs).to_array(), [-2, 3, -5, 7]);
    assert_eq!(
        (GpuSimd::<u32, 4>::splat(24) / GpuSimd::from_array([2, 3, 4, 6])).to_array(),
        [12, 8, 6, 4]
    );
}

#[test]
fn reviewed_storage_floats_are_valid_lanes() {
    let halves = GpuSimd::<F16, 2>::splat(F16::from_f32(1.5));
    let bfloat = GpuSimd::<Bf16, 8>::splat(Bf16::from_f32(-2.0));
    let fp8 = GpuSimd::<Fp8E4M3Fnuz, 16>::splat(Fp8E4M3Fnuz::ONE);

    assert_eq!(halves.to_array().map(F16::to_f32), [1.5; 2]);
    assert_eq!(bfloat.to_array().map(Bf16::to_f32), [-2.0; 8]);
    assert_eq!(fp8.to_array().map(Fp8E4M3Fnuz::to_bits), [0x40; 16]);
}
