use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE, AssemblyConstraint, AssemblyEffect,
    AssemblyOperand, AssemblyOperandKind, AssemblyOption, AssemblySourceIdentity, BasicBlock,
    BlockId, DiagnosticCode, Function, InlineAssembly, InlineAssemblyTarget, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, TargetCapability,
    Terminator, Type, ValueDef, ValueId, WorkgroupSize, decode_module_v3, encode_module_v1,
    encode_module_v2, encode_module_v3, verify_module,
};

fn source() -> AssemblySourceIdentity {
    AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32])
}

fn assembly(mnemonic: &str) -> InlineAssembly {
    InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: source(),
        mnemonic: mnemonic.to_owned(),
        operands: vec![
            AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
        ],
        options: BTreeSet::from([
            AssemblyOption::NoMemory,
            AssemblyOption::Pure,
            AssemblyOption::NoStack,
        ]),
        declared_effects: BTreeSet::new(),
    }
}

fn module_with(assembly: InlineAssembly) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32))],
        OperationKind::InlineAssembly(assembly),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("assembly-module");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "assembly_kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn operation_mut(module: &mut Module) -> &mut InlineAssembly {
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
        unreachable!()
    };
    assembly
}

#[test]
fn verifies_source_bound_assembly_and_derives_target_capability() {
    let module = module_with(assembly("v_add_u32"));
    verify_module(&module).unwrap();

    let operation = &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0];
    assert!(operation.memory_effects().is_empty());
    assert_eq!(
        operation.required_capabilities(),
        BTreeSet::from([TargetCapability::Extension {
            namespace: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME.to_owned(),
        }])
    );
}

#[test]
fn rejects_missing_source_authority_and_textual_injection() {
    let mut module = module_with(assembly("v_add_u32"));
    operation_mut(&mut module).source.statement = [0; 32];
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));

    let module = module_with(assembly("v_add_u32\n.global hidden"));
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));
}

#[test]
fn rejects_result_and_constraint_mismatches() {
    let mut module = module_with(assembly("v_add_u32"));
    operation_mut(&mut module).operands[0].kind = AssemblyOperandKind::Output { result_index: 7 };
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::ResultArity));

    let mut module = module_with(assembly("v_add_u32"));
    operation_mut(&mut module).operands[1].constraint = AssemblyConstraint::ImmediateI32;
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));

    let mut module = module_with(assembly("v_add_u32"));
    operation_mut(&mut module).operands[2] = AssemblyOperand {
        kind: AssemblyOperandKind::ImmediateI32(9),
        constraint: AssemblyConstraint::Vgpr32,
    };
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));
}

#[test]
fn rejects_options_that_hide_declared_effects() {
    let mut module = module_with(assembly("global_load_dword"));
    operation_mut(&mut module)
        .declared_effects
        .insert(AssemblyEffect::ReadGlobal);
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));

    let mut module = module_with(assembly("v_add_u32"));
    operation_mut(&mut module)
        .options
        .remove(&AssemblyOption::NoMemory);
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidInlineAssembly));
}

#[test]
fn v3_round_trips_and_frozen_versions_reject_inline_assembly() {
    let module = module_with(assembly("v_add_u32"));
    let bytes = encode_module_v3(&module).unwrap();
    assert_eq!(decode_module_v3(&bytes).unwrap(), module);
    assert!(encode_module_v1(&module).is_err());
    assert!(encode_module_v2(&module).is_err());

    for end in 0..bytes.len() {
        assert!(decode_module_v3(&bytes[..end]).is_err(), "prefix {end}");
    }
}

#[test]
fn v3_single_bit_mutations_never_bypass_canonical_decoding() {
    let bytes = encode_module_v3(&module_with(assembly("v_add_u32"))).unwrap();
    for byte_index in 0..bytes.len() {
        for bit in 0..8 {
            let mut mutated = bytes.clone();
            mutated[byte_index] ^= 1 << bit;
            if let Ok(decoded) = decode_module_v3(&mutated) {
                assert_eq!(encode_module_v3(&decoded).unwrap(), mutated);
            }
        }
    }
}
