use dialect_mir::{
    MirAddressSpace, MirAggregateLayout, MirAuthorizedDeviceImport, MirBasicBlock, MirBinaryOp,
    MirBlockId, MirBlockParameter, MirBody, MirBodyForm, MirCall, MirCallAuthority, MirCallReturn,
    MirCallSignature, MirCallable, MirCallee, MirConstant, MirConstantValue, MirEdge,
    MirExecutableModule, MirExecutableTarget, MirExecutableVersion, MirExternalCallRegistry,
    MirExternalCallReturn, MirExternalCallSignature, MirField, MirFunction, MirIntrinsic,
    MirLayout, MirLocalDecl, MirLocalId, MirLocalKind, MirMutability, MirOperand, MirPlace,
    MirProjection, MirRvalue, MirScalarType, MirSemanticType, MirSourceSpan, MirStatement,
    MirStatementKind, MirTerminator, MirTerminatorKind, MirTypeId, MirTypeKind, MirUnwindAction,
    MirValueId,
};

fn external_registry(
    identity: &str,
    contract: &str,
    inputs: Vec<MirSemanticType>,
    output: MirExternalCallReturn,
    can_unwind: bool,
) -> MirExternalCallRegistry {
    MirExternalCallRegistry::try_new(vec![MirAuthorizedDeviceImport {
        identity: identity.into(),
        contract: contract.into(),
        signature: MirExternalCallSignature {
            inputs,
            output,
            can_unwind,
        },
    }])
    .unwrap()
}

fn ty(kind: MirTypeKind, size: u64, align: u64) -> MirSemanticType {
    MirSemanticType {
        layout: MirLayout::sized(size, align),
        kind,
    }
}

fn fixture_types() -> (Vec<MirSemanticType>, MirTypeId, MirTypeId) {
    let bool_ty = ty(MirTypeKind::Scalar(MirScalarType::Bool), 1, 1);
    let u32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
        4,
        4,
    );
    let mut types = vec![bool_ty.clone(), u32_ty.clone()];
    types.sort_by_key(|item| item.canonical_text().unwrap());
    let bool_id = MirTypeId(types.iter().position(|item| item == &bool_ty).unwrap() as u32);
    let u32_id = MirTypeId(types.iter().position(|item| item == &u32_ty).unwrap() as u32);
    (types, bool_id, u32_id)
}

fn local(ty: MirTypeId, kind: MirLocalKind, mutable: bool) -> MirLocalDecl {
    MirLocalDecl {
        ty,
        kind,
        mutable,
        storage_address_space: MirAddressSpace::DEFAULT,
        name: None,
        span: None,
    }
}

fn terminator(kind: MirTerminatorKind) -> MirTerminator {
    MirTerminator { kind, span: None }
}

fn place_module() -> MirExecutableModule {
    let (types, _, u32_id) = fixture_types();
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget {
            pointer_width_bits: 32,
            thread_index_width_bits: 32,
        },
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "fixture::identity::<u32>".into(),
            span: Some(MirSourceSpan {
                file: "tests/identity.rs".into(),
                byte_start: 10,
                byte_end: 42,
                line: 2,
                column: 1,
            }),
            body: MirBody {
                form: MirBodyForm::Places,
                locals: vec![
                    local(u32_id, MirLocalKind::Return, true),
                    local(u32_id, MirLocalKind::Argument, false),
                ],
                blocks: vec![MirBasicBlock {
                    parameters: vec![],
                    statements: vec![MirStatement {
                        kind: MirStatementKind::Assign {
                            place: MirPlace::local(MirLocalId(0), u32_id),
                            value: MirRvalue::Use(MirOperand::Copy(MirPlace::local(
                                MirLocalId(1),
                                u32_id,
                            ))),
                        },
                        span: None,
                    }],
                    terminator: terminator(MirTerminatorKind::Return),
                }],
                entry: MirBlockId(0),
            },
        }],
    }
}

fn ssa_module() -> MirExecutableModule {
    let (types, _, u32_id) = fixture_types();
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget {
            pointer_width_bits: 32,
            thread_index_width_bits: 32,
        },
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "fixture::ssa::<u32>".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Ssa {
                    promoted_locals: vec![MirLocalId(1)],
                },
                locals: vec![
                    local(u32_id, MirLocalKind::Return, true),
                    local(u32_id, MirLocalKind::Argument, false),
                ],
                blocks: vec![
                    MirBasicBlock {
                        parameters: vec![MirBlockParameter {
                            value: MirValueId(0),
                            ty: u32_id,
                            origin: Some(MirLocalId(1)),
                        }],
                        statements: vec![MirStatement {
                            kind: MirStatementKind::Define {
                                value: MirValueId(1),
                                ty: u32_id,
                                rvalue: MirRvalue::BinaryOp {
                                    op: MirBinaryOp::Add,
                                    lhs: MirOperand::Value(MirValueId(0)),
                                    rhs: MirOperand::Constant(MirConstant {
                                        ty: u32_id,
                                        value: MirConstantValue::Integer(1),
                                    }),
                                },
                            },
                            span: None,
                        }],
                        terminator: terminator(MirTerminatorKind::Goto(MirEdge {
                            target: MirBlockId(1),
                            arguments: vec![MirOperand::Value(MirValueId(1))],
                        })),
                    },
                    MirBasicBlock {
                        parameters: vec![MirBlockParameter {
                            value: MirValueId(2),
                            ty: u32_id,
                            origin: Some(MirLocalId(1)),
                        }],
                        statements: vec![MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(0), u32_id),
                                value: MirRvalue::Use(MirOperand::Value(MirValueId(2))),
                            },
                            span: None,
                        }],
                        terminator: terminator(MirTerminatorKind::Return),
                    },
                ],
                entry: MirBlockId(0),
            },
        }],
    }
}

#[test]
fn verifies_place_and_explicit_ssa_forms() {
    place_module().validate().unwrap();
    ssa_module().validate().unwrap();
}

#[test]
fn rejects_cross_block_value_use_without_an_edge_argument() {
    let mut module = ssa_module();
    let MirStatementKind::Assign { value, .. } =
        &mut module.functions[0].body.blocks[1].statements[0].kind
    else {
        unreachable!();
    };
    *value = MirRvalue::Use(MirOperand::Value(MirValueId(1)));

    let error = module.validate().unwrap_err();
    assert!(
        error
            .reason()
            .contains("not a parameter or prior definition")
    );
}

#[test]
fn rejects_edge_arity_and_type_mismatches() {
    let mut module = ssa_module();
    let MirTerminatorKind::Goto(edge) = &mut module.functions[0].body.blocks[0].terminator.kind
    else {
        unreachable!();
    };
    edge.arguments.clear();
    let error = module.validate().unwrap_err();
    assert!(
        error
            .reason()
            .contains("supplies 0 arguments for 1 parameters")
    );

    let mut module = ssa_module();
    let (_, bool_id, _) = fixture_types();
    let MirTerminatorKind::Goto(edge) = &mut module.functions[0].body.blocks[0].terminator.kind
    else {
        unreachable!();
    };
    edge.arguments[0] = MirOperand::Constant(MirConstant {
        ty: bool_id,
        value: MirConstantValue::Bool(true),
    });
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("type mismatch")
    );
}

#[test]
fn rejects_forged_promoted_places_and_noncanonical_values() {
    let mut module = ssa_module();
    module.functions[0].body.blocks[1].statements[0] = MirStatement {
        kind: MirStatementKind::Assign {
            place: MirPlace::local(MirLocalId(0), module.functions[0].body.locals[0].ty),
            value: MirRvalue::Use(MirOperand::Copy(MirPlace::local(
                MirLocalId(1),
                module.functions[0].body.locals[1].ty,
            ))),
        },
        span: None,
    };
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("promoted local")
    );

    let mut module = ssa_module();
    module.functions[0].body.blocks[1].parameters[0].value = MirValueId(9);
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("numbered canonically")
    );
}

#[test]
fn rejects_unreachable_blocks_and_invalid_spans() {
    let mut module = place_module();
    module.functions[0].body.blocks.push(MirBasicBlock {
        parameters: vec![],
        statements: vec![],
        terminator: terminator(MirTerminatorKind::Return),
    });
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("unreachable")
    );

    let mut module = place_module();
    module.functions[0].span.as_mut().unwrap().byte_start = 100;
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("start exceeds")
    );
}

#[test]
fn rejects_invalid_binary_and_checked_operation_profiles() {
    let mut module = ssa_module();
    let (types, bool_ty, _) = fixture_types();
    module.types = types;
    let MirStatementKind::Define { rvalue, .. } =
        &mut module.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    *rvalue = MirRvalue::BinaryOp {
        op: MirBinaryOp::Add,
        lhs: MirOperand::Constant(MirConstant {
            ty: bool_ty,
            value: MirConstantValue::Bool(false),
        }),
        rhs: MirOperand::Constant(MirConstant {
            ty: bool_ty,
            value: MirConstantValue::Bool(true),
        }),
    };
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("invalid for its operand type")
    );

    let mut module = ssa_module();
    let MirStatementKind::Define { rvalue, .. } =
        &mut module.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let (_, _, u32_ty) = fixture_types();
    *rvalue = MirRvalue::CheckedBinaryOp {
        op: MirBinaryOp::Div,
        lhs: MirOperand::Constant(MirConstant {
            ty: u32_ty,
            value: MirConstantValue::Integer(4),
        }),
        rhs: MirOperand::Constant(MirConstant {
            ty: u32_ty,
            value: MirConstantValue::Integer(2),
        }),
    };
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("limited to integer add")
    );
}

#[test]
fn rejects_untouched_return_and_originless_entry_parameters() {
    let mut module = place_module();
    module.functions[0].body.blocks[0].statements.clear();
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("not definitely initialized"));

    let mut module = ssa_module();
    module.functions[0].body.blocks[0].parameters[0].origin = None;
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("require an argument-local origin"));
}

#[test]
fn rejects_double_move_of_a_conservatively_non_copy_value() {
    let u32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
        4,
        4,
    );
    let tuple_ty = ty(
        MirTypeKind::Tuple(MirAggregateLayout {
            fields: vec![MirField {
                name: None,
                offset: 0,
                ty: u32_ty.clone(),
            }],
            padding: vec![],
        }),
        4,
        4,
    );
    let mut types = vec![u32_ty.clone(), tuple_ty.clone()];
    types.sort_by_key(|item| item.canonical_text().unwrap());
    let u32_id = MirTypeId(types.iter().position(|item| item == &u32_ty).unwrap() as u32);
    let tuple_id = MirTypeId(types.iter().position(|item| item == &tuple_ty).unwrap() as u32);
    let mut module = MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget {
            pointer_width_bits: 32,
            thread_index_width_bits: 32,
        },
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "fixture::double_move".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals: vec![
                    local(u32_id, MirLocalKind::Return, true),
                    local(tuple_id, MirLocalKind::Argument, false),
                    local(tuple_id, MirLocalKind::Temporary, true),
                ],
                blocks: vec![MirBasicBlock {
                    parameters: vec![],
                    statements: vec![
                        MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(0), u32_id),
                                value: MirRvalue::Use(MirOperand::Constant(MirConstant {
                                    ty: u32_id,
                                    value: MirConstantValue::Integer(0),
                                })),
                            },
                            span: None,
                        },
                        MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(2), tuple_id),
                                value: MirRvalue::Use(MirOperand::Move(MirPlace::local(
                                    MirLocalId(1),
                                    tuple_id,
                                ))),
                            },
                            span: None,
                        },
                        MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(2), tuple_id),
                                value: MirRvalue::Use(MirOperand::Move(MirPlace::local(
                                    MirLocalId(1),
                                    tuple_id,
                                ))),
                            },
                            span: None,
                        },
                    ],
                    terminator: terminator(MirTerminatorKind::Return),
                }],
                entry: MirBlockId(0),
            },
        }],
    };

    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("moved and not reinitialized"));

    module.functions[0].body.blocks[0].statements.pop();
    module.validate().unwrap();
}

#[test]
fn tracks_call_destinations_separately_on_normal_and_unwind_edges() {
    let mut module = place_module();
    let return_ty = module.functions[0].body.locals[0].ty;
    let return_semantic_ty = module.type_at(return_ty).unwrap().clone();
    module.callables.push(MirCallable {
        identity: "fixture::may_unwind".into(),
        authority: MirCallAuthority::DeviceImport {
            contract: "fixture::may_unwind::contract".into(),
        },
        signature: MirCallSignature {
            inputs: vec![],
            output: MirCallReturn::Value(return_ty),
            can_unwind: true,
        },
    });
    module.functions[0].body.blocks = vec![
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::Call(MirCall {
                callee: MirCallee::Direct("fixture::may_unwind".into()),
                arguments: vec![],
                destination: Some(MirPlace::local(MirLocalId(0), return_ty)),
                target: Some(MirEdge::new(MirBlockId(1))),
                unwind: MirUnwindAction::Cleanup(MirEdge::new(MirBlockId(2))),
            })),
        },
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::Return),
        },
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::Return),
        },
    ];

    let registry = external_registry(
        "fixture::may_unwind",
        "fixture::may_unwind::contract",
        vec![],
        MirExternalCallReturn::Value(return_semantic_ty),
        true,
    );
    let error = module.validate_with_registry(&registry).unwrap_err();
    assert_eq!(
        error.path(),
        "module.functions[0].body.blocks[2].terminator"
    );
    assert!(error.reason().contains("not definitely initialized"));
}

#[test]
fn rejects_unknown_callees_and_switch_values_outside_the_discriminant() {
    let mut module = place_module();
    module.functions[0].body.blocks[0].terminator = terminator(MirTerminatorKind::Call(MirCall {
        callee: MirCallee::Intrinsic("unknown.intrinsic".into()),
        arguments: vec![],
        destination: None,
        target: None,
        unwind: MirUnwindAction::Unreachable,
    }));
    let error = module.validate().unwrap_err();
    assert!(
        error
            .reason()
            .contains("absent from the authority registry")
    );

    let mut module = place_module();
    let argument_ty = module.functions[0].body.locals[1].ty;
    module.functions[0].body.blocks.push(MirBasicBlock {
        parameters: vec![],
        statements: vec![],
        terminator: terminator(MirTerminatorKind::Return),
    });
    module.functions[0].body.blocks[0].terminator = terminator(MirTerminatorKind::SwitchInt {
        discr: MirOperand::Copy(MirPlace::local(MirLocalId(1), argument_ty)),
        targets: vec![(1_u128 << 32, MirEdge::new(MirBlockId(1)))],
        otherwise: MirEdge::new(MirBlockId(1)),
    });
    let error = module.validate().unwrap_err();
    assert!(
        error
            .reason()
            .contains("does not fit the discriminant type")
    );
}

#[test]
fn immutable_locals_have_exactly_one_initialization_and_entry_backedges_are_rejected() {
    let mut module = place_module();
    module.functions[0].body.locals[0].mutable = false;
    module.validate().unwrap();

    let duplicate = module.functions[0].body.blocks[0].statements[0].clone();
    module.functions[0].body.blocks[0]
        .statements
        .push(duplicate);
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("initialized exactly once"));

    let mut module = place_module();
    module.functions[0].body.blocks[0].terminator =
        terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(0))));
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("canonical entry block"));
}

fn sequence_module(slice: bool) -> MirExecutableModule {
    let u32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
        4,
        4,
    );
    let array2_ty = ty(
        MirTypeKind::Array {
            element: Box::new(u32_ty.clone()),
            length: 2,
        },
        8,
        4,
    );
    let array4_ty = ty(
        MirTypeKind::Array {
            element: Box::new(u32_ty.clone()),
            length: 4,
        },
        16,
        4,
    );
    let slice_ty = MirSemanticType {
        layout: MirLayout::dynamically_sized(4),
        kind: MirTypeKind::Slice {
            element: Box::new(u32_ty.clone()),
        },
    };
    let mut types = vec![
        u32_ty.clone(),
        array2_ty.clone(),
        array4_ty.clone(),
        slice_ty.clone(),
    ];
    types.sort_by_key(|item| item.canonical_text().unwrap());
    let id = |needle: &MirSemanticType| {
        MirTypeId(types.iter().position(|item| item == needle).unwrap() as u32)
    };
    let u32_id = id(&u32_ty);
    let array2_id = id(&array2_ty);
    let sequence_id = if slice { id(&slice_ty) } else { id(&array4_ty) };
    let projection = if slice {
        MirProjection::Subslice {
            from: 1,
            to: 1,
            from_end: true,
            min_length: 2,
        }
    } else {
        MirProjection::Subslice {
            from: 1,
            to: 3,
            from_end: false,
            min_length: 4,
        }
    };
    let projected_ty = if slice { sequence_id } else { array2_id };
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget {
            pointer_width_bits: 32,
            thread_index_width_bits: 32,
        },
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "fixture::sequence".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals: vec![
                    local(u32_id, MirLocalKind::Return, true),
                    local(sequence_id, MirLocalKind::Argument, false),
                    local(u32_id, MirLocalKind::Argument, false),
                ],
                blocks: vec![MirBasicBlock {
                    parameters: vec![],
                    statements: vec![MirStatement {
                        kind: MirStatementKind::Assign {
                            place: MirPlace::local(MirLocalId(0), u32_id),
                            value: MirRvalue::Len(MirPlace {
                                local: MirLocalId(1),
                                projection: vec![projection],
                                ty: projected_ty,
                            }),
                        },
                        span: None,
                    }],
                    terminator: terminator(MirTerminatorKind::Return),
                }],
                entry: MirBlockId(0),
            },
        }],
    }
}

#[test]
fn enforces_rustc_constant_index_and_subslice_semantics() {
    let module = sequence_module(false);
    module.validate().unwrap();

    let mut invalid = module.clone();
    let MirStatementKind::Assign { value, .. } =
        &mut invalid.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::Len(place) = value else {
        unreachable!();
    };
    let MirProjection::Subslice { from_end, .. } = &mut place.projection[0] else {
        unreachable!();
    };
    *from_end = true;
    let MirProjection::Subslice { to, .. } = &mut place.projection[0] else {
        unreachable!();
    };
    *to = 1;
    invalid.validate().unwrap();

    let mut invalid = module.clone();
    let MirStatementKind::Assign { value, .. } =
        &mut invalid.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::Len(place) = value else {
        unreachable!();
    };
    let MirProjection::Subslice { to, .. } = &mut place.projection[0] else {
        unreachable!();
    };
    *to = 5;
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .reason()
            .contains("bounds exceed")
    );

    let mut constant = module;
    let u32_id = constant.functions[0].body.locals[0].ty;
    let MirStatementKind::Assign { value, .. } =
        &mut constant.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    *value = MirRvalue::Use(MirOperand::Copy(MirPlace {
        local: MirLocalId(1),
        projection: vec![MirProjection::ConstantIndex {
            offset: 3,
            min_length: 4,
            from_end: false,
        }],
        ty: u32_id,
    }));
    constant.validate().unwrap();
    let MirStatementKind::Assign { value, .. } =
        &mut constant.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::Use(MirOperand::Copy(place)) = value else {
        unreachable!();
    };
    let MirProjection::ConstantIndex { from_end, .. } = &mut place.projection[0] else {
        unreachable!();
    };
    *from_end = true;
    constant.validate().unwrap();

    {
        let MirStatementKind::Assign { value, .. } =
            &mut constant.functions[0].body.blocks[0].statements[0].kind
        else {
            unreachable!();
        };
        let MirRvalue::Use(MirOperand::Copy(place)) = value else {
            unreachable!();
        };
        let MirProjection::ConstantIndex {
            offset,
            min_length,
            from_end,
        } = &mut place.projection[0]
        else {
            unreachable!();
        };
        *offset = 999;
        *min_length = 4;
        *from_end = false;
    }
    assert!(
        constant
            .validate()
            .unwrap_err()
            .reason()
            .contains("static array bounds")
    );

    let MirStatementKind::Assign { value, .. } =
        &mut constant.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::Use(MirOperand::Copy(place)) = value else {
        unreachable!();
    };
    let MirProjection::ConstantIndex {
        offset, min_length, ..
    } = &mut place.projection[0]
    else {
        unreachable!();
    };
    *offset = 3;
    *min_length = 999;
    assert!(
        constant
            .validate()
            .unwrap_err()
            .reason()
            .contains("must equal the static array length")
    );

    let mut too_wide = sequence_module(false);
    let u32_id = too_wide.functions[0].body.locals[0].ty;
    let MirStatementKind::Assign { value, .. } =
        &mut too_wide.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    *value = MirRvalue::Use(MirOperand::Copy(MirPlace {
        local: MirLocalId(1),
        projection: vec![MirProjection::ConstantIndex {
            offset: u64::from(u32::MAX) + 1,
            min_length: u64::from(u32::MAX) + 2,
            from_end: false,
        }],
        ty: u32_id,
    }));
    assert!(
        too_wide
            .validate()
            .unwrap_err()
            .reason()
            .contains("target usize width")
    );

    let slice = sequence_module(true);
    assert!(
        slice
            .validate()
            .unwrap_err()
            .reason()
            .contains("external bound witness")
    );
}

#[test]
fn target_controls_usize_index_and_thread_index_widths() {
    let mut thread = place_module();
    let MirStatementKind::Assign { value, .. } =
        &mut thread.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    *value = MirRvalue::ThreadIndex1d;
    thread.validate().unwrap();
    thread.target.thread_index_width_bits = 64;
    assert!(
        thread
            .validate()
            .unwrap_err()
            .reason()
            .contains("required result type")
    );

    let mut len = sequence_module(false);
    len.target.pointer_width_bits = 64;
    assert!(
        len.validate()
            .unwrap_err()
            .reason()
            .contains("required result type")
    );

    let mut index = sequence_module(false);
    let u32_id = index.functions[0].body.locals[0].ty;
    let MirStatementKind::Assign { value, .. } =
        &mut index.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    *value = MirRvalue::Use(MirOperand::Copy(MirPlace {
        local: MirLocalId(1),
        projection: vec![MirProjection::Index {
            local: MirLocalId(2),
        }],
        ty: u32_id,
    }));
    assert!(
        index
            .validate()
            .unwrap_err()
            .reason()
            .contains("external range witness")
    );
}

#[derive(Clone, Copy)]
struct PointerTypes {
    u32_ty: MirTypeId,
    shared_ref_as0: MirTypeId,
    shared_ref_as1: MirTypeId,
    mutable_ref_as1: MirTypeId,
    const_ptr_as0: MirTypeId,
    const_ptr_as1: MirTypeId,
    mut_ptr_as1: MirTypeId,
}

fn pointer_module(raw: bool) -> (MirExecutableModule, PointerTypes) {
    let u32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
        4,
        4,
    );
    let pointer = |raw: bool, mutability, address_space| {
        ty(
            if raw {
                MirTypeKind::RawPointer {
                    pointee: Box::new(u32_ty.clone()),
                    mutability,
                    address_space,
                }
            } else {
                MirTypeKind::Reference {
                    referent: Box::new(u32_ty.clone()),
                    mutability,
                    address_space,
                }
            },
            4,
            4,
        )
    };
    let shared_ref_as0 = pointer(false, MirMutability::Immutable, MirAddressSpace(0));
    let shared_ref_as1 = pointer(false, MirMutability::Immutable, MirAddressSpace(1));
    let mutable_ref_as1 = pointer(false, MirMutability::Mutable, MirAddressSpace(1));
    let const_ptr_as0 = pointer(true, MirMutability::Immutable, MirAddressSpace(0));
    let const_ptr_as1 = pointer(true, MirMutability::Immutable, MirAddressSpace(1));
    let mut_ptr_as1 = pointer(true, MirMutability::Mutable, MirAddressSpace(1));
    let mut types = vec![
        u32_ty.clone(),
        shared_ref_as0.clone(),
        shared_ref_as1.clone(),
        mutable_ref_as1.clone(),
        const_ptr_as0.clone(),
        const_ptr_as1.clone(),
        mut_ptr_as1.clone(),
    ];
    types.sort_by_key(|item| item.canonical_text().unwrap());
    let id = |needle: &MirSemanticType| {
        MirTypeId(types.iter().position(|item| item == needle).unwrap() as u32)
    };
    let ids = PointerTypes {
        u32_ty: id(&u32_ty),
        shared_ref_as0: id(&shared_ref_as0),
        shared_ref_as1: id(&shared_ref_as1),
        mutable_ref_as1: id(&mutable_ref_as1),
        const_ptr_as0: id(&const_ptr_as0),
        const_ptr_as1: id(&const_ptr_as1),
        mut_ptr_as1: id(&mut_ptr_as1),
    };
    let result_ty = if raw {
        ids.const_ptr_as1
    } else {
        ids.shared_ref_as1
    };
    let mut argument = local(ids.u32_ty, MirLocalKind::Argument, false);
    argument.storage_address_space = MirAddressSpace(1);
    let value = if raw {
        MirRvalue::AddressOf {
            mutability: MirMutability::Immutable,
            place: MirPlace::local(MirLocalId(1), ids.u32_ty),
            ty: result_ty,
        }
    } else {
        MirRvalue::Ref {
            mutability: MirMutability::Immutable,
            place: MirPlace::local(MirLocalId(1), ids.u32_ty),
            ty: result_ty,
        }
    };
    (
        MirExecutableModule {
            version: MirExecutableVersion::V1,
            target: MirExecutableTarget {
                pointer_width_bits: 32,
                thread_index_width_bits: 32,
            },
            types,
            callables: vec![],
            functions: vec![MirFunction {
                identity: "fixture::pointer".into(),
                span: None,
                body: MirBody {
                    form: MirBodyForm::Places,
                    locals: vec![local(result_ty, MirLocalKind::Return, true), argument],
                    blocks: vec![MirBasicBlock {
                        parameters: vec![],
                        statements: vec![MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(0), result_ty),
                                value,
                            },
                            span: None,
                        }],
                        terminator: terminator(MirTerminatorKind::Return),
                    }],
                    entry: MirBlockId(0),
                },
            }],
        },
        ids,
    )
}

#[test]
fn references_and_raw_addresses_require_exact_mutability_and_address_space() {
    let (reference, ids) = pointer_module(false);
    reference.validate().unwrap();

    let mut wrong_space = reference.clone();
    let MirStatementKind::Assign { value, .. } =
        &mut wrong_space.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::Ref { ty, .. } = value else {
        unreachable!();
    };
    *ty = ids.shared_ref_as0;
    assert!(
        wrong_space
            .validate()
            .unwrap_err()
            .reason()
            .contains("address space")
    );

    let mut mutable = reference;
    mutable.functions[0].body.locals[0].ty = ids.mutable_ref_as1;
    let MirStatementKind::Assign { place, value } =
        &mut mutable.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    place.ty = ids.mutable_ref_as1;
    let MirRvalue::Ref { mutability, ty, .. } = value else {
        unreachable!();
    };
    *mutability = MirMutability::Mutable;
    *ty = ids.mutable_ref_as1;
    assert!(
        mutable
            .validate()
            .unwrap_err()
            .reason()
            .contains("requires a writable place")
    );

    let (address, ids) = pointer_module(true);
    address.validate().unwrap();
    let mut wrong_space = address.clone();
    let MirStatementKind::Assign { value, .. } =
        &mut wrong_space.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    let MirRvalue::AddressOf { ty, .. } = value else {
        unreachable!();
    };
    *ty = ids.const_ptr_as0;
    assert!(
        wrong_space
            .validate()
            .unwrap_err()
            .reason()
            .contains("address space")
    );

    let mut mutable = address;
    mutable.functions[0].body.locals[0].ty = ids.mut_ptr_as1;
    let MirStatementKind::Assign { place, value } =
        &mut mutable.functions[0].body.blocks[0].statements[0].kind
    else {
        unreachable!();
    };
    place.ty = ids.mut_ptr_as1;
    let MirRvalue::AddressOf { mutability, ty, .. } = value else {
        unreachable!();
    };
    *mutability = MirMutability::Mutable;
    *ty = ids.mut_ptr_as1;
    assert!(
        mutable
            .validate()
            .unwrap_err()
            .reason()
            .contains("requires a writable place")
    );
}

#[derive(Clone, Copy)]
struct IntrinsicTypes {
    unit: MirTypeId,
    u32_ty: MirTypeId,
    const_ptr: MirTypeId,
    mut_ptr: MirTypeId,
}

fn intrinsic_module() -> (MirExecutableModule, IntrinsicTypes) {
    let unit = ty(MirTypeKind::Unit, 0, 1);
    let u32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
        4,
        4,
    );
    let i32_ty = ty(
        MirTypeKind::Scalar(MirScalarType::Int {
            signed: true,
            bits: 32,
        }),
        4,
        4,
    );
    let pointer = |mutability| {
        ty(
            MirTypeKind::RawPointer {
                pointee: Box::new(u32_ty.clone()),
                mutability,
                address_space: MirAddressSpace(1),
            },
            4,
            4,
        )
    };
    let const_ptr = pointer(MirMutability::Immutable);
    let mut_ptr = pointer(MirMutability::Mutable);
    let mut types = vec![
        unit.clone(),
        u32_ty.clone(),
        i32_ty.clone(),
        const_ptr.clone(),
        mut_ptr.clone(),
    ];
    types.sort_by_key(|item| item.canonical_text().unwrap());
    let id = |needle: &MirSemanticType| {
        MirTypeId(types.iter().position(|item| item == needle).unwrap() as u32)
    };
    let ids = IntrinsicTypes {
        unit: id(&unit),
        u32_ty: id(&u32_ty),
        const_ptr: id(&const_ptr),
        mut_ptr: id(&mut_ptr),
    };
    let i32_id = id(&i32_ty);
    let callables = vec![
        MirCallable {
            identity: "fe2o3.copy_nonoverlapping".into(),
            authority: MirCallAuthority::Intrinsic(MirIntrinsic::CopyNonOverlapping),
            signature: MirCallSignature {
                inputs: vec![ids.const_ptr, ids.mut_ptr, ids.u32_ty],
                output: MirCallReturn::Value(ids.unit),
                can_unwind: false,
            },
        },
        MirCallable {
            identity: "fe2o3.pointer_distance".into(),
            authority: MirCallAuthority::Intrinsic(MirIntrinsic::PointerDistance),
            signature: MirCallSignature {
                inputs: vec![ids.const_ptr, ids.const_ptr],
                output: MirCallReturn::Value(i32_id),
                can_unwind: false,
            },
        },
        MirCallable {
            identity: "fe2o3.volatile_load".into(),
            authority: MirCallAuthority::Intrinsic(MirIntrinsic::VolatileLoad),
            signature: MirCallSignature {
                inputs: vec![ids.const_ptr],
                output: MirCallReturn::Value(ids.u32_ty),
                can_unwind: false,
            },
        },
        MirCallable {
            identity: "fe2o3.volatile_store".into(),
            authority: MirCallAuthority::Intrinsic(MirIntrinsic::VolatileStore),
            signature: MirCallSignature {
                inputs: vec![ids.mut_ptr, ids.u32_ty],
                output: MirCallReturn::Value(ids.unit),
                can_unwind: false,
            },
        },
    ];
    (
        MirExecutableModule {
            version: MirExecutableVersion::V1,
            target: MirExecutableTarget {
                pointer_width_bits: 32,
                thread_index_width_bits: 32,
            },
            types,
            callables,
            functions: vec![MirFunction {
                identity: "fixture::intrinsic_authority".into(),
                span: None,
                body: MirBody {
                    form: MirBodyForm::Places,
                    locals: vec![
                        local(ids.u32_ty, MirLocalKind::Return, true),
                        local(ids.u32_ty, MirLocalKind::Argument, false),
                    ],
                    blocks: vec![MirBasicBlock {
                        parameters: vec![],
                        statements: vec![MirStatement {
                            kind: MirStatementKind::Assign {
                                place: MirPlace::local(MirLocalId(0), ids.u32_ty),
                                value: MirRvalue::Use(MirOperand::Copy(MirPlace::local(
                                    MirLocalId(1),
                                    ids.u32_ty,
                                ))),
                            },
                            span: None,
                        }],
                        terminator: terminator(MirTerminatorKind::Return),
                    }],
                    entry: MirBlockId(0),
                },
            }],
        },
        ids,
    )
}

#[test]
fn intrinsic_variants_have_closed_exact_signatures() {
    let (module, ids) = intrinsic_module();
    module.validate().unwrap();

    let mut wrong_inputs = module.clone();
    wrong_inputs.callables[2].signature.inputs.clear();
    assert!(
        wrong_inputs
            .validate()
            .unwrap_err()
            .reason()
            .contains("exactly 1 inputs")
    );

    let mut wrong_output = module.clone();
    wrong_output.callables[2].signature.output = MirCallReturn::Value(ids.unit);
    assert!(
        wrong_output
            .validate()
            .unwrap_err()
            .reason()
            .contains("exactly match")
    );

    let mut forged_unwind = module.clone();
    forged_unwind.callables[2].signature.can_unwind = true;
    assert!(
        forged_unwind
            .validate()
            .unwrap_err()
            .reason()
            .contains("cannot unwind")
    );

    let mut forged_copy = module.clone();
    forged_copy.callables[0].signature.inputs[0] = ids.mut_ptr;
    assert!(
        forged_copy
            .validate()
            .unwrap_err()
            .reason()
            .contains("mutability")
    );

    let mut forged_distance = module.clone();
    forged_distance.callables[1].signature.inputs[1] = ids.mut_ptr;
    assert!(
        forged_distance
            .validate()
            .unwrap_err()
            .reason()
            .contains("mutability")
    );

    let mut forged_store = module;
    forged_store.callables[3].signature.inputs[0] = ids.const_ptr;
    assert!(
        forged_store
            .validate()
            .unwrap_err()
            .reason()
            .contains("mutability")
    );
}

#[test]
fn callable_registry_enforces_signatures_and_closed_intrinsic_authority() {
    let mut module = place_module();
    let u32_ty = module.functions[0].body.locals[0].ty;
    let u32_semantic_ty = module.type_at(u32_ty).unwrap().clone();
    module.callables.push(MirCallable {
        identity: "fixture::typed".into(),
        authority: MirCallAuthority::DeviceImport {
            contract: "fixture::typed::contract".into(),
        },
        signature: MirCallSignature {
            inputs: vec![u32_ty],
            output: MirCallReturn::Diverging,
            can_unwind: false,
        },
    });
    module.functions[0].body.blocks[0].terminator = terminator(MirTerminatorKind::Call(MirCall {
        callee: MirCallee::Direct("fixture::typed".into()),
        arguments: vec![],
        destination: None,
        target: None,
        unwind: MirUnwindAction::Unreachable,
    }));
    let registry = external_registry(
        "fixture::typed",
        "fixture::typed::contract",
        vec![u32_semantic_ty],
        MirExternalCallReturn::Diverging,
        false,
    );
    assert!(
        module
            .validate_with_registry(&registry)
            .unwrap_err()
            .reason()
            .contains("argument count")
    );

    let mut module = place_module();
    module.callables.push(MirCallable {
        identity: "unknown.intrinsic".into(),
        authority: MirCallAuthority::Intrinsic(dialect_mir::MirIntrinsic::VolatileLoad),
        signature: MirCallSignature {
            inputs: vec![],
            output: MirCallReturn::Diverging,
            can_unwind: false,
        },
    });
    assert!(
        module
            .validate()
            .unwrap_err()
            .reason()
            .contains("closed authority")
    );

    let mut malformed = place_module();
    malformed.callables.push(MirCallable {
        identity: malformed.functions[0].identity.clone(),
        authority: MirCallAuthority::DefinedFunction,
        signature: MirCallSignature {
            inputs: vec![],
            output: MirCallReturn::Diverging,
            can_unwind: false,
        },
    });
    malformed.functions[0].body.locals.clear();
    let error = malformed.validate().unwrap_err();
    assert!(error.reason().contains("no return local"));
}

#[test]
fn device_imports_require_an_exact_external_authority() {
    let mut module = place_module();
    let u32_ty = module.functions[0].body.locals[0].ty;
    let semantic_u32 = module.type_at(u32_ty).unwrap().clone();
    module.callables.push(MirCallable {
        identity: "fixture::trusted".into(),
        authority: MirCallAuthority::DeviceImport {
            contract: "fixture::trusted::v1".into(),
        },
        signature: MirCallSignature {
            inputs: vec![u32_ty],
            output: MirCallReturn::Value(u32_ty),
            can_unwind: false,
        },
    });
    let registry = external_registry(
        "fixture::trusted",
        "fixture::trusted::v1",
        vec![semantic_u32.clone()],
        MirExternalCallReturn::Value(semantic_u32),
        false,
    );

    module.validate_with_registry(&registry).unwrap();
    assert!(module.validate().unwrap_err().reason().contains("external"));

    let mut forged_contract = module.clone();
    let MirCallAuthority::DeviceImport { contract } = &mut forged_contract.callables[0].authority
    else {
        unreachable!();
    };
    *contract = "fixture::forged::v1".into();
    assert!(
        forged_contract
            .validate_with_registry(&registry)
            .unwrap_err()
            .reason()
            .contains("exactly match")
    );

    let mut forged_signature = module.clone();
    forged_signature.callables[0].signature.inputs.clear();
    assert!(
        forged_signature
            .validate_with_registry(&registry)
            .unwrap_err()
            .reason()
            .contains("exactly match")
    );

    let mut forged_unwind = module.clone();
    forged_unwind.callables[0].signature.can_unwind = true;
    assert!(
        forged_unwind
            .validate_with_registry(&registry)
            .unwrap_err()
            .reason()
            .contains("exactly match")
    );

    let mut forged_identity = module;
    forged_identity.callables[0].identity = "fixture::untrusted".into();
    assert!(
        forged_identity
            .validate_with_registry(&registry)
            .unwrap_err()
            .reason()
            .contains("absent")
    );
}

#[test]
fn defined_function_unwind_declarations_cover_body_effects() {
    let mut module = place_module();
    let identity = module.functions[0].identity.clone();
    let return_ty = module.functions[0].body.locals[0].ty;
    let argument_ty = module.functions[0].body.locals[1].ty;
    let bool_ty = MirTypeId(
        module
            .types
            .iter()
            .position(|ty| matches!(ty.kind, MirTypeKind::Scalar(MirScalarType::Bool)))
            .unwrap() as u32,
    );
    module.callables.push(MirCallable {
        identity,
        authority: MirCallAuthority::DefinedFunction,
        signature: MirCallSignature {
            inputs: vec![argument_ty],
            output: MirCallReturn::Value(return_ty),
            can_unwind: false,
        },
    });
    module.validate().unwrap();

    module.functions[0].body.blocks[0].terminator = terminator(MirTerminatorKind::Assert {
        condition: MirOperand::Constant(MirConstant {
            ty: bool_ty,
            value: MirConstantValue::Bool(true),
        }),
        expected: true,
        message: dialect_mir::MirAssertMessage::User("probe".into()),
        target: MirEdge::new(MirBlockId(1)),
        unwind: MirUnwindAction::Unreachable,
    });
    module.functions[0].body.blocks.push(MirBasicBlock {
        parameters: vec![],
        statements: vec![],
        terminator: terminator(MirTerminatorKind::Return),
    });

    module.callables[0].signature.can_unwind = true;
    module.validate().unwrap();
    module.callables[0].signature.can_unwind = false;
    let error = module.validate().unwrap_err();
    assert_eq!(error.path(), "module.callables[0].signature.can_unwind");
    assert!(error.reason().contains("body may unwind"));
}

#[test]
fn joins_require_initialization_on_every_normal_predecessor() {
    let mut module = place_module();
    let return_ty = module.functions[0].body.locals[0].ty;
    let argument_ty = module.functions[0].body.locals[1].ty;
    let initialize = module.functions[0].body.blocks[0].statements[0].clone();
    module.functions[0].body.blocks = vec![
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::SwitchInt {
                discr: MirOperand::Copy(MirPlace::local(MirLocalId(1), argument_ty)),
                targets: vec![(0, MirEdge::new(MirBlockId(1)))],
                otherwise: MirEdge::new(MirBlockId(2)),
            }),
        },
        MirBasicBlock {
            parameters: vec![],
            statements: vec![initialize],
            terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(3)))),
        },
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(3)))),
        },
        MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::Return),
        },
    ];
    assert_eq!(module.functions[0].body.locals[0].ty, return_ty);
    let error = module.validate().unwrap_err();
    assert!(error.reason().contains("only some incoming paths"));
}
