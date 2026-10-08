//! Synthetic scope and original Context admission controls, not native-copy admission.
#![cfg(test)]

use super::*;

type Scope<'a> = RuntimeGfx942GeneratedScopeV1<'a, 'a, KfdRuntimeBackendV1, Borrowed<'a>>;

fn fixture<'a>(
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
) -> (
    Scope<'a>,
    RuntimeGfx942ScopedTicketV1<'a>,
    RuntimeStreamIdV1,
    RuntimeMemoryRegionV1,
) {
    fixture_with_capacity(context, decoded, dropped, 1)
}

fn fixture_with_capacity<'a>(
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
    capacity: usize,
) -> (
    Scope<'a>,
    RuntimeGfx942ScopedTicketV1<'a>,
    RuntimeStreamIdV1,
    RuntimeMemoryRegionV1,
) {
    let (mut scope, prepared, producer_stream) =
        super::sdma_backing::fixture(context, decoded, dropped);
    let (slots, copies) = allocate_rosters(capacity).unwrap();
    scope.slots = slots;
    scope.copies = copies;
    scope.capacity = capacity;
    scope.hooks.data_copy =
        Some(|_, _, _, _, _, _, _| panic!("registration must not enter a copy"));
    let ticket = scope
        .admit_with_storage(prepared, producer_stream, true)
        .unwrap();
    let (stream, destination) = {
        let _permit = scope.epoch.enter().unwrap();
        let device = scope.context.devices()[0].id();
        let bytes = scope.slots[0].roster.buffers[0].unwrap().bytes;
        let stream = scope.context.create_stream(device).unwrap();
        let allocation = scope
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, bytes, 8)
            .unwrap();
        (
            stream,
            RuntimeMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: bytes,
            },
        )
    };
    (scope, ticket, stream, destination)
}

fn admit_next<'a>(
    scope: &mut Scope<'a>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
) -> RuntimeGfx942ScopedTicketV1<'a> {
    let (prepared, stream) = {
        let _permit = scope.epoch.enter().unwrap();
        let stream = scope
            .context
            .create_stream(scope.context.devices()[0].id())
            .unwrap();
        let prepared = scope.context.bound_preparation_for_test_v1(Borrowed {
            ticks: Cell::new(0),
            decoded,
            dropped,
            domain: std::sync::Arc::new(()),
            completion_order: None,
        });
        (prepared, stream)
    };
    scope.admit_with_storage(prepared, stream, true).unwrap()
}

fn settle_scripted_copy(scope: &mut Scope<'_>) {
    // This releases the real test hold, not a native copy or physical DATA owner.
    scope.hooks.data_copy = Some(|context, _, _, hold, _, _, _| {
        context.validate_unpublished_hold_v1(hold)?;
        context.release_unpublished_hold_v1(hold)?;
        Ok(true)
    });
    scope.progress_v1().unwrap();
}

fn destination_state(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    allocation: RuntimeAllocationIdV1,
) -> fe2o3_runtime_model::ContextAllocationStateV1 {
    context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_allocation(context.allocations[&allocation].journal.unwrap())
        .unwrap()
}

fn context() -> RuntimeContextV1<KfdRuntimeBackendV1> {
    RuntimeContextV1::open_with_version_journal_v1(
        KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1(),
        16,
        16,
    )
    .unwrap()
}

#[test]
fn registration_preserves_original_ticket_and_cancels_only_before_adoption() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, ticket, stream, destination) = fixture(&mut context, &decoded, &dropped);
    let before = destination_state(scope.context, destination.allocation);
    scope
        .copy_generated_output_to_v1(&ticket, stream, destination)
        .unwrap();
    assert_eq!(scope.slots[0].lifecycle.phase, Phase::Adopting);
    assert!(
        scope.slots[0]
            .data_copy
            .as_ref()
            .unwrap()
            .submission
            .is_none()
    );
    assert!(scope.context.submissions.is_empty());
    assert_eq!(
        destination_state(scope.context, destination.allocation),
        before
    );
    assert_eq!((decoded.get(), dropped.get()), (0, 0));
    assert!(
        scope
            .copy_generated_output_to_v1(&ticket, stream, destination)
            .is_err()
    );
    assert_eq!(
        scope.cancel_before_adoption_v1(&ticket).unwrap(),
        RuntimeGfx942ScopedCancelResultV1::CancelledBeforeSubmission
    );
    assert!(scope.context.submissions.is_empty());
    assert_eq!((decoded.get(), dropped.get()), (0, 1));
    drop(scope);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn cancelled_request_allows_another_original_producer_to_reuse_destination_and_stream() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, first, stream, destination) =
        fixture_with_capacity(&mut context, &decoded, &dropped, 2);
    let before = destination_state(scope.context, destination.allocation);
    scope
        .copy_generated_output_to_v1(&first, stream, destination)
        .unwrap();
    let second = admit_next(&mut scope, &decoded, &dropped);
    assert!(matches!(
        scope.copy_generated_output_to_v1(&second, stream, destination),
        Err(RuntimeGfx942ScopeErrorV1::Context(
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)
        ))
    ));
    assert_eq!(
        scope.cancel_before_adoption_v1(&first).unwrap(),
        RuntimeGfx942ScopedCancelResultV1::CancelledBeforeSubmission
    );
    assert_eq!(scope.slots[first.index].lifecycle.phase, Phase::Cancelled);
    assert!(scope.slots[first.index].data_copy.is_some());
    scope
        .copy_generated_output_to_v1(&second, stream, destination)
        .unwrap();
    assert!(
        scope
            .copy_generated_output_to_v1(&second, stream, destination)
            .is_err()
    );
    assert_eq!(
        destination_state(scope.context, destination.allocation),
        before
    );
    assert!(scope.context.submissions.is_empty());
    assert_eq!((decoded.get(), dropped.get()), (0, 1));
    scope.cancel_before_adoption_v1(&second).unwrap();
    drop(scope);
    assert_eq!((decoded.get(), dropped.get()), (0, 2));
    assert!(context.cleanup().is_complete());
}

#[test]
fn settled_request_allows_another_original_producer_to_reuse_destination_and_stream() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, first, stream, destination) =
        fixture_with_capacity(&mut context, &decoded, &dropped, 2);
    let before = destination_state(scope.context, destination.allocation);
    scope
        .copy_generated_output_to_v1(&first, stream, destination)
        .unwrap();
    pending_copy(&mut scope, &first);
    settle_scripted_copy(&mut scope);
    assert_eq!(scope.slots[first.index].lifecycle.phase, Phase::Settled);
    assert!(scope.slots[first.index].data_copy.is_some());
    assert!(!scope.context.has_unpublished_holds_v1());
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    let second = admit_next(&mut scope, &decoded, &dropped);
    scope
        .copy_generated_output_to_v1(&second, stream, destination)
        .unwrap();
    assert!(
        scope
            .copy_generated_output_to_v1(&second, stream, destination)
            .is_err()
    );
    assert_eq!(
        destination_state(scope.context, destination.allocation),
        before
    );
    assert!(scope.context.submissions.is_empty());
    scope.cancel_before_adoption_v1(&second).unwrap();
    drop(scope);
    assert_eq!((decoded.get(), dropped.get()), (1, 2));
    assert!(context.cleanup().is_complete());
}

#[test]
fn unsettled_requests_keep_destination_and_stream_conflicts() {
    for phase in [Phase::Adopting, Phase::RetainedProducer, Phase::Copying] {
        let mut context = context();
        let (decoded, dropped) = (Cell::new(0), Cell::new(0));
        let (mut scope, first, stream, destination) =
            fixture_with_capacity(&mut context, &decoded, &dropped, 2);
        let before = destination_state(scope.context, destination.allocation);
        scope
            .copy_generated_output_to_v1(&first, stream, destination)
            .unwrap();
        scope.hooks.complete = |_, _, _, _| Ok(false);
        scope.hooks.data_copy = Some(|_, _, _, _, _, _, _| Ok(false));
        for _ in 0..8 {
            if scope.slots[first.index].lifecycle.phase == phase {
                break;
            }
            scope.progress_v1().unwrap();
        }
        assert_eq!(scope.slots[first.index].lifecycle.phase, phase);
        let second = admit_next(&mut scope, &decoded, &dropped);
        let (other_stream, other_destination) = {
            let _permit = scope.epoch.enter().unwrap();
            let device = scope.context.devices()[0].id();
            let stream = scope.context.create_stream(device).unwrap();
            let allocation = scope
                .context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    destination.byte_len,
                    8,
                )
                .unwrap();
            (
                stream,
                RuntimeMemoryRegionV1 {
                    allocation,
                    ..destination
                },
            )
        };
        let other_before = destination_state(scope.context, other_destination.allocation);
        for (candidate_stream, candidate_destination) in [
            (stream, other_destination),
            (other_stream, destination),
            (stream, destination),
        ] {
            assert!(matches!(
                scope.copy_generated_output_to_v1(&second, candidate_stream, candidate_destination),
                Err(RuntimeGfx942ScopeErrorV1::Context(
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)
                ))
            ));
            assert!(scope.slots[second.index].data_copy.is_none());
            assert_eq!(scope.slots[second.index].lifecycle.phase, Phase::Adopting);
            assert_eq!(scope.slots[first.index].lifecycle.phase, phase);
            assert_eq!(
                destination_state(scope.context, destination.allocation),
                before
            );
            assert_eq!(
                destination_state(scope.context, other_destination.allocation),
                other_before
            );
            assert!(scope.context.submissions.is_empty());
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
        }
        scope.cancel_before_adoption_v1(&second).unwrap();
        if phase == Phase::Adopting {
            scope.cancel_before_adoption_v1(&first).unwrap();
        } else {
            settle_scripted_copy(&mut scope);
            assert_eq!(scope.slots[first.index].lifecycle.phase, Phase::Settled);
        }
        drop(scope);
        assert_eq!(dropped.get(), 2);
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn unknown_request_keeps_conflict_and_both_original_carriers() {
    const CHILD: &str = "FE2O3_DATA_COPY_REUSE_UNKNOWN";
    const TEST: &str = "context::generated_scope::tests::data_copy::unknown_request_keeps_conflict_and_both_original_carriers";
    const MARKER: &str = "DATA_COPY_UNKNOWN_CONFLICT_ORIGINALS_RETAINED";
    if std::env::var_os(CHILD).is_some() {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut context = context();
        let (decoded, dropped) = (Cell::new(0), Cell::new(0));
        let (mut scope, first, stream, destination) =
            fixture_with_capacity(&mut context, &decoded, &dropped, 2);
        let before = destination_state(scope.context, destination.allocation);
        scope
            .copy_generated_output_to_v1(&first, stream, destination)
            .unwrap();
        pending_copy(&mut scope, &first);
        let second = admit_next(&mut scope, &decoded, &dropped);
        scope.hooks.data_copy = Some(|_, _, _, _, _, _, _| {
            Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
        });
        assert!(scope.progress_v1().is_err());
        assert_eq!(scope.slots[first.index].lifecycle.phase, Phase::Unknown);
        assert_eq!(scope.slots[second.index].lifecycle.phase, Phase::Adopting);
        assert!(!scope.context.is_terminal());
        assert!(matches!(
            scope.copy_generated_output_to_v1(&second, stream, destination),
            Err(RuntimeGfx942ScopeErrorV1::Context(
                RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)
            ))
        ));
        assert!(scope.slots[first.index].data_copy.is_some());
        assert!(scope.slots[second.index].data_copy.is_none());
        assert!(
            scope
                .slots
                .iter()
                .all(|slot| slot.lifecycle.value.is_some())
        );
        assert_eq!(
            destination_state(scope.context, destination.allocation),
            before
        );
        assert!(scope.context.submissions.is_empty());
        assert!(scope.context.has_unpublished_holds_v1());
        assert_eq!(
            (decoded.get(), dropped.get(), scope.pending_v1()),
            (0, 0, 2)
        );
        eprintln!("{MARKER}");
        drop(scope);
        panic!("unknown copy conflict returned after releasing owners");
    }
    super::unpublished::abort_child(TEST, CHILD, "unknown", MARKER);
}

#[test]
fn registration_refuses_foreign_ticket_extent_access_and_stream_without_mutation() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, ticket, stream, destination) = fixture(&mut context, &decoded, &dropped);
    let before = destination_state(scope.context, destination.allocation);
    let foreign = RuntimeGfx942ScopedTicketV1 {
        scope: Rc::new(()),
        index: ticket.index,
        invariant: PhantomData,
    };
    assert!(matches!(
        scope.copy_generated_output_to_v1(&foreign, stream, destination),
        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    ));
    let producer_stream = scope.slots[0].hold.stream();
    assert!(
        scope
            .copy_generated_output_to_v1(&ticket, producer_stream, destination)
            .is_err()
    );
    for bad in [
        RuntimeMemoryRegionV1 {
            byte_offset: 1,
            ..destination
        },
        RuntimeMemoryRegionV1 {
            byte_len: destination.byte_len - 1,
            ..destination
        },
        RuntimeMemoryRegionV1 {
            access: RuntimeAccessV1::Read,
            ..destination
        },
    ] {
        assert!(
            scope
                .copy_generated_output_to_v1(&ticket, stream, bad)
                .is_err()
        );
        assert!(scope.slots[0].data_copy.is_none());
        assert!(scope.context.submissions.is_empty());
        assert_eq!(
            destination_state(scope.context, destination.allocation),
            before
        );
        assert_eq!(scope.slots[0].lifecycle.phase, Phase::Adopting);
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
    }
    scope.slots[0].sdma_backed = false;
    assert!(
        scope
            .copy_generated_output_to_v1(&ticket, stream, destination)
            .is_err()
    );
    scope.slots[0].sdma_backed = true;
    scope.hooks.data_copy = None;
    assert!(
        scope
            .copy_generated_output_to_v1(&ticket, stream, destination)
            .is_err()
    );
    assert!(scope.slots[0].data_copy.is_none());
    assert_eq!(
        destination_state(scope.context, destination.allocation),
        before
    );
    scope.cancel_before_adoption_v1(&ticket).unwrap();
    drop(scope);
    assert!(context.cleanup().is_complete());
}

#[test]
fn stale_destination_refuses_before_copy_or_producer_adoption() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, ticket, stream, destination) = fixture(&mut context, &decoded, &dropped);
    {
        let _permit = scope.epoch.enter().unwrap();
        scope
            .context
            .release_allocation(destination.allocation)
            .unwrap();
    }
    assert!(
        scope
            .copy_generated_output_to_v1(&ticket, stream, destination)
            .is_err()
    );
    assert!(scope.slots[0].data_copy.is_none() && scope.context.submissions.is_empty());
    assert_eq!(scope.slots[0].lifecycle.phase, Phase::Adopting);
    assert_eq!((decoded.get(), dropped.get()), (0, 0));
    scope.cancel_before_adoption_v1(&ticket).unwrap();
    drop(scope);
    assert!(context.cleanup().is_complete());
}

#[test]
fn graph_reservation_refuses_before_ticket_lookup_or_destination_writer() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) =
        super::sdma_backing::fixture(&mut context, &decoded, &dropped);
    let (token, destination) = {
        let _permit = scope.epoch.enter().unwrap();
        let device = scope.context.devices()[0].id();
        let allocation = scope
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 4, 8)
            .unwrap();
        (
            scope.context.reserve_graph_v1(1).unwrap(),
            RuntimeMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 4,
            },
        )
    };
    let before = destination_state(scope.context, destination.allocation);
    let missing = RuntimeGfx942ScopedTicketV1 {
        scope: scope.identity.clone(),
        index: usize::MAX,
        invariant: PhantomData,
    };
    assert!(matches!(
        scope.copy_generated_output_to_v1(&missing, stream, destination),
        Err(RuntimeGfx942ScopeErrorV1::Context(
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)
        ))
    ));
    assert!(scope.slots.is_empty() && scope.context.submissions.is_empty());
    assert!(!scope.context.has_unpublished_holds_v1());
    assert_eq!(
        destination_state(scope.context, destination.allocation),
        before
    );
    {
        let _permit = scope.epoch.enter().unwrap();
        scope.context.close_graph_issue_v1(token).unwrap();
        scope.context.release_graph_v1(token).unwrap();
    }
    drop(prepared);
    drop(scope);
    assert_eq!((decoded.get(), dropped.get()), (0, 1));
    assert!(context.cleanup().is_complete());
}

fn pending_copy(scope: &mut Scope<'_>, ticket: &RuntimeGfx942ScopedTicketV1<'_>) {
    scope.hooks.complete = |_, _, _, _| Ok(false);
    scope.hooks.data_copy = Some(|_, _, _, _, _, _, _| Ok(false));
    for _ in 0..8 {
        scope.progress_v1().unwrap();
        if scope.slots[ticket.index].lifecycle.phase == Phase::Copying {
            return;
        }
    }
    panic!("scripted producer did not reach Copying");
}

#[test]
fn copying_observer_abandonment_keeps_carrier_until_exact_hook_and_decoder() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, ticket, stream, destination) = fixture(&mut context, &decoded, &dropped);
    scope
        .copy_generated_output_to_v1(&ticket, stream, destination)
        .unwrap();
    let observer = scope.completion_future_v1(&ticket).unwrap();
    pending_copy(&mut scope, &ticket);
    drop(observer);
    assert_eq!(
        scope.cancel_before_adoption_v1(&ticket).unwrap(),
        RuntimeGfx942ScopedCancelResultV1::NotCancellable
    );
    assert_eq!(
        scope.cancel_before_publication_v1(&ticket).unwrap(),
        RuntimeGfx942ScopedCancelResultV1::NotCancellable
    );
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (0, 0, 1)
    );
    assert!(scope.context.has_unpublished_holds_v1());
    assert!(scope.completion_v1(&ticket).unwrap().is_none());
    scope.hooks.data_copy = Some(|context, prepared, _, hold, _, _, _| {
        assert_eq!(
            (
                prepared.value().decoded.get(),
                prepared.value().dropped.get()
            ),
            (0, 0)
        );
        context.validate_unpublished_hold_v1(hold)?;
        context.release_unpublished_hold_v1(hold)?;
        Ok(true)
    });
    scope.progress_v1().unwrap();
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (1, 1, 0)
    );
    assert_eq!(scope.slots[0].lifecycle.phase, Phase::Settled);
    assert!(!scope.context.has_unpublished_holds_v1());
    drop(scope);
    assert!(context.cleanup().is_complete());
}

#[test]
fn copying_drop_forget_and_step_ambiguity_keep_original_carrier_and_hold() {
    const CHILD: &str = "FE2O3_DATA_COPY_SCOPE_FAILURE";
    const TEST: &str = "context::generated_scope::tests::data_copy::copying_drop_forget_and_step_ambiguity_keep_original_carrier_and_hold";
    const MARKER: &str = "DATA_COPY_ORIGINAL_CARRIER_AND_HOLD_ROOTED";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut context = context();
        let (decoded, dropped) = (Cell::new(0), Cell::new(0));
        let (mut scope, ticket, stream, destination) = fixture(&mut context, &decoded, &dropped);
        scope
            .copy_generated_output_to_v1(&ticket, stream, destination)
            .unwrap();
        pending_copy(&mut scope, &ticket);
        if mode == "error" || mode == "unwind" {
            scope.hooks.data_copy = if mode == "error" {
                Some(|_, _, _, _, _, _, _| Err(RuntimeValidationErrorV1::ContextTerminal.into()))
            } else {
                Some(|_, _, _, _, _, _, _| panic!("scripted original copy ambiguity"))
            };
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scope.progress_v1()));
            if mode == "unwind" {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
            assert!(matches!(
                scope.progress_v1(),
                Err(RuntimeGfx942ScopeErrorV1::Unknown)
            ));
        }
        assert_eq!(
            (decoded.get(), dropped.get(), scope.pending_v1()),
            (0, 0, 1)
        );
        assert!(scope.slots[0].lifecycle.value.is_some());
        assert!(scope.slots[0].lifecycle.outcome.is_none());
        assert!(scope.context.has_unpublished_holds_v1());
        if mode == "forget" {
            std::mem::forget(scope);
            assert!(!context.cleanup().is_complete());
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
            eprintln!("{MARKER}");
            drop(context);
        } else {
            eprintln!("{MARKER}");
            drop(scope);
        }
        panic!("copying scope returned after releasing owners");
    }
    for mode in ["drop", "forget", "error", "unwind"] {
        super::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}
