use super::*;

#[test]
fn switch_keys_v3_threshold_preserves_original_order_and_high_bits() {
    for count in [0, 1, 15, 16, 17, 18, 255, 256, 257] {
        let keys = (0..count)
            .rev()
            .map(|n| (n as u64) << 55 | 17)
            .collect::<Vec<_>>();
        let original = keys.clone();
        validate_switch_case_keys_v3(64, true, false, SwitchKeyKindAttrV3::LegacyU64, &keys)
            .unwrap();
        assert_eq!(keys, original);
        if count >= 2 {
            let mut duplicate = keys;
            duplicate[count - 1] = duplicate[0];
            assert_eq!(
                validate_switch_case_keys_v3(
                    64,
                    false,
                    false,
                    SwitchKeyKindAttrV3::LegacyU64,
                    &duplicate
                ),
                Err(SwitchErrorV3::Keys)
            );
        }
    }
}

#[test]
fn switch_keys_v3_threshold_resource_terms_include_every_radix_owner_and_pass() {
    let small = switch_key_validation_resources_v3(SwitchKeyKindAttrV3::LegacyU64, 16).unwrap();
    assert_eq!(small.work_upper_bound(), 184);
    assert_eq!(small.scratch_storage_upper_bound(), 0);
    let large = switch_key_validation_resources_v3(SwitchKeyKindAttrV3::LegacyU64, 17).unwrap();
    assert_eq!(large.work_upper_bound(), 4621);
    let words = |bytes: usize| bytes.div_ceil(std::mem::size_of::<usize>());
    assert_eq!(
        large.scratch_storage_upper_bound(),
        2 * 17 * words(8) + 256 + 2 * words(std::mem::size_of::<Vec<u64>>()) + 16
    );
    let typed = switch_key_validation_resources_v3(SwitchKeyKindAttrV3::U64, 17).unwrap();
    assert_eq!(typed.work_upper_bound(), 66);
    assert_eq!(typed.scratch_storage_upper_bound(), 0);
}

#[test]
fn switch_keys_v3_type_shape_cannot_authorize_an_invalid_key_domain() {
    for (width, signed, index, kind, keys) in [
        (1, false, false, SwitchKeyKindAttrV3::EmptyTyped, vec![]),
        (32, false, true, SwitchKeyKindAttrV3::Index, vec![0]),
        (64, true, true, SwitchKeyKindAttrV3::Index, vec![0]),
        (128, false, false, SwitchKeyKindAttrV3::U64, vec![0]),
        (8, false, false, SwitchKeyKindAttrV3::LegacyU64, vec![256]),
    ] {
        assert!(validate_switch_case_keys_v3(width, signed, index, kind, &keys).is_err());
    }
    assert_eq!(
        switch_key_validation_resources_v3(SwitchKeyKindAttrV3::LegacyU64, MAX_SWITCH_CASES_V3 + 1),
        Err(SwitchErrorV3::Limit)
    );
}
