use crate::AmdGpuTarget;
use crate::kernel_ir_lowering::{exact_gfx942_xnack_minus_target, translate_and_verify_for_target};
use crate::mir_import::{
    MirBlock, MirCallee, MirFunction, MirFunctionKind, MirImportedType, MirKernelProfile, MirLocal,
    MirLocalRole, MirModule, MirOperandRef, MirPlaceRef, MirTerminator, MirTerminatorKind,
    MirTypeShape,
};
use crate::trusted_device_items::TrustedDeviceItem;
use dialect_mir::MirType;
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    OperationKind, TargetCapability,
};

#[test]
fn collective_target_authority_accepts_only_exact_canonical_gfx942_xnack_minus() {
    assert!(exact_gfx942_xnack_minus_target(&AmdGpuTarget::new("gfx942:xnack-")).is_some());

    for rejected in [
        "gfx942",
        "gfx942:xnack+",
        "gfx942:sramecc+:xnack-",
        "gfx942:sramecc-:xnack-",
        "gfx942:xnack-:sramecc+",
        "gfx942:xnack-:xnack-",
        "gfx942:xnack-:xnack+",
        "gfx942:future+",
        "gfx941",
        "gfx950",
        "gfx1100",
    ] {
        assert!(
            exact_gfx942_xnack_minus_target(&AmdGpuTarget::new(rejected)).is_none(),
            "unexpectedly admitted {rejected}"
        );
    }
}

#[test]
fn active_wave64_and_static_lds_reduction_reach_exact_gfx942_ir() {
    let module = translate_and_verify_for_target(
        &wave_lds_v1_module(true),
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .expect("authenticated wave/LDS V1");
    let operations = operations(&module);
    let target_binding = TargetCapability::Extension {
        namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
    };
    assert!(module.required_capabilities.contains(&target_binding));
    assert!(
        module.functions[0]
            .required_capabilities
            .contains(&target_binding)
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, OperationKind::WorkgroupMemory(_)))
            .count(),
        1
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, OperationKind::WorkgroupBarrier(_)))
            .count(),
        18
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, OperationKind::Wave(_)))
            .count(),
        8,
        "one ballot, one lane-id, and six shuffles"
    );

    let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
        .expect("wave/LDS V1 LLVM");
    assert_eq!(llvm.matches("call i64 @llvm.amdgcn.ballot.i64").count(), 1);
    assert_eq!(llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count(), 6);
    assert_eq!(
        llvm.matches("call void @llvm.amdgcn.s.barrier()").count(),
        18
    );
    assert!(llvm.contains("addrspace(3) global [256 x i32] undef, align 4"));
    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(llvm.contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\""));
}

#[test]
fn static_lds_authority_cannot_be_replaced_by_an_ordinary_local() {
    let error = translate_and_verify_for_target(
        &wave_lds_v1_module(false),
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .unwrap_err();
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("static LDS did not originate from the authenticated compiler constructor")
    }));
}

#[test]
fn wave64_sum_profiles_reach_shuffle_llvm_for_all_admitted_types() {
    for (item, shape, expected_add) in [
        (
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            MirTypeShape::U32,
            "add i32",
        ),
        (
            TrustedDeviceItem::Gfx942Wave64InclusiveScanSum,
            MirTypeShape::I32,
            "add i32",
        ),
        (
            TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum,
            MirTypeShape::F32,
            "fadd float",
        ),
    ] {
        let module = translate_and_verify_for_target(
            &collective_module(item, shape.clone(), true),
            &AmdGpuTarget::new("gfx942:xnack-"),
        )
        .expect("wave collective");
        let operations = operations(&module);
        assert!(
            operations
                .iter()
                .any(|operation| matches!(operation, OperationKind::Wave(_)))
        );
        assert!(
            !operations
                .iter()
                .any(|operation| matches!(operation, OperationKind::WorkgroupMemory(_)))
        );

        let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
            .expect("wave collective LLVM");
        assert!(llvm.contains("@llvm.amdgcn.ds.bpermute"));
        assert!(llvm.contains(expected_add));
        if shape == MirTypeShape::F32 {
            assert!(llvm.contains("bitcast float"));
            assert!(llvm.contains("bitcast i32"));
        }
    }
}

#[test]
fn workgroup_sum_profiles_reach_real_lds_and_barrier_llvm() {
    for item in [
        TrustedDeviceItem::Gfx942WorkgroupReduceSum,
        TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum,
        TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum,
    ] {
        let module = translate_and_verify_for_target(
            &collective_module(item, MirTypeShape::U32, true),
            &AmdGpuTarget::new("gfx942:xnack-"),
        )
        .expect("workgroup collective");
        let operations = operations(&module);
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation, OperationKind::WorkgroupMemory(_)))
                .count(),
            1
        );
        assert!(
            operations
                .iter()
                .any(|operation| matches!(operation, OperationKind::WorkgroupBarrier(_)))
        );
        let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
            .expect("workgroup collective LLVM");
        assert!(llvm.contains("addrspace(3) global [256 x i32]"));
        assert!(llvm.contains("call void @llvm.amdgcn.s.barrier()"));
        assert!(llvm.contains("load i32, ptr addrspace(3)"));
        assert!(llvm.contains("store i32"));
    }
}

#[test]
fn gfx942_deferred_barrier_is_release_then_physical_wait() {
    let module =
        translate_and_verify_for_target(&barrier_module(), &AmdGpuTarget::new("gfx942:xnack-"))
            .expect("deferred gfx942 barrier");
    let operations = operations(&module);
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, OperationKind::Fence(_)))
            .count(),
        1
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, OperationKind::WorkgroupBarrier(_)))
            .count(),
        1
    );

    let llvm = dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module)
        .expect("deferred barrier LLVM");
    assert!(llvm.contains("fence syncscope(\"workgroup\") release"));
    assert!(llvm.contains("call void @llvm.amdgcn.s.barrier()"));
    assert!(llvm.contains("fence syncscope(\"workgroup\") acquire"));
    assert!(!llvm.contains("s.barrier.signal"));
    assert!(!llvm.contains("s.barrier.wait"));
}

#[test]
fn collective_calls_reject_wrong_target_type_context_and_arity() {
    let wrong_target = translate_and_verify_for_target(
        &collective_module(
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            MirTypeShape::U32,
            true,
        ),
        &AmdGpuTarget::new("gfx1100"),
    )
    .unwrap_err();
    assert!(wrong_target.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires exact gfx942:xnack- General V3")
    }));

    let unsupported_type = translate_and_verify_for_target(
        &collective_module(
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            MirTypeShape::F64,
            true,
        ),
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .unwrap_err();
    assert!(unsupported_type.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("support exactly u32, i32, and f32")
    }));

    let forged_context = translate_and_verify_for_target(
        &collective_module(
            TrustedDeviceItem::Gfx942Wave64ReduceSum,
            MirTypeShape::U32,
            false,
        ),
        &AmdGpuTarget::new("gfx942:xnack-"),
    )
    .unwrap_err();
    assert!(forged_context.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("did not originate from the authenticated compiler constructor")
    }));

    let mut wrong_arity = collective_module(
        TrustedDeviceItem::Gfx942Wave64ReduceSum,
        MirTypeShape::U32,
        true,
    );
    let MirTerminatorKind::Call { operands, .. } = &mut wrong_arity.functions[0].blocks[1]
        .terminator
        .as_mut()
        .expect("collective terminator")
        .kind
    else {
        panic!("call")
    };
    operands.pop();
    let error = translate_and_verify_for_target(&wrong_arity, &AmdGpuTarget::new("gfx942:xnack-"))
        .unwrap_err();
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects 3 operand"))
    );
}

fn collective_module(
    item: TrustedDeviceItem,
    value_shape: MirTypeShape,
    with_context: bool,
) -> MirModule {
    let result_shape = value_shape.clone();
    let workgroup = matches!(
        item,
        TrustedDeviceItem::Gfx942WorkgroupReduceSum
            | TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum
            | TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum
    );
    let context_block = if with_context {
        block(
            0,
            call(
                TrustedDeviceItem::Gfx942CollectivesFromCompiler,
                Vec::new(),
                2,
                1,
            ),
        )
    } else {
        block(0, MirTerminatorKind::Goto { target: 1 })
    };
    let operands = if workgroup {
        vec![operand(4), operand(2), operand(5), operand(1)]
    } else {
        vec![operand(4), operand(2), operand(1)]
    };
    MirModule {
        functions: vec![MirFunction {
            export_name: "collective_v1".to_owned(),
            rust_path: "tests::collective_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 1,
            local_count: 6,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Arg, value_shape),
                local(
                    2,
                    MirLocalRole::Temp,
                    adt("fe2o3_device::Gfx942Collectives"),
                ),
                local(3, MirLocalRole::Temp, result_shape),
                local(
                    4,
                    MirLocalRole::Temp,
                    adt("fe2o3_device::SubgroupTileOrWorkgroup"),
                ),
                local(
                    5,
                    MirLocalRole::Temp,
                    adt("fe2o3_device::WorkgroupCollectiveScratch"),
                ),
            ],
            blocks: vec![
                context_block,
                block(1, call(item, operands, 3, 2)),
                block(2, MirTerminatorKind::Return),
            ],
            frontend_contract: None,
            matrix_frontend_abi: None,
        }],
    }
}

fn barrier_module() -> MirModule {
    MirModule {
        functions: vec![MirFunction {
            export_name: "barrier_v1".to_owned(),
            rust_path: "tests::barrier_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 0,
            local_count: 3,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Temp, MirTypeShape::Unit),
                local(2, MirLocalRole::Temp, MirTypeShape::Unit),
            ],
            blocks: vec![
                block(
                    0,
                    call(TrustedDeviceItem::Gfx942BarrierArrive, Vec::new(), 1, 1),
                ),
                block(
                    1,
                    call(TrustedDeviceItem::Gfx942BarrierWait, Vec::new(), 2, 2),
                ),
                block(2, MirTerminatorKind::Return),
            ],
            frontend_contract: None,
            matrix_frontend_abi: None,
        }],
    }
}

fn wave_lds_v1_module(with_scratch: bool) -> MirModule {
    let scratch_block = if with_scratch {
        block(
            1,
            call(
                TrustedDeviceItem::Gfx942StaticLdsU32x256,
                vec![operand(3)],
                4,
                2,
            ),
        )
    } else {
        block(1, MirTerminatorKind::Goto { target: 2 })
    };
    MirModule {
        functions: vec![MirFunction {
            export_name: "gfx942_wave_lds_v1".to_owned(),
            rust_path: "tests::gfx942_wave_lds_v1".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
            arg_count: 2,
            local_count: 7,
            locals: vec![
                local(0, MirLocalRole::Return, MirTypeShape::Unit),
                local(1, MirLocalRole::Arg, MirTypeShape::U32),
                local(2, MirLocalRole::Arg, MirTypeShape::U32),
                local(
                    3,
                    MirLocalRole::Temp,
                    adt("fe2o3_device::Gfx942Collectives"),
                ),
                local(
                    4,
                    MirLocalRole::Temp,
                    adt("fe2o3_device::Gfx942StaticLdsU32x256"),
                ),
                local(5, MirLocalRole::Temp, MirTypeShape::U32),
                local(6, MirLocalRole::Temp, MirTypeShape::U32),
            ],
            blocks: vec![
                block(
                    0,
                    call(
                        TrustedDeviceItem::Gfx942CollectivesFromCompiler,
                        Vec::new(),
                        3,
                        1,
                    ),
                ),
                scratch_block,
                block(
                    2,
                    call(
                        TrustedDeviceItem::Gfx942Wave64ReduceActiveU32,
                        vec![operand(3), operand(1), operand(2)],
                        5,
                        3,
                    ),
                ),
                block(
                    3,
                    call(
                        TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32,
                        vec![operand(3), operand(4), operand(1), operand(2)],
                        6,
                        4,
                    ),
                ),
                block(4, MirTerminatorKind::Return),
            ],
            frontend_contract: None,
            matrix_frontend_abi: None,
        }],
    }
}

fn operations(module: &fe2o3_kernel_ir::Module) -> Vec<&OperationKind> {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect()
}

fn block(index: usize, kind: MirTerminatorKind) -> MirBlock {
    MirBlock {
        index,
        statements: Vec::new(),
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

fn adt(identity: &str) -> MirTypeShape {
    MirTypeShape::Adt {
        identity: identity.to_owned(),
    }
}

fn imported(shape: MirTypeShape) -> MirImportedType {
    let (kind, rust) = match shape {
        MirTypeShape::Unit => (MirType::Unit, "()"),
        MirTypeShape::U32 => (MirType::I32, "u32"),
        MirTypeShape::I32 => (MirType::I32, "i32"),
        MirTypeShape::F32 => (MirType::F32, "f32"),
        MirTypeShape::F64 => (MirType::F64, "f64"),
        MirTypeShape::Adt { .. } => (MirType::Unknown, "adt"),
        _ => (MirType::Unknown, "unsupported"),
    };
    MirImportedType {
        kind,
        rust: rust.to_owned(),
        shape,
    }
}
