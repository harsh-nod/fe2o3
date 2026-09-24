//! Synthetic canonical content tests: no authenticated source or native claim.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942CompleteBodyStepVNext as Step,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role, Module, Operation, OperationKind, ScalarType, TargetCapability,
    Type, ValueDef,
};
#[path = "gfx942_complete_body_canonical_emission_v19_fixtures.rs"]
mod fixtures;
fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV19, usize) {
    let mut work = Work::new(8_000_000);
    let mut budget = Budget::new(&mut work, 8_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
            module,
            &mut budget,
        )
        .expect("synthetic canonical admission");
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}
fn render(module: &Module) -> Gfx942CompleteBodyCanonicalEmissionV19 {
    let (owner, owner_storage) = admit(module);
    let mut work = Work::new(GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19 + 11);
    let mut budget = Budget::new(&mut work, 8_000_000);
    budget.charge_work(11).unwrap();
    budget.reserve_storage(owner_storage + 73).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (output, receipt) =
        lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    assert_eq!(output.canonical_identity(), owner.identity());
    assert_eq!(budget.storage(), owner_storage + 73);
    assert_eq!(
        budget.work(),
        11 + GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(
        receipt.retained_storage(),
        std::mem::size_of::<Gfx942CompleteBodyCanonicalEmissionV19>()
            + GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1
            + GFX942_COMPLETE_BODY_LLVM_BYTES_V1
    );
    output
}
fn reject_profile(module: &Module) {
    let (owner, retained) = admit(module);
    let mut work = Work::new(GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19);
    let mut budget = Budget::new(&mut work, 8_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    assert!(
        lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), retained + 73);
}
#[test]
fn emits_actual_single_block_and_all_eight_operation_correspondences() {
    let output = render(&fixtures::single());
    assert_eq!(output.operations().count(), 8);
    assert_eq!(output.blocks().count(), 1);
    assert!(output.llvm_ir().contains("@synthetic_entry("));
    assert!(
        output
            .emission()
            .assembly_template()
            .contains("v_mov_b32_e32 v33, v34")
    );
    let store = output
        .operations()
        .find(|operation| operation.operation_ordinal == 7)
        .unwrap();
    let Gfx942CompleteBodyCanonicalOperationLoweringV19::GuardedOutputStore { assembly_line } =
        store.lowering
    else {
        panic!("store relation");
    };
    assert_eq!(
        output
            .emission()
            .assembly_template()
            .lines()
            .nth(usize::from(assembly_line)),
        Some("global_store_dword v[2:3], v33, off")
    );
    let block = output.blocks().next().unwrap();
    assert_eq!(
        output
            .emission()
            .assembly_template()
            .lines()
            .nth(usize::from(block.terminator_first_assembly_line)),
        Some("s_endpgm")
    );
}
#[test]
fn derives_diamond_from_actual_edges_and_keeps_labels_distinct_from_ordinals() {
    let output = render(&fixtures::diamond());
    assert_eq!(output.operations().count(), 12);
    assert_eq!(
        output
            .blocks()
            .map(|block| (block.block.0, block.authored_label))
            .collect::<Vec<_>>(),
        [(0, 250), (1, 4), (2, 0), (3, 7)]
    );
    assert_eq!(output.emission().instructions().count(), 3);
    assert!(
        output
            .emission()
            .assembly_template()
            .contains("s_cbranch_scc1 .Lfe2o3_cb_${:uid}_b1")
    );
    assert!(
        output
            .emission()
            .assembly_template()
            .contains("s_branch .Lfe2o3_cb_${:uid}_b2")
    );
    assert_eq!(
        output
            .operations()
            .filter(|operation| matches!(
                operation.lowering,
                Gfx942CompleteBodyCanonicalOperationLoweringV19::SelectorZero { .. }
                    | Gfx942CompleteBodyCanonicalOperationLoweringV19::SelectorCompare { .. }
            ))
            .count(),
        2
    );
}
#[test]
fn actual_instruction_change_changes_identity_text_and_descriptor() {
    let before = render(&fixtures::single());
    let mut module = fixtures::single();
    let OperationKind::Gfx942CompleteBodyStep(step) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        panic!("step");
    };
    step.instruction = Instruction::Move {
        destination: Destination::Output,
        source: Role::Input2,
    };
    step.operands = [Some(ValueId(3)), None];
    let after = render(&module);
    assert_ne!(before.canonical_identity(), after.canonical_identity());
    assert_ne!(before.llvm_ir(), after.llvm_ir());
    assert_ne!(
        before.emission().instructions().next().unwrap().descriptor,
        after.emission().instructions().next().unwrap().descriptor
    );
}
#[test]
fn dead_and_self_writes_have_individual_exact_operation_lines() {
    let mut module = fixtures::single();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) = &mut block.operations[0].kind
    else {
        panic!("declaration");
    };
    declaration.instruction_count = 3;
    let movement = |instruction, result, source, operand| {
        Operation::new(
            vec![ValueDef::new(
                ValueId(result),
                Type::Scalar(ScalarType::U32),
            )],
            OperationKind::Gfx942CompleteBodyStep(Step {
                authored_block: 0,
                authored_instruction: instruction,
                instruction: Instruction::Move {
                    destination: Destination::Scratch,
                    source,
                },
                operands: [Some(ValueId(operand)), None],
            }),
        )
    };
    block.operations.insert(1, movement(0, 30, Role::Input1, 2));
    block
        .operations
        .insert(2, movement(1, 31, Role::Scratch, 30));
    let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[3].kind else {
        panic!("step");
    };
    step.authored_instruction = 2;
    let output = render(&module);
    assert_eq!(output.operations().count(), 10);
    assert_eq!(output.emission().instructions().count(), 3);
    assert!(
        output
            .emission()
            .assembly_template()
            .contains("v_mov_b32_e32 v32, v32")
    );
}
#[test]
fn refuses_non_complete_body_v19_without_projecting_an_older_version() {
    reject_profile(&Module::new("empty_v19"));
}
#[test]
fn refuses_extra_capabilities_even_when_the_graph_is_semantically_verified() {
    let mut module = fixtures::single();
    module.required_capabilities.insert(TargetCapability::Int64);
    reject_profile(&module);
    let mut module = fixtures::single();
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::Int64);
    reject_profile(&module);
    let mut module = fixtures::single();
    module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::Int64);
    reject_profile(&module);
}
#[test]
fn symbol_intersection_accepts_128_and_refuses_129_or_digit_first() {
    for (symbol, accepted) in [
        ("a".repeat(128), true),
        ("a".repeat(129), false),
        ("1kernel".into(), false),
    ] {
        let mut module = fixtures::single();
        module.functions[0].id = symbol.clone().into();
        module.kernels[0].entry = symbol.into();
        if accepted {
            render(&module);
        } else {
            reject_profile(&module);
        }
    }
}
#[test]
fn budget_work_exact_and_one_below_preserve_floor_and_ledger() {
    let (owner, retained) = admit(&fixtures::diamond());
    for short in [false, true] {
        let mut work =
            Work::new(11 + GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19 - usize::from(short));
        let mut budget = Budget::new(&mut work, 8_000_000);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(retained + 73).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget);
        assert_eq!(result.is_ok(), !short);
        if short {
            assert!(matches!(
                result,
                Err(Gfx942CompleteBodyCanonicalEmissionErrorV19::Resource(
                    Resource::Work(_)
                ))
            ));
        }
        assert_eq!(budget.storage(), retained + 73);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            budget.work(),
            if short {
                12
            } else {
                11 + GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19
            }
        );
    }
}
#[test]
fn storage_exact_and_one_below_are_admitted_before_render_allocation() {
    let (owner, retained) = admit(&fixtures::single());
    let scratch = extract::scratch_storage()
        + std::mem::size_of::<Gfx942CompleteBodyPlanV1>()
        + std::mem::size_of::<Gfx942CompleteBodyCanonicalEmissionV19>()
        + GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1
        + GFX942_COMPLETE_BODY_LLVM_BYTES_V1;
    for short in [false, true] {
        let mut work = Work::new(GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19);
        let mut budget = Budget::new(&mut work, retained + 73 + scratch - usize::from(short));
        budget.reserve_storage(retained + 73).unwrap();
        let result =
            lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget);
        assert_eq!(result.is_ok(), !short);
        if short {
            assert!(matches!(
                result,
                Err(Gfx942CompleteBodyCanonicalEmissionErrorV19::Resource(
                    Resource::Storage(_)
                ))
            ));
        }
        assert_eq!(budget.storage(), retained + 73);
        assert_eq!(
            budget.work(),
            GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19
        );
    }
}
#[test]
fn repeated_emission_keeps_first_output_floor_and_cumulative_work() {
    let (owner, retained) = admit(&fixtures::single());
    let mut work = Work::new(2 * GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19);
    let mut budget = Budget::new(&mut work, 8_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    let (first, receipt) =
        lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let (second, _) =
        lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    assert_eq!(first, second);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        budget.work(),
        2 * GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19
    );
    assert_eq!(first.into_llvm_ir(), second.llvm_ir());
}
