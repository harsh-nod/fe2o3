use super::*;

#[derive(Debug, Eq, PartialEq)]
struct FoundationSnapshot {
    identity: model::DeviceIdentityStateV1,
    memory: model::MemoryLifecycleStateV1,
    certificate: Option<crate::queue::QueueCertificateSnapshotV1>,
}

fn snapshot(foundation: &QueueModelFoundationV1) -> FoundationSnapshot {
    FoundationSnapshot {
        identity: foundation.identity().clone(),
        memory: foundation.memory().clone(),
        certificate: foundation.certificate_snapshot_for_test(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct NativeSnapshot {
    next_live_loan_generation: u64,
    currentness: usize,
    operational_currentness: usize,
    operations: Vec<&'static str>,
    cleanup: Vec<CleanupCallV1>,
    usage: Option<Gfx942DeviceBackingUsageV1>,
}

fn native(fixture: &BackingConstructorFixture) -> NativeSnapshot {
    NativeSnapshot {
        next_live_loan_generation: fixture.ownership.next_live_loan_generation,
        currentness: fixture.engine.backend.currentness_calls,
        operational_currentness: fixture.engine.backend.operational_currentness_calls,
        operations: fixture.engine.backend.operations.clone(),
        cleanup: fixture.engine.backend.cleanup_calls.clone(),
        usage: fixture.usage(),
    }
}

fn restore(
    fixture: &mut BackingConstructorFixture,
    queue: &mut QueueModelFoundationV1,
) -> Result<(), MemorySessionError> {
    fixture.ownership.restore_foundation(
        &mut fixture.engine,
        &mut fixture.foundation,
        queue,
        fixture.device,
        fixture.vm,
    )
}

#[test]
fn foundation_restore_commits_exact_owners_once_without_native_effects() {
    let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
    let mut fixture = BackingConstructorFixture::new(Some(budget));
    let authority = fixture.mapped_device();
    let mut queue = fixture.transfer(&[&authority]).unwrap();
    let mut original = snapshot(&queue);
    assert!(original.certificate.take().is_some());
    let placeholder = snapshot(&fixture.foundation);
    let before = native(&fixture);

    restore(&mut fixture, &mut queue).unwrap();

    assert_eq!(snapshot(&fixture.foundation), original);
    assert_eq!(snapshot(&queue), placeholder);
    assert!(fixture.ownership.is_session_owned());
    assert_eq!(fixture.engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(native(&fixture), before);
    assert!(matches!(
        restore(&mut fixture, &mut queue),
        Err(MemorySessionError::Model(
            "shared queue foundation ownership phase"
        ))
    ));
    assert_eq!(snapshot(&fixture.foundation), original);
    assert_eq!(snapshot(&queue), placeholder);
    assert!(fixture.ownership.is_session_owned());
    assert_eq!(fixture.engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(native(&fixture), before);
}

#[test]
fn foundation_restore_rejection_retains_both_owners_and_backing_charge() {
    for case in 0..5 {
        let budget = Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap();
        let mut fixture = BackingConstructorFixture::new(Some(budget));
        let authority = fixture.mapped_device();
        let mut queue = fixture.transfer(&[&authority]).unwrap();
        let issuer = fixture.ownership.queue_owned_issuer().unwrap();
        match case {
            0 => fixture.vm.id.0 += 1,
            1 => fixture.engine.session_id += 1,
            2 => {
                fixture.ownership.phase =
                    QueueModelOwnershipPhaseV1::QueueOwned { issuer: issuer + 1 };
            }
            3 => queue
                .revoke_invariant_certificate(
                    fixture.engine.session_id,
                    fixture.device,
                    fixture.vm,
                    issuer,
                )
                .unwrap(),
            4 => queue
                .replace_memory_after_sealed_transition(
                    model::MemoryLifecycleStateV1::new_monotonic_non_reusable(
                        queue.memory().domain_id(),
                    ),
                )
                .unwrap(),
            _ => unreachable!(),
        }
        let queue_before = snapshot(&queue);
        let session_before = snapshot(&fixture.foundation);
        let phase = fixture.ownership.phase;
        let before = native(&fixture);

        assert!(matches!(
            restore(&mut fixture, &mut queue),
            Err(MemorySessionError::Model(
                "shared queue model ownership restoration"
            ))
        ));

        assert_eq!(snapshot(&queue), queue_before, "case {case}");
        assert_eq!(snapshot(&fixture.foundation), session_before, "case {case}");
        assert_eq!(fixture.ownership.phase, phase, "case {case}");
        assert_eq!(native(&fixture), before, "case {case}");
        assert_eq!(
            fixture.engine.phase(),
            SharedMemorySessionPhaseV1::Quarantined
        );
    }
}

#[test]
fn foundation_restore_rejects_live_loan_without_consuming_it() {
    let mut fixture = BackingConstructorFixture::new(None);
    let mut queue = fixture.transfer(&[]).unwrap();
    let loan = fixture
        .ownership
        .loan_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    let queue_before = snapshot(&queue);
    let session_before = snapshot(&fixture.foundation);
    let phase = fixture.ownership.phase;
    let before = native(&fixture);

    assert!(matches!(
        restore(&mut fixture, &mut queue),
        Err(MemorySessionError::Model(
            "shared queue foundation ownership phase"
        ))
    ));
    assert_eq!(snapshot(&queue), queue_before);
    assert_eq!(snapshot(&fixture.foundation), session_before);
    assert_eq!(fixture.ownership.phase, phase);
    assert_eq!(fixture.engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(native(&fixture), before);

    fixture
        .ownership
        .reclaim_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
            loan,
        )
        .unwrap();
    restore(&mut fixture, &mut queue).unwrap();
    let mut expected = session_before;
    expected.certificate = None;
    assert_eq!(snapshot(&fixture.foundation), expected);
    assert_eq!(snapshot(&queue), queue_before);
    assert_eq!(native(&fixture), before);
}
