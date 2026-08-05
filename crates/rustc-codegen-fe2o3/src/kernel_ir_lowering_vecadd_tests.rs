use super::*;
use crate::mir_import::{MirImportedType, MirLocal, MirLocalRole, MirSwitchTarget};
use crate::record_lowering::plan_from_records;
use dialect_mir::{MirOp, MirType};

#[test]
fn vecadd_fixture_translates_to_verified_typed_cfg() {
    let translated = translate_and_verify(&vecadd_fixture()).expect("vecadd should translate");
    verify_module(&translated).expect("translated vecadd should verify");

    assert_eq!(translated.kernels.len(), 1);
    assert_eq!(translated.functions.len(), 4);
    let function = translated
        .functions
        .iter()
        .find(|function| function.id.as_str() == "fe2o3_vecadd::fe2o3_kernel_vecadd")
        .expect("kernel definition");
    assert_eq!(
        function.signature.parameters,
        vec![
            global_slice(AccessMode::ReadOnly),
            global_slice(AccessMode::ReadOnly),
            global_slice(AccessMode::ReadWrite),
        ]
    );

    let body = function.body.as_ref().expect("kernel body");
    assert_eq!(body.blocks.len(), 11, "ten MIR blocks plus assert trap");
    let operations = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::Call { .. }
        )),
        4
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::SliceLength { .. }
        )),
        2
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::SliceData { .. }
        )),
        2
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::GetElementPointer { .. }
        )),
        2
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::Load { .. }
        )),
        2
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::Store { .. }
        )),
        1
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        )),
        1
    );
    assert_eq!(
        count_ops(&operations, |kind| matches!(
            kind,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                ..
            }
        )),
        2
    );

    let callees = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, .. } => Some(callee.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        callees,
        vec![
            "fe2o3_device::thread::index_1d",
            "fe2o3_device::DisjointSlice::<f32>::get_mut",
            "fe2o3_device::ThreadIndex::get",
            "fe2o3_device::ThreadIndex::get",
        ]
    );
    assert_eq!(
        body.blocks
            .iter()
            .filter(|block| matches!(block.terminator, Some(Terminator::ConditionalBranch { .. })))
            .count(),
        2
    );
    assert!(matches!(
        body.blocks[2].terminator,
        Some(Terminator::Switch { .. })
    ));
    assert!(matches!(
        body.blocks
            .last()
            .and_then(|block| block.terminator.as_ref()),
        Some(Terminator::Unreachable)
    ));
}

#[test]
fn vecadd_translation_is_deterministic() {
    let fixture = vecadd_fixture();
    let first = translate_and_verify(&fixture).expect("first translation");
    let second = translate_and_verify(&fixture).expect("second translation");

    assert_eq!(first, second);
    assert_eq!(format!("{first:#?}"), format!("{second:#?}"));
}

#[test]
fn malformed_vecadd_fixture_reports_its_mir_block() {
    let mut fixture = vecadd_fixture();
    fixture.functions[0].blocks[8].terminator = None;

    let errors = translate_and_verify(&fixture).expect_err("missing terminator must fail");

    assert!(errors.contains(TranslationDiagnosticCode::MalformedMir));
    assert_eq!(errors.diagnostics()[0].location.block, Some(8));
    assert!(errors.diagnostics()[0].message.contains("no terminator"));
}

#[test]
fn unsupported_rvalue_reports_source_location() {
    let mut fixture = vecadd_fixture();
    let statement = &mut fixture.functions[0].blocks[7].statements[1];
    statement.rvalue = Some(MirRvalueKind::Repeat);
    statement.operation = Some("repeat".to_string());

    let errors = translate_and_verify(&fixture).expect_err("repeat must be rejected");

    assert!(errors.contains(TranslationDiagnosticCode::UnsupportedRvalue));
    let diagnostic = &errors.diagnostics()[0];
    assert_eq!(diagnostic.location.block, Some(7));
    assert_eq!(diagnostic.location.statement, Some(1));
    assert_eq!(
        diagnostic
            .location
            .source
            .as_ref()
            .map(|source| source.file.as_str()),
        Some("examples/vecadd/src/main.rs")
    );
}

#[test]
fn vecadd_fixture_still_populates_the_legacy_record_plan() {
    let fixture = vecadd_fixture();
    let plan = plan_from_records(&fixture.dialect_records());
    let function = plan.function("vecadd").expect("legacy vecadd plan");

    assert_eq!(function.op_count(MirOp::Call), 4);
    assert_eq!(function.op_count(MirOp::Load), 2);
    assert_eq!(function.op_count(MirOp::Store), 1);
    assert_eq!(
        function.ops_by(MirOp::Store)[0].operation.as_deref(),
        Some("add")
    );
    assert_eq!(function.op_count(MirOp::Lt), 2);
    assert_eq!(function.op_count(MirOp::Assert), 2);
    assert_eq!(function.op_count(MirOp::Switch), 1);
}

fn count_ops(operations: &[&Operation], predicate: impl Fn(&OperationKind) -> bool) -> usize {
    operations
        .iter()
        .filter(|operation| predicate(&operation.kind))
        .count()
}

fn global_slice(access: AccessMode) -> Type {
    Type::slice(Type::F32, AddressSpace::Global, access)
}

fn vecadd_fixture() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            export_name: "vecadd".to_string(),
            rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
            kind: MirFunctionKind::Kernel,
            arg_count: 3,
            local_count: 17,
            locals: vec![
                local(
                    0,
                    MirLocalRole::Return,
                    MirType::Unit,
                    "()",
                    MirTypeShape::Unit,
                ),
                local(1, MirLocalRole::Arg, MirType::Slice, "&[f32]", slice(false)),
                local(2, MirLocalRole::Arg, MirType::Slice, "&[f32]", slice(false)),
                local(
                    3,
                    MirLocalRole::Arg,
                    MirType::DisjointSlice,
                    "fe2o3_device::DisjointSlice<f32>",
                    MirTypeShape::DisjointSlice {
                        element: Box::new(MirTypeShape::F32),
                    },
                ),
                local(
                    4,
                    MirLocalRole::Temp,
                    MirType::Unknown,
                    "fe2o3_device::ThreadIndex",
                    adt("fe2o3_device::ThreadIndex"),
                ),
                local(
                    5,
                    MirLocalRole::Temp,
                    MirType::Unknown,
                    "Option<&mut f32>",
                    adt("std::option::Option"),
                ),
                local(
                    6,
                    MirLocalRole::Temp,
                    MirType::Ptr,
                    "&mut fe2o3_device::DisjointSlice<f32>",
                    MirTypeShape::Reference {
                        pointee: Box::new(MirTypeShape::DisjointSlice {
                            element: Box::new(MirTypeShape::F32),
                        }),
                        mutable: true,
                    },
                ),
                local(
                    7,
                    MirLocalRole::Temp,
                    MirType::Unknown,
                    "isize",
                    MirTypeShape::ISize,
                ),
                local(
                    8,
                    MirLocalRole::Temp,
                    MirType::Ptr,
                    "&mut f32",
                    MirTypeShape::Reference {
                        pointee: Box::new(MirTypeShape::F32),
                        mutable: true,
                    },
                ),
                local(
                    9,
                    MirLocalRole::Temp,
                    MirType::F32,
                    "f32",
                    MirTypeShape::F32,
                ),
                local(
                    10,
                    MirLocalRole::Temp,
                    MirType::USize,
                    "usize",
                    MirTypeShape::USize,
                ),
                local(
                    11,
                    MirLocalRole::Temp,
                    MirType::USize,
                    "usize",
                    MirTypeShape::USize,
                ),
                local(
                    12,
                    MirLocalRole::Temp,
                    MirType::I1,
                    "bool",
                    MirTypeShape::Bool,
                ),
                local(
                    13,
                    MirLocalRole::Temp,
                    MirType::F32,
                    "f32",
                    MirTypeShape::F32,
                ),
                local(
                    14,
                    MirLocalRole::Temp,
                    MirType::USize,
                    "usize",
                    MirTypeShape::USize,
                ),
                local(
                    15,
                    MirLocalRole::Temp,
                    MirType::USize,
                    "usize",
                    MirTypeShape::USize,
                ),
                local(
                    16,
                    MirLocalRole::Temp,
                    MirType::I1,
                    "bool",
                    MirTypeShape::Bool,
                ),
            ],
            blocks: vec![
                block(0, vec![], call(index_1d(), vec![], 4, 1)),
                block(
                    1,
                    vec![assign(
                        0,
                        place(6),
                        vec![operand(3)],
                        MirRvalueKind::Ref,
                        "ref",
                    )],
                    call(get_mut(), vec![operand(6), operand(4)], 5, 2),
                ),
                block(
                    2,
                    vec![assign(
                        0,
                        place(7),
                        vec![operand(5)],
                        MirRvalueKind::Discriminant,
                        "discriminant",
                    )],
                    MirTerminatorKind::SwitchInt {
                        discriminant: operand(7),
                        targets: vec![
                            MirSwitchTarget {
                                value: 1,
                                target: 3,
                            },
                            MirSwitchTarget {
                                value: 0,
                                target: 8,
                            },
                        ],
                        otherwise: 9,
                    },
                ),
                block(
                    3,
                    vec![assign(
                        0,
                        place(8),
                        vec![MirOperandRef::Place(MirPlaceRef {
                            local: 5,
                            projection: vec![
                                MirProjectionElem::Downcast { variant: 1 },
                                MirProjectionElem::Field(0),
                            ],
                        })],
                        MirRvalueKind::Use,
                        "use",
                    )],
                    call(thread_get(), vec![operand(4)], 10, 4),
                ),
                block(
                    4,
                    vec![
                        assign(
                            0,
                            place(11),
                            vec![operand(1)],
                            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                            "ptr_metadata",
                        ),
                        assign(
                            1,
                            place(12),
                            vec![operand(10), operand(11)],
                            MirRvalueKind::Binary(MirBinaryOp::Lt),
                            "lt",
                        ),
                    ],
                    MirTerminatorKind::Assert {
                        condition: operand(12),
                        expected: true,
                        target: 5,
                    },
                ),
                block(
                    5,
                    vec![assign(
                        0,
                        place(9),
                        vec![indexed_operand(1, 10)],
                        MirRvalueKind::Use,
                        "use",
                    )],
                    call(thread_get(), vec![operand(4)], 14, 6),
                ),
                block(
                    6,
                    vec![
                        assign(
                            0,
                            place(15),
                            vec![operand(2)],
                            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                            "ptr_metadata",
                        ),
                        assign(
                            1,
                            place(16),
                            vec![operand(14), operand(15)],
                            MirRvalueKind::Binary(MirBinaryOp::Lt),
                            "lt",
                        ),
                    ],
                    MirTerminatorKind::Assert {
                        condition: operand(16),
                        expected: true,
                        target: 7,
                    },
                ),
                block(
                    7,
                    vec![
                        assign(
                            0,
                            place(13),
                            vec![indexed_operand(2, 14)],
                            MirRvalueKind::Use,
                            "use",
                        ),
                        assign(
                            1,
                            MirPlaceRef {
                                local: 8,
                                projection: vec![MirProjectionElem::Deref],
                            },
                            vec![operand(9), operand(13)],
                            MirRvalueKind::Binary(MirBinaryOp::Add),
                            "add",
                        ),
                    ],
                    MirTerminatorKind::Goto { target: 8 },
                ),
                block(8, vec![], MirTerminatorKind::Return),
                block(9, vec![], MirTerminatorKind::Unreachable),
            ],
        }],
    }
}

fn local(
    index: usize,
    role: MirLocalRole,
    kind: MirType,
    rust: &str,
    shape: MirTypeShape,
) -> MirLocal {
    MirLocal {
        index,
        role,
        ty: MirImportedType {
            kind,
            rust: rust.to_string(),
            shape,
        },
    }
}

fn slice(mutable: bool) -> MirTypeShape {
    MirTypeShape::Slice {
        element: Box::new(MirTypeShape::F32),
        mutable,
    }
}

fn adt(identity: &str) -> MirTypeShape {
    MirTypeShape::Adt {
        identity: identity.to_string(),
    }
}

fn block(index: usize, statements: Vec<MirStatement>, kind: MirTerminatorKind) -> MirBlock {
    MirBlock {
        index,
        statements,
        terminator: Some(MirTerminator {
            kind,
            source: Some(source(index + 6, 5)),
        }),
    }
}

fn assign(
    index: usize,
    destination: MirPlaceRef,
    operands: Vec<MirOperandRef>,
    rvalue: MirRvalueKind,
    operation: &str,
) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(destination),
        operands,
        rvalue: Some(rvalue),
        operation: Some(operation.to_string()),
        source: Some(source(index + 6, 9)),
    }
}

fn call(
    callee: MirCallee,
    operands: Vec<MirOperandRef>,
    destination: usize,
    target: usize,
) -> MirTerminatorKind {
    MirTerminatorKind::Call {
        callee: Some(callee),
        target: Some(target),
        destination: Some(place(destination)),
        operands,
    }
}

fn index_1d() -> MirCallee {
    MirCallee {
        identity: "fe2o3_device::thread::index_1d".to_string(),
        kind: MirKnownCall::ThreadIndex1d,
    }
}

fn thread_get() -> MirCallee {
    MirCallee {
        identity: "fe2o3_device::ThreadIndex::get".to_string(),
        kind: MirKnownCall::ThreadIndexGet,
    }
}

fn get_mut() -> MirCallee {
    MirCallee {
        identity: "fe2o3_device::DisjointSlice::<f32>::get_mut".to_string(),
        kind: MirKnownCall::DisjointSliceGetMut,
    }
}

fn place(local: usize) -> MirPlaceRef {
    MirPlaceRef {
        local,
        projection: Vec::new(),
    }
}

fn operand(local: usize) -> MirOperandRef {
    MirOperandRef::Place(place(local))
}

fn indexed_operand(local: usize, index: usize) -> MirOperandRef {
    MirOperandRef::Place(MirPlaceRef {
        local,
        projection: vec![
            MirProjectionElem::Deref,
            MirProjectionElem::Index { local: index },
        ],
    })
}

fn source(line: usize, column: usize) -> MirSourceLocation {
    MirSourceLocation {
        file: "examples/vecadd/src/main.rs".to_string(),
        line,
        column,
    }
}
