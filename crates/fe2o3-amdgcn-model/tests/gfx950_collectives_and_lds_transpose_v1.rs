use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir,
    lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1,
};
use fe2o3_kernel_ir::*;

fn collective_and_lds_transpose_module(format: Gfx950LdsTransposeFormatV1) -> Module {
    collective_and_lds_transpose_module_with_workgroup(format, 64)
}

fn collective_and_lds_transpose_module_with_workgroup(
    format: Gfx950LdsTransposeFormatV1,
    workgroup_x: u32,
) -> Module {
    let source = Type::slice(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let storage = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let parameter_types = vec![
        source,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::F32,
    ];
    let parameters = (0..parameter_types.len() as u32)
        .map(ValueId)
        .collect::<Vec<_>>();
    let mut operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(10), storage.clone())],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Current { format },
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(11), storage.clone())],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Stage {
                    format,
                    storage: ValueId(10),
                    source_slice: ValueId(0),
                    offset: ValueId(1),
                    rows: ValueId(2),
                    columns: ValueId(3),
                    stride: ValueId(4),
                    token_base: ValueId(5),
                    reduction_base: ValueId(6),
                },
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(12), storage)],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Publish {
                    format,
                    storage: ValueId(11),
                },
            )),
        ),
        Operation::new(
            (13..21)
                .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
                .collect(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Read {
                    format,
                    storage: ValueId(12),
                },
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(22), Type::F32)],
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(7),
                    tile_width: 16,
                    kind: WaveF32ReductionKindV1::Sum,
                },
                WaveWidth::Wave64,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(23), Type::F32)],
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(7),
                    tile_width: 16,
                    kind: WaveF32ReductionKindV1::Maximum,
                },
                WaveWidth::Wave64,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(24), Type::F32)],
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::BroadcastF32 {
                    value: ValueId(22),
                    source_lane: ValueId(21),
                    tile_width: 16,
                },
                WaveWidth::Wave64,
            )),
        ),
    ];
    let block = BasicBlock {
        id: BlockId(0),
        parameters: vec![],
        operations: std::mem::take(&mut operations),
        terminator: Some(Terminator::Return { values: vec![] }),
    };
    let mut function = Function::kernel_entry(
        "collective_impl",
        Signature::new(parameter_types, vec![]),
        parameters,
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();

    let mut kernel = Kernel::new(
        "collective",
        "collective_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(workgroup_x, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();

    let mut module = Module::new("tests::gfx950_collectives_and_lds_transpose");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn masked_wave64_broadcast_module(mask: Option<u32>, mask_on_left: bool) -> Module {
    let mut module = collective_and_lds_transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let mut broadcast = block.operations.pop().expect("broadcast operation");
    let mask_value = ValueId(25);
    let masked_value = ValueId(26);
    if let Some(mask) = mask {
        block.operations.push(Operation::effect_free(
            ValueDef::new(mask_value, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(mask)),
        ));
    }
    let dynamic = ValueId(13);
    let other = mask.map_or(ValueId(14), |_| mask_value);
    let (lhs, rhs) = if mask_on_left {
        (other, dynamic)
    } else {
        (dynamic, other)
    };
    block.operations.push(Operation::effect_free(
        ValueDef::new(masked_value, Type::Scalar(ScalarType::U32)),
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        },
    ));
    let OperationKind::Wave(wave) = &mut broadcast.kind else {
        panic!("expected broadcast")
    };
    let WaveOperationKind::BroadcastF32 {
        source_lane,
        tile_width,
        ..
    } = &mut wave.kind
    else {
        panic!("expected f32 broadcast")
    };
    *source_lane = masked_value;
    *tile_width = 64;
    block.operations.push(broadcast);
    module
}

fn assert_llvm_parses(llvm: &str, label: &str) {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-gfx950-collectives-{}-{label}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("module.ll");
    let output = directory.join("module.bc");
    fs::write(&input, llvm).unwrap();
    let status = Command::new("llvm-as")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .status()
        .unwrap();
    let _ = fs::remove_dir_all(directory);
    assert!(status.success(), "LLVM rejected:\n{llvm}");
}

#[test]
fn lowers_exact_fp4_and_fp8_collective_and_lds_transpose_modules() {
    for (format, bytes, intrinsic, calls, stores) in [
        (
            Gfx950LdsTransposeFormatV1::Fp4E2M1,
            1024,
            "llvm.amdgcn.ds.read.tr4.b64.v2i32",
            2,
            16,
        ),
        (
            Gfx950LdsTransposeFormatV1::Fp8E4M3,
            2048,
            "llvm.amdgcn.ds.read.tr8.b64.v2i32",
            4,
            32,
        ),
    ] {
        let module = collective_and_lds_transpose_module_with_workgroup(format, 256);
        verify_module(&module).unwrap();
        assert!(VerifiedCanonicalKernelIrV8::from_module(module.clone()).is_err());
        let anchored = lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &module,
            &KernelId::new("collective"),
            fe2o3_amdgcn_model::ProductionSemanticAnchorKirIdentityV1::from_v9(
                &VerifiedCanonicalKernelIrV9::from_module(module.clone()).unwrap(),
            ),
        )
        .unwrap();
        let operations = module.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .len();
        assert_eq!(
            anchored.matches("call void @llvm.pseudoprobe(").count(),
            operations
        );
        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
        let workgroup_bytes = bytes * 4;
        assert!(llvm.contains(&format!(
            "@__fe2o3_lds_collective_10 = internal addrspace(3) global [{workgroup_bytes} x i8] undef, align 64"
        )));
        assert!(llvm.contains("%v10.transpose.wave.i32 = lshr i32 %v10.transpose.local.i32, 6"));
        assert!(llvm.contains(&format!(
            "%v10.transpose.byte.offset.i32 = mul i32 %v10.transpose.wave.i32, {bytes}"
        )));
        assert!(llvm.contains(&format!(
            "%v10 = getelementptr [{workgroup_bytes} x i8], ptr addrspace(3) @__fe2o3_lds_collective_10, i32 0, i32 %v10.transpose.byte.offset.i32"
        )));
        assert_eq!(
            llvm.matches(&format!(" = call <2 x i32> @{intrinsic}"))
                .count(),
            calls
        );
        assert_eq!(llvm.matches("store i8 ").count(), stores);
        assert_eq!(
            llvm.matches("call void asm sideeffect \"s_barrier\", \"\"()")
                .count(),
            1
        );
        assert!(llvm.contains("fence syncscope(\"workgroup\") release"));
        assert!(llvm.contains("fence syncscope(\"workgroup\") acquire"));
        assert_eq!(llvm.matches(" = fadd float ").count(), 4);
        assert_eq!(llvm.matches(" = fcmp olt float ").count(), 4);
        assert_eq!(llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count(), 9);
        assert!(llvm.contains(".source = add i32 %v24.tile.base, 7"));
        assert!(!llvm.contains(".tile.relative = and i32 7"));
        assert!(
            llvm.contains(
                "%v11.transpose.row.inbounds.0 = icmp ult i64 %v11.transpose.row.0, %arg2"
            )
        );
        assert!(llvm.contains(
            "%v11.transpose.column.inbounds.0 = icmp ult i64 %v11.transpose.column.0, %arg3"
        ));
        assert!(llvm.contains("@llvm.umul.with.overflow.i64(i64 %v11.transpose.row.0, i64 %arg4)"));
        assert!(llvm.contains(
            "@llvm.uadd.with.overflow.i64(i64 %v11.transpose.row.offset.0, i64 %v11.transpose.column.0)"
        ));
        match format {
            Gfx950LdsTransposeFormatV1::Fp4E2M1 => {
                assert!(llvm.contains(
                    "%v11.transpose.depth.i32.1 = add i32 %v11.transpose.depth.base, 16"
                ));
                assert!(llvm.contains("%v11.transpose.row.band.i32.16 = add i32 0, 0"));
            }
            Gfx950LdsTransposeFormatV1::Fp8E4M3 => {
                assert!(llvm.contains(
                    "%v11.transpose.token.band = shl i32 %v11.transpose.token.band.bit, 3"
                ));
                assert!(llvm.contains(
                    "%v11.transpose.row.band.i32.0 = add i32 %v11.transpose.token.band, 0"
                ));
                assert!(llvm.contains(
                    "%v11.transpose.depth.i32.2 = add i32 %v11.transpose.depth.base, 64"
                ));
                assert!(llvm.contains(
                    "%v11.transpose.depth.i32.3 = add i32 %v11.transpose.depth.base, 72"
                ));
            }
        }
        assert_llvm_parses(&llvm, intrinsic);
    }
}

#[test]
fn rejects_wrong_target_invalid_collective_protocol_and_unbounded_broadcast() {
    let module = collective_and_lds_transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3);
    let wrong_target =
        lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &KernelId::new("collective"))
            .unwrap_err();
    assert!(
        wrong_target
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == LoweringDiagnosticCode::UnsupportedCapability })
    );

    for invalid_width in [0, 3, 65] {
        let mut invalid = module.clone();
        set_reduction_width(&mut invalid, 5, invalid_width);
        assert!(verify_module(&invalid).is_err());
        assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&invalid).is_err());
    }

    let mut partial_wave = module.clone();
    let OperationKind::Wave(reduction) =
        &mut partial_wave.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind
    else {
        unreachable!()
    };
    reduction.active_lanes = 32;
    assert!(verify_module(&partial_wave).is_err());
    assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&partial_wave).is_err());

    let mut unbounded = module;
    let block = &mut unbounded.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations[4].kind = OperationKind::Binary {
        op: BinaryOp::Add,
        lhs: ValueId(13),
        rhs: ValueId(14),
    };
    let error = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&unbounded).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("statically bounded tile-local source lane"),
        "{error:?}"
    );
}

#[test]
fn lowers_mask63_wave64_broadcast_and_rejects_hostile_masks() {
    for mask_on_left in [false, true] {
        let module = masked_wave64_broadcast_module(Some(63), mask_on_left);
        verify_module(&module).unwrap();
        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
        assert!(llvm.contains(".tile.base = and i32 %v24.lane, -64"));
        assert!(llvm.contains(".source = add i32 %v24.tile.base, %v26"));
    }
    for hostile in [
        masked_wave64_broadcast_module(Some(64), false),
        masked_wave64_broadcast_module(None, false),
    ] {
        let error = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&hostile).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("statically bounded tile-local source lane"),
            "{error:?}"
        );
    }
}

#[test]
fn rejected_profile_does_not_depend_on_undeclared_capabilities() {
    let mut module = collective_and_lds_transpose_module(Gfx950LdsTransposeFormatV1::Fp4E2M1);
    module.required_capabilities = BTreeSet::new();
    assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).is_err());
}

fn set_reduction_width(module: &mut Module, operation_index: usize, width: u32) {
    let OperationKind::Wave(reduction) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[operation_index].kind
    else {
        panic!("expected wave reduction")
    };
    let WaveOperationKind::ReduceF32 { tile_width, .. } = &mut reduction.kind else {
        panic!("expected f32 reduction")
    };
    *tile_width = width;
}

fn set_broadcast_width_and_zero_source(module: &mut Module, width: u32) {
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations[4].kind = OperationKind::Constant(Constant::U32(0));
    let OperationKind::Wave(broadcast) = &mut block.operations[7].kind else {
        panic!("expected wave broadcast")
    };
    let WaveOperationKind::BroadcastF32 { tile_width, .. } = &mut broadcast.kind else {
        panic!("expected f32 broadcast")
    };
    *tile_width = width;
}

#[test]
fn lowers_every_admitted_power_of_two_reduction_width() {
    for width in [1_u32, 2, 4, 8, 16, 32, 64] {
        let mut module = collective_and_lds_transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3);
        set_reduction_width(&mut module, 5, width);
        set_reduction_width(&mut module, 6, width);
        verify_module(&module).unwrap();

        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
        let stages = width.trailing_zeros() as usize;
        assert_eq!(
            llvm.matches(" = fadd float ").count(),
            stages,
            "width {width}"
        );
        assert_eq!(
            llvm.matches(" = fcmp olt float ").count(),
            stages,
            "width {width}"
        );
        assert_eq!(
            llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count(),
            stages * 2 + 1,
            "width {width}"
        );
        if width == 1 {
            assert!(llvm.contains("%v22 = select i1 true, float %arg7, float %arg7"));
            assert!(llvm.contains("%v23 = select i1 true, float %arg7, float %arg7"));
        } else {
            let last_stage = stages - 1;
            let last_distance = width / 2;
            assert!(llvm.contains(&format!(
                "%v22.source.{last_stage} = xor i32 %v22.lane, {last_distance}"
            )));
            assert!(llvm.contains(&format!(
                "%v23.source.{last_stage} = xor i32 %v23.lane, {last_distance}"
            )));
        }
        assert_llvm_parses(&llvm, &format!("reduce-width-{width}"));
    }
}

#[test]
fn lowers_every_admitted_power_of_two_broadcast_width() {
    for width in [1_u32, 2, 4, 8, 16, 32, 64] {
        let mut module = collective_and_lds_transpose_module(Gfx950LdsTransposeFormatV1::Fp8E4M3);
        set_broadcast_width_and_zero_source(&mut module, width);
        verify_module(&module).unwrap();

        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
        assert!(llvm.contains(&format!(
            "%v24.tile.base = and i32 %v24.lane, {}",
            -(width as i32)
        )));
        assert_llvm_parses(&llvm, &format!("broadcast-width-{width}"));
    }
}
