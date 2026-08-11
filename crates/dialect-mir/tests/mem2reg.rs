use dialect_mir::{
    MAX_MEM2REG_OUTPUT_ITEMS, MirAddressSpace, MirAuthorizedDeviceImport, MirBasicBlock,
    MirBinaryOp, MirBlockId, MirBody, MirBodyForm, MirCall, MirCallAuthority, MirCallReturn,
    MirCallSignature, MirCallable, MirCallee, MirConstant, MirConstantValue, MirEdge,
    MirExecutableModule, MirExecutableTarget, MirExecutableVersion, MirExternalCallRegistry,
    MirExternalCallReturn, MirExternalCallSignature, MirFunction, MirLayout, MirLocalDecl,
    MirLocalId, MirLocalKind, MirOperand, MirPlace, MirRvalue, MirScalarType, MirSemanticType,
    MirStatement, MirStatementKind, MirTerminator, MirTerminatorKind, MirTypeId, MirTypeKind,
    MirUnwindAction, MirValueId, promote_module_to_ssa, promote_module_to_ssa_with_registry,
};

#[derive(Clone, Copy)]
struct Types {
    bool_ty: MirTypeId,
    u32_ty: MirTypeId,
}

fn types() -> (Vec<MirSemanticType>, Types) {
    let bool_ty = MirSemanticType {
        layout: MirLayout::sized(1, 1),
        kind: MirTypeKind::Scalar(MirScalarType::Bool),
    };
    let u32_ty = MirSemanticType {
        layout: MirLayout::sized(4, 4),
        kind: MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
    };
    let mut table = vec![bool_ty.clone(), u32_ty.clone()];
    table.sort_by_key(|ty| ty.canonical_text().unwrap());
    let ids = Types {
        bool_ty: MirTypeId(table.iter().position(|ty| ty == &bool_ty).unwrap() as u32),
        u32_ty: MirTypeId(table.iter().position(|ty| ty == &u32_ty).unwrap() as u32),
    };
    (table, ids)
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

fn place(local: u32, ty: MirTypeId) -> MirPlace {
    MirPlace::local(MirLocalId(local), ty)
}

fn copy(local: u32, ty: MirTypeId) -> MirOperand {
    MirOperand::Copy(place(local, ty))
}

fn integer(value: u128, ty: MirTypeId) -> MirOperand {
    MirOperand::Constant(MirConstant {
        ty,
        value: MirConstantValue::Integer(value),
    })
}

fn assign(local: u32, ty: MirTypeId, value: MirRvalue) -> MirStatement {
    MirStatement {
        kind: MirStatementKind::Assign {
            place: place(local, ty),
            value,
        },
        span: None,
    }
}

fn terminator(kind: MirTerminatorKind) -> MirTerminator {
    MirTerminator { kind, span: None }
}

fn loop_module() -> MirExecutableModule {
    let (types, ids) = types();
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget::gfx942(),
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "mem2reg::sum::<u32>".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals: vec![
                    local(ids.u32_ty, MirLocalKind::Return, true),
                    local(ids.u32_ty, MirLocalKind::Argument, false),
                    local(ids.u32_ty, MirLocalKind::Temporary, true),
                    local(ids.u32_ty, MirLocalKind::Temporary, true),
                    local(ids.bool_ty, MirLocalKind::Temporary, true),
                ],
                blocks: vec![
                    MirBasicBlock {
                        parameters: vec![],
                        statements: vec![
                            assign(2, ids.u32_ty, MirRvalue::Use(integer(0, ids.u32_ty))),
                            assign(3, ids.u32_ty, MirRvalue::Use(integer(0, ids.u32_ty))),
                        ],
                        terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(
                            1,
                        )))),
                    },
                    MirBasicBlock {
                        parameters: vec![],
                        statements: vec![assign(
                            4,
                            ids.bool_ty,
                            MirRvalue::BinaryOp {
                                op: MirBinaryOp::Lt,
                                lhs: copy(2, ids.u32_ty),
                                rhs: copy(1, ids.u32_ty),
                            },
                        )],
                        terminator: terminator(MirTerminatorKind::SwitchInt {
                            discr: copy(4, ids.bool_ty),
                            targets: vec![(1, MirEdge::new(MirBlockId(2)))],
                            otherwise: MirEdge::new(MirBlockId(3)),
                        }),
                    },
                    MirBasicBlock {
                        parameters: vec![],
                        statements: vec![
                            assign(
                                3,
                                ids.u32_ty,
                                MirRvalue::BinaryOp {
                                    op: MirBinaryOp::Add,
                                    lhs: copy(3, ids.u32_ty),
                                    rhs: copy(2, ids.u32_ty),
                                },
                            ),
                            assign(
                                2,
                                ids.u32_ty,
                                MirRvalue::BinaryOp {
                                    op: MirBinaryOp::Add,
                                    lhs: copy(2, ids.u32_ty),
                                    rhs: integer(1, ids.u32_ty),
                                },
                            ),
                        ],
                        terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(
                            1,
                        )))),
                    },
                    MirBasicBlock {
                        parameters: vec![],
                        statements: vec![assign(
                            0,
                            ids.u32_ty,
                            MirRvalue::Use(copy(3, ids.u32_ty)),
                        )],
                        terminator: terminator(MirTerminatorKind::Return),
                    },
                ],
                entry: MirBlockId(0),
            },
        }],
    }
}

fn value(operand: &MirOperand) -> MirValueId {
    let MirOperand::Value(value) = operand else {
        panic!("expected an SSA value, got {operand:?}");
    };
    *value
}

#[test]
fn promotes_loop_carried_values_through_explicit_backedge_arguments() {
    let input = loop_module();
    let input = input.validate().unwrap();
    let (output, report) = promote_module_to_ssa(&input).unwrap();
    output.validate().unwrap();

    assert_eq!(report.promoted_local_count(), 3);
    assert_eq!(
        report.functions[0].promoted_locals,
        vec![MirLocalId(1), MirLocalId(2), MirLocalId(3),]
    );
    assert_eq!(report.inserted_parameter_count(), 3);
    assert_eq!(report.inserted_definition_count(), 4);

    let body = &output.functions[0].body;
    assert!(matches!(body.form, MirBodyForm::Ssa { .. }));
    assert_eq!(body.blocks[0].parameters.len(), 1);
    assert_eq!(body.blocks[1].parameters.len(), 2);
    assert!(body.blocks[2].parameters.is_empty());
    assert!(body.blocks[3].parameters.is_empty());

    let MirTerminatorKind::Goto(entry_to_header) = &body.blocks[0].terminator.kind else {
        unreachable!();
    };
    assert_eq!(
        entry_to_header
            .arguments
            .iter()
            .map(value)
            .collect::<Vec<_>>(),
        vec![MirValueId(1), MirValueId(2)]
    );

    let MirTerminatorKind::Goto(backedge) = &body.blocks[2].terminator.kind else {
        unreachable!();
    };
    assert_eq!(
        backedge.arguments.iter().map(value).collect::<Vec<_>>(),
        vec![MirValueId(6), MirValueId(5)]
    );
    assert_eq!(
        body.blocks[1]
            .parameters
            .iter()
            .map(|parameter| parameter.origin.unwrap())
            .collect::<Vec<_>>(),
        vec![MirLocalId(2), MirLocalId(3)]
    );

    let encoded = output.to_bytes().unwrap();
    assert_eq!(MirExecutableModule::from_bytes(&encoded).unwrap(), output);
}

#[test]
fn leaves_storage_marked_and_not_entry_initialized_locals_as_slots() {
    let mut storage_marked = loop_module();
    storage_marked.functions[0].body.blocks[0]
        .statements
        .insert(
            0,
            MirStatement {
                kind: MirStatementKind::StorageLive(MirLocalId(2)),
                span: None,
            },
        );
    let storage_marked = storage_marked.validate().unwrap();
    let (_, report) = promote_module_to_ssa(&storage_marked).unwrap();
    assert_eq!(
        report.functions[0].promoted_locals,
        vec![MirLocalId(1), MirLocalId(3)]
    );

    let mut late_initialized = loop_module();
    late_initialized.functions[0].body.blocks[0]
        .statements
        .remove(0);
    let late_u32 = late_initialized.functions[0].body.locals[2].ty;
    late_initialized.functions[0].body.blocks[1]
        .statements
        .insert(0, assign(2, late_u32, MirRvalue::Use(integer(0, late_u32))));
    let late_initialized = late_initialized.validate().unwrap();
    let (output, report) = promote_module_to_ssa(&late_initialized).unwrap();
    assert_eq!(
        report.functions[0].promoted_locals,
        vec![MirLocalId(1), MirLocalId(3)]
    );
    assert!(output.functions[0].body.blocks.iter().any(|block| {
        block.statements.iter().any(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::Assign { place, .. } if place.local == MirLocalId(2)
            )
        })
    }));
}

#[test]
fn rejects_repromotion_and_invalid_input_without_partial_output() {
    let input = loop_module().validate().unwrap();
    let (ssa, _) = promote_module_to_ssa(&input).unwrap();
    let error = promote_module_to_ssa(&ssa).unwrap_err();
    assert!(error.reason().contains("only place-form"));

    let mut invalid = loop_module();
    invalid.functions[0].body.blocks[0].terminator =
        terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(99))));
    let error = promote_module_to_ssa_with_registry(&invalid, &MirExternalCallRegistry::default())
        .unwrap_err();
    assert!(error.reason().contains("does not exist"));
}

#[test]
fn validated_input_is_owned_and_transform_output_stays_validated() {
    let mut untrusted = loop_module();
    let validated = untrusted.validate().unwrap();
    untrusted.functions[0].body.blocks[0].terminator =
        terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(99))));
    assert!(untrusted.validate().is_err());

    let (output, _) = promote_module_to_ssa(&validated).unwrap();
    output.as_module().validate().unwrap();
    let mut recovered_data = output.into_unvalidated();
    recovered_data.functions[0].body.blocks[0].terminator =
        terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(99))));
    assert!(recovered_data.validate().is_err());
}

#[test]
fn leaves_call_defined_locals_as_slots() {
    let mut input = loop_module();
    input.functions[0].body.blocks[0].statements.remove(0);
    let ids = Types {
        bool_ty: input.functions[0].body.locals[4].ty,
        u32_ty: input.functions[0].body.locals[2].ty,
    };
    let semantic_u32 = input.type_at(ids.u32_ty).unwrap().clone();
    input.functions[0].body.blocks[0].terminator = terminator(MirTerminatorKind::Call(MirCall {
        callee: MirCallee::Direct("fixture::next".into()),
        arguments: vec![],
        destination: Some(place(2, ids.u32_ty)),
        target: Some(MirEdge::new(MirBlockId(1))),
        unwind: MirUnwindAction::Unreachable,
    }));
    input.callables.push(MirCallable {
        identity: "fixture::next".into(),
        authority: MirCallAuthority::DeviceImport {
            contract: "fixture::next::contract".into(),
        },
        signature: MirCallSignature {
            inputs: vec![],
            output: MirCallReturn::Value(ids.u32_ty),
            can_unwind: false,
        },
    });
    let registry = MirExternalCallRegistry::try_new(vec![MirAuthorizedDeviceImport {
        identity: "fixture::next".into(),
        contract: "fixture::next::contract".into(),
        signature: MirExternalCallSignature {
            inputs: vec![],
            output: MirExternalCallReturn::Value(semantic_u32),
            can_unwind: false,
        },
    }])
    .unwrap();
    let input = input.validate_with_registry(&registry).unwrap();
    let (output, report) = promote_module_to_ssa(&input).unwrap();
    assert_eq!(
        report.functions[0].promoted_locals,
        vec![MirLocalId(1), MirLocalId(3)]
    );
    let MirTerminatorKind::Call(call) = &output.functions[0].body.blocks[0].terminator.kind else {
        unreachable!();
    };
    assert_eq!(call.destination.as_ref().unwrap().local, MirLocalId(2));
    assert_eq!(call.target.as_ref().unwrap().arguments.len(), 1);
}

#[test]
fn linear_control_flow_does_not_amplify_block_arguments() {
    let (types, ids) = types();
    let argument_count = 256_u32;
    let block_count = 257_u32;
    let mut locals = vec![local(ids.u32_ty, MirLocalKind::Return, true)];
    locals.extend((0..argument_count).map(|_| local(ids.u32_ty, MirLocalKind::Argument, false)));
    let statements = (1..=argument_count)
        .map(|argument| assign(0, ids.u32_ty, MirRvalue::Use(copy(argument, ids.u32_ty))))
        .collect::<Vec<_>>();
    let blocks = (0..block_count)
        .map(|block| MirBasicBlock {
            parameters: vec![],
            statements: if block == 0 {
                statements.clone()
            } else {
                vec![]
            },
            terminator: if block + 1 == block_count {
                terminator(MirTerminatorKind::Return)
            } else {
                terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(block + 1))))
            },
        })
        .collect();
    let input = MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget::gfx942(),
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "mem2reg::amplification".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals,
                blocks,
                entry: MirBlockId(0),
            },
        }],
    };

    let input = input.validate().unwrap();
    let (output, report) = promote_module_to_ssa(&input).unwrap();
    assert_eq!(report.inserted_parameter_count(), argument_count as usize);
    assert!(
        output.functions[0].body.blocks[1..]
            .iter()
            .all(|block| block.parameters.is_empty())
    );
}

#[test]
fn rejects_repeated_join_amplification_before_transforming() {
    let (types, ids) = types();
    let local_count = 256_u32;
    let diamond_count = 90_u32;
    let mut locals = vec![local(ids.u32_ty, MirLocalKind::Return, true)];
    locals.extend((0..local_count).map(|_| local(ids.u32_ty, MirLocalKind::Temporary, true)));

    let initialize = (1..=local_count)
        .map(|local| assign(local, ids.u32_ty, MirRvalue::Use(integer(0, ids.u32_ty))))
        .collect::<Vec<_>>();
    let update = || {
        (1..=local_count)
            .map(|local| {
                assign(
                    local,
                    ids.u32_ty,
                    MirRvalue::BinaryOp {
                        op: MirBinaryOp::Add,
                        lhs: copy(local, ids.u32_ty),
                        rhs: integer(1, ids.u32_ty),
                    },
                )
            })
            .collect::<Vec<_>>()
    };

    let mut blocks = vec![MirBasicBlock {
        parameters: vec![],
        statements: initialize,
        terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(1)))),
    }];
    for diamond in 0..diamond_count {
        let decision = 1 + diamond * 4;
        let left = decision + 1;
        let right = decision + 2;
        let join = decision + 3;
        blocks.push(MirBasicBlock {
            parameters: vec![],
            statements: vec![],
            terminator: terminator(MirTerminatorKind::SwitchInt {
                discr: MirOperand::Constant(MirConstant {
                    ty: ids.bool_ty,
                    value: MirConstantValue::Bool(true),
                }),
                targets: vec![(1, MirEdge::new(MirBlockId(left)))],
                otherwise: MirEdge::new(MirBlockId(right)),
            }),
        });
        for _ in 0..2 {
            blocks.push(MirBasicBlock {
                parameters: vec![],
                statements: update(),
                terminator: terminator(MirTerminatorKind::Goto(MirEdge::new(MirBlockId(join)))),
            });
        }
        let terminator_kind = if diamond + 1 == diamond_count {
            MirTerminatorKind::Return
        } else {
            MirTerminatorKind::Goto(MirEdge::new(MirBlockId(join + 1)))
        };
        let statements = if diamond + 1 == diamond_count {
            vec![assign(0, ids.u32_ty, MirRvalue::Use(copy(1, ids.u32_ty)))]
        } else {
            vec![]
        };
        blocks.push(MirBasicBlock {
            parameters: vec![],
            statements,
            terminator: terminator(terminator_kind),
        });
    }

    let input = MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget::gfx942(),
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: "mem2reg::join_amplification".into(),
            span: None,
            body: MirBody {
                form: MirBodyForm::Places,
                locals,
                blocks,
                entry: MirBlockId(0),
            },
        }],
    };

    let input = input.validate().unwrap();
    let error = promote_module_to_ssa(&input).unwrap_err();
    assert!(error.reason().contains("generated items"));
    assert!(
        error
            .reason()
            .contains(&MAX_MEM2REG_OUTPUT_ITEMS.to_string())
    );
}
