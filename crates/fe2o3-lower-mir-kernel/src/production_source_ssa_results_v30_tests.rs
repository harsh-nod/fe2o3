use super::*;

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn probe(allowance: Option<(usize, usize)>, fault: u8) -> (SourceOwnedResultV18<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let adopted = prepared.adopted_storage();
    budget
        .reserve_storage(budget.peak_storage() + 1 - budget.storage())
        .unwrap();
    if let Some((work, storage)) = allowance {
        budget
            .charge_work(MODULE_LIMIT - budget.work() - work)
            .unwrap();
        budget
            .reserve_storage(MODULE_LIMIT - budget.storage() - storage)
            .unwrap();
    }
    let floor = budget.storage();
    let before = budget.work();
    let result = prepared.with_checked_source_v18(&mut budget, |source, budget| {
        source.with_analysis_v18(budget, |scope| {
            scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    for root in 0..source.root_count(budget)? {
                        let owner = source.root_row(root)?;
                        let rows = &owner.rvalue_results.as_ref().unwrap().values;
                        assert!(!rows.is_empty());
                        for row in rows {
                            let value = if fault == 1 {
                                SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(
                                    u32::MAX,
                                ))
                            } else {
                                row.original
                            };
                            let instance = if fault == 2 { usize::MAX } else { row.instance };
                            let definition = relation
                                .ssa_scalar_definition_v30(root, instance, value, budget)?;
                            match row.endpoint {
                                SourceRvalueEndpointV30::Unit => assert_eq!(definition, None),
                                SourceRvalueEndpointV30::Scalar { value, scalar } => {
                                    let actual = &inventory.definitions()[definition.unwrap()];
                                    assert_eq!(actual.value, Some(value));
                                    assert_eq!(actual.ty, &Type::Scalar(scalar));
                                }
                                SourceRvalueEndpointV30::Unmodeled => {
                                    panic!("ordinary scalar fixture acquired an unmodeled binding")
                                }
                            }
                        }
                    }
                    Ok(())
                })
            })
        })
    });
    assert_eq!(budget.storage(), floor - adopted);
    (
        result,
        budget.work() - before,
        budget.peak_storage() - floor,
    )
}

#[test]
fn retained_source_ssa_locators_join_exact_original_values_to_actual_definitions() {
    probe(None, 0).0.unwrap();
    assert!(matches!(
        probe(None, 1).0,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original SSA result definition is foreign"
        ))
    ));
    assert!(matches!(
        probe(None, 2).0,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "instance ordinal"
        ))
    ));
}

#[test]
fn retained_source_ssa_locators_have_exact_and_one_short_complete_query_resources() {
    let (result, work, storage) = probe(None, 0);
    result.unwrap();
    let exact = probe(Some((work, storage)), 0);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for short_work in [true, false] {
        let result = probe(
            Some((
                work - usize::from(short_work),
                storage - usize::from(!short_work),
            )),
            0,
        )
        .0
        .unwrap_err();
        match entrance_resource(result) {
            ArgumentResourceV1::Work(_) if short_work => {}
            ArgumentResourceV1::Storage(_) if !short_work => {}
            error => panic!("exact source SSA locator boundary: {error:?}"),
        }
    }
}

#[test]
fn retained_source_ssa_replay_rejects_omission_value_instance_and_endpoint_changes() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
    prepared
        .with_checked_source_v18(&mut budget, |source, budget| {
            let mut repeated = false;
            for root in 0..source.root_count(budget)? {
                let original = source.root_row(root)?.rvalue_results.as_ref().unwrap();
                assert!(!original.values.is_empty());
                repeated |= original.values.iter().any(|row| row.instance != 0);
                assert!(original.values.windows(2).all(|pair| (
                    pair[0].instance,
                    pair[0].original
                ) < (
                    pair[1].instance,
                    pair[1].original
                )));
                for fault in 0..5 {
                    let mut changed = OwnedSourceRvaluesV30 {
                        source: original.source,
                        ledger: original.ledger,
                        rows: original.rows.clone(),
                        values: original.values.clone(),
                        carriers: original.carriers.clone(),
                        index_readers: original.index_readers.clone(),
                        storage: original.storage,
                    };
                    match fault {
                        0 => {}
                        1 => {
                            changed.values.pop();
                        }
                        2 => changed.values[0].instance = usize::MAX,
                        3 => {
                            changed.values[0].original = SsaValueV1::Definition(
                                fe2o3_mir_model::SsaDefinitionIdV1::new(u32::MAX),
                            )
                        }
                        4 => {
                            changed.values[0].endpoint = match changed.values[0].endpoint {
                                SourceRvalueEndpointV30::Unmodeled => SourceRvalueEndpointV30::Unit,
                                _ => SourceRvalueEndpointV30::Unmodeled,
                            }
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(
                        original.matches_replay_v30(&changed, budget).map_err(|_| {
                            ProductionSourceOwnedViewErrorV18::Binding("test SSA replay failed")
                        })?,
                        fault == 0
                    );
                }
            }
            assert!(repeated);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retained_source_ssa_query_rejects_a_funded_foreign_ledger_before_charging() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let refused = std::cell::Cell::new(None);
    let result: SourceOwnedResultV18<()> =
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        let row = source.root_row(0)?.rvalue_results.as_ref().unwrap().values[0];
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage())?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        let error = relation
                            .ssa_scalar_definition_v30(0, row.instance, row.original, &mut foreign)
                            .unwrap_err();
                        assert!(matches!(
                            error,
                            ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            )
                        ));
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                        refused.set(Some(budget.storage()));
                        Err(error)
                    })
                })
            })
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(Some(budget.storage()), refused.get());
}

fn prepared_control(cycle: bool, budget: &mut ArgumentBudgetV1<'_>) -> ProductionPreparedSourceV18 {
    let owner = module_fixture_owner(ModuleFixture::Ordinary);
    let source = owner.source_semantic();
    assert_eq!(source.types().len(), 3);
    assert!(matches!(
        source.types()[2].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ));
    let functions: Vec<_> = source
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, prior)| {
            let base = 180 + ordinal as u8 * 10;
            let switch = |yes, no| SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(1, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, yes),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
                )
                .unwrap(),
            };
            let go =
                |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
            let mut locals = prior.locals().to_vec();
            locals.push(local(base, U32, SemanticLocalRoleV1::Temporary));
            let blocks = vec![
                block(base + 1, vec![], switch(1, 2)),
                block(
                    base + 2,
                    vec![assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                    )],
                    go(3),
                ),
                block(
                    base + 3,
                    vec![assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(
                                U32,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(7, 4).unwrap(),
                                ),
                            ),
                        )),
                    )],
                    go(3),
                ),
                block(
                    base + 4,
                    vec![assign(
                        place(1, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
                    )],
                    if cycle { switch(4, 0) } else { go(4) },
                ),
                block(base + 5, vec![], SemanticTerminatorKindV1::Return),
            ];
            SemanticFunctionDeclV1::new(
                prior.identity(),
                prior.role(),
                prior.item_definition_identity(),
                prior.monomorphization_identity(),
                prior.generic_type_arguments_identity(),
                prior.const_generic_arguments_identity(),
                prior.source(),
                prior.abi().clone(),
                locals,
                SemanticBlockIdV1::from_index(0),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(prior.kernel_entry().unwrap().clone())
        })
        .collect();
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        // The replaced constant Assert was the only use of the trailing Bool.
        source.types()[..2].to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let launches: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, &launches).unwrap();
    let source = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let identity = *source.source_semantic_sha256();
    ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
        source,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: &identity,
            roots: &[],
            classes: &[ProductionScopeCallableCandidateV29::Ordinary; 2],
            events: &[],
        },
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

#[test]
fn retained_source_ssa_locators_preserve_diamond_and_entry_loop_parameters() {
    for cycle in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let prepared = prepared_control(cycle, &mut budget);
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    for root in 0..source.root_count(budget)? {
                        let rows = &source.root_row(root)?.rvalue_results.as_ref().unwrap().values;
                        let mut merge = 0;
                        let mut entry = 0;
                        for row in rows {
                            let Some(definition) = relation.ssa_scalar_definition_v30(root, row.instance, row.original, budget)? else { continue };
                            let actual = &inventory.definitions()[definition];
                            if let SsaValueV1::BlockArgument { block, .. } = row.original {
                                assert!(matches!(actual.coordinate, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { .. }));
                                merge += usize::from(block.get() == 3);
                                entry += usize::from(block.get() == 0);
                            }
                        }
                        assert!(merge > 0);
                        assert_eq!(entry > 0, cycle);
                    }
                    Ok(())
                })
            }))
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
