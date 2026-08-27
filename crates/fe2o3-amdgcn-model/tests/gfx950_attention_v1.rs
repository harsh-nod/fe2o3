use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn attention_module(format: Gfx950LdsTransposeFormatV1) -> Module {
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
        "attention_impl",
        Signature::new(parameter_types, vec![]),
        parameters,
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();

    let mut kernel = Kernel::new(
        "attention",
        "attention_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();

    let mut module = Module::new("tests::gfx950_attention");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn assert_llvm_parses(llvm: &str, label: &str) {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-gfx950-attention-{}-{label}",
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
fn lowers_exact_fp4_and_fp8_attention_modules() {
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
        let module = attention_module(format);
        verify_module(&module).unwrap();
        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
        assert!(llvm.contains(&format!(
            "@__fe2o3_lds_attention_10 = internal addrspace(3) global [{bytes} x i8] undef, align 64"
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
fn rejects_wrong_target_collective_width_and_unbounded_broadcast() {
    let module = attention_module(Gfx950LdsTransposeFormatV1::Fp8E4M3);
    let wrong_target =
        lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &KernelId::new("attention"))
            .unwrap_err();
    assert!(
        wrong_target
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == LoweringDiagnosticCode::UnsupportedCapability })
    );

    let mut wrong_width = module.clone();
    let OperationKind::Wave(reduction) =
        &mut wrong_width.functions[0].body.as_mut().unwrap().blocks[0].operations[5].kind
    else {
        unreachable!()
    };
    let WaveOperationKind::ReduceF32 { tile_width, .. } = &mut reduction.kind else {
        unreachable!()
    };
    *tile_width = 8;
    let error = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&wrong_width).unwrap_err();
    assert!(
        error.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == LoweringDiagnosticCode::UnsupportedWaveOperation
        }),
        "{error:?}"
    );

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
fn rejected_profile_does_not_depend_on_undeclared_capabilities() {
    let mut module = attention_module(Gfx950LdsTransposeFormatV1::Fp4E2M1);
    module.required_capabilities = BTreeSet::new();
    assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).is_err());
}
