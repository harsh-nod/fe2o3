//! Independent executable R38 model for bounded persistent-compute wait/recycle.
//!
//! A published persistent dispatch is observed through a finite `Pending^n`
//! prefix followed by one abstract R36 fused poll/recycle result. Deadline and
//! observation-maximum boundaries are checked only after a Pending result, so
//! even a zero deadline performs its first observation. The transition is a
//! constant-time closed form; it performs no polling or I/O.
//!
//! The model was designed against signed production commit
//! `a1ea30cffbd24a5714a5fe0318b4231f42e98727`.
//! Its route projection begins only after the earlier R37 active-SDMA guard has
//! not matched. Consequently, `R38EntryPhaseV1::Other` means another entry in
//! that residual compute-routing domain; it does not include a published
//! directional or same-device SDMA operation, which uses the R37 native route.
//! Lower foreign/phase retryable preflights are defensive contracted inputs to
//! the runtime-handler projection. Their inclusion does not prove that those
//! outcomes are reachable after one or more Pending observations.
//! The executable owner is affine: `run_model_only` consumes one transition
//! attempt. Explicitly dropping the owner, or consuming it through an invalid
//! limits/script error, does not preserve or prove custody in this model.
//!
//! Identities, counts, timing markers, failures, and completion truth are
//! contracted mathematical inputs. This model does not refine production Rust,
//! KFD, HSA, HIP, a native queue, a clock, driver or firmware behavior, hardware
//! completion or coherence, progress, liveness, timing, parity, or performance.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R38PersistentWaitBindingV1 {
    pub lane: u8,
    pub submission: u64,
    pub stream: u64,
    pub prior_stream_submission: u64,
    pub allocation: u64,
    pub allocation_storage_generation: u64,
    pub module: u64,
    pub dependency: u64,
    pub event: u64,
    pub dispatch_shape_digest: u64,
    pub queue_occurrence: u64,
    pub attachment_generation: u64,
    pub dispatch_generation: u64,
    pub completion_batch: u64,
    pub signal_generation: u64,
    pub next_signal_generation: u64,
    pub module_retain_count: u8,
    pub dependency_retain_count: u8,
    pub event_retain_count: u8,
    pub allocation_owner_count: u8,
    pub completion_reservation_count: u8,
    pub completion_midpoint: u64,
}

impl R38PersistentWaitBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.lane < 3
            && self.submission != 0
            && self.stream != 0
            && self.prior_stream_submission != 0
            && self.prior_stream_submission != self.submission
            && self.allocation != 0
            && self.allocation_storage_generation != 0
            && self.module != 0
            && self.dependency != 0
            && self.event != 0
            && self.dispatch_shape_digest != 0
            && self.queue_occurrence != 0
            && self.attachment_generation != 0
            && self.dispatch_generation != 0
            && self.completion_batch != 0
            && self.signal_generation != 0
            && matches!(self.signal_generation.checked_add(1), Some(next)
                if next == self.next_signal_generation)
            && self.module_retain_count != 0
            && self.dependency_retain_count != 0
            && self.event_retain_count != 0
            && self.allocation_owner_count != 0
            && self.completion_reservation_count != 0
            && self.completion_midpoint != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38CallV1 {
    Poll,
    Wait,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38EntryPhaseV1 {
    PublishedPersistent,
    PreparedPersistent,
    Materialized,
    /// A residual compute-route entry after the active-SDMA guard has missed.
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38RouteV1 {
    Poll,
    BoundedPersistentWait,
    LegacyPollWait,
}

/// Selects a route within the residual domain after the R37 active-SDMA guard.
pub const fn r38_route_model_only(call: R38CallV1, phase: R38EntryPhaseV1) -> R38RouteV1 {
    match (call, phase) {
        (R38CallV1::Poll, _) => R38RouteV1::Poll,
        (R38CallV1::Wait, R38EntryPhaseV1::PublishedPersistent) => {
            R38RouteV1::BoundedPersistentWait
        }
        (
            R38CallV1::Wait,
            R38EntryPhaseV1::PreparedPersistent
            | R38EntryPhaseV1::Materialized
            | R38EntryPhaseV1::Other,
        ) => R38RouteV1::LegacyPollWait,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38DeadlineV1 {
    Zero,
    Positive { pending_observation_limit: u8 },
}

impl R38DeadlineV1 {
    pub const fn is_valid(self) -> bool {
        match self {
            Self::Zero => true,
            Self::Positive {
                pending_observation_limit,
            } => pending_observation_limit > 0 && pending_observation_limit <= 2,
        }
    }

    const fn pending_observation_limit(self) -> u8 {
        match self {
            Self::Zero => 1,
            Self::Positive {
                pending_observation_limit,
            } => pending_observation_limit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R38WaitLimitsV1 {
    pub deadline: R38DeadlineV1,
    /// Finite stand-in for the production `u64::MAX` saturation boundary.
    pub observation_max: u8,
}

impl R38WaitLimitsV1 {
    pub const fn is_valid(self) -> bool {
        self.deadline.is_valid() && self.observation_max > 0 && self.observation_max <= 3
    }

    const fn stop_after_pending(self) -> (u8, R38TimeoutReasonV1) {
        let deadline = self.deadline.pending_observation_limit();
        if self.observation_max <= deadline {
            (self.observation_max, R38TimeoutReasonV1::ObservationMaximum)
        } else {
            (deadline, R38TimeoutReasonV1::Deadline)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38RetainedNativeStageV1 {
    Published,
    Completed,
    Recycled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38FailureStageV1 {
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

impl R38FailureStageV1 {
    pub const fn retained_native_stage(self) -> R38RetainedNativeStageV1 {
        match self {
            Self::PublishedState | Self::DispatchGeneration | Self::CompletionObservation => {
                R38RetainedNativeStageV1::Published
            }
            Self::DispatchCompletion
            | Self::AllocationCompletion
            | Self::SignalGeneration
            | Self::SignalReset
            | Self::ClosingCurrentness
            | Self::RecycleCurrentness
            | Self::RecycleInfrastructure => R38RetainedNativeStageV1::Completed,
            Self::DispatchRecycle => R38RetainedNativeStageV1::Recycled,
        }
    }

    pub const fn observes_ready_midpoint(self) -> bool {
        matches!(
            self,
            Self::SignalGeneration
                | Self::SignalReset
                | Self::ClosingCurrentness
                | Self::RecycleCurrentness
                | Self::RecycleInfrastructure
                | Self::DispatchRecycle
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38RetryablePreflightV1 {
    Poll,
    Recycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38R36TerminalResultV1 {
    Recycled,
    RetryablePreflight(R38RetryablePreflightV1),
    ProcessTeardown {
        stage: R38FailureStageV1,
        terminal_token: u64,
    },
}

impl R38R36TerminalResultV1 {
    pub const fn is_valid(self) -> bool {
        match self {
            Self::Recycled | Self::RetryablePreflight(_) => true,
            Self::ProcessTeardown { terminal_token, .. } => terminal_token != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R38WaitScriptV1 {
    pub pending_before_terminal: u8,
    pub terminal: R38R36TerminalResultV1,
}

impl R38WaitScriptV1 {
    pub const fn is_valid(self) -> bool {
        self.pending_before_terminal <= 2 && self.terminal.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38OutcomeV1 {
    Pending,
    Recycled,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38TerminalCustodyV1 {
    Published,
    Completed,
    Recycled,
    ProcessTeardown {
        stage: R38FailureStageV1,
        retained_native_stage: R38RetainedNativeStageV1,
        terminal_token: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38ExecutionPhaseV1 {
    PublishedPersistent,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38AllocationStorageV1 {
    ComputeInFlight { submission: u64, generation: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38TimeoutReasonV1 {
    Deadline,
    ObservationMaximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R38WaitSnapshotV1 {
    pub binding: R38PersistentWaitBindingV1,
    pub route: R38RouteV1,
    pub outcome: R38OutcomeV1,
    pub custody: R38TerminalCustodyV1,
    pub failure_stage: Option<R38FailureStageV1>,
    pub terminal_poisoned: bool,
    pub active_present: bool,
    pub active_execution: R38ExecutionPhaseV1,
    pub active_lane: Option<u8>,
    pub active_submission: Option<u64>,
    pub lane_submission: Option<u64>,
    pub lane_stream: Option<u64>,
    pub allocation_storage: R38AllocationStorageV1,
    pub module_retain_count: u8,
    pub dependency_retain_count: u8,
    pub event_retain_count: u8,
    pub allocation_owner_count: u8,
    pub allocation_current_owner: Option<u64>,
    pub stream_tail_submission: Option<u64>,
    pub stream_current_owner: Option<u64>,
    pub completion_reservation_count: u8,
    pub submission_recorded: bool,
    pub observations: u8,
    pub timeout_reason: Option<R38TimeoutReasonV1>,
    pub r36_composition_count: u8,
    pub completion_midpoint: Option<u64>,
    pub r36_poll_ready: bool,
    pub r36_recycle_finished: bool,
    pub published_authority_count: u8,
    pub completed_authority_count: u8,
    pub recycled_authority_count: u8,
    pub teardown_authority_count: u8,
}

impl R38WaitSnapshotV1 {
    pub const fn has_exactly_one_stage_authority(&self) -> bool {
        self.published_authority_count as u16
            + self.completed_authority_count as u16
            + self.recycled_authority_count as u16
            + self.teardown_authority_count as u16
            == 1
    }

    /// Exact modeled timeout custody, excluding observation/result fields.
    pub fn same_timeout_operational_custody(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.route == other.route
            && self.custody == other.custody
            && self.active_present == other.active_present
            && self.active_execution == other.active_execution
            && self.active_lane == other.active_lane
            && self.active_submission == other.active_submission
            && self.lane_submission == other.lane_submission
            && self.lane_stream == other.lane_stream
            && self.allocation_storage == other.allocation_storage
            && self.module_retain_count == other.module_retain_count
            && self.dependency_retain_count == other.dependency_retain_count
            && self.event_retain_count == other.event_retain_count
            && self.allocation_owner_count == other.allocation_owner_count
            && self.allocation_current_owner == other.allocation_current_owner
            && self.stream_tail_submission == other.stream_tail_submission
            && self.stream_current_owner == other.stream_current_owner
            && self.completion_reservation_count == other.completion_reservation_count
            && self.submission_recorded == other.submission_recorded
            && self.published_authority_count == other.published_authority_count
            && self.completed_authority_count == other.completed_authority_count
            && self.recycled_authority_count == other.recycled_authority_count
            && self.teardown_authority_count == other.teardown_authority_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R38ModelErrorV1 {
    InvalidBinding,
    InvalidLimits,
    InvalidScript,
}

/// Move-only owner for the finite R38 wait-and-recycle model.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R38BoundedPersistentComputeWaitRecycleModelV1,
///     R38PersistentWaitBindingV1};
/// let binding: R38PersistentWaitBindingV1 = todo!();
/// let owner = R38BoundedPersistentComputeWaitRecycleModelV1::new_model_only(binding).unwrap();
/// let duplicate = owner.clone();
/// # let _ = duplicate;
/// ```
///
/// One owner also cannot execute its Published authority twice:
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R38BoundedPersistentComputeWaitRecycleModelV1,
///     R38DeadlineV1, R38PersistentWaitBindingV1, R38R36TerminalResultV1,
///     R38WaitLimitsV1, R38WaitScriptV1};
/// let binding = R38PersistentWaitBindingV1 {
///     lane: 0, submission: 13, stream: 17, prior_stream_submission: 11,
///     allocation: 19, allocation_storage_generation: 23, module: 29,
///     dependency: 31, event: 37, dispatch_shape_digest: 41,
///     queue_occurrence: 43, attachment_generation: 47,
///     dispatch_generation: 53, completion_batch: 59, signal_generation: 61,
///     next_signal_generation: 62, module_retain_count: 2,
///     dependency_retain_count: 3, event_retain_count: 4,
///     allocation_owner_count: 1, completion_reservation_count: 1,
///     completion_midpoint: 67,
/// };
/// let limits = R38WaitLimitsV1 {
///     deadline: R38DeadlineV1::Zero,
///     observation_max: 1,
/// };
/// let script = R38WaitScriptV1 {
///     pending_before_terminal: 0,
///     terminal: R38R36TerminalResultV1::Recycled,
/// };
/// let owner = R38BoundedPersistentComputeWaitRecycleModelV1::new_model_only(binding).unwrap();
/// let first = owner.run_model_only(true, limits, script);
/// let replay = owner.run_model_only(true, limits, script);
/// # let _ = (first, replay);
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R38BoundedPersistentComputeWaitRecycleModelV1 {
    published: R38PublishedAuthorityV1,
}

#[derive(Debug, Eq, PartialEq)]
struct R38PublishedAuthorityV1 {
    binding: R38PersistentWaitBindingV1,
}

impl R38BoundedPersistentComputeWaitRecycleModelV1 {
    pub fn new_model_only(binding: R38PersistentWaitBindingV1) -> Result<Self, R38ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R38ModelErrorV1::InvalidBinding);
        }
        Ok(Self {
            published: R38PublishedAuthorityV1 { binding },
        })
    }

    pub fn initial_snapshot_model_only(&self) -> R38WaitSnapshotV1 {
        initial_snapshot(self.published.binding)
    }

    pub fn run_model_only(
        self,
        queue_present: bool,
        limits: R38WaitLimitsV1,
        script: R38WaitScriptV1,
    ) -> Result<R38WaitSnapshotV1, R38ModelErrorV1> {
        let published = self.published;
        if !limits.is_valid() {
            return Err(R38ModelErrorV1::InvalidLimits);
        }
        if !script.is_valid() {
            return Err(R38ModelErrorV1::InvalidScript);
        }
        Ok(execute_model_only(published, queue_present, limits, script))
    }
}

fn initial_snapshot(binding: R38PersistentWaitBindingV1) -> R38WaitSnapshotV1 {
    R38WaitSnapshotV1 {
        binding,
        route: R38RouteV1::BoundedPersistentWait,
        outcome: R38OutcomeV1::Pending,
        custody: R38TerminalCustodyV1::Published,
        failure_stage: None,
        terminal_poisoned: false,
        active_present: true,
        active_execution: R38ExecutionPhaseV1::PublishedPersistent,
        active_lane: Some(binding.lane),
        active_submission: Some(binding.submission),
        lane_submission: Some(binding.submission),
        lane_stream: Some(binding.stream),
        allocation_storage: R38AllocationStorageV1::ComputeInFlight {
            submission: binding.submission,
            generation: binding.allocation_storage_generation,
        },
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

fn set_authority_counts(state: &mut R38WaitSnapshotV1) {
    state.published_authority_count =
        u8::from(matches!(state.custody, R38TerminalCustodyV1::Published));
    state.completed_authority_count =
        u8::from(matches!(state.custody, R38TerminalCustodyV1::Completed));
    state.recycled_authority_count =
        u8::from(matches!(state.custody, R38TerminalCustodyV1::Recycled));
    state.teardown_authority_count = u8::from(matches!(
        state.custody,
        R38TerminalCustodyV1::ProcessTeardown { .. }
    ));
}

fn remove_active_execution(state: &mut R38WaitSnapshotV1) {
    state.active_present = false;
    state.active_execution = R38ExecutionPhaseV1::Absent;
    state.active_lane = None;
    state.active_submission = None;
    state.lane_submission = None;
}

fn missing_queue_state(mut state: R38WaitSnapshotV1) -> R38WaitSnapshotV1 {
    state.outcome = R38OutcomeV1::Terminal;
    state.terminal_poisoned = true;
    remove_active_execution(&mut state);
    state
}

fn timeout_state(
    mut state: R38WaitSnapshotV1,
    observations: u8,
    reason: R38TimeoutReasonV1,
) -> R38WaitSnapshotV1 {
    state.observations = observations;
    state.r36_composition_count = observations;
    state.timeout_reason = Some(reason);
    state
}

fn recycled_state(mut state: R38WaitSnapshotV1, observations: u8) -> R38WaitSnapshotV1 {
    state.outcome = R38OutcomeV1::Recycled;
    state.custody = R38TerminalCustodyV1::Recycled;
    remove_active_execution(&mut state);
    state.observations = observations;
    state.r36_composition_count = observations;
    state.completion_midpoint = Some(state.binding.completion_midpoint);
    state.r36_poll_ready = true;
    state.r36_recycle_finished = true;
    set_authority_counts(&mut state);
    state
}

fn consume_lower_retryable_preflight_state(
    mut state: R38WaitSnapshotV1,
    observations: u8,
    custody: R38TerminalCustodyV1,
) -> R38WaitSnapshotV1 {
    state.outcome = R38OutcomeV1::Terminal;
    state.custody = custody;
    state.terminal_poisoned = true;
    remove_active_execution(&mut state);
    state.observations = observations;
    state.r36_composition_count = observations;
    if custody == R38TerminalCustodyV1::Completed {
        state.completion_midpoint = Some(state.binding.completion_midpoint);
        state.r36_poll_ready = true;
    }
    set_authority_counts(&mut state);
    state
}

fn process_teardown_state(
    mut state: R38WaitSnapshotV1,
    observations: u8,
    stage: R38FailureStageV1,
    terminal_token: u64,
) -> R38WaitSnapshotV1 {
    state.outcome = R38OutcomeV1::Terminal;
    state.custody = R38TerminalCustodyV1::ProcessTeardown {
        stage,
        retained_native_stage: stage.retained_native_stage(),
        terminal_token,
    };
    state.failure_stage = Some(stage);
    state.terminal_poisoned = true;
    remove_active_execution(&mut state);
    state.observations = observations;
    state.r36_composition_count = observations;
    if stage.observes_ready_midpoint() {
        state.completion_midpoint = Some(state.binding.completion_midpoint);
        state.r36_poll_ready = true;
    }
    set_authority_counts(&mut state);
    state
}

fn execute_model_only(
    published: R38PublishedAuthorityV1,
    queue_present: bool,
    limits: R38WaitLimitsV1,
    script: R38WaitScriptV1,
) -> R38WaitSnapshotV1 {
    let state = initial_snapshot(published.binding);
    if !queue_present {
        return missing_queue_state(state);
    }

    let terminal_observation = script.pending_before_terminal + 1;
    let (stop_after_pending, timeout_reason) = limits.stop_after_pending();
    if terminal_observation > stop_after_pending {
        return timeout_state(state, stop_after_pending, timeout_reason);
    }

    match script.terminal {
        R38R36TerminalResultV1::Recycled => recycled_state(state, terminal_observation),
        R38R36TerminalResultV1::RetryablePreflight(preflight) => {
            let custody = match preflight {
                R38RetryablePreflightV1::Poll => R38TerminalCustodyV1::Published,
                R38RetryablePreflightV1::Recycle => R38TerminalCustodyV1::Completed,
            };
            consume_lower_retryable_preflight_state(state, terminal_observation, custody)
        }
        R38R36TerminalResultV1::ProcessTeardown {
            stage,
            terminal_token,
        } => process_teardown_state(state, terminal_observation, stage, terminal_token),
    }
}
