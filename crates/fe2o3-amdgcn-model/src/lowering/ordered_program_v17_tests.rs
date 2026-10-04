use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, Gfx942OrderedProgramRegistersV1, Gfx942U32ProgramV1,
    MemoryAccess, ValueDef,
};

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}

// Deliberately synthetic shape-admitted module: this fixture does not represent
// authenticated Rust source, a compiler receipt, or final artifact evidence.
fn fixture() -> Module {
    let mut descriptors = [0; 16];
    descriptors[..3].copy_from_slice(&[0x0085, 0x0133, 0x019d]);
    fixture_with_program(Gfx942U32ProgramV1::from_descriptors(3, descriptors).unwrap())
}

fn fixture_with_program(program: Gfx942U32ProgramV1) -> Module {
    let region = Gfx942OrderedProgramV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(5), ValueId(1), ValueId(2)],
        program,
    )
    .unwrap();
    let region = Operation::effect_free(
        ValueDef::new(ValueId(6), scalar()),
        OperationKind::Gfx942OrderedProgram(region),
    );
    let capabilities = region.required_capabilities();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), scalar()),
            OperationKind::Constant(Constant::U32(17)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), scalar()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(4),
            },
        ),
        region,
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(6),
                access: MemoryAccess::new(KernelAddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "region_impl",
        Signature::new(
            vec![
                scalar(),
                scalar(),
                scalar(),
                Type::pointer(scalar(), KernelAddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "region_kernel",
        "region_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("synthetic_ordered_program_v17");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV17 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000_000);
    VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

fn lower(module: &Module) -> Result<String, LoweringErrors> {
    lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(&admit(module))
}

fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}

#[test]
fn actual_retained_owner_lowers_one_closed_sideeffect_unit_and_surrounding_operations() {
    let module = fixture();
    let owner = admit(&module);
    let bytes = owner.canonical_bytes().to_vec();
    let llvm = lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner).unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(owner.module(), &module);
    assert_eq!(
        llvm.lines()
            .filter(|line| line.starts_with("target datalayout = "))
            .collect::<Vec<_>>(),
        [format!(
            "target datalayout = \"{}\"",
            fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
        )]
    );
    assert!(!llvm.contains("target datalayout = \"e-m:e-"));
    assert_eq!(llvm.matches(" asm sideeffect ").count(), 1);
    assert!(llvm.contains("v_xor_b32_e32 v32, $1, $2\\0A\\09v_and_b32_e32 v32, v32, $3\\0A\\09v_xor_b32_e32 $0, $2, v32"));
    assert!(llvm.contains("\"=&{v33},{v34},{v35},{v36},~{v32}\"(i32 %v5, i32 %arg1, i32 %arg2)"));
    assert!(llvm.contains("vgpr-high-water=37 (binding extent; final descriptor unverified)"));
    assert!(llvm.contains(" add i32 "));
    assert!(llvm.contains("store i32 %v6, ptr addrspace(1) %arg3, align 4"));
    assert!(llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(llvm.contains("-wavefrontsize32,+wavefrontsize64,-xnack"));
    assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
    assert!(llvm.contains("!{i32 64, i32 1, i32 1}"));
    assert!(!llvm.contains("~{exec}"));
    assert!(!llvm.contains("~{memory}"));
}

#[test]
fn unused_output_still_retains_the_whole_ordered_sideeffect_unit() {
    let mut module = fixture();
    blocks(&mut module)[0].operations.pop();
    let llvm = lower(&module).unwrap();
    assert_eq!(llvm.matches(" asm sideeffect ").count(), 1);
    assert!(llvm.contains("v_xor_b32_e32 v32"));
    assert!(llvm.contains("v_and_b32_e32 v32, v32, $3"));
    assert!(llvm.contains("v_xor_b32_e32 $0, $2, v32"));
}

#[test]
fn nonadjacent_and_boundary_registers_derive_the_exact_constraints_and_extent() {
    for (scratch, output, inputs, extent) in [(0, 1, [2, 3, 4], 5), (63, 0, [62, 17, 1], 64)] {
        let mut module = fixture();
        let operation = &mut blocks(&mut module)[0].operations[2];
        let OperationKind::Gfx942OrderedProgram(region) = operation.kind else {
            unreachable!()
        };
        operation.kind = OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                region.source(),
                Gfx942OrderedProgramRegistersV1::new(scratch, output, inputs).unwrap(),
                *region.inputs(),
                *region.program(),
            )
            .unwrap(),
        );
        let llvm = lower(&module).unwrap();
        assert!(llvm.contains(&format!(
            "v_xor_b32_e32 v{scratch}, $1, $2\\0A\\09v_and_b32_e32 v{scratch}, v{scratch}, $3\\0A\\09v_xor_b32_e32 $0, $2, v{scratch}"
        )));
        let [a, b, c] = inputs;
        assert!(llvm.contains(&format!(
            "\"=&{{v{output}}},{{v{a}}},{{v{b}}},{{v{c}}},~{{v{scratch}}}\""
        )));
        assert!(llvm.contains(&format!("vgpr-high-water={extent} ")));
    }
}

#[test]
fn old_raw_module_entries_and_other_target_contexts_cannot_admit_the_new_region() {
    let module = fixture();
    for result in [
        lower_compiler_module_to_llvm_ir(&module),
        lower_compiler_module_to_gfx942_llvm_ir(&module),
        lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&module),
        lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id),
    ] {
        assert!(result.is_err());
    }
    let owner = admit(&module);
    for target in [
        LoweringTarget::Baseline,
        LoweringTarget::Gfx942StrictFloatV1,
        LoweringTarget::Gfx950XnackMinusV1,
    ] {
        assert!(validate_owner_context(owner.module(), target, &owner).is_err());
    }
    let distinct_owner = admit(&module);
    assert!(
        validate_owner_context(
            distinct_owner.module(),
            LoweringTarget::Gfx942XnackMinusV1,
            &owner
        )
        .is_err()
    );
}

fn program_capability() -> TargetCapability {
    TargetCapability::Extension {
        namespace: fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE.to_owned(),
        name: fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME.to_owned(),
    }
}

fn fixture_without_program() -> Module {
    let mut module = fixture();
    blocks(&mut module)[0].operations[2] = Operation::effect_free(
        ValueDef::new(ValueId(6), scalar()),
        OperationKind::Constant(Constant::U32(29)),
    );
    for capabilities in [
        &mut module.required_capabilities,
        &mut module.kernels[0].required_capabilities,
        &mut module.functions[0].required_capabilities,
    ] {
        assert!(capabilities.remove(&program_capability()));
    }
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "unused_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper_block],
    ));
    module
}

fn declare_unused_program_capability(module: &mut Module, scope: usize) {
    match scope {
        0 => &mut module.required_capabilities,
        1 => &mut module.kernels[0].required_capabilities,
        2 => &mut module.functions[0].required_capabilities,
        3 => &mut module.functions[1].required_capabilities,
        _ => unreachable!(),
    }
    .insert(program_capability());
}

fn assert_program_capability_refused(result: Result<String, LoweringErrors>, scope: usize) {
    let errors = result.expect_err("unused program capability requires an actual V17 context");
    assert!(
        errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == LoweringDiagnosticCode::UnsupportedCapability
                && diagnostic
                    .message
                    .contains(fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME)
        }),
        "scope={scope}: {errors}"
    );
}

#[test]
fn legacy_complete_module_rejects_unused_program_capability_in_every_scope() {
    let baseline = fixture_without_program();
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&baseline).unwrap();
    for scope in 0..4 {
        let mut module = baseline.clone();
        declare_unused_program_capability(&mut module, scope);
        verify_module(&module).unwrap();
        assert_program_capability_refused(
            lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&module),
            scope,
        );
    }
}

#[test]
fn individual_kernel_entries_reject_unused_program_capability_in_each_selected_scope() {
    let baseline = fixture_without_program();
    lower_kernel_to_gfx942_xnack_minus_llvm_ir(&baseline, &baseline.kernels[0].id).unwrap();
    lower_kernel_to_gfx942_xnack_minus_replay_llvm_ir_v1(&baseline, &baseline.kernels[0].id)
        .unwrap();
    // Legacy individual-kernel lowering does not lower unrelated helpers. Its
    // module, selected kernel and selected entry gates must all remain closed.
    for scope in 0..3 {
        let mut module = baseline.clone();
        declare_unused_program_capability(&mut module, scope);
        verify_module(&module).unwrap();
        assert_program_capability_refused(
            lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &module.kernels[0].id),
            scope,
        );
        assert_program_capability_refused(
            lower_kernel_to_gfx942_xnack_minus_replay_llvm_ir_v1(&module, &module.kernels[0].id),
            scope,
        );
    }
}

#[test]
fn actual_v16_owner_does_not_admit_unused_program_capability_in_any_scope() {
    use fe2o3_kernel_ir::{
        Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1, VerifiedCanonicalKernelIrModuleV16,
    };

    let mut baseline = fixture_without_program();
    let operation = Operation::effect_free(
        ValueDef::new(ValueId(6), scalar()),
        OperationKind::Gfx942OrderedRegion(
            Gfx942OrderedRegionV1::new(
                AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                [ValueId(5), ValueId(1), ValueId(2)],
            )
            .unwrap(),
        ),
    );
    let capabilities = operation.required_capabilities();
    blocks(&mut baseline)[0].operations[2] = operation;
    baseline.required_capabilities = capabilities.clone();
    baseline.kernels[0].required_capabilities = capabilities.clone();
    baseline.functions[0].required_capabilities = capabilities;
    let lower_v16 = |module: &Module| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000_000);
        let (owner, _storage) =
            VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                module,
                &mut budget,
            )
            .unwrap();
        lower_canonical_v16_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner)
    };
    lower_v16(&baseline).unwrap();
    for scope in 0..4 {
        let mut module = baseline.clone();
        declare_unused_program_capability(&mut module, scope);
        // Canonical shape admission is deliberately not lowering authority.
        assert_program_capability_refused(lower_v16(&module), scope);
    }
}

#[test]
fn all_three_capabilities_are_required_in_each_scope_and_conflicting_wave_is_rejected() {
    let requirements = fixture().required_capabilities;
    assert_eq!(requirements.len(), 3);
    for scope in 0..3 {
        for missing in &requirements {
            let mut module = fixture();
            match scope {
                0 => &mut module.required_capabilities,
                1 => &mut module.kernels[0].required_capabilities,
                _ => &mut module.functions[0].required_capabilities,
            }
            .remove(missing);
            assert!(lower(&module).is_err(), "scope={scope} missing={missing:?}");
        }
        let mut module = fixture();
        match scope {
            0 => &mut module.required_capabilities,
            1 => &mut module.kernels[0].required_capabilities,
            _ => &mut module.functions[0].required_capabilities,
        }
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
        // Conflicting exact widths are rejected even earlier by canonical
        // admission, so no immutable owner exists to pass to LLVM lowering.
        assert!(verify_module(&module).is_err());
    }
}

#[test]
fn missing_or_different_required_workgroup_is_rejected() {
    for size in [
        None,
        Some(WorkgroupSize::new(32, 1, 1)),
        Some(WorkgroupSize::new(32, 2, 1)),
        Some(WorkgroupSize::new(128, 1, 1)),
    ] {
        let mut module = fixture();
        if size == Some(WorkgroupSize::new(32, 2, 1)) {
            module.kernels[0].domain = LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
            };
        }
        module.kernels[0].workgroup_size = size;
        assert!(lower(&module).is_err());
    }
}

#[test]
fn no_region_multiple_regions_or_multiple_roots_do_not_enter_the_closed_profile() {
    let mut module = fixture();
    blocks(&mut module)[0].operations.truncate(2);
    assert!(lower(&module).is_err());
    module = fixture();
    let mut second = blocks(&mut module)[0].operations[2].clone();
    second.results[0].id = ValueId(7);
    blocks(&mut module)[0].operations.insert(3, second);
    assert!(lower(&module).is_err());
    module = fixture();
    let mut second_kernel = module.kernels[0].clone();
    second_kernel.id = KernelId::from("second_kernel");
    module.kernels.push(second_kernel);
    assert!(lower(&module).is_err());
}

#[test]
fn region_in_an_uncalled_helper_is_rejected_even_with_one_root() {
    let mut module = fixture();
    let helper_region = blocks(&mut module)[0].operations[2].clone();
    blocks(&mut module)[0].operations.truncate(2);
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(helper_region);
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    let mut helper = Function::internal_helper(
        "hidden_region",
        Signature::new(vec![scalar(); 3], vec![scalar()]),
        vec![ValueId(5), ValueId(1), ValueId(2)],
        vec![helper_block],
    );
    helper.required_capabilities = module.required_capabilities.clone();
    module.functions.push(helper);
    assert!(lower(&module).is_err());
}

fn split_prefix(module: &mut Module) {
    let suffix = blocks(module)[0].operations.split_off(2);
    blocks(module)[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(1));
    body.operations = suffix;
    body.terminator = Some(Terminator::Return { values: vec![] });
    blocks(module).push(body);
}

#[test]
fn unconditional_entry_prefix_is_allowed_but_conditional_bypass_and_backedges_are_not() {
    let mut module = fixture();
    split_prefix(&mut module);
    assert!(lower(&module).is_ok());
    blocks(&mut module)[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(8), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ));
    blocks(&mut module)[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(8),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    blocks(&mut module).push(exit);
    assert!(lower(&module).is_err());
    module = fixture();
    split_prefix(&mut module);
    blocks(&mut module)[1].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    assert!(lower(&module).is_err());
}

#[test]
fn existing_ssa_inline_marker_can_precede_the_region_in_the_same_body() {
    let mut module = fixture();
    let old = Operation::effect_free(
        ValueDef::new(ValueId(7), scalar()),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [8; 32]),
            mnemonic: "v_mov_b32".to_owned(),
            operands: vec![
                fe2o3_kernel_ir::AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                fe2o3_kernel_ir::AssemblyOperand::input(ValueId(5), AssemblyConstraint::Vgpr32),
            ],
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    );
    let capabilities = old.required_capabilities();
    module.required_capabilities.extend(capabilities.clone());
    module.kernels[0]
        .required_capabilities
        .extend(capabilities.clone());
    module.functions[0]
        .required_capabilities
        .extend(capabilities);
    blocks(&mut module)[0].operations.insert(2, old);
    let llvm = lower(&module).unwrap();
    assert_eq!(llvm.matches(" asm sideeffect ").count(), 2);
    assert!(llvm.contains("v_mov_b32 $0, $1"));
    assert!(llvm.contains("v_xor_b32_e32 v32"));
}

#[test]
fn all_six_typed_mnemonics_have_exact_arity_in_one_assembly_unit() {
    for (descriptor, expected) in [
        (0x0008, "v_mov_b32_e32 $0, $1"),
        (0x0089, "v_add_u32_e32 $0, $1, $2"),
        (0x008a, "v_sub_u32_e32 $0, $1, $2"),
        (0x008b, "v_and_b32_e32 $0, $1, $2"),
        (0x008c, "v_or_b32_e32 $0, $1, $2"),
        (0x008d, "v_xor_b32_e32 $0, $1, $2"),
    ] {
        let mut descriptors = [0; 16];
        descriptors[0] = descriptor;
        let program = Gfx942U32ProgramV1::from_descriptors(1, descriptors).unwrap();
        let llvm = lower(&fixture_with_program(program)).unwrap();
        assert_eq!(llvm.matches(" asm sideeffect ").count(), 1);
        assert!(llvm.contains(&format!("asm sideeffect \"{expected}\", ")));
        assert!(!llvm.contains("\\0A\\09"));
        assert!(llvm.contains("\"=&{v33},{v34},{v35},{v36},~{v32}\""));
    }
}

#[test]
fn sixteen_steps_keep_exact_order_self_moves_dead_steps_and_output_reads() {
    let mut descriptors = [0; 16];
    descriptors[0] = 0x0008; // mov out,input0
    descriptors[1] = 0x0040; // mov scratch,out
    descriptors[2..15].fill(0x0030); // thirteen scratch self moves
    descriptors[15] = 0x0000; // dead final mov scratch,input0
    let program = Gfx942U32ProgramV1::from_descriptors(16, descriptors).unwrap();
    let mut module = fixture_with_program(program);
    blocks(&mut module)[0].operations.pop(); // Unused logical output.
    let llvm = lower(&module).unwrap();
    let asm = llvm
        .lines()
        .find(|line| line.contains(" asm sideeffect "))
        .unwrap();
    assert_eq!(llvm.matches(" asm sideeffect ").count(), 1);
    assert_eq!(asm.matches("\\0A\\09").count(), 15);
    assert!(asm.contains("asm sideeffect \"v_mov_b32_e32 $0, $1\\0A\\09v_mov_b32_e32 v32, $0"));
    assert_eq!(asm.matches("v_mov_b32_e32 v32, v32").count(), 13);
    assert!(asm.contains("\\0A\\09v_mov_b32_e32 v32, $1\", "));
    assert!(asm.contains("\"=&{v33},{v34},{v35},{v36},~{v32}\""));
    // Text emission only: this is not final machine encoding/effect qualification.
}
