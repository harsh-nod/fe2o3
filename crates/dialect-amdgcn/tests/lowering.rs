use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::{LoweringDiagnosticCode, lower_kernel_to_llvm_ir};
use fe2o3_kernel_ir::*;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-amdgcn-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical_test_target(text: &str) -> Result<&str, String> {
    let mut components = text.split(':');
    let processor = components.next().unwrap_or_default();
    let suffix = processor.strip_prefix("gfx").unwrap_or_default();
    if suffix.len() < 3 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid AMDGPU processor {processor:?}"));
    }

    let mut last_order = 0;
    for feature in components {
        let order = match feature {
            "sramecc+" | "sramecc-" => 1,
            "xnack+" | "xnack-" => 2,
            _ => return Err(format!("invalid AMDGPU target feature {feature:?}")),
        };
        if order <= last_order {
            return Err(format!("non-canonical AMDGPU target feature {feature:?}"));
        }
        last_order = order;
    }
    Ok(processor)
}

fn assert_occurrences(text: &str, needle: &str, expected: usize) {
    assert_eq!(
        text.matches(needle).count(),
        expected,
        "expected {expected} occurrence(s) of {needle:?} in:\n{text}"
    );
}

fn symbol_blocks(report: &str) -> Vec<&str> {
    report.split("\n  Symbol {\n").skip(1).collect()
}

fn symbol_block<'a>(report: &'a str, name: &str) -> &'a str {
    let needle = format!("    Name: {name} (");
    let matches = symbol_blocks(report)
        .into_iter()
        .filter(|block| block.contains(&needle))
        .collect::<Vec<_>>();
    let [block] = matches.as_slice() else {
        panic!(
            "expected exactly one symbol named {name:?}, found {}",
            matches.len()
        )
    };
    block
}

fn metadata(report: &str) -> &str {
    let marker = "        AMDGPU Metadata: ---\n";
    let matches = report.split(marker).skip(1).collect::<Vec<_>>();
    let [metadata] = matches.as_slice() else {
        panic!(
            "expected exactly one decoded AMDGPU metadata note, found {}",
            matches.len()
        )
    };
    metadata
        .split("\n...\n")
        .next()
        .expect("metadata terminator must follow the decoded note")
}

fn assert_metadata_argument(
    metadata: &str,
    name: &str,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) {
    let name_line = format!(".name:           {name}");
    let blocks = metadata
        .split("\n      - ")
        .filter(|block| block.contains(&name_line))
        .collect::<Vec<_>>();
    let [block] = blocks.as_slice() else {
        panic!(
            "expected exactly one metadata argument named {name:?}, found {}",
            blocks.len()
        )
    };
    assert!(block.contains(&format!("        .offset:         {offset}\n")));
    assert!(block.contains(&format!("        .size:           {size}\n")));
    assert!(block.contains(&format!("        .value_kind:     {value_kind}")));
    match address_space {
        Some(space) => assert!(block.contains(&format!(".address_space:  {space}\n"))),
        None => assert!(!block.contains(".address_space:")),
    }
}

fn global_slice(access: AccessMode) -> Type {
    Type::slice(Type::F32, AddressSpace::Global, access)
}

fn global_pointer(access: AccessMode) -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, access)
}

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn fill_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });

    let mut body = BasicBlock::new(BlockId(1));
    body.operations = vec![
        op(
            5,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            6,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    body.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });

    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });

    let function = Function::definition(
        "fill_impl",
        Signature::new(vec![global_slice(AccessMode::ReadWrite), Type::F32], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, body, exit],
    );
    let mut kernel = Kernel::new(
        "fill",
        "fill_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::fill");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn first_code(module: &Module, kernel: &str) -> LoweringDiagnosticCode {
    lower_kernel_to_llvm_ir(module, &KernelId::new(kernel))
        .unwrap_err()
        .diagnostics()[0]
        .code
}

#[test]
fn dynamic_1d_fill_matches_the_exact_golden() {
    let output = lower_kernel_to_llvm_ir(&fill_module(), &KernelId::new("fill")).unwrap();
    assert_eq!(output, include_str!("fixtures/fill_g1.ll"));
    assert!(output.contains("mul i64 %v2.group, 64"));
    assert!(!output.contains("256"));
    assert!(!output.contains("getelementptr inbounds"));
}

#[test]
fn static_and_dynamic_1d_extents_lower_identically() {
    let dynamic = fill_module();
    let mut static_extent = dynamic.clone();
    static_extent.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(4096),
    };

    assert_eq!(
        lower_kernel_to_llvm_ir(&dynamic, &KernelId::new("fill")).unwrap(),
        lower_kernel_to_llvm_ir(&static_extent, &KernelId::new("fill")).unwrap()
    );
}

#[test]
fn lowering_is_deterministic() {
    let module = fill_module();
    let outputs = (0..32)
        .map(|_| lower_kernel_to_llvm_ir(&module, &KernelId::new("fill")).unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(outputs.len(), 1);
}

#[test]
fn extended_subset_lowers_constants_casts_arithmetic_loads_and_volatile_access() {
    let mut module = fill_module();
    let function = &mut module.functions[0];
    let blocks = &mut function.body.as_mut().unwrap().blocks;
    blocks[0].operations.insert(
        1,
        op(
            7,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(0)),
        ),
    );
    blocks[0].operations.insert(
        2,
        op(
            8,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(7),
                to: Type::INDEX,
            },
        ),
    );
    blocks[0].operations.insert(
        3,
        op(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
    );
    if let OperationKind::Compare { lhs, .. } = &mut blocks[0].operations[5].kind {
        *lhs = ValueId(9);
    }
    if let OperationKind::GetElementPointer { offset, .. } = &mut blocks[1].operations[1].kind {
        *offset = ValueId(9);
    }
    let mut volatile_load = MemoryAccess::new(AddressSpace::Global, 8);
    volatile_load.volatile = true;
    blocks[1].operations.insert(
        2,
        op(
            10,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(6),
                access: volatile_load,
            },
        ),
    );
    blocks[1].operations.insert(
        3,
        op(
            11,
            Type::F32,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(1),
            },
        ),
    );
    let OperationKind::Store { value, access, .. } = &mut blocks[1].operations[4].kind else {
        panic!("store expected")
    };
    *value = ValueId(11);
    let mut volatile_store = MemoryAccess::new(AddressSpace::Global, 16);
    volatile_store.volatile = true;
    *access = volatile_store;
    let mut dead = BasicBlock::new(BlockId(3));
    dead.terminator = Some(Terminator::Unreachable);
    blocks.push(dead);

    let output = lower_kernel_to_llvm_ir(&module, &KernelId::new("fill")).unwrap();
    assert!(output.contains("%v8 = zext i32 0 to i64"));
    assert!(output.contains("%v9 = add i64 %v2, %v8"));
    assert!(output.contains("load volatile float, ptr addrspace(1) %v6, align 8"));
    assert!(output.contains("store volatile float %v11, ptr addrspace(1) %v6, align 16"));
    assert!(output.contains("bb3:\n  unreachable"));
}

#[test]
fn verifier_runs_before_lowering_and_ambiguous_ids_fail_closed() {
    let mut malformed = fill_module();
    let OperationKind::Compare { lhs, .. } =
        &mut malformed.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("compare expected")
    };
    *lhs = ValueId(999);
    assert_eq!(
        first_code(&malformed, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::UndefinedValue)
    );

    let mut ambiguous = fill_module();
    ambiguous.kernels.push(ambiguous.kernels[0].clone());
    assert_eq!(
        first_code(&ambiguous, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::DuplicateKernel)
    );
}

#[test]
fn kernel_selection_and_symbol_names_are_fail_closed() {
    let module = fill_module();
    assert_eq!(
        first_code(&module, "missing"),
        LoweringDiagnosticCode::MissingKernel
    );

    let mut unsafe_name = fill_module();
    unsafe_name.kernels[0].id = KernelId::new("fill\nret_void");
    assert_eq!(
        first_code(&unsafe_name, "fill\nret_void"),
        LoweringDiagnosticCode::UnsafeSymbolName
    );
}

#[test]
fn kernel_selection_requires_an_exact_identity() {
    let mut module = fill_module();
    module.kernels[0].id = KernelId::new("fill_extra");

    for alias in ["fill", "fill_impl", "extra"] {
        assert_eq!(
            first_code(&module, alias),
            LoweringDiagnosticCode::MissingKernel
        );
    }
    let output = lower_kernel_to_llvm_ir(&module, &KernelId::new("fill_extra")).unwrap();
    assert!(output.contains("define amdgpu_kernel void @fill_extra("));
    assert!(!output.contains("define amdgpu_kernel void @fill("));
}

#[test]
fn only_rank_is_restricted_while_workgroup_size_is_mandatory() {
    let mut missing_size = fill_module();
    missing_size.kernels[0].workgroup_size = None;
    assert_eq!(
        first_code(&missing_size, "fill"),
        LoweringDiagnosticCode::MissingWorkgroupSize
    );

    for domain in [
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(1),
        },
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(1),
            z: LaunchExtent::Static(1),
        },
    ] {
        let mut module = fill_module();
        module.kernels[0].domain = domain;
        assert_eq!(
            first_code(&module, "fill"),
            LoweringDiagnosticCode::UnsupportedLaunchDomain
        );
    }
}

#[test]
fn invalid_or_unbounded_workgroup_geometry_is_rejected() {
    let mut zero = fill_module();
    zero.kernels[0].workgroup_size = Some(WorkgroupSize::new(0, 1, 1));
    assert_eq!(
        first_code(&zero, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::InvalidWorkgroupSize)
    );

    let mut zero_extent = fill_module();
    zero_extent.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(0),
    };
    assert_eq!(
        first_code(&zero_extent, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::InvalidLaunchDomain)
    );

    let mut oversized = fill_module();
    oversized.kernels[0].workgroup_size = Some(WorkgroupSize::new(1025, 1, 1));
    assert_eq!(
        first_code(&oversized, "fill"),
        LoweringDiagnosticCode::UnsupportedWorkgroupSize
    );

    for size in [WorkgroupSize::new(64, 2, 1), WorkgroupSize::new(64, 1, 2)] {
        let mut inactive_axis = fill_module();
        inactive_axis.kernels[0].workgroup_size = Some(size);
        assert_eq!(
            first_code(&inactive_axis, "fill"),
            LoweringDiagnosticCode::InputVerification(DiagnosticCode::InvalidWorkgroupSize)
        );
    }
}

#[test]
fn declarations_and_kernel_results_are_rejected_by_input_verification() {
    let mut declaration = fill_module();
    declaration.functions[0].body = None;
    assert_eq!(
        first_code(&declaration, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::KernelEntryDeclaration)
    );

    let mut result = fill_module();
    result.functions[0].signature.results.push(Type::F32);
    result.functions[0].body.as_mut().unwrap().blocks[2].terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    assert_eq!(
        first_code(&result, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::KernelReturnsValue)
    );
}

#[test]
fn every_capability_owner_is_rejected_at_its_location() {
    for owner in 0..3 {
        let mut module = fill_module();
        match owner {
            0 => {
                module
                    .required_capabilities
                    .insert(TargetCapability::Float64);
            }
            1 => {
                module.kernels[0]
                    .required_capabilities
                    .insert(TargetCapability::Float64);
            }
            _ => {
                module.functions[0]
                    .required_capabilities
                    .insert(TargetCapability::Float64);
            }
        }
        let errors = lower_kernel_to_llvm_ir(&module, &KernelId::new("fill")).unwrap_err();
        assert_eq!(
            errors.diagnostics()[0].code,
            LoweringDiagnosticCode::UnsupportedCapability
        );
        assert_eq!(
            errors.diagnostics()[0].location.function.is_some(),
            owner == 2
        );
        assert_eq!(
            errors.diagnostics()[0].location.kernel.is_some(),
            owner != 0
        );
    }
}

#[test]
fn unsupported_parameter_types_and_address_spaces_are_rejected() {
    let cases = [
        (Type::F64, LoweringDiagnosticCode::UnsupportedType),
        (
            Type::slice(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
            LoweringDiagnosticCode::UnsupportedAddressSpace,
        ),
        (
            global_pointer(AccessMode::ReadWrite),
            LoweringDiagnosticCode::UnsupportedParameter,
        ),
        (Type::Unit, LoweringDiagnosticCode::UnsupportedParameter),
    ];
    for (parameter, expected) in cases {
        let mut module = fill_module();
        module.functions[0].signature.parameters.push(parameter);
        module.functions[0]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .push(ValueId(20));
        assert_eq!(first_code(&module, "fill"), expected);
    }
}

#[test]
fn read_only_and_cross_address_space_stores_fail_before_emission() {
    let mut read_only = fill_module();
    read_only.functions[0].signature.parameters[0] = global_slice(AccessMode::ReadOnly);
    let body = read_only.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations[0].results[0].ty = global_pointer(AccessMode::ReadOnly);
    body.blocks[1].operations[1].results[0].ty = global_pointer(AccessMode::ReadOnly);
    assert_eq!(
        first_code(&read_only, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::InvalidMemoryAccess)
    );

    let mut mismatched_space = fill_module();
    let OperationKind::Store { access, .. } =
        &mut mismatched_space.functions[0].body.as_mut().unwrap().blocks[1].operations[2].kind
    else {
        panic!("store expected")
    };
    access.address_space = AddressSpace::Workgroup;
    assert_eq!(
        first_code(&mismatched_space, "fill"),
        LoweringDiagnosticCode::InputVerification(DiagnosticCode::InvalidMemoryAccess)
    );
}

#[test]
fn rocm_test_targets_are_exact_canonical_amd_target_ids() {
    for target in ["gfx1151", "gfx942:sramecc+:xnack-"] {
        assert_eq!(
            canonical_test_target(target).unwrap(),
            target.split(':').next().unwrap()
        );
    }
    for target in [
        "",
        " gfx1151",
        "gfx1151 ",
        "gfx942:xnack-:sramecc+",
        "gfx942:sramecc+:sramecc-",
        "gfx942:xnack",
        "gfx9-generic",
        "--help",
        "gfx1151\n-mcpu=gfx942",
    ] {
        assert!(
            canonical_test_target(target).is_err(),
            "accepted adversarial target {target:?}"
        );
    }
}

#[test]
fn excluded_operations_constants_casts_and_comparisons_have_located_errors() {
    let mut divide = fill_module();
    divide.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            op(
                20,
                Type::F32,
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(1),
                    rhs: ValueId(1),
                },
            ),
        );
    let error = lower_kernel_to_llvm_ir(&divide, &KernelId::new("fill")).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        LoweringDiagnosticCode::UnsupportedOperation
    );
    assert_eq!(error.diagnostics()[0].location.block, Some(BlockId(0)));
    assert_eq!(error.diagnostics()[0].location.operation, Some(0));

    let mut nan = fill_module();
    nan.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            op(
                20,
                Type::F32,
                OperationKind::Constant(Constant::F32Bits(f32::NAN.to_bits())),
            ),
        );
    assert_eq!(
        first_code(&nan, "fill"),
        LoweringDiagnosticCode::UnsupportedConstant
    );

    let mut bad_cast = fill_module();
    let operations = &mut bad_cast.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        0,
        op(
            20,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(7)),
        ),
    );
    operations.insert(
        1,
        op(
            21,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::SignExtend,
                value: ValueId(20),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
    );
    assert_eq!(
        first_code(&bad_cast, "fill"),
        LoweringDiagnosticCode::UnsupportedCast
    );

    let mut float_compare = fill_module();
    let OperationKind::Compare { lhs, rhs, .. } =
        &mut float_compare.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("compare expected")
    };
    *lhs = ValueId(1);
    *rhs = ValueId(1);
    assert_eq!(
        first_code(&float_compare, "fill"),
        LoweringDiagnosticCode::UnsupportedOperation
    );
}

#[test]
fn block_arguments_and_switches_are_explicitly_outside_g1() {
    let mut arguments = fill_module();
    let blocks = &mut arguments.functions[0].body.as_mut().unwrap().blocks;
    blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(20), Type::F32));
    let Terminator::ConditionalBranch { then_arguments, .. } =
        blocks[0].terminator.as_mut().unwrap()
    else {
        panic!("conditional branch expected")
    };
    then_arguments.push(ValueId(1));
    assert_eq!(
        first_code(&arguments, "fill"),
        LoweringDiagnosticCode::UnsupportedBlockArguments
    );

    let mut switch = fill_module();
    switch.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Switch {
        selector: ValueId(2),
        cases: vec![SwitchCase {
            value: 0,
            target: BlockId(1),
            arguments: vec![],
        }],
        default_target: BlockId(2),
        default_arguments: vec![],
    });
    assert_eq!(
        first_code(&switch, "fill"),
        LoweringDiagnosticCode::UnsupportedTerminator
    );
}

#[test]
#[ignore = "requires the ROCm LLVM toolchain"]
fn rocm_compiles_the_golden_to_an_amdgpu_code_object() {
    let clang = std::env::var_os("FE2O3_ROCM_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/rocm/llvm/bin/clang"));
    let target_text = std::env::var("FE2O3_TARGET")
        .expect("FE2O3_TARGET must name an exact canonical AMDGPU target");
    let processor = canonical_test_target(&target_text).unwrap();
    let directory = TemporaryDirectory::new("g1");
    let input = directory.join("fill.ll");
    let output = directory.join("fill.hsaco");
    let llvm_ir = lower_kernel_to_llvm_ir(&fill_module(), &KernelId::new("fill")).unwrap();
    assert_eq!(llvm_ir, include_str!("fixtures/fill_g1.ll"));
    assert_eq!(
        llvm_ir,
        lower_kernel_to_llvm_ir(&fill_module(), &KernelId::new("fill")).unwrap()
    );
    fs::write(&input, &llvm_ir).unwrap();
    assert_eq!(fs::read_to_string(&input).unwrap(), llvm_ir);

    let result = Command::new(&clang)
        .arg("--target=amdgcn-amd-amdhsa")
        .arg(format!("-mcpu={target_text}"))
        .arg("-nogpulib")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "clang failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let object = fs::read(&output).unwrap();
    assert!(object.len() > 64);

    let readobj = std::env::var_os("FE2O3_ROCM_LLVM_READOBJ")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut tool = clang;
            tool.set_file_name("llvm-readobj");
            tool
        });
    let result = Command::new(readobj)
        .args(["--file-header", "--symbols", "--notes"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "llvm-readobj failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = String::from_utf8(result.stdout).unwrap();

    for invariant in [
        "Format: elf64-amdgpu",
        "Arch: amdgcn",
        "Class: 64-bit (0x2)",
        "DataEncoding: LittleEndian (0x1)",
        "OS/ABI: AMDGPU_HSA (0x40)",
        "ABIVersion: 4",
        "Type: SharedObject (0x3)",
        "Machine: EM_AMDGPU (0xE0)",
    ] {
        assert!(
            report.contains(invariant),
            "missing {invariant:?} in:\n{report}"
        );
    }
    let processor_flag = format!("EF_AMDGPU_MACH_AMDGCN_{}", processor.to_ascii_uppercase());
    assert_occurrences(&report, &processor_flag, 1);
    for feature in target_text.split(':').skip(1) {
        let feature_flag = match feature {
            "sramecc+" => "EF_AMDGPU_FEATURE_SRAMECC_ON_V4",
            "sramecc-" => "EF_AMDGPU_FEATURE_SRAMECC_OFF_V4",
            "xnack+" => "EF_AMDGPU_FEATURE_XNACK_ON_V4",
            "xnack-" => "EF_AMDGPU_FEATURE_XNACK_OFF_V4",
            _ => unreachable!("canonical_test_target rejected unknown features"),
        };
        assert_occurrences(&report, feature_flag, 1);
    }

    let entry = symbol_block(&report, "fill");
    assert!(entry.contains("    Binding: Global (0x1)"));
    assert!(entry.contains("    Type: Function (0x2)"));
    assert!(entry.contains("      STV_PROTECTED (0x3)"));
    assert!(entry.contains("    Section: .text"));
    let descriptor = symbol_block(&report, "fill.kd");
    assert!(descriptor.contains("    Size: 64"));
    assert!(descriptor.contains("    Binding: Global (0x1)"));
    assert!(descriptor.contains("    Type: Object (0x1)"));
    assert!(descriptor.contains("    Section: .rodata"));
    let global_functions = symbol_blocks(&report)
        .into_iter()
        .filter(|block| {
            block.contains("    Binding: Global (0x1)")
                && block.contains("    Type: Function (0x2)")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        global_functions.len(),
        1,
        "unexpected global kernel symbols"
    );

    assert_occurrences(&report, "Type: NT_AMDGPU_METADATA (AMDGPU Metadata)", 1);
    let metadata = metadata(&report);
    assert_occurrences(metadata, "    .symbol:         fill.kd", 1);
    assert_occurrences(metadata, "    .name:           fill", 1);
    assert!(metadata.contains("    .group_segment_fixed_size: 0\n"));
    assert!(metadata.contains("    .kernarg_segment_align: 8\n"));
    assert!(metadata.contains("    .max_flat_workgroup_size: 64\n"));
    assert!(metadata.contains("    .reqd_workgroup_size:\n      - 64\n      - 1\n      - 1\n"));
    assert!(metadata.contains("    .uses_dynamic_stack: false\n"));
    assert_metadata_argument(metadata, "arg0.data", 0, 8, "global_buffer", Some("global"));
    assert_metadata_argument(metadata, "arg0.len", 8, 8, "by_value", None);
    assert_metadata_argument(metadata, "arg1", 16, 4, "by_value", None);
    assert_occurrences(metadata, ".name:", 4);
    assert_occurrences(metadata, "amdhsa.version:\n  - 1\n  - 2", 1);
    let expected_target = if target_text.contains(':') {
        format!("amdhsa.target:   'amdgcn-amd-amdhsa--{target_text}'")
    } else {
        format!("amdhsa.target:   amdgcn-amd-amdhsa--{target_text}")
    };
    assert_occurrences(metadata, &expected_target, 1);
}
