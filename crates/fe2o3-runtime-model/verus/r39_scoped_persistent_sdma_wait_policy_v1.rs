// Independent finite R39 model for the scoped persistent-SDMA wait policy.
// Time points, observations, and the full R37 snapshot are contracted
// mathematical inputs. This proves no Rust-to-Verus or production-Rust
// refinement and no concrete Instant, CPU action, KFD/HSA/HIP, native queue,
// driver, firmware, hardware completion, coherence, progress, liveness,
// timing, parity, or performance property.
use vstd::prelude::*;

verus! {

pub open spec fn active_spin_floor_ns_v1() -> nat { 50_000 }
pub open spec fn spin_attempts_v1() -> nat { 64 }
pub open spec fn yield_attempts_v1() -> nat { 16 }
pub open spec fn initial_sleep_ns_v1() -> nat { 25_000 }
pub open spec fn max_sleep_ns_v1() -> nat { 1_000_000 }
pub open spec fn u32_max_v1() -> nat { 4_294_967_295 }
pub open spec fn u64_max_v1() -> nat { 18_446_744_073_709_551_615 }

#[derive(PartialEq, Eq)]
pub enum WaitSiteV1 {
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

#[derive(PartialEq, Eq)]
pub enum WaitProfileV1 { Default, ScopedPersistentSdma(nat) }

#[derive(PartialEq, Eq)]
pub enum CopyKindV1 { Directional, SameDevice }

#[derive(PartialEq, Eq)]
pub enum RouteV1 { Poll, LegacyWaitPoll, NativeDirectionalWait, NativeSameDeviceWait }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Pending, Succeeded, Terminal }

#[derive(PartialEq, Eq)]
pub enum ActivePhaseV1 { Published(CopyKindV1), Ready, Absent }

#[derive(PartialEq, Eq)]
pub enum StorageV1 { InFlight(nat, nat), Restored(nat) }

#[derive(PartialEq, Eq)]
pub struct NativeIdentityV1 { pub owner_id: nat, pub request_id: nat }

#[derive(PartialEq, Eq)]
pub struct OrderedFrameV1 { pub predecessor: nat, pub current: nat, pub successor: nat }

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
pub enum NativeCustodyV1 {
    ActivePublished(NativeIdentityV1),
    RestoredPair,
    TerminalPending(NativeIdentityV1),
    TerminalCompleted(NativeIdentityV1),
    TerminalTeardown(nat),
}

// This mirrors every public coordinate of the R37 executable snapshot.
#[derive(PartialEq, Eq)]
pub struct R37SnapshotV1 {
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
    pub native_observation_count: nat,
    pub settled: bool,
    pub completion_recorded: bool,
    pub continuation_ready: bool,
    pub continuation_publication_count: nat,
}

#[derive(PartialEq, Eq)]
pub enum CompletionObservationV1 { Ready, Pending }

#[derive(PartialEq, Eq)]
pub enum WaitActionV1 { Spin, Yield, Sleep(nat) }

#[derive(PartialEq, Eq)]
pub enum WaitDecisionV1 { Ready, TimedOut, Pause(WaitActionV1) }

pub struct StepV1 {
    pub snapshot: R37SnapshotV1,
    pub site: WaitSiteV1,
    pub profile: WaitProfileV1,
    pub active_spin_until_ns: Option<nat>,
    pub observation_count: nat,
    pub attempts: nat,
    pub next_sleep_ns: nat,
    pub decision: WaitDecisionV1,
}

pub open spec fn selected_v1(site: WaitSiteV1) -> bool {
    site == WaitSiteV1::DirectionalPersistentSingle
        || site == WaitSiteV1::DirectionalPersistentWindow
        || site == WaitSiteV1::SameDevicePersistentWindow
}

pub open spec fn excluded_v1(site: WaitSiteV1) -> bool {
    site == WaitSiteV1::GenericPersistentSingle
        || site == WaitSiteV1::OrdinarySingle
        || site == WaitSiteV1::OrdinaryBatchStriped
        || site == WaitSiteV1::FusedSynchronousDirectional
        || site == WaitSiteV1::XgmiSingle
        || site == WaitSiteV1::XgmiBatch
        || site == WaitSiteV1::PersistentCompute
}

pub open spec fn profile_v1(site: WaitSiteV1) -> WaitProfileV1 {
    if selected_v1(site) {
        WaitProfileV1::ScopedPersistentSdma(active_spin_floor_ns_v1())
    } else {
        WaitProfileV1::Default
    }
}

pub open spec fn active_spin_until_v1(
    site: WaitSiteV1,
    started_ns: nat,
    deadline_ns: nat,
) -> Option<nat> {
    if !selected_v1(site) {
        None
    } else if started_ns > u64_max_v1() - active_spin_floor_ns_v1() {
        Some(deadline_ns)
    } else if started_ns + active_spin_floor_ns_v1() < deadline_ns {
        Some(started_ns + active_spin_floor_ns_v1())
    } else {
        Some(deadline_ns)
    }
}

pub open spec fn increment_attempt_v1(attempts: nat) -> nat {
    if attempts == u32_max_v1() { u32_max_v1() } else { attempts + 1 }
}

pub open spec fn doubled_sleep_v1(next_sleep_ns: nat) -> nat {
    if next_sleep_ns * 2 < max_sleep_ns_v1() {
        next_sleep_ns * 2
    } else {
        max_sleep_ns_v1()
    }
}

pub open spec fn default_action_v1(
    attempts_after_increment: nat,
    next_sleep_ns: nat,
    remaining_ns: nat,
) -> WaitActionV1 {
    if attempts_after_increment <= spin_attempts_v1() {
        WaitActionV1::Spin
    } else if attempts_after_increment <= spin_attempts_v1() + yield_attempts_v1() {
        WaitActionV1::Yield
    } else if next_sleep_ns < remaining_ns {
        WaitActionV1::Sleep(next_sleep_ns)
    } else {
        WaitActionV1::Sleep(remaining_ns)
    }
}

pub open spec fn pause_action_v1(
    site: WaitSiteV1,
    started_ns: nat,
    deadline_ns: nat,
    action_now_ns: nat,
    attempts_after_increment: nat,
    next_sleep_ns: nat,
) -> WaitActionV1 {
    if selected_v1(site)
        && action_now_ns < active_spin_until_v1(site, started_ns, deadline_ns).unwrap()
    {
        WaitActionV1::Spin
    } else {
        default_action_v1(
            attempts_after_increment,
            next_sleep_ns,
            if action_now_ns < deadline_ns {
                (deadline_ns - action_now_ns) as nat
            } else {
                0
            },
        )
    }
}

pub open spec fn valid_input_v1(
    started_ns: nat,
    deadline_ns: nat,
    deadline_check_ns: nat,
    action_now_ns: nat,
    attempts: nat,
    next_sleep_ns: nat,
) -> bool {
    &&& started_ns <= u64_max_v1()
    &&& deadline_ns <= u64_max_v1()
    &&& started_ns <= deadline_check_ns
    &&& deadline_check_ns <= action_now_ns
    &&& action_now_ns <= u64_max_v1()
    &&& attempts <= u32_max_v1()
    &&& next_sleep_ns > 0
    &&& next_sleep_ns <= max_sleep_ns_v1()
}

pub open spec fn execute_step_v1(
    snapshot: R37SnapshotV1,
    site: WaitSiteV1,
    started_ns: nat,
    deadline_ns: nat,
    deadline_check_ns: nat,
    action_now_ns: nat,
    attempts: nat,
    next_sleep_ns: nat,
    observation: CompletionObservationV1,
) -> StepV1 {
    let spin_until = active_spin_until_v1(site, started_ns, deadline_ns);
    if observation == CompletionObservationV1::Ready {
        StepV1 {
            snapshot, site, profile: profile_v1(site), active_spin_until_ns: spin_until,
            observation_count: 1, attempts, next_sleep_ns, decision: WaitDecisionV1::Ready,
        }
    } else if deadline_check_ns >= deadline_ns {
        StepV1 {
            snapshot, site, profile: profile_v1(site), active_spin_until_ns: spin_until,
            observation_count: 1, attempts, next_sleep_ns, decision: WaitDecisionV1::TimedOut,
        }
    } else {
        let next_attempt = increment_attempt_v1(attempts);
        let action = pause_action_v1(
            site, started_ns, deadline_ns, action_now_ns, next_attempt, next_sleep_ns,
        );
        let next_sleep = match action {
            WaitActionV1::Sleep(_) => doubled_sleep_v1(next_sleep_ns),
            _ => next_sleep_ns,
        };
        StepV1 {
            snapshot, site, profile: profile_v1(site), active_spin_until_ns: spin_until,
            observation_count: 1, attempts: next_attempt, next_sleep_ns: next_sleep,
            decision: WaitDecisionV1::Pause(action),
        }
    }
}

pub proof fn three_selected_sites_use_exact_floor_v1(site: WaitSiteV1)
    requires selected_v1(site),
    ensures profile_v1(site) == WaitProfileV1::ScopedPersistentSdma(50_000),
{}

pub proof fn seven_excluded_sites_keep_default_v1(site: WaitSiteV1)
    requires excluded_v1(site),
    ensures profile_v1(site) == WaitProfileV1::Default,
{}

pub proof fn checked_floor_add_without_overflow_v1(
    site: WaitSiteV1, started_ns: nat, deadline_ns: nat,
)
    requires
        selected_v1(site),
        started_ns <= u64_max_v1() - active_spin_floor_ns_v1(),
        started_ns + active_spin_floor_ns_v1() < deadline_ns,
    ensures active_spin_until_v1(site, started_ns, deadline_ns)
        == Some(started_ns + active_spin_floor_ns_v1()),
{}

pub proof fn checked_floor_add_overflow_clamps_to_deadline_v1(
    site: WaitSiteV1, started_ns: nat, deadline_ns: nat,
)
    requires
        selected_v1(site),
        started_ns > u64_max_v1() - active_spin_floor_ns_v1(),
    ensures active_spin_until_v1(site, started_ns, deadline_ns) == Some(deadline_ns),
{}

pub proof fn floor_is_clamped_to_earlier_deadline_v1(
    site: WaitSiteV1, started_ns: nat, deadline_ns: nat,
)
    requires
        selected_v1(site),
        started_ns <= u64_max_v1() - active_spin_floor_ns_v1(),
        deadline_ns <= started_ns + active_spin_floor_ns_v1(),
    ensures active_spin_until_v1(site, started_ns, deadline_ns) == Some(deadline_ns),
{}

pub proof fn zero_deadline_pending_observes_then_times_out_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        started_ns <= u64_max_v1(),
        attempts <= u32_max_v1(),
        next_sleep_ns > 0,
        next_sleep_ns <= max_sleep_ns_v1(),
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, started_ns, started_ns, started_ns, attempts,
            next_sleep_ns, CompletionObservationV1::Pending,
        );
        &&& step.observation_count == 1
        &&& step.decision == WaitDecisionV1::TimedOut
    },
{}

pub proof fn expired_ready_observation_wins_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            attempts, next_sleep_ns,
        ),
        deadline_check_ns >= deadline_ns,
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts,
            next_sleep_ns, CompletionObservationV1::Ready,
        );
        &&& step.observation_count == 1
        &&& step.decision == WaitDecisionV1::Ready
        &&& step.attempts == attempts
        &&& step.next_sleep_ns == next_sleep_ns
    },
{}

pub proof fn expired_pending_performs_no_action_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            attempts, next_sleep_ns,
        ),
        deadline_check_ns >= deadline_ns,
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts,
            next_sleep_ns, CompletionObservationV1::Pending,
        );
        &&& step.observation_count == 1
        &&& step.decision == WaitDecisionV1::TimedOut
        &&& step.attempts == attempts
        &&& step.next_sleep_ns == next_sleep_ns
    },
{}

pub proof fn before_floor_uses_active_spin_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            attempts, next_sleep_ns,
        ),
        selected_v1(site),
        deadline_check_ns < deadline_ns,
        action_now_ns < active_spin_until_v1(site, started_ns, deadline_ns).unwrap(),
    ensures execute_step_v1(
        snapshot, site, started_ns, deadline_ns, deadline_check_ns,
        action_now_ns, attempts,
        next_sleep_ns, CompletionObservationV1::Pending,
    ).decision == WaitDecisionV1::Pause(WaitActionV1::Spin),
{}

pub proof fn exact_floor_boundary_resumes_default_stage_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, next_sleep_ns: nat,
)
    requires
        selected_v1(site),
        started_ns <= u64_max_v1() - active_spin_floor_ns_v1(),
        started_ns + active_spin_floor_ns_v1() < deadline_ns,
        deadline_ns <= u64_max_v1(),
        next_sleep_ns > 0,
        next_sleep_ns <= max_sleep_ns_v1(),
    ensures execute_step_v1(
        snapshot, site, started_ns, deadline_ns,
        started_ns + active_spin_floor_ns_v1(),
        started_ns + active_spin_floor_ns_v1(), spin_attempts_v1(),
        next_sleep_ns, CompletionObservationV1::Pending,
    ).decision == WaitDecisionV1::Pause(WaitActionV1::Yield),
{}

pub proof fn pending_live_increments_attempt_before_action_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            attempts, next_sleep_ns,
        ),
        deadline_check_ns < deadline_ns,
    ensures
        execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts, next_sleep_ns, CompletionObservationV1::Pending,
        ).attempts == increment_attempt_v1(attempts),
        increment_attempt_v1(u32_max_v1()) == u32_max_v1(),
{}

pub proof fn deadline_passage_between_samples_pauses_with_zero_remaining_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            spin_attempts_v1() + yield_attempts_v1(), next_sleep_ns,
        ),
        selected_v1(site),
        deadline_check_ns < deadline_ns,
        action_now_ns >= deadline_ns,
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, spin_attempts_v1() + yield_attempts_v1(),
            next_sleep_ns, CompletionObservationV1::Pending,
        );
        &&& step.decision == WaitDecisionV1::Pause(WaitActionV1::Sleep(0))
        &&& step.attempts == spin_attempts_v1() + yield_attempts_v1() + 1
    },
{}

pub proof fn default_spin_stage_is_exact_v1(
    attempts_after_increment: nat, next_sleep_ns: nat, remaining_ns: nat,
)
    requires attempts_after_increment <= spin_attempts_v1(),
    ensures default_action_v1(attempts_after_increment, next_sleep_ns, remaining_ns)
        == WaitActionV1::Spin,
{}

pub proof fn default_yield_stage_is_exact_v1(
    attempts_after_increment: nat, next_sleep_ns: nat, remaining_ns: nat,
)
    requires
        attempts_after_increment > spin_attempts_v1(),
        attempts_after_increment <= spin_attempts_v1() + yield_attempts_v1(),
    ensures default_action_v1(attempts_after_increment, next_sleep_ns, remaining_ns)
        == WaitActionV1::Yield,
{}

pub proof fn sleep_is_bounded_by_remaining_deadline_v1(
    attempts_after_increment: nat, next_sleep_ns: nat, remaining_ns: nat,
)
    requires attempts_after_increment > spin_attempts_v1() + yield_attempts_v1(),
    ensures match default_action_v1(
        attempts_after_increment, next_sleep_ns, remaining_ns,
    ) {
        WaitActionV1::Sleep(duration_ns) => duration_ns <= remaining_ns,
        _ => false,
    },
{}

pub proof fn sleep_backoff_is_bounded_by_max_v1(next_sleep_ns: nat)
    requires next_sleep_ns <= max_sleep_ns_v1(),
    ensures doubled_sleep_v1(next_sleep_ns) <= max_sleep_ns_v1(),
{}

pub proof fn every_step_retains_full_r37_snapshot_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
    observation: CompletionObservationV1,
)
    requires valid_input_v1(
        started_ns, deadline_ns, deadline_check_ns, action_now_ns,
        attempts, next_sleep_ns,
    ),
    ensures {
        let after = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts,
            next_sleep_ns, observation,
        ).snapshot;
        &&& after.binding == snapshot.binding
        &&& after.route == snapshot.route
        &&& after.outcome == snapshot.outcome
        &&& after.active_present == snapshot.active_present
        &&& after.active_phase == snapshot.active_phase
        &&& after.published_index_retained == snapshot.published_index_retained
        &&& after.published_index_frame == snapshot.published_index_frame
        &&& after.source_storage == snapshot.source_storage
        &&& after.destination_storage == snapshot.destination_storage
        &&& after.dependency_retain_count == snapshot.dependency_retain_count
        &&& after.source_custody_count == snapshot.source_custody_count
        &&& after.destination_custody_count == snapshot.destination_custody_count
        &&& after.stream_owner_count == snapshot.stream_owner_count
        &&& after.stream_current_retained == snapshot.stream_current_retained
        &&& after.stream_frame == snapshot.stream_frame
        &&& after.native_custody == snapshot.native_custody
        &&& after.terminal_poisoned == snapshot.terminal_poisoned
        &&& after.native_observation_count == snapshot.native_observation_count
        &&& after.settled == snapshot.settled
        &&& after.completion_recorded == snapshot.completion_recorded
        &&& after.continuation_ready == snapshot.continuation_ready
        &&& after.continuation_publication_count == snapshot.continuation_publication_count
    },
{}

pub proof fn timeout_retains_full_r37_snapshot_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires
        valid_input_v1(
            started_ns, deadline_ns, deadline_check_ns, action_now_ns,
            attempts, next_sleep_ns,
        ),
        deadline_check_ns >= deadline_ns,
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts,
            next_sleep_ns, CompletionObservationV1::Pending,
        );
        &&& step.decision == WaitDecisionV1::TimedOut
        &&& step.snapshot == snapshot
        &&& step.snapshot.native_custody == snapshot.native_custody
        &&& step.snapshot.stream_frame == snapshot.stream_frame
        &&& step.snapshot.source_storage == snapshot.source_storage
        &&& step.snapshot.destination_storage == snapshot.destination_storage
    },
{}

pub proof fn ready_retains_completion_custody_and_continuation_v1(
    snapshot: R37SnapshotV1, site: WaitSiteV1, started_ns: nat,
    deadline_ns: nat, deadline_check_ns: nat, action_now_ns: nat,
    attempts: nat, next_sleep_ns: nat,
)
    requires valid_input_v1(
        started_ns, deadline_ns, deadline_check_ns, action_now_ns,
        attempts, next_sleep_ns,
    ),
    ensures {
        let step = execute_step_v1(
            snapshot, site, started_ns, deadline_ns, deadline_check_ns,
            action_now_ns, attempts,
            next_sleep_ns, CompletionObservationV1::Ready,
        );
        &&& step.decision == WaitDecisionV1::Ready
        &&& step.snapshot == snapshot
        &&& step.snapshot.native_custody == snapshot.native_custody
        &&& step.snapshot.completion_recorded == snapshot.completion_recorded
        &&& step.snapshot.continuation_ready == snapshot.continuation_ready
        &&& step.snapshot.continuation_publication_count
            == snapshot.continuation_publication_count
    },
{}

pub proof fn initial_cursor_constants_are_exact_v1()
    ensures
        spin_attempts_v1() == 64,
        yield_attempts_v1() == 16,
        initial_sleep_ns_v1() == 25_000,
        max_sleep_ns_v1() == 1_000_000,
{}

} // verus!
