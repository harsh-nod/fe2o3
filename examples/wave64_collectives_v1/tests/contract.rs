use fe2o3_wave64_collectives_v1::{
    EMPTY_MASK_POLICY_V1, INACTIVE_LANE_OUTPUT_POLICY_V1, MAX_EXACT_INPUT_MAGNITUDE_V1,
    PHYSICAL_EXECUTION_POLICY_V1, WAVE64_LANES_V1, lane_is_active_v1, lane_outputs_v1,
};

#[test]
fn exact_wave_width_and_corpus_bound_are_stable() {
    assert_eq!(WAVE64_LANES_V1, 64);
    assert_eq!(MAX_EXACT_INPUT_MAGNITUDE_V1, 1024.0);
    const {
        assert!(64.0 * MAX_EXACT_INPUT_MAGNITUDE_V1 < 16_777_216.0);
    }
}

#[test]
fn explicit_mask_selects_exactly_its_wave64_bits() {
    let mask = (1_u64 << 0) | (1_u64 << 7) | (1_u64 << 31) | (1_u64 << 63);
    for lane in 0..WAVE64_LANES_V1 {
        assert_eq!(
            lane_is_active_v1(mask, lane),
            matches!(lane, 0 | 7 | 31 | 63)
        );
    }
    assert!(!lane_is_active_v1(u64::MAX, 64));
    assert!(!lane_is_active_v1(u64::MAX, usize::MAX));
}

#[test]
fn lane_ownership_is_bounded_and_injective_in_every_output() {
    let accesses: Vec<_> = (0..WAVE64_LANES_V1)
        .map(|lane| lane_outputs_v1(lane).unwrap())
        .collect();
    assert!(lane_outputs_v1(WAVE64_LANES_V1).is_none());

    for access in &accesses {
        assert_eq!(access.lane, access.reduction_index);
        assert_eq!(access.lane, access.inclusive_index);
        assert_eq!(access.lane, access.exclusive_index);
        assert!(access.lane < WAVE64_LANES_V1);
    }
    for left in 0..accesses.len() {
        for right in left + 1..accesses.len() {
            assert_ne!(
                accesses[left].reduction_index,
                accesses[right].reduction_index
            );
            assert_ne!(
                accesses[left].inclusive_index,
                accesses[right].inclusive_index
            );
            assert_ne!(
                accesses[left].exclusive_index,
                accesses[right].exclusive_index
            );
        }
    }
}

#[test]
fn policy_text_distinguishes_logical_masking_from_physical_execution() {
    assert!(EMPTY_MASK_POLICY_V1.contains("accepted"));
    assert!(EMPTY_MASK_POLICY_V1.contains("+0.0"));
    assert!(INACTIVE_LANE_OUTPUT_POLICY_V1.contains("contribute +0.0"));
    assert!(INACTIVE_LANE_OUTPUT_POLICY_V1.contains("publish +0.0"));
    assert!(PHYSICAL_EXECUTION_POLICY_V1.contains("all 64 physical lanes"));
    assert!(PHYSICAL_EXECUTION_POLICY_V1.contains("convergently"));
}
