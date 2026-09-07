// Independent finite R51 model for the public R48 dependency lifecycle.
// Production-named trace labels are comparison fixtures only. Every identity,
// completion fact, currentness fact, native boundary, and public-custody value
// is a mathematical input. This is not a Rust-to-Verus or production-Rust
// refinement and establishes no native, KFD/HSA/HIP, hardware, progress,
// teardown, parity, or performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn active_target_bound_v1() -> nat { 128 }
pub open spec fn dependency_bound_v1() -> nat { 256 }

#[derive(PartialEq, Eq)]
pub enum TargetPhaseV1 {
    Prepared,
    NativePublished,
    Published,
    Completed,
    Released,
    Terminal,
}

#[derive(PartialEq, Eq)]
pub enum ProductionTransitionV1 {
    EnsureTargetCapacity,
    ReserveAcceptanceEpoch,
    RetainDependencyReadersForTarget,
    BeginTargetUse,
    PublishNative,
    BindPublishedTarget,
    PollComputeDependencyDispatch,
    ObservePublishedTargetOnce,
    ReleaseDependencyReaderEventBatch,
    ReleaseAfterDependentCompletion,
}

#[derive(PartialEq, Eq)]
pub struct SourceEventV1 {
    pub owner: nat,
    pub session: nat,
    pub source_lane: nat,
    pub source_arena: nat,
    pub source_epoch: nat,
    pub public_custody: nat,
    pub signal: nat,
}

#[derive(PartialEq, Eq)]
pub struct TargetCustodyV1 {
    pub owner: nat,
    pub session: nat,
    pub target_epoch: nat,
    pub target_lane: nat,
    pub source_lane: nat,
    pub source_arena: nat,
    pub dispatch_custody: nat,
    pub target_event_custody: nat,
    pub target_signal: nat,
    pub dependency_count: nat,
    pub phase: TargetPhaseV1,
}

#[derive(PartialEq, Eq)]
pub struct OwnerStateV1 {
    pub next_epoch: nat,
    pub active_targets: nat,
    pub source_event_pins: nat,
    pub source_reader_pins: nat,
    pub target_event_pins: nat,
    pub native_effects: nat,
    pub recycle_mutations: nat,
    pub terminal: bool,
}

pub struct AdmissionV1 {
    pub owner: nat,
    pub session: nat,
    pub target_lane: nat,
    pub source_lane: nat,
    pub source_arena: nat,
    pub next_epoch: nat,
    pub events: Seq<SourceEventV1>,
}

#[derive(PartialEq, Eq)]
pub struct CompletionObservationV1 {
    pub target_epoch: nat,
    pub dispatch_custody: nat,
    pub completed: bool,
}

#[derive(PartialEq, Eq)]
pub struct CompletedSignalCustodyV1 {
    pub public_custody: nat,
    pub signal: nat,
}

pub open spec fn source_event_valid_v1(event: SourceEventV1) -> bool {
    event.owner > 0
        && event.session > 0
        && event.source_lane > 0
        && event.source_arena > 0
        && event.source_epoch > 0
        && event.public_custody > 0
        && event.signal > 0
}

pub open spec fn distinct_events_v1(events: Seq<SourceEventV1>) -> bool {
    forall|i: int, j: int|
        0 <= i < events.len() && 0 <= j < events.len() && i != j
            ==> events[i].public_custody != events[j].public_custody
                && events[i].signal != events[j].signal
}

pub open spec fn exactly_one_source_arena_v1(admission: AdmissionV1) -> bool {
    admission.events.len() > 0
        && forall|i: int| 0 <= i < admission.events.len()
            ==> admission.events[i].source_arena == admission.source_arena
                && admission.events[i].source_lane == admission.source_lane
}

pub open spec fn admitted_v1(state: OwnerStateV1, admission: AdmissionV1) -> bool {
    !state.terminal
        && state.next_epoch > 0
        && admission.owner > 0
        && admission.session > 0
        && admission.next_epoch == state.next_epoch
        && admission.next_epoch > 0
        && admission.target_lane > 0
        && admission.source_lane > 0
        && admission.target_lane != admission.source_lane
        && admission.source_arena > 0
        && 1 <= admission.events.len() <= dependency_bound_v1()
        && state.active_targets < active_target_bound_v1()
        && state.source_event_pins >= admission.events.len()
        && exactly_one_source_arena_v1(admission)
        && distinct_events_v1(admission.events)
        && forall|i: int| 0 <= i < admission.events.len() ==> {
            let event = #[trigger] admission.events[i];
            &&& source_event_valid_v1(event)
            &&& event.owner == admission.owner
            &&& event.session == admission.session
            &&& event.source_epoch < admission.next_epoch
        }
}

pub open spec fn rejected_state_v1(state: OwnerStateV1) -> OwnerStateV1 { state }

pub open spec fn begin_state_v1(state: OwnerStateV1, count: nat) -> OwnerStateV1 {
    OwnerStateV1 {
        next_epoch: state.next_epoch + 1,
        active_targets: state.active_targets + 1,
        source_event_pins: state.source_event_pins,
        source_reader_pins: state.source_reader_pins + count,
        target_event_pins: state.target_event_pins + 1,
        native_effects: state.native_effects,
        recycle_mutations: state.recycle_mutations,
        terminal: false,
    }
}

pub open spec fn begin_target_v1(admission: AdmissionV1, dispatch: nat, target_event: nat, signal: nat)
    -> TargetCustodyV1
{
    TargetCustodyV1 {
        owner: admission.owner,
        session: admission.session,
        target_epoch: admission.next_epoch,
        target_lane: admission.target_lane,
        source_lane: admission.source_lane,
        source_arena: admission.source_arena,
        dispatch_custody: dispatch,
        target_event_custody: target_event,
        target_signal: signal,
        dependency_count: admission.events.len(),
        phase: TargetPhaseV1::Prepared,
    }
}

pub open spec fn set_phase_v1(target: TargetCustodyV1, phase: TargetPhaseV1)
    -> TargetCustodyV1
{
    TargetCustodyV1 { phase, ..target }
}

pub open spec fn ring_full_retry_state_v1(state: OwnerStateV1) -> OwnerStateV1 { state }

pub open spec fn rollback_state_v1(state: OwnerStateV1, count: nat) -> OwnerStateV1
    recommends state.active_targets > 0, state.source_reader_pins >= count,
               state.target_event_pins > 0,
{
    OwnerStateV1 {
        next_epoch: state.next_epoch,
        active_targets: (state.active_targets - 1) as nat,
        source_event_pins: state.source_event_pins,
        source_reader_pins: (state.source_reader_pins - count) as nat,
        target_event_pins: (state.target_event_pins - 1) as nat,
        native_effects: state.native_effects,
        recycle_mutations: state.recycle_mutations,
        terminal: false,
    }
}

pub open spec fn pending_state_v1(state: OwnerStateV1) -> OwnerStateV1 { state }

pub open spec fn exact_completion_v1(
    target: TargetCustodyV1,
    observation: CompletionObservationV1,
) -> bool {
    target.phase == TargetPhaseV1::Published
        && observation.completed
        && observation.target_epoch == target.target_epoch
        && observation.dispatch_custody == target.dispatch_custody
}

pub open spec fn completed_state_v1(state: OwnerStateV1, count: nat) -> OwnerStateV1
    recommends state.source_event_pins >= count, state.source_reader_pins >= count,
{
    OwnerStateV1 {
        next_epoch: state.next_epoch,
        active_targets: state.active_targets,
        source_event_pins: (state.source_event_pins - count) as nat,
        source_reader_pins: (state.source_reader_pins - count) as nat,
        target_event_pins: state.target_event_pins,
        native_effects: state.native_effects,
        recycle_mutations: state.recycle_mutations,
        terminal: false,
    }
}

pub open spec fn complete_or_reject_state_v1(
    state: OwnerStateV1,
    target: TargetCustodyV1,
    observation: CompletionObservationV1,
) -> OwnerStateV1 {
    if exact_completion_v1(target, observation)
        && state.source_event_pins >= target.dependency_count
        && state.source_reader_pins >= target.dependency_count
    {
        completed_state_v1(state, target.dependency_count)
    } else {
        state
    }
}

pub open spec fn released_state_v1(state: OwnerStateV1) -> OwnerStateV1
    recommends state.active_targets > 0,
{
    OwnerStateV1 { active_targets: (state.active_targets - 1) as nat, ..state }
}

pub open spec fn pinned_v1(state: OwnerStateV1) -> bool {
    state.source_event_pins > 0
        || state.source_reader_pins > 0
        || state.target_event_pins > 0
}

pub open spec fn recycle_or_reject_v1(
    state: OwnerStateV1,
    completed: CompletedSignalCustodyV1,
) -> (OwnerStateV1, CompletedSignalCustodyV1) {
    if pinned_v1(state) || state.terminal {
        (state, completed)
    } else {
        (
            OwnerStateV1 { recycle_mutations: state.recycle_mutations + 1, ..state },
            completed,
        )
    }
}

pub open spec fn terminalize_v1(state: OwnerStateV1, target: TargetCustodyV1)
    -> (OwnerStateV1, TargetCustodyV1)
{
    (
        OwnerStateV1 { terminal: true, native_effects: state.native_effects + 1, ..state },
        set_phase_v1(target, TargetPhaseV1::Terminal),
    )
}

pub open spec fn absorb_v1(state: OwnerStateV1, attempted: OwnerStateV1) -> OwnerStateV1 {
    if state.terminal { state } else { attempted }
}

pub open spec fn teardown_allowed_v1(state: OwnerStateV1) -> bool {
    state.active_targets == 0
        && state.source_event_pins == 0
        && state.source_reader_pins == 0
        && state.target_event_pins == 0
}

pub open spec fn success_trace_v1() -> Seq<ProductionTransitionV1> {
    seq![
        ProductionTransitionV1::EnsureTargetCapacity,
        ProductionTransitionV1::ReserveAcceptanceEpoch,
        ProductionTransitionV1::RetainDependencyReadersForTarget,
        ProductionTransitionV1::BeginTargetUse,
        ProductionTransitionV1::PublishNative,
        ProductionTransitionV1::BindPublishedTarget,
        ProductionTransitionV1::PollComputeDependencyDispatch,
        ProductionTransitionV1::ObservePublishedTargetOnce,
        ProductionTransitionV1::ReleaseDependencyReaderEventBatch,
        ProductionTransitionV1::ReleaseAfterDependentCompletion,
    ]
}

// Obligation 1: the public target storage and event fan-in bounds are exact.
pub proof fn exact_finite_bounds_v1()
    ensures active_target_bound_v1() == 128, dependency_bound_v1() == 256,
{}

// Obligation 2: the admission predicate has a concrete non-vacuous witness.
pub proof fn admitted_domain_is_nonempty_v1() {
    let state = OwnerStateV1 {
        next_epoch: 2, active_targets: 0, source_event_pins: 1,
        source_reader_pins: 0, target_event_pins: 0, native_effects: 0,
        recycle_mutations: 0, terminal: false,
    };
    let event = SourceEventV1 {
        owner: 1, session: 2, source_lane: 3, source_arena: 4,
        source_epoch: 1, public_custody: 5, signal: 6,
    };
    let admission = AdmissionV1 {
        owner: 1, session: 2, target_lane: 7, source_lane: 3,
        source_arena: 4, next_epoch: 2, events: seq![event],
    };
    assert(admitted_v1(state, admission));
}

// Obligation 3: any admitted target has one through 256 dependencies.
pub proof fn admitted_dependency_count_is_bounded_v1(state: OwnerStateV1, admission: AdmissionV1)
    requires admitted_v1(state, admission),
    ensures 1 <= admission.events.len() <= 256,
{}

// Obligation 4: every admitted event names the target's one source arena/lane.
pub proof fn admitted_target_has_exactly_one_source_arena_v1(
    state: OwnerStateV1, admission: AdmissionV1, index: int,
)
    requires admitted_v1(state, admission), 0 <= index < admission.events.len(),
    ensures admission.events[index].source_arena == admission.source_arena,
            admission.events[index].source_lane == admission.source_lane,
{}

// Obligation 5: pure preflight rejection does not consume an epoch.
pub proof fn preflight_rejection_does_not_burn_v1(state: OwnerStateV1)
    ensures rejected_state_v1(state).next_epoch == state.next_epoch,
            rejected_state_v1(state) == state,
{}

// Obligation 6: accepted admission burns the nonzero target epoch.
pub proof fn accepted_epoch_is_nonzero_and_burned_v1(state: OwnerStateV1, admission: AdmissionV1)
    requires admitted_v1(state, admission),
    ensures begin_target_v1(admission, 1, 2, 3).target_epoch == state.next_epoch,
            begin_target_v1(admission, 1, 2, 3).target_epoch > 0,
            begin_state_v1(state, admission.events.len()).next_epoch == state.next_epoch + 1,
{}

// Obligation 7: all admitted source epochs are strictly earlier.
pub proof fn source_epochs_precede_target_v1(
    state: OwnerStateV1, admission: AdmissionV1, index: int,
)
    requires admitted_v1(state, admission), 0 <= index < admission.events.len(),
    ensures admission.events[index].source_epoch < admission.next_epoch,
{}

// Obligation 8: begin retains exactly one reader per event and one target event.
pub proof fn begin_pin_accounting_is_exact_v1(state: OwnerStateV1, admission: AdmissionV1)
    requires admitted_v1(state, admission),
    ensures {
        let next = begin_state_v1(state, admission.events.len());
        &&& next.source_event_pins == state.source_event_pins
        &&& next.source_reader_pins == state.source_reader_pins + admission.events.len()
        &&& next.target_event_pins == state.target_event_pins + 1
        &&& next.active_targets == state.active_targets + 1
    },
{}

// Obligations 9-12: the successful lifecycle has the exact phase chain.
pub proof fn prepared_to_native_published_v1(target: TargetCustodyV1)
    requires target.phase == TargetPhaseV1::Prepared,
    ensures set_phase_v1(target, TargetPhaseV1::NativePublished).phase
        == TargetPhaseV1::NativePublished,
{}

pub proof fn native_published_to_published_v1(target: TargetCustodyV1)
    requires target.phase == TargetPhaseV1::NativePublished,
    ensures set_phase_v1(target, TargetPhaseV1::Published).phase == TargetPhaseV1::Published,
{}

pub proof fn published_to_completed_v1(target: TargetCustodyV1)
    requires target.phase == TargetPhaseV1::Published,
    ensures set_phase_v1(target, TargetPhaseV1::Completed).phase == TargetPhaseV1::Completed,
{}

pub proof fn completed_to_released_v1(target: TargetCustodyV1)
    requires target.phase == TargetPhaseV1::Completed,
    ensures set_phase_v1(target, TargetPhaseV1::Released).phase == TargetPhaseV1::Released,
{}

// Obligation 13: phase changes preserve every abstract public-custody identity.
pub proof fn public_custody_is_stable_across_phase_v1(
    target: TargetCustodyV1, phase: TargetPhaseV1,
)
    ensures {
        let next = set_phase_v1(target, phase);
        &&& next.owner == target.owner
        &&& next.session == target.session
        &&& next.target_epoch == target.target_epoch
        &&& next.dispatch_custody == target.dispatch_custody
        &&& next.target_event_custody == target.target_event_custody
        &&& next.target_signal == target.target_signal
        &&& next.source_arena == target.source_arena
    },
{}

// Obligation 14: ring-full retry is a native- and host-state no-op.
pub proof fn ring_full_retry_has_no_effect_v1(state: OwnerStateV1)
    ensures ring_full_retry_state_v1(state) == state,
            ring_full_retry_state_v1(state).native_effects == state.native_effects,
{}

// Obligation 15: retry rollback removes exactly the temporary reader/target pins.
pub proof fn retryable_rollback_is_exact_v1(state: OwnerStateV1, count: nat)
    requires state.active_targets > 0, state.source_reader_pins >= count,
             state.target_event_pins > 0,
    ensures {
        let next = rollback_state_v1(state, count);
        &&& next.active_targets + 1 == state.active_targets
        &&& next.source_event_pins == state.source_event_pins
        &&& next.source_reader_pins + count == state.source_reader_pins
        &&& next.target_event_pins + 1 == state.target_event_pins
        &&& next.native_effects == state.native_effects
    },
{}

// Obligation 16: rollback cannot rewind the already-burned epoch.
pub proof fn rollback_preserves_burned_epoch_v1(state: OwnerStateV1, count: nat)
    requires state.active_targets > 0, state.source_reader_pins >= count,
             state.target_event_pins > 0,
    ensures rollback_state_v1(state, count).next_epoch == state.next_epoch,
{}

// Obligation 17: completion requires exact epoch and stable dispatch custody.
pub proof fn exact_completion_authenticates_target_v1(
    target: TargetCustodyV1, observation: CompletionObservationV1,
)
    requires exact_completion_v1(target, observation),
    ensures target.phase == TargetPhaseV1::Published,
            observation.completed,
            observation.target_epoch == target.target_epoch,
            observation.dispatch_custody == target.dispatch_custody,
{}

// Obligation 18: a pending observation retains the unchanged owner state.
pub proof fn pending_poll_has_no_mutation_v1(state: OwnerStateV1)
    ensures pending_state_v1(state) == state,
{}

// Obligation 19: non-exact completion cannot release either source pin class.
pub proof fn inexact_completion_releases_nothing_v1(
    state: OwnerStateV1, target: TargetCustodyV1, observation: CompletionObservationV1,
)
    requires !exact_completion_v1(target, observation),
    ensures complete_or_reject_state_v1(state, target, observation) == state,
{}

// Obligation 20: exact completion atomically releases one reader/event pair per source.
pub proof fn exact_completion_releases_source_pins_once_v1(
    state: OwnerStateV1, target: TargetCustodyV1, observation: CompletionObservationV1,
)
    requires exact_completion_v1(target, observation),
             state.source_event_pins >= target.dependency_count,
             state.source_reader_pins >= target.dependency_count,
    ensures {
        let next = complete_or_reject_state_v1(state, target, observation);
        &&& next.source_event_pins + target.dependency_count == state.source_event_pins
        &&& next.source_reader_pins + target.dependency_count == state.source_reader_pins
        &&& next.target_event_pins == state.target_event_pins
        &&& next.active_targets == state.active_targets
    },
{}

// Obligation 21: a Completed custody is not completion-eligible a second time.
pub proof fn source_pin_release_is_exactly_once_v1(
    state: OwnerStateV1, target: TargetCustodyV1, observation: CompletionObservationV1,
)
    requires target.phase == TargetPhaseV1::Completed,
    ensures complete_or_reject_state_v1(state, target, observation) == state,
{}

// Obligation 22: target-event custody survives dependent source-pin discharge.
pub proof fn dependent_completion_retains_target_event_pin_v1(state: OwnerStateV1, count: nat)
    requires state.source_event_pins >= count, state.source_reader_pins >= count,
    ensures completed_state_v1(state, count).target_event_pins == state.target_event_pins,
{}

// Obligation 23: any dependency pin returns unchanged completion/state on eager recycle.
pub proof fn signal_pinned_recycle_is_proven_no_effect_v1(
    state: OwnerStateV1, completed: CompletedSignalCustodyV1,
)
    requires pinned_v1(state),
    ensures recycle_or_reject_v1(state, completed).0 == state,
            recycle_or_reject_v1(state, completed).1 == completed,
            recycle_or_reject_v1(state, completed).0.recycle_mutations
                == state.recycle_mutations,
{}

// Obligation 24: exact zero-pin recycle performs one modeled reset mutation.
pub proof fn unpinned_recycle_mutates_once_v1(
    state: OwnerStateV1, completed: CompletedSignalCustodyV1,
)
    requires !pinned_v1(state), !state.terminal,
    ensures recycle_or_reject_v1(state, completed).0.recycle_mutations
                == state.recycle_mutations + 1,
{}

// Obligation 25: released targets remove exactly one active record.
pub proof fn release_removes_one_active_target_v1(state: OwnerStateV1)
    requires state.active_targets > 0,
    ensures released_state_v1(state).active_targets + 1 == state.active_targets,
{}

// Obligation 26: terminalization retains active and pin custody.
pub proof fn terminal_retains_custody_v1(state: OwnerStateV1, target: TargetCustodyV1)
    ensures {
        let terminal = terminalize_v1(state, target);
        &&& terminal.0.terminal
        &&& terminal.0.active_targets == state.active_targets
        &&& terminal.0.source_event_pins == state.source_event_pins
        &&& terminal.0.source_reader_pins == state.source_reader_pins
        &&& terminal.0.target_event_pins == state.target_event_pins
        &&& terminal.1.phase == TargetPhaseV1::Terminal
    },
{}

// Obligation 27: the terminal state is absorbing.
pub proof fn terminal_is_absorbing_v1(state: OwnerStateV1, attempted: OwnerStateV1)
    requires state.terminal,
    ensures absorb_v1(state, attempted) == state,
{}

// Obligation 28: teardown admission is iff active and all pin counts are zero.
pub proof fn teardown_iff_quiescent_v1(state: OwnerStateV1)
    ensures teardown_allowed_v1(state) <==> {
        &&& state.active_targets == 0
        &&& state.source_event_pins == 0
        &&& state.source_reader_pins == 0
        &&& state.target_event_pins == 0
    },
{}

// Obligation 29: active target number 128 is admitted by the storage bound.
pub proof fn target_one_hundred_twenty_eight_is_admissible_v1()
    ensures 127 < active_target_bound_v1(),
{}

// Obligation 30: target number 129 is rejected by the exact storage bound.
pub proof fn target_one_hundred_twenty_nine_is_rejected_v1()
    ensures !(128 < active_target_bound_v1()),
{}

// Obligation 31: the comparison fixture names all ten successful transitions in order.
pub proof fn production_named_success_trace_is_exact_v1()
    ensures success_trace_v1().len() == 10,
            success_trace_v1()[0] == ProductionTransitionV1::EnsureTargetCapacity,
            success_trace_v1()[1] == ProductionTransitionV1::ReserveAcceptanceEpoch,
            success_trace_v1()[2] == ProductionTransitionV1::RetainDependencyReadersForTarget,
            success_trace_v1()[3] == ProductionTransitionV1::BeginTargetUse,
            success_trace_v1()[4] == ProductionTransitionV1::PublishNative,
            success_trace_v1()[5] == ProductionTransitionV1::BindPublishedTarget,
            success_trace_v1()[6] == ProductionTransitionV1::PollComputeDependencyDispatch,
            success_trace_v1()[7] == ProductionTransitionV1::ObservePublishedTargetOnce,
            success_trace_v1()[8] == ProductionTransitionV1::ReleaseDependencyReaderEventBatch,
            success_trace_v1()[9] == ProductionTransitionV1::ReleaseAfterDependentCompletion,
{}

} // verus!
