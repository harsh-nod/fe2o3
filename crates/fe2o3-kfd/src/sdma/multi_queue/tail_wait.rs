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

use super::{
    Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaQueueSetV1,
    ValidatedMultiQueueCompletionEntryV1,
};
use crate::sdma::{
    GFX942_SDMA_MAX_STRIPED_QUEUES_V1, Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1,
    SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1, SDMA_OP_FENCE,
};
use crate::shared_memory::SharedGttMemorySessionV1;
use crate::wait::MonotonicWaitV1;

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
        let Gfx942SdmaQueueSetV1::Striped { owners, .. } = queue_set else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped tail binding requires striped SDMA queues",
            ));
        };

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
    fn pause(&mut self);
}

impl TailWaitCursorV1 for MonotonicWaitV1 {
    fn deadline_reached(&self) -> bool {
        MonotonicWaitV1::expired(self)
    }

    fn pause(&mut self) {
        MonotonicWaitV1::pause(self);
    }
}

fn observe_tail_rounds_until_v1<T, E, W: TailWaitCursorV1>(
    tails: &[T],
    mut tail_queue_bit: impl FnMut(&T) -> u16,
    mut observe_tail: impl FnMut(&T) -> Result<bool, E>,
    wait: &mut W,
) -> Result<u16, E> {
    loop {
        let mut active_queue_mask = 0_u16;
        let mut ready_queue_mask = 0_u16;
        for tail in tails {
            let queue_bit = tail_queue_bit(tail);
            active_queue_mask |= queue_bit;
            if observe_tail(tail)? {
                ready_queue_mask |= queue_bit;
            }
        }
        if ready_queue_mask == active_queue_mask {
            return Ok(ready_queue_mask);
        }
        if wait.deadline_reached() {
            return Ok(ready_queue_mask);
        }
        wait.pause();
    }
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
    pub(crate) fn wait_prepared_striped_multi_queue_tails_retaining_for(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        retained: &mut Option<Gfx942SdmaMultiQueueSubmissionV1>,
        timeout: Duration,
    ) -> Gfx942SdmaStripedTailWaitOutcomeV1 {
        let audit = with_borrowed_striped_submission_v1(retained, |submission| {
            self.audit_prepared_striped_multi_queue_tails_for(memory, submission, timeout)
        });
        match audit {
            Ok(Gfx942SdmaStripedTailWaitAuditV1::AllReady(audit)) => {
                let permit = audit.authorize_retirement();
                let submission = retained.take().unwrap_or_else(|| std::process::abort());
                Gfx942SdmaStripedTailWaitOutcomeV1::Completed(
                    self.retire_after_striped_full_audit_no_unwind_v1(permit, submission),
                )
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

    fn audit_prepared_striped_multi_queue_tails_for<'a>(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        submission: &'a Gfx942SdmaMultiQueueSubmissionV1,
        timeout: Duration,
    ) -> Result<Gfx942SdmaStripedTailWaitAuditV1<'a>, Gfx942SdmaErrorV1> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Gfx942SdmaErrorV1::Contract("striped tail wait deadline"))?;
        let prepared = PreparedStripedTailWaitV1::bind(self, submission)?;
        memory.check_queue_operational_currentness()?;
        let mut wait = MonotonicWaitV1::until(deadline);
        let active_queues = &prepared.active_queues[..usize::from(prepared.tail_count)];
        let ready_tail_queue_mask = observe_tail_rounds_until_v1(
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
        );
        let ready_tail_queue_mask = ready_tail_queue_mask?;
        let final_audit =
            self.full_ordered_striped_audit_v1(memory, &prepared, ready_tail_queue_mask)?;
        memory.check_queue_operational_currentness()?;
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
        let Self::Striped { owners, .. } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped tail observation requires striped SDMA queues",
            ));
        };
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
        let Self::Striped { owners, .. } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped full audit requires striped SDMA queues",
            ));
        };
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
        let Self::Striped { owners, .. } = self else {
            std::process::abort();
        };
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
mod tests {
    use super::*;
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        QueueKeyV1, VmIdV1, VmKeyV1,
    };

    struct InjectedTailWaitCursorV1 {
        expire_after_pauses: usize,
        pauses: usize,
    }

    impl TailWaitCursorV1 for InjectedTailWaitCursorV1 {
        fn deadline_reached(&self) -> bool {
            self.pauses >= self.expire_after_pauses
        }

        fn pause(&mut self) {
            self.pauses += 1;
        }
    }

    fn queue_key(queue: u64) -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(7),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(3),
            },
            id: QueueInstanceIdV1(queue),
            generation: QueueGenerationV1(5),
        }
    }

    #[test]
    fn exact_fence_encoding_carries_system_scope_and_snoop() {
        let packet = crate::sdma::Gfx942SdmaCopySubmissionV1::new(1, 2, 3, 4, 5).unwrap();
        assert!(gfx942_system_snoop_tail_fence_is_exact_v1());
        assert_eq!(SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1, 0x0053_0005);
        assert_eq!(packet.fence_header(), SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1);
        assert!(tail_signal_generation_is_exact_v1(7, 7));
        assert!(!tail_signal_generation_is_exact_v1(7, 8));
        assert!(!tail_signal_generation_is_exact_v1(0, 0));
    }

    #[test]
    fn bound_tail_rejects_every_ordered_roster_identity_substitution() {
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner: queue_key(11),
            queue_id: 17,
            slot: 9,
            generation: 23,
        };
        let ordered = [ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 2,
            ticket,
        }];
        let tail = BoundStripedTailV1 {
            queue_ordinal: 2,
            request_index: 0,
            ticket,
        };
        assert!(bound_tail_matches_ordered_roster_v1(&ordered, tail));

        let mutations: [fn(&mut BoundStripedTailV1); 6] = [
            |tail| tail.request_index = 1,
            |tail| tail.queue_ordinal = 3,
            |tail| tail.ticket.owner = queue_key(12),
            |tail| tail.ticket.queue_id = 18,
            |tail| tail.ticket.slot = 10,
            |tail| tail.ticket.generation = 24,
        ];
        for mutate in mutations {
            let mut substituted = tail;
            mutate(&mut substituted);
            assert!(!bound_tail_matches_ordered_roster_v1(&ordered, substituted));
        }
    }

    #[test]
    fn injected_tail_rounds_visit_every_tail_until_ready() {
        let tails = [10_u8, 11, 12, 13];
        let mut observations = Vec::new();
        let mut wait = InjectedTailWaitCursorV1 {
            expire_after_pauses: usize::MAX,
            pauses: 0,
        };
        let ready_mask = observe_tail_rounds_until_v1(
            &tails,
            |tail| 1_u16 << *tail,
            |tail| {
                let round = observations.len() / tails.len();
                observations.push((round, *tail));
                Ok::<bool, ()>(round == 2)
            },
            &mut wait,
        )
        .unwrap();
        assert_eq!(ready_mask, 0b1111 << 10);
        assert_eq!(wait.pauses, 2);
        assert_eq!(observations.len(), 3 * tails.len());
        for observed_round in observations.chunks_exact(tails.len()) {
            assert_eq!(
                observed_round
                    .iter()
                    .map(|(_, tail)| *tail)
                    .collect::<Vec<_>>(),
                tails
            );
        }
    }

    #[test]
    fn injected_deadline_occurs_only_after_one_complete_pending_round() {
        let tails = [0_u8, 1, 2];
        let mut observations = Vec::new();
        let mut wait = InjectedTailWaitCursorV1 {
            expire_after_pauses: 0,
            pauses: 0,
        };
        let ready_mask = observe_tail_rounds_until_v1(
            &tails,
            |tail| 1_u16 << *tail,
            |tail| {
                observations.push(*tail);
                Ok::<bool, ()>(false)
            },
            &mut wait,
        )
        .unwrap();
        assert_eq!(ready_mask, 0);
        assert_eq!(observations, tails);
    }

    #[test]
    fn injected_tail_error_stops_without_a_full_roster_observation() {
        let tails = [0_u8, 1, 2, 3];
        let mut observations = Vec::new();
        let mut wait = InjectedTailWaitCursorV1 {
            expire_after_pauses: usize::MAX,
            pauses: 0,
        };
        let result = observe_tail_rounds_until_v1(
            &tails,
            |tail| 1_u16 << *tail,
            |tail| {
                observations.push(*tail);
                if *tail == 2 {
                    Err("tail error")
                } else {
                    Ok(false)
                }
            },
            &mut wait,
        );
        assert_eq!(result, Err("tail error"));
        assert_eq!(observations, [0, 1, 2]);
    }

    #[test]
    fn final_audit_distinguishes_completion_timeout_and_tail_contract_violation() {
        assert_eq!(
            classify_striped_full_audit_v1(true, 0, 0b1111),
            StripedFullAuditDispositionV1::AllReady
        );
        assert_eq!(
            classify_striped_full_audit_v1(true, 0, 0),
            StripedFullAuditDispositionV1::AllReady
        );
        assert_eq!(
            classify_striped_full_audit_v1(false, 0b1010, 0b0101),
            StripedFullAuditDispositionV1::TimedOut
        );
        assert_eq!(
            classify_striped_full_audit_v1(false, 0b0010, 0b1010),
            StripedFullAuditDispositionV1::TailOrderingViolation
        );
        // A tail that transitions to ready during the mandatory final scan is
        // still paired with pending prefixes from that same scan.
        let last_tail_round = 0_u16;
        let final_audit_ready_tails = 0b0010_u16;
        assert_eq!(
            classify_striped_full_audit_v1(
                false,
                0b0010,
                last_tail_round | final_audit_ready_tails,
            ),
            StripedFullAuditDispositionV1::TailOrderingViolation
        );
    }

    #[test]
    fn injected_final_audit_observes_every_entry_once_before_timeout() {
        let entries = [0_u16, 1, 2, 3, 4, 5, 6];
        for pending in [0_u16, 3, 6] {
            let mut observed = Vec::new();
            let (ready, pending_queue_mask) = observe_full_ordered_roster_v1(&entries, |entry| {
                observed.push(*entry);
                Ok::<(u16, bool), ()>((1, *entry != pending))
            })
            .unwrap();
            assert!(!ready);
            assert_eq!(observed, entries);
            assert_eq!(
                classify_striped_full_audit_v1(ready, pending_queue_mask, 0),
                StripedFullAuditDispositionV1::TimedOut
            );
        }
    }

    #[test]
    fn injected_all_ready_epoch_counts_tail_rounds_one_audit_and_separate_moves() {
        let tails = [0_u8, 1, 2, 3];
        let entries = [0_u16, 1, 2, 3, 4, 5, 6, 7, 8];
        let mut tail_observations = 0_usize;
        let mut wait = InjectedTailWaitCursorV1 {
            expire_after_pauses: usize::MAX,
            pauses: 0,
        };
        let ready_tail_queue_mask = observe_tail_rounds_until_v1(
            &tails,
            |tail| 1_u16 << *tail,
            |_| {
                let round = tail_observations / tails.len();
                tail_observations += 1;
                Ok::<bool, ()>(round == 2)
            },
            &mut wait,
        )
        .unwrap();
        let mut final_observations = 0_usize;
        let (all_ready, pending_mask) = observe_full_ordered_roster_v1(&entries, |_| {
            final_observations += 1;
            Ok::<(u16, bool), ()>((1, true))
        })
        .unwrap();
        assert_eq!(
            classify_striped_full_audit_v1(all_ready, pending_mask, ready_tail_queue_mask,),
            StripedFullAuditDispositionV1::AllReady
        );
        let retirement_moves = entries.len();
        assert_eq!(tail_observations, 3 * tails.len());
        assert_eq!(final_observations, entries.len());
        assert_eq!(retirement_moves, entries.len());
        assert_eq!(
            tail_observations + final_observations,
            3 * tails.len() + entries.len()
        );
    }

    #[test]
    fn injected_identity_substitution_fails_at_exact_tail_or_roster_entry() {
        let expected_tails = [(0_u16, 7_u32, 4_u16, 11_u32), (1, 8, 9, 12)];
        let mut observed_tails = expected_tails;
        observed_tails[1].3 += 1;
        let mut visited = Vec::new();
        let mut wait = InjectedTailWaitCursorV1 {
            expire_after_pauses: usize::MAX,
            pauses: 0,
        };
        let result = observe_tail_rounds_until_v1(
            &observed_tails,
            |tail| 1_u16 << tail.0,
            |tail| {
                let index = visited.len();
                visited.push(*tail);
                if expected_tails.get(index) == Some(tail) {
                    Ok(true)
                } else {
                    Err("tail generation substitution")
                }
            },
            &mut wait,
        );
        assert_eq!(result, Err("tail generation substitution"));
        assert_eq!(visited, observed_tails);

        let expected_roster = [(0_u16, 0_u16, 4_u16, 11_u32), (1, 1, 9, 12), (2, 0, 5, 13)];
        let mut observed_roster = expected_roster;
        observed_roster[2].1 = 1;
        let mut visited = Vec::new();
        let result = observe_full_ordered_roster_v1(&observed_roster, |entry| {
            let index = visited.len();
            visited.push(*entry);
            if expected_roster.get(index) == Some(entry) {
                Ok((1, true))
            } else {
                Err("ordered roster substitution")
            }
        });
        assert_eq!(result, Err("ordered roster substitution"));
        assert_eq!(visited, observed_roster);
    }

    #[test]
    fn injected_fallible_currentness_step_retains_exact_outer_custody() {
        let retained = Some(super::super::striped_submission_for_unwind_test());
        let exact = retained.as_ref().unwrap().exact_identity_for_unwind_test();
        let result = with_borrowed_striped_submission_v1(&retained, |submission| {
            assert_eq!(submission.exact_identity_for_unwind_test(), exact);
            Err::<(), _>(Gfx942SdmaErrorV1::Contract(
                "injected closing currentness failure",
            ))
        });
        assert!(matches!(
            result,
            Err(Gfx942SdmaErrorV1::Contract(
                "injected closing currentness failure"
            ))
        ));
        assert_eq!(
            retained.as_ref().unwrap().exact_identity_for_unwind_test(),
            exact
        );
    }

    #[test]
    fn every_bound_queue_requires_the_exact_modulo_two_engine() {
        for queue_ordinal in 0..GFX942_SDMA_MAX_STRIPED_QUEUES_V1 {
            let expected = (queue_ordinal % 2) as u32;
            assert!(striped_owner_engine_matches_v1(
                queue_ordinal,
                Some(expected)
            ));
            assert!(!striped_owner_engine_matches_v1(queue_ordinal, None));
            assert!(!striped_owner_engine_matches_v1(
                queue_ordinal,
                Some(expected ^ 1)
            ));
            assert!(!striped_owner_engine_matches_v1(queue_ordinal, Some(2)));
        }
    }

    #[test]
    fn optimized_source_has_no_post_bind_allocation_or_second_full_scan() {
        let source = include_str!("tail_wait.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        assert!(!source.contains("Vec::"));
        assert!(!source.contains("collect::<"));
        assert!(!source.contains("observe_entire_completion_roster("));
        assert!(!source.contains("retire_prepared_striped_multi_queue_completion"));
        assert_eq!(source.matches("for entry in entries").count(), 1);
        assert!(source.contains("Gfx942SdmaStripedAllReadyAuditV1<'a>"));
        assert!(source.contains("submission: &'a Gfx942SdmaMultiQueueSubmissionV1"));
        assert!(!source.contains("pub(crate) struct Gfx942SdmaStripedAllReadyAuditV1"));
        assert!(!source.contains("pub(crate) struct Gfx942SdmaStripedRetirementPermitV1"));
        assert!(!source.contains("*const Gfx942SdmaMultiQueueSubmissionV1"));
        assert!(!source.contains("core::ptr"));
        let retirement = source
            .split("fn retire_after_striped_full_audit_infallible_v1")
            .nth(1)
            .unwrap();
        assert!(retirement.contains("permit: Gfx942SdmaStripedRetirementPermitV1"));
        assert!(
            retirement.find("completed.capacity()").unwrap()
                < retirement.find("for entry in").unwrap()
        );
        assert!(!retirement.contains("observe_validated_slot_in_current_scope"));
        assert!(!retirement.contains("validate_ticket"));
        assert!(!retirement.contains("validated_slot_remains_present"));
        let no_unwind = source
            .split("fn retire_after_striped_full_audit_no_unwind_v1")
            .nth(1)
            .unwrap()
            .split("fn retire_after_striped_full_audit_infallible_v1")
            .next()
            .unwrap();
        assert!(no_unwind.contains("abort_if_striped_retirement_unwinds_v1"));
    }

    #[test]
    fn retained_record_header_is_derived_from_each_emitted_ordinary_packet() {
        let source = include_str!("../../sdma.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        assert_eq!(
            source
                .matches("fence_header: packet.fence_header()")
                .count(),
            1
        );
        assert_eq!(
            source
                .matches("fence_header: copy.packet.fence_header()")
                .count(),
            1
        );
        assert_eq!(
            source
                .matches("fence_header: item.packet.fence_header()")
                .count(),
            1
        );
    }
}
