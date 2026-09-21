use super::xgmi_batch_wait::{
    completed_identities, fixture, retained_identities, write_completion,
};
use super::*;

fn expired() -> XgmiSingleDeadlineV1 {
    XgmiSingleDeadlineV1::Absolute(Instant::now())
}

#[test]
fn scalar_absolute_wait_preserves_the_callers_exact_deadline() {
    let deadline = Instant::now() - Duration::from_secs(1);
    assert_eq!(
        XgmiSingleDeadlineV1::Absolute(deadline).resolve().unwrap(),
        deadline
    );
    assert!(matches!(
        XgmiSingleDeadlineV1::Relative(Duration::MAX).resolve(),
        Err(Gfx942SdmaErrorV1::Contract("XGMI SDMA wait deadline"))
    ));
}

#[test]
fn ready_scalar_wait_at_expiry_returns_only_its_exact_mapping_pair() {
    for slot in 0..2 {
        let (mut memory, mut owner, tickets, identities) = fixture();
        write_completion(
            &mut memory,
            &mut owner,
            slot,
            i64::from(tickets[slot].generation),
        );
        let completed = owner
            .wait_xgmi_for_in_current_scope(&mut memory, tickets[slot], expired())
            .unwrap();
        assert_eq!(completed_identities(vec![completed]), [identities[slot]]);
        assert_eq!(retained_identities(&owner), [identities[1 - slot]]);
        assert!(!owner.is_poisoned());
    }
}

#[test]
fn legacy_zero_duration_still_observes_a_ready_completion() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
    let completed = owner
        .wait_xgmi_for_in_current_scope(
            &mut memory,
            tickets[0],
            XgmiSingleDeadlineV1::Relative(Duration::ZERO),
        )
        .unwrap();
    assert_eq!(completed_identities(vec![completed]), [identities[0]]);
    assert_eq!(retained_identities(&owner), [identities[1]]);
}

#[test]
fn timeout_retains_both_owners_then_retry_reuses_the_same_ticket() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    for deadline in [expired(), XgmiSingleDeadlineV1::Relative(Duration::ZERO)] {
        assert!(matches!(
            owner.wait_xgmi_for_in_current_scope(&mut memory, tickets[0], deadline),
            Err(Gfx942SdmaErrorV1::Timeout)
        ));
        assert_eq!(retained_identities(&owner), identities);
        assert!(!owner.is_poisoned());
    }
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
    let completed = owner
        .wait_xgmi_for_in_current_scope(&mut memory, tickets[0], expired())
        .unwrap();
    assert_eq!(completed_identities(vec![completed]), [identities[0]]);
    assert_eq!(retained_identities(&owner), [identities[1]]);
    assert!(
        owner
            .wait_xgmi_for_in_current_scope(&mut memory, tickets[0], expired())
            .is_err()
    );
    assert_eq!(retained_identities(&owner), [identities[1]]);
}

#[test]
fn malformed_scalar_ticket_is_rejected_before_any_observation_or_retirement() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    // Any attempted completion observation would panic, even for a ready slot.
    memory.sdma_mapping_panic_v1(owner.completions.as_ref().unwrap(), "observe_i64_acquire");
    for kind in 0..5 {
        let mut bad = tickets[0];
        match kind {
            0 => bad.owner = queue_key(8, 11, 13),
            1 => bad.queue_id += 1,
            2 => bad.slot = u16::MAX,
            3 => bad.generation += 1,
            _ => bad.slot = 2,
        }
        assert!(matches!(
            owner.wait_xgmi_for_in_current_scope(&mut memory, bad, expired()),
            Err(Gfx942SdmaErrorV1::Contract(_))
        ));
        assert_eq!(retained_identities(&owner), identities);
        assert!(!owner.is_poisoned());
    }
}

#[test]
fn scalar_invalid_completion_and_observation_panic_retain_the_exact_owners() {
    for value in [-1, 1, 4, i64::MAX] {
        let (mut memory, mut owner, tickets, identities) = fixture();
        write_completion(&mut memory, &mut owner, 0, value);
        assert!(matches!(
            owner.wait_xgmi_for_in_current_scope(&mut memory, tickets[0], expired()),
            Err(Gfx942SdmaErrorV1::Contract(
                "unexpected XGMI SDMA completion value"
            ))
        ));
        assert!(owner.is_poisoned());
        assert_eq!(retained_identities(&owner), identities);
    }
    let (mut memory, mut owner, tickets, identities) = fixture();
    memory.sdma_mapping_panic_v1(owner.completions.as_ref().unwrap(), "observe_i64_acquire");
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.wait_xgmi_for_in_current_scope(&mut memory, tickets[0], expired());
    }))
    .unwrap_err();
    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&("N1 mapped panic", "observe_i64_acquire"))
    );
    assert_eq!(retained_identities(&owner), identities);
}

#[test]
fn scalar_and_singleton_batch_match_completion_and_retained_custody() {
    for value in [0, 3, -1, 4] {
        let (mut scalar_memory, mut scalar, scalar_tickets, scalar_ids) = fixture();
        let (mut batch_memory, mut batch, batch_tickets, batch_ids) = fixture();
        write_completion(&mut scalar_memory, &mut scalar, 0, value);
        write_completion(&mut batch_memory, &mut batch, 0, value);
        let deadline = Instant::now();
        let single = scalar.wait_xgmi_for_in_current_scope(
            &mut scalar_memory,
            scalar_tickets[0],
            XgmiSingleDeadlineV1::Absolute(deadline),
        );
        let many = batch.wait_many_xgmi_for_in_current_scope(
            &mut batch_memory,
            &[batch_tickets[0]],
            XgmiBatchDeadlineV1::Absolute(deadline),
        );
        match (single, many) {
            (Ok(single), Ok(many)) => {
                assert_eq!(completed_identities(vec![single]), [scalar_ids[0]]);
                assert_eq!(completed_identities(many), [batch_ids[0]]);
                assert_eq!(retained_identities(&scalar), [scalar_ids[1]]);
                assert_eq!(retained_identities(&batch), [batch_ids[1]]);
            }
            (Err(Gfx942SdmaErrorV1::Timeout), Err(Gfx942SdmaErrorV1::Timeout)) if value == 0 => {
                assert_eq!(retained_identities(&scalar), scalar_ids);
                assert_eq!(retained_identities(&batch), batch_ids);
            }
            (Err(Gfx942SdmaErrorV1::Contract(_)), Err(Gfx942SdmaErrorV1::Contract(_)))
                if value != 3 =>
            {
                assert_eq!(retained_identities(&scalar), scalar_ids);
                assert_eq!(retained_identities(&batch), batch_ids);
            }
            _ => panic!("scalar and singleton batch differ"),
        }
        assert_eq!(scalar.is_poisoned(), batch.is_poisoned());
    }
}

#[test]
fn closing_failure_preserves_completed_pair_or_retained_ticket() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
    let completed = owner
        .wait_xgmi_for_in_current_scope(&mut memory, tickets[0], expired())
        .unwrap();
    match classify_xgmi_wait_result(
        Ok(completed),
        Err(Gfx942SdmaErrorV1::Contract("close")),
        tickets[0],
    ) {
        Err(Gfx942XgmiWaitFailureV1::CompletedCurrentnessIndeterminate {
            error: Gfx942SdmaErrorV1::Contract("close"),
            completed,
        }) => assert_eq!(completed_identities(vec![completed]), [identities[0]]),
        _ => panic!("closing failure must retain completed owners as indeterminate"),
    }
    match classify_xgmi_wait_result(
        Err(Gfx942SdmaErrorV1::Timeout),
        Err(Gfx942SdmaErrorV1::Contract("close")),
        tickets[1],
    ) {
        Err(Gfx942XgmiWaitFailureV1::Retained {
            error: Gfx942SdmaErrorV1::Contract("close"),
            ticket,
        }) => assert_eq!(ticket, tickets[1]),
        _ => panic!("closing error must win without retiring the pending ticket"),
    }
    assert_eq!(retained_identities(&owner), [identities[1]]);
}

#[test]
fn singleton_wait_uses_scalar_storage_and_keeps_the_currentness_driver() {
    let source = include_str!("../../sdma.rs");
    let scalar = source
        .split("fn wait_xgmi_for_in_current_scope(")
        .nth(1)
        .unwrap()
        .split("fn wait_many_xgmi_for_in_current_scope(")
        .next()
        .unwrap();
    for allocation in ["Vec", "vec!", "try_reserve", "Box"] {
        assert!(!scalar.contains(allocation));
    }
    let method = source
        .split("pub fn wait_until(")
        .nth(1)
        .unwrap()
        .split("pub fn wait_batch_for(")
        .next()
        .unwrap();
    assert!(method.contains("self.queue.wait_for_with_currentness("));
    assert!(method.contains("XgmiSingleDeadlineV1::Absolute(deadline)"));
    assert!(method.contains("XgmiRouteCurrentnessV1::BatchScoped"));
    assert!(!method.contains("Duration"));
    let driver = source
        .split("fn wait_for_with_currentness(")
        .nth(1)
        .unwrap()
        .split("fn validate_route_currentness(")
        .next()
        .unwrap();
    let checks: Vec<_> = driver
        .match_indices("Self::validate_route_currentness(")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(checks.len(), 2);
    let native = driver
        .find("owner.wait_xgmi_for_in_current_scope(")
        .unwrap();
    let classify = driver
        .find("classify_xgmi_wait_result(result, post, ticket)")
        .unwrap();
    assert!(checks[0] < native && native < checks[1] && checks[1] < classify);
}
