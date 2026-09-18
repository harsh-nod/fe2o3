//! Genuine retained source fixtures; no fabricated native or signed owner.
use super::*;
use crate::{
    OriginalNativeFormalMemoryErrorV1 as NError, analyze_original_unit_local_formal_memory_v1,
};
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason as Reason,
    FormalMemoryObligationAnalysis as Analysis, derive_kernel_memory_obligations_for_launch,
};

#[test]
fn original_n_indexed_discharge_handles_repeated_calls_and_no_call_roots() {
    for calls in [&[2, 2][..], &[0, 2][..]] {
        let source = produced_erased_owner(UnitCase::Initializer, calls);
        let floor = FLOOR + source.retained_storage_floor_v1();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (reports, receipt) =
            analyze_original_unit_local_formal_memory_v1(&source, &mut budget).unwrap();
        assert_eq!(reports.kernels().len(), calls.len());
        assert!(std::ptr::eq(
            reports.original(),
            source.original_source().executable()
        ));
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(reports);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn original_n_unit_calls_keep_all_original_report_coordinates_and_raw_effects() {
    for expected in [false, true] {
        for roots in [1, 2] {
            for mutation in [false, true] {
                let input = if mutation {
                    fixture6_with_live_integer_identity(expected, roots)
                } else {
                    continue6(fixture5(expected, roots, None))
                };
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(input.floor).unwrap();
                let original = input.source.original_source().executable();
                let (reports, storage) =
                    analyze_original_unit_local_formal_memory_v1(&input.source, &mut budget)
                        .unwrap();
                assert_eq!(budget.storage(), input.floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(std::ptr::eq(reports.original(), original));
                assert!(!reports.grants_artifact_or_launch_authority());
                assert_eq!(reports.kernels().len(), roots);
                assert_ne!(
                    original.canonical().canonical_bytes(),
                    input.source.erased().canonical().canonical_bytes()
                );
                for (kernel, report) in original.module().kernels.iter().zip(reports.kernels()) {
                    let Analysis::Incomplete { partial, reasons } =
                        derive_kernel_memory_obligations_for_launch(
                            original.module(),
                            &kernel.id,
                            ExplicitLaunchExtent::Exact {
                                rank: kernel.domain.rank(),
                                extents: crate::production_formal_memory_v1::witness_extents(
                                    &kernel.domain,
                                ),
                            },
                            FormalIndexWidth::Bits64,
                        )
                        .unwrap()
                    else {
                        panic!("raw original-N private call must remain impure")
                    };
                    assert!(!reasons.is_empty());
                    assert!(
                        reasons
                            .iter()
                            .all(|reason| matches!(reason, Reason::CallEffectsUnavailable { .. }))
                    );
                    assert_eq!(report, &partial);
                    assert!(!report.accesses().is_empty());
                    let body = original
                        .module()
                        .function(&kernel.entry)
                        .unwrap()
                        .body
                        .as_ref()
                        .unwrap();
                    for access in report.accesses() {
                        let location = access.location();
                        let block = body
                            .blocks
                            .iter()
                            .find(|block| block.id == location.block)
                            .unwrap();
                        assert!(matches!(
                            &block.operations[location.operation_index].kind,
                            OperationKind::Load { .. } | OperationKind::Store { .. }
                        ));
                    }
                }
                drop(reports);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), input.floor);
                // The analysis did not consume or relabel the original source.
                input.source.verify_equivalence(&mut budget).unwrap();
                assert_eq!(budget.storage(), input.floor);
            }
        }
    }
}

#[test]
fn original_n_unit_calls_require_exact_source_owner_root_site_and_callee() {
    let input = continue6(fixture5(true, 2, None));
    let foreign = continue6(fixture5(true, 2, None));
    assert_eq!(
        input
            .source
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        foreign
            .source
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes()
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(input.floor).unwrap();
    crate::production_semantic_kir_v1::original_native_formal_v1::exercise_call_joins_v1(
        &input.source,
        foreign.source.original_source().executable(),
        &mut budget,
    );
    assert_eq!(budget.storage(), input.floor);
}

#[test]
fn original_n_unit_calls_replay_source_before_discharge_and_refuse_hostile_custody() {
    for mutation in 0..4 {
        let mut input = continue6(fixture5(true, 2, None));
        match mutation {
            0 => {
                input.source.operations[0] =
                    Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
            }
            1 => {
                input.source.roots[0].access_sources[0] =
                    ProductionRankedAccessSourceV1::new(1, Some(99), 0, 0, 5)
            }
            2 => input.source.original.launch_roots[0].global_extents[0] += 1,
            3 => input.source.roots.swap(0, 1),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(input.floor).unwrap();
        assert!(matches!(
            analyze_original_unit_local_formal_memory_v1(&input.source, &mut budget),
            Err(NError::Source(_))
        ));
        assert_eq!(budget.storage(), input.floor);
    }
}

#[test]
fn original_n_unit_calls_exact_and_one_short_budgets_preserve_borrowed_floor() {
    let input = continue6(fixture5(true, 2, None));
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(input.floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = analyze_original_unit_local_formal_memory_v1(&input.source, &mut budget);
        assert_eq!(budget.storage(), input.floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let ok = result.is_ok();
        drop(result);
        (ok, budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(WORK, STORAGE);
    assert!(ok);
    assert!(peak > input.floor);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let floor = input.source.retained_storage_floor_v1() - 1;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        analyze_original_unit_local_formal_memory_v1(&input.source, &mut budget),
        Err(NError::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
        ))
    ));
    assert_eq!(budget.storage(), floor);
}
