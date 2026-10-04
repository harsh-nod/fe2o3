use super::*;

#[test]
fn fixed_const_values_preserve_exact_count_packing_and_dead_steps() {
    let one = program_from_values([1, 8, 0, 0, 0]).unwrap();
    assert_eq!(one.count(), 1);
    assert_eq!(one.packed_words(), [8, 0, 0, 0]);
    let three = program_from_values([3, 1_773_841_612_933, 0, 0, 0]).unwrap();
    assert_eq!(&three.descriptors()[..3], &[133, 307, 413]);
    let sixteen = program_from_values([
        16,
        52_918_682_905_280_512,
        50_947_245_666_337_089,
        54_606_459_036_172_348,
        20_266_267_064_467_512,
    ])
    .unwrap();
    assert_eq!(sixteen.count(), 16);
    assert_eq!(sixteen.descriptors()[14], 16); // retained dead scratch move
    assert_eq!(sixteen.descriptors()[15], 72); // retained output self-move
}

#[test]
fn malformed_const_values_fail_without_repair_or_truncation() {
    for count in [0, 17, 255, 256, u64::MAX] {
        assert!(program_from_values([count, 8, 0, 0, 0]).is_err());
    }
    for word in [14, 15, 88, 120, 136, 1032, 56, 72, 0] {
        assert!(program_from_values([1, word, 0, 0, 0]).is_err(), "{word}");
    }
    for word_slot in 1..4 {
        let mut values = [1, 8, 0, 0, 0];
        values[word_slot + 1] = 1;
        assert!(program_from_values(values).is_err());
    }
    assert!(program_from_values([1, 8 | (8 << 16), 0, 0, 0]).is_err());
}

#[test]
fn physical_roles_require_five_actual_distinct_in_range_literals() {
    let registers =
        registers_from_literals([Some(32), Some(33), Some(34), Some(35), Some(36)]).unwrap();
    assert_eq!(registers.inputs(), [34, 35, 36]);
    assert_eq!(registers.vgpr_high_water(), 37);
    for slot in 0..5 {
        for bad in [None, Some(64), Some(255), Some(256), Some(u128::MAX)] {
            let mut values = [Some(0), Some(1), Some(2), Some(3), Some(63)];
            values[slot] = bad;
            assert!(registers_from_literals(values).is_err());
        }
        for other in 0..slot {
            let mut values = [Some(0), Some(1), Some(2), Some(3), Some(63)];
            values[slot] = values[other];
            assert!(registers_from_literals(values).is_err());
        }
    }
}
