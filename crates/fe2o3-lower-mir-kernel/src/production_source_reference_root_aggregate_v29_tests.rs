use super::*;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Mixed(Input),
    Aggregate,
    Tuple,
    Wide,
    Deep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flow {
    SiblingRead,
    MovedRead,
    RestoreOne,
    RestoreBoth,
    Branch,
    PartialBranch,
    Loop,
}

fn pair(types: &mut Vec<SemanticTypeDeclV1>, tuple: bool, count: usize) -> SemanticTypeIdV1 {
    let fields = SemanticAggregateTypeV1::new(vec![U32; count]).unwrap();
    declaration(
        types,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(count as u64 * 4),
            4,
            SemanticBackendReprV1::Memory { sized: true },
            false,
            SemanticAggregateLayoutV1::new(
                (0..count as u64).map(|field| field * 4).collect(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        if tuple {
            SemanticTypeShapeV1::Tuple(fields)
        } else {
            SemanticTypeShapeV1::Aggregate(fields)
        },
        None,
    )
}

fn owner(
    shape: Shape,
    flow: Flow,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    let base = logical_owner(
        match shape {
            Shape::Mixed(input) => input,
            _ => Input::CapturedContext,
        },
        true,
        LogicalFlow::Reinitialize,
    )
    .unwrap();
    let source = base.source_semantic();
    let mut types = source.types().to_vec();
    let original = source.functions()[0].locals()[1].ty();
    let mixed = match types[original.index() as usize].shape() {
        SemanticTypeShapeV1::Aggregate(fields) => fields.fields()[0],
        _ => panic!("original nested fixture changed"),
    };
    let (inner, selected, mut tail) = match shape {
        Shape::Mixed(_) => (mixed, original, vec![field(1, U32)]),
        Shape::Aggregate | Shape::Tuple | Shape::Wide => {
            let width = if matches!(shape, Shape::Wide) { 64 } else { 2 };
            let inner = pair(&mut types, matches!(shape, Shape::Tuple), width);
            let size = width as u64 * 4;
            let outer = aggregate(
                &mut types,
                vec![inner, inner],
                vec![0, size],
                size * 2,
                4,
                SemanticBackendReprV1::Memory { sized: true },
                None,
            );
            (inner, outer, vec![field(1, U32)])
        }
        Shape::Deep => {
            let mut inner = U32;
            let mut tail = Vec::new();
            let mut size = 4;
            for _ in 0..32 {
                tail.push(field(1, inner));
                inner = aggregate(
                    &mut types,
                    vec![U32, inner],
                    vec![0, 4],
                    size + 4,
                    4,
                    SemanticBackendReprV1::Memory { sized: true },
                    None,
                );
                size += 4;
            }
            tail.reverse();
            let outer = aggregate(
                &mut types,
                vec![inner, inner],
                vec![0, size],
                size * 2,
                4,
                SemanticBackendReprV1::Memory { sized: true },
                None,
            );
            (inner, outer, tail)
        }
    };
    let mut projections = vec![field(0, inner)];
    projections.extend_from_slice(&tail);
    let a = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, U32).unwrap();
    tail.insert(0, field(1, inner));
    let b = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), tail, U32).unwrap();
    let moving = |place: &SemanticPlaceV1, to| {
        assign(
            super::place(to, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place.clone())),
        )
    };
    let copying = |place: &SemanticPlaceV1, to| {
        assign(
            super::place(to, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place.clone())),
        )
    };
    let restore = |target: &SemanticPlaceV1, from| {
        assign(
            target.clone(),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(super::place(from, U32))),
        )
    };
    let goto = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let invoke = |target| {
        call(
            1,
            vec![SemanticOperandV1::Move(super::place(1, selected))],
            target,
        )
    };
    let choose = |left, right| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(super::place(2, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(left),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(right),
            ),
        )
        .unwrap(),
    };
    let blocks = match flow {
        Flow::SiblingRead | Flow::MovedRead => vec![
            block(60, vec![moving(&a, 2)], goto(1)),
            block(
                61,
                vec![
                    copying(if flow == Flow::SiblingRead { &b } else { &a }, 4),
                    restore(&a, 2),
                ],
                invoke(2),
            ),
            block(62, vec![], SemanticTerminatorKindV1::Return),
        ],
        Flow::RestoreOne | Flow::RestoreBoth => vec![
            block(60, vec![moving(&a, 2), moving(&b, 4)], goto(1)),
            block(61, vec![restore(&a, 2)], goto(2)),
            block(
                62,
                if flow == Flow::RestoreBoth {
                    vec![restore(&b, 4)]
                } else {
                    vec![]
                },
                invoke(3),
            ),
            block(63, vec![], SemanticTerminatorKindV1::Return),
        ],
        Flow::Branch | Flow::PartialBranch => vec![
            block(60, vec![copying(&a, 2)], choose(1, 2)),
            block(
                61,
                if flow == Flow::Branch {
                    vec![moving(&a, 2), restore(&a, 2)]
                } else {
                    vec![moving(&a, 2)]
                },
                goto(3),
            ),
            block(62, vec![], goto(3)),
            block(63, vec![], invoke(4)),
            block(64, vec![], SemanticTerminatorKindV1::Return),
        ],
        Flow::Loop => vec![
            block(60, vec![copying(&a, 2)], goto(1)),
            block(61, vec![], choose(2, 3)),
            block(62, vec![moving(&a, 2), restore(&a, 2)], goto(1)),
            block(63, vec![], invoke(4)),
            block(64, vec![], SemanticTerminatorKindV1::Return),
        ],
    };
    let abi = |tag, kernel| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            source.target_layout_identity(),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value_abi(&types, selected))],
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap()
    };
    let mut locals = vec![
        local(210, UNIT, SemanticLocalRoleV1::Return),
        local(211, selected, SemanticLocalRoleV1::Argument(0)),
        local(212, U32, SemanticLocalRoleV1::Temporary),
        local(213, CONTEXT, SemanticLocalRoleV1::Temporary),
        local(214, U32, SemanticLocalRoleV1::Temporary),
    ];
    if selected != original {
        // Preserve the original closed type roster when changing the argument.
        locals.push(local(215, original, SemanticLocalRoleV1::Temporary));
    }
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        abi(203, true),
        locals,
        blocks,
    )
    .with_kernel_entry(source.functions()[0].kernel_entry().unwrap().clone());
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        abi(205, false),
        vec![
            local(230, UNIT, SemanticLocalRoleV1::Return),
            local(231, selected, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(240, vec![], SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        source.callables().to_vec(),
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
}

struct Outcome {
    result: Result<(), ProductionSemanticKirErrorV1>,
    work: usize,
    peak: usize,
    prefix_work: usize,
    prefix_peak: usize,
    denied_work: bool,
    denied_storage: bool,
}

fn run(shape: Shape, flow: Flow, work_limit: usize, storage_limit: usize) -> Outcome {
    let mut owner = owner(shape, flow).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let table_floor = budget.storage();
    let prefix_work = budget.work();
    let prefix_peak = budget.peak_storage();
    let mut root_called = false;
    let mut callback_called = false;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            root_called = true;
            let floor = budget.storage();
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, root, budget| {
                    callback_called = true;
                    plan.check_owner(instances, budget)?;
                    let entry = plan.entries[instances.root().index()].unwrap();
                    let argument = &plan.states[entry][1];
                    assert!(matches!(
                        plan.nodes[argument.node.unwrap()].kind,
                        SourceReferenceNodeKindV29::Plain(Some(_))
                    ));
                    assert!(root.snapshot_initialized(
                        plan.storage_snapshots[argument.storage.unwrap()],
                        &[],
                        budget
                    )?);
                    if matches!(flow, Flow::SiblingRead | Flow::RestoreBoth) {
                        let block = if flow == Flow::SiblingRead { 1 } else { 2 };
                        let partial = plan
                            .blocks
                            .iter()
                            .find(|row| {
                                row.instance == instances.root() && row.block.index() == block
                            })
                            .unwrap();
                        let state = &plan.states[partial.entry][1];
                        let node = state.node.unwrap();
                        assert!(matches!(
                            plan.nodes[node].kind,
                            SourceReferenceNodeKindV29::Aggregate { count: 2, .. }
                        ));
                        assert!(!root.snapshot_initialized(
                            plan.storage_snapshots[state.storage.unwrap()],
                            &[],
                            budget
                        )?);
                        let types = owner.source_semantic().types();
                        let selected = plan.nodes[node].ty;
                        let SemanticTypeShapeV1::Aggregate(fields) =
                            types[selected.index() as usize].shape()
                        else {
                            panic!("outer source shape")
                        };
                        let inner = fields.fields()[0];
                        let a = [field(0, inner), field(1, U32)];
                        let b = [field(1, inner), field(1, U32)];
                        assert_eq!(
                            root.snapshot_initialized(
                                plan.storage_snapshots[state.storage.unwrap()],
                                &a,
                                budget
                            )?,
                            flow == Flow::RestoreBoth
                        );
                        assert_eq!(
                            root.snapshot_initialized(
                                plan.storage_snapshots[state.storage.unwrap()],
                                &b,
                                budget
                            )?,
                            flow == Flow::SiblingRead
                        );
                        let SourceReferenceNodeKindV29::Aggregate { first, .. } =
                            plan.nodes[node].kind
                        else {
                            unreachable!()
                        };
                        if flow == Flow::SiblingRead {
                            assert_eq!(
                                plan.nodes[plan.children[first + 1]].kind,
                                SourceReferenceNodeKindV29::Plain(None),
                                "untouched nested sibling must not be recursively expanded"
                            );
                        }
                        let moved_parent =
                            plan.children[first + usize::from(flow == Flow::RestoreBoth)];
                        let SourceReferenceNodeKindV29::Aggregate { first, .. } =
                            plan.nodes[moved_parent].kind
                        else {
                            panic!("selected inner path must remain represented")
                        };
                        let absent = &plan.nodes[plan.children[first + 1]];
                        assert_eq!(absent.kind, SourceReferenceNodeKindV29::Absent);
                        assert!(absent.inactive.is_some());
                    }
                    let helper = instances
                        .calls(instances.root())
                        .unwrap()
                        .iter()
                        .find_map(|call| call.child())
                        .unwrap();
                    let helper_entry = plan.entries[helper.index()].unwrap();
                    let value = &plan.states[helper_entry][1];
                    assert!(root.snapshot_initialized(
                        plan.storage_snapshots[value.storage.unwrap()],
                        &[],
                        budget
                    )?);
                    Ok(())
                },
            );
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        });
    let result = match result {
        Ok(result) => result,
        Err(production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error)) => {
            Err(error.into())
        }
        Err(error) => panic!("original source instance construction failed: {error:?}"),
    };
    assert_eq!(budget.storage(), table_floor);
    if result.is_ok() {
        assert!(
            root_called && callback_called,
            "positive source must reach its actual C1/C2 callback"
        );
    }
    let mut outcome = Outcome {
        result,
        work: budget.work(),
        peak: budget.peak_storage(),
        prefix_work,
        prefix_peak,
        denied_work: false,
        denied_storage: budget.failed_storage().is_some(),
    };
    let release = layouts.release(&mut budget);
    assert_eq!(release.is_err(), root_called && outcome.result.is_err());
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    outcome.denied_work = work.failed_work().is_some();
    outcome
}

const LIMIT: usize = 100_000_000;

#[test]
fn original_mixed_partial_and_restore_controls_cover_both_nominal_carriers() {
    for input in [Input::CapturedContext, Input::CapturedWorkgroupReference] {
        for nested in [false, true] {
            logical_case(
                input,
                nested,
                LogicalFlow::PartialMove,
                LogicalCheck::OriginalStates,
            )
            .unwrap();
            logical_case(
                input,
                nested,
                LogicalFlow::Reinitialize,
                LogicalCheck::OriginalStates,
            )
            .unwrap();
        }
    }
}

#[test]
fn ordinary_and_mixed_nested_roots_preserve_surviving_sibling_reads() {
    for shape in [
        Shape::Aggregate,
        Shape::Tuple,
        Shape::Wide,
        Shape::Mixed(Input::CapturedContext),
        Shape::Mixed(Input::CapturedWorkgroupReference),
    ] {
        run(shape, Flow::SiblingRead, LIMIT, LIMIT).result.unwrap();
    }
}

#[test]
fn restoring_one_field_does_not_erase_a_second_fields_inactive_marker() {
    for shape in [
        Shape::Aggregate,
        Shape::Tuple,
        Shape::Mixed(Input::CapturedContext),
    ] {
        run(shape, Flow::RestoreBoth, LIMIT, LIMIT).result.unwrap();
        // These original invalid sources are rejected by SSA before C1 exists.
        assert!(matches!(
            owner(shape, Flow::RestoreOne),
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                function: ROOT,
                block: 2,
                statement: None,
                local: 1,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            })
        ));
        assert!(matches!(
            owner(shape, Flow::MovedRead),
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                function: ROOT,
                block: 1,
                statement: Some(0),
                local: 1,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            })
        ));
        run(shape, Flow::RestoreBoth, LIMIT, LIMIT).result.unwrap();
    }
}

#[test]
fn unchanged_and_expanded_predecessors_share_the_existing_fixed_point() {
    for shape in [
        Shape::Aggregate,
        Shape::Tuple,
        Shape::Deep,
        Shape::Mixed(Input::CapturedContext),
    ] {
        run(shape, Flow::Branch, LIMIT, LIMIT).result.unwrap();
        assert!(matches!(
            owner(shape, Flow::PartialBranch),
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                function: ROOT,
                block: 3,
                statement: None,
                local: 1,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            })
        ));
        run(shape, Flow::Loop, LIMIT, LIMIT).result.unwrap();
    }
}

#[test]
fn actual_root_partial_transfer_has_exact_work_and_storage_boundaries() {
    let shape = Shape::Mixed(Input::CapturedContext);
    let baseline = run(shape, Flow::RestoreBoth, LIMIT, LIMIT);
    baseline.result.unwrap();
    assert!(baseline.work > baseline.prefix_work + 1);
    assert!(baseline.peak > baseline.prefix_peak + 1);
    let exact = run(shape, Flow::RestoreBoth, baseline.work, baseline.peak);
    exact.result.unwrap();
    assert_eq!((exact.work, exact.peak), (baseline.work, baseline.peak));
    let work = run(shape, Flow::RestoreBoth, baseline.work - 1, baseline.peak);
    assert!(work.result.is_err() && work.denied_work);
    assert!(matches!(
        work.result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    let storage = run(shape, Flow::RestoreBoth, baseline.work, baseline.peak - 1);
    assert!(storage.result.is_err() && storage.denied_storage);
    assert!(matches!(
        storage.result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn private_representation_rejoin_refuses_stale_root_generation_and_invalid_field() {
    let mut owner = owner(Shape::Aggregate, Flow::SiblingRead).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        // This private representation probe uses actual admitted instances and
        // their captured C1 entry. It does not claim a C2 initialization fact.
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        builder.function(instances.root(), None, budget).unwrap();
        builder.frames[instances.root().index()] = builder.plan.entries[instances.root().index()];
        let source = instances.instance(instances.root()).unwrap().declaration();
        let SemanticStatementKindV1::Assign(assignment) = source.blocks()[0].statements()[0].kind()
        else {
            panic!("original assignment")
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) = assignment.value().kind()
        else {
            panic!("original projected move")
        };
        let site = SourceReferenceSiteV29 {
            instance: instances.root(),
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(0),
        };
        let mut target = builder
            .resolve_reference_place(site, place, SourceReferenceAccessV29::Read, budget)
            .unwrap();
        let before = builder.local(target.instance, target.local).unwrap();
        let replacement = builder.plain(U32, budget).unwrap();
        let other = builder
            .plain(builder.plan.nodes[target.representation_root].ty, budget)
            .unwrap();
        let root = target.representation_root;
        target.representation_root = other;
        assert!(
            builder
                .replace_reference_fields(&target, replacement, budget)
                .is_err()
        );
        assert_eq!(
            builder.local(target.instance, target.local).unwrap(),
            before
        );
        target.representation_root = root;
        target.generation += 1;
        assert!(
            builder
                .replace_reference_fields(&target, replacement, budget)
                .is_err()
        );
        assert_eq!(
            builder.local(target.instance, target.local).unwrap(),
            before
        );
        target.generation -= 1;
        let original_projection = target.projections[0];
        target.projections[0] = field(2, original_projection.result_type());
        assert!(
            builder
                .replace_reference_fields(&target, replacement, budget)
                .is_err()
        );
        assert_eq!(
            builder.local(target.instance, target.local).unwrap(),
            before
        );
        target.projections[0] = original_projection;
        builder
            .replace_reference_fields(&target, replacement, budget)
            .unwrap();
        let after = builder.local(target.instance, target.local).unwrap();
        assert_ne!(after.node, before.node);
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.storage, before.storage);
        let original_nodes = builder.plan.nodes.len();
        let original_children = builder.plan.children.len();
        let expanded = builder.expand_plain_reference_fields(root, budget).unwrap();
        assert_eq!(builder.plan.nodes.len(), original_nodes + 3);
        assert_eq!(builder.plan.children.len(), original_children + 2);
        let SourceReferenceNodeKindV29::Aggregate { first, count } =
            builder.plan.nodes[expanded].kind
        else {
            panic!("ordinary immediate roster")
        };
        assert_eq!(count, 2);
        for child in &builder.plan.children[first..first + count] {
            assert_eq!(
                builder.plan.nodes[*child].kind,
                SourceReferenceNodeKindV29::Plain(None)
            );
            assert!(builder.plan.nodes[*child].storage.is_none());
        }
        let forward = builder.merge_node(root, expanded, budget).unwrap();
        let reverse = builder.merge_node(expanded, root, budget).unwrap();
        assert!(builder.nodes_equal(forward, reverse, 0, budget).unwrap());
        assert!(
            builder
                .merge_node_at_depth(root, expanded, 256, budget)
                .is_err()
        );
        drop(builder);
        budget.release_storage(budget.storage() - floor).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn plain_array_enum_and_union_nodes_never_acquire_guessed_field_rosters() {
    let base = owner(Shape::Aggregate, Flow::SiblingRead).unwrap();
    let source = base.source_semantic();
    let mut types = source.types().to_vec();
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::array(4, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 2,
        },
        None,
    );
    let union = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4,
            4,
            SemanticFieldsShapeV1::Union { field_count: 2 },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        None,
    );
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
    );
    let enumeration = declaration(
        &mut types,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            4,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                (0..2)
                    .map(|index| {
                        SemanticEnumVariantLayoutV1::from_rustc(
                            index,
                            8,
                            4,
                            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                            SemanticBackendReprV1::memory(true),
                            None,
                            false,
                            None,
                            4,
                            0,
                            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
                        )
                        .unwrap()
                    })
                    .collect(),
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        index,
                        SemanticAggregateTypeV1::new(vec![U32]).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
        None,
    );
    let mut functions = source.functions().to_vec();
    let original = &functions[0];
    let mut locals = original.locals().to_vec();
    for (index, ty) in [array, enumeration, union].into_iter().enumerate() {
        locals.push(local(245 + index as u8, ty, SemanticLocalRoleV1::Temporary));
    }
    functions[0] = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        for ty in [array, enumeration, union] {
            let plain = builder.plain(ty, budget).unwrap();
            let counts = (builder.plan.nodes.len(), builder.plan.children.len());
            assert_eq!(
                builder
                    .ordinary_reference_field_count(plain, budget)
                    .unwrap(),
                None
            );
            assert!(
                builder
                    .expand_plain_reference_fields(plain, budget)
                    .is_err()
            );
            assert_eq!(
                (builder.plan.nodes.len(), builder.plan.children.len()),
                counts
            );
            assert_eq!(
                builder.plan.nodes[plain].kind,
                SourceReferenceNodeKindV29::Plain(None)
            );
            assert!(builder.plan.nodes[plain].storage.is_none());
        }
        let left = builder.plain(U32, budget).unwrap();
        let right = builder.plain(U32, budget).unwrap();
        let first = builder.plan.children.len();
        emission_push_v1(&mut builder.plan.children, left, budget).unwrap();
        emission_push_v1(&mut builder.plan.children, right, budget).unwrap();
        let represented = builder
            .node(
                array,
                SourceReferenceNodeKindV29::Aggregate { first, count: 2 },
                budget,
            )
            .unwrap();
        for (offset, from_end) in [(1, false), (1, true)] {
            let projection = SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length: 2,
                    from_end,
                },
                U32,
            )
            .unwrap();
            assert_eq!(
                builder
                    .array_projection_node(represented, projection, budget)
                    .unwrap(),
                right
            );
        }
        drop(builder);
        budget.release_storage(budget.storage() - floor).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
