//! Real constructed-parent foundations and mapped tokens run the production trim driver.

use super::*;
use crate::queue::dispatch_binding::DispatchDataInputStorageV1;
use crate::queue::live::pool_trim::{
    PoolTrimContextV1, PoolTrimPartsV1, SdmaPoolTrimCustodyV1, trim_in_place,
};
use crate::sdma::Gfx942SdmaBufferStorageV1;
use crate::shared_memory::{CleanupStageV1 as Stage, DataCleanupMetadataV1};

#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    Error,
    Panic,
    Regression,
}

struct TrimParent {
    parent: Parent,
    free: Vec<Gfx942SdmaBufferV1>,
    custody: Option<SdmaPoolTrimCustodyV1>,
    opening: Fault,
    closing: Fault,
    calls: Vec<&'static str>,
}

impl PoolTrimContextV1 for TrimParent {
    type Memory = Memory;
    fn parts(&mut self) -> Result<PoolTrimPartsV1<'_, Memory>, ComputeAqlQueueSessionErrorV1> {
        Ok(PoolTrimPartsV1 {
            memory: &mut self.parent.engine.backend.session,
            free: &mut self.free,
            custody: &mut self.custody,
        })
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.calls.push("loan");
        match self.opening {
            Fault::Error => return Err(ComputeAqlQueueSessionErrorV1::Contract("trim loan")),
            Fault::Panic => std::panic::panic_any("trim loan"),
            _ => (),
        }
        let e = &mut self.parent.engine;
        assert!(e.backend.foundation_in_engine);
        let loan = e.backend.session.primary_loan(&mut e.foundation)?;
        e.backend.foundation_in_engine = false;
        Ok(loan)
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.calls.push("retake");
        match self.closing {
            Fault::Error => return Err(ComputeAqlQueueSessionErrorV1::Contract("trim retake")),
            Fault::Panic => std::panic::panic_any("trim retake"),
            Fault::Regression => self
                .parent
                .engine
                .backend
                .session
                .primary_regress_loan_revision_v1(&loan),
            Fault::None => (),
        }
        let e = &mut self.parent.engine;
        assert!(!e.backend.foundation_in_engine);
        e.backend.session.primary_reclaim(&mut e.foundation, loan)?;
        e.backend.foundation_in_engine = true;
        Ok(())
    }
    fn poison(&mut self) {
        self.parent.poison_release();
    }
}

impl TrimParent {
    fn new(parent: Parent) -> Self {
        Self {
            parent,
            free: Vec::new(),
            custody: None,
            opening: Fault::None,
            closing: Fault::None,
            calls: Vec::new(),
        }
    }

    fn populate(&mut self, count: usize, host_first: bool) {
        let loan = self.loan().unwrap();
        for i in 0..count {
            let memory = &mut self.parent.engine.backend.session;
            let data = if i.is_multiple_of(2) == host_first {
                memory.host(true)
            } else {
                memory.device(false)
            };
            let storage = match data.into_parts().storage {
                DispatchDataInputStorageV1::HostVisible(token) => {
                    Gfx942SdmaBufferStorageV1::Host(token)
                }
                DispatchDataInputStorageV1::Device(lease) => {
                    Gfx942SdmaBufferStorageV1::Device(lease)
                }
            };
            let mut buffer =
                Gfx942SdmaBufferV1::from_bridge_parts(storage, self.parent.key, 1, 17 + i as u64);
            buffer.advance_pool_generation().unwrap();
            assert_eq!(buffer.pool_generation(), 2);
            self.free.push(buffer);
        }
        self.retake(loan).unwrap();
        self.calls.clear();
    }

    fn no_retry(&mut self) {
        let before = self
            .parent
            .engine
            .backend
            .session
            .data_release_snapshot_v1(&self.parent.engine.foundation);
        let active = self
            .custody
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .observation();
        let count = self.custody.as_ref().unwrap().released;
        let free: Vec<_> = self
            .free
            .iter()
            .map(Gfx942SdmaBufferV1::cleanup_metadata)
            .collect();
        let calls = self.calls.clone();
        assert!(matches!(
            trim_in_place(self),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA pool trim"
            ))
        ));
        assert_eq!(self.calls, calls);
        assert_eq!(self.custody.as_ref().unwrap().released, count);
        assert_eq!(
            self.custody
                .as_ref()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .observation(),
            active
        );
        assert_eq!(
            self.free
                .iter()
                .map(Gfx942SdmaBufferV1::cleanup_metadata)
                .collect::<Vec<_>>(),
            free
        );
        assert_eq!(
            self.parent
                .engine
                .backend
                .session
                .data_release_snapshot_v1(&self.parent.engine.foundation),
            before
        );
    }
}

#[test]
fn constructed_pool_trim_empty_repeated_and_directional_teardown() {
    let (parent, t, gate) = sdma_cases::with_sdma(false);
    let mut f = TrimParent::new(parent);
    assert_eq!(trim_in_place(&mut f).unwrap(), 0);
    assert!(f.calls.is_empty() && f.custody.is_none());
    for host_first in [false, true] {
        f.populate(3, host_first);
        let order: Vec<_> = f
            .free
            .iter()
            .rev()
            .map(Gfx942SdmaBufferV1::storage_identity)
            .collect();
        let before = f
            .parent
            .engine
            .backend
            .session
            .data_release_snapshot_v1(&f.parent.engine.foundation);
        assert_eq!(trim_in_place(&mut f).unwrap(), 3);
        assert!(f.free.is_empty() && f.custody.is_none() && !f.parent.poisoned);
        assert_eq!(
            f.calls,
            ["loan", "retake", "loan", "retake", "loan", "retake"]
        );
        before.assert_prepared_data_prefix_v1(
            &f.parent.engine.backend.session,
            &f.parent.engine.foundation,
            &order,
            3,
            None,
        );
    }
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    state.release_in_place(&mut f.parent).unwrap();
    f.parent
        .engine
        .backend
        .session
        .primary_assert_all_released_v1();
    assert!(state.complete && state.sdma.as_ref().unwrap().is_complete());
    assert_eq!(gate.observation(), (false, false));
    assert_no_retry(&mut f.parent, &mut state, &t);
    state.sdma.as_mut().unwrap().cleanup_local_mappings();
}

#[derive(Clone, Copy, Debug)]
enum CleanupFault {
    Native(&'static str),
    Currentness(usize),
    Projection(Stage),
}

fn failure(index: usize, host_first: bool, fault: CleanupFault, panic: bool) {
    let (parent, _, _) = constructed(false);
    let mut f = TrimParent::new(parent);
    f.populate(3, host_first);
    let order: Vec<_> = f
        .free
        .iter()
        .rev()
        .map(Gfx942SdmaBufferV1::storage_identity)
        .collect();
    let metadata: Vec<_> = f
        .free
        .iter()
        .map(Gfx942SdmaBufferV1::cleanup_metadata)
        .collect();
    let before = f
        .parent
        .engine
        .backend
        .session
        .data_release_snapshot_v1(&f.parent.engine.foundation);
    let memory = &mut f.parent.engine.backend.session;
    match fault {
        CleanupFault::Native(op) => memory.primary_arm_data_native_v1(index, op, panic),
        CleanupFault::Currentness(offset) => {
            memory.primary_arm_data_currentness_v1(index, offset, panic)
        }
        CleanupFault::Projection(stage) => {
            memory.primary_arm_data_projection_v1(index, stage, panic)
        }
    }
    let result = catch_unwind(AssertUnwindSafe(|| trim_in_place(&mut f)));
    if panic {
        let payload = result.unwrap_err();
        match fault {
            CleanupFault::Native(op) => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", op))
            ),
            CleanupFault::Currentness(_) => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "currentness"))
            ),
            CleanupFault::Projection(stage) => assert_eq!(
                payload.downcast_ref::<(&str, Stage)>(),
                Some(&("control cleanup projection", stage))
            ),
        }
    } else {
        let expected = match fault {
            CleanupFault::Native(op) => op,
            CleanupFault::Currentness(_) => "currentness",
            CleanupFault::Projection(_) => "control cleanup projection",
        };
        assert!(
            matches!(result.unwrap(), Err(ComputeAqlQueueSessionErrorV1::Sdma(crate::sdma::Gfx942SdmaErrorV1::Memory(MemorySessionError::Injected(actual)))) if actual == expected)
        );
    }
    assert!(f.parent.poisoned);
    assert_eq!(f.calls.len(), 2 * (index + 1));
    let root = f.custody.as_ref().unwrap();
    assert_eq!(root.released, index);
    let active = root.active.as_ref().unwrap().observation();
    assert!(active.started && active.failed && !active.complete);
    match fault {
        CleanupFault::Native(op) => {
            assert_eq!(
                active.owner,
                if op == "unmap_gpu" {
                    "Mapped"
                } else {
                    "Unmapped"
                }
            );
            assert!(!active.native_disposed);
            assert_eq!(
                active.stage,
                if op == "unmap_gpu" {
                    Stage::NativeUnmap
                } else {
                    Stage::NativeRelease
                }
            );
        }
        CleanupFault::Currentness(offset) => {
            let host = index.is_multiple_of(2) == host_first;
            let disposed = offset == if host { 6 } else { 5 };
            assert_eq!(
                active.owner,
                if offset <= 2 {
                    "Mapped"
                } else if disposed {
                    "NativeDisposed"
                } else {
                    "Unmapped"
                }
            );
            assert_eq!(active.native_disposed, disposed);
        }
        CleanupFault::Projection(stage) => {
            assert_eq!(active.stage, stage);
            assert_eq!(active.native_disposed, stage == Stage::ReleaseCommit);
        }
    }
    assert_eq!(
        active.metadata,
        DataCleanupMetadataV1::Sdma(metadata[2 - index].clone())
    );
    assert_eq!(
        f.free
            .iter()
            .map(Gfx942SdmaBufferV1::cleanup_metadata)
            .collect::<Vec<_>>(),
        metadata[..2 - index]
    );
    before.assert_prepared_data_prefix_v1(
        &f.parent.engine.backend.session,
        &f.parent.engine.foundation,
        &order,
        index,
        Some(&active),
    );
    f.no_retry();
}

#[test]
fn constructed_pool_trim_native_errors_and_panics_keep_first_middle_last_owners() {
    for host_first in [false, true] {
        for index in 0_usize..3 {
            let host = index.is_multiple_of(2) == host_first;
            for op in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
                if !host && op == "unmap_cpu" {
                    continue;
                }
                for panic in [false, true] {
                    failure(index, host_first, CleanupFault::Native(op), panic);
                }
            }
        }
    }
}

#[test]
fn constructed_pool_trim_currentness_and_projection_keep_exact_prefixes() {
    for host in [false, true] {
        for offset in 1..=if host { 6 } else { 5 } {
            for panic in [false, true] {
                failure(1, !host, CleanupFault::Currentness(offset), panic);
            }
        }
    }
    for stage in [
        Stage::UnmapProjection,
        Stage::UnmapCommit,
        Stage::ReleaseProjection,
        Stage::ReleaseCommit,
    ] {
        for panic in [false, true] {
            failure(1, false, CleanupFault::Projection(stage), panic);
        }
    }
}

#[test]
fn constructed_pool_trim_opening_rejection_or_panic_keeps_original_input() {
    for fault in [Fault::Error, Fault::Panic, Fault::Regression] {
        let (parent, _, _) = constructed(false);
        let mut f = TrimParent::new(parent);
        f.populate(3, true);
        if fault == Fault::Regression {
            f.parent
                .engine
                .backend
                .session
                .primary_expire_loan_generation_v1();
        }
        f.opening = fault;
        let before = f
            .parent
            .engine
            .backend
            .session
            .data_release_snapshot_v1(&f.parent.engine.foundation);
        let original = f.free.last().unwrap().cleanup_metadata();
        let result = catch_unwind(AssertUnwindSafe(|| trim_in_place(&mut f)));
        if fault == Fault::Panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"trim loan")
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(f.calls, ["loan"]);
        assert!(f.parent.poisoned && f.parent.engine.backend.foundation_in_engine);
        let root = f.custody.as_ref().unwrap();
        assert_eq!(root.released, 0);
        let active = root.active.as_ref().unwrap().observation();
        assert_eq!(active.metadata, DataCleanupMetadataV1::Sdma(original));
        assert_eq!(active.owner, "Original");
        assert!(!active.started);
        assert_eq!(
            f.parent
                .engine
                .backend
                .session
                .data_release_snapshot_v1(&f.parent.engine.foundation),
            before
        );
        f.no_retry();
    }
}

#[test]
fn constructed_pool_trim_cleanup_retake_matrix_preserves_receipts_and_first_panic() {
    for host in [false, true] {
        for cleanup in [Fault::None, Fault::Error, Fault::Panic] {
            for closing in [Fault::None, Fault::Error, Fault::Panic] {
                if cleanup == Fault::None && closing == Fault::None {
                    continue;
                }
                let (parent, _, _) = constructed(false);
                let mut f = TrimParent::new(parent);
                f.populate(1, host);
                let order = [f.free[0].storage_identity()];
                let metadata = f.free[0].cleanup_metadata();
                let before = f
                    .parent
                    .engine
                    .backend
                    .session
                    .data_release_snapshot_v1(&f.parent.engine.foundation);
                if cleanup != Fault::None {
                    f.parent.engine.backend.session.primary_arm_data_native_v1(
                        0,
                        "free",
                        cleanup == Fault::Panic,
                    );
                }
                f.closing = closing;
                let result = catch_unwind(AssertUnwindSafe(|| trim_in_place(&mut f)));
                if cleanup == Fault::Panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "free"))
                    );
                } else if closing == Fault::Panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"trim retake")
                    );
                } else if closing == Fault::Error {
                    assert!(matches!(
                        result.unwrap(),
                        Err(ComputeAqlQueueSessionErrorV1::Contract("trim retake"))
                    ));
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert!(f.parent.poisoned && f.free.is_empty());
                assert_eq!(f.calls, ["loan", "retake"]);
                assert_eq!(
                    f.parent.engine.backend.foundation_in_engine,
                    closing == Fault::None
                );
                let root = f.custody.as_ref().unwrap();
                assert_eq!(root.released, 0);
                let active = root.active.as_ref().unwrap().observation();
                assert_eq!(active.complete, cleanup == Fault::None);
                assert_eq!(active.metadata, DataCleanupMetadataV1::Sdma(metadata));
                before.assert_prepared_data_prefix_v1(
                    &f.parent.engine.backend.session,
                    &f.parent.engine.foundation,
                    &order,
                    usize::from(cleanup == Fault::None),
                    (cleanup != Fault::None).then_some(&active),
                );
                f.no_retry();
            }
        }
    }
}

#[test]
fn constructed_pool_trim_actual_reclaim_rejection_retains_disposed_receipt() {
    let (parent, _, _) = constructed(false);
    let mut f = TrimParent::new(parent);
    f.populate(1, true);
    f.closing = Fault::Regression;
    assert!(trim_in_place(&mut f).is_err());
    assert!(
        f.parent.poisoned && !f.parent.engine.backend.foundation_in_engine && f.free.is_empty()
    );
    let root = f.custody.as_ref().unwrap();
    assert_eq!(root.released, 0);
    let active = root.active.as_ref().unwrap().observation();
    assert!(active.complete && active.native_disposed);
    let before = f
        .parent
        .engine
        .backend
        .session
        .control_release_loan_snapshot_v1();
    assert!(trim_in_place(&mut f).is_err());
    assert_eq!(f.calls, ["loan", "retake"]);
    assert_eq!(
        f.parent
            .engine
            .backend
            .session
            .control_release_loan_snapshot_v1(),
        before
    );
    assert_eq!(
        f.custody
            .as_ref()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .observation(),
        active
    );
}
