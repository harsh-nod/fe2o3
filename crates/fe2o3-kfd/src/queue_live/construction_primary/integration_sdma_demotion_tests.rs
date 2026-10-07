//! Demotion borrows genuine promoted mappings through the shared production driver.

use super::*;
use crate::persistent_directional_sdma::{
    Gfx942DirectionalPersistentSdmaDemotionCustodyV1 as DemotionCustody,
    Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 as DemotionRoot,
};
use crate::queue::live::sdma_demotion::{SdmaDemotionContextV1, admit_allocation, demote_in_place};

struct DemotionParent {
    promotion: PromotionParent,
    root: Option<DemotionRoot>,
    validation: Fault,
    validation_calls: std::cell::Cell<usize>,
}

impl DemotionParent {
    fn new() -> Self {
        Self {
            promotion: PromotionParent::new(),
            root: None,
            validation: Fault::None,
            validation_calls: std::cell::Cell::new(0),
        }
    }
    fn allocation(
        &mut self,
        physical: usize,
        logical: u64,
    ) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        let mut buffer = self.promotion.buffer(physical);
        buffer.set_logical_bytes(logical);
        let allocation = promote_in_place(&mut self.promotion, buffer)
            .unwrap_or_else(|failure| panic!("{:?}", failure.error()));
        self.promotion.base.calls.clear();
        allocation
    }
}

impl SdmaDemotionContextV1 for DemotionParent {
    fn owner(&self) -> QueueKeyV1 {
        self.promotion.owner()
    }
    fn preflight(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.root.is_some() || self.is_terminal() || self.promotion.base.parent.sdma.is_none() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unavailable demotion fixture",
            ));
        }
        let pair = self
            .promotion
            .base
            .parent
            .sdma
            .as_ref()
            .and_then(Gfx942SdmaQueueSetV1::directional_observation)
            .and_then(|observation| admit_persistent_directional_sdma_pair_v1(observation).ok());
        admit_allocation(allocation, pair == Some(allocation.attachment.pair))
    }
    fn roots(&mut self) -> (&mut Option<DemotionRoot>, &mut usize) {
        (&mut self.root, &mut self.promotion.base.outstanding)
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::loan(&mut self.promotion.base)
    }
    fn validate(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.validation_calls.set(self.validation_calls.get() + 1);
        self.promotion
            .base
            .parent
            .engine
            .backend
            .session
            .primary_validate_sdma_demotion_mapping_v1(&self.root.as_ref().unwrap().allocation)?;
        match self.validation {
            Fault::Error => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "demotion validation",
            )),
            Fault::Panic => std::panic::panic_any("demotion validation"),
            _ => Ok(()),
        }
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        SdmaAllocationContextV1::retake(&mut self.promotion.base, loan)
    }
    fn is_terminal(&self) -> bool {
        self.promotion.is_terminal()
    }
    fn poison(&mut self) {
        SdmaAllocationContextV1::poison(&mut self.promotion.base);
    }
}

fn recovered(
    failure: Gfx942DirectionalPersistentSdmaDemotionFailureV1,
    terminal: bool,
) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    match failure.into_parts().1 {
        DemotionCustody::Retryable(allocation) if !terminal => allocation,
        DemotionCustody::ProcessTeardown(root) if terminal => root.allocation,
        _ => panic!("wrong demotion failure classification"),
    }
}

#[test]
fn constructed_sdma_demotion_success_and_healthy_retries_preserve_mapping_then_refund() {
    for (physical, logical) in [(17, 17), (4097, 4097), (8192, 17)] {
        for reject in 0..3 {
            let mut f = DemotionParent::new();
            let mut allocation = f.allocation(physical, logical);
            let before = allocation.attachment;
            let e = &f.promotion.base.parent.engine;
            let memory = e.backend.session.insertion_memory_snapshot_v1();
            let resources = original_resource_ids(&f.promotion.base.parent);
            if reject != 0 {
                if reject == 1 {
                    f.promotion.base.opening = Fault::Error;
                } else {
                    f.validation = Fault::Error;
                }
                allocation = recovered(demote_in_place(&mut f, allocation).unwrap_err(), false);
                assert_eq!(allocation.attachment, before);
                assert!(!f.is_terminal() && f.root.is_none());
                assert_eq!(f.promotion.base.outstanding, 1);
                f.promotion.base.opening = Fault::None;
                f.validation = Fault::None;
            }
            let buffer =
                demote_in_place(&mut f, allocation).unwrap_or_else(|e| panic!("{:?}", e.error()));
            assert_eq!(
                buffer_observation(&buffer),
                (
                    before.storage_identity,
                    before.pool_generation + 1,
                    before.logical_bytes,
                    before.physical_bytes
                )
            );
            assert!(f.root.is_none() && !f.is_terminal());
            assert_eq!(f.promotion.base.outstanding, 1);
            let e = &f.promotion.base.parent.engine;
            assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
            e.backend
                .session
                .primary_authenticate(&e.foundation)
                .unwrap();
            assert_eq!(original_resource_ids(&f.promotion.base.parent), resources);
            f.promotion.base.release(buffer);
            f.promotion.base.shutdown();
        }
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn constructed_sdma_demotion_validation_retake_matrix_retains_original_allocation() {
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
                let mut f = DemotionParent::new();
                let allocation = f.allocation(17, 17);
                let retry = f.allocation(4097, 17);
                let before = allocation.attachment;
                let owner_before = allocation.owner.ownership_snapshot_for_test_v1();
                let retry_before = retry.attachment;
                let e = &f.promotion.base.parent.engine;
                let memory = e.backend.session.insertion_memory_snapshot_v1();
                let host = e.backend.session.coherent_insertion_snapshot_v1();
                let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
                let resources = original_resource_ids(&f.promotion.base.parent);
                let signals = Memory::primary_token_identity(
                    f.promotion.base.parent.signals.as_ref().unwrap(),
                );
                f.validation = validation;
                f.promotion.base.closing = closing;
                f.promotion.base.poison_panics = poison_panics;
                let result = catch_unwind(AssertUnwindSafe(|| demote_in_place(&mut f, allocation)));
                let terminal = validation == Fault::Panic || closing != Fault::None;
                let panics = validation == Fault::Panic
                    || matches!(closing, Fault::Panic | Fault::AfterPanic);
                assert_eq!(f.is_terminal(), terminal);
                assert_eq!(f.promotion.base.outstanding, 2);
                assert_eq!(f.promotion.base.calls, ["loan", "retake"]);
                assert_eq!(f.validation_calls.get(), 1);
                if panics {
                    let expected = if validation == Fault::Panic {
                        "demotion validation"
                    } else if closing == Fault::Panic {
                        "allocation retake"
                    } else {
                        "allocation retake after"
                    };
                    assert_eq!(
                        result.err().unwrap().downcast_ref::<&str>(),
                        Some(&expected)
                    );
                    assert_eq!(f.root.as_ref().unwrap().allocation.attachment, before);
                    assert_eq!(
                        f.root
                            .as_ref()
                            .unwrap()
                            .allocation
                            .owner
                            .ownership_snapshot_for_test_v1(),
                        owner_before
                    );
                    assert_eq!(
                        recovered(demote_in_place(&mut f, retry).unwrap_err(), true).attachment,
                        retry_before
                    );
                    assert_eq!(f.root.as_ref().unwrap().allocation.attachment, before);
                    assert_eq!(f.promotion.base.calls, ["loan", "retake"]);
                    assert_eq!(f.validation_calls.get(), 1);
                } else if validation == Fault::None && closing == Fault::None {
                    let buffer = result
                        .unwrap()
                        .unwrap_or_else(|e| panic!("{:?}", e.error()));
                    assert_eq!(
                        buffer_observation(&buffer),
                        (
                            before.storage_identity,
                            before.pool_generation + 1,
                            before.logical_bytes,
                            before.physical_bytes
                        )
                    );
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
                            ComputeAqlQueueSessionErrorV1::Contract("demotion validation")
                        )),
                        _ => unreachable!(),
                    }
                    let allocation = recovered(failure, terminal);
                    assert_eq!(allocation.attachment, before);
                    assert_eq!(
                        allocation.owner.ownership_snapshot_for_test_v1(),
                        owner_before
                    );
                    assert!(f.root.is_none());
                }
                let e = &f.promotion.base.parent.engine;
                assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
                assert!(e.backend.session.coherent_insertion_snapshot_v1() == host);
                assert_eq!(original_resource_ids(&f.promotion.base.parent), resources);
                assert_eq!(
                    Memory::primary_token_identity(
                        f.promotion.base.parent.signals.as_ref().unwrap()
                    ),
                    signals
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
fn constructed_sdma_demotion_opening_failure_never_validates_or_loses_custody() {
    for fault in [Fault::Error, Fault::Panic] {
        let mut f = DemotionParent::new();
        let allocation = f.allocation(4097, 4097);
        let before = allocation.attachment;
        let owner_before = allocation.owner.ownership_snapshot_for_test_v1();
        let e = &f.promotion.base.parent.engine;
        let memory = e.backend.session.insertion_memory_snapshot_v1();
        let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
        f.promotion.base.opening = fault;
        f.promotion.base.poison_panics = true;
        let result = catch_unwind(AssertUnwindSafe(|| demote_in_place(&mut f, allocation)));
        if fault == Fault::Error {
            let allocation = recovered(result.unwrap().unwrap_err(), false);
            assert_eq!(allocation.attachment, before);
            assert_eq!(
                allocation.owner.ownership_snapshot_for_test_v1(),
                owner_before
            );
            assert!(f.root.is_none() && !f.is_terminal());
        } else {
            assert_eq!(
                result.err().unwrap().downcast_ref::<&str>(),
                Some(&"allocation loan")
            );
            assert_eq!(f.root.as_ref().unwrap().allocation.attachment, before);
            assert_eq!(
                f.root
                    .as_ref()
                    .unwrap()
                    .allocation
                    .owner
                    .ownership_snapshot_for_test_v1(),
                owner_before
            );
            assert!(f.is_terminal());
        }
        assert_eq!(f.promotion.base.calls, ["loan"]);
        assert_eq!(f.validation_calls.get(), 0);
        assert_eq!(f.promotion.base.outstanding, 1);
        let e = &f.promotion.base.parent.engine;
        assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
        assert_eq!(e.backend.session.primary_loan_state_v1(&e.foundation), loan);
    }
}

#[test]
fn constructed_sdma_demotion_rejects_active_uses_without_invalidating_their_leases() {
    for prepared in [false, true] {
        let mut f = DemotionParent::new();
        let mut allocation = f.allocation(4097, 4097);
        let before = allocation.attachment;
        let request = Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::LocalSdmaDestination,
            0,
            17,
        )
        .unwrap();
        let reserved = allocation.owner.reserve(request, None).unwrap();
        if prepared {
            let lease = allocation.owner.prepare(reserved).unwrap();
            allocation = recovered(demote_in_place(&mut f, allocation).unwrap_err(), false);
            assert_eq!(allocation.owner.live_use_count(), 1);
            allocation.owner.cancel_prepared(lease).unwrap();
        } else {
            allocation = recovered(demote_in_place(&mut f, allocation).unwrap_err(), false);
            assert_eq!(allocation.owner.live_use_count(), 1);
            allocation.owner.cancel_reserved(reserved).unwrap();
        }
        assert_eq!(allocation.attachment, before);
        assert_eq!(f.promotion.base.calls, ["loan", "retake"]);
        let buffer =
            demote_in_place(&mut f, allocation).unwrap_or_else(|e| panic!("{:?}", e.error()));
        assert_eq!(buffer.pool_generation(), before.pool_generation + 1);
        f.promotion.base.release(buffer);
        f.promotion.base.shutdown();
    }
}

#[test]
fn constructed_sdma_demotion_admission_and_quarantine_preserve_generation_and_debit() {
    for case in 0..6 {
        let mut f = DemotionParent::new();
        let mut allocation = f.allocation(17, 17);
        match case {
            0 => allocation.attachment.pool_generation = u64::MAX,
            1 => allocation.attachment.queue = crate::queue::live::tests::test_queue_key(91, 1),
            2 => allocation.attachment.pair.host_to_device_queue_id += 1,
            3 => f.promotion.base.parent.poisoned = true,
            4 => allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss(),
            5 => f.promotion.base.outstanding = 0,
            _ => unreachable!(),
        }
        let before = allocation.attachment;
        let quarantine = allocation.owner.quarantine_reason();
        let allocation = recovered(demote_in_place(&mut f, allocation).unwrap_err(), case == 3);
        assert_eq!(allocation.attachment, before);
        assert_eq!(allocation.owner.quarantine_reason(), quarantine);
        assert_eq!(f.promotion.base.outstanding, usize::from(case != 5));
        assert_eq!(f.validation_calls.get(), usize::from(case >= 4));
        assert!(f.root.is_none());
        assert_eq!(f.is_terminal(), case == 3);
    }
}

#[test]
fn constructed_sdma_demotion_public_guards_preserve_root_and_drop_aborts() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SDMA_DEMOTION_DROP";
    const TEST: &str = "queue::live::construction_primary::integration_tests::release_cases::sdma_allocation_cases::promotion::demotion::constructed_sdma_demotion_public_guards_preserve_root_and_drop_aborts";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("demotion guards checked; dropping retained root"));
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
    let mut f = DemotionParent::new();
    let allocation = f.allocation(17, 17);
    let retry = f.allocation(4097, 4097);
    let before = allocation.attachment;
    let retry_before = retry.attachment;
    let mut session = core::mem::ManuallyDrop::new(
        crate::queue::live::tests::persistent_compute_cancellation_test_session(
            f.owner(),
            None,
            None,
        ),
    );
    session.sdma_demotion = Some(DemotionRoot { allocation });
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
                "unfinished SDMA demotion"
            ))
        ));
    }
    assert!(!session.sdma_device_pool.activity_started);
    assert_eq!(session.sdma_outstanding_buffers, 0);
    assert_eq!(session.sdma_pool_reuse_count, 0);
    session.terminal_poisoned = true;
    let failure = session
        .demote_directional_persistent_allocation_to_sdma_device_buffer_v1(retry)
        .unwrap_err();
    assert_eq!(recovered(failure, true).attachment, retry_before);
    assert_eq!(
        session
            .sdma_demotion
            .as_ref()
            .unwrap()
            .allocation
            .attachment,
        before
    );
    eprintln!("demotion guards checked; dropping retained root");
    drop(core::mem::ManuallyDrop::into_inner(session));
    panic!("unfinished demotion Drop returned");
}

#[test]
fn constructed_sdma_demotion_foreign_mapping_rejected_by_real_engine_after_retake() {
    let mut owner = DemotionParent::new();
    let owner_trace = trace();
    let mut allocation = owner.allocation(4097, 17);
    let original = allocation.attachment;
    let owner_before = allocation.owner.ownership_snapshot_for_test_v1();
    let owner_memory = owner
        .promotion
        .base
        .parent
        .engine
        .backend
        .session
        .insertion_memory_snapshot_v1();
    let mut receiver = DemotionParent::new();
    let receiver_trace = trace();
    allocation.attachment.queue = receiver.owner();
    allocation.attachment.pair = admit_persistent_directional_sdma_pair_v1(
        receiver
            .promotion
            .base
            .parent
            .sdma
            .as_ref()
            .unwrap()
            .directional_observation()
            .unwrap(),
    )
    .unwrap();
    let routed = allocation.attachment;
    let e = &receiver.promotion.base.parent.engine;
    let memory = e.backend.session.insertion_memory_snapshot_v1();
    let loan = e.backend.session.primary_loan_state_v1(&e.foundation);
    let failure = demote_in_place(&mut receiver, allocation).unwrap_err();
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    let mut allocation = recovered(failure, false);
    assert_eq!(allocation.attachment, routed);
    assert_eq!(
        allocation.owner.ownership_snapshot_for_test_v1(),
        owner_before
    );
    assert_eq!(receiver.promotion.base.calls, ["loan", "retake"]);
    assert_eq!(receiver.validation_calls.get(), 1);
    assert!(!receiver.is_terminal() && receiver.root.is_none());
    assert_eq!(receiver.promotion.base.outstanding, 0);
    let e = &receiver.promotion.base.parent.engine;
    assert_eq!(e.backend.session.insertion_memory_snapshot_v1(), memory);
    assert_eq!(
        e.backend.session.primary_loan_state_v1(&e.foundation),
        (loan.0, None, loan.2 + 1)
    );
    assert_eq!(
        owner
            .promotion
            .base
            .parent
            .engine
            .backend
            .session
            .insertion_memory_snapshot_v1(),
        owner_memory
    );
    assert_eq!(owner.promotion.base.outstanding, 1);
    allocation.attachment = original;
    ACTIVE.with(|active| *active.borrow_mut() = Some(owner_trace));
    let buffer =
        demote_in_place(&mut owner, allocation).unwrap_or_else(|e| panic!("{:?}", e.error()));
    owner.promotion.base.release(buffer);
    owner.promotion.base.shutdown();
    ACTIVE.with(|active| *active.borrow_mut() = Some(receiver_trace));
    receiver.promotion.base.shutdown();
}

#[test]
fn constructed_sdma_demotion_public_foreign_disabled_and_terminal_ingress_is_inert() {
    for terminal in [false, true] {
        for foreign in [false, true] {
            let mut f = DemotionParent::new();
            let allocation = f.allocation(4097, 17);
            let before = allocation.attachment;
            let owner_before = allocation.owner.ownership_snapshot_for_test_v1();
            let key = if foreign {
                crate::queue::live::tests::test_queue_key(91, 1)
            } else {
                f.owner()
            };
            let mut session = core::mem::ManuallyDrop::new(
                crate::queue::live::tests::persistent_compute_cancellation_test_session(
                    key, None, None,
                ),
            );
            session.terminal_poisoned = terminal;
            let failure = session
                .demote_directional_persistent_allocation_to_sdma_device_buffer_v1(allocation)
                .unwrap_err();
            let expected = if foreign {
                "foreign directional persistent SDMA allocation owner"
            } else if terminal {
                "terminal queue session requires process teardown"
            } else {
                "SDMA copy engine is not enabled"
            };
            assert!(
                matches!(failure.error(), ComputeAqlQueueSessionErrorV1::Contract(message) if *message == expected)
            );
            let allocation = recovered(failure, terminal && !foreign);
            assert_eq!(allocation.attachment, before);
            assert_eq!(
                allocation.owner.ownership_snapshot_for_test_v1(),
                owner_before
            );
            assert_eq!(session.terminal_poisoned, terminal);
            assert!(session.sdma_demotion.is_none() && session.engine.is_none());
            assert!(!session.sdma_device_pool.activity_started);
            assert_eq!(session.sdma_outstanding_buffers, 0);
            assert_eq!(session.sdma_pool_reuse_count, 0);
            let buffer =
                demote_in_place(&mut f, allocation).unwrap_or_else(|e| panic!("{:?}", e.error()));
            f.promotion.base.release(buffer);
            f.promotion.base.shutdown();
        }
    }
}
