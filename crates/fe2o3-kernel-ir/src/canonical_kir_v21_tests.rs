//! Exact inert canonical/state/resource controls; not source/native/runtime admission.
use super::*;
pub(crate) use crate::gfx942_physical_global_copy_fixture_v21_tests as fixture;
use crate::*;
type Opcode = Gfx942PhysicalGlobalCopyOpcodeV1;
fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV21 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn refused(module: &Module) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(73).unwrap();
    assert!(
        VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
            module,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 73);
    assert!(budget.work() > 0);
}
fn block(module: &mut Module) -> &mut BasicBlock {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0]
}
fn payload(operation: &mut Operation) -> &mut Gfx942PhysicalGlobalCopyStepV1 {
    let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut operation.kind else {
        panic!("step")
    };
    step
}
fn declaration(module: &mut Module) -> &mut Gfx942PhysicalGlobalCopyDeclarationV1 {
    let OperationKind::Gfx942PhysicalGlobalCopyDeclaration(d) =
        &mut block(module).operations[0].kind
    else {
        panic!("declaration")
    };
    d
}
fn nth(module: &mut Module, opcode: Opcode, n: usize) -> &mut Operation {
    block(module).operations.iter_mut().filter(|operation|
        matches!(&operation.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(step) if step.instruction.opcode==opcode)
    ).nth(n).unwrap()
}
fn recensus(module: &mut Module) {
    let body = block(module);
    for (ordinal, operation) in body.operations.iter_mut().skip(1).enumerate() {
        let step = payload(operation);
        step.native_ordinal = ordinal as u8;
        step.site = fixture::site(ordinal as u8 + 2);
    }
    let count = body.operations.len() as u8;
    let d = declaration(module);
    d.native_instruction_count = count;
    d.block.native_ordinal = Some(count - 1);
    d.block.terminator_site = fixture::site(count + 1);
}
#[test]
fn exact_owner_inverse_identity_and_retained_storage() {
    let module = fixture::module();
    let owner = admit(&module);
    let d = gfx942_physical_global_copy_declaration_v21(&owner).unwrap();
    assert_eq!(d.native_instruction_count, 24);
    assert_eq!(d.parameters, [ValueId(0), ValueId(1)]);
    let bytes = encode_module_v21(&module).unwrap();
    assert_eq!(decode_module_v21(&bytes).unwrap(), module);
    assert_eq!(&bytes[8..10], &21_u16.to_le_bytes());
    assert_eq!(owner.module(), &module);
    assert_ne!(owner.module().functions.as_ptr(), module.functions.as_ptr());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(73).unwrap();
    let (decoded, receipt) =
        VerifiedCanonicalKernelIrModuleV21::from_canonical_bytes_with_verification_budget_v21(
            &bytes,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 73);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(decoded, owner);
    let mut hash = Sha256::new();
    hash.update((VERIFIED_CANONICAL_KERNEL_IR_V21_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V21_IDENTITY_DOMAIN_V1);
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V21_IDENTITY_POLICY_V1.to_le_bytes());
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
#[test]
fn all_older_encoders_and_wrong_version_decoders_refuse() {
    type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
    let encoders: [Encoder; 17] = [
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
        encode_module_v20,
    ];
    let module = fixture::module();
    for encode in encoders {
        assert!(encode(&module).is_err());
    }
    let bytes = encode_module_v21(&module).unwrap();
    for version in [0u16, 1, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, u16::MAX] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            decode_module_v21(&changed),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
    }
    assert!(decode_module_v20(&bytes).is_err());
    for encode in encoders {
        let ordinary = Module::new("old_empty");
        let old = encode(&ordinary).unwrap();
        assert!(decode_module_v21(&old).is_err());
        assert_eq!(&old[..8], &KERNEL_IR_MAGIC_V1);
    }
}
#[test]
fn exact_and_one_short_resources_preserve_nonzero_floor_and_work() {
    let module = fixture::module();
    let bytes = encode_module_v21(&module).unwrap();
    for from_bytes in [false, true] {
        let run = |budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
            if from_bytes {
                VerifiedCanonicalKernelIrModuleV21::from_canonical_bytes_with_verification_budget_v21(&bytes,budget)
            } else {
                VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
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
            let result = run(&mut budget);
            assert_eq!(result.is_ok(), case == 0);
            if case == 1 {
                assert!(
                    matches!(result, Err(CanonicalKernelIrReplayAdmissionErrorV21::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == 11 + needed_work && error.limit() + 1 == error.actual())
                );
            } else if case == 2 {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV21::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                    )) | Err(CanonicalKernelIrReplayAdmissionErrorV21::Decode(
                        KernelIrDecodeError::Resource(
                            CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                        )
                    ))
                ));
                assert_eq!(budget.failed_storage(), Some(needed_storage));
            }
            assert_eq!(budget.storage(), 73);
            if case == 0 {
                assert_eq!(budget.work(), 11 + needed_work);
            }
        }
    }
}
#[test]
fn repeated_failures_do_not_refund_work_or_erase_peak() {
    let mut module = fixture::module();
    declaration(&mut module).parameters.swap(0, 1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(79).unwrap();
    let mut prior_work = 0;
    let mut prior_peak = 0;
    for _ in 0..3 {
        assert!(
            VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
                &module,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 79);
        assert!(budget.work() > prior_work);
        assert!(budget.peak_storage() >= prior_peak);
        prior_work = budget.work();
        prior_peak = budget.peak_storage();
    }
}
#[test]
fn read_and_store_waits_are_scalar_and_load_has_exact_full_exec_roster() {
    let mut module = fixture::module();
    let load = nth(&mut module, Opcode::GlobalLoadDword, 0);
    assert_eq!(load.results.len(), 1);
    assert_eq!(load.results[0].ty, Type::Scalar(ScalarType::U32));
    assert_eq!(load.operands().len(), 3);
    for opcode in [Opcode::WaitLgkm0, Opcode::WaitVm0] {
        let wait = nth(&mut module, opcode, 0);
        assert!(wait.operands().is_empty());
        assert!(wait.results.is_empty());
    }
    let shift = nth(&mut module, Opcode::ScalarLshl32, 0);
    assert_eq!(shift.results.len(), 2);
    assert_eq!(shift.results[1].ty, Type::BOOL);
    let load = nth(&mut module, Opcode::GlobalLoadDword, 0);
    payload(load).operands[2] = None;
    refused(&module);
}
#[test]
fn missing_wrong_counter_or_late_read_wait_is_refused_after_fresh_census() {
    for case in 0..3 {
        let mut module = fixture::module();
        let operations = &mut block(&mut module).operations;
        let position=operations.iter().position(|op|matches!(&op.kind,
            OperationKind::Gfx942PhysicalGlobalCopyStep(step) if step.instruction.opcode==Opcode::GlobalLoadDword)).unwrap();
        match case {
            0 => {
                operations.remove(position + 1);
            }
            1 => {
                payload(&mut operations[position + 1]).instruction.opcode = Opcode::WaitLgkm0;
            }
            _ => {
                operations.swap(position + 1, position + 2);
            }
        }
        recensus(&mut module);
        refused(&module);
    }
}
#[test]
fn second_load_pending_overwrite_and_pending_return_refuse() {
    for case in 0..3 {
        let mut module = fixture::module();
        let ops = &mut block(&mut module).operations;
        let position=ops.iter().position(|op|matches!(&op.kind,
            OperationKind::Gfx942PhysicalGlobalCopyStep(step) if step.instruction.opcode==Opcode::GlobalLoadDword)).unwrap();
        if case == 2 {
            ops.truncate(position + 1);
        } else {
            let mut op = ops[if case == 0 { position } else { position + 2 }].clone();
            op.results[0].id = ValueId(700);
            if case == 1 {
                payload(&mut op).instruction.destination = 8;
            }
            ops[position + 1] = op;
        }
        recensus(&mut module);
        refused(&module);
    }
}
#[test]
fn pointer_root_half_carry_and_actual_index_substitutions_refuse() {
    for case in 0..5 {
        let mut module = fixture::module();
        match case {
            0 => {
                payload(nth(&mut module, Opcode::LoadKernargPair, 0))
                    .instruction
                    .immediate = 16;
                payload(nth(&mut module, Opcode::LoadKernargPair, 2))
                    .instruction
                    .immediate = 0;
            }
            1 => {
                let wrong = nth(&mut module, Opcode::LoadKernargPair, 2).results[1].id;
                let moved = payload(nth(&mut module, Opcode::VectorMove32, 1));
                moved.instruction.source0 = 13;
                moved.operands[0] = Some(wrong);
            }
            2 => {
                let wrong = nth(&mut module, Opcode::VectorAddCarryIn, 0).results[1].id;
                payload(nth(&mut module, Opcode::VectorAddCarryIn, 1)).operands[3] = Some(wrong);
            }
            3 => {
                let wrong = nth(&mut module, Opcode::LoadKernargPair, 1).results[0].id;
                payload(nth(&mut module, Opcode::VectorCompareGtU64, 0)).operands[0] = Some(wrong);
            }
            _ => {
                let wrong = nth(&mut module, Opcode::VectorMove32, 0).results[0].id;
                payload(nth(&mut module, Opcode::GlobalStoreDword, 0)).operands[2] = Some(wrong);
            }
        }
        refused(&module);
    }
}
#[test]
fn distinct_equal_zero_ssa_substitution_is_rejected_after_positive_admission() {
    let mut module = fixture::module_with_extra_zero(true);
    drop(admit(&module));
    let extra = block(&mut module)
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::Gfx942PhysicalGlobalCopyStep(step)
                if step.instruction.destination == 30 =>
            {
                Some(op.results[0].id)
            }
            _ => None,
        })
        .unwrap();
    let shift = payload(nth(&mut module, Opcode::VectorLshlrev64, 0));
    assert_ne!(shift.operands[1], Some(extra));
    shift.operands[1] = Some(extra);
    refused(&module);
}
#[test]
fn mask_restore_store_wait_and_wrong_terminal_refuse() {
    for case in 0..5 {
        let mut module = fixture::module();
        match case {
            0 => {
                let wrong = nth(&mut module, Opcode::VectorAddCarryIn, 1).results[1].id;
                payload(nth(&mut module, Opcode::SaveAndMaskExec, 0)).operands[0] = Some(wrong);
            }
            1 => {
                let wait = nth(&mut module, Opcode::WaitVm0, 1);
                payload(wait).instruction.opcode = Opcode::WaitLgkm0;
            }
            2 => {
                let wrong = nth(&mut module, Opcode::LoadKernargPair, 0).results[0].id;
                payload(nth(&mut module, Opcode::RestoreExec, 0)).operands[0] = Some(wrong);
            }
            3 => {
                block(&mut module).terminator = Some(Terminator::Branch {
                    target: BlockId(0),
                    arguments: vec![],
                });
            }
            _ => {
                let op = nth(&mut module, Opcode::GlobalStoreDword, 0);
                payload(op).instruction.immediate = 4;
            }
        }
        refused(&module);
    }
}
#[test]
fn sites_capabilities_launch_signature_and_extra_blocks_fail_closed() {
    for case in 0..8 {
        let mut module = fixture::module();
        match case {
            0 => {
                declaration(&mut module).origin.root_axes[2] = [0; 32];
            }
            1 => {
                let first = payload(nth(&mut module, Opcode::LoadKernargPair, 0)).site;
                payload(nth(&mut module, Opcode::LoadKernargPair, 1))
                    .site
                    .raw_block = first.raw_block;
            }
            2 => {
                payload(nth(&mut module, Opcode::GlobalLoadDword, 0))
                    .site
                    .occurrence = 33;
            }
            3 => {
                module.required_capabilities.clear();
            }
            4 => {
                declaration(&mut module).maximum_workgroups = [3, 1, 1];
            }
            5 => {
                module.functions[0].signature.parameters.swap(0, 1);
            }
            6 => {
                module.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .blocks
                    .push(BasicBlock::new(BlockId(1)));
            }
            _ => {
                declaration(&mut module).block.label = 1;
            }
        }
        refused(&module);
    }
}
#[test]
fn ordered_unknown_generic_effects_remain_unmodeled_not_memory_free() {
    let module = fixture::module();
    for op in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
        assert!(!op.has_complete_effect_summary());
    }
    // Canonical structural custody has no runtime pointer or alias binding.
    let owner = admit(&module);
    assert!(gfx942_physical_global_copy_declaration_v21(&owner).is_some());
    let generic = derive_kernel_memory_obligations_from_verified_for_launch(
        owner.verified_module_ref_v1(),
        &owner.module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [128, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!generic.is_complete());
    assert!(generic.incomplete_reasons().iter().all(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
    )));
}
