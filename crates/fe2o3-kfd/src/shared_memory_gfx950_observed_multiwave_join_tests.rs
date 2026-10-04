use super::*;

#[test]
fn multiwave_terminal_requires_all_nine_words_and_three_exact_arrivals() {
    for owners in [0b01_01_01, 0b10_10_10, 0b01_10_01] {
        terminal([1, 0, 7, 7, owners, 0, 128, 128, 128]).unwrap();
    }
    let good = [1, 0, 7, 7, 0b01_10_01, 0, 128, 128, 128];
    for i in 0..9 {
        let mut bad = good;
        bad[i] ^= 1;
        assert!(terminal(bad).is_err(), "word {i}");
    }
    for task in 0..3 {
        for invalid in [0, 3] {
            let mut bad = good;
            bad[4] = (bad[4] & !(3 << (task * 2))) | (invalid << (task * 2));
            assert!(terminal(bad).is_err());
        }
        for invalid in [0, 64, 127, 129, u32::MAX] {
            let mut bad = good;
            bad[6 + task] = invalid;
            assert!(terminal(bad).is_err());
        }
    }
    let mut bad = good;
    bad[4] |= 1 << 6;
    assert!(terminal(bad).is_err());
    assert_eq!(INITIAL, [1, 3, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(EXTENTS, [1024, 1536, 36]);
}

#[test]
fn multiwave_image_digest_extent_and_deadline_refuse_before_native_setup() {
    let bad = [0; 64];
    let sha = <[u8; 32]>::from(Sha256::digest(bad));
    for timeout in [0, 1, 10_000, 10_001, u32::MAX] {
        assert!(validate_image(&bad, sha, timeout).is_err());
    }
    assert!(validate_image(&bad, [0; 32], 100).is_err());
    assert!(validate_image(&bad, [1; 32], 100).is_err());
    assert!(validate_image(&[], sha, 100).is_err());
    assert!(validate_image(&vec![0; MAX_OBJECT_BYTES + 1], sha, 100).is_err());
}

#[test]
fn multiwave_readback_bits_and_exact_geometry_remain_explicit() {
    let words = [0, 0x8000_0000, 0x3f80_0000, 0x7fc0_0001];
    let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    assert_eq!(decode::<4>(&bytes).unwrap(), words);
    assert!(decode::<3>(&bytes).is_err());
    assert!(decode::<4>(&bytes[..15]).is_err());
    assert_eq!(geometry().unwrap().grid(), [256, 1, 1]);
    assert_eq!(geometry().unwrap().workgroup(), [128, 1, 1]);
    assert_eq!(GROUP_BYTES, 1024);
    assert_eq!(SYMBOL, "finite_multiwave_join_source_v1");
}

#[test]
#[ignore = "requires an actual reviewed new-profile image and SHA; CPU-only"]
fn actual_multiwave_image_passes_distinct_same_engine_intake() {
    let path = std::env::var("FE2O3_MULTIWAVE_JOIN_HSACO").expect("actual image path");
    let digest = std::env::var("FE2O3_MULTIWAVE_JOIN_SHA256").expect("actual image SHA256");
    assert_eq!(digest.len(), 64);
    let expected =
        std::array::from_fn(|i| u8::from_str_radix(&digest[i * 2..i * 2 + 2], 16).unwrap());
    let bytes = std::fs::read(path).unwrap();
    validate_image(&bytes, expected, 10_000).unwrap();
    assert!(validate_image(&bytes, expected, 0).is_err());
    let mut bad = bytes;
    bad[0] ^= 1;
    assert!(validate_image(&bad, expected, 100).is_err());
}
