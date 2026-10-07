//! Independent executable R30 model for a bound host-content certificate.
//!
//! All identities, digests, completion observations, currentness results, and
//! mutation reports are caller-constructed model inputs. In particular, this
//! module does not implement or verify SHA-256, coherent CPU writes, DMA/HSA
//! completion, cache visibility, or currentness. It performs no I/O and does
//! not refine executable Rust, the production runtime, KFD, HSA, HIP, firmware,
//! or hardware. A production consumer must separately establish those contracts
//! and prove a refinement into these transitions.

use crate::IdentityDigestV1;

pub const R30_BOUND_HOST_CONTENT_CERTIFICATE_SCHEMA_VERSION_V1: u16 = 1;
pub const R30_MAX_HOST_EXTENT_BYTES_V1: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30BoundHostStorageV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub storage_id: u64,
    pub storage_generation: u64,
    pub pool_generation: u64,
    pub logical_extent_bytes: u64,
    pub physical_extent_bytes: u64,
}

impl R30BoundHostStorageV1 {
    pub const fn is_valid(self) -> bool {
        self.queue_id != 0
            && self.queue_generation != 0
            && self.storage_id != 0
            && self.storage_generation != 0
            && self.pool_generation != 0
            && self.logical_extent_bytes != 0
            && self.logical_extent_bytes <= R30_MAX_HOST_EXTENT_BYTES_V1
            && self.physical_extent_bytes != 0
            && self.physical_extent_bytes <= R30_MAX_HOST_EXTENT_BYTES_V1
            && self.logical_extent_bytes <= self.physical_extent_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30HostStorageRangeV1 {
    pub logical_offset: u64,
    pub logical_bytes: u64,
    pub physical_offset: u64,
    pub physical_bytes: u64,
}

impl R30HostStorageRangeV1 {
    pub const fn is_exact_full_extent(self, storage: R30BoundHostStorageV1) -> bool {
        self.logical_offset == 0
            && self.logical_bytes == storage.logical_extent_bytes
            && self.physical_offset == 0
            && self.physical_bytes == storage.physical_extent_bytes
            && self.logical_bytes == self.physical_bytes
    }

    pub fn is_nonempty_in_bounds(self, storage: R30BoundHostStorageV1) -> bool {
        self.logical_bytes != 0
            && self.physical_bytes != 0
            && matches!(
                self.logical_offset.checked_add(self.logical_bytes),
                Some(end) if end <= storage.logical_extent_bytes
            )
            && matches!(
                self.physical_offset.checked_add(self.physical_bytes),
                Some(end) if end <= storage.physical_extent_bytes
            )
    }
}

/// Opaque digest bytes asserted by a future SHA-256 evidence layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30HostContentDigestV1(pub IdentityDigestV1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30BoundHostContentCertificateV1 {
    pub storage: R30BoundHostStorageV1,
    pub full_range: R30HostStorageRangeV1,
    pub digest: R30HostContentDigestV1,
}

impl R30BoundHostContentCertificateV1 {
    pub fn is_exact_for(self, storage: R30BoundHostStorageV1) -> bool {
        self.storage == storage && self.full_range.is_exact_full_extent(storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30FullH2dCompletionV1 {
    pub completion_generation: u64,
    pub storage: R30BoundHostStorageV1,
    pub full_range: R30HostStorageRangeV1,
}

impl R30FullH2dCompletionV1 {
    pub fn is_exact_for(self, storage: R30BoundHostStorageV1) -> bool {
        self.storage == storage && self.full_range.is_exact_full_extent(storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30ReadyContentV1 {
    pub completion_generation: u64,
    pub digest: R30HostContentDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30TerminalPromotionCustodyV1 {
    pub completion: R30FullH2dCompletionV1,
    pub stored_certificate: Option<R30BoundHostContentCertificateV1>,
    pub stage: R30TerminalPromotionStageV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30TerminalPromotionStageV1 {
    OpeningCurrentnessAmbiguous,
    ClosingCurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30CertificatePhaseV1 {
    Host,
    FullH2dCompleted,
    Ready,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30ContractedCurrentnessV1 {
    Current,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30CurrentnessEnvelopeV1 {
    pub opening: R30ContractedCurrentnessV1,
    pub closing: R30ContractedCurrentnessV1,
}

impl R30CurrentnessEnvelopeV1 {
    pub const fn all_current() -> Self {
        Self {
            opening: R30ContractedCurrentnessV1::Current,
            closing: R30ContractedCurrentnessV1::Current,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30HostMutationKindV1 {
    CpuDestinationWrite,
    SdmaDestinationWrite,
    Resize,
    Recycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30MutationOrderingV1 {
    pub kind: R30HostMutationKindV1,
    pub invalidation_step: u64,
    pub possible_mutation_step: u64,
}

impl R30MutationOrderingV1 {
    pub const fn invalidated_before_possible_mutation(self) -> bool {
        self.invalidation_step < self.possible_mutation_step
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30PromotionOutcomeV1 {
    Ready,
    RetryableNoEffect,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R30BoundHostContentCertificateErrorV1 {
    InvalidStorage,
    InvalidRange,
    InvalidGeneration,
    IllegalPhase,
    CurrentnessAmbiguous,
    TransitionClockExhausted,
    TerminalAbsorbed,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R30BoundHostContentCertificateSnapshotV1 {
    pub storage: R30BoundHostStorageV1,
    pub phase: R30CertificatePhaseV1,
    pub certificate: Option<R30BoundHostContentCertificateV1>,
    pub pending_completion: Option<R30FullH2dCompletionV1>,
    pub ready: Option<R30ReadyContentV1>,
    pub terminal_custody: Option<R30TerminalPromotionCustodyV1>,
    pub retired_completion_generation: u64,
    pub transition_clock: u64,
    pub last_certificate_invalidation_step: Option<u64>,
    pub last_mutation_ordering: Option<R30MutationOrderingV1>,
}

pub struct R30BoundHostContentCertificateModelV1 {
    state: R30BoundHostContentCertificateSnapshotV1,
}

impl R30BoundHostContentCertificateModelV1 {
    pub fn new_model_only(
        storage: R30BoundHostStorageV1,
    ) -> Result<Self, R30BoundHostContentCertificateErrorV1> {
        if !storage.is_valid() {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidStorage);
        }
        Ok(Self {
            state: R30BoundHostContentCertificateSnapshotV1 {
                storage,
                phase: R30CertificatePhaseV1::Host,
                certificate: None,
                pending_completion: None,
                ready: None,
                terminal_custody: None,
                retired_completion_generation: 0,
                transition_clock: 0,
                last_certificate_invalidation_step: None,
                last_mutation_ordering: None,
            },
        })
    }

    pub const fn snapshot(&self) -> R30BoundHostContentCertificateSnapshotV1 {
        self.state
    }

    /// Models a contracted coherent CPU write and its post-write currentness
    /// check. Invalidation is recorded before the possible write; a certificate
    /// is installed only after an exact full write is reported current.
    pub fn complete_exact_full_cpu_write_model_only(
        &mut self,
        range: R30HostStorageRangeV1,
        digest: R30HostContentDigestV1,
        currentness: R30CurrentnessEnvelopeV1,
    ) -> Result<R30BoundHostContentCertificateV1, R30BoundHostContentCertificateErrorV1> {
        self.require_mutable_host_phase()?;
        if !range.is_exact_full_extent(self.state.storage) {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidRange);
        }
        let invalidation_step = self
            .state
            .transition_clock
            .checked_add(1)
            .ok_or(R30BoundHostContentCertificateErrorV1::TransitionClockExhausted)?;
        let possible_mutation_step = invalidation_step
            .checked_add(1)
            .ok_or(R30BoundHostContentCertificateErrorV1::TransitionClockExhausted)?;
        self.state.certificate = None;
        self.state.transition_clock = invalidation_step;
        self.state.last_certificate_invalidation_step = Some(invalidation_step);
        self.state.last_mutation_ordering = None;
        if currentness.opening != R30ContractedCurrentnessV1::Current {
            return Err(R30BoundHostContentCertificateErrorV1::CurrentnessAmbiguous);
        }
        self.state.transition_clock = possible_mutation_step;
        self.state.last_mutation_ordering = Some(R30MutationOrderingV1 {
            kind: R30HostMutationKindV1::CpuDestinationWrite,
            invalidation_step,
            possible_mutation_step,
        });
        if currentness.closing != R30ContractedCurrentnessV1::Current {
            return Err(R30BoundHostContentCertificateErrorV1::CurrentnessAmbiguous);
        }
        let certificate = R30BoundHostContentCertificateV1 {
            storage: self.state.storage,
            full_range: range,
            digest,
        };
        self.state.certificate = Some(certificate);
        Ok(certificate)
    }

    pub fn cpu_destination_write_model_only(
        &mut self,
        range: R30HostStorageRangeV1,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_mutable_host_phase()?;
        if !range.is_nonempty_in_bounds(self.state.storage) {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidRange);
        }
        self.invalidate_before_possible_mutation(R30HostMutationKindV1::CpuDestinationWrite)
    }

    pub fn sdma_destination_write_model_only(
        &mut self,
        range: R30HostStorageRangeV1,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_mutable_host_phase()?;
        if !range.is_nonempty_in_bounds(self.state.storage) {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidRange);
        }
        self.invalidate_before_possible_mutation(R30HostMutationKindV1::SdmaDestinationWrite)
    }

    pub fn resize_model_only(
        &mut self,
        logical_extent_bytes: u64,
        physical_extent_bytes: u64,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_mutable_host_phase()?;
        let resized = R30BoundHostStorageV1 {
            logical_extent_bytes,
            physical_extent_bytes,
            ..self.state.storage
        };
        if !resized.is_valid() {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidStorage);
        }
        self.invalidate_before_possible_mutation(R30HostMutationKindV1::Resize)?;
        self.state.storage = resized;
        Ok(())
    }

    pub fn recycle_model_only(
        &mut self,
        storage_generation: u64,
        pool_generation: u64,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_mutable_host_phase()?;
        if storage_generation <= self.state.storage.storage_generation
            || pool_generation <= self.state.storage.pool_generation
        {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidGeneration);
        }
        self.invalidate_before_possible_mutation(R30HostMutationKindV1::Recycle)?;
        self.state.storage.storage_generation = storage_generation;
        self.state.storage.pool_generation = pool_generation;
        Ok(())
    }

    /// Exact H2D source admission is observational and preserves the live
    /// certificate byte-for-byte.
    pub fn use_as_h2d_source_model_only(
        &self,
        certificate: R30BoundHostContentCertificateV1,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_not_terminal()?;
        if self.state.certificate != Some(certificate)
            || !certificate.is_exact_for(self.state.storage)
        {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidRange);
        }
        Ok(())
    }

    /// Records a contracted exact full-H2D completion. DMA execution and
    /// completion truth are outside this model; this transition preserves the
    /// source certificate.
    pub fn record_exact_full_h2d_completion_model_only(
        &mut self,
        full_range: R30HostStorageRangeV1,
        completion_generation: u64,
    ) -> Result<R30FullH2dCompletionV1, R30BoundHostContentCertificateErrorV1> {
        self.require_not_terminal()?;
        if self.state.phase != R30CertificatePhaseV1::Host {
            return Err(R30BoundHostContentCertificateErrorV1::IllegalPhase);
        }
        if !full_range.is_exact_full_extent(self.state.storage) {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidRange);
        }
        if completion_generation == 0
            || completion_generation <= self.state.retired_completion_generation
        {
            return Err(R30BoundHostContentCertificateErrorV1::InvalidGeneration);
        }
        let completion = R30FullH2dCompletionV1 {
            completion_generation,
            storage: self.state.storage,
            full_range,
        };
        self.state.phase = R30CertificatePhaseV1::FullH2dCompleted;
        self.state.pending_completion = Some(completion);
        Ok(completion)
    }

    /// Promotes only an exact live completion/certificate pair. A coordinate
    /// mismatch is a retryable byte-for-byte no-effect outcome. Ambiguous
    /// currentness enters an absorbing terminal state with exact custody. The
    /// successful update retires the completion and mints `Ready` atomically.
    pub fn promote_model_only(
        &mut self,
        completion: R30FullH2dCompletionV1,
        certificate: R30BoundHostContentCertificateV1,
        currentness: R30CurrentnessEnvelopeV1,
    ) -> Result<R30PromotionOutcomeV1, R30BoundHostContentCertificateErrorV1> {
        if self.state.phase == R30CertificatePhaseV1::TerminalAbsorbed {
            return Ok(R30PromotionOutcomeV1::TerminalAbsorbed);
        }
        if self.state.phase != R30CertificatePhaseV1::FullH2dCompleted {
            return Err(R30BoundHostContentCertificateErrorV1::IllegalPhase);
        }
        if self.state.pending_completion != Some(completion) {
            return Ok(R30PromotionOutcomeV1::RetryableNoEffect);
        }
        let terminal_stage = if currentness.opening == R30ContractedCurrentnessV1::Ambiguous {
            Some(R30TerminalPromotionStageV1::OpeningCurrentnessAmbiguous)
        } else if currentness.closing == R30ContractedCurrentnessV1::Ambiguous {
            Some(R30TerminalPromotionStageV1::ClosingCurrentnessAmbiguous)
        } else {
            None
        };
        if let Some(stage) = terminal_stage {
            self.state.phase = R30CertificatePhaseV1::TerminalAbsorbed;
            let stored_certificate = self.state.certificate;
            self.state.certificate = None;
            self.state.pending_completion = None;
            self.state.ready = None;
            self.state.terminal_custody = Some(R30TerminalPromotionCustodyV1 {
                completion,
                stored_certificate,
                stage,
            });
            return Ok(R30PromotionOutcomeV1::TerminalAbsorbed);
        }
        let exact = self.state.certificate == Some(certificate)
            && completion.is_exact_for(self.state.storage)
            && certificate.is_exact_for(self.state.storage);
        if !exact {
            return Ok(R30PromotionOutcomeV1::RetryableNoEffect);
        }

        self.state.phase = R30CertificatePhaseV1::Ready;
        self.state.pending_completion = None;
        self.state.ready = Some(R30ReadyContentV1 {
            completion_generation: completion.completion_generation,
            digest: certificate.digest,
        });
        self.state.retired_completion_generation = completion.completion_generation;
        Ok(R30PromotionOutcomeV1::Ready)
    }

    pub fn validate_global_invariants(&self) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        if !self.state.storage.is_valid() {
            return Err(R30BoundHostContentCertificateErrorV1::InvariantViolation);
        }
        if matches!(
            self.state.certificate,
            Some(certificate) if !certificate.is_exact_for(self.state.storage)
        ) {
            return Err(R30BoundHostContentCertificateErrorV1::InvariantViolation);
        }
        if matches!(
            self.state.last_mutation_ordering,
            Some(ordering)
                if !ordering.invalidated_before_possible_mutation()
                || ordering.possible_mutation_step != self.state.transition_clock
                || ordering.invalidation_step.checked_add(1)
                    != Some(ordering.possible_mutation_step)
        ) {
            return Err(R30BoundHostContentCertificateErrorV1::InvariantViolation);
        }
        if matches!(
            self.state.last_certificate_invalidation_step,
            Some(step) if step > self.state.transition_clock
        ) {
            return Err(R30BoundHostContentCertificateErrorV1::InvariantViolation);
        }
        let phase_valid = match self.state.phase {
            R30CertificatePhaseV1::Host => {
                self.state.pending_completion.is_none()
                    && self.state.ready.is_none()
                    && self.state.terminal_custody.is_none()
                    && self.state.retired_completion_generation == 0
            }
            R30CertificatePhaseV1::FullH2dCompleted => {
                matches!(
                    self.state.pending_completion,
                    Some(completion)
                        if completion.completion_generation
                            > self.state.retired_completion_generation
                            && completion.is_exact_for(self.state.storage)
                ) && self.state.ready.is_none()
                    && self.state.terminal_custody.is_none()
                    && self.state.retired_completion_generation == 0
            }
            R30CertificatePhaseV1::Ready => {
                self.state.pending_completion.is_none()
                    && matches!(
                        self.state.ready,
                        Some(ready)
                            if ready.completion_generation
                                == self.state.retired_completion_generation
                                && ready.completion_generation != 0
                    )
                    && self.state.terminal_custody.is_none()
            }
            R30CertificatePhaseV1::TerminalAbsorbed => {
                self.state.certificate.is_none()
                    && self.state.pending_completion.is_none()
                    && self.state.ready.is_none()
                    && matches!(
                        self.state.terminal_custody,
                        Some(custody)
                            if custody.completion.is_exact_for(self.state.storage)
                                && match custody.stored_certificate {
                                    Some(certificate) => certificate.is_exact_for(self.state.storage),
                                    None => true,
                                }
                    )
            }
        };
        if !phase_valid {
            return Err(R30BoundHostContentCertificateErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn invalidate_before_possible_mutation(
        &mut self,
        kind: R30HostMutationKindV1,
    ) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        let invalidation_step = self
            .state
            .transition_clock
            .checked_add(1)
            .ok_or(R30BoundHostContentCertificateErrorV1::TransitionClockExhausted)?;
        let possible_mutation_step = invalidation_step
            .checked_add(1)
            .ok_or(R30BoundHostContentCertificateErrorV1::TransitionClockExhausted)?;
        self.state.certificate = None;
        self.state.transition_clock = possible_mutation_step;
        self.state.last_certificate_invalidation_step = Some(invalidation_step);
        self.state.last_mutation_ordering = Some(R30MutationOrderingV1 {
            kind,
            invalidation_step,
            possible_mutation_step,
        });
        Ok(())
    }

    fn require_not_terminal(&self) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        if self.state.phase == R30CertificatePhaseV1::TerminalAbsorbed {
            Err(R30BoundHostContentCertificateErrorV1::TerminalAbsorbed)
        } else {
            Ok(())
        }
    }

    fn require_mutable_host_phase(&self) -> Result<(), R30BoundHostContentCertificateErrorV1> {
        self.require_not_terminal()?;
        if matches!(
            self.state.phase,
            R30CertificatePhaseV1::Host | R30CertificatePhaseV1::Ready
        ) {
            Ok(())
        } else {
            Err(R30BoundHostContentCertificateErrorV1::IllegalPhase)
        }
    }
}
