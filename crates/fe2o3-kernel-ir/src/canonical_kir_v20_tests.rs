//! Inert exact graph/codec/resource controls, not source/native/GPU qualification.
use super::*;
use crate::gfx942_physical_entry_fixture_v20_tests as fixture;
#[path = "formal_memory_obligations/physical_entry_v20_tests.rs"]
mod formal_memory;
use crate::*;

type Opcode = Gfx942PhysicalEntryOpcodeV20;
fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV20 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
        module,
        &mut budget,
    )
    .expect("inert physical fixture")
    .0
}
fn refused(module: &Module) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(73).unwrap();
    assert!(
        VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
            module,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 73);
    assert!(budget.work() > 0);
}
fn step(module: &mut Module, opcode: Opcode) -> &mut Operation {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(&operation.kind, OperationKind::Gfx942PhysicalEntryStep(step)
                if step.instruction.opcode == opcode)
        })
        .unwrap()
}
fn payload(operation: &mut Operation) -> &mut Gfx942PhysicalEntryStepVNext {
    let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind else {
        panic!("step")
    };
    step
}
fn declaration(module: &mut Module) -> &mut Gfx942PhysicalEntryDeclarationVNext {
    let OperationKind::Gfx942PhysicalEntryDeclaration(value) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    value
}
#[test]
fn exact_copy_and_diamond_inverse_identity_and_retained_owner() {
    for select in [false, true] {
        let source = fixture::module(select);
        let owner = admit(&source);
        let declaration = gfx942_physical_entry_declaration_v20(&owner).unwrap();
        assert_eq!(
            declaration.native_instruction_count,
            if select { 25 } else { 21 }
        );
        let bytes = encode_module_v20(&source).unwrap();
        assert_eq!(decode_module_v20(&bytes).unwrap(), source);
        assert_eq!(&bytes[8..10], &20_u16.to_le_bytes());
        assert_eq!(owner.module(), &source);
        assert_ne!(owner.module().functions.as_ptr(), source.functions.as_ptr());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
        budget.reserve_storage(73).unwrap();
        let (decoded, receipt) =
            VerifiedCanonicalKernelIrModuleV20::from_canonical_bytes_with_verification_budget_v20(
                &bytes,
                &mut budget,
            )
            .unwrap();
        assert_eq!(budget.storage(), 73);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(decoded, owner);
        let mut hash = Sha256::new();
        hash.update(
            (VERIFIED_CANONICAL_KERNEL_IR_V20_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes(),
        );
        hash.update(VERIFIED_CANONICAL_KERNEL_IR_V20_IDENTITY_DOMAIN_V1);
        hash.update(VERIFIED_CANONICAL_KERNEL_IR_V20_IDENTITY_POLICY_V1.to_le_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
        assert_eq!(
            owner.identity().digest(),
            &<[u8; 32]>::from(hash.finalize())
        );
        drop(decoded);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 73);
    }
}
#[test]
fn older_profiles_refuse_new_operations_and_v20_never_falls_back() {
    type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
    let old: [Encoder; 16] = [
        encode_module_v1,
        encode_module_v2,
        encode_module_v3,
        encode_module_v4,
        encode_module_v5,
        encode_module_v6,
        encode_module_v7,
        encode_module_v8,
        encode_module_v9,
        encode_module_v10,
        encode_module_v11,
        encode_module_v12,
        encode_module_v15,
        encode_module_v16,
        encode_module_v17,
        encode_module_v19,
    ];
    let module = fixture::module(false);
    for encode in old {
        assert!(encode(&module).is_err());
    }
    let bytes = encode_module_v20(&module).unwrap();
    for version in [0_u16, 1, 12, 13, 14, 15, 16, 17, 18, 19, 21, u16::MAX] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            decode_module_v20(&changed),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
    }
    assert!(decode_module_v19(&bytes).is_err());
    let legacy = Module::new("old_empty");
    assert_eq!(
        decode_module_v19(&encode_module_v19(&legacy).unwrap()).unwrap(),
        legacy
    );
}
#[test]
fn exact_and_one_short_work_storage_keep_cumulative_nonzero_floor() {
    let module = fixture::module(true);
    let bytes = encode_module_v20(&module).unwrap();
    for from_bytes in [false, true] {
        let run = |budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
            if from_bytes {
                VerifiedCanonicalKernelIrModuleV20::from_canonical_bytes_with_verification_budget_v20(&bytes,budget)
            } else {
                VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
                    &module, budget,
                )
            }
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
        budget.reserve_storage(73).unwrap();
        drop(run(&mut budget).unwrap());
        let needed_work = budget.work();
        let needed_storage = budget.peak_storage();
        for case in 0..3 {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(11 + needed_work - usize::from(case == 1));
            work.charge_work(11).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                needed_storage - usize::from(case == 2),
            );
            budget.reserve_storage(73).unwrap();
            assert_eq!(run(&mut budget).is_ok(), case == 0);
            assert_eq!(budget.storage(), 73);
            if case == 0 {
                assert_eq!(budget.work(), 11 + needed_work);
            }
        }
    }
}
#[test]
fn fresh_scc_results_and_scalar_waits_have_exact_rosters() {
    let mut module = fixture::module(false);
    let shift = step(&mut module, Opcode::ScalarLshl32);
    assert_eq!(shift.results.len(), 2);
    assert_eq!(shift.results[1].ty, Type::BOOL);
    assert_eq!(shift.operands().len(), 1);
    for opcode in [Opcode::WaitLgkm0, Opcode::WaitVm0] {
        let wait = step(&mut module, opcode);
        assert!(wait.operands().is_empty());
        assert!(wait.results.is_empty());
    }
    step(&mut module, Opcode::ScalarLshl32).results.pop();
    refused(&module);
}
#[test]
fn dominating_distinct_equal_zero_operand_substitution_refuses_exact_register_ssa() {
    let mut module = fixture::module(false);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let exec = block.operations[0].results[4].id;
    let extra = ValueId(500);
    let site = |n: u8| Gfx942PhysicalEntrySourceSiteVNext {
        occurrence: n,
        raw_block: 100 + u32::from(n),
        semantic_block_index: 72 - u32::from(n),
        semantic_block_identity: [n + 1; 32],
        semantic_callable_index: 7,
    };
    block.operations.insert(
        1,
        Operation::new(
            vec![ValueDef::new(extra, Type::Scalar(ScalarType::U32))],
            OperationKind::Gfx942PhysicalEntryStep(Gfx942PhysicalEntryStepVNext {
                site: site(2),
                native_ordinal: 0,
                instruction: Gfx942PhysicalEntryInstructionVNext {
                    opcode: Opcode::VectorMove32,
                    destination: 9,
                    source0: 255,
                    source1: 0,
                    immediate: 0,
                },
                operands: [Some(exec), None, None, None, None, None],
            }),
        ),
    );
    for (index, operation) in block.operations.iter_mut().enumerate().skip(1) {
        let step = payload(operation);
        step.native_ordinal = (index - 1) as u8;
        step.site = site((index + 1) as u8);
    }
    let count = block.operations.len() as u8;
    let declaration = declaration(&mut module);
    declaration.native_instruction_count = count;
    declaration.blocks[0].native_ordinal = Some(count - 1);
    declaration.blocks[0].terminator_site = site(count + 1);
    drop(admit(&module)); // the additional full-EXEC zero definition is genuinely admitted
    let shift = payload(step(&mut module, Opcode::VectorLshlrev64));
    assert_ne!(shift.operands[1], Some(extra));
    shift.operands[1] = Some(extra); // both dominate and equal zero, but v9 cannot replace v3
    refused(&module);
}
#[test]
fn source_and_native_exact_census_mutations_refuse() {
    for mutation in 0..7 {
        let mut module = fixture::module(true);
        match mutation {
            0 => declaration(&mut module).native_instruction_count -= 1,
            1 => declaration(&mut module).blocks[2].native_ordinal = Some(16),
            2 => payload(step(&mut module, Opcode::VectorAddU32)).native_ordinal = 0,
            3 => {
                payload(step(&mut module, Opcode::VectorAddU32))
                    .site
                    .occurrence = 0
            }
            4 => {
                payload(step(&mut module, Opcode::VectorAddU32))
                    .site
                    .raw_block = 100
            }
            5 => {
                payload(step(&mut module, Opcode::VectorAddU32))
                    .site
                    .semantic_block_index = 72
            }
            _ => declaration(&mut module).blocks[1].label = 250,
        }
        refused(&module);
    }
}
#[test]
fn exact_target_wave_launch_signature_and_capability_scopes_refuse_drift() {
    for mutation in 0..8 {
        let mut module = fixture::module(false);
        match mutation {
            0 => {
                module.required_capabilities.clear();
            }
            1 => {
                module.functions[0].required_capabilities.clear();
            }
            2 => {
                module.kernels[0].required_capabilities.clear();
            }
            3 => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            4 => declaration(&mut module).maximum_workgroups = [3, 1, 1],
            5 => module.functions[0].signature.parameters[1] = Type::BOOL,
            6 => module.kernels[0].id = KernelId::new("different_entry"),
            _ => {
                module.required_capabilities.insert(TargetCapability::Int64);
            }
        }
        refused(&module);
    }
}
#[test]
fn missing_load_wait_and_overwriting_pending_load_refuse() {
    let mut module = fixture::module(false);
    payload(step(&mut module, Opcode::WaitLgkm0))
        .instruction
        .opcode = Opcode::WaitVm0;
    refused(&module);
    let mut module = fixture::module(false);
    payload(step(&mut module, Opcode::LoadKernargDword))
        .instruction
        .destination = 8;
    refused(&module);
}
#[test]
fn pointer_carry_and_length_generation_substitutions_refuse() {
    for mutation in 0..5 {
        let mut module = fixture::module(false);
        let instruction = &mut payload(step(
            &mut module,
            match mutation {
                0 => Opcode::VectorMove32,
                1 => Opcode::VectorAddCarry,
                2 => Opcode::VectorAddCarryIn,
                3 => Opcode::VectorCompareGtU64,
                _ => Opcode::VectorLshlrev64,
            },
        ))
        .instruction;
        match mutation {
            0 => instruction.source0 = 12,
            1 => instruction.source0 = 10,
            2 => instruction.source0 = 3,
            3 => instruction.source0 = 8,
            _ => instruction.immediate = 3,
        }
        refused(&module);
    }
}
#[test]
fn masked_valu_extra_store_wrong_exec_restore_and_missing_wait_refuse() {
    for mutation in 0..5 {
        let mut module = fixture::module(false);
        match mutation {
            0 => {
                payload(step(&mut module, Opcode::GlobalStoreDword)).instruction =
                    Gfx942PhysicalEntryInstructionVNext {
                        opcode: Opcode::VectorMove32,
                        destination: 9,
                        source0: 12,
                        source1: 0,
                        immediate: 0,
                    }
            }
            1 => {
                payload(step(&mut module, Opcode::WaitVm0))
                    .instruction
                    .opcode = Opcode::GlobalStoreDword
            }
            2 => {
                payload(step(&mut module, Opcode::RestoreExec))
                    .instruction
                    .source0 = 10
            }
            3 => payload(step(&mut module, Opcode::SaveAndMaskExec)).operands[0] = Some(ValueId(9)),
            _ => {
                payload(step(&mut module, Opcode::WaitVm0))
                    .instruction
                    .opcode = Opcode::WaitLgkm0
            }
        }
        refused(&module);
    }
}
#[test]
fn actual_diamond_condition_edges_phi_and_tail_are_not_shadow_metadata() {
    for mutation in 0..5 {
        let mut module = fixture::module(true);
        let body = module.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => {
                if let Some(Terminator::ConditionalBranch { condition, .. }) =
                    &mut body.blocks[0].terminator
                {
                    *condition = ValueId(1);
                }
            }
            1 => {
                if let Some(Terminator::ConditionalBranch { then_target, .. }) =
                    &mut body.blocks[0].terminator
                {
                    *then_target = BlockId(1);
                }
            }
            2 => {
                if let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[1].terminator {
                    arguments.clear();
                }
            }
            3 => body.blocks[3].parameters[0].ty = Type::BOOL,
            _ => {
                body.blocks[3].terminator = Some(Terminator::Return {
                    values: vec![ValueId(1)],
                })
            }
        }
        refused(&module);
    }
}
#[test]
fn new_operations_remain_ordered_and_generic_memory_analysis_is_not_discharged() {
    let module = fixture::module(false);
    for op in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
        assert!(!op.has_complete_effect_summary());
        assert_eq!(op.required_capabilities().len(), 3);
        assert_eq!(
            op.compiler_ordering_effects_v12(),
            CompilerOrderingEffectSummaryV12::ordered_region()
        );
    }
}
