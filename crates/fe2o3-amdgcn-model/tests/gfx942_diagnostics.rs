use std::collections::BTreeSet;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir,
    lower_compiler_module_to_llvm_ir, lower_kernel_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}

fn diagnostic_capability() -> TargetCapability {
    TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    }
}

fn constant(block: &mut BasicBlock, id: u32, value: u32) {
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(id), u32_type()),
        OperationKind::Constant(Constant::U32(value)),
    ));
}

fn diagnostic_module() -> Module {
    let mut diagnostic_block = BasicBlock::new(BlockId(0));
    constant(&mut diagnostic_block, 0, 0x1234_5678);
    constant(&mut diagnostic_block, 1, 11);
    constant(&mut diagnostic_block, 2, 22);
    constant(&mut diagnostic_block, 3, 73);
    constant(&mut diagnostic_block, 4, 0x7654_3210);
    constant(&mut diagnostic_block, 5, 41);

    let operations = [
        AmdGpuDiagnosticOperation::Clock32,
        AmdGpuDiagnosticOperation::Print {
            format_id: ValueId(0),
            arguments: vec![ValueId(1), ValueId(2)],
        },
        AmdGpuDiagnosticOperation::ProfilingMarker { marker: ValueId(3) },
        AmdGpuDiagnosticOperation::DebugTrap,
        AmdGpuDiagnosticOperation::AssertFail {
            site_id: ValueId(4),
            line: ValueId(5),
        },
        AmdGpuDiagnosticOperation::Trap,
    ];
    diagnostic_block
        .operations
        .push(operations[0].operation(Some(ValueId(6))));
    for operation in &operations[1..5] {
        diagnostic_block.operations.push(operation.operation(None));
    }
    diagnostic_block.terminator = Some(Terminator::Unreachable);

    let mut trap_block = BasicBlock::new(BlockId(1));
    trap_block.operations.push(operations[5].operation(None));
    trap_block.terminator = Some(Terminator::Unreachable);

    let function = Function::kernel_entry(
        "diagnostic_impl",
        Signature::new(Vec::new(), Vec::new()),
        Vec::new(),
        vec![diagnostic_block, trap_block],
    );
    let mut kernel = Kernel::new(
        "diagnostic_kernel",
        "diagnostic_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::gfx942_diagnostics");
    module.functions.push(function);
    let mut declarations = BTreeSet::new();
    for operation in operations {
        let declaration = operation.declaration();
        if declarations.insert(declaration.id.clone()) {
            module.functions.push(declaration);
        }
    }
    module.required_capabilities.insert(diagnostic_capability());
    module.kernels.push(kernel);
    module
}

fn diagnostic_call_mut(
    module: &mut Module,
    predicate: impl Fn(&AmdGpuDiagnosticOperation) -> bool,
) -> &mut OperationKind {
    &mut module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| block.operations.iter_mut())
        .find(|operation| {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                return false;
            };
            AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                .is_some_and(|diagnostic| predicate(&diagnostic))
        })
        .unwrap()
        .kind
}

#[test]
fn gfx942_emits_bounded_nonconvergent_diagnostic_ir() {
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&diagnostic_module()).unwrap();
    assert_eq!(
        llvm.matches("declare i64 @llvm.amdgcn.s.memrealtime()")
            .count(),
        1
    );
    assert_eq!(llvm.matches("declare void @llvm.trap()").count(), 1);
    assert_eq!(llvm.matches("declare void @llvm.debugtrap()").count(), 1);
    assert!(llvm.contains("call i64 @llvm.amdgcn.s.memrealtime()"));
    assert!(llvm.contains("trunc i64 %v6.i64 to i32"));
    assert!(llvm.contains("asm sideeffect \"s_nop 22136\", \"\"()"));
    assert!(llvm.contains("asm sideeffect \"s_nop 4660\", \"\"()"));
    assert!(llvm.contains("asm sideeffect \"s_nop 73\", \"\"()"));
    assert_eq!(
        llvm.matches("asm sideeffect \"s_nop 0\", \"v\"(i32")
            .count(),
        2
    );
    assert_eq!(llvm.matches("call void @llvm.trap()").count(), 2);
    assert!(llvm.contains("call void @llvm.debugtrap()"));
    assert!(!llvm.contains("convergent"));
    assert!(!llvm.contains("__fe2o3_ir_amdgpu_diagnostics"));
}

#[test]
fn single_kernel_lowering_declares_trap_and_terminates_without_fallthrough() {
    let module = diagnostic_module();
    let llvm = lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id).unwrap();
    assert_eq!(llvm.matches("declare void @llvm.trap()").count(), 1);
    assert_eq!(llvm.matches("call void @llvm.trap()").count(), 2);
    assert_eq!(
        llvm.matches("call void @llvm.trap()\n  unreachable")
            .count(),
        2
    );
    assert!(!llvm.contains("call void @llvm.trap()\n  ret"));
    assert!(!llvm.contains("call void @llvm.trap()\n  br"));
}

#[test]
fn assert_fail_fallthrough_is_rejected_before_llvm_emission() {
    let mut module = diagnostic_module();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Return { values: vec![] });
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
            ))
    );
}

#[test]
fn baseline_missing_capability_and_dynamic_metadata_fail_closed() {
    assert!(
        lower_compiler_module_to_llvm_ir(&diagnostic_module())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );

    let mut missing = diagnostic_module();
    missing.required_capabilities.clear();
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&missing)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );

    let mut dynamic_format = diagnostic_module();
    let OperationKind::Call { arguments, .. } =
        diagnostic_call_mut(&mut dynamic_format, |operation| {
            matches!(operation, AmdGpuDiagnosticOperation::Print { .. })
        })
    else {
        unreachable!()
    };
    arguments[0] = ValueId(6);
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&dynamic_format)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedDiagnosticOperation)
    );

    let mut oversized_marker = diagnostic_module();
    let OperationKind::Constant(Constant::U32(marker)) =
        &mut oversized_marker.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *marker = 65_536;
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&oversized_marker)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedDiagnosticOperation)
    );
}

#[test]
#[ignore = "requires ROCm LLVM with gfx942 support"]
fn rocm_llc_compiles_bounded_diagnostics_without_linking() {
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-gfx942-diagnostics-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("diagnostics.ll");
    let output = directory.join("diagnostics.o");
    fs::write(
        &input,
        lower_compiler_module_to_gfx942_llvm_ir(&diagnostic_module()).unwrap(),
    )
    .unwrap();
    let result = Command::new(llc)
        .args([
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-filetype=obj",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&directory);
    assert!(
        result.status.success(),
        "llc rejected diagnostics: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
