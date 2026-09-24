//! Shared inert canonical fixture, real Engine observations; no Rust/native/source custody.
use fe2o3_kernel_ir as physical_global_copy_fixture_ir;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942PhysicalGlobalCopyOpcodeV1 as Op, Module,
    OperationKind, ScalarType, ValueId, VerifiedCanonicalKernelIrModuleV21 as Owner,
};
use fe2o3_kir_debugger::{
    PhysicalGlobalCopyDebugNavigationV21 as Nav, PhysicalGlobalCopyDebugSessionV21 as Session,
};
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_global_copy_v21.rs"]
mod fixture;
const WORK: usize = 512 * 1024 * 1024;
const STORAGE: usize = 512 * 1024 * 1024;
fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_reachable_functions: 1,
        max_reachable_operations: 64,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 10000,
        max_call_depth: 1,
        max_ssa_values: 160,
        max_allocations: 4,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_memory_access_records: 1024,
        ..SimulationLimitsV1::default()
    }
}
fn options(records: usize) -> PhysicalGlobalCopyDebugOptionsV21 {
    PhysicalGlobalCopyDebugOptionsV21::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 160, 4, 8192).unwrap(),
        records,
    )
    .unwrap()
}
fn edited_module(edited: bool) -> Module {
    let mut module = fixture::module();
    if edited {
        for op in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
            if let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut op.kind {
                match step.instruction.opcode {
                    Op::GlobalLoadDword => step.instruction.destination = 22,
                    Op::GlobalStoreDword => step.instruction.source1 = 22,
                    _ => {}
                }
            }
        }
    }
    module
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
            Owner::from_module_ref_with_verification_budget_v21(&edited_module(edited), b)
        })
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(owner_receipt.retained_storage()))
        .unwrap();
    let (module, view_receipt) = ledger
        .with_budget(|b| {
            AdmittedSimulationModuleV1::admit_v21_with_verification_budget(&owner, limits(), b)
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
        "physical_global_copy_fixture",
        [64, 1, 1],
        [64, 1, 1],
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
) -> PhysicalGlobalCopyDebugCaptureV21 {
    let (owner, module, ledger) = prepared(work, storage, edited);
    module.capture_physical_global_copy_debug_v21(&owner, req, options(records), ledger)
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
    record: PhysicalGlobalCopyDebugRecordRefV21<'a>,
    id: ValueId,
) -> Option<PhysicalGlobalCopyDebugBindingRefV21<'a>> {
    (0..record.binding_count(0)?)
        .filter_map(|n| record.binding(0, n))
        .find(|b| b.value() == id)
}
fn checkpoint(
    capture: &PhysicalGlobalCopyDebugCaptureV21,
    operation: u32,
    phase: SimulationDebugCheckpointPhaseV1,
) -> PhysicalGlobalCopyDebugRecordRefV21<'_> {
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
        matches!(&o.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s) if s.instruction.opcode==opcode)).unwrap();
    (
        index as u32,
        op.results.first().map_or(ValueId(u32::MAX), |r| r.id),
    )
}
fn check_final(
    capture: &PhysicalGlobalCopyDebugCaptureV21,
    req: &SimulationRequestV1,
    output_len: usize,
) {
    let record = (0..capture.len())
        .rev()
        .filter_map(|n| capture.record(n))
        .find(|r| r.phase().is_some())
        .unwrap();
    assert_eq!(record.memory_allocation(2), None);
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
            let written = allocation == 1 && (8..8 + 4 * output_len.min(64)).contains(&i);
            let expected = if written {
                word((i - 8) / 4).to_le_bytes()[i % 4]
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
fn typed_copy_capture_retains_both_allocations_register_edit_and_tail_canaries() {
    for edited in [false, true] {
        let req = request(128, 129, None);
        let original = req.clone();
        let c = capture(WORK, STORAGE, edited, &req, 8192);
        assert_eq!(c.identity().wire_version(), 21);
        assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
        assert_eq!(c.error(), None);
        assert_eq!(c.stop(), None);
        check_final(&c, &req, 129);
        assert_eq!(req, original);
        assert_eq!(
            (0..c.len())
                .filter(|&n| c.record(n).unwrap().is_committed_store())
                .count(),
            64
        );
        let floor = c.usage().entry_storage;
        assert!(floor > 73);
        assert_eq!(c.into_budget().storage(), floor);
    }
}
#[test]
fn pending_kernarg_and_read_binding_stay_opaque_until_actual_wait_updates_same_ssa() {
    use PhysicalGlobalCopyDebugSymbolicKindV21 as K;
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    let module = fixture::module();
    let c = capture(WORK, STORAGE, false, &request(128, 129, None), 8192);
    assert_eq!(c.stop(), None);
    let (load, id) = site(&module, Op::GlobalLoadDword);
    assert!(binding(checkpoint(&c, load, Before), id).is_none());
    for (operation, phase) in [(load, After), (load + 1, Before)] {
        let b = binding(checkpoint(&c, operation, phase), id).unwrap();
        assert_eq!(b.symbolic_kind(), Some(K::PendingGlobalRead));
        assert_eq!(b.scalar(), None);
        assert_eq!(b.logical_pointer(), None);
    }
    let ready = binding(checkpoint(&c, load + 1, After), id).unwrap();
    assert_eq!(ready.value(), id);
    assert_eq!(ready.scalar(), Some(ScalarBitsV1::u32(word(0))));
    assert_eq!(ready.symbolic_kind(), None);
    for (operation, pending_kind, length) in [
        (1, K::PendingPointerLow, None),
        (2, K::PendingLengthLow, Some(128)),
        (3, K::PendingPointerLow, None),
        (4, K::PendingLengthLow, Some(129)),
    ] {
        let id = module.functions[0].body.as_ref().unwrap().blocks[0].operations[operation].results
            [0]
        .id;
        let pending = binding(checkpoint(&c, operation as u32, After), id).unwrap();
        assert_eq!(pending.symbolic_kind(), Some(pending_kind));
        assert_eq!(pending.scalar(), None);
        let ready = binding(checkpoint(&c, 5, After), id).unwrap();
        assert_eq!(ready.scalar(), length.map(ScalarBitsV1::u32));
        assert_eq!(
            ready.symbolic_kind(),
            length.is_none().then_some(K::PointerLow)
        );
    }
    let (store, _) = site(&module, Op::GlobalStoreDword);
    for phase in [Before, After] {
        // The existing Engine commits the store before the authored VM wait;
        // the wait is still a real checkpoint, not a fabricated completion event.
        assert_eq!(
            checkpoint(&c, store + 1, phase).memory_byte_at(1, 8),
            Some((word(0).to_le_bytes()[0], true))
        );
    }
    for row in (0..c.len()).filter_map(|i| c.record(i)) {
        for b in (0..row.binding_count(0).unwrap_or(0)).filter_map(|i| row.binding(0, i)) {
            if b.symbolic_kind().is_some() {
                assert_eq!(b.scalar(), None);
                assert_eq!(b.logical_pointer(), None);
            }
        }
    }
}
#[test]
fn zero_and_partial_output_masks_do_not_suppress_full_exec_reads() {
    for output in [0, 1, 13, 64] {
        let req = request(128, output, None);
        let c = capture(WORK, STORAGE, false, &req, 8192);
        assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
        assert_eq!(c.stop(), None);
        check_final(&c, &req, output);
        assert_eq!(
            (0..c.len())
                .filter(|&n| c.record(n).unwrap().is_committed_store())
                .count(),
            output
        );
        let module = fixture::module();
        let (load, id) = site(&module, Op::GlobalLoadDword);
        let ready = (0..c.len())
            .filter_map(|n| c.record(n))
            .filter(|r| {
                r.phase() == Some(SimulationDebugCheckpointPhaseV1::AfterOperation)
                    && r.site().operation == load + 1
            })
            .filter(|r| {
                binding(*r, id).unwrap().scalar()
                    == Some(ScalarBitsV1::u32(word(r.invocation().global[0] as usize)))
            })
            .count();
        assert_eq!(ready, 64);
    }
}
#[test]
fn uninitialized_or_short_input_fails_even_when_no_output_lane_is_active() {
    for (req, uninitialized) in [
        (request(128, 0, Some(63)), true),
        (request(63, 0, None), false),
    ] {
        let (owner, module, ledger) = prepared(WORK, STORAGE, false);
        let Err(SimulationErrorV1::Execution(error)) =
            module.simulate(&req, SimulationTargetV1::amdgpu_64(), limits())
        else {
            panic!("actual full-EXEC input refusal");
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
        let c = module.capture_physical_global_copy_debug_v21(&owner, &req, options(8192), ledger);
        assert_eq!(
            c.outcome(),
            PhysicalGlobalCopyDebugOutcomeV21::ExecutionFailed
        );
        assert_eq!(
            (0..c.len())
                .filter(|&n| c.record(n).unwrap().is_committed_store())
                .count(),
            0
        );
        assert_eq!(c.error(), None);
        let floor = c.usage().entry_storage;
        assert_eq!(c.into_budget().storage(), floor);
    }
}
#[test]
fn same_backing_refuses_overlap_and_nonoverlap_before_any_capture() {
    for offset in [8, 264] {
        let mut req = request(64, 64, None);
        req.shared_buffers[0].buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0; 1024],
            vec![true; 1024],
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
                64,
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        );
        assert_eq!(8 + 64 * 4 <= offset, offset == 264);
        let (owner, module, ledger) = prepared(WORK, STORAGE, false);
        assert!(matches!(
            module.preflight(&req, SimulationTargetV1::amdgpu_64(), limits()),
            Err(SimulationPreflightErrorV1::PhysicalGlobalCopyAliasedArgumentsV21)
        ));
        let c = module.capture_physical_global_copy_debug_v21(&owner, &req, options(8192), ledger);
        assert_eq!(
            c.outcome(),
            PhysicalGlobalCopyDebugOutcomeV21::PreflightRefused
        );
        assert!(c.is_empty());
        let floor = c.usage().entry_storage;
        assert_eq!(c.into_budget().storage(), floor);
    }
}
#[test]
fn exact_cumulative_work_and_storage_one_short_preserve_original_floor_and_history() {
    let req = request(128, 129, None);
    let baseline = capture(WORK, STORAGE, false, &req, 8192);
    assert_eq!(baseline.stop(), None);
    let u = baseline.usage();
    let count = baseline.len();
    let exact = capture(u.work, u.peak_storage, false, &req, 8192);
    assert_eq!(exact.usage(), u);
    assert_eq!(exact.len(), count);
    assert_eq!(exact.stop(), None);
    let short_work = capture(u.work - 1, u.peak_storage, false, &req, 8192);
    let short_storage = capture(u.work, u.peak_storage - 1, false, &req, 8192);
    for c in [&short_work, &short_storage] {
        assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
        assert!(matches!(
            c.stop(),
            Some(PhysicalGlobalCopyDebugCaptureStopV21::Resource(_))
        ));
        assert!(c.len() < count);
    }
    assert!(short_work.usage().failed_work.is_some());
    assert!(short_storage.usage().failed_storage.is_some());
    for c in [baseline, exact, short_work, short_storage] {
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
fn inherited_denials_and_failed_bootstrap_do_not_reset_or_publish_records() {
    let (owner, module, mut ledger) = prepared(WORK, STORAGE, false);
    ledger.with_budget(|b| {
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
    });
    let c = module.capture_physical_global_copy_debug_v21(
        &owner,
        &request(128, 129, None),
        options(8192),
        ledger,
    );
    assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
    assert_eq!(c.usage().failed_work, Some(usize::MAX));
    assert_eq!(c.usage().failed_storage, Some(usize::MAX));
    let (owner, module, mut ledger) = prepared(WORK, STORAGE, false);
    ledger
        .with_budget(|b| b.reserve_storage(STORAGE - b.storage()))
        .unwrap();
    let floor = ledger.storage();
    let c = module.capture_physical_global_copy_debug_v21(
        &owner,
        &request(128, 129, None),
        options(8192),
        ledger,
    );
    assert!(matches!(
        c.error(),
        Some(PhysicalGlobalCopyDebugCaptureErrorV21::Resource(_))
    ));
    assert!(c.is_empty());
    assert_eq!(c.into_budget().storage(), floor);
}
#[test]
fn foreign_canonical_owner_refuses_and_legacy_debug_is_still_unavailable() {
    let (owner, module, ledger) = prepared(WORK, STORAGE, false);
    let (other, _, _) = prepared(WORK, STORAGE, true);
    let req = request(128, 129, None);
    let c = module.capture_physical_global_copy_debug_v21(&other, &req, options(8192), ledger);
    assert_eq!(
        c.error(),
        Some(PhysicalGlobalCopyDebugCaptureErrorV21::OwnerMismatch)
    );
    assert!(c.is_empty());
    let mut sink = NoopSimulationDebugSinkV1;
    assert!(matches!(
        module.simulate_debugged_with_sink(
            &req,
            SimulationTargetV1::amdgpu_64(),
            limits(),
            SimulationDebugCaptureLimitsV1::new(1, 160, 4, 8192).unwrap(),
            &mut sink
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21
        ))
    ));
    assert_ne!(owner.identity().digest(), other.identity().digest());
}
#[test]
fn bounded_cutoffs_do_not_stop_engine_or_claim_complete_observation() {
    for records in [0, 1, 7] {
        let c = capture(WORK, STORAGE, false, &request(128, 129, None), records);
        assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
        assert_eq!(c.len(), records);
        assert_eq!(
            c.stop(),
            Some(PhysicalGlobalCopyDebugCaptureStopV21::RecordLimit)
        );
    }
    let (owner, module, ledger) = prepared(WORK, STORAGE, false);
    let tiny = PhysicalGlobalCopyDebugOptionsV21::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 160, 1, 8192).unwrap(),
        8192,
    )
    .unwrap();
    let c = module.capture_physical_global_copy_debug_v21(
        &owner,
        &request(128, 129, None),
        tiny,
        ledger,
    );
    assert_eq!(c.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
    assert!(c.is_empty());
    assert_eq!(
        c.stop(),
        Some(PhysicalGlobalCopyDebugCaptureStopV21::SnapshotUnavailable)
    );
}
#[test]
fn actual_wait_and_store_reverse_navigation_is_observation_only() {
    let mut s = session(WORK, 8192);
    assert_eq!(s.capture_stop(), None);
    let module = fixture::module();
    let (store, _) = site(&module, Op::GlobalStoreDword);
    let (load, id) = site(&module, Op::GlobalLoadDword);
    let locate = |s: &Session, operation, phase| {
        (0..s.records_len())
            .find(|&n| {
                let r = s.record(n).unwrap();
                r.invocation().global == [0, 0, 0]
                    && r.site().operation == operation
                    && r.phase() == Some(phase)
            })
            .unwrap()
    };
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    let before = locate(&s, store, Before);
    let after = locate(&s, store, After);
    for (index, byte, init) in [
        (before, 0xa5, false),
        (after, word(0).to_le_bytes()[0], true),
        (before, 0xa5, false),
        (after, word(0).to_le_bytes()[0], true),
    ] {
        assert!(matches!(s.seek(index), Nav::Record { .. }));
        let r = s.current().unwrap();
        assert_eq!(r.memory_byte_at(1, 8), Some((byte, init)));
        assert_eq!(
            r.memory_byte_at(0, 8),
            Some((word(0).to_le_bytes()[0], true))
        );
    }
    assert!(matches!(s.step_reverse(), Nav::Record { .. }));
    assert!(s.current().unwrap().is_committed_store());
    assert!(matches!(s.step_reverse(),Nav::Record{index,..}if index==before));
    let pending = locate(&s, load, After);
    let ready = locate(&s, load + 1, After);
    for index in [ready, pending, ready] {
        s.seek(index);
        let b = binding(s.current().unwrap(), id).unwrap();
        assert_eq!(b.scalar().is_some(), index == ready);
        assert_eq!(b.symbolic_kind().is_some(), index == pending);
    }
    let usage = s.usage();
    let ledger = s.into_budget();
    assert_eq!(ledger.storage(), usage.entry_storage);
    assert_eq!(ledger.work(), usage.work);
}
#[test]
fn incomplete_invalid_or_resource_denied_navigation_never_moves_cursor() {
    let mut partial = session(WORK, 7);
    assert!(matches!(partial.seek(6), Nav::Record { .. }));
    assert_eq!(partial.seek(7), Nav::Incomplete);
    assert_eq!(partial.cursor(), Some(6));
    assert_eq!(partial.seek(8), Nav::Unavailable);
    assert_eq!(partial.cursor(), Some(6));
    let baseline = session(WORK, 8192);
    let work = baseline.usage().work;
    let mut exact = session(work + 1, 8192);
    assert!(matches!(exact.seek(0), Nav::Record { index: 0, .. }));
    let prior = exact.usage();
    assert_eq!(exact.step_forward(), Nav::Unavailable);
    assert_eq!(exact.cursor(), Some(0));
    assert_eq!(exact.usage().work, prior.work);
    assert!(exact.usage().failed_work.is_some());
    let usage = exact.usage();
    let returned = exact.into_budget();
    assert_eq!(returned.storage(), usage.entry_storage);
    assert_eq!(returned.failed_work(), usage.failed_work);
    let mut short = session(work, 8192);
    assert_eq!(short.seek(0), Nav::Unavailable);
    assert_eq!(short.cursor(), None);
    assert!(
        std::mem::size_of::<Session>()
            <= std::mem::size_of::<PhysicalGlobalCopyDebugCaptureV21>()
                + std::mem::size_of::<Option<usize>>()
    );
}
#[test]
fn missing_required_waits_or_wrong_load_origin_never_becomes_a_typed_debug_owner() {
    for mode in 0..4 {
        let mut module = fixture::module();
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        if mode < 3 {
            let target = if mode == 2 {
                Op::WaitLgkm0
            } else {
                Op::WaitVm0
            };
            let wait=operations.iter_mut().filter(|o|matches!(&o.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s)if s.instruction.opcode==target)).nth(usize::from(mode == 1)).unwrap();
            let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut wait.kind else {
                unreachable!()
            };
            step.instruction.opcode = if mode == 2 {
                Op::WaitVm0
            } else {
                Op::WaitLgkm0
            };
        } else {
            let load=operations.iter_mut().find(|o|matches!(&o.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s)if s.instruction.opcode==Op::GlobalLoadDword)).unwrap();
            let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut load.kind else {
                unreachable!()
            };
            step.operands[0] = step.operands[1];
        }
        let mut ledger = Owned::new(Work::new(WORK), STORAGE);
        ledger.with_budget(|b| b.reserve_storage(73)).unwrap();
        assert!(
            ledger
                .with_budget(|b| Owner::from_module_ref_with_verification_budget_v21(&module, b))
                .is_err()
        );
        assert_eq!(ledger.storage(), 73);
        assert!(ledger.work() > 0);
    }
}

#[cfg(target_os = "linux")]
#[path = "physical_global_copy_v21/actual_source.rs"]
mod actual_source;
