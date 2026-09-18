//! Synthetic provider labels exercise agreement, not producer authentication.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_lower_mir_kernel::{
    ProductionExecutionSourceInputV29 as SourceInput, ProductionScopeCallKindV29 as CallKind,
    ProductionScopeCallableCandidateV29 as Class, ProductionScopeEventCandidateV29 as Event,
    ProductionScopeEventKindV29 as EventKind, with_checked_execution_source_v29,
};

fn rows(ssa: &ProductionSemanticSsaOwnerV1) -> (Vec<Class>, Vec<Event>) {
    (
        vec![
            Class::Provider {
                function: HELPER,
                identity: ssa.source_semantic().functions()[0].identity(),
            },
            Class::Ordinary,
            Class::Ordinary,
        ],
        vec![
            Event {
                function: HELPER,
                block: SemanticBlockIdV1::from_index(0),
                statement_count: 0,
                kind: EventKind::Return,
            },
            Event {
                function: ROOT,
                block: SemanticBlockIdV1::from_index(1),
                statement_count: 0,
                kind: EventKind::Call {
                    callee: SemanticCallableIdV1::from_index(0),
                    kind: CallKind::Provider,
                },
            },
        ],
    )
}

fn refuses(ssa: &ProductionSemanticSsaOwnerV1, input: SourceInput<'_>, expected: Error) {
    let roster = launch(ssa);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let error = with_checked_execution_source_v29(ssa, &roster, input, &mut budget, |_, _| {
        panic!("invalid census reached consumer")
    })
    .unwrap_err();
    assert_eq!(error, expected);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.failed_storage().is_none());
}

#[test]
fn complete_source_census_preserves_source_objects_and_exact_resource_bounds() {
    let ssa = fixture(Transport::Copy, 100);
    let roster = launch(&ssa);
    let (classes, events) = rows(&ssa);
    let roots = [input(&ssa)];
    let candidate = SourceInput {
        semantic_sha256: ssa.source_semantic_sha256(),
        roots: &roots,
        classes: &classes,
        events: &events,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let mut visits = 0;
    with_checked_execution_source_v29(&ssa, &roster, candidate, &mut budget, |root, budget| {
        visits += 1;
        assert!(std::ptr::eq(root.semantic_ssa(), &ssa));
        assert_eq!(root.root_id(), ROOT);
        assert_eq!(budget.storage(), FLOOR);
        Ok(())
    })
    .unwrap();
    assert_eq!(visits, 1);
    let exact = budget.work();
    for prior in [0, 11] {
        for limit in [0, exact / 2, exact - 1, exact] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(prior + limit);
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.charge_work(prior).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result =
                with_checked_execution_source_v29(&ssa, &roster, candidate, &mut budget, |_, _| {
                    Ok(())
                });
            if limit == exact {
                result.unwrap();
                assert_eq!(budget.work(), prior + exact);
            } else {
                assert!(matches!(
                    result,
                    Err(Error::Resource(Resource::Work { .. }))
                ));
                assert!((prior..=prior + limit).contains(&budget.work()));
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn complete_source_census_rejects_missing_reordered_and_changed_rows() {
    let ssa = fixture(Transport::Copy, 100);
    let (classes, events) = rows(&ssa);
    let roots = [input(&ssa)];
    let candidate = SourceInput {
        semantic_sha256: ssa.source_semantic_sha256(),
        roots: &roots,
        classes: &classes,
        events: &events,
    };
    let wrong = bytes(255);
    refuses(
        &ssa,
        SourceInput {
            semantic_sha256: &wrong,
            ..candidate
        },
        Error::Source,
    );
    refuses(
        &ssa,
        SourceInput {
            roots: &[],
            ..candidate
        },
        Error::RootCensus,
    );
    refuses(
        &ssa,
        SourceInput {
            roots: &[roots[0], roots[0]],
            ..candidate
        },
        Error::RootCensus,
    );
    refuses(
        &ssa,
        SourceInput {
            classes: &classes[..2],
            ..candidate
        },
        Error::CallableCensus,
    );
    for changed in [
        vec![],
        vec![events[0]],
        vec![events[1]],
        vec![events[1], events[0]],
        vec![events[0], events[0], events[1]],
        vec![events[0], events[1], events[0]],
    ] {
        refuses(
            &ssa,
            SourceInput {
                events: &changed,
                ..candidate
            },
            Error::ScopeEventCensus,
        );
    }
    for index in 0..events.len() {
        for mutation in 0..5 {
            let mut changed = events.clone();
            match mutation {
                0 => changed[index].function = SemanticFunctionIdV1::from_index(99),
                1 => changed[index].block = SemanticBlockIdV1::from_index(99),
                2 => changed[index].statement_count += 1,
                3 => changed[index].kind = EventKind::Abort,
                _ => {
                    changed[index].kind = EventKind::Call {
                        callee: ISSUER,
                        kind: CallKind::Ordinary,
                    }
                }
            }
            refuses(
                &ssa,
                SourceInput {
                    events: &changed,
                    ..candidate
                },
                Error::ScopeEventCensus,
            );
        }
    }
    let mut changed = classes.clone();
    changed[0] = Class::Provider {
        function: HELPER,
        identity: SemanticFunctionIdentityV1::from_sha256(wrong),
    };
    refuses(
        &ssa,
        SourceInput {
            classes: &changed,
            ..candidate
        },
        Error::CallableCensus,
    );
    changed[0] = Class::Ordinary;
    refuses(
        &ssa,
        SourceInput {
            classes: &changed,
            ..candidate
        },
        Error::ScopeEventCensus,
    );
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation_identity,
        ..
    } = &ssa.source_semantic().callables()[2]
    else {
        panic!("issuer");
    };
    changed[2] = Class::Derive {
        binding: binding.identity(),
        operation: *operation_identity,
        context: CONTEXT,
        workgroup: CONTEXT,
    };
    refuses(
        &ssa,
        SourceInput {
            classes: &changed,
            ..candidate
        },
        Error::CallableCensus,
    );
    let mut changed = roots;
    changed[0].helper_arguments = &[];
    refuses(
        &ssa,
        SourceInput {
            roots: &changed,
            ..candidate
        },
        Error::Arguments,
    );
}

#[test]
fn complete_source_census_preserves_consumer_storage_and_failure() {
    let ssa = fixture(Transport::Copy, 100);
    let roster = launch(&ssa);
    let (classes, events) = rows(&ssa);
    let roots = [input(&ssa)];
    let candidate = SourceInput {
        semantic_sha256: ssa.source_semantic_sha256(),
        roots: &roots,
        classes: &classes,
        events: &events,
    };
    for fail in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR + 11);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_checked_execution_source_v29(
            &ssa,
            &roster,
            candidate,
            &mut budget,
            |_, budget| {
                budget.reserve_storage(11)?;
                if fail { Err(Error::Arguments) } else { Ok(()) }
            },
        );
        assert_eq!(result, if fail { Err(Error::Arguments) } else { Ok(()) });
        assert_eq!(budget.storage(), FLOOR + 11);
    }
}
