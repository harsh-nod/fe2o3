use super::*;
use crate::production_semantic_kir_v1::*;

const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);

#[derive(Clone, Copy)]
enum Shape {
    Tuple,
    Struct,
    RustCall,
}

// Source/SSA fixtures below deliberately use test-only nominal producers.
// They exercise shared emission and construction, not authenticated issuance.
fn owner(shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let mut types = execution_owner(Flow::Linear)
        .unwrap()
        .source_semantic()
        .types()
        .to_vec();
    let fields = SemanticAggregateTypeV1::new(vec![CONTEXT, U32]).unwrap();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        if matches!(shape, Shape::Struct) {
            SemanticTypeShapeV1::Aggregate(fields)
        } else {
            SemanticTypeShapeV1::Tuple(fields)
        },
    ));
    let rust_call = matches!(shape, Shape::RustCall);
    let call = |input, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                if rust_call {
                    vec![
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            UNIT,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Move(place(input, PAIR)),
                    ]
                } else {
                    vec![SemanticOperandV1::Move(place(input, PAIR))]
                },
                Some(SemanticCallDestinationV1::new(
                    place(3, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        abi(203, true, &[PAIR, PAIR]),
        vec![
            local(210, UNIT, SemanticLocalRoleV1::Return),
            local(211, PAIR, SemanticLocalRoleV1::Argument(0)),
            local(212, PAIR, SemanticLocalRoleV1::Argument(1)),
            local(213, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(220, vec![], call(1, 1)),
            block(221, vec![], call(2, 2)),
            block(222, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"execution_parameter_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([204; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let helper_abi = if rust_call {
        SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            false,
            false,
            1,
            vec![UNIT, PAIR],
            U32,
            vec![
                SemanticAbiArgumentV1::source(ignored(UNIT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(0, ignored(CONTEXT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(U32)),
            ],
            direct(U32),
        )
        .unwrap()
    } else {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(direct(PAIR))],
            direct(U32),
        )
        .unwrap()
    };
    let projected = |field, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let (locals, context, scalar_input, temporary) = if rust_call {
        (
            vec![
                local(230, U32, SemanticLocalRoleV1::Return),
                local(231, UNIT, SemanticLocalRoleV1::Argument(0)),
                local(
                    232,
                    CONTEXT,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: 0,
                    },
                ),
                local(
                    233,
                    U32,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: 1,
                    },
                ),
                local(234, CONTEXT, SemanticLocalRoleV1::Temporary),
            ],
            place(2, CONTEXT),
            place(3, U32),
            4,
        )
    } else {
        (
            vec![
                local(230, U32, SemanticLocalRoleV1::Return),
                local(231, PAIR, SemanticLocalRoleV1::Argument(0)),
                local(232, CONTEXT, SemanticLocalRoleV1::Temporary),
            ],
            projected(0, CONTEXT),
            projected(1, U32),
            2,
        )
    };
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        locals,
        vec![block(
            240,
            vec![
                assign(
                    place(temporary, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(context)),
                ),
                assign(
                    place(0, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitXor,
                        left: SemanticOperandV1::Copy(scalar_input),
                        right: scalar(7),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(HELPER),
        ],
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

fn child_plan() -> LoweredFunctionPlanV1 {
    LoweredFunctionPlanV1 {
        correspondence_owner: ROOT,
        semantic_function: HELPER,
        kernel_ir_function: FunctionId::new("scoped_child"),
        role: SemanticKirFunctionRoleV1::InternalHelper,
        parameter_declarations: vec![],
        parameter_types: vec![Type::Scalar(ScalarType::U32)],
        parameter_values: vec![ValueId(300)],
        call_arguments: vec![],
        parameter_local_bindings: vec![],
        parameter_component_bindings: vec![],
        ignored_parameter_bindings: vec![],
        result_types: vec![Type::Scalar(ScalarType::U32)],
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    OtherInstance,
    WrongType,
    WrongSource,
    WrongSsa,
    WrongRoot,
    WrongProjection,
    Moved,
    Alias,
    ConstructorInstance,
}

fn run(
    shape: Shape,
    fault: Fault,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    usize,
) {
    let mut owner = owner(shape);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        let captured = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|_| execution_call_error_v29())?;
        budget.reserve_storage(captured.retained_storage())?;
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let first = instances.calls(instances.root()).unwrap()[0]
                .child()
                .unwrap();
            let second = instances.calls(instances.root()).unwrap()[1]
                .child()
                .unwrap();
            let prepared = with_execution_availability_v29(
                instances,
                instances.root(),
                budget,
                |mut cursor, budget| {
                    let semantic = owner.source_semantic();
                    let seed = SemanticExecutionBindingV29::context(
                        semantic.types(),
                        CONTEXT,
                        ProductionCallOccurrenceV1 {
                            caller: instances.root(),
                            block: SemanticBlockIdV1::from_index(0),
                        },
                        ValueId(90),
                    )
                    .unwrap();
                    let binding = |id| {
                        SemanticValueBindingV1::Aggregate(vec![
                            if matches!(fault, Fault::Moved) {
                                SemanticValueBindingV1::MovedExecution
                            } else {
                                SemanticValueBindingV1::Execution(seed.clone())
                            },
                            SemanticValueBindingV1::Value {
                                id: ValueId(id),
                                ty: Type::Scalar(ScalarType::U32),
                            },
                        ])
                    };
                    cursor.entry_seeds = vec![(1, binding(20)), (2, binding(21))];
                    let mut parent = SemanticFunctionLoweringV1::new_interprocedural(
                        semantic.types(),
                        semantic.callables(),
                        &semantic.functions()[0],
                        owner.plan_for_function(ROOT).unwrap(),
                        ROOT,
                        ROOT,
                        BTreeMap::new(),
                        BTreeMap::new(),
                        vec![],
                        SemanticParameterBindingsV1 {
                            declarations: &[],
                            values: &[ValueId(20), ValueId(21)],
                            types: &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
                            local_bindings: Some(&[]),
                        },
                        None,
                        Some([64, 1, 1]),
                        BTreeSet::new(),
                        1,
                        false,
                        1024,
                        PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(1, 1024)),
                        None,
                        CallReturnBufferV1::empty(),
                        Some(budget),
                        SemanticEmissionPlacementV1 {
                            first_block: 0,
                            first_value: 200,
                        },
                        Some(cursor),
                    )?;
                    let mut block = BasicBlock::new(BlockId(0));
                    parent.begin_block(SemanticBlockIdV1::from_index(0), &mut block)?;
                    let incoming = instances.incoming(first).unwrap();
                    let rust_call = matches!(shape, Shape::RustCall);
                    let projections = [HelperCallArgumentV1 {
                        source_argument: u32::from(rust_call),
                        tuple_field: rust_call.then_some(1),
                        component: Some(0),
                    }];
                    let prepared = parent.prepare_defined_call_arguments_v1(
                        SemanticBlockIdV1::from_index(0),
                        incoming.source(),
                        HELPER,
                        DefinedCallArgumentSignatureV1 {
                            projection: DefinedCallProjectionV29::Execution,
                            semantic_types: semantic.functions()[1].abi().source_input_types(),
                            projections: &projections,
                            parameter_types: vec![Type::Scalar(ScalarType::U32)],
                        },
                        &mut block.operations,
                    )?;
                    assert_eq!(prepared.arguments, [ValueId(20)]);
                    Ok(prepared)
                },
            )?;
            let mut prepared = prepared;
            let mut plan = child_plan();
            match fault {
                Fault::WrongType => {
                    prepared.execution.as_mut().unwrap().parameter_types[0] = Type::INDEX
                }
                Fault::WrongSource => prepared.execution.as_mut().unwrap().source.semantic[0] ^= 1,
                Fault::WrongSsa => {
                    prepared.execution.as_mut().unwrap().source.ssa =
                        self::owner(Shape::Struct).identity()
                }
                Fault::WrongRoot => prepared.execution.as_mut().unwrap().source.root = HELPER,
                Fault::WrongProjection => {
                    prepared.execution.as_mut().unwrap().projections[0].source_argument = 99
                }
                Fault::Alias => plan.parameter_values[0] = ValueId(90),
                _ => {}
            }
            let selected = if matches!(fault, Fault::OtherInstance) {
                second
            } else {
                first
            };
            let (arguments, parameters) =
                prepare_execution_parameters_v29(instances, selected, prepared, &plan, budget)?;
            assert_eq!(arguments, [ValueId(20)]);
            let selected = if matches!(fault, Fault::ConstructorInstance) {
                second
            } else {
                first
            };
            with_execution_availability_v29(instances, selected, budget, |mut cursor, budget| {
                cursor.parameters = Some(parameters);
                let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                lower_one_semantic_function_v1(
                    owner.source_semantic(),
                    &plan,
                    owner.plan_for_function(HELPER).unwrap(),
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    None,
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
                        first_value: 300,
                    },
                    Some(cursor),
                )
            })
        })
    })();
    drop(budget);
    (result, work.work())
}

#[test]
fn scoped_arguments_reach_shared_constructor_entry_archive_and_scalar_emission() {
    for shape in [Shape::Tuple, Shape::Struct, Shape::RustCall] {
        let result = run(shape, Fault::None, 10_000_000, 10_000_000).0.unwrap();
        assert_eq!(
            result.function.signature.parameters,
            [Type::Scalar(ScalarType::U32)]
        );
        assert_eq!(
            result.function.signature.results,
            [Type::Scalar(ScalarType::U32)]
        );
        let observation = result.execution_observation.unwrap();
        let nominal = observation.bindings.values().any(|binding| match binding {
            SemanticValueBindingV1::Execution(value) => value.value() == ValueId(90),
            SemanticValueBindingV1::Aggregate(fields) => matches!(&fields[0],
                SemanticValueBindingV1::Execution(value) if value.value() == ValueId(90)),
            _ => false,
        });
        assert!(
            nominal,
            "callee archive must preserve the original nominal producer"
        );
        assert!(
            result
                .function
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|op| {
                    matches!(
                        op.kind,
                        OperationKind::Binary {
                            lhs: ValueId(300),
                            ..
                        }
                    )
                }),
            "scalar body must use its own parameter, not caller ValueId(20)"
        );
    }
}

#[test]
fn scoped_call_rejects_instance_identity_type_projection_and_tombstone_substitution() {
    for fault in [
        Fault::OtherInstance,
        Fault::WrongType,
        Fault::WrongSource,
        Fault::WrongSsa,
        Fault::WrongRoot,
        Fault::WrongProjection,
        Fault::Moved,
        Fault::Alias,
        Fault::ConstructorInstance,
    ] {
        assert!(run(Shape::Tuple, fault, 10_000_000, 10_000_000).0.is_err());
    }
}

#[test]
fn scoped_call_obeys_exact_and_one_short_shared_work_budget() {
    let (result, exact) = run(Shape::RustCall, Fault::None, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(
        run(Shape::RustCall, Fault::None, exact, 10_000_000)
            .0
            .is_ok()
    );
    assert!(
        run(Shape::RustCall, Fault::None, exact - 1, 10_000_000)
            .0
            .is_err()
    );
}
