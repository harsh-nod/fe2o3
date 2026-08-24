use super::*;
use crate::mir_import::{MirImportedType, MirLocal, MirLocalRole, MirSwitchTarget};
use dialect_mir::MirType;

#[test]
fn gfx942_loop_break_and_continue_lower_to_verified_block_arguments() {
    let module = translate_and_verify_for_target(&loop_fixture(), &AmdGpuTarget::new("gfx942"))
        .expect("bounded gfx942 loop must lower");
    let function = module
        .functions
        .iter()
        .find(|function| function.id.as_str() == "tests::bounded_loop")
        .expect("kernel definition");
    let body = function.body.as_ref().expect("kernel body");

    for block_index in [1, 2, 3, 4, 5] {
        let block = body
            .blocks
            .iter()
            .find(|block| block.id.0 == block_index)
            .expect("loop block");
        assert_eq!(block.parameters.len(), 1, "bb{block_index}");
        assert_eq!(block.parameters[0].ty, Type::Scalar(ScalarType::U32));
    }

    let dispatch = body
        .blocks
        .iter()
        .find(|block| block.id.0 == 2)
        .expect("break/continue dispatch");
    let Terminator::Switch {
        cases,
        default_arguments,
        ..
    } = dispatch.terminator.as_ref().expect("dispatch terminator")
    else {
        panic!("expected integer dispatch switch");
    };
    assert_eq!(cases.len(), 2);
    assert!(cases.iter().all(|case| case.arguments.len() == 1));
    assert_eq!(default_arguments.len(), 1);
}

#[test]
fn boolean_switch_with_complement_default_lowers_to_conditional_branch() {
    let module = translate_and_verify(&boolean_branch_fixture())
        .expect("canonical rustc boolean branch must lower");
    let body = module.functions[0]
        .body
        .as_ref()
        .expect("boolean branch body");
    assert!(matches!(
        body.blocks[0].terminator,
        Some(Terminator::ConditionalBranch { .. })
    ));
}

#[test]
fn boolean_switch_rejects_non_boolean_cases() {
    let mut fixture = boolean_branch_fixture();
    let MirTerminatorKind::SwitchInt { targets, .. } = &mut fixture.functions[0].blocks[0]
        .terminator
        .as_mut()
        .expect("boolean switch")
        .kind
    else {
        panic!("boolean switch")
    };
    targets[0].value = 2;

    let error = translate_and_verify(&fixture).expect_err("non-boolean case must fail closed");
    assert!(error.to_string().contains("non-boolean case value 2"));
}

#[test]
fn mutable_control_flow_rejects_every_non_gfx942_profile() {
    for target in ["gfx90a", "gfx950"] {
        let error = translate_and_verify_for_target(&loop_fixture(), &AmdGpuTarget::new(target))
            .expect_err("mutable control flow must be target bounded");
        assert!(
            error
                .to_string()
                .contains("only for the exact gfx942 target profile"),
            "{target}: {error}"
        );
    }
}

#[test]
fn duplicate_live_value_edges_fail_closed() {
    let mut fixture = loop_fixture();
    let MirTerminatorKind::SwitchInt { otherwise, .. } = &mut fixture.functions[0].blocks[2]
        .terminator
        .as_mut()
        .expect("dispatch terminator")
        .kind
    else {
        panic!("dispatch switch");
    };
    *otherwise = 3;

    let error = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
        .expect_err("duplicate live edges must fail");
    assert!(
        error
            .to_string()
            .contains("multiple live-value edges to bb3"),
        "{error}"
    );
}

#[test]
fn mutable_local_read_before_entry_definition_fails_closed() {
    let mut fixture = loop_fixture();
    fixture.functions[0].blocks[0].statements.clear();
    fixture.functions[0].blocks[5].statements.push(assign(
        0,
        1,
        vec![u32_constant(9)],
        MirRvalueKind::Use,
    ));

    let error = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
        .expect_err("undefined loop-carried local must fail");
    assert!(
        error
            .to_string()
            .contains("reads a mutable local before its entry definition"),
        "{error}"
    );
}

#[test]
fn control_flow_block_budget_fails_closed() {
    let mut fixture = loop_fixture();
    fixture.functions[0].blocks = (0..129)
        .map(|index| MirBlock {
            index,
            statements: Vec::new(),
            terminator: Some(terminator(MirTerminatorKind::Return)),
        })
        .collect();

    let error = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
        .expect_err("oversized CFG must fail");
    assert!(
        error.to_string().contains("at most 128 MIR blocks"),
        "{error}"
    );
}

#[test]
fn gfx942_fieldless_enum_match_uses_authenticated_discriminants() {
    let module = translate_and_verify_for_target(&enum_fixture(), &AmdGpuTarget::new("gfx942"))
        .expect("fieldless enum match must lower");
    let function = module
        .functions
        .iter()
        .find(|function| function.id.as_str() == "tests::enum_match")
        .expect("kernel definition");
    let body = function.body.as_ref().expect("kernel body");
    let header = body
        .blocks
        .iter()
        .find(|block| block.id.0 == 1)
        .expect("match header");
    assert_eq!(header.parameters.len(), 1);
    assert_eq!(header.parameters[0].ty, Type::Scalar(ScalarType::I64));
    assert!(matches!(header.terminator, Some(Terminator::Switch { .. })));

    let variants = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::I64(value)) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(variants, [2, 7]);
}

#[test]
fn unauthenticated_enum_aggregate_remains_rejected() {
    let mut fixture = enum_fixture();
    fixture.functions[0].blocks[0].statements[0].rvalue = Some(MirRvalueKind::Aggregate);

    let error = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
        .expect_err("generic aggregate must not impersonate a fieldless enum");
    assert!(
        error
            .to_string()
            .contains("unsupported structured MIR rvalue"),
        "{error}"
    );
}

fn loop_fixture() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            semantic_instance: None,
            export_name: "bounded_loop".to_owned(),
            rust_path: "tests::bounded_loop".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: None,
            frontend_contract: None,
            matrix_frontend_abi: None,
            arg_count: 0,
            local_count: 4,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Temp, MirTypeShape::U32),
                local(2, MirLocalRole::Temp, MirTypeShape::Bool),
                local(3, MirLocalRole::Temp, MirTypeShape::U32),
            ],
            blocks: vec![
                MirBlock {
                    index: 0,
                    statements: vec![assign(0, 1, vec![u32_constant(0)], MirRvalueKind::Use)],
                    terminator: Some(terminator(MirTerminatorKind::Goto { target: 1 })),
                },
                MirBlock {
                    index: 1,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::SwitchInt {
                        discriminant: operand(1),
                        targets: vec![MirSwitchTarget {
                            value: 8,
                            target: 4,
                        }],
                        otherwise: 2,
                    })),
                },
                MirBlock {
                    index: 2,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::SwitchInt {
                        discriminant: operand(1),
                        targets: vec![
                            MirSwitchTarget {
                                value: 1,
                                target: 3,
                            },
                            MirSwitchTarget {
                                value: 5,
                                target: 4,
                            },
                        ],
                        otherwise: 5,
                    })),
                },
                MirBlock {
                    index: 3,
                    statements: vec![assign(
                        0,
                        1,
                        vec![operand(1), u32_constant(1)],
                        MirRvalueKind::Binary(MirBinaryOp::Add),
                    )],
                    terminator: Some(terminator(MirTerminatorKind::Goto { target: 1 })),
                },
                MirBlock {
                    index: 4,
                    statements: vec![assign(0, 3, vec![operand(1)], MirRvalueKind::Use)],
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                },
                MirBlock {
                    index: 5,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::Goto { target: 3 })),
                },
            ],
        }],
    }
}

fn enum_fixture() -> MirModule {
    let enum_shape = MirTypeShape::Adt {
        identity: "tests::Mode".to_owned(),
    };
    MirModule {
        functions: vec![MirFunction {
            semantic_instance: None,
            export_name: "enum_match".to_owned(),
            rust_path: "tests::enum_match".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: None,
            frontend_contract: None,
            matrix_frontend_abi: None,
            arg_count: 0,
            local_count: 3,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Temp, enum_shape),
                local(2, MirLocalRole::Temp, MirTypeShape::I64),
            ],
            blocks: vec![
                MirBlock {
                    index: 0,
                    statements: vec![assign(
                        0,
                        1,
                        Vec::new(),
                        MirRvalueKind::FieldlessEnumVariant(2),
                    )],
                    terminator: Some(terminator(MirTerminatorKind::Goto { target: 1 })),
                },
                MirBlock {
                    index: 1,
                    statements: vec![assign(0, 2, vec![operand(1)], MirRvalueKind::Discriminant)],
                    terminator: Some(terminator(MirTerminatorKind::SwitchInt {
                        discriminant: operand(2),
                        targets: vec![MirSwitchTarget {
                            value: 2,
                            target: 2,
                        }],
                        otherwise: 3,
                    })),
                },
                MirBlock {
                    index: 2,
                    statements: vec![assign(
                        0,
                        1,
                        Vec::new(),
                        MirRvalueKind::FieldlessEnumVariant(7),
                    )],
                    terminator: Some(terminator(MirTerminatorKind::Goto { target: 1 })),
                },
                MirBlock {
                    index: 3,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                },
            ],
        }],
    }
}

fn boolean_branch_fixture() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            semantic_instance: None,
            export_name: "boolean_branch".to_owned(),
            rust_path: "tests::boolean_branch".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: None,
            frontend_contract: None,
            matrix_frontend_abi: None,
            arg_count: 0,
            local_count: 2,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Temp, MirTypeShape::Bool),
            ],
            blocks: vec![
                MirBlock {
                    index: 0,
                    statements: vec![assign(0, 1, vec![bool_constant(true)], MirRvalueKind::Use)],
                    terminator: Some(terminator(MirTerminatorKind::SwitchInt {
                        discriminant: operand(1),
                        targets: vec![MirSwitchTarget {
                            value: 0,
                            target: 1,
                        }],
                        otherwise: 2,
                    })),
                },
                MirBlock {
                    index: 1,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                },
                MirBlock {
                    index: 2,
                    statements: Vec::new(),
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                },
            ],
        }],
    }
}

fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
    let (kind, rust) = match shape {
        MirTypeShape::Unit => (MirType::Unit, "()"),
        MirTypeShape::Bool => (MirType::I1, "bool"),
        MirTypeShape::U32 => (MirType::I32, "u32"),
        MirTypeShape::I64 => (MirType::I64, "i64"),
        MirTypeShape::Adt { .. } => (MirType::Unknown, "tests::Mode"),
        _ => (MirType::Unknown, "<unknown>"),
    };
    MirLocal {
        index,
        role,
        ty: MirImportedType {
            kind,
            rust: rust.to_owned(),
            shape,
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        },
    }
}

fn assign(
    index: usize,
    destination: usize,
    operands: Vec<MirOperandRef>,
    rvalue: MirRvalueKind,
) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(place(destination)),
        operands,
        rvalue: Some(rvalue),
        semantic_rvalue_type: None,
        operation: Some("structured".to_owned()),
        source: Some(source()),
    }
}

fn operand(local: usize) -> MirOperandRef {
    MirOperandRef::Place(place(local))
}

fn u32_constant(value: u32) -> MirOperandRef {
    MirOperandRef::Constant {
        ty: MirImportedType {
            kind: MirType::I32,
            rust: "u32".to_owned(),
            shape: MirTypeShape::U32,
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        },
        literal: MirConstant::U32(value),
        value: value.to_string(),
    }
}

fn bool_constant(value: bool) -> MirOperandRef {
    MirOperandRef::Constant {
        ty: MirImportedType {
            kind: MirType::I1,
            rust: "bool".to_owned(),
            shape: MirTypeShape::Bool,
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        },
        literal: MirConstant::Bool(value),
        value: value.to_string(),
    }
}

fn place(local: usize) -> MirPlaceRef {
    MirPlaceRef {
        local,
        projection: Vec::new(),
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    }
}

fn terminator(kind: MirTerminatorKind) -> MirTerminator {
    MirTerminator {
        kind,
        source: Some(source()),
    }
}

fn source() -> MirSourceLocation {
    MirSourceLocation {
        file: "tests/control_flow.rs".to_owned(),
        line: 1,
        column: 1,
    }
}
