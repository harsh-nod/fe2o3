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
    assert_eq!(intrinsics.len(), 4);
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

    let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
        .expect("gfx942 memory LLVM");
    assert!(llvm.contains("sdiv exact i64"));
    assert!(llvm.contains("load volatile float, ptr addrspace(1)"));
    assert!(llvm.contains("store volatile float"));
    assert!(llvm.contains("mul nuw i64 %arg4, 4"));
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
        module.functions[0].locals[1].ty = imported(slice_shape(false, MirTypeShape::F16));
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
    module.functions[0].locals[10].ty = imported(MirTypeShape::Reference {
        pointee: Box::new(slice_shape(true, MirTypeShape::F32)),
        mutable: true,
    });
    module.functions[0].blocks[3].statements[0].operands = vec![operand(1)];
    let MirTerminatorKind::Call { operands, .. } = &mut module.functions[0].blocks[3]
        .terminator
        .as_mut()
        .expect("copy terminator")
        .kind
    else {
        panic!("copy call")
    };
    operands[2] = operand(10);

    let errors =
        translate_and_verify_for_target(&module, &AmdGpuTarget::new("gfx942")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove dynamic region disjointness")
    }));
}

fn memory_module() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            export_name: "memory_v1".to_owned(),
            rust_path: "tests::memory_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 5,
            local_count: 12,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Arg, slice_shape(false, MirTypeShape::F32)),
                local(2, MirLocalRole::Arg, disjoint_shape(MirTypeShape::F32)),
                local(3, MirLocalRole::Arg, MirTypeShape::USize),
                local(4, MirLocalRole::Arg, MirTypeShape::USize),
                local(5, MirLocalRole::Arg, MirTypeShape::USize),
                local(6, MirLocalRole::Temp, MirTypeShape::ISize),
                local(7, MirLocalRole::Temp, MirTypeShape::F32),
                local(
                    8,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: true,
                    },
                ),
                local(9, MirLocalRole::Temp, MirTypeShape::Unit),
                local(
                    10,
                    MirLocalRole::Temp,
                    MirTypeShape::Reference {
                        pointee: Box::new(disjoint_shape(MirTypeShape::F32)),
                        mutable: true,
                    },
                ),
                local(11, MirLocalRole::Temp, MirTypeShape::Unit),
            ],
            blocks: vec![
                block(
                    0,
                    Vec::new(),
                    call(
                        TrustedDeviceItem::MemoryOffsetFrom,
                        vec![operand(1), operand(4), operand(3)],
                        6,
                        1,
                    ),
                ),
                block(
                    1,
                    Vec::new(),
                    call(
                        TrustedDeviceItem::MemoryVolatileLoad,
                        vec![operand(1), operand(3)],
                        7,
                        2,
                    ),
                ),
                block(
                    2,
                    vec![assign_ref(0, 8, 2)],
                    call(
                        TrustedDeviceItem::MemoryVolatileStore,
                        vec![operand(8), operand(4), operand(7)],
                        9,
                        3,
                    ),
                ),
                block(
                    3,
                    vec![assign_ref(0, 10, 2)],
                    call(
                        TrustedDeviceItem::MemoryCopyNonOverlapping,
                        vec![operand(1), operand(3), operand(10), operand(4), operand(5)],
                        11,
                        4,
                    ),
                ),
                block(4, Vec::new(), MirTerminatorKind::Return),
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

fn assign_ref(index: usize, destination: usize, source: usize) -> MirStatement {
    MirStatement {
        index,
        kind: MirStatementKind::Assign,
        destination: Some(place(destination)),
        operands: vec![operand(source)],
        rvalue: Some(MirRvalueKind::Ref),
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
