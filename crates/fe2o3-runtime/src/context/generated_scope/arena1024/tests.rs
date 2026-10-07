//! Negative Context/epoch custody controls, not a native-positive fixture.

use super::super::tests::{Borrowed, context};
use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};
use std::sync::Arc;

fn metadata() -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 16 << 20),
        16,
    )
    .unwrap()
}

#[test]
fn independent_arena_scope_rejects_cross_profile_before_callbacks_or_holds() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    for profile in [
        GeneratedProfileV1::NativeFillArena1024,
        GeneratedProfileV1::IndependentFillArena1024,
    ] {
        let epoch = context.scope_epoch.begin().unwrap();
        let account = metadata();
        let before = account.usage();
        let mut scope = RuntimeGfx942Arena1024ScopeV1::<Borrowed<'_>> {
            epoch,
            context: &mut context,
            root: None,
            metadata: account.clone(),
            identity: Rc::new(()),
            deadline: Instant::now() + Duration::from_secs(10),
            profile,
            invariant: PhantomData,
        };
        let refused = match profile {
            GeneratedProfileV1::NativeFillArena1024 => {
                scope.try_submit_independent_v1::<()>(device, stream, |_| {
                    panic!("wrong family callback")
                })
            }
            GeneratedProfileV1::IndependentFillArena1024 => {
                scope.try_submit_v1::<()>(device, stream, |_| panic!("wrong family callback"))
            }
            _ => unreachable!(),
        };
        assert!(matches!(
            refused,
            Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                RuntimeGfx942ScopeErrorV1::Readback(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
            ))
        ));
        assert!(scope.root.is_none());
        assert_eq!(scope.pending_v1(), 0);
        assert_eq!(account.usage(), before);
        assert!(!scope.context.has_unpublished_holds_v1());
        drop(scope);
    }
    assert!(context.cleanup().is_complete());
}

#[test]
fn independent_arena_public_async_refuses_synthetic_device_without_authority() {
    let mut context = context();
    let account = metadata();
    let before = account.usage();
    let operation = context
        .with_generated_gfx942_independent_arena1024_scope_async_v1::<Borrowed<'_>, (), _, _>(
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
fn arena1024_public_async_refuses_synthetic_device_before_account_epoch_or_callback() {
    let mut context = context();
    let account = metadata();
    let before = account.usage();
    let operation = context
        .with_generated_gfx942_arena1024_scope_async_v1::<Borrowed<'_>, (), _, _>(
            account.clone(),
            Instant::now() + Duration::from_secs(10),
            |_| std::future::ready(()),
            async |_| panic!("no admitted native device"),
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
fn arena1024_empty_scope_refuses_all_ticket_families_and_closes_only_its_epoch() {
    let mut context = context();
    let epoch = context.scope_epoch.begin().unwrap();
    let mut scope = RuntimeGfx942Arena1024ScopeV1::<Borrowed<'_>> {
        epoch,
        context: &mut context,
        root: None,
        metadata: metadata(),
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(10),
        profile: GeneratedProfileV1::NativeFillArena1024,
        invariant: PhantomData,
    };
    for identity in [Rc::clone(&scope.identity), Rc::new(())] {
        for member in [0, 1023, 1024, usize::MAX] {
            let ticket = RuntimeGfx942Arena1024TicketV1 {
                identity: Rc::clone(&identity),
                member,
                invariant: PhantomData,
            };
            assert!(matches!(
                scope.result_future_v1(&ticket),
                Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
            ));
            assert!(matches!(
                scope.result_matches_owner_v1(&ticket, &Arc::new(())),
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
fn arena1024_unknown_root_drop_and_forget_cannot_release_original_context_hold() {
    const CHILD: &str = "FE2O3_ARENA1024_UNKNOWN_CHILD";
    const TEST: &str = "context::generated_scope::arena1024::tests::arena1024_unknown_root_drop_and_forget_cannot_release_original_context_hold";
    const MARKER: &str = "ARENA1024_ORIGINAL_HOLD_AND_RESULT_STORAGE_RETAINED";
    if let Some(mode) = std::env::var_os(CHILD) {
        unknown_child(mode.to_str().unwrap(), MARKER);
    }
    for mode in ["drop", "forget"] {
        super::super::tests::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}

fn unknown_child(mode: &str, marker: &str) -> ! {
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
    let account = metadata();
    let results = results::prepare(&account, |_| {
        Ok(crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::new(
            (),
        )))
    })
    .unwrap();
    let copied = HostMetadataTableV1::try_new(SLOTS, Some(&account), || false).unwrap();
    let epoch = context.scope_epoch.begin().unwrap();
    let hold = {
        let _permit = epoch.enter().unwrap();
        context
            .hold_unpublished_stream_with_access_v1(stream, None)
            .unwrap()
    };
    let (_, projection) = crate::authorized_execution::tests::source_projection();
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    // No prepared carrier or native session is fabricated. Only the actual
    // Context hold/epoch and original charged observer storage are exercised.
    let mut scope = RuntimeGfx942Arena1024ScopeV1::<Borrowed<'_>> {
        epoch,
        context: &mut context,
        root: Some(Root {
            observations: None,
            prepared: None,
            storage: None,
            roster,
            hold,
            state: State::Unknown,
            cells: results.cells,
            copied,
            payload: results.payload,
        }),
        metadata: account.clone(),
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(10),
        profile: GeneratedProfileV1::NativeFillArena1024,
        invariant: PhantomData,
    };
    let ticket = scope.ticket_v1(1023).unwrap();
    let mut future = scope.result_future_v1(&ticket).unwrap();
    assert!(matches!(
        scope.progress_v1(),
        Err(RuntimeGfx942ScopeErrorV1::Unknown)
    ));
    assert!(matches!(
        Pin::new(&mut future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop())),
        Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
    ));
    assert_eq!(scope.pending_v1(), 1);
    assert_eq!(account.usage().retained_records, 3);
    assert!(scope.context.has_unpublished_holds_v1());
    if mode == "forget" {
        core::mem::forget(scope);
        assert!(!context.cleanup().is_complete());
        eprintln!("{marker}");
        drop(context);
    } else {
        eprintln!("{marker}");
        drop(scope);
    }
    panic!("unknown arena returned through owner destruction")
}
