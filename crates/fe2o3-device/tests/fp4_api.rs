use core::mem::{align_of, size_of};

use fe2o3_amd_target::{AdvancedCapabilityStatus, AmdTargetId, MfmaFamily, MxFormat};
use fe2o3_device::{
    Fp4E2M1Error, Fp4E2M1Ocp, Fp4E2M1Ocpx8, Gfx950Fp4MfmaAMatrix, Gfx950Fp4MfmaBMatrix,
};

// Independent numerical oracle: choose the closest mathematical value, rather
// than inspecting the implementation's integer midpoint or widening tables.
const POSITIVE_VALUES: [f64; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
fn reference_rne_saturating(value: f32) -> Result<u8, Fp4E2M1Error> {
    if value.is_nan() {
        return Err(Fp4E2M1Error::Nan);
    }
    let sign = if value.is_sign_negative() { 8 } else { 0 };
    let magnitude = f64::from(value).abs();
    if magnitude >= 6.0 {
        return Ok(sign | 7);
    }
    let mut best = 0;
    let mut distance = magnitude;
    for (index, candidate) in POSITIVE_VALUES.iter().enumerate().skip(1) {
        let next_distance = (magnitude - candidate).abs();
        if next_distance < distance || (next_distance == distance && index % 2 == 0) {
            best = index;
            distance = next_distance;
        }
    }
    Ok(sign | best as u8)
}
fn convert(value: f32) -> Result<u8, Fp4E2M1Error> {
    Fp4E2M1Ocp::try_from_f32_rne_saturating(value).map(Fp4E2M1Ocp::to_bits)
}

#[test]
fn layout_constants_and_const_evaluation_are_representation_only() {
    const TIE: Fp4E2M1Ocp = match Fp4E2M1Ocp::try_from_f32_rne_saturating(2.5) {
        Ok(value) => value,
        Err(_) => panic!("finite constant must convert"),
    };
    const PACKED: Fp4E2M1Ocpx8 = Fp4E2M1Ocpx8::from_array([TIE; 8]);
    assert_eq!(size_of::<Fp4E2M1Ocp>(), 1);
    assert_eq!(align_of::<Fp4E2M1Ocp>(), 1);
    assert_eq!(size_of::<Fp4E2M1Ocpx8>(), 4);
    assert_eq!(align_of::<Fp4E2M1Ocpx8>(), 4);
    assert_eq!(TIE.to_bits(), 4);
    assert_eq!(PACKED.to_bits(), 0x4444_4444);
    assert_eq!(Fp4E2M1Ocp::ONE.to_bits(), 2);
    assert_eq!(Fp4E2M1Ocp::MIN.to_f32(), -6.0);
    assert_eq!(Fp4E2M1Ocp::MAX.to_f32(), 6.0);
    assert_eq!(Fp4E2M1Ocp::MIN_POSITIVE.to_f32(), 1.0);
    assert_eq!(Fp4E2M1Ocp::MIN_POSITIVE_SUBNORMAL.to_f32(), 0.5);
}

#[test]
fn all_sixteen_codepoints_widen_and_round_trip_with_both_zero_signs() {
    for bits in 0_u8..16 {
        let value = Fp4E2M1Ocp::try_from_bits(bits).unwrap();
        let magnitude = POSITIVE_VALUES[(bits & 7) as usize] as f32;
        let reference = if bits & 8 == 0 { magnitude } else { -magnitude };
        assert_eq!(value.to_bits(), bits);
        assert_eq!(value.to_f32().to_bits(), reference.to_bits());
        assert_eq!(convert(value.to_f32()), Ok(bits));
        assert_eq!(f32::from(value).to_bits(), reference.to_bits());
    }
}

#[test]
fn upper_encoding_bits_are_rejected_instead_of_silently_masked() {
    for bits in 16_u8..=u8::MAX {
        assert_eq!(
            Fp4E2M1Ocp::try_from_bits(bits),
            Err(Fp4E2M1Error::InvalidEncoding(bits))
        );
        for index in 0..8 {
            let mut unpacked = [3; 8];
            unpacked[index] = bits;
            assert_eq!(
                Fp4E2M1Ocpx8::try_from_unpacked_bytes(unpacked),
                Err(Fp4E2M1Error::InvalidEncoding(bits))
            );
        }
    }
}

#[test]
fn every_midpoint_and_adjacent_f32_obeys_even_ties_for_both_signs() {
    for (midpoint, lower, tie, upper) in [
        (0.25_f32, 0, 0, 1),
        (0.75, 1, 2, 2),
        (1.25, 2, 2, 3),
        (1.75, 3, 4, 4),
        (2.5, 4, 4, 5),
        (3.5, 5, 6, 6),
        (5.0, 6, 6, 7),
    ] {
        for (input, expected) in [
            (f32::from_bits(midpoint.to_bits() - 1), lower),
            (midpoint, tie),
            (f32::from_bits(midpoint.to_bits() + 1), upper),
        ] {
            assert_eq!(convert(input), Ok(expected));
            assert_eq!(convert(-input), Ok(expected | 8));
            assert_eq!(convert(input), reference_rne_saturating(input));
            assert_eq!(convert(-input), reference_rne_saturating(-input));
        }
    }
}

#[test]
fn zero_and_f32_subnormal_underflow_preserve_the_input_sign() {
    for magnitude in [0_u32, 1, 0x007f_ffff, 0x0080_0000, 0x3e7f_ffff] {
        assert_eq!(convert(f32::from_bits(magnitude)), Ok(0));
        assert_eq!(convert(f32::from_bits(magnitude | 0x8000_0000)), Ok(8));
    }
    assert_eq!(Fp4E2M1Ocp::ZERO.to_f32().to_bits(), 0);
    assert_eq!(Fp4E2M1Ocp::NEG_ZERO.to_f32().to_bits(), 0x8000_0000);
    assert_eq!(Fp4E2M1Ocp::ZERO, Fp4E2M1Ocp::NEG_ZERO);
    assert_eq!(convert(0.5), Ok(1));
    assert_eq!(convert(-0.5), Ok(9));
}

#[test]
fn finite_overflow_and_infinity_saturate_but_nan_is_not_made_into_data() {
    for input in [6.0, 6.25, 7.0, 1000.0, f32::MAX, f32::INFINITY] {
        assert_eq!(convert(input), Ok(7));
        assert_eq!(convert(-input), Ok(15));
    }
    for bits in [
        0x7f80_0001,
        0x7fbf_ffff,
        0x7fc0_0000,
        0x7fff_ffff,
        0xff80_0001,
        0xffbf_ffff,
        0xffc0_0000,
        0xffff_ffff,
    ] {
        assert_eq!(convert(f32::from_bits(bits)), Err(Fp4E2M1Error::Nan));
    }
}

#[test]
fn bounded_exponent_fraction_corpus_matches_an_independent_distance_oracle() {
    // 256 exponent bands * 9 fraction patterns * both signs = 4,608 values,
    // including normal/subnormal/zero/infinity and quiet/signaling NaN classes.
    for exponent in 0_u32..=255 {
        for fraction in [
            0,
            1,
            0x0001_ffff,
            0x0020_0000,
            0x003f_ffff,
            0x0040_0000,
            0x0055_5555,
            0x007f_fffe,
            0x007f_ffff,
        ] {
            for sign in [0, 0x8000_0000] {
                let input = f32::from_bits(sign | (exponent << 23) | fraction);
                assert_eq!(
                    convert(input),
                    reference_rne_saturating(input),
                    "input bits {:#010x}",
                    input.to_bits()
                );
            }
        }
    }
}

#[test]
fn classification_negation_and_ordering_keep_e2m1_not_fnuz_semantics() {
    for bits in 0_u8..16 {
        let value = Fp4E2M1Ocp::try_from_bits(bits).unwrap();
        assert!(value.is_finite());
        assert!(!value.is_nan() && !value.is_infinite());
        assert_eq!(value.is_zero(), bits & 7 == 0);
        assert_eq!(value.is_subnormal(), bits & 7 == 1);
        assert_eq!(value.is_normal(), bits & 7 >= 2);
        assert_eq!(value.is_sign_negative(), bits & 8 != 0);
        assert_eq!(value.abs().to_bits(), bits & 7);
        assert_eq!((-value).to_bits(), bits ^ 8);
        for other in 0_u8..16 {
            let other = Fp4E2M1Ocp::try_from_bits(other).unwrap();
            assert_eq!(value == other, value.to_f32() == other.to_f32());
            assert_eq!(
                value.partial_cmp(&other),
                value.to_f32().partial_cmp(&other.to_f32())
            );
        }
    }
}

#[test]
fn packing_has_exact_bit_position_order_and_distinct_unpacked_matrix_storage() {
    let values = core::array::from_fn(|index| Fp4E2M1Ocp::try_from_bits(index as u8).unwrap());
    let packed = Fp4E2M1Ocpx8::from_array(values);
    assert_eq!(packed.to_bits(), 0x7654_3210);
    assert_eq!(packed.to_unpacked_bytes(), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        packed.to_array().map(Fp4E2M1Ocp::to_bits),
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(
        Fp4E2M1Ocpx8::try_from_unpacked_bytes([0, 1, 2, 3, 4, 5, 6, 7])
            .unwrap()
            .to_bits(),
        0x7654_3210
    );
    assert_eq!(
        Fp4E2M1Ocpx8::from_bits(0xfedc_ba98).to_unpacked_bytes(),
        [8, 9, 10, 11, 12, 13, 14, 15]
    );
    for index in [8, 9, usize::MAX] {
        assert!(packed.element(index).is_none());
    }
}

#[test]
fn each_of_sixteen_codes_in_each_storage_element_round_trips_without_cross_talk() {
    for index in 0..8 {
        for value in 0_u8..16 {
            let mut bytes = [5; 8];
            bytes[index] = value;
            let packed = Fp4E2M1Ocpx8::try_from_unpacked_bytes(bytes).unwrap();
            assert_eq!(packed.to_unpacked_bytes(), bytes);
            assert_eq!(packed.to_array().map(Fp4E2M1Ocp::to_bits), bytes);
            assert_eq!(packed.element(index).unwrap().to_bits(), value);
            assert_eq!(
                Fp4E2M1Ocpx8::from_array(packed.to_array()).to_bits(),
                packed.to_bits()
            );
        }
    }
    for bit in 0..32 {
        let packed = Fp4E2M1Ocpx8::from_bits(1_u32 << bit);
        for index in 0..8 {
            assert_eq!(
                packed.element(index).unwrap().to_bits(),
                if index == bit / 4 { 1 << (bit % 4) } else { 0 }
            );
        }
    }
}

#[test]
fn mixed_packed_bits_round_trip_and_representation_equality_is_explicit() {
    let mut raw = 0x9123_abcd_u32;
    for _ in 0..512 {
        let packed = Fp4E2M1Ocpx8::from_bits(raw);
        assert_eq!(Fp4E2M1Ocpx8::from_array(packed.to_array()).to_bits(), raw);
        assert_eq!(
            Fp4E2M1Ocpx8::try_from_unpacked_bytes(packed.to_unpacked_bytes())
                .unwrap()
                .to_bits(),
            raw
        );
        raw = raw.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    }
    let negative_zeros = Fp4E2M1Ocpx8::from_bits(0x8888_8888);
    assert_eq!(negative_zeros, Fp4E2M1Ocpx8::ZERO);
    assert_ne!(negative_zeros.to_bits(), Fp4E2M1Ocpx8::ZERO.to_bits());
}

#[test]
fn checked_unpacked_bytes_fit_existing_gfx950_a_and_b_matrix_views() {
    let values = [0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 6.0, -6.0]
        .map(|value| Fp4E2M1Ocp::try_from_f32_rne_saturating(value).unwrap());
    let bytes = Fp4E2M1Ocpx8::from_array(values).to_unpacked_bytes();
    assert_eq!(bytes, [0, 8, 1, 9, 2, 10, 7, 15]);
    assert!(Gfx950Fp4MfmaAMatrix::row_major(&bytes, 0, 1, 8, 8).is_ok());
    assert!(Gfx950Fp4MfmaBMatrix::row_major(&bytes, 0, 8, 1, 1).is_ok());
    // Dense packed bytes are not the eight one-byte elements required by a view.
    let dense = Fp4E2M1Ocpx8::from_array(values).to_bits().to_le_bytes();
    assert!(Gfx950Fp4MfmaAMatrix::row_major(&dense, 0, 1, 8, 8).is_err());
    assert!(Gfx950Fp4MfmaBMatrix::row_major(&dense, 0, 8, 1, 1).is_err());
}

#[test]
fn storage_values_do_not_transfer_gfx950_fp4_capability_to_gfx942() {
    let gfx950 = AmdTargetId::parse("gfx950:xnack-")
        .unwrap()
        .capabilities()
        .unwrap();
    let gfx942 = AmdTargetId::parse("gfx942:xnack-")
        .unwrap()
        .capabilities()
        .unwrap();
    assert_eq!(
        gfx950.mfma_family_support(MfmaFamily::F32FromFp4Ocp),
        AdvancedCapabilityStatus::Supported
    );
    assert_eq!(
        gfx950.mx_format_support(MxFormat::Fp4),
        AdvancedCapabilityStatus::Supported
    );
    assert_ne!(
        gfx942.mfma_family_support(MfmaFamily::F32FromFp4Ocp),
        AdvancedCapabilityStatus::Supported
    );
    assert_ne!(
        gfx942.mx_format_support(MxFormat::Fp4),
        AdvancedCapabilityStatus::Supported
    );
    // This is existing target metadata, not emitted-code or hardware evidence.
    assert_eq!(Fp4E2M1Ocp::MAX.to_f32(), 6.0);
}
