//! Inert canonical/formal controls, not actual Rust or runtime authority.
use super::*;
use crate::gfx942_physical_global_copy_fixture_v21_tests as fixture;
use crate::{CanonicalKernelIrWorkBudgetV1 as Work, Module};
fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV21 {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn launch(count: u64) -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    }
}
fn report(owner: &VerifiedCanonicalKernelIrModuleV21) -> PhysicalGlobalCopyMemoryObligationsV21 {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(79).unwrap();
    let (report, storage) = derive_physical_global_copy_memory_obligations_v21(
        owner,
        &owner.module().kernels[0].id,
        launch(128),
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 79);
    assert_eq!(budget.work(), WORK);
    assert_eq!(storage.retained_storage(), retained().unwrap());
    report
}
fn validate(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    report: &PhysicalGlobalCopyMemoryObligationsV21,
) -> Result<(), PhysicalGlobalCopyMemoryErrorV21> {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    let floor = 79 + retained().unwrap();
    budget.reserve_storage(floor).unwrap();
    let result = validate_physical_global_copy_memory_obligations_v21(
        owner,
        &owner.module().kernels[0].id,
        launch(128),
        FormalIndexWidth::Bits64,
        report,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), WORK);
    result
}
#[test]
fn global_copy_formal_two_real_allocations_read_write_bounds_and_alias_remain_conditional() {
    for extra in [false, true] {
        let owner = owner(&fixture::module_with_extra_zero(extra));
        let report = report(&owner);
        assert_eq!(report.canonical_identity(), owner.identity().digest());
        let global = report.global();
        assert_eq!(global.allocations().len(), 2);
        assert_eq!(global.accesses().len(), 2);
        assert_eq!(global.allocations()[0].access(), AccessMode::ReadOnly);
        assert_eq!(global.allocations()[1].access(), AccessMode::ReadWrite);
        assert_eq!(global.accesses()[0].kind(), FormalMemoryAccessKind::Read);
        assert_eq!(global.accesses()[1].kind(), FormalMemoryAccessKind::Write);
        for (i, access) in global.accesses().iter().enumerate() {
            assert_eq!(access.allocation().parameter_index(), i as u32);
            assert_eq!(
                access.byte_offset(),
                ByteExpression::invocation_affine(0, 4)
            );
            assert_eq!(access.domain(), FormalAccessDomainV1::LaunchEnvelope);
        }
        assert_eq!(global.bounds_requirements().len(), 2);
        assert!(
            global
                .bounds_requirements()
                .iter()
                .all(|b| b.minimum_byte_len() == Some(512))
        );
        let [alias] = global.runtime_alias_requirements() else {
            panic!("real alias condition");
        };
        assert_eq!(alias.left().parameter_index(), 0);
        assert_eq!(alias.right().parameter_index(), 1);
        for range in [alias.left_accessed_bytes(), alias.right_accessed_bytes()] {
            let range = range.unwrap();
            assert_eq!((range.start(), range.end_exclusive()), (0, 512));
        }
        assert!(global.inter_invocation_conflicts().is_empty());
        let r = report.runtime_requirements();
        assert!(
            r.requires_input_readable()
                && r.requires_input_initialized()
                && r.requires_output_writable()
                && r.requires_input_output_disjoint()
        );
        assert_eq!(
            (r.minimum_input_bytes(), r.minimum_output_bytes()),
            (512, 512)
        );
        assert!(!r.grants_runtime_binding_authority());
        assert!(!report.grants_artifact_or_launch_authority());
        validate(&owner, &report).unwrap();
    }
}
#[test]
fn global_copy_formal_records_exact_ordered_kernarg_slots_and_actual_lgkm_occurrence() {
    let owner = owner(&fixture::module());
    let report = report(&owner);
    let block = &owner.module().functions[0].body.as_ref().unwrap().blocks[0];
    assert_eq!(
        report.kernarg_reads().map(|r| r.byte_offset()),
        [0, 8, 16, 24]
    );
    assert_eq!(
        report.kernarg_reads().map(|r| r.slot()),
        [
            PhysicalGlobalCopyKernargSlotV21::InputPointer,
            PhysicalGlobalCopyKernargSlotV21::InputLength,
            PhysicalGlobalCopyKernargSlotV21::OutputPointer,
            PhysicalGlobalCopyKernargSlotV21::OutputLength
        ]
    );
    for row in report.kernarg_reads() {
        let op = &block.operations[row.location().operation_index];
        let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = op.kind else {
            panic!("step");
        };
        assert_eq!(row.source_site(), step.site);
        assert_eq!(row.results(), [op.results[0].id, op.results[1].id]);
        assert_eq!(row.base().map(Some), [step.operands[0], step.operands[1]]);
        assert_eq!((row.byte_width(), row.alignment()), (8, 8));
        let OperationKind::Gfx942PhysicalGlobalCopyStep(wait) =
            block.operations[row.ready_at().operation_index].kind
        else {
            panic!("wait");
        };
        assert_eq!(wait.instruction.opcode, Opcode::WaitLgkm0);
        assert_eq!(wait.site, row.ready_source_site());
    }
    let abi = report.kernarg_abi();
    assert_eq!((abi.minimum_bytes(), abi.alignment()), (32, 8));
    assert_eq!(abi.disjoint_output().parameter_index(), 1);
    assert!(
        abi.requires_live_kernarg()
            && abi.requires_readable_kernarg()
            && abi.requires_immutable_kernarg()
    );
}
#[test]
fn global_copy_formal_retains_original_read_result_full_exec_and_masked_store_waits() {
    let owner = owner(&fixture::module());
    let report = report(&owner);
    let block = &owner.module().functions[0].body.as_ref().unwrap().blocks[0];
    let read = report.input_read();
    let store = report.output_store();
    assert_eq!(read.result(), store.value());
    assert_eq!(read.access().index(), store.access().index());
    assert_eq!(read.access().exec(), block.operations[0].results[4].id);
    assert_ne!(read.access().exec(), store.access().exec());
    assert_eq!(
        read.ready_at().operation_index,
        read.access().location().operation_index + 1
    );
    assert_eq!(
        store.ready_at().operation_index,
        store.access().location().operation_index + 1
    );
    assert_eq!(
        store.restore_at().operation_index,
        store.access().location().operation_index + 2
    );
    assert_ne!(read.ready_at(), store.ready_at());
    for (location, site, opcode) in [
        (read.ready_at(), read.ready_source_site(), Opcode::WaitVm0),
        (store.ready_at(), store.ready_source_site(), Opcode::WaitVm0),
        (
            store.restore_at(),
            store.restore_source_site(),
            Opcode::RestoreExec,
        ),
        (
            store.mask_at(),
            store.mask_source_site(),
            Opcode::SaveAndMaskExec,
        ),
        (
            store.comparison_at(),
            store.comparison_source_site(),
            Opcode::VectorCompareGtU64,
        ),
    ] {
        let OperationKind::Gfx942PhysicalGlobalCopyStep(s) =
            block.operations[location.operation_index].kind
        else {
            panic!("site");
        };
        assert_eq!(s.site, site);
        assert_eq!(s.instruction.opcode, opcode);
    }
}
#[test]
fn global_copy_formal_replay_rejects_every_detached_condition_site_and_ssa_mutation() {
    let owner = owner(&fixture::module());
    for which in 0..31 {
        let mut r = report(&owner);
        match which {
            0 => r.canonical_identity[0] ^= 1,
            1 => r.kernarg_reads.swap(0, 1),
            2 => r.kernarg_reads[1] = r.kernarg_reads[0],
            3 => r.kernarg_reads[0].slot = PhysicalGlobalCopyKernargSlotV21::OutputPointer,
            4 => r.kernarg_reads[0].offset = 16,
            5 => r.kernarg_reads[0].site.raw_block += 1,
            6 => r.kernarg_reads[0].ready_at.operation_index += 1,
            7 => r.kernarg_reads[0].ready_site.raw_block += 1,
            8 => r.kernarg_reads[0].results.swap(0, 1),
            9 => r.kernarg_reads[0].base.swap(0, 1),
            10 => r.read.result = r.read.access.index[0],
            11 => r.read.ready_at = r.store.ready_at,
            12 => r.read.ready_site = r.store.ready_site,
            13 => r.read.access.exec = r.store.access.exec,
            14 => r.store.value = r.store.access.index[0],
            15 => r.store.comparison_site.raw_block += 1,
            16 => r.store.restore_at.operation_index -= 1,
            17 => {
                r.global.accesses.remove(0);
            }
            18 => r.global.runtime_alias_requirements.clear(),
            19 => r.runtime.minimum_input_bytes = 4,
            20 => r.runtime.minimum_output_bytes = 4,
            21 => r.runtime.input_readable = false,
            22 => r.runtime.input_initialized = false,
            23 => r.runtime.output_writable = false,
            24 => r.runtime.input_output_disjoint = false,
            25 => r.abi.immutable = false,
            26 => r.abi.live = false,
            27 => r.abi.readable = false,
            28 => r.abi.disjoint_output = r.runtime.input,
            29 => r.global.allocations[0].access = AccessMode::ReadWrite,
            30 => r.global.runtime_alias_requirements[0].right = r.runtime.input,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                validate(&owner, &r),
                Err(PhysicalGlobalCopyMemoryErrorV21::Profile(
                    "global copy complete memory report changed"
                ))
            ),
            "mutation{which}"
        );
    }
}
#[test]
fn global_copy_formal_foreign_subject_and_actual_extra_step_do_not_share_report() {
    let first = owner(&fixture::module());
    let second = owner(&fixture::module_with_extra_zero(true));
    let first_report = report(&first);
    assert!(validate(&second, &first_report).is_err());
    assert_ne!(
        first_report.input_read().access().location(),
        report(&second).input_read().access().location()
    );
}
#[test]
fn global_copy_formal_64_launch_uses256_bytes_without_changing_full_exec_semantics() {
    let owner = owner(&fixture::module());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, 4_000_000);
    let (r, _) = derive_physical_global_copy_memory_obligations_v21(
        &owner,
        &owner.module().kernels[0].id,
        launch(64),
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap();
    assert_eq!(r.runtime_requirements().minimum_input_bytes(), 256);
    assert_eq!(r.runtime_requirements().minimum_output_bytes(), 256);
    assert!(
        r.global()
            .bounds_requirements()
            .iter()
            .all(|b| b.minimum_byte_len() == Some(256))
    );
}
#[test]
fn global_copy_formal_wrong_launch_kernel_and_width_refuse_without_resetting_floor() {
    let owner = owner(&fixture::module());
    for (kernel, launch, width) in [
        (
            KernelId::new("wrong"),
            launch(128),
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            ExplicitLaunchExtent::Unknown,
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            launch(192),
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            launch(128),
            FormalIndexWidth::Bits32,
        ),
    ] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, 4_000_000);
        budget.reserve_storage(79).unwrap();
        assert!(
            derive_physical_global_copy_memory_obligations_v21(
                &owner,
                &kernel,
                launch,
                width,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 79);
        assert_eq!(budget.work(), WORK);
    }
}
#[test]
fn global_copy_formal_exact_and_one_short_work_storage_keep_prior_usage() {
    let owner = owner(&fixture::module());
    let peak = 79 + retained().unwrap() + SCRATCH;
    for (work_limit, storage_limit, success) in [
        (11 + WORK, peak, true),
        (10 + WORK, peak, false),
        (11 + WORK, peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(79).unwrap();
        assert_eq!(
            derive_physical_global_copy_memory_obligations_v21(
                &owner,
                &owner.module().kernels[0].id,
                launch(128),
                FormalIndexWidth::Bits64,
                &mut budget
            )
            .is_ok(),
            success
        );
        assert_eq!(budget.storage(), 79);
        if success {
            assert_eq!(budget.work(), 11 + WORK);
            assert_eq!(budget.peak_storage(), peak);
        }
        if storage_limit < peak {
            assert_eq!(budget.failed_storage(), Some(peak));
        }
    }
}
#[test]
fn global_copy_formal_replay_floor_denial_charges_work_and_preserves_denial_history() {
    let owner = owner(&fixture::module());
    let r = report(&owner);
    let mut work = Work::new(WORK * 3);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(retained().unwrap() - 1).unwrap();
    let floor = budget.storage();
    for attempt in 1..=3 {
        assert!(matches!(
            validate_physical_global_copy_memory_obligations_v21(
                &owner,
                &owner.module().kernels[0].id,
                launch(128),
                FormalIndexWidth::Bits64,
                &r,
                &mut budget
            ),
            Err(PhysicalGlobalCopyMemoryErrorV21::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), attempt);
    }
}
#[test]
fn global_copy_formal_repeated_mutation_failure_work_is_cumulative() {
    let owner = owner(&fixture::module());
    let mut r = report(&owner);
    r.runtime.input_initialized = false;
    let mut work = Work::new(7 + 2 * WORK);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.charge_work(7).unwrap();
    let floor = 79 + retained().unwrap();
    budget.reserve_storage(floor).unwrap();
    for attempt in 1..=2 {
        assert!(
            validate_physical_global_copy_memory_obligations_v21(
                &owner,
                &owner.module().kernels[0].id,
                launch(128),
                FormalIndexWidth::Bits64,
                &r,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 7 + attempt * WORK);
    }
}
#[test]
fn global_copy_generic_formal_still_refuses_unmodeled_memory() {
    let owner = owner(&fixture::module());
    let generic = derive_kernel_memory_obligations_from_verified_for_launch(
        owner.verified_module_ref_v1(),
        &owner.module().kernels[0].id,
        launch(128),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!generic.is_complete());
    assert!(generic.incomplete_reasons().iter().any(|r| matches!(
        r,
        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
    )));
}
#[test]
fn global_copy_changed_wait_root_data_or_equal_valued_ssa_cannot_produce_formal_owner() {
    for which in 0..6 {
        let mut module = fixture::module_with_extra_zero(true);
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let find = |opcode| {
            ops.iter().position(|op|matches!(op.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s)if s.instruction.opcode==opcode)).unwrap()
        };
        let lgkm = find(Opcode::WaitLgkm0);
        let load = find(Opcode::GlobalLoadDword);
        let store = find(Opcode::GlobalStoreDword);
        let shift = find(Opcode::VectorLshlrev64);
        let zero=ops.iter().filter(|op|matches!(op.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s)if s.instruction.opcode==Opcode::VectorMove32&&s.instruction.source0==255))
            .map(|op|op.results[0].id).collect::<Vec<_>>();
        let OperationKind::Gfx942PhysicalGlobalCopyStep(read) = ops[load].kind else {
            unreachable!();
        };
        let selected = match which {
            0 => load + 1,
            1 => lgkm,
            2 | 4 => store,
            3 => shift,
            5 => 1,
            _ => unreachable!(),
        };
        let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut ops[selected].kind else {
            unreachable!();
        };
        match which {
            0 => step.instruction.opcode = Opcode::WaitLgkm0,
            1 => step.instruction.opcode = Opcode::WaitVm0,
            2 => {
                step.instruction.source0 = 6;
                step.operands[0] = read.operands[0];
                step.operands[1] = read.operands[1];
            }
            3 => step.operands[1] = Some(zero[1]), // distinct SSA, same numerical zero
            4 => {
                step.instruction.source1 = 3;
                step.operands[2] = Some(zero[0]);
            }
            5 => step.instruction.immediate = 8,
            _ => unreachable!(),
        }
        let mut work = Work::new(16_000_000);
        let mut budget = Budget::new(&mut work, 32_000_000);
        budget.reserve_storage(79).unwrap();
        assert!(
            VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
                &module,
                &mut budget
            )
            .is_err(),
            "canonical mutant{which}"
        );
        assert_eq!(budget.storage(), 79);
    }
}
