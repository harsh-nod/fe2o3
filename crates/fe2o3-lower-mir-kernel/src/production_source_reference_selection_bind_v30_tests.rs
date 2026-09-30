// These holders exercise real source emission without admitting a selected
// memory access before the conditional leaf and final consumers exist.
fn full_selection_bind_owner(entry_loop: bool) -> ProductionSemanticSsaOwnerV1 {
    let prior = if entry_loop {
        entry_loop_selection_owner()
    } else {
        selection_owner(false, false)
    };
    let source = prior.source_semantic();
    let mut functions = source.functions().to_vec();
    let selected = usize::from(entry_loop);
    let original = &functions[selected];
    let mut blocks = original.blocks().to_vec();
    if entry_loop {
        let mut statements = blocks[0].statements().to_vec();
        statements[0] = assign(
            3,
            REFERENCE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, REFERENCE))),
        );
        statements[1] = assign(
            2,
            U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                U32,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
            ))),
        );
        blocks[0] = block(100, statements, blocks[0].terminator().kind().clone());
    } else {
        let SemanticStatementKindV1::Assign(previous) = blocks[8].statements()[0].kind() else {
            panic!("selected reference assignment");
        };
        blocks[8] = block(
            8,
            vec![assign(
                previous.destination().local().index(),
                REFERENCE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(7, REFERENCE))),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 9)),
        );
    }
    functions[selected] = rebuild(original, original.locals().to_vec(), blocks);
    admitted_owner(
        source.types().to_vec(),
        functions,
        source.callables().to_vec(),
    )
}

thread_local! {
    static FULL_SELECTION_BIND_ENTRY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FULL_SELECTION_BIND_PHASES: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
}

fn query_full_selection_bind(
    pending: &PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let entry = FULL_SELECTION_BIND_ENTRY.get();
    let instance = if entry {
        ProductionCallInstanceIdV1(1)
    } else {
        instances.root()
    };
    let block = if entry { 0 } else { 8 };
    let function = instances.instance(instance).unwrap().declaration();
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[block].statements()[0].kind()
    else {
        panic!("original selected copy");
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)) = assignment.value().kind()
    else {
        panic!("original selected holder use");
    };
    let floor = budget.storage();
    let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
    let result = with_source_reference_selection_actual_v30(
        plan,
        &index,
        instance,
        site(block as u32),
        ExecutionOperandV29::RvalueOperand(0),
        source,
        budget,
        |graph, actual, _| {
            let SourceReferenceSelectionStepV29::Parameter {
                invocation, count, ..
            } = graph.nodes[0].step
            else {
                panic!("the full binder must see the original selection parameter");
            };
            assert_eq!(count, if entry { 1 } else { 2 });
            assert_eq!(invocation.is_some(), entry);
            assert_eq!(actual.nodes[0].invocation.is_some(), entry);
            assert_eq!(actual.edges.len(), count);
            assert!(actual.edges.iter().all(|edge| edge.argument.is_some()));
            let leaves = graph
                .nodes
                .iter()
                .filter(|node| matches!(node.step, SourceReferenceSelectionStepV29::Leaf(_)))
                .count();
            assert_eq!(leaves, if entry { 1 } else { 2 });
            Ok(())
        },
    );
    index.discard(budget)?;
    assert_eq!(budget.storage(), floor);
    result
}

fn observe_full_selection_bind(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let observation = SOURCE_EMISSION_OBSERVATION_V29.get().unwrap();
    assert_eq!(
        observation.owner,
        std::ptr::from_ref(instances.owner()) as usize
    );
    assert_eq!(observation.slot, std::ptr::from_ref(budget) as usize);
    assert!(observation.ledger == budget.work_ledger_identity_v1());
    let phase = match observation.phase {
        SourceEmissionPhaseV29::Admission => 0,
        SourceEmissionPhaseV29::ConstructionReplay => 1,
        SourceEmissionPhaseV29::ConsumerReplay => panic!("materialization phases only"),
    };
    query_full_selection_bind(pending, instances, plan, budget)?;
    let entry = FULL_SELECTION_BIND_ENTRY.get();
    let instance = if entry {
        ProductionCallInstanceIdV1(1)
    } else {
        instances.root()
    };
    let block = SemanticBlockIdV1::from_index(if entry { 0 } else { 8 });
    let index = pending
        .coordinates
        .controls
        .rows
        .iter()
        .position(|row| {
            row.instance == instance
                && row.semantic_block == Some(block)
                && row.origin == InstanceControlOriginV1::Retained
        })
        .unwrap();
    let original = pending.coordinates.controls.rows[index].original_block;
    pending.coordinates.controls.rows[index].original_block = BlockId(u32::MAX);
    assert_actual_selection_error(query_full_selection_bind(pending, instances, plan, budget));
    pending.coordinates.controls.rows[index].original_block = original;
    query_full_selection_bind(pending, instances, plan, budget)?;
    if entry {
        let sidecar = pending
            .sidecars
            .rows
            .iter()
            .position(|row| row.source_call_instance == Some(instance))
            .unwrap();
        let component = pending.sidecars.rows[sidecar]
            .invocation_entry
            .as_ref()
            .unwrap()
            .arguments
            .iter()
            .find(|row| row.original.variable().get() == 1)
            .unwrap()
            .first_component;
        let original = pending.sidecars.rows[sidecar]
            .invocation_entry
            .as_ref()
            .unwrap()
            .components[component]
            .parameter;
        pending.sidecars.rows[sidecar]
            .invocation_entry
            .as_mut()
            .unwrap()
            .components[component]
            .parameter = ValueId(u32::MAX);
        assert_actual_selection_error(query_full_selection_bind(pending, instances, plan, budget));
        pending.sidecars.rows[sidecar]
            .invocation_entry
            .as_mut()
            .unwrap()
            .components[component]
            .parameter = original;
        query_full_selection_bind(pending, instances, plan, budget)?;
    }
    let mut counts = FULL_SELECTION_BIND_PHASES.get();
    counts[phase] += 1;
    FULL_SELECTION_BIND_PHASES.set(counts);
    Ok(())
}

fn install_full_selection_bind_observer(
    _: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    _: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(Some(observe_full_selection_bind));
    Ok(())
}

fn run_full_selection_bind(entry: bool) {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    struct Restore(
        Option<ScopedSlotCustodyObserverV29>,
        Option<RootExecutionArchiveObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(install_full_selection_bind_observer)),
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(None),
    );
    FULL_SELECTION_BIND_ENTRY.set(entry);
    FULL_SELECTION_BIND_PHASES.set([0; 2]);
    let owner = full_selection_bind_owner(entry);
    let occurrence_storage = owner.occurrence_storage().unwrap().retained_storage();
    let semantic = owner.source_semantic();
    let hash = *owner.source_semantic_sha256();
    let binding = *semantic.functions()[0]
        .kernel_entry()
        .unwrap()
        .kernel_binding_identity()
        .as_bytes();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "issued_pointer_source",
            binding,
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let arguments = ["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| ProductionKernelArgumentAbiArgumentV18 {
            semantic_type_identity: semantic.types()[CARRIER.index() as usize].identity(),
            kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
                argument: LogicalArgumentV1::disjoint_slice(
                    ordinal as u16,
                    ValidName::new(name).unwrap(),
                    &source,
                    &layout,
                    fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                    (ordinal * 16) as u32,
                )
                .unwrap(),
            },
        })
        .collect::<Vec<_>>();
    let roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: &binding,
        export: "issued_pointer_source",
        arguments: &arguments,
        explicit_argument_bytes: 32,
        kernarg_alignment_bytes: 8,
    }];
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &hash,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000_000);
    budget.reserve_storage(37 + occurrence_storage).unwrap();
    let pending =
        ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
            owner,
            launch,
            input,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap_or_else(|error| panic!("full selection binding entry={entry}: {error:?}"));
    assert_eq!(FULL_SELECTION_BIND_PHASES.get(), [1, 1]);
    let geps = pending
        .pending_module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::GetElementPointer { .. }))
        .count();
    assert_eq!(geps, if entry { 1 } else { 2 });
    assert!(
        pending
            .pending_module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            ))
    );
    let retained = pending.adopted_storage();
    assert_eq!(budget.storage(), 37 + occurrence_storage + retained);
    drop(pending);
    budget.release_storage(retained).unwrap();
    budget.release_storage(occurrence_storage).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn distinct_reference_diamond_full_source_archive_binding_rejects_changed_controls() {
    run_full_selection_bind(false);
}

#[test]
fn entry_reference_loop_full_source_archive_binding_rejects_changed_invocation_component() {
    run_full_selection_bind(true);
}

#[test]
fn selected_control_and_parallel_join_indexes_have_logarithmic_matching_work() {
    let mut previous = None;
    for count in [64usize, 256, 1024] {
        let controls = (0..count)
            .rev()
            .map(|index| InstanceControlV1 {
                instance: ProductionCallInstanceIdV1(0),
                original_block: BlockId(index as u32),
                semantic_block: Some(SemanticBlockIdV1::from_index(index as u32)),
                physical_block: BlockId((index + count) as u32),
                origin: InstanceControlOriginV1::Retained,
                return_values: None,
                expected_branch: None,
            })
            .collect::<Vec<_>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        with_canonical_call_scratch_v1(&mut budget, |budget| {
            budget.reserve_storage(source_reference_selection_actual_headers_v30()?)?;
            let rows = source_reference_selection_control_index_v30(&controls, budget)?;
            for index in 0..count {
                let row = source_reference_selection_control_v30(
                    &rows,
                    ProductionCallInstanceIdV1(0),
                    SemanticBlockIdV1::from_index(index as u32),
                    budget,
                )?;
                assert_eq!(row.terminal, BlockId((index + count) as u32));
            }
            Ok::<_, ProductionSemanticKirErrorV1>(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 17);
        let used = budget.work();
        assert!(
            used <= 64 * count * (count.ilog2() as usize + 1) + 4096,
            "control count={count}, work={used}"
        );
        if let Some(prior) = previous {
            assert!(used <= 6 * prior);
        }
        previous = Some(used);
    }
    with_plan(&selection_owner(false, false), |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, _| {
                let mut previous = None;
                for count in [64usize, 256, 1024] {
                    let (mut function, mut bound) = actual_selection_fixture(graph, false);
                    function.signature.parameters[2] = Type::U32;
                    let inputs = [bound.edges[0].original.input, bound.edges[1].original.input];
                    let source_edge = bound.edges[0];
                    bound.edges = (0..count)
                        .map(|ordinal| {
                            let mut row = source_edge;
                            row.original.edge =
                                SsaEdgeIdV1::new(SsaBlockIdV1::new(3), ordinal as u32);
                            row.original.input = inputs[ordinal % 2];
                            row.source = BlockId(10);
                            row.ordinal = ordinal;
                            row
                        })
                        .collect();
                    let SourceReferenceSelectionStepV29::Parameter {
                        count: incoming, ..
                    } = &mut bound.nodes[0].original.step
                    else {
                        panic!("original parameter fixture");
                    };
                    *incoming = count;
                    let body = function.body.as_mut().unwrap();
                    body.blocks
                        .retain(|block| !matches!(block.id, BlockId(11) | BlockId(12)));
                    body.blocks[0].terminator = Some(Terminator::Switch {
                        selector: ValueId(2),
                        cases: (0..count - 1)
                            .map(|ordinal| fe2o3_kernel_ir::SwitchCase {
                                value: ordinal as u64,
                                target: BlockId(13),
                                arguments: vec![ValueId((ordinal % 2) as u32)],
                            })
                            .collect(),
                        default_target: BlockId(13),
                        default_arguments: vec![ValueId(((count - 1) % 2) as u32)],
                    });
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                    budget.reserve_storage(17).unwrap();
                    with_canonical_call_scratch_v1(&mut budget, |budget| {
                        budget.reserve_storage(source_reference_selection_actual_headers_v30()?)?;
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function, &actual, &mut bound, budget,
                        )
                    })
                    .unwrap();
                    assert_eq!(budget.storage(), 17);
                    assert_eq!(
                        bound
                            .edges
                            .iter()
                            .filter(|edge| edge.argument.is_some())
                            .count(),
                        count
                    );
                    let used = budget.work();
                    assert!(
                        used <= 128 * count * (count.ilog2() as usize + 1) + 65536,
                        "parallel count={count}, work={used}"
                    );
                    if let Some(prior) = previous {
                        assert!(
                            used <= 6 * prior,
                            "parallel count={count}: {used} vs {prior}"
                        );
                    }
                    previous = Some(used);
                }
                Ok(())
            },
        )
        .unwrap();
    });
}
