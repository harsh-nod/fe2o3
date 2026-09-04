// Independent bounded R24 portable-progress model. This is not a refinement
// of executable Rust, runtime threads, KFD, HSA, HIP, hardware, or liveness.
use vstd::prelude::*;

verus! {

pub open spec fn max_registrations_v1() -> nat { 65_536 }
pub open spec fn max_budget_v1() -> nat { 1_024 }
pub open spec fn max_window_packets_v1() -> nat { 63 }
pub open spec fn max_transfer_packets_v1() -> nat { 65 }

#[derive(PartialEq, Eq)]
pub struct KeyV1 {
    pub context_generation: nat,
    pub event_id: nat,
    pub stream_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct ConfigV1 {
    pub capacity: nat,
    pub poll_budget: nat,
    pub flush_budget: nat,
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    WindowPending { ordinal: nat, packets: nat },
    ContinuationReady { completed: nat, tail: nat, polled: bool },
    TerminalSucceeded,
    TerminalQuarantined,
}

#[derive(PartialEq, Eq)]
pub enum PollDispositionV1 { Pending, Retryable, Completed, TerminalFailure }

#[derive(PartialEq, Eq)]
pub enum FlushDispositionV1 { Published, Retryable, TerminalFailure }

#[derive(PartialEq, Eq)]
pub struct EntryV1 {
    pub key: KeyV1,
    pub total_packets: nat,
    pub event_installed: bool,
    pub stream_installed: bool,
    pub custody_retained: bool,
    pub observing: bool,
    pub abandoned: bool,
    pub phase: PhaseV1,
}

#[derive(PartialEq, Eq)]
pub struct RegistrationCountsV1 {
    pub entries: nat,
    pub events: nat,
    pub streams: nat,
}

#[derive(PartialEq, Eq)]
pub struct EngineV1 {
    pub config: ConfigV1,
    pub registrations: RegistrationCountsV1,
    pub poll_cursor: nat,
    pub flush_cursor: nat,
    pub poll_visits: nat,
    pub flush_visits: nat,
    pub stopped: bool,
}

pub open spec fn valid_key_v1(key: KeyV1) -> bool {
    key.context_generation > 0 && key.event_id > 0 && key.stream_id > 0
}

pub open spec fn valid_config_v1(config: ConfigV1) -> bool {
    0 < config.capacity <= max_registrations_v1()
        && 0 < config.poll_budget <= max_budget_v1()
        && 0 < config.flush_budget <= max_budget_v1()
}

pub open spec fn valid_registration_counts_v1(counts: RegistrationCountsV1,
    capacity: nat) -> bool
{
    counts.events == counts.streams && counts.events <= counts.entries
        && counts.events <= capacity && counts.entries <= max_registrations_v1()
}

pub open spec fn initial_packets_v1(total: nat) -> nat {
    if total <= max_window_packets_v1() { total } else { max_window_packets_v1() }
}

pub open spec fn remaining_packets_v1(total: nat, completed: nat) -> nat {
    if completed <= total { (total - completed) as nat } else { 0 }
}

pub open spec fn valid_phase_v1(entry: EntryV1) -> bool {
    match entry.phase {
        PhaseV1::WindowPending { ordinal, packets } =>
            (ordinal == 0 && packets == initial_packets_v1(entry.total_packets))
            || (ordinal == 1 && entry.total_packets > max_window_packets_v1()
                && packets == remaining_packets_v1(entry.total_packets, max_window_packets_v1())),
        PhaseV1::ContinuationReady { completed, tail, polled } =>
            entry.total_packets > max_window_packets_v1()
                && completed == max_window_packets_v1()
                && tail == remaining_packets_v1(entry.total_packets, completed) && polled,
        PhaseV1::TerminalSucceeded | PhaseV1::TerminalQuarantined => !entry.observing,
    }
}

pub open spec fn valid_entry_v1(entry: EntryV1) -> bool {
    &&& valid_key_v1(entry.key)
    &&& 0 < entry.total_packets <= max_transfer_packets_v1()
    &&& entry.event_installed && entry.stream_installed
    &&& entry.custody_retained
    &&& (!entry.abandoned || !entry.observing)
    &&& valid_phase_v1(entry)
}

pub open spec fn valid_engine_v1(engine: EngineV1) -> bool {
    &&& valid_config_v1(engine.config)
    &&& valid_registration_counts_v1(engine.registrations, engine.config.capacity)
    &&& (engine.registrations.entries == 0
        || (engine.poll_cursor < engine.registrations.entries
            && engine.flush_cursor < engine.registrations.entries))
}

pub open spec fn registration_allowed_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat, event_duplicate: bool, stream_duplicate: bool,
    preflight_ok: bool) -> bool
{
    valid_engine_v1(engine) && !engine.stopped && valid_key_v1(key)
        && 0 < total_packets <= max_transfer_packets_v1()
        && !event_duplicate && !stream_duplicate && preflight_ok
        && engine.registrations.events < engine.config.capacity
        && engine.registrations.entries < max_registrations_v1()
}

pub open spec fn register_pair_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat, event_duplicate: bool, stream_duplicate: bool,
    preflight_ok: bool) -> EngineV1
{
    if registration_allowed_v1(engine, key, total_packets, event_duplicate,
        stream_duplicate, preflight_ok) {
        EngineV1 {
            registrations: RegistrationCountsV1 {
                entries: engine.registrations.entries + 1,
                events: engine.registrations.events + 1,
                streams: engine.registrations.streams + 1 },
            ..engine
        }
    } else { engine }
}

pub open spec fn retire_registration_v1(engine: EngineV1) -> EngineV1 {
    if engine.registrations.events > 0 && engine.registrations.streams > 0 {
        EngineV1 {
            registrations: RegistrationCountsV1 {
                entries: engine.registrations.entries,
                events: (engine.registrations.events - 1) as nat,
                streams: (engine.registrations.streams - 1) as nat },
            ..engine
        }
    } else { engine }
}

pub open spec fn admitted_poll_count_v1(config: ConfigV1, requested: nat) -> nat {
    if valid_config_v1(config) && requested <= config.poll_budget { requested } else { 0 }
}

pub open spec fn admitted_flush_count_v1(config: ConfigV1, requested: nat) -> nat {
    if valid_config_v1(config) && requested <= config.flush_budget { requested } else { 0 }
}

pub open spec fn cyclic_slot_v1(cursor: nat, distance: nat, slots: nat) -> nat {
    (cursor + distance) % slots
}

pub open spec fn cursor_after_v1(cursor: nat, visits: nat, slots: nat) -> nat {
    (cursor + visits) % slots
}

pub open spec fn sample_key_v1() -> KeyV1 {
    KeyV1 { context_generation: 24, event_id: 25, stream_id: 26 }
}

pub open spec fn sample_config_v1() -> ConfigV1 {
    ConfigV1 { capacity: 4, poll_budget: 2, flush_budget: 1 }
}

pub open spec fn sample_engine_v1() -> EngineV1 {
    EngineV1 { config: sample_config_v1(),
        registrations: RegistrationCountsV1 { entries: 0, events: 0, streams: 0 },
        poll_cursor: 0, flush_cursor: 0, poll_visits: 0, flush_visits: 0,
        stopped: false }
}

pub open spec fn sample_first_window_v1() -> EntryV1 {
    EntryV1 { key: sample_key_v1(), total_packets: 65,
        event_installed: true, stream_installed: true, custody_retained: true,
        observing: true, abandoned: false,
        phase: PhaseV1::WindowPending { ordinal: 0, packets: 63 } }
}

pub open spec fn poll_entry_v1(entry: EntryV1, disposition: PollDispositionV1)
    -> EntryV1
{
    match entry.phase {
        PhaseV1::TerminalSucceeded | PhaseV1::TerminalQuarantined => entry,
        PhaseV1::WindowPending { ordinal, packets } => match disposition {
            PollDispositionV1::Pending => entry,
            PollDispositionV1::Retryable => EntryV1 { observing: false, ..entry },
            PollDispositionV1::TerminalFailure => EntryV1 {
                phase: PhaseV1::TerminalQuarantined, observing: false, ..entry },
            PollDispositionV1::Completed => {
                if ordinal == 1 || packets == entry.total_packets {
                    EntryV1 { phase: PhaseV1::TerminalSucceeded, observing: false, ..entry }
                } else {
                    EntryV1 { phase: PhaseV1::ContinuationReady {
                        completed: packets, tail: remaining_packets_v1(entry.total_packets, packets),
                        polled: true }, ..entry }
                }
            },
        },
        PhaseV1::ContinuationReady { .. } => entry,
    }
}

pub open spec fn flush_entry_v1(entry: EntryV1, disposition: FlushDispositionV1)
    -> EntryV1
{
    match entry.phase {
        PhaseV1::TerminalSucceeded | PhaseV1::TerminalQuarantined => entry,
        PhaseV1::ContinuationReady { completed: _, tail, polled } => {
            match disposition {
                FlushDispositionV1::Retryable => entry,
                FlushDispositionV1::TerminalFailure => EntryV1 {
                    phase: PhaseV1::TerminalQuarantined, observing: false, ..entry },
                FlushDispositionV1::Published => {
                    if polled {
                        EntryV1 { phase: PhaseV1::WindowPending {
                            ordinal: 1, packets: tail }, ..entry }
                    } else { entry }
                },
            }
        },
        PhaseV1::WindowPending { .. } => entry,
    }
}

pub open spec fn abandon_v1(entry: EntryV1) -> EntryV1 {
    EntryV1 { observing: false, abandoned: true, ..entry }
}

pub open spec fn stop_v1(engine: EngineV1) -> EngineV1 {
    EngineV1 {
        registrations: RegistrationCountsV1 {
            entries: engine.registrations.entries, events: 0, streams: 0 },
        stopped: true,
        ..engine
    }
}

pub proof fn constants_are_exact_v1()
    ensures max_registrations_v1() == 65536, max_budget_v1() == 1024,
        max_window_packets_v1() == 63, max_transfer_packets_v1() == 65, {}

pub proof fn sample_values_are_valid_v1()
    ensures valid_key_v1(sample_key_v1()), valid_config_v1(sample_config_v1()),
        valid_engine_v1(sample_engine_v1()), valid_entry_v1(sample_first_window_v1()), {}

pub proof fn successful_registration_installs_event_and_stream_atomically_v1(
    engine: EngineV1, key: KeyV1, total_packets: nat)
    requires registration_allowed_v1(engine, key, total_packets, false, false, true),
    ensures {
        let installed = register_pair_v1(engine, key, total_packets, false, false, true);
        &&& installed.registrations.entries == engine.registrations.entries + 1
        &&& installed.registrations.events == engine.registrations.events + 1
        &&& installed.registrations.streams == engine.registrations.streams + 1
    }, {}

pub proof fn rejected_event_preflight_has_no_half_install_v1(engine: EngineV1,
    key: KeyV1, total_packets: nat)
    ensures register_pair_v1(engine, key, total_packets, false, false, false) == engine, {}

pub proof fn duplicate_event_registration_is_atomic_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    ensures register_pair_v1(engine, key, total_packets, true, false, true) == engine, {}

pub proof fn duplicate_stream_registration_is_atomic_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    ensures register_pair_v1(engine, key, total_packets, false, true, true) == engine, {}

pub proof fn stopped_registration_is_atomic_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    requires engine.stopped,
    ensures register_pair_v1(engine, key, total_packets, false, false, true) == engine, {}

pub proof fn registration_capacity_is_bounded_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    requires valid_engine_v1(engine),
        engine.registrations.events == engine.config.capacity,
    ensures register_pair_v1(engine, key, total_packets, false, false, true) == engine, {}

pub proof fn registration_history_is_bounded_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    requires valid_engine_v1(engine),
        engine.registrations.entries == max_registrations_v1(),
    ensures register_pair_v1(engine, key, total_packets, false, false, true) == engine, {}

pub proof fn retired_registration_frees_active_capacity_v1(engine: EngineV1, key: KeyV1,
    total_packets: nat)
    requires valid_engine_v1(engine), !engine.stopped,
        engine.registrations.events > 0,
        engine.registrations.entries < max_registrations_v1(),
        valid_key_v1(key), 0 < total_packets <= max_transfer_packets_v1(),
    ensures registration_allowed_v1(retire_registration_v1(engine), key, total_packets,
        false, false, true), {}

pub proof fn poll_budget_is_bounded_v1(config: ConfigV1, requested: nat)
    requires valid_config_v1(config),
    ensures admitted_poll_count_v1(config, requested) <= config.poll_budget, {}

pub proof fn flush_budget_is_bounded_v1(config: ConfigV1, requested: nat)
    requires valid_config_v1(config),
    ensures admitted_flush_count_v1(config, requested) <= config.flush_budget, {}

pub proof fn poll_and_flush_budgets_are_independent_v1(config: ConfigV1,
    poll_requested: nat, flush_requested: nat)
    requires valid_config_v1(config), poll_requested <= config.poll_budget,
        flush_requested > config.flush_budget,
    ensures admitted_poll_count_v1(config, poll_requested) == poll_requested,
        admitted_flush_count_v1(config, flush_requested) == 0, {}

pub proof fn cyclic_visitation_stays_in_roster_v1(cursor: nat, distance: nat,
    slots: nat)
    requires slots > 0, cursor < slots,
    ensures cyclic_slot_v1(cursor, distance, slots) < slots, {}

pub proof fn cyclic_visitation_wraps_stably_v1()
    ensures cyclic_slot_v1(2, 0, 3) == 2, cyclic_slot_v1(2, 1, 3) == 0,
        cyclic_slot_v1(2, 2, 3) == 1, cursor_after_v1(2, 2, 3) == 1, {}

pub proof fn pending_poll_is_observation_only_v1(entry: EntryV1)
    ensures poll_entry_v1(entry, PollDispositionV1::Pending) == entry, {}

pub proof fn retryable_poll_preserves_registration_and_custody_v1(entry: EntryV1)
    requires match entry.phase {
        PhaseV1::WindowPending { .. } => true,
        _ => false,
    },
    ensures {
        let retry = poll_entry_v1(entry, PollDispositionV1::Retryable);
        &&& retry.event_installed == entry.event_installed
        &&& retry.stream_installed == entry.stream_installed
        &&& retry.custody_retained == entry.custody_retained
        &&& retry.phase == entry.phase
        &&& !retry.observing
    }, {}

pub proof fn first_window_completion_requires_later_flush_v1()
    ensures poll_entry_v1(sample_first_window_v1(), PollDispositionV1::Completed).phase
        == (PhaseV1::ContinuationReady { completed: 63, tail: 2, polled: true }), {}

pub proof fn continuation_publication_is_poll_gated_v1(entry: EntryV1)
    requires entry.phase == (PhaseV1::ContinuationReady {
        completed: 63, tail: 2, polled: false }),
    ensures flush_entry_v1(entry, FlushDispositionV1::Published) == entry, {}

pub proof fn polled_continuation_publishes_exact_tail_v1()
    ensures {
        let polled = poll_entry_v1(sample_first_window_v1(), PollDispositionV1::Completed);
        flush_entry_v1(polled, FlushDispositionV1::Published).phase
            == (PhaseV1::WindowPending { ordinal: 1, packets: 2 })
    }, {}

pub proof fn retryable_flush_preserves_registration_and_custody_v1(entry: EntryV1)
    ensures flush_entry_v1(entry, FlushDispositionV1::Retryable) == entry, {}

pub proof fn tail_completion_is_terminal_without_third_window_v1()
    ensures {
        let first = poll_entry_v1(sample_first_window_v1(), PollDispositionV1::Completed);
        let tail = flush_entry_v1(first, FlushDispositionV1::Published);
        poll_entry_v1(tail, PollDispositionV1::Completed).phase
            == PhaseV1::TerminalSucceeded
        && !poll_entry_v1(tail, PollDispositionV1::Completed).observing
    }, {}

pub proof fn poll_failure_is_terminal_and_retaining_v1(entry: EntryV1)
    requires valid_entry_v1(entry),
        entry.phase == (PhaseV1::WindowPending { ordinal: 0,
            packets: initial_packets_v1(entry.total_packets) }),
    ensures {
        let failed = poll_entry_v1(entry, PollDispositionV1::TerminalFailure);
        &&& failed.phase == PhaseV1::TerminalQuarantined
        &&& !failed.observing
        &&& failed.event_installed && failed.stream_installed && failed.custody_retained
    }, {}

pub proof fn flush_failure_is_terminal_and_retaining_v1(entry: EntryV1)
    requires valid_entry_v1(entry),
        entry.phase == (PhaseV1::ContinuationReady { completed: 63, tail: 2, polled: true }),
    ensures {
        let failed = flush_entry_v1(entry, FlushDispositionV1::TerminalFailure);
        &&& failed.phase == PhaseV1::TerminalQuarantined
        &&& !failed.observing
        &&& failed.event_installed && failed.stream_installed && failed.custody_retained
    }, {}

pub proof fn terminal_success_is_absorbing_v1(entry: EntryV1,
    poll: PollDispositionV1, flush: FlushDispositionV1)
    requires entry.phase == PhaseV1::TerminalSucceeded,
    ensures poll_entry_v1(entry, poll) == entry, flush_entry_v1(entry, flush) == entry, {}

pub proof fn terminal_quarantine_is_absorbing_v1(entry: EntryV1,
    poll: PollDispositionV1, flush: FlushDispositionV1)
    requires entry.phase == PhaseV1::TerminalQuarantined,
    ensures poll_entry_v1(entry, poll) == entry, flush_entry_v1(entry, flush) == entry, {}

pub proof fn abandon_is_observation_only_v1(entry: EntryV1)
    ensures {
        let abandoned = abandon_v1(entry);
        &&& !abandoned.observing && abandoned.abandoned
        &&& abandoned.phase == entry.phase
        &&& abandoned.event_installed == entry.event_installed
        &&& abandoned.stream_installed == entry.stream_installed
        &&& abandoned.custody_retained == entry.custody_retained
    }, {}

pub proof fn drop_has_same_observation_only_shape_v1(entry: EntryV1)
    ensures abandon_v1(entry).phase == entry.phase,
        abandon_v1(entry).custody_retained == entry.custody_retained, {}

pub proof fn stop_performs_no_final_progress_v1(engine: EngineV1)
    ensures stop_v1(engine).poll_visits == engine.poll_visits,
        stop_v1(engine).flush_visits == engine.flush_visits,
        stop_v1(engine).poll_cursor == engine.poll_cursor,
        stop_v1(engine).flush_cursor == engine.flush_cursor, {}

pub proof fn stop_preserves_registration_history_v1(engine: EngineV1)
    ensures stop_v1(engine).registrations.entries == engine.registrations.entries, {}

pub proof fn stop_retires_active_registration_counts_v1(engine: EngineV1)
    ensures stop_v1(engine).registrations.events == 0,
        stop_v1(engine).registrations.streams == 0, {}

pub proof fn stop_is_absorbing_v1(engine: EngineV1)
    ensures stop_v1(stop_v1(engine)) == stop_v1(engine), {}

pub proof fn sixty_five_packets_plan_as_sixty_three_plus_two_v1()
    ensures initial_packets_v1(max_transfer_packets_v1()) == 63,
        remaining_packets_v1(max_transfer_packets_v1(),
            initial_packets_v1(max_transfer_packets_v1())) == 2, {}

fn main() {}

}
