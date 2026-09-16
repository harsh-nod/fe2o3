//! Public receipt recovery and terminal re-entry with fixture and constructed custody.

use super::super::tests::{
    prepared_persistent_compute_cancellation_fixture,
    prepared_three_binding_persistent_compute_cancellation_fixture_v1,
};
use super::*;
use crate::persistent_compute::PersistentComputeCancellationCustodyV1;
use crate::queue::dispatch_binding::control_release::RetainedControlSnapshotV1 as OwnerSnapshot;
use crate::queue::live::persistent_cancel::{PersistentCancelContextV1, PersistentCancelRootV1};

fn fixture(three: bool) -> ComputeAqlQueueSessionV1 {
    let queue = test_queue_key(291, 1);
    if three {
        prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue).0
    } else {
        prepared_persistent_compute_cancellation_fixture(queue, 9123, None, Some(7)).0
    }
}

#[derive(Clone, Copy)]
enum ExpectedError {
    Poisoned,
    Phase,
    Substituted,
}

fn terminal_error(three: bool) -> ExpectedError {
    if three {
        ExpectedError::Phase
    } else {
        ExpectedError::Poisoned
    }
}

fn cancel(
    session: &mut ComputeAqlQueueSessionV1,
    three: bool,
    binding: PersistentComputeBindingKeyV1,
    expected: ExpectedError,
) -> bool {
    let (error, recovered) = if three {
        let failure = session
            .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(
                Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
                    binding,
                    thread_affinity: PhantomData,
                },
            )
            .unwrap_err();
        (
            failure.error,
            failure.recovered.map(|receipt| receipt.binding),
        )
    } else {
        let failure = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(
                Gfx942PreparedPersistentComputeDispatchV1 {
                    binding,
                    thread_affinity: PhantomData,
                },
            )
            .unwrap_err();
        (
            failure.error,
            failure.recovered.map(|receipt| receipt.binding),
        )
    };
    if let Some(recovered) = recovered {
        assert_eq!(recovered, binding);
    }
    match expected {
        ExpectedError::Poisoned => assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        )),
        ExpectedError::Phase => assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        )),
        ExpectedError::Substituted => assert!(
            matches!(error, ComputeAqlQueueSessionErrorV1::Contract(name) if name == if three { "three-binding persistent cancellation returned substituted storage" } else { "persistent compute cancellation returned substituted storage" })
        ),
    }
    recovered.is_some()
}

fn entries_snapshot(
    attachment: &BoundedPersistentComputeAttachmentV1,
) -> impl std::fmt::Debug + PartialEq + use<> {
    attachment
        .entries
        .iter()
        .map(|entry| {
            (
                entry.storage_identity,
                entry
                    .allocation
                    .owner
                    .local_native_for_sdma()
                    .map(|lease| lease.storage_identity()),
                entry.allocation.owner.live_use_count(),
                entry.allocation.owner.quarantine_reason(),
                (
                    entry.authenticated_sha256,
                    entry.fully_initialized,
                    entry.effect,
                ),
                match &entry.state {
                    PersistentComputeUseStateV1::Prepared(lease) => {
                        Some(lease.cancellation_identity_for_test())
                    }
                    _ => None,
                },
            )
        })
        .collect::<Vec<_>>()
}

fn native_snapshot(
    native: &PersistentComputeCancellationCustodyV1,
) -> impl std::fmt::Debug + PartialEq + use<> {
    (
        (
            native.returned.as_ptr() as usize,
            native.returned.capacity(),
        ),
        native
            .returned
            .iter()
            .map(|data| (data.sdma_storage_identity(), data.is_fully_initialized()))
            .collect::<Vec<_>>(),
        native
            .mapped
            .iter()
            .map(|lease| lease.as_ref().map(|lease| lease.storage_identity()))
            .collect::<Vec<_>>(),
        (
            native.restored,
            native.cancelled,
            native.count,
            native.restore_started,
            native.original_attached,
        ),
        native.generation,
        native.initialized,
        native.cleanup.as_ref().map(OwnerSnapshot::root_v1),
        native.output.len(),
    )
}

fn snapshot(session: &ComputeAqlQueueSessionV1) -> impl std::fmt::Debug + PartialEq + use<> {
    let attachment = session.persistent_compute.as_ref().unwrap();
    let native = match attachment.terminal_custody.as_ref().unwrap() {
        PersistentComputeTerminalNativeCustodyV1::Cancellation(native) => {
            Some(native_snapshot(native))
        }
        _ => None,
    };
    (
        attachment.binding,
        attachment.predecessor_dispatch_generation,
        entries_snapshot(attachment),
        session.persistent_compute_terminal_stage_v1(),
        native,
    )
}

pub(in crate::queue::live) fn assert_constructed_terminal_reentry_v1(
    root: PersistentCancelRootV1,
    dispatch: Option<DispatchResourceOwnerV1>,
) {
    assert!(
        root.native.cleanup.is_some(),
        "genuine lower cleanup retained"
    );
    assert!(root.native.output.is_empty());
    let attachment = root.attachment.as_ref().unwrap();
    let binding = attachment.binding;
    let three = attachment.entries.len() == 3;
    let expected_entries = entries_snapshot(attachment);
    let expected_native = native_snapshot(&root.native);
    let expected_dispatch = dispatch
        .as_ref()
        .map(OwnerSnapshot::persistent_cancel_owner_v1);
    let mut session = persistent_compute_cancellation_test_session(binding.queue, None, None);
    session.dispatch = dispatch;
    session.terminal_poisoned = true;
    PersistentCancelContextV1::retain(&mut session, root);
    let attachment = session.persistent_compute.as_ref().unwrap();
    assert_eq!(entries_snapshot(attachment), expected_entries);
    let Some(PersistentComputeTerminalNativeCustodyV1::Cancellation(native)) =
        &attachment.terminal_custody
    else {
        panic!("production retention lost constructed cancellation custody")
    };
    assert_eq!(native_snapshot(native), expected_native);
    let before = snapshot(&session);
    let _ = take_dispatch_terminal_process_gate_record_v1();
    for _ in 0..3 {
        assert!(!cancel(&mut session, three, binding, terminal_error(three)));
        assert_eq!(snapshot(&session), before);
        assert_eq!(
            session
                .dispatch
                .as_ref()
                .map(OwnerSnapshot::persistent_cancel_owner_v1),
            expected_dispatch,
        );
        assert!(session.persistent_compute_test_release.is_none());
        assert_eq!(
            (
                session.detached_dispatch_generation,
                session.detached_data_count,
                session.detached_next_insertion_index,
                session.detached_data_identities.len(),
            ),
            (None, 0, None, 0),
        );
    }
    let _ = take_dispatch_terminal_process_gate_record_v1();
}

#[test]
fn persistent_cancel_public_terminal_reentry_preserves_complete_cancellation_custody() {
    for three in [false, true] {
        let mut session = fixture(three);
        let binding = session.persistent_compute.as_ref().unwrap().binding;
        session.persistent_compute_test_release.as_mut().unwrap().0 = 99;
        let _ = take_dispatch_terminal_process_gate_record_v1();
        assert!(!cancel(
            &mut session,
            three,
            binding,
            ExpectedError::Substituted
        ));
        assert!(session.terminal_poisoned);
        assert_eq!(take_dispatch_terminal_process_gate_record_v1(), three);
        let before = snapshot(&session);
        for _ in 0..3 {
            assert!(!cancel(&mut session, three, binding, terminal_error(three)));
            assert_eq!(snapshot(&session), before);
            assert!(session.persistent_compute_test_release.is_none());
        }
        let foreign = PersistentComputeBindingKeyV1 {
            queue: test_queue_key(292, 1),
            ..binding
        };
        assert!(cancel(
            &mut session,
            three,
            foreign,
            ExpectedError::Poisoned
        ));
        assert_eq!(snapshot(&session), before);
        let _ = take_dispatch_terminal_process_gate_record_v1();
    }
}

#[test]
fn persistent_cancel_public_terminal_ingress_preserves_preexisting_detached_custody() {
    for three in [false, true] {
        let mut session = fixture(three);
        let (_, data) = session.persistent_compute_test_release.take().unwrap();
        let expected = data
            .iter()
            .map(|data| data.sdma_storage_identity())
            .collect::<Vec<_>>();
        let attachment = session.persistent_compute.as_mut().unwrap();
        let binding = attachment.binding;
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
            PersistentComputeTerminalDataV1::from_vec(data),
        ));
        session.terminal_poisoned = true;
        let before = snapshot(&session);
        for _ in 0..2 {
            assert!(!cancel(&mut session, three, binding, terminal_error(three)));
            assert_eq!(snapshot(&session), before);
            let PersistentComputeTerminalNativeCustodyV1::Data(data) = session
                .persistent_compute
                .as_ref()
                .unwrap()
                .terminal_custody
                .as_ref()
                .unwrap()
            else {
                panic!("existing terminal data was overwritten")
            };
            assert_eq!(
                data.iter()
                    .map(|data| data.sdma_storage_identity())
                    .collect::<Vec<_>>(),
                expected
            );
        }
        let _ = take_dispatch_terminal_process_gate_record_v1();
    }
}

#[test]
fn persistent_cancel_public_wrong_state_preserves_exact_published_lease() {
    for three in [false, true] {
        for index in 0..if three { 3 } else { 1 } {
            let mut session = fixture(three);
            let attachment = session.persistent_compute.as_mut().unwrap();
            let binding = attachment.binding;
            let entry = &mut attachment.entries[index];
            let PersistentComputeUseStateV1::Prepared(prepared) =
                core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined)
            else {
                unreachable!()
            };
            let published = entry.allocation.owner.publish(prepared).unwrap();
            let expected = published.cancellation_identity_for_test();
            entry.state = PersistentComputeUseStateV1::Published(published);
            assert!(!cancel(&mut session, three, binding, ExpectedError::Phase));
            for (position, entry) in session
                .persistent_compute
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .enumerate()
            {
                assert_eq!(entry.allocation.owner.live_use_count(), 1);
                if position == index {
                    let PersistentComputeUseStateV1::Published(published) = &entry.state else {
                        panic!("mismatched Published lease was discarded")
                    };
                    assert_eq!(published.cancellation_identity_for_test(), expected);
                    assert_eq!(entry.allocation.owner.quarantine_reason(), None);
                } else {
                    assert!(matches!(
                        entry.state,
                        PersistentComputeUseStateV1::Quarantined
                    ));
                    assert_eq!(
                        entry.allocation.owner.quarantine_reason(),
                        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
                    );
                }
            }
            assert!(session.persistent_compute_test_release.is_some());
            assert!(session.terminal_poisoned);
            let _ = take_dispatch_terminal_process_gate_record_v1();
        }
    }
}

#[test]
fn persistent_cancel_public_terminal_wrong_shape_or_generation_returns_exact_receipt() {
    for three in [false, true] {
        let mut session = fixture(three);
        let (_, data) = session.persistent_compute_test_release.take().unwrap();
        let attachment = session.persistent_compute.as_mut().unwrap();
        let binding = attachment.binding;
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
            PersistentComputeTerminalDataV1::from_vec(data),
        ));
        session.terminal_poisoned = true;
        let before = snapshot(&session);
        assert!(cancel(
            &mut session,
            !three,
            binding,
            ExpectedError::Poisoned
        ));
        assert_eq!(snapshot(&session), before);
        let stale = PersistentComputeBindingKeyV1 {
            attachment_generation: binding.attachment_generation + 1,
            ..binding
        };
        assert!(cancel(&mut session, three, stale, ExpectedError::Poisoned));
        assert_eq!(snapshot(&session), before);
        let _ = take_dispatch_terminal_process_gate_record_v1();
    }
}
