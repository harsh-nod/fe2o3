//! Independent executable R39 model for the scoped persistent-SDMA wait policy.
//!
//! The model selects the elapsed active-spin floor only for the three named
//! persistent-SDMA wait sites and executes one abstract completion observation.
//! Observation happens before the deadline test. A Pending result that remains
//! before the deadline advances the adaptive cursor once, using a separately
//! sampled action time; Ready and expired Pending results perform no wait
//! action. The entire public R37 wait snapshot is carried through unchanged.
//!
//! Nanosecond positions, completion observations, and the R37 snapshot are
//! contracted mathematical inputs. This model does not refine production Rust,
//! `Instant`, KFD, HSA, HIP, native queues, drivers, firmware, hardware timing,
//! completion, coherence, progress, liveness, parity, or performance.

use crate::R37WaitSnapshotV1;

pub const R39_PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_NS_V1: u64 = 50_000;
pub const R39_DEFAULT_SPIN_ATTEMPTS_V1: u32 = 64;
pub const R39_DEFAULT_YIELD_ATTEMPTS_V1: u32 = 16;
pub const R39_DEFAULT_INITIAL_SLEEP_NS_V1: u64 = 25_000;
pub const R39_DEFAULT_MAX_SLEEP_NS_V1: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39WaitSiteV1 {
    DirectionalPersistentSingle,
    DirectionalPersistentWindow,
    SameDevicePersistentWindow,
    GenericPersistentSingle,
    OrdinarySingle,
    OrdinaryBatchStriped,
    FusedSynchronousDirectional,
    XgmiSingle,
    XgmiBatch,
    PersistentCompute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39WaitProfileV1 {
    Default,
    ScopedPersistentSdma { active_spin_floor_ns: u64 },
}

pub const fn r39_wait_profile_model_only(site: R39WaitSiteV1) -> R39WaitProfileV1 {
    match site {
        R39WaitSiteV1::DirectionalPersistentSingle
        | R39WaitSiteV1::DirectionalPersistentWindow
        | R39WaitSiteV1::SameDevicePersistentWindow => R39WaitProfileV1::ScopedPersistentSdma {
            active_spin_floor_ns: R39_PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_NS_V1,
        },
        R39WaitSiteV1::GenericPersistentSingle
        | R39WaitSiteV1::OrdinarySingle
        | R39WaitSiteV1::OrdinaryBatchStriped
        | R39WaitSiteV1::FusedSynchronousDirectional
        | R39WaitSiteV1::XgmiSingle
        | R39WaitSiteV1::XgmiBatch
        | R39WaitSiteV1::PersistentCompute => R39WaitProfileV1::Default,
    }
}

pub const fn r39_active_spin_until_model_only(
    site: R39WaitSiteV1,
    started_ns: u64,
    deadline_ns: u64,
) -> Option<u64> {
    match r39_wait_profile_model_only(site) {
        R39WaitProfileV1::Default => None,
        R39WaitProfileV1::ScopedPersistentSdma {
            active_spin_floor_ns,
        } => Some(match started_ns.checked_add(active_spin_floor_ns) {
            Some(spin_until) if spin_until < deadline_ns => spin_until,
            Some(_) | None => deadline_ns,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39CompletionObservationV1 {
    Ready,
    Pending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39WaitActionV1 {
    Spin,
    Yield,
    Sleep { nanoseconds: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39WaitDecisionV1 {
    Ready,
    TimedOut,
    Pause(R39WaitActionV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R39WaitStepV1 {
    pub snapshot: R37WaitSnapshotV1,
    pub site: R39WaitSiteV1,
    pub profile: R39WaitProfileV1,
    pub active_spin_until_ns: Option<u64>,
    pub observation_count: u8,
    pub attempts: u32,
    pub next_sleep_ns: u64,
    pub decision: R39WaitDecisionV1,
}

impl R39WaitStepV1 {
    /// Enumerates the full public R37 snapshot rather than relying on a subset.
    pub fn retains_full_r37_snapshot(&self, before: &R37WaitSnapshotV1) -> bool {
        self.snapshot.binding == before.binding
            && self.snapshot.route == before.route
            && self.snapshot.outcome == before.outcome
            && self.snapshot.active_present == before.active_present
            && self.snapshot.active_phase == before.active_phase
            && self.snapshot.published_index_retained == before.published_index_retained
            && self.snapshot.published_index_frame == before.published_index_frame
            && self.snapshot.source_storage == before.source_storage
            && self.snapshot.destination_storage == before.destination_storage
            && self.snapshot.dependency_retain_count == before.dependency_retain_count
            && self.snapshot.source_custody_count == before.source_custody_count
            && self.snapshot.destination_custody_count == before.destination_custody_count
            && self.snapshot.stream_owner_count == before.stream_owner_count
            && self.snapshot.stream_current_retained == before.stream_current_retained
            && self.snapshot.stream_frame == before.stream_frame
            && self.snapshot.native_custody == before.native_custody
            && self.snapshot.terminal_poisoned == before.terminal_poisoned
            && self.snapshot.native_observation_count == before.native_observation_count
            && self.snapshot.settled == before.settled
            && self.snapshot.completion_recorded == before.completion_recorded
            && self.snapshot.continuation_ready == before.continuation_ready
            && self.snapshot.continuation_publication_count == before.continuation_publication_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R39ModelErrorV1 {
    InvalidSleep,
    InvalidObservationTime,
}

struct R39WaitAuthorityV1 {
    snapshot: R37WaitSnapshotV1,
}

/// Move-only owner of one abstract R39 wait-policy step.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R37WaitSnapshotV1,
///     R39ScopedPersistentSdmaWaitPolicyModelV1, R39WaitSiteV1};
/// let snapshot: R37WaitSnapshotV1 = todo!();
/// let owner = R39ScopedPersistentSdmaWaitPolicyModelV1::new_model_only(
///     snapshot, R39WaitSiteV1::DirectionalPersistentSingle, 0, 50_000,
/// ).unwrap();
/// let duplicate = owner.clone();
/// # let _ = duplicate;
/// ```
pub struct R39ScopedPersistentSdmaWaitPolicyModelV1 {
    authority: R39WaitAuthorityV1,
    site: R39WaitSiteV1,
    started_ns: u64,
    deadline_ns: u64,
    active_spin_until_ns: Option<u64>,
    attempts: u32,
    next_sleep_ns: u64,
}

impl R39ScopedPersistentSdmaWaitPolicyModelV1 {
    pub fn new_model_only(
        snapshot: R37WaitSnapshotV1,
        site: R39WaitSiteV1,
        started_ns: u64,
        deadline_ns: u64,
    ) -> Result<Self, R39ModelErrorV1> {
        Self::new_with_cursor_model_only(
            snapshot,
            site,
            started_ns,
            deadline_ns,
            0,
            R39_DEFAULT_INITIAL_SLEEP_NS_V1,
        )
    }

    pub fn new_with_cursor_model_only(
        snapshot: R37WaitSnapshotV1,
        site: R39WaitSiteV1,
        started_ns: u64,
        deadline_ns: u64,
        attempts: u32,
        next_sleep_ns: u64,
    ) -> Result<Self, R39ModelErrorV1> {
        if next_sleep_ns == 0 || next_sleep_ns > R39_DEFAULT_MAX_SLEEP_NS_V1 {
            return Err(R39ModelErrorV1::InvalidSleep);
        }
        Ok(Self {
            authority: R39WaitAuthorityV1 { snapshot },
            site,
            started_ns,
            deadline_ns,
            active_spin_until_ns: r39_active_spin_until_model_only(site, started_ns, deadline_ns),
            attempts,
            next_sleep_ns,
        })
    }

    pub fn observe_model_only(
        self,
        deadline_check_ns: u64,
        action_now_ns: u64,
        observation: R39CompletionObservationV1,
    ) -> Result<R39WaitStepV1, R39ModelErrorV1> {
        if deadline_check_ns < self.started_ns || action_now_ns < deadline_check_ns {
            return Err(R39ModelErrorV1::InvalidObservationTime);
        }
        Ok(execute_step_model_only(
            self,
            deadline_check_ns,
            action_now_ns,
            observation,
        ))
    }
}

fn execute_step_model_only(
    model: R39ScopedPersistentSdmaWaitPolicyModelV1,
    deadline_check_ns: u64,
    action_now_ns: u64,
    observation: R39CompletionObservationV1,
) -> R39WaitStepV1 {
    let profile = r39_wait_profile_model_only(model.site);
    let mut step = R39WaitStepV1 {
        snapshot: model.authority.snapshot,
        site: model.site,
        profile,
        active_spin_until_ns: model.active_spin_until_ns,
        observation_count: 1,
        attempts: model.attempts,
        next_sleep_ns: model.next_sleep_ns,
        decision: R39WaitDecisionV1::Ready,
    };
    if observation == R39CompletionObservationV1::Ready {
        return step;
    }
    if deadline_check_ns >= model.deadline_ns {
        step.decision = R39WaitDecisionV1::TimedOut;
        return step;
    }

    step.attempts = step.attempts.saturating_add(1);
    let action = if model
        .active_spin_until_ns
        .is_some_and(|active_spin_until_ns| action_now_ns < active_spin_until_ns)
        || step.attempts <= R39_DEFAULT_SPIN_ATTEMPTS_V1
    {
        R39WaitActionV1::Spin
    } else if step.attempts <= R39_DEFAULT_SPIN_ATTEMPTS_V1 + R39_DEFAULT_YIELD_ATTEMPTS_V1 {
        R39WaitActionV1::Yield
    } else {
        let remaining = model.deadline_ns.saturating_sub(action_now_ns);
        let nanoseconds = step.next_sleep_ns.min(remaining);
        step.next_sleep_ns = step
            .next_sleep_ns
            .saturating_mul(2)
            .min(R39_DEFAULT_MAX_SLEEP_NS_V1);
        R39WaitActionV1::Sleep { nanoseconds }
    };
    step.decision = R39WaitDecisionV1::Pause(action);
    step
}
