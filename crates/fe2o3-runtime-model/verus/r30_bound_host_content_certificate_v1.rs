// Independent finite R30 bound host-content certificate model. SHA-256
// computation, coherent-write truth, DMA/HSA completion, and both currentness
// observations are contracted inputs. This is not a refinement of executable
// Rust, the production runtime, KFD/HSA/HIP, firmware, or hardware.
use vstd::prelude::*;

verus! {

pub open spec fn max_extent_bytes_v1() -> nat { 256 * 1024 * 1024 }

#[derive(PartialEq, Eq)]
pub struct BoundStorageV1 {
    pub queue: nat,
    pub queue_generation: nat,
    pub storage: nat,
    pub storage_generation: nat,
    pub pool_generation: nat,
    pub logical_extent: nat,
    pub physical_extent: nat,
}

#[derive(PartialEq, Eq)]
pub struct StorageRangeV1 {
    pub logical_offset: nat,
    pub logical_bytes: nat,
    pub physical_offset: nat,
    pub physical_bytes: nat,
}

#[derive(PartialEq, Eq)]
pub struct CertificateV1 {
    pub storage: BoundStorageV1,
    pub full_range: StorageRangeV1,
    // Abstract digest coordinate supplied by a future SHA-256 refinement.
    pub digest: nat,
}

#[derive(PartialEq, Eq)]
pub struct FullH2dCompletionV1 {
    pub generation: nat,
    pub storage: BoundStorageV1,
    pub full_range: StorageRangeV1,
}

#[derive(PartialEq, Eq)]
pub struct ReadyV1 {
    pub generation: nat,
    pub digest: nat,
}

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 {
    OpeningCurrentnessAmbiguous,
    ClosingCurrentnessAmbiguous,
}

#[derive(PartialEq, Eq)]
pub struct TerminalCustodyV1 {
    pub completion: FullH2dCompletionV1,
    pub stored_certificate: Option<CertificateV1>,
    pub stage: TerminalStageV1,
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Host, FullH2dCompleted, Ready, TerminalAbsorbed }

#[derive(PartialEq, Eq)]
pub enum CurrentnessV1 { Current, Ambiguous }

#[derive(PartialEq, Eq)]
pub struct CurrentnessEnvelopeV1 {
    pub opening: CurrentnessV1,
    pub closing: CurrentnessV1,
}

#[derive(PartialEq, Eq)]
pub enum MutationKindV1 { CpuDestination, SdmaDestination, Resize, Recycle }

#[derive(PartialEq, Eq)]
pub struct CertificateStateV1 {
    pub storage: BoundStorageV1,
    pub phase: PhaseV1,
    pub certificate: Option<CertificateV1>,
    pub pending: Option<FullH2dCompletionV1>,
    pub ready: Option<ReadyV1>,
    pub terminal: Option<TerminalCustodyV1>,
    pub retired_generation: nat,
    pub transition_clock: nat,
    pub last_invalidation_step: nat,
    pub last_possible_mutation_step: nat,
    pub last_mutation_kind: Option<MutationKindV1>,
}

pub open spec fn valid_storage_v1(storage: BoundStorageV1) -> bool {
    &&& storage.queue > 0
    &&& storage.queue_generation > 0
    &&& storage.storage > 0
    &&& storage.storage_generation > 0
    &&& storage.pool_generation > 0
    &&& 0 < storage.logical_extent <= storage.physical_extent
    &&& storage.physical_extent <= max_extent_bytes_v1()
}

pub open spec fn exact_full_range_v1(range: StorageRangeV1,
    storage: BoundStorageV1) -> bool
{
    &&& range.logical_offset == 0
    &&& range.physical_offset == 0
    &&& range.logical_bytes == storage.logical_extent
    &&& range.physical_bytes == storage.physical_extent
    // The production authenticated full-write/H2D contract transfers one
    // equal byte count; padded unequal extents cannot be certified.
    &&& range.logical_bytes == range.physical_bytes
}

pub open spec fn exact_certificate_v1(certificate: CertificateV1,
    storage: BoundStorageV1) -> bool
{
    certificate.storage == storage
        && exact_full_range_v1(certificate.full_range, storage)
}

pub open spec fn all_current_v1() -> CurrentnessEnvelopeV1 {
    CurrentnessEnvelopeV1 {
        opening: CurrentnessV1::Current,
        closing: CurrentnessV1::Current,
    }
}

pub open spec fn valid_state_v1(state: CertificateStateV1) -> bool {
    &&& valid_storage_v1(state.storage)
    &&& state.last_invalidation_step <= state.last_possible_mutation_step
    &&& state.last_possible_mutation_step <= state.transition_clock
    &&& match state.last_mutation_kind {
        Some(_) => state.last_invalidation_step + 1 == state.last_possible_mutation_step
            && state.last_possible_mutation_step == state.transition_clock,
        None => state.last_possible_mutation_step == 0
            && (state.last_invalidation_step == 0
                || state.last_invalidation_step == state.transition_clock),
    }
    &&& match state.certificate {
        Some(certificate) => exact_certificate_v1(certificate, state.storage),
        None => true,
    }
    &&& match state.phase {
        PhaseV1::Host => state.pending.is_none() && state.ready.is_none()
            && state.terminal.is_none() && state.retired_generation == 0,
        PhaseV1::FullH2dCompleted => state.ready.is_none() && state.terminal.is_none()
            && state.retired_generation == 0
            && match state.pending {
                Some(completion) => completion.generation > 0
                    && completion.storage == state.storage
                    && exact_full_range_v1(completion.full_range, state.storage),
                None => false,
            },
        PhaseV1::Ready => state.pending.is_none() && state.terminal.is_none()
            && match state.ready {
                Some(ready) => ready.generation > 0
                    && ready.generation == state.retired_generation,
                None => false,
            },
        PhaseV1::TerminalAbsorbed => state.pending.is_none() && state.ready.is_none()
            && state.certificate.is_none() && state.retired_generation == 0
            && match state.terminal {
                Some(custody) => custody.completion.generation > 0
                    && custody.completion.storage == state.storage
                    && exact_full_range_v1(custody.completion.full_range, state.storage)
                    && match custody.stored_certificate {
                        Some(certificate) => exact_certificate_v1(certificate, state.storage),
                        None => true,
                    },
                None => false,
            },
    }
}

pub open spec fn initial_state_v1(storage: BoundStorageV1) -> CertificateStateV1 {
    CertificateStateV1 {
        storage,
        phase: PhaseV1::Host,
        certificate: None,
        pending: None,
        ready: None,
        terminal: None,
        retired_generation: 0,
        transition_clock: 0,
        last_invalidation_step: 0,
        last_possible_mutation_step: 0,
        last_mutation_kind: None,
    }
}

pub open spec fn certificate_for_v1(storage: BoundStorageV1, range: StorageRangeV1,
    digest: nat) -> CertificateV1
{
    CertificateV1 { storage, full_range: range, digest }
}

pub open spec fn invalidated_before_mutation_v1(state: CertificateStateV1,
    kind: MutationKindV1) -> CertificateStateV1
{
    CertificateStateV1 {
        certificate: None,
        transition_clock: state.transition_clock + 2,
        last_invalidation_step: state.transition_clock + 1,
        last_possible_mutation_step: state.transition_clock + 2,
        last_mutation_kind: Some(kind),
        ..state
    }
}

pub open spec fn complete_full_write_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat, currentness: CurrentnessEnvelopeV1)
    -> CertificateStateV1
{
    if (state.phase != PhaseV1::Host && state.phase != PhaseV1::Ready)
        || !exact_full_range_v1(range, state.storage) { state }
    else {
        // Production clears before its opening shared-memory observation.
        let invalidated = CertificateStateV1 {
            certificate: None,
            transition_clock: state.transition_clock + 1,
            last_invalidation_step: state.transition_clock + 1,
            last_possible_mutation_step: 0,
            last_mutation_kind: None,
            ..state
        };
        if currentness.opening == CurrentnessV1::Ambiguous { invalidated }
        else {
            let mutated = CertificateStateV1 {
                transition_clock: state.transition_clock + 2,
                last_possible_mutation_step: state.transition_clock + 2,
                last_mutation_kind: Some(MutationKindV1::CpuDestination),
                ..invalidated
            };
            if currentness.closing == CurrentnessV1::Ambiguous { mutated }
        else { CertificateStateV1 {
            certificate: Some(certificate_for_v1(state.storage, range, digest)),
            ..mutated
        }}
        }
    }
}

pub open spec fn destination_write_v1(state: CertificateStateV1,
    kind: MutationKindV1) -> CertificateStateV1
{
    if (state.phase != PhaseV1::Host && state.phase != PhaseV1::Ready)
        || (kind != MutationKindV1::CpuDestination
            && kind != MutationKindV1::SdmaDestination) { state }
    else { invalidated_before_mutation_v1(state, kind) }
}

pub open spec fn resize_v1(state: CertificateStateV1,
    logical_extent: nat, physical_extent: nat) -> CertificateStateV1
{
    let resized = BoundStorageV1 { logical_extent, physical_extent, ..state.storage };
    if (state.phase != PhaseV1::Host && state.phase != PhaseV1::Ready)
        || !valid_storage_v1(resized) { state }
    else {
        let invalidated = invalidated_before_mutation_v1(state, MutationKindV1::Resize);
        CertificateStateV1 { storage: resized, ..invalidated }
    }
}

pub open spec fn recycle_v1(state: CertificateStateV1,
    storage_generation: nat, pool_generation: nat) -> CertificateStateV1
{
    if (state.phase != PhaseV1::Host && state.phase != PhaseV1::Ready)
        || storage_generation <= state.storage.storage_generation
        || pool_generation <= state.storage.pool_generation { state }
    else {
        let recycled = BoundStorageV1 {
            storage_generation,
            pool_generation,
            ..state.storage
        };
        let invalidated = invalidated_before_mutation_v1(state, MutationKindV1::Recycle);
        CertificateStateV1 { storage: recycled, ..invalidated }
    }
}

// Exact H2D source use is observational and preserves every state coordinate.
pub open spec fn h2d_source_v1(state: CertificateStateV1,
    certificate: CertificateV1) -> CertificateStateV1
{
    if state.certificate == Some(certificate)
        && exact_certificate_v1(certificate, state.storage) { state } else { state }
}

pub open spec fn completion_for_v1(storage: BoundStorageV1, range: StorageRangeV1,
    generation: nat) -> FullH2dCompletionV1
{
    FullH2dCompletionV1 { generation, storage, full_range: range }
}

pub open spec fn record_full_h2d_completion_v1(state: CertificateStateV1,
    range: StorageRangeV1, generation: nat) -> CertificateStateV1
{
    if state.phase == PhaseV1::Host
        && exact_full_range_v1(range, state.storage) && generation > 0
    {
        CertificateStateV1 {
            phase: PhaseV1::FullH2dCompleted,
            pending: Some(completion_for_v1(state.storage, range, generation)),
            ..state
        }
    } else { state }
}

pub open spec fn promotion_completion_matches_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1) -> bool
{
    &&& state.phase == PhaseV1::FullH2dCompleted
    &&& state.pending == Some(completion)
    &&& completion.storage == state.storage
    &&& exact_full_range_v1(completion.full_range, state.storage)
}

pub open spec fn promotion_certificate_matches_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1) -> bool
{
    &&& state.certificate == Some(certificate)
    &&& exact_certificate_v1(certificate, state.storage)
}

pub open spec fn promotion_matches_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1) -> bool
{
    promotion_completion_matches_v1(state, completion)
        && promotion_certificate_matches_v1(state, completion, certificate)
}

pub open spec fn terminal_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1,
    stage: TerminalStageV1) -> CertificateStateV1
{
    CertificateStateV1 {
        phase: PhaseV1::TerminalAbsorbed,
        certificate: None,
        pending: None,
        ready: None,
        terminal: Some(TerminalCustodyV1 {
            completion, stored_certificate: state.certificate, stage }),
        ..state
    }
}

pub open spec fn promote_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1,
    currentness: CurrentnessEnvelopeV1) -> CertificateStateV1
{
    if state.phase == PhaseV1::TerminalAbsorbed { state }
    else if !promotion_completion_matches_v1(state, completion) { state }
    else if currentness.opening == CurrentnessV1::Ambiguous {
        terminal_promotion_v1(state, completion,
            TerminalStageV1::OpeningCurrentnessAmbiguous)
    } else if currentness.closing == CurrentnessV1::Ambiguous {
        terminal_promotion_v1(state, completion,
            TerminalStageV1::ClosingCurrentnessAmbiguous)
    } else if !promotion_certificate_matches_v1(state, completion, certificate) { state }
    else {
        CertificateStateV1 {
            phase: PhaseV1::Ready,
            pending: None,
            ready: Some(ReadyV1 {
                generation: completion.generation, digest: certificate.digest }),
            terminal: None,
            retired_generation: completion.generation,
            ..state
        }
    }
}

pub proof fn initial_state_is_valid_v1(storage: BoundStorageV1)
    requires valid_storage_v1(storage),
    ensures valid_state_v1(initial_state_v1(storage)), {}

pub proof fn certificate_requires_equal_full_extents_v1(certificate: CertificateV1,
    storage: BoundStorageV1)
    requires exact_certificate_v1(certificate, storage),
    ensures storage.logical_extent == storage.physical_extent,
        certificate.full_range.logical_offset == 0,
        certificate.full_range.physical_offset == 0, {}

pub proof fn certificate_binds_queue_and_storage_identity_v1(certificate: CertificateV1,
    storage: BoundStorageV1)
    requires exact_certificate_v1(certificate, storage),
    ensures certificate.storage.queue == storage.queue,
        certificate.storage.queue_generation == storage.queue_generation,
        certificate.storage.storage == storage.storage,
        certificate.storage.storage_generation == storage.storage_generation, {}

pub proof fn certificate_binds_pool_and_extents_v1(certificate: CertificateV1,
    storage: BoundStorageV1)
    requires exact_certificate_v1(certificate, storage),
    ensures certificate.storage.pool_generation == storage.pool_generation,
        certificate.full_range.logical_bytes == storage.logical_extent,
        certificate.full_range.physical_bytes == storage.physical_extent, {}

pub proof fn constructed_certificate_binds_digest_v1(storage: BoundStorageV1,
    range: StorageRangeV1, digest: nat)
    requires exact_full_range_v1(range, storage),
    ensures exact_certificate_v1(certificate_for_v1(storage, range, digest), storage),
        certificate_for_v1(storage, range, digest).digest == digest, {}

pub proof fn write_opening_ambiguity_clears_without_mutation_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat, closing: CurrentnessV1)
    requires state.phase == PhaseV1::Host, exact_full_range_v1(range, state.storage),
    ensures {
        let post = complete_full_write_v1(state, range, digest, CurrentnessEnvelopeV1 {
            opening: CurrentnessV1::Ambiguous, closing });
        &&& post.certificate.is_none()
        &&& post.transition_clock == state.transition_clock + 1
        &&& post.last_invalidation_step == state.transition_clock + 1
        &&& post.last_possible_mutation_step == 0
        &&& post.last_mutation_kind.is_none()
    }, {}

pub proof fn inexact_write_claim_is_atomic_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat, currentness: CurrentnessEnvelopeV1)
    requires !exact_full_range_v1(range, state.storage),
    ensures complete_full_write_v1(state, range, digest, currentness) == state, {}

pub proof fn write_closing_ambiguity_is_uncertified_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat)
    requires state.phase == PhaseV1::Host, exact_full_range_v1(range, state.storage),
    ensures {
        let post = complete_full_write_v1(state, range, digest, CurrentnessEnvelopeV1 {
            opening: CurrentnessV1::Current, closing: CurrentnessV1::Ambiguous });
        &&& post.certificate.is_none()
        &&& post.last_invalidation_step == state.transition_clock + 1
        &&& post.last_possible_mutation_step == state.transition_clock + 2
    }, {}

pub proof fn exact_current_write_establishes_certificate_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat)
    requires state.phase == PhaseV1::Host, exact_full_range_v1(range, state.storage),
    ensures {
        let post = complete_full_write_v1(state, range, digest, all_current_v1());
        &&& post.certificate == Some(certificate_for_v1(state.storage, range, digest))
        &&& exact_certificate_v1(post.certificate.unwrap(), state.storage)
        &&& post.last_invalidation_step < post.last_possible_mutation_step
    }, {}

pub proof fn exact_current_write_preserves_validity_v1(state: CertificateStateV1,
    range: StorageRangeV1, digest: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        exact_full_range_v1(range, state.storage),
    ensures valid_state_v1(complete_full_write_v1(state, range, digest, all_current_v1())), {}

pub proof fn cpu_destination_invalidates_before_mutation_v1(state: CertificateStateV1)
    requires state.phase == PhaseV1::Host,
    ensures {
        let post = destination_write_v1(state, MutationKindV1::CpuDestination);
        &&& post.certificate.is_none()
        &&& post.last_invalidation_step == state.transition_clock + 1
        &&& post.last_possible_mutation_step == state.transition_clock + 2
    }, {}

pub proof fn sdma_destination_invalidates_before_mutation_v1(state: CertificateStateV1)
    requires state.phase == PhaseV1::Host,
    ensures {
        let post = destination_write_v1(state, MutationKindV1::SdmaDestination);
        &&& post.certificate.is_none()
        &&& post.last_invalidation_step == state.transition_clock + 1
        &&& post.last_possible_mutation_step == state.transition_clock + 2
    }, {}

pub proof fn resize_invalidates_before_extent_change_v1(state: CertificateStateV1,
    logical_extent: nat, physical_extent: nat)
    requires state.phase == PhaseV1::Host,
        valid_storage_v1(BoundStorageV1 { logical_extent, physical_extent, ..state.storage }),
    ensures {
        let post = resize_v1(state, logical_extent, physical_extent);
        &&& post.certificate.is_none()
        &&& post.last_invalidation_step < post.last_possible_mutation_step
        &&& post.storage.logical_extent == logical_extent
        &&& post.storage.physical_extent == physical_extent
    }, {}

pub proof fn recycle_invalidates_before_generation_change_v1(state: CertificateStateV1,
    storage_generation: nat, pool_generation: nat)
    requires state.phase == PhaseV1::Host,
        storage_generation > state.storage.storage_generation,
        pool_generation > state.storage.pool_generation,
    ensures {
        let post = recycle_v1(state, storage_generation, pool_generation);
        &&& post.certificate.is_none()
        &&& post.last_invalidation_step < post.last_possible_mutation_step
        &&& post.storage.storage_generation == storage_generation
        &&& post.storage.pool_generation == pool_generation
    }, {}

pub proof fn destination_invalidation_preserves_validity_v1(state: CertificateStateV1,
    kind: MutationKindV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        kind == MutationKindV1::CpuDestination || kind == MutationKindV1::SdmaDestination,
    ensures valid_state_v1(destination_write_v1(state, kind)), {}

pub proof fn resize_preserves_validity_v1(state: CertificateStateV1,
    logical_extent: nat, physical_extent: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        valid_storage_v1(BoundStorageV1 { logical_extent, physical_extent, ..state.storage }),
    ensures valid_state_v1(resize_v1(state, logical_extent, physical_extent)), {}

pub proof fn recycle_preserves_validity_v1(state: CertificateStateV1,
    storage_generation: nat, pool_generation: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        storage_generation > state.storage.storage_generation,
        pool_generation > state.storage.pool_generation,
    ensures valid_state_v1(recycle_v1(state, storage_generation, pool_generation)), {}

pub proof fn h2d_source_preserves_exact_state_v1(state: CertificateStateV1,
    certificate: CertificateV1)
    requires state.certificate == Some(certificate),
        exact_certificate_v1(certificate, state.storage),
    ensures h2d_source_v1(state, certificate) == state, {}

pub proof fn full_h2d_completion_preserves_source_certificate_v1(state: CertificateStateV1,
    certificate: CertificateV1, generation: nat)
    requires state.phase == PhaseV1::Host, state.certificate == Some(certificate),
        exact_certificate_v1(certificate, state.storage), generation > 0,
    ensures {
        let post = record_full_h2d_completion_v1(
            state, certificate.full_range, generation);
        &&& post.certificate == state.certificate
        &&& post.pending == Some(completion_for_v1(
            state.storage, certificate.full_range, generation))
        &&& post.phase == PhaseV1::FullH2dCompleted
    }, {}

pub proof fn full_h2d_completion_preserves_validity_v1(state: CertificateStateV1,
    certificate: CertificateV1, generation: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        state.certificate == Some(certificate), generation > 0,
    ensures valid_state_v1(record_full_h2d_completion_v1(
        state, certificate.full_range, generation)), {}

pub proof fn full_h2d_completion_can_preserve_missing_certificate_v1(
    state: CertificateStateV1, range: StorageRangeV1, generation: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Host,
        state.certificate.is_none(), exact_full_range_v1(range, state.storage), generation > 0,
    ensures {
        let post = record_full_h2d_completion_v1(state, range, generation);
        &&& post.phase == PhaseV1::FullH2dCompleted
        &&& post.certificate.is_none()
        &&& post.pending == Some(completion_for_v1(state.storage, range, generation))
        &&& valid_state_v1(post)
    }, {}

pub proof fn promotion_completion_mismatch_is_atomic_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1,
    currentness: CurrentnessEnvelopeV1)
    requires state.phase != PhaseV1::TerminalAbsorbed,
        !promotion_completion_matches_v1(state, completion),
    ensures promote_v1(state, completion, certificate, currentness) == state, {}

pub proof fn promotion_certificate_mismatch_is_retryable_after_currentness_v1(
    state: CertificateStateV1, completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.phase != PhaseV1::TerminalAbsorbed,
        promotion_completion_matches_v1(state, completion),
        !promotion_certificate_matches_v1(state, completion, certificate),
    ensures promote_v1(state, completion, certificate, all_current_v1()) == state, {}

pub proof fn opening_ambiguity_has_exact_terminal_custody_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1, closing: CurrentnessV1)
    requires promotion_completion_matches_v1(state, completion),
    ensures {
        let post = promote_v1(state, completion, certificate, CurrentnessEnvelopeV1 {
            opening: CurrentnessV1::Ambiguous, closing });
        &&& post.phase == PhaseV1::TerminalAbsorbed
        &&& post.terminal == Some(TerminalCustodyV1 {
            completion, stored_certificate: state.certificate,
            stage: TerminalStageV1::OpeningCurrentnessAmbiguous })
        &&& post.retired_generation == state.retired_generation
    }, {}

pub proof fn closing_ambiguity_has_exact_terminal_custody_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires promotion_completion_matches_v1(state, completion),
    ensures {
        let post = promote_v1(state, completion, certificate, CurrentnessEnvelopeV1 {
            opening: CurrentnessV1::Current, closing: CurrentnessV1::Ambiguous });
        &&& post.phase == PhaseV1::TerminalAbsorbed
        &&& post.terminal == Some(TerminalCustodyV1 {
            completion, stored_certificate: state.certificate,
            stage: TerminalStageV1::ClosingCurrentnessAmbiguous })
        &&& post.retired_generation == state.retired_generation
    }, {}

pub proof fn promotion_ambiguity_retires_nothing_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1,
    currentness: CurrentnessEnvelopeV1)
    requires promotion_completion_matches_v1(state, completion),
        currentness.opening == CurrentnessV1::Ambiguous
            || currentness.closing == CurrentnessV1::Ambiguous,
    ensures promote_v1(state, completion, certificate, currentness).retired_generation
        == state.retired_generation, {}

pub proof fn terminal_promotion_is_absorbing_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1,
    currentness: CurrentnessEnvelopeV1)
    requires state.phase == PhaseV1::TerminalAbsorbed,
    ensures promote_v1(state, completion, certificate, currentness) == state, {}

pub proof fn successful_promotion_mints_exact_ready_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires promotion_matches_v1(state, completion, certificate),
    ensures {
        let post = promote_v1(state, completion, certificate, all_current_v1());
        &&& post.phase == PhaseV1::Ready
        &&& post.pending.is_none()
        &&& post.ready == Some(ReadyV1 {
            generation: completion.generation, digest: certificate.digest })
        &&& post.certificate == Some(certificate)
    }, {}

pub proof fn successful_promotion_retires_exact_completion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires promotion_matches_v1(state, completion, certificate),
    ensures promote_v1(state, completion, certificate, all_current_v1()).retired_generation
        == completion.generation, {}

pub proof fn terminal_promotion_preserves_validity_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1,
    stage: TerminalStageV1)
    requires valid_state_v1(state), promotion_completion_matches_v1(state, completion),
    ensures valid_state_v1(terminal_promotion_v1(state, completion, stage)), {}

pub proof fn successful_promotion_preserves_validity_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires valid_state_v1(state), promotion_matches_v1(state, completion, certificate),
    ensures valid_state_v1(promote_v1(state, completion, certificate, all_current_v1())), {}

pub proof fn ready_recycle_preserves_ready_digest_v1(state: CertificateStateV1,
    storage_generation: nat, pool_generation: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
        storage_generation > state.storage.storage_generation,
        pool_generation > state.storage.pool_generation,
    ensures {
        let post = recycle_v1(state, storage_generation, pool_generation);
        &&& post.phase == PhaseV1::Ready
        &&& post.ready == state.ready
        &&& post.certificate.is_none()
        &&& valid_state_v1(post)
    }, {}

pub proof fn ready_destination_write_preserves_ready_digest_v1(state: CertificateStateV1,
    kind: MutationKindV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
        kind == MutationKindV1::CpuDestination || kind == MutationKindV1::SdmaDestination,
    ensures {
        let post = destination_write_v1(state, kind);
        &&& post.phase == PhaseV1::Ready
        &&& post.ready == state.ready
        &&& post.certificate.is_none()
        &&& valid_state_v1(post)
    }, {}

pub proof fn queue_substitution_blocks_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.phase == PhaseV1::FullH2dCompleted,
        certificate.storage.queue != state.storage.queue,
    ensures !promotion_matches_v1(state, completion, certificate), {}

pub proof fn storage_generation_substitution_blocks_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.phase == PhaseV1::FullH2dCompleted,
        certificate.storage.storage_generation != state.storage.storage_generation,
    ensures !promotion_matches_v1(state, completion, certificate), {}

pub proof fn pool_generation_substitution_blocks_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.phase == PhaseV1::FullH2dCompleted,
        certificate.storage.pool_generation != state.storage.pool_generation,
    ensures !promotion_matches_v1(state, completion, certificate), {}

pub proof fn range_substitution_blocks_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.phase == PhaseV1::FullH2dCompleted,
        !exact_full_range_v1(certificate.full_range, state.storage),
    ensures !promotion_matches_v1(state, completion, certificate), {}

pub proof fn digest_substitution_blocks_promotion_v1(state: CertificateStateV1,
    completion: FullH2dCompletionV1, certificate: CertificateV1)
    requires state.pending == Some(completion), state.certificate.is_some(),
        certificate.digest != state.certificate.unwrap().digest,
    ensures !promotion_matches_v1(state, completion, certificate), {}

}

fn main() {}
