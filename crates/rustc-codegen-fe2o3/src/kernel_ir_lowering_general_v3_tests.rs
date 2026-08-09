use super::*;
use crate::mir_import::{
    MirImportedType, MirLocal, MirLocalRole, MirPlaceRef, MirProjectionElem, MirStatement,
    MirSwitchTarget,
};
use dialect_mir::MirType;

#[test]
fn exact_alpha_and_zeta_bodies_lower_together() {
    let module = lower_general_v3(&alpha_zeta_module()).expect("alpha/zeta module");

    assert_eq!(
        module
            .kernels
            .iter()
            .map(|kernel| kernel.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert_eq!(
        module.functions.len(),
        2,
        "trusted helpers must be semantic operations"
    );
    assert!(
        module
            .functions
            .iter()
            .all(|function| function.body.is_some())
    );
    assert!(module.functions.iter().all(|function| {
        !function.id.as_str().contains("DisjointSlice::<T>")
            && !function.id.as_str().contains("ThreadIndex::get")
    }));

    let alpha = function(&module, "tests::alpha");
    assert_eq!(
        alpha.signature.parameters,
        vec![
            Type::F32,
            slice(AccessMode::ReadOnly),
            slice(AccessMode::ReadWrite),
        ]
    );
    let zeta = function(&module, "tests::zeta");
    assert_eq!(
        zeta.signature.parameters,
        vec![
            slice(AccessMode::ReadOnly),
            slice(AccessMode::ReadOnly),
            Type::F32,
            slice(AccessMode::ReadWrite),
        ]
    );
}

#[test]
fn s09_alpha_requires_exact_guarded_cfg_and_dataflow() {
    let exact = s09_alpha();
    let authenticated_path = exact.rust_path.clone();
    crate::source_debug::validate_alpha_mir_body(&exact, &authenticated_path)
        .expect("exact S09 alpha MIR");

    // The generated symbol is bound to the integrated Cargo dependency graph. The isolated
    // S09 branch identity must not remain valid after that graph is merged.
    let mut isolated_graph_identity = exact.clone();
    isolated_graph_identity.rust_path =
        "fe2o3_typed_alias_spoof::general_genuine::__fe2o3_host_kernel_v1_stale_isolated_graph"
            .to_owned();
    assert!(
        crate::source_debug::validate_alpha_mir_body(
            &isolated_graph_identity,
            &authenticated_path,
        )
        .is_err(),
        "isolated-graph S09 DefPath identity was admitted after integration"
    );

    let mut disconnected_guard = exact.clone();
    let MirTerminatorKind::Assert { condition, .. } = &mut disconnected_guard.blocks[4]
        .terminator
        .as_mut()
        .expect("guard terminator")
        .kind
    else {
        panic!("S09 guard assert")
    };
    *condition = operand(5);
    assert!(
        crate::source_debug::validate_alpha_mir_body(&disconnected_guard, &authenticated_path)
            .is_err()
    );

    let mut wrong_store = exact.clone();
    wrong_store.blocks[5].statements[1].destination = Some(MirPlaceRef {
        local: 8,
        projection: vec![MirProjectionElem::Deref],
    });
    assert!(
        crate::source_debug::validate_alpha_mir_body(&wrong_store, &authenticated_path).is_err()
    );

    let mut alternate_output = exact.clone();
    let MirTerminatorKind::Call { operands, .. } = &mut alternate_output.blocks[2]
        .terminator
        .as_mut()
        .expect("get-mut terminator")
        .kind
    else {
        panic!("S09 get-mut call")
    };
    operands[0] = operand(3);
    assert!(
        crate::source_debug::validate_alpha_mir_body(&alternate_output, &authenticated_path)
            .is_err()
    );
}

#[test]
fn alpha_zeta_share_semantic_helper_lowering_and_emit_fmul_fadd() {
    let module = lower_general_v3(&alpha_zeta_module()).expect("alpha/zeta module");
    for rust_path in ["tests::alpha", "tests::zeta"] {
        let operations = operations(function(&module, rust_path));
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Intrinsic(_)))
                .count(),
            1,
            "{rust_path}"
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .count(),
            0,
            "{rust_path}"
        );
        assert!(operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                ..
            }
        )));
    }

    let llvm = dialect_amdgcn::lower_compiler_module_to_gfx942_llvm_ir(&module)
        .expect("gfx942 compiler-module LLVM");
    assert!(llvm.contains("fmul float"), "{llvm}");
    assert_eq!(llvm.matches("fadd float").count(), 2, "{llvm}");
    assert!(!llvm.contains("DisjointSlice::<T>::get_mut"), "{llvm}");
}

#[test]
fn alpha_multiply_may_write_the_guarded_payload_directly() {
    let mut alpha = alpha();
    let arithmetic = &mut alpha.blocks[4].statements;
    arithmetic[1].destination = Some(MirPlaceRef {
        local: 8,
        projection: vec![MirProjectionElem::Deref],
    });
    arithmetic.remove(2);

    let module = lower_general_v3(&MirModule {
        functions: vec![alpha],
    })
    .expect("direct guarded f32 store");
    let operations = operations(function(&module, "tests::alpha"));
    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::Binary {
            op: BinaryOp::Multiply,
            ..
        }
    )));
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
    );
}

#[test]
fn general_v3_multiply_claim_requires_imported_f32_operands() {
    let mut module = alpha_zeta_module();
    let loaded = module.functions[0]
        .locals
        .iter_mut()
        .find(|local| local.index == 10)
        .expect("alpha loaded value");
    loaded.ty = imported(MirTypeShape::U32);

    let errors = lower_general_v3(&module).expect_err("non-f32 multiply must remain unowned");

    assert!(errors.contains(TranslationDiagnosticCode::UnsupportedRvalue));
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("f32 multiply requires an exact General V3 alpha/zeta kernel context")
    }));
}

#[test]
fn gfx942_target_id_feature_states_preserve_the_float_profile() {
    translate_and_verify_for_target(&alpha_zeta_module(), &AmdGpuTarget::new("gfx942:xnack-"))
        .expect("canonical gfx942 target IDs must retain the gfx942 floating-point profile");
}

#[test]
fn general_v3_rejects_wrong_index_untrusted_callee_and_wrong_profile() {
    let mut wrong_index = alpha_zeta_module();
    let get_mut = &mut wrong_index.functions[0].blocks[1]
        .terminator
        .as_mut()
        .expect("get_mut terminator")
        .kind;
    let MirTerminatorKind::Call { operands, .. } = get_mut else {
        panic!("get_mut call")
    };
    operands[1] = usize_constant(0);
    let errors = lower_general_v3(&wrong_index).expect_err("untrusted index provenance");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("did not originate from trusted thread::index_1d")
    }));

    let mut lookalike = alpha_zeta_module();
    let get_mut = &mut lookalike.functions[0].blocks[1]
        .terminator
        .as_mut()
        .expect("get_mut terminator")
        .kind;
    let MirTerminatorKind::Call { callee, .. } = get_mut else {
        panic!("get_mut call")
    };
    *callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::DisjointSliceGetMut.canonical_path(),
    ));
    let errors = lower_general_v3(&lookalike).expect_err("callee spelling is not authority");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no classified trusted device identity")
    }));

    let mut renamed = alpha_zeta_module();
    renamed.functions[0].export_name = "alpha_lookalike".to_string();
    let errors = lower_general_v3(&renamed).expect_err("kernel name is part of the exact slice");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact General V3 alpha/zeta kernel context")
    }));

    let mut untyped = alpha_zeta_module();
    for function in &mut untyped.functions {
        function.typed_profile = None;
    }
    let errors = translate_and_verify_for_target(&untyped, &AmdGpuTarget::new("gfx942"))
        .expect_err("f32 multiply requires exact General V3 context");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an exact General V3 alpha/zeta kernel context")
    }));

    let errors =
        translate_and_verify_for_target(&alpha_zeta_module(), &AmdGpuTarget::new("gfx1100"))
            .expect_err("f32 arithmetic profile must be exact");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires the exact gfx942 floating-point profile")
    }));

    let zeta_only = MirModule {
        functions: vec![zeta()],
    };
    let errors = translate_and_verify_for_target(&zeta_only, &AmdGpuTarget::new("gfx1100"))
        .expect_err("zeta addition must independently require gfx942");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("f32 addition requires the exact gfx942 floating-point profile")
    }));
    let errors = translate_and_verify_with_float_target(
        &zeta_only,
        Some(Gfx942FloatTarget),
        StrictFloatPolicy::CustomLlvmPipeline,
    )
    .expect_err("zeta addition must independently reject custom LLVM pipelines");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects custom -Cllvm-args and -Cpasses")
    }));
}

#[test]
fn general_v3_thread_index_spelling_does_not_cross_the_extension_boundary() {
    let mut lookalike = MirModule {
        functions: vec![alpha()],
    };
    let index_call = &mut lookalike.functions[0].blocks[0]
        .terminator
        .as_mut()
        .expect("thread-index terminator")
        .kind;
    let MirTerminatorKind::Call { callee, .. } = index_call else {
        panic!("thread-index call")
    };
    *callee = Some(MirCallee::untrusted_for_test(
        TrustedDeviceItem::ThreadIndex1d.canonical_path(),
    ));

    let errors = lower_general_v3(&lookalike).expect_err("callee spelling is not authority");

    assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no classified trusted device identity")
    }));
}

#[test]
fn option_payload_cannot_escape_the_bounds_checked_some_region() {
    let mut false_to_store = MirModule {
        functions: vec![alpha()],
    };
    let switch = &mut false_to_store.functions[0].blocks[2]
        .terminator
        .as_mut()
        .expect("Option switch")
        .kind;
    let MirTerminatorKind::SwitchInt { targets, .. } = switch else {
        panic!("Option switch")
    };
    targets
        .iter_mut()
        .find(|target| target.value == 0)
        .expect("None edge")
        .target = 4;
    let errors = lower_general_v3(&false_to_store).expect_err("false edge reaches store");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Option payload alias escapes the bounds-checked Some region")
    }));

    let mut merged_edges = MirModule {
        functions: vec![alpha()],
    };
    let switch = &mut merged_edges.functions[0].blocks[2]
        .terminator
        .as_mut()
        .expect("Option switch")
        .kind;
    let MirTerminatorKind::SwitchInt { targets, .. } = switch else {
        panic!("Option switch")
    };
    let some_target = targets
        .iter()
        .find(|target| target.value == 1)
        .expect("Some edge")
        .target;
    targets
        .iter_mut()
        .find(|target| target.value == 0)
        .expect("None edge")
        .target = some_target;
    let errors = lower_general_v3(&merged_edges).expect_err("Some and None edges must differ");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boolean switch must have exact 0/1 cases")
    }));
}

fn lower_general_v3(module: &MirModule) -> Result<Module, TranslationErrors> {
    translate_and_verify_for_target(module, &AmdGpuTarget::new("gfx942"))
}

fn alpha_zeta_module() -> MirModule {
    MirModule {
        functions: vec![alpha(), zeta()],
    }
}

fn alpha() -> MirFunction {
    kernel(
        "alpha",
        vec![
            local(1, MirLocalRole::Arg, MirTypeShape::F32),
            local(2, MirLocalRole::Arg, slice_shape(false)),
            local(3, MirLocalRole::Arg, disjoint_shape()),
        ],
        4,
        vec![
            local(10, MirLocalRole::Temp, MirTypeShape::F32),
            local(11, MirLocalRole::Temp, MirTypeShape::F32),
        ],
        vec![
            assign(0, place(10), vec![indexed(2, 9)], MirRvalueKind::Use),
            assign(
                1,
                place(11),
                vec![operand(10), operand(1)],
                MirRvalueKind::Binary(MirBinaryOp::Mul),
            ),
            store(2, 8, 11),
        ],
    )
}

fn s09_alpha() -> MirFunction {
    let locals = vec![
        local(0, MirLocalRole::Return, MirTypeShape::Unit),
        local(1, MirLocalRole::Arg, MirTypeShape::F32),
        local(2, MirLocalRole::Arg, slice_shape(false)),
        local(3, MirLocalRole::Arg, disjoint_shape()),
        local(4, MirLocalRole::Temp, thread_index_shape()),
        local(5, MirLocalRole::Temp, MirTypeShape::USize),
        local(
            6,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(thread_index_shape()),
                mutable: false,
            },
        ),
        local(
            7,
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: "std::option::Option".to_owned(),
            },
        ),
        local(
            8,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(disjoint_shape()),
                mutable: true,
            },
        ),
        local(9, MirLocalRole::Temp, MirTypeShape::ISize),
        local(
            10,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::F32),
                mutable: true,
            },
        ),
        local(11, MirLocalRole::Temp, MirTypeShape::F32),
        local(12, MirLocalRole::Temp, MirTypeShape::USize),
        local(13, MirLocalRole::Temp, MirTypeShape::Bool),
    ];
    MirFunction {
        export_name: "alpha".to_owned(),
        rust_path: "fe2o3_typed_alias_spoof::general_genuine::__fe2o3_host_kernel_v1_2f5e34fba662b3a3fb8dc387a1c06a6214b61f23005c3155f8f5fb8954a28b67".to_owned(),
        kind: MirFunctionKind::KernelEntry,
        typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
        arg_count: 3,
        local_count: 14,
        locals,
        blocks: vec![
            block(
                0,
                vec![],
                call(TrustedDeviceItem::ThreadIndex1d, vec![], 4, 1),
            ),
            block(
                1,
                vec![assign(0, place(6), vec![operand(4)], MirRvalueKind::Ref)],
                call(TrustedDeviceItem::ThreadIndexGet, vec![operand(6)], 5, 2),
            ),
            block(
                2,
                vec![assign(0, place(8), vec![operand(3)], MirRvalueKind::Ref)],
                call(
                    TrustedDeviceItem::DisjointSliceGetMut,
                    vec![operand(8), operand(4)],
                    7,
                    3,
                ),
            ),
            block(
                3,
                vec![assign(
                    0,
                    place(9),
                    vec![operand(7)],
                    MirRvalueKind::Discriminant,
                )],
                MirTerminatorKind::SwitchInt {
                    discriminant: operand(9),
                    targets: vec![
                        MirSwitchTarget {
                            value: 1,
                            target: 4,
                        },
                        MirSwitchTarget {
                            value: 0,
                            target: 6,
                        },
                    ],
                    otherwise: 7,
                },
            ),
            block(
                4,
                vec![
                    assign(
                        0,
                        place(10),
                        vec![MirOperandRef::Place(MirPlaceRef {
                            local: 7,
                            projection: vec![
                                MirProjectionElem::Downcast { variant: 1 },
                                MirProjectionElem::Field(0),
                            ],
                        })],
                        MirRvalueKind::Use,
                    ),
                    assign(
                        1,
                        place(12),
                        vec![operand(2)],
                        MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                    ),
                    assign(
                        2,
                        place(13),
                        vec![operand(5), operand(12)],
                        MirRvalueKind::Binary(MirBinaryOp::Lt),
                    ),
                ],
                MirTerminatorKind::Assert {
                    condition: operand(13),
                    expected: true,
                    target: 5,
                },
            ),
            block(
                5,
                vec![
                    assign(0, place(11), vec![indexed(2, 5)], MirRvalueKind::Use),
                    assign(
                        1,
                        MirPlaceRef {
                            local: 10,
                            projection: vec![MirProjectionElem::Deref],
                        },
                        vec![operand(11), operand(1)],
                        MirRvalueKind::Binary(MirBinaryOp::Mul),
                    ),
                ],
                MirTerminatorKind::Goto { target: 6 },
            ),
            block(6, vec![], MirTerminatorKind::Return),
            block(7, vec![], MirTerminatorKind::Unreachable),
        ],
        frontend_contract: None,
    }
}

fn zeta() -> MirFunction {
    kernel(
        "zeta",
        vec![
            local(1, MirLocalRole::Arg, slice_shape(false)),
            local(2, MirLocalRole::Arg, slice_shape(false)),
            local(3, MirLocalRole::Arg, MirTypeShape::F32),
            local(4, MirLocalRole::Arg, disjoint_shape()),
        ],
        5,
        vec![
            local(11, MirLocalRole::Temp, MirTypeShape::F32),
            local(12, MirLocalRole::Temp, MirTypeShape::F32),
            local(13, MirLocalRole::Temp, MirTypeShape::F32),
            local(14, MirLocalRole::Temp, MirTypeShape::F32),
        ],
        vec![
            assign(0, place(11), vec![indexed(1, 10)], MirRvalueKind::Use),
            assign(1, place(12), vec![indexed(2, 10)], MirRvalueKind::Use),
            assign(
                2,
                place(13),
                vec![operand(11), operand(12)],
                MirRvalueKind::Binary(MirBinaryOp::Add),
            ),
            assign(
                3,
                place(14),
                vec![operand(13), operand(3)],
                MirRvalueKind::Binary(MirBinaryOp::Add),
            ),
            store(4, 9, 14),
        ],
    )
}

fn kernel(
    name: &str,
    arguments: Vec<MirLocal>,
    index_local: usize,
    arithmetic_locals: Vec<MirLocal>,
    arithmetic: Vec<MirStatement>,
) -> MirFunction {
    let output_local = arguments.last().expect("output argument").index;
    let output_ref = index_local + 1;
    let option = index_local + 2;
    let discriminant = index_local + 3;
    let payload = index_local + 4;
    let linear_index = index_local + 5;
    let mut locals = vec![local(0, MirLocalRole::Return, MirTypeShape::Unit)];
    locals.extend(arguments);
    locals.extend([
        local(index_local, MirLocalRole::Temp, thread_index_shape()),
        local(
            output_ref,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(disjoint_shape()),
                mutable: true,
            },
        ),
        local(
            option,
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: "core::option::Option".to_string(),
            },
        ),
        local(discriminant, MirLocalRole::Temp, MirTypeShape::ISize),
        local(
            payload,
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::F32),
                mutable: true,
            },
        ),
        local(linear_index, MirLocalRole::Temp, MirTypeShape::USize),
    ]);
    locals.extend(arithmetic_locals);
    locals.sort_by_key(|local| local.index);

    MirFunction {
        export_name: name.to_string(),
        rust_path: format!("tests::{name}"),
        kind: MirFunctionKind::KernelEntry,
        typed_profile: Some(crate::mir_import::MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
        arg_count: output_local,
        local_count: locals.len(),
        locals,
        blocks: vec![
            block(
                0,
                Vec::new(),
                call(TrustedDeviceItem::ThreadIndex1d, Vec::new(), index_local, 1),
            ),
            block(
                1,
                vec![assign(
                    0,
                    place(output_ref),
                    vec![operand(output_local)],
                    MirRvalueKind::Ref,
                )],
                call(
                    TrustedDeviceItem::DisjointSliceGetMut,
                    vec![operand(output_ref), operand(index_local)],
                    option,
                    2,
                ),
            ),
            block(
                2,
                vec![assign(
                    0,
                    place(discriminant),
                    vec![operand(option)],
                    MirRvalueKind::Discriminant,
                )],
                MirTerminatorKind::SwitchInt {
                    discriminant: operand(discriminant),
                    targets: vec![
                        MirSwitchTarget {
                            value: 1,
                            target: 3,
                        },
                        MirSwitchTarget {
                            value: 0,
                            target: 5,
                        },
                    ],
                    otherwise: 6,
                },
            ),
            block(
                3,
                vec![assign(
                    0,
                    place(payload),
                    vec![MirOperandRef::Place(MirPlaceRef {
                        local: option,
                        projection: vec![
                            MirProjectionElem::Downcast { variant: 1 },
                            MirProjectionElem::Field(0),
                        ],
                    })],
                    MirRvalueKind::Use,
                )],
                call(
                    TrustedDeviceItem::ThreadIndexGet,
                    vec![operand(index_local)],
                    linear_index,
                    4,
                ),
            ),
            block(4, arithmetic, MirTerminatorKind::Goto { target: 5 }),
            block(5, Vec::new(), MirTerminatorKind::Return),
            block(6, Vec::new(), MirTerminatorKind::Unreachable),
        ],
        frontend_contract: None,
    }
}

fn function<'a>(module: &'a Module, id: &str) -> &'a Function {
    module
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .expect("kernel definition")
}

fn operations(function: &Function) -> Vec<&Operation> {
    function
        .body
        .as_ref()
        .expect("kernel body")
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect()
}

fn block(index: usize, statements: Vec<MirStatement>, kind: MirTerminatorKind) -> MirBlock {
    MirBlock {
        index,
        statements,
        terminator: Some(MirTerminator { kind, source: None }),
    }
}

fn call(
    item: TrustedDeviceItem,
    operands: Vec<MirOperandRef>,
    destination: usize,
    target: usize,
) -> MirTerminatorKind {
    MirTerminatorKind::Call {
        callee: Some(MirCallee::trusted_for_test(item)),
        target: Some(target),
        destination: Some(place(destination)),
        operands,
    }
}

fn assign(
    index: usize,
    destination: MirPlaceRef,
    operands: Vec<MirOperandRef>,
    rvalue: MirRvalueKind,
) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(destination),
        operands,
        rvalue: Some(rvalue),
        operation: None,
        source: None,
    }
}

fn store(index: usize, pointer_local: usize, value_local: usize) -> MirStatement {
    assign(
        index,
        MirPlaceRef {
            local: pointer_local,
            projection: vec![MirProjectionElem::Deref],
        },
        vec![operand(value_local)],
        MirRvalueKind::Use,
    )
}

fn indexed(slice_local: usize, index_local: usize) -> MirOperandRef {
    MirOperandRef::Place(MirPlaceRef {
        local: slice_local,
        projection: vec![
            MirProjectionElem::Deref,
            MirProjectionElem::Index { local: index_local },
        ],
    })
}

fn operand(local: usize) -> MirOperandRef {
    MirOperandRef::Place(place(local))
}

fn place(local: usize) -> MirPlaceRef {
    MirPlaceRef {
        local,
        projection: Vec::new(),
    }
}

fn usize_constant(value: u64) -> MirOperandRef {
    MirOperandRef::Constant {
        ty: imported(MirTypeShape::USize),
        literal: MirConstant::USize(value),
        value: value.to_string(),
    }
}

fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
    MirLocal {
        index,
        role,
        ty: imported(shape),
    }
}

fn imported(shape: MirTypeShape) -> MirImportedType {
    let (kind, rust) = match &shape {
        MirTypeShape::Unit => (MirType::Unit, "()"),
        MirTypeShape::F32 => (MirType::F32, "f32"),
        MirTypeShape::USize => (MirType::USize, "usize"),
        MirTypeShape::ISize => (MirType::I64, "isize"),
        MirTypeShape::Slice { .. } => (MirType::Slice, "&[f32]"),
        MirTypeShape::DisjointSlice { .. } => (MirType::DisjointSlice, "DisjointSlice<f32>"),
        MirTypeShape::Reference { .. } => (MirType::Ptr, "&mut T"),
        MirTypeShape::Adt { .. } => (MirType::Unknown, "adt"),
        _ => (MirType::Unknown, "unknown"),
    };
    MirImportedType {
        kind,
        rust: rust.to_string(),
        shape,
    }
}

fn thread_index_shape() -> MirTypeShape {
    MirTypeShape::Adt {
        identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_string(),
    }
}

fn slice_shape(mutable: bool) -> MirTypeShape {
    MirTypeShape::Slice {
        element: Box::new(MirTypeShape::F32),
        mutable,
    }
}

fn disjoint_shape() -> MirTypeShape {
    MirTypeShape::DisjointSlice {
        element: Box::new(MirTypeShape::F32),
    }
}

fn slice(access: AccessMode) -> Type {
    Type::slice(Type::F32, AddressSpace::Global, access)
}
