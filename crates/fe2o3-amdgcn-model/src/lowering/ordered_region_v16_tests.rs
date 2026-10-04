use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, Gfx942OrderedRegionRegistersV1, MemoryAccess, ValueDef,
};

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}

// Deliberately synthetic shape-admitted module: this fixture does not represent
// authenticated Rust source, a compiler receipt, or final artifact evidence.
fn fixture() -> Module {
    let region = Gfx942OrderedRegionV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(5), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let region = Operation::effect_free(
        ValueDef::new(ValueId(6), scalar()),
        OperationKind::Gfx942OrderedRegion(region),
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
    let mut module = Module::new("synthetic_ordered_region_v16");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV16 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000_000);
    VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

fn lower(module: &Module) -> Result<String, LoweringErrors> {
    lower_canonical_v16_compiler_module_to_gfx942_xnack_minus_llvm_ir(&admit(module))
}

fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}

#[test]
fn actual_retained_owner_lowers_one_closed_sideeffect_unit_and_surrounding_operations() {
    let module = fixture();
    let owner = admit(&module);
    let bytes = owner.canonical_bytes().to_vec();
    let llvm = lower_canonical_v16_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner).unwrap();
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
    assert!(llvm.contains("v_xor_b32_e32 v32, $1, $2\\0A\\09v_add_u32_e32 $0, v32, $3"));
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
    assert!(llvm.contains("v_add_u32_e32 $0, v32, $3"));
}

#[test]
fn nonadjacent_and_boundary_registers_derive_the_exact_constraints_and_extent() {
    for (scratch, output, inputs, extent) in [(0, 1, [2, 3, 4], 5), (63, 0, [62, 17, 1], 64)] {
        let mut module = fixture();
        let operation = &mut blocks(&mut module)[0].operations[2];
        let OperationKind::Gfx942OrderedRegion(region) = operation.kind else {
            unreachable!()
        };
        operation.kind = OperationKind::Gfx942OrderedRegion(
            Gfx942OrderedRegionV1::new(
                region.source(),
                Gfx942OrderedRegionRegistersV1::new(scratch, output, inputs).unwrap(),
                *region.inputs(),
            )
            .unwrap(),
        );
        let llvm = lower(&module).unwrap();
        assert!(llvm.contains(&format!(
            "v_xor_b32_e32 v{scratch}, $1, $2\\0A\\09v_add_u32_e32 $0, v{scratch}, $3"
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
