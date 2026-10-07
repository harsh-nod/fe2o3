// Independent finite R44 summary model for a live-foundation invariant
// certificate. Full-validation and inductive-preservation truth are contracted
// mathematical inputs. Registry-occurrence uniqueness is also a contracted
// premise; the model proves only that a nonzero occurrence is checked exactly.
// The model proves no Rust ownership/refinement, panic, allocator, KFD, HSA,
// HIP, ioctl, native, hardware, progress, parity, or performance property.

use vstd::prelude::*;

verus! {

pub open spec fn maximum_generation_v1() -> nat { 18_446_744_073_709_551_615 }

#[derive(PartialEq, Eq)]
pub enum OwnershipPhaseV1 {
    SessionOwned,
    QueueOwned,
    SessionOwnedLiveLoan { generation: nat },
}

#[derive(PartialEq, Eq)]
pub struct FoundationV1 {
    pub session: nat,
    pub domain: nat,
    pub device: nat,
    pub vm: nat,
    pub identity_generation: nat,
    pub memory_generation: nat,
    pub revision: nat,
}

#[derive(PartialEq, Eq)]
pub struct CertificateV1 {
    pub issuer_occurrence: nat,
    pub certificate_id: nat,
    pub foundation: FoundationV1,
}

#[derive(PartialEq, Eq)]
pub struct LoanV1 {
    pub session: nat,
    pub generation: nat,
    pub issuer_occurrence: nat,
    pub certificate_id: nat,
}

// This value reports a contracted global scan; it is not native evidence.
#[derive(PartialEq, Eq)]
pub struct FullValidationV1 {
    pub foundation: FoundationV1,
    pub all_invariants_valid: bool,
}

// `preserves_full_invariants` is a contracted inductive check, not a theorem
// about the production allocation lifecycle.
#[derive(PartialEq, Eq)]
pub struct InductiveMutationV1 {
    pub session: nat,
    pub issuer_occurrence: nat,
    pub certificate_id: nat,
    pub from_revision: nat,
    pub to_revision: nat,
    pub preserves_full_invariants: bool,
}

#[derive(PartialEq, Eq)]
pub struct RegistryV1 {
    pub registry_occurrence: nat,
    pub phase: OwnershipPhaseV1,
    pub foundation: FoundationV1,
    pub next_loan_generation: nat,
    pub certificate: Option<CertificateV1>,
    pub full_validation_scans: nat,
    pub inductive_mutation_checks: nat,
}

pub open spec fn valid_foundation_v1(foundation: FoundationV1) -> bool {
    foundation.session > 0
        && foundation.domain > 0
        && foundation.device > 0
        && foundation.vm > 0
        && 0 < foundation.identity_generation <= maximum_generation_v1()
        && 0 < foundation.memory_generation <= maximum_generation_v1()
        && 0 < foundation.revision <= maximum_generation_v1()
}

pub open spec fn certificate_matches_v1(state: RegistryV1, certificate: CertificateV1) -> bool {
    certificate.issuer_occurrence == state.registry_occurrence
        && certificate.certificate_id > 0
        && certificate.foundation == state.foundation
        && state.certificate == Some(certificate)
}

pub open spec fn valid_state_v1(state: RegistryV1) -> bool {
    state.registry_occurrence > 0
        && valid_foundation_v1(state.foundation)
        && 0 < state.next_loan_generation <= maximum_generation_v1()
        && match state.phase {
            OwnershipPhaseV1::SessionOwned => state.certificate.is_none(),
            OwnershipPhaseV1::QueueOwned => match state.certificate {
                Some(certificate) => certificate.certificate_id > 0
                    && certificate.issuer_occurrence == state.registry_occurrence
                    && certificate.foundation == state.foundation,
                None => false,
            },
            OwnershipPhaseV1::SessionOwnedLiveLoan { generation } =>
                generation > 0
                    && generation + 1 == state.next_loan_generation
                    && match state.certificate {
                        Some(certificate) => certificate.certificate_id > 0
                            && certificate.issuer_occurrence == state.registry_occurrence
                            && certificate.foundation == state.foundation,
                        None => false,
                    },
        }
}

pub open spec fn initial_state_v1(
    registry_occurrence: nat,
    foundation: FoundationV1,
    next_generation: nat,
) -> RegistryV1 {
    RegistryV1 {
        registry_occurrence,
        phase: OwnershipPhaseV1::SessionOwned,
        foundation,
        next_loan_generation: next_generation,
        certificate: None,
        full_validation_scans: 0,
        inductive_mutation_checks: 0,
    }
}

pub open spec fn validation_is_exact_v1(
    state: RegistryV1,
    validation: FullValidationV1,
) -> bool {
    validation.all_invariants_valid && validation.foundation == state.foundation
}

pub open spec fn minted_certificate_v1(state: RegistryV1) -> CertificateV1 {
    CertificateV1 {
        issuer_occurrence: state.registry_occurrence,
        certificate_id: 1,
        foundation: state.foundation,
    }
}

pub open spec fn transfer_to_queue_or_reject_v1(
    state: RegistryV1,
    validation: FullValidationV1,
) -> RegistryV1 {
    if valid_state_v1(state)
        && state.phase == OwnershipPhaseV1::SessionOwned
        && validation_is_exact_v1(state, validation)
    {
        RegistryV1 {
            registry_occurrence: state.registry_occurrence,
            phase: OwnershipPhaseV1::QueueOwned,
            foundation: state.foundation,
            next_loan_generation: state.next_loan_generation,
            certificate: Some(minted_certificate_v1(state)),
            full_validation_scans: state.full_validation_scans + 1,
            inductive_mutation_checks: state.inductive_mutation_checks,
        }
    } else {
        state
    }
}

pub open spec fn loan_for_v1(state: RegistryV1, certificate: CertificateV1) -> LoanV1 {
    LoanV1 {
        session: state.foundation.session,
        generation: state.next_loan_generation,
        issuer_occurrence: certificate.issuer_occurrence,
        certificate_id: certificate.certificate_id,
    }
}

pub open spec fn begin_live_loan_or_reject_v1(
    state: RegistryV1,
    certificate: CertificateV1,
) -> RegistryV1 {
    if valid_state_v1(state)
        && state.phase == OwnershipPhaseV1::QueueOwned
        && certificate_matches_v1(state, certificate)
        && state.next_loan_generation < maximum_generation_v1()
    {
        RegistryV1 {
            registry_occurrence: state.registry_occurrence,
            phase: OwnershipPhaseV1::SessionOwnedLiveLoan {
                generation: state.next_loan_generation,
            },
            foundation: state.foundation,
            next_loan_generation: state.next_loan_generation + 1,
            certificate: state.certificate,
            full_validation_scans: state.full_validation_scans,
            inductive_mutation_checks: state.inductive_mutation_checks,
        }
    } else {
        state
    }
}

pub open spec fn loan_is_exact_v1(
    state: RegistryV1,
    loan: LoanV1,
    live_certificate: CertificateV1,
) -> bool {
    match state.phase {
        OwnershipPhaseV1::SessionOwnedLiveLoan { generation } =>
            loan.session == state.foundation.session
                && loan.generation == generation
                && loan.issuer_occurrence == state.registry_occurrence
                && loan.certificate_id == live_certificate.certificate_id
                && certificate_matches_v1(state, live_certificate),
        _ => false,
    }
}

pub open spec fn reclaim_live_loan_or_reject_v1(
    state: RegistryV1,
    loan: LoanV1,
    live_certificate: CertificateV1,
) -> RegistryV1 {
    if valid_state_v1(state) && loan_is_exact_v1(state, loan, live_certificate) {
        RegistryV1 {
            registry_occurrence: state.registry_occurrence,
            phase: OwnershipPhaseV1::QueueOwned,
            foundation: state.foundation,
            next_loan_generation: state.next_loan_generation,
            certificate: state.certificate,
            full_validation_scans: state.full_validation_scans,
            inductive_mutation_checks: state.inductive_mutation_checks,
        }
    } else {
        state
    }
}

pub open spec fn mutation_is_admitted_v1(
    state: RegistryV1,
    loan: LoanV1,
    live_certificate: CertificateV1,
    mutation: InductiveMutationV1,
) -> bool {
    valid_state_v1(state)
        && loan_is_exact_v1(state, loan, live_certificate)
        && mutation.session == state.foundation.session
        && mutation.issuer_occurrence == state.registry_occurrence
        && mutation.certificate_id == live_certificate.certificate_id
        && mutation.from_revision == state.foundation.revision
        && mutation.from_revision < maximum_generation_v1()
        && mutation.to_revision == mutation.from_revision + 1
        && mutation.preserves_full_invariants
}

pub open spec fn foundation_after_mutation_v1(
    foundation: FoundationV1,
    mutation: InductiveMutationV1,
) -> FoundationV1 {
    FoundationV1 { revision: mutation.to_revision, ..foundation }
}

pub open spec fn apply_mutation_or_reject_v1(
    state: RegistryV1,
    loan: LoanV1,
    live_certificate: CertificateV1,
    mutation: InductiveMutationV1,
) -> RegistryV1 {
    if mutation_is_admitted_v1(state, loan, live_certificate, mutation) {
        let foundation = foundation_after_mutation_v1(state.foundation, mutation);
        RegistryV1 {
            registry_occurrence: state.registry_occurrence,
            phase: state.phase,
            foundation,
            next_loan_generation: state.next_loan_generation,
            certificate: Some(CertificateV1 {
                issuer_occurrence: live_certificate.issuer_occurrence,
                certificate_id: live_certificate.certificate_id,
                foundation,
            }),
            full_validation_scans: state.full_validation_scans,
            inductive_mutation_checks: state.inductive_mutation_checks + 1,
        }
    } else {
        state
    }
}

pub open spec fn restore_to_session_or_reject_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    validation: FullValidationV1,
) -> RegistryV1 {
    if valid_state_v1(state)
        && state.phase == OwnershipPhaseV1::QueueOwned
        && certificate_matches_v1(state, certificate)
        && validation_is_exact_v1(state, validation)
    {
        RegistryV1 {
            registry_occurrence: state.registry_occurrence,
            phase: OwnershipPhaseV1::SessionOwned,
            foundation: state.foundation,
            next_loan_generation: state.next_loan_generation,
            certificate: None,
            full_validation_scans: state.full_validation_scans + 1,
            inductive_mutation_checks: state.inductive_mutation_checks,
        }
    } else {
        state
    }
}

pub open spec fn reusable_queue_authority_v1(state: RegistryV1) -> bool {
    state.phase == OwnershipPhaseV1::QueueOwned && state.certificate.is_some()
}

pub open spec fn reusable_session_authority_v1(state: RegistryV1) -> bool {
    state.phase == OwnershipPhaseV1::SessionOwned && state.certificate.is_none()
}

// Recursive composition of any finite number of loan/reclaim cycles.
pub open spec fn run_valid_cycles_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    cycles: nat,
) -> RegistryV1
    decreases cycles,
{
    if cycles == 0 {
        state
    } else {
        let loan = loan_for_v1(state, certificate);
        let loaned = begin_live_loan_or_reject_v1(state, certificate);
        let reclaimed = reclaim_live_loan_or_reject_v1(loaned, loan, certificate);
        run_valid_cycles_v1(reclaimed, certificate, (cycles - 1) as nat)
    }
}

// Obligation 1: the generation bound is exact.
pub proof fn generation_bound_is_exact_v1()
    ensures maximum_generation_v1() == 18_446_744_073_709_551_615,
{}

// Obligation 2: a valid initial foundation begins session-owned and uncertified.
pub proof fn initial_foundation_is_session_owned_v1(
    registry_occurrence: nat,
    foundation: FoundationV1,
    next_generation: nat,
)
    requires
        registry_occurrence > 0,
        valid_foundation_v1(foundation),
        0 < next_generation <= maximum_generation_v1(),
    ensures {
        let state = initial_state_v1(registry_occurrence, foundation, next_generation);
        &&& valid_state_v1(state)
        &&& reusable_session_authority_v1(state)
        &&& !reusable_queue_authority_v1(state)
        &&& state.full_validation_scans == 0
    },
{}

// Obligation 3: exact full validation mints the first queue certificate.
pub proof fn exact_transfer_mints_certificate_v1(
    state: RegistryV1,
    validation: FullValidationV1,
)
    requires
        valid_state_v1(state),
        state.phase == OwnershipPhaseV1::SessionOwned,
        validation_is_exact_v1(state, validation),
    ensures {
        let transferred = transfer_to_queue_or_reject_v1(state, validation);
        &&& transferred.phase == OwnershipPhaseV1::QueueOwned
        &&& transferred.certificate == Some(minted_certificate_v1(state))
        &&& transferred.full_validation_scans == state.full_validation_scans + 1
        &&& valid_state_v1(transferred)
    },
{}

// Obligation 4: an unvalidated transfer changes nothing and mints nothing.
pub proof fn unvalidated_transfer_is_rejected_v1(
    state: RegistryV1,
    validation: FullValidationV1,
)
    requires !validation.all_invariants_valid,
    ensures transfer_to_queue_or_reject_v1(state, validation) == state,
{}

// Obligation 5: a substituted foundation validation changes nothing.
pub proof fn substituted_transfer_is_rejected_v1(
    state: RegistryV1,
    validation: FullValidationV1,
)
    requires validation.foundation != state.foundation,
    ensures transfer_to_queue_or_reject_v1(state, validation) == state,
{}

// Obligation 6: an exact queue certificate opens one exact generation loan.
pub proof fn exact_live_loan_is_linear_v1(state: RegistryV1, certificate: CertificateV1)
    requires
        valid_state_v1(state),
        state.phase == OwnershipPhaseV1::QueueOwned,
        certificate_matches_v1(state, certificate),
        state.next_loan_generation < maximum_generation_v1(),
    ensures {
        let loaned = begin_live_loan_or_reject_v1(state, certificate);
        let loan = loan_for_v1(state, certificate);
        &&& loaned.phase == OwnershipPhaseV1::SessionOwnedLiveLoan {
            generation: state.next_loan_generation,
        }
        &&& loaned.next_loan_generation == state.next_loan_generation + 1
        &&& loan_is_exact_v1(loaned, loan, certificate)
        &&& loaned.full_validation_scans == state.full_validation_scans
        &&& valid_state_v1(loaned)
    },
{}

// Obligation 7: a second loan while one is live is failure atomic.
pub proof fn duplicate_live_loan_is_rejected_v1(state: RegistryV1, certificate: CertificateV1)
    requires matches!(state.phase, OwnershipPhaseV1::SessionOwnedLiveLoan { .. }),
    ensures begin_live_loan_or_reject_v1(state, certificate) == state,
{}

// Obligation 8: loan-generation exhaustion is failure atomic.
pub proof fn loan_generation_overflow_is_rejected_v1(state: RegistryV1, certificate: CertificateV1)
    requires state.next_loan_generation == maximum_generation_v1(),
    ensures begin_live_loan_or_reject_v1(state, certificate) == state,
{}

// Obligation 9: a cross-session loan cannot reclaim the foundation.
pub proof fn cross_session_reclaim_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
)
    requires loan.session != state.foundation.session,
    ensures reclaim_live_loan_or_reject_v1(state, loan, certificate) == state,
{}

// Additional obligation: a foreign registry issuer cannot open or restore
// custody even when every foundation field is identical.
pub proof fn cross_registry_certificate_is_rejected_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    validation: FullValidationV1,
)
    requires certificate.issuer_occurrence != state.registry_occurrence,
    ensures
        begin_live_loan_or_reject_v1(state, certificate) == state,
        restore_to_session_or_reject_v1(state, certificate, validation) == state,
{}

// Obligation 10: a stale generation cannot reclaim the foundation.
pub proof fn stale_generation_reclaim_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    live_generation: nat,
)
    requires
        state.phase == (OwnershipPhaseV1::SessionOwnedLiveLoan {
            generation: live_generation,
        }),
        loan.generation != live_generation,
    ensures reclaim_live_loan_or_reject_v1(state, loan, certificate) == state,
{}

// Obligation 11: reclaim outside the live-loan phase is failure atomic.
pub proof fn reclaim_out_of_phase_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
)
    requires !matches!(state.phase, OwnershipPhaseV1::SessionOwnedLiveLoan { .. }),
    ensures reclaim_live_loan_or_reject_v1(state, loan, certificate) == state,
{}

// Obligation 12: exact reclaim restores queue custody with no global scan.
pub proof fn exact_reclaim_restores_queue_certificate_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
)
    requires valid_state_v1(state), loan_is_exact_v1(state, loan, certificate),
    ensures {
        let reclaimed = reclaim_live_loan_or_reject_v1(state, loan, certificate);
        &&& reclaimed.phase == OwnershipPhaseV1::QueueOwned
        &&& reclaimed.certificate == state.certificate
        &&& reclaimed.full_validation_scans == state.full_validation_scans
        &&& valid_state_v1(reclaimed)
    },
{}

// Obligation 13: an admitted inductive mutation updates both exact revisions.
pub proof fn admitted_mutation_preserves_certificate_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires mutation_is_admitted_v1(state, loan, certificate, mutation),
    ensures {
        let changed = apply_mutation_or_reject_v1(state, loan, certificate, mutation);
        &&& changed.foundation.revision == mutation.to_revision
        &&& changed.certificate == Some(CertificateV1 {
            issuer_occurrence: certificate.issuer_occurrence,
            certificate_id: certificate.certificate_id,
            foundation: changed.foundation,
        })
        &&& changed.inductive_mutation_checks == state.inductive_mutation_checks + 1
        &&& valid_state_v1(changed)
    },
{}

// Obligation 14: admitted local mutation performs no repeated global scan.
pub proof fn admitted_mutation_preserves_scan_count_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires mutation_is_admitted_v1(state, loan, certificate, mutation),
    ensures apply_mutation_or_reject_v1(
        state, loan, certificate, mutation,
    ).full_validation_scans == state.full_validation_scans,
{}

// Obligation 15: noninductive mutation leaves live custody unchanged and
// cannot directly produce reusable queue or session authority.
pub proof fn noninductive_mutation_cannot_yield_reusable_authority_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires
        valid_state_v1(state),
        matches!(state.phase, OwnershipPhaseV1::SessionOwnedLiveLoan { .. }),
        !mutation.preserves_full_invariants,
    ensures {
        let rejected = apply_mutation_or_reject_v1(state, loan, certificate, mutation);
        &&& rejected == state
        &&& !reusable_queue_authority_v1(rejected)
        &&& !reusable_session_authority_v1(rejected)
    },
{}

// Obligation 16: a stale mutation revision is failure atomic.
pub proof fn stale_mutation_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires mutation.from_revision != state.foundation.revision,
    ensures apply_mutation_or_reject_v1(state, loan, certificate, mutation) == state,
{}

// Obligation 17: a substituted mutation session is failure atomic.
pub proof fn substituted_mutation_session_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires mutation.session != state.foundation.session,
    ensures apply_mutation_or_reject_v1(state, loan, certificate, mutation) == state,
{}

// Obligation 18: a substituted live certificate is failure atomic.
pub proof fn substituted_live_certificate_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires !certificate_matches_v1(state, certificate),
    ensures apply_mutation_or_reject_v1(state, loan, certificate, mutation) == state,
{}

// Obligation 19: revision overflow cannot be certified.
pub proof fn mutation_generation_overflow_is_rejected_v1(
    state: RegistryV1,
    loan: LoanV1,
    certificate: CertificateV1,
    mutation: InductiveMutationV1,
)
    requires state.foundation.revision == maximum_generation_v1(),
    ensures apply_mutation_or_reject_v1(state, loan, certificate, mutation) == state,
{}

// Obligation 20: exact final validation consumes the queue certificate.
pub proof fn exact_restore_validates_and_consumes_certificate_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    validation: FullValidationV1,
)
    requires
        valid_state_v1(state),
        state.phase == OwnershipPhaseV1::QueueOwned,
        certificate_matches_v1(state, certificate),
        validation_is_exact_v1(state, validation),
    ensures {
        let restored = restore_to_session_or_reject_v1(state, certificate, validation);
        &&& restored.phase == OwnershipPhaseV1::SessionOwned
        &&& restored.certificate.is_none()
        &&& restored.full_validation_scans == state.full_validation_scans + 1
        &&& reusable_session_authority_v1(restored)
        &&& !reusable_queue_authority_v1(restored)
        &&& valid_state_v1(restored)
    },
{}

// Obligation 21: restore without the exact certificate cannot yield session authority.
pub proof fn restore_without_valid_certificate_is_rejected_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    validation: FullValidationV1,
)
    requires
        state.phase == OwnershipPhaseV1::QueueOwned,
        !certificate_matches_v1(state, certificate),
    ensures {
        let rejected = restore_to_session_or_reject_v1(state, certificate, validation);
        &&& rejected == state
        &&& !reusable_session_authority_v1(rejected)
    },
{}

// Obligation 22: final invalid or substituted validation is failure atomic.
pub proof fn invalid_final_validation_is_rejected_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    validation: FullValidationV1,
)
    requires !validation_is_exact_v1(state, validation),
    ensures restore_to_session_or_reject_v1(state, certificate, validation) == state,
{}

// Obligation 23: one exact begin/reclaim pair adds zero global scans.
pub proof fn one_cycle_adds_zero_global_scans_v1(state: RegistryV1, certificate: CertificateV1)
    requires
        valid_state_v1(state),
        state.phase == OwnershipPhaseV1::QueueOwned,
        certificate_matches_v1(state, certificate),
        state.next_loan_generation < maximum_generation_v1(),
    ensures {
        let loan = loan_for_v1(state, certificate);
        let loaned = begin_live_loan_or_reject_v1(state, certificate);
        let reclaimed = reclaim_live_loan_or_reject_v1(loaned, loan, certificate);
        &&& reclaimed.phase == OwnershipPhaseV1::QueueOwned
        &&& reclaimed.full_validation_scans == state.full_validation_scans
        &&& reclaimed.next_loan_generation == state.next_loan_generation + 1
        &&& reclaimed.certificate == state.certificate
        &&& reclaimed.foundation == state.foundation
        &&& valid_state_v1(reclaimed)
    },
{}

// Obligation 24: any finite valid-cycle summary adds zero global scans.
pub proof fn arbitrary_valid_cycles_add_zero_global_scans_v1(
    state: RegistryV1,
    certificate: CertificateV1,
    cycles: nat,
)
    requires
        valid_state_v1(state),
        state.phase == OwnershipPhaseV1::QueueOwned,
        certificate_matches_v1(state, certificate),
        state.next_loan_generation + cycles <= maximum_generation_v1(),
    ensures {
        let cycled = run_valid_cycles_v1(state, certificate, cycles);
        &&& cycled.phase == OwnershipPhaseV1::QueueOwned
        &&& cycled.full_validation_scans == state.full_validation_scans
        &&& cycled.next_loan_generation == state.next_loan_generation + cycles
        &&& cycled.certificate == state.certificate
        &&& valid_state_v1(cycled)
    },
    decreases cycles,
{
    if cycles > 0 {
        let loan = loan_for_v1(state, certificate);
        let loaned = begin_live_loan_or_reject_v1(state, certificate);
        let reclaimed = reclaim_live_loan_or_reject_v1(loaned, loan, certificate);
        one_cycle_adds_zero_global_scans_v1(state, certificate);
        assert(certificate_matches_v1(reclaimed, certificate));
        assert(reclaimed.next_loan_generation + (cycles - 1) as nat
            <= maximum_generation_v1());
        arbitrary_valid_cycles_add_zero_global_scans_v1(
            reclaimed,
            certificate,
            (cycles - 1) as nat,
        );
    }
}

// Obligation 25: transfer, any valid cycles, then restore uses exactly two scans.
pub proof fn lifecycle_uses_exactly_trust_boundary_scans_v1(
    initial: RegistryV1,
    opening: FullValidationV1,
    closing: FullValidationV1,
    cycles: nat,
)
    requires
        valid_state_v1(initial),
        initial.phase == OwnershipPhaseV1::SessionOwned,
        initial.full_validation_scans == 0,
        validation_is_exact_v1(initial, opening),
        closing.all_invariants_valid,
        closing.foundation == initial.foundation,
        initial.next_loan_generation + cycles <= maximum_generation_v1(),
    ensures {
        let transferred = transfer_to_queue_or_reject_v1(initial, opening);
        let certificate = minted_certificate_v1(initial);
        let cycled = run_valid_cycles_v1(transferred, certificate, cycles);
        let restored = restore_to_session_or_reject_v1(cycled, certificate, closing);
        &&& transferred.full_validation_scans == 1
        &&& cycled.full_validation_scans == 1
        &&& restored.full_validation_scans == 2
        &&& restored.phase == OwnershipPhaseV1::SessionOwned
        &&& restored.certificate.is_none()
    },
{
    let transferred = transfer_to_queue_or_reject_v1(initial, opening);
    let certificate = minted_certificate_v1(initial);
    assert(certificate_matches_v1(transferred, certificate));
    assert(valid_state_v1(transferred));
    arbitrary_valid_cycles_add_zero_global_scans_v1(transferred, certificate, cycles);
}

}
