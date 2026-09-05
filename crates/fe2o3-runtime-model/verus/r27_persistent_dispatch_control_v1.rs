// Independent finite R27 model of the prepare-once/replay-many persistent
// dispatch-control protocol. This proves properties of the model below; it is
// not a refinement of executable Rust, KFD/HSA/HIP, firmware, or hardware.
use vstd::prelude::*;

verus! {

pub open spec fn max_generation_v1() -> nat { 18_446_744_073_709_551_615 }

#[derive(PartialEq, Eq)]
pub struct ControlIdentityV1 {
    pub queue_occurrence: nat,
    pub vm: nat,
    pub semantic_digest: nat,
    pub content_role: nat,
    pub data_layout: nat,
    pub storage_identity: nat,
    pub effect: nat,
}

#[derive(PartialEq, Eq)]
pub enum ControlPhaseV1 {
    Ordinary,
    Attached,
    DataDetached,
}

#[derive(PartialEq, Eq)]
pub struct ReplayRequestV1 {
    pub expected_identity: ControlIdentityV1,
    pub expected_recycled_predecessor: nat,
}

#[derive(PartialEq, Eq)]
pub struct ControlStateV1 {
    pub phase: ControlPhaseV1,
    pub identity: Option<ControlIdentityV1>,
    pub active_generation: nat,
    pub detached_generation: Option<nat>,
    pub queue_authorities: nat,
    pub external_authorities: nat,
    pub premise_count: nat,
    pub code_retained: bool,
    pub kernarg_retained: bool,
    pub packet_retained: bool,
    pub retain_events: nat,
    pub release_events: nat,
    pub published: bool,
}

pub open spec fn valid_identity_v1(identity: ControlIdentityV1) -> bool {
    identity.queue_occurrence > 0 && identity.vm > 0 && identity.semantic_digest > 0
        && identity.content_role > 0 && identity.data_layout > 0
        && identity.storage_identity > 0 && identity.effect > 0
}

pub open spec fn exact_identity_v1(left: ControlIdentityV1,
    right: ControlIdentityV1) -> bool
{
    left == right
}

pub open spec fn exactly_one_data_authority_v1(state: ControlStateV1) -> bool {
    state.queue_authorities + state.external_authorities == 1
}

pub open spec fn authority_event_balance_v1(state: ControlStateV1) -> bool {
    &&& state.release_events <= state.retain_events
    &&& if state.queue_authorities == 1 {
        state.retain_events == state.release_events + 1
    } else {
        state.retain_events == state.release_events
    }
}

pub open spec fn valid_state_v1(state: ControlStateV1) -> bool {
    &&& exactly_one_data_authority_v1(state)
    &&& authority_event_balance_v1(state)
    &&& match state.identity {
        Some(identity) => valid_identity_v1(identity),
        None => true,
    }
    &&& match state.phase {
        ControlPhaseV1::Ordinary => {
            &&& state.identity.is_none()
            &&& state.active_generation == 0
            &&& state.queue_authorities == 0
            &&& state.external_authorities == 1
            &&& state.premise_count == 0
            &&& !state.code_retained
            &&& !state.kernarg_retained
            &&& !state.packet_retained
            &&& !state.published
        },
        ControlPhaseV1::Attached => {
            &&& state.identity.is_some()
            &&& state.active_generation > 0
            &&& state.active_generation <= max_generation_v1()
            &&& state.detached_generation.is_none()
            &&& state.queue_authorities == 1
            &&& state.external_authorities == 0
            &&& state.premise_count == 1
            &&& state.code_retained
            &&& state.kernarg_retained
            &&& state.packet_retained
        },
        ControlPhaseV1::DataDetached => {
            &&& state.identity.is_some()
            &&& state.active_generation > 0
            &&& state.active_generation <= max_generation_v1()
            &&& state.detached_generation == Some(state.active_generation)
            &&& state.queue_authorities == 0
            &&& state.external_authorities == 1
            &&& state.premise_count == 1
            &&& state.code_retained
            &&& state.kernarg_retained
            &&& state.packet_retained
            &&& !state.published
        },
    }
}

pub open spec fn initial_state_v1() -> ControlStateV1 {
    ControlStateV1 {
        phase: ControlPhaseV1::Ordinary,
        identity: None,
        active_generation: 0,
        detached_generation: None,
        queue_authorities: 0,
        external_authorities: 1,
        premise_count: 0,
        code_retained: false,
        kernarg_retained: false,
        packet_retained: false,
        retain_events: 0,
        release_events: 0,
        published: false,
    }
}

pub open spec fn initial_prepare_allowed_v1(state: ControlStateV1,
    identity: ControlIdentityV1) -> bool
{
    &&& valid_state_v1(state)
    &&& state.phase == ControlPhaseV1::Ordinary
    &&& state.detached_generation.is_none()
    &&& valid_identity_v1(identity)
}

pub open spec fn initial_prepare_v1(state: ControlStateV1,
    identity: ControlIdentityV1) -> ControlStateV1
{
    if initial_prepare_allowed_v1(state, identity) {
        ControlStateV1 {
            phase: ControlPhaseV1::Attached,
            identity: Some(identity),
            active_generation: 1,
            queue_authorities: 1,
            external_authorities: 0,
            premise_count: 1,
            code_retained: true,
            kernarg_retained: true,
            packet_retained: true,
            retain_events: state.retain_events + 1,
            ..state
        }
    } else { state }
}

pub open spec fn publish_allowed_v1(state: ControlStateV1,
    identity: ControlIdentityV1, generation: nat) -> bool
{
    &&& valid_state_v1(state)
    &&& state.phase == ControlPhaseV1::Attached
    &&& state.identity == Some(identity)
    &&& state.active_generation == generation
    &&& state.queue_authorities == 1
}

pub open spec fn publish_v1(state: ControlStateV1, identity: ControlIdentityV1,
    generation: nat) -> ControlStateV1
{
    if publish_allowed_v1(state, identity, generation) {
        ControlStateV1 { published: true, ..state }
    } else { state }
}

pub open spec fn recycle_detach_allowed_v1(state: ControlStateV1,
    completed_generation: nat) -> bool
{
    &&& valid_state_v1(state)
    &&& state.phase == ControlPhaseV1::Attached
    &&& state.published
    &&& state.active_generation == completed_generation
    &&& state.queue_authorities == 1
}

pub open spec fn recycle_detach_v1(state: ControlStateV1,
    completed_generation: nat) -> ControlStateV1
{
    if recycle_detach_allowed_v1(state, completed_generation) {
        ControlStateV1 {
            phase: ControlPhaseV1::DataDetached,
            detached_generation: Some(completed_generation),
            queue_authorities: 0,
            external_authorities: 1,
            release_events: state.release_events + 1,
            published: false,
            ..state
        }
    } else { state }
}

pub open spec fn replay_allowed_v1(state: ControlStateV1,
    request: ReplayRequestV1) -> bool
{
    &&& valid_state_v1(state)
    &&& state.phase == ControlPhaseV1::DataDetached
    &&& state.identity == Some(request.expected_identity)
    &&& state.detached_generation == Some(request.expected_recycled_predecessor)
    &&& state.active_generation == request.expected_recycled_predecessor
    &&& request.expected_recycled_predecessor < max_generation_v1()
    &&& state.queue_authorities == 0
    &&& state.external_authorities == 1
    &&& state.premise_count == 1
    &&& state.code_retained
    &&& state.kernarg_retained
    &&& state.packet_retained
}

pub open spec fn replay_v1(state: ControlStateV1,
    request: ReplayRequestV1) -> ControlStateV1
{
    if replay_allowed_v1(state, request) {
        ControlStateV1 {
            phase: ControlPhaseV1::Attached,
            active_generation: request.expected_recycled_predecessor + 1,
            detached_generation: None,
            queue_authorities: 1,
            external_authorities: 0,
            retain_events: state.retain_events + 1,
            ..state
        }
    } else { state }
}

pub open spec fn evict_control_allowed_v1(state: ControlStateV1,
    expected_detached_generation: nat) -> bool
{
    &&& valid_state_v1(state)
    &&& state.phase == ControlPhaseV1::DataDetached
    &&& state.detached_generation == Some(expected_detached_generation)
    &&& state.queue_authorities == 0
    &&& state.external_authorities == 1
    &&& state.premise_count == 1
}

pub open spec fn evict_control_v1(state: ControlStateV1,
    expected_detached_generation: nat) -> ControlStateV1
{
    if evict_control_allowed_v1(state, expected_detached_generation) {
        ControlStateV1 {
            phase: ControlPhaseV1::Ordinary,
            identity: None,
            active_generation: 0,
            premise_count: 0,
            code_retained: false,
            kernarg_retained: false,
            packet_retained: false,
            ..state
        }
    } else { state }
}

pub open spec fn sample_identity_v1() -> ControlIdentityV1 {
    ControlIdentityV1 {
        queue_occurrence: 27,
        vm: 31,
        semantic_digest: 37,
        content_role: 41,
        data_layout: 43,
        storage_identity: 47,
        effect: 53,
    }
}

pub open spec fn sample_attached_v1() -> ControlStateV1 {
    initial_prepare_v1(initial_state_v1(), sample_identity_v1())
}

pub open spec fn sample_published_v1() -> ControlStateV1 {
    publish_v1(sample_attached_v1(), sample_identity_v1(), 1)
}

pub open spec fn sample_detached_v1() -> ControlStateV1 {
    recycle_detach_v1(sample_published_v1(), 1)
}

pub open spec fn sample_replay_request_v1() -> ReplayRequestV1 {
    ReplayRequestV1 {
        expected_identity: sample_identity_v1(),
        expected_recycled_predecessor: 1,
    }
}

pub proof fn constants_and_initial_state_are_valid_v1()
    ensures max_generation_v1() == 18446744073709551615,
        valid_identity_v1(sample_identity_v1()),
        valid_state_v1(initial_state_v1()), {}

pub proof fn initial_prepare_preserves_validity_and_retains_once_v1(state: ControlStateV1,
    identity: ControlIdentityV1)
    requires initial_prepare_allowed_v1(state, identity),
    ensures {
        let prepared = initial_prepare_v1(state, identity);
        &&& valid_state_v1(prepared)
        &&& prepared.phase == ControlPhaseV1::Attached
        &&& prepared.identity == Some(identity)
        &&& prepared.queue_authorities == 1
        &&& prepared.external_authorities == 0
        &&& prepared.retain_events == state.retain_events + 1
        &&& prepared.release_events == state.release_events
    }, {}

pub proof fn incompatible_publication_identity_is_no_effect_v1(state: ControlStateV1,
    expected: ControlIdentityV1, incompatible: ControlIdentityV1, generation: nat)
    requires state.identity == Some(expected), incompatible != expected, !state.published,
    ensures publish_v1(state, incompatible, generation) == state,
        !publish_v1(state, incompatible, generation).published, {}

pub proof fn stale_publication_generation_is_no_effect_v1(state: ControlStateV1,
    identity: ControlIdentityV1, stale_generation: nat)
    requires stale_generation != state.active_generation,
    ensures publish_v1(state, identity, stale_generation) == state, {}

pub proof fn publication_requires_attached_authority_v1(state: ControlStateV1,
    identity: ControlIdentityV1, generation: nat)
    requires state.phase != ControlPhaseV1::Attached,
    ensures publish_v1(state, identity, generation) == state, {}

pub proof fn successful_publication_preserves_valid_state_and_authority_v1(
    state: ControlStateV1, identity: ControlIdentityV1, generation: nat)
    requires publish_allowed_v1(state, identity, generation),
    ensures {
        let published = publish_v1(state, identity, generation);
        &&& valid_state_v1(published)
        &&& published.published
        &&& published.identity == state.identity
        &&& published.active_generation == state.active_generation
        &&& published.queue_authorities == 1
        &&& published.external_authorities == 0
        &&& published.retain_events == state.retain_events
        &&& published.release_events == state.release_events
    }, {}

pub proof fn recycle_detach_preserves_validity_and_releases_once_v1(state: ControlStateV1,
    generation: nat)
    requires recycle_detach_allowed_v1(state, generation),
    ensures {
        let detached = recycle_detach_v1(state, generation);
        &&& valid_state_v1(detached)
        &&& detached.phase == ControlPhaseV1::DataDetached
        &&& detached.detached_generation == Some(generation)
        &&& detached.queue_authorities == 0
        &&& detached.external_authorities == 1
        &&& detached.retain_events == state.retain_events
        &&& detached.release_events == state.release_events + 1
    }, {}

pub proof fn recycle_detach_cannot_release_twice_v1(state: ControlStateV1,
    generation: nat)
    requires state.phase == ControlPhaseV1::DataDetached,
    ensures recycle_detach_v1(state, generation) == state, {}

pub proof fn replay_requires_data_detached_v1(state: ControlStateV1,
    request: ReplayRequestV1)
    requires state.phase != ControlPhaseV1::DataDetached,
    ensures replay_v1(state, request) == state, {}

pub proof fn replay_requires_exact_recycled_predecessor_v1(state: ControlStateV1,
    request: ReplayRequestV1)
    requires state.detached_generation != Some(request.expected_recycled_predecessor),
    ensures replay_v1(state, request) == state, {}

pub proof fn replay_rejects_incompatible_identity_atomically_v1(state: ControlStateV1,
    request: ReplayRequestV1)
    requires state.identity != Some(request.expected_identity),
    ensures replay_v1(state, request) == state, {}

pub proof fn successful_replay_preserves_validity_and_retains_once_v1(state: ControlStateV1,
    request: ReplayRequestV1)
    requires replay_allowed_v1(state, request),
    ensures {
        let replayed = replay_v1(state, request);
        &&& valid_state_v1(replayed)
        &&& replayed.phase == ControlPhaseV1::Attached
        &&& replayed.identity == state.identity
        &&& replayed.queue_authorities == 1
        &&& replayed.external_authorities == 0
        &&& replayed.retain_events == state.retain_events + 1
        &&& replayed.release_events == state.release_events
    }, {}

pub proof fn successful_replay_generation_strictly_advances_v1(state: ControlStateV1,
    request: ReplayRequestV1)
    requires replay_allowed_v1(state, request),
    ensures replay_v1(state, request).active_generation
        > request.expected_recycled_predecessor,
        replay_v1(state, request).active_generation == state.active_generation + 1, {}

pub proof fn incompatible_replay_cannot_publish_v1(state: ControlStateV1,
    request: ReplayRequestV1, publish_identity: ControlIdentityV1, generation: nat)
    requires state.phase == ControlPhaseV1::DataDetached,
        state.identity != Some(request.expected_identity),
    ensures publish_v1(replay_v1(state, request), publish_identity, generation)
        == state, {}

pub proof fn control_eviction_requires_data_detached_v1(state: ControlStateV1,
    expected_detached_generation: nat)
    requires state.phase != ControlPhaseV1::DataDetached,
    ensures evict_control_v1(state, expected_detached_generation) == state, {}

pub proof fn control_eviction_requires_exact_detached_generation_v1(state: ControlStateV1,
    expected_detached_generation: nat)
    requires state.detached_generation != Some(expected_detached_generation),
    ensures evict_control_v1(state, expected_detached_generation) == state, {}

pub proof fn control_only_eviction_preserves_detached_generation_and_authority_v1(
    state: ControlStateV1, expected_detached_generation: nat)
    requires evict_control_allowed_v1(state, expected_detached_generation),
    ensures {
        let evicted = evict_control_v1(state, expected_detached_generation);
        &&& valid_state_v1(evicted)
        &&& evicted.phase == ControlPhaseV1::Ordinary
        &&& evicted.detached_generation == state.detached_generation
        &&& evicted.detached_generation == Some(expected_detached_generation)
        &&& evicted.queue_authorities == 0
        &&& evicted.external_authorities == 1
        &&& evicted.retain_events == state.retain_events
        &&& evicted.release_events == state.release_events
        &&& evicted.premise_count == 0
        &&& !evicted.code_retained
        &&& !evicted.kernarg_retained
        &&& !evicted.packet_retained
    }, {}

pub proof fn sample_replay_chain_advances_without_authority_duplication_v1()
    ensures {
        let detached = sample_detached_v1();
        let replayed = replay_v1(detached, sample_replay_request_v1());
        &&& valid_state_v1(detached)
        &&& valid_state_v1(replayed)
        &&& detached.active_generation == 1
        &&& replayed.active_generation == 2
        &&& exactly_one_data_authority_v1(detached)
        &&& exactly_one_data_authority_v1(replayed)
        &&& detached.retain_events == 1
        &&& detached.release_events == 1
        &&& replayed.retain_events == 2
        &&& replayed.release_events == 1
    }, {}

pub proof fn sample_control_eviction_preserves_recycled_ledger_v1()
    ensures {
        let detached = sample_detached_v1();
        let evicted = evict_control_v1(detached, 1);
        &&& valid_state_v1(evicted)
        &&& evicted.phase == ControlPhaseV1::Ordinary
        &&& evicted.detached_generation == Some(1)
        &&& evicted.external_authorities == 1
        &&& evicted.retain_events == evicted.release_events
    }, {}

fn main() {}

}
