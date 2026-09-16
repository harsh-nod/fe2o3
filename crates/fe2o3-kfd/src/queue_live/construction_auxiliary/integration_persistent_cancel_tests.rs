//! Registered storage, real prepared controls and the production model-loan driver.

use super::*;
use crate::persistent_compute::PersistentComputeAttachmentEntryV1;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1 as OwnerSnapshot;
use crate::queue::dispatch_binding::preparation::persistent_cancel_control_in_memory_v1;
use crate::queue::live::persistent_cancel::{
    CancelPointV1, CancelShapeV1, PersistentCancelContextV1, PersistentCancelRootV1,
    settle_persistent_cancel_v1,
};
use crate::queue::live::rebind_tests::persistent_cancel::assert_constructed_terminal_reentry_v1;
use crate::queue::live::recycled_detach::RecycledDetachLedgerV1;
use crate::shared_memory::{CleanupStageV1, ControlReleaseMemorySnapshotV1 as MemorySnapshot};
use arrayvec::ArrayVec;

#[derive(Default, Debug, PartialEq, Eq)]
struct Ledger {
    generation: Option<u64>,
    count: usize,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    next: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
enum Corruption {
    Generation,
    Count,
    Identity,
    Initialization,
    DigestOnCold,
    NativeFailure(CancelPointV1, usize),
}

struct Case {
    scope: Box<Scope>,
    attachment: Option<BoundedPersistentComputeAttachmentV1>,
    dispatch: Option<DispatchResourceOwnerV1>,
    retained: Option<PersistentCancelRootV1>,
    displaced: Vec<Gfx942FixedDispatchDataV1>,
    expected: Vec<(Gfx942DeviceMemoryIdentityV1, bool, Option<[u8; 32]>)>,
    prepared_identities: Vec<crate::persistent_allocation::PersistentUseIdentityForTestV1>,
    ledger: Ledger,
    shape: CancelShapeV1,
    entered: usize,
    loans: usize,
    poisons: usize,
    process_poison: bool,
    skip_callback: bool,
    poison_panic: bool,
    ledger_panic: bool,
    fault: Option<(CancelPointV1, Outcome)>,
    corruption: Option<Corruption>,
    points: Vec<CancelPointV1>,
    before_cleanup: Option<MemorySnapshot>,
    after_cleanup: Option<MemorySnapshot>,
    capture_memory: bool,
}

fn pair() -> Gfx942DirectionalSdmaQueueObservationV1 {
    Gfx942DirectionalSdmaQueueObservationV1 {
        host_to_device: Gfx942SdmaQueueObservationV1 {
            queue_id: 17,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_H2D_ENGINE_INDEX_V1),
        },
        device_to_host: Gfx942SdmaQueueObservationV1 {
            queue_id: 23,
            ring_bytes: crate::sdma::GFX942_SDMA_RING_BYTES_V1,
            maximum_in_flight: crate::sdma::GFX942_SDMA_MAX_IN_FLIGHT_V1 as u16,
            engine_index: Some(crate::sdma::GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        },
        admitted_engine_count: 2,
        admitted_queues_per_engine: 8,
    }
}

impl Case {
    fn new(
        shape: CancelShapeV1,
        initialized: bool,
        authenticated: bool,
        predecessor: Option<u64>,
    ) -> Self {
        let (mut scope, result, _) = prefix_case_with_probe(
            false,
            |_| {},
            |_, _| {},
            run_auxiliary,
            |scope, failed| {
                assert!(!failed);
                assert_pair(scope);
            },
            false,
        );
        assert!(result.is_ok());
        let queue = scope.primary.completed.as_ref().unwrap().key;
        let binding = PersistentComputeBindingKeyV1 {
            queue,
            attachment_generation: 1,
        };
        let mut prepared = None;
        let (operation, retake) = scope
            .parent
            .with_preparation_custody(|memory| {
                let count = if shape == CancelShapeV1::Single { 1 } else { 3 };
                assert!(
                    initialized || count == 1,
                    "three-binding admission requires initialized output"
                );
                let mut entries = ArrayVec::new();
                let mut data = Vec::new();
                let mut expected = Vec::new();
                for index in 0..count {
                    let is_read = count == 3 && index < 2;
                    let initialized = initialized || is_read;
                    let original = memory.device(initialized);
                    let content = original.initialized_content();
                    let digest = if authenticated {
                        content.map(|c| c.sha256())
                    } else {
                        None
                    };
                    let buffer = Gfx942SdmaBufferV1::from_bridge_parts(
                        original.into_sdma_storage(),
                        queue,
                        1,
                        4096,
                    );
                    let (mut allocation, outstanding) =
                        promote_directional_persistent_sdma_custody_v1(
                            buffer,
                            admit_persistent_directional_sdma_pair_v1(pair()).unwrap(),
                            1,
                        )
                        .unwrap();
                    assert_eq!(outstanding, 1);
                    let request = Gfx942PersistentUseRequestV1::new(
                        if is_read {
                            Gfx942PersistentOperationV1::ComputeRead
                        } else {
                            Gfx942PersistentOperationV1::ComputeWrite
                        },
                        0,
                        4096,
                    )
                    .unwrap();
                    let reserved = allocation.owner.reserve(request, None).unwrap();
                    let use_lease = allocation.owner.prepare(reserved).unwrap();
                    let lease = allocation
                        .owner
                        .detach_local_native_for_compute(&use_lease)
                        .unwrap();
                    let identity = lease.storage_identity();
                    let returned = if let Some(content) = content {
                        Gfx942FixedDispatchDataV1::initialized(
                            Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                                lease, content,
                            )
                            .unwrap(),
                        )
                    } else {
                        Gfx942FixedDispatchDataV1::uninitialized(lease)
                    };
                    entries.push(PersistentComputeAttachmentEntryV1 {
                        allocation,
                        authenticated_sha256: digest,
                        fully_initialized: initialized,
                        state: PersistentComputeUseStateV1::Prepared(use_lease),
                        storage_identity: Some(identity),
                        effect: if is_read {
                            Gfx942PersistentComputeEffectV1::Read
                        } else {
                            Gfx942PersistentComputeEffectV1::Write
                        },
                    });
                    expected.push((identity, initialized, digest));
                    data.push(returned);
                }
                let dispatch =
                    persistent_cancel_control_in_memory_v1(memory, queue, data, predecessor);
                prepared = Some((
                    BoundedPersistentComputeAttachmentV1 {
                        entries,
                        binding,
                        predecessor_dispatch_generation: predecessor,
                        terminal_custody: None,
                    },
                    dispatch,
                    expected,
                ));
                Ok(())
            })
            .unwrap();
        operation.unwrap();
        retake.unwrap();
        let (attachment, dispatch, expected) = prepared.unwrap();
        let prepared_identities = attachment
            .entries
            .iter()
            .map(|entry| {
                let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                    unreachable!()
                };
                prepared.cancellation_identity_for_test()
            })
            .collect();
        Self {
            scope,
            attachment: Some(attachment),
            dispatch: Some(dispatch),
            retained: None,
            displaced: Vec::new(),
            expected,
            prepared_identities,
            ledger: Ledger::default(),
            shape,
            entered: 0,
            loans: 0,
            poisons: 0,
            process_poison: false,
            skip_callback: false,
            poison_panic: false,
            ledger_panic: false,
            fault: None,
            corruption: None,
            points: Vec::new(),
            before_cleanup: None,
            after_cleanup: None,
            capture_memory: false,
        }
    }

    fn run(
        &mut self,
    ) -> std::thread::Result<
        Result<ArrayVec<Gfx942PersistentComputeInputV1, 3>, ComputeAqlQueueSessionErrorV1>,
    > {
        let shape = self.shape;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            settle_persistent_cancel_v1(self, shape)
        }))
    }

    fn memory_mut(&mut self) -> &mut Memory {
        &mut self
            .scope
            .parent
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
    }

    fn assert_terminal(&self) -> &PersistentCancelRootV1 {
        assert_eq!(
            self.ledger,
            Ledger::default(),
            "failure must not commit any ledger field"
        );
        assert!(self.scope.parent.poisoned);
        assert_eq!(self.poisons, 1);
        let root = self
            .retained
            .as_ref()
            .expect("complete cancellation root retained");
        assert_eq!(
            root.attachment.as_ref().unwrap().entries.len(),
            self.expected.len()
        );
        root
    }
}

impl PersistentCancelContextV1 for Case {
    type Memory = Memory;
    fn attachment(&mut self) -> &mut Option<BoundedPersistentComputeAttachmentV1> {
        &mut self.attachment
    }
    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1> {
        &mut self.dispatch
    }
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Memory),
    ) -> Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>
    {
        self.loans += 1;
        if self.skip_callback {
            return Ok(((), Ok(())));
        }
        let entered = &mut self.entered;
        let before = &mut self.before_cleanup;
        let after = &mut self.after_cleanup;
        let capture = self.capture_memory;
        let (callback, retake) = self.scope.parent.with_preparation_custody(|memory| {
            *entered += 1;
            if capture {
                *before = Some(memory.control_release_loan_snapshot_v1());
            }
            operation(memory);
            if capture {
                *after = Some(memory.control_release_loan_snapshot_v1());
            }
            Ok(())
        })?;
        Ok(((), retake.and(callback)))
    }
    fn ledger(&mut self) -> RecycledDetachLedgerV1<'_> {
        if self.ledger_panic {
            panic!("cancel ledger access");
        }
        RecycledDetachLedgerV1 {
            generation: &mut self.ledger.generation,
            count: &mut self.ledger.count,
            identities: &mut self.ledger.identities,
            next: &mut self.ledger.next,
        }
    }
    fn retain(&mut self, root: PersistentCancelRootV1) {
        assert!(self.retained.is_none());
        self.retained = Some(root);
    }
    fn poison(&mut self, process: bool) {
        assert!(self.retained.is_some(), "retention precedes final poison");
        self.poisons += 1;
        self.process_poison |= process;
        self.scope.parent.poison();
        if self.poison_panic {
            panic!("cancel final poison");
        }
    }
    fn checkpoint(
        &mut self,
        point: CancelPointV1,
        root: &mut PersistentCancelRootV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.points.push(point);
        if let Some(corruption) = self.corruption {
            let attachment = root.attachment.as_mut().unwrap();
            match corruption {
                Corruption::Generation if point == CancelPointV1::Returned => {
                    root.native.generation = Some(99)
                }
                Corruption::Count if point == CancelPointV1::Returned => {
                    self.displaced.push(root.native.returned.pop().unwrap())
                }
                Corruption::Identity if point == CancelPointV1::Returned => {
                    attachment.entries[0].storage_identity = None
                }
                Corruption::Initialization if point == CancelPointV1::Returned => {
                    attachment.entries[0].fully_initialized =
                        !attachment.entries[0].fully_initialized
                }
                Corruption::DigestOnCold if point == CancelPointV1::Returned => {
                    let entry = attachment.entries.last_mut().unwrap();
                    entry.authenticated_sha256 = Some([0xcc; 32]);
                    entry.fully_initialized = false;
                    let data = root.native.returned.pop().unwrap();
                    let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
                        unreachable!()
                    };
                    root.native
                        .returned
                        .push(Gfx942FixedDispatchDataV1::uninitialized(lease));
                }
                Corruption::NativeFailure(at, index) if point == at => attachment.entries[index]
                    .allocation
                    .owner
                    .quarantine_for_caller_reported_currentness_loss(),
                _ => {}
            }
        }
        if let Some((at, fault)) = self.fault
            && at == point
        {
            outcome("cancel checkpoint", fault)?;
        }
        Ok(())
    }
}

#[test]
fn persistent_cancel_constructed_native_cleanup_preserves_exact_control_prefix_and_data() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for position in 0..2 {
            for (step, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                .into_iter()
                .enumerate()
            {
                for panicked in [false, true] {
                    let mut case = Case::new(shape, true, true, Some(7));
                    case.capture_memory = true;
                    let owner =
                        OwnerSnapshot::persistent_cancel_owner_v1(case.dispatch.as_ref().unwrap());
                    assert_eq!(owner.order_v1().len(), 2);
                    let queue = case.attachment.as_ref().unwrap().binding.queue;
                    let session_id = case.memory_mut().primary_session_id();
                    case.memory_mut()
                        .arm_control_release_native_v1(position, operation, panicked);
                    let result = case.run();
                    if panicked {
                        assert_eq!(
                            result.err().unwrap().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(matches!(result.unwrap(),
                            Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(name))
                            )) if name == operation
                        ));
                    }
                    let root = case.assert_terminal();
                    let cleanup = root.native.cleanup.as_ref().unwrap();
                    owner.assert_persistent_cancel_prefix_v1(
                        cleanup,
                        position,
                        step,
                        panicked,
                        &root.native.returned,
                    );
                    let snapshot = OwnerSnapshot::root_v1(cleanup);
                    let active = snapshot.active_v1().unwrap();
                    assert!(active.started && active.failed && !active.native_disposed);
                    assert_eq!(
                        active.stage,
                        if step == 0 {
                            CleanupStageV1::NativeUnmap
                        } else {
                            CleanupStageV1::NativeRelease
                        }
                    );
                    assert_eq!(root.native.stage(), None);
                    assert_eq!(root.native.restored, 0);
                    case.before_cleanup
                        .as_ref()
                        .unwrap()
                        .assert_control_transition_snapshot_v1(
                            case.after_cleanup.as_ref().unwrap(),
                            (queue.vm, session_id),
                            &owner.order_v1(),
                            (
                                position,
                                step > 0,
                                step + 1,
                                Some((step > 0, step, false, step == 2)),
                            ),
                        );
                    let engine = &case.scope.primary.completed.as_ref().unwrap().engine;
                    case.after_cleanup.as_ref().unwrap().assert_after_retake_v1(
                        engine
                            .backend
                            .session
                            .control_release_snapshot_v1(&engine.foundation),
                        case.before_cleanup.as_ref().unwrap(),
                        false,
                    );
                    assert_constructed_terminal_reentry_v1(
                        case.retained.take().unwrap(),
                        case.dispatch.take(),
                    );
                }
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_restores_exact_initialized_cold_and_digest_free_inputs() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for (initialized, authenticated) in [(false, false), (true, false), (true, true)] {
            if shape == CancelShapeV1::Three && !initialized {
                continue;
            }
            for predecessor in [None, Some(0), Some(7)] {
                let mut case = Case::new(shape, initialized, authenticated, predecessor);
                let inputs = case.run().unwrap().unwrap();
                assert_eq!((case.loans, case.entered, case.poisons), (1, 1, 0));
                assert!(
                    case.attachment.is_none() && case.dispatch.is_none() && case.retained.is_none()
                );
                assert_eq!(case.ledger.generation, Some(predecessor.unwrap_or(0)));
                assert_eq!(case.ledger.next, Some(0));
                assert_eq!(inputs.len(), case.expected.len());
                for (input, &(identity, initialized, digest)) in
                    inputs.into_iter().zip(&case.expected)
                {
                    let (allocation, actual_digest, actual_initialized) = input.into_parts();
                    assert_eq!((actual_digest, actual_initialized), (digest, initialized));
                    assert_eq!(
                        allocation
                            .owner
                            .local_native_for_sdma()
                            .unwrap()
                            .storage_identity(),
                        identity
                    );
                    assert_eq!(allocation.owner.live_use_count(), 0);
                    assert_eq!(allocation.owner.quarantine_reason(), None);
                }
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_model_errors_and_panics_retain_complete_roster() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for phase in 0..5 {
            for fault in [Outcome::Error, Outcome::Panic] {
                let mut case = Case::new(shape, true, true, Some(7));
                match phase {
                    0 => case.scope.parent.faults.loan = fault,
                    1 => case.scope.parent.faults.reclaim_before = fault,
                    2 => case.scope.parent.faults.reclaim_after = fault,
                    3 => case.scope.parent.faults.operation = fault,
                    _ => {
                        case.skip_callback = true;
                    }
                }
                let result = case.run();
                if phase == 4 || fault == Outcome::Error {
                    assert!(result.unwrap().is_err());
                } else {
                    assert!(result.is_err());
                }
                let root = case.assert_terminal();
                assert_eq!(
                    case.process_poison,
                    shape == CancelShapeV1::Three || (phase != 4 && fault == Outcome::Panic)
                );
                assert_eq!(root.native.restored, 0);
                assert_eq!(root.native.cancelled, 0);
                assert_eq!(case.entered, usize::from(phase != 0 && phase != 4));
                if case.entered == 0 {
                    assert!(case.dispatch.is_some());
                    assert!(root.native.cleanup.is_none());
                } else {
                    assert!(root.native.cleanup.as_ref().unwrap().is_complete());
                    assert_eq!(root.native.returned.len(), case.expected.len());
                }
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_shape_rejection_precedes_native_restore_and_ledger() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for corruption in [
            Corruption::Generation,
            Corruption::Count,
            Corruption::Identity,
            Corruption::DigestOnCold,
            Corruption::Initialization,
        ] {
            let mut case = Case::new(shape, shape == CancelShapeV1::Three, false, None);
            case.corruption = Some(corruption);
            let result = case.run().unwrap();
            if shape == CancelShapeV1::Single && matches!(corruption, Corruption::Initialization) {
                let mut inputs =
                    result.expect("single cancellation accepts returned initialization");
                assert!(
                    !inputs.pop().unwrap().is_fully_initialized(),
                    "attachment flags cannot promote cold returned storage"
                );
                continue;
            }
            assert!(result.is_err());
            let root = case.assert_terminal();
            assert_eq!((root.native.restored, root.native.cancelled), (0, 0));
            assert!(!root.native.restore_started);
            assert!(
                root.attachment
                    .as_ref()
                    .unwrap()
                    .entries
                    .iter()
                    .all(|entry| {
                        entry.allocation.owner.local_native_for_sdma().is_none()
                            && matches!(entry.state, PersistentComputeUseStateV1::Quarantined)
                    })
            );
        }
    }
}

#[test]
fn persistent_cancel_constructed_restore_and_cancel_prefixes_retain_failed_leases() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        let count = if shape == CancelShapeV1::Single { 1 } else { 3 };
        for cancel in [false, true] {
            for index in 0..count {
                for failure in 0..3 {
                    let mut case = Case::new(shape, true, true, Some(7));
                    let point = if cancel {
                        CancelPointV1::Cancel(index)
                    } else {
                        CancelPointV1::Restore(index)
                    };
                    if failure == 0 {
                        case.corruption = Some(Corruption::NativeFailure(point, index));
                    } else {
                        case.fault = Some((
                            point,
                            if failure == 1 {
                                Outcome::Error
                            } else {
                                Outcome::Panic
                            },
                        ));
                    }
                    let result = case.run();
                    if failure == 2 {
                        assert!(result.is_err());
                    } else {
                        assert!(result.unwrap().is_err());
                    }
                    let root = case.assert_terminal();
                    assert_eq!(
                        case.process_poison,
                        shape == CancelShapeV1::Three || failure == 2
                    );
                    let restored = if cancel { count } else { index };
                    let cancelled = if cancel { index } else { 0 };
                    assert_eq!(
                        (root.native.restored, root.native.cancelled),
                        (restored, cancelled)
                    );
                    assert!(root.native.restore_started);
                    for (position, entry) in
                        root.attachment.as_ref().unwrap().entries.iter().enumerate()
                    {
                        let identity = case.expected[position].0;
                        if position < restored {
                            assert_eq!(
                                entry
                                    .allocation
                                    .owner
                                    .local_native_for_sdma()
                                    .unwrap()
                                    .storage_identity(),
                                identity
                            );
                            assert!(root.native.mapped[position].is_none());
                        } else {
                            assert!(entry.allocation.owner.local_native_for_sdma().is_none());
                            assert_eq!(
                                root.native.mapped[position]
                                    .as_ref()
                                    .unwrap()
                                    .storage_identity(),
                                identity
                            );
                        }
                        if position < cancelled {
                            assert_eq!(entry.allocation.owner.live_use_count(), 0);
                        } else {
                            let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state
                            else {
                                panic!("failed/suffix Prepared lease lost")
                            };
                            assert_eq!(prepared.sequence(), 1);
                            assert_eq!(prepared.request().range().byte_len(), 4096);
                            assert_eq!(
                                prepared.cancellation_identity_for_test(),
                                case.prepared_identities[position]
                            );
                            if failure != 0 || position != index {
                                assert!(
                                    entry
                                        .allocation
                                        .owner
                                        .preflight_cancel_prepared(prepared)
                                        .is_ok()
                                );
                            }
                            assert_eq!(entry.allocation.owner.live_use_count(), 1);
                        }
                    }
                    if (restored > 0 && restored < count) || (cancelled > 0 && cancelled < count) {
                        assert_eq!(root.native.stage(), None);
                    }
                    assert_constructed_terminal_reentry_v1(
                        case.retained.take().unwrap(),
                        case.dispatch.take(),
                    );
                }
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_retake_error_or_panic_overrides_normal_lower_error() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for fault in [Outcome::Error, Outcome::Panic] {
            let mut case = Case::new(shape, true, true, Some(7));
            case.memory_mut().arm_control_release_projection_v1(
                0,
                CleanupStageV1::ReleaseCommit,
                false,
            );
            case.scope.parent.faults.reclaim_after = fault;
            let result = case.run();
            if fault == Outcome::Error {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "auxiliary-retake-complete"
                    ))
                ));
            } else {
                assert_eq!(
                    result
                        .err()
                        .unwrap()
                        .downcast_ref::<(&str, usize)>()
                        .unwrap()
                        .0,
                    "auxiliary-retake-complete"
                );
            }
            let root = case.assert_terminal();
            assert_eq!(root.native.returned.len(), case.expected.len());
            assert_eq!(root.native.restored, 0);
            assert!(root.native.cleanup.is_some());
        }
    }
}

#[test]
fn persistent_cancel_constructed_lower_cleanup_retains_data_and_primary_panic() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for position in 0..2 {
            for panicked in [false, true] {
                let mut case = Case::new(shape, true, true, Some(7));
                case.memory_mut().arm_control_release_projection_v1(
                    position,
                    CleanupStageV1::ReleaseCommit,
                    panicked,
                );
                if panicked {
                    case.scope.parent.faults.reclaim_after = Outcome::Panic;
                    case.poison_panic = true;
                }
                let result = case.run();
                if panicked {
                    assert_eq!(
                        result
                            .err()
                            .unwrap()
                            .downcast_ref::<(&str, CleanupStageV1)>(),
                        Some(&("control cleanup projection", CleanupStageV1::ReleaseCommit))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                let root = case.assert_terminal();
                assert!(root.native.cleanup.is_some());
                assert!(!root.native.cleanup.as_ref().unwrap().is_complete());
                assert_eq!(root.native.stage(), None);
                assert_eq!(root.native.restored, 0);
                assert_eq!(
                    root.native.returned.len(),
                    if panicked { 0 } else { case.expected.len() }
                );
                assert_eq!(case.entered, 1);
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_output_and_ledger_panics_keep_all_restored_owners() {
    for shape in [CancelShapeV1::Single, CancelShapeV1::Three] {
        for ledger in [false, true] {
            let mut case = Case::new(shape, true, true, Some(7));
            case.ledger_panic = ledger;
            if !ledger {
                case.fault = Some((CancelPointV1::Output, Outcome::Panic));
            }
            assert!(case.run().is_err());
            let root = case.assert_terminal();
            assert!(case.process_poison);
            assert_eq!(root.native.restored, case.expected.len());
            assert_eq!(root.native.cancelled, case.expected.len());
            for (entry, &(identity, _, _)) in root
                .attachment
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .zip(&case.expected)
            {
                assert_eq!(
                    entry
                        .allocation
                        .owner
                        .local_native_for_sdma()
                        .unwrap()
                        .storage_identity(),
                    identity
                );
                assert_eq!(entry.allocation.owner.live_use_count(), 0);
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_three_preflights_reject_before_changing_any_prefix() {
    for cancel in [false, true] {
        for index in 0..3 {
            let mut case = Case::new(CancelShapeV1::Three, true, true, Some(7));
            let point = if cancel {
                CancelPointV1::CancelPreflight
            } else {
                CancelPointV1::RestorePreflight
            };
            case.corruption = Some(Corruption::NativeFailure(point, index));
            assert!(case.run().unwrap().is_err());
            let root = case.assert_terminal();
            assert!(case.process_poison);
            assert_eq!(
                (root.native.restored, root.native.cancelled),
                (if cancel { 3 } else { 0 }, 0)
            );
            assert_eq!(root.native.restore_started, cancel);
            for (entry, &(identity, _, _)) in root
                .attachment
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .zip(&case.expected)
            {
                if cancel {
                    assert_eq!(
                        entry
                            .allocation
                            .owner
                            .local_native_for_sdma()
                            .unwrap()
                            .storage_identity(),
                        identity
                    );
                    assert!(matches!(
                        entry.state,
                        PersistentComputeUseStateV1::Prepared(_)
                    ));
                    assert_eq!(entry.allocation.owner.live_use_count(), 1);
                } else {
                    assert!(entry.allocation.owner.local_native_for_sdma().is_none());
                }
            }
        }
    }
}

#[test]
fn persistent_cancel_constructed_single_returned_initialization_is_not_downgraded() {
    let mut case = Case::new(CancelShapeV1::Single, true, false, None);
    case.corruption = Some(Corruption::Initialization);
    let mut inputs = case.run().unwrap().unwrap();
    assert!(matches!(
        inputs.pop().unwrap(),
        Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_)
    ));
    assert_eq!((case.entered, case.poisons), (1, 0));
}
