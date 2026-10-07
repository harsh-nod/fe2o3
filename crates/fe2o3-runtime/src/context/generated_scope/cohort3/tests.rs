//! CPU lifecycle composition only; all native hooks are intentionally excluded.

use super::super::tests::{Borrowed, context};
use super::*;
use std::cell::Cell;
use std::sync::Arc;

fn members<'a>(
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
    domains: &[Arc<()>; 3],
) -> Cohort<Borrowed<'a>> {
    Cohort::new(std::array::from_fn(|index| Borrowed {
        ticks: Cell::new(index),
        decoded,
        dropped,
        domain: Arc::clone(&domains[index]),
        completion_order: None,
    }))
}

fn cpu_scope<'scope, 'env, 'owners>(
    context: &'env mut RuntimeContextV1<KfdRuntimeBackendV1>,
) -> RuntimeGfx942GeneratedScopeV1<'scope, 'env, KfdRuntimeBackendV1, Cohort<Borrowed<'owners>>> {
    let (slots, copies) = allocate_rosters(1).unwrap();
    RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context,
        slots,
        copies,
        graph: None,
        capacity: 1,
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(10),
        invariant: PhantomData,
        hooks: Hooks {
            domains: cohort_domains_v1,
            decode: RuntimeGfx942PreparedV1::complete_cohort3_readbacks_v1,
            progress_graph: |_| Ok(0),
            progress_copies: |_| Ok(0),
            reserve: |_, _| {
                let (_, p) = crate::authorized_execution::tests::source_projection();
                GeneratedHostRosterV1::from_projection(&p)
            },
            preflight: |_, _, _, _, _| Ok(()),
            ready: |_, _| Ok(true),
            adopt: |_, _, _, _| Ok(()),
            progress: |_, prepared, _, _| {
                let members = &prepared.value().members;
                let all = members.iter().all(|m| m.ticks.get() == 0);
                for member in members {
                    member.ticks.set(member.ticks.get().saturating_sub(1));
                }
                Ok(all)
            },
            complete: |context, _, _, hold| {
                context
                    .release_unpublished_hold_v1(hold)
                    .map_err(Into::into)
            },
            unpublished: |_, _| panic!("aggregate cancellation not exposed"),
            rejected: |_, _| Ok(false),
            retire_rejected: |_, _, _, _| panic!("aggregate rejection settlement not exposed"),
            retire_unpublished: |_, _| panic!("no fake native retirement"),
            copy_progress: |_, _| panic!("aggregate copy not exposed"),
            graph_submit: |_, _, _| panic!("aggregate graph not exposed"),
            graph_progress: |_, _, _| panic!("aggregate graph not exposed"),
        },
    }
}

#[test]
fn cohort3_completion_domains_reject_alias_and_keep_order_without_scalar_projection() {
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let domains = std::array::from_fn(|_| Arc::new(()));
    let original = members(&decoded, &dropped, &domains);
    let captured = cohort_domains_v1(&original).unwrap();
    assert!(!captured.matches_single(&domains[0]));
    let CompletionDomainsV1::Cohort3(values) = captured else {
        panic!("aggregate only")
    };
    for index in 0..3 {
        assert!(values[index].matches_owner(&domains[index]));
        assert!(!values[index].matches_owner(&domains[(index + 1) % 3]));
    }
    let aliases = [
        Arc::clone(&domains[0]),
        Arc::clone(&domains[0]),
        Arc::clone(&domains[2]),
    ];
    let alias = members(&decoded, &dropped, &aliases);
    assert!(matches!(
        cohort_domains_v1(&alias),
        Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
    ));
    assert_eq!((decoded.get(), dropped.get()), (0, 0));
    drop((original, alias));
    assert_eq!((decoded.get(), dropped.get()), (0, 6));
}

#[test]
fn cohort3_whole_lifecycle_retains_three_decoders_until_one_settlement() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let domains = std::array::from_fn(|_| Arc::new(()));
    let prepared = context.bound_preparation_for_test_v1(members(&decoded, &dropped, &domains));
    let mut scope = cpu_scope(&mut context);
    let ticket = RuntimeGfx942ScopedCohort3TicketV1 {
        ticket: scope.admit(prepared, stream).unwrap(),
    };
    assert!(matches!(
        scope.check_submission(),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    for _ in 0..4 {
        scope.progress_v1().unwrap();
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        for (index, gate) in domains.iter().enumerate() {
            assert!(
                !scope
                    .cohort3_completion_matches_owner_v1(&ticket, index, gate)
                    .unwrap()
            );
        }
    }
    scope.progress_v1().unwrap();
    assert_eq!((decoded.get(), dropped.get()), (3, 3));
    assert_eq!(scope.pending_v1(), 0);
    for (index, gate) in domains.iter().enumerate() {
        assert!(
            scope
                .cohort3_completion_matches_owner_v1(&ticket, index, gate)
                .unwrap()
        );
        assert!(
            !scope
                .completion_matches_owner_v1(&ticket.ticket, gate)
                .unwrap()
        );
        assert!(
            !scope
                .cohort3_completion_matches_owner_v1(&ticket, (index + 1) % 3, gate)
                .unwrap()
        );
    }
    assert!(
        scope
            .cohort3_completion_matches_owner_v1(&ticket, 3, &domains[0])
            .is_err()
    );
    drop(scope);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn cohort3_public_scope_refuses_synthetic_device_before_callback_or_epoch() {
    let mut context = context();
    let result = context.with_generated_gfx942_cohort3_scope_v1::<Borrowed<'_>, ()>(
        Instant::now() + Duration::from_secs(10),
        |_| panic!("no admitted native device"),
    );
    assert!(result.is_err());
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn cohort3_unknown_effect_retains_all_originals_and_fails_stop_before_disposal() {
    const CHILD: &str = "FE2O3_COHORT3_UNKNOWN_CHILD";
    const TEST: &str = "context::generated_scope::cohort3::tests::cohort3_unknown_effect_retains_all_originals_and_fails_stop_before_disposal";
    const MARKER: &str = "COHORT3_THREE_ORIGINALS_RETAINED";
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
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let domains = std::array::from_fn(|_| Arc::new(()));
        let prepared = context.bound_preparation_for_test_v1(members(&decoded, &dropped, &domains));
        let mut scope = cpu_scope(&mut context);
        scope.hooks.adopt = if mode == "panic" {
            |_, _, _, _| panic!("injected aggregate adoption unwind")
        } else {
            |_, _, _, _| Err(RuntimeValidationErrorV1::ContextReserved.into())
        };
        let ticket = scope.admit(prepared, stream).unwrap();
        let mut future = scope.completion_future_v1(&ticket).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scope.progress_v1()));
        if mode == "panic" {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
        assert_eq!(
            scope.slots[0]
                .lifecycle
                .value
                .as_ref()
                .unwrap()
                .value()
                .members
                .len(),
            3
        );
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        assert!(scope.context.has_unpublished_holds_v1());
        assert!(matches!(
            scope.progress_v1(),
            Err(RuntimeGfx942ScopeErrorV1::Unknown)
        ));
        use std::future::Future;
        assert!(matches!(
            std::pin::Pin::new(&mut future)
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop())),
            std::task::Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
        ));
        eprintln!("{MARKER}");
        drop(scope);
        panic!("aggregate unknown released original custody");
    }
    for mode in ["error", "panic"] {
        super::super::tests::unpublished::abort_child(TEST, CHILD, mode, MARKER);
    }
}
