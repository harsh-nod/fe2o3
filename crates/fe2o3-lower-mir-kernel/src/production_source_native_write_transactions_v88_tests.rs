use super::*;

const LIMIT: usize = 1_000_000_000;

fn run(
    count: u32,
    receiver: usize,
    mode: u8,
    work: usize,
    storage: usize,
    reached: &std::cell::Cell<bool>,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    let retained_floor = std::cell::Cell::new(None);
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_source_v87(
        write_calls_v86::write_owner_count_v87(true, true, receiver, count),
        fe2o3_kernel_descriptor::AccessMode::WriteOnly,
        work,
        storage,
        &retained_floor,
        |source, budget| {
            let floor = budget.storage();
            let result = source.with_checked_mixed_fixedpoint_optimization_v18(
                budget,
                |original, optimized, budget| {
                    slice_view_v1::test_source_native_writes_v88(
                        original,
                        optimized,
                        count as usize,
                        receiver,
                        mode,
                        reached,
                        budget,
                    )?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            );
            if matches!(mode, 5 | 6) {
                assert!(result.is_err());
                assert!(budget.storage() > floor);
                retained_floor.set(Some(budget.storage()));
            }
            let (output, (), receipt) = result?;
            assert_eq!(
                receipt.retained_storage(),
                size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
            );
            drop(output);
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    )
}

#[test]
fn source_native_writes_v88_join_actual_calls_both_roots_and_commoned_metadata() {
    for count in [1, 2, 4] {
        for receiver in [0, 1] {
            let reached = std::cell::Cell::new(false);
            run(count, receiver, 0, LIMIT, LIMIT, &reached).0.unwrap();
            assert!(reached.get());
        }
    }
}

#[test]
fn source_native_writes_v88_refuse_legacy_profile_and_wrong_original_ordinal() {
    for mode in [1, 2] {
        let reached = std::cell::Cell::new(false);
        assert!(run(2, 0, mode, LIMIT, LIMIT, &reached).0.is_err());
        assert!(
            reached.get(),
            "control {mode} reached authentic native scope"
        );
    }
}

#[test]
fn source_native_writes_v88_drop_error_and_unwind_captures_before_refund() {
    for mode in [3, 4] {
        let reached = std::cell::Cell::new(false);
        assert!(run(2, 0, mode, LIMIT, LIMIT, &reached).0.is_err());
        assert!(
            reached.get(),
            "control {mode} entered authentic source/native callback"
        );
    }
}

#[test]
fn source_native_writes_v88_refuse_copied_operand_predicate_and_coordinate_substitutions() {
    for mode in 16..=23 {
        let reached = std::cell::Cell::new(false);
        assert!(run(2, 0, mode, LIMIT, LIMIT, &reached).0.is_err());
        assert!(reached.get(), "mutation {mode} reached authentic endpoint");
    }
}

#[test]
fn source_native_writes_v88_refuse_foreign_ledger_and_undercut_without_refunding() {
    for mode in [5, 6] {
        let reached = std::cell::Cell::new(false);
        assert!(run(2, 0, mode, LIMIT, LIMIT, &reached).0.is_err());
        assert!(
            reached.get(),
            "custody control {mode} reached authentic endpoint"
        );
    }
}

#[test]
fn source_native_writes_v88_whole_transaction_exact_and_one_short_resources() {
    let reached = std::cell::Cell::new(false);
    let (result, work, storage) = run(2, 0, 0, LIMIT, LIMIT, &reached);
    result.unwrap();
    assert!(reached.get());
    let reached = std::cell::Cell::new(false);
    let (result, exact_work, exact_storage) = run(2, 0, 0, work, storage, &reached);
    result.unwrap();
    assert!(reached.get());
    assert_eq!((exact_work, exact_storage), (work, storage));
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        assert!(
            run(2, 0, 0, work, storage, &std::cell::Cell::new(false))
                .0
                .is_err()
        );
    }
}
