use super::*;
use fe2o3_kernel_ir::ValueId;

fn matrix() -> MatrixOperation {
    MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(
        [ValueId(0); 8],
        [ValueId(1); 8],
        [ValueId(2); 4],
    )
    .with_declared_tensor_layout(
        TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
    )
}

#[test]
fn only_the_exact_fp4_identity_scale_layout_is_supported() {
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
    let mut wrong = exact.clone();
    wrong.tensor_layout.as_mut().unwrap().a.shape = [16, 64];
    assert!(!supported(&wrong));
    let mut mixed = exact.clone();
    mixed.tensor_layout =
        Some(TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64());
    assert!(!supported(&mixed));
    let mut fp8 = exact.clone();
    fp8.tensor_layout =
        Some(TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64());
    assert!(!supported(&fp8));
    let mut wrong = exact;
    wrong.tensor_layout.as_mut().unwrap().b.fragment_elements = 31;
    assert!(!supported(&wrong));
}

#[test]
fn all_nibbles_and_upper_bits_are_classified_without_masking() {
    let expected = [
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(4),
        Some(6),
        Some(8),
        Some(12),
        None,
        Some(-1),
        Some(-2),
        Some(-3),
        Some(-4),
        Some(-6),
        Some(-8),
        Some(-12),
    ];
    for code in 0..256 {
        assert_eq!(
            half_units(code),
            expected.get(code as usize).copied().flatten()
        );
    }
    assert_eq!(half_units(u32::MAX), None);
}

#[test]
fn quarter_domain_boundary_and_adjacent_f32_values_are_exact() {
    for q in [
        0, 1, -1, 2, -2, 3, -3, 4, 127, -127, 4_194_303, -4_194_303, 4_194_304, -4_194_304,
    ] {
        let bits = ((q as f64) / 4.0) as f32;
        assert_eq!(accumulator_quarters(bits.to_bits()), Some(q));
    }
    for bits in [
        0x8000_0000,
        1,
        0x8000_0001,
        0x3e00_0000,
        0xbe00_0000,
        0x497f_fffe + 1,
        0x4980_0001,
        0xc980_0001,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0x7f80_0001,
    ] {
        assert_eq!(accumulator_quarters(bits), None, "{bits:08x}");
    }
}

#[test]
fn exact_encoding_matches_independent_host_reference_at_all_possible_products() {
    let values = [0_i64, 1, 2, 3, 4, 6, 8, 12, -1, -2, -3, -4, -6, -8, -12];
    for a in values {
        for b in values {
            for c in [0, 1, -1, 4_194_304, -4_194_304] {
                let q = c + 128 * a * b;
                assert_eq!(quarters_to_bits(q), ((q as f64 / 4.0) as f32).to_bits());
            }
        }
    }
    for q in -4096..=4096 {
        assert_eq!(quarters_to_bits(q), ((q as f64 / 4.0) as f32).to_bits());
    }
}
