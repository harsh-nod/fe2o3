use super::*;

fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn selector_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = nominal_owner(Input::Workgroup, false);
    let semantic = original.source_semantic();
    // This ordinary root uses only Unit, U32 and its array, not nominal roles.
    let mut types = semantic.types()[..2].to_vec();
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            4,
            SemanticFieldsShapeV1::array(4, 4),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            16,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 4,
        },
        None,
    );
    let indexed = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(1)),
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap()
    };
    let root = function(
        210,
        SemanticFunctionRoleV1::KernelRoot,
        abi(211, true, &[U32]),
        vec![
            local(212, UNIT, SemanticLocalRoleV1::Return),
            local(213, U32, SemanticLocalRoleV1::Argument(0)),
            local(214, array, SemanticLocalRoleV1::Temporary),
            local(215, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(
            216,
            vec![
                assign(
                    place(2, array),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            vec![literal(11), literal(12), literal(13), literal(14)],
                        )
                        .unwrap(),
                    ),
                ),
                assign(indexed(), SemanticRvalueKindV1::Use(literal(99))),
                assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(indexed())),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_selector_plan(
    consume: impl for<'owner, 'view, 'root, 'source, 'work> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        source_storage_v29::SourceStorageRootV29<'view, 'root, 'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        (),
        source_storage_v29::SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = selector_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 20_000_000);
    budget.reserve_storage(FLOOR)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                source_storage_v29::with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |plan, root, budget| consume(plan, root, budget),
                ),
            )
        })
        .unwrap();
    let safe = layouts.permits_root_emission_refund(&owner, 0, &budget);
    let cleanup = layouts.release(&mut budget);
    let demands_cleanup = if safe {
        demands.discard(&mut budget)
    } else {
        drop(demands);
        Err(ArgumentResourceV1::Accounting.into())
    };
    result.and(cleanup).and(demands_cleanup)
}

#[test]
fn original_fixed_array_selector_joins_exact_index_use_and_current_ssa() {
    with_selector_plan(|plan, _, budget| {
        assert_eq!(plan.selectors.len(), 2);
        let a = plan.selectors[0];
        let b = plan.selectors[1];
        assert_eq!(a.value, b.value);
        assert_eq!(a.canonical, b.canonical);
        assert_ne!(a.occurrence, b.occurrence);
        assert_ne!(a.source, b.source);
        assert_eq!(a.length, 4);
        assert_eq!(a.local.index(), 1);
        let source = a.check(plan.instances, budget)?;
        let occurrences = plan.instances.occurrences(a.instance).unwrap();
        let occurrence = &occurrences.events()[a.occurrence];
        assert_eq!(occurrence.role(), ExecutionEventV29::ProjectionIndexUse(0));
        assert_eq!(
            plan.selector_at(a.instance, occurrence.site(), source, 0, budget)?
                .unwrap()
                .0,
            0
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn selector_refuses_same_shaped_place_wrong_occurrence_type_extent_and_ssa() {
    with_selector_plan(|plan, _, budget| {
        let original = plan.selectors[0];
        for mutation in 0..6 {
            let mut changed = original;
            match mutation {
                0 => changed.source = plan.selectors[1].source,
                1 => changed.occurrence = plan.selectors[1].occurrence,
                2 => changed.local = SemanticLocalIdV1::from_index(3),
                3 => changed.array = U32,
                4 => changed.length += 1,
                _ => changed.projection += 1,
            }
            assert!(
                changed.check(plan.instances, budget).is_err(),
                "mutation {mutation}"
            );
        }
        let original_place = original.check(plan.instances, budget)?;
        let copy = original_place.clone();
        let occurrences = plan.instances.occurrences(original.instance).unwrap();
        let event = &occurrences.events()[original.occurrence];
        assert!(
            plan.selector_at(original.instance, event.site(), &copy, 0, budget)?
                .is_none()
        );
        let mut changed = original;
        changed.value = SourceReferenceSelectorValueV29::Promoted(SsaValueV1::BlockArgument {
            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
            variable: fe2o3_mir_model::SsaVariableIdV1::new(3),
        });
        assert!(changed.check(plan.instances, budget).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_selected_mutation_never_initializes_the_whole_array_or_another_ssa_index() {
    with_selector_plan(|plan, root, budget| {
        let row = plan.selectors[0];
        let entry = plan.entries[row.instance.index()].unwrap();
        let snapshot = plan.storage_snapshots[plan.states[entry][2].storage.unwrap()];
        assert!(!root.snapshot_initialized(snapshot, &[], budget)?);
        let source = row.check(plan.instances, budget)?;
        let occurrences = plan.instances.occurrences(row.instance).unwrap();
        let event = &occurrences.events()[row.occurrence];
        let (block, statement) = scoped_memory_site_key_v29(event.site());
        let context = Some((
            SourceReferenceSiteV29 {
                instance: row.instance,
                block: SemanticBlockIdV1::from_index(block),
                statement: statement.map(|value| value as usize),
            },
            source as *const SemanticPlaceV1 as usize,
        ));
        let selected = root.mutate_selected_snapshot(
            snapshot,
            source.projections(),
            plan,
            context,
            source_storage_v29::SourceStorageRootMutationV29::Initialize,
            budget,
        )?;
        assert!(root.snapshot_selected_initialized(
            selected,
            source.projections(),
            plan,
            context,
            budget
        )?);
        assert!(!root.snapshot_initialized(selected, &[], budget)?);
        let other = [SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 4,
                from_end: false,
            },
            U32,
        )
        .unwrap()];
        assert!(!root.snapshot_initialized(selected, &other, budget)?);
        let stats = root.snapshot_statistics(budget)?;
        for _ in 0..8 {
            assert_eq!(
                root.mutate_selected_snapshot(
                    selected,
                    source.projections(),
                    plan,
                    context,
                    source_storage_v29::SourceStorageRootMutationV29::Initialize,
                    budget
                )?,
                selected
            );
        }
        assert_eq!(root.snapshot_statistics(budget)?, stats);
        let expired = root.forget_snapshot_selectors(selected, &[row.canonical], plan, budget)?;
        assert!(!root.snapshot_selected_initialized(
            expired,
            source.projections(),
            plan,
            context,
            budget
        )?);
        Ok(())
    })
    .unwrap();
}
