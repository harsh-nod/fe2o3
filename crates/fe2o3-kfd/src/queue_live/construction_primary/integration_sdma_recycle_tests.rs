//! Genuine constructed mappings exercise the shared cache/disposal transaction.

#![allow(
    clippy::result_large_err,
    reason = "exercise the production inline returned-owner failure without adding allocation"
)]

use super::*;
use crate::queue::live::sdma_recycle::{
    SdmaRecycleContextV1, SdmaRecycleCustodyV1 as Root, SdmaRecycleRootsV1, recycle_in_place,
};
use crate::shared_memory::{CleanupStageV1 as Stage, DataCleanupMetadataV1};

struct RecycleParent {
    base: AllocationParent,
    root: Option<Root>,
    free: Vec<Gfx942SdmaBufferV1>,
    device_limits: Option<crate::Gfx942DevicePoolLimitsV1>,
    host_limits: Option<crate::Gfx942HostPoolLimitsV1>,
    admission: Fault,
    policy: Fault,
    cleanup: Fault,
    skip_cleanup: bool,
    reserve: bool,
    activity: bool,
    policy_calls: std::cell::Cell<usize>,
}

impl RecycleParent {
    fn new() -> Self {
        Self {
            base: AllocationParent::new(),
            root: None,
            free: Vec::new(),
            device_limits: None,
            host_limits: None,
            admission: Fault::None,
            policy: Fault::None,
            cleanup: Fault::None,
            skip_cleanup: false,
            reserve: true,
            activity: false,
            policy_calls: std::cell::Cell::new(0),
        }
    }
    fn buffer(&mut self, host: bool, bytes: usize) -> Gfx942SdmaBufferV1 {
        let buffer = self.base.allocate(host, bytes).unwrap();
        self.base.calls.clear();
        buffer
    }
    fn limits(&mut self, bytes: u64, records: usize) {
        self.device_limits = Some(crate::Gfx942DevicePoolLimitsV1::new(bytes, records).unwrap());
        self.host_limits = Some(crate::Gfx942HostPoolLimitsV1::new(bytes, records).unwrap());
    }
    fn finish(mut self) {
        assert!(self.root.is_none() && self.base.outstanding == 0);
        while let Some(buffer) = self.free.pop() {
            // Explicit fixture checkout restores the one logical outstanding debit.
            self.base.outstanding += 1;
            recycle_in_place(&mut self, buffer, true).unwrap();
        }
        self.base.shutdown();
    }
}

impl SdmaRecycleContextV1 for RecycleParent {
    fn owner(&self) -> QueueKeyV1 {
        self.base.parent.key
    }
    fn note_foreign_attempt(&mut self) {
        if !self.base.parent.poisoned && self.root.is_none() {
            self.activity = true;
        }
    }
    fn roots(&mut self) -> SdmaRecycleRootsV1<'_> {
        SdmaRecycleRootsV1 {
            custody: &mut self.root,
            free: &mut self.free,
            outstanding: &mut self.base.outstanding,
        }
    }
    fn admit(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.activity = true;
        match self.admission {
            Fault::Error => {
                return Err(ComputeAqlQueueSessionErrorV1::Contract("recycle admission"));
            }
            Fault::Panic => std::panic::panic_any("recycle admission"),
            _ => (),
        }
        if self.base.parent.poisoned || self.base.parent.sdma.is_none() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unavailable recycle fixture",
            ));
        }
        Ok(())
    }
    fn should_dispose(&self) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        self.policy_calls.set(self.policy_calls.get() + 1);
        let Some(Root::Buffer(buffer)) = self.root.as_ref() else {
            panic!("rooted policy input");
        };
        let decision = self
            .base
            .parent
            .engine
            .backend
            .session
            .primary_sdma_recycle_disposition_v1(
                self.owner(),
                self.device_limits,
                self.host_limits,
                &self.free,
                buffer,
            )
            .map_err(Gfx942SdmaErrorV1::from)?;
        match self.policy {
            Fault::Error => Err(ComputeAqlQueueSessionErrorV1::Contract("recycle policy")),
            Fault::Panic => std::panic::panic_any("recycle policy"),
            _ => Ok(decision),
        }
    }
    fn reserve_cache(&mut self) -> bool {
        self.reserve && self.free.try_reserve(1).is_ok()
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::loan(&mut self.base)
    }
    fn release(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self.cleanup {
            Fault::Error => return Err(ComputeAqlQueueSessionErrorV1::Contract("recycle cleanup")),
            Fault::Panic => std::panic::panic_any("recycle cleanup"),
            _ => (),
        }
        if self.skip_cleanup {
            return Ok(());
        }
        let Some(Root::Disposal(root)) = self.root.as_mut() else {
            panic!("rooted disposal");
        };
        self.base
            .parent
            .engine
            .backend
            .session
            .release_data(root)
            .map_err(Gfx942SdmaErrorV1::from)?;
        match self.cleanup {
            Fault::AfterError => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "recycle cleanup after",
            )),
            Fault::AfterPanic => std::panic::panic_any("recycle cleanup after"),
            _ => Ok(()),
        }
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::retake(&mut self.base, loan)
    }
    fn poison(&mut self) {
        SdmaAllocationContextV1::poison(&mut self.base);
    }
}

#[test]
fn constructed_sdma_recycle_cache_disposal_and_explicit_release_refund_exact_backing() {
    for host in [false, true] {
        for bytes in [17, 4097] {
            for mode in 0..4 {
                let mut f = RecycleParent::new();
                let buffer = f.buffer(host, bytes);
                let metadata = buffer.cleanup_metadata();
                let id = buffer.storage_identity();
                let generation = buffer.pool_generation();
                let resources = original_resource_ids(&f.base.parent);
                let e = &f.base.parent.engine;
                let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
                if mode == 1 {
                    f.limits(16384, 2);
                }
                if mode == 2 {
                    f.limits(0, 0);
                }
                recycle_in_place(&mut f, buffer, mode == 3).unwrap();
                assert!(f.root.is_none() && !f.base.parent.poisoned && f.activity);
                assert_eq!(f.base.outstanding, 0);
                assert_eq!(original_resource_ids(&f.base.parent), resources);
                let e = &f.base.parent.engine;
                if mode < 2 {
                    assert!(f.base.calls.is_empty());
                    assert_eq!(f.free.len(), 1);
                    assert_eq!(f.free[0].storage_identity(), id);
                    assert_eq!(f.free[0].pool_generation(), generation + 1);
                    assert_eq!(f.free[0].requested_bytes(), bytes as u64);
                    assert_eq!(
                        e.backend.session.data_release_snapshot_v1(&e.foundation),
                        before
                    );
                    assert_ne!(f.free[0].cleanup_metadata(), metadata);
                } else {
                    assert!(f.free.is_empty());
                    assert_eq!(f.base.calls, ["loan", "retake"]);
                    before.assert_prepared_data_prefix_v1(
                        &e.backend.session,
                        &e.foundation,
                        &[id],
                        1,
                        None,
                    );
                }
                assert_eq!(f.policy_calls.get(), usize::from(mode != 3));
                f.finish();
            }
        }
    }
}

#[test]
fn constructed_sdma_recycle_pressure_preserves_cached_neighbors_and_disposes_only_candidate() {
    for host in [false, true] {
        let mut f = RecycleParent::new();
        f.limits(4096, 1);
        let neighbor = f.buffer(host, 17);
        recycle_in_place(&mut f, neighbor, false).unwrap();
        let neighbor = f.free[0].cleanup_metadata();
        let buffer = f.buffer(host, 4097);
        let id = buffer.storage_identity();
        let e = &f.base.parent.engine;
        let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
        recycle_in_place(&mut f, buffer, false).unwrap();
        assert_eq!(f.free.len(), 1);
        assert_eq!(f.free[0].cleanup_metadata(), neighbor);
        assert!(f.root.is_none() && f.base.outstanding == 0);
        let e = &f.base.parent.engine;
        before.assert_prepared_data_prefix_v1(&e.backend.session, &e.foundation, &[id], 1, None);
        f.finish();
    }
}

#[test]
fn constructed_sdma_recycle_reservation_rejection_returns_exact_buffer_for_successful_retry() {
    for host in [false, true] {
        let mut f = RecycleParent::new();
        f.limits(8192, 2);
        let neighbor = f.buffer(!host, 17);
        recycle_in_place(&mut f, neighbor, false).unwrap();
        let neighbor = f.free[0].cleanup_metadata();
        let buffer = f.buffer(host, 17);
        let metadata = buffer.cleanup_metadata();
        let e = &f.base.parent.engine;
        let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
        f.reserve = false;
        let (error, buffer) = recycle_in_place(&mut f, buffer, false)
            .unwrap_err()
            .into_parts();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Contract("SDMA pool allocation failed")
        ));
        let buffer = buffer.expect("healthy reservation rejection");
        assert_eq!(buffer.cleanup_metadata(), metadata);
        assert_eq!(f.free[0].cleanup_metadata(), neighbor);
        assert!(f.root.is_none() && !f.base.parent.poisoned && f.base.calls.is_empty());
        assert_eq!(f.base.outstanding, 1);
        let e = &f.base.parent.engine;
        assert_eq!(
            e.backend.session.data_release_snapshot_v1(&e.foundation),
            before
        );
        f.reserve = true;
        recycle_in_place(&mut f, buffer, false).unwrap();
        assert_eq!(f.base.outstanding, 0);
        assert_eq!(f.free.len(), 2);
        f.finish();
    }
}

#[test]
fn constructed_sdma_recycle_admission_policy_and_generation_failure_retain_original_input() {
    for host in [false, true] {
        for case in 0..8 {
            let mut f = RecycleParent::new();
            let buffer = f.buffer(host, 17);
            let buffer = if case == 7 {
                let (storage, owner, _, logical) = buffer.into_bridge_parts();
                Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, u64::MAX, logical)
            } else {
                buffer
            };
            let metadata = buffer.cleanup_metadata();
            match case {
                0 => f.admission = Fault::Error,
                1 => f.admission = Fault::Panic,
                2 => f.base.parent.poisoned = true,
                3 => f.base.outstanding = 0,
                4 => f.policy = Fault::Error,
                5 => f.policy = Fault::Panic,
                6 => {
                    f.limits(4096, 1);
                    crate::queue::dispatch_binding::preparation::PreparationMemoryV1::quarantine(
                        &mut f.base.parent.engine.backend.session,
                    );
                }
                _ => (),
            }
            f.base.poison_panics = true;
            let outstanding = f.base.outstanding;
            let e = &f.base.parent.engine;
            let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
            let result = catch_unwind(AssertUnwindSafe(|| recycle_in_place(&mut f, buffer, false)));
            if matches!(case, 1 | 5) {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&if case == 1 {
                        "recycle admission"
                    } else {
                        "recycle policy"
                    })
                );
            } else {
                assert!(result.unwrap().unwrap_err().into_parts().1.is_none());
            }
            let Some(Root::Buffer(buffer)) = &f.root else {
                panic!("retained original input");
            };
            assert_eq!(buffer.cleanup_metadata(), metadata);
            assert!(f.base.parent.poisoned && f.free.is_empty() && f.base.calls.is_empty());
            assert_eq!(f.base.outstanding, outstanding);
            let e = &f.base.parent.engine;
            assert_eq!(
                e.backend.session.data_release_snapshot_v1(&e.foundation),
                before
            );
        }
    }
}

#[test]
fn constructed_sdma_recycle_cleanup_retake_matrix_keeps_receipts_and_first_panic() {
    for host in [false, true] {
        for cleanup in [
            Fault::None,
            Fault::Error,
            Fault::Panic,
            Fault::AfterError,
            Fault::AfterPanic,
        ] {
            for closing in [
                Fault::None,
                Fault::Error,
                Fault::Panic,
                Fault::AfterError,
                Fault::AfterPanic,
                Fault::Regression,
            ] {
                for poison_panics in [false, true] {
                    let mut f = RecycleParent::new();
                    let buffer = f.buffer(host, 17);
                    let id = buffer.storage_identity();
                    let metadata = buffer.cleanup_metadata();
                    f.cleanup = cleanup;
                    f.base.closing = closing;
                    f.base.poison_panics = poison_panics;
                    let e = &f.base.parent.engine;
                    let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
                    let result =
                        catch_unwind(AssertUnwindSafe(|| recycle_in_place(&mut f, buffer, true)));
                    let panic = match cleanup {
                        Fault::Panic => Some("recycle cleanup"),
                        Fault::AfterPanic => Some("recycle cleanup after"),
                        _ => match closing {
                            Fault::Panic => Some("allocation retake"),
                            Fault::AfterPanic => Some("allocation retake after"),
                            _ => None,
                        },
                    };
                    let success = cleanup == Fault::None && closing == Fault::None;
                    if let Some(expected) = panic {
                        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&expected));
                    } else if success {
                        result.unwrap().unwrap();
                    } else {
                        let (error, recovered) = result.unwrap().unwrap_err().into_parts();
                        assert!(recovered.is_none());
                        if closing == Fault::Error {
                            assert!(matches!(
                                error,
                                ComputeAqlQueueSessionErrorV1::Contract("allocation retake")
                            ));
                        } else if closing == Fault::AfterError {
                            assert!(matches!(
                                error,
                                ComputeAqlQueueSessionErrorV1::Contract("allocation retake after")
                            ));
                        }
                    }
                    assert_eq!(f.base.calls, ["loan", "retake"]);
                    assert!(f.free.is_empty());
                    if success {
                        assert!(
                            f.root.is_none() && !f.base.parent.poisoned && f.base.outstanding == 0
                        );
                        f.finish();
                    } else {
                        assert!(f.base.parent.poisoned && f.base.outstanding == 1);
                        let Some(Root::Disposal(root)) = &f.root else {
                            panic!("retained cleanup receipt");
                        };
                        let active = root.observation();
                        assert_eq!(active.metadata, DataCleanupMetadataV1::Sdma(metadata));
                        assert_eq!(
                            root.is_complete(),
                            !matches!(cleanup, Fault::Error | Fault::Panic)
                        );
                        let e = &f.base.parent.engine;
                        before.assert_prepared_data_prefix_v1(
                            &e.backend.session,
                            &e.foundation,
                            &[id],
                            usize::from(root.is_complete()),
                            (!root.is_complete()).then_some(&active),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn constructed_sdma_recycle_loan_failure_and_incomplete_success_never_remove_debit() {
    for host in [false, true] {
        for case in 0..3 {
            let mut f = RecycleParent::new();
            let buffer = f.buffer(host, 17);
            let metadata = buffer.cleanup_metadata();
            if case < 2 {
                f.base.opening = if case == 0 {
                    Fault::Error
                } else {
                    Fault::Panic
                };
            } else {
                f.skip_cleanup = true;
            }
            f.base.poison_panics = true;
            let result = catch_unwind(AssertUnwindSafe(|| recycle_in_place(&mut f, buffer, true)));
            if case == 1 {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"allocation loan")
                );
            } else {
                assert!(result.unwrap().unwrap_err().into_parts().1.is_none());
            }
            assert_eq!(
                f.base.calls,
                if case < 2 {
                    vec!["loan"]
                } else {
                    vec!["loan", "retake"]
                }
            );
            let Some(Root::Disposal(root)) = &f.root else {
                panic!("retained input cleanup");
            };
            assert!(!root.is_complete());
            assert_eq!(
                root.observation().metadata,
                DataCleanupMetadataV1::Sdma(metadata)
            );
            assert_eq!(f.base.outstanding, 1);
            assert!(f.base.parent.poisoned && f.free.is_empty());
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum NativeFault {
    Operation(&'static str),
    Currentness(usize),
    Projection(Stage),
}

#[test]
fn constructed_sdma_recycle_native_failures_retain_exact_disposal_and_account_prefixes() {
    for host in [false, true] {
        let mut faults = vec![
            NativeFault::Operation("unmap_gpu"),
            NativeFault::Operation("free"),
            NativeFault::Operation("release_va_reservation"),
        ];
        if host {
            faults.push(NativeFault::Operation("unmap_cpu"));
        }
        faults.extend((1..=if host { 6 } else { 5 }).map(NativeFault::Currentness));
        if host {
            faults.extend(
                [
                    Stage::UnmapProjection,
                    Stage::UnmapCommit,
                    Stage::ReleaseProjection,
                    Stage::ReleaseCommit,
                ]
                .map(NativeFault::Projection),
            );
        }
        for fault in faults {
            for panic in [false, true] {
                let mut f = RecycleParent::new();
                f.limits(0, 0);
                let buffer = f.buffer(host, 17);
                let id = buffer.storage_identity();
                let metadata = buffer.cleanup_metadata();
                let e = &f.base.parent.engine;
                let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
                let memory = &mut f.base.parent.engine.backend.session;
                match fault {
                    NativeFault::Operation(op) => memory.primary_arm_data_native_v1(0, op, panic),
                    NativeFault::Currentness(offset) => {
                        memory.primary_arm_data_currentness_v1(0, offset, panic)
                    }
                    NativeFault::Projection(stage) => {
                        memory.primary_arm_data_projection_v1(0, stage, panic)
                    }
                }
                f.base.poison_panics = true;
                let result =
                    catch_unwind(AssertUnwindSafe(|| recycle_in_place(&mut f, buffer, false)));
                if panic {
                    assert!(result.is_err(), "host={host}, fault={fault:?}");
                    let payload = result.unwrap_err();
                    match fault {
                        NativeFault::Operation(op) => assert_eq!(
                            payload.downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", op))
                        ),
                        NativeFault::Currentness(_) => assert_eq!(
                            payload.downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", "currentness"))
                        ),
                        NativeFault::Projection(stage) => assert_eq!(
                            payload.downcast_ref::<(&str, Stage)>(),
                            Some(&("control cleanup projection", stage))
                        ),
                    }
                } else {
                    let result = result.unwrap();
                    assert!(result.is_err(), "host={host}, fault={fault:?}");
                    assert!(result.unwrap_err().into_parts().1.is_none());
                }
                assert!(f.base.parent.poisoned && f.free.is_empty());
                assert_eq!(f.base.outstanding, 1);
                assert_eq!(f.base.calls, ["loan", "retake"]);
                let Some(Root::Disposal(root)) = &f.root else {
                    panic!("retained native cleanup");
                };
                let active = root.observation();
                assert!(active.started && active.failed && !active.complete);
                assert_eq!(active.metadata, DataCleanupMetadataV1::Sdma(metadata));
                let e = &f.base.parent.engine;
                before.assert_prepared_data_prefix_v1(
                    &e.backend.session,
                    &e.foundation,
                    &[id],
                    0,
                    Some(&active),
                );
            }
        }
    }
}

#[test]
fn constructed_sdma_recycle_foreign_attempt_latches_only_healthy_empty_session() {
    for release in [false, true] {
        for terminal in [false, true] {
            let mut f = RecycleParent::new();
            let buffer = f.buffer(false, 17);
            let metadata = buffer.cleanup_metadata();
            let e = &f.base.parent.engine;
            let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
            let mut session =
                crate::queue::live::tests::persistent_compute_cancellation_test_session(
                    crate::queue::live::tests::test_queue_key(91, 1),
                    None,
                    None,
                );
            session.terminal_poisoned = terminal;
            let failure = if release {
                session.release_sdma_buffer(buffer)
            } else {
                session.recycle_sdma_buffer(buffer)
            }
            .unwrap_err();
            let (error, recovered) = failure.into_parts();
            assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner")
            ));
            let buffer = recovered.expect("foreign buffer returned unchanged");
            assert_eq!(buffer.cleanup_metadata(), metadata);
            assert_eq!(session.sdma_device_pool.activity_started, !terminal);
            assert_eq!(session.terminal_poisoned, terminal);
            assert!(session.sdma_recycle.is_none() && session.sdma_pool_free.is_empty());
            assert_eq!(
                (
                    session.sdma_outstanding_buffers,
                    session.sdma_pool_reuse_count
                ),
                (0, 0)
            );
            assert_eq!(
                e.backend.session.data_release_snapshot_v1(&e.foundation),
                before
            );
            if !terminal {
                assert!(matches!(
                    session
                        .sdma_device_pool
                        .configure(crate::Gfx942DevicePoolLimitsV1::new(0, 0).unwrap()),
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA device pool configuration is immutable after configuration or activity"
                    ))
                ));
                assert!(matches!(
                    session.configure_sdma_host_pool_v1(
                        crate::Gfx942HostPoolLimitsV1::new(0, 0).unwrap()
                    ),
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA host pool configuration requires a fresh SDMA resource history"
                    ))
                ));
            }
            drop(session);
            recycle_in_place(&mut f, buffer, true).unwrap();
            f.finish();
        }
    }
}

#[test]
fn constructed_sdma_recycle_public_guards_and_second_input_abort_without_overwrite() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SDMA_RECYCLE_DROP";
    const TEST: &str = "queue::live::construction_primary::integration_tests::release_cases::sdma_allocation_cases::recycle::constructed_sdma_recycle_public_guards_and_second_input_abort_without_overwrite";
    let Some(mode) = std::env::var_os(CHILD) else {
        for mode in [
            "buffer",
            "disposal",
            "recycle",
            "release",
            "recycle-disabled",
            "release-disabled",
            "recycle-terminal",
            "release-terminal",
        ] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, mode)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
            assert!(stderr.contains("recycle guards checked; entering fail-closed boundary"));
            assert!(!stderr.contains("panicked at"));
        }
        return;
    };
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    let mut f = RecycleParent::new();
    let buffer = f.buffer(false, 17);
    let metadata = buffer.cleanup_metadata();
    let mut session = core::mem::ManuallyDrop::new(
        crate::queue::live::tests::persistent_compute_cancellation_test_session(
            f.owner(),
            None,
            None,
        ),
    );
    let concrete = mode.to_str().unwrap().contains('-');
    if concrete {
        session.sdma_outstanding_buffers = 1;
        session.terminal_poisoned = mode.to_str().unwrap().ends_with("terminal");
        let e = &f.base.parent.engine;
        let before = e.backend.session.data_release_snapshot_v1(&e.foundation);
        let result = if mode.to_str().unwrap().starts_with("release") {
            session.release_sdma_buffer(buffer)
        } else {
            session.recycle_sdma_buffer(buffer)
        };
        let (error, recovered) = result.unwrap_err().into_parts();
        assert!(recovered.is_none());
        assert!(
            matches!(error, ComputeAqlQueueSessionErrorV1::Contract(message) if message == if mode.to_str().unwrap().ends_with("terminal") { "terminal queue session requires process teardown" } else { "SDMA copy engine is not enabled" })
        );
        assert!(session.terminal_poisoned && session.sdma_pool_free.is_empty());
        assert_eq!(
            e.backend.session.data_release_snapshot_v1(&e.foundation),
            before
        );
    } else {
        session.sdma_recycle = Some(if mode == "disposal" {
            Root::Disposal(DataCleanupCustodyV1::from_sdma(buffer))
        } else {
            Root::Buffer(buffer)
        });
    }
    for result in [
        session.enable_sdma_copy_engine().map(|_| ()),
        session
            .enable_gfx942_directional_sdma_copy_engines()
            .map(|_| ()),
        session
            .enable_gfx942_sdma_copy_engine_on_engine_index(0)
            .map(|_| ()),
        session
            .enable_gfx942_striped_sdma_copy_engines(2)
            .map(|_| ()),
        session
            .enable_gfx942_two_native_sdma_logical_mux_v2(2)
            .map(|_| ()),
        session
            .enable_gfx942_directional_and_striped_sdma_copy_engines_v1(2)
            .map(|_| ()),
        session.allocate_sdma_host_buffer(17).map(|_| ()),
        session.allocate_sdma_device_buffer(17, 4096).map(|_| ()),
        session.allocate_sdma_pooled_host_buffer(17).map(|_| ()),
        session
            .allocate_sdma_pooled_device_buffer(17, 4096)
            .map(|_| ()),
        session.trim_sdma_memory_pool().map(|_| ()),
        session.supports_retained_primary_release_v1().map(|_| ()),
        session.preflight_primary_release_v1(),
        session
            .destroy_queue_and_event(QueueDestroyModeV1::Release)
            .map(|_| ()),
    ] {
        assert!(matches!(
            result,
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA recycle"
            ))
        ));
    }
    assert_eq!(session.sdma_device_pool.activity_started, concrete);
    assert_eq!(
        (
            session.sdma_outstanding_buffers,
            session.sdma_pool_reuse_count
        ),
        (usize::from(concrete), 0)
    );
    match session.sdma_recycle.as_ref().unwrap() {
        Root::Buffer(buffer) => assert_eq!(buffer.cleanup_metadata(), metadata),
        Root::Disposal(root) => assert_eq!(
            root.observation().metadata,
            DataCleanupMetadataV1::Sdma(metadata)
        ),
    }
    let mut second = f.buffer(false, 17);
    if concrete {
        use crate::persistent_directional_sdma::Gfx942DirectionalPersistentSdmaPromotionCustodyV1;
        let failure = session
            .promote_sdma_device_buffer_to_directional_persistent_allocation_v1(second)
            .unwrap_err();
        let Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(retained) =
            failure.into_parts().1
        else {
            panic!("terminal recycler must not classify promotion as retryable");
        };
        second = retained.buffer;
    }
    let (storage, owner, generation, logical) = second.into_bridge_parts();
    let foreign = Gfx942SdmaBufferV1::from_bridge_parts(
        storage,
        crate::queue::live::tests::test_queue_key(91, 1),
        generation,
        logical,
    );
    let second = session
        .recycle_sdma_buffer(foreign)
        .unwrap_err()
        .into_parts()
        .1
        .expect("foreign owner returns before terminal admission");
    assert_eq!(session.sdma_device_pool.activity_started, concrete);
    let (storage, _, generation, logical) = second.into_bridge_parts();
    let second = Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, generation, logical);
    eprintln!("recycle guards checked; entering fail-closed boundary");
    if mode == "recycle" {
        let _ = session.recycle_sdma_buffer(second);
    } else if mode == "release" {
        let _ = session.release_sdma_buffer(second);
    } else {
        drop(core::mem::ManuallyDrop::into_inner(session));
    }
    panic!("unfinished recycle fail-closed boundary returned");
}
