use super::*;

#[test]
fn final_state_checks_every_word_and_all_three_owners() {
    for owners in [0b01_01_01, 0b10_10_10, 0b01_10_01] {
        terminal([1, 0, 7, 7, owners, 0]).unwrap();
    }
    let good = [1, 0, 7, 7, 0b01_10_01, 0];
    for i in 0..6 {
        let mut bad = good;
        bad[i] = match i {
            0 => 2,
            1 => 1,
            2 | 3 => 3,
            4 => 0,
            5 => 1,
            _ => unreachable!(),
        };
        assert!(terminal(bad).is_err());
    }
    for owner in 0..3 {
        for invalid in [0, 3] {
            let mut bad = good;
            bad[4] = (bad[4] & !(3 << (owner * 2))) | (invalid << (owner * 2));
            assert!(terminal(bad).is_err());
        }
    }
    let mut bad = good;
    bad[4] |= 1 << 6;
    assert!(terminal(bad).is_err());
}

#[test]
fn exact_image_and_deadline_refuse_before_native_setup() {
    for timeout in [0, 1, 10_000, 10_001, u32::MAX] {
        assert!(validate_image(&vec![0; OBJECT_BYTES], timeout).is_err());
    }
    for len in [0, OBJECT_BYTES - 1, OBJECT_BYTES + 1] {
        assert!(validate_image(&vec![0; len], 100).is_err());
    }
}

#[test]
fn logical_readback_lengths_and_all_ieee_bits_are_preserved() {
    let words = [0, 0x8000_0000, 0x3f80_0000, 0x7fc0_0001];
    let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    assert_eq!(decode::<4>(&bytes).unwrap(), words);
    assert!(decode::<3>(&bytes).is_err());
    assert!(decode::<4>(&bytes[..15]).is_err());
    assert_eq!(geometry().unwrap().grid(), [256, 1, 1]);
    assert_eq!(geometry().unwrap().workgroup(), [128, 1, 1]);
}

#[test]
#[ignore = "requires the exact retained HSACO; CPU-only, no device open"]
fn retained_fixed_image_passes_same_engine_intake() {
    let path = std::env::var("FE2O3_FIXED_SHARED_FINITE_JOIN_HSACO").expect("exact image path");
    let bytes = std::fs::read(path).unwrap();
    validate_image(&bytes, 10_000).unwrap();
    assert!(validate_image(&bytes, 0).is_err());
    assert!(validate_image(&bytes, 10_001).is_err());
    let mut bad = bytes;
    bad[100] ^= 1;
    assert!(validate_image(&bad, 100).is_err());
}
