use super::*;
use crate::CanonicalFormalSourceScopeV20;

fn with_scope<'owner>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    consume: impl FnOnce(
        &CanonicalFormalSourceScopeV20<'_, 'owner>,
        &mut Budget<'_>,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut result = None;
    let mut completed = false;
    let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        let floor = budget.storage();
        result = Some(report(
            owner,
            0,
            effects,
            Launch::Exact(launch(8)),
            FormalIndexWidth::Bits64,
            ControlFlowLimits::DEFAULT,
            budget,
            |view, budget| {
                let scope = view.source_scope_v20(budget)?;
                assert!(std::ptr::eq(view.analysis(), scope.report()));
                consume(scope, budget)
            },
        ));
        assert_eq!(budget.storage(), floor);
        completed = true;
        Ok(())
    });
    assert!(completed, "all report settlement assertions must complete");
    assert_eq!(budget.storage(), FLOOR);
    let result = result.unwrap();
    if result.is_ok() {
        assert_eq!(outer, Ok(()));
    }
    result
}

#[test]
fn retained_source_scope_uses_original_sparse_cfg_and_dense_definition_coordinates() {
    let input = diamond(false, true);
    with_owner(&input, |owner| {
        let expected = legacy(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Exact,
        );
        let mut completed = false;
        assert_eq!(
            with_scope(owner, |scope, budget| {
                assert!(std::ptr::eq(scope.original_owner(), owner));
                assert!(std::ptr::eq(
                    scope.original_function(),
                    &owner.module().functions[0]
                ));
                assert_eq!(scope.report(), &expected);
                assert_eq!(scope.root_index(), 0);
                assert_eq!(scope.launch_input(), Launch::Exact(launch(8)));
                assert_eq!(scope.index_width(), FormalIndexWidth::Bits64);
                let blocks = &scope.original_function().body.as_ref().unwrap().blocks;
                assert_eq!(scope.block_count(budget)?, blocks.len());
                for (ordinal, block) in blocks.iter().enumerate() {
                    assert!(std::ptr::eq(
                        scope.block_at(ordinal, budget)?.unwrap(),
                        block
                    ));
                    assert_eq!(scope.block_ordinal(block.id, budget)?, Some(ordinal));
                    assert!(scope.reachable(block.id, budget)?);
                }
                assert!(scope.block_at(blocks.len(), budget)?.is_none());
                assert_eq!(scope.block_ordinal(BlockId(88), budget)?, None);
                assert!(!scope.reachable(BlockId(88), budget)?);
                assert_eq!(scope.definition_count(budget)?, 8);
                for (ordinal, id) in [0, 1, 2, 3, 10, 11, 12, 20].into_iter().enumerate() {
                    let (actual, _, operation) = scope.definition(ValueId(id), budget)?.unwrap();
                    assert_eq!(actual, ordinal);
                    assert_eq!(operation.is_some(), matches!(id, 10 | 11 | 12));
                    if let Some(operation) = operation {
                        assert_eq!(operation.results[0].id, ValueId(id));
                    }
                }
                assert!(scope.definition(ValueId(999), budget)?.is_none());
                assert!(scope.unique_predecessor_dominates(
                    BlockId(0),
                    BlockId(90),
                    BlockId(90),
                    budget
                )?);
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(0),
                    BlockId(90),
                    BlockId(7000),
                    budget
                )?);
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(90),
                    BlockId(7000),
                    BlockId(7000),
                    budget
                )?);
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(90),
                    BlockId(0),
                    BlockId(90),
                    budget
                )?);
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(999),
                    BlockId(90),
                    BlockId(90),
                    budget
                )?);
                completed = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(completed);
    });
}

#[test]
fn retained_source_scope_never_invents_dead_or_multiple_result_producers() {
    let mut input = straight();
    let body = input.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(Operation::new(
        vec![
            ValueDef::new(ValueId(600), Type::INDEX),
            ValueDef::new(ValueId(601), Type::INDEX),
        ],
        OperationKind::Call {
            callee: FunctionId::new("pair"),
            arguments: vec![],
        },
    ));
    let mut dead = BasicBlock::new(BlockId(9999));
    dead.operations.push(index(900, 17));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(dead);
    input.functions.push(Function::declaration(
        "pair",
        Signature::new(vec![], vec![Type::INDEX, Type::INDEX]),
    ));
    for function in &mut input.functions {
        function.required_capabilities = function.derived_capabilities();
        input
            .required_capabilities
            .extend(function.required_capabilities.iter().cloned());
    }
    with_owner(&input, |owner| {
        let expected = legacy(
            owner,
            launch(8),
            FormalIndexWidth::Bits64,
            Interpretation::Exact,
        );
        let mut completed = false;
        assert_eq!(
            with_scope(owner, |scope, budget| {
                assert_eq!(scope.report(), &expected);
                assert!(!scope.reachable(BlockId(9999), budget)?);
                for id in [0, 600, 601, 900] {
                    let (_, ty, operation) = scope.definition(ValueId(id), budget)?.unwrap();
                    assert!(operation.is_none());
                    if id != 0 {
                        assert_eq!(ty, &Type::INDEX);
                    }
                }
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(9999),
                    BlockId(0),
                    BlockId(0),
                    budget
                )?);
                completed = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(completed);
    });
}

#[test]
fn retained_source_scope_cfg_query_is_structural_not_conditional_truth() {
    for duplicate_targets in [false, true] {
        let mut entry = BasicBlock::new(BlockId(90));
        entry.terminator = if duplicate_targets {
            Some(Terminator::ConditionalBranch {
                condition: ValueId(2),
                then_target: BlockId(0),
                then_arguments: vec![],
                else_target: BlockId(0),
                else_arguments: vec![],
            })
        } else {
            branch(0, &[])
        };
        let mut exit = BasicBlock::new(BlockId(0));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        with_owner(&module(vec![entry, exit]), |owner| {
            let mut completed = false;
            assert_eq!(
                with_scope(owner, |scope, budget| {
                    // Distinct predecessor count is one even for duplicate branch
                    // targets. This structural result cannot prove either truth arm.
                    assert!(scope.unique_predecessor_dominates(
                        BlockId(90),
                        BlockId(0),
                        BlockId(0),
                        budget
                    )?);
                    assert!(!scope.unique_predecessor_dominates(
                        BlockId(0),
                        BlockId(90),
                        BlockId(0),
                        budget
                    )?);
                    assert!(!scope.unique_predecessor_dominates(
                        BlockId(90),
                        BlockId(0),
                        BlockId(999),
                        budget
                    )?);
                    completed = true;
                    Ok(())
                }),
                Ok(())
            );
            assert!(completed);
        });
    }
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = branch(90, &[]);
    let mut loop_block = BasicBlock::new(BlockId(90));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(90),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(3));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    with_owner(&module(vec![entry, loop_block, exit]), |owner| {
        let mut completed = false;
        assert_eq!(
            with_scope(owner, |scope, budget| {
                assert!(!scope.unique_predecessor_dominates(
                    BlockId(0),
                    BlockId(90),
                    BlockId(3),
                    budget
                )?);
                completed = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(completed);
    });
}

#[test]
fn retained_source_scope_independent_queries_are_logarithmic_without_new_backing() {
    for count in [1_u32, 16, 64, 256] {
        let mut input = straight();
        for ordinal in 0..count {
            input.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(index(1000 + ordinal * 97, u64::from(ordinal)));
        }
        with_owner(&input, |owner| {
            let mut completed = false;
            assert_eq!(
                with_scope(owner, |scope, budget| {
                    let definitions = scope.definition_count(budget)?;
                    assert_eq!(definitions, count as usize + 8);
                    let logarithm = (usize::BITS - definitions.leading_zeros()) as usize;
                    let start = (budget.work(), budget.storage(), budget.peak_storage());
                    for ordinal in 0..count {
                        let (_, _, operation) = scope
                            .definition(ValueId(1000 + ordinal * 97), budget)?
                            .unwrap();
                        assert!(
                            matches!(operation.unwrap().kind, OperationKind::Constant(Constant::Index(value)) if value == u64::from(ordinal))
                        );
                    }
                    // Two bounded definition searches, one single-block search,
                    // one retained CFG lookup and fixed identity/row steps.
                    assert!(budget.work() - start.0 <= count as usize * (2 * logarithm + 16));
                    assert_eq!(
                        (budget.storage(), budget.peak_storage()),
                        (start.1, start.2)
                    );
                    completed = true;
                    Ok(())
                }),
                Ok(())
            );
            assert!(completed);
        });
    }
}

#[test]
fn retained_source_scope_same_account_owner_or_root_failure_is_sticky() {
    let input = straight();
    with_owner(&input, |owner| {
        with_owner(&input, |foreign| {
            for wrong_owner in [false, true] {
                let mut completed = false;
                let result = with_scope(owner, |scope, budget| {
                    let error = scope
                        .check(
                            if wrong_owner { foreign } else { owner },
                            usize::from(!wrong_owner),
                            budget,
                        )
                        .unwrap_err();
                    assert_eq!(error, Error::Resource(ResourceError::Accounting));
                    let work = budget.work();
                    assert_eq!(scope.check(owner, 0, budget), Err(error.clone()));
                    assert_eq!(scope.definition_count(budget), Err(error.clone()));
                    assert_eq!(budget.work(), work);
                    completed = true;
                    Ok(())
                });
                assert!(completed);
                assert_eq!(result, Err(Error::Resource(ResourceError::Accounting)));
            }
        })
    });
}

#[test]
fn retained_source_scope_foreign_budget_does_not_poison_original_account() {
    with_owner(&straight(), |owner| {
        let mut completed = false;
        assert_eq!(
            with_scope(owner, |scope, budget| {
                let original = (budget.work(), budget.storage());
                let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut foreign = Budget::new(&mut work, 0);
                assert!(foreign.charge_work(1).is_err());
                assert_eq!(
                    scope.block_count(&mut foreign),
                    Err(Error::Resource(ResourceError::Accounting))
                );
                assert_eq!((budget.work(), budget.storage()), original);
                assert_eq!(scope.block_count(budget)?, 1);
                scope.check(owner, 0, budget)?;
                completed = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(completed);
    });
}

#[test]
fn retained_source_scope_original_work_or_storage_denial_precedes_restored_floor() {
    with_owner(&straight(), |owner| {
        for storage in [false, true] {
            let mut completed = false;
            let mut expected = None;
            let result = with_scope(owner, |scope, budget| {
                let first = if storage {
                    budget.reserve_storage(LIMIT)
                } else {
                    budget.charge_work(LIMIT)
                }
                .unwrap_err();
                let error = Error::from(first);
                let (work, floor) = (budget.work(), budget.storage());
                budget.release_storage(1).unwrap();
                assert_eq!(budget.storage(), floor - 1);
                assert_eq!(scope.block_count(budget), Err(error.clone()));
                budget.reserve_storage(1).unwrap();
                assert_eq!(scope.check(owner, 0, budget), Err(error.clone()));
                assert_eq!(budget.work(), work);
                expected = Some(error);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert_eq!(result, Err(expected.unwrap()));
        }
    });
}

#[test]
fn retained_source_scope_report_floor_refusal_cannot_be_repaired_and_forgotten() {
    with_owner(&store_fixture(2, false), |owner| {
        let mut completed = false;
        let result = with_scope(owner, |scope, budget| {
            let floor = budget.storage();
            budget.release_storage(1).unwrap();
            assert_eq!(
                scope.block_count(budget),
                Err(Error::Resource(ResourceError::Accounting))
            );
            budget.reserve_storage(1).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                scope.check(owner, 0, budget),
                Err(Error::Resource(ResourceError::Accounting))
            );
            completed = true;
            Ok(())
        });
        assert!(completed);
        assert_eq!(result, Err(Error::Resource(ResourceError::Accounting)));
    });
}

#[test]
fn retained_source_scope_two_unit_block_count_has_exact_and_one_short_query_cuts() {
    with_owner(&straight(), |owner| {
        for remaining in [2, 1] {
            let mut completed = false;
            let result = with_scope(owner, |scope, budget| {
                // Test-owned padding isolates the two independent fixed charges:
                // one query entry, then one retained block-count observation.
                budget
                    .charge_work(LIMIT - budget.work() - remaining)
                    .unwrap();
                let observed = scope.block_count(budget);
                if remaining == 2 {
                    assert_eq!(observed, Ok(1));
                } else {
                    assert!(
                        matches!(observed, Err(Error::Resource(ResourceError::Work(error))) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
                    );
                }
                assert_eq!(budget.work(), LIMIT);
                let refusal = scope.block_count(budget).unwrap_err();
                assert!(
                    matches!(refusal, Error::Resource(ResourceError::Work(error)) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
                );
                assert_eq!(budget.work(), LIMIT);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert!(
                matches!(result, Err(Error::Resource(ResourceError::Work(error))) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
            );
        }
    });
}

fn source_run(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), Error>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut observed = None;
    let mut entered = false;
    let mut settled = false;
    let mut queried = false;
    let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        entered = true;
        let floor = budget.storage();
        observed = Some(report(
            owner,
            0,
            effects,
            Launch::Exact(launch(8)),
            FormalIndexWidth::Bits64,
            ControlFlowLimits::DEFAULT,
            budget,
            |view, budget| {
                let scope = view.source_scope_v20(budget)?;
                scope.check(owner, 0, budget)?;
                for _ in 0..3 {
                    assert_eq!(scope.block_count(budget)?, 1);
                    assert_eq!(scope.definition(ValueId(10), budget)?.unwrap().0, 4);
                    assert!(!scope.unique_predecessor_dominates(
                        BlockId(0),
                        BlockId(0),
                        BlockId(0),
                        budget
                    )?);
                }
                queried = true;
                Ok(())
            },
        ));
        assert_eq!(budget.storage(), floor);
        settled = true;
        Ok(())
    });
    assert!(!entered || settled, "post-denial settlement must complete");
    assert_eq!(budget.storage(), FLOOR);
    let result = match (observed, outer) {
        (_, Err(error)) => Err(Error::Effects(error)),
        (Some(result), Ok(())) => result,
        (None, Ok(())) => panic!("missing source report attempt"),
    };
    (result, budget.work(), budget.peak_storage(), queried)
}

#[test]
fn retained_source_scope_complete_query_path_has_exact_and_one_short_shared_limits() {
    with_owner(&straight(), |owner| {
        let full = source_run(owner, LIMIT, LIMIT);
        assert_eq!(full.0, Ok(()));
        assert!(full.3);
        let exact = source_run(owner, full.1, full.2);
        assert_eq!(exact.0, Ok(()));
        assert!(exact.3);
        assert_eq!((exact.1, exact.2), (full.1, full.2));
        let work = source_run(owner, full.1 - 1, full.2);
        assert!(
            matches!(work.0, Err(Error::Effects(CanonicalEffectErrorV19::Resource(Resource::Work(error)))) if error.actual() > error.limit() && error.limit() == full.1 - 1)
        );
        let storage = source_run(owner, full.1, full.2 - 1);
        assert!(
            matches!(storage.0, Err(Error::Effects(CanonicalEffectErrorV19::Resource(Resource::Storage(error)))) if error.actual() > error.limit() && error.limit() == full.2 - 1)
        );
    });
}
