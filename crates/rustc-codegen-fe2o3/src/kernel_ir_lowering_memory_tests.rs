use super::*;
use crate::mir_import::{
    MirBlock, MirImportedType, MirLocal, MirLocalRole, MirStatement, MirTypeShape,
};
use dialect_mir::MirType;
use fe2o3_kernel_ir::{
    CopyNonOverlappingContract, MemoryElementType, MemoryIntrinsicOperation,
    PointerDistanceContract, PointerDistanceKind, PointerDistanceUnit, VolatileAccessContract,
};

#[test]
fn recognized_memory_calls_reach_verified_ir_and_gfx942_llvm() {
    let module =
        translate_and_verify_for_target(&memory_module(), &AmdGpuTarget::new("gfx942:xnack-"))
            .expect("bounded memory-v1 module");
    let function = module
        .functions
        .iter()
        .find(|function| function.id.as_str() == "tests::memory_v1")
        .expect("memory kernel");
    let intrinsics = function
        .body
        .as_ref()
        .expect("memory body")
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            let OperationKind::MemoryIntrinsic(intrinsic) = &operation.kind else {
                return None;
            };
            Some(intrinsic)
        })
        .collect::<Vec<_>>();
    assert_eq!(intrinsics.len(), 5);
    assert!(matches!(
        intrinsics[0],
        MemoryIntrinsicOperation::PointerDistance {
            kind: PointerDistanceKind::Signed,
            unit: PointerDistanceUnit::Elements,
            element: MemoryElementType::Scalar(ScalarType::F32),
            address_space: AddressSpace::Global,
            layout,
            contract,
            ..
        } if *layout == MemoryElementType::Scalar(ScalarType::F32).expected_layout()
            && *contract == PointerDistanceContract::supported_rust(PointerDistanceKind::Signed)
    ));
    assert!(matches!(
        intrinsics[1],
        MemoryIntrinsicOperation::VolatileLoad {
            element: MemoryElementType::Scalar(ScalarType::F32),
            address_space: AddressSpace::Global,
            layout,
            contract,
            ..
        } if *layout == MemoryElementType::Scalar(ScalarType::F32).expected_layout()
            && *contract == VolatileAccessContract::rust_allocation_load()
    ));
    assert!(matches!(
        intrinsics[2],
        MemoryIntrinsicOperation::VolatileStore {
            element: MemoryElementType::Scalar(ScalarType::F32),
            address_space: AddressSpace::Global,
            layout,
            contract,
            ..
        } if *layout == MemoryElementType::Scalar(ScalarType::F32).expected_layout()
            && *contract == VolatileAccessContract::rust_allocation_store()
    ));
    assert!(matches!(
        intrinsics[3],
        MemoryIntrinsicOperation::CopyNonOverlapping {
            element: MemoryElementType::Scalar(ScalarType::F32),
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout,
            contract,
            ..
        } if *layout == MemoryElementType::Scalar(ScalarType::F32).expected_layout()
            && *contract == CopyNonOverlappingContract::supported_rust()
    ));
    assert!(matches!(
        intrinsics[4],
        MemoryIntrinsicOperation::CopyNonOverlapping {
            element: MemoryElementType::Scalar(ScalarType::F32),
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout,
            contract,
            ..
        } if *layout == MemoryElementType::Scalar(ScalarType::F32).expected_layout()
            && *contract == CopyNonOverlappingContract::supported_rust()
    ));
    let MemoryIntrinsicOperation::CopyNonOverlapping {
        count: copy_one_count,
        ..
    } = intrinsics[3]
    else {
        unreachable!("checked one-element copy intrinsic")
    };
    assert!(
        function
            .body
            .as_ref()
            .expect("memory body")
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                operation
                    .results
                    .iter()
                    .any(|result| result.id == *copy_one_count)
                    && operation.kind == OperationKind::Constant(Constant::Index(1))
            })
    );
    let MemoryIntrinsicOperation::CopyNonOverlapping {
        count: general_count,
        ..
    } = intrinsics[4]
    else {
        unreachable!("checked general copy intrinsic")
    };
    assert_eq!(
        *general_count,
        function.body.as_ref().expect("memory body").parameters[3],
        "the unsafe expert copy must retain its runtime count parameter"
    );
    assert_ne!(general_count, copy_one_count);

    let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
        .expect("gfx942 memory LLVM");
    assert!(llvm.contains("sdiv exact i64"));
    assert!(llvm.contains("load volatile float, ptr addrspace(1)"));
    assert!(llvm.contains("store volatile float"));
    assert!(llvm.contains("mul nuw i64 %arg3, 4"));
    assert!(llvm.contains("call void @llvm.memcpy.p1.p1.i64"));
    assert!(llvm.contains("ptr addrspace(1) align 4"));
}

#[test]
fn memory_calls_forward_promoted_ssa_values_to_their_successor() {
    let mut input = memory_module();
    input.functions[0].blocks[0]
        .statements
        .push(assign_use(0, 3, 4));

    let module = translate_and_verify_for_target(&input, &AmdGpuTarget::new("gfx942:xnack-"))
        .expect("memory call with live promoted successor value");
    let function = module
        .functions
        .iter()
        .find(|function| function.id.as_str() == "tests::memory_v1")
        .expect("memory kernel");
    let body = function.body.as_ref().expect("memory body");
    let entry = body
        .blocks
        .iter()
        .find(|block| block.id.0 == 0)
        .expect("entry block");
    let successor = body
        .blocks
        .iter()
        .find(|block| block.id.0 == 1)
        .expect("successor block");
    let Terminator::Branch { arguments, .. } = entry.terminator.as_ref().expect("entry terminator")
    else {
        panic!("memory intrinsic must branch to its successor")
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(successor.parameters.len(), 1);
}

#[test]
fn memory_v1_lowers_authenticated_disjoint_slice_length() {
    let module = MirModule {
        functions: vec![MirFunction {
            semantic_instance: None,
            export_name: "memory_len_v1".to_owned(),
            rust_path: "tests::memory_len_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 1,
            local_count: 4,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Arg, disjoint_shape(MirTypeShape::F32)),
                local(
                    2,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: false,
                    },
                ),
                local(3, MirLocalRole::Temp, MirTypeShape::USize),
            ],
            blocks: vec![
                block(
                    0,
                    vec![assign_shared_ref(0, 2, 1)],
                    call(TrustedDeviceItem::DisjointSliceLen, vec![operand(2)], 3, 1),
                ),
                block(1, Vec::new(), MirTerminatorKind::Return),
            ],
            frontend_contract: None,
            matrix_frontend_abi: None,
        }],
    };

    let lowered = translate_and_verify_for_target(&module, &AmdGpuTarget::new("gfx942:xnack-"))
        .expect("authenticated DisjointSlice length");
    let body = lowered.functions[0]
        .body
        .as_ref()
        .expect("memory length body");
    assert!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::SliceLength { .. }))
    );
}

#[test]
fn recognized_memory_calls_fail_closed_outside_the_bounded_profile() {
    let untyped = {
        let mut module = memory_module();
        module.functions[0].typed_profile = None;
        module
    };
    let errors =
        translate_and_verify_for_target(&untyped, &AmdGpuTarget::new("gfx942")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires the gfx942 General V3 memory-v1 profile")
    }));

    let errors = translate_and_verify_for_target(&memory_module(), &AmdGpuTarget::new("gfx1100"))
        .unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires the gfx942 General V3 memory-v1 profile")
    }));
}

#[test]
fn memory_v1_rejects_read_only_destinations_and_unsupported_elements() {
    let read_only_destination = {
        let mut module = memory_module();
        module.functions[0].locals[2].ty = imported(slice_shape(false, MirTypeShape::F32));
        module
    };
    let errors =
        translate_and_verify_for_target(&read_only_destination, &AmdGpuTarget::new("gfx942"))
            .unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("destination must be a writable DisjointSlice")
    }));

    let unsupported_element = {
        let mut module = memory_module();
        module.functions[0].locals[1].ty = imported(slice_shape(
            false,
            MirTypeShape::Adt {
                identity: "tests::UnsupportedElement".to_owned(),
            },
        ));
        module
    };
    let errors =
        translate_and_verify_for_target(&unsupported_element, &AmdGpuTarget::new("gfx942"))
            .unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("argument local1 has unsupported type")
    }));
}

#[test]
fn memory_v1_rejects_a_copy_whose_slice_identity_overlaps() {
    let mut module = memory_module();
    module.functions[0].locals[1].ty = imported(slice_shape(true, MirTypeShape::F32));
    module.functions[0].locals[12].ty = imported(MirTypeShape::Reference {
        pointee: Box::new(slice_shape(true, MirTypeShape::F32)),
        mutable: true,
    });
    module.functions[0].blocks[5].statements[0].operands = vec![operand(1)];
    let MirTerminatorKind::Call { operands, .. } = &mut module.functions[0].blocks[5]
        .terminator
        .as_mut()
        .expect("copy terminator")
        .kind
    else {
        panic!("copy call")
    };
    operands[2] = operand(12);

    let errors =
        translate_and_verify_for_target(&module, &AmdGpuTarget::new("gfx942")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove dynamic region disjointness")
    }));
}

#[test]
fn memory_v1_rejects_integer_and_unproven_write_authority() {
    let mut integer = memory_module();
    let MirTerminatorKind::Call { operands, .. } = &mut integer.functions[0].blocks[4]
        .terminator
        .as_mut()
        .expect("store terminator")
        .kind
    else {
        panic!("store call")
    };
    operands[1] = operand(4);
    let errors =
        translate_and_verify_for_target(&integer, &AmdGpuTarget::new("gfx942")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("shared reference to the trusted DisjointIndex type")
    }));

    let mut unproven = memory_module();
    unproven.functions[0].blocks[4].statements[1].operands = vec![operand(4)];
    let errors =
        translate_and_verify_for_target(&unproven, &AmdGpuTarget::new("gfx942")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("did not originate from trusted thread::index_1d().into_disjoint()")
    }));
}

fn memory_module() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            semantic_instance: None,
            export_name: "memory_v1".to_owned(),
            rust_path: "tests::memory_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 4,
            local_count: 17,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Arg, slice_shape(false, MirTypeShape::F32)),
                local(2, MirLocalRole::Arg, disjoint_shape(MirTypeShape::F32)),
                local(3, MirLocalRole::Arg, MirTypeShape::USize),
                local(4, MirLocalRole::Arg, MirTypeShape::USize),
                trusted_adt_local(5, TrustedDeviceItem::ThreadIndex),
                trusted_adt_local(6, TrustedDeviceItem::DisjointIndex),
                local(7, MirLocalRole::Temp, MirTypeShape::ISize),
                local(8, MirLocalRole::Temp, MirTypeShape::F32),
                local(
                    9,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: true,
                    },
                ),
                trusted_adt_reference_local(10, TrustedDeviceItem::DisjointIndex),
                local(11, MirLocalRole::Temp, MirTypeShape::Unit),
                local(
                    12,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: true,
                    },
                ),
                trusted_adt_reference_local(13, TrustedDeviceItem::DisjointIndex),
                local(14, MirLocalRole::Temp, MirTypeShape::Unit),
                local(
                    15,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: true,
                    },
                ),
                local(16, MirLocalRole::Temp, MirTypeShape::Unit),
            ],
            blocks: vec![
                block(
                    0,
                    Vec::new(),
                    call(TrustedDeviceItem::ThreadIndex1d, Vec::new(), 5, 1),
                ),
                block(
                    1,
                    Vec::new(),
                    call(
                        TrustedDeviceItem::ThreadIndexIntoDisjoint,
                        vec![operand(5)],
                        6,
                        2,
                    ),
                ),
                block(
                    2,
                    Vec::new(),
                    call(
                        TrustedDeviceItem::MemoryOffsetFrom,
                        vec![operand(1), operand(4), operand(3)],
                        7,
                        3,
                    ),
                ),
                block(
                    3,
                    Vec::new(),
                    call(
                        TrustedDeviceItem::MemoryVolatileLoad,
                        vec![operand(1), operand(3)],
                        8,
                        4,
                    ),
                ),
                block(
                    4,
                    vec![assign_ref(0, 9, 2, true), assign_ref(1, 10, 6, false)],
                    call(
                        TrustedDeviceItem::MemoryVolatileStore,
                        vec![operand(9), operand(10), operand(8)],
                        11,
                        5,
                    ),
                ),
                block(
                    5,
                    vec![assign_ref(0, 12, 2, true), assign_ref(1, 13, 6, false)],
                    call(
                        TrustedDeviceItem::MemoryCopyOneNonOverlapping,
                        vec![operand(1), operand(3), operand(12), operand(13)],
                        14,
                        6,
                    ),
                ),
                block(
                    6,
                    vec![assign_ref(0, 15, 2, true)],
                    call(
                        TrustedDeviceItem::MemoryCopyNonOverlapping,
                        vec![operand(1), operand(3), operand(15), operand(3), operand(4)],
                        16,
                        7,
                    ),
                ),
                block(7, Vec::new(), MirTerminatorKind::Return),
            ],
            frontend_contract: None,
            matrix_frontend_abi: None,
        }],
    }
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

fn assign_ref(index: usize, destination: usize, source: usize, mutable: bool) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(place(destination)),
        operands: vec![operand(source)],
        rvalue: Some(MirRvalueKind::Reference(if mutable {
            crate::mir_import::MirBorrowKind::MutableDefault
        } else {
            crate::mir_import::MirBorrowKind::Shared
        })),
        semantic_rvalue_type: None,
        operation: None,
        source: None,
    }
}

fn assign_shared_ref(index: usize, destination: usize, source: usize) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(place(destination)),
        operands: vec![operand(source)],
        rvalue: Some(MirRvalueKind::Reference(
            crate::mir_import::MirBorrowKind::Shared,
        )),
        semantic_rvalue_type: None,
        operation: None,
        source: None,
    }
}

fn assign_use(index: usize, destination: usize, source: usize) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(place(destination)),
        operands: vec![operand(source)],
        rvalue: Some(MirRvalueKind::Use),
        semantic_rvalue_type: None,
        operation: None,
        source: None,
    }
}

fn operand(local: usize) -> MirOperandRef {
    MirOperandRef::Place(place(local))
}

fn place(local: usize) -> MirPlaceRef {
    MirPlaceRef {
        local,
        projection: Vec::new(),
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    }
}

fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
    MirLocal {
        index,
        role,
        ty: imported(shape),
    }
}

fn trusted_adt_local(index: usize, item: TrustedDeviceItem) -> MirLocal {
    local(
        index,
        MirLocalRole::Temp,
        MirTypeShape::Adt {
            identity: item.canonical_path().to_owned(),
        },
    )
}

fn trusted_adt_reference_local(index: usize, item: TrustedDeviceItem) -> MirLocal {
    local(
        index,
        MirLocalRole::Temp,
        MirTypeShape::Reference {
            pointee: Box::new(MirTypeShape::Adt {
                identity: item.canonical_path().to_owned(),
            }),
            mutable: false,
        },
    )
}

fn imported(shape: MirTypeShape) -> MirImportedType {
    let (kind, rust) = match &shape {
        MirTypeShape::Unit => (MirType::Unit, "()"),
        MirTypeShape::F32 => (MirType::F32, "f32"),
        MirTypeShape::F16 => (MirType::Unknown, "f16"),
        MirTypeShape::USize => (MirType::USize, "usize"),
        MirTypeShape::ISize => (MirType::I64, "isize"),
        MirTypeShape::Slice { .. } => (MirType::Slice, "slice"),
        MirTypeShape::DisjointSlice { .. } => (MirType::DisjointSlice, "DisjointSlice"),
        MirTypeShape::Reference { .. } => (MirType::Ptr, "reference"),
        _ => (MirType::Unknown, "unknown"),
    };
    MirImportedType {
        kind,
        rust: rust.to_owned(),
        shape,
        semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
    }
}

fn slice_shape(mutable: bool, element: MirTypeShape) -> MirTypeShape {
    MirTypeShape::Slice {
        element: Box::new(element),
        mutable,
    }
}

fn disjoint_shape(element: MirTypeShape) -> MirTypeShape {
    MirTypeShape::DisjointSlice {
        element: Box::new(element),
    }
}
