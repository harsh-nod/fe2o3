use super::*;
use fe2o3_kernel_ir::ValueId;

fn matrix() -> MatrixOperation {
    MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(
        [ValueId(0); 8],
        [ValueId(1); 8],
        [ValueId(2); 4],
    )
    .with_declared_tensor_layout(
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
    )
}
#[test]
fn only_exact_ordered_mixed_profile_is_admitted() {
    let exact = matrix();
    assert!(supported(&exact));
    let mut wrong = exact.clone();
    wrong.tensor_layout = None;
    assert!(!supported(&wrong));
    for layout in [
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
        TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
    ] {
        let mut wrong = exact.clone();
        wrong.tensor_layout = Some(layout);
        assert!(!supported(&wrong));
    }
    let mut wrong = exact.clone();
    let layout = wrong.tensor_layout.as_mut().unwrap();
    std::mem::swap(&mut layout.a, &mut layout.b);
    assert!(!supported(&wrong));
    let mut wrong = exact.clone();
    wrong.tensor_layout.as_mut().unwrap().b.fragment_elements = 31;
    assert!(!supported(&wrong));
    let mut wrong = exact.clone();
    wrong.active_lanes = 32;
    assert!(!supported(&wrong));
    let mut wrong = exact.clone();
    wrong.convergence = Convergence::uniform(SynchronizationScope::Workgroup);
    assert!(!supported(&wrong));
    let mut wrong = exact;
    let MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } = &mut wrong.kind else {
        unreachable!()
    };
    *profile = MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64();
    assert!(!supported(&wrong));
}
// Independent real-value oracles; neither calls the implementation codecs.
fn fp4(code: u8) -> f64 {
    let magnitude = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0][usize::from(code & 7)];
    if code & 8 == 0 { magnitude } else { -magnitude }
}
fn fp8(code: u8) -> f64 {
    let sign = if code & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((code >> 3) & 15);
    let mantissa = f64::from(code & 7);
    if exponent == 15 && mantissa == 7.0 {
        return f64::NAN;
    }
    if exponent == 0 {
        return sign * mantissa * 2.0_f64.powi(-9);
    }
    sign * (1.0 + mantissa / 8.0) * 2.0_f64.powi(exponent - 7)
}
#[test]
fn all_codes_preserve_distinct_fp4_and_ocp_fp8_domains() {
    for code in 0..16 {
        let expected = if code == 8 {
            None
        } else {
            Some((2.0 * fp4(code as u8)) as i64)
        };
        assert_eq!(half_units(code), expected);
    }
    for code in 0..256 {
        let value = fp8(code as u8);
        let expected = if code != 0x80
            && value.is_finite()
            && value.abs() <= 16.0
            && (value * 4.0).fract() == 0.0
        {
            Some((value * 4.0) as i64)
        } else {
            None
        };
        assert_eq!(quarter_units(code), expected, "OCP code {code:02x}");
    }
    assert_eq!(half_units(16), None);
    assert_eq!(quarter_units(256), None);
    assert_eq!(quarter_units(0x38), Some(4));
}
#[test]
fn every_admitted_mixed_product_and_extreme_sum_matches_independent_oracle() {
    for a in 0..16 {
        if a == 8 {
            continue;
        }
        for b in 0..256 {
            let Some(bq) = quarter_units(b) else {
                continue;
            };
            for c in [0, 1, -1, 4_194_303, -4_194_303, 4_194_304, -4_194_304] {
                let q = c + 128 * 2 * half_units(a).unwrap() * bq;
                assert!(q.abs() <= 4_390_912);
                let oracle = c as f64 / 16.0 + 128.0 * fp4(a as u8) * fp8(b as u8);
                assert_eq!(sixteenths_to_bits(q), (oracle as f32).to_bits());
            }
        }
    }
    assert_eq!(sixteenths_to_bits(0), 0);
}
#[test]
fn accumulator_admission_remains_bit_exact_and_does_not_round() {
    for q in [
        0, 1, -1, 3, -3, 4_194_303, -4_194_303, 4_194_304, -4_194_304,
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
    ] {
        assert_eq!(accumulator_sixteenths(bits), None, "{bits:08x}");
    }
}
