use core::mem::{align_of, size_of};

use fe2o3_amd_target::{
    ADVANCED_CAPABILITY_MODEL_REVISION, AdvancedCapabilityStatus, AmdTargetId, Fp8Format,
    MfmaFamily, MxFormat,
};
use fe2o3_device::{MxScaleConversionError, MxScaleE8M0, MxScaleE8M0x4};

#[test]
fn e8m0_layout_constants_and_boundaries_are_exact() {
    assert_eq!(size_of::<MxScaleE8M0>(), 1);
    assert_eq!(align_of::<MxScaleE8M0>(), 1);
    assert_eq!(size_of::<MxScaleE8M0x4>(), 4);
    assert_eq!(align_of::<MxScaleE8M0x4>(), 4);
    assert_eq!(MxScaleE8M0::__fe2o3_rust_layout_v1(), (1, 1));

    assert_eq!(MxScaleE8M0::MIN.to_f32().to_bits(), 0x0040_0000);
    assert_eq!(MxScaleE8M0::MIN.exponent(), Some(-127));
    assert_eq!(MxScaleE8M0::ONE.to_f32(), 1.0);
    assert_eq!(MxScaleE8M0::ONE.exponent(), Some(0));
    assert_eq!(MxScaleE8M0::MAX.to_f32(), 2.0_f32.powi(127));
    assert_eq!(MxScaleE8M0::MAX.exponent(), Some(127));
    assert!(MxScaleE8M0::NAN.to_f32().is_nan());
    assert_eq!(MxScaleE8M0::NAN.exponent(), None);
}

#[test]
fn every_finite_e8m0_encoding_round_trips_exactly() {
    for raw in 0_u8..=0xfe {
        let scale = MxScaleE8M0::from_bits(raw);
        assert!(!scale.is_nan());
        assert_eq!(MxScaleE8M0::try_from_f32(scale.to_f32()), Ok(scale));
        assert_eq!(scale.exponent(), Some(raw as i16 - 127));
    }
    assert!(MxScaleE8M0::from_bits(0xff).is_nan());
}

#[test]
fn lossy_or_special_scale_conversions_are_rejected() {
    for (input, expected) in [
        (0.0, MxScaleConversionError::Zero),
        (-0.0, MxScaleConversionError::Negative),
        (-1.0, MxScaleConversionError::Negative),
        (1.5, MxScaleConversionError::NotPowerOfTwo),
        (f32::from_bits(1), MxScaleConversionError::NotPowerOfTwo),
        (f32::INFINITY, MxScaleConversionError::Infinite),
        (f32::NEG_INFINITY, MxScaleConversionError::Infinite),
        (f32::NAN, MxScaleConversionError::Nan),
    ] {
        assert_eq!(MxScaleE8M0::try_from_f32(input), Err(expected));
    }
}

#[test]
fn packed_scales_preserve_bit_order() {
    let scales = [
        MxScaleE8M0::MIN,
        MxScaleE8M0::ONE,
        MxScaleE8M0::MAX,
        MxScaleE8M0::NAN,
    ];
    let packed = MxScaleE8M0x4::from_array(scales);
    assert_eq!(packed.to_bits(), 0xfffe_7f00);
    assert_eq!(packed.to_array(), scales);
    for (index, scale) in scales.into_iter().enumerate() {
        assert_eq!(packed.lane(index), Some(scale));
    }
    assert_eq!(packed.lane(4), None);
    assert_eq!(MxScaleE8M0x4::default(), MxScaleE8M0x4::NAN);
}

#[test]
fn gfx942_admits_only_reviewed_fnuz_formats() {
    for target_text in ["gfx942", "gfx942:xnack-", "gfx942:sramecc+:xnack-"] {
        let target = AmdTargetId::parse(target_text).unwrap();
        let capabilities = target.capabilities().unwrap();
        assert_eq!(capabilities.advanced_model_identity().target(), target);
        assert_eq!(
            capabilities.advanced_model_identity().revision(),
            ADVANCED_CAPABILITY_MODEL_REVISION
        );
        for format in [Fp8Format::E4M3Fnuz, Fp8Format::E5M2Fnuz] {
            assert_eq!(
                capabilities.fp8_format_support(format),
                AdvancedCapabilityStatus::Supported
            );
        }
        for family in [MfmaFamily::F32FromFp8Fnuz, MfmaFamily::F32FromBf8Fnuz] {
            assert_eq!(
                capabilities.mfma_family_support(family),
                AdvancedCapabilityStatus::Supported
            );
        }
    }
}

#[test]
fn numeric_admission_is_target_exact_for_mx_and_fnuz() {
    let gfx942 = AmdTargetId::parse("gfx942:xnack-").unwrap();
    let capabilities = gfx942.capabilities().unwrap();
    for format in [MxFormat::Fp8, MxFormat::Bf8, MxFormat::Fp4] {
        assert_eq!(
            capabilities.mx_format_support(format),
            AdvancedCapabilityStatus::Unsupported
        );
    }

    for target_text in ["gfx90a", "gfx1100"] {
        let target = AmdTargetId::parse(target_text).unwrap();
        let capabilities = target.capabilities().unwrap();
        for format in [Fp8Format::E4M3Fnuz, Fp8Format::E5M2Fnuz] {
            assert_ne!(
                capabilities.fp8_format_support(format),
                AdvancedCapabilityStatus::Supported
            );
        }
        for format in [MxFormat::Fp8, MxFormat::Bf8, MxFormat::Fp4] {
            assert_eq!(
                capabilities.mx_format_support(format),
                AdvancedCapabilityStatus::Unreviewed
            );
        }
    }

    let gfx950 = AmdTargetId::parse("gfx950").unwrap();
    let capabilities = gfx950.capabilities().unwrap();
    for format in [MxFormat::Fp8, MxFormat::Bf8, MxFormat::Fp4] {
        assert_eq!(
            capabilities.mx_format_support(format),
            AdvancedCapabilityStatus::Supported
        );
    }
}
