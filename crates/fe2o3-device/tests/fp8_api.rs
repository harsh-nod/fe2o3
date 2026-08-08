use core::mem::{align_of, size_of};

use fe2o3_device::{Fp8E4M3Fnuz, Fp8E4M3Fnuzx4, Fp8E5M2Fnuz, Fp8E5M2Fnuzx4, LdsElement};

fn assert_lds_element<T: LdsElement>() {}

fn rocm_decode_fingerprint<T>(decode: impl Fn(u8) -> T, bits: impl Fn(T) -> u32) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for raw in u8::MIN..=u8::MAX {
        for byte in bits(decode(raw)).to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn rocm_boundary_encode_fingerprint<T>(
    decode: impl Fn(u8) -> f32,
    encode: impl Fn(f32) -> T,
    bits: impl Fn(T) -> u8,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for raw in 0_u8..=0x7f {
        hash ^= bits(encode(decode(raw))) as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for lower in 0_u8..0x7f {
        let midpoint = (decode(lower) + decode(lower + 1)) * 0.5;
        let below = f32::from_bits(midpoint.to_bits() - 1);
        let above = f32::from_bits(midpoint.to_bits() + 1);
        for input in [below, midpoint, above, -below, -midpoint, -above] {
            hash ^= bits(encode(input)) as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn signed_encoding(magnitude: u8) -> u8 {
    if magnitude == 0 { 0 } else { magnitude | 0x80 }
}

fn assert_all_rounding_boundaries<T>(
    decode: impl Fn(u8) -> f32,
    encode: impl Fn(f32) -> T,
    bits: impl Fn(T) -> u8,
) {
    for lower in 0_u8..0x7f {
        let midpoint = (decode(lower) + decode(lower + 1)) * 0.5;
        let below = f32::from_bits(midpoint.to_bits() - 1);
        let above = f32::from_bits(midpoint.to_bits() + 1);
        let tie = if lower & 1 == 0 { lower } else { lower + 1 };

        assert_eq!(bits(encode(below)), lower, "below tie after {lower:#04x}");
        assert_eq!(bits(encode(midpoint)), tie, "tie after {lower:#04x}");
        assert_eq!(
            bits(encode(above)),
            lower + 1,
            "above tie after {lower:#04x}"
        );

        assert_eq!(
            bits(encode(-below)),
            signed_encoding(lower),
            "negative below tie after {lower:#04x}"
        );
        assert_eq!(
            bits(encode(-midpoint)),
            signed_encoding(tie),
            "negative tie after {lower:#04x}"
        );
        assert_eq!(
            bits(encode(-above)),
            signed_encoding(lower + 1),
            "negative above tie after {lower:#04x}"
        );
    }
}

#[test]
fn scalar_layout_and_constants_are_stable() {
    assert_lds_element::<Fp8E4M3Fnuz>();
    assert_lds_element::<Fp8E5M2Fnuz>();
    assert_eq!(size_of::<Fp8E4M3Fnuz>(), 1);
    assert_eq!(align_of::<Fp8E4M3Fnuz>(), 1);
    assert_eq!(size_of::<Fp8E5M2Fnuz>(), 1);
    assert_eq!(align_of::<Fp8E5M2Fnuz>(), 1);

    assert_eq!(Fp8E4M3Fnuz::ONE.to_bits(), 0x40);
    assert_eq!(Fp8E4M3Fnuz::MAX.to_f32(), 240.0);
    assert_eq!(Fp8E4M3Fnuz::MIN.to_f32(), -240.0);
    assert_eq!(Fp8E4M3Fnuz::MIN_POSITIVE.to_f32(), 2.0_f32.powi(-7));
    assert_eq!(
        Fp8E4M3Fnuz::MIN_POSITIVE_SUBNORMAL.to_f32(),
        2.0_f32.powi(-10)
    );

    assert_eq!(Fp8E5M2Fnuz::ONE.to_bits(), 0x40);
    assert_eq!(Fp8E5M2Fnuz::MAX.to_f32(), 57_344.0);
    assert_eq!(Fp8E5M2Fnuz::MIN.to_f32(), -57_344.0);
    assert_eq!(Fp8E5M2Fnuz::MIN_POSITIVE.to_f32(), 2.0_f32.powi(-15));
    assert_eq!(
        Fp8E5M2Fnuz::MIN_POSITIVE_SUBNORMAL.to_f32(),
        2.0_f32.powi(-17)
    );
}

#[test]
fn packed_x4_layout_lane_order_and_lds_contract_are_stable() {
    assert_lds_element::<Fp8E4M3Fnuzx4>();
    assert_lds_element::<Fp8E5M2Fnuzx4>();
    assert_eq!(size_of::<Fp8E4M3Fnuzx4>(), 4);
    assert_eq!(align_of::<Fp8E4M3Fnuzx4>(), 4);
    assert_eq!(size_of::<Fp8E5M2Fnuzx4>(), 4);
    assert_eq!(align_of::<Fp8E5M2Fnuzx4>(), 4);

    let e4 = Fp8E4M3Fnuzx4::new(
        Fp8E4M3Fnuz::from_bits(0x01),
        Fp8E4M3Fnuz::from_bits(0x23),
        Fp8E4M3Fnuz::from_bits(0x45),
        Fp8E4M3Fnuz::from_bits(0x67),
    );
    assert_eq!(e4.to_bits(), 0x6745_2301);
    assert_eq!(
        e4.to_array().map(Fp8E4M3Fnuz::to_bits),
        [0x01, 0x23, 0x45, 0x67]
    );

    let e5 = Fp8E5M2Fnuzx4::from_array([
        Fp8E5M2Fnuz::from_bits(0x89),
        Fp8E5M2Fnuz::from_bits(0xab),
        Fp8E5M2Fnuz::from_bits(0xcd),
        Fp8E5M2Fnuz::from_bits(0xef),
    ]);
    assert_eq!(e5.to_bits(), 0xefcd_ab89);
    assert_eq!(e5.lane0().to_bits(), 0x89);
    assert_eq!(e5.lane1().to_bits(), 0xab);
    assert_eq!(e5.lane2().to_bits(), 0xcd);
    assert_eq!(e5.lane3().to_bits(), 0xef);
}

#[test]
fn every_single_bit_packing_mutation_stays_in_its_lane() {
    for bit in 0..u32::BITS {
        let raw = 1_u32 << bit;
        let lane = (bit / 8) as usize;
        let lane_bit = 1_u8 << (bit % 8);

        let e4 = Fp8E4M3Fnuzx4::from_bits(raw);
        let e5 = Fp8E5M2Fnuzx4::from_bits(raw);
        assert_eq!(e4.to_bits(), raw);
        assert_eq!(e5.to_bits(), raw);
        for index in 0..4 {
            let expected = if index == lane { lane_bit } else { 0 };
            assert_eq!(e4.to_array()[index].to_bits(), expected);
            assert_eq!(e5.to_array()[index].to_bits(), expected);
        }
    }
}

#[test]
fn every_e4m3_fnuz_encoding_round_trips() {
    for raw in u8::MIN..=u8::MAX {
        let value = Fp8E4M3Fnuz::from_bits(raw);
        assert_eq!(value.to_bits(), raw);
        assert_eq!(Fp8E4M3Fnuz::from_f32(value.to_f32()).to_bits(), raw);
        assert_eq!(value.is_nan(), raw == 0x80);
        assert_eq!(value.is_finite(), raw != 0x80);
        assert!(!value.is_infinite());
    }
}

#[test]
fn every_e5m2_fnuz_encoding_round_trips() {
    for raw in u8::MIN..=u8::MAX {
        let value = Fp8E5M2Fnuz::from_bits(raw);
        assert_eq!(value.to_bits(), raw);
        assert_eq!(Fp8E5M2Fnuz::from_f32(value.to_f32()).to_bits(), raw);
        assert_eq!(value.is_nan(), raw == 0x80);
        assert_eq!(value.is_finite(), raw != 0x80);
        assert!(!value.is_infinite());
    }
}

#[test]
fn exhaustive_decode_tables_match_rocm_7_2_4_reference() {
    assert_eq!(
        rocm_decode_fingerprint(Fp8E4M3Fnuz::from_bits, |value| { value.to_f32().to_bits() }),
        0x10b5_2e16_89fa_0f98
    );
    assert_eq!(
        rocm_decode_fingerprint(Fp8E5M2Fnuz::from_bits, |value| { value.to_f32().to_bits() }),
        0x5bcf_a8ed_cce2_bf98
    );
}

#[test]
fn boundary_encode_corpus_matches_rocm_7_2_4_satfinite_reference() {
    assert_eq!(
        rocm_boundary_encode_fingerprint(
            |raw| Fp8E4M3Fnuz::from_bits(raw).to_f32(),
            Fp8E4M3Fnuz::from_f32,
            Fp8E4M3Fnuz::to_bits,
        ),
        0xd062_82ff_a323_b04f
    );
    assert_eq!(
        rocm_boundary_encode_fingerprint(
            |raw| Fp8E5M2Fnuz::from_bits(raw).to_f32(),
            Fp8E5M2Fnuz::from_f32,
            Fp8E5M2Fnuz::to_bits,
        ),
        0xd062_82ff_a323_b04f
    );
}

#[test]
fn every_positive_and_negative_rounding_boundary_is_rne() {
    assert_all_rounding_boundaries(
        |raw| Fp8E4M3Fnuz::from_bits(raw).to_f32(),
        Fp8E4M3Fnuz::from_f32,
        Fp8E4M3Fnuz::to_bits,
    );
    assert_all_rounding_boundaries(
        |raw| Fp8E5M2Fnuz::from_bits(raw).to_f32(),
        Fp8E5M2Fnuz::from_f32,
        Fp8E5M2Fnuz::to_bits,
    );
}

#[test]
fn nan_infinity_overflow_underflow_and_negative_zero_are_canonical() {
    for input in [
        f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0xffc0_1234),
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_eq!(Fp8E4M3Fnuz::from_f32(input).to_bits(), 0x80);
        assert_eq!(Fp8E5M2Fnuz::from_f32(input).to_bits(), 0x80);
    }

    assert_eq!(Fp8E4M3Fnuz::from_f32(241.0), Fp8E4M3Fnuz::MAX);
    assert_eq!(Fp8E4M3Fnuz::from_f32(-241.0), Fp8E4M3Fnuz::MIN);
    assert_eq!(Fp8E5M2Fnuz::from_f32(60_000.0), Fp8E5M2Fnuz::MAX);
    assert_eq!(Fp8E5M2Fnuz::from_f32(-60_000.0), Fp8E5M2Fnuz::MIN);
    assert_eq!(Fp8E4M3Fnuz::from_f32(2.0_f32.powi(-11)).to_bits(), 0);
    assert_eq!(Fp8E5M2Fnuz::from_f32(2.0_f32.powi(-18)).to_bits(), 0);
    assert_eq!(Fp8E4M3Fnuz::from_f32(-0.0).to_bits(), 0);
    assert_eq!(Fp8E5M2Fnuz::from_f32(-0.0).to_bits(), 0);
}

#[test]
fn classification_and_sign_mutations_preserve_fnuz_special_values() {
    for raw in 1_u8..8 {
        assert!(Fp8E4M3Fnuz::from_bits(raw).is_subnormal());
        assert!(Fp8E4M3Fnuz::from_bits(raw | 0x80).is_subnormal());
    }
    for raw in 1_u8..4 {
        assert!(Fp8E5M2Fnuz::from_bits(raw).is_subnormal());
        assert!(Fp8E5M2Fnuz::from_bits(raw | 0x80).is_subnormal());
    }

    assert_eq!((-Fp8E4M3Fnuz::ZERO).to_bits(), 0);
    assert_eq!((-Fp8E4M3Fnuz::NAN).to_bits(), 0x80);
    assert_eq!(Fp8E4M3Fnuz::NAN.abs().to_bits(), 0x80);
    assert!(Fp8E4M3Fnuz::NAN.is_sign_negative());
    assert_eq!(Fp8E4M3Fnuz::NAN.to_f32().to_bits(), 0xffc0_0000);
    assert!(Fp8E4M3Fnuz::NAN.to_f32().is_sign_negative());
    assert_eq!((-Fp8E5M2Fnuz::ZERO).to_bits(), 0);
    assert_eq!((-Fp8E5M2Fnuz::NAN).to_bits(), 0x80);
    assert_eq!(Fp8E5M2Fnuz::NAN.abs().to_bits(), 0x80);
    assert!(Fp8E5M2Fnuz::NAN.is_sign_negative());
    assert_eq!(Fp8E5M2Fnuz::NAN.to_f32().to_bits(), 0xffc0_0000);
    assert!(Fp8E5M2Fnuz::NAN.to_f32().is_sign_negative());
}
