use dialect_amdgcn::{LoweringDiagnosticCode, lower_compiler_module_to_llvm_ir};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, Function, FunctionId,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, Module, Operation, OperationKind,
    Signature, TargetCapability, Terminator, Type, ValueDef, ValueId, WaveWidth, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupSize,
};

fn returning_block(operations: Vec<Operation>, values: Vec<ValueId>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values });
    block
}

fn call(result: u32, callee: &str, argument: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(result),
            Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
        ),
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: vec![ValueId(argument)],
        },
    )
}

fn void_call(callee: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: vec![],
        },
    )
}

fn void_helper(id: &str, callees: &[&str]) -> Function {
    Function::internal_helper(
        id,
        Signature::new(vec![], vec![]),
        vec![],
        vec![returning_block(
            callees.iter().map(|callee| void_call(callee)).collect(),
            vec![],
        )],
    )
}

fn void_entry(id: &str, callees: &[&str]) -> Function {
    Function::kernel_entry(
        id,
        Signature::new(vec![], vec![]),
        vec![],
        vec![returning_block(
            callees.iter().map(|callee| void_call(callee)).collect(),
            vec![],
        )],
    )
}

fn wave_kernel(id: &str, entry: &str, width: WaveWidth) -> Kernel {
    let mut kernel = kernel(id, entry, 64);
    kernel.required_capabilities.clear();
    kernel
        .required_capabilities
        .insert(TargetCapability::WaveWidth(width));
    kernel
}

fn kernel_entry(id: &str, callees: &[&str]) -> Function {
    let parameter_count = callees.len().max(1);
    let operations = callees
        .iter()
        .enumerate()
        .map(|(index, callee)| call((parameter_count + index) as u32, callee, index as u32))
        .collect();
    Function::kernel_entry(
        id,
        Signature::new(
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32); parameter_count],
            vec![],
        ),
        (0..parameter_count)
            .map(|index| ValueId(index as u32))
            .collect(),
        vec![returning_block(operations, vec![])],
    )
}

fn kernel(id: &str, entry: &str, workgroup_x: u32) -> Kernel {
    let mut kernel = Kernel::new(
        id,
        entry,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(workgroup_x, 1, 1));
    kernel
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    kernel
}

fn compiler_module() -> Module {
    let scale = Function::definition(
        "scale",
        Signature::new(
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
        ),
        vec![ValueId(0)],
        vec![returning_block(
            vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(1), Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)),
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::I32(2)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(2), Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)),
                    OperationKind::Binary {
                        op: BinaryOp::Multiply,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                ),
            ],
            vec![ValueId(2)],
        )],
    );
    let public_adjust = Function::device_ffi_export(
        "public_adjust",
        Signature::new(
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
        ),
        vec![ValueId(0)],
        vec![returning_block(
            vec![call(1, "external_bias", 0)],
            vec![ValueId(1)],
        )],
    );
    let external = Function::declaration(
        "external_bias",
        Signature::new(
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
            vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::I32)],
        ),
    );

    let mut module = Module::new("tests::compiler_module");
    module.functions = vec![
        kernel_entry("zeta_entry", &["scale", "public_adjust"]),
        external,
        scale,
        kernel_entry("alpha_entry", &["scale"]),
        public_adjust,
    ];
    module.kernels = vec![
        kernel("zeta_kernel", "zeta_entry", 128),
        kernel("alpha_kernel", "alpha_entry", 64),
    ];
    module
}

#[test]
fn multi_entry_module_matches_exact_golden() {
    let actual = lower_compiler_module_to_llvm_ir(&compiler_module()).expect("supported module");
    assert_eq!(actual, include_str!("fixtures/compiler_module_g3.ll"));
    assert_eq!(actual.matches("define internal i32 @scale").count(), 1);
    assert_eq!(actual.matches("define i32 @public_adjust").count(), 1);
    assert_eq!(actual.matches("declare i32 @external_bias").count(), 1);
    assert!(!actual.contains("@alpha_entry"));
    assert!(!actual.contains("@zeta_entry"));
    assert!(!actual.contains("target datalayout"));
    assert!(!actual.contains("target-cpu"));
    assert!(!actual.contains("code_object_version"));
}

#[test]
fn canonical_order_is_independent_of_module_vector_order() {
    let baseline = lower_compiler_module_to_llvm_ir(&compiler_module()).unwrap();
    let mut permuted = compiler_module();
    permuted.functions.reverse();
    permuted.kernels.reverse();
    assert_eq!(
        lower_compiler_module_to_llvm_ir(&permuted).unwrap(),
        baseline
    );
}

#[test]
fn pointer_declarations_and_visible_definitions_preserve_physical_types() {
    let pointer = Type::pointer(
        Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut module = compiler_module();
    module.functions.push(Function::declaration(
        "consume_pointer",
        Signature::new(vec![pointer.clone()], vec![]),
    ));
    let mut identity = Function::definition(
        "identity_pointer",
        Signature::new(vec![pointer.clone()], vec![pointer]),
        vec![ValueId(0)],
        vec![returning_block(vec![], vec![ValueId(0)])],
    );
    identity
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    module.functions.push(identity);

    let llvm = lower_compiler_module_to_llvm_ir(&module).unwrap();
    assert!(llvm.contains("declare void @consume_pointer(ptr addrspace(1))"));
    assert!(llvm.contains(
        "define internal ptr addrspace(1) @identity_pointer(ptr addrspace(1) %arg0) nounwind \"target-features\"=\"-wavefrontsize32,+wavefrontsize64\""
    ));
    assert!(llvm.contains("ret ptr addrspace(1) %arg0"));
}

#[test]
fn intrinsic_declarations_are_aggregated_once_across_kernel_entries() {
    let mut module = compiler_module();
    for entry in ["alpha_entry", "zeta_entry"] {
        let function = module
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == entry)
            .unwrap();
        function.body.as_mut().unwrap().blocks[0].operations.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(20), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
        );
    }

    let llvm = lower_compiler_module_to_llvm_ir(&module).unwrap();
    assert_eq!(
        llvm.matches("declare i32 @llvm.amdgcn.workitem.id.x")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("declare i32 @llvm.amdgcn.workgroup.id.x")
            .count(),
        1
    );
    assert!(llvm.contains("attributes #2 = { nounwind readnone speculatable willreturn }"));
}

#[test]
fn missing_kernels_and_output_namespace_collisions_fail_closed() {
    let mut missing = compiler_module();
    missing.kernels.clear();
    let error = lower_compiler_module_to_llvm_ir(&missing).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::MissingKernel));

    let mut collision = compiler_module();
    collision.kernels[0].id = "scale".into();
    let error = lower_compiler_module_to_llvm_ir(&collision).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::ConflictingSymbol));
}

#[test]
fn generated_lds_symbols_share_the_module_collision_domain() {
    let mut module = compiler_module();
    let alpha = module
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "alpha_entry")
        .unwrap();
    alpha.body.as_mut().unwrap().blocks[0].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(
                ValueId(40),
                Type::pointer(
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
        ),
    );
    module.functions.push(Function::declaration(
        "__fe2o3_lds_alpha_kernel_40",
        Signature::new(vec![], vec![]),
    ));

    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::ConflictingSymbol));
    assert!(error.to_string().contains("LDS value %40"));
}

#[test]
fn duplicate_function_names_are_rejected_before_symbol_planning() {
    let mut module = compiler_module();
    module.functions.push(Function::declaration(
        "scale",
        Signature::new(vec![], vec![]),
    ));
    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::InputVerification(
        fe2o3_kernel_ir::DiagnosticCode::DuplicateFunction,
    )));
}

#[test]
fn one_definition_cannot_back_multiple_kernel_exports() {
    let mut module = compiler_module();
    let old_entry = module.kernels[1].entry.clone();
    module.kernels[1].entry = module.kernels[0].entry.clone();
    module
        .functions
        .iter_mut()
        .find(|function| function.id == old_entry)
        .unwrap()
        .role = fe2o3_kernel_ir::FunctionRole::InternalHelper;
    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::ConflictingSymbol));
}

#[test]
fn helpers_cannot_call_kernel_entries() {
    let mut module = compiler_module();
    let scale = module
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "scale")
        .unwrap();
    let body = scale.body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "alpha_entry".into(),
                arguments: vec![ValueId(0)],
            },
        ),
    );

    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::UnsupportedOperation));
    assert!(
        error
            .to_string()
            .contains("kernel entry function alpha_entry")
    );
}

#[test]
fn unsupported_helper_abis_and_context_dependent_bodies_are_atomic_errors() {
    let mut slice = compiler_module();
    slice.functions.push(Function::declaration(
        "slice_import",
        Signature::new(
            vec![Type::slice(
                Type::F32,
                fe2o3_kernel_ir::AddressSpace::Global,
                fe2o3_kernel_ir::AccessMode::ReadOnly,
            )],
            vec![],
        ),
    ));
    let error = lower_compiler_module_to_llvm_ir(&slice).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::UnsupportedParameter));

    let mut contextual = compiler_module();
    let scale = contextual
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "scale")
        .unwrap();
    let body = scale.body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
    );
    let error = lower_compiler_module_to_llvm_ir(&contextual).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::UnsupportedOperation));
    assert!(error.to_string().contains("function scale"));
}

#[test]
fn unsafe_import_names_and_multi_value_results_are_rejected() {
    let mut unsafe_name = compiler_module();
    unsafe_name.functions.push(Function::declaration(
        "external::bias",
        Signature::new(vec![], vec![]),
    ));
    let error = lower_compiler_module_to_llvm_ir(&unsafe_name).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::UnsafeSymbolName));

    let mut multiple = compiler_module();
    multiple.functions.push(Function::declaration(
        "pair_import",
        Signature::new(
            vec![],
            vec![
                Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
                Type::Scalar(fe2o3_kernel_ir::ScalarType::I32),
            ],
        ),
    ));
    let error = lower_compiler_module_to_llvm_ir(&multiple).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::UnsupportedResults));
}

#[test]
fn mixed_wave_kernels_cannot_share_a_direct_helper() {
    let mut module = Module::new("tests::mixed_direct");
    module.functions = vec![
        void_entry("wave32_entry", &["shared"]),
        void_entry("wave64_entry", &["shared"]),
        void_helper("shared", &[]),
    ];
    module.kernels = vec![
        wave_kernel("wave32_kernel", "wave32_entry", WaveWidth::Wave32),
        wave_kernel("wave64_kernel", "wave64_entry", WaveWidth::Wave64),
    ];

    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::IncompatibleWaveCallGraph));
    assert!(error.to_string().contains("helper SCC [shared]"));
    assert!(error.to_string().contains("Wave32"));
    assert!(error.to_string().contains("Wave64"));
}

#[test]
fn mixed_wave_kernels_cannot_enter_different_nodes_of_one_recursive_scc() {
    let mut module = Module::new("tests::mixed_recursive");
    module.functions = vec![
        void_entry("wave32_entry", &["recursive_a"]),
        void_entry("wave64_entry", &["recursive_b"]),
        void_helper("recursive_a", &["recursive_b"]),
        void_helper("recursive_b", &["recursive_a"]),
    ];
    module.kernels = vec![
        wave_kernel("wave32_kernel", "wave32_entry", WaveWidth::Wave32),
        wave_kernel("wave64_kernel", "wave64_entry", WaveWidth::Wave64),
    ];

    let first = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    let second = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert_eq!(first, second);
    assert!(first.contains(LoweringDiagnosticCode::IncompatibleWaveCallGraph));
    assert!(first.to_string().contains("recursive_a, recursive_b"));
}

fn effective_wave_module() -> Module {
    let i32_type = Type::Scalar(fe2o3_kernel_ir::ScalarType::I32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), i32_type.clone()),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::I32(11)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), i32_type.clone()),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::I32(22)),
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(1)],
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(2)],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge
        .parameters
        .push(ValueDef::new(ValueId(3), i32_type.clone()));
    merge.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let branching = Function::internal_helper(
        "branching",
        Signature::new(vec![Type::BOOL], vec![i32_type]),
        vec![ValueId(0)],
        vec![entry, then_block, else_block, merge],
    );
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![returning_block(
            vec![call(1, "branching", 0), void_call("recursive_a")],
            vec![],
        )],
    );
    let mut module = Module::new("tests::effective_wave");
    module.functions = vec![
        entry,
        branching,
        void_helper("recursive_a", &["recursive_b"]),
        void_helper("recursive_b", &["recursive_a"]),
    ];
    module.kernels = vec![wave_kernel("kernel", "entry", WaveWidth::Wave32)];
    module
}

#[test]
fn inherited_wave_mode_reaches_branch_phi_helpers_and_recursive_sccs() {
    let module = effective_wave_module();
    let llvm = lower_compiler_module_to_llvm_ir(&module).unwrap();
    let wave32 = "\"target-features\"=\"+wavefrontsize32,-wavefrontsize64\"";
    assert!(llvm.contains(&format!(
        "define internal i32 @branching(i1 %arg0) nounwind {wave32}"
    )));
    assert!(llvm.contains(&format!(
        "define internal void @recursive_a() nounwind {wave32}"
    )));
    assert!(llvm.contains(&format!(
        "define internal void @recursive_b() nounwind {wave32}"
    )));
    assert!(llvm.contains("%v3 = phi i32 [ 11, %bb1 ], [ 22, %bb2 ]"));
    assert!(llvm.contains(&format!(
        "attributes #0 = {{ nounwind \"amdgpu-flat-work-group-size\"=\"64,64\" {wave32} }}"
    )));
}

#[test]
fn unreachable_helper_roots_require_an_explicit_mode() {
    let mut module = Module::new("tests::unreachable_helper");
    module.functions = vec![void_entry("entry", &[]), void_helper("orphan", &[])];
    module.kernels = vec![wave_kernel("kernel", "entry", WaveWidth::Wave64)];

    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::MissingWaveWidth));
    assert!(error.to_string().contains("helper SCC [orphan]"));

    module.functions[1]
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    let llvm = lower_compiler_module_to_llvm_ir(&module).unwrap();
    assert!(llvm.contains(
        "define internal void @orphan() nounwind \"target-features\"=\"-wavefrontsize32,+wavefrontsize64\""
    ));
}

#[test]
fn call_graph_edge_limit_fails_before_scc_allocation_can_grow_unbounded() {
    const OVER_LIMIT: usize = 131_073;
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![returning_block(
            (0..OVER_LIMIT).map(|_| void_call("external")).collect(),
            vec![],
        )],
    );
    let mut module = Module::new("tests::call_graph_limit");
    module.functions = vec![
        entry,
        Function::external_import("external", Signature::new(vec![], vec![])),
    ];
    module.kernels = vec![wave_kernel("kernel", "entry", WaveWidth::Wave64)];

    let error = lower_compiler_module_to_llvm_ir(&module).unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::ResourceLimit));
    assert!(
        error
            .to_string()
            .contains("compiler-module call edges count 131073 exceeds limit 131072")
    );
}

#[test]
#[ignore = "requires clang and LLVM tools with gfx1151 support"]
fn gfx1151_object_symbols_and_effective_wave_helpers_are_physical() {
    use std::ffi::OsString;
    use std::process::Command;

    let clang = std::env::var_os("CLANG").unwrap_or_else(|| OsString::from("clang"));
    let readelf =
        std::env::var_os("LLVM_READELF").unwrap_or_else(|| OsString::from("llvm-readelf"));
    let objdump =
        std::env::var_os("LLVM_OBJDUMP").unwrap_or_else(|| OsString::from("llvm-objdump"));
    let directory = std::env::temp_dir();
    let suffix = std::process::id();
    let role_ir = directory.join(format!("fe2o3-g3-roles-{suffix}.ll"));
    let role_object = directory.join(format!("fe2o3-g3-roles-{suffix}.o"));
    let wave_ir = directory.join(format!("fe2o3-g3-wave-{suffix}.ll"));
    let wave_object = directory.join(format!("fe2o3-g3-wave-{suffix}.o"));

    let compile = |ir: &std::path::Path, object: &std::path::Path| {
        let output = Command::new(&clang)
            .args([
                "-x",
                "ir",
                "--target=amdgcn-amd-amdhsa",
                "-mcpu=gfx1151",
                "-nogpulib",
                "-c",
            ])
            .arg(ir)
            .arg("-o")
            .arg(object)
            .output()
            .expect("run clang");
        assert!(
            output.status.success(),
            "clang failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    std::fs::write(
        &role_ir,
        lower_compiler_module_to_llvm_ir(&compiler_module()).unwrap(),
    )
    .unwrap();
    compile(&role_ir, &role_object);
    let symbols = Command::new(&readelf)
        .arg("-s")
        .arg(&role_object)
        .output()
        .expect("run llvm-readelf");
    assert!(symbols.status.success());
    let symbols = String::from_utf8(symbols.stdout).unwrap();
    let symbol_line = |name: &str| {
        symbols
            .lines()
            .find(|line| line.split_whitespace().last() == Some(name))
            .unwrap_or_else(|| panic!("missing symbol {name}"))
    };
    assert!(symbol_line("scale").contains("FUNC    LOCAL  DEFAULT"));
    assert!(symbol_line("public_adjust").contains("FUNC    GLOBAL DEFAULT"));
    assert!(symbol_line("alpha_kernel").contains("FUNC    GLOBAL PROTECTED"));
    assert!(symbol_line("zeta_kernel").contains("FUNC    GLOBAL PROTECTED"));
    assert!(symbol_line("external_bias").contains("GLOBAL DEFAULT   UND"));

    std::fs::write(
        &wave_ir,
        lower_compiler_module_to_llvm_ir(&effective_wave_module()).unwrap(),
    )
    .unwrap();
    compile(&wave_ir, &wave_object);
    let disassembly = Command::new(&objdump)
        .args(["-d", "--mcpu=gfx1151"])
        .arg(&wave_object)
        .output()
        .expect("run llvm-objdump");
    assert!(disassembly.status.success());
    let disassembly = String::from_utf8(disassembly.stdout).unwrap();
    assert!(disassembly.contains("file format elf64-amdgpu"));
    assert!(disassembly.contains("<kernel>"));
    assert!(disassembly.contains("<branching>"));
    assert!(disassembly.contains("<recursive_a>"));
    assert!(disassembly.contains("<recursive_b>"));
    assert!(disassembly.contains("s_endpgm"));

    for path in [role_ir, role_object, wave_ir, wave_object] {
        let _ = std::fs::remove_file(path);
    }
}
