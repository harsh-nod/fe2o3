#![forbid(unsafe_code)]

use fe2o3_fill::fill_reference;

const RESULT_BITS: u32 = 0x422a_0000;

#[test]
fn point_reference_overwrites_every_initial_value() {
    for point in [0, 1, 63, 64, 65, usize::MAX] {
        for initial_bits in [
            0x0000_0000,
            0x8000_0000,
            0x7f7f_ffff,
            0xff7f_ffff,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0123,
            0xffc0_0456,
        ] {
            let mut value = f32::from_bits(initial_bits);
            fill_reference(point, &mut value);
            assert_eq!(value.to_bits(), RESULT_BITS);
        }
    }
}

#[test]
fn point_reference_iteration_preserves_elements_outside_the_output() {
    const CANARY_BITS: u32 = 0xc600_0200;
    for length in [0, 1, 63, 64, 65, 127, 128, 129] {
        let mut backing = [f32::from_bits(CANARY_BITS); 131];
        for (point, value) in backing[1..length + 1].iter_mut().enumerate() {
            fill_reference(point, value);
        }
        for (index, value) in backing.iter().enumerate() {
            let expected = if (1..length + 1).contains(&index) {
                RESULT_BITS
            } else {
                CANARY_BITS
            };
            assert_eq!(value.to_bits(), expected, "length={length}, index={index}");
        }
    }
}
