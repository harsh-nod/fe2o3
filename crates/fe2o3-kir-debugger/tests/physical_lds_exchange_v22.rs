//! Shared inert canonical fixture, real Engine observations; no Rust/native/source custody.
use fe2o3_kernel_ir as physical_lds_exchange_fixture_ir;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942PhysicalLdsExchangeOpcodeV1 as Op, Module,
    OperationKind, ScalarType, ValueId, VerifiedCanonicalKernelIrModuleV22 as Owner,
};
use fe2o3_kir_debugger::{
    PhysicalLdsExchangeDebugNavigationV22 as Nav, PhysicalLdsExchangeDebugSessionV22 as Session,
};
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_lds_exchange_v22.rs"]
mod fixture;
const WORK: usize = 512 * 1024 * 1024;
const STORAGE: usize = 512 * 1024 * 1024;
fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_reachable_functions: 1,
        max_reachable_operations: 64,
        max_invocations: 128,
        max_workgroups: 1,
        max_scheduled_slots: 128,
        max_steps: 10000,
        max_call_depth: 1,
        max_ssa_values: 192,
        max_allocations: 4,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_memory_access_records: 1024,
        ..SimulationLimitsV1::default()
    }
}
fn options(records: usize) -> PhysicalLdsExchangeDebugOptionsV22 {
    PhysicalLdsExchangeDebugOptionsV22::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 192, 4, 8192).unwrap(),
        records,
    )
    .unwrap()
}
fn edited_module(edited: bool) -> Module {
    if edited {
        fixture::module_with_registers(true)
    } else {
        fixture::module()
    }
}
fn prepared(
    work: usize,
    storage: usize,
    edited: bool,
) -> (Owner, AdmittedSimulationModuleV1, Owned) {
    let mut ledger = Owned::new(Work::new(work), storage);
    ledger
        .with_budget(|b| {
            b.reserve_storage(73)?;
            b.charge_work(11)
        })
        .unwrap();
    let (owner, owner_receipt) = ledger
        .with_budget(|b| {
            Owner::from_module_ref_with_verification_budget_v22(&edited_module(edited), b)
        })
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(owner_receipt.retained_storage()))
        .unwrap();
    let (module, view_receipt) = ledger
        .with_budget(|b| {
            AdmittedSimulationModuleV1::admit_v22_with_verification_budget(&owner, limits(), b)
        })
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(view_receipt.retained_storage()))
        .unwrap();
    (owner, module, ledger)
}
fn word(index: usize) -> u32 {
    (index as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x8000_0001)
}
fn request(
    input_len: usize,
    output_len: usize,
    uninitialized: Option<usize>,
) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let mut input = vec![0x5a; (input_len + 4) * 4];
    for index in 0..input_len {
        input[(index + 2) * 4..(index + 3) * 4].copy_from_slice(&word(index).to_le_bytes());
    }
    let mut initialized = vec![true; input.len()];
    if let Some(i) = uninitialized {
        initialized[(i + 2) * 4..(i + 3) * 4].fill(false);
    }
    let input = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        input,
        initialized,
        target,
    )
    .unwrap();
    let output = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; (output_len + 4) * 4],
        vec![false; (output_len + 4) * 4],
        target,
    )
    .unwrap();
    SimulationRequestV1::new(
        "physical_lds_exchange_fixture",
        [128, 1, 1],
        [128, 1, 1],
        [
            (7, AccessMode::ReadOnly, input_len),
            (9, AccessMode::ReadWrite, output_len),
        ]
        .into_iter()
        .map(|(id, access, len)| {
            SimulationArgumentV1::BufferView(
                BufferViewArgumentV1::new(
                    BufferBackingIdV1(id),
                    ScalarType::U32,
                    access,
                    4,
                    8,
                    len,
                    target,
                )
                .unwrap(),
            )
        })
        .collect(),
    )
    .with_shared_buffers(vec![
        SharedBufferV1 {
            id: BufferBackingIdV1(7),
            buffer: input,
        },
        SharedBufferV1 {
            id: BufferBackingIdV1(9),
            buffer: output,
        },
    ])
}
fn capture(
    work: usize,
    storage: usize,
    edited: bool,
    req: &SimulationRequestV1,
    records: usize,
) -> PhysicalLdsExchangeDebugCaptureV22 {
    let (owner, module, ledger) = prepared(work, storage, edited);
    module.capture_physical_lds_exchange_debug_v22(&owner, req, options(records), ledger)
}
fn session(work: usize, records: usize) -> Session {
    let (owner, module, ledger) = prepared(work, STORAGE, false);
    Session::capture(
        &module,
        &owner,
        &request(128, 129, None),
        options(records),
        ledger,
    )
}
fn binding<'a>(
    record: PhysicalLdsExchangeDebugRecordRefV22<'a>,
    id: ValueId,
) -> Option<PhysicalLdsExchangeDebugBindingRefV22<'a>> {
    (0..record.binding_count(0)?)
        .filter_map(|n| record.binding(0, n))
        .find(|b| b.value() == id)
}
fn checkpoint(
    capture: &PhysicalLdsExchangeDebugCaptureV22,
    operation: u32,
    phase: SimulationDebugCheckpointPhaseV1,
) -> PhysicalLdsExchangeDebugRecordRefV22<'_> {
    (0..capture.len())
        .filter_map(|n| capture.record(n))
        .find(|r| {
            r.invocation().global == [0, 0, 0]
                && r.site().operation == operation
                && r.phase() == Some(phase)
        })
        .unwrap()
}
fn site(module: &Module, opcode: Op) -> (u32, ValueId) {
    let (index,op)=module.functions[0].body.as_ref().unwrap().blocks[0].operations.iter().enumerate().find(|(_,o)|
        matches!(&o.kind,OperationKind::Gfx942PhysicalLdsExchangeStep(s) if s.instruction.opcode==opcode)).unwrap();
    (
        index as u32,
        op.results.first().map_or(ValueId(u32::MAX), |r| r.id),
    )
}
fn check_final(
    capture: &PhysicalLdsExchangeDebugCaptureV22,
    req: &SimulationRequestV1,
    output_len: usize,
) {
    let record = (0..capture.len())
        .rev()
        .filter_map(|n| capture.record(n))
        .find(|r| r.phase().is_some())
        .unwrap();
    assert_eq!(
        record.memory_address_space(2),
        Some(fe2o3_kernel_ir::AddressSpace::Workgroup)
    );
    assert_eq!(record.memory_allocation(2).unwrap().1, 512);
    assert_eq!(record.memory_allocation(3), None);
    for i in 0..512 {
        assert_eq!(
            record.memory_byte_at(2, i),
            Some((word(i / 4).to_le_bytes()[i % 4], true))
        );
    }
    for allocation in 0..2 {
        assert_eq!(
            record.memory_address_space(allocation),
            Some(fe2o3_kernel_ir::AddressSpace::Global)
        );
        let original = &req.shared_buffers[allocation].buffer;
        assert_eq!(
            record.memory_allocation(allocation).unwrap().1,
            original.bytes().len()
        );
        for i in 0..original.bytes().len() {
            let written = allocation == 1 && (8..8 + 4 * output_len.min(128)).contains(&i);
            let expected = if written {
                word(((i - 8) / 4) ^ 64).to_le_bytes()[i % 4]
            } else {
                original.bytes()[i]
            };
            assert_eq!(
                record.memory_byte_at(allocation, i),
                Some((
                    expected,
                    if written {
                        true
                    } else {
                        original.initialized()[i]
                    }
                ))
            );
        }
    }
}

#[test]
fn typed_capture_retains_three_allocations_and_both_waves_with_edited_registers() {
    for edited in [false, true] {
        let req = request(128, 129, None);
        let original = req.clone();
        let c = capture(WORK, STORAGE, edited, &req, 16384);
        assert_eq!(c.identity().wire_version(), 22);
        assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
        assert_eq!(c.error(), None);
        assert_eq!(c.stop(), None);
        check_final(&c, &req, 129);
        assert_eq!(req, original);
        let before = checkpoint(&c, 0, SimulationDebugCheckpointPhaseV1::BeforeOperation);
        assert_eq!(before.memory_allocation(2), None);
        let after = checkpoint(&c, 0, SimulationDebugCheckpointPhaseV1::AfterOperation);
        assert_eq!(
            after.memory_address_space(2),
            Some(fe2o3_kernel_ir::AddressSpace::Workgroup)
        );
        for i in 0..512 {
            assert!(!after.memory_byte_at(2, i).unwrap().1);
        }
        let mut lanes = [false; 128];
        for r in (0..c.len())
            .filter_map(|n| c.record(n))
            .filter(|r| r.phase().is_some())
        {
            lanes[r.invocation().local[0] as usize] = true;
        }
        assert!(lanes.into_iter().all(|seen| seen));
        let u = c.usage();
        let b = c.into_budget();
        assert_eq!(b.storage(), u.entry_storage);
        assert_eq!(b.work(), u.work);
        assert_eq!(b.peak_storage(), u.peak_storage);
    }
}
#[test]
fn lds_completion_is_distinct_from_full_group_publication_and_peer_read() {
    use SimulationDebugBarrierActionV1::{Arrive, Release};
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    use SimulationDebugMemoryAccessV1::{Read, WriteCommitted};
    use fe2o3_kernel_ir::AddressSpace::Workgroup;
    let c = capture(WORK, STORAGE, false, &request(128, 129, None), 16384);
    assert_eq!(c.stop(), None);
    let (write, _) = site(&fixture::module(), Op::LdsWriteB32);
    for (operation, phase) in [(write, Before), (write, After), (write + 1, Before)] {
        assert_eq!(
            checkpoint(&c, operation, phase)
                .memory_byte_at(2, 0)
                .unwrap()
                .1,
            false
        );
    }
    assert_eq!(
        checkpoint(&c, write + 1, After).memory_byte_at(2, 0),
        Some((word(0).to_le_bytes()[0], true))
    );
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let mut arrivals = Vec::new();
    let mut releases = Vec::new();
    for index in 0..c.len() {
        let r = c.record(index).unwrap();
        if let Some((access, _, offset, len, space)) = r.memory_access() {
            if space == Workgroup {
                assert_eq!(len, 4);
                let local = r.invocation().local[0] as usize;
                match access {
                    WriteCommitted => {
                        assert_eq!(offset, local * 4);
                        assert_eq!(r.site().operation, write);
                        writes.push(index);
                    }
                    Read => {
                        assert_eq!(offset, (local ^ 64) * 4);
                        reads.push(index);
                    }
                    _ => panic!("no LDS atomics in exact profile"),
                }
            }
        }
        if let Some((action, phase, participants)) = r.barrier() {
            assert_eq!(phase, 0);
            match action {
                Arrive => {
                    assert_eq!(participants, 1);
                    arrivals.push(index);
                }
                Release => {
                    assert_eq!(participants, 128);
                    releases.push(index);
                }
            }
        }
    }
    assert_eq!(writes.len(), 128);
    assert_eq!(reads.len(), 128);
    assert_eq!(arrivals.len(), 128);
    assert_eq!(releases.len(), 1);
    let release = releases[0];
    assert!(writes.into_iter().all(|i| i < release));
    assert!(arrivals.into_iter().all(|i| i < release));
    assert!(reads.into_iter().all(|i| i > release));
}
#[test]
fn global_and_lds_pending_values_become_ready_only_at_actual_matching_wait() {
    use PhysicalLdsExchangeDebugSymbolicKindV22 as K;
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    let m = fixture::module();
    let c = capture(WORK, STORAGE, false, &request(128, 129, None), 16384);
    assert_eq!(c.stop(), None);
    for (opcode, kind, ready) in [
        (Op::GlobalLoadDword, K::PendingGlobalRead, word(0)),
        (Op::LdsReadB32, K::PendingLdsRead, word(64)),
    ] {
        let (load, id) = site(&m, opcode);
        assert!(binding(checkpoint(&c, load, Before), id).is_none());
        for (operation, phase) in [(load, After), (load + 1, Before)] {
            let b = binding(checkpoint(&c, operation, phase), id).unwrap();
            assert_eq!(b.symbolic_kind(), Some(kind));
            assert_eq!(b.scalar(), None);
            assert_eq!(b.logical_pointer(), None);
        }
        let b = binding(checkpoint(&c, load + 1, After), id).unwrap();
        assert_eq!(b.value(), id);
        assert_eq!(b.symbolic_kind(), None);
        assert_eq!(b.scalar(), Some(ScalarBitsV1::u32(ready)));
        let count = (0..c.len())
            .filter_map(|i| c.record(i))
            .filter(|r| r.phase() == Some(After) && r.site().operation == load + 1)
            .filter(|r| {
                let local = r.invocation().local[0] as usize;
                binding(*r, id).unwrap().scalar()
                    == Some(ScalarBitsV1::u32(word(if opcode == Op::LdsReadB32 {
                        local ^ 64
                    } else {
                        local
                    })))
            })
            .count();
        assert_eq!(count, 128);
    }
    for r in (0..c.len()).filter_map(|i| c.record(i)) {
        for b in (0..r.binding_count(0).unwrap_or(0)).filter_map(|i| r.binding(0, i)) {
            if b.symbolic_kind().is_some() {
                assert_eq!(b.scalar(), None);
                assert_eq!(b.logical_pointer(), None);
            }
        }
    }
}
#[test]
fn masked_output_keeps_full_input_and_lds_exchange_with_canaries() {
    for output in [0, 1, 63, 64, 65, 127] {
        let req = request(128, output, None);
        let c = capture(WORK, STORAGE, false, &req, 16384);
        assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
        assert_eq!(c.stop(), None);
        check_final(&c, &req, output);
        let global_writes = (0..c.len())
            .filter_map(|i| c.record(i))
            .filter(|r| {
                matches!(
                    r.memory_access(),
                    Some((
                        SimulationDebugMemoryAccessV1::WriteCommitted,
                        _,
                        _,
                        _,
                        fe2o3_kernel_ir::AddressSpace::Global
                    ))
                )
            })
            .count();
        assert_eq!(global_writes, output);
    }
}
#[test]
fn uninitialized_and_short_input_fail_even_with_zero_output() {
    for (req, uninitialized) in [
        (request(128, 0, Some(127)), true),
        (request(127, 0, None), false),
    ] {
        let (_, module, _) = prepared(WORK, STORAGE, false);
        let Err(SimulationErrorV1::Execution(error)) =
            module.simulate(&req, SimulationTargetV1::amdgpu_64(), limits())
        else {
            panic!("exact full-EXEC input refusal");
        };
        if uninitialized {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UninitializedRead { .. }
            ));
        } else {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::OutOfBounds { .. }
            ));
        }
        let c = capture(WORK, STORAGE, false, &req, 16384);
        assert_eq!(
            c.outcome(),
            PhysicalLdsExchangeDebugOutcomeV22::ExecutionFailed
        );
        assert!(!(0..c.len()).filter_map(|i| c.record(i)).any(|r| matches!(
            r.memory_access(),
            Some((
                SimulationDebugMemoryAccessV1::WriteCommitted,
                _,
                _,
                _,
                fe2o3_kernel_ir::AddressSpace::Global
            ))
        )));
        assert_eq!(c.error(), None);
        let floor = c.usage().entry_storage;
        assert_eq!(c.into_budget().storage(), floor);
    }
}
#[test]
fn same_backing_and_wrong_workgroup_refuse_before_capture() {
    for offset in [8, 520] {
        let mut req = request(128, 128, None);
        req.shared_buffers[0].buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0; 1040],
            vec![true; 1040],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        req.arguments[1] = SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                BufferBackingIdV1(7),
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                offset,
                128,
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        );
        assert_eq!(8 + 128 * 4 <= offset, offset == 520);
        let (_, module, _) = prepared(WORK, STORAGE, false);
        assert!(matches!(
            module.preflight(&req, SimulationTargetV1::amdgpu_64(), limits()),
            Err(SimulationPreflightErrorV1::PhysicalLdsExchangeAliasedArgumentsV22)
        ));
        let c = capture(WORK, STORAGE, false, &req, 16384);
        assert_eq!(
            c.outcome(),
            PhysicalLdsExchangeDebugOutcomeV22::PreflightRefused
        );
        assert!(c.is_empty());
    }
    let mut req = request(128, 128, None);
    req.workgroup = WorkgroupShapeV1([64, 1, 1]);
    let c = capture(WORK, STORAGE, false, &req, 16384);
    assert_eq!(
        c.outcome(),
        PhysicalLdsExchangeDebugOutcomeV22::PreflightRefused
    );
    assert!(c.is_empty());
}
#[test]
fn exact_cumulative_resources_one_short_return_floor_without_resetting_denials() {
    let req = request(128, 129, None);
    let baseline = capture(WORK, STORAGE, false, &req, 16384);
    assert_eq!(baseline.stop(), None);
    let u = baseline.usage();
    let count = baseline.len();
    let exact = capture(u.work, u.peak_storage, false, &req, 16384);
    assert_eq!(exact.usage(), u);
    assert_eq!(exact.len(), count);
    let sw = capture(u.work - 1, u.peak_storage, false, &req, 16384);
    let ss = capture(u.work, u.peak_storage - 1, false, &req, 16384);
    for c in [&sw, &ss] {
        assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
        assert!(matches!(
            c.stop(),
            Some(PhysicalLdsExchangeDebugCaptureStopV22::Resource(_))
        ));
        assert!(c.len() < count);
    }
    assert!(sw.usage().failed_work.is_some());
    assert!(ss.usage().failed_storage.is_some());
    for c in [baseline, exact, sw, ss] {
        let u = c.usage();
        let b = c.into_budget();
        assert_eq!(b.storage(), u.entry_storage);
        assert_eq!(b.work(), u.work);
        assert_eq!(b.peak_storage(), u.peak_storage);
        assert_eq!(b.failed_work(), u.failed_work);
        assert_eq!(b.failed_storage(), u.failed_storage);
    }
}
#[test]
fn foreign_owner_legacy_debug_and_small_capture_limits_do_not_open_authority() {
    let (owner, module, ledger) = prepared(WORK, STORAGE, false);
    let (other, _, _) = prepared(WORK, STORAGE, true);
    assert_ne!(owner.identity(), other.identity());
    let req = request(128, 129, None);
    let c = module.capture_physical_lds_exchange_debug_v22(&other, &req, options(16384), ledger);
    assert_eq!(
        c.error(),
        Some(PhysicalLdsExchangeDebugCaptureErrorV22::OwnerMismatch)
    );
    assert!(c.is_empty());
    let mut sink = NoopSimulationDebugSinkV1;
    assert!(matches!(
        module.simulate_debugged_with_sink(
            &req,
            SimulationTargetV1::amdgpu_64(),
            limits(),
            SimulationDebugCaptureLimitsV1::new(1, 192, 4, 8192).unwrap(),
            &mut sink
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::PhysicalLdsExchangeDebugUnavailableV22
        ))
    ));
    for records in [0, 1, 7] {
        let c = capture(WORK, STORAGE, false, &req, records);
        assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
        assert_eq!(c.len(), records);
        assert_eq!(
            c.stop(),
            Some(PhysicalLdsExchangeDebugCaptureStopV22::RecordLimit)
        );
    }
    let (owner, module, ledger) = prepared(WORK, STORAGE, false);
    let tiny = PhysicalLdsExchangeDebugOptionsV22::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 192, 2, 8192).unwrap(),
        16384,
    )
    .unwrap();
    let c = module.capture_physical_lds_exchange_debug_v22(&owner, &req, tiny, ledger);
    assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
    assert_eq!(
        c.stop(),
        Some(PhysicalLdsExchangeDebugCaptureStopV22::SnapshotUnavailable)
    );
}
#[test]
fn reverse_selects_actual_pending_ready_and_store_observations_only() {
    let mut s = session(WORK, 16384);
    assert_eq!(s.capture_stop(), None);
    let m = fixture::module();
    let (load, id) = site(&m, Op::LdsReadB32);
    let (store, _) = site(&m, Op::GlobalStoreDword);
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    let locate = |s: &Session, op, phase| {
        (0..s.records_len())
            .find(|&n| {
                let r = s.record(n).unwrap();
                r.invocation().local == [0, 0, 0]
                    && r.site().operation == op
                    && r.phase() == Some(phase)
            })
            .unwrap()
    };
    let pending = locate(&s, load, After);
    let ready = locate(&s, load + 1, After);
    for index in [ready, pending, ready] {
        assert!(matches!(s.seek(index), Nav::Record { .. }));
        let b = binding(s.current().unwrap(), id).unwrap();
        assert_eq!(b.scalar().is_some(), index == ready);
        assert_eq!(b.symbolic_kind().is_some(), index == pending);
    }
    let before = locate(&s, store, Before);
    let after = locate(&s, store, After);
    for (index, byte, init) in [
        (after, word(64).to_le_bytes()[0], true),
        (before, 0xa5, false),
        (after, word(64).to_le_bytes()[0], true),
    ] {
        assert!(matches!(s.seek(index), Nav::Record { .. }));
        assert_eq!(
            s.current().unwrap().memory_byte_at(1, 8),
            Some((byte, init))
        );
    }
    assert!(matches!(s.step_reverse(), Nav::Record { .. }));
    assert!(s.current().unwrap().is_committed_store());
    assert!(matches!(s.step_reverse(),Nav::Record{index,..}if index==before));
    let u = s.usage();
    let b = s.into_budget();
    assert_eq!(b.storage(), u.entry_storage);
}
#[test]
fn invalid_incomplete_and_budget_denied_navigation_is_transactional() {
    let mut partial = session(WORK, 7);
    assert!(matches!(partial.seek(6), Nav::Record { .. }));
    assert_eq!(partial.seek(7), Nav::Incomplete);
    assert_eq!(partial.cursor(), Some(6));
    assert_eq!(partial.seek(8), Nav::Unavailable);
    assert_eq!(partial.cursor(), Some(6));
    let work = session(WORK, 16384).usage().work;
    let mut s = session(work + 1, 16384);
    assert!(matches!(s.seek(0), Nav::Record { index: 0, .. }));
    let u = s.usage();
    assert_eq!(s.step_forward(), Nav::Unavailable);
    assert_eq!(s.cursor(), Some(0));
    assert_eq!(s.usage().work, u.work);
    assert!(s.usage().failed_work.is_some());
    let floor = s.usage().entry_storage;
    assert_eq!(s.into_budget().storage(), floor);
    assert!(
        std::mem::size_of::<Session>()
            <= std::mem::size_of::<PhysicalLdsExchangeDebugCaptureV22>()
                + std::mem::size_of::<Option<usize>>()
    );
}

#[test]
fn inherited_denials_and_failed_bootstrap_keep_original_floor_and_history() {
    let (owner, module, mut ledger) = prepared(WORK, STORAGE, false);
    ledger.with_budget(|b| {
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
    });
    let c = module.capture_physical_lds_exchange_debug_v22(
        &owner,
        &request(128, 129, None),
        options(16384),
        ledger,
    );
    assert_eq!(c.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
    assert_eq!(c.stop(), None);
    assert_eq!(c.usage().failed_work, Some(usize::MAX));
    assert_eq!(c.usage().failed_storage, Some(usize::MAX));
    let u = c.usage();
    let b = c.into_budget();
    assert_eq!(b.storage(), u.entry_storage);
    assert_eq!(b.failed_work(), u.failed_work);
    assert_eq!(b.failed_storage(), u.failed_storage);
    let (owner, module, mut ledger) = prepared(WORK, STORAGE, false);
    ledger
        .with_budget(|b| b.reserve_storage(STORAGE - b.storage()))
        .unwrap();
    let floor = ledger.storage();
    let c = module.capture_physical_lds_exchange_debug_v22(
        &owner,
        &request(128, 129, None),
        options(16384),
        ledger,
    );
    assert!(matches!(
        c.error(),
        Some(PhysicalLdsExchangeDebugCaptureErrorV22::Resource(_))
    ));
    assert!(c.is_empty());
    assert_eq!(c.into_budget().storage(), floor);
}
