//! Executable finite R44 model for a live-foundation invariant certificate.
//!
//! A queue certificate is minted only by an exact full validation at the
//! session-to-queue boundary. Each live mutation loan consumes that queue
//! certificate and produces exact session/generation loan custody plus a live
//! certificate. Admitted inductive mutations update the certified foundation;
//! rejected mutations return both owners without changing registry state.
//! Reclaim restores queue-certificate custody without a global scan. Final
//! lifecycle restore performs one exact full validation and consumes it.
//!
//! Registry occurrence uniqueness is a caller-supplied model premise. The
//! constructor checks only that it is nonzero; this model has no global issuer
//! allocator or production occurrence authority.
//!
//! Full-validation truth and inductive-preservation truth are caller-supplied
//! mathematical inputs. This model performs no production validation or I/O
//! and establishes no Rust refinement, panic, allocator, KFD, HSA, HIP,
//! hardware, progress, parity, or performance claim.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R44FoundationOwnershipPhaseV1 {
    SessionOwned,
    QueueOwned,
    SessionOwnedLiveLoan { generation: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R44FoundationIdentityV1 {
    pub session_occurrence: u64,
    pub domain_id: u64,
    pub device_key: u64,
    pub vm_key: u64,
    pub identity_generation: u64,
    pub memory_generation: u64,
    pub foundation_revision: u64,
}

impl R44FoundationIdentityV1 {
    pub const fn is_valid_model_only(self) -> bool {
        self.session_occurrence != 0
            && self.domain_id != 0
            && self.device_key != 0
            && self.vm_key != 0
            && self.identity_generation != 0
            && self.memory_generation != 0
            && self.foundation_revision != 0
    }
}

/// Contracted full-scan result; it is not native evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R44FullFoundationValidationV1 {
    pub foundation: R44FoundationIdentityV1,
    pub all_invariants_valid: bool,
}

impl R44FullFoundationValidationV1 {
    pub const fn new_model_only(
        foundation: R44FoundationIdentityV1,
        all_invariants_valid: bool,
    ) -> Self {
        Self {
            foundation,
            all_invariants_valid,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R44CertificateRecordV1 {
    issuer_occurrence: u64,
    certificate_id: u64,
    foundation: R44FoundationIdentityV1,
}

/// Move-only queue-owned certificate custody.
#[derive(Debug, Eq, PartialEq)]
pub struct R44QueueFoundationInvariantCertificateV1 {
    record: R44CertificateRecordV1,
}

impl R44QueueFoundationInvariantCertificateV1 {
    pub fn substitute_issuer_model_only(mut self, issuer_occurrence: u64) -> Self {
        self.record.issuer_occurrence = issuer_occurrence;
        self
    }

    pub fn substitute_session_model_only(mut self, session_occurrence: u64) -> Self {
        self.record.foundation.session_occurrence = session_occurrence;
        self
    }

    pub fn substitute_revision_model_only(mut self, foundation_revision: u64) -> Self {
        self.record.foundation.foundation_revision = foundation_revision;
        self
    }

    pub fn substitute_certificate_id_model_only(mut self, certificate_id: u64) -> Self {
        self.record.certificate_id = certificate_id;
        self
    }
}

/// Move-only exact session/generation authority for one live loan.
#[derive(Debug, Eq, PartialEq)]
pub struct R44LiveFoundationLoanV1 {
    session_occurrence: u64,
    generation: u64,
    issuer_occurrence: u64,
    certificate_id: u64,
}

impl R44LiveFoundationLoanV1 {
    pub fn substitute_session_model_only(mut self, session_occurrence: u64) -> Self {
        self.session_occurrence = session_occurrence;
        self
    }

    pub fn substitute_generation_model_only(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }
}

/// Move-only certificate custody while the session owns the live foundation.
#[derive(Debug, Eq, PartialEq)]
pub struct R44LiveFoundationInvariantCertificateV1 {
    record: R44CertificateRecordV1,
}

impl R44LiveFoundationInvariantCertificateV1 {
    pub fn substitute_revision_model_only(mut self, foundation_revision: u64) -> Self {
        self.record.foundation.foundation_revision = foundation_revision;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R44InductiveFoundationMutationKindV1 {
    AcquireReservation,
    BindAllocation,
    MapAllocation,
    PublishAllocation,
    CompleteAllocation,
    UnmapAllocation,
    ReleaseAllocation,
}

/// Contracted mutation check; `preserves_full_invariants` is not proved here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R44InductiveFoundationMutationV1 {
    pub session_occurrence: u64,
    pub issuer_occurrence: u64,
    pub certificate_id: u64,
    pub from_revision: u64,
    pub to_revision: u64,
    pub kind: R44InductiveFoundationMutationKindV1,
    pub preserves_full_invariants: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R44FoundationCertificateErrorV1 {
    InvalidRegistryOccurrence,
    InvalidFoundation,
    WrongPhase,
    FullValidationRejected,
    FoundationSubstitution,
    InvalidCertificate,
    DuplicateLiveLoan,
    CrossSessionLoan,
    StaleLiveLoan,
    LiveLoanGenerationExhausted,
    MutationOutOfPhase,
    NonInductiveMutation,
    StaleMutation,
    FoundationGenerationExhausted,
    ReclaimOutOfPhase,
    FullValidationScanExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R44FoundationCertificateSnapshotV1 {
    pub registry_occurrence: u64,
    pub phase: R44FoundationOwnershipPhaseV1,
    pub foundation: R44FoundationIdentityV1,
    pub next_live_loan_generation: u64,
    pub certificate_identity: Option<(u64, u64, u64)>,
    pub full_validation_scans: u64,
    pub inductive_mutation_checks: u64,
    pub reusable_queue_authority: bool,
    pub reusable_session_authority: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R44LiveFoundationCertificateRegistryV1 {
    registry_occurrence: u64,
    phase: R44FoundationOwnershipPhaseV1,
    foundation: R44FoundationIdentityV1,
    next_live_loan_generation: u64,
    certificate: Option<R44CertificateRecordV1>,
    full_validation_scans: u64,
    inductive_mutation_checks: u64,
}

impl R44LiveFoundationCertificateRegistryV1 {
    pub fn new_model_only(
        registry_occurrence: u64,
        foundation: R44FoundationIdentityV1,
        next_live_loan_generation: u64,
    ) -> Result<Self, R44FoundationCertificateErrorV1> {
        if registry_occurrence == 0 {
            return Err(R44FoundationCertificateErrorV1::InvalidRegistryOccurrence);
        }
        if !foundation.is_valid_model_only() || next_live_loan_generation == 0 {
            return Err(R44FoundationCertificateErrorV1::InvalidFoundation);
        }
        Ok(Self {
            registry_occurrence,
            phase: R44FoundationOwnershipPhaseV1::SessionOwned,
            foundation,
            next_live_loan_generation,
            certificate: None,
            full_validation_scans: 0,
            inductive_mutation_checks: 0,
        })
    }

    pub fn snapshot_model_only(&self) -> R44FoundationCertificateSnapshotV1 {
        R44FoundationCertificateSnapshotV1 {
            registry_occurrence: self.registry_occurrence,
            phase: self.phase,
            foundation: self.foundation,
            next_live_loan_generation: self.next_live_loan_generation,
            certificate_identity: self.certificate.map(|record| {
                (
                    record.issuer_occurrence,
                    record.certificate_id,
                    record.foundation.foundation_revision,
                )
            }),
            full_validation_scans: self.full_validation_scans,
            inductive_mutation_checks: self.inductive_mutation_checks,
            reusable_queue_authority: self.phase == R44FoundationOwnershipPhaseV1::QueueOwned
                && self.certificate.is_some(),
            reusable_session_authority: self.phase == R44FoundationOwnershipPhaseV1::SessionOwned
                && self.certificate.is_none(),
        }
    }

    pub fn transfer_to_queue_model_only(
        &mut self,
        validation: R44FullFoundationValidationV1,
    ) -> Result<R44QueueFoundationInvariantCertificateV1, R44FoundationCertificateErrorV1> {
        if self.phase != R44FoundationOwnershipPhaseV1::SessionOwned {
            return Err(R44FoundationCertificateErrorV1::WrongPhase);
        }
        if validation.foundation != self.foundation {
            return Err(R44FoundationCertificateErrorV1::FoundationSubstitution);
        }
        if !validation.all_invariants_valid {
            return Err(R44FoundationCertificateErrorV1::FullValidationRejected);
        }
        let next_scans = self
            .full_validation_scans
            .checked_add(1)
            .ok_or(R44FoundationCertificateErrorV1::FullValidationScanExhausted)?;
        let record = R44CertificateRecordV1 {
            issuer_occurrence: self.registry_occurrence,
            certificate_id: 1,
            foundation: self.foundation,
        };
        self.phase = R44FoundationOwnershipPhaseV1::QueueOwned;
        self.certificate = Some(record);
        self.full_validation_scans = next_scans;
        Ok(R44QueueFoundationInvariantCertificateV1 { record })
    }

    pub fn begin_live_loan_model_only(
        &mut self,
        certificate: R44QueueFoundationInvariantCertificateV1,
    ) -> Result<
        (
            R44LiveFoundationLoanV1,
            R44LiveFoundationInvariantCertificateV1,
        ),
        (
            R44FoundationCertificateErrorV1,
            R44QueueFoundationInvariantCertificateV1,
        ),
    > {
        if self.phase != R44FoundationOwnershipPhaseV1::QueueOwned {
            let error = if matches!(
                self.phase,
                R44FoundationOwnershipPhaseV1::SessionOwnedLiveLoan { .. }
            ) {
                R44FoundationCertificateErrorV1::DuplicateLiveLoan
            } else {
                R44FoundationCertificateErrorV1::WrongPhase
            };
            return Err((error, certificate));
        }
        if self.certificate != Some(certificate.record) {
            let error = if certificate.record.foundation.session_occurrence
                != self.foundation.session_occurrence
            {
                R44FoundationCertificateErrorV1::CrossSessionLoan
            } else {
                R44FoundationCertificateErrorV1::InvalidCertificate
            };
            return Err((error, certificate));
        }
        let generation = self.next_live_loan_generation;
        let Some(next_generation) = generation.checked_add(1) else {
            return Err((
                R44FoundationCertificateErrorV1::LiveLoanGenerationExhausted,
                certificate,
            ));
        };
        self.phase = R44FoundationOwnershipPhaseV1::SessionOwnedLiveLoan { generation };
        self.next_live_loan_generation = next_generation;
        let loan = R44LiveFoundationLoanV1 {
            session_occurrence: self.foundation.session_occurrence,
            generation,
            issuer_occurrence: certificate.record.issuer_occurrence,
            certificate_id: certificate.record.certificate_id,
        };
        Ok((
            loan,
            R44LiveFoundationInvariantCertificateV1 {
                record: certificate.record,
            },
        ))
    }

    pub fn apply_inductive_mutation_model_only(
        &mut self,
        loan: R44LiveFoundationLoanV1,
        live_certificate: R44LiveFoundationInvariantCertificateV1,
        mutation: R44InductiveFoundationMutationV1,
    ) -> Result<
        (
            R44LiveFoundationLoanV1,
            R44LiveFoundationInvariantCertificateV1,
        ),
        (
            R44FoundationCertificateErrorV1,
            R44LiveFoundationLoanV1,
            R44LiveFoundationInvariantCertificateV1,
        ),
    > {
        let expected_generation = match self.phase {
            R44FoundationOwnershipPhaseV1::SessionOwnedLiveLoan { generation } => generation,
            _ => {
                return Err((
                    R44FoundationCertificateErrorV1::MutationOutOfPhase,
                    loan,
                    live_certificate,
                ));
            }
        };
        if loan.session_occurrence != self.foundation.session_occurrence
            || live_certificate.record.foundation.session_occurrence
                != self.foundation.session_occurrence
            || mutation.session_occurrence != self.foundation.session_occurrence
        {
            return Err((
                R44FoundationCertificateErrorV1::CrossSessionLoan,
                loan,
                live_certificate,
            ));
        }
        if loan.generation != expected_generation {
            return Err((
                R44FoundationCertificateErrorV1::StaleLiveLoan,
                loan,
                live_certificate,
            ));
        }
        if self.certificate != Some(live_certificate.record)
            || loan.issuer_occurrence != self.registry_occurrence
            || mutation.issuer_occurrence != self.registry_occurrence
            || loan.certificate_id != live_certificate.record.certificate_id
            || mutation.certificate_id != live_certificate.record.certificate_id
        {
            return Err((
                R44FoundationCertificateErrorV1::InvalidCertificate,
                loan,
                live_certificate,
            ));
        }
        if !mutation.preserves_full_invariants {
            return Err((
                R44FoundationCertificateErrorV1::NonInductiveMutation,
                loan,
                live_certificate,
            ));
        }
        if mutation.from_revision != self.foundation.foundation_revision {
            return Err((
                R44FoundationCertificateErrorV1::StaleMutation,
                loan,
                live_certificate,
            ));
        }
        let Some(next_revision) = mutation.from_revision.checked_add(1) else {
            return Err((
                R44FoundationCertificateErrorV1::FoundationGenerationExhausted,
                loan,
                live_certificate,
            ));
        };
        if mutation.to_revision != next_revision {
            return Err((
                R44FoundationCertificateErrorV1::StaleMutation,
                loan,
                live_certificate,
            ));
        }
        let Some(next_checks) = self.inductive_mutation_checks.checked_add(1) else {
            return Err((
                R44FoundationCertificateErrorV1::FoundationGenerationExhausted,
                loan,
                live_certificate,
            ));
        };
        self.foundation.foundation_revision = next_revision;
        let record = R44CertificateRecordV1 {
            issuer_occurrence: live_certificate.record.issuer_occurrence,
            certificate_id: live_certificate.record.certificate_id,
            foundation: self.foundation,
        };
        self.certificate = Some(record);
        self.inductive_mutation_checks = next_checks;
        Ok((loan, R44LiveFoundationInvariantCertificateV1 { record }))
    }

    pub fn reclaim_live_loan_model_only(
        &mut self,
        loan: R44LiveFoundationLoanV1,
        live_certificate: R44LiveFoundationInvariantCertificateV1,
    ) -> Result<
        R44QueueFoundationInvariantCertificateV1,
        (
            R44FoundationCertificateErrorV1,
            R44LiveFoundationLoanV1,
            R44LiveFoundationInvariantCertificateV1,
        ),
    > {
        let expected_generation = match self.phase {
            R44FoundationOwnershipPhaseV1::SessionOwnedLiveLoan { generation } => generation,
            _ => {
                return Err((
                    R44FoundationCertificateErrorV1::ReclaimOutOfPhase,
                    loan,
                    live_certificate,
                ));
            }
        };
        if loan.session_occurrence != self.foundation.session_occurrence
            || live_certificate.record.foundation.session_occurrence
                != self.foundation.session_occurrence
        {
            return Err((
                R44FoundationCertificateErrorV1::CrossSessionLoan,
                loan,
                live_certificate,
            ));
        }
        if loan.generation != expected_generation {
            return Err((
                R44FoundationCertificateErrorV1::StaleLiveLoan,
                loan,
                live_certificate,
            ));
        }
        if self.certificate != Some(live_certificate.record)
            || loan.issuer_occurrence != self.registry_occurrence
            || loan.certificate_id != live_certificate.record.certificate_id
        {
            return Err((
                R44FoundationCertificateErrorV1::InvalidCertificate,
                loan,
                live_certificate,
            ));
        }
        self.phase = R44FoundationOwnershipPhaseV1::QueueOwned;
        Ok(R44QueueFoundationInvariantCertificateV1 {
            record: live_certificate.record,
        })
    }

    pub fn restore_to_session_model_only(
        &mut self,
        certificate: R44QueueFoundationInvariantCertificateV1,
        validation: R44FullFoundationValidationV1,
    ) -> Result<
        (),
        (
            R44FoundationCertificateErrorV1,
            R44QueueFoundationInvariantCertificateV1,
        ),
    > {
        if self.phase != R44FoundationOwnershipPhaseV1::QueueOwned {
            return Err((R44FoundationCertificateErrorV1::WrongPhase, certificate));
        }
        if self.certificate != Some(certificate.record) {
            return Err((
                R44FoundationCertificateErrorV1::InvalidCertificate,
                certificate,
            ));
        }
        if validation.foundation != self.foundation {
            return Err((
                R44FoundationCertificateErrorV1::FoundationSubstitution,
                certificate,
            ));
        }
        if !validation.all_invariants_valid {
            return Err((
                R44FoundationCertificateErrorV1::FullValidationRejected,
                certificate,
            ));
        }
        let Some(next_scans) = self.full_validation_scans.checked_add(1) else {
            return Err((
                R44FoundationCertificateErrorV1::FullValidationScanExhausted,
                certificate,
            ));
        };
        self.phase = R44FoundationOwnershipPhaseV1::SessionOwned;
        self.certificate = None;
        self.full_validation_scans = next_scans;
        Ok(())
    }
}
