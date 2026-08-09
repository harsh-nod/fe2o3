use dialect_mir::{
    MirBasicBlock, MirBinaryOp, MirBlockId, MirBlockParameter, MirBody, MirBodyForm, MirConstant,
    MirConstantValue, MirEdge, MirExecutableModule, MirExecutableVersion, MirFunction, MirLayout,
    MirLocalDecl, MirLocalId, MirLocalKind, MirOperand, MirPlace, MirRvalue, MirScalarType,
    MirSemanticType, MirSourceSpan, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeId, MirTypeKind, MirValueId,
};

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
        types,
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
        types,
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
