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

#[cfg(test)]
mod debug_line_tests {
    use super::*;
    use crate::{
        OrderedProgramDebugLineErrorV17 as E, OrderedProgramDebugLineV17,
        lower_canonical_v17_compiler_module_with_debug_line_to_gfx942_xnack_minus_llvm_ir as emit,
    };
    use fe2o3_kernel_ir::{
        DebugSourceMapFileV1 as File, DebugSourceMapKirSiteV1 as Site,
        DebugSourceMapSiteV1 as Mapping, DebugSourceMapSpanV1 as Span,
    };

    fn source() -> AssemblySourceIdentity {
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32])
    }
    fn site() -> Site {
        Site::operation(0, 0, 2)
    }
    fn span() -> Span {
        Span::new([5; 32], 10, 20, 27, 9).unwrap()
    }
    fn files() -> Vec<File> {
        vec![File::new([5; 32], 100, "src/kernel.rs".into()).unwrap()]
    }
    fn sites() -> Vec<Mapping> {
        vec![Mapping::new(site(), vec![span()]).unwrap()]
    }
    fn error(owner: &VerifiedCanonicalKernelIrModuleV17, sites: &[Mapping], files: &[File]) -> E {
        OrderedProgramDebugLineV17::try_new(owner, owner.identity(), source(), site(), sites, files)
            .err()
            .expect("exact typed refusal")
    }

    #[test]
    fn default_is_exact_old_bytes_and_opt_in_changes_only_debug_attachments_and_nodes() {
        let owner = admit(&fixture());
        let original =
            lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner).unwrap();
        assert_eq!(emit(&owner, None).unwrap(), original);
        assert!(!original.contains("!dbg"));
        let files = files();
        let line = OrderedProgramDebugLineV17::try_new(
            &owner,
            owner.identity(),
            source(),
            site(),
            &sites(),
            &files,
        )
        .unwrap();
        let bytes = owner.canonical_bytes().to_vec();
        let actual = emit(&owner, Some(&line)).unwrap();
        let body = actual
            .split("!llvm.dbg.cu")
            .next()
            .unwrap()
            .replace(" !dbg !5", "")
            .replace(", !dbg !6", "");
        assert_eq!(body, original);
        assert_eq!(actual.matches(" !dbg !5").count(), 1);
        assert_eq!(actual.matches(", !dbg !6").count(), 1);
        assert!(actual.contains("!6 = !DILocation(line: 27, column: 9, scope: !5)"));
        assert!(actual.contains("emissionKind: LineTablesOnly"));
        assert!(actual.contains("line: 0, type: !3, scopeLine: 0"));
        assert!(!actual.contains("inlinedAt"));
        assert!(!actual.contains("checksum"));
        assert_eq!(owner.canonical_bytes(), bytes);
    }

    #[test]
    fn stale_edited_kir_and_equal_bytes_different_owner_are_not_rebound() {
        let old = admit(&fixture());
        let mut edited_module = fixture();
        blocks(&mut edited_module)[0].operations[0].kind =
            OperationKind::Constant(Constant::U32(18));
        let edited = admit(&edited_module);
        assert_ne!(old.identity(), edited.identity());
        let files = files();
        assert_eq!(
            OrderedProgramDebugLineV17::try_new(
                &edited,
                old.identity(),
                source(),
                site(),
                &sites(),
                &files
            )
            .err(),
            Some(E::CanonicalIdentityMismatch)
        );
        let line = OrderedProgramDebugLineV17::try_new(
            &old,
            old.identity(),
            source(),
            site(),
            &sites(),
            &files,
        )
        .unwrap();
        let equal_but_distinct_owner = admit(&fixture());
        assert_eq!(old.identity(), equal_but_distinct_owner.identity());
        let refused = emit(&equal_but_distinct_owner, Some(&line)).unwrap_err();
        assert_eq!(refused.diagnostics().len(), 1);
        assert_eq!(
            refused.diagnostics()[0].code,
            LoweringDiagnosticCode::SemanticAnchorIdentityMismatch
        );
        assert!(emit(&old, Some(&line)).is_ok());
    }

    #[test]
    fn source_and_all_three_site_coordinates_are_exact() {
        let owner = admit(&fixture());
        let files = files();
        for field in 0..4 {
            let mut wrong = source();
            match field {
                0 => wrong.frontend_unit[0] ^= 1,
                1 => wrong.function[0] ^= 1,
                2 => wrong.contract[0] ^= 1,
                _ => wrong.statement[0] ^= 1,
            }
            assert_eq!(
                OrderedProgramDebugLineV17::try_new(
                    &owner,
                    owner.identity(),
                    wrong,
                    site(),
                    &sites(),
                    &files
                )
                .err(),
                Some(E::SourceIdentityMismatch)
            );
        }
        for bad in [
            Site::operation(1, 0, 2),
            Site::operation(0, 1, 2),
            Site::operation(0, 0, 1),
        ] {
            assert_eq!(
                OrderedProgramDebugLineV17::try_new(
                    &owner,
                    owner.identity(),
                    source(),
                    bad,
                    &sites(),
                    &files
                )
                .err(),
                Some(E::SiteMismatch)
            );
        }
    }

    #[test]
    fn missing_and_ambiguous_site_span_file_refuse_without_guessing() {
        let owner = admit(&fixture());
        assert_eq!(error(&owner, &[], &files()), E::MissingSite);
        assert_eq!(
            error(&owner, &[sites().remove(0), sites().remove(0)], &files()),
            E::AmbiguousSite
        );
        let two = Mapping::new(
            site(),
            vec![span(), Span::new([5; 32], 30, 40, 28, 2).unwrap()],
        )
        .unwrap();
        assert_eq!(error(&owner, &[two], &files()), E::AmbiguousSpan);
        assert_eq!(error(&owner, &sites(), &[]), E::MissingFile);
        assert_eq!(
            error(&owner, &sites(), &[files().remove(0), files().remove(0)]),
            E::AmbiguousFile
        );
        assert_eq!(
            error(
                &owner,
                &sites(),
                &[File::new([6; 32], 100, "wrong.rs".into()).unwrap()]
            ),
            E::MissingFile
        );
    }

    #[test]
    fn byte_extent_and_llvm_column_width_are_checked_before_emission() {
        let owner = admit(&fixture());
        assert_eq!(
            error(
                &owner,
                &sites(),
                &[File::new([5; 32], 19, "short.rs".into()).unwrap()]
            ),
            E::SpanOutsideFile
        );
        let bad =
            Mapping::new(site(), vec![Span::new([5; 32], 10, 20, 27, 65536).unwrap()]).unwrap();
        assert_eq!(error(&owner, &[bad], &files()), E::UnsupportedColumn);
        let files = files();
        let ok = Mapping::new(
            site(),
            vec![Span::new([5; 32], 10, 100, 27, 65535).unwrap()],
        )
        .unwrap();
        assert!(
            OrderedProgramDebugLineV17::try_new(
                &owner,
                owner.identity(),
                source(),
                site(),
                &[ok],
                &files
            )
            .is_ok()
        );
    }

    #[test]
    fn bounded_source_roster_and_path_escaping_do_not_inject_metadata() {
        let owner = admit(&fixture());
        let many = vec![files().remove(0); 17];
        assert_eq!(error(&owner, &sites(), &many), E::ResourceLimit);
        let many = vec![sites().remove(0); 4097];
        assert_eq!(error(&owner, &many, &files()), E::ResourceLimit);
        let files = vec![File::new([5; 32], 100, "a\"\\\nλ.rs".into()).unwrap()];
        let line = OrderedProgramDebugLineV17::try_new(
            &owner,
            owner.identity(),
            source(),
            site(),
            &sites(),
            &files,
        )
        .unwrap();
        let actual = emit(&owner, Some(&line)).unwrap();
        assert!(actual.contains("filename: \"a\\22\\5C\\0A\\CE\\BB.rs\""));
        assert_eq!(actual.matches("!DIFile(").count(), 1);
    }

    #[test]
    fn direct_deserialization_cannot_bypass_selected_file_validation() {
        let owner = admit(&fixture());
        for path in [String::new(), "x".repeat(4097), "x\0y".into()] {
            let mut raw = serde_json::to_value(files().remove(0)).unwrap();
            raw["display_path"] = serde_json::json!(path);
            let file: File = serde_json::from_value(raw).unwrap();
            assert_eq!(error(&owner, &sites(), &[file]), E::InvalidFile);
        }
        let mut raw = serde_json::to_value(files().remove(0)).unwrap();
        raw["byte_len"] = serde_json::json!(0);
        assert_eq!(
            error(&owner, &sites(), &[serde_json::from_value(raw).unwrap()]),
            E::InvalidFile
        );
    }

    #[test]
    fn direct_deserialization_cannot_silently_make_location_unknown() {
        let owner = admit(&fixture());
        for field in ["line", "column"] {
            let mut raw = serde_json::to_value(sites().remove(0)).unwrap();
            raw["spans"][0][field] = serde_json::json!(0);
            let bad: Mapping = serde_json::from_value(raw).unwrap();
            assert_eq!(error(&owner, &[bad], &files()), E::InvalidSpan);
        }
        assert_eq!(
            crate::MAX_ORDERED_PROGRAM_DEBUG_LINE_LLVM_BYTES_V17,
            4 * 1024 * 1024
        );
    }

    #[test]
    fn block_coordinates_are_roster_ordinals_not_block_ids() {
        let mut module = fixture();
        blocks(&mut module)[0].id = BlockId(17);
        let owner = admit(&module);
        let files = files();
        let line = OrderedProgramDebugLineV17::try_new(
            &owner,
            owner.identity(),
            source(),
            site(),
            &sites(),
            &files,
        )
        .unwrap();
        assert!(
            emit(&owner, Some(&line))
                .unwrap()
                .contains("!DILocation(line: 27, column: 9, scope: !5)")
        );
        assert_eq!(
            OrderedProgramDebugLineV17::try_new(
                &owner,
                owner.identity(),
                source(),
                Site::operation(0, 17, 2),
                &sites(),
                &files
            )
            .err(),
            Some(E::SiteMismatch)
        );
    }

    #[test]
    fn total_span_cardinality_boundary_is_checked_before_selection() {
        let owner = admit(&fixture());
        let make = |count| {
            Mapping::new(
                site(),
                (0..count)
                    .map(|n| Span::new([5; 32], n, n + 1, 27, 9).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        assert_eq!(error(&owner, &[make(8192)], &files()), E::AmbiguousSpan);
        assert_eq!(error(&owner, &[make(8193)], &files()), E::ResourceLimit);
    }

    #[test]
    #[ignore = "root-selected CPU-only LLVM/LLD recipe; inert synthetic LLVM to stdout"]
    fn print_synthetic_ordered_debug_line_llvm() {
        let owner = admit(&fixture());
        let files = files();
        let line = OrderedProgramDebugLineV17::try_new(
            &owner,
            owner.identity(),
            source(),
            site(),
            &sites(),
            &files,
        )
        .unwrap();
        println!(
            "FE2O3_ORDERED_DEBUG_LLVM_BEGIN\n{}FE2O3_ORDERED_DEBUG_LLVM_END",
            emit(&owner, Some(&line)).unwrap()
        );
    }

    // Synthetic model fixture only. The ordinary identity helper is deliberately
    // unlocated; both calls surround the located ordered operation.
    fn helper_call_line_fixture() -> String {
        let mut module = fixture();
        let mut helper_block = BasicBlock::new(BlockId(0));
        helper_block.terminator = Some(Terminator::Return {
            values: vec![ValueId(0)],
        });
        module.functions.push(Function::internal_helper(
            "ordinary_line_helper",
            Signature::new(vec![scalar()], vec![scalar()]),
            vec![ValueId(0)],
            vec![helper_block],
        ));
        let operations = &mut blocks(&mut module)[0].operations;
        operations.insert(
            2,
            Operation::effect_free(
                ValueDef::new(ValueId(7), scalar()),
                OperationKind::Call {
                    callee: "ordinary_line_helper".into(),
                    arguments: vec![ValueId(5)],
                },
            ),
        );
        let OperationKind::Gfx942OrderedProgram(region) = operations[3].kind else {
            unreachable!()
        };
        operations[3].kind = OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                region.source(),
                region.registers(),
                [ValueId(7), ValueId(1), ValueId(2)],
                *region.program(),
            )
            .unwrap(),
        );
        operations.insert(
            4,
            Operation::effect_free(
                ValueDef::new(ValueId(8), scalar()),
                OperationKind::Call {
                    callee: "ordinary_line_helper".into(),
                    arguments: vec![ValueId(6)],
                },
            ),
        );
        let OperationKind::Store { value, .. } = &mut operations[5].kind else {
            unreachable!()
        };
        *value = ValueId(8);
        let owner = admit(&module);
        let files = files();
        let selected = Site::operation(0, 0, 3);
        let sites = [Mapping::new(selected, vec![span()]).unwrap()];
        let line = OrderedProgramDebugLineV17::try_new(
            &owner,
            owner.identity(),
            source(),
            selected,
            &sites,
            &files,
        )
        .unwrap();
        let old =
            lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner).unwrap();
        assert_eq!(emit(&owner, None).unwrap(), old);
        let llvm = emit(&owner, Some(&line)).unwrap();
        let calls: Vec<_> = llvm
            .lines()
            .filter(|line| line.contains("call i32 @") && line.contains("ordinary_line_helper"))
            .collect();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|line| !line.contains("!dbg")));
        assert_eq!(llvm.matches(", !dbg !6").count(), 1);
        assert_eq!(llvm.matches(" !dbg !5").count(), 1);
        let positions: Vec<_> = llvm
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                line.contains("call i32 @") && line.contains("ordinary_line_helper")
                    || line.contains(" asm sideeffect ")
            })
            .map(|(_, line)| line.contains(" asm sideeffect "))
            .collect();
        assert_eq!(positions, [false, true, false]);
        llvm
    }

    #[test]
    fn ordinary_helper_calls_surround_opt_in_region_without_invented_locations() {
        let _ = helper_call_line_fixture();
    }

    #[test]
    #[ignore = "explicit bounded synthetic input for actual LLVM22 verifier-only control"]
    fn print_synthetic_ordered_debug_helper_line_llvm() {
        let llvm = helper_call_line_fixture();
        println!(
            "FE2O3_ORDERED_DEBUG_HELPER_LLVM_BEGIN\n{llvm}FE2O3_ORDERED_DEBUG_HELPER_LLVM_END"
        );
    }
}
