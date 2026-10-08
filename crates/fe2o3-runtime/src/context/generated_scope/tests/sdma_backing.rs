#![cfg(test)]

use super::*;

type Scope<'a> = RuntimeGfx942GeneratedScopeV1<'a, 'a, KfdRuntimeBackendV1, Borrowed<'a>>;

fn singleton_roster(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    prepared: &mut RuntimeGfx942PreparedV1<Borrowed<'_>>,
) -> Result<GeneratedHostRosterV1, crate::RuntimeGfx942GeneratedReservationErrorV1> {
    let mut roster = (hooks().reserve)(context, prepared)?;
    // Synthetic lifecycle metadata only; no native source/owner is fabricated.
    roster.count = 1;
    roster.buffers[1..].fill(None);
    Ok(roster)
}

pub(super) fn fixture<'a>(
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
) -> (
    Scope<'a>,
    RuntimeGfx942PreparedV1<Borrowed<'a>>,
    RuntimeStreamIdV1,
) {
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let prepared = context.bound_preparation_for_test_v1(Borrowed {
        ticks: Cell::new(0),
        decoded,
        dropped,
        domain: std::sync::Arc::new(()),
        completion_order: None,
    });
    let (slots, copies) = allocate_rosters(1).unwrap();
    let mut hooks = hooks();
    hooks.reserve = singleton_roster;
    hooks.adopt = |_, prepared, _, _| {
        prepared.value().ticks.set(1);
        Ok(())
    };
    hooks.sdma_adoption = Some((
        |_, _| Ok(true),
        |_, prepared, _, _| {
            prepared.value().ticks.set(2);
            Ok(())
        },
    ));
    let scope = RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context,
        slots,
        copies,
        graph: None,
        capacity: 1,
        identity: Rc::new(()),
        deadline: Instant::now() + Duration::from_secs(30),
        hooks,
        invariant: PhantomData,
    };
    (scope, prepared, stream)
}

#[test]
fn ordinary_slot_keeps_ordinary_adoption_even_with_opt_in_hooks_available() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) = fixture(&mut context, &decoded, &dropped);
    scope.admit(prepared, stream).unwrap();
    assert!(!scope.slots[0].sdma_backed);
    scope.progress_v1().unwrap();
    assert_eq!(
        scope.slots[0]
            .lifecycle
            .value
            .as_ref()
            .unwrap()
            .value()
            .ticks
            .get(),
        1
    );
    scope.drain_v1().unwrap();
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
}

#[test]
fn opted_slot_waits_for_its_own_ready_gate_then_uses_only_sdma_adoption() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) = fixture(&mut context, &decoded, &dropped);
    let (_, adopt) = scope.hooks.sdma_adoption.unwrap();
    scope.hooks.sdma_adoption = Some((|_, _| Ok(false), adopt));
    scope.admit_with_storage(prepared, stream, true).unwrap();
    assert!(scope.slots[0].sdma_backed);
    assert_eq!(scope.progress_v1().unwrap(), 0);
    assert_eq!(scope.slots[0].lifecycle.phase, Phase::Adopting);
    assert_eq!(dropped.get(), 0);
    scope.hooks.sdma_adoption = Some((|_, _| Ok(true), adopt));
    scope.progress_v1().unwrap();
    assert_eq!(
        scope.slots[0]
            .lifecycle
            .value
            .as_ref()
            .unwrap()
            .value()
            .ticks
            .get(),
        2
    );
    scope.drain_v1().unwrap();
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
}

#[test]
fn missing_opt_in_hooks_refuse_before_any_hold_or_slot() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) = fixture(&mut context, &decoded, &dropped);
    scope.hooks.sdma_adoption = None;
    assert!(scope.admit_with_storage(prepared, stream, true).is_err());
    assert!(scope.slots.is_empty());
    assert_eq!((decoded.get(), dropped.get()), (0, 1));
    drop(scope);
    context.destroy_stream(stream).unwrap();
}

#[test]
fn multiple_data_extents_refuse_without_changing_default_roster_acceptance() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) = fixture(&mut context, &decoded, &dropped);
    scope.hooks.reserve = hooks().reserve;
    assert!(scope.admit_with_storage(prepared, stream, true).is_err());
    assert!(scope.slots.is_empty());
    drop(scope);
    context.destroy_stream(stream).unwrap();
}

#[test]
fn foreign_profile_refuses_even_with_one_descriptive_extent() {
    let mut context = context();
    let (decoded, dropped) = (Cell::new(0), Cell::new(0));
    let (mut scope, prepared, stream) = fixture(&mut context, &decoded, &dropped);
    scope.hooks.reserve = |context, prepared| {
        let mut roster = singleton_roster(context, prepared)?;
        roster.source_identity = crate::generated_source::GeneratedSourceIdentityV1::Cohort3(
            core::array::from_fn(|_| std::sync::Arc::new(())),
        );
        Ok(roster)
    };
    assert!(scope.admit_with_storage(prepared, stream, true).is_err());
    assert!(scope.slots.is_empty());
    drop(scope);
    context.destroy_stream(stream).unwrap();
}

#[test]
fn public_opt_in_rejects_synthetic_backend_before_preparation() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let called = Cell::new(false);
    let mut scope = context
        .new_generated_scope_v1::<Borrowed<'_>>(1, Instant::now() + Duration::from_secs(30))
        .unwrap();
    let result = scope.try_submit_sdma_backed_v1(device, stream, |_| {
        called.set(true);
        Err::<Borrowed<'_>, ()>(())
    });
    assert!(matches!(
        result,
        Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(_))
    ));
    assert!(!called.get());
    assert!(scope.slots.is_empty());
    drop(scope);
    context.destroy_stream(stream).unwrap();
}
