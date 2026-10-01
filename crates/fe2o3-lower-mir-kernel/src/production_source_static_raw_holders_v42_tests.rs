use super::*;

fn original_read<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
) -> (SourceReferenceSiteV29, &'a SemanticPlaceV1) {
    let instance = plan.instances.root();
    let function = plan.instances.instance(instance).unwrap().declaration();
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, row) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
                assignment.value().kind()
            else {
                continue;
            };
            if place.projections().len() > 1
                && place.projections().last().unwrap().kind()
                    == SemanticProjectionKindV1::Dereference
            {
                return (
                    SourceReferenceSiteV29 {
                        instance,
                        block: SemanticBlockIdV1::from_index(block as u32),
                        statement: Some(statement),
                    },
                    place,
                );
            }
        }
    }
    panic!("genuine nested holder read required")
}

fn headers() -> usize {
    use std::mem::size_of;
    type E = ProductionSemanticKirErrorV1;
    type Fields<'a> = (
        &'a SemanticFunctionDeclV1,
        &'a [SemanticTypeDeclV1],
        &'a [SemanticProjectionV1],
        &'a [SemanticProjectionV1],
        std::slice::Iter<'a, SemanticProjectionV1>,
        &'a SemanticProjectionV1,
        &'a SemanticTypeDeclV1,
        &'a SemanticPointerTypeV1,
        SemanticTypeIdV1,
        (SourceReferenceAccessIndexKeyV29, usize),
        SourceReferenceRawAccessV29,
        SourceReferenceRawHolderV29,
        Option<SourceReferenceRawAccessV29>,
        std::ops::Range<usize>,
    );
    assert_eq!(
        size_of::<Fields<'_>>(),
        size_of::<SourceStaticRawHolderFrameV42<'_>>()
    );
    type ShapeFields<'a> = (
        &'a SemanticFunctionDeclV1,
        &'a [SemanticTypeDeclV1],
        &'a SemanticPlaceV1,
        &'a [SemanticProjectionV1],
        std::slice::Iter<'a, SemanticProjectionV1>,
        &'a SemanticProjectionV1,
        &'a SemanticTypeDeclV1,
        &'a SemanticPointerTypeV1,
        SemanticTypeIdV1,
        SourceReferenceAccessV29,
        usize,
    );
    assert_eq!(
        size_of::<ShapeFields<'_>>(),
        size_of::<SourceStaticRawHolderShapeFrameV42<'_>>()
    );
    size_of::<Fields<'_>>()
        + 2 * size_of::<Result<Fields<'_>, E>>()
        + size_of::<SourceReferenceRawAccessV29>()
        + 2 * size_of::<Result<SourceReferenceRawAccessV29, E>>()
        + size_of::<ShapeFields<'_>>()
        + 2 * size_of::<Result<ShapeFields<'_>, E>>()
}

#[test]
fn original_static_raw_holders_join_nested_fields_arrays_and_from_end_bounds() {
    assert_eq!(headers(), source_static_raw_holder_headers_v42().unwrap());
    for immutable in [false, true] {
        for mode in [4, 5, 6, 7, 8] {
            let owner =
                projected_pointer_test_owner_v29(entrance_control_owner(false), immutable, mode);
            let mut completed = false;
            with_selected_pointer_test_plan_v29(owner, |plan, budget| {
                let (site, place) = original_read(plan);
                let crossing = place.projections().len() - 1;
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let row = source_static_raw_holder_v42(
                    plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    crossing,
                    budget,
                )?;
                let lookup =
                    (plan.raw_accesses.len().checked_ilog2().unwrap_or(0) as usize + 2) * 16;
                // Owner 5 + scratch 2 + fixed queries 8+8+16 + each path step 6+1.
                assert_eq!(budget.work() - before.0, 39 + 7 * crossing + lookup);
                assert_eq!(budget.storage(), before.1);
                assert_eq!(budget.peak_storage(), before.2.max(before.1 + headers()));
                assert_eq!(
                    (row.site, row.source, row.projection),
                    (site, place as *const _ as usize, crossing)
                );
                assert_eq!(row.holder.count, crossing);
                assert_eq!(row.holder.local, place.local());
                completed = true;
                Ok(())
            })
            .unwrap_or_else(|error| panic!("immutable={immutable}, mode={mode}: {error:?}"));
            assert!(completed, "immutable={immutable}, mode={mode}");
        }
    }
}

fn nested_holder_overwrite_owner_v43(
    immutable: bool,
    rewrite: bool,
    expire: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let owner = projected_pointer_test_owner_v29(entrance_control_owner(false), immutable, 8);
    let semantic = owner.source_semantic();
    let old = &semantic.functions()[0];
    let mut statements = old.blocks()[0].statements().to_vec();
    if !rewrite {
        statements.retain(|statement| {
            let destination = match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => Some(assignment.destination()),
                SemanticStatementKindV1::Store(store) => Some(store.destination()),
                _ => None,
            };
            !destination.is_some_and(|place| {
                place.local().index() == 4
                    && place.projections().first().is_some_and(|projection| {
                        projection.kind() == SemanticProjectionKindV1::Dereference
                    })
            })
        });
    }
    if expire {
        let before = statements
            .iter()
            .position(|statement| {
                matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local().index() == 7)
            })
            .unwrap();
        statements.insert(
            before,
            SemanticStatementV1::new(
                old.source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(if rewrite {
                    2
                } else {
                    9
                })),
            ),
        );
    }
    let root = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        vec![
            SemanticBasicBlockV1::new(
                old.blocks()[0].identity(),
                old.blocks()[0].source(),
                statements,
                old.blocks()[0].terminator().clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

#[test]
fn nested_static_holder_writes_keep_exact_current_origins_and_do_not_revive_dead_targets() {
    for immutable in [false, true] {
        for rewrite in [false, true] {
            let owner = nested_holder_overwrite_owner_v43(immutable, rewrite, false);
            let mut completed = false;
            with_selected_pointer_test_plan_v29(owner, |plan, budget| {
                let (site, place) = original_read(plan);
                let access = source_static_raw_holder_v42(
                    plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    place.projections().len() - 1,
                    budget,
                )?;
                let set = &plan.raw_sets[access.set];
                assert_eq!(set.local.index(), if rewrite { 2 } else { 9 });
                assert_eq!(set.mutable, !immutable);
                assert!(set.count > 0);
                for choice in &plan.raw_choices[set.first..set.first + set.count] {
                    assert!(!choice.expired);
                    let origin = &plan.raw_origins[choice.origin];
                    assert_eq!(origin.local, set.local);
                    assert_eq!(origin.mutable, !immutable);
                }
                completed = true;
                Ok(())
            })
            .unwrap();
            assert!(completed);
            let owner = nested_holder_overwrite_owner_v43(immutable, rewrite, true);
            let mut accepted = false;
            let result = with_selected_pointer_test_plan_v29(owner, |_, _| {
                accepted = true;
                Ok(())
            });
            assert!(!accepted && result.is_err());
            assert!(
                format!("{:?}", result.unwrap_err())
                    .contains("source raw pointer outlived its storage activation")
            );
        }
    }
}

#[test]
fn nested_static_assignment_header_accounts_for_the_retained_parent_walk() {
    type Fields<'a> = (
        (usize, usize, usize),
        Vec<SourceReferenceStaticParentV43>,
        SourceReferenceLocalV29,
        SourceReferenceNodeV29,
        SemanticProjectionV1,
        (usize, usize, SemanticTypeIdV1),
        Option<usize>,
        [usize; 8],
        [&'a (); 8],
    );
    assert_eq!(
        std::mem::size_of::<SourceReferenceStaticParentV43>(),
        std::mem::size_of::<(usize, usize, usize)>()
    );
    assert_eq!(
        source_reference_static_assignment_headers_v43().unwrap(),
        std::mem::size_of::<Fields<'_>>()
            + 2 * std::mem::size_of::<Result<Fields<'_>, ProductionSemanticKirErrorV1>>()
    );
}

#[test]
fn original_static_raw_holders_refuse_cloned_changed_sites_and_wrong_crossings_stickily() {
    for fault in 0..4 {
        let owner = projected_pointer_test_owner_v29(entrance_control_owner(false), false, 8);
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
            let (site, original) = original_read(plan);
            let clone = original.clone();
            let place = if fault == 0 { &clone } else { original };
            let changed = SourceReferenceSiteV29 {
                statement: site.statement.map(|n| n + 1),
                ..site
            };
            let queried = source_static_raw_holder_v42(
                plan,
                if fault == 1 { changed } else { site },
                place,
                if fault == 2 {
                    SourceReferenceAccessV29::Write
                } else {
                    SourceReferenceAccessV29::Read
                },
                original.projections().len() - if fault == 3 { 2 } else { 1 },
                budget,
            );
            assert!(queried.is_err());
            assert!(
                source_static_raw_holder_v42(
                    plan,
                    site,
                    original,
                    SourceReferenceAccessV29::Read,
                    original.projections().len() - 1,
                    budget
                )
                .is_err()
            );
            completed = true;
            Err(scoped_object_error_v29())
        });
        assert!(completed && result.is_err(), "fault={fault}");
    }
}

#[test]
fn original_static_raw_holders_refuse_changed_static_types_bounds_and_pointer_crossings() {
    for fault in 0..7 {
        let owner = projected_pointer_test_owner_v29(entrance_control_owner(false), false, 8);
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
            let (site, original) = original_read(plan);
            assert_eq!(original.projections().len(), 3);
            let mut path = original.projections().to_vec();
            let field_type = path[0].result_type();
            let pointer_type = path[1].result_type();
            match fault {
                0 => {
                    path[0] =
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), field_type)
                            .unwrap()
                }
                1 => {
                    path[0] =
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), pointer_type)
                            .unwrap()
                }
                2 | 3 | 4 => {
                    path[1] = SemanticProjectionV1::new(
                        SemanticProjectionKindV1::ConstantIndex {
                            offset: if fault == 3 {
                                3
                            } else if fault == 4 {
                                2
                            } else {
                                1
                            },
                            minimum_length: 3,
                            from_end: fault != 4,
                        },
                        pointer_type,
                    )
                    .unwrap()
                }
                5 => {
                    path[1] = SemanticProjectionV1::new(
                        SemanticProjectionKindV1::ConstantIndex {
                            offset: 1,
                            minimum_length: 2,
                            from_end: false,
                        },
                        original.ty(),
                    )
                    .unwrap()
                }
                6 => {
                    path[1] = SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Dereference,
                        pointer_type,
                    )
                    .unwrap()
                }
                _ => unreachable!(),
            }
            let changed = SemanticPlaceV1::new(original.local(), path, original.ty()).unwrap();
            let floor = budget.storage();
            let function = plan
                .instances
                .instance(site.instance)
                .unwrap()
                .declaration();
            let types = plan.instances.owner().source_semantic().types();
            // This validator has no access-map/occurrence query. The failure
            // must be the malformed shape, not the cloned place identity.
            let expected = match fault {
                0 => "static raw holder field is outside its original type",
                1 | 5 => "static raw holder projected type differs from its original child",
                2..=4 => "static raw holder index exceeds its original array bounds",
                6 => "static raw holder prefix is not a static field or array selection",
                _ => unreachable!(),
            };
            assert!(matches!(source_static_raw_holder_shape_v42(
                function, types, &changed, SourceReferenceAccessV29::Read, 2, budget,
            ), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == expected));
            assert_eq!(
                source_static_raw_holder_shape_v42(
                    function,
                    types,
                    original,
                    SourceReferenceAccessV29::Read,
                    2,
                    budget,
                )?,
                pointer_type,
                "a failed pure shape query cannot poison or authorize an occurrence"
            );
            assert!(
                source_static_raw_holder_v42(
                    plan,
                    site,
                    &changed,
                    SourceReferenceAccessV29::Read,
                    2,
                    budget,
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
            assert!(
                source_static_raw_holder_v42(
                    plan,
                    site,
                    original,
                    SourceReferenceAccessV29::Read,
                    2,
                    budget,
                )
                .is_err()
            );
            completed = true;
            Err(scoped_object_error_v29())
        });
        assert!(completed && result.is_err(), "fault={fault}");
    }
}

#[test]
fn original_static_raw_holders_preserve_exact_and_one_short_query_resources() {
    for mode in 0..4 {
        let owner = projected_pointer_test_owner_v29(entrance_control_owner(false), false, 8);
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
            let (site, place) = original_read(plan);
            let crossing = place.projections().len() - 1;
            let work = 39
                + crossing * 7
                + (plan.raw_accesses.len().checked_ilog2().unwrap_or(0) as usize + 2) * 16;
            if mode < 2 {
                budget.charge_work(20_000_000 - budget.work() - work + mode)?;
            } else {
                budget.reserve_storage(
                    64 * 1024 * 1024 - budget.storage() - headers() + (mode - 2),
                )?;
            }
            let floor = budget.storage();
            let value = source_static_raw_holder_v42(
                plan,
                site,
                place,
                SourceReferenceAccessV29::Read,
                crossing,
                budget,
            );
            assert_eq!(budget.storage(), floor);
            match mode {
                0 | 2 => assert!(value.is_ok()),
                1 => assert!(matches!(
                    value,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error)
                        )
                    ) if error.actual() == 20_000_001 && error.limit() == 20_000_000
                )),
                3 => assert!(matches!(
                    value,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(error)
                        )
                    ) if error.actual() == 64 * 1024 * 1024 + 1
                        && error.limit() == 64 * 1024 * 1024
                )),
                _ => unreachable!(),
            }
            completed = true;
            Err(scoped_object_error_v29())
        });
        assert!(completed && result.is_err(), "mode={mode}");
    }
}

#[test]
fn original_static_raw_holder_custody_refuses_foreign_and_restored_credit() {
    for foreign in [false, true] {
        let owner = projected_pointer_test_owner_v29(entrance_control_owner(false), false, 8);
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(owner, |plan, budget| {
            let (site, place) = original_read(plan);
            let crossing = place.projections().len() - 1;
            let floor = budget.storage();
            if foreign {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut other = ArgumentBudgetV1::new(&mut work, usize::MAX);
                other.reserve_storage(floor)?;
                let before = (other.work(), other.storage(), other.peak_storage());
                assert!(matches!(
                    source_static_raw_holder_v42(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        crossing,
                        &mut other
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!(
                    (other.work(), other.storage(), other.peak_storage()),
                    before
                );
            } else {
                let debit = floor - plan.retained_floor + 1;
                budget.release_storage(debit)?;
                assert!(matches!(
                    source_static_raw_holder_v42(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        crossing,
                        budget
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                budget.reserve_storage(debit)?;
                assert_eq!(budget.storage(), floor);
            }
            assert!(matches!(
                source_static_raw_holder_v42(
                    plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    crossing,
                    budget
                ),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            completed = true;
            Err(scoped_object_error_v29())
        });
        assert!(completed && result.is_err());
    }
}

fn original_projected_aggregate_owner() -> ProductionSemanticSsaOwnerV1 {
    projected_pointer_test_owner_v29(entrance_control_owner(false), false, 9)
}

fn original_projected_aggregate_keys(
    owner: &ProductionSemanticSsaOwnerV1,
) -> Vec<(usize, bool, u32)> {
    let function = &owner.source_semantic().functions()[0];
    let mut found = None;
    for (statement, row) in function.blocks()[0].statements().iter().enumerate() {
        let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
            continue;
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            continue;
        };
        let place = assignment.destination();
        if place.projections().is_empty() {
            continue;
        }
        assert!(found.is_none());
        assert_eq!(place.projections().len(), 1);
        assert_eq!(
            place.projections()[0].kind(),
            SemanticProjectionKindV1::Field(0)
        );
        assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Array);
        assert_eq!(aggregate.operands().len(), 2);
        assert!(
            aggregate
                .operands()
                .iter()
                .all(|operand| matches!(operand, SemanticOperandV1::Copy(_)))
        );
        // One admitted original assignment supplies both ordinal families.
        // The checker only compares these inert keys: this is not a fabricated
        // physical operation, retained carrier, or source-access receipt.
        let mut keys = Vec::new();
        for prefix in 1..=place.projections().len() {
            keys.push((statement, false, prefix as u32));
        }
        for operand in 0..aggregate.operands().len() {
            keys.push((statement, true, operand as u32));
        }
        found = Some(keys);
    }
    found.expect("genuinely admitted projected aggregate assignment required")
}

#[test]
fn original_projected_aggregate_operand_and_prefix_census_namespaces_are_distinct() {
    let owner = original_projected_aggregate_owner();
    let mut keys = original_projected_aggregate_keys(&owner);
    assert_eq!(keys.len(), 3);
    assert!(keys.iter().any(|left| {
        keys.iter()
            .any(|right| left.0 == right.0 && left.2 == right.2 && left.1 != right.1)
    }));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 31);
    budget.reserve_storage(31).unwrap();
    check_source_static_object_project_keys_v42(&mut keys, &mut budget).unwrap();
    assert_eq!((budget.storage(), budget.peak_storage()), (31, 31));
    assert!(keys.windows(2).all(|rows| rows[0] != rows[1]));
}

#[test]
fn original_projected_aggregate_census_refuses_duplicates_within_either_family() {
    let owner = original_projected_aggregate_owner();
    let original = original_projected_aggregate_keys(&owner);
    for family in [false, true] {
        let mut keys = original.clone();
        keys.push(
            *original
                .iter()
                .find(|row| row.1 == family && row.2 == 1)
                .unwrap(),
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        assert!(check_source_static_object_project_keys_v42(&mut keys, &mut budget).is_err());
        assert_eq!((budget.storage(), budget.peak_storage()), (31, 31));
    }
}

#[test]
fn original_projected_aggregate_key_coverage_does_not_admit_unsupported_full_emission() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(None));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        scalar_payload_prepared_from_v18(original_projected_aggregate_owner, &mut budget);
    let mut completed = false;
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            completed = true;
            Ok(())
        });
    assert!(
        !completed && result.is_err(),
        "key uniqueness is not aggregate memory authority"
    );
    assert!(format!("{:?}", result.unwrap_err()).contains(
        "typed allocation identity or representation requires its exact source contract"
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
