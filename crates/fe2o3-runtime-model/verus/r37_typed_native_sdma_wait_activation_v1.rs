// Independent finite R37 model for typed native SDMA wait activation.
// All identities, counts, storage values, deadlines, and native outcomes are
// contracted mathematical inputs. This file proves no Rust-to-Verus or
// production-Rust refinement and no KFD/HSA/HIP, driver, firmware, native
// queue, clock, hardware completion, coherence, progress, liveness, parity,
// or performance property.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CopyKindV1 { Directional, SameDevice }

#[derive(PartialEq, Eq)]
pub enum DeadlineV1 { Zero, Positive }

#[derive(PartialEq, Eq)]
pub enum CompletionDispositionV1 { Settle, ContinuationReady }

#[derive(PartialEq, Eq)]
pub enum IdentityChangeStageV1 { Pending, Completed }

#[derive(PartialEq, Eq)]
pub struct NativeIdentityV1 {
    pub owner_id: nat,
    pub request_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct OrderedFrameV1 {
    pub predecessor: nat,
    pub current: nat,
    pub successor: nat,
}

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub kind: CopyKindV1,
    pub submission: nat,
    pub stream: nat,
    pub source_allocation: nat,
    pub destination_allocation: nat,
    pub source_storage_generation: nat,
    pub destination_storage_generation: nat,
    pub restored_source_storage: nat,
    pub restored_destination_storage: nat,
    pub dependency_submission: nat,
    pub dependency_retain_count: nat,
    pub source_custody_count: nat,
    pub destination_custody_count: nat,
    pub stream_owner_count: nat,
    pub published_index_frame: OrderedFrameV1,
    pub stream_frame: OrderedFrameV1,
    pub native_identity: NativeIdentityV1,
}

#[derive(PartialEq, Eq)]
pub enum NativeWaitObservationV1 {
    Complete,
    ExactTypedTimeout(NativeIdentityV1),
    NonTimeoutRetryable(NativeIdentityV1),
    IdentityChange(IdentityChangeStageV1, NativeIdentityV1),
    Teardown(nat),
}

#[derive(PartialEq, Eq)]
pub enum CallV1 { Poll, Wait }

#[derive(PartialEq, Eq)]
pub enum EntryPhaseV1 { PublishedDirectional, PublishedSameDevice, Ready, Other }

#[derive(PartialEq, Eq)]
pub enum RouteV1 { Poll, LegacyWaitPoll, NativeDirectionalWait, NativeSameDeviceWait }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Pending, Succeeded, Terminal }

#[derive(PartialEq, Eq)]
pub enum ActivePhaseV1 { Published(CopyKindV1), Ready, Absent }

#[derive(PartialEq, Eq)]
pub enum StorageV1 { InFlight(nat, nat), Restored(nat) }

#[derive(PartialEq, Eq)]
pub enum NativeCustodyV1 {
    ActivePublished(NativeIdentityV1),
    RestoredPair,
    TerminalPending(NativeIdentityV1),
    TerminalCompleted(NativeIdentityV1),
    TerminalTeardown(nat),
}

pub struct StateV1 {
    pub binding: BindingV1,
    pub route: RouteV1,
    pub outcome: OutcomeV1,
    pub active_present: bool,
    pub active_phase: ActivePhaseV1,
    pub published_index_retained: bool,
    pub published_index_frame: OrderedFrameV1,
    pub source_storage: StorageV1,
    pub destination_storage: StorageV1,
    pub dependency_retain_count: nat,
    pub source_custody_count: nat,
    pub destination_custody_count: nat,
    pub stream_owner_count: nat,
    pub stream_current_retained: bool,
    pub stream_frame: OrderedFrameV1,
    pub native_custody: NativeCustodyV1,
    pub terminal_poisoned: bool,
    pub native_observations: nat,
    pub settled: bool,
    pub completion_recorded: bool,
    pub continuation_ready: bool,
    pub continuation_publications: nat,
}

// These proof values are copyable mathematical carriers. They establish no
// Rust move-only ownership or borrow property.
pub struct PublishedAuthorityV1 { pub binding: BindingV1 }
pub struct TerminalAuthorityV1 { pub custody: NativeCustodyV1 }
pub struct RestoredAuthorityV1 { pub binding: BindingV1 }

pub open spec fn valid_identity_v1(identity: NativeIdentityV1) -> bool {
    identity.owner_id > 0 && identity.request_id > 0
}

pub open spec fn valid_frame_v1(frame: OrderedFrameV1, current: nat) -> bool {
    &&& frame.predecessor < frame.current
    &&& frame.current < frame.successor
    &&& frame.current == current
}

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.submission > 0
    &&& binding.stream > 0
    &&& binding.source_allocation > 0
    &&& binding.destination_allocation > 0
    &&& binding.source_allocation != binding.destination_allocation
    &&& binding.source_storage_generation > 0
    &&& binding.destination_storage_generation > 0
    &&& binding.restored_source_storage > 0
    &&& binding.restored_destination_storage > 0
    &&& binding.dependency_submission > 0
    &&& binding.dependency_retain_count > 0
    &&& binding.source_custody_count > 0
    &&& binding.destination_custody_count > 0
    &&& binding.stream_owner_count > 0
    &&& valid_frame_v1(binding.published_index_frame, binding.submission)
    &&& valid_frame_v1(binding.stream_frame, binding.submission)
    &&& valid_identity_v1(binding.native_identity)
}

/// Input-only observation contract. It invokes no transition relation and
/// compares no output state.
pub open spec fn valid_observation_v1(
    binding: BindingV1,
    observation: NativeWaitObservationV1,
) -> bool {
    match observation {
        NativeWaitObservationV1::Complete => true,
        NativeWaitObservationV1::ExactTypedTimeout(identity)
        | NativeWaitObservationV1::NonTimeoutRetryable(identity) => {
            identity == binding.native_identity
        }
        NativeWaitObservationV1::IdentityChange(_, identity) => {
            valid_identity_v1(identity) && identity != binding.native_identity
        }
        NativeWaitObservationV1::Teardown(token) => token > 0,
    }
}

pub open spec fn route_v1(call: CallV1, phase: EntryPhaseV1) -> RouteV1 {
    match (call, phase) {
        (CallV1::Poll, _) => RouteV1::Poll,
        (CallV1::Wait, EntryPhaseV1::PublishedDirectional) => {
            RouteV1::NativeDirectionalWait
        }
        (CallV1::Wait, EntryPhaseV1::PublishedSameDevice) => {
            RouteV1::NativeSameDeviceWait
        }
        (CallV1::Wait, EntryPhaseV1::Ready)
        | (CallV1::Wait, EntryPhaseV1::Other) => RouteV1::LegacyWaitPoll,
    }
}

pub open spec fn route_for_kind_v1(kind: CopyKindV1) -> RouteV1 {
    if kind == CopyKindV1::Directional {
        RouteV1::NativeDirectionalWait
    } else {
        RouteV1::NativeSameDeviceWait
    }
}

pub open spec fn initial_state_v1(binding: BindingV1) -> StateV1 {
    StateV1 {
        binding,
        route: route_for_kind_v1(binding.kind),
        outcome: OutcomeV1::Pending,
        active_present: true,
        active_phase: ActivePhaseV1::Published(binding.kind),
        published_index_retained: true,
        published_index_frame: binding.published_index_frame,
        source_storage: StorageV1::InFlight(
            binding.submission,
            binding.source_storage_generation,
        ),
        destination_storage: StorageV1::InFlight(
            binding.submission,
            binding.destination_storage_generation,
        ),
        dependency_retain_count: binding.dependency_retain_count,
        source_custody_count: binding.source_custody_count,
        destination_custody_count: binding.destination_custody_count,
        stream_owner_count: binding.stream_owner_count,
        stream_current_retained: true,
        stream_frame: binding.stream_frame,
        native_custody: NativeCustodyV1::ActivePublished(binding.native_identity),
        terminal_poisoned: false,
        native_observations: 0,
        settled: false,
        completion_recorded: false,
        continuation_ready: false,
        continuation_publications: 0,
    }
}

pub open spec fn with_one_observation_v1(state: StateV1) -> StateV1 {
    StateV1 { native_observations: 1, ..state }
}

pub open spec fn terminal_state_v1(
    state: StateV1,
    custody: NativeCustodyV1,
) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Terminal,
        active_present: false,
        active_phase: ActivePhaseV1::Absent,
        published_index_retained: false,
        native_custody: custody,
        terminal_poisoned: true,
        ..state
    }
}

pub open spec fn restored_state_v1(state: StateV1) -> StateV1 {
    StateV1 {
        published_index_retained: false,
        source_storage: StorageV1::Restored(state.binding.restored_source_storage),
        destination_storage: StorageV1::Restored(state.binding.restored_destination_storage),
        native_custody: NativeCustodyV1::RestoredPair,
        ..state
    }
}

pub open spec fn decrement_v1(value: nat) -> nat {
    if value > 0 { (value - 1) as nat } else { 0 }
}

pub open spec fn settled_state_v1(state: StateV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Succeeded,
        active_present: false,
        active_phase: ActivePhaseV1::Absent,
        dependency_retain_count: decrement_v1(state.binding.dependency_retain_count),
        source_custody_count: decrement_v1(state.binding.source_custody_count),
        destination_custody_count: decrement_v1(state.binding.destination_custody_count),
        stream_owner_count: decrement_v1(state.binding.stream_owner_count),
        stream_current_retained: false,
        settled: true,
        completion_recorded: true,
        ..state
    }
}

pub open spec fn continuation_state_v1(state: StateV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Pending,
        active_present: true,
        active_phase: ActivePhaseV1::Ready,
        continuation_ready: true,
        ..state
    }
}

pub open spec fn execute_wait_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
) -> StateV1 {
    let state = with_one_observation_v1(initial_state_v1(binding));
    match observation {
        NativeWaitObservationV1::Complete => {
            let state = restored_state_v1(state);
            if completion == CompletionDispositionV1::Settle {
                settled_state_v1(state)
            } else {
                continuation_state_v1(state)
            }
        }
        NativeWaitObservationV1::ExactTypedTimeout(_) => state,
        NativeWaitObservationV1::NonTimeoutRetryable(identity) => {
            terminal_state_v1(state, NativeCustodyV1::TerminalPending(identity))
        }
        NativeWaitObservationV1::IdentityChange(stage, identity) => {
            let custody = if stage == IdentityChangeStageV1::Pending {
                NativeCustodyV1::TerminalPending(identity)
            } else {
                NativeCustodyV1::TerminalCompleted(identity)
            };
            terminal_state_v1(state, custody)
        }
        NativeWaitObservationV1::Teardown(token) => {
            terminal_state_v1(state, NativeCustodyV1::TerminalTeardown(token))
        }
    }
}

pub open spec fn same_operational_custody_v1(left: StateV1, right: StateV1) -> bool {
    &&& left.binding == right.binding
    &&& left.active_present == right.active_present
    &&& left.active_phase == right.active_phase
    &&& left.published_index_retained == right.published_index_retained
    &&& left.published_index_frame == right.published_index_frame
    &&& left.source_storage == right.source_storage
    &&& left.destination_storage == right.destination_storage
    &&& left.dependency_retain_count == right.dependency_retain_count
    &&& left.source_custody_count == right.source_custody_count
    &&& left.destination_custody_count == right.destination_custody_count
    &&& left.stream_owner_count == right.stream_owner_count
    &&& left.stream_current_retained == right.stream_current_retained
    &&& left.stream_frame == right.stream_frame
    &&& left.native_custody == right.native_custody
}

pub open spec fn terminal_preserves_in_flight_retains_v1(state: StateV1) -> bool {
    &&& state.source_storage == StorageV1::InFlight(
        state.binding.submission,
        state.binding.source_storage_generation,
    )
    &&& state.destination_storage == StorageV1::InFlight(
        state.binding.submission,
        state.binding.destination_storage_generation,
    )
    &&& state.dependency_retain_count == state.binding.dependency_retain_count
    &&& state.source_custody_count == state.binding.source_custody_count
    &&& state.destination_custody_count == state.binding.destination_custody_count
    &&& state.stream_owner_count == state.binding.stream_owner_count
    &&& state.stream_current_retained
    &&& state.stream_frame == state.binding.stream_frame
}

pub open spec fn recoverable_published_pending_v1(state: StateV1) -> bool {
    &&& state.outcome == OutcomeV1::Pending
    &&& state.active_present
    &&& state.active_phase == ActivePhaseV1::Published(state.binding.kind)
    &&& state.published_index_retained
    &&& !state.terminal_poisoned
}

pub open spec fn is_complete_v1(observation: NativeWaitObservationV1) -> bool {
    observation == NativeWaitObservationV1::Complete
}

pub open spec fn is_exact_timeout_v1(observation: NativeWaitObservationV1) -> bool {
    match observation {
        NativeWaitObservationV1::ExactTypedTimeout(_) => true,
        _ => false,
    }
}

pub proof fn explicit_poll_route_is_unchanged_v1(phase: EntryPhaseV1)
    ensures route_v1(CallV1::Poll, phase) == RouteV1::Poll,
{}

pub proof fn published_wait_activates_exact_native_route_v1(kind: CopyKindV1)
    ensures
        kind == CopyKindV1::Directional ==> route_v1(
            CallV1::Wait,
            EntryPhaseV1::PublishedDirectional,
        ) == RouteV1::NativeDirectionalWait,
        kind == CopyKindV1::SameDevice ==> route_v1(
            CallV1::Wait,
            EntryPhaseV1::PublishedSameDevice,
        ) == RouteV1::NativeSameDeviceWait,
{}

pub proof fn nonpublished_wait_keeps_legacy_poll_route_v1(phase: EntryPhaseV1)
    requires phase == EntryPhaseV1::Ready || phase == EntryPhaseV1::Other,
    ensures route_v1(CallV1::Wait, phase) == RouteV1::LegacyWaitPoll,
{}

pub proof fn wait_performs_exactly_one_native_observation_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_observation_v1(binding, observation),
    ensures execute_wait_v1(binding, deadline, observation, completion).native_observations == 1,
{}

pub proof fn zero_deadline_still_observes_once_v1(
    binding: BindingV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_observation_v1(binding, observation),
    ensures execute_wait_v1(
        binding,
        DeadlineV1::Zero,
        observation,
        completion,
    ).native_observations == 1,
{}

pub proof fn recoverable_published_pending_iff_exact_typed_timeout_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_observation_v1(binding, observation),
    ensures recoverable_published_pending_v1(
        execute_wait_v1(binding, deadline, observation, completion),
    ) <==> is_exact_timeout_v1(observation),
{}

pub proof fn exact_typed_timeout_restores_all_operational_custody_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    identity: NativeIdentityV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        identity == binding.native_identity,
    ensures
        same_operational_custody_v1(
            execute_wait_v1(
                binding,
                deadline,
                NativeWaitObservationV1::ExactTypedTimeout(identity),
                completion,
            ),
            initial_state_v1(binding),
        ),
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::ExactTypedTimeout(identity),
            completion,
        ).native_observations == 1,
{}

pub proof fn non_timeout_retryable_is_terminal_with_exact_pending_custody_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    identity: NativeIdentityV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        identity == binding.native_identity,
    ensures
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::NonTimeoutRetryable(identity),
            completion,
        ).outcome == OutcomeV1::Terminal,
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::NonTimeoutRetryable(identity),
            completion,
        ).native_custody == NativeCustodyV1::TerminalPending(identity),
        terminal_preserves_in_flight_retains_v1(execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::NonTimeoutRetryable(identity),
            completion,
        )),
{}

pub proof fn pending_identity_change_retains_returned_owner_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    returned: NativeIdentityV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_identity_v1(returned),
        returned != binding.native_identity,
    ensures
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::IdentityChange(
                IdentityChangeStageV1::Pending,
                returned,
            ),
            completion,
        ).native_custody == NativeCustodyV1::TerminalPending(returned),
        terminal_preserves_in_flight_retains_v1(execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::IdentityChange(
                IdentityChangeStageV1::Pending,
                returned,
            ),
            completion,
        )),
{}

pub proof fn completed_identity_change_retains_returned_owner_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    returned: NativeIdentityV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_identity_v1(returned),
        returned != binding.native_identity,
    ensures
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::IdentityChange(
                IdentityChangeStageV1::Completed,
                returned,
            ),
            completion,
        ).native_custody == NativeCustodyV1::TerminalCompleted(returned),
        terminal_preserves_in_flight_retains_v1(execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::IdentityChange(
                IdentityChangeStageV1::Completed,
                returned,
            ),
            completion,
        )),
{}

pub proof fn teardown_retains_opaque_terminal_custody_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    terminal_token: nat,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        terminal_token > 0,
    ensures
        execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::Teardown(terminal_token),
            completion,
        ).native_custody == NativeCustodyV1::TerminalTeardown(terminal_token),
        terminal_preserves_in_flight_retains_v1(execute_wait_v1(
            binding,
            deadline,
            NativeWaitObservationV1::Teardown(terminal_token),
            completion,
        )),
{}

pub proof fn complete_settlement_restores_storage_and_releases_once_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
)
    requires valid_binding_v1(binding),
    ensures
        {
            let state = execute_wait_v1(
                binding,
                deadline,
                NativeWaitObservationV1::Complete,
                CompletionDispositionV1::Settle,
            );
            &&& state.binding == binding
            &&& state.route == route_for_kind_v1(binding.kind)
            &&& state.outcome == OutcomeV1::Succeeded
            &&& !state.active_present
            &&& state.active_phase == ActivePhaseV1::Absent
            &&& !state.published_index_retained
            &&& state.published_index_frame == binding.published_index_frame
            &&& state.source_storage == StorageV1::Restored(binding.restored_source_storage)
            &&& state.destination_storage == StorageV1::Restored(binding.restored_destination_storage)
            &&& state.dependency_retain_count + 1 == binding.dependency_retain_count
            &&& state.source_custody_count + 1 == binding.source_custody_count
            &&& state.destination_custody_count + 1 == binding.destination_custody_count
            &&& state.stream_owner_count + 1 == binding.stream_owner_count
            &&& !state.stream_current_retained
            &&& state.stream_frame == binding.stream_frame
            &&& state.native_custody == NativeCustodyV1::RestoredPair
            &&& !state.terminal_poisoned
            &&& state.native_observations == 1
            &&& state.settled
            &&& state.completion_recorded
            &&& !state.continuation_ready
            &&& state.continuation_publications == 0
        },
{}

pub proof fn complete_continuation_is_ready_unpublished_and_retained_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
)
    requires valid_binding_v1(binding),
    ensures
        {
            let state = execute_wait_v1(
                binding,
                deadline,
                NativeWaitObservationV1::Complete,
                CompletionDispositionV1::ContinuationReady,
            );
            &&& state.binding == binding
            &&& state.route == route_for_kind_v1(binding.kind)
            &&& state.outcome == OutcomeV1::Pending
            &&& state.active_present
            &&& state.active_phase == ActivePhaseV1::Ready
            &&& !state.published_index_retained
            &&& state.published_index_frame == binding.published_index_frame
            &&& state.source_storage == StorageV1::Restored(binding.restored_source_storage)
            &&& state.destination_storage == StorageV1::Restored(binding.restored_destination_storage)
            &&& state.dependency_retain_count == binding.dependency_retain_count
            &&& state.source_custody_count == binding.source_custody_count
            &&& state.destination_custody_count == binding.destination_custody_count
            &&& state.stream_owner_count == binding.stream_owner_count
            &&& state.stream_current_retained
            &&& state.stream_frame == binding.stream_frame
            &&& state.native_custody == NativeCustodyV1::RestoredPair
            &&& !state.terminal_poisoned
            &&& state.native_observations == 1
            &&& !state.settled
            &&& !state.completion_recorded
            &&& state.continuation_ready
            &&& state.continuation_publications == 0
        },
{}

pub proof fn only_completion_can_settle_or_create_continuation_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_observation_v1(binding, observation),
    ensures
        {
            let state = execute_wait_v1(binding, deadline, observation, completion);
            (state.settled || state.continuation_ready) <==> is_complete_v1(observation)
        },
{}

pub proof fn terminal_outcomes_remove_only_operational_indexes_v1(
    binding: BindingV1,
    deadline: DeadlineV1,
    observation: NativeWaitObservationV1,
    completion: CompletionDispositionV1,
)
    requires
        valid_binding_v1(binding),
        valid_observation_v1(binding, observation),
        observation != NativeWaitObservationV1::Complete,
        !is_exact_timeout_v1(observation),
    ensures
        {
            let state = execute_wait_v1(binding, deadline, observation, completion);
            &&& state.outcome == OutcomeV1::Terminal
            &&& state.terminal_poisoned
            &&& !state.active_present
            &&& !state.published_index_retained
            &&& terminal_preserves_in_flight_retains_v1(state)
        },
{}

} // verus!
