// Independent finite R38 model for bounded persistent-compute wait/recycle.
// All identities, counts, bounds, failures, timing markers, and completion
// results are contracted mathematical inputs. This proves no Rust-to-Verus or
// production-Rust refinement and no KFD/HSA/HIP, native queue, driver,
// firmware, clock, hardware completion, coherence, progress, liveness, timing,
// parity, or performance property.
// Designed against signed production commit
// a1ea30cffbd24a5714a5fe0318b4231f42e98727.
// The route projection begins only after the earlier R37 active-SDMA guard has
// not matched. EntryPhaseV1::Other denotes another entry in that residual
// compute-routing domain; it excludes published directional and same-device
// SDMA operations, which use the R37 native route.
// Foreign/phase RetryablePreflight terminal observations are defensive
// contracted inputs. Their admission proves runtime-handler custody behavior,
// not their reachability after any particular Pending prefix.
// Every state-transition theorem below assumes the valid contracted
// binding/limits/script domain; the route lemmas prove only their finite route
// algebra.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub lane: nat,
    pub submission: nat,
    pub stream: nat,
    pub prior_stream_submission: nat,
    pub allocation: nat,
    pub allocation_storage_generation: nat,
    pub module: nat,
    pub dependency: nat,
    pub event: nat,
    pub dispatch_shape_digest: nat,
    pub queue_occurrence: nat,
    pub attachment_generation: nat,
    pub dispatch_generation: nat,
    pub completion_batch: nat,
    pub signal_generation: nat,
    pub next_signal_generation: nat,
    pub module_retain_count: nat,
    pub dependency_retain_count: nat,
    pub event_retain_count: nat,
    pub allocation_owner_count: nat,
    pub completion_reservation_count: nat,
    pub completion_midpoint: nat,
}

#[derive(PartialEq, Eq)]
pub enum CallV1 { Poll, Wait }

#[derive(PartialEq, Eq)]
pub enum EntryPhaseV1 { PublishedPersistent, PreparedPersistent, Materialized, Other }

#[derive(PartialEq, Eq)]
pub enum RouteV1 { Poll, BoundedPersistentWait, LegacyPollWait }

#[derive(PartialEq, Eq)]
pub enum DeadlineV1 { Zero, Positive(nat) }

#[derive(PartialEq, Eq)]
pub struct LimitsV1 {
    pub deadline: DeadlineV1,
    pub observation_max: nat,
}

#[derive(PartialEq, Eq)]
pub enum FailureStageV1 {
    PublishedState,
    DispatchGeneration,
    CompletionObservation,
    DispatchCompletion,
    AllocationCompletion,
    SignalGeneration,
    SignalReset,
    ClosingCurrentness,
    RecycleCurrentness,
    RecycleInfrastructure,
    DispatchRecycle,
}

#[derive(PartialEq, Eq)]
pub enum RetainedNativeStageV1 { Published, Completed, Recycled }

#[derive(PartialEq, Eq)]
pub enum RetryablePreflightV1 { Poll, Recycle }

#[derive(PartialEq, Eq)]
pub enum R36TerminalResultV1 {
    Recycled,
    RetryablePreflight(RetryablePreflightV1),
    ProcessTeardown(FailureStageV1, nat),
}

#[derive(PartialEq, Eq)]
pub struct WaitScriptV1 {
    pub pending_before_terminal: nat,
    pub terminal: R36TerminalResultV1,
}

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Pending, Recycled, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    Published,
    Completed,
    Recycled,
    ProcessTeardown(FailureStageV1, RetainedNativeStageV1, nat),
}

#[derive(PartialEq, Eq)]
pub enum ExecutionPhaseV1 { PublishedPersistent, Absent }

#[derive(PartialEq, Eq)]
pub enum AllocationStorageV1 { ComputeInFlight(nat, nat) }

#[derive(PartialEq, Eq)]
pub enum TimeoutReasonV1 { Deadline, ObservationMaximum }

pub struct StateV1 {
    pub binding: BindingV1,
    pub route: RouteV1,
    pub outcome: OutcomeV1,
    pub custody: CustodyV1,
    pub failure_stage: Option<FailureStageV1>,
    pub terminal_poisoned: bool,
    pub active_present: bool,
    pub active_execution: ExecutionPhaseV1,
    pub active_lane: Option<nat>,
    pub active_submission: Option<nat>,
    pub lane_submission: Option<nat>,
    pub lane_stream: Option<nat>,
    pub allocation_storage: AllocationStorageV1,
    pub module_retain_count: nat,
    pub dependency_retain_count: nat,
    pub event_retain_count: nat,
    pub allocation_owner_count: nat,
    pub allocation_current_owner: Option<nat>,
    pub stream_tail_submission: Option<nat>,
    pub stream_current_owner: Option<nat>,
    pub completion_reservation_count: nat,
    pub submission_recorded: bool,
    pub observations: nat,
    pub timeout_reason: Option<TimeoutReasonV1>,
    pub r36_composition_count: nat,
    pub completion_midpoint: Option<nat>,
    pub r36_poll_ready: bool,
    pub r36_recycle_finished: bool,
    pub published_authority_count: nat,
    pub completed_authority_count: nat,
    pub recycled_authority_count: nat,
    pub teardown_authority_count: nat,
}

// Mathematical proof carriers are copyable and establish no Rust ownership.
pub struct PublishedAuthorityV1 { pub binding: BindingV1 }
pub struct CompletedAuthorityV1 { pub binding: BindingV1 }
pub struct RecycledAuthorityV1 { pub binding: BindingV1 }
pub struct TeardownAuthorityV1 { pub binding: BindingV1 }

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.lane < 3
    &&& binding.submission > 0
    &&& binding.stream > 0
    &&& binding.prior_stream_submission > 0
    &&& binding.prior_stream_submission != binding.submission
    &&& binding.allocation > 0
    &&& binding.allocation_storage_generation > 0
    &&& binding.module > 0
    &&& binding.dependency > 0
    &&& binding.event > 0
    &&& binding.dispatch_shape_digest > 0
    &&& binding.queue_occurrence > 0
    &&& binding.attachment_generation > 0
    &&& binding.dispatch_generation > 0
    &&& binding.completion_batch > 0
    &&& binding.signal_generation > 0
    &&& binding.next_signal_generation == binding.signal_generation + 1
    &&& binding.module_retain_count > 0
    &&& binding.dependency_retain_count > 0
    &&& binding.event_retain_count > 0
    &&& binding.allocation_owner_count > 0
    &&& binding.completion_reservation_count > 0
    &&& binding.completion_midpoint > 0
}

pub open spec fn valid_deadline_v1(deadline: DeadlineV1) -> bool {
    match deadline {
        DeadlineV1::Zero => true,
        DeadlineV1::Positive(limit) => limit > 0 && limit <= 2,
    }
}

pub open spec fn valid_limits_v1(limits: LimitsV1) -> bool {
    valid_deadline_v1(limits.deadline)
        && limits.observation_max > 0
        && limits.observation_max <= 3
}

pub open spec fn valid_terminal_v1(terminal: R36TerminalResultV1) -> bool {
    match terminal {
        R36TerminalResultV1::Recycled | R36TerminalResultV1::RetryablePreflight(_) => true,
        R36TerminalResultV1::ProcessTeardown(_, token) => token > 0,
    }
}

pub open spec fn valid_script_v1(script: WaitScriptV1) -> bool {
    script.pending_before_terminal <= 2 && valid_terminal_v1(script.terminal)
}

// This selector's domain is residual compute routing after the active-SDMA
// guard has missed; Other has that scoped meaning.
pub open spec fn route_v1(call: CallV1, phase: EntryPhaseV1) -> RouteV1 {
    match (call, phase) {
        (CallV1::Poll, _) => RouteV1::Poll,
        (CallV1::Wait, EntryPhaseV1::PublishedPersistent) => RouteV1::BoundedPersistentWait,
        (CallV1::Wait, EntryPhaseV1::PreparedPersistent)
        | (CallV1::Wait, EntryPhaseV1::Materialized)
        | (CallV1::Wait, EntryPhaseV1::Other) => RouteV1::LegacyPollWait,
    }
}

pub open spec fn deadline_limit_v1(deadline: DeadlineV1) -> nat {
    match deadline {
        DeadlineV1::Zero => 1,
        DeadlineV1::Positive(limit) => limit,
    }
}

pub open spec fn stop_after_pending_v1(limits: LimitsV1) -> nat {
    if limits.observation_max <= deadline_limit_v1(limits.deadline) {
        limits.observation_max
    } else {
        deadline_limit_v1(limits.deadline)
    }
}

pub open spec fn timeout_reason_v1(limits: LimitsV1) -> TimeoutReasonV1 {
    if limits.observation_max <= deadline_limit_v1(limits.deadline) {
        TimeoutReasonV1::ObservationMaximum
    } else {
        TimeoutReasonV1::Deadline
    }
}

pub open spec fn retained_native_stage_v1(stage: FailureStageV1) -> RetainedNativeStageV1 {
    match stage {
        FailureStageV1::PublishedState
        | FailureStageV1::DispatchGeneration
        | FailureStageV1::CompletionObservation => RetainedNativeStageV1::Published,
        FailureStageV1::DispatchCompletion
        | FailureStageV1::AllocationCompletion
        | FailureStageV1::SignalGeneration
        | FailureStageV1::SignalReset
        | FailureStageV1::ClosingCurrentness
        | FailureStageV1::RecycleCurrentness
        | FailureStageV1::RecycleInfrastructure => RetainedNativeStageV1::Completed,
        FailureStageV1::DispatchRecycle => RetainedNativeStageV1::Recycled,
    }
}

pub open spec fn stage_observes_midpoint_v1(stage: FailureStageV1) -> bool {
    match stage {
        FailureStageV1::SignalGeneration
        | FailureStageV1::SignalReset
        | FailureStageV1::ClosingCurrentness
        | FailureStageV1::RecycleCurrentness
        | FailureStageV1::RecycleInfrastructure
        | FailureStageV1::DispatchRecycle => true,
        _ => false,
    }
}

pub open spec fn initial_state_v1(binding: BindingV1) -> StateV1 {
    StateV1 {
        binding,
        route: RouteV1::BoundedPersistentWait,
        outcome: OutcomeV1::Pending,
        custody: CustodyV1::Published,
        failure_stage: None,
        terminal_poisoned: false,
        active_present: true,
        active_execution: ExecutionPhaseV1::PublishedPersistent,
        active_lane: Some(binding.lane),
        active_submission: Some(binding.submission),
        lane_submission: Some(binding.submission),
        lane_stream: Some(binding.stream),
        allocation_storage: AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        ),
        module_retain_count: binding.module_retain_count,
        dependency_retain_count: binding.dependency_retain_count,
        event_retain_count: binding.event_retain_count,
        allocation_owner_count: binding.allocation_owner_count,
        allocation_current_owner: Some(binding.submission),
        stream_tail_submission: Some(binding.submission),
        stream_current_owner: Some(binding.submission),
        completion_reservation_count: binding.completion_reservation_count,
        submission_recorded: false,
        observations: 0,
        timeout_reason: None,
        r36_composition_count: 0,
        completion_midpoint: None,
        r36_poll_ready: false,
        r36_recycle_finished: false,
        published_authority_count: 1,
        completed_authority_count: 0,
        recycled_authority_count: 0,
        teardown_authority_count: 0,
    }
}

pub open spec fn missing_queue_state_v1(state: StateV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Terminal,
        terminal_poisoned: true,
        active_present: false,
        active_execution: ExecutionPhaseV1::Absent,
        active_lane: None,
        active_submission: None,
        lane_submission: None,
        ..state
    }
}

pub open spec fn timeout_state_v1(
    state: StateV1,
    observations: nat,
    reason: TimeoutReasonV1,
) -> StateV1 {
    StateV1 {
        observations,
        timeout_reason: Some(reason),
        r36_composition_count: observations,
        ..state
    }
}

pub open spec fn recycled_state_v1(state: StateV1, observations: nat) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Recycled,
        custody: CustodyV1::Recycled,
        active_present: false,
        active_execution: ExecutionPhaseV1::Absent,
        active_lane: None,
        active_submission: None,
        lane_submission: None,
        observations,
        r36_composition_count: observations,
        completion_midpoint: Some(state.binding.completion_midpoint),
        r36_poll_ready: true,
        r36_recycle_finished: true,
        published_authority_count: 0,
        completed_authority_count: 0,
        recycled_authority_count: 1,
        teardown_authority_count: 0,
        ..state
    }
}

pub open spec fn consume_lower_retryable_preflight_state_v1(
    state: StateV1,
    observations: nat,
    preflight: RetryablePreflightV1,
) -> StateV1 {
    let custody = match preflight {
        RetryablePreflightV1::Poll => CustodyV1::Published,
        RetryablePreflightV1::Recycle => CustodyV1::Completed,
    };
    StateV1 {
        outcome: OutcomeV1::Terminal,
        custody,
        failure_stage: None,
        terminal_poisoned: true,
        active_present: false,
        active_execution: ExecutionPhaseV1::Absent,
        active_lane: None,
        active_submission: None,
        lane_submission: None,
        observations,
        r36_composition_count: observations,
        completion_midpoint: if preflight == RetryablePreflightV1::Recycle {
            Some(state.binding.completion_midpoint)
        } else {
            None
        },
        r36_poll_ready: preflight == RetryablePreflightV1::Recycle,
        published_authority_count: if custody == CustodyV1::Published { 1 } else { 0 },
        completed_authority_count: if custody == CustodyV1::Completed { 1 } else { 0 },
        recycled_authority_count: 0,
        teardown_authority_count: 0,
        ..state
    }
}

pub open spec fn process_teardown_state_v1(
    state: StateV1,
    observations: nat,
    stage: FailureStageV1,
    terminal_token: nat,
) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Terminal,
        custody: CustodyV1::ProcessTeardown(
            stage,
            retained_native_stage_v1(stage),
            terminal_token,
        ),
        failure_stage: Some(stage),
        terminal_poisoned: true,
        active_present: false,
        active_execution: ExecutionPhaseV1::Absent,
        active_lane: None,
        active_submission: None,
        lane_submission: None,
        observations,
        r36_composition_count: observations,
        completion_midpoint: if stage_observes_midpoint_v1(stage) {
            Some(state.binding.completion_midpoint)
        } else {
            None
        },
        r36_poll_ready: stage_observes_midpoint_v1(stage),
        published_authority_count: 0,
        completed_authority_count: 0,
        recycled_authority_count: 0,
        teardown_authority_count: 1,
        ..state
    }
}

pub open spec fn execute_wait_v1(
    binding: BindingV1,
    queue_present: bool,
    limits: LimitsV1,
    script: WaitScriptV1,
) -> StateV1 {
    let state = initial_state_v1(binding);
    if !queue_present {
        missing_queue_state_v1(state)
    } else {
        let terminal_observation = script.pending_before_terminal + 1;
        let stop = stop_after_pending_v1(limits);
        if terminal_observation > stop {
            timeout_state_v1(state, stop, timeout_reason_v1(limits))
        } else {
            match script.terminal {
                R36TerminalResultV1::Recycled => recycled_state_v1(state, terminal_observation),
                R36TerminalResultV1::RetryablePreflight(preflight) => {
                    consume_lower_retryable_preflight_state_v1(
                        state,
                        terminal_observation,
                        preflight,
                    )
                }
                R36TerminalResultV1::ProcessTeardown(stage, token) => {
                    process_teardown_state_v1(state, terminal_observation, stage, token)
                }
            }
        }
    }
}

pub open spec fn terminal_reached_v1(limits: LimitsV1, script: WaitScriptV1) -> bool {
    script.pending_before_terminal + 1 <= stop_after_pending_v1(limits)
}

pub open spec fn exactly_one_authority_v1(state: StateV1) -> bool {
    state.published_authority_count
        + state.completed_authority_count
        + state.recycled_authority_count
        + state.teardown_authority_count == 1
}

pub proof fn explicit_poll_route_is_unchanged_v1(phase: EntryPhaseV1)
    ensures route_v1(CallV1::Poll, phase) == RouteV1::Poll,
{}

pub proof fn published_wait_uses_bounded_persistent_route_v1()
    ensures route_v1(CallV1::Wait, EntryPhaseV1::PublishedPersistent)
        == RouteV1::BoundedPersistentWait,
{}

pub proof fn prepared_wait_keeps_legacy_poll_route_v1()
    ensures route_v1(CallV1::Wait, EntryPhaseV1::PreparedPersistent) == RouteV1::LegacyPollWait,
{}

pub proof fn materialized_wait_keeps_legacy_poll_route_v1()
    ensures route_v1(CallV1::Wait, EntryPhaseV1::Materialized) == RouteV1::LegacyPollWait,
{}

pub proof fn other_wait_keeps_legacy_poll_route_v1()
    ensures route_v1(CallV1::Wait, EntryPhaseV1::Other) == RouteV1::LegacyPollWait,
{}

pub proof fn valid_stop_boundary_is_positive_v1(limits: LimitsV1)
    requires valid_limits_v1(limits),
    ensures stop_after_pending_v1(limits) > 0,
{}

pub proof fn present_queue_always_observes_before_termination_v1(
    binding: BindingV1,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires valid_binding_v1(binding), valid_limits_v1(limits), valid_script_v1(script),
    ensures execute_wait_v1(binding, true, limits, script).observations > 0,
{}

pub proof fn zero_deadline_performs_exactly_one_observation_v1(
    binding: BindingV1,
    observation_max: nat,
    script: WaitScriptV1,
)
    requires
        valid_binding_v1(binding),
        observation_max > 0,
        observation_max <= 3,
        valid_script_v1(script),
    ensures execute_wait_v1(
        binding,
        true,
        LimitsV1 { deadline: DeadlineV1::Zero, observation_max },
        script,
    ).observations == 1,
{}

pub proof fn observation_count_never_exceeds_maximum_v1(
    binding: BindingV1,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires valid_binding_v1(binding), valid_limits_v1(limits), valid_script_v1(script),
    ensures execute_wait_v1(binding, true, limits, script).observations <= limits.observation_max,
{}

pub proof fn timeout_restores_every_published_coordinate_v1(
    binding: BindingV1,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        valid_script_v1(script),
        !terminal_reached_v1(limits, script),
    ensures {
        let state = execute_wait_v1(binding, true, limits, script);
        &&& state.binding == binding
        &&& state.route == RouteV1::BoundedPersistentWait
        &&& state.outcome == OutcomeV1::Pending
        &&& state.custody == CustodyV1::Published
        &&& state.failure_stage == None
        &&& !state.terminal_poisoned
        &&& state.active_present
        &&& state.active_execution == ExecutionPhaseV1::PublishedPersistent
        &&& state.active_lane == Some(binding.lane)
        &&& state.active_submission == Some(binding.submission)
        &&& state.lane_submission == Some(binding.submission)
        &&& state.lane_stream == Some(binding.stream)
        &&& state.allocation_storage == AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        )
        &&& state.module_retain_count == binding.module_retain_count
        &&& state.dependency_retain_count == binding.dependency_retain_count
        &&& state.event_retain_count == binding.event_retain_count
        &&& state.allocation_owner_count == binding.allocation_owner_count
        &&& state.allocation_current_owner == Some(binding.submission)
        &&& state.stream_tail_submission == Some(binding.submission)
        &&& state.stream_current_owner == Some(binding.submission)
        &&& state.completion_reservation_count == binding.completion_reservation_count
        &&& !state.submission_recorded
        &&& state.observations == stop_after_pending_v1(limits)
        &&& state.timeout_reason == Some(timeout_reason_v1(limits))
        &&& state.r36_composition_count == state.observations
        &&& state.completion_midpoint == None
        &&& !state.r36_poll_ready
        &&& !state.r36_recycle_finished
        &&& state.published_authority_count == 1
        &&& state.completed_authority_count == 0
        &&& state.recycled_authority_count == 0
        &&& state.teardown_authority_count == 0
    },
{}

pub proof fn ready_composes_through_r36_recycle_with_every_coordinate_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
    ensures {
        let script = WaitScriptV1 {
            pending_before_terminal,
            terminal: R36TerminalResultV1::Recycled,
        };
        let state = execute_wait_v1(binding, true, limits, script);
        &&& state.binding == binding
        &&& state.route == RouteV1::BoundedPersistentWait
        &&& state.outcome == OutcomeV1::Recycled
        &&& state.custody == CustodyV1::Recycled
        &&& state.failure_stage == None
        &&& !state.terminal_poisoned
        &&& !state.active_present
        &&& state.active_execution == ExecutionPhaseV1::Absent
        &&& state.active_lane == None
        &&& state.active_submission == None
        &&& state.lane_submission == None
        &&& state.lane_stream == Some(binding.stream)
        &&& state.allocation_storage == AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        )
        &&& state.module_retain_count == binding.module_retain_count
        &&& state.dependency_retain_count == binding.dependency_retain_count
        &&& state.event_retain_count == binding.event_retain_count
        &&& state.allocation_owner_count == binding.allocation_owner_count
        &&& state.allocation_current_owner == Some(binding.submission)
        &&& state.stream_tail_submission == Some(binding.submission)
        &&& state.stream_current_owner == Some(binding.submission)
        &&& state.completion_reservation_count == binding.completion_reservation_count
        &&& !state.submission_recorded
        &&& state.observations == pending_before_terminal + 1
        &&& state.timeout_reason == None
        &&& state.r36_composition_count == state.observations
        &&& state.completion_midpoint == Some(binding.completion_midpoint)
        &&& state.r36_poll_ready
        &&& state.r36_recycle_finished
        &&& state.published_authority_count == 0
        &&& state.completed_authority_count == 0
        &&& state.recycled_authority_count == 1
        &&& state.teardown_authority_count == 0
    },
{}

pub proof fn lower_retryable_preflight_is_consumed_into_terminal_retained_custody_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
    preflight: RetryablePreflightV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
    ensures {
        let state = execute_wait_v1(
            binding,
            true,
            limits,
            WaitScriptV1 {
                pending_before_terminal,
                terminal: R36TerminalResultV1::RetryablePreflight(preflight),
            },
        );
        let custody = if preflight == RetryablePreflightV1::Poll {
            CustodyV1::Published
        } else {
            CustodyV1::Completed
        };
        &&& state.binding == binding
        &&& state.route == RouteV1::BoundedPersistentWait
        &&& state.outcome == OutcomeV1::Terminal
        &&& state.custody == custody
        &&& state.failure_stage == None
        &&& state.terminal_poisoned
        &&& !state.active_present
        &&& state.active_execution == ExecutionPhaseV1::Absent
        &&& state.active_lane == None
        &&& state.active_submission == None
        &&& state.lane_submission == None
        &&& state.lane_stream == Some(binding.stream)
        &&& state.allocation_storage == AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        )
        &&& state.module_retain_count == binding.module_retain_count
        &&& state.dependency_retain_count == binding.dependency_retain_count
        &&& state.event_retain_count == binding.event_retain_count
        &&& state.allocation_owner_count == binding.allocation_owner_count
        &&& state.allocation_current_owner == Some(binding.submission)
        &&& state.stream_tail_submission == Some(binding.submission)
        &&& state.stream_current_owner == Some(binding.submission)
        &&& state.completion_reservation_count == binding.completion_reservation_count
        &&& !state.submission_recorded
        &&& state.observations == pending_before_terminal + 1
        &&& state.timeout_reason == None
        &&& state.r36_composition_count == state.observations
        &&& state.completion_midpoint == if preflight == RetryablePreflightV1::Recycle {
            Some(binding.completion_midpoint)
        } else {
            None
        }
        &&& state.r36_poll_ready == (preflight == RetryablePreflightV1::Recycle)
        &&& !state.r36_recycle_finished
        &&& state.published_authority_count
            == if preflight == RetryablePreflightV1::Poll { 1nat } else { 0nat }
        &&& state.completed_authority_count
            == if preflight == RetryablePreflightV1::Recycle { 1nat } else { 0nat }
        &&& state.recycled_authority_count == 0
        &&& state.teardown_authority_count == 0
    },
{}

pub proof fn teardown_failure_has_exact_opaque_custody_and_every_coordinate_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
    stage: FailureStageV1,
    terminal_token: nat,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
        terminal_token > 0,
    ensures {
        let state = execute_wait_v1(
            binding,
            true,
            limits,
            WaitScriptV1 {
                pending_before_terminal,
                terminal: R36TerminalResultV1::ProcessTeardown(stage, terminal_token),
            },
        );
        &&& state.binding == binding
        &&& state.route == RouteV1::BoundedPersistentWait
        &&& state.outcome == OutcomeV1::Terminal
        &&& state.custody == CustodyV1::ProcessTeardown(
            stage,
            retained_native_stage_v1(stage),
            terminal_token,
        )
        &&& state.failure_stage == Some(stage)
        &&& state.terminal_poisoned
        &&& !state.active_present
        &&& state.active_execution == ExecutionPhaseV1::Absent
        &&& state.active_lane == None
        &&& state.active_submission == None
        &&& state.lane_submission == None
        &&& state.lane_stream == Some(binding.stream)
        &&& state.allocation_storage == AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        )
        &&& state.module_retain_count == binding.module_retain_count
        &&& state.dependency_retain_count == binding.dependency_retain_count
        &&& state.event_retain_count == binding.event_retain_count
        &&& state.allocation_owner_count == binding.allocation_owner_count
        &&& state.allocation_current_owner == Some(binding.submission)
        &&& state.stream_tail_submission == Some(binding.submission)
        &&& state.stream_current_owner == Some(binding.submission)
        &&& state.completion_reservation_count == binding.completion_reservation_count
        &&& !state.submission_recorded
        &&& state.observations == pending_before_terminal + 1
        &&& state.timeout_reason == None
        &&& state.r36_composition_count == state.observations
        &&& state.completion_midpoint == if stage_observes_midpoint_v1(stage) {
            Some(binding.completion_midpoint)
        } else {
            None
        }
        &&& state.r36_poll_ready == stage_observes_midpoint_v1(stage)
        &&& !state.r36_recycle_finished
        &&& state.published_authority_count == 0
        &&& state.completed_authority_count == 0
        &&& state.recycled_authority_count == 0
        &&& state.teardown_authority_count == 1
    },
{}

pub proof fn missing_queue_retains_published_authority_and_every_coordinate_v1(
    binding: BindingV1,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires valid_binding_v1(binding), valid_limits_v1(limits), valid_script_v1(script),
    ensures {
        let state = execute_wait_v1(binding, false, limits, script);
        &&& state.binding == binding
        &&& state.route == RouteV1::BoundedPersistentWait
        &&& state.outcome == OutcomeV1::Terminal
        &&& state.custody == CustodyV1::Published
        &&& state.failure_stage == None
        &&& state.terminal_poisoned
        &&& !state.active_present
        &&& state.active_execution == ExecutionPhaseV1::Absent
        &&& state.active_lane == None
        &&& state.active_submission == None
        &&& state.lane_submission == None
        &&& state.lane_stream == Some(binding.stream)
        &&& state.allocation_storage == AllocationStorageV1::ComputeInFlight(
            binding.submission,
            binding.allocation_storage_generation,
        )
        &&& state.module_retain_count == binding.module_retain_count
        &&& state.dependency_retain_count == binding.dependency_retain_count
        &&& state.event_retain_count == binding.event_retain_count
        &&& state.allocation_owner_count == binding.allocation_owner_count
        &&& state.allocation_current_owner == Some(binding.submission)
        &&& state.stream_tail_submission == Some(binding.submission)
        &&& state.stream_current_owner == Some(binding.submission)
        &&& state.completion_reservation_count == binding.completion_reservation_count
        &&& !state.submission_recorded
        &&& state.observations == 0
        &&& state.timeout_reason == None
        &&& state.r36_composition_count == 0
        &&& state.completion_midpoint == None
        &&& !state.r36_poll_ready
        &&& !state.r36_recycle_finished
        &&& state.published_authority_count == 1
        &&& state.completed_authority_count == 0
        &&& state.recycled_authority_count == 0
        &&& state.teardown_authority_count == 0
    },
{}

pub proof fn every_result_has_exactly_one_stage_authority_v1(
    binding: BindingV1,
    queue_present: bool,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires valid_binding_v1(binding), valid_limits_v1(limits), valid_script_v1(script),
    ensures exactly_one_authority_v1(execute_wait_v1(binding, queue_present, limits, script)),
{}

pub proof fn pre_ready_failures_have_no_midpoint_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
    stage: FailureStageV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
        !stage_observes_midpoint_v1(stage),
    ensures execute_wait_v1(
        binding,
        true,
        limits,
        WaitScriptV1 {
            pending_before_terminal,
            terminal: R36TerminalResultV1::ProcessTeardown(stage, 1),
        },
    ).completion_midpoint == None,
{}

pub proof fn recycle_failures_capture_midpoint_before_terminal_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
    stage: FailureStageV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
        stage_observes_midpoint_v1(stage),
    ensures {
        let state = execute_wait_v1(
            binding,
            true,
            limits,
            WaitScriptV1 {
                pending_before_terminal,
                terminal: R36TerminalResultV1::ProcessTeardown(stage, 1),
            },
        );
        &&& state.completion_midpoint == Some(binding.completion_midpoint)
        &&& state.r36_poll_ready
        &&& !state.r36_recycle_finished
    },
{}

pub proof fn timeout_never_reaches_ready_or_recycle_v1(
    binding: BindingV1,
    limits: LimitsV1,
    script: WaitScriptV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        valid_script_v1(script),
        !terminal_reached_v1(limits, script),
    ensures {
        let state = execute_wait_v1(binding, true, limits, script);
        &&& state.completion_midpoint == None
        &&& !state.r36_poll_ready
        &&& !state.r36_recycle_finished
        &&& state.observations == state.r36_composition_count
    },
{}

pub proof fn terminal_failure_never_reports_recycle_success_v1(
    binding: BindingV1,
    limits: LimitsV1,
    pending_before_terminal: nat,
    terminal: R36TerminalResultV1,
)
    requires
        valid_binding_v1(binding),
        valid_limits_v1(limits),
        pending_before_terminal <= 2,
        valid_terminal_v1(terminal),
        terminal != R36TerminalResultV1::Recycled,
        pending_before_terminal + 1 <= stop_after_pending_v1(limits),
    ensures !execute_wait_v1(
        binding,
        true,
        limits,
        WaitScriptV1 {
            pending_before_terminal,
            terminal,
        },
    ).r36_recycle_finished,
{}

} // verus!
