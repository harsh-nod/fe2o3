//! Exact inert canonical/state/resource controls; not source/native/runtime admission.
use super::*;
pub(crate) use crate::gfx942_physical_lds_exchange_fixture_v22_tests as fixture;
use crate::*;
type Opcode = Gfx942PhysicalLdsExchangeOpcodeV1;
fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV22 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
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
        VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
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
fn payload(operation: &mut Operation) -> &mut Gfx942PhysicalLdsExchangeStepV1 {
    let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = &mut operation.kind else {
        panic!("step")
    };
    step
}
fn declaration(module: &mut Module) -> &mut Gfx942PhysicalLdsExchangeDeclarationV1 {
    let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(d) =
        &mut block(module).operations[0].kind
    else {
        panic!("declaration")
    };
    d
}
fn nth(module: &mut Module, opcode: Opcode, n: usize) -> &mut Operation {
    block(module).operations.iter_mut().filter(|operation|
        matches!(&operation.kind,OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode==opcode)
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
    let d = gfx942_physical_lds_exchange_declaration_v22(&owner).unwrap();
    assert_eq!(d.native_instruction_count, 32);
    assert_eq!(d.parameters, [ValueId(0), ValueId(1)]);
    let bytes = encode_module_v22(&module).unwrap();
    assert_eq!(decode_module_v22(&bytes).unwrap(), module);
    assert_eq!(&bytes[8..10], &22_u16.to_le_bytes());
    assert_eq!(owner.module(), &module);
    assert_ne!(owner.module().functions.as_ptr(), module.functions.as_ptr());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(73).unwrap();
    let (decoded, receipt) =
        VerifiedCanonicalKernelIrModuleV22::from_canonical_bytes_with_verification_budget_v22(
            &bytes,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 73);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(decoded, owner);
    let mut hash = Sha256::new();
    hash.update((VERIFIED_CANONICAL_KERNEL_IR_V22_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V22_IDENTITY_DOMAIN_V1);
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V22_IDENTITY_POLICY_V1.to_le_bytes());
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
    let encoders: [Encoder; 18] = [
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
        encode_module_v21,
    ];
    let module = fixture::module();
    for encode in encoders {
        assert!(encode(&module).is_err());
    }
    let bytes = encode_module_v22(&module).unwrap();
    for version in [
        0u16,
        1,
        12,
        13,
        14,
        15,
        16,
        17,
        18,
        19,
        20,
        21,
        23,
        u16::MAX,
    ] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            decode_module_v22(&changed),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
    }
    assert!(decode_module_v20(&bytes).is_err());
    assert!(decode_module_v21(&bytes).is_err());
    for encode in encoders {
        let ordinary = Module::new("old_empty");
        let old = encode(&ordinary).unwrap();
        assert!(decode_module_v22(&old).is_err());
        assert_eq!(&old[..8], &KERNEL_IR_MAGIC_V1);
    }
}
#[test]
fn exact_and_one_short_resources_preserve_nonzero_floor_and_work() {
    let module = fixture::module();
    let bytes = encode_module_v22(&module).unwrap();
    for from_bytes in [false, true] {
        let run = |budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
            if from_bytes {
                VerifiedCanonicalKernelIrModuleV22::from_canonical_bytes_with_verification_budget_v22(&bytes,budget)
            } else {
                VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
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
                    matches!(result, Err(CanonicalKernelIrReplayAdmissionErrorV22::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == 11 + needed_work && error.limit() + 1 == error.actual())
                );
            } else if case == 2 {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV22::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                    )) | Err(CanonicalKernelIrReplayAdmissionErrorV22::Decode(
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
            VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
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
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode==Opcode::GlobalLoadDword)).unwrap();
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
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode==Opcode::GlobalLoadDword)).unwrap();
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
    assert!(gfx942_physical_lds_exchange_declaration_v22(&owner).is_some());
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

#[test]
fn register_edits_frame_and_two_wave_launch_are_exact() {
    for edited in [false, true] {
        let owner = admit(&fixture::module_with_registers(edited));
        let d = gfx942_physical_lds_exchange_declaration_v22(&owner).unwrap();
        assert_eq!(d.workgroup, [128, 1, 1]);
        assert_eq!(d.maximum_workgroups, [1, 1, 1]);
        assert_eq!(
            d.lds_frame,
            Gfx942PhysicalLdsExchangeFrameV1 {
                byte_offset: 0,
                byte_length: 512,
                alignment: 4,
                publication_epoch: 1,
            }
        );
    }
    for case in 0..7 {
        let mut module = fixture::module();
        let d = declaration(&mut module);
        match case {
            0 => d.lds_frame.byte_offset = 4,
            1 => d.lds_frame.byte_length = 508,
            2 => d.lds_frame.alignment = 8,
            3 => d.lds_frame.publication_epoch = 0,
            4 => d.workgroup = [64, 1, 1],
            5 => d.maximum_workgroups = [2, 1, 1],
            _ => d.native_instruction_count = 33,
        }
        refused(&module);
    }
}
#[test]
fn exact_local_peer_wait_and_published_data_relations_refuse_mutations() {
    for case in 0..12 {
        let mut module = fixture::module();
        match case {
            0 => {
                payload(nth(&mut module, Opcode::VectorXor32, 0))
                    .instruction
                    .immediate = 32
            }
            1 => {
                payload(nth(&mut module, Opcode::VectorLshlrev32, 0))
                    .instruction
                    .immediate = 1
            }
            2 => {
                let index = nth(&mut module, Opcode::VectorAddU32, 0).results[0].id;
                let step = payload(nth(&mut module, Opcode::VectorLshlrev32, 0));
                step.instruction.source0 = 2;
                step.operands[0] = Some(index);
            }
            3 => {
                let local = nth(&mut module, Opcode::VectorLshlrev32, 0).results[0].id;
                let step = payload(nth(&mut module, Opcode::LdsReadB32, 0));
                step.instruction.source0 = 16;
                step.operands[0] = Some(local);
            }
            4 => {
                let input = nth(&mut module, Opcode::GlobalLoadDword, 0).results[0].id;
                let step = payload(nth(&mut module, Opcode::GlobalStoreDword, 0));
                step.instruction.source1 = 8;
                step.operands[2] = Some(input);
            }
            5 => {
                payload(nth(&mut module, Opcode::WaitLgkm0, 1))
                    .instruction
                    .opcode = Opcode::WaitVm0
            }
            6 => {
                payload(nth(&mut module, Opcode::WaitLgkm0, 2))
                    .instruction
                    .opcode = Opcode::WaitVm0
            }
            7 => {
                payload(nth(&mut module, Opcode::WorkgroupPublishBarrier, 0))
                    .instruction
                    .opcode = Opcode::WaitLgkm0
            }
            8 => {
                let ops = &mut block(&mut module).operations;
                ops.swap(17, 18); // publication before the write's LGKM wait
                recensus(&mut module);
            }
            9 => {
                let ops = &mut block(&mut module).operations;
                ops.swap(18, 21); // LDS read before publication
                recensus(&mut module);
            }
            10 => payload(nth(&mut module, Opcode::LdsWriteB32, 0)).operands[2] = None,
            _ => payload(nth(&mut module, Opcode::LdsReadB32, 0)).operands[1] = None,
        }
        refused(&module);
    }
}
#[test]
fn exact_waited_lds_value_cannot_be_replaced_by_distinct_scalar_ssa() {
    let mut module = fixture::module();
    drop(admit(&module));
    let prior_input = nth(&mut module, Opcode::GlobalLoadDword, 0).results[0].id;
    let store = payload(nth(&mut module, Opcode::GlobalStoreDword, 0));
    assert_ne!(store.operands[2], Some(prior_input));
    store.operands[2] = Some(prior_input);
    refused(&module);
}
#[test]
fn same_owner_inert_view_budget_exact_one_short_and_nonzero_floor() {
    let owner = admit(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(97).unwrap();
    let (module, receipt) = owner
        .decoded_inert_view_with_verification_budget_v22(&mut budget)
        .unwrap();
    assert_eq!(&module, owner.module());
    assert_eq!(budget.storage(), 97);
    let needed_work = budget.work();
    let needed_storage = budget.peak_storage();
    assert!(receipt.retained_storage() > 0);
    drop(module);
    for case in 0..3 {
        let mut work =
            CanonicalKernelIrWorkBudgetV1::new(13 + needed_work - usize::from(case == 1));
        work.charge_work(13).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            needed_storage - usize::from(case == 2),
        );
        budget.reserve_storage(97).unwrap();
        let result = owner.decoded_inert_view_with_verification_budget_v22(&mut budget);
        assert_eq!(result.is_ok(), case == 0);
        assert_eq!(budget.storage(), 97);
        if case == 0 {
            assert_eq!(budget.work(), 13 + needed_work);
        }
        if case == 2 {
            assert_eq!(budget.failed_storage(), Some(needed_storage));
        }
    }
}

#[test]
fn distinct_equal_zero_scalar_and_vector_definitions_do_not_alias() {
    let mut module = fixture::module();
    drop(admit(&module));
    // The exact one-workgroup profile fixes groupX=0, so both the shifted
    // scalar group base and explicit vector zero evaluate to zero. Their
    // distinct definitions/register units still cannot substitute for one another.
    let scalar_zero = nth(&mut module, Opcode::ScalarLshl32, 0).results[0].id;
    let shift = payload(nth(&mut module, Opcode::VectorLshlrev64, 0));
    assert_ne!(shift.operands[1], Some(scalar_zero));
    shift.operands[1] = Some(scalar_zero);
    refused(&module);
}
#[test]
fn new_lds_result_and_implicit_use_rosters_do_not_invent_m0_or_exec_wait_inputs() {
    let mut module = fixture::module();
    let write = nth(&mut module, Opcode::LdsWriteB32, 0);
    assert!(write.results.is_empty());
    assert_eq!(write.operands().len(), 3);
    let read = nth(&mut module, Opcode::LdsReadB32, 0);
    assert_eq!(read.results.len(), 1);
    assert_eq!(read.results[0].ty, Type::Scalar(ScalarType::U32));
    assert_eq!(read.operands().len(), 2);
    let barrier = nth(&mut module, Opcode::WorkgroupPublishBarrier, 0);
    assert!(barrier.results.is_empty() && barrier.operands().is_empty());
    for n in 0..3 {
        let wait = nth(&mut module, Opcode::WaitLgkm0, n);
        assert!(wait.results.is_empty() && wait.operands().is_empty());
    }
}
