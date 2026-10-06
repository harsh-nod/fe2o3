use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1 as Work, Kernel, Signature, StorageLayoutKindV1,
    StorageLayoutLimitsV1, StorageLayoutV1, Terminator, ValueDef,
    VerifiedCanonicalKernelIrModuleV18,
};
use std::cell::Cell;

thread_local! {
    static COMPLETED: Cell<usize> = const { Cell::new(0) };
    static CAPTURE_DROPS: Cell<usize> = const { Cell::new(0) };
}

fn fixture(count: usize, separate: bool, external: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    for ordinal in 0..count {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(u32::from(separate && ordinal % 2 != 0)),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    if external {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "external".into(),
                arguments: vec![],
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut module = Module::new("metered_report_phase");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![pointer.clone(), pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    if external {
        module.functions.push(Function::external_import(
            "external",
            Signature::new(vec![], vec![]),
        ));
    }
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn with_original<R>(
    module: &Module,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    consume: impl FnOnce(&CanonicalOwnerFormalAnalysisV18<'_>) -> R,
) -> R {
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 20_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let mut original = CanonicalOwnerFormalScopeV18::new(&owner, Default::default()).unwrap();
    let report = original
        .derive(&KernelId::new("root"), launch, width)
        .unwrap();
    // The independent predecessor still uses the explicitly inert API. The
    // phase tests below do not claim its work or owner backing on their ledger.
    let result = consume(&report);
    drop(report);
    drop(original);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    result
}

fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [2, 1, 1],
    }
}

fn inspect(view: &ReportRowsV18<'_, '_>, _: &mut Budget<'_>) -> ResultV18<()> {
    assert_eq!(
        view.bounds.as_slice(),
        view.original.analysis().obligations().bounds_requirements()
    );
    assert!(view.overflows.iter().all(|location| {
        view.original.analysis().incomplete_reasons().contains(
            &FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                location: *location,
            },
        )
    }));
    assert_eq!(
        view.aliases.as_slice(),
        view.original
            .analysis()
            .obligations()
            .runtime_alias_requirements()
    );
    assert_eq!(
        view.conflicts.as_slice(),
        view.original
            .analysis()
            .obligations()
            .inter_invocation_conflicts()
    );
    COMPLETED.with(|value| value.set(value.get() + 1));
    Ok(())
}

type Consumer = for<'v, 'o, 'b, 'w> fn(&ReportRowsV18<'v, 'o>, &'b mut Budget<'w>) -> ResultV18<()>;

fn independent_headers() -> usize {
    2 * size_of::<std::thread::Result<ResultV18<()>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<ReportRowsV18<'_, '_>>()
        + size_of::<LiveReportMeterV18<'_, '_>>()
        + size_of::<PhaseStateV18>()
        + size_of::<(&CanonicalOwnerFormalAnalysisV18<'_>, &mut Budget<'_>)>()
        + size_of::<Vec<(FormalAllocationIdentity, AllocationEnvelope)>>()
        + size_of::<Vec<RuntimeAliasRequirement>>()
        + size_of::<Vec<InterInvocationConflictRequirement>>()
        + size_of::<Vec<FormalBoundsRequirement>>()
        + size_of::<Vec<FunctionOperationLocation>>()
        + size_of::<Consumer>()
        + size_of::<Option<Consumer>>()
}

#[test]
fn actual_owner_reports_preserve_complete_incomplete_rows_and_order() {
    for count in [0, 1, 2, 3, 8, 44] {
        for separate in [false, true] {
            for external in [false, true] {
                with_original(
                    &fixture(count, separate, external),
                    launch(),
                    FormalIndexWidth::Bits64,
                    |original| {
                        let before = original.analysis().clone();
                        let owner = original.owner();
                        let mut work = Work::new(1_000_000);
                        let mut budget = Budget::new(&mut work, 1_000_000);
                        budget.reserve_storage(37).unwrap();
                        let completed = COMPLETED.with(Cell::get);
                        with_report_rows_v18(original, &mut budget, inspect as Consumer).unwrap();
                        assert_eq!(COMPLETED.with(Cell::get), completed + 1);
                        assert_eq!(budget.storage(), 37);
                        assert!(original.belongs_to(owner));
                        assert_eq!(original.analysis(), &before);
                        assert_eq!(original.analysis().is_complete(), !external);
                    },
                );
            }
        }
    }
}

#[test]
fn exact_independent_single_store_work_and_capacity_bounds() {
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            // Entry4; bounds16+push1+grow2; alias11; merge8; conflict35.
            let work_limit = 4 + 19 + 11 + 8 + 35;
            let peak = independent_headers()
                + 2 * size_of::<FormalBoundsRequirement>()
                + (2 * size_of::<(FormalAllocationIdentity, AllocationEnvelope)>())
                    .max(2 * size_of::<InterInvocationConflictRequirement>());
            for (work_cap, storage_cap, success) in [
                (work_limit, 37 + peak, true),
                (work_limit - 1, 37 + peak, false),
                (work_limit, 36 + peak, false),
            ] {
                let mut work = Work::new(work_cap);
                let mut budget = Budget::new(&mut work, storage_cap);
                budget.reserve_storage(37).unwrap();
                let completed = COMPLETED.with(Cell::get);
                let result = with_report_rows_v18(original, &mut budget, inspect as Consumer);
                assert_eq!(result.is_ok(), success);
                assert_eq!(COMPLETED.with(Cell::get), completed + usize::from(success));
                assert_eq!(budget.storage(), 37);
                if success {
                    assert_eq!(budget.work(), work_limit);
                    assert_eq!(budget.peak_storage(), 37 + peak);
                } else if work_cap < work_limit {
                    assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Work(error))) if error.actual() == work_limit && error.limit() == work_cap)
                    );
                    assert_eq!(budget.failed_work(), Some(work_limit));
                } else {
                    assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Storage { actual, limit })) if actual == 37 + peak && limit == storage_cap)
                    );
                    assert_eq!(budget.failed_storage(), Some(37 + peak));
                }
            }
        },
    );
}

#[test]
fn exact_two_allocation_alias_and_conflict_capacity_overlap() {
    with_original(
        &fixture(2, true, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let expected_work = 4 + 36 + 20 + 8 + 16 + 11 + 100;
            let entries = 2 * size_of::<(FormalAllocationIdentity, AllocationEnvelope)>();
            let aliases = 2 * size_of::<RuntimeAliasRequirement>();
            let conflicts = 2 * size_of::<InterInvocationConflictRequirement>();
            let peak = independent_headers()
                + 2 * size_of::<FormalBoundsRequirement>()
                + aliases
                + entries.max(conflicts);
            let mut work = Work::new(expected_work);
            let mut budget = Budget::new(&mut work, 37 + peak);
            budget.reserve_storage(37).unwrap();
            with_report_rows_v18(original, &mut budget, inspect as Consumer).unwrap();
            assert_eq!(budget.work(), expected_work);
            assert_eq!(budget.peak_storage(), 37 + peak);
            assert_eq!(budget.storage(), 37);
            assert_eq!(
                original
                    .analysis()
                    .obligations()
                    .runtime_alias_requirements()
                    .len(),
                1
            );
            assert_eq!(
                original
                    .analysis()
                    .obligations()
                    .inter_invocation_conflicts()
                    .len(),
                2
            );
        },
    );
}

#[test]
fn repeated_phases_share_work_but_refund_only_destroyed_scratch() {
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            for limit in [153, 154] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 100_000);
                budget.reserve_storage(37).unwrap();
                with_report_rows_v18(original, &mut budget, inspect as Consumer).unwrap();
                let peak = budget.peak_storage();
                let completed = COMPLETED.with(Cell::get);
                let second = with_report_rows_v18(original, &mut budget, inspect as Consumer);
                assert_eq!(second.is_ok(), limit == 154);
                assert_eq!(
                    COMPLETED.with(Cell::get),
                    completed + usize::from(limit == 154)
                );
                assert_eq!(budget.storage(), 37);
                assert_eq!(budget.peak_storage(), peak);
                if limit == 153 {
                    assert_eq!(budget.failed_work(), Some(154));
                    let before = budget.work();
                    assert!(matches!(
                        with_report_rows_v18(original, &mut budget, inspect as Consumer),
                        Err(ReportConstructionErrorV18::PriorDenial {
                            work: Some(154),
                            storage: None
                        })
                    ));
                    assert_eq!(budget.work(), before);
                }
            }
        },
    );
}

#[test]
fn unknown_launch_and_unsupported_width_keep_original_incomplete_report() {
    for (extent, width) in [
        (ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64),
        (launch(), FormalIndexWidth::Bits32),
    ] {
        with_original(&fixture(3, true, true), extent, width, |original| {
            let unchanged = original.analysis().clone();
            assert!(!unchanged.is_complete());
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            with_report_rows_v18(original, &mut budget, inspect as Consumer).unwrap();
            assert_eq!(original.analysis(), &unchanged);
            assert_eq!(budget.storage(), 0);
        });
    }
}

#[test]
fn swallowed_direct_denials_precede_later_rejection_and_restore_floor() {
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            for storage in [false, true] {
                let mut work = Work::new(100_000);
                let mut budget = Budget::new(&mut work, 100_000);
                budget.reserve_storage(37).unwrap();
                let completed = Cell::new(false);
                let result = with_report_rows_v18(original, &mut budget, |_, budget| {
                    let denied = if storage {
                        budget.reserve_storage(100_000)
                    } else {
                        budget.charge_work(100_000)
                    };
                    assert!(denied.is_err());
                    completed.set(true);
                    Err::<(), _>(ReportConstructionErrorV18::Rejected)
                });
                assert!(completed.get());
                assert!(
                    matches!(
                        result,
                        Err(ReportConstructionErrorV18::PriorDenial {
                            work: Some(_),
                            storage: None
                        })
                    ) == !storage
                );
                assert!(
                    matches!(
                        result,
                        Err(ReportConstructionErrorV18::PriorDenial {
                            work: None,
                            storage: Some(_)
                        })
                    ) == storage
                );
                assert_eq!(budget.storage(), 37);
                let before = budget.work();
                assert!(matches!(
                    with_report_rows_v18(original, &mut budget, inspect as Consumer),
                    Err(ReportConstructionErrorV18::PriorDenial { .. })
                ));
                assert_eq!(budget.work(), before);
            }
        },
    );
}

#[test]
fn callback_rejection_panic_and_floor_undercut_never_escape_as_success() {
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            for mode in 0..3 {
                let mut work = Work::new(100_000);
                let mut budget = Budget::new(&mut work, 100_000);
                budget.reserve_storage(37).unwrap();
                let entered = Cell::new(false);
                let result = with_report_rows_v18(original, &mut budget, |_, budget| {
                    entered.set(true);
                    match mode {
                        0 => Err::<(), _>(ReportConstructionErrorV18::Rejected),
                        1 => panic!("phase consumer unwind"),
                        _ => {
                            budget.release_storage(budget.storage()).unwrap();
                            Ok(())
                        }
                    }
                });
                assert!(entered.get());
                assert_eq!(
                    result,
                    Err(match mode {
                        0 => ReportConstructionErrorV18::Rejected,
                        1 => ReportConstructionErrorV18::Panicked,
                        _ => Resource::Accounting.into(),
                    })
                );
                assert_eq!(budget.storage(), if mode == 2 { 0 } else { 37 });
            }
        },
    );
}

#[test]
fn foreign_budget_is_not_refunded_or_used_to_select_a_denial() {
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let mut work = Work::new(100_000);
            let mut foreign_work = Work::new(0);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut foreign = Budget::new(&mut foreign_work, 100_000);
            budget.reserve_storage(37).unwrap();
            foreign.reserve_storage(19).unwrap();
            assert!(foreign.charge_work(1).is_err());
            let entered = Cell::new(false);
            let result = with_report_rows_v18(original, &mut budget, |_, budget| {
                std::mem::swap(budget, &mut foreign);
                entered.set(true);
                Ok(())
            });
            assert!(entered.get());
            assert_eq!(result, Err(Resource::Accounting.into()));
            assert_eq!(
                (budget.work(), budget.storage(), budget.failed_work()),
                (0, 19, Some(1))
            );
            assert!(
                foreign.storage() > 37,
                "the moved original account was not refunded"
            );
            // Test-owned cleanup after both report vectors and protected scope died.
            let released = foreign.storage() - 37;
            foreign.release_storage(released).unwrap();
            assert_eq!(foreign.storage(), 37);
        },
    );
}

#[test]
fn aligned_callback_capture_is_paid_before_empty_phase_callback() {
    #[repr(align(64))]
    struct Capture([u8; 96]);
    with_original(
        &fixture(0, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let expected =
                independent_headers() - size_of::<Consumer>() - size_of::<Option<Consumer>>()
                    + size_of::<Capture>()
                    + size_of::<Option<Capture>>();
            for limit in [37 + expected - 1, 37 + expected] {
                let mut work = Work::new(4);
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(37).unwrap();
                let completed = COMPLETED.with(Cell::get);
                let capture = Capture([7; 96]);
                let consumer = move |_: &ReportRowsV18<'_, '_>, _: &mut Budget<'_>| {
                    assert_eq!(std::hint::black_box(&capture).0[0], 7);
                    COMPLETED.with(|value| value.set(value.get() + 1));
                    Ok(())
                };
                assert_eq!(std::mem::size_of_val(&consumer), size_of::<Capture>());
                let result = with_report_rows_v18(original, &mut budget, consumer);
                let success = limit == 37 + expected;
                assert_eq!(result.is_ok(), success);
                assert_eq!(COMPLETED.with(Cell::get), completed + usize::from(success));
                assert_eq!(budget.storage(), 37);
                if !success {
                    assert_eq!(
                        result,
                        Err(ReportConstructionErrorV18::Resource(
                            GuardedResourceErrorV1::Storage {
                                actual: 37 + expected,
                                limit
                            }
                        ))
                    );
                }
            }
        },
    );
}

#[test]
fn rejected_result_destructor_is_drained_before_original_credit_refund() {
    struct Explodes;
    impl Drop for Explodes {
        fn drop(&mut self) {
            panic!("rejected phase result destructor");
        }
    }
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            budget.reserve_storage(37).unwrap();
            let completed = Cell::new(false);
            let result = with_report_rows_v18(original, &mut budget, |_, budget| {
                assert!(budget.charge_work(100_000).is_err());
                completed.set(true);
                Ok(Explodes)
            });
            assert!(completed.get());
            assert!(matches!(
                result,
                Err(ReportConstructionErrorV18::PriorDenial {
                    work: Some(_),
                    storage: None
                })
            ));
            assert_eq!(budget.storage(), 37);
        },
    );
}

#[test]
fn repeated_same_allocation_rows_pay_old_and_replacement_conflict_capacity() {
    with_original(
        &fixture(2, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let expected_work = 4 + 36 + 20 + 8 + 16 + 105;
            let peak = independent_headers()
                + 2 * size_of::<FormalBoundsRequirement>()
                + (2 * size_of::<(FormalAllocationIdentity, AllocationEnvelope)>())
                    .max(6 * size_of::<InterInvocationConflictRequirement>());
            for (work_cap, storage_cap, success) in [
                (expected_work, 37 + peak, true),
                (expected_work - 1, 37 + peak, false),
                (expected_work, 36 + peak, false),
            ] {
                let mut work = Work::new(work_cap);
                let mut budget = Budget::new(&mut work, storage_cap);
                budget.reserve_storage(37).unwrap();
                let completed = COMPLETED.with(Cell::get);
                let result = with_report_rows_v18(original, &mut budget, inspect as Consumer);
                assert_eq!(result.is_ok(), success);
                assert_eq!(COMPLETED.with(Cell::get), completed + usize::from(success));
                assert_eq!(budget.storage(), 37);
                if success {
                    assert_eq!(
                        (budget.work(), budget.peak_storage()),
                        (expected_work, 37 + peak)
                    );
                } else if work_cap < expected_work {
                    assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Work(error))) if error.actual() == expected_work && error.limit() == work_cap)
                    );
                } else {
                    assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Storage { actual, limit })) if actual == 37 + peak && limit == storage_cap)
                    );
                }
            }
            assert_eq!(
                original
                    .analysis()
                    .obligations()
                    .inter_invocation_conflicts()
                    .len(),
                3
            );
        },
    );
}

#[test]
fn rejected_capture_destructor_cannot_mask_prior_or_construction_denial() {
    struct Capture;
    impl Drop for Capture {
        fn drop(&mut self) {
            CAPTURE_DROPS.with(|value| value.set(value.get() + 1));
            panic!("rejected capture destructor");
        }
    }
    with_original(
        &fixture(1, false, false),
        launch(),
        FormalIndexWidth::Bits64,
        |original| {
            let frames =
                independent_headers() - size_of::<Consumer>() - size_of::<Option<Consumer>>()
                    + size_of::<Capture>()
                    + size_of::<Option<Capture>>();
            for mode in 0..4 {
                let work_cap = if mode == 1 { 3 } else { 100_000 };
                let storage_cap = match mode {
                    2 => 37 + frames - 1,
                    3 => 37 + frames + 2 * size_of::<FormalBoundsRequirement>() - 1,
                    _ => 100_000,
                };
                let mut work = Work::new(work_cap);
                let mut budget = Budget::new(&mut work, storage_cap);
                budget.reserve_storage(37).unwrap();
                if mode == 0 {
                    assert!(budget.charge_work(100_001).is_err());
                }
                let completed = COMPLETED.with(Cell::get);
                let dropped = CAPTURE_DROPS.with(Cell::get);
                let capture = Capture;
                let consumer = move |_: &ReportRowsV18<'_, '_>, _: &mut Budget<'_>| {
                    std::hint::black_box(&capture);
                    COMPLETED.with(|value| value.set(value.get() + 1));
                    Ok(())
                };
                assert_eq!(std::mem::size_of_val(&consumer), size_of::<Capture>());
                let result = with_report_rows_v18(original, &mut budget, consumer);
                assert_eq!(COMPLETED.with(Cell::get), completed);
                assert_eq!(CAPTURE_DROPS.with(Cell::get), dropped + 1);
                assert_eq!(budget.storage(), 37);
                match mode {
                    0 => assert_eq!(
                        result,
                        Err(ReportConstructionErrorV18::PriorDenial {
                            work: Some(100_001),
                            storage: None
                        })
                    ),
                    1 => assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Work(error))) if error.actual() == 4 && error.limit() == 3)
                    ),
                    _ => assert!(
                        matches!(result, Err(ReportConstructionErrorV18::Resource(GuardedResourceErrorV1::Storage { actual, limit })) if actual == storage_cap + 1 && limit == storage_cap)
                    ),
                }
            }
        },
    );
}

#[test]
fn genuine_overflow_reasons_and_conflict_rows_remain_unmodified_and_ordered() {
    let mut module = fixture(2, false, false);
    let pointer = module.functions[0].signature.parameters[0].clone();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    for operation in &mut block.operations {
        let OperationKind::Store { pointer, .. } = &mut operation.kind else {
            unreachable!()
        };
        *pointer = ValueId(4);
    }
    block.operations.splice(
        0..0,
        [
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::INDEX),
                OperationKind::Constant(Constant::Index(u64::MAX / 4)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(0),
                    offset: ValueId(3),
                },
            ),
        ],
    );
    with_original(&module, launch(), FormalIndexWidth::Bits64, |original| {
        let expected = [
            FunctionOperationLocation::new(BlockId(0), 2),
            FunctionOperationLocation::new(BlockId(0), 3),
        ];
        assert_eq!(
            original.analysis().incomplete_reasons(),
            expected.map(
                |location| FormalMemoryIncompleteReason::AddressArithmeticOverflow { location }
            )
        );
        assert_eq!(original.analysis().obligations().accesses().len(), 2);
        assert_eq!(
            original
                .analysis()
                .obligations()
                .inter_invocation_conflicts()
                .len(),
            3
        );
        assert!(
            original
                .analysis()
                .obligations()
                .bounds_requirements()
                .is_empty()
        );
        let completed = Cell::new(false);
        let mut work = Work::new(162);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(37).unwrap();
        with_report_rows_v18(original, &mut budget, |view, _| {
            assert_eq!(view.overflows.as_slice(), expected);
            assert!(view.bounds.is_empty());
            assert_eq!(view.conflicts.len(), 3);
            assert!(!view.original.analysis().is_complete());
            completed.set(true);
            Ok(())
        })
        .unwrap();
        assert!(completed.get());
        // Entry4; bounds16*2 + ordered reason insertions3+2; alias scan16;
        // three self/same-allocation candidates plus conflict growth105.
        assert_eq!(budget.work(), 4 + 37 + 16 + 105);
        assert_eq!(budget.storage(), 37);
    });
}

#[test]
fn overflow_reason_index_pays_comparisons_dedup_moves_and_replacement() {
    for (work_cap, storage_cap, success) in [
        (16, 6 * size_of::<FunctionOperationLocation>(), true),
        (15, 6 * size_of::<FunctionOperationLocation>(), false),
        (16, 6 * size_of::<FunctionOperationLocation>() - 1, false),
    ] {
        let mut work = Work::new(work_cap);
        let mut budget = Budget::new(&mut work, storage_cap);
        let mut rows = Vec::<FunctionOperationLocation>::new();
        let result = {
            let mut meter = LiveReportMeterV18 {
                budget: &mut budget,
            };
            [3, 1, 3, 2].into_iter().try_for_each(|ordinal| {
                rows.overflow(
                    FunctionOperationLocation::new(BlockId(0), ordinal),
                    &mut meter,
                )
            })
        };
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(
                rows,
                [1, 2, 3].map(|ordinal| FunctionOperationLocation::new(BlockId(0), ordinal))
            );
            assert_eq!(budget.work(), 3 + 3 + 2 + 8);
            assert_eq!(
                budget.peak_storage(),
                6 * size_of::<FunctionOperationLocation>()
            );
            assert_eq!(budget.storage(), 4 * size_of::<FunctionOperationLocation>());
        } else if work_cap == 15 {
            assert!(
                matches!(result, Err(GuardedResourceErrorV1::Work(error)) if error.actual() == 16 && error.limit() == 15)
            );
        } else {
            assert!(
                matches!(result, Err(GuardedResourceErrorV1::Storage { actual, limit }) if actual == 6 * size_of::<FunctionOperationLocation>() && limit == storage_cap)
            );
        }
        // This low-level container test owns the backing and its remaining
        // credit. There is no report consumer or enclosing production scope.
        drop(rows);
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn inspect_retained(view: &ReportRowsV18<'_, '_>, budget: &mut Budget<'_>) -> ResultV18<()> {
    let retained = view.bounds.capacity() * size_of::<FormalBoundsRequirement>()
        + view.overflows.capacity() * size_of::<FunctionOperationLocation>()
        + view.aliases.capacity() * size_of::<RuntimeAliasRequirement>()
        + view.conflicts.capacity() * size_of::<InterInvocationConflictRequirement>();
    assert_eq!(budget.storage(), 37 + independent_headers() + retained);
    inspect(view, budget)
}

#[test]
fn retained_report_capacities_stay_paid_after_alias_scratch_is_destroyed() {
    for count in [0, 1, 2, 3, 8, 44] {
        with_original(
            &fixture(count, true, false),
            launch(),
            FormalIndexWidth::Bits64,
            |original| {
                let mut work = Work::new(1_000_000);
                let mut budget = Budget::new(&mut work, 1_000_000);
                budget.reserve_storage(37).unwrap();
                let completed = COMPLETED.with(Cell::get);
                with_report_rows_v18(original, &mut budget, inspect_retained as Consumer).unwrap();
                assert_eq!(COMPLETED.with(Cell::get), completed + 1);
                assert_eq!(budget.storage(), 37);
            },
        );
    }
}
