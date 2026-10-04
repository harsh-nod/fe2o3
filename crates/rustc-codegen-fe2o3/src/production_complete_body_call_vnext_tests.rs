use super::*;

#[test]
fn exact_values_pack_without_count_narrowing_or_padding_loss() {
    let values = [1, 1, 0x4100, 0, 0, 0, 8, 0, 0, 0];
    let packed = packed_from_values(values).unwrap();
    assert_eq!(packed.block_count(), 1);
    assert_eq!(packed.instruction_words(), [8, 0, 0, 0]);
    for slot in 0..2 {
        let mut changed = values;
        changed[slot] = 256;
        assert!(packed_from_values(changed).is_err());
        changed[slot] = u64::MAX;
        assert!(packed_from_values(changed).is_err());
    }
}

#[test]
fn descriptor_counts_reserved_bits_and_all_unused_words_refuse() {
    let values = [1, 1, 0x4100, 0, 0, 0, 8, 0, 0, 0];
    for (slot, value) in [
        (0, 0),
        (0, 9),
        (1, 0),
        (1, 17),
        (2, 0x6100),
        (2, 0x8000_4100),
        (6, 6),
        (6, 0xfc00),
    ] {
        let mut changed = values;
        changed[slot] = value;
        assert!(packed_from_values(changed).is_err());
    }
    for slot in [3, 4, 5, 7, 8, 9] {
        let mut changed = values;
        changed[slot] = 1;
        assert!(packed_from_values(changed).is_err());
    }
}

#[test]
fn roles_are_exact_distinct_literals_outside_all_prologue_reserved_vgprs() {
    let valid = [Some(8), Some(9), Some(10), Some(11), Some(63)];
    assert_eq!(registers_from_literals(valid).unwrap(), [8, 9, 10, 11, 63]);
    for slot in 0..5 {
        for value in (0..8).chain(64..=256) {
            let mut changed = valid;
            changed[slot] = Some(value);
            assert!(registers_from_literals(changed).is_err());
        }
        let mut changed = valid;
        changed[slot] = None;
        assert!(registers_from_literals(changed).is_err());
        changed[slot] = Some(u128::MAX);
        assert!(registers_from_literals(changed).is_err());
    }
}

#[test]
fn every_role_alias_pair_is_refused_before_a_shift_can_overflow() {
    let valid = [Some(32), Some(33), Some(34), Some(35), Some(36)];
    for left in 0..5 {
        for right in left + 1..5 {
            let mut changed = valid;
            changed[right] = changed[left];
            assert!(registers_from_literals(changed).is_err());
        }
    }
}
