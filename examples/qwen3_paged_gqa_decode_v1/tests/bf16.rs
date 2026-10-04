use fe2o3_qwen3_paged_gqa_decode_v1::{Bf16ConversionErrorV1, Bf16V1};

#[test]
fn signed_exception_and_rounding_boundaries_are_exact() {
    for bits in [
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0001,
        0xffc0_0001,
        0x7f80_0001,
        0xff80_0001,
    ] {
        assert_eq!(
            Bf16V1::from_f32_rne(f32::from_bits(bits)),
            Err(Bf16ConversionErrorV1::NonFiniteInput)
        );
    }
    for bits in [0x7f7f_8000, 0xff7f_8000, 0x7f7f_ffff, 0xff7f_ffff] {
        assert!(f32::from_bits(bits).is_finite());
        assert_eq!(
            Bf16V1::from_f32_rne(f32::from_bits(bits)),
            Err(Bf16ConversionErrorV1::NonFiniteOutput)
        );
    }
    for (bits, expected) in [
        (0x0000_0000, 0x0000),
        (0x8000_0000, 0x8000),
        (0x3f80_8000, 0x3f80),
        (0x3f81_8000, 0x3f82),
        (0xbf80_8000, 0xbf80),
        (0xbf81_8000, 0xbf82),
        (0x0000_8000, 0x0000),
        (0x0000_8001, 0x0001),
        (0x8000_8000, 0x8000),
        (0x8000_8001, 0x8001),
        (0x7f7f_0000, 0x7f7f),
        (0xff7f_0000, 0xff7f),
        (0x7f7f_7fff, 0x7f7f),
        (0xff7f_7fff, 0xff7f),
    ] {
        assert_eq!(
            Bf16V1::from_f32_rne(f32::from_bits(bits))
                .unwrap()
                .to_bits(),
            expected,
            "source bits {bits:08x}"
        );
    }
}
