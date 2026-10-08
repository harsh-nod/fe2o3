//! Owner/observer refusal controls only; no successful native hook is simulated.

use super::super::tests::{Borrowed, context};
use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};
use std::{cell::Cell, sync::Arc};

// SAFETY: these negative controls never provide a source/completion view or a
// successful decoder. They cannot enter native execution.
#[allow(unsafe_code)]
unsafe impl RuntimeGfx942RegistryCompletionCarrierV1 for Borrowed<'_> {
    fn registry_completion_domain_v1(
        &self,
    ) -> Result<crate::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        Ok(crate::RuntimeGeneratedResultDomainV1::from_owner(
            Arc::clone(&self.domain),
        ))
    }
    fn with_registry_completion_view_v1(
        &mut self,
        _: impl for<'a> FnOnce(
            crate::RuntimeGfx942GeneratedCompletionViewV1<'a, ()>,
        ) -> Result<(), NativeError>,
    ) -> Result<(), NativeError> {
        panic!("negative fixture cannot expose native completion")
    }
    fn decode_registry_readback_retaining_source_v1(&mut self) -> Outcome {
        panic!("negative fixture cannot decode a native result")
    }
}

// SAFETY: this refusing fixture never supplies a second domain, source view or
// decoder. It tests only pre-entry refusal and cannot satisfy native admission.
#[allow(unsafe_code)]
unsafe impl crate::RuntimeGfx942RegistryRepeat2CarrierV1 for Borrowed<'_> {}

#[test]
fn registry_repeat2_public_async_refuses_synthetic_device_before_callback_or_charge() {
    let mut context = context();
    let account = account();
    let before = account.usage();
    let operation = context
        .with_generated_gfx942_registry4_repeat2_scope_async_v1::<Borrowed<'_>, (), _, _>(
            account.clone(),
            Instant::now() + Duration::from_secs(10),
            |_| std::future::ready(()),
            async |_| panic!("no native device"),
        );
    assert!(
        futures_executor::LocalPool::new()
            .run_until(operation)
            .is_err()
    );
    assert_eq!(account.usage(), before);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn registry_repeat2_requires_all_four_successful_decoders_before_rearm() {
    let mut outcomes: [Option<Outcome>; 4] = core::array::from_fn(|_| None);
    for member in 0..4 {
        assert!(scope::decoded_cycle_state(0, true, &outcomes).is_none());
        outcomes[member] = Some(Ok(()));
    }
    assert!(scope::decoded_cycle_state(0, true, &outcomes) == Some(State::Rearming));
    assert!(scope::decoded_cycle_state(1, true, &outcomes) == Some(State::Closing));
    assert!(scope::decoded_cycle_state(0, false, &outcomes) == Some(State::Closing));
    for member in 0..4 {
        outcomes[member] = Some(Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage));
        assert!(scope::decoded_cycle_state(0, true, &outcomes) == Some(State::Closing));
        outcomes[member] = Some(Ok(()));
    }
}

fn account() -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
        16,
    )
    .unwrap()
}

#[test]
fn registry16_public_async_refuses_synthetic_device_before_metadata_epoch_or_callback() {
    let mut context = context();
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, u64::MAX),
        18,
    )
    .unwrap();
    let before = account.usage();
    let operation = context
        .with_generated_gfx942_registry16_scope_async_v1::<Borrowed<'_>, (), _, _>(
            account.clone(),
            Instant::now() + Duration::from_secs(10),
            |_| std::future::ready(()),
            async |_| panic!("no native device"),
        );
    assert!(
        futures_executor::LocalPool::new()
            .run_until(operation)
            .is_err()
    );
    assert_eq!(account.usage(), before);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn registry16_requires_every_original_decoded_result_before_common_close() {
    let mut outcomes: [Option<Outcome>; 16] = core::array::from_fn(|_| None);
    for member in 0..16 {
        assert!(scope::decoded_cycle_state(0, false, &outcomes).is_none());
        outcomes[member] = Some(Ok(()));
    }
    assert!(scope::decoded_cycle_state(0, false, &outcomes) == Some(State::Closing));
    for member in 0..16 {
        outcomes[member] = Some(Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage));
        assert!(scope::decoded_cycle_state(0, false, &outcomes) == Some(State::Closing));
        outcomes[member] = Some(Ok(()));
    }
}

#[test]
fn registry4_public_scope_refuses_synthetic_device_before_metadata_epoch_or_callback() {
    let mut context = context();
    let account = account();
    let before = account.usage();
    let result = context.with_generated_gfx942_registry4_scope_v1::<Borrowed<'_>, ()>(
        account.clone(),
        Instant::now() + Duration::from_secs(10),
        |_| panic!("no native device"),
    );
    assert!(result.is_err());
    assert_eq!(account.usage(), before);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn registry4_closed_deadline_refuses_before_original_account_admission() {
    let mut context = context();
    let account = account();
    let before = account.usage();
    let result = context.with_generated_gfx942_registry4_scope_v1::<Borrowed<'_>, ()>(
        account.clone(),
        Instant::now(),
        |_| panic!("expired scope"),
    );
    assert!(matches!(result, Err(RuntimeGfx942ScopeErrorV1::Deadline)));
    assert_eq!(account.usage(), before);
    assert!(context.cleanup().is_complete());
}

#[test]
fn registry4_no_root_means_no_observer_or_result_even_with_same_scope_identity() {
    let mut context = context();
    let epoch = context.scope_epoch.begin().unwrap();
    let mut scope = RuntimeGfx942Registry4ScopeV1::<Borrowed<'_>> {
        epoch,
        context: &mut context,
        root: None,
        storage: None,
        repeat2: false,
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(10),
        invariant: PhantomData,
    };
    let gate = Arc::new(());
    for identity in [Rc::clone(&scope.identity), Rc::new(())] {
        for member in [0, 3, 4, usize::MAX] {
            let ticket = RuntimeGfx942Registry4TicketV1 {
                identity: Rc::clone(&identity),
                member,
                cycle: 0,
                invariant: PhantomData,
            };
            assert!(matches!(
                scope.result_future_v1(&ticket),
                Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
            ));
            assert!(matches!(
                scope.result_matches_owner_v1(&ticket, &gate),
                Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
            ));
        }
    }
    assert_eq!(scope.pending_v1(), 0);
    assert_eq!(scope.progress_v1().unwrap(), 0);
    drop(scope);
    assert!(context.cleanup().is_complete());
}

#[test]
fn registry4_unfinished_root_drop_or_forget_cannot_release_originals_or_context() {
    const CHILD: &str = "FE2O3_REGISTRY4_UNFINISHED_CHILD";
    const TEST: &str = "context::generated_scope::registry4::tests::registry4_unfinished_root_drop_or_forget_cannot_release_originals_or_context";
    const MARKER: &str = "REGISTRY4_ORIGINALS_AND_COMMON_HOLD_RETAINED";
    if let Some(mode) = std::env::var_os(CHILD) {
        unfinished_root::<4>(mode.to_str().unwrap(), MARKER);
    }
    for mode in ["drop", "forget", "repeat_drop", "repeat_forget"] {
        super::super::tests::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}

fn unfinished_root<const N: usize>(mode: &str, marker: &str) -> ! {
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable).unwrap();
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .unwrap();
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let gates: [Arc<()>; N] = core::array::from_fn(|_| Arc::new(()));
    let prepared =
        context.bound_preparation_for_test_v1(Registry::new(core::array::from_fn(|index| {
            Borrowed {
                ticks: Cell::new(0),
                decoded: &decoded,
                dropped: &dropped,
                domain: Arc::clone(&gates[index]),
                completion_order: None,
            }
        })));
    let epoch = context.scope_epoch.begin().unwrap();
    let hold = {
        let _permit = epoch.enter().unwrap();
        context
            .hold_unpublished_stream_with_access_v1(stream, None)
            .unwrap()
    };
    let (_, projection) = crate::authorized_execution::tests::source_projection();
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    let pairs: [_; N] = core::array::from_fn(|_| crate::async_engine::RuntimeAsyncReplyV1::pair());
    let mut index = 0;
    let mut futures = core::array::from_fn(|_| None);
    let replies = pairs.map(|(reply, future)| {
        futures[index] = Some(future);
        index += 1;
        reply
    });
    let mut scope = RuntimeGfx942Registry4ScopeV1 {
        epoch,
        context: &mut context,
        root: Some(Root {
            prepared: Some(prepared),
            storage: None,
            roster,
            hold,
            state: State::Unknown,
            outcomes: core::array::from_fn(|_| None),
            domains: gates.map(crate::RuntimeGeneratedResultDomainV1::from_owner),
            replies,
            futures,
            second: None,
            cycle: 0,
        }),
        storage: None,
        repeat2: false,
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(10),
        invariant: PhantomData,
    };
    if mode == "repeat_drop" || mode == "repeat_forget" {
        scope.repeat2 = true;
        scope.root.as_mut().unwrap().second = Some(CycleResults::new(core::array::from_fn(|_| {
            crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::new(()))
        })));
    }
    let ticket = RuntimeGfx942Registry4TicketV1 {
        identity: Rc::clone(&scope.identity),
        member: 0,
        cycle: 0,
        invariant: PhantomData,
    };
    let mut observer = scope.result_future_v1(&ticket).unwrap();
    assert!(matches!(
        scope.progress_v1(),
        Err(RuntimeGfx942ScopeErrorV1::Unknown)
    ));
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut std::task::Context::from_waker(std::task::Waker::noop())),
        Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
    ));
    assert_eq!((decoded.get(), dropped.get()), (0, 0));
    assert_eq!(scope.pending_v1(), 1);
    assert!(scope.context.has_unpublished_holds_v1());
    if mode == "forget" || mode == "repeat_forget" {
        core::mem::forget(scope);
        assert!(!context.cleanup().is_complete());
        eprintln!("{marker}");
        drop(context);
    } else {
        eprintln!("{marker}");
        drop(scope);
    }
    panic!("unfinished common root was disposed");
}

#[test]
fn registry16_unfinished_root_drop_or_forget_preserves_all_originals_and_context_hold() {
    const CHILD: &str = "FE2O3_REGISTRY16_UNFINISHED_CHILD";
    const TEST: &str = "context::generated_scope::registry4::tests::registry16_unfinished_root_drop_or_forget_preserves_all_originals_and_context_hold";
    const MARKER: &str = "REGISTRY16_ORIGINALS_AND_COMMON_HOLD_RETAINED";
    if let Some(mode) = std::env::var_os(CHILD) {
        unfinished_root::<16>(mode.to_str().unwrap(), MARKER);
    }
    for mode in ["drop", "forget"] {
        super::super::tests::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}
