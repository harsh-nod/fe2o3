use super::super::accesses::body_source_v19::BodyErrorV19;
use super::*;
use crate::formal_memory_obligations::PhysicalLaunchInterpretationV2 as Interpretation;
use crate::{CanonicalEffectErrorV19, with_canonical_effects_v19};
use std::result::Result;

const LIMIT: usize = 50_000_000;
const FLOOR: usize = 23;

// Exact pre-refactor body/assembly/private/call policy. This reference does not
// call the new shared decision engine and is never a production fallback.
include!("body_original_reference_v19_tests.rs");
include!("launch_original_reference_v19_tests.rs");

fn store_fixture(stores: usize, different_allocations: bool) -> Module {
    let mut result = straight();
    let operations = &mut result.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.push(op(
        200,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(7)),
    ));
    for ordinal in 0..stores {
        operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(if different_allocations && ordinal % 2 == 1 {
                    1
                } else {
                    11
                }),
                value: ValueId(200),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    result
}

fn launch(count: u64) -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    }
}

fn legacy(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    interpretation: Interpretation,
) -> FormalMemoryObligationAnalysis {
    let effects = crate::interprocedural_effects::analyze_interprocedural_effects_from_storage_v18(
        owner.verified_storage_module_ref_v1(),
    )
    .unwrap();
    let legacy = derive_kernel_memory_obligations_with_launch_interpretation(
        owner.module(),
        &owner.module().kernels[0].id,
        launch,
        width,
        None,
        None,
        &effects,
        interpretation,
    )
    .unwrap();
    let mut reader = effect_reader_v19::LegacyEffectReaderV19(&effects);
    let original = original_body_report_v19(
        owner.module(),
        &owner.module().kernels[0].id,
        launch,
        width,
        None,
        None,
        &mut reader,
        interpretation,
    )
    .unwrap();
    assert_eq!(legacy, original);
    original
}

struct Run {
    result: std::result::Result<(), BodyErrorV19>,
    reports: Vec<FormalMemoryObligationAnalysis>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn run(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    interpretation: Interpretation,
    repeats: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Run {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut result = None;
    let mut entered = false;
    let mut reports = Vec::new();
    let effects_result = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        entered = true;
        result = Some((|| -> std::result::Result<(), BodyErrorV19> {
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)?;
            let mut slots = affine.private_slots(owner, 0, budget)?;
            let mut pointers = slots.pointers(owner, 0, budget)?;
            let mut accesses = pointers.accesses(owner, 0, budget)?;
            let retained = budget.storage();
            for _ in 0..repeats {
                let report = accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch,
                    width,
                    interpretation,
                    budget,
                    |view, _| {
                        assert!(std::ptr::eq(view.original_owner(), owner));
                        assert!(std::ptr::eq(
                            view.original_function(),
                            &owner.module().functions[0]
                        ));
                        assert_eq!(view.root_index(), 0);
                        assert_eq!(view.launch_extent(), launch);
                        assert_eq!(
                            matches!(view.interpretation(), Interpretation::Envelope),
                            matches!(interpretation, Interpretation::Envelope)
                        );
                        // Independent test-owned observation, not compiler ledger storage.
                        reports.push(view.analysis().clone());
                        Ok(())
                    },
                );
                assert_eq!(budget.storage(), retained);
                if let Err(error) = report {
                    let before = (budget.work(), budget.storage());
                    let again = accesses.with_report_v19(
                        owner,
                        0,
                        effects,
                        launch,
                        width,
                        interpretation,
                        budget,
                        |_, _| panic!("failed report callback must not run"),
                    );
                    assert_eq!(again, Err(error.clone()));
                    assert_eq!((budget.work(), budget.storage()), before);
                    accesses.release(budget)?;
                    pointers.release(budget)?;
                    slots.release(budget)?;
                    affine.release(budget)?;
                    return Err(error);
                }
            }
            accesses.release(budget)?;
            pointers.release(budget)?;
            slots.release(budget)?;
            affine.release(budget)?;
            Ok(())
        })());
        Ok(())
    });
    assert!(
        !entered || result.is_some(),
        "inner assertions must complete even after a resource denial"
    );
    let result = match (result, effects_result) {
        (Some(Ok(())), result) => result.map_err(BodyErrorV19::Effects),
        (Some(result), _) => result,
        (None, result) => result.map_err(BodyErrorV19::Effects),
    };
    // A denied predecessor may retain its own credit until this test-owned
    // outer scratch scope drops all original contexts, never a borrowed report.
    budget.rollback_storage(FLOOR).unwrap();
    Run {
        result,
        reports,
        work: budget.work(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    }
}

#[test]
fn paid_formal_report_matches_original_access_bounds_aliases_conflicts_and_replays() {
    for (count, distinct) in [(0, false), (1, false), (2, false), (2, true), (7, true)] {
        with_owner(&store_fixture(count, distinct), |owner| {
            let expected = legacy(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
            );
            let actual = run(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                2,
                LIMIT,
                LIMIT,
            );
            assert_eq!(actual.result, Ok(()));
            assert_eq!(actual.reports, vec![expected.clone(), expected]);
            assert_eq!(actual.failed_work, None);
            assert_eq!(actual.failed_storage, None);
        });
    }
}

#[test]
fn paid_formal_report_preserves_exact_physical_and_incomplete_invocation_semantics() {
    let mut module = store_fixture(2, false);
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(2),
    };
    with_owner(&module, |owner| {
        for extent in [
            ExplicitLaunchExtent::Unknown,
            launch(0),
            launch(1),
            launch(2),
            launch(8),
        ] {
            for width in [
                FormalIndexWidth::Unknown,
                FormalIndexWidth::Bits32,
                FormalIndexWidth::Bits64,
            ] {
                for interpretation in [Interpretation::Exact, Interpretation::Envelope] {
                    let expected = legacy(owner, extent, width, interpretation);
                    let actual = run(owner, extent, width, interpretation, 1, LIMIT, LIMIT);
                    assert_eq!(actual.result, Ok(()));
                    assert_eq!(actual.reports, vec![expected]);
                }
            }
        }
        let exact = run(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Exact,
            1,
            LIMIT,
            LIMIT,
        );
        assert!(!exact.reports[0].is_complete());
        let physical = run(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Envelope,
            1,
            LIMIT,
            LIMIT,
        );
        assert!(physical.reports[0].is_complete());
        assert_eq!(
            physical.reports[0]
                .obligations()
                .inter_invocation_conflicts()
                .len(),
            3
        );
        assert_eq!(owner.module().kernels[0].domain, module.kernels[0].domain);
    });
}

#[test]
fn paid_formal_report_ranked_envelopes_keep_all_axis_shape_and_overflow_refusals() {
    for domain in [
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(3),
        },
        LaunchDomain::D3 {
            x: LaunchExtent::Static(2),
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Static(3),
        },
    ] {
        let mut module = store_fixture(2, false);
        module.kernels[0].domain = domain;
        with_owner(&module, |owner| {
            let rank = owner.module().kernels[0].domain.rank();
            let good = if rank == 2 { [5, 7, 1] } else { [5, 7, 9] };
            let insufficient = if rank == 2 { [5, 2, 1] } else { [5, 7, 2] };
            for extent in [
                ExplicitLaunchExtent::Unknown,
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: good,
                },
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: insufficient,
                },
                ExplicitLaunchExtent::Exact {
                    rank: 0,
                    extents: [1, 1, 1],
                },
                ExplicitLaunchExtent::Exact {
                    rank: 4,
                    extents: [1, 1, 1],
                },
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [1, 1, 1],
                },
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: [0, 3, 3],
                },
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: [u64::MAX, 3, if rank == 2 { 1 } else { 3 }],
                },
                ExplicitLaunchExtent::Exact {
                    rank: 2,
                    extents: [5, 3, 2],
                },
            ] {
                for interpretation in [Interpretation::Exact, Interpretation::Envelope] {
                    let expected = legacy(owner, extent, FormalIndexWidth::Bits64, interpretation);
                    let actual = run(
                        owner,
                        extent,
                        FormalIndexWidth::Bits64,
                        interpretation,
                        1,
                        LIMIT,
                        LIMIT,
                    );
                    assert_eq!(actual.result, Ok(()));
                    assert_eq!(actual.reports, vec![expected]);
                }
            }
            let padded = run(
                owner,
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: good,
                },
                FormalIndexWidth::Bits64,
                Interpretation::Envelope,
                1,
                LIMIT,
                LIMIT,
            );
            assert_eq!(padded.result, Ok(()));
            assert!(padded.reports[0].is_complete());
            let incomplete = run(
                owner,
                ExplicitLaunchExtent::Exact {
                    rank,
                    extents: insufficient,
                },
                FormalIndexWidth::Bits64,
                Interpretation::Envelope,
                1,
                LIMIT,
                LIMIT,
            );
            assert_eq!(incomplete.result, Ok(()));
            assert!(matches!(incomplete.reports[0].incomplete_reasons(),
                [FormalMemoryIncompleteReason::StaticLaunchAxisExtentMismatch { axis, expected: 3, actual: 2 }]
                    if *axis == if rank == 2 { Axis::Y } else { Axis::Z }));
        });
    }
}

#[test]
fn paid_formal_report_preserves_guarded_reads_switches_and_dynamic_pointer_refusals() {
    let mut cases = vec![store_fixture(1, false)];
    for switch in [
        None,
        Some(Type::Scalar(ScalarType::U32)),
        Some(Type::Scalar(ScalarType::U64)),
    ] {
        cases.push(crate::formal_memory_obligations::guarded_access_v1::tests::fixture(switch));
    }
    let mut dynamic = store_fixture(1, false);
    let operations = &mut dynamic.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.last_mut().unwrap().kind = OperationKind::Store {
        pointer: ValueId(13),
        value: ValueId(200),
        access: MemoryAccess::new(AddressSpace::Global, 4),
    };
    cases.push(dynamic);
    for module in cases {
        with_owner(&module, |owner| {
            let expected = legacy(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
            );
            let actual = run(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                1,
                LIMIT,
                LIMIT,
            );
            assert_eq!(actual.result, Ok(()));
            assert_eq!(actual.reports, vec![expected]);
        });
    }
}

#[test]
fn paid_formal_report_orders_long_callee_reasons_without_losing_original_call_rows() {
    for bytes in [1, 32, 513] {
        let name = "z".repeat(bytes);
        let mut module = store_fixture(1, false);
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        for callee in [&name, "a", &name] {
            operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new(callee),
                    arguments: vec![],
                },
            ));
        }
        module
            .functions
            .push(Function::declaration(name, Signature::new(vec![], vec![])));
        module
            .functions
            .push(Function::declaration("a", Signature::new(vec![], vec![])));
        with_owner(&module, |owner| {
            let expected = legacy(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
            );
            let actual = run(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                2,
                LIMIT,
                LIMIT,
            );
            assert_eq!(actual.result, Ok(()));
            assert_eq!(actual.reports, vec![expected.clone(), expected]);
            assert_eq!(actual.reports[0].incomplete_reasons().len(), 3);
        });
    }
}

#[test]
fn paid_formal_report_preserves_private_generic_escape_and_unsupported_effect_policy_together() {
    for escape in [false, true] {
        let mut module = store_fixture(1, false);
        let private = pointer(AddressSpace::Private);
        let generic = pointer(AddressSpace::Generic);
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations.extend([
            op(
                300,
                private.clone(),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            op(
                301,
                generic.clone(),
                OperationKind::Cast {
                    kind: CastKind::PointerToGeneric,
                    value: ValueId(300),
                    to: generic,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(301),
                    value: ValueId(200),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            ),
            op(
                302,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(301),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            ),
            op(
                310,
                pointer(AddressSpace::Workgroup),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Workgroup,
                    alignment: 4,
                },
            ),
        ]);
        if escape {
            operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("private_escape"),
                    arguments: vec![ValueId(300)],
                },
            ));
            module.functions.push(Function::declaration(
                "private_escape",
                Signature::new(vec![private], vec![]),
            ));
        }
        for function in &mut module.functions {
            function.required_capabilities = function.derived_capabilities();
            module
                .required_capabilities
                .extend(function.required_capabilities.iter().cloned());
        }
        with_owner(&module, |owner| {
            let expected = legacy(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
            );
            let actual = run(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                1,
                LIMIT,
                LIMIT,
            );
            assert_eq!(actual.result, Ok(()));
            assert_eq!(actual.reports, vec![expected]);
            assert_eq!(actual.reports[0].obligations().accesses().len(), 1);
            assert!(
                actual.reports[0]
                    .incomplete_reasons()
                    .iter()
                    .any(|reason| matches!(
                        reason,
                        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
                    ))
            );
            assert_eq!(
                actual.reports[0]
                    .incomplete_reasons()
                    .iter()
                    .any(|reason| matches!(
                        reason,
                        FormalMemoryIncompleteReason::CallEffectsUnavailable { .. }
                    )),
                escape
            );
            assert_eq!(
                actual.reports[0]
                    .incomplete_reasons()
                    .iter()
                    .any(|reason| matches!(
                        reason,
                        FormalMemoryIncompleteReason::UnsupportedPointerDerivation { .. }
                    )),
                escape
            );
        });
    }
}

#[test]
fn paid_formal_report_exact_and_one_short_resources_cover_same_allocation_vector_growth() {
    with_owner(&store_fixture(2, false), |owner| {
        let measured = run(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Exact,
            2,
            LIMIT,
            LIMIT,
        );
        assert_eq!(measured.result, Ok(()));
        assert_eq!(
            measured.reports[0]
                .obligations()
                .inter_invocation_conflicts()
                .len(),
            3
        );
        let exact = run(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Exact,
            2,
            measured.work,
            measured.peak,
        );
        assert_eq!(exact.result, Ok(()));
        assert_eq!(exact.reports, measured.reports);
        for (work, storage, resource) in [
            (measured.work - 1, measured.peak, 0),
            (measured.work, measured.peak - 1, 1),
        ] {
            let short = run(
                owner,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                2,
                work,
                storage,
            );
            assert!(short.result.is_err());
            if resource == 0 {
                assert!(short.failed_work.is_some_and(|actual| actual > work));
                assert_eq!(short.failed_storage, None);
            } else {
                assert!(short.failed_storage.is_some_and(|actual| actual > storage));
                assert_eq!(short.failed_work, None);
            }
        }
    });
}

#[test]
fn paid_formal_report_is_derived_afresh_from_changed_actual_owner() {
    let original = store_fixture(1, false);
    let changed = store_fixture(2, true);
    with_owner(&original, |first| {
        with_owner(&changed, |second| {
            let left = run(
                first,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                1,
                LIMIT,
                LIMIT,
            );
            let right = run(
                second,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                1,
                LIMIT,
                LIMIT,
            );
            assert_eq!(left.result, Ok(()));
            assert_eq!(right.result, Ok(()));
            assert_ne!(left.reports, right.reports);
            assert_eq!(
                left.reports[0],
                legacy(
                    first,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact
                )
            );
            assert_eq!(
                right.reports[0],
                legacy(
                    second,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact
                )
            );
        })
    });
}

#[test]
fn paid_formal_report_rejects_foreign_account_without_poisoning_original_custody() {
    with_owner(&store_fixture(1, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut completed = false;
        with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget).unwrap();
            let mut slots = affine.private_slots(owner, 0, budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, budget).unwrap();
            let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
            let before = (budget.work(), budget.storage());
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, LIMIT);
            foreign.reserve_storage(before.1).unwrap();
            let result = accesses.with_report_v19(
                owner,
                0,
                effects,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                &mut foreign,
                |_, _| panic!("foreign account callback"),
            );
            assert_eq!(
                result,
                Err(BodyErrorV19::Source(ResourceError::Accounting.into()))
            );
            assert_eq!((budget.work(), budget.storage()), before);
            accesses
                .with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    |view, _| {
                        assert_eq!(view.analysis().obligations().accesses().len(), 1);
                        completed = true;
                        Ok(())
                    },
                )
                .unwrap();
            accesses.release(budget).unwrap();
            pointers.release(budget).unwrap();
            slots.release(budget).unwrap();
            affine.release(budget).unwrap();
            Ok(())
        })
        .unwrap();
        assert!(completed);
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn paid_formal_report_rejects_equal_byte_foreign_owner_and_wrong_root_before_callback() {
    let module = store_fixture(1, false);
    with_owner(&module, |owner| {
        with_owner(&module, |foreign| {
            assert_eq!(owner.canonical_bytes(), foreign.canonical_bytes());
            for bad_root in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                    let mut affine =
                        ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)
                            .unwrap();
                    let mut slots = affine.private_slots(owner, 0, budget).unwrap();
                    let mut pointers = slots.pointers(owner, 0, budget).unwrap();
                    let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
                    let before = (budget.work(), budget.storage());
                    let result = accesses.with_report_v19(
                        if bad_root { owner } else { foreign },
                        usize::from(bad_root),
                        effects,
                        launch(8),
                        FormalIndexWidth::Bits64,
                        Interpretation::Exact,
                        budget,
                        |_, _| panic!("foreign source callback"),
                    );
                    assert_eq!(
                        result,
                        Err(BodyErrorV19::Source(ResourceError::Accounting.into()))
                    );
                    assert_eq!((budget.work(), budget.storage()), before);
                    let again = accesses.with_report_v19(
                        owner,
                        0,
                        effects,
                        launch(8),
                        FormalIndexWidth::Bits64,
                        Interpretation::Exact,
                        budget,
                        |_, _| panic!("poisoned source callback"),
                    );
                    assert_eq!(again, result);
                    accesses.release(budget).unwrap();
                    pointers.release(budget).unwrap();
                    slots.release(budget).unwrap();
                    affine.release(budget).unwrap();
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.storage(), 0);
            }
        })
    });
}

#[test]
fn paid_formal_report_call_free_body_still_authenticates_actual_effect_scope_owner() {
    let module = store_fixture(0, false);
    with_owner(&module, |owner| {
        with_owner(&module, |foreign| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut completed = false;
            let outer = with_canonical_effects_v19(foreign, &mut budget, |effects, budget| {
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, budget).unwrap();
                let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
                let retained = budget.storage();
                let result = accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    |_, _| panic!("foreign effects callback"),
                );
                assert_eq!(
                    result,
                    Err(BodyErrorV19::Effects(CanonicalEffectErrorV19::ForeignOwner))
                );
                assert_eq!(budget.storage(), retained);
                accesses.release(budget).unwrap();
                pointers.release(budget).unwrap();
                slots.release(budget).unwrap();
                affine.release(budget).unwrap();
                completed = true;
                Ok(())
            });
            assert_eq!(outer, Err(CanonicalEffectErrorV19::ForeignOwner));
            assert!(completed);
            assert_eq!(budget.storage(), 0);
        })
    });
}

#[test]
fn paid_formal_report_callback_undercut_and_ledger_swap_cannot_complete_or_refund_foreign_credit() {
    with_owner(&store_fixture(1, false), |owner| {
        for swap in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, LIMIT);
            foreign.reserve_storage(19).unwrap();
            let mut completed = false;
            let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, budget).unwrap();
                let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
                let retained = budget.storage();
                let mut report_floor = 0;
                let result = accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    |_, budget| {
                        report_floor = budget.storage();
                        if swap {
                            std::mem::swap(budget, &mut foreign);
                        } else {
                            budget.release_storage(1).unwrap();
                        }
                        Ok(())
                    },
                );
                assert_eq!(
                    result,
                    Err(BodyErrorV19::Source(ResourceError::Accounting.into()))
                );
                assert!(report_floor > retained);
                if swap {
                    assert_eq!(budget.storage(), 19);
                    assert_eq!(foreign.storage(), report_floor);
                    std::mem::swap(budget, &mut foreign);
                } else {
                    assert_eq!(budget.storage(), report_floor - 1);
                    budget.reserve_storage(1).unwrap();
                }
                // The report was dropped before the refused settlement. This
                // test-owned restoration retires only its now-dead backing.
                budget.rollback_storage(retained).unwrap();
                let before = (budget.work(), budget.storage());
                assert_eq!(
                    accesses.with_report_v19(
                        owner,
                        0,
                        effects,
                        launch(8),
                        FormalIndexWidth::Bits64,
                        Interpretation::Exact,
                        budget,
                        |_, _| panic!("retry after custody failure"),
                    ),
                    result
                );
                assert_eq!((budget.work(), budget.storage()), before);
                accesses.release(budget).unwrap();
                pointers.release(budget).unwrap();
                slots.release(budget).unwrap();
                affine.release(budget).unwrap();
                completed = true;
                Ok(())
            });
            assert_eq!(outer, Ok(()));
            assert!(completed);
            assert_eq!(budget.storage(), 0);
            assert_eq!(foreign.storage(), 19);
        }
    });
}

#[test]
fn paid_formal_report_callback_rejection_panic_and_surplus_storage_remain_fail_closed() {
    with_owner(&store_fixture(2, false), |owner| {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut reached = false;
            let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, budget).unwrap();
                let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
                let retained = budget.storage();
                let result = accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    |view, budget| {
                        assert_eq!(
                            view.analysis()
                                .obligations()
                                .inter_invocation_conflicts()
                                .len(),
                            3
                        );
                        reached = true;
                        match mode {
                            0 => Err(BodyErrorV19::Rejected),
                            1 => panic!("deliberate report consumer panic"),
                            _ => {
                                budget.reserve_storage(17).unwrap();
                                Ok(())
                            }
                        }
                    },
                );
                let expected = match mode {
                    0 => BodyErrorV19::Rejected,
                    1 => BodyErrorV19::Panicked,
                    _ => BodyErrorV19::Source(ResourceError::Accounting.into()),
                };
                assert_eq!(result, Err(expected.clone()));
                assert_eq!(budget.storage(), retained + if mode == 2 { 17 } else { 0 });
                let before = (budget.work(), budget.storage());
                assert_eq!(
                    accesses.with_report_v19(
                        owner,
                        0,
                        effects,
                        launch(8),
                        FormalIndexWidth::Bits64,
                        Interpretation::Exact,
                        budget,
                        |_, _| panic!("retry after consumer refusal"),
                    ),
                    Err(expected)
                );
                assert_eq!((budget.work(), budget.storage()), before);
                if mode == 2 {
                    budget.release_storage(17).unwrap();
                }
                accesses.release(budget).unwrap();
                pointers.release(budget).unwrap();
                slots.release(budget).unwrap();
                affine.release(budget).unwrap();
                Ok(())
            });
            assert_eq!(outer, Ok(()));
            assert!(reached);
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn paid_formal_report_preserves_first_denial_when_rejected_captures_panic() {
    use std::cell::Cell;
    struct PanicDrop<'a>(&'a Cell<usize>);
    impl Drop for PanicDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected report capture destructor");
        }
    }
    with_owner(&store_fixture(1, false), |owner| {
        for construction in [false, true] {
            let dropped = Cell::new(0);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let completed = Cell::new(false);
            let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, budget).unwrap();
                let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
                let retained = budget.storage();
                if construction {
                    // Zero remaining capacity: the report frame is the first
                    // denial, after entry validates the genuine retained floor.
                    budget.reserve_storage(LIMIT - retained).unwrap();
                } else {
                    assert!(budget.reserve_storage(LIMIT).is_err());
                }
                let floor = budget.storage();
                let capture = PanicDrop(&dropped);
                let result = accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    move |_, _| {
                        let _capture = capture;
                        panic!("denied report body must not run");
                    },
                );
                assert!(matches!(result, Err(BodyErrorV19::Source(_))));
                assert_eq!(dropped.get(), 1);
                assert!(budget.failed_storage().is_some_and(|actual| actual > LIMIT));
                assert_eq!(budget.storage(), floor);
                if construction {
                    budget.release_storage(LIMIT - retained).unwrap();
                }
                accesses.release(budget).unwrap();
                pointers.release(budget).unwrap();
                slots.release(budget).unwrap();
                affine.release(budget).unwrap();
                completed.set(true);
                Ok(())
            });
            assert!(matches!(outer, Err(CanonicalEffectErrorV19::Resource(_))));
            assert_eq!(dropped.get(), 1);
            assert!(
                completed.get(),
                "post-denial assertions and releases must finish"
            );
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn paid_formal_report_ignored_callback_work_denial_precedes_floor_undercut_and_retry() {
    with_owner(&store_fixture(1, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut completed = false;
        let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, budget).unwrap();
            let mut slots = affine.private_slots(owner, 0, budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, budget).unwrap();
            let mut accesses = pointers.accesses(owner, 0, budget).unwrap();
            let retained = budget.storage();
            let mut first = None;
            let result = accesses.with_report_v19(
                owner,
                0,
                effects,
                launch(8),
                FormalIndexWidth::Bits64,
                Interpretation::Exact,
                budget,
                |_, budget| {
                    first = Some(BodyErrorV19::Source(
                        budget.charge_work(LIMIT).unwrap_err().into(),
                    ));
                    budget.release_storage(1).unwrap();
                    Ok(())
                },
            );
            assert_eq!(result, Err(first.unwrap()));
            assert!(budget.failed_work().is_some_and(|actual| actual > LIMIT));
            assert_eq!(budget.failed_storage(), None);
            budget.reserve_storage(1).unwrap();
            budget.rollback_storage(retained).unwrap();
            let before = (budget.work(), budget.storage());
            assert_eq!(
                accesses.with_report_v19(
                    owner,
                    0,
                    effects,
                    launch(8),
                    FormalIndexWidth::Bits64,
                    Interpretation::Exact,
                    budget,
                    |_, _| panic!("ignored work-denial retry"),
                ),
                result
            );
            assert_eq!((budget.work(), budget.storage()), before);
            accesses.release(budget).unwrap();
            pointers.release(budget).unwrap();
            slots.release(budget).unwrap();
            affine.release(budget).unwrap();
            completed = true;
            Ok(())
        });
        assert!(matches!(outer, Err(CanonicalEffectErrorV19::Resource(_))));
        assert!(completed);
        assert_eq!(budget.storage(), 0);
    });
}
