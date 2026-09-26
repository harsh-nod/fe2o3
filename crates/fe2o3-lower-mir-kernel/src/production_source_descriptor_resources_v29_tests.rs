// These thresholds are derived from the cached two-origin union schedule,
// independently of a measured successful production request.
fn descriptor_union_scratch_v29() -> usize {
    2 * size_of::<SourceDescriptorUnionV29>()
        + size_of::<SourceDescriptorSetKeyV29>()
        + 3 * size_of::<Option<SourceDescriptorOriginV29>>()
        + 2 * size_of::<Result<Option<SourceDescriptorOriginV29>, ProductionSemanticKirErrorV1>>()
}

#[test]
fn actual_descriptor_fixed_point_retains_whole_root_alternatives_and_unknown() {
    for flow in [DescriptorFlowV29::Branch, DescriptorFlowV29::Loop, DescriptorFlowV29::Mixed] {
        let mut owner = descriptor_flow_owner_v29(flow);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let capture = owner.try_capture_occurrences_with_budget_v1(&mut budget).unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let roots = fixture.roots();
        production_call_instances_v1::with_production_call_instances_v1(
            &owner, SemanticFunctionIdV1::from_index(0), &mut budget, |instances, budget| {
                let floor = budget.storage();
                budget.reserve_storage(size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()
                    + size_of::<SourceReferenceBuilderV29<'_, '_, '_>>()).unwrap();
                let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                    &owner, ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget,
                ).unwrap();
                let lens = profile.descriptor_root(&owner, 0, budget).unwrap();
                let plan = SourceReferenceBuilderV29::new_with_descriptors(
                    instances, SourceReferenceStorageV29::ScalarCells, None, Some(lens), budget,
                ).unwrap().build(budget).unwrap();
                let block = if matches!(flow, DescriptorFlowV29::Loop) { 1 } else { 3 };
                let entry = plan.blocks.iter().find(|entry|
                    entry.instance == instances.root() && entry.block.index() == block).unwrap();
                let node = plan.states[entry.entry][4].node.unwrap();
                let descriptor = plan.descriptor_sets[plan.nodes[node].descriptor.unwrap()];
                let origins = &plan.descriptor_origins[descriptor.first..descriptor.first + descriptor.count];
                let mixed = matches!(flow, DescriptorFlowV29::Mixed);
                assert_eq!(descriptor.has_unknown, mixed);
                assert_eq!(descriptor.representation, if mixed { AddressSpace::Generic } else { AddressSpace::Global });
                assert_eq!(origins.iter().map(|row| row.original_argument).collect::<Vec<_>>(),
                    if mixed { vec![0] } else { vec![0, 1] });
                assert!(origins.iter().all(|row| row.data_component == 0 && row.length_component == 1));
                drop(plan);
                drop(profile);
                budget.release_storage(budget.storage() - floor).unwrap();
                Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        ).unwrap();
        assert_eq!(budget.storage(), capture.retained_storage());
        drop(roots);
        drop(fixture);
        drop(owner);
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn correlated_descriptor_union_interning_has_independent_exact_resource_boundaries() {
    for boundary in 0..5 {
        let mut owner = descriptor_flow_owner_v29(DescriptorFlowV29::Root);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let capture = owner.try_capture_occurrences_with_budget_v1(&mut budget).unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
        let roots = fixture.roots();
        let mut checked = false;
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            SemanticFunctionIdV1::from_index(0),
            &mut budget,
            |instances, budget| {
                let floor = budget.storage();
                budget
                    .reserve_storage(
                        size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()
                            + size_of::<SourceReferenceBuilderV29<'_, '_, '_>>(),
                    )
                    .unwrap();
                let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                    &owner,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .unwrap();
                let lens = profile.descriptor_root(&owner, 0, budget).unwrap();
                let mut builder = SourceReferenceBuilderV29::new_with_descriptors(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    None,
                    Some(lens),
                    budget,
                )
                .unwrap();
                builder.collect_storage_selectors(budget).unwrap();
                builder.function(instances.root(), None, budget).unwrap();
                assert_eq!(builder.plan.descriptor_sets.len(), 2);
                let a = builder.plan.descriptor_sets[0];
                let b = builder.plan.descriptor_sets[1];
                assert_eq!((a.count, b.count), (1, 1));
                assert_eq!(
                    builder.plan.descriptor_origins[a.first].original_argument,
                    0
                );
                assert_eq!(
                    builder.plan.descriptor_origins[b.first].original_argument,
                    1
                );
                let atoms = |row: SourceDescriptorSetV29| SourceDescriptorAtomsV29::Retained {
                    first: row.first,
                    count: row.count,
                };
                let united = builder
                    .intern_descriptor_set(atoms(a), atoms(b), false, AddressSpace::Global, budget)
                    .unwrap();
                assert_eq!(builder.plan.descriptor_sets.len(), 3);
                let rows = builder.plan.descriptor_origins.len();
                let retained_capacities = (
                    builder.plan.descriptor_sets.capacity(),
                    builder.plan.descriptor_origins.capacity(),
                );
                // Three iterator polls, two hash steps, lookup in a three-row
                // index, one collision candidate, then exact two-row equality.
                let expected_work = 3 * 2 + 2 * 3 + (1 + 2) * 16 + 1 + 3 * 2 + 2;
                assert_eq!(expected_work, 69);
                let scratch = descriptor_union_scratch_v29();
                if matches!(boundary, 1 | 2) {
                    budget
                        .charge_work(
                            MODULE_LIMIT - budget.work() - expected_work
                                + usize::from(boundary == 2),
                        )
                        .unwrap();
                }
                if matches!(boundary, 3 | 4) {
                    budget
                        .reserve_storage(
                            MODULE_LIMIT - budget.storage() - scratch + usize::from(boundary == 4),
                        )
                        .unwrap();
                }
                let before_work = budget.work();
                let before_storage = budget.storage();
                let result = builder.intern_descriptor_set(
                    atoms(b),
                    atoms(a),
                    false,
                    AddressSpace::Global,
                    budget,
                );
                match boundary {
                    2 => assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    )),
                    4 => assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    )),
                    _ => {
                        assert_eq!(result.unwrap(), united);
                        assert_eq!(budget.work() - before_work, expected_work);
                        assert_eq!(budget.storage() - before_storage, scratch);
                    }
                }
                assert_eq!(builder.plan.descriptor_sets.len(), 3);
                assert_eq!(builder.plan.descriptor_origins.len(), rows);
                assert_eq!(
                    (
                        builder.plan.descriptor_sets.capacity(),
                        builder.plan.descriptor_origins.capacity()
                    ),
                    retained_capacities
                );
                if boundary == 0 {
                    for _ in 0..128 {
                        assert_eq!(
                            builder
                                .intern_descriptor_set(
                                    atoms(a),
                                    atoms(b),
                                    false,
                                    AddressSpace::Global,
                                    budget
                                )
                                .unwrap(),
                            united
                        );
                    }
                    assert_eq!(builder.plan.descriptor_sets.len(), 3);
                    assert_eq!(builder.plan.descriptor_origins.len(), rows);
                }
                checked = true;
                drop(builder);
                drop(profile);
                budget.release_storage(budget.storage() - floor).unwrap();
                Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        assert!(checked);
        assert_eq!(budget.storage(), MODULE_FLOOR + capture.retained_storage());
        drop(roots);
        drop(fixture);
        drop(owner);
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn descriptor_c1_queries_preserve_original_ledger_and_sticky_custody_failure() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        other.reserve_storage(73).unwrap();
        let mut checked = false;
        with_descriptor_flow_prepared_v29(
            DescriptorFlowV29::Mixed,
            true,
            false,
            &mut budget,
            |prepared, budget| {
                let result = prepared.with_checked_source_v18(budget, |source, budget| {
                    assert_eq!(source.kernel_argument_abi_count(0, budget)?, Some(4));
                    let floor = budget.storage();
                    if foreign {
                        assert!(matches!(
                            source.canonical(&mut other),
                            Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                        ));
                        assert_eq!(other.storage(), 73);
                    } else {
                        budget.release_storage(1)?;
                        assert!(matches!(
                            source.canonical(budget),
                            Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                        ));
                        budget.reserve_storage(1)?;
                    }
                    let work = budget.work();
                    assert!(matches!(
                        source.canonical(budget),
                        Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                    ));
                    assert_eq!(budget.work(), work);
                    assert_eq!(budget.storage(), floor);
                    checked = true;
                    Ok(())
                });
                assert!(matches!(
                    result,
                    Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                ));
            },
        );
        assert!(checked);
        assert!(
            budget.storage() >= MODULE_FLOOR,
            "caught custody refusal must not authorize a refund from another request"
        );
        assert_eq!(other.storage(), 73);
    }
}

fn descriptor_traversal_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = descriptor_flow_owner_v29(DescriptorFlowV29::Root);
    let semantic = original.source_semantic();
    let source = &semantic.functions()[0];
    let slice = source.abi().source_input_types()[0];
    let word = source.abi().source_input_types()[2];
    let mut types = semantic.types().to_vec();
    let mut fields = vec![word; 254];
    fields.push(slice);
    let tuple = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(254 * 8 + 16),
            8,
            SemanticAggregateLayoutV1::new((0..255).map(|field| field * 8).collect(), vec![])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
        None,
    );
    let mut operands = vec![SemanticOperandV1::Copy(place(3, word)); 254];
    operands.push(SemanticOperandV1::Copy(place(1, slice)));
    let mut locals = source.locals().to_vec();
    locals[4] = local(206, tuple, SemanticLocalRoleV1::Temporary);
    let root = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        source.abi().clone(),
        locals,
        vec![block(
            211,
            vec![assign(
                place(4, tuple),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple, operands)
                        .unwrap(),
                ),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .with_kernel_entry(source.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
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

fn with_descriptor_traversal_plan_v29(
    profiled: bool,
    inspect: impl FnOnce(
        &mut SourceReferencePlanV29<'_, '_>,
        usize,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Option<usize> {
    let mut owner = descriptor_traversal_owner_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let capture = owner.try_capture_occurrences_with_budget_v1(&mut budget).unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let roots = fixture.roots();
    let mut checked = false;
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            budget
                .reserve_storage(
                    size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()
                        + size_of::<SourceReferenceBuilderV29<'_, '_, '_>>(),
                )
                .unwrap();
            let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                &owner,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .unwrap();
            let lens = profile.descriptor_root(&owner, 0, budget).unwrap();
            let mut plan = SourceReferenceBuilderV29::new_with_descriptors(
                instances,
                SourceReferenceStorageV29::ScalarCells,
                None,
                profiled.then_some(lens),
                budget,
            )
            .unwrap()
            .build(budget)
            .unwrap();
            let tuple = instances.instance(instances.root()).unwrap().declaration().locals()[4].ty();
            let node = plan
                .nodes
                .iter()
                .position(|row| row.ty == tuple && matches!(row.kind, SourceReferenceNodeKindV29::Aggregate { .. }))
                .unwrap();
            let SourceReferenceNodeKindV29::Aggregate { first, count } = plan.nodes[node].kind else {
                panic!("actual source aggregate");
            };
            assert_eq!(count, 255);
            let children = &plan.children[first..first + count];
            assert!(children.iter().all(|child| *child < node));
            assert!(children[..254].iter().all(|child| plan.nodes[*child].descriptor.is_none()));
            let descriptor = plan.nodes[children[254]].descriptor;
            assert_eq!(descriptor.is_some(), profiled);
            if let Some(descriptor) = descriptor {
                let row = plan.descriptor_sets[descriptor];
                assert_eq!((row.count, row.has_unknown, row.representation), (1, false, AddressSpace::Global));
                let origin = plan.descriptor_origins[row.first];
                assert_eq!((origin.original_argument, origin.data_component, origin.length_component), (0, 0, 1));
            }
            inspect(&mut plan, node, budget);
            checked = true;
            drop(plan);
            drop(profile);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    assert!(checked);
    assert_eq!(budget.storage(), MODULE_FLOOR + capture.retained_storage());
    drop(roots);
    drop(fixture);
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
    drop(budget);
    work.failed_work()
}

#[test]
fn descriptor_presence_traversal_has_independent_exact_256_node_work_boundaries() {
    // One aggregate and 255 leaves. Each node debits one unit; each child
    // debits five for owner custody and one for the edge, including the last.
    const WORK: usize = 256 + 255 * (5 + 1);
    assert_eq!(WORK, 1786);
    for short in [false, true] {
        for prior_denial in [false, true] {
            let failed = with_descriptor_traversal_plan_v29(true, |plan, node, budget| {
                if prior_denial {
                    let rejected = MODULE_LIMIT + 37 - budget.work();
                    assert!(matches!(budget.charge_work(rejected), Err(ArgumentResourceV1::Work(_))));
                }
                budget.charge_work(MODULE_LIMIT - budget.work() - WORK + usize::from(short)).unwrap();
                let before_work = budget.work();
                let floor = budget.storage();
                assert!(plan.failure.first_error().is_none());
                let mut nodes = 0;
                let result = source_descriptor_node_present_v29(plan, node, &mut nodes, budget);
                if short {
                    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                    assert_eq!(nodes, 255, "refused debit precedes visiting node 256");
                } else {
                    assert!(result.unwrap());
                    assert_eq!(nodes, 256);
                }
                assert_eq!(budget.work() - before_work, WORK - usize::from(short));
                assert_eq!(budget.storage(), floor);
                assert!(plan.failure.first_error().is_none(), "the terminal node debit does not replace source failure history");
            });
            assert_eq!(failed, if prior_denial { Some(MODULE_LIMIT + 37) } else { short.then_some(MODULE_LIMIT + 1) });
        }
    }
}

#[test]
fn descriptor_presence_traversal_refuses_deep_257_and_non_older_children() {
    for hostile in 0..3 {
        assert_eq!(with_descriptor_traversal_plan_v29(true, |plan, original, budget| {
            let SourceReferenceNodeKindV29::Aggregate { first, count } = plan.nodes[original].kind else {
                panic!("actual source aggregate");
            };
            let mut node = original;
            // Only hostile topology is assembled here, after actual owner/profile
            // construction. It never supplies an admission or a memory proof.
            if hostile == 0 {
                node = plan.children[first + count - 1];
                for _ in 0..256 {
                    let child = node;
                    let first = plan.children.len();
                    emission_push_v1(&mut plan.children, child, budget).unwrap();
                    let mut row = plan.nodes[original];
                    row.kind = SourceReferenceNodeKindV29::Aggregate { first, count: 1 };
                    node = plan.nodes.len();
                    emission_push_v1(&mut plan.nodes, row, budget).unwrap();
                    assert!(child < node);
                }
            } else if hostile == 1 {
                plan.children[first] = original;
            } else {
                let forward = plan.nodes.len();
                let row = plan.nodes[plan.children[first + count - 1]];
                emission_push_v1(&mut plan.nodes, row, budget).unwrap();
                plan.children[first] = forward;
                assert!(forward > original);
            }
            let before_work = budget.work();
            let floor = budget.storage();
            let mut nodes = 0;
            let error = source_descriptor_node_present_v29(plan, node, &mut nodes, budget).unwrap_err();
            if hostile == 0 {
                assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported { .. }));
                assert_eq!(nodes, 257);
                assert_eq!(budget.work() - before_work, 257 + 256 * (5 + 1));
            } else {
                assert!(matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)));
                assert_eq!(nodes, 1);
                assert_eq!(budget.work() - before_work, 1 + 5 + 1);
            }
            assert_eq!(budget.storage(), floor);
            assert!(plan.failure.first_error().is_none());
        }), None);
    }
}

#[test]
fn descriptor_presence_missing_profile_and_prior_source_failure_do_not_mint_authority() {
    assert_eq!(with_descriptor_traversal_plan_v29(false, |plan, node, budget| {
        assert!(plan.descriptor_sets.is_empty());
        assert!(plan.descriptor_origins.is_empty());
        let before_work = budget.work();
        let floor = budget.storage();
        let mut nodes = 0;
        assert!(!source_descriptor_node_present_v29(plan, node, &mut nodes, budget).unwrap());
        assert_eq!(nodes, 0);
        assert_eq!(budget.work(), before_work);
        assert_eq!(budget.storage(), floor);
        assert!(plan.failure.first_error().is_none());
    }), None);
    assert_eq!(with_descriptor_traversal_plan_v29(true, |plan, node, budget| {
        plan.failure.record_resource(ArgumentResourceV1::Arithmetic);
        let SourceReferenceNodeKindV29::Aggregate { first, .. } = plan.nodes[node].kind else {
            panic!("actual source aggregate");
        };
        plan.children[first] = node;
        let floor = budget.storage();
        for _ in 0..2 {
            let before_work = budget.work();
            let mut nodes = 0;
            assert!(matches!(source_descriptor_node_present_v29(plan, node, &mut nodes, budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Arithmetic))));
            assert_eq!(nodes, 1);
            assert_eq!(budget.work() - before_work, 1);
            assert_eq!(budget.storage(), floor);
            assert!(matches!(plan.failure.first_error(),
                Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Arithmetic))));
        }
    }), None);
}
