//! Independent executable R36 model for fused completion poll and recycle.
//!
//! The model compares an abstract split `poll` then `recycle` composition with
//! the fused public composition. Every poll, recycle, and currentness result is
//! a caller-supplied finite observation. The model performs no I/O and does not
//! refine production Rust, KFD, HSA, HIP, drivers, firmware, hardware clocks,
//! completion truth, coherence, progress, liveness, or performance.
//!
//! The comparison is a custody-and-ordering projection. It preserves the exact
//! modeled binding, outcome, Published/Completed/Recycled custody, Poll versus
//! Recycle failure route, midpoint marker, stage-authority cardinality, and
//! logical completion/reset/recycle ordering. It excludes production/public
//! error identity, a real `Instant`, physical event timing, and the currentness
//! check count. Successful counts are checked separately as four for the split
//! composition and three for the fused composition.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R36CompletionBindingV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub attachment_generation: u64,
    pub dispatch_generation: u64,
    pub completion_batch_id: u64,
    pub signal_slot: u32,
    pub signal_generation: u64,
    pub next_signal_generation: u64,
}

impl R36CompletionBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.queue_id != 0
            && self.queue_generation != 0
            && self.attachment_generation != 0
            && self.dispatch_generation != 0
            && self.completion_batch_id != 0
            && matches!(self.signal_generation.checked_add(1), Some(next)
                if next == self.next_signal_generation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36PollObservationV1 {
    PublishedStateFailure,
    DispatchGenerationFailure,
    CompletionObservationFailure,
    DispatchCompletionFailure,
    AllocationCompletionFailure,
    Pending,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36RecycleObservationV1 {
    SignalGenerationFailure,
    SignalResetFailure,
    ClosingCurrentnessFailure,
    RecycleCurrentnessFailure,
    RecycleInfrastructureFailure,
    DispatchRecycleFailure,
    Recycled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R36CompletionObservationsV1 {
    pub poll: R36PollObservationV1,
    /// Observation made only by the split recycle entry path.
    pub split_recycle_opening_currentness_succeeded: bool,
    /// Abstract profiler marker captured on Ready before recycle begins.
    pub completion_midpoint: u64,
    pub recycle: R36RecycleObservationV1,
}

impl R36CompletionObservationsV1 {
    /// Input-only premise for removing the split recycle-opening check.
    ///
    /// It invokes neither runner and compares no output state. Non-Ready poll
    /// paths never reach the removed check. A Ready path is admitted only when
    /// that extra split observation succeeds.
    pub const fn fusion_premise(self) -> bool {
        !matches!(self.poll, R36PollObservationV1::Ready)
            || self.split_recycle_opening_currentness_succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36OutcomeV1 {
    Pending,
    Recycled,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36CustodyV1 {
    Published,
    Completed,
    Recycled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36FailureRouteV1 {
    Poll,
    Recycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R36FailurePointV1 {
    PublishedState,
    DispatchGeneration,
    CompletionObservation,
    DispatchCompletion,
    AllocationCompletion,
    SplitRecycleOpeningCurrentness,
    SignalGeneration,
    SignalReset,
    ClosingCurrentness,
    RecycleCurrentness,
    RecycleInfrastructure,
    DispatchRecycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R36CompletionSnapshotV1 {
    pub binding: R36CompletionBindingV1,
    pub outcome: R36OutcomeV1,
    pub custody: R36CustodyV1,
    pub failure_route: Option<R36FailureRouteV1>,
    pub failure_point: Option<R36FailurePointV1>,
    pub terminal_poisoned: bool,
    pub completion_midpoint: Option<u64>,
    pub poll_event_index: u8,
    pub dispatch_completion_event_index: Option<u8>,
    pub allocation_completion_event_index: Option<u8>,
    pub midpoint_event_index: Option<u8>,
    pub signal_reset_event_index: Option<u8>,
    pub closing_currentness_event_index: Option<u8>,
    pub dispatch_recycle_event_index: Option<u8>,
    pub attachment_recycle_event_index: Option<u8>,
    pub published_authority_count: u8,
    pub completed_authority_count: u8,
    pub recycled_authority_count: u8,
    pub currentness_check_count: u8,
    pub all_currentness_observations_succeeded: bool,
}

impl R36CompletionSnapshotV1 {
    /// Equality of the modeled custody-and-ordering projection.
    ///
    /// The check count is intentionally excluded because successful split and
    /// fused paths contain four and three checks respectively.
    pub fn same_projected_custody_and_ordering_semantics(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.outcome == other.outcome
            && self.custody == other.custody
            && self.failure_route == other.failure_route
            && self.failure_point == other.failure_point
            && self.terminal_poisoned == other.terminal_poisoned
            && self.completion_midpoint == other.completion_midpoint
            && self.poll_event_index == other.poll_event_index
            && self.dispatch_completion_event_index == other.dispatch_completion_event_index
            && self.allocation_completion_event_index == other.allocation_completion_event_index
            && self.midpoint_event_index == other.midpoint_event_index
            && self.signal_reset_event_index == other.signal_reset_event_index
            && self.closing_currentness_event_index == other.closing_currentness_event_index
            && self.dispatch_recycle_event_index == other.dispatch_recycle_event_index
            && self.attachment_recycle_event_index == other.attachment_recycle_event_index
            && self.published_authority_count == other.published_authority_count
            && self.completed_authority_count == other.completed_authority_count
            && self.recycled_authority_count == other.recycled_authority_count
            && self.all_currentness_observations_succeeded
                == other.all_currentness_observations_succeeded
    }

    pub const fn has_exactly_one_stage_authority(&self) -> bool {
        self.published_authority_count as u16
            + self.completed_authority_count as u16
            + self.recycled_authority_count as u16
            == 1
    }

    pub const fn successful_recycle_is_ordered(&self) -> bool {
        match (
            self.midpoint_event_index,
            self.signal_reset_event_index,
            self.closing_currentness_event_index,
            self.dispatch_recycle_event_index,
            self.attachment_recycle_event_index,
        ) {
            (Some(midpoint), Some(reset), Some(closing), Some(dispatch), Some(attachment)) => {
                midpoint < reset && reset < closing && closing < dispatch && dispatch < attachment
            }
            _ => false,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R36ModelErrorV1 {
    InvalidBinding,
}

/// Move-only owner for the finite completion-poll/recycle comparison.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R36CompletionBindingV1,
///     R36CompletionPollRecycleModelV1};
/// let binding = R36CompletionBindingV1 {
///     queue_id: 1, queue_generation: 2, attachment_generation: 3,
///     dispatch_generation: 4, completion_batch_id: 5, signal_slot: 0,
///     signal_generation: 6, next_signal_generation: 7,
/// };
/// let owner = R36CompletionPollRecycleModelV1::new_model_only(binding).unwrap();
/// let duplicated = owner.clone();
/// # let _ = duplicated;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R36CompletionPollRecycleModelV1 {
    binding: R36CompletionBindingV1,
}

struct R36PublishedAuthorityV1 {
    binding: R36CompletionBindingV1,
}

struct R36CompletedAuthorityV1 {
    binding: R36CompletionBindingV1,
}

struct R36RecycledAuthorityV1 {
    binding: R36CompletionBindingV1,
}

impl R36CompletionPollRecycleModelV1 {
    pub fn new_model_only(binding: R36CompletionBindingV1) -> Result<Self, R36ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R36ModelErrorV1::InvalidBinding);
        }
        Ok(Self { binding })
    }

    pub fn run_split_model_only(
        &self,
        observations: R36CompletionObservationsV1,
    ) -> R36CompletionSnapshotV1 {
        execute_model_only(self.binding, observations, false)
    }

    pub fn run_fused_model_only(
        &self,
        observations: R36CompletionObservationsV1,
    ) -> R36CompletionSnapshotV1 {
        execute_model_only(self.binding, observations, true)
    }
}

fn initial_snapshot(binding: R36CompletionBindingV1) -> R36CompletionSnapshotV1 {
    R36CompletionSnapshotV1 {
        binding,
        outcome: R36OutcomeV1::Terminal,
        custody: R36CustodyV1::Published,
        failure_route: None,
        failure_point: None,
        terminal_poisoned: false,
        completion_midpoint: None,
        poll_event_index: 1,
        dispatch_completion_event_index: None,
        allocation_completion_event_index: None,
        midpoint_event_index: None,
        signal_reset_event_index: None,
        closing_currentness_event_index: None,
        dispatch_recycle_event_index: None,
        attachment_recycle_event_index: None,
        published_authority_count: 1,
        completed_authority_count: 0,
        recycled_authority_count: 0,
        currentness_check_count: 0,
        all_currentness_observations_succeeded: true,
    }
}

fn poll_failure(
    mut state: R36CompletionSnapshotV1,
    point: R36FailurePointV1,
    custody: R36CustodyV1,
) -> R36CompletionSnapshotV1 {
    state.outcome = R36OutcomeV1::Terminal;
    state.custody = custody;
    state.failure_route = Some(R36FailureRouteV1::Poll);
    state.failure_point = Some(point);
    state.terminal_poisoned = true;
    set_authority_counts(&mut state, custody);
    state
}

fn recycle_failure(
    mut state: R36CompletionSnapshotV1,
    point: R36FailurePointV1,
    custody: R36CustodyV1,
) -> R36CompletionSnapshotV1 {
    state.outcome = R36OutcomeV1::Terminal;
    state.custody = custody;
    state.failure_route = Some(R36FailureRouteV1::Recycle);
    state.failure_point = Some(point);
    state.terminal_poisoned = true;
    state.all_currentness_observations_succeeded = !matches!(
        point,
        R36FailurePointV1::SplitRecycleOpeningCurrentness
            | R36FailurePointV1::ClosingCurrentness
            | R36FailurePointV1::RecycleCurrentness
    );
    set_authority_counts(&mut state, custody);
    state
}

fn set_authority_counts(state: &mut R36CompletionSnapshotV1, custody: R36CustodyV1) {
    state.published_authority_count = u8::from(matches!(custody, R36CustodyV1::Published));
    state.completed_authority_count = u8::from(matches!(custody, R36CustodyV1::Completed));
    state.recycled_authority_count = u8::from(matches!(custody, R36CustodyV1::Recycled));
}

fn advance_to_completed(published: R36PublishedAuthorityV1) -> R36CompletedAuthorityV1 {
    R36CompletedAuthorityV1 {
        binding: published.binding,
    }
}

fn advance_to_recycled(completed: R36CompletedAuthorityV1) -> R36RecycledAuthorityV1 {
    R36RecycledAuthorityV1 {
        binding: completed.binding,
    }
}

fn execute_model_only(
    binding: R36CompletionBindingV1,
    observations: R36CompletionObservationsV1,
    fused: bool,
) -> R36CompletionSnapshotV1 {
    let published = R36PublishedAuthorityV1 { binding };
    let mut state = initial_snapshot(binding);
    match observations.poll {
        R36PollObservationV1::PublishedStateFailure => {
            return poll_failure(
                state,
                R36FailurePointV1::PublishedState,
                R36CustodyV1::Published,
            );
        }
        R36PollObservationV1::DispatchGenerationFailure => {
            return poll_failure(
                state,
                R36FailurePointV1::DispatchGeneration,
                R36CustodyV1::Published,
            );
        }
        R36PollObservationV1::CompletionObservationFailure => {
            state.currentness_check_count = 1;
            state.all_currentness_observations_succeeded = false;
            return poll_failure(
                state,
                R36FailurePointV1::CompletionObservation,
                R36CustodyV1::Published,
            );
        }
        R36PollObservationV1::DispatchCompletionFailure => {
            state.currentness_check_count = 2;
            return poll_failure(
                state,
                R36FailurePointV1::DispatchCompletion,
                R36CustodyV1::Completed,
            );
        }
        R36PollObservationV1::AllocationCompletionFailure => {
            state.currentness_check_count = 2;
            state.dispatch_completion_event_index = Some(2);
            return poll_failure(
                state,
                R36FailurePointV1::AllocationCompletion,
                R36CustodyV1::Completed,
            );
        }
        R36PollObservationV1::Pending => {
            state.outcome = R36OutcomeV1::Pending;
            state.currentness_check_count = 2;
            return state;
        }
        R36PollObservationV1::Ready => {}
    }

    let completed = advance_to_completed(published);
    state.custody = R36CustodyV1::Completed;
    set_authority_counts(&mut state, R36CustodyV1::Completed);
    state.currentness_check_count = 2;
    state.dispatch_completion_event_index = Some(2);
    state.allocation_completion_event_index = Some(3);
    state.completion_midpoint = Some(observations.completion_midpoint);
    state.midpoint_event_index = Some(4);

    if !fused {
        state.currentness_check_count += 1;
        if !observations.split_recycle_opening_currentness_succeeded {
            state.all_currentness_observations_succeeded = false;
            return recycle_failure(
                state,
                R36FailurePointV1::SplitRecycleOpeningCurrentness,
                R36CustodyV1::Completed,
            );
        }
    }

    match observations.recycle {
        R36RecycleObservationV1::SignalGenerationFailure => recycle_failure(
            state,
            R36FailurePointV1::SignalGeneration,
            R36CustodyV1::Completed,
        ),
        R36RecycleObservationV1::SignalResetFailure => {
            state.signal_reset_event_index = Some(5);
            recycle_failure(
                state,
                R36FailurePointV1::SignalReset,
                R36CustodyV1::Completed,
            )
        }
        R36RecycleObservationV1::ClosingCurrentnessFailure => {
            state.signal_reset_event_index = Some(5);
            state.closing_currentness_event_index = Some(6);
            state.currentness_check_count += 1;
            recycle_failure(
                state,
                R36FailurePointV1::ClosingCurrentness,
                R36CustodyV1::Completed,
            )
        }
        R36RecycleObservationV1::RecycleCurrentnessFailure => recycle_failure(
            state,
            R36FailurePointV1::RecycleCurrentness,
            R36CustodyV1::Completed,
        ),
        R36RecycleObservationV1::RecycleInfrastructureFailure => recycle_failure(
            state,
            R36FailurePointV1::RecycleInfrastructure,
            R36CustodyV1::Completed,
        ),
        R36RecycleObservationV1::DispatchRecycleFailure => {
            state.signal_reset_event_index = Some(5);
            state.closing_currentness_event_index = Some(6);
            state.dispatch_recycle_event_index = Some(7);
            state.currentness_check_count += 1;
            recycle_failure(
                state,
                R36FailurePointV1::DispatchRecycle,
                R36CustodyV1::Recycled,
            )
        }
        R36RecycleObservationV1::Recycled => {
            let recycled = advance_to_recycled(completed);
            debug_assert_eq!(recycled.binding, binding);
            state.outcome = R36OutcomeV1::Recycled;
            state.custody = R36CustodyV1::Recycled;
            state.signal_reset_event_index = Some(5);
            state.closing_currentness_event_index = Some(6);
            state.dispatch_recycle_event_index = Some(7);
            state.attachment_recycle_event_index = Some(8);
            state.currentness_check_count += 1;
            set_authority_counts(&mut state, R36CustodyV1::Recycled);
            state
        }
    }
}
