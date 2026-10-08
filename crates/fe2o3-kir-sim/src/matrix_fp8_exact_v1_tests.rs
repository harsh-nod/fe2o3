use super::*;
use fe2o3_kernel_ir::ValueId;

fn matrix() -> MatrixOperation {
    MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(
        [ValueId(0); 8],
        [ValueId(1); 8],
        [ValueId(2); 4],
    )
    .with_declared_tensor_layout(
        TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
    )
}

#[test]
fn exact_ocp_profile_and_split_k_layout_only() {
    let exact = matrix();
    assert!(supported(&exact));
    let mut wrong = exact.clone();
    wrong.tensor_layout = None;
    assert!(!supported(&wrong));
    let mut wrong = exact.clone();
    wrong.active_lanes = 32;
    assert!(!supported(&wrong));
    let mut wrong = exact.clone();
    wrong.convergence = Convergence::uniform(SynchronizationScope::Workgroup);
    assert!(!supported(&wrong));
    for layout in [
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
    ] {
        let mut wrong = exact.clone();
        wrong.tensor_layout = Some(layout);
        assert!(!supported(&wrong));
    }
    let mut wrong = exact.clone();
    wrong.tensor_layout.as_mut().unwrap().b.fragment_elements = 31;
    assert!(!supported(&wrong));
    let mut wrong = exact;
    let MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } = &mut wrong.kind else {
        unreachable!()
    };
    *profile = MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64();
    assert!(!supported(&wrong));
}

// Independent real-value OCP oracle: never uses implementation bit arithmetic.
fn decode(byte: u8) -> f64 {
    let sign = if byte & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((byte >> 3) & 15);
    let mantissa = f64::from(byte & 7);
    if exponent == 15 && mantissa == 7.0 {
        return f64::NAN;
    }
    if exponent == 0 {
        return sign * mantissa * 2.0_f64.powi(-9);
    }
    sign * (1.0 + mantissa / 8.0) * 2.0_f64.powi(exponent - 7)
}

#[test]
fn every_byte_has_exact_ocp_domain_classification() {
    for byte in 0..256 {
        let actual = decode(byte as u8);
        let expected = if byte != 0x80
            && actual.is_finite()
            && actual.abs() <= 16.0
            && (actual * 4.0).fract() == 0.0
        {
            Some((actual * 4.0) as i64)
        } else {
            None
        };
        assert_eq!(quarter_units(byte), expected, "byte {byte:02x}");
    }
    for byte in [256, u32::MAX] {
        assert_eq!(quarter_units(byte), None);
    }
    assert_eq!(quarter_units(0x38), Some(4)); // OCP +1, not FNUZ.
    assert_eq!(quarter_units(0x58), Some(64));
    assert_eq!(quarter_units(0xd8), Some(-64));
}

#[test]
fn accumulator_grid_and_adjacent_boundaries_are_not_rounded() {
    for q in [
        0, 1, -1, 2, -2, 3, -3, 16, 127, -127, 4_194_303, -4_194_303, 4_194_304, -4_194_304,
    ] {
        assert_eq!(
            accumulator_sixteenths(((q as f64 / 16.0) as f32).to_bits()),
            Some(q)
        );
    }
    for bits in [
        0x8000_0000,
        1,
        0x8000_0001,
        0x3d00_0000,
        0xbd00_0000,
        0x487f_ffff,
        0x4880_0001,
        0xc880_0001,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0x7f80_0001,
    ] {
        assert_eq!(accumulator_sixteenths(bits), None, "{bits:08x}");
    }
}

#[test]
fn all_admitted_products_extremes_and_cancellation_encode_exactly() {
    let values: Vec<i64> = (0..256).filter_map(quarter_units).collect();
    for a in &values {
        for b in &values {
            for c in [0, 1, -1, 4_194_304, -4_194_304] {
                let q = c + 128 * a * b;
                assert_eq!(sixteenths_to_bits(q), ((q as f64 / 16.0) as f32).to_bits());
            }
        }
    }
    for q in -4096..=4096 {
        assert_eq!(sixteenths_to_bits(q), ((q as f64 / 16.0) as f32).to_bits());
    }
    assert_eq!(sixteenths_to_bits(0), 0);
}
