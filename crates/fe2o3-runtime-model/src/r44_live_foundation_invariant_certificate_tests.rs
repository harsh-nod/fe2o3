use super::r44_live_foundation_invariant_certificate::*;

fn foundation(session_occurrence: u64) -> R44FoundationIdentityV1 {
    R44FoundationIdentityV1 {
        session_occurrence,
        domain_id: 3,
        device_key: 5,
        vm_key: 7,
        identity_generation: 11,
        memory_generation: 13,
        foundation_revision: 17,
    }
}

fn validation(
    foundation: R44FoundationIdentityV1,
    all_invariants_valid: bool,
) -> R44FullFoundationValidationV1 {
    R44FullFoundationValidationV1::new_model_only(foundation, all_invariants_valid)
}

fn registry(session_occurrence: u64, next_loan: u64) -> R44LiveFoundationCertificateRegistryV1 {
    registry_with_occurrence(
        session_occurrence,
        session_occurrence.checked_add(1_000).unwrap(),
        next_loan,
    )
}

fn registry_with_occurrence(
    session_occurrence: u64,
    registry_occurrence: u64,
    next_loan: u64,
) -> R44LiveFoundationCertificateRegistryV1 {
    R44LiveFoundationCertificateRegistryV1::new_model_only(
        registry_occurrence,
        foundation(session_occurrence),
        next_loan,
    )
    .unwrap()
}

fn transfer(
    registry: &mut R44LiveFoundationCertificateRegistryV1,
    session_occurrence: u64,
) -> R44QueueFoundationInvariantCertificateV1 {
    registry
        .transfer_to_queue_model_only(validation(foundation(session_occurrence), true))
        .unwrap()
}

fn mutation(
    session_occurrence: u64,
    from_revision: u64,
    preserves_full_invariants: bool,
) -> R44InductiveFoundationMutationV1 {
    R44InductiveFoundationMutationV1 {
        session_occurrence,
        issuer_occurrence: session_occurrence.checked_add(1_000).unwrap(),
        certificate_id: 1,
        from_revision,
        to_revision: from_revision.saturating_add(1),
        kind: R44InductiveFoundationMutationKindV1::MapAllocation,
        preserves_full_invariants,
    }
}

#[test]
fn same_foundation_cross_registry_certificate_swap_is_rejected() {
    let mut primary = registry_with_occurrence(67, 1_067, 1);
    let mut other = registry_with_occurrence(67, 2_067, 1);
    let primary_certificate = transfer(&mut primary, 67);
    let other_certificate = transfer(&mut other, 67);
    let primary_before = primary.snapshot_model_only();
    let other_before = other.snapshot_model_only();

    let other_certificate = match primary.begin_live_loan_model_only(other_certificate) {
        Err((R44FoundationCertificateErrorV1::InvalidCertificate, certificate)) => certificate,
        other => panic!("unexpected cross-registry result: {other:?}"),
    };
    assert_eq!(primary.snapshot_model_only(), primary_before);
    assert_eq!(other.snapshot_model_only(), other_before);

    primary
        .restore_to_session_model_only(primary_certificate, validation(foundation(67), true))
        .unwrap();
    other
        .restore_to_session_model_only(other_certificate, validation(foundation(67), true))
        .unwrap();
}

#[test]
fn certificate_mints_only_after_exact_full_transfer_validation() {
    assert_eq!(
        R44LiveFoundationCertificateRegistryV1::new_model_only(0, foundation(19), 1),
        Err(R44FoundationCertificateErrorV1::InvalidRegistryOccurrence)
    );
    let mut registry = registry(19, 1);
    let before = registry.snapshot_model_only();
    assert_eq!(
        registry.transfer_to_queue_model_only(validation(foundation(19), false)),
        Err(R44FoundationCertificateErrorV1::FullValidationRejected)
    );
    assert_eq!(registry.snapshot_model_only(), before);

    let mut substituted = foundation(19);
    substituted.memory_generation = 99;
    assert_eq!(
        registry.transfer_to_queue_model_only(validation(substituted, true)),
        Err(R44FoundationCertificateErrorV1::FoundationSubstitution)
    );
    assert_eq!(registry.snapshot_model_only(), before);

    let _certificate = transfer(&mut registry, 19);
    let after = registry.snapshot_model_only();
    assert_eq!(after.phase, R44FoundationOwnershipPhaseV1::QueueOwned);
    assert_eq!(after.full_validation_scans, 1);
    assert!(after.reusable_queue_authority);
    assert!(!after.reusable_session_authority);
}

#[test]
fn admitted_inductive_mutation_updates_certificate_without_global_scan() {
    let mut registry = registry(23, 1);
    let certificate = transfer(&mut registry, 23);
    let (loan, live) = registry.begin_live_loan_model_only(certificate).unwrap();
    let before = registry.snapshot_model_only();
    let (loan, live) = registry
        .apply_inductive_mutation_model_only(loan, live, mutation(23, 17, true))
        .unwrap();
    let after = registry.snapshot_model_only();
    assert_eq!(after.foundation.foundation_revision, 18);
    assert_eq!(after.inductive_mutation_checks, 1);
    assert_eq!(after.full_validation_scans, before.full_validation_scans);
    let _certificate = registry.reclaim_live_loan_model_only(loan, live).unwrap();
    assert!(registry.snapshot_model_only().reusable_queue_authority);
}

#[test]
fn rejected_noninductive_and_stale_mutations_preserve_live_custody() {
    let mut registry = registry(29, 1);
    let certificate = transfer(&mut registry, 29);
    let (loan, live) = registry.begin_live_loan_model_only(certificate).unwrap();
    let before = registry.snapshot_model_only();
    let (loan, live) =
        match registry.apply_inductive_mutation_model_only(loan, live, mutation(29, 17, false)) {
            Err((R44FoundationCertificateErrorV1::NonInductiveMutation, loan, live)) => {
                (loan, live)
            }
            other => panic!("unexpected noninductive result: {other:?}"),
        };
    assert_eq!(registry.snapshot_model_only(), before);
    assert!(!before.reusable_queue_authority);
    assert!(!before.reusable_session_authority);

    let (loan, live) =
        match registry.apply_inductive_mutation_model_only(loan, live, mutation(29, 16, true)) {
            Err((R44FoundationCertificateErrorV1::StaleMutation, loan, live)) => (loan, live),
            other => panic!("unexpected stale result: {other:?}"),
        };
    assert_eq!(registry.snapshot_model_only(), before);
    let _certificate = registry.reclaim_live_loan_model_only(loan, live).unwrap();
}

#[test]
fn duplicate_stale_and_cross_session_loans_reject_without_mutation() {
    let mut primary = registry(31, 1);
    let certificate = transfer(&mut primary, 31).substitute_session_model_only(32);
    let queue_owned = primary.snapshot_model_only();
    let certificate = match primary.begin_live_loan_model_only(certificate) {
        Err((R44FoundationCertificateErrorV1::CrossSessionLoan, certificate)) => certificate,
        other => panic!("unexpected cross-session admission result: {other:?}"),
    }
    .substitute_session_model_only(31);
    assert_eq!(primary.snapshot_model_only(), queue_owned);
    let (loan, live) = primary.begin_live_loan_model_only(certificate).unwrap();

    let mut other = registry(37, 1);
    let foreign = transfer(&mut other, 37);
    let before = primary.snapshot_model_only();
    let _foreign = match primary.begin_live_loan_model_only(foreign) {
        Err((R44FoundationCertificateErrorV1::DuplicateLiveLoan, certificate)) => certificate,
        other => panic!("unexpected duplicate result: {other:?}"),
    };
    assert_eq!(primary.snapshot_model_only(), before);

    let stale = loan.substitute_generation_model_only(99);
    let (stale, live) = match primary.reclaim_live_loan_model_only(stale, live) {
        Err((R44FoundationCertificateErrorV1::StaleLiveLoan, loan, live)) => (loan, live),
        other => panic!("unexpected stale loan result: {other:?}"),
    };
    assert_eq!(primary.snapshot_model_only(), before);
    let loan = stale.substitute_generation_model_only(1);
    let cross = loan.substitute_session_model_only(41);
    let (cross, live) = match primary.reclaim_live_loan_model_only(cross, live) {
        Err((R44FoundationCertificateErrorV1::CrossSessionLoan, loan, live)) => (loan, live),
        other => panic!("unexpected cross-session result: {other:?}"),
    };
    assert_eq!(primary.snapshot_model_only(), before);
    let loan = cross.substitute_session_model_only(31);
    let _certificate = primary.reclaim_live_loan_model_only(loan, live).unwrap();
}

#[test]
fn generation_overflow_and_out_of_phase_reclaim_are_inert() {
    let mut overflow = registry(43, u64::MAX);
    let certificate = transfer(&mut overflow, 43);
    let before = overflow.snapshot_model_only();
    let _certificate = match overflow.begin_live_loan_model_only(certificate) {
        Err((R44FoundationCertificateErrorV1::LiveLoanGenerationExhausted, certificate)) => {
            certificate
        }
        other => panic!("unexpected generation result: {other:?}"),
    };
    assert_eq!(overflow.snapshot_model_only(), before);

    let mut active = registry(47, 1);
    let certificate = transfer(&mut active, 47);
    let (loan, live) = active.begin_live_loan_model_only(certificate).unwrap();
    let mut wrong = registry(47, 1);
    let before = wrong.snapshot_model_only();
    let (_loan, _live) = match wrong.reclaim_live_loan_model_only(loan, live) {
        Err((R44FoundationCertificateErrorV1::ReclaimOutOfPhase, loan, live)) => (loan, live),
        other => panic!("unexpected out-of-phase result: {other:?}"),
    };
    assert_eq!(wrong.snapshot_model_only(), before);
}

#[test]
fn final_restore_revalidates_and_consumes_the_certificate() {
    let mut registry = registry(53, 1);
    let certificate = transfer(&mut registry, 53);
    let invalid = certificate.substitute_revision_model_only(99);
    let before = registry.snapshot_model_only();
    let certificate =
        match registry.restore_to_session_model_only(invalid, validation(foundation(53), true)) {
            Err((R44FoundationCertificateErrorV1::InvalidCertificate, certificate)) => certificate,
            other => panic!("unexpected invalid certificate result: {other:?}"),
        }
        .substitute_revision_model_only(17);
    assert_eq!(registry.snapshot_model_only(), before);

    let certificate = match registry
        .restore_to_session_model_only(certificate, validation(foundation(53), false))
    {
        Err((R44FoundationCertificateErrorV1::FullValidationRejected, certificate)) => certificate,
        other => panic!("unexpected invalid validation result: {other:?}"),
    };
    assert_eq!(registry.snapshot_model_only(), before);
    registry
        .restore_to_session_model_only(certificate, validation(foundation(53), true))
        .unwrap();
    let after = registry.snapshot_model_only();
    assert_eq!(after.phase, R44FoundationOwnershipPhaseV1::SessionOwned);
    assert_eq!(after.full_validation_scans, 2);
    assert_eq!(after.certificate_identity, None);
    assert!(after.reusable_session_authority);
    assert!(!after.reusable_queue_authority);
}

#[test]
fn arbitrarily_many_valid_cycles_add_no_global_scans() {
    let mut registry = registry(59, 1);
    let mut certificate = transfer(&mut registry, 59);
    for _ in 0..4096 {
        let (loan, live) = registry.begin_live_loan_model_only(certificate).unwrap();
        certificate = registry.reclaim_live_loan_model_only(loan, live).unwrap();
    }
    let before_restore = registry.snapshot_model_only();
    assert_eq!(before_restore.full_validation_scans, 1);
    assert_eq!(before_restore.next_live_loan_generation, 4097);
    registry
        .restore_to_session_model_only(certificate, validation(foundation(59), true))
        .unwrap();
    assert_eq!(registry.snapshot_model_only().full_validation_scans, 2);
}

#[test]
fn valid_cycles_with_inductive_mutations_still_use_only_boundary_scans() {
    let mut registry = registry(61, 1);
    let mut certificate = transfer(&mut registry, 61);
    for (revision, offset) in (17_u64..).zip(0..128) {
        let (loan, live) = registry.begin_live_loan_model_only(certificate).unwrap();
        let (loan, live) = registry
            .apply_inductive_mutation_model_only(
                loan,
                live,
                R44InductiveFoundationMutationV1 {
                    kind: match offset % 7 {
                        0 => R44InductiveFoundationMutationKindV1::AcquireReservation,
                        1 => R44InductiveFoundationMutationKindV1::BindAllocation,
                        2 => R44InductiveFoundationMutationKindV1::MapAllocation,
                        3 => R44InductiveFoundationMutationKindV1::PublishAllocation,
                        4 => R44InductiveFoundationMutationKindV1::CompleteAllocation,
                        5 => R44InductiveFoundationMutationKindV1::UnmapAllocation,
                        _ => R44InductiveFoundationMutationKindV1::ReleaseAllocation,
                    },
                    ..mutation(61, revision, true)
                },
            )
            .unwrap();
        certificate = registry.reclaim_live_loan_model_only(loan, live).unwrap();
    }
    let snapshot = registry.snapshot_model_only();
    assert_eq!(snapshot.foundation.foundation_revision, 145);
    assert_eq!(snapshot.inductive_mutation_checks, 128);
    assert_eq!(snapshot.full_validation_scans, 1);
    registry
        .restore_to_session_model_only(certificate, validation(snapshot.foundation, true))
        .unwrap();
    assert_eq!(registry.snapshot_model_only().full_validation_scans, 2);
}
