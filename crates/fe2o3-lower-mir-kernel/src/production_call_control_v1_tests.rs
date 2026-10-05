use super::*;

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn check_control(
    owner: &ProductionSemanticSsaOwnerV1,
    check: impl FnOnce(&ProductionCallInstancePlanV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut completed = false;
    with_production_call_instances_v1(owner, ROOT, &mut budget, |plan, _| {
        check(plan);
        completed = true;
        Ok::<_, Error>(())
    })
    .unwrap();
    assert!(completed);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn call_control_preserves_repeated_returning_instances_and_source_coordinates() {
    check_control(&fixture(Case::Ordinary, true), |plan| {
        for (index, row) in plan.instances().iter().enumerate() {
            let instance = plan.id_at(index).unwrap();
            assert_eq!(plan.instance_reachable(instance), Some(true));
            assert_eq!(plan.instance_may_return(instance), Some(true));
            for block in 0..row.declaration().blocks().len() {
                assert_eq!(
                    plan.block_reachable(instance, SemanticBlockIdV1::from_index(block as u32)),
                    Some(true)
                );
            }
        }
        for call in plan.calls(plan.root()).unwrap() {
            assert_eq!(
                plan.call_control(call.occurrence()),
                Some(ProductionCallControlV1::MayReturn)
            );
        }
        assert_eq!(
            plan.call_control(ProductionCallOccurrenceV1 {
                caller: plan.root(),
                block: SemanticBlockIdV1::from_index(2)
            }),
            None
        );
        assert_eq!(
            plan.block_reachable(plan.root(), SemanticBlockIdV1::from_index(3)),
            None
        );
    });
}

#[test]
fn call_control_nonreturning_helper_does_not_fabricate_a_caller_return_or_visit_dead_instances() {
    for case in [Case::Loop, Case::Abort] {
        check_control(&fixture(case, true), |plan| {
            let root = plan.root();
            assert_eq!(plan.instance_reachable(root), Some(true));
            assert_eq!(plan.instance_may_return(root), Some(false));
            assert_eq!(
                plan.returns(root).unwrap().count(),
                1,
                "syntactic roster must remain intact"
            );
            assert_eq!(
                plan.block_reachable(root, SemanticBlockIdV1::from_index(0)),
                Some(true)
            );
            for block in [1, 2] {
                assert_eq!(
                    plan.block_reachable(root, SemanticBlockIdV1::from_index(block)),
                    Some(false)
                );
            }
            let calls = plan.calls(root).unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(
                plan.call_control(calls[0].occurrence()),
                Some(ProductionCallControlV1::NoNormalReturn)
            );
            assert_eq!(
                plan.call_control(calls[1].occurrence()),
                Some(ProductionCallControlV1::Unreachable)
            );
            for (ordinal, call) in calls.iter().enumerate() {
                let child = call.child().unwrap();
                assert_eq!(plan.instance_reachable(child), Some(ordinal == 0));
                assert_eq!(plan.instance_may_return(child), Some(false));
                for nested in plan.calls(child).unwrap() {
                    let leaf = nested.child().unwrap();
                    assert_eq!(plan.instance_reachable(leaf), Some(ordinal == 0));
                    assert_eq!(plan.instance_may_return(leaf), Some(true));
                }
            }
        });
    }
}

#[test]
fn call_control_unknown_conditions_keep_normal_return_and_duplicate_edges() {
    for otherwise in [1, 2] {
        let owner = fixture_with(Case::Ordinary, true, |functions| {
            let old = &functions[1];
            functions[1] = function(
                50,
                old.role(),
                old.abi().clone(),
                old.locals().to_vec(),
                vec![
                    block(
                        70,
                        vec![],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(1, U32)),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
                            )
                            .unwrap(),
                        },
                    ),
                    block(71, vec![], SemanticTerminatorKindV1::Return),
                    block(
                        72,
                        vec![],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
                    ),
                ],
            );
        });
        check_control(&owner, |plan| {
            assert_eq!(plan.instance_may_return(plan.root()), Some(true));
            for call in plan.calls(plan.root()).unwrap() {
                assert_eq!(
                    plan.call_control(call.occurrence()),
                    Some(ProductionCallControlV1::MayReturn)
                );
                let child = call.child().unwrap();
                assert_eq!(plan.instance_may_return(child), Some(true));
                assert_eq!(
                    plan.block_reachable(child, SemanticBlockIdV1::from_index(2)),
                    Some(otherwise == 2)
                );
            }
        });
    }
}

#[test]
fn call_control_no_normal_return_preserves_the_distinct_unwind_edge() {
    let owner = fixture_with(Case::Abort, true, |functions| {
        let old = &functions[0];
        let SemanticTerminatorKindV1::Call(original) = old.blocks()[0].terminator().kind() else {
            unreachable!()
        };
        let call = SemanticDirectCallV1::new_callable(
            original.callee(),
            original.arguments().to_vec(),
            original.destination().cloned(),
            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 2)),
        )
        .unwrap();
        functions[0] = function(
            20,
            old.role(),
            old.abi().clone(),
            old.locals().to_vec(),
            vec![
                block(40, vec![], SemanticTerminatorKindV1::Call(call)),
                old.blocks()[1].clone(),
                old.blocks()[2].clone(),
            ],
        )
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    });
    check_control(&owner, |plan| {
        let root = plan.root();
        assert_eq!(plan.instance_may_return(root), Some(true));
        assert_eq!(
            plan.block_reachable(root, SemanticBlockIdV1::from_index(1)),
            Some(false)
        );
        assert_eq!(
            plan.block_reachable(root, SemanticBlockIdV1::from_index(2)),
            Some(true)
        );
        let calls = plan.calls(root).unwrap();
        let first = calls
            .iter()
            .find(|call| call.occurrence().block.index() == 0)
            .unwrap();
        assert_eq!(
            plan.call_control(first.occurrence()),
            Some(ProductionCallControlV1::NoNormalReturn)
        );
    });
}

#[test]
fn call_control_limits_are_exact_for_dead_suffixes_and_nested_loops() {
    for case in [Case::Ordinary, Case::Loop, Case::Abort] {
        let owner = fixture(case, true);
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = with_production_call_instances_v1(&owner, ROOT, &mut budget, |_, _| {
                Ok::<_, Error>(())
            });
            assert_eq!(budget.storage(), FLOOR);
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, storage) = run(usize::MAX, usize::MAX);
        result.unwrap();
        run(work, storage).0.unwrap();
        assert!(matches!(
            run(work - 1, storage).0,
            Err(Error::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            run(work, storage - 1).0,
            Err(Error::Resource(ResourceError::Storage(_)))
        ));
    }
}
