use super::*;
use fe2o3_kernel_ir::Constant;

#[path = "call_instance_preparation_v1.rs"]
mod call_instance_preparation_v1;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy)]
enum BodyCase {
    Direct,
    RepeatedCalls(u32),
    UnitJoin,
    InvalidUnitJoin { storage_dead: bool },
    Reordered,
}

fn owner(
    expanded: bool,
    moved: bool,
    fields: &[SemanticTypeIdV1],
    case: BodyCase,
) -> ProductionSemanticMirOwnerV1 {
    owner_with_tuple_ownership(
        expanded,
        moved,
        fields,
        case,
        SemanticSourceArgumentOwnershipV1::ByValue,
    )
}

fn owner_with_tuple_ownership(
    expanded: bool,
    moved: bool,
    fields: &[SemanticTypeIdV1],
    case: BodyCase,
    tuple_ownership: SemanticSourceArgumentOwnershipV1,
) -> ProductionSemanticMirOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let layout = SemanticLayoutIdentityV1::from_sha256(bytes(250));
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            source,
        )
    };
    let assign = |place: SemanticPlaceV1, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(place.ty(), value),
            )),
        )
    };
    let function = |tag, role, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mut size = 0;
    let offsets = fields
        .iter()
        .map(|ty| {
            let offset = size;
            if *ty == U32 {
                size += 4;
            }
            offset
        })
        .collect();
    let (tuple_layout, tuple_shape) = if fields.is_empty() {
        (unit_type().layout().clone(), SemanticTypeShapeV1::Unit)
    } else {
        (
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                if size == 0 { 1 } else { 4 },
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields.to_vec()).unwrap()),
        )
    };
    let tuple_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(6)),
        SemanticLayoutIdentityV1::from_sha256(bytes(6)),
        tuple_layout,
        tuple_shape,
    );
    let tuple_value = if fields.is_empty() {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            TUPLE,
            SemanticConstantValueV1::ZeroSized,
        )))
    } else {
        let mut next = 29;
        let values = fields
            .iter()
            .map(|ty| {
                if *ty == UNIT {
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        UNIT,
                        SemanticConstantValueV1::ZeroSized,
                    ))
                } else {
                    let value = next;
                    next = 11;
                    scalar_constant(U32, value, 4)
                }
            })
            .collect();
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple, values).unwrap(),
        )
    };
    let tuple_operand = if moved {
        SemanticOperandV1::Move(local_place(1, TUPLE))
    } else {
        SemanticOperandV1::Copy(local_place(1, TUPLE))
    };
    let call = |target| {
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![scalar_constant(U32, 7, 4), tuple_operand.clone()],
            Some(SemanticCallDestinationV1::new(
                local_place(2, U32),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
    };
    let call_count = if let BodyCase::RepeatedCalls(count) = case {
        count
    } else {
        1
    };
    let mut root_blocks = (0..call_count)
        .map(|index| {
            block(
                40 + index as u8,
                if index == 0 {
                    vec![assign(local_place(1, TUPLE), tuple_value.clone())]
                } else {
                    vec![]
                },
                SemanticTerminatorKindV1::Call(call(index + 1)),
            )
        })
        .collect::<Vec<_>>();
    root_blocks.push(block(
        40 + call_count as u8,
        vec![assign(
            local_place(3, U32),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(local_place(2, U32)),
                right: scalar_constant(U32, 1, 4),
            },
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(20)),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let root = function(
        20,
        SemanticFunctionRoleV1::KernelRoot,
        root_abi,
        vec![
            local(30, UNIT, SemanticLocalRoleV1::Return),
            local(31, TUPLE, SemanticLocalRoleV1::Temporary),
            local(32, U32, SemanticLocalRoleV1::Temporary),
            local(33, U32, SemanticLocalRoleV1::Temporary),
        ],
        root_blocks,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"rust_call_tuple_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(42)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let mut adjusted = vec![SemanticAbiArgumentV1::source(direct_abi_value(U32))];
    adjusted.extend(fields.iter().enumerate().map(|(field, ty)| {
        SemanticAbiArgumentV1::rust_call_tuple_field(
            field as u32,
            if *ty == UNIT {
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
            } else {
                direct_abi_value(*ty)
            },
        )
    }));
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256(bytes(50)),
        layout,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![U32, TUPLE],
        U32,
        adjusted,
        direct_abi_value(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue,
        tuple_ownership,
    ])
    .unwrap();
    let mut locals = vec![
        local(60, U32, SemanticLocalRoleV1::Return),
        local(61, U32, SemanticLocalRoleV1::Argument(0)),
    ];
    if expanded {
        locals.extend((0..fields.len()).map(|index| {
            let field = if matches!(case, BodyCase::Reordered) {
                fields.len() - 1 - index
            } else {
                index
            };
            local(
                62 + index as u8,
                fields[field],
                SemanticLocalRoleV1::RustCallTupleField {
                    argument: 1,
                    field: field as u32,
                },
            )
        }));
    } else {
        locals.push(local(62, TUPLE, SemanticLocalRoleV1::Argument(1)));
    }
    let field_place = |field: usize| {
        if expanded {
            let index = if matches!(case, BodyCase::Reordered) {
                fields.len() - 1 - field
            } else {
                field
            };
            local_place(2 + index as u32, U32)
        } else {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field as u32), U32)
                        .unwrap(),
                ],
                U32,
            )
            .unwrap()
        }
    };
    let scalar_fields = fields
        .iter()
        .enumerate()
        .filter_map(|(i, ty)| (*ty == U32).then_some(i))
        .collect::<Vec<_>>();
    let value = match scalar_fields.as_slice() {
        [first, second, ..] => SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Subtract,
            left: SemanticOperandV1::Copy(field_place(*first)),
            right: SemanticOperandV1::Copy(field_place(*second)),
        },
        [first] => SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_place(*first))),
        [] => SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, U32))),
    };
    let helper_blocks = if matches!(case, BodyCase::UnitJoin | BodyCase::InvalidUnitJoin { .. }) {
        assert_eq!(fields, [UNIT]);
        locals.push(local(90, UNIT, SemanticLocalRoleV1::Temporary));
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let unit = SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ));
        let (ty, replacement, read) = if expanded {
            (UNIT, SemanticRvalueKindV1::Use(unit), local_place(2, UNIT))
        } else {
            (
                TUPLE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple, vec![unit])
                        .unwrap(),
                ),
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), UNIT)
                            .unwrap(),
                    ],
                    UNIT,
                )
                .unwrap(),
            )
        };
        let invalidation = match case {
            BodyCase::InvalidUnitJoin { storage_dead } => vec![SemanticStatementV1::new(
                source,
                if storage_dead {
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2))
                } else {
                    SemanticStatementKindV1::Deinitialize(local_place(2, ty))
                },
            )],
            _ => vec![],
        };
        vec![
            block(
                70,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(local_place(1, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                71,
                invalidation,
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                72,
                vec![assign(local_place(2, ty), replacement)],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                73,
                vec![
                    assign(
                        local_place(3, UNIT),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
                    ),
                    assign(local_place(0, U32), value),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else {
        vec![block(
            70,
            vec![assign(local_place(0, U32), value)],
            SemanticTerminatorKindV1::Return,
        )]
    };
    let helper = function(
        50,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        locals,
        helper_blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout),
        vec![
            unit_type(),
            scalar_type(
                5,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            tuple_type,
        ],
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn rust_call_empty_and_ignored_tuples_require_source_ownership_in_both_forms() {
    for expanded in [false, true] {
        for fields in [vec![], vec![UNIT], vec![UNIT, UNIT]] {
            for ownership in [
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::Unspecified,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                SemanticSourceArgumentOwnershipV1::RawPointer,
            ] {
                let result = ProductionSemanticKirOwnerV1::try_lower(
                    owner_with_tuple_ownership(
                        expanded,
                        false,
                        &fields,
                        BodyCase::Direct,
                        ownership,
                    ),
                    ProductionSemanticKirLimitsV1::default(),
                );
                if ownership == SemanticSourceArgumentOwnershipV1::ByValue {
                    result.unwrap().verify_equivalence().unwrap();
                } else {
                    let expected = if !fields.is_empty() {
                        "helper parameter is not an exact by-value scalar aggregate or shared slice"
                    } else if expanded {
                        "empty expanded helper argument lacks an exact by-value source ABI"
                    } else {
                        "helper entry local has no physical argument"
                    };
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                function: 1, block: None, statement: None, detail
                            }) if detail == expected
                        ),
                        "expanded={expanded}, fields={fields:?}, ownership={ownership:?}: {result:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rust_call_packed_and_expanded_copy_and_move_preserve_physical_arguments() {
    for fields in [
        vec![],
        vec![UNIT],
        vec![U32],
        vec![U32, U32],
        vec![U32, UNIT, U32],
    ] {
        for expanded in [false, true] {
            for moved in [false, true] {
                let lowered = ProductionSemanticKirOwnerV1::try_lower(
                    owner(expanded, moved, &fields, BodyCase::Direct),
                    ProductionSemanticKirLimitsV1::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("fields {fields:?}, expanded {expanded}, moved {moved}: {error:?}")
                });
                lowered.verify_equivalence().unwrap();
                let module = lowered.module();
                let entry = module.functions[0].body.as_ref().unwrap();
                let helper = &module.functions[1];
                let expected: Vec<u32> = std::iter::once(7)
                    .chain(
                        [29, 11]
                            .into_iter()
                            .take(fields.iter().filter(|ty| **ty == U32).count()),
                    )
                    .collect();
                assert_eq!(
                    helper.signature.parameters,
                    vec![Type::Scalar(ScalarType::U32); expected.len()]
                );
                assert_eq!(helper.signature.results, [Type::Scalar(ScalarType::U32)]);
                let operations = entry
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .collect::<Vec<_>>();
                let calls = operations
                    .iter()
                    .filter_map(|op| match &op.kind {
                        OperationKind::Call { arguments, .. } => Some(arguments),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(calls.len(), 1);
                let constants = operations
                    .iter()
                    .filter_map(|op| match &op.kind {
                        OperationKind::Constant(Constant::U32(value)) => {
                            Some((op.results[0].id, *value))
                        }
                        _ => None,
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                assert_eq!(constants.len(), expected.len() + 1);
                assert_eq!(
                    calls[0]
                        .iter()
                        .map(|value| constants[value])
                        .collect::<Vec<_>>(),
                    expected
                );
                let effects = analyze_interprocedural_effects_v1(module).unwrap();
                assert!(effects.function(&helper.id).unwrap().is_complete_and_pure());
                verify_module(module).unwrap();
                let result = operations
                    .iter()
                    .find(|op| matches!(op.kind, OperationKind::Call { .. }))
                    .unwrap()
                    .results[0]
                    .id;
                assert!(operations.iter().any(|op| matches!(op.kind,
                    OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, .. } if lhs == result)));
                if expected.len() == 3 {
                    let body = helper.body.as_ref().unwrap();
                    assert!(body.blocks[0].operations.iter().any(|op| matches!(&op.kind,
                        OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Subtract), lhs, rhs }
                        if *lhs == body.parameters[1] && *rhs == body.parameters[2])));
                }
            }
        }
    }
}

#[test]
fn rust_call_ignored_arguments_merge_without_physical_phi_parameters() {
    for expanded in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner(expanded, false, &[UNIT], BodyCase::UnitJoin),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        let helper = &lowered.module().functions[1];
        assert_eq!(helper.signature.parameters, [Type::Scalar(ScalarType::U32)]);
        assert!(
            helper
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .all(|block| block.parameters.is_empty())
        );
    }
}

#[test]
fn rust_call_field_roles_not_local_order_determine_parameter_correspondence() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        owner(true, false, &[U32, U32], BodyCase::Reordered),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let helper = lowered.module().functions[1].body.as_ref().unwrap();
    let bindings = lowered
        .correspondence()
        .parameter_bindings()
        .iter()
        .filter(|binding| binding.semantic_function() == SemanticFunctionIdV1::from_index(1))
        .collect::<Vec<_>>();
    for (local, parameter) in [(1, 0), (2, 2), (3, 1)] {
        assert!(
            bindings
                .iter()
                .any(|binding| binding.semantic_local().index() == local
                    && binding.kernel_ir_value() == helper.parameters[parameter])
        );
    }
    assert!(
        helper.blocks[0]
            .operations
            .iter()
            .any(|op| matches!(op.kind,
        OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Subtract), lhs, rhs }
        if lhs == helper.parameters[1] && rhs == helper.parameters[2]))
    );
}

#[test]
fn rust_call_repeated_tuple_expansion_is_bounded_before_materialization() {
    let Err(error) = ProductionSemanticKirOwnerV1::try_lower(
        owner(false, false, &[U32; 20], BodyCase::RepeatedCalls(40)),
        ProductionSemanticKirLimitsV1::new_with_max_operations(10, 100, 100, 400),
    ) else {
        panic!("repeated arguments must exceed the work budget");
    };
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisStorage, actual, limit: 400,
    } if actual > 400),
        "{error:?}"
    );
}

#[test]
fn rust_call_ignored_argument_budget_has_an_exact_boundary() {
    for expanded in [false, true] {
        let _lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner(expanded, false, &[UNIT; 20], BodyCase::RepeatedCalls(40)),
            ProductionSemanticKirLimitsV1::new_with_max_operations(10, 100, 100, 861),
        )
        .expect("21 argument rows at one declaration and 40 calls fit exactly");
        for field in [UNIT, U32] {
            let Err(error) = ProductionSemanticKirOwnerV1::try_lower(
                owner(expanded, false, &[field; 20], BodyCase::RepeatedCalls(40)),
                ProductionSemanticKirLimitsV1::new_with_max_operations(10, 100, 100, 860),
            ) else {
                panic!("one fewer row must reject before body materialization");
            };
            assert!(
                matches!(
                    error,
                    ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                        actual: 861,
                        limit: 860,
                    }
                ),
                "{error}"
            );
        }
    }
}

#[test]
fn rust_call_empty_source_tuple_still_consumes_an_argument_row() {
    for expanded in [false, true] {
        let Err(error) = ProductionSemanticKirOwnerV1::try_lower(
            owner(expanded, false, &[], BodyCase::RepeatedCalls(40)),
            ProductionSemanticKirLimitsV1::new_with_max_operations(10, 100, 100, 81),
        ) else {
            panic!("receiver and empty tuple at 41 sites require 82 rows");
        };
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                    actual: 82,
                    limit: 81,
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn rust_call_zero_sized_join_preserves_lifetime_and_initialization_checks() {
    for expanded in [false, true] {
        for storage_dead in [false, true] {
            let result = ProductionSemanticKirOwnerV1::try_lower(
                owner(
                    expanded,
                    false,
                    &[UNIT],
                    BodyCase::InvalidUnitJoin { storage_dead },
                ),
                ProductionSemanticKirLimitsV1::default(),
            );
            assert!(
                result.is_err(),
                "expanded={expanded}, storage_dead={storage_dead}"
            );
        }
    }
}
