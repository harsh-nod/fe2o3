use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir, lower_kernel_to_gfx942_llvm_ir,
    lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn cast_module(destinations: &[ScalarType]) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    for (index, to) in destinations.iter().copied().enumerate() {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(index as u32 + 1), Type::Scalar(to)),
            OperationKind::Cast {
                kind: CastKind::FloatToInteger,
                value: ValueId(0),
                to: Type::Scalar(to),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::float-casts");
    module.functions.push(Function::kernel_entry(
        "cast_entry",
        Signature::new(vec![Type::Scalar(ScalarType::F32)], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "cast_kernel",
        "cast_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

#[test]
fn all_supported_float_to_integer_casts_use_saturating_intrinsics() {
    let module = cast_module(&[
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::U64,
    ]);
    let kernel = KernelId::new("cast_kernel");
    let mut gfx950 = module.clone();
    for capability in [
        gfx950_xnack_minus_target_capability(),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ] {
        gfx950.required_capabilities.insert(capability.clone());
        gfx950.functions[0]
            .required_capabilities
            .insert(capability.clone());
        gfx950.kernels[0].required_capabilities.insert(capability);
    }
    for llvm in [
        lower_kernel_to_llvm_ir(&module, &kernel).unwrap(),
        lower_kernel_to_gfx942_llvm_ir(&module, &kernel).unwrap(),
        lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap(),
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&gfx950).unwrap(),
    ] {
        assert!(!llvm.contains(" = fptosi "));
        assert!(!llvm.contains(" = fptoui "));
        for signedness in ['s', 'u'] {
            for width in [8, 16, 32, 64] {
                let intrinsic = format!("llvm.fpto{signedness}i.sat.i{width}.f32");
                assert_eq!(
                    llvm.matches(&format!("declare i{width} @{intrinsic}(float)"))
                        .count(),
                    1,
                    "{intrinsic}: {llvm}"
                );
                assert_eq!(
                    llvm.matches(&format!("call i{width} @{intrinsic}(float "))
                        .count(),
                    if signedness == 'u' && width == 64 {
                        2
                    } else {
                        1
                    },
                    "{intrinsic}: {llvm}"
                );
            }
        }
    }
}

#[test]
fn float_to_integer_lowering_retains_unsupported_width_diagnostics() {
    for to in [ScalarType::I128, ScalarType::U128] {
        let error = lower_kernel_to_llvm_ir(&cast_module(&[to]), &KernelId::new("cast_kernel"))
            .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnsupportedCast));
    }
}
