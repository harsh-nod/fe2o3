//! Genuine registered owners and SDMA transitions with CPU-injected completion.
//! These tests do not execute a GPU copy or establish hardware performance.

use super::*;
use crate::persistent_compute::{
    Gfx942PersistentComputeStoragePromotionCustodyV1 as StorageCustody,
    Gfx942PersistentComputeStoragePromotionFailureV1 as StorageFailure,
    Gfx942PersistentComputeStoragePromotionTerminalCustodyV1 as StorageRoot,
};
use crate::queue::live::initialized_storage::{
    InitializedStorageContextV1, admit_allocation, promote_in_place as convert,
};

struct StorageParent {
    copy: SynchronousParent,
    root: Option<StorageRoot>,
    currentness_calls: usize,
    currentness_fault: Option<(usize, Fault)>,
    validation: Fault,
}

impl StorageParent {
    fn new() -> Self {
        Self {
            copy: SynchronousParent::new(),
            root: None,
            currentness_calls: 0,
            currentness_fault: None,
            validation: Fault::None,
        }
    }

    fn completed(&mut self, ranges: &[(u64, u32)]) -> Gfx942DirectionalPersistentSdmaCompletedV1 {
        self.completed_with_source(ranges, true)
    }

    fn completed_with_source(
        &mut self,
        ranges: &[(u64, u32)],
        initialized: bool,
    ) -> Gfx942DirectionalPersistentSdmaCompletedV1 {
        let mut input = self
            .copy
            .input(Gfx942PersistentSdmaDirectionV1::HostToDevice, 4096);
        if initialized {
            let base = &mut self.copy.promotion.base;
            let loan = SdmaAllocationContextV1::loan(base).unwrap();
            crate::sdma::write_host_buffer_for_test_v1(
                &mut base.parent.engine.backend.session,
                &mut input.host,
                0,
                &[0x69; 4096],
            )
            .unwrap();
            SdmaAllocationContextV1::retake(base, loan).unwrap();
        }
        input.host_offset = 0;
        for (index, &(offset, bytes)) in ranges.iter().enumerate() {
            input.device_offset = offset;
            input.copy_bytes = bytes;
            let completed = execute(&mut self.copy, input)
                .unwrap_or_else(|failure| panic!("{:?}", failure_error(&failure)));
            if index + 1 == ranges.len() {
                return completed;
            }
            let (allocation, host, frontier) = completed.into_parts();
            input = DirectionalPersistentSdmaAdmittedRequestV1 {
                allocation: allocation.retire_settled_frontier_v1(frontier).unwrap(),
                host,
                direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 0,
            };
        }
        panic!("nonempty copy ranges")
    }

    fn allocation(
        &mut self,
        ranges: &[(u64, u32)],
    ) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        let (allocation, host, frontier) = self.completed(ranges).into_parts();
        let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
        self.copy.promotion.base.release(host);
        self.copy.promotion.base.calls.clear();
        allocation
    }

    fn release(mut self, allocation: Gfx942DirectionalQueuePersistentAllocationV1) {
        let (buffer, debit) = demote_directional_persistent_sdma_custody_v1(
            allocation,
            self.copy.promotion.base.outstanding,
        )
        .unwrap();
        self.copy.promotion.base.outstanding = debit;
        self.copy.promotion.base.release(buffer);
        self.copy.promotion.base.shutdown();
    }
}

impl InitializedStorageContextV1 for StorageParent {
    fn owner(&self) -> QueueKeyV1 {
        self.copy.promotion.owner()
    }
    fn preflight(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.is_terminal() || self.root.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unavailable storage fixture",
            ));
        }
        let pair = self
            .copy
            .promotion
            .base
            .parent
            .sdma
            .as_ref()
            .and_then(Gfx942SdmaQueueSetV1::directional_observation)
            .and_then(|observation| admit_persistent_directional_sdma_pair_v1(observation).ok());
        admit_allocation(
            allocation,
            self.owner(),
            pair == Some(allocation.attachment.pair),
        )
    }
    fn root(&mut self) -> &mut Option<StorageRoot> {
        &mut self.root
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::loan(&mut self.copy.promotion.base)
    }
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.currentness_calls += 1;
        if let Some((at, fault)) = self.currentness_fault
            && at == self.currentness_calls
        {
            match fault {
                Fault::Error => {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "storage currentness",
                    ));
                }
                Fault::Panic => std::panic::panic_any("storage currentness"),
                _ => unreachable!(),
            }
        }
        self.copy
            .promotion
            .base
            .parent
            .engine
            .backend
            .session
            .check_queue_operational_currentness()
            .map_err(Into::into)
    }
    fn validate_mapping(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.copy
            .promotion
            .base
            .parent
            .engine
            .backend
            .session
            .primary_validate_sdma_demotion_mapping_v1(&self.root.as_ref().unwrap().allocation)?;
        match self.validation {
            Fault::Error => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "storage validation",
            )),
            Fault::Panic => std::panic::panic_any("storage validation"),
            _ => Ok(()),
        }
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::retake(&mut self.copy.promotion.base, loan)
    }
    fn is_terminal(&self) -> bool {
        self.copy.promotion.is_terminal()
    }
    fn poison(&mut self) {
        SdmaAllocationContextV1::poison(&mut self.copy.promotion.base);
    }
}

fn returned(
    failure: StorageFailure,
    terminal: bool,
) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    match failure.into_parts().1 {
        StorageCustody::Retryable(allocation) if !terminal => allocation,
        StorageCustody::ProcessTeardown(root) if terminal => root.allocation,
        _ => panic!("wrong storage conversion custody"),
    }
}

#[test]
fn initialized_storage_conversion_accepts_full_and_contiguous_completed_copies_without_reallocation()
 {
    for ranges in [&[(0, 4096)][..], &[(0, 2048), (2048, 2048)][..]] {
        let mut f = StorageParent::new();
        let allocation = f.allocation(ranges);
        let attachment = allocation.attachment;
        let owner = allocation.owner.ownership_snapshot_for_test_v1();
        let memory = f
            .copy
            .promotion
            .base
            .parent
            .engine
            .backend
            .session
            .insertion_memory_snapshot_v1();
        let ready = convert(&mut f, allocation).unwrap();
        assert_eq!(ready.allocation.attachment, attachment);
        assert_eq!(
            ready.allocation.owner.ownership_snapshot_for_test_v1(),
            owner
        );
        assert_eq!(f.currentness_calls, 2);
        assert_eq!(f.copy.promotion.base.calls, ["loan", "retake"]);
        assert_eq!(f.copy.promotion.base.outstanding, 1);
        assert_eq!(
            f.copy
                .promotion
                .base
                .parent
                .engine
                .backend
                .session
                .insertion_memory_snapshot_v1(),
            memory
        );
        let input = Gfx942PersistentComputeInputV1::InitializedStorage(ready);
        assert!(input.is_fully_initialized());
        assert!(format!("{input:?}").contains("initialized: true"));
        let (allocation, origin) = input.into_parts();
        assert_eq!(
            origin,
            crate::persistent_compute::PersistentComputeInitializationV1::InitializedStorage
        );
        f.release(allocation);
    }
}

#[test]
fn initialized_storage_conversion_rejects_completed_copy_from_unknown_source() {
    let mut f = StorageParent::new();
    let (allocation, host, frontier) = f.completed_with_source(&[(0, 4096)], false).into_parts();
    let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
    f.copy.promotion.base.calls.clear();
    let allocation = returned(convert(&mut f, allocation).unwrap_err(), false);
    assert_eq!(f.currentness_calls, 0);
    assert!(f.copy.promotion.base.calls.is_empty());
    f.copy.promotion.base.release(host);
    f.release(allocation);
}

#[test]
fn initialized_storage_conversion_rejects_partial_or_gapped_coverage_without_native_effects() {
    for ranges in [
        &[(0, 2048)][..],
        &[(2048, 2048)][..],
        &[(0, 1024), (2048, 2048)][..],
    ] {
        let mut f = StorageParent::new();
        let allocation = f.allocation(ranges);
        let before = allocation.owner.ownership_snapshot_for_test_v1();
        let allocation = returned(convert(&mut f, allocation).unwrap_err(), false);
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
        assert_eq!(f.currentness_calls, 0);
        assert!(f.copy.promotion.base.calls.is_empty());
        f.release(allocation);
    }
}

#[test]
fn initialized_storage_conversion_opening_failure_preserves_original_owner() {
    for fault in [Fault::Error, Fault::Panic] {
        let mut f = StorageParent::new();
        let allocation = f.allocation(&[(0, 4096)]);
        let owner = allocation.owner.ownership_snapshot_for_test_v1();
        f.copy.promotion.base.opening = fault;
        f.copy.promotion.base.poison_panics = true;
        let result = catch_unwind(AssertUnwindSafe(|| convert(&mut f, allocation)));
        let allocation = match result {
            Err(payload) => {
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"allocation loan"));
                f.root.take().unwrap().allocation
            }
            Ok(Err(failure)) => returned(failure, false),
            Ok(Ok(_)) => panic!("failed loan admitted"),
        };
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), owner);
        assert_eq!(f.is_terminal(), fault == Fault::Panic);
        assert_eq!(f.currentness_calls, 0);
        assert_eq!(f.copy.promotion.base.calls, ["loan"]);
    }
}

#[test]
fn initialized_storage_conversion_public_adapter_preserves_retry_and_existing_root() {
    let mut f = StorageParent::new();
    let allocation = f.allocation(&[(0, 4096)]);
    let before = allocation.owner.ownership_snapshot_for_test_v1();
    let mut session = crate::queue::live::tests::persistent_compute_cancellation_test_session(
        f.owner(),
        None,
        None,
    );
    session.sdma = f.copy.promotion.base.parent.sdma.take();
    let failure = session
        .promote_initialized_persistent_allocation_for_compute_v1(allocation)
        .unwrap_err();
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract("missing queue engine")
    ));
    let allocation = returned(failure, false);
    assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
    assert!(session.initialized_storage_promotion.is_none());
    f.copy.promotion.base.parent.sdma = session.sdma.take();
    let retry = f.allocation(&[(0, 4096)]);
    let retry_before = retry.owner.ownership_snapshot_for_test_v1();
    session.initialized_storage_promotion = Some(StorageRoot { allocation });
    let failure = session
        .promote_initialized_persistent_allocation_for_compute_v1(retry)
        .unwrap_err();
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract("unfinished initialized storage promotion")
    ));
    let retry = returned(failure, false);
    assert_eq!(retry.owner.ownership_snapshot_for_test_v1(), retry_before);
    let allocation = session
        .initialized_storage_promotion
        .take()
        .unwrap()
        .allocation;
    assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
    let (buffer, debit) =
        demote_directional_persistent_sdma_custody_v1(retry, f.copy.promotion.base.outstanding)
            .unwrap();
    f.copy.promotion.base.outstanding = debit;
    f.copy.promotion.base.release(buffer);
    f.release(allocation);
}

#[test]
fn initialized_storage_conversion_public_guards_preserve_root_and_drop_aborts() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_INITIALIZED_STORAGE_DROP";
    const TEST: &str = "queue::live::construction_primary::integration_tests::release_cases::sdma_allocation_cases::promotion::synchronous::initialized_storage::initialized_storage_conversion_public_guards_preserve_root_and_drop_aborts";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("storage guards checked; dropping retained root"));
        assert!(!stderr.contains("panicked at"));
        return;
    }
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    let mut f = StorageParent::new();
    let allocation = f.allocation(&[(0, 4096)]);
    let before = allocation.owner.ownership_snapshot_for_test_v1();
    let mut session = core::mem::ManuallyDrop::new(
        crate::queue::live::tests::persistent_compute_cancellation_test_session(
            f.owner(),
            None,
            None,
        ),
    );
    session.initialized_storage_promotion = Some(StorageRoot { allocation });
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
                "unfinished initialized storage promotion"
            ))
        ));
    }
    assert_eq!(
        session
            .initialized_storage_promotion
            .as_ref()
            .unwrap()
            .allocation
            .owner
            .ownership_snapshot_for_test_v1(),
        before
    );
    assert!(!session.sdma_device_pool.activity_started);
    assert_eq!(session.sdma_outstanding_buffers, 0);
    eprintln!("storage guards checked; dropping retained root");
    drop(core::mem::ManuallyDrop::into_inner(session));
    panic!("unfinished storage conversion Drop returned");
}

#[test]
fn initialized_storage_conversion_requires_retirement_even_after_full_completion() {
    let mut f = StorageParent::new();
    let (allocation, host, frontier) = f.completed(&[(0, 4096)]).into_parts();
    f.copy.promotion.base.calls.clear();
    let allocation = returned(convert(&mut f, allocation).unwrap_err(), false);
    assert_eq!(allocation.owner.retained_settled_use_count(), 1);
    assert_eq!(f.currentness_calls, 0);
    assert!(f.copy.promotion.base.calls.is_empty());
    let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
    let ready = convert(&mut f, allocation).unwrap();
    f.copy.promotion.base.release(host);
    f.release(ready.into_allocation());
}

#[test]
fn initialized_storage_conversion_rejects_changed_scope_without_losing_owner() {
    for case in 0..6 {
        let mut f = StorageParent::new();
        let mut allocation = f.allocation(&[(0, 4096)]);
        let attachment = allocation.attachment;
        match case {
            0 => allocation.attachment.pool_generation += 1,
            1 => allocation.attachment.logical_bytes -= 1,
            2 => allocation.attachment.physical_bytes += 4096,
            3 => allocation.attachment.queue.generation.0 += 1,
            4 => allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss(),
            5 => {
                let lease = allocation.owner.detach_local_native_for_sdma().unwrap();
                allocation
                    .owner
                    .restore_local_native_from_sdma(lease)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = allocation.owner.ownership_snapshot_for_test_v1();
        let failure = convert(&mut f, allocation).unwrap_err();
        let mut allocation = if case == 3 {
            let StorageCustody::ForeignQueue(allocation) = failure.into_parts().1 else {
                panic!("foreign")
            };
            allocation
        } else {
            returned(failure, false)
        };
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
        assert_eq!(f.currentness_calls, 0);
        assert!(f.copy.promotion.base.calls.is_empty());
        if case != 4 {
            allocation.attachment = attachment;
            f.release(allocation);
        }
    }
}

#[test]
fn initialized_storage_conversion_rejects_reserved_and_prepared_uses() {
    for prepared in [false, true] {
        let mut f = StorageParent::new();
        let mut allocation = f.allocation(&[(0, 4096)]);
        let request =
            Gfx942PersistentUseRequestV1::new(Gfx942PersistentOperationV1::ComputeRead, 0, 4096)
                .unwrap();
        let reserved = allocation.owner.reserve(request, None).unwrap();
        if prepared {
            let usage = allocation.owner.prepare(reserved).unwrap();
            allocation = returned(convert(&mut f, allocation).unwrap_err(), false);
            allocation.owner.cancel_prepared(usage).unwrap();
        } else {
            allocation = returned(convert(&mut f, allocation).unwrap_err(), false);
            allocation.owner.cancel_reserved(reserved).unwrap();
        }
        assert_eq!(f.currentness_calls, 0);
        assert!(f.copy.promotion.base.calls.is_empty());
        let ready = convert(&mut f, allocation).unwrap();
        f.release(ready.into_allocation());
    }
}

#[test]
fn initialized_storage_conversion_currentness_and_retake_failures_retain_exact_custody() {
    for validation in [Fault::None, Fault::Error, Fault::Panic] {
        for closing in [
            Fault::None,
            Fault::Error,
            Fault::Panic,
            Fault::AfterError,
            Fault::AfterPanic,
            Fault::Regression,
        ] {
            for poison_panics in [false, true] {
                let mut f = StorageParent::new();
                let allocation = f.allocation(&[(0, 4096)]);
                let attachment = allocation.attachment;
                let owner = allocation.owner.ownership_snapshot_for_test_v1();
                f.validation = validation;
                f.copy.promotion.base.closing = closing;
                f.copy.promotion.base.poison_panics = poison_panics;
                let result = catch_unwind(AssertUnwindSafe(|| convert(&mut f, allocation)));
                let terminal = validation == Fault::Panic || closing != Fault::None;
                assert_eq!(f.is_terminal(), terminal);
                let allocation = match result {
                    Err(payload) => {
                        let expected = if validation == Fault::Panic {
                            "storage validation"
                        } else if closing == Fault::Panic {
                            "allocation retake"
                        } else {
                            "allocation retake after"
                        };
                        assert_eq!(payload.downcast_ref::<&str>(), Some(&expected));
                        f.root.take().unwrap().allocation
                    }
                    Ok(Err(failure)) => returned(failure, terminal),
                    Ok(Ok(ready)) => ready.into_allocation(),
                };
                assert_eq!(allocation.attachment, attachment);
                assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), owner);
                assert_eq!(f.copy.promotion.base.outstanding, 1);
                assert_eq!(f.copy.promotion.base.calls, ["loan", "retake"]);
            }
        }
    }
    for at in [1, 2] {
        for fault in [Fault::Error, Fault::Panic] {
            let mut f = StorageParent::new();
            let allocation = f.allocation(&[(0, 4096)]);
            let owner = allocation.owner.ownership_snapshot_for_test_v1();
            f.currentness_fault = Some((at, fault));
            f.copy.promotion.base.poison_panics = true;
            let result = catch_unwind(AssertUnwindSafe(|| convert(&mut f, allocation)));
            let allocation = match result {
                Err(payload) => {
                    assert_eq!(payload.downcast_ref::<&str>(), Some(&"storage currentness"));
                    f.root.take().unwrap().allocation
                }
                Ok(Err(failure)) => returned(failure, true),
                Ok(Ok(_)) => panic!("currentness failure admitted"),
            };
            assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), owner);
            assert!(f.is_terminal());
            assert_eq!(f.currentness_calls, at);
            assert_eq!(f.copy.promotion.base.calls, ["loan", "retake"]);
        }
    }
}
