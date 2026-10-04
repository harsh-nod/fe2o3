//! Actual verified V20 CPU observations. Fixture identities are inert, not source custody.
use fe2o3_kernel_ir as physical_entry_fixture_ir;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Gfx942PhysicalEntryOpcodeV20 as Op, Module, OperationKind, ScalarType,
    VerifiedCanonicalKernelIrModuleV20 as Owner,
};
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_entry_v20.rs"]
mod fixture;

fn owner(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v20(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_reachable_functions: 1,
        max_reachable_operations: 64,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 10_000,
        max_call_depth: 1,
        max_ssa_values: 160,
        max_allocations: 4,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_memory_access_records: 1024,
        ..SimulationLimitsV1::default()
    }
}
fn options(records: usize) -> PhysicalEntryDebugOptionsV20 {
    PhysicalEntryDebugOptionsV20::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 160, 4, 8192).unwrap(),
        records,
    )
    .unwrap()
}
fn request(selector: u32, length: usize) -> SimulationRequestV1 {
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; length * 4],
        vec![false; length * 4],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    SimulationRequestV1::new(
        "physical_fixture",
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(19)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(23)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(42)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(selector)),
        ],
    )
}
fn ledger(work: usize, storage: usize, floor: usize) -> Owned {
    let mut work = Work::new(work);
    work.charge_work(11).unwrap();
    let mut ledger = Owned::new(work, storage);
    ledger
        .with_budget(|budget| budget.reserve_storage(floor))
        .unwrap();
    ledger
}
fn setup(select: bool, edited: bool) -> (Owner, AdmittedSimulationModuleV1, usize) {
    let mut module = fixture::module(select);
    if edited {
        for block in &mut module.functions[0].body.as_mut().unwrap().blocks {
            for operation in &mut block.operations {
                if let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind {
                    if step.instruction.opcode == Op::VectorMove32
                        && step.instruction.destination == 8
                    {
                        step.instruction.destination = 22;
                    }
                    if step.instruction.opcode == Op::GlobalStoreDword {
                        step.instruction.source1 = 22;
                    }
                }
            }
        }
    }
    let (owner, bytes) = owner(&module);
    let admitted = AdmittedSimulationModuleV1::admit_v20(&owner, limits()).unwrap();
    let floor = 97 + bytes + admitted.admitted_resident_bytes();
    (owner, admitted, floor)
}
fn capture(
    select: bool,
    edited: bool,
    selector: u32,
    length: usize,
) -> PhysicalEntryDebugCaptureV20 {
    let (owner, admitted, floor) = setup(select, edited);
    admitted.capture_physical_entry_debug_v20(
        &owner,
        &request(selector, length),
        options(4096),
        ledger(100_000_000, 512 * 1024 * 1024, floor),
    )
}
fn check_last_memory(capture: &PhysicalEntryDebugCaptureV20, expected: u32, length: usize) {
    let record = (0..capture.len())
        .rev()
        .filter_map(|n| capture.record(n))
        .find(|r| r.phase().is_some())
        .unwrap();
    let (_allocation, bytes) = record.memory_allocation(0).unwrap();
    assert_eq!(bytes, length * 4);
    for offset in 0..bytes {
        let (byte, initialized) = record.memory_byte_at(0, offset).unwrap();
        assert_eq!(initialized, offset < 64 * 4);
        assert_eq!(
            byte,
            if offset < 64 * 4 {
                expected.to_le_bytes()[offset % 4]
            } else {
                0xa5
            }
        );
    }
}

#[test]
fn typed_capture_one_diamond_and_register_edit_use_real_engine_checkpoints() {
    for (select, edited, selector) in [
        (false, false, 0),
        (false, true, 0),
        (true, false, 0),
        (true, true, 1),
    ] {
        let capture = capture(select, edited, selector, 65);
        assert_eq!(capture.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
        assert_eq!(capture.error(), None);
        assert_eq!(capture.stop(), None);
        assert_eq!(capture.identity().wire_version(), 20);
        check_last_memory(&capture, if select && selector != 0 { 23 } else { 19 }, 65);
        let mut symbolic = 0;
        for index in 0..capture.len() {
            let row = capture.record(index).unwrap();
            for n in 0..row.binding_count(0).unwrap_or(0) {
                let value = row.binding(0, n).unwrap();
                if value.symbolic_kind().is_some() {
                    symbolic += 1;
                    assert!(value.scalar().is_none());
                    assert!(value.logical_pointer().is_none());
                }
            }
        }
        assert!(symbolic > 0);
        assert!(capture.usage().retained_storage > 0);
        let floor = capture.usage().entry_storage;
        assert_eq!(capture.into_budget().storage(), floor);
    }
}

#[test]
fn partial_and_empty_exec_still_capture_scalar_wait_restore_and_final_memory() {
    for length in [0, 1, 17, 64] {
        let capture = capture(false, false, 0, length);
        assert_eq!(capture.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
        assert_eq!(capture.stop(), None);
        let stores = (0..capture.len())
            .filter(|&n| capture.record(n).unwrap().is_committed_store())
            .count();
        assert_eq!(stores, length);
        check_last_memory(&capture, 19, length);
        let last = capture.record(capture.len() - 1).unwrap();
        assert_eq!(
            last.phase(),
            Some(SimulationDebugCheckpointPhaseV1::AfterOperation)
        );
        let module = fixture::module(false);
        let body = module.functions[0].body.as_ref().unwrap();
        let restore = body.blocks[0]
            .operations
            .iter()
            .enumerate()
            .find(|(_, op)| {
                matches!(&op.kind,OperationKind::Gfx942PhysicalEntryStep(step)
                if step.instruction.opcode==Op::RestoreExec)
            })
            .unwrap();
        assert_eq!(last.site().operation, restore.0 as u32);
        let restored = restore.1.results[0].id;
        assert_eq!(
            last.binding_count(0).unwrap(),
            body.parameters.len()
                + body.blocks[0]
                    .operations
                    .iter()
                    .map(|op| op.results.len())
                    .sum::<usize>()
        );
        // Inspect the actual restore result, not an older full-EXEC definition.
        assert!((0..last.binding_count(0).unwrap()).any(|n| {
            let value = last.binding(0, n).unwrap();
            value.value() == restored
                && value
                    .scalar()
                    .is_some_and(|s| s.ty() == ScalarType::U64 && s.bits() == u64::MAX as u128)
        }));
    }
}

#[test]
fn symbolic_carries_stay_unavailable_while_bounds_compare_produces_actual_vcc() {
    let capture = capture(false, false, 0, 17);
    let mut carried = false;
    let mut compared = false;
    for index in 0..capture.len() {
        let record = capture.record(index).unwrap();
        if record.phase() != Some(SimulationDebugCheckpointPhaseV1::AfterOperation)
            || record.invocation().global[0] != 0
        {
            continue;
        }
        for n in 0..record.binding_count(0).unwrap() {
            let value = record.binding(0, n).unwrap();
            if matches!(
                value.symbolic_kind(),
                Some(
                    PhysicalEntryDebugSymbolicKindV20::CarryLow
                        | PhysicalEntryDebugSymbolicKindV20::CarryHigh
                )
            ) {
                carried = true;
                assert_eq!(value.scalar(), None);
            }
            if value
                .scalar()
                .is_some_and(|v| v.ty() == ScalarType::U64 && v.bits() == (1u128 << 17) - 1)
            {
                compared = true;
            }
        }
    }
    assert!(carried && compared);
}

#[test]
fn same_owned_budget_exact_work_peak_one_below_and_nonzero_floor() {
    let (owner, admitted, floor) = setup(false, false);
    let req = request(0, 64);
    let run = |work, storage| {
        admitted.capture_physical_entry_debug_v20(
            &owner,
            &req,
            options(4096),
            ledger(work, storage, floor),
        )
    };
    let baseline = run(100_000_000, 512 * 1024 * 1024);
    assert_eq!(baseline.stop(), None);
    let usage = baseline.usage();
    let count = baseline.len();
    let exact = run(usage.work, usage.peak_storage);
    assert_eq!(exact.stop(), None);
    assert_eq!(exact.len(), count);
    assert_eq!(exact.usage(), usage);
    let short_work = run(usage.work - 1, usage.peak_storage);
    assert_eq!(
        short_work.outcome(),
        PhysicalEntryDebugOutcomeV20::Completed
    );
    assert!(matches!(
        short_work.stop(),
        Some(PhysicalEntryDebugCaptureStopV20::Resource(_))
    ));
    assert!(short_work.len() < count);
    let short_storage = run(usage.work, usage.peak_storage - 1);
    assert_eq!(
        short_storage.outcome(),
        PhysicalEntryDebugOutcomeV20::Completed
    );
    assert!(matches!(
        short_storage.stop(),
        Some(PhysicalEntryDebugCaptureStopV20::Resource(_))
    ));
    assert!(short_storage.len() < count);
    for capture in [baseline, exact, short_work, short_storage] {
        let usage = capture.usage();
        let returned = capture.into_budget();
        assert_eq!(returned.storage(), floor);
        assert_eq!(returned.work(), usage.work);
        assert_eq!(returned.failed_work(), usage.failed_work);
        assert_eq!(returned.failed_storage(), usage.failed_storage);
    }
}

#[test]
fn record_and_snapshot_limits_stop_observation_without_stopping_execution() {
    let (owner, admitted, floor) = setup(false, false);
    for records in [0, 1, 7] {
        let capture = admitted.capture_physical_entry_debug_v20(
            &owner,
            &request(0, 64),
            options(records),
            ledger(100_000_000, 512 * 1024 * 1024, floor),
        );
        assert_eq!(capture.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
        assert_eq!(capture.len(), records);
        assert_eq!(
            capture.stop(),
            Some(PhysicalEntryDebugCaptureStopV20::RecordLimit)
        );
    }
    let tiny = PhysicalEntryDebugOptionsV20::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 1, 4, 8192).unwrap(),
        4096,
    )
    .unwrap();
    let capture = admitted.capture_physical_entry_debug_v20(
        &owner,
        &request(0, 64),
        tiny,
        ledger(100_000_000, 512 * 1024 * 1024, floor),
    );
    assert_eq!(capture.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
    assert!(capture.is_empty());
    assert_eq!(
        capture.stop(),
        Some(PhysicalEntryDebugCaptureStopV20::SnapshotUnavailable)
    );
}

#[test]
fn substituted_owner_profile_and_failed_resource_entry_never_publish_records() {
    let (owner, admitted, floor) = setup(false, false);
    let (other, _, _) = setup(true, false);
    let mismatch = admitted.capture_physical_entry_debug_v20(
        &other,
        &request(0, 64),
        options(4096),
        ledger(100_000_000, 512 * 1024 * 1024, floor),
    );
    assert_eq!(
        mismatch.error(),
        Some(PhysicalEntryDebugCaptureErrorV20::OwnerMismatch)
    );
    assert!(mismatch.is_empty());
    let short = admitted.capture_physical_entry_debug_v20(
        &owner,
        &request(0, 64),
        options(4096),
        ledger(100_000_000, floor, floor),
    );
    assert!(matches!(
        short.error(),
        Some(PhysicalEntryDebugCaptureErrorV20::Resource(_))
    ));
    assert!(short.is_empty());
    assert_eq!(short.into_budget().storage(), floor);
    let mut req = request(0, 64);
    req.grid.0[0] = 63;
    let refused = admitted.capture_physical_entry_debug_v20(
        &owner,
        &req,
        options(4096),
        ledger(100_000_000, 512 * 1024 * 1024, floor),
    );
    assert_eq!(
        refused.outcome(),
        PhysicalEntryDebugOutcomeV20::PreflightRefused
    );
    assert!(refused.is_empty());
}

#[test]
fn denial_history_is_retained_and_legacy_debug_entry_remains_refused() {
    let (owner, admitted, floor) = setup(false, false);
    let mut ledger = ledger(100_000_000, 512 * 1024 * 1024, floor);
    ledger.with_budget(|budget| {
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
    });
    let capture =
        admitted.capture_physical_entry_debug_v20(&owner, &request(0, 64), options(4096), ledger);
    assert_eq!(capture.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
    assert_eq!(capture.usage().failed_storage, Some(usize::MAX));
    assert_eq!(capture.usage().failed_work, Some(usize::MAX));
    let mut sink = NoopSimulationDebugSinkV1;
    assert!(matches!(
        admitted.simulate_debugged_with_sink(
            &request(0, 64),
            SimulationTargetV1::amdgpu_64(),
            limits(),
            SimulationDebugCaptureLimitsV1::new(1, 160, 4, 8192).unwrap(),
            &mut sink
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20
        ))
    ));
}

#[test]
fn ordinary_v20_owner_is_not_promoted_to_physical_capture() {
    let mut module = fixture::module(false);
    module.required_capabilities.clear();
    module.kernels[0].required_capabilities.clear();
    module.functions[0].required_capabilities.clear();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.clear();
    let (owner, bytes) = owner(&module);
    let admitted = AdmittedSimulationModuleV1::admit_v20(&owner, limits()).unwrap();
    let floor = 97 + bytes + admitted.admitted_resident_bytes();
    let capture = admitted.capture_physical_entry_debug_v20(
        &owner,
        &request(0, 64),
        options(4096),
        ledger(100_000_000, 512 * 1024 * 1024, floor),
    );
    assert_eq!(
        capture.error(),
        Some(PhysicalEntryDebugCaptureErrorV20::NotPhysicalEntry)
    );
    assert!(capture.is_empty());
    assert_eq!(capture.into_budget().storage(), floor);
}

#[test]
fn wrong_pointer_half_and_carry_are_refused_before_any_typed_capture_can_exist() {
    for operand in [0, 3] {
        let mut module = fixture::module(false);
        let body = module.functions[0].body.as_mut().unwrap();
        let operation = body.blocks[0]
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(&operation.kind,OperationKind::Gfx942PhysicalEntryStep(step)
                if step.instruction.opcode==Op::VectorAddCarryIn)
            })
            .unwrap();
        let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind else {
            unreachable!()
        };
        // Both substitutions have the correct scalar width, but wrong exact SSA/register origin.
        step.operands[operand] = if operand == 0 {
            step.operands[1]
        } else {
            step.operands[2]
        };
        let mut work = Work::new(16_000_000);
        let mut budget = Budget::new(&mut work, 16_000_000);
        budget.reserve_storage(97).unwrap();
        assert!(Owner::from_module_ref_with_verification_budget_v20(&module, &mut budget).is_err());
        assert_eq!(budget.storage(), 97);
    }
}
