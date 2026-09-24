use super::*;
#[test]
fn one_stop_output_init_and_all_sixty_four_completed_words_have_distinct_canaries() {
    let mut b = [0; PAGE];
    initialize_output(&mut b).unwrap();
    check_output(&b, false).unwrap();
    assert!(check_output(&b, true).is_err());
    for lane in 0..64_u32 {
        let p = 8 + lane as usize * 4;
        b[p..p + 4].copy_from_slice(&(0x1357_9bdf ^ lane).to_le_bytes());
    }
    check_output(&b, true).unwrap();
    assert!(check_output(&b, false).is_err());
}
#[test]
fn every_byte_of_output_canary_payload_and_unused_backing_is_checked() {
    let mut b = [0; PAGE];
    initialize_output(&mut b).unwrap();
    for i in 0..PAGE {
        b[i] ^= 1;
        assert!(check_output(&b, false).is_err(), "byte {i}");
        b[i] ^= 1;
    }
    assert!(check_output(&b[..272], false).is_err());
}
#[test]
fn signal_base_and_atomic_value_are_distinct_and_exactly_bounded() {
    assert_eq!(signal_addresses(4096, 4096).unwrap(), (4096, 4104));
    for (b, n) in [(0, 4096), (4097, 4096), (4096, 272), (u64::MAX - 63, 4096)] {
        assert!(signal_addresses(b, n).is_err());
    }
}
#[test]
fn exact_kernarg_initializer_expected_values_and_every_byte_refusal() {
    let mut b = [0; KERNARG_BYTES];
    b[..8].copy_from_slice(&0x1008_u64.to_le_bytes());
    for o in [8, 12, 16] {
        b[o..o + 4].copy_from_slice(&1_u32.to_le_bytes());
    }
    for (o, v) in [(20, 64_u16), (22, 1), (24, 1), (72, 1)] {
        b[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    check_kernarg(&b, 0x1008).unwrap();
    for i in 0..b.len() {
        b[i] ^= 1;
        assert!(check_kernarg(&b, 0x1008).is_err(), "byte{i}");
        b[i] ^= 1;
    }
    assert!(check_kernarg(&b, 0x1010).is_err());
}
#[test]
fn fixed_native_entry_has_store_wait_trap_and_end_not_an_invented_stop_pc() {
    assert_eq!(ENTRY.len(), 84);
    assert_eq!(&ENTRY[64..72], &[0, 128, 112, 220, 4, 8, 127, 0]);
    assert_eq!(
        &ENTRY[72..],
        &[0, 0, 140, 191, 3, 0, 146, 191, 0, 0, 129, 191]
    );
    assert_eq!(
        u32::from_le_bytes(DESCRIPTOR[52..56].try_into().unwrap()),
        132
    );
    assert_eq!(
        u32::from_le_bytes(DESCRIPTOR[8..12].try_into().unwrap()),
        264
    );
}
#[test]
fn arbitrary_bytes_cannot_select_the_fixed_profile() {
    for b in [&[][..], &[0; 16][..], &[0; OBJECT_BYTES][..]] {
        assert!(validate_object(b).is_err());
    }
}
/// Root-only CPU selected-artifact gate; performs no allocation/queue/native work.
/// The file remains inert input, never source or execution custody.
#[test]
#[ignore = "requires root-pinned fixed qualified artifact"]
fn actual_one_stop_fixed_artifact_contract() {
    use std::io::Read;
    let path =
        std::env::var_os("FE2O3_ONE_STOP_FIXED_ARTIFACT_TEST").expect("explicit root test input");
    let mut b = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take((OBJECT_BYTES + 1) as u64)
        .read_to_end(&mut b)
        .unwrap();
    let c = validate_object(&b).unwrap();
    assert_eq!(c.entry_bytes(), ENTRY);
    drop(c);
    for i in 0..b.len() {
        b[i] ^= 1;
        assert!(validate_object(&b).is_err(), "changed object byte{i}");
        b[i] ^= 1;
    }
}
