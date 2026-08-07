use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::{
    LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir,
    lower_compiler_module_to_llvm_ir, lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-inline-assembly-{}-{nonce}",
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

fn source() -> AssemblySourceIdentity {
    AssemblySourceIdentity::new([0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32])
}

fn assembly(mnemonic: &str, constraint: AssemblyConstraint, input_count: usize) -> InlineAssembly {
    let mut operands = vec![AssemblyOperand::output(0, constraint)];
    operands.extend(
        (0..input_count).map(|index| AssemblyOperand::input(ValueId(index as u32), constraint)),
    );
    InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: source(),
        mnemonic: mnemonic.to_owned(),
        operands,
        options: BTreeSet::from([
            AssemblyOption::NoMemory,
            AssemblyOption::Pure,
            AssemblyOption::PreservesFlags,
            AssemblyOption::NoStack,
        ]),
        declared_effects: BTreeSet::new(),
    }
}

fn module_with(assembly: InlineAssembly) -> Module {
    let input_count = assembly.operands.len() - 1;
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(
            ValueId(input_count as u32),
            Type::Scalar(ScalarType::U32),
        )],
        OperationKind::InlineAssembly(assembly),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        "assembly_impl",
        Signature::new(vec![Type::Scalar(ScalarType::U32); input_count], vec![]),
        (0..input_count)
            .map(|index| ValueId(index as u32))
            .collect(),
        vec![block],
    );
    function
        .required_capabilities
        .insert(inline_assembly_capability());
    let mut kernel = Kernel::new(
        "assembly_kernel",
        "assembly_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tests::gfx942_inline_assembly");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn inline_assembly_capability() -> TargetCapability {
    TargetCapability::Extension {
        namespace: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME.to_owned(),
    }
}

fn assembly_mut(module: &mut Module) -> &mut InlineAssembly {
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        unreachable!()
    };
    assembly
}

#[test]
fn v_add_has_exact_gfx942_golden_ir() {
    let module = module_with(assembly("v_add_u32", AssemblyConstraint::Vgpr32, 2));
    let llvm = lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into()).unwrap();
    assert_eq!(
        llvm,
        include_str!("fixtures/gfx942_v_add_inline_assembly.ll")
    );
    assert!(llvm.contains("call i32 asm \"v_add_u32 $0, $1, $2\", \"=v,v,v\""));
    assert!(!llvm.contains("sideeffect"));
    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
}

#[test]
fn baseline_and_missing_capability_fail_closed() {
    let module = module_with(assembly("v_add_u32", AssemblyConstraint::Vgpr32, 2));
    assert!(
        lower_kernel_to_llvm_ir(&module, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );
    assert!(
        lower_compiler_module_to_llvm_ir(&module)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );

    let mut missing = module;
    missing.functions[0].required_capabilities.clear();
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&missing, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedCapability)
    );
}

#[test]
fn hidden_memory_control_flow_convergence_and_special_state_fail_closed() {
    for mnemonic in [
        "global_load_dword",
        "ds_write_b32",
        "s_barrier",
        "s_branch",
        "v_readfirstlane_b32",
        "s_add_u32",
    ] {
        let module = module_with(assembly(mnemonic, AssemblyConstraint::Vgpr32, 2));
        let error =
            lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into()).expect_err(mnemonic);
        assert!(
            error.contains(LoweringDiagnosticCode::UnsupportedAssemblyInstruction),
            "{mnemonic}: {error}"
        );
    }
}

#[test]
fn operand_roles_constraints_and_types_are_exact() {
    let mut module = module_with(assembly("v_add_u32", AssemblyConstraint::Vgpr32, 2));
    assembly_mut(&mut module).operands[1].constraint = AssemblyConstraint::Sgpr32;
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::AssemblyOperandMismatch)
    );

    let mut module = module_with(assembly("v_add_u32", AssemblyConstraint::Vgpr32, 2));
    assembly_mut(&mut module).operands[2].kind = AssemblyOperandKind::InOut {
        input: ValueId(1),
        result_index: 0,
    };
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::DuplicateValue
            ))
    );

    let mut module = module_with(assembly("s_mov_b32", AssemblyConstraint::Sgpr32, 1));
    assembly_mut(&mut module).operands[0].constraint = AssemblyConstraint::Vgpr32;
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::AssemblyOperandMismatch)
    );
}

#[test]
fn declared_and_instruction_effects_must_match_exactly() {
    let mut module = module_with(assembly("v_add_u32", AssemblyConstraint::Vgpr32, 2));
    let assembly = assembly_mut(&mut module);
    assembly.options.remove(&AssemblyOption::NoMemory);
    assembly.options.insert(AssemblyOption::ReadOnly);
    assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
    assert!(
        lower_kernel_to_gfx942_llvm_ir(&module, &"assembly_kernel".into())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::AssemblyEffectMismatch)
    );
}

#[test]
fn non_pure_statements_remain_side_effecting_and_source_identity_is_required() {
    let mut module = module_with(assembly("v_xor_b32", AssemblyConstraint::Vgpr32, 2));
    assembly_mut(&mut module)
        .options
        .remove(&AssemblyOption::Pure);
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
    assert!(llvm.contains("asm sideeffect \"v_xor_b32 $0, $1, $2\""));

    assembly_mut(&mut module).source.contract = [0; 32];
    assert!(matches!(
        lower_compiler_module_to_gfx942_llvm_ir(&module),
        Err(error) if error.contains(LoweringDiagnosticCode::InputVerification(
            DiagnosticCode::InvalidInlineAssembly
        ))
    ));
}

#[test]
fn every_admitted_instruction_lowers_to_a_canonical_static_template() {
    for (mnemonic, constraint, inputs) in [
        ("v_mov_b32", AssemblyConstraint::Vgpr32, 1),
        ("s_mov_b32", AssemblyConstraint::Sgpr32, 1),
        ("v_add_u32", AssemblyConstraint::Vgpr32, 2),
        ("v_sub_u32", AssemblyConstraint::Vgpr32, 2),
        ("v_and_b32", AssemblyConstraint::Vgpr32, 2),
        ("v_or_b32", AssemblyConstraint::Vgpr32, 2),
        ("v_xor_b32", AssemblyConstraint::Vgpr32, 2),
    ] {
        let module = module_with(assembly(mnemonic, constraint, inputs));
        let llvm = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
        assert!(
            llvm.contains(&format!("asm \"{mnemonic} $0, $1")),
            "{mnemonic}"
        );
        assert_eq!(llvm.matches(mnemonic).count(), 1, "{mnemonic}");
    }
}

#[test]
#[ignore = "requires ROCm LLVM tools with gfx942 support"]
fn rocm_compiles_links_and_inspects_gfx942_inline_assembly() {
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD");
    let readelf = std::env::var("FE2O3_LLVM_READELF").expect("set FE2O3_LLVM_READELF");
    let objdump = std::env::var("FE2O3_LLVM_OBJDUMP").expect("set FE2O3_LLVM_OBJDUMP");
    let directory = TemporaryDirectory::new();

    for (mnemonic, constraint, inputs) in [
        ("v_mov_b32", AssemblyConstraint::Vgpr32, 1),
        ("s_mov_b32", AssemblyConstraint::Sgpr32, 1),
        ("v_add_u32", AssemblyConstraint::Vgpr32, 2),
        ("v_sub_u32", AssemblyConstraint::Vgpr32, 2),
        ("v_and_b32", AssemblyConstraint::Vgpr32, 2),
        ("v_or_b32", AssemblyConstraint::Vgpr32, 2),
        ("v_xor_b32", AssemblyConstraint::Vgpr32, 2),
    ] {
        let mut module = module_with(assembly(mnemonic, constraint, inputs));
        assembly_mut(&mut module)
            .options
            .remove(&AssemblyOption::Pure);
        let llvm = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
        let source = directory.join(&format!("{mnemonic}.ll"));
        let object = directory.join(&format!("{mnemonic}.o"));
        let hsaco = directory.join(&format!("{mnemonic}.hsaco"));
        fs::write(&source, llvm).unwrap();
        let compile = Command::new(&llc)
            .args([
                "-mtriple=amdgcn-amd-amdhsa",
                "-mcpu=gfx942",
                "--amdhsa-code-object-version=6",
                "-filetype=obj",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "llc rejected {mnemonic}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let link = Command::new(&lld)
            .arg("-shared")
            .arg(&object)
            .arg("-o")
            .arg(&hsaco)
            .output()
            .unwrap();
        assert!(
            link.status.success(),
            "lld rejected {mnemonic}: {}",
            String::from_utf8_lossy(&link.stderr)
        );
        let notes = Command::new(&readelf)
            .arg("--notes")
            .arg(&hsaco)
            .output()
            .unwrap();
        assert!(notes.status.success());
        assert!(String::from_utf8_lossy(&notes.stdout).contains("gfx942"));
        let disassembly = Command::new(&objdump)
            .arg("-d")
            .arg("--mcpu=gfx942")
            .arg(&hsaco)
            .output()
            .unwrap();
        assert!(disassembly.status.success());
        let instruction_family = mnemonic.split('_').take(2).collect::<Vec<_>>().join("_");
        assert!(
            String::from_utf8_lossy(&disassembly.stdout).contains(&instruction_family),
            "missing {instruction_family} for {mnemonic} in disassembly"
        );
    }
}
