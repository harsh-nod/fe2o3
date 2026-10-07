//! Private tail-fence wait for one sealed striped-SDMA submission.
//!
//! Each ordinary gfx942 submission is one linear copy followed by the fence
//! encoded by `SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1` (`mtype=3`, `sys=1`,
//! `snoop=1`). The optimization relies on the contracted gfx942 queue rule
//! that one queue executes these packet occurrences in submission order and
//! that observing its exact tail fence value means all preceding copy/fence
//! occurrences on that queue are complete and system-visible. The binding
//! below checks the exact queue occurrence, slot, generation, and retained
//! record for every tail. Firmware ordering and CPU/GPU coherence remain
//! external native contracts; this module is not a Rust refinement of R46.

use std::time::{Duration, Instant};

use super::tail_wait_cpu::{
    ThreadWaitCpuMeasurementV1, finish_tail_cpu_measurement_v1, profile_tail_cpu_snapshot_v1,
};
use super::{
    Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaQueueSetV1,
    Gfx942SdmaStripedDiagnosticSpinBudgetV1, Gfx942SdmaStripedWaitCpuMeasurementStatusV1,
    Gfx942SdmaStripedWaitDiagnosticsV1, ValidatedMultiQueueCompletionEntryV1,
};
use crate::sdma::{
    GFX942_SDMA_MAX_STRIPED_QUEUES_V1, Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1,
    SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1, SDMA_OP_FENCE,
};
use crate::shared_memory::SharedGttMemorySessionV1;
use crate::wait::{MonotonicWaitV1, WaitActionV1};

const GFX942_STRIPED_SDMA_MAX_SLEEP_V1: Duration = Duration::from_micros(25);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundStripedTailV1 {
    queue_ordinal: u16,
    request_index: u16,
    ticket: Gfx942SdmaCopyTicketV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundActiveQueueV1 {
    queue_ordinal: u8,
    queue_bit: u16,
}

struct PreparedStripedTailWaitV1<'a> {
    submission: &'a Gfx942SdmaMultiQueueSubmissionV1,
    tails_by_queue: [Option<BoundStripedTailV1>; GFX942_SDMA_MAX_STRIPED_QUEUES_V1],
    active_queues: [BoundActiveQueueV1; GFX942_SDMA_MAX_STRIPED_QUEUES_V1],
    active_queue_mask: u16,
    tail_count: u8,
}

impl<'a> PreparedStripedTailWaitV1<'a> {
    fn bind(
        queue_set: &Gfx942SdmaQueueSetV1,
        submission: &'a Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        if !gfx942_system_snoop_tail_fence_is_exact_v1() {
            return Err(Gfx942SdmaErrorV1::Contract("striped tail fence encoding"));
        }
        queue_set.confirm_striped_multi_queue_completion(submission)?;
        let owners = queue_set.multi_queue_owners_v1()?;

        let mut tails_by_queue = [None; GFX942_SDMA_MAX_STRIPED_QUEUES_V1];
        let mut active_queues = [BoundActiveQueueV1 {
            queue_ordinal: 0,
            queue_bit: 0,
        }; GFX942_SDMA_MAX_STRIPED_QUEUES_V1];
        let mut active_queue_mask = 0_u16;
        let mut tail_count = 0_usize;
        for shard in &submission.shards {
            let queue_ordinal = shard.queue_ordinal();
            let Some(owner) = owners.get(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail owner disappeared",
                ));
            };
            owner.require_live()?;
            let (Some(&request_index), Some(&ticket)) =
                (shard.request_indices.last(), shard.tickets.last())
            else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail requires a nonempty shard",
                ));
            };
            let Ok(queue_ordinal_u16) = u16::try_from(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail queue ordinal width",
                ));
            };
            let tail = BoundStripedTailV1 {
                queue_ordinal: queue_ordinal_u16,
                request_index,
                ticket,
            };
            if !bound_tail_matches_ordered_roster_v1(&submission.completion.ordered, tail) {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail completion-roster substitution",
                ));
            }
            let slot_index = match owner.validate_ticket(ticket) {
                Ok(slot) => slot,
                Err(_) => {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "striped tail ticket substitution",
                    ));
                }
            };
            if !striped_owner_engine_matches_v1(queue_ordinal, owner.engine_index) {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail queue-to-engine binding",
                ));
            }
            if owner.completions.is_none()
                || owner
                    .records
                    .get(slot_index)
                    .and_then(Option::as_ref)
                    .is_none_or(|record| {
                        record.fence_header != SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1
                            || !tail_signal_generation_is_exact_v1(
                                ticket.generation,
                                record.completion_value,
                            )
                    })
            {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail signal-fence binding",
                ));
            }
            let Some(tail_slot) = tails_by_queue.get_mut(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract("striped tail roster capacity"));
            };
            if tail_slot.is_some() {
                return Err(Gfx942SdmaErrorV1::Contract("duplicate striped tail queue"));
            }
            let Some(active_queue) = active_queues.get_mut(tail_count) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped active-tail roster capacity",
                ));
            };
            let Some(queue_bit) = 1_u16.checked_shl(queue_ordinal as u32) else {
                return Err(Gfx942SdmaErrorV1::Contract("striped tail queue ordinal"));
            };
            let Ok(queue_ordinal_u8) = u8::try_from(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped tail queue ordinal width",
                ));
            };
            *tail_slot = Some(tail);
            *active_queue = BoundActiveQueueV1 {
                queue_ordinal: queue_ordinal_u8,
                queue_bit,
            };
            active_queue_mask |= queue_bit;
            tail_count += 1;
        }
        if tail_count != submission.plan.active_shard_count() || tail_count == 0 {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped tail active-shard roster",
            ));
        }

        Ok(Self {
            submission,
            tails_by_queue,
            active_queues,
            active_queue_mask,
            tail_count: tail_count as u8,
        })
    }
}

enum StripedFullAuditOutcomeV1 {
    AllReady,
    Pending,
    TailOrderingViolation,
}

/// Move-only proof that one exact borrowed submission passed the sole ordered
/// status/preflight scan and the closing operational-currentness check.
struct Gfx942SdmaStripedAllReadyAuditV1<'a> {
    submission: &'a Gfx942SdmaMultiQueueSubmissionV1,
}

/// Opaque permit minted only after matching the all-ready audit back to the
/// still externally retained submission.
struct Gfx942SdmaStripedRetirementPermitV1 {
    ordered_len: usize,
}

impl Gfx942SdmaStripedAllReadyAuditV1<'_> {
    /// Consumes the exact lifetime-bound borrow before the caller moves the
    /// sole retained submission into the abort-only retirement suffix.
    fn authorize_retirement(self) -> Gfx942SdmaStripedRetirementPermitV1 {
        Gfx942SdmaStripedRetirementPermitV1 {
            ordered_len: self.submission.completion.ordered.len(),
        }
    }
}

enum Gfx942SdmaStripedTailWaitAuditV1<'a> {
    AllReady(Gfx942SdmaStripedAllReadyAuditV1<'a>),
    Pending,
    TailOrderingViolation,
}

pub(crate) enum Gfx942SdmaStripedTailWaitOutcomeV1 {
    Completed(Gfx942SdmaMultiQueueCompletedV1),
    Pending,
    Terminal(Gfx942SdmaErrorV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StripedFullAuditDispositionV1 {
    AllReady,
    TimedOut,
    TailOrderingViolation,
}

trait TailWaitCursorV1 {
    fn deadline_reached(&self) -> bool;
    fn pause(&mut self) -> WaitActionV1;
}

impl TailWaitCursorV1 for MonotonicWaitV1 {
    fn deadline_reached(&self) -> bool {
        MonotonicWaitV1::expired(self)
    }

    fn pause(&mut self) -> WaitActionV1 {
        MonotonicWaitV1::pause_observed(self)
    }
}

fn saturating_duration_ns_v1(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn profile_start_v1<const PROFILE: bool>() -> Option<Instant> {
    if PROFILE { Some(Instant::now()) } else { None }
}

fn profile_elapsed_ns_v1<const PROFILE: bool>(started: Option<Instant>) -> u64 {
    if PROFILE {
        started.map_or(0, |started| saturating_duration_ns_v1(started.elapsed()))
    } else {
        0
    }
}

const fn diagnostic_spin_budget_is_admitted_for_profile_v1<const PROFILE: bool>(
    diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
) -> bool {
    PROFILE
        || matches!(
            diagnostic_spin_budget,
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current
        )
}

fn record_thread_wait_cpu_measurement_v1(
    diagnostics: &mut Gfx942SdmaStripedWaitDiagnosticsV1,
    measurement: ThreadWaitCpuMeasurementV1,
) {
    diagnostics.tail_scan_thread_cpu_ns = None;
    diagnostics.tail_scan_voluntary_context_switches = None;
    diagnostics.tail_scan_involuntary_context_switches = None;
    match measurement {
        ThreadWaitCpuMeasurementV1::Available {
            thread_cpu_ns,
            voluntary_context_switches,
            involuntary_context_switches,
        } => {
            diagnostics.tail_scan_cpu_measurement_status =
                Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Available;
            diagnostics.tail_scan_thread_cpu_ns = Some(thread_cpu_ns);
            diagnostics.tail_scan_voluntary_context_switches = Some(voluntary_context_switches);
            diagnostics.tail_scan_involuntary_context_switches = Some(involuntary_context_switches);
        }
        ThreadWaitCpuMeasurementV1::Unavailable => {
            diagnostics.tail_scan_cpu_measurement_status =
                Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Unavailable;
        }
        ThreadWaitCpuMeasurementV1::Invalid => {
            diagnostics.tail_scan_cpu_measurement_status =
                Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Invalid;
        }
    }
}

fn observe_tail_rounds_profiled_until_v1<const PROFILE: bool, T, E, W: TailWaitCursorV1>(
    tails: &[T],
    mut tail_queue_bit: impl FnMut(&T) -> u16,
    mut observe_tail: impl FnMut(&T) -> Result<bool, E>,
    wait: &mut W,
    diagnostics: &mut Gfx942SdmaStripedWaitDiagnosticsV1,
    scan_started: Option<Instant>,
) -> Result<u16, E> {
    loop {
        if PROFILE {
            diagnostics.tail_scan_rounds = diagnostics.tail_scan_rounds.saturating_add(1);
        }
        let mut active_queue_mask = 0_u16;
        let mut ready_queue_mask = 0_u16;
        for tail in tails {
            let queue_bit = tail_queue_bit(tail);
            active_queue_mask |= queue_bit;
            if PROFILE {
                diagnostics.tail_observations = diagnostics.tail_observations.saturating_add(1);
            }
            if observe_tail(tail)? {
                ready_queue_mask |= queue_bit;
            }
        }
        if PROFILE && ready_queue_mask != 0 && diagnostics.first_tail_ready_ns.is_none() {
            diagnostics.first_tail_ready_ns = Some(profile_elapsed_ns_v1::<PROFILE>(scan_started));
        }
        if ready_queue_mask == active_queue_mask {
            if PROFILE {
                diagnostics.all_tails_ready_ns =
                    Some(profile_elapsed_ns_v1::<PROFILE>(scan_started));
            }
            return Ok(ready_queue_mask);
        }
        if wait.deadline_reached() {
            return Ok(ready_queue_mask);
        }
        let action = wait.pause();
        if PROFILE {
            match action {
                WaitActionV1::Spin => {
                    diagnostics.spin_pauses = diagnostics.spin_pauses.saturating_add(1);
                }
                WaitActionV1::Yield => {
                    diagnostics.yield_pauses = diagnostics.yield_pauses.saturating_add(1);
                }
                WaitActionV1::Sleep(duration) => {
                    diagnostics.sleep_pauses = diagnostics.sleep_pauses.saturating_add(1);
                    diagnostics.requested_sleep_ns = diagnostics
                        .requested_sleep_ns
                        .saturating_add(saturating_duration_ns_v1(duration));
                }
            }
        }
    }
}

#[cfg(test)]
fn observe_tail_rounds_until_v1<T, E, W: TailWaitCursorV1>(
    tails: &[T],
    tail_queue_bit: impl FnMut(&T) -> u16,
    observe_tail: impl FnMut(&T) -> Result<bool, E>,
    wait: &mut W,
) -> Result<u16, E> {
    observe_tail_rounds_profiled_until_v1::<false, _, _, _>(
        tails,
        tail_queue_bit,
        observe_tail,
        wait,
        &mut Gfx942SdmaStripedWaitDiagnosticsV1::default(),
        None,
    )
}

fn observe_full_ordered_roster_v1<T, E>(
    entries: &[T],
    mut observe: impl FnMut(&T) -> Result<(u16, bool), E>,
) -> Result<(bool, u16), E> {
    let mut all_entries_ready = true;
    let mut pending_queue_mask = 0_u16;
    for entry in entries {
        let (queue_bit, ready) = observe(entry)?;
        if !ready {
            all_entries_ready = false;
            pending_queue_mask |= queue_bit;
        }
    }
    Ok((all_entries_ready, pending_queue_mask))
}

fn bound_tail_matches_ordered_roster_v1(
    ordered: &[ValidatedMultiQueueCompletionEntryV1],
    tail: BoundStripedTailV1,
) -> bool {
    ordered.get(usize::from(tail.request_index))
        == Some(&ValidatedMultiQueueCompletionEntryV1 {
            request_index: tail.request_index,
            queue_ordinal: tail.queue_ordinal,
            ticket: tail.ticket,
        })
}

const fn classify_striped_full_audit_v1(
    all_entries_ready: bool,
    pending_queue_mask: u16,
    ready_tail_queue_mask: u16,
) -> StripedFullAuditDispositionV1 {
    if all_entries_ready {
        StripedFullAuditDispositionV1::AllReady
    } else if pending_queue_mask & ready_tail_queue_mask != 0 {
        StripedFullAuditDispositionV1::TailOrderingViolation
    } else {
        StripedFullAuditDispositionV1::TimedOut
    }
}

const fn gfx942_system_snoop_tail_fence_is_exact_v1() -> bool {
    const MTYPE_MASK: u32 = 3 << 16;
    const SYSTEM_SCOPE: u32 = 1 << 20;
    const SNOOP: u32 = 1 << 22;
    SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1 == SDMA_OP_FENCE | MTYPE_MASK | SYSTEM_SCOPE | SNOOP
}

const fn tail_signal_generation_is_exact_v1(ticket_generation: u32, completion_value: u32) -> bool {
    ticket_generation != 0 && completion_value == ticket_generation
}

fn striped_owner_engine_matches_v1(queue_ordinal: usize, engine_index: Option<u32>) -> bool {
    engine_index == Some((queue_ordinal % 2) as u32)
}

fn abort_if_striped_retirement_unwinds_v1<R>(operation: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => {
            core::mem::forget(payload);
            std::process::abort();
        }
    }
}

fn with_borrowed_striped_submission_v1<'a, T, R, E>(
    retained: &'a Option<T>,
    operation: impl FnOnce(&'a T) -> Result<R, E>,
) -> Result<R, E> {
    let Some(submission) = retained.as_ref() else {
        std::process::abort();
    };
    operation(submission)
}

impl Gfx942SdmaQueueSetV1 {
    /// Keeps the caller's exact submission in `retained` through every
    /// fallible/native operation. The sole move occurs only after the private
    /// lifetime-bound audit is consumed, immediately before the abort-only
    /// retirement suffix.
    pub(crate) fn wait_prepared_striped_multi_queue_tails_retaining_for<const PROFILE: bool>(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        retained: &mut Option<Gfx942SdmaMultiQueueSubmissionV1>,
        timeout: Duration,
        diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
        diagnostics: &mut Gfx942SdmaStripedWaitDiagnosticsV1,
    ) -> Gfx942SdmaStripedTailWaitOutcomeV1 {
        if !diagnostic_spin_budget_is_admitted_for_profile_v1::<PROFILE>(diagnostic_spin_budget) {
            return Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(Gfx942SdmaErrorV1::Contract(
                "diagnostic striped spin budget requires profiling",
            ));
        }
        if PROFILE {
            diagnostics.diagnostic_spin_budget = diagnostic_spin_budget;
        }
        let audit = with_borrowed_striped_submission_v1(retained, |submission| {
            self.audit_prepared_striped_multi_queue_tails_for::<PROFILE>(
                memory,
                submission,
                timeout,
                diagnostic_spin_budget,
                diagnostics,
            )
        });
        match audit {
            Ok(Gfx942SdmaStripedTailWaitAuditV1::AllReady(audit)) => {
                let permit = audit.authorize_retirement();
                let submission = retained.take().unwrap_or_else(|| std::process::abort());
                let retirement_started = profile_start_v1::<PROFILE>();
                let completed =
                    self.retire_after_striped_full_audit_no_unwind_v1(permit, submission);
                if PROFILE {
                    diagnostics.retirement_ns =
                        profile_elapsed_ns_v1::<PROFILE>(retirement_started);
                }
                Gfx942SdmaStripedTailWaitOutcomeV1::Completed(completed)
            }
            Ok(Gfx942SdmaStripedTailWaitAuditV1::Pending) => {
                Gfx942SdmaStripedTailWaitOutcomeV1::Pending
            }
            Ok(Gfx942SdmaStripedTailWaitAuditV1::TailOrderingViolation) => {
                Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(Gfx942SdmaErrorV1::Contract(
                    "ready striped tail preceded by pending copy",
                ))
            }
            Err(error) => Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(error),
        }
    }

    fn audit_prepared_striped_multi_queue_tails_for<'a, const PROFILE: bool>(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        submission: &'a Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
        diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
        diagnostics: &mut Gfx942SdmaStripedWaitDiagnosticsV1,
    ) -> Result<Gfx942SdmaStripedTailWaitAuditV1<'a>, Gfx942SdmaErrorV1> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("striped tail wait deadline"))?;
        let bind_started = profile_start_v1::<PROFILE>();
        let prepared = PreparedStripedTailWaitV1::bind(self, submission)?;
        if PROFILE {
            diagnostics.active_queue_count = prepared.tail_count;
            diagnostics.request_count =
                u16::try_from(submission.plan.request_count()).unwrap_or(u16::MAX);
            diagnostics.bind_ns = profile_elapsed_ns_v1::<PROFILE>(bind_started);
        }

        let opening_currentness_started = profile_start_v1::<PROFILE>();
        memory.check_queue_operational_currentness()?;
        if PROFILE {
            diagnostics.opening_currentness_ns =
                profile_elapsed_ns_v1::<PROFILE>(opening_currentness_started);
        }
        let mut wait = match diagnostic_spin_budget.active_spin_floor() {
            Some(active_spin_floor) => {
                MonotonicWaitV1::until_with_active_spin_floor_and_sleep_ceiling(
                    deadline,
                    active_spin_floor,
                    GFX942_STRIPED_SDMA_MAX_SLEEP_V1,
                )
            }
            None => MonotonicWaitV1::until_with_sleep_ceiling(
                deadline,
                GFX942_STRIPED_SDMA_MAX_SLEEP_V1,
            ),
        };
        let active_queues = &prepared.active_queues[..usize::from(prepared.tail_count)];
        let tail_scan_started = profile_start_v1::<PROFILE>();
        let tail_scan_cpu_started = profile_tail_cpu_snapshot_v1::<PROFILE>();
        let ready_tail_queue_mask = observe_tail_rounds_profiled_until_v1::<PROFILE, _, _, _>(
            active_queues,
            |active_queue| active_queue.queue_bit,
            |active_queue| {
                let tail = prepared
                    .tails_by_queue
                    .get(usize::from(active_queue.queue_ordinal))
                    .copied()
                    .flatten()
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "bound striped tail roster changed",
                    ))?;
                if !bound_tail_matches_ordered_roster_v1(
                    &prepared.submission.completion.ordered,
                    tail,
                ) {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "striped tail changed after binding",
                    ));
                }
                self.observe_bound_striped_tail_v1(memory, tail)
            },
            &mut wait,
            diagnostics,
            tail_scan_started,
        );
        if PROFILE {
            record_thread_wait_cpu_measurement_v1(
                diagnostics,
                finish_tail_cpu_measurement_v1(tail_scan_cpu_started),
            );
            diagnostics.tail_scan_ns = profile_elapsed_ns_v1::<PROFILE>(tail_scan_started);
        }
        let ready_tail_queue_mask = ready_tail_queue_mask?;

        let final_audit_started = profile_start_v1::<PROFILE>();
        let final_audit =
            self.full_ordered_striped_audit_v1(memory, &prepared, ready_tail_queue_mask)?;
        if PROFILE {
            diagnostics.final_audit_ns = profile_elapsed_ns_v1::<PROFILE>(final_audit_started);
        }

        let closing_currentness_started = profile_start_v1::<PROFILE>();
        memory.check_queue_operational_currentness()?;
        if PROFILE {
            diagnostics.closing_currentness_ns =
                profile_elapsed_ns_v1::<PROFILE>(closing_currentness_started);
        }
        match final_audit {
            StripedFullAuditOutcomeV1::AllReady => Ok(Gfx942SdmaStripedTailWaitAuditV1::AllReady(
                Gfx942SdmaStripedAllReadyAuditV1 { submission },
            )),
            StripedFullAuditOutcomeV1::Pending => Ok(Gfx942SdmaStripedTailWaitAuditV1::Pending),
            StripedFullAuditOutcomeV1::TailOrderingViolation => {
                Ok(Gfx942SdmaStripedTailWaitAuditV1::TailOrderingViolation)
            }
        }
    }

    fn observe_bound_striped_tail_v1(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        tail: BoundStripedTailV1,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        let owners = self.multi_queue_owners_mut_v1()?;
        let owner =
            owners
                .get_mut(usize::from(tail.queue_ordinal))
                .ok_or(Gfx942SdmaErrorV1::Contract(
                    "striped tail owner disappeared",
                ))?;
        let slot = owner.validate_ticket(tail.ticket)?;
        owner.observe_validated_slot_in_current_scope(memory, slot)
    }

    fn full_ordered_striped_audit_v1(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        prepared: &PreparedStripedTailWaitV1<'_>,
        ready_tail_queue_mask: u16,
    ) -> Result<StripedFullAuditOutcomeV1, Gfx942SdmaErrorV1> {
        let (all_entries_ready, pending_queue_mask, final_ready_tail_mask, seen_tail_mask) = self
            .observe_full_ordered_striped_roster_v1(
            memory,
            &prepared.submission.completion.ordered,
            &prepared.tails_by_queue,
        )?;
        if seen_tail_mask != prepared.active_queue_mask {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped final-audit tail roster",
            ));
        }
        match classify_striped_full_audit_v1(
            all_entries_ready,
            pending_queue_mask,
            ready_tail_queue_mask | final_ready_tail_mask,
        ) {
            StripedFullAuditDispositionV1::AllReady => Ok(StripedFullAuditOutcomeV1::AllReady),
            StripedFullAuditDispositionV1::TailOrderingViolation => {
                Ok(StripedFullAuditOutcomeV1::TailOrderingViolation)
            }
            StripedFullAuditDispositionV1::TimedOut => Ok(StripedFullAuditOutcomeV1::Pending),
        }
    }

    fn observe_full_ordered_striped_roster_v1(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        entries: &[ValidatedMultiQueueCompletionEntryV1],
        tails_by_queue: &[Option<BoundStripedTailV1>; GFX942_SDMA_MAX_STRIPED_QUEUES_V1],
    ) -> Result<(bool, u16, u16, u16), Gfx942SdmaErrorV1> {
        let owners = self.multi_queue_owners_mut_v1()?;
        let mut final_ready_tail_mask = 0_u16;
        let mut seen_tail_mask = 0_u16;
        let (all_entries_ready, pending_queue_mask) =
            observe_full_ordered_roster_v1(entries, |entry| {
                let owner = owners.get_mut(usize::from(entry.queue_ordinal)).ok_or(
                    Gfx942SdmaErrorV1::Contract("striped full-audit owner disappeared"),
                )?;
                let slot = owner.validate_ticket(entry.ticket)?;
                let queue_bit = 1_u16.checked_shl(u32::from(entry.queue_ordinal)).ok_or(
                    Gfx942SdmaErrorV1::Contract("striped full-audit queue ordinal"),
                )?;
                let ready = owner.observe_validated_slot_in_current_scope(memory, slot)?;
                let bound_tail = tails_by_queue
                    .get(usize::from(entry.queue_ordinal))
                    .copied()
                    .flatten()
                    .ok_or(Gfx942SdmaErrorV1::Contract(
                        "striped final-audit tail missing",
                    ))?;
                if bound_tail.request_index == entry.request_index {
                    if !bound_tail_matches_ordered_roster_v1(entries, bound_tail) {
                        return Err(Gfx942SdmaErrorV1::Contract(
                            "striped final-audit tail substitution",
                        ));
                    }
                    seen_tail_mask |= queue_bit;
                    if ready {
                        final_ready_tail_mask |= queue_bit;
                    }
                }
                Ok((queue_bit, ready))
            })?;
        Ok((
            all_entries_ready,
            pending_queue_mask,
            final_ready_tail_mask,
            seen_tail_mask,
        ))
    }

    /// Abort-only suffix: every fallible/native/currentness operation and the
    /// exact-submission authorization precedes the ownership move into here.
    fn retire_after_striped_full_audit_no_unwind_v1(
        &mut self,
        permit: Gfx942SdmaStripedRetirementPermitV1,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Gfx942SdmaMultiQueueCompletedV1 {
        abort_if_striped_retirement_unwinds_v1(|| {
            self.retire_after_striped_full_audit_infallible_v1(permit, submission)
        })
    }

    fn retire_after_striped_full_audit_infallible_v1(
        &mut self,
        permit: Gfx942SdmaStripedRetirementPermitV1,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Gfx942SdmaMultiQueueCompletedV1 {
        let owners = self
            .multi_queue_owners_mut_v1()
            .unwrap_or_else(|_| std::process::abort());
        let (plan, _shards, mut completion) = submission.into_parts();
        // Binding authenticated an empty `completed` vector whose capacity is
        // at least the exact ordered length. Nothing can mutate either vector
        // while the private witness borrows their sole owner, so these pushes
        // cannot grow.
        if !completion.completed.is_empty()
            || completion.ordered.len() != permit.ordered_len
            || completion.completed.capacity() < completion.ordered.len()
        {
            std::process::abort();
        }
        for entry in completion.ordered {
            let Some(owner) = owners.get_mut(usize::from(entry.queue_ordinal)) else {
                std::process::abort();
            };
            completion
                .completed
                .push(owner.retire_validated_slot(usize::from(entry.ticket.slot)));
        }
        Gfx942SdmaMultiQueueCompletedV1 {
            plan,
            completed: completion.completed,
        }
    }
}

#[cfg(test)]
#[path = "tail_wait/tests.rs"]
mod tests;
