//! Inert canonical/formal component tests, not live source or runtime custody.
use super::fixture;
use crate::*;
use CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use CanonicalKernelIrWorkBudgetV1 as Work;

fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV20 {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn report(owner: &VerifiedCanonicalKernelIrModuleV20) -> PhysicalEntryMemoryObligationsV20 {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(127).unwrap();
    let (result, storage) = derive_physical_entry_memory_obligations_v20(
        owner,
        &owner.module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [128, 1, 1],
        },
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 127);
    assert!(storage.retained_storage() > std::mem::size_of_val(&result));
    result
}
#[test]
fn physical_formal_one_and_diamond_cover_real_output_and_all_six_kernarg_reads() {
    for select in [false, true] {
        let owner = owner(&fixture::module(select));
        let report = report(&owner);
        assert_eq!(report.canonical_identity(), owner.identity().digest());
        assert_eq!(report.kernarg_reads().len(), 6);
        assert_eq!(
            report
                .kernarg_reads()
                .iter()
                .map(|r| (r.byte_offset(), r.byte_width(), r.alignment()))
                .collect::<Vec<_>>(),
            [
                (0, 8, 8),
                (8, 8, 8),
                (16, 4, 4),
                (20, 4, 4),
                (24, 4, 4),
                (28, 4, 4)
            ]
        );
        let output = report.output();
        assert_eq!(output.allocations().len(), 1);
        assert_eq!(output.allocations()[0].identity().parameter_index(), 0);
        assert_eq!(output.accesses().len(), 1);
        assert_eq!(output.accesses()[0].kind(), FormalMemoryAccessKind::Write);
        assert_eq!(
            output.accesses()[0].byte_offset(),
            ByteExpression::invocation_affine(0, 4)
        );
        assert_eq!(
            output.accesses()[0].domain(),
            FormalAccessDomainV1::LaunchEnvelope
        );
        assert_eq!(output.bounds_requirements().len(), 1);
        assert_eq!(
            output.bounds_requirements()[0].minimum_byte_len(),
            Some(512)
        );
        assert!(output.runtime_alias_requirements().is_empty());
        assert!(output.inter_invocation_conflicts().is_empty());
        assert_eq!(report.kernarg_abi().minimum_bytes(), 32);
        assert_eq!(report.kernarg_abi().alignment(), 8);
        assert_eq!(
            report.kernarg_abi().disjoint_output(),
            output.allocations()[0].identity()
        );
        assert!(report.kernarg_abi().requires_immutable_kernarg());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}
#[test]
fn physical_formal_read_and_wait_records_join_actual_operations_sites_and_ssa() {
    let owner = owner(&fixture::module(true));
    let report = report(&owner);
    let body = owner.module().functions[0].body.as_ref().unwrap();
    for read in report.kernarg_reads() {
        let location = read.location();
        let operation = &body
            .blocks
            .iter()
            .find(|b| b.id == location.block)
            .unwrap()
            .operations[location.operation_index];
        let OperationKind::Gfx942PhysicalEntryStep(step) = operation.kind else {
            panic!("load")
        };
        assert_eq!(read.source_site(), step.site);
        assert_eq!(
            read.base(),
            [step.operands[0].unwrap(), step.operands[1].unwrap()]
        );
        assert_eq!(read.results()[0], Some(operation.results[0].id));
        assert_eq!(read.results()[1], operation.results.get(1).map(|v| v.id));
        let ready = read.ready_at();
        assert_eq!(ready.block, location.block);
        assert!(ready.operation_index > location.operation_index);
        let OperationKind::Gfx942PhysicalEntryStep(wait) = body
            .blocks
            .iter()
            .find(|b| b.id == ready.block)
            .unwrap()
            .operations[ready.operation_index]
            .kind
        else {
            panic!("wait")
        };
        assert_eq!(
            wait.instruction.opcode,
            Gfx942PhysicalEntryOpcodeV20::WaitLgkm0
        );
        assert_eq!(read.ready_source_site(), wait.site);
    }
    let store = report.store();
    let location = store.location();
    let operation = &body
        .blocks
        .iter()
        .find(|b| b.id == location.block)
        .unwrap()
        .operations[location.operation_index];
    let OperationKind::Gfx942PhysicalEntryStep(step) = operation.kind else {
        panic!("store")
    };
    assert_eq!(store.source_site(), step.site);
    assert_eq!(
        store.address(),
        [step.operands[0].unwrap(), step.operands[1].unwrap()]
    );
    assert_eq!(store.value(), step.operands[2].unwrap());
    assert_eq!(store.exec(), step.operands[3].unwrap());
    assert_eq!(report.output().accesses()[0].location(), location);
    assert_eq!(store.output(), body.parameters[0]);
    let mask_location = store.mask_at();
    let mask = &body
        .blocks
        .iter()
        .find(|b| b.id == mask_location.block)
        .unwrap()
        .operations[mask_location.operation_index];
    let OperationKind::Gfx942PhysicalEntryStep(mask_step) = mask.kind else {
        panic!("mask")
    };
    assert_eq!(
        mask_step.instruction.opcode,
        Gfx942PhysicalEntryOpcodeV20::SaveAndMaskExec
    );
    assert_eq!(mask_step.site, store.mask_source_site());
    assert_eq!(mask.results[2].id, store.exec());
    let compare_location = store.comparison_at();
    let compare = &body
        .blocks
        .iter()
        .find(|b| b.id == compare_location.block)
        .unwrap()
        .operations[compare_location.operation_index];
    let OperationKind::Gfx942PhysicalEntryStep(compare_step) = compare.kind else {
        panic!("compare")
    };
    assert_eq!(
        compare_step.instruction.opcode,
        Gfx942PhysicalEntryOpcodeV20::VectorCompareGtU64
    );
    assert_eq!(compare_step.site, store.comparison_source_site());
    assert_eq!(
        store.length(),
        [
            compare_step.operands[0].unwrap(),
            compare_step.operands[1].unwrap()
        ]
    );
    assert_eq!(
        store.index(),
        [
            compare_step.operands[2].unwrap(),
            compare_step.operands[3].unwrap()
        ]
    );
    assert_eq!(mask_step.operands[0], Some(compare.results[0].id));
}
#[test]
fn physical_formal_tracks_actual_read_slot_not_a_fixed_six_slot_plan() {
    let mut module = fixture::module(false);
    let body = module.functions[0].body.as_mut().unwrap();
    let operation=body.blocks[0].operations.iter_mut().find(|op|matches!(op.kind,OperationKind::Gfx942PhysicalEntryStep(s)if s.instruction.opcode==Gfx942PhysicalEntryOpcodeV20::LoadKernargDword&&s.instruction.immediate==24)).unwrap();
    let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind else {
        panic!("load")
    };
    step.instruction.immediate = 20;
    let owner = owner(&module);
    let report = report(&owner);
    assert_eq!(report.kernarg_reads()[4].byte_offset(), 20);
    assert_eq!(
        report.kernarg_reads()[4].slot(),
        PhysicalEntryKernargSlotV20::ScalarArgument(2)
    );
}
#[test]
fn generic_formal_does_not_turn_physical_memory_into_complete_empty() {
    let owner = owner(&fixture::module(false));
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
    assert!(generic.incomplete_reasons().iter().any(|r| matches!(
        r,
        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
    )));
    assert!(!report(&owner).output().accesses().is_empty());
}
#[test]
fn physical_formal_rejects_wrong_kernel_launch_width_and_preserves_budget_floor() {
    let owner = owner(&fixture::module(false));
    for (kernel, launch, width) in [
        (
            KernelId::new("other"),
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [128, 1, 1],
            },
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            ExplicitLaunchExtent::Unknown,
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [192, 1, 1],
            },
            FormalIndexWidth::Bits64,
        ),
        (
            owner.module().kernels[0].id.clone(),
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [128, 1, 1],
            },
            FormalIndexWidth::Bits32,
        ),
    ] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        budget.reserve_storage(127).unwrap();
        assert!(
            derive_physical_entry_memory_obligations_v20(
                &owner,
                &kernel,
                launch,
                width,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 127);
    }
}
#[test]
fn physical_formal_resource_refusal_keeps_existing_account() {
    let owner = owner(&fixture::module(false));
    for (work_limit, storage_limit) in [(0, 4_000_000), (1_000_000, 128)] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(127).unwrap();
        assert!(matches!(
            derive_physical_entry_memory_obligations_v20(
                &owner,
                &owner.module().kernels[0].id,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [128, 1, 1]
                },
                FormalIndexWidth::Bits64,
                &mut budget
            ),
            Err(PhysicalEntryMemoryErrorV20::Resource(_))
        ));
        assert_eq!(budget.storage(), 127);
    }
}
