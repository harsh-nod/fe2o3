use super::*;

mod resource_tests {
    use super::*;
    include!("production_execution_identity_retained_call_resources_v1_tests.rs");
}

thread_local! {
    static CALL_EFFECT_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn with_effect_index(
    mut owner: ProductionSemanticSsaOwnerV1,
    expected_work: Option<usize>,
    consume: impl FnOnce(
        &mut ExecutionIdentitySourceIndexV1<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(37)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let floor = budget.storage();
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = (|| {
                budget.reserve_storage(std::mem::size_of::<
                    ExecutionIdentitySourceIndexV1<'_, '_>,
                >())?;
                let mut index = ExecutionIdentitySourceIndexV1::new(instances, budget)?;
                consume(&mut index, budget)
            })();
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 37);
    drop(budget);
    assert_eq!(work.failed_work(), expected_work);
    result
}

fn function_instance(
    index: &ExecutionIdentitySourceIndexV1<'_, '_>,
    function: u32,
) -> ProductionCallInstanceIdV1 {
    index
        .instances
        .instances()
        .iter()
        .enumerate()
        .find_map(|(ordinal, row)| {
            let id = index.instances.id_at(ordinal).unwrap();
            (row.function().index() == function
                && index.instances.instance_reachable(id) == Some(true))
            .then_some(id)
        })
        .unwrap()
}

fn observe_ignored_borrow(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut count = 0;
    for (ordinal, row) in instances.instances().iter().enumerate() {
        if row.function() != CALLBACK {
            continue;
        }
        let instance = instances.id_at(ordinal).unwrap();
        let actual = emitted[ordinal].as_ref().unwrap();
        assert_eq!(actual.source_call_instance, Some(instance));
        let entry = instances.occurrences(instance).unwrap();
        let original = entry
            .entry_definitions()
            .iter()
            .find(|entry| entry.variable().get() == 1)
            .unwrap();
        assert!(
            original.value().is_none(),
            "original storage classification stays retained"
        );
        let observation = actual.execution_observation.as_ref().unwrap();
        let seed = observation.retained_seeds[1].as_ref().unwrap();
        assert_eq!(seed.role, SemanticExecutionRoleV29::Workgroup);
        assert_eq!(
            instances
                .instance(seed.identity.producer.caller)
                .unwrap()
                .function(),
            HELPER
        );
        let borrow = observation
            .bindings
            .values()
            .find_map(|binding| match binding {
                SemanticValueBindingV1::ExecutionBorrow(binding)
                    if binding.occurrence.instance == instance =>
                {
                    Some(binding)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(&borrow.borrowed, seed);
        assert_eq!(borrow.source_local.index(), 1);
        assert_eq!(borrow.kind, SemanticBorrowKindV1::Shared);
        let call = instances
            .calls(instance)
            .unwrap()
            .iter()
            .find(|call| call.child().is_some())
            .unwrap();
        let child = call.child().unwrap();
        assert_eq!(instances.instance(child).unwrap().function().index(), 3);
        assert_eq!(
            emitted[child.index()]
                .as_ref()
                .unwrap()
                .source_call_instance,
            Some(child)
        );
        count += 1;
    }
    assert_eq!(count, 1);
    CALL_EFFECT_OBSERVED.set(count);
    Ok(())
}

pub(super) fn prove_ignored_borrow_callee() {
    CALL_EFFECT_OBSERVED.set(0);
    let result = run_suffix_owner(
        retained_call_escape_owner,
        observe_ignored_borrow,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(CALL_EFFECT_OBSERVED.get(), 1);
}

#[test]
fn retained_call_closure_uses_original_bodies_before_any_identity_equations() {
    use scoped_root_tests::fixtures::{FreshReturnCaseV1, fresh_tile_return_owner_v1};
    for case in [
        FreshReturnCaseV1::Borrowed,
        FreshReturnCaseV1::SharedTarget,
        FreshReturnCaseV1::Straight,
        FreshReturnCaseV1::TwoExits,
        FreshReturnCaseV1::Loop,
        FreshReturnCaseV1::Nested,
        FreshReturnCaseV1::Mixed,
        FreshReturnCaseV1::NoNormalReturn,
    ] {
        let mut observed = false;
        with_effect_index(fresh_tile_return_owner_v1(case), None, |index, budget| {
            assert!(index.retained.is_empty());
            let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
            assert_eq!(effects.closed.len(), index.instances.instances().len());
            for ordinal in 0..effects.closed.len() {
                let instance = index.instances.id_at(ordinal).unwrap();
                if index.instances.instance_reachable(instance) == Some(false) {
                    assert!(!effects.closed[ordinal]);
                    assert!(!effects.accepts(index.instances, instance, budget)?);
                }
            }
            for function in [2, 3, 4] {
                let instance = function_instance(index, function);
                let expected = case != FreshReturnCaseV1::Mixed || function == 4;
                assert_eq!(
                    effects.accepts(index.instances, instance, budget)?, expected,
                    "{case:?} function={function}"
                );
            }
            if case == FreshReturnCaseV1::Mixed {
                let child = index.instances.instance(function_instance(index, 4)).unwrap();
                let construction = child.declaration().blocks()[2].statements()[0].kind();
                assert!(matches!(construction, SemanticStatementKindV1::Assign(assignment)
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
                        if aggregate.operands().len() == 2 && aggregate.operands().iter().all(|operand|
                            matches!(operand, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                                if place.projections().is_empty())))));
                let caller = index.instances.instance(function_instance(index, 3)).unwrap();
                let extraction = caller.declaration().blocks()[1].statements()[0].kind();
                assert!(matches!(extraction, SemanticStatementKindV1::Assign(assignment)
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                        if matches!(place.projections()[0].kind(), SemanticProjectionKindV1::Field(0)))));
            }
            assert!(
                index.retained.is_empty(),
                "effect closure must not install a seed or solve identities"
            );
            if matches!(
                case,
                FreshReturnCaseV1::SharedTarget | FreshReturnCaseV1::NoNormalReturn
            ) {
                let child = function_instance(index, 4);
                assert_eq!(index.instances.instance_may_return(child), Some(false));
                let incoming = index.instances.incoming(child).unwrap();
                assert_eq!(
                    index.instances.call_control(incoming.occurrence()),
                    Some(ProductionCallControlV1::NoNormalReturn)
                );
            }
            effects.discard(budget)?;
            observed = true;
            Ok(())
        })
        .unwrap();
        assert!(observed);
    }
}

#[test]
fn missing_or_foreign_retained_call_proof_cannot_certify_equal_ordinal_calls() {
    let mut observed = 0;
    with_effect_index(retained_call_escape_owner(), None, |index, budget| {
        let caller = function_instance(index, CALLBACK.index());
        let call = index
            .instances
            .calls(caller)
            .unwrap()
            .iter()
            .find(|call| call.child().is_some())
            .unwrap();
        let missing = vec![false; index.instances.instances().len()];
        assert!(!execution_identity_retained_call_v1(
            index,
            &missing,
            caller,
            SsaBlockIdV1::new(call.occurrence().block.index()),
            call.source(),
            budget
        )?);
        let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
        assert!(effects.accepts(index.instances, caller, budget)?);
        let copy = call.source().clone();
        assert!(matches!(
            execution_identity_retained_call_v1(
                index,
                &effects.closed,
                caller,
                SsaBlockIdV1::new(call.occurrence().block.index()),
                &copy,
                budget
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        let mut other_owner = retained_call_escape_owner();
        let capture = other_owner
            .try_capture_occurrences_with_budget_v1(budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage())?;
        production_call_instances_v1::with_production_call_instances_v1(
            &other_owner,
            ROOT,
            budget,
            |other, budget| {
                assert_eq!(other.id_at(caller.index()), Some(caller));
                assert!(matches!(
                    effects.accepts(other, caller, budget),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                observed += 1;
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        drop(other_owner);
        budget.release_storage(capture.retained_storage())?;
        effects.discard(budget)?;
        observed += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(observed, 2);
}

#[test]
fn retained_call_lookup_refuses_missing_replaced_and_cross_instance_rows() {
    for fault in 0..3 {
        let mut observed = false;
        with_effect_index(retained_call_escape_owner(), None, |index, budget| {
            let caller = function_instance(index, CALLBACK.index());
            let call = index
                .instances
                .calls(caller)
                .unwrap()
                .iter()
                .find(|call| call.child().is_some())
                .unwrap();
            let key = (caller.index(), call.occurrence().block.index());
            let saved = index.calls[&key];
            let closed = vec![true; index.instances.instances().len()];
            let result = match fault {
                0 => {
                    index.calls.remove(&key);
                    execution_identity_retained_call_v1(
                        index,
                        &closed,
                        caller,
                        SsaBlockIdV1::new(key.1),
                        call.source(),
                        budget,
                    )
                }
                1 => {
                    index.calls.insert(key, usize::MAX);
                    execution_identity_retained_call_v1(
                        index,
                        &closed,
                        caller,
                        SsaBlockIdV1::new(key.1),
                        call.source(),
                        budget,
                    )
                }
                _ => {
                    let sibling = function_instance(index, HELPER.index());
                    execution_identity_retained_call_v1(
                        index,
                        &closed,
                        sibling,
                        SsaBlockIdV1::new(key.1),
                        call.source(),
                        budget,
                    )
                }
            };
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "{result:?}"
            );
            index.calls.insert(key, saved);
            observed = true;
            Ok(())
        })
        .unwrap();
        assert!(observed);
    }
}

fn harmful_callee_owner(address: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = retained_call_escape_owner();
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let mut functions = source.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    let mut statements = helper.blocks()[0].statements().to_vec();
    if address {
        let raw = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
        let temporary = locals.len() as u32;
        locals.push(local(178, raw, SemanticLocalRoleV1::Temporary));
        statements.push(assign(
            place(temporary, raw),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(0, U32),
            },
        ));
    } else {
        // An actual memory Store, not a metadata flag or an unused callee name.
        statements.push(store(place(0, U32), literal(11)));
    }
    let body = block(177, statements, SemanticTerminatorKindV1::Return);
    functions[3] = function(174, helper.role(), helper.abi().clone(), locals, vec![body]);
    assert_eq!(&functions[..3], &source.functions()[..3]);
    retained_rebuild(types, functions, source.callables().to_vec())
}

#[test]
fn real_memory_write_and_address_observation_refuse_transitive_retained_call_closure() {
    for address in [false, true] {
        let mut observed = false;
        with_effect_index(harmful_callee_owner(address), None, |index, budget| {
            let child = function_instance(index, 3);
            let original = index.instances.instance(child).unwrap().declaration();
            assert!(matches!(original.blocks()[0].statements()[1].kind(),
                SemanticStatementKindV1::Store(_)) || matches!(original.blocks()[0].statements()[1].kind(),
                SemanticStatementKindV1::Assign(assignment) if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { .. })));
            let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
            assert!(!effects.accepts(index.instances, child, budget)?);
            assert!(!effects.accepts(index.instances, function_instance(index, CALLBACK.index()), budget)?);
            effects.discard(budget)?;
            observed = true;
            Ok(())
        }).unwrap();
        assert!(observed);
    }
}
