//! Inert source correspondence only, never native construction or authority.

use super::*;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        32,
    )
    .unwrap()
}

#[test]
fn independent_arena1024_keeps_order_contract_and_original_member_domains_distinct() {
    let funded = account(16 << 20);
    let ordered = carrier_with_order(1, false);
    let independent = carrier_with_order(1, true);
    let left = ordered.source().validate(42).unwrap();
    let right = independent.source().validate(42).unwrap();
    assert_eq!(
        right.source_identity.profile(),
        GeneratedProfileV1::IndependentArenaMember
    );
    assert!(!left.matches(&right));
    assert_ne!(
        left.dispatch_contract_sha256,
        right.dispatch_contract_sha256
    );
    assert_eq!(
        independent.storage.data().invocation_binding(),
        independent.authority.binding
    );
    assert!(matches!(
        RuntimeGfx942GeneratedArena1024V1::try_new(&funded, 42, |index| Ok::<_, ()>(
            carrier_with_order(index % 3, true)
        )),
        Err(RuntimeGfx942ArenaPreparationErrorV1::Source(
            RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch
        ))
    ));
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert!(matches!(
        RuntimeGfx942GeneratedIndependentArena1024V1::try_new(&funded, 42, |index| Ok::<_, ()>(
            carrier_with_order(index % 3, index != 731)
        )),
        Err(RuntimeGfx942ArenaPreparationErrorV1::Source(
            RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch
        ))
    ));
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    let arena = RuntimeGfx942GeneratedIndependentArena1024V1::try_new(&funded, 42, |index| {
        Ok::<_, ()>(carrier_with_order(index % 3, true))
    })
    .unwrap();
    let roster = arena.0.validate_sources(42).unwrap();
    assert_eq!(
        roster.source_identity.profile(),
        GeneratedProfileV1::IndependentFillArena1024
    );
    let packets = arena.0.packets.as_ref().unwrap().packets();
    assert_eq!(packets.len(), 1024);
    assert!(
        packets
            .iter()
            .all(|packet| packet.ordering() == fe2o3_aql::AqlDispatchOrderingV1::Independent)
    );
    for index in [0, 1, 2, 1023] {
        let actual = arena.0.members[index]
            .as_ref()
            .unwrap()
            .source()
            .validate(42)
            .unwrap();
        assert_eq!(actual.readback_bytes, 4 * (1 + (index % 3) as u64 * 64));
        assert!(arena.0.member_original(index).unwrap().matches(&actual));
    }
    let common = match &roster.source_identity {
        GeneratedSourceIdentityV1::IndependentArena1024(original) => Arc::clone(original),
        _ => unreachable!(),
    };
    assert!(
        !roster
            .source_identity
            .matches(&GeneratedSourceIdentityV1::Arena1024(common))
    );
    arena
        .0
        .with_native_inputs_v1(42, &roster, |_, bytes| {
            assert_eq!(bytes, roster.readback_bytes);
            Ok(())
        })
        .unwrap()
        .unwrap();
    let last = arena.0.members[1023]
        .as_ref()
        .unwrap()
        .authority
        .stale
        .clone();
    assert!(
        arena
            .0
            .with_native_inputs_v1(42, &roster, |_, _| {
                last.set(true);
                Ok(())
            })
            .is_err()
    );
    last.set(false);
    drop(arena);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn arena1024_source_headers_precede_callbacks_and_member_refusal_restores_credit() {
    let short = account(0);
    let result = RuntimeGfx942GeneratedArena1024V1::<Carrier>::try_new(
        &short,
        42,
        |_| -> Result<Carrier, ()> { panic!("unfunded initializer") },
    );
    assert!(matches!(
        result,
        Err(RuntimeGfx942ArenaPreparationErrorV1::Capacity)
    ));
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    let funded = account(16 << 20);
    let mut calls = 0;
    let result = RuntimeGfx942GeneratedArena1024V1::try_new(&funded, 42, |index| {
        calls += 1;
        if index == 17 {
            Err(index)
        } else {
            Ok(carrier(index % 4))
        }
    });
    assert!(matches!(
        result,
        Err(RuntimeGfx942ArenaPreparationErrorV1::Member {
            index: 17,
            error: 17
        })
    ));
    assert_eq!(calls, 18);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(funded.usage().retained_records, 0);
}

#[test]
fn arena1024_retains_original_mixed_ranges_and_distinct_family_without_scalar_authority() {
    let funded = account(16 << 20);
    let arena = RuntimeGfx942GeneratedArena1024V1::try_new(&funded, 42, |index| {
        Ok::<_, ()>(carrier(index % 4))
    })
    .unwrap();
    let roster = arena.validate_sources(42).unwrap();
    assert_eq!(
        roster.source_identity.profile(),
        GeneratedProfileV1::NativeFillArena1024
    );
    assert_eq!(
        (roster.count, roster.fixup_count, roster.readback_bytes),
        (1, 1, 397_312)
    );
    assert_eq!(arena.members.len(), 1024);
    assert_eq!(arena.packets.as_ref().unwrap().packets().len(), 1024);
    let expected = arena.member_original(0).unwrap();
    let first = arena.members[0]
        .as_ref()
        .unwrap()
        .source()
        .validate(42)
        .unwrap();
    let second = arena.members[4]
        .as_ref()
        .unwrap()
        .source()
        .validate(42)
        .unwrap();
    assert!(expected.matches(&first));
    assert!(
        !expected.matches(&second),
        "equal byte lengths cannot replace the original view source"
    );
    assert!(!roster.matches(&first));
    let fake_scalar = GeneratedSourceIdentityV1::Singleton(match &roster.source_identity {
        GeneratedSourceIdentityV1::Arena1024(owner) => Arc::clone(owner),
        _ => unreachable!(),
    });
    assert!(!roster.source_identity.matches(&fake_scalar));
    assert!(core::mem::size_of_val(&arena) < 4096);
    assert_eq!(funded.usage().retained_records, 3);
    drop(arena);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn arena1024_input_loan_closes_last_original_without_recursive_roster_stack() {
    let funded = account(16 << 20);
    let arena = RuntimeGfx942GeneratedArena1024V1::try_new(&funded, 42, |index| {
        Ok::<_, ()>(carrier(index % 4))
    })
    .unwrap();
    let roster = arena.validate_sources(42).unwrap();
    let stale = arena.members[1023]
        .as_ref()
        .unwrap()
        .authority
        .stale
        .clone();
    let calls = Cell::new(0);
    arena
        .with_native_inputs_v1(42, &roster, |_program, bytes| {
            calls.set(calls.get() + 1);
            assert_eq!(bytes, 397_312);
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(
        arena
            .with_native_inputs_v1(42, &roster, |_, _| {
                stale.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(
        arena
            .with_native_inputs_v1(42, &roster, |_, _| panic!("stale source entered callback"))
            .is_err()
    );
    stale.set(false);
    arena.revalidate_sources().unwrap();
}

#[test]
fn arena1024_effect_free_initializer_panic_disposes_only_the_original_prefix() {
    let funded = account(16 << 20);
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = RuntimeGfx942GeneratedArena1024V1::try_new(&funded, 42, |index| {
            calls += 1;
            assert_ne!(index, 31, "inert initializer panic");
            Ok::<_, ()>(carrier(index % 4))
        });
    }));
    assert!(result.is_err());
    assert_eq!(calls, 32);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(funded.usage().retained_records, 0);
}
