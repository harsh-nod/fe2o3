//! Promotion uses original constructed queues, fresh mapped leases and real model loans.

use super::*;
#[path = "integration_sdma_demotion_tests.rs"]
mod demotion;
#[path = "integration_sdma_synchronous_tests.rs"]
mod synchronous;
use crate::persistent_directional_sdma::{
    Gfx942DirectionalPersistentSdmaPromotionCustodyV1 as Custody,
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 as Root,
    Gfx942PersistentDirectionalSdmaPairV1, demote_directional_persistent_sdma_custody_v1,
};
use crate::queue::live::sdma_promotion::{SdmaPromotionContextV1, admit_buffer, promote_in_place};

struct PromotionParent {
    base: AllocationParent,
    root: Option<Root>,
    validation: Fault,
    validation_calls: std::cell::Cell<usize>,
}

impl PromotionParent {
    fn new() -> Self {
        Self {
            base: AllocationParent::new(),
            root: None,
            validation: Fault::None,
            validation_calls: std::cell::Cell::new(0),
        }
    }
    fn buffer(&mut self, bytes: usize) -> Gfx942SdmaBufferV1 {
        let buffer = self.base.allocate(false, bytes).unwrap();
        self.base.calls.clear();
        buffer
    }
}

impl SdmaPromotionContextV1 for PromotionParent {
    fn owner(&self) -> QueueKeyV1 {
        self.base.parent.key
    }
    fn preflight(
        &self,
        buffer: &Gfx942SdmaBufferV1,
    ) -> Result<Gfx942PersistentDirectionalSdmaPairV1, ComputeAqlQueueSessionErrorV1> {
        if self.root.is_some() || self.base.parent.poisoned {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA promotion",
            ));
        }
        admit_buffer(
            buffer,
            self.base
                .parent
                .sdma
                .as_ref()
                .and_then(Gfx942SdmaQueueSetV1::directional_observation),
        )
    }
    fn roots(&mut self) -> (&mut Option<Root>, &mut usize) {
        (&mut self.root, &mut self.base.outstanding)
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::loan(&mut self.base)
    }
    fn validate(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.validation_calls.set(self.validation_calls.get() + 1);
        self.base
            .parent
            .engine
            .backend
            .session
            .primary_validate_sdma_device_mapping_v1(&self.root.as_ref().unwrap().buffer)?;
        match self.validation {
            Fault::Error => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "promotion validation",
            )),
            Fault::Panic => std::panic::panic_any("promotion validation"),
            _ => Ok(()),
        }
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::retake(&mut self.base, loan)
    }
    fn is_terminal(&self) -> bool {
        self.base.parent.poisoned
    }
    fn poison(&mut self) {
        SdmaAllocationContextV1::poison(&mut self.base);
    }
}

fn buffer_observation(
    buffer: &Gfx942SdmaBufferV1,
) -> (
    crate::sdma::Gfx942SdmaBufferStorageIdentityV1,
    u64,
    u64,
    u64,
) {
    (
        buffer.storage_identity(),
        buffer.pool_generation(),
        buffer.requested_bytes(),
        buffer.physical_bytes(),
    )
}

fn returned_buffer(
    failure: Gfx942DirectionalPersistentSdmaPromotionFailureV1,
    terminal: bool,
) -> Gfx942SdmaBufferV1 {
    match failure.into_parts().1 {
        Custody::Retryable(buffer) if !terminal => buffer,
        Custody::ProcessTeardown(root) if terminal => root.buffer,
        _ => panic!("wrong promotion failure class"),
    }
}

#[test]
fn constructed_sdma_promotion_success_preserves_exact_mapping_pair_and_debit() {
    for (physical_request, logical) in [(17, 17), (4097, 4097), (8192, 17)] {
        let mut f = PromotionParent::new();
        let mut buffer = f.buffer(physical_request);
        buffer.set_logical_bytes(logical);
        let before = buffer_observation(&buffer);
        let pair = f.preflight(&buffer).unwrap();
        let e = &f.base.parent.engine;
        let memory = e.backend.session.insertion_memory_snapshot_v1();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        let resources = original_resource_ids(&f.base.parent);
        let allocation = promote_in_place(&mut f, buffer)
            .unwrap_or_else(|failure| panic!("{:?}", failure.error()));
        assert!(f.root.is_none() && !f.is_terminal());
        assert_eq!(f.base.outstanding, 1);
        assert_eq!(f.base.calls, ["loan", "retake"]);
        assert_eq!(f.validation_calls.get(), 1);
        assert_eq!(allocation.attachment.storage_identity, before.0);
        assert_eq!(allocation.attachment.pool_generation, before.1);
        assert_eq!(allocation.attachment.logical_bytes, before.2);
        assert_eq!(allocation.attachment.physical_bytes, before.3);
        assert_eq!(allocation.attachment.queue, f.owner());
        assert_eq!(allocation.attachment.pair, pair);
        assert_eq!(allocation.owner.byte_len(), before.3);
        assert_eq!(allocation.owner.live_use_count(), 0);
        let e = &f.base.parent.engine;
        assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
        assert_eq!(
            e.backend.session.primary_loan_state_v1(&e.foundation),
            (loan.0, None, loan.2 + 1)
        );
        e.backend
            .session
            .primary_authenticate(&e.foundation)
            .unwrap();
        assert_eq!(original_resource_ids(&f.base.parent), resources);
        let (buffer, debit) =
            demote_directional_persistent_sdma_custody_v1(allocation, f.base.outstanding).unwrap();
        assert_eq!(debit, 1);
        assert_eq!(
            buffer_observation(&buffer),
            (before.0, before.1 + 1, before.2, before.3)
        );
        f.base
            .parent
            .engine
            .backend
            .session
            .primary_validate_sdma_device_mapping_v1(&buffer)
            .unwrap();
        f.base.release(buffer);
        f.base.shutdown();
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn constructed_sdma_promotion_validation_retake_matrix_retains_exact_owner() {
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
                let mut f = PromotionParent::new();
                let buffer = f.buffer(17);
                let retry = f.buffer(4097);
                let before = buffer_observation(&buffer);
                let retry_before = buffer_observation(&retry);
                let e = &f.base.parent.engine;
                let memory = e.backend.session.insertion_memory_snapshot_v1();
                let host = e.backend.session.coherent_insertion_snapshot_v1();
                let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
                let resources = original_resource_ids(&f.base.parent);
                let signals =
                    Memory::primary_token_identity(f.base.parent.signals.as_ref().unwrap());
                let sdma = f
                    .base
                    .parent
                    .sdma
                    .as_ref()
                    .unwrap()
                    .directional_observation();
                f.validation = validation;
                f.base.closing = closing;
                f.base.poison_panics = poison_panics;
                let result = catch_unwind(AssertUnwindSafe(|| promote_in_place(&mut f, buffer)));
                let terminal = validation == Fault::Panic || closing != Fault::None;
                let panics = validation == Fault::Panic
                    || matches!(closing, Fault::Panic | Fault::AfterPanic);
                assert_eq!(f.is_terminal(), terminal);
                assert_eq!(f.base.outstanding, 2);
                assert_eq!(f.validation_calls.get(), 1);
                assert_eq!(f.base.calls, ["loan", "retake"]);
                if panics {
                    let expected = if validation == Fault::Panic {
                        "promotion validation"
                    } else if closing == Fault::Panic {
                        "allocation retake"
                    } else {
                        "allocation retake after"
                    };
                    assert_eq!(
                        result.err().unwrap().downcast_ref::<&str>(),
                        Some(&expected)
                    );
                    assert_eq!(buffer_observation(&f.root.as_ref().unwrap().buffer), before);
                    let failed_retry = promote_in_place(&mut f, retry).unwrap_err();
                    assert_eq!(
                        buffer_observation(&returned_buffer(failed_retry, true)),
                        retry_before
                    );
                    assert_eq!(buffer_observation(&f.root.as_ref().unwrap().buffer), before);
                    assert_eq!(f.base.calls, ["loan", "retake"]);
                    assert_eq!(f.validation_calls.get(), 1);
                } else if validation == Fault::None && closing == Fault::None {
                    let allocation = result
                        .unwrap()
                        .unwrap_or_else(|failure| panic!("{:?}", failure.error()));
                    assert_eq!(allocation.attachment.storage_identity, before.0);
                    assert!(f.root.is_none());
                } else {
                    let failure = result.unwrap().unwrap_err();
                    match closing {
                        Fault::Error => assert!(matches!(
                            failure.error(),
                            ComputeAqlQueueSessionErrorV1::Contract("allocation retake")
                        )),
                        Fault::AfterError => assert!(matches!(
                            failure.error(),
                            ComputeAqlQueueSessionErrorV1::Contract("allocation retake after")
                        )),
                        Fault::Regression => assert!(matches!(
                            failure.error(),
                            ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                                "fixture live foundation reclaim"
                            ))
                        )),
                        Fault::None => assert!(matches!(
                            failure.error(),
                            ComputeAqlQueueSessionErrorV1::Contract("promotion validation")
                        )),
                        _ => unreachable!(),
                    }
                    assert_eq!(
                        buffer_observation(&returned_buffer(failure, terminal)),
                        before
                    );
                    assert!(f.root.is_none());
                }
                let e = &f.base.parent.engine;
                assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
                assert!(e.backend.session.coherent_insertion_snapshot_v1() == host);
                assert_eq!(original_resource_ids(&f.base.parent), resources);
                assert_eq!(
                    Memory::primary_token_identity(f.base.parent.signals.as_ref().unwrap()),
                    signals
                );
                assert_eq!(
                    f.base
                        .parent
                        .sdma
                        .as_ref()
                        .unwrap()
                        .directional_observation(),
                    sdma
                );
                let reclaimed =
                    matches!(closing, Fault::None | Fault::AfterError | Fault::AfterPanic);
                assert_eq!(e.backend.foundation_in_engine, reclaimed);
                assert_eq!(
                    e.backend.session.primary_loan_state_v1(&e.foundation),
                    (loan.0, (!reclaimed).then_some(loan.2), loan.2 + 1)
                );
            }
        }
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn constructed_sdma_promotion_loan_failure_retains_and_does_not_validate() {
    for opening in [Fault::Error, Fault::Panic] {
        let mut f = PromotionParent::new();
        let buffer = f.buffer(17);
        let before = buffer_observation(&buffer);
        let e = &f.base.parent.engine;
        let memory = e.backend.session.insertion_memory_snapshot_v1();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        f.base.opening = opening;
        f.base.poison_panics = true;
        let result = catch_unwind(AssertUnwindSafe(|| promote_in_place(&mut f, buffer)));
        if opening == Fault::Error {
            assert_eq!(
                buffer_observation(&returned_buffer(result.unwrap().unwrap_err(), false)),
                before
            );
            assert!(!f.is_terminal() && f.root.is_none());
        } else {
            assert_eq!(
                result.err().unwrap().downcast_ref::<&str>(),
                Some(&"allocation loan")
            );
            assert_eq!(buffer_observation(&f.root.as_ref().unwrap().buffer), before);
            assert!(f.is_terminal());
        }
        assert_eq!(f.base.calls, ["loan"]);
        assert_eq!(f.validation_calls.get(), 0);
        assert_eq!(f.base.outstanding, 1);
        let e = &f.base.parent.engine;
        assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
        assert_eq!(e.backend.session.primary_loan_state_v1(&e.foundation), loan);
    }
}

#[test]
fn constructed_sdma_promotion_retry_reuses_exact_returned_mapping_then_refunds() {
    for opening in [false, true] {
        let mut f = PromotionParent::new();
        let buffer = f.buffer(4097);
        let before = buffer_observation(&buffer);
        if opening {
            f.base.opening = Fault::Error;
        } else {
            f.validation = Fault::Error;
        }
        let failure = promote_in_place(&mut f, buffer).unwrap_err();
        let buffer = returned_buffer(failure, false);
        assert_eq!(buffer_observation(&buffer), before);
        assert!(!f.is_terminal() && f.root.is_none());
        f.base.opening = Fault::None;
        f.validation = Fault::None;
        let allocation = promote_in_place(&mut f, buffer)
            .unwrap_or_else(|failure| panic!("{:?}", failure.error()));
        assert_eq!(allocation.attachment.storage_identity, before.0);
        assert_eq!(f.base.outstanding, 1);
        let (buffer, debit) =
            demote_directional_persistent_sdma_custody_v1(allocation, f.base.outstanding).unwrap();
        assert_eq!(debit, 1);
        assert_eq!(
            buffer_observation(&buffer),
            (before.0, before.1 + 1, before.2, before.3)
        );
        f.base.release(buffer);
        f.base.shutdown();
    }
}

#[test]
fn constructed_sdma_promotion_rejects_real_foreign_mapping_after_retake() {
    let mut owner = PromotionParent::new();
    let owner_trace = trace();
    let buffer = owner.buffer(17);
    let mut f = PromotionParent::new();
    let receiver_trace = trace();
    let (storage, _, generation, logical) = buffer.into_bridge_parts();
    let buffer = Gfx942SdmaBufferV1::from_bridge_parts(storage, f.owner(), generation, logical);
    let before = buffer_observation(&buffer);
    let failure = promote_in_place(&mut f, buffer).unwrap_err();
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
            MemorySessionError::InvalidDeviceMemoryAuthority
        ))
    ));
    let buffer = returned_buffer(failure, false);
    assert_eq!(buffer_observation(&buffer), before);
    assert_eq!(f.base.calls, ["loan", "retake"]);
    assert!(!f.is_terminal() && f.root.is_none());
    assert_eq!(f.base.outstanding, 0);
    let (storage, _, generation, logical) = buffer.into_bridge_parts();
    ACTIVE.with(|active| *active.borrow_mut() = Some(owner_trace));
    owner.base.release(Gfx942SdmaBufferV1::from_bridge_parts(
        storage,
        owner.owner(),
        generation,
        logical,
    ));
    owner.base.shutdown();
    ACTIVE.with(|active| *active.borrow_mut() = Some(receiver_trace));
    f.base.shutdown();
}

#[test]
fn constructed_sdma_promotion_preflight_preserves_buffer_without_loan() {
    for case in 0..6 {
        let mut f = PromotionParent::new();
        let buffer = f.base.allocate(case == 0, 17).unwrap();
        f.base.calls.clear();
        let (storage, owner, generation, logical) = buffer.into_bridge_parts();
        let buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            if case == 1 {
                crate::queue::live::tests::test_queue_key(91, 1)
            } else {
                owner
            },
            if case == 2 { 0 } else { generation },
            if case == 3 { 8193 } else { logical },
        );
        let before = buffer_observation(&buffer);
        let sdma = if case == 4 {
            f.base.parent.sdma.take()
        } else {
            None
        };
        if case == 5 {
            f.base.parent.poisoned = true;
        }
        let failure = promote_in_place(&mut f, buffer).unwrap_err();
        assert_eq!(
            buffer_observation(&returned_buffer(failure, case == 5)),
            before
        );
        assert!(f.base.calls.is_empty() && f.root.is_none());
        assert_eq!(f.base.outstanding, 1);
        if let Some(sdma) = sdma {
            f.base.parent.sdma = Some(sdma);
        }
    }
}

#[test]
fn constructed_sdma_promotion_zero_debit_is_terminal_only_after_validation_and_retake() {
    let mut f = PromotionParent::new();
    let buffer = f.buffer(17);
    let before = buffer_observation(&buffer);
    f.base.outstanding = 0;
    f.base.poison_panics = true;
    let failure = promote_in_place(&mut f, buffer).unwrap_err();
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract(
            "directional persistent SDMA promotion custody mismatch"
        )
    ));
    assert_eq!(buffer_observation(&returned_buffer(failure, true)), before);
    assert_eq!(f.base.outstanding, 0);
    assert_eq!(f.base.calls, ["loan", "retake"]);
    assert_eq!(f.validation_calls.get(), 1);
    assert!(f.is_terminal() && f.root.is_none());
}

#[test]
fn constructed_sdma_promotion_public_guards_are_inert_and_drop_aborts() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SDMA_PROMOTION_DROP";
    const TEST: &str = "queue::live::construction_primary::integration_tests::release_cases::sdma_allocation_cases::promotion::constructed_sdma_promotion_public_guards_are_inert_and_drop_aborts";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("promotion guards checked; dropping retained root"));
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
    let mut f = PromotionParent::new();
    let buffer = f.buffer(17);
    let before = buffer_observation(&buffer);
    // An engine-less public root exercises guard ordering, not native teardown.
    let mut session = core::mem::ManuallyDrop::new(
        crate::queue::live::tests::persistent_compute_cancellation_test_session(
            f.owner(),
            None,
            None,
        ),
    );
    session.sdma_promotion = Some(Root { buffer });
    let ledger = (
        session.sdma_device_pool.activity_started,
        session.sdma_outstanding_buffers,
        session.sdma_pool_reuse_count,
    );
    for result in [
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
                "unfinished SDMA promotion"
            ))
        ));
    }
    assert_eq!(
        (
            session.sdma_device_pool.activity_started,
            session.sdma_outstanding_buffers,
            session.sdma_pool_reuse_count
        ),
        ledger
    );
    assert_eq!(
        buffer_observation(&session.sdma_promotion.as_ref().unwrap().buffer),
        before
    );
    eprintln!("promotion guards checked; dropping retained root");
    drop(core::mem::ManuallyDrop::into_inner(session));
    panic!("unfinished promotion Drop returned");
}
