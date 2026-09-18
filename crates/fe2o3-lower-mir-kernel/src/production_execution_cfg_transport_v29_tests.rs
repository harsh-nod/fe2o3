use super::*;
use crate::production_semantic_kir_v1::*;

const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

mod event_tests {
    include!("production_execution_events_v29_tests.rs");
}

#[derive(Clone, Copy)]
enum Shape {
    Diamond,
    Mixed,
    DifferentSources,
    MovedSibling,
    Storage,
    Independent,
    ScalarBinary,
    ScalarSwitch,
    AssertMessage,
    AssertConstant,
    Cycle,
}

fn cfg_owner(shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let original = execution_owner(Flow::Linear).unwrap();
    let mut types = original.source_semantic().types().to_vec();
    let mixed = matches!(
        shape,
        Shape::Mixed
            | Shape::MovedSibling
            | Shape::Storage
            | Shape::ScalarBinary
            | Shape::ScalarSwitch
            | Shape::AssertMessage
            | Shape::AssertConstant
    );
    if mixed {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([201; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![CONTEXT, U32]).unwrap()),
        ));
    }
    let bool_ty = SemanticTypeIdV1::from_index(types.len() as u32);
    if matches!(shape, Shape::AssertMessage | Shape::AssertConstant) {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([240; 32]),
            SemanticLayoutIdentityV1::from_sha256([240; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
    }
    let result = if mixed { PAIR } else { CONTEXT };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let pair = |value| {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![SemanticOperandV1::Move(place(1, CONTEXT)), scalar(value)],
            )
            .unwrap(),
        )
    };
    let arm = |number, source_local| {
        let value = if mixed {
            pair(number)
        } else {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(source_local, CONTEXT)))
        };
        assign(place(4, result), value)
    };
    let field = |index, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let blocks = if matches!(
        shape,
        Shape::ScalarBinary | Shape::ScalarSwitch | Shape::AssertMessage | Shape::AssertConstant
    ) {
        let operand = SemanticOperandV1::Copy(field(1, U32));
        let mut statements = vec![assign(place(4, PAIR), pair(42))];
        let terminator = match shape {
            Shape::ScalarBinary => {
                statements.push(assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: operand,
                        right: scalar(1),
                    },
                ));
                goto(1)
            }
            Shape::ScalarSwitch => SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand,
                targets: SemanticSwitchTargetsV1::new(
                    vec![],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
            Shape::AssertMessage | Shape::AssertConstant => SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    bool_ty,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                )),
                expected: true,
                message: SemanticAssertMessageV1::DivisionByZero(
                    if matches!(shape, Shape::AssertConstant) {
                        scalar(42)
                    } else {
                        operand
                    },
                ),
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            _ => unreachable!(),
        };
        vec![
            block(211, statements, terminator),
            block(212, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else if matches!(shape, Shape::Storage) {
        let storage = |live, index| {
            SemanticStatementV1::new(
                source(),
                if live {
                    SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(index))
                } else {
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(index))
                },
            )
        };
        vec![block(
            211,
            vec![
                storage(true, 4),
                assign(place(4, PAIR), pair(42)),
                assign(
                    place(6, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, CONTEXT))),
                ),
                assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, U32))),
                ),
                storage(false, 4),
                storage(false, 1),
            ],
            SemanticTerminatorKindV1::Return,
        )]
    } else if matches!(shape, Shape::Independent) {
        vec![block(
            211,
            vec![
                arm(0, 1),
                assign(
                    place(6, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, CONTEXT))),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )]
    } else if matches!(shape, Shape::MovedSibling) {
        vec![
            block(
                211,
                vec![
                    assign(place(4, PAIR), pair(42)),
                    assign(
                        place(6, CONTEXT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, CONTEXT))),
                    ),
                ],
                goto(1),
            ),
            block(212, vec![], goto(2)),
            block(
                213,
                vec![assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, U32))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else if matches!(shape, Shape::Cycle) {
        vec![
            block(211, vec![], goto(1)),
            block(
                212,
                vec![
                    arm(11, 1),
                    assign(
                        place(1, CONTEXT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, CONTEXT))),
                    ),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(213, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                211,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(212, vec![arm(11, 1)], goto(3)),
            block(
                213,
                vec![arm(
                    22,
                    if matches!(shape, Shape::DifferentSources) {
                        2
                    } else {
                        1
                    },
                )],
                goto(3),
            ),
            block(
                214,
                vec![assign(
                    place(5, result),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, result))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([202; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        vec![
            SemanticAbiArgumentV1::source(ignored(CONTEXT)),
            SemanticAbiArgumentV1::source(ignored(CONTEXT)),
            SemanticAbiArgumentV1::source(direct(U32)),
        ],
        ignored(UNIT),
    )
    .unwrap();
    let root = function(
        203,
        SemanticFunctionRoleV1::KernelRoot,
        abi,
        vec![
            local(220, UNIT, SemanticLocalRoleV1::Return),
            local(221, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(222, CONTEXT, SemanticLocalRoleV1::Argument(1)),
            local(223, U32, SemanticLocalRoleV1::Argument(2)),
            local(224, result, SemanticLocalRoleV1::Temporary),
            local(225, result, SemanticLocalRoleV1::Temporary),
            local(226, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"cfg_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([204; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
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

fn lower_cfg_fixture(
    shape: Shape,
    change_seed: impl FnOnce(&mut SemanticExecutionBindingV29),
    inspect: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        &SemanticExecutionBindingV29,
        Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    ),
) {
    lower_cfg_fixture_with_cursor(shape, change_seed, |_| {}, inspect);
}

fn lower_cfg_fixture_with_cursor(
    shape: Shape,
    change_seed: impl FnOnce(&mut SemanticExecutionBindingV29),
    change_cursor: impl FnOnce(&mut ExecutionAvailabilityV29<'_>),
    inspect: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        &SemanticExecutionBindingV29,
        Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    ),
) {
    let mut owner = cfg_owner(shape);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let seed = SemanticExecutionBindingV29::context(
            owner.source_semantic().types(),
            CONTEXT,
            ProductionCallOccurrenceV1 {
                caller: instances.root(),
                block: SemanticBlockIdV1::from_index(0),
            },
            ValueId(90),
        )
        .unwrap();
        let mut other = seed.clone();
        change_seed(&mut other);
        let result = with_execution_availability_v29(
            instances,
            instances.root(),
            budget,
            |mut cursor, budget| {
                cursor.entry_seeds = vec![
                    (1, SemanticValueBindingV1::Execution(seed.clone())),
                    (2, SemanticValueBindingV1::Execution(other)),
                ];
                change_cursor(&mut cursor);
                let plan = LoweredFunctionPlanV1 {
                    correspondence_owner: ROOT,
                    semantic_function: ROOT,
                    kernel_ir_function: FunctionId::new("cfg_fixture"),
                    role: SemanticKirFunctionRoleV1::KernelEntry,
                    parameter_declarations: vec![(2, 3, U32)],
                    parameter_types: vec![Type::Scalar(ScalarType::U32)],
                    parameter_values: vec![ValueId(100)],
                    call_arguments: vec![],
                    parameter_local_bindings: vec![PlannedParameterLocalBindingV1::Direct {
                        local: 3,
                        value: ValueId(100),
                        ty: Type::Scalar(ScalarType::U32),
                    }],
                    parameter_component_bindings: vec![],
                    borrowed_parameter_bindings: vec![],
                    ignored_parameter_bindings: vec![],
                    result_types: vec![],
                };
                let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                lower_one_semantic_function_v1(
                    owner.source_semantic(),
                    &plan,
                    owner.plan_for_function(ROOT).unwrap(),
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    Some([64, 1, 1]),
                    BTreeSet::new(),
                    1,
                    false,
                    1024,
                    None,
                    &mut private,
                    None,
                    budget,
                    SemanticEmissionPlacementV1 {
                        first_block: 17,
                        first_value: 100,
                    },
                    Some(cursor),
                )
            },
        );
        inspect(&owner, &seed, result);
        Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

#[test]
fn actual_emitter_preserves_identity_through_a_real_source_phi() {
    lower_cfg_fixture(
        Shape::Diamond,
        |_| {},
        |owner, seed, result| {
            let result = result.unwrap();
            let phi = SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(3),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(4),
            };
            assert!(
                owner
                    .plan_for_function(ROOT)
                    .unwrap()
                    .plan()
                    .transport_variables(SsaBlockIdV1::new(3))
                    .unwrap()
                    .iter()
                    .any(|variable| variable.get() == 4)
            );
            let observation = result.execution_observation.unwrap();
            assert!(
                matches!(&observation.bindings[&phi], SemanticValueBindingV1::Execution(value) if value == seed)
            );
            assert!(
                matches!(&observation.locals[5], Some(SemanticValueBindingV1::Execution(value)) if value == seed)
            );
            assert!(observation.locals[4].is_none());
            assert!(result.function.body.unwrap().blocks.iter().all(|block| {
                block
                    .parameters
                    .iter()
                    .all(|value| !matches!(value.ty, Type::Execution(_)))
            }));
        },
    );
}

#[test]
fn aggregate_join_uses_scalar_parameters_without_changing_nominal_identity() {
    lower_cfg_fixture(
        Shape::Mixed,
        |_| {},
        |_, seed, result| {
            let result = result.unwrap();
            let body = result.function.body.unwrap();
            let joined = body
                .blocks
                .iter()
                .find(|block| block.id == BlockId(20))
                .unwrap();
            assert_eq!(joined.parameters.len(), 1);
            assert_eq!(joined.parameters[0].ty, Type::Scalar(ScalarType::U32));
            for (predecessor, literal) in [(18, 11), (19, 22)] {
                let block = body
                    .blocks
                    .iter()
                    .find(|block| block.id == BlockId(predecessor))
                    .unwrap();
                let constant = block
                    .operations
                    .iter()
                    .find(|op| op.kind == OperationKind::Constant(Constant::U32(literal)))
                    .unwrap();
                let Some(Terminator::Branch { target, arguments }) = &block.terminator else {
                    panic!("missing predecessor branch")
                };
                assert_eq!(*target, joined.id);
                assert_eq!(arguments.as_slice(), &[constant.results[0].id]);
            }
            let observation = result.execution_observation.unwrap();
            let Some(SemanticValueBindingV1::Aggregate(fields)) = &observation.locals[5] else {
                panic!("missing joined tuple");
            };
            assert!(
                matches!(&fields[0], SemanticValueBindingV1::Execution(value) if value == seed)
            );
            assert!(
                matches!(&fields[1], SemanticValueBindingV1::Value { id, .. } if *id == joined.parameters[0].id)
            );
        },
    );
}

#[test]
fn non_phi_pass_through_preserves_moved_sibling_state() {
    lower_cfg_fixture(
        Shape::MovedSibling,
        |_| {},
        |owner, seed, result| {
            for block in [1, 2] {
                assert!(
                    owner
                        .plan_for_function(ROOT)
                        .unwrap()
                        .plan()
                        .transport_variables(SsaBlockIdV1::new(block))
                        .unwrap()
                        .is_empty()
                );
            }
            let result = result.unwrap();
            let body = result.function.body.as_ref().unwrap();
            let entry = body
                .blocks
                .iter()
                .find(|block| block.id == BlockId(17))
                .unwrap();
            let constant = entry
                .operations
                .iter()
                .find(|op| op.kind == OperationKind::Constant(Constant::U32(42)))
                .unwrap();
            let observation = result.execution_observation.unwrap();
            assert!(
                matches!(&observation.locals[6], Some(SemanticValueBindingV1::Execution(value)) if value == seed)
            );
            let Some(SemanticValueBindingV1::Aggregate(fields)) = &observation.locals[4] else {
                panic!("missing forwarded tuple");
            };
            assert!(matches!(fields[0], SemanticValueBindingV1::MovedExecution));
            let SemanticValueBindingV1::Value { id, .. } = &fields[1] else {
                panic!("missing scalar sibling");
            };
            assert_eq!(*id, constant.results[0].id);
            for (source, destination) in [(17, 18), (18, 19)] {
                let block = body
                    .blocks
                    .iter()
                    .find(|block| block.id == BlockId(source))
                    .unwrap();
                let Some(Terminator::Branch { target, arguments }) = &block.terminator else {
                    panic!("missing pass-through branch")
                };
                assert_eq!(*target, BlockId(destination));
                assert!(arguments.is_empty());
                assert!(
                    body.blocks
                        .iter()
                        .find(|block| block.id == *target)
                        .unwrap()
                        .parameters
                        .is_empty()
                );
            }
            assert!(
                matches!(observation.locals[3], Some(SemanticValueBindingV1::Value { id: output, .. }) if output == *id)
            );
        },
    );
}

#[test]
fn differing_producers_or_values_cannot_merge_even_with_consistent_arm_archives() {
    lower_cfg_fixture(
        Shape::DifferentSources,
        |_| {},
        |_, _, result| {
            assert!(result.is_ok(), "unchanged identities must merge");
        },
    );
    for change_producer in [false, true] {
        lower_cfg_fixture(
            Shape::DifferentSources,
            |other| {
                if change_producer {
                    other.identity.producer.block = SemanticBlockIdV1::from_index(1);
                } else {
                    other.identity.value = ValueId(91);
                }
                other.context = other.identity;
            },
            |_, _, result| {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "execution CFG transport differs from its captured SSA state")
                );
            },
        );
    }
}

#[test]
fn captured_call_result_edge_is_exact_and_claimed_once() {
    captured_execution(Flow::Linear, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            let target = SemanticBlockIdV1::from_index(1);
            let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0);
            let definition = cursor
                .ssa
                .plan()
                .edge_definitions(edge)
                .unwrap()
                .iter()
                .find(|definition| definition.variable().get() == 2)
                .unwrap()
                .value();
            let seed = SemanticExecutionBindingV29::context(
                cursor.cfg.types,
                CONTEXT,
                ProductionCallOccurrenceV1 {
                    caller: cursor.instance,
                    block,
                },
                ValueId(90),
            )
            .unwrap();
            let held = vec![
                None,
                None,
                Some(SemanticValueBindingV1::Execution(seed.clone())),
            ];
            let archive = BTreeMap::from([(definition, SemanticValueBindingV1::Execution(seed))]);
            cursor.begin_block(block, budget)?;
            assert!(
                cursor
                    .transport_edge(block, 1, target, &held, &archive, budget)
                    .is_err()
            );
            assert!(
                cursor
                    .transport_edge(
                        block,
                        0,
                        SemanticBlockIdV1::from_index(2),
                        &held,
                        &archive,
                        budget
                    )
                    .is_err()
            );
            assert!(cursor.cfg.edges.iter().all(|edge| !edge.claimed));
            let arguments = cursor.ssa.plan().edge_arguments(edge).unwrap();
            let definitions = cursor.ssa.plan().edge_definitions(edge).unwrap();
            let substituted = cursor
                .ssa
                .plan()
                .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(2), 0))
                .unwrap();
            assert_eq!(definitions.len(), substituted.len());
            assert_ne!(definitions, substituted);
            assert!(
                cursor
                    .check_cfg_edge_plan(block, 0, target, arguments, substituted, budget)
                    .is_err()
            );
            assert!(
                cursor
                    .check_cfg_edge_plan(block, 0, target, arguments, &[], budget)
                    .is_err()
            );
            cursor.check_cfg_edge_plan(block, 0, target, arguments, definitions, budget)?;
            cursor.transport_edge(block, 0, target, &held, &archive, budget)?;
            assert!(
                cursor
                    .transport_edge(block, 0, target, &held, &archive, budget)
                    .is_err()
            );
            cursor.finish_block(budget)?;
            cursor.begin_block(target, budget)?;
            cursor.enter_cfg(target, budget)?;
            assert_eq!(cursor.current[2], Some(definition));
            Ok(())
        })
        .unwrap();
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let target = SemanticBlockIdV1::from_index(1);
            cursor.begin_block(target, budget)?;
            assert!(cursor.enter_cfg(target, budget).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn nominal_cfg_construction_and_edge_queries_have_exact_resource_boundaries() {
    captured_execution(Flow::Branches, |plan, _| {
        let helper = plan.id_at(1).unwrap();
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(37).unwrap();
            let result =
                with_execution_availability_v29(plan, helper, &mut budget, |mut cursor, budget| {
                    let bytes = budget.storage();
                    let block = SemanticBlockIdV1::from_index(0);
                    let target = SemanticBlockIdV1::from_index(1);
                    let definition = cursor
                        .ssa
                        .plan()
                        .entry_definitions()
                        .iter()
                        .find(|definition| definition.variable().get() == 1)
                        .unwrap()
                        .value();
                    let seed = SemanticExecutionBindingV29::context(
                        cursor.cfg.types,
                        CONTEXT,
                        ProductionCallOccurrenceV1 {
                            caller: plan.root(),
                            block,
                        },
                        ValueId(90),
                    )
                    .unwrap();
                    let held = vec![
                        None,
                        Some(SemanticValueBindingV1::Execution(seed.clone())),
                        None,
                        None,
                    ];
                    let archive =
                        BTreeMap::from([(definition, SemanticValueBindingV1::Execution(seed))]);
                    cursor.begin_block(block, budget)?;
                    cursor.transport_edge(block, 0, target, &held, &archive, budget)?;
                    cursor.transport_edge(
                        block,
                        1,
                        SemanticBlockIdV1::from_index(2),
                        &held,
                        &archive,
                        budget,
                    )?;
                    cursor.finish_block(budget)?;
                    cursor.begin_block(target, budget)?;
                    cursor.enter_cfg(target, budget)?;
                    Ok((budget.work(), bytes))
                });
            assert_eq!(budget.storage(), 37);
            result
        };
        let (work, bytes) = run(10_000_000, 10_000_000).unwrap();
        assert_eq!(run(work, bytes).unwrap(), (work, bytes));
        assert!(matches!(
            run(work - 1, bytes),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
        ));
        assert!(run(work, bytes - 1).is_err());
        with_execution_availability_v29(
            plan,
            helper,
            &mut ArgumentBudgetV1::new(
                &mut CanonicalKernelIrWorkBudgetV1::new(10_000_000),
                10_000_000,
            ),
            |mut cursor, budget| {
                cursor.begin_block(SemanticBlockIdV1::from_index(0), budget)?;
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
                assert!(
                    cursor
                        .transport_edge(
                            SemanticBlockIdV1::from_index(0),
                            0,
                            SemanticBlockIdV1::from_index(1),
                            &[],
                            &BTreeMap::new(),
                            &mut foreign
                        )
                        .is_err()
                );
                assert_eq!(foreign.work(), 0);
                assert!(cursor.cfg.edges.iter().all(|edge| !edge.claimed));
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn temporary_scalar_vectors_release_on_error_and_unwind_but_outputs_stay_paid() {
    let binding = SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Value {
        id: ValueId(1),
        ty: Type::Scalar(ScalarType::U32),
    }]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
    budget.reserve_storage(37).unwrap();
    with_execution_cfg_values_v29(&binding, &mut budget, |values, budget| {
        assert_eq!(values.len(), 1);
        budget.reserve_storage(19)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 56);
    let error = with_execution_cfg_values_v29(
        &binding,
        &mut budget,
        |_, _| -> Result<(), ProductionSemanticKirErrorV1> { Err(execution_cfg_error_v29()) },
    );
    assert!(error.is_err());
    assert_eq!(budget.storage(), 56);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = with_execution_cfg_values_v29(
            &binding,
            &mut budget,
            |_, _| -> Result<(), ProductionSemanticKirErrorV1> { panic!("fixture unwind") },
        );
    }));
    assert!(unwind.is_err());
    assert_eq!(budget.storage(), 56);
}

#[test]
fn cyclic_nominal_cfg_is_not_enabled_by_the_transport_projection() {
    lower_cfg_fixture(
        Shape::Cycle,
        |_| {},
        |_, _, result| {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == "execution CFG transport differs from its captured SSA state")
            );
        },
    );
}

#[test]
fn nominal_borrow_merge_checks_the_complete_occurrence_and_parent() {
    use execution_binding_tests as fixture;
    let types = fixture::types();
    let instance = ProductionCallInstanceIdV1(0);
    let block = SemanticBlockIdV1::from_index(0);
    let context = SemanticExecutionBindingV29::context(
        &types,
        fixture::CONTEXT,
        ProductionCallOccurrenceV1 {
            caller: instance,
            block,
        },
        ValueId(90),
    )
    .unwrap();
    let destination = place(2, fixture::SHARED_CONTEXT);
    let source = place(1, fixture::CONTEXT);
    let original = SemanticExecutionBorrowBindingV29::from_source(
        &types,
        SemanticExecutionBorrowSourceV29 {
            instance,
            block,
            statement: 0,
            destination: &destination,
            kind: SemanticBorrowKindV1::Shared,
            source: &source,
        },
        &context,
    )
    .unwrap();
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.occurrence.instance = ProductionCallInstanceIdV1(1),
            1 => changed.occurrence.statement = 1,
            2 => changed.parent = Some(original.occurrence),
            _ => changed.source_local = SemanticLocalIdV1::from_index(3),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
        let binding = SemanticValueBindingV1::ExecutionBorrow(original.clone());
        let changed = SemanticValueBindingV1::ExecutionBorrow(changed);
        let mut leaves = [None];
        merge_execution_cfg_binding_v29(
            &types,
            fixture::SHARED_CONTEXT,
            &binding,
            &binding,
            &mut leaves.iter_mut(),
            &mut 0,
            &mut budget,
        )
        .unwrap();
        merge_execution_cfg_binding_v29(
            &types,
            fixture::SHARED_CONTEXT,
            &binding,
            &binding,
            &mut leaves.iter_mut(),
            &mut 0,
            &mut budget,
        )
        .unwrap();
        assert!(
            merge_execution_cfg_binding_v29(
                &types,
                fixture::SHARED_CONTEXT,
                &changed,
                &changed,
                &mut leaves.iter_mut(),
                &mut 0,
                &mut budget
            )
            .is_err()
        );
    }
}

#[test]
fn non_phi_rebuild_preserves_archived_index_but_phi_requires_canonical_u64() {
    let types = execution_binding_tests::types();
    let values = [ValueDef::new(ValueId(90), Type::Scalar(ScalarType::Index))];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
    let actual = rebuild_execution_cfg_binding_v29(
        &types,
        SemanticTypeIdV1::from_index(0),
        false,
        &mut [].iter(),
        &mut values.iter(),
        &mut 0,
        &mut budget,
    )
    .unwrap();
    assert!(matches!(
        actual,
        SemanticValueBindingV1::Value {
            id: ValueId(90),
            ty: Type::Scalar(ScalarType::Index)
        }
    ));
    assert!(
        rebuild_execution_cfg_binding_v29(
            &types,
            SemanticTypeIdV1::from_index(0),
            true,
            &mut [].iter(),
            &mut values.iter(),
            &mut 0,
            &mut budget
        )
        .is_err()
    );
    let canonical = [ValueDef::new(ValueId(91), Type::Scalar(ScalarType::U64))];
    assert!(
        rebuild_execution_cfg_binding_v29(
            &types,
            SemanticTypeIdV1::from_index(0),
            true,
            &mut [].iter(),
            &mut canonical.iter(),
            &mut 0,
            &mut budget
        )
        .is_ok()
    );
}

#[test]
fn enum_payload_is_not_silently_reduced_to_its_discriminant() {
    let binding = SemanticValueBindingV1::Enum {
        discriminant: ValueId(90),
        discriminant_ty: Type::Scalar(ScalarType::U32),
        semantic_type: U32,
        variant: Some(1),
        payloads: BTreeMap::from([(
            1,
            vec![SemanticValueBindingV1::Value {
                id: ValueId(91),
                ty: Type::Scalar(ScalarType::U32),
            }],
        )]),
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
    budget.reserve_storage(37).unwrap();
    let mut consumed = false;
    assert!(
        with_execution_cfg_values_v29(&binding, &mut budget, |_, _| {
            consumed = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!consumed);
    assert_eq!(budget.storage(), 37);
}

#[test]
fn metered_binding_copy_covers_aggregate_buffers_and_nested_type_boxes() {
    let binding = SemanticValueBindingV1::Aggregate(vec![
        SemanticValueBindingV1::MovedExecution,
        SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Value {
            id: ValueId(7),
            ty: Type::pointer(
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        }]),
    ]);
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        let copied = clone_execution_cfg_binding_v29(&binding, &mut 0, &mut budget)?;
        let SemanticValueBindingV1::Aggregate(fields) = copied else {
            panic!("missing outer aggregate")
        };
        assert_eq!(fields.len(), 2);
        assert!(matches!(fields[0], SemanticValueBindingV1::MovedExecution));
        let SemanticValueBindingV1::Aggregate(nested) = &fields[1] else {
            panic!("missing nested aggregate")
        };
        assert_eq!(nested.len(), 1);
        let SemanticValueBindingV1::Value { id, ty } = &nested[0] else {
            panic!("missing copied pointer")
        };
        assert_eq!(*id, ValueId(7));
        assert_eq!(
            *ty,
            Type::pointer(
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly
                ),
                AddressSpace::Global,
                AccessMode::ReadOnly
            )
        );
        assert_eq!(
            budget.storage(),
            37 + (fields.capacity() + nested.capacity())
                * std::mem::size_of::<SemanticValueBindingV1>()
                + 2 * std::mem::size_of::<Type>()
        );
        Ok::<_, ProductionSemanticKirErrorV1>((budget.work(), budget.storage()))
    };
    let (work, storage) = run(10_000, 1_000_000).unwrap();
    assert_eq!(run(work, storage).unwrap(), (work, storage));
    assert!(run(work - 1, storage).is_err());
    assert!(run(work, storage - 1).is_err());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let mut nodes = MAX_SSA_VALUE_COMPONENTS_V1;
    assert!(clone_execution_cfg_binding_v29(&binding, &mut nodes, &mut budget).is_err());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn nominal_root_backedge_is_rejected_before_entering_the_root() {
    captured_execution(Flow::Loop { redefine: true }, |plan, budget| {
        let helper = plan.id_at(1).unwrap();
        with_execution_availability_v29(plan, helper, budget, |mut cursor, budget| {
            let entry = cursor.function.entry();
            cursor.begin_block(entry, budget)?;
            assert!(cursor.enter_cfg(entry, budget).is_err());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn populated_diamond_join_still_requires_both_predecessors() {
    let mut owner = cfg_owner(Shape::Diamond);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |plan, budget| {
        with_execution_availability_v29(plan, plan.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(1);
            let target = SemanticBlockIdV1::from_index(3);
            let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(1), 0);
            let value = cursor
                .ssa
                .plan()
                .edge_arguments(edge)
                .unwrap()
                .iter()
                .find(|argument| argument.variable().get() == 4)
                .unwrap()
                .value();
            let seed = SemanticExecutionBindingV29::context(
                cursor.cfg.types,
                CONTEXT,
                ProductionCallOccurrenceV1 {
                    caller: cursor.instance,
                    block: SemanticBlockIdV1::from_index(0),
                },
                ValueId(90),
            )
            .unwrap();
            let mut held = vec![None; 7];
            held[4] = Some(SemanticValueBindingV1::Execution(seed.clone()));
            let archive = BTreeMap::from([(value, SemanticValueBindingV1::Execution(seed))]);
            cursor.begin_block(block, budget)?;
            cursor.use_place(
                execution_site_v29(block, Some(0)),
                ExecutionOperandV29::RvalueOperand(0),
                &place(1, CONTEXT),
                true,
                budget,
            )?;
            cursor.define(
                execution_site_v29(block, Some(0)),
                SemanticLocalIdV1::from_index(4),
                value,
                budget,
            )?;
            cursor.transport_edge(block, 0, target, &held, &archive, budget)?;
            assert_eq!(cursor.cfg.incoming[3], 2);
            assert_eq!(cursor.cfg.arrived[3], 1);
            assert!(
                cursor.cfg.entries[cursor.cfg.ranges[3].clone()]
                    .iter()
                    .all(|entry| entry.value.is_some())
            );
            cursor.finish_block(budget)?;
            cursor.begin_block(target, budget)?;
            assert!(cursor.enter_cfg(target, budget).is_err());
            Ok(())
        })
        .unwrap();
        Ok::<(), production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

#[test]
fn transparent_memory_walk_is_bounded_and_cannot_erase_nominal_types() {
    let mut types = execution_binding_tests::types();
    let index = types.len() as u32;
    let wrapper = |field| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([231; 32]),
            SemanticLayoutIdentityV1::from_sha256([231; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(field)]).unwrap(),
            ),
        )
    };
    types.push(wrapper(0));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
    assert_eq!(
        execution_cfg_memory_type_v29(&types, SemanticTypeIdV1::from_index(index), &mut budget)
            .unwrap(),
        Type::Scalar(ScalarType::U64)
    );
    assert!(
        execution_cfg_memory_type_v29(&types, execution_binding_tests::CONTEXT, &mut budget)
            .is_err()
    );
    types[index as usize] = wrapper(index);
    let before = budget.work();
    assert!(
        execution_cfg_memory_type_v29(&types, SemanticTypeIdV1::from_index(index), &mut budget)
            .is_err()
    );
    assert_eq!(budget.work() - before, MAX_SSA_VALUE_COMPONENTS_V1 + 1);
    assert_eq!(budget.storage(), 0);
}
