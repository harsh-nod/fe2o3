use fe2o3_amdgcn_model::{
    lower_compiler_module_to_gfx942_llvm_ir, lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn realtime_module() -> Module {
    let diagnostic = AmdGpuDiagnosticOperation::Realtime64;
    let mut block = BasicBlock::new(BlockId(0));
    block
        .operations
        .push(diagnostic.operation(Some(ValueId(0))));
    block
        .operations
        .push(diagnostic.operation(Some(ValueId(1))));
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    let mut function = Function::kernel_entry(
        "realtime_impl",
        Signature::new(Vec::new(), Vec::new()),
        Vec::new(),
        vec![block],
    );
    function.required_capabilities = diagnostic.required_capabilities();
    let mut kernel = Kernel::new(
        "realtime_kernel",
        "realtime_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = diagnostic.required_capabilities();
    let mut module = Module::new("tests::gfx950_realtime64");
    module.functions.push(function);
    module.functions.push(diagnostic.declaration());
    module.required_capabilities = diagnostic.required_capabilities();
    module.kernels.push(kernel);
    module
}

#[test]
fn realtime64_retains_two_full_width_observations() {
    let module = realtime_module();
    let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&module).unwrap();
    assert_eq!(
        llvm.matches("declare i64 @llvm.amdgcn.s.memrealtime()")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("call i64 @llvm.amdgcn.s.memrealtime()")
            .count(),
        2
    );
    assert!(!llvm.contains("trunc i64"));
    assert!(!llvm.contains("speculatable"));
    let diagnostic = AmdGpuDiagnosticOperation::Realtime64;
    assert!(diagnostic.is_nondeterministic_observation());
    assert!(!diagnostic.is_terminating());
    assert_eq!(
        diagnostic.result_type(),
        Some(Type::Scalar(ScalarType::U64))
    );
    assert!(
        diagnostic
            .operation(Some(ValueId(0)))
            .memory_effects()
            .is_empty()
    );
    let effect = diagnostic
        .operation(Some(ValueId(0)))
        .combined_effect_summary_v12();
    assert!(effect.memory().is_pure());
    assert!(effect.compiler_ordering().has_ordered_execution());
    assert!(!effect.is_pure());
    let summaries = analyze_interprocedural_effects_v1(&module).unwrap();
    let summary = summaries
        .function(&FunctionId::new("realtime_impl"))
        .unwrap();
    assert!(summary.is_complete());
    assert!(!summary.is_complete_and_pure());
}

#[test]
fn realtime64_rejects_wrong_target_arguments_and_result_width() {
    let module = realtime_module();
    assert!(lower_compiler_module_to_gfx942_llvm_ir(&module).is_err());
    let mut wrong_width = module.clone();
    wrong_width.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
        Type::Scalar(ScalarType::U32);
    assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&wrong_width).is_err());
    let mut wrong_arguments = module;
    let OperationKind::Call { arguments, .. } =
        &mut wrong_arguments.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments.push(ValueId(0));
    assert!(lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&wrong_arguments).is_err());
}

#[test]
fn realtime64_ordering_propagates_through_an_internal_call() {
    let mut module = realtime_module();
    module.functions[0].role = FunctionRole::InternalHelper;
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::Call {
            callee: FunctionId::new("realtime_impl"),
            arguments: Vec::new(),
        },
    ));
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    let mut entry = Function::kernel_entry(
        "caller",
        Signature::new(Vec::new(), Vec::new()),
        Vec::new(),
        vec![block],
    );
    entry.required_capabilities = AmdGpuDiagnosticOperation::Realtime64.required_capabilities();
    module.functions.push(entry);
    module.kernels[0].entry = FunctionId::new("caller");
    let summaries = analyze_interprocedural_effects_v1(&module).unwrap();
    let caller = summaries.function(&FunctionId::new("caller")).unwrap();
    assert!(caller.is_complete());
    assert!(caller.summary().memory().is_pure());
    assert!(caller.summary().compiler_ordering().has_ordered_execution());
    assert!(!caller.is_complete_and_pure());
}

#[test]
#[ignore = "requires explicit FE2O3_OPT and FE2O3_LLC with gfx950 support"]
fn realtime64_survives_llvm_o2_and_emits_two_native_counter_reads() {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT");
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("fe2o3-realtime64-{}-{nonce}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("input.ll");
    let optimized = directory.join("optimized.ll");
    let assembly = directory.join("output.s");
    fs::write(
        &input,
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&realtime_module()).unwrap(),
    )
    .unwrap();
    let output = Command::new(opt)
        .args(["-S", "-passes=default<O2>", "-verify-each"])
        .arg(&input)
        .arg("-o")
        .arg(&optimized)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let llvm = fs::read_to_string(&optimized).unwrap();
    assert_eq!(
        llvm.matches("call i64 @llvm.amdgcn.s.memrealtime()")
            .count(),
        2,
        "{llvm}"
    );
    let output = Command::new(llc)
        .args([
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu=gfx950",
            "-filetype=asm",
        ])
        .arg(&optimized)
        .arg("-o")
        .arg(&assembly)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let isa = fs::read_to_string(&assembly).unwrap();
    assert_eq!(isa.matches("s_memrealtime").count(), 2, "{isa}");
    fs::remove_dir_all(directory).unwrap();
}
