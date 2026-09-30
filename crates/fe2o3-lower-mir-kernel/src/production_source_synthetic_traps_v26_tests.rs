struct ResetSyntheticTrapModeV26;
impl Drop for ResetSyntheticTrapModeV26 {
    fn drop(&mut self) {
        ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(None);
    }
}

#[test]
fn mixed_synthetic_trap_v26_matches_complete_original_shared_and_rmw_span_census() {
    let _reset = ResetSyntheticTrapModeV26;
    for exclusive in [false, true] {
        ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(Some((0, 0)));
        let (result, _, _, reached) =
            run_mixed_handoff_v26(exclusive, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_ok(), "exclusive={exclusive}: {result:?}");
        assert!(reached);
        let (_, count) =
            ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(None)
                .unwrap();
        let expected = MIXED_FIXTURE_SOURCE_TRAPS_V26.get();
        assert_eq!(
            count, expected,
            "exclusive={exclusive}: complete original synthetic census"
        );
        if !exclusive {
            assert!(
                expected > 0,
                "shared positive must exercise a genuine assertion sink"
            );
        }
    }
}

#[test]
fn mixed_synthetic_trap_v26_refuses_omission_foreign_source_and_duplicate_ownership() {
    let _reset = ResetSyntheticTrapModeV26;
    for (fault, diagnostic) in [
        (0, ""),
        (1, "Unresolved"),
        (2, "mixed synthetic trap source instance or span differs"),
        (3, "mixed synthetic trap has duplicate source spans"),
    ] {
        ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(Some((
            fault, 0,
        )));
        let (result, _, _, reached) =
            run_mixed_handoff_v26(false, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        if fault == 0 {
            assert!(result.is_ok(), "authentic positive baseline: {result:?}");
            assert!(reached);
        } else {
            assert!(result.is_err(), "fault={fault}");
            assert!(!reached, "fault={fault} must refuse before the consumer");
            let actual = format!("{result:?}");
            assert!(actual.contains(diagnostic), "fault={fault}: {actual}");
            if fault == 1 {
                assert!(actual.contains("requirement: Call"));
            }
        }
    }
}

#[test]
fn mixed_synthetic_trap_v26_has_exact_and_one_short_transaction_accounting() {
    let _reset = ResetSyntheticTrapModeV26;
    ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(Some((0, 0)));
    let (result, work, storage, reached) =
        run_mixed_handoff_v26(false, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    assert!(result.is_ok(), "{result:?}");
    assert!(reached);
    let (_, count) =
        ProductionMixedMemoryCheckedNativePoliciesV26::synthetic_trap_test_mode_v26(None).unwrap();
    assert!(count > 0);
    let (exact, used, peak, reached) = run_mixed_handoff_v26(false, 0, work, storage);
    assert!(exact.is_ok(), "{exact:?}");
    assert!(reached);
    assert_eq!((used, peak), (work, storage));
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        let (short, _, _, _) = run_mixed_handoff_v26(false, 0, work, storage);
        assert!(short.is_err());
    }
}
