use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

// Shared producer tests also exercise untrusted proposals directly.

fn relation_rows(
    owner: &ProductionPreRankedKirOwnerV1,
) -> (&SemanticKirFunctionCorrespondenceV1, ArgumentTraceV1<'_>) {
    let association = owner
        .correspondence
        .lowered_functions()
        .iter()
        .find(|row| {
            row.correspondence_owner().index() == 1
                && row.role() == SemanticKirFunctionRoleV1::KernelEntry
        })
        .unwrap();
    let same = |root, function| {
        root == association.correspondence_owner() && function == association.semantic_function()
    };
    (
        association,
        ArgumentTraceV1 {
            direct: argument_group_v1(&owner.correspondence.parameter_bindings, |row| {
                same(row.correspondence_owner, row.semantic_function)
            }),
            components: argument_group_v1(
                &owner.correspondence.parameter_component_bindings,
                |row| same(row.correspondence_owner, row.semantic_function),
            ),
            ignored: argument_group_v1(&owner.correspondence.ignored_parameter_bindings, |row| {
                same(row.correspondence_owner, row.semantic_function)
            }),
        },
    )
}

#[test]
fn shared_relation_preserves_nonphysical_source_ordinal_and_rechecks_whole_identity() {
    use fe2o3_pliron::ProductionSourceArgumentErrorV1 as E;
    let owner = materialize_argument_view(shifted_source());
    let root = SemanticFunctionIdV1::from_index(1);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let floor = owner.retained_analysis_storage_v1() + 31;
    budget.reserve_storage(floor).unwrap();
    let relation = owner
        .checked_source_argument_relation_v1(root, root, &mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    let values = parameters(&owner);
    let binding = relation
        .bind_whole_parameter_v1(4, values[4], &mut budget)
        .unwrap();
    assert_eq!(binding.canonical_parameter(), 4);
    assert_eq!(binding.canonical_value(), values[4]);
    assert_eq!(binding.source_argument(), 3);
    assert_eq!(binding.adjusted_argument(), 3);
    assert_eq!(binding.semantic_local().index(), 1);
    assert_eq!(binding.semantic_type(), U32);
    relation.require_binding_v1(&binding, &mut budget).unwrap();
    for (slot, value) in values
        .iter()
        .copied()
        .enumerate()
        .take(4)
        .map(|(slot, value)| (slot as u32, value))
        .chain([(4, values[0]), (5, values[4]), (u32::MAX, values[4])])
    {
        assert!(matches!(
            relation.bind_whole_parameter_v1(slot, value, &mut budget),
            Err(E::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn shared_relation_requires_original_owner_subject_and_ledger() {
    use fe2o3_pliron::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let other = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let root = SemanticFunctionIdV1::from_index(1);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let floor = owner.retained_analysis_storage_v1() + 41;
    budget.reserve_storage(floor).unwrap();
    let relation = owner
        .checked_source_argument_relation_v1(root, root, &mut budget)
        .unwrap();
    let binding = relation
        .bind_whole_parameter_v1(0, parameters(&owner)[0], &mut budget)
        .unwrap();
    relation
        .require_source_owner_v1(&owner.semantic_ssa, &mut budget)
        .unwrap();
    assert!(matches!(
        relation.require_source_owner_v1(&other.semantic_ssa, &mut budget),
        Err(E::CorrespondenceMismatch)
    ));
    assert!(matches!(
        relation.replay_v1(
            other.executable().module(),
            relation.canonical_function(),
            &mut budget
        ),
        Err(E::CorrespondenceMismatch)
    ));
    let other_function = other
        .executable()
        .module()
        .function(relation.association().kernel_ir_function())
        .unwrap();
    assert!(matches!(
        relation.replay_v1(relation.canonical_module(), other_function, &mut budget),
        Err(E::CorrespondenceMismatch)
    ));
    let mut other_work = Work::new(usize::MAX);
    let mut other_budget = ArgumentBudgetV1::new(&mut other_work, usize::MAX);
    other_budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        relation.replay_v1(
            relation.canonical_module(),
            relation.canonical_function(),
            &mut other_budget
        ),
        Err(E::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Accounting
        ))
    ));
    let other_relation = owner
        .checked_source_argument_relation_v1(root, root, &mut other_budget)
        .unwrap();
    assert!(matches!(
        other_relation.require_binding_v1(&binding, &mut other_budget),
        Err(E::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Accounting
        ))
    ));
    relation.require_binding_v1(&binding, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(other_budget.storage(), floor);
}

#[test]
fn shared_relation_replays_within_the_original_owned_ledger_view() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let owner = materialize_argument_view(shifted_source());
    let root = SemanticFunctionIdV1::from_index(1);
    let floor = owner.retained_analysis_storage_v1() + 41;
    let mut ledger = Owned::new(Work::new(usize::MAX), usize::MAX);
    ledger.with_budget(|budget| budget.reserve_storage(floor).unwrap());
    let check = |budget: &mut ArgumentBudgetV1<'_>| {
        let relation = owner
            .checked_source_argument_relation_v1(root, root, budget)
            .unwrap();
        let binding = relation
            .bind_whole_parameter_v1(4, parameters(&owner)[4], budget)
            .unwrap();
        budget.charge_work(1).unwrap();
        relation.require_binding_v1(&binding, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        binding.source_argument()
    };
    assert_eq!(ledger.with_budget(check), 3);
    let first_work = ledger.work();
    let peak = ledger.peak_storage();
    assert!(first_work > 0);
    assert!(peak > floor);
    assert_eq!(ledger.with_budget(check), 3);
    assert_eq!(ledger.work(), first_work * 2);
    assert_eq!(ledger.storage(), floor);
    assert_eq!(ledger.peak_storage(), peak);
}

#[test]
fn public_shared_producer_rejects_same_typed_input_swaps_and_incomplete_trace() {
    use fe2o3_pliron::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let (association, trace) = relation_rows(&owner);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let floor = owner.retained_analysis_storage_v1() + 43;
    budget.reserve_storage(floor).unwrap();
    let produce = |direct: &[SemanticKirParameterBindingV1],
                   components: &[SemanticKirParameterComponentBindingV1],
                   ignored: &[SemanticKirIgnoredParameterBindingV1],
                   association: &SemanticKirFunctionCorrespondenceV1,
                   budget: &mut ArgumentBudgetV1<'_>| {
        owner
            .semantic_ssa
            .check_source_arguments_v1(
                owner.executable.verified_module_ref_v1(),
                association,
                ArgumentTraceV1 {
                    direct,
                    components,
                    ignored,
                },
                budget,
            )
            .map(|_| ())
    };
    assert_eq!(trace.direct.len(), 1);
    assert_eq!(trace.components.len(), 4);
    assert_eq!(
        trace.components[0].semantic_component_type,
        trace.components[2].semantic_component_type
    );
    assert_ne!(
        trace.components[0].semantic_local,
        trace.components[2].semantic_local
    );
    produce(
        trace.direct,
        trace.components,
        trace.ignored,
        association,
        &mut budget,
    )
    .unwrap();
    // Proposal ordering is inert: reordering complete rows is not a substitution.
    let mut reordered = trace.components.to_vec();
    reordered.swap(0, 2);
    produce(
        trace.direct,
        &reordered,
        trace.ignored,
        association,
        &mut budget,
    )
    .unwrap();
    for mutation in 0..8 {
        let mut direct = trace.direct.to_vec();
        let mut components = trace.components.to_vec();
        let mut ignored = trace.ignored.to_vec();
        let mut proposed = association.clone();
        match mutation {
            0 => {
                let value = components[0].kernel_ir_value;
                components[0].kernel_ir_value = components[2].kernel_ir_value;
                components[2].kernel_ir_value = value;
            }
            1 => {
                let first = components[0].semantic_local;
                let second = components[2].semantic_local;
                for row in &mut components {
                    row.semantic_local = if row.semantic_local == first {
                        second
                    } else {
                        first
                    };
                }
            }
            2 => {
                std::mem::swap(
                    &mut direct[0].kernel_ir_value,
                    &mut components[0].kernel_ir_value,
                );
            }
            3 => {
                ignored.clear();
            }
            4 => {
                components[0].projection[0] = SemanticKirParameterProjectionV1::Field(2);
            }
            5 => {
                proposed.correspondence_owner = SemanticFunctionIdV1::from_index(0);
            }
            6 => {
                proposed.role = SemanticKirFunctionRoleV1::InternalHelper;
            }
            _ => {
                components.push(components[0].clone());
            }
        }
        assert!(matches!(
            produce(&direct, &components, &ignored, &proposed, &mut budget),
            Err(E::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn shared_relation_replay_uses_exact_cumulative_work_and_scratch_limits() {
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let root = SemanticFunctionIdV1::from_index(1);
    let value = parameters(&owner)[0];
    let floor = owner.retained_analysis_storage_v1() + 47;
    let check = |budget: &mut ArgumentBudgetV1<'_>| -> Result<u32, ProductionSemanticKirErrorV1> {
        let relation = owner.checked_source_argument_relation_v1(root, root, budget)?;
        let binding = relation.bind_whole_parameter_v1(0, value, budget)?;
        relation.require_binding_v1(&binding, budget)?;
        Ok(binding.source_argument())
    };
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(check(&mut budget).unwrap(), 0);
    let exact_work = budget.work();
    let peak = budget.peak_storage();
    assert!(peak > floor);
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, accepts) in [
        (exact_work, peak, true),
        (exact_work - 1, peak, false),
        (exact_work, peak - 1, false),
        (exact_work, floor, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = check(&mut budget);
        if accepts {
            assert_eq!(result.unwrap(), 0);
            assert_eq!(budget.work(), exact_work);
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
        assert_eq!(budget.storage(), floor);
        assert!(ledger == budget.work_ledger_identity_v1());
    }
}

fn with_shared_query_view<'work, R>(
    owner: &ProductionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'work>,
    use_view: impl for<'scope> FnOnce(
        source_arguments_v1::ProductionArgumentViewV1<'scope>,
        &'scope mut ArgumentBudgetV1<'work>,
    )
        -> Result<R, source_arguments_v1::ProductionSourceArgumentErrorV1>,
) -> Result<R, source_arguments_v1::ProductionSourceArgumentErrorV1> {
    let (association, trace) = relation_rows(owner);
    let function = owner
        .executable()
        .module()
        .function(association.kernel_ir_function())
        .unwrap();
    source_arguments_v1::with_parameter_correspondence_v1(
        owner.semantic_ssa.source_semantic(),
        association,
        function,
        trace,
        budget,
        use_view,
    )
}

fn shared_view_query(
    view: &mut source_arguments_v1::ProductionArgumentViewV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    query: usize,
    reborrow: bool,
    value: ValueId,
    visited: &mut usize,
) -> Result<(), source_arguments_v1::ProductionSourceArgumentErrorV1> {
    if reborrow {
        return shared_view_query(
            &mut view.reborrow_v1(),
            budget,
            query,
            false,
            value,
            visited,
        );
    }
    match query {
        0 => view
            .source_arguments(budget)
            .map(|rows| assert!(rows.len() > 0)),
        1 => view
            .adjusted_arguments(budget)
            .map(|rows| assert!(rows.len() > 0)),
        2 => view
            .ignored_local(budget, SemanticLocalIdV1::from_index(1))
            .map(|_| ()),
        3 => view.physical(budget, 0).map(|row| assert!(row.is_some())),
        4 => view.visit_nodes(budget, |_| {
            *visited += 1;
            Ok(())
        }),
        5 => view.visit_result_structure_v1(U32, budget, |_, _, _| {
            *visited += 1;
            Ok(())
        }),
        6 => view
            .whole_parameter_v1(budget, 0, value)
            .map(|row| assert!(row.is_some())),
        _ => unreachable!(),
    }
}

#[test]
fn shared_view_queries_refuse_foreign_ledgers_before_visiting_and_preserve_original() {
    use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let value = parameters(&owner)[0];
    for query in 0..7 {
        for reborrow in [false, true] {
            for limit in [4, 5] {
                let mut work = Work::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                budget.reserve_storage(31).unwrap();
                let mut foreign_work = Work::new(limit);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
                with_shared_query_view(&owner, &mut budget, |mut view, budget| {
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    foreign.reserve_storage(budget.storage())?;
                    let mut visited = 0;
                    let result = shared_view_query(
                        &mut view,
                        &mut foreign,
                        query,
                        reborrow,
                        value,
                        &mut visited,
                    );
                    if limit == 4 {
                        assert!(matches!(result, Err(E::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error)
                        )) if error.actual() == 5 && error.limit() == 4));
                        assert_eq!(foreign.work(), 0);
                    } else {
                        assert!(matches!(
                            result,
                            Err(E::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(foreign.work(), 5);
                    }
                    assert_eq!(visited, 0);
                    assert_eq!(foreign.storage(), before.1);
                    assert_eq!(foreign.peak_storage(), before.1);
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        before,
                    );
                    shared_view_query(&mut view, budget, query, reborrow, value, &mut visited)?;
                    let cost = budget.work() - before.0;
                    let peak = budget.peak_storage();
                    shared_view_query(&mut view, budget, query, reborrow, value, &mut visited)?;
                    assert_eq!(budget.work() - before.0, 2 * cost);
                    assert_eq!(budget.storage(), before.1);
                    assert_eq!(budget.peak_storage(), peak);
                    assert_eq!(visited > 0, matches!(query, 4 | 5));
                    if query == 2 {
                        assert_eq!(cost, 9);
                    } else if query == 3 {
                        assert_eq!(cost, 45);
                    }
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.storage(), 31);
            }
        }
    }
}

#[test]
fn shared_view_queries_refuse_same_work_in_another_slot_and_replaced_original_slot() {
    use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let value = parameters(&owner)[0];
    for query in 0..7 {
        for reborrow in [false, true] {
            let mut foreign_work = Work::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(31).unwrap();
            with_shared_query_view(&owner, &mut budget, |mut view, budget| {
                let ledger = budget.work_ledger_identity_v1();
                let prefix = budget.work();
                let retained = budget.storage();
                foreign.reserve_storage(retained)?;
                std::mem::swap(budget, &mut foreign);
                assert!(foreign.work_ledger_identity_v1() == ledger);
                let mut visited = 0;
                assert!(matches!(
                    shared_view_query(
                        &mut view,
                        &mut foreign,
                        query,
                        reborrow,
                        value,
                        &mut visited,
                    ),
                    Err(E::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(foreign.work(), prefix + 5);
                assert_eq!(foreign.storage(), retained);
                assert!(matches!(
                    shared_view_query(&mut view, budget, query, reborrow, value, &mut visited,),
                    Err(E::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(budget.work(), 5);
                assert_eq!(budget.storage(), retained);
                assert_eq!(visited, 0);
                std::mem::swap(budget, &mut foreign);
                assert!(budget.work_ledger_identity_v1() == ledger);
                shared_view_query(&mut view, budget, query, reborrow, value, &mut visited)?;
                assert_eq!(budget.storage(), retained);
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), 31);
        }
    }
}

#[test]
fn shared_view_queries_refuse_undercut_retained_floor_and_allow_restored_queries() {
    use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let value = parameters(&owner)[0];
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(31).unwrap();
    with_shared_query_view(&owner, &mut budget, |mut view, budget| {
        let retained = budget.storage();
        assert!(retained > 31);
        for query in 0..7 {
            for reborrow in [false, true] {
                budget.release_storage(1)?;
                let prefix = budget.work();
                let mut visited = 0;
                assert!(matches!(
                    shared_view_query(&mut view, budget, query, reborrow, value, &mut visited,),
                    Err(E::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!(visited, 0);
                assert_eq!(budget.work(), prefix + 5);
                assert_eq!(budget.storage(), retained - 1);
                budget.reserve_storage(1)?;
                shared_view_query(&mut view, budget, query, reborrow, value, &mut visited)?;
                assert_eq!(budget.storage(), retained);
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 31);
}

#[test]
fn shared_view_callback_floor_refusal_overrides_results_without_queries_or_underflow() {
    use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    for mode in 0..4 {
        for callback_error in [false, true] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let floor = 31;
            budget.reserve_storage(floor).unwrap();
            let mut prefix = 0;
            let mut left = 0;
            let result = with_shared_query_view(&owner, &mut budget, |_, budget| {
                let retained = budget.storage();
                assert!(retained > floor);
                left = match mode {
                    0 => retained - 1,
                    1 => floor,
                    2 => floor - 1,
                    _ => 0,
                };
                budget.release_storage(retained - left)?;
                prefix = budget.work();
                if callback_error {
                    Err(E::Visitor)
                } else {
                    Ok(())
                }
            });
            assert!(matches!(
                result,
                Err(E::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(budget.work(), prefix);
            assert_eq!(budget.storage(), left.min(floor));
            budget.reserve_storage(floor - budget.storage()).unwrap();
            with_shared_query_view(&owner, &mut budget, |mut view, budget| {
                assert!(view.physical(budget, 0)?.is_some());
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn shared_view_callback_replacement_overrides_results_without_refunding_foreign_storage() {
    use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    for callback_error in [false, true] {
        let mut foreign_work = Work::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        let original = budget.work_ledger_identity_v1();
        let mut retained = 0;
        let mut prefix = 0;
        let result = with_shared_query_view(&owner, &mut budget, |_, budget| {
            retained = budget.storage();
            prefix = budget.work();
            foreign.reserve_storage(retained)?;
            std::mem::swap(budget, &mut foreign);
            if callback_error {
                Err(E::Visitor)
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(E::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, retained, retained)
        );
        assert_eq!((foreign.work(), foreign.storage()), (prefix, retained));
        assert!(foreign.work_ledger_identity_v1() == original);
        std::mem::swap(&mut budget, &mut foreign);
        // Only the test can restore the displaced ledger after its indices drop.
        budget.release_storage(retained - 31).unwrap();
        with_shared_query_view(&owner, &mut budget, |mut view, budget| {
            assert!(view.physical(budget, 0)?.is_some());
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 31);
    }
}

// These existing scalar fixtures test argument identity, not output coverage.
fn resolve(
    owner: &ProductionPreRankedKirOwnerV1,
    slot: usize,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<ConditionalOutputArgumentV1>, ProductionSemanticKirErrorV1> {
    let root = SemanticFunctionIdV1::from_index(1);
    owner.with_checked_arguments_v1(root, root, budget, |view| {
        conditional_output_argument_v1(view, slot, value)
    })
}

fn parameters(owner: &ProductionPreRankedKirOwnerV1) -> &[ValueId] {
    let row = owner
        .correspondence
        .lowered_functions()
        .iter()
        .find(|row| {
            row.correspondence_owner().index() == 1
                && row.role() == SemanticKirFunctionRoleV1::KernelEntry
        })
        .unwrap();
    &owner
        .executable()
        .module()
        .function(row.kernel_ir_function())
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .parameters
}

#[test]
fn whole_argument_resolution_rejects_component_projections_and_wrong_values() {
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let values = parameters(&owner);
    assert_eq!(values.len(), 5);
    let whole = resolve(&owner, 0, values[0], &mut budget).unwrap().unwrap();
    assert_eq!(
        (whole.source, whole.adjusted, whole.local.index(), whole.ty),
        (0, 0, 1, U32)
    );
    for (slot, &value) in values.iter().enumerate().skip(1) {
        assert_eq!(resolve(&owner, slot, value, &mut budget).unwrap(), None);
    }
    for (slot, value) in [(0, values[1]), (5, values[0]), (usize::MAX, values[0])] {
        assert!(matches!(
            resolve(&owner, slot, value, &mut budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), 19);
    }
}

fn shifted_source() -> ProductionSemanticSsaOwnerV1 {
    let original = argument_owner_shape(false, ArgumentTupleShape::Mixed, true, true);
    let semantic = original.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .map(|function| {
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                return function.clone();
            }
            let mut arguments = function.abi().arguments().to_vec();
            arguments.swap(0, 3);
            let abi = SemanticFunctionAbiV1::from_rustc(
                function.abi().identity(),
                semantic.target().identity(),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                4,
                arguments,
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 4])
            .unwrap();
            let locals = function
                .locals()
                .iter()
                .map(|local| {
                    let role = match local.role() {
                        SemanticLocalRoleV1::Argument(0) => SemanticLocalRoleV1::Argument(3),
                        SemanticLocalRoleV1::Argument(3) => SemanticLocalRoleV1::Argument(0),
                        other => other,
                    };
                    SemanticLocalDeclV1::new(local.identity(), local.ty(), role, local.source())
                })
                .collect();
            SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                abi,
                locals,
                function.entry(),
                function.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone())
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn source_ordinal_does_not_follow_erased_or_expanded_physical_slots() {
    let owner = materialize_argument_view(shifted_source());
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let values = parameters(&owner);
    let whole = resolve(&owner, 4, values[4], &mut budget).unwrap().unwrap();
    assert_eq!(
        (whole.source, whole.adjusted, whole.local.index(), whole.ty),
        (3, 3, 1, U32)
    );
    assert_ne!(whole.source, 4);
    for (slot, &value) in values.iter().enumerate().take(4) {
        assert_eq!(resolve(&owner, slot, value, &mut budget).unwrap(), None);
    }
    assert_eq!(budget.storage(), 0);
}

#[test]
fn output_argument_join_uses_the_callers_work_and_storage_budget() {
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let value = parameters(&owner)[0];
    let floor = 23;
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let expected = resolve(&owner, 0, value, &mut budget).unwrap();
    let exact_work = budget.work();
    let peak = budget.peak_storage();
    assert!(peak > floor);
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, accepts) in [
        (exact_work, peak, true),
        (exact_work - 1, peak, false),
        (exact_work, peak - 1, false),
        (exact_work, floor, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = resolve(&owner, 0, value, &mut budget);
        if accepts {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(budget.work(), exact_work);
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                ),
                "{result:?}"
            );
        }
        assert_eq!(budget.storage(), floor);
        assert!(ledger == budget.work_ledger_identity_v1());
    }
}
