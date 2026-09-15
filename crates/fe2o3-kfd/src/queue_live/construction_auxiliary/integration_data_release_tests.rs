//! Real lower cleanup and model loans composed through the production release sequencer.
//! Linux facade forwarding and hardware execution have separate qualification boundaries.

use super::*;
use crate::queue::dispatch_binding::DispatchDataInputStorageV1;
use crate::queue::live::data_release::{
    DataReleaseContextV1, SettledDataReleaseV1, settle_data_release_v1,
};
use crate::sdma::Gfx942SdmaBufferStorageIdentityV1 as Identity;
use crate::shared_memory::{
    CleanupStageV1, DataCleanupCustodyV1, DataCleanupMetadataV1, DataCleanupObservationV1,
    DataReleaseSnapshotV1, DispatchDataReleaseV1,
};

#[derive(Default)]
struct ReleaseTrace {
    calls: usize,
    lower_calls: usize,
    skip_lower: bool,
    before_lower: Option<DataCleanupObservationV1>,
    before_retake: Option<DataCleanupObservationV1>,
    retained: Vec<DataCleanupCustodyV1>,
    poisons: usize,
    panicked: bool,
    commit_ready: bool,
    commit_panic: bool,
}

struct ReleaseContext<'a> {
    base: Context<'a>,
    trace: &'a mut ReleaseTrace,
}

impl DataReleaseContextV1 for ReleaseContext<'_> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.base.require_unbound()
    }

    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        if self.trace.commit_ready && self.trace.commit_panic {
            self.trace.commit_panic = false;
            panic!("data release commit ledger access");
        }
        self.base.ledger()
    }

    fn release(
        &mut self,
        root: &mut DataCleanupCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.trace.calls += 1;
        self.trace.before_lower = Some(root.observation());
        let before = self.base.ledger_snapshot();
        let ledger = self.base.ledger();
        let storage = (ledger.identities.as_ptr(), ledger.identities.capacity());
        let trace = &mut self.trace;
        let result = self.base.parent.with_preparation_custody(|memory| {
            let result = catch_unwind(AssertUnwindSafe(|| {
                if trace.skip_lower {
                    return Ok(());
                }
                trace.lower_calls += 1;
                memory.release_data(root)
            }));
            trace.before_retake = Some(root.observation());
            match result {
                Ok(result) => result.map_err(Into::into),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
        assert_eq!(
            self.base.ledger_snapshot(),
            before,
            "no release ledger commit before retake"
        );
        let ledger = self.base.ledger();
        assert_eq!(
            (ledger.identities.as_ptr(), ledger.identities.capacity()),
            storage
        );
        let (operation, retake) = result?;
        retake?;
        operation?;
        self.trace.commit_ready = true;
        Ok(())
    }

    fn retain(&mut self, root: DataCleanupCustodyV1) {
        self.trace.retained.push(root);
    }

    fn poison(&mut self, panicked: bool) {
        assert!(
            !self.trace.retained.is_empty(),
            "retain cleanup custody before poison"
        );
        self.trace.poisons += 1;
        self.trace.panicked |= panicked;
        if let Some(lane) = self.base.lane.as_mut() {
            lane.completion_owner.poison_owner();
            lane.submission.as_mut().unwrap().poison();
            lane.unpublished_dispatch.continuation = None;
        } else {
            self.base.primary.continuation = None;
        }
        self.base.parent.poison();
        if panicked {
            Fixture::poison();
        }
    }
}

struct ReleaseFixture {
    base: InsertionFixture,
    trace: ReleaseTrace,
    index: usize,
    identity: Identity,
    metadata: DataCleanupMetadataV1,
    currentness_before: usize,
}

impl ReleaseFixture {
    fn new(ordinal: usize, kind: usize) -> Self {
        let mut base = InsertionFixture::new(ordinal);
        let index = if kind == 4 { 2 } else { kind };
        if kind == 4 {
            let DispatchDataInputStorageV1::Device(lease) =
                base.data.remove(index).into_parts().storage
            else {
                panic!("device-uninitialized fixture slot changed kind");
            };
            // Synthetic completed-dispatch typing, not a GPU completion observation.
            let data = Gfx942FixedDispatchDataV1::initialized_after_dispatch_for_test(lease);
            base.context().ledger().identities[index] = data.storage_identity();
            base.data.insert(index, data);
        }
        let data = &base.data[index];
        let metadata = DataCleanupMetadataV1::Fixed {
            identity: data.storage_identity(),
            layout: data.layout(),
            fully_initialized: data.is_fully_initialized(),
            initialized_content: data.initialized_content(),
        };
        let identity = data.sdma_storage_identity();
        assert!(matches!(
            (kind, data.storage_identity()),
            (
                0,
                Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedContent(_)
            ) | (
                1,
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized(_)
            ) | (
                2,
                Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(_)
            ) | (
                3,
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(_)
            ) | (
                4,
                Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedAfterDispatch(_)
            )
        ));
        assert_eq!(data.is_fully_initialized(), matches!(kind, 0 | 1 | 4));
        assert_eq!(data.initialized_content().is_some(), kind == 0);
        let currentness_before = base.memory().observation().calls[0];
        Self {
            base,
            trace: ReleaseTrace::default(),
            index,
            identity,
            metadata,
            currentness_before,
        }
    }

    fn snapshot(&self) -> DataReleaseSnapshotV1 {
        let engine = &self.base.scope.primary.completed.as_ref().unwrap().engine;
        self.base
            .memory()
            .data_release_snapshot_v1(&engine.foundation)
    }

    fn assert_prefix(&self, before: &DataReleaseSnapshotV1, completed: bool) {
        assert!(
            self.trace.before_retake.is_some(),
            "the real model-loan callback was entered"
        );
        let engine = &self.base.scope.primary.completed.as_ref().unwrap().engine;
        before.assert_prepared_data_prefix_v1(
            self.base.memory(),
            &engine.foundation,
            &[self.identity],
            usize::from(completed),
            (!completed)
                .then(|| self.trace.retained[0].observation())
                .as_ref(),
        );
    }

    fn assert_native(
        &self,
        phase: SharedMemorySessionPhaseV1,
        currentness: usize,
        poisoned: usize,
    ) {
        let after = self.base.memory().observation();
        assert_eq!(after.phase, phase);
        assert_eq!(after.calls[0] - self.currentness_before, currentness);
        assert_eq!(
            self.base.memory().data_release_process_poisoned_v1(),
            poisoned
        );
    }

    fn release(&mut self, index: usize) -> SettledDataReleaseV1 {
        let data = self.base.data.remove(index);
        settle_data_release_v1(
            &mut ReleaseContext {
                base: self.base.context(),
                trace: &mut self.trace,
            },
            data,
        )
    }

    fn retained(&self) -> DataCleanupObservationV1 {
        assert_eq!(self.trace.retained.len(), 1);
        let retained = self.trace.retained[0].observation();
        assert_eq!(retained.metadata, self.metadata);
        if let Some(before) = &self.trace.before_retake {
            assert_eq!(
                &retained, before,
                "retake cannot discard or rewrite the cleanup receipt"
            );
        }
        retained
    }

    fn assert_retry(&mut self) {
        assert!(self.base.scope.parent.poisoned);
        self.base.memory_mut().clear_data_release_faults_v1();
        self.base.scope.parent.faults = Faults::default();
        let before = self.snapshot();
        let ledger = self.base.context().ledger_snapshot();
        let retained = self
            .trace
            .retained
            .iter()
            .map(|r| r.observation())
            .collect::<Vec<_>>();
        let calls = (self.trace.calls, self.trace.lower_calls, self.trace.poisons);
        let retry = self.release(0);
        assert!(!retry.transport);
        assert!(matches!(
            retry.result,
            Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Poisoned
            )))
        ));
        assert_eq!(
            self.snapshot(),
            before,
            "terminal retry must not touch native state or refunds"
        );
        assert_eq!(self.base.context().ledger_snapshot(), ledger);
        assert_eq!(
            (self.trace.calls, self.trace.lower_calls, self.trace.poisons),
            calls
        );
        assert_eq!(self.trace.retained.len(), retained.len() + 1);
        for (root, before) in self.trace.retained.iter().zip(retained) {
            assert_eq!(root.observation(), before);
        }
        let retry = self.trace.retained.last().unwrap().observation();
        assert_eq!(retry.owner, "Original");
        assert!(!retry.started);
    }
}

fn arm_retake(f: &mut ReleaseFixture, closing: usize) {
    match closing {
        0 => f.base.scope.parent.faults.reclaim_before = Outcome::Error,
        1 => f.base.scope.parent.faults.reclaim_before = Outcome::Panic,
        2 => f.base.scope.parent.faults.reclaim_after = Outcome::Error,
        3 => f.base.scope.parent.faults.reclaim_after = Outcome::Panic,
        4 => f.base.scope.parent.faults.regress_revision = true,
        _ => unreachable!(),
    }
}

fn next_occurrence(name: &str) -> usize {
    trace()
        .borrow()
        .calls
        .iter()
        .filter(|&&s| s == name)
        .count()
        + 1
}

fn assert_retake_error(released: &SettledDataReleaseV1, closing: usize, pre: usize, post: usize) {
    match (closing, &released.result) {
        (0, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name)))) => {
            assert_eq!(*name, "auxiliary-retake")
        }
        (2, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name)))) => {
            assert_eq!(*name, "auxiliary-retake-complete")
        }
        (4, Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))) => {
            assert_eq!(*name, "fixture live foundation reclaim")
        }
        (1, Err(payload)) => assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-retake", pre))
        ),
        (3, Err(payload)) => assert_eq!(
            payload.downcast_ref::<(&str, usize)>(),
            Some(&("auxiliary-retake-complete", post))
        ),
        _ => panic!("unexpected release retake failure provenance"),
    }
}

#[test]
fn data_release_all_five_variants_commit_after_retake_and_reuse_exact_hole() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            let mut f = ReleaseFixture::new(ordinal, kind);
            let before = f.snapshot();
            let loan = f.base.loan_state();
            let (mut expected, count, _) = f.base.context().ledger_snapshot();
            let storage = {
                let mut context = f.base.context();
                let ledger = context.ledger();
                (ledger.identities.as_ptr(), ledger.identities.capacity())
            };
            let released = f.release(f.index);
            assert!(!released.transport);
            released.into_result().unwrap();
            expected.remove(f.index);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (expected.clone(), count - 1, Some(f.index))
            );
            let mut context = f.base.context();
            let ledger = context.ledger();
            assert_eq!(
                (ledger.identities.as_ptr(), ledger.identities.capacity()),
                storage
            );
            assert!(f.trace.retained.is_empty());
            assert_eq!(
                (f.trace.calls, f.trace.lower_calls, f.trace.poisons),
                (1, 1, 0)
            );
            let receipt = f.trace.before_retake.as_ref().unwrap();
            assert_eq!(receipt.metadata, f.metadata);
            assert!(receipt.complete && receipt.native_disposed && !receipt.failed);
            assert_eq!(receipt.owner, "NativeDisposed");
            assert_eq!(f.base.loan_state(), (loan.0, None, loan.2 + 1));
            f.assert_prefix(&before, true);
            f.assert_native(
                SharedMemorySessionPhaseV1::Active,
                if matches!(kind, 1 | 3) { 6 } else { 5 },
                0,
            );

            let (bytes, content) = input();
            let inserted = f.base.insert(None, bytes, 4096, content);
            assert!(!inserted.transport);
            let inserted = inserted.into_result().unwrap();
            expected.insert(f.index, inserted.storage_identity());
            f.base.data.insert(f.index, inserted);
            assert_eq!(f.base.context().ledger_snapshot(), (expected, count, None));
            f.base.restore_and_transport(false);
        }
    }
}

#[test]
fn data_release_complete_receipt_survives_every_failed_retake_without_ledger_commit() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for closing in 0..5 {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let loan = f.base.loan_state();
                let offset = trace().borrow().calls.len();
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                arm_retake(&mut f, closing);
                let released = f.release(f.index);
                assert!(released.transport);
                assert_retake_error(&released, closing, pre, post);
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let receipt = f.retained();
                assert!(receipt.complete && receipt.native_disposed && !receipt.failed);
                assert_eq!(receipt.owner, "NativeDisposed");
                f.assert_prefix(&before, true);
                assert_eq!(
                    f.base.loan_state(),
                    (
                        loan.0,
                        (!matches!(closing, 2 | 3)).then_some(loan.2),
                        loan.2 + 1
                    )
                );
                for name in ["auxiliary-loan", "auxiliary-retake"] {
                    assert_eq!(
                        trace().borrow().calls[offset..]
                            .iter()
                            .filter(|&&s| s == name)
                            .count(),
                        1
                    );
                }
                assert_eq!(
                    (f.trace.calls, f.trace.lower_calls, f.trace.poisons),
                    (1, 1, 1)
                );
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_native_error_and_panic_preserve_exact_prefix_and_neighbors() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            let operations: &[&str] = if matches!(kind, 1 | 3) {
                &["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
            } else {
                &["unmap_gpu", "free", "release_va_reservation"]
            };
            for &operation in operations {
                for panicked in [false, true] {
                    let mut f = ReleaseFixture::new(ordinal, kind);
                    let before = f.snapshot();
                    let ledger = f.base.context().ledger_snapshot();
                    f.base.memory_mut().fail_cleanup(operation, panicked);
                    let released = f.release(f.index);
                    assert!(released.transport);
                    if panicked {
                        assert_eq!(
                            released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(
                            matches!(released.result, Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(name)))) if name == operation)
                        );
                    }
                    assert_eq!(f.base.context().ledger_snapshot(), ledger);
                    let retained = f.retained();
                    assert!(retained.started && retained.failed && !retained.complete);
                    assert!(!retained.native_disposed);
                    f.assert_prefix(&before, false);
                    assert_eq!(f.trace.panicked, panicked);
                    let currentness = match operation {
                        "unmap_gpu" => 1,
                        "unmap_cpu" => 3,
                        "free" => {
                            if matches!(kind, 1 | 3) {
                                4
                            } else {
                                3
                            }
                        }
                        _ => {
                            if matches!(kind, 1 | 3) {
                                5
                            } else {
                                4
                            }
                        }
                    };
                    f.assert_native(SharedMemorySessionPhaseV1::Quarantined, currentness, 0);
                    f.assert_retry();
                    f.base.restore_and_transport(true);
                }
            }
        }
    }
}

#[test]
fn data_release_closing_currentness_failure_retains_native_disposal_receipt() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for panicked in [false, true] {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let offset = if matches!(kind, 1 | 3) { 6 } else { 5 };
                f.base
                    .memory_mut()
                    .insertion_arm_currentness_v1(offset, panicked);
                let released = f.release(f.index);
                assert!(released.transport);
                if panicked {
                    assert_eq!(
                        released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        released.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                            MemorySessionError::Injected("currentness")
                        )))
                    ));
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let retained = f.retained();
                assert!(retained.native_disposed && retained.failed && !retained.complete);
                assert_eq!(retained.owner, "NativeDisposed");
                f.assert_prefix(&before, false);
                f.assert_native(SharedMemorySessionPhaseV1::Quarantined, offset, 0);
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_original_native_panic_wins_over_secondary_retake_failures() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for closing in 0..4 {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let offset = trace().borrow().calls.len();
                f.base.memory_mut().fail_cleanup("free", true);
                arm_retake(&mut f, closing);
                let released = f.release(f.index);
                assert!(released.transport);
                assert_eq!(
                    released.result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "free"))
                );
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert!(f.retained().failed);
                f.assert_prefix(&before, false);
                assert_eq!(
                    trace().borrow().calls[offset..]
                        .iter()
                        .filter(|&&s| s == "auxiliary-retake")
                        .count(),
                    1
                );
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_missing_and_duplicate_identity_retain_before_native_work() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for duplicate in [false, true] {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let other = (f.index + 1) % f.base.data.len();
                let mut context = f.base.context();
                let ids = context.ledger().identities;
                if duplicate {
                    ids[other] = ids[f.index];
                } else {
                    ids[f.index] = ids[other];
                }
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let released = f.release(f.index);
                assert!(released.transport);
                if duplicate {
                    assert!(matches!(
                        released.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "duplicate detached storage identity"
                        )))
                    ));
                } else {
                    assert!(matches!(
                        released.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::InvalidData {
                                index: 4,
                                detail: "detached release storage identity"
                            }
                        )))
                    ));
                }
                assert_eq!(f.snapshot(), before);
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert_eq!(
                    (f.trace.calls, f.trace.lower_calls, f.trace.poisons),
                    (0, 0, 1)
                );
                assert_eq!(f.retained().owner, "Original");
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_model_opening_rejection_and_panic_retain_original_input() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for opening in 0..3 {
                let mut f = ReleaseFixture::new(ordinal, kind);
                match opening {
                    0 => f.base.scope.parent.faults.loan = Outcome::Error,
                    1 => f.base.scope.parent.faults.loan = Outcome::Panic,
                    _ => f.base.memory_mut().primary_expire_loan_generation_v1(),
                }
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let occurrence = next_occurrence("auxiliary-loan");
                let released = f.release(f.index);
                assert!(released.transport);
                match (opening, &released.result) {
                    (0, Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name)))) => {
                        assert_eq!(*name, "auxiliary-loan")
                    }
                    (1, Err(payload)) => assert_eq!(
                        payload.downcast_ref::<(&str, usize)>(),
                        Some(&("auxiliary-loan", occurrence))
                    ),
                    (
                        2,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                            name,
                        )))),
                    ) => assert_eq!(*name, "fixture live foundation loan"),
                    _ => panic!("unexpected release opening failure provenance"),
                }
                assert_eq!(f.snapshot(), before);
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert_eq!(
                    (f.trace.calls, f.trace.lower_calls, f.trace.poisons),
                    (1, 0, 1)
                );
                assert!(f.trace.before_retake.is_none());
                let retained = f.retained();
                assert_eq!(retained.owner, "Original");
                assert!(!retained.started);
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_incomplete_success_and_commit_panic_cannot_remove_ledger_entry() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for commit_panic in [false, true] {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                f.trace.skip_lower = !commit_panic;
                f.trace.commit_panic = commit_panic;
                let released = f.release(f.index);
                assert!(released.transport);
                if commit_panic {
                    assert_eq!(
                        released.result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"data release commit ledger access")
                    );
                } else {
                    assert!(matches!(
                        released.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                            MemorySessionError::InvalidAllocationAuthority
                        )))
                    ));
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let retained = f.retained();
                assert_eq!(retained.complete, commit_panic);
                assert_eq!(
                    retained.owner,
                    if commit_panic {
                        "NativeDisposed"
                    } else {
                        "Original"
                    }
                );
                f.assert_prefix(&before, commit_panic);
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_partial_unmap_preserves_exact_prefix_without_reusable_hole() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                f.base.memory_mut().arm_data_release_unmap_v1(prefix, errno);
                let released = f.release(f.index);
                assert!(released.transport);
                let error = released.result.unwrap().unwrap_err();
                let host = matches!(kind, 1 | 3);
                if prefix > 1 {
                    let expected = if host {
                        "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                    } else {
                        "device-memory UNMAP_MEMORY_FROM_GPU cumulative n_success"
                    };
                    assert!(matches!(error, ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::KernelResultMalformed(detail)
                    ) if detail == expected));
                } else if errno {
                    assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                            "unmap_gpu"
                        ))
                    ));
                } else {
                    let expected = if host {
                        "shared UNMAP_MEMORY_FROM_GPU full prefix"
                    } else {
                        "device-memory UNMAP_MEMORY_FROM_GPU full prefix"
                    };
                    assert!(matches!(error, ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::KernelResultMalformed(detail)
                    ) if detail == expected));
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let retained = f.retained();
                assert_eq!(retained.unmap, (true, Some(!errno), Some(prefix)));
                assert_eq!(retained.owner, "Mapped");
                assert_eq!(retained.disposal, [(false, None); 3]);
                f.assert_prefix(&before, false);
                f.assert_native(SharedMemorySessionPhaseV1::Quarantined, 1, 0);
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_host_model_commit_failures_retain_unmap_or_disposal_receipt() {
    for ordinal in 0..3 {
        for kind in [1, 3] {
            for stage in [CleanupStageV1::UnmapCommit, CleanupStageV1::ReleaseCommit] {
                for panicked in [false, true] {
                    let mut f = ReleaseFixture::new(ordinal, kind);
                    let before = f.snapshot();
                    let ledger = f.base.context().ledger_snapshot();
                    f.base
                        .memory_mut()
                        .arm_data_release_projection_v1(stage, panicked);
                    let released = f.release(f.index);
                    assert!(released.transport);
                    if panicked {
                        assert_eq!(
                            released
                                .result
                                .unwrap_err()
                                .downcast_ref::<(&str, CleanupStageV1)>(),
                            Some(&("control cleanup projection", stage))
                        );
                    } else {
                        assert!(matches!(
                            released.result,
                            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                                MemorySessionError::Injected("control cleanup projection")
                            )))
                        ));
                    }
                    assert_eq!(f.base.context().ledger_snapshot(), ledger);
                    let retained = f.retained();
                    assert_eq!(retained.stage, stage);
                    assert_eq!(
                        retained.native_disposed,
                        stage == CleanupStageV1::ReleaseCommit
                    );
                    assert!(retained.failed && !retained.complete);
                    f.assert_prefix(&before, false);
                    f.assert_native(
                        SharedMemorySessionPhaseV1::Quarantined,
                        if stage == CleanupStageV1::UnmapCommit {
                            2
                        } else {
                            6
                        },
                        0,
                    );
                    f.assert_retry();
                    f.base.restore_and_transport(true);
                }
            }
        }
    }
}

#[test]
fn data_release_retake_failure_precedes_ordinary_native_error() {
    for ordinal in 0..3 {
        for kind in [0, 1] {
            for closing in 0..4 {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                let pre = next_occurrence("auxiliary-retake");
                let post = next_occurrence("auxiliary-retake-complete");
                f.base.memory_mut().fail_cleanup("free", false);
                arm_retake(&mut f, closing);
                let released = f.release(f.index);
                assert!(released.transport);
                assert_retake_error(&released, closing, pre, post);
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert!(f.retained().failed);
                f.assert_prefix(&before, false);
                f.assert_native(
                    SharedMemorySessionPhaseV1::Quarantined,
                    if kind == 1 { 4 } else { 3 },
                    0,
                );
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_revision_exhaustion_poison_hook_preserves_original_error() {
    for ordinal in 0..3 {
        for kind in [1, 3] {
            for stage in [
                CleanupStageV1::UnmapPreflight,
                CleanupStageV1::ReleasePreflight,
            ] {
                let mut f = ReleaseFixture::new(ordinal, kind);
                let before = f.snapshot();
                let ledger = f.base.context().ledger_snapshot();
                f.base.memory_mut().exhaust_data_release_revision_v1(stage);
                let released = f.release(f.index);
                assert!(released.transport);
                assert!(matches!(
                    released.result,
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Model(
                            "queue foundation certificate revision exhausted"
                        )
                    )))
                ));
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let retained = f.retained();
                assert!(retained.failed && !retained.complete && !retained.native_disposed);
                assert_eq!(retained.stage, stage);
                f.assert_prefix(&before, false);
                f.assert_native(
                    SharedMemorySessionPhaseV1::Quarantined,
                    if stage == CleanupStageV1::UnmapPreflight {
                        0
                    } else {
                        2
                    },
                    1,
                );
                f.assert_retry();
                f.base.restore_and_transport(true);
            }
        }
    }
}

#[test]
fn data_release_preflight_rejection_retains_input_without_new_terminal_transport() {
    for ordinal in 0..3 {
        for kind in 0..5 {
            let mut f = ReleaseFixture::new(ordinal, kind);
            if let Some(lane) = f.base.lane.as_mut() {
                lane.unpublished_dispatch.continuation = None;
            } else {
                f.base.primary.continuation = None;
            }
            let before = f.snapshot();
            let ledger = f.base.context().ledger_snapshot();
            let released = f.release(f.index);
            assert!(!released.transport);
            assert!(matches!(
                released.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )))
            ));
            assert_eq!(f.snapshot(), before);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert_eq!(
                (f.trace.calls, f.trace.lower_calls, f.trace.poisons),
                (0, 0, 0)
            );
            assert!(!f.base.scope.parent.poisoned);
            let retained = f.retained();
            assert_eq!(retained.owner, "Original");
            assert!(!retained.started);
            f.base.restore_and_transport(false);
        }
    }
}
