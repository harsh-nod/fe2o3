thread_local! {
    static OPTIMIZED_ALIAS_GRAPH_V18: std::cell::Cell<(bool, u8)> = const { std::cell::Cell::new((false, 0)) };
}

fn optimized_alias_graph_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let (immutable, graph) = OPTIMIZED_ALIAS_GRAPH_V18.get();
    let owner = selected_pointer_test_owner_v29(module_fixture_owner(ModuleFixture::Ordinary), immutable,
        if graph == 3 { 1 } else { graph });
    if graph == 0 { return owner; }
    let semantic = owner.source_semantic();
    let original = &semantic.functions()[0];
    let mut blocks = original.blocks().to_vec();
    if matches!(graph, 1 | 2) {
        // Keep the merged pointer live through a real source memory read. The
        // original fixture discarded both helper results, permitting phi DCE.
        let last = blocks.last_mut().unwrap();
        assert!(matches!(last.terminator().kind(), SemanticTerminatorKindV1::Return));
        let mut statements = last.statements().to_vec();
        let result_pointer = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(8), vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
        ], U32).unwrap();
        statements.insert(0, assign(place(6, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(result_pointer))));
        *last = SemanticBasicBlockV1::new(last.identity(), last.source(), statements,
            last.terminator().clone()).unwrap();
    } else {
        let first = &original.blocks()[0];
        let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = first.terminator().kind() else {
            panic!("existing dynamic diamond switch required");
        };
        let edge = |role| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(1));
        blocks[0] = SemanticBasicBlockV1::new(first.identity(), first.source(), first.statements().to_vec(),
            SemanticTerminatorV1::new(first.terminator().source(), SemanticTerminatorKindV1::SwitchInt {
                discriminant: discriminant.clone(),
                targets: SemanticSwitchTargetsV1::new(vec![
                    SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue)),
                    SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue)),
                ], edge(SemanticEdgeRoleV1::SwitchOtherwise)).unwrap(),
            })).unwrap();
    }
    let root = SemanticFunctionDeclV1::new(original.identity(), original.role(), original.item_definition_identity(),
        original.monomorphization_identity(), original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(), original.source(), original.abi().clone(),
        original.locals().to_vec(), original.entry(), blocks).unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
    let admitted = InertSemanticMirRequestV1::new_with_callables(semantic.target(), semantic.types().to_vec(),
        vec![], vec![], vec![], functions, semantic.callables().to_vec(), semantic.roots().to_vec())
        .unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn optimized_alias_real_source_graphs_cover_repeated_callers_merges_diamonds_loops_and_duplicate_edges() {
    for immutable in [false, true] {
        for graph in 0..4 {
            OPTIMIZED_ALIAS_GRAPH_V18.set((immutable, graph));
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = scalar_payload_prepared_from_v18(optimized_alias_graph_owner_v18, &mut budget);
            let entered = std::cell::Cell::new(false);
            let completed = std::cell::Cell::new(false);
            let result = with_production_optimized_consumer_v18(prepared, &mut budget, |original, optimized, budget| {
                entered.set(true);
                assert_eq!(original.source.instance_count(0, budget)?, 3,
                    "root plus two genuine helper invocations");
                assert_eq!(original.source.instance(0, 1, budget)?.0, original.source.instance(0, 2, budget)?.0);
                assert_ne!(original.source.instance(0, 1, budget)?.1, original.source.instance(0, 2, budget)?.1,
                    "caller occurrence coordinates must remain distinct");
                assert!(original.source.instance_active(0, 1, budget)?);
                assert!(original.source.instance_active(0, 2, budget)?);
                let floor = budget.storage();
                budget.reserve_storage(std::mem::size_of::<ProductionOptimizedSourceCfgRootV18<'_, '_>>())?;
                let cfg = optimized.output_root_cfg_v18(0, budget)?;
                let mut merged = 0;
                let mut merged_pointer_parameters = 0;
                let mut pointer_edge_roles = 0;
                let mut duplicate_edges = 0;
                cfg.visit(&mut OutputCfgMeterV18 { source: original.source, budget }, |event, meter| {
                    match event {
                        ProductionOptimizedSourceCfgEventV18::Block { segments, .. } => {
                            for segment in segments.iter().skip(1) {
                                meter.charge_work(1)?;
                                merged += 1;
                                let function = &original.inventory.functions()[segment.input.function.0 as usize];
                                let block = &original.inventory.blocks()[function.blocks.start + segment.input.block as usize];
                                for parameter in &original.inventory.definitions()[block.parameters.clone()] {
                                    meter.charge_work(1)?;
                                    if matches!(parameter.ty, Type::Pointer(_)) { merged_pointer_parameters += 1; }
                                }
                            }
                        }
                        ProductionOptimizedSourceCfgEventV18::Terminator { edges, arguments, argument_origins, .. } => {
                            for pair in edges.windows(2) {
                                meter.charge_work(1)?;
                                if pair[0].target == pair[1].target {
                                    assert_ne!(pair[0].coordinate, pair[1].coordinate);
                                    duplicate_edges += 1;
                                }
                            }
                            for (argument, origin) in arguments.iter().zip(argument_origins) {
                                meter.charge_work(1)?;
                                assert_eq!(argument.coordinate, origin.output);
                                let row = &cfg.inventory().definitions()[argument.incoming_definition];
                                if matches!(row.ty, Type::Pointer(_)) {
                                    pointer_edge_roles += 1;
                                    assert_eq!(row.value, Some(argument.value));
                                }
                            }
                        }
                        ProductionOptimizedSourceCfgEventV18::Operation { .. } => {}
                    }
                    Ok(())
                })?;
                drop(cfg);
                budget.release_storage(budget.storage() - floor)?;
                assert!(merged > 0, "actual optimizer must merge the expanded helper blocks");
                assert!(merged_pointer_parameters > 0, "real merged pointer parameters, not a nominal source graph");
                if graph == 3 { assert!(duplicate_edges >= 2, "both cases and default must remain distinct output edges"); }
                if matches!(graph, 1 | 2) { assert!(pointer_edge_roles > 0, "diamond/loop must retain actual pointer edge payloads"); }
                original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                    analyses.with_memory_versions(budget, |input, output, budget| {
                        let (aliases, uses) = scoped_raw_admission_v29::test_optimized_alias_census_v18(
                            original, optimized, input, output, budget)?;
                        assert!(aliases >= merged_pointer_parameters,
                            "each eliminated pointer parameter must receive a logical currentness definition");
                        assert!(uses > 0);
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original, optimized, 0, input, output, budget, |checked, budget| {
                                let mut count = 0;
                                checked.visit_accesses(budget, |_, _, _, actual, budget| {
                                    budget.charge_work(1)?;
                                    if actual.is_some() { count += 1; }
                                    Ok(())
                                })?;
                                assert!(count > 0);
                                completed.set(true);
                                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                            },
                        )
                    })
                })
            });
            assert!(entered.get(), "graph={graph}, immutable={immutable}: {result:?}");
            assert!(completed.get(), "graph={graph}, immutable={immutable}: {result:?}");
            result.unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

fn same_allocation_alias_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let owner = physical_address_owner(PhysicalAddressCase::Stored);
    let semantic = owner.source_semantic();
    let original = &semantic.functions()[0];
    let raw = original.locals()[3].ty();
    assert_eq!(original.locals()[4].ty(), raw);
    let first = &original.blocks()[0];
    let mut statements = first.statements().to_vec();
    assert!(matches!(statements[5].kind(), SemanticStatementKindV1::Assign(assignment)
        if assignment.destination().local().index() == 4
            && matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
                if place.local().index() == 6)));
    // Two authentic AddressOf occurrences in one live activation fold to the
    // same backing. Distinct static StorageLive generations must not alias.
    statements[5] = assign(place(4, raw), SemanticRvalueKindV1::AddressOf {
        mutability: SemanticMutabilityV1::Mutable, place: place(2, U32),
    });
    statements.insert(7, SemanticStatementV1::new(source(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(3, raw), SemanticOperandV1::Copy(place(4, raw)),
            SemanticVolatilityV1::NonVolatile, None))));
    let mut blocks = original.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(first.identity(), first.source(), statements,
        first.terminator().clone()).unwrap();
    let root = SemanticFunctionDeclV1::new(original.identity(), original.role(), original.item_definition_identity(),
        original.monomorphization_identity(), original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(), original.source(), original.abi().clone(),
        original.locals().to_vec(), original.entry(), blocks).unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
    let admitted = InertSemanticMirRequestV1::new_with_callables(semantic.target(), semantic.types().to_vec(),
        vec![], vec![], vec![], functions, semantic.callables().to_vec(), semantic.roots().to_vec())
        .unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn optimized_same_allocation_source_use_and_merge_lineage_require_the_independent_transition_checker() {
    use fe2o3_kernel_analysis::{check_canonical_kir_transition_v18, CanonicalKirTransitionErrorV1 as TransitionError};
    use fe2o3_kernel_ir::{CanonicalKirTransitionCandidateV1 as Candidate, CanonicalKirUseCoordinateV1 as Usage};
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(
            same_allocation_alias_source_owner_v18, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result = with_actual_optimized_transition_v18(prepared, &mut budget, |source, checked, budget| {
            let floor = budget.storage();
            budget.reserve_storage(2 * std::mem::size_of::<Vec<usize>>() + std::mem::size_of::<Candidate<'_>>())?;
            let rows = checked.rows();
            let mut uses = emission_vec_v1(rows.uses.len(), budget).unwrap();
            let mut segments = emission_vec_v1(rows.segments.len(), budget).unwrap();
            budget.charge_work(rows.uses.len() + rows.segments.len())?;
            uses.extend_from_slice(rows.uses);
            segments.extend_from_slice(rows.segments);
            let (accepted, receipt) = check_canonical_kir_transition_v18(
                checked.input(), checked.output(), rows, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            assert!(!accepted.grants_authority());
            drop(accepted);
            budget.release_storage(receipt.retained_storage())?;
            let mut pair = None;
            for (index, row) in uses.iter().enumerate() {
                budget.charge_work(1)?;
                let Usage::OperationOperand { operation, operand: 1 } = row.output else { continue; };
                let function = &checked.output().functions()[operation.block.function.0 as usize];
                let block = &checked.output().blocks()[function.blocks.start + operation.block.block as usize];
                let actual = &checked.output().operations()[block.operations.start + operation.operation as usize];
                if !matches!(actual.operation.kind, OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. })) { continue; }
                let use_row = &checked.output().uses()[actual.operands.start + 1];
                if !matches!(checked.output().definitions()[use_row.definition].ty, Type::Pointer(_)) { continue; }
                for (earlier, prior) in uses[..index].iter().enumerate() {
                    budget.charge_work(1)?;
                    let Usage::OperationOperand { operation: original, operand: 1 } = prior.output else { continue; };
                    let function = &checked.output().functions()[original.block.function.0 as usize];
                    let block = &checked.output().blocks()[function.blocks.start + original.block.block as usize];
                    let actual = &checked.output().operations()[block.operations.start + original.operation as usize];
                    if matches!(actual.operation.kind, OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }))
                        && checked.output().uses()[actual.operands.start + 1].value == use_row.value
                        && prior.input != row.input
                    { pair = Some((earlier, index)); }
                }
            }
            let (earlier, later) = pair.expect("distinct live aliases share one folded backing but different original holder-store uses");
            let expected = match fault {
                0 => { uses.remove(later); TransitionError::IncompleteRows }
                1 => {
                    uses[later].input = uses[earlier].input;
                    TransitionError::Rule("final operation operand origin")
                }
                2 => {
                    let Usage::OperationOperand { mut operation, operand } = uses[later].input else { panic!("source store payload role"); };
                    operation.block.function.0 = u32::MAX;
                    uses[later].input = Usage::OperationOperand { operation, operand };
                    TransitionError::InvalidCoordinate
                }
                3 => {
                    segments.iter_mut().find_map(|row| row.connector.as_mut())
                        .expect("actual checked merge connector").successor = u32::MAX;
                    TransitionError::InvalidCoordinate
                }
                _ => unreachable!(),
            };
            let candidate = Candidate { uses: &uses, segments: &segments, ..rows };
            let rejected = check_canonical_kir_transition_v18(checked.input(), checked.output(), candidate, budget).unwrap_err();
            assert_eq!(rejected, expected, "fault {fault}");
            drop((uses, segments));
            budget.release_storage(budget.storage() - floor)?;
            assert_eq!(source.root_count(budget)?, 1, "rejecting inert candidates must not mutate original authority");
            completed.set(true);
            Ok(())
        });
        assert!(completed.get(), "fault={fault}: {result:?}");
        result.unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
