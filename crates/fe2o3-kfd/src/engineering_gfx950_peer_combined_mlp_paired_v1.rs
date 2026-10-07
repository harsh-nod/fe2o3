//! Private R1 -> MLP -> validator -> paired barrier -> R2 coordinator.
use super::*;

#[path = "engineering_gfx950_peer_combined_mlp_paired_arena_v1.rs"]
mod arena;
#[path = "engineering_gfx950_peer_combined_mlp_paired_profiles_v1.rs"]
mod profiles;
#[path = "engineering_gfx950_peer_combined_mlp_paired_session_v1.rs"]
mod session;
pub(super) use session::Session;

#[path = "engineering_gfx950_peer_combined_mlp_paired_terminal_v1.rs"]
mod terminal_pair;

#[derive(Clone, Copy, Eq, PartialEq)]
enum TerminalPolicy {
    Legacy,
    PairedFences,
}

#[path = "engineering_gfx950_peer_combined_mlp_paired_retained_v1.rs"]
mod retained;
pub(super) use retained::{RetainedPair, UnboundPair, rearm_pairs};

#[path = "engineering_gfx950_peer_combined_mlp_paired_facade_v1.rs"]
mod facade;
pub use retained::scoped_layer::{
    Gfx950EngineeringPeerScopedCurrentnessCountsV1, Gfx950EngineeringPeerScopedPrefixInputsV1,
    Gfx950EngineeringPeerScopedWarmLayerObservationV1,
};

pub use facade::{
    Gfx950EngineeringPeerGuardedMlpBankEntryV1, Gfx950EngineeringPeerGuardedMlpInputsV1,
    Gfx950EngineeringPeerGuardedMlpObservationV1, Gfx950EngineeringPeerGuardedMlpRankInputsV1,
    Gfx950EngineeringPeerRetainedGuardedMlpPairV1, Gfx950EngineeringPeerUnboundGuardedMlpPairV1,
};

pub(super) struct RankInputs<'a> {
    pub(super) kernels: [&'a Gfx950EngineeringPeerKernelV1; 4],
    pub(super) mlp_roots: [Gfx950EngineeringPeerBufferV1; 10],
    pub(super) residual_input: Gfx950EngineeringPeerBufferV1,
    pub(super) output: Gfx950EngineeringPeerBufferV1,
}

pub(super) struct Inputs<'a> {
    pub(super) ranks: [RankInputs<'a>; 2],
    pub(super) partials: [Gfx950EngineeringPeerBufferV1; 2],
    pub(super) projection_sha256: [u8; 32],
    pub(super) mlp_sha256: [u8; 32],
}

/// Observations only; this non-Clone result does not authorize rearm or reuse.
#[derive(Debug)]
pub(super) struct Completion {
    pub(super) states: [CombinedMlpSnapshotV1; 2],
    pub(super) observed_queue_frontiers: [(u64, u64); 2],
    pub(super) segment_host_ns: u64,
}

fn deadline_check(now: Instant, deadline: Instant) -> Result<()> {
    if now >= deadline {
        return Err("paired guarded MLP aggregate deadline expired".into());
    }
    Ok(())
}

trait CoordinatorBackend {
    fn now(&mut self) -> Instant;
    fn preflight(&mut self) -> Result<()>;
    fn consume(&mut self, rank: usize) -> Result<()>;
    fn reserve(&mut self, rank: usize) -> Result<()>;
    fn fence(&mut self) -> Result<()>;
    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()>;
    fn poll(&mut self, deadline: Instant) -> Result<bool>;
    fn retire(&mut self, rank: usize, deadline: Instant) -> Result<()>;
    fn terminal(&mut self) -> Result<()>;
    fn complete(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1>;
    fn finish(
        &mut self,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<[(u64, u64); 2]>;
    fn complete_terminal_pair_until(
        &mut self,
        deadline: Instant,
    ) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
        legacy_terminal_until(self, deadline)
    }
    fn pause(&mut self);
    fn poison(&mut self);
}

fn legacy_terminal_until(
    backend: &mut (impl CoordinatorBackend + ?Sized),
    deadline: Instant,
) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
    backend.terminal()?;
    deadline_check(backend.now(), deadline)?;
    let first = backend.complete(0)?;
    deadline_check(backend.now(), deadline)?;
    let second = backend.complete(1)?;
    let states = [first, second];
    deadline_check(backend.now(), deadline)?;
    let frontiers = backend.finish(&states, deadline)?;
    Ok((states, frontiers))
}

fn coordinate(backend: &mut impl CoordinatorBackend, timeout_ms: u32) -> Result<Completion> {
    coordinate_until(backend, timeout_ms, None)
}

fn coordinate_until(
    backend: &mut impl CoordinatorBackend,
    timeout_ms: u32,
    outer_deadline: Option<Instant>,
) -> Result<Completion> {
    coordinate_with_terminal_policy(backend, timeout_ms, outer_deadline, TerminalPolicy::Legacy)
}

fn coordinate_with_terminal_policy(
    backend: &mut impl CoordinatorBackend,
    timeout_ms: u32,
    outer_deadline: Option<Instant>,
    terminal_policy: TerminalPolicy,
) -> Result<Completion> {
    let started = backend.now();
    let result = (|| {
        if !(1..=10_000).contains(&timeout_ms) {
            return Err("paired guarded MLP timeout outside 1..10000 ms".into());
        }
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(timeout_ms)))
            .ok_or("paired guarded MLP deadline overflow")?;
        let deadline = outer_deadline.map_or(deadline, |outer| outer.min(deadline));
        if outer_deadline.is_some() {
            deadline_check(backend.now(), deadline)?;
        }
        backend.preflight()?;
        deadline_check(backend.now(), deadline)?;
        // submit() checks idle queues, so both owners are consumed before either
        // software ring reservation. Every kernel and arena is already prepared.
        for rank in 0..2 {
            backend.consume(rank)?;
            deadline_check(backend.now(), deadline)?;
        }
        for rank in 0..2 {
            backend.reserve(rank)?;
            deadline_check(backend.now(), deadline)?;
        }
        for rank in 0..2 {
            backend.fence()?;
            deadline_check(backend.now(), deadline)?;
            backend.publish(rank, deadline)?;
            deadline_check(backend.now(), deadline)?;
        }
        // Both complete batches are exposed before the first completion poll.
        loop {
            deadline_check(backend.now(), deadline)?;
            backend.fence()?;
            deadline_check(backend.now(), deadline)?;
            if backend.poll(deadline)? {
                break;
            }
            backend.pause();
        }
        for rank in 0..2 {
            deadline_check(backend.now(), deadline)?;
            backend.retire(rank, deadline)?;
        }
        deadline_check(backend.now(), deadline)?;
        let (states, observed_queue_frontiers) = match terminal_policy {
            TerminalPolicy::Legacy => legacy_terminal_until(backend, deadline)?,
            TerminalPolicy::PairedFences => backend.complete_terminal_pair_until(deadline)?,
        };
        let finished = backend.now();
        deadline_check(finished, deadline)?;
        Ok(Completion {
            states,
            observed_queue_frontiers,
            segment_host_ns: u64::try_from(finished.saturating_duration_since(started).as_nanos())
                .map_err(explain)?,
        })
    })();
    if result.is_err() {
        backend.poison();
    }
    result
}

struct Native<'group, 'kernel> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    owners: &'group mut [CombinedMlpStateV1; 2],
    inputs: Inputs<'kernel>,
    output_policy: profiles::OutputPolicy,
    timeout_ms: u32,
    generation: u64,
    staged: Option<arena::Staged>,
    reusable: Option<(arena::Reusable, Instant)>,
    validated_terminal: Option<[CombinedMlpSnapshotV1; 2]>,
    currentness: scoped_currentness::Currentness<'group>,
}

impl CoordinatorBackend for Native<'_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn preflight(&mut self) -> Result<()> {
        self.group.require_active()?;
        profiles::validate_policy(self.group)?;
        self.currentness.idle_group(self.group)?;
        self.generation = profiles::validate_owners(self.group, self.owners)?;
        let prepared = profiles::prepare_with_currentness(
            self.group,
            self.owners,
            &self.inputs,
            self.timeout_ms,
            self.output_policy,
            &mut self.currentness,
        )?;
        for context in &self.group.contexts {
            require_sequence_capacity(
                context.ring.write(),
                context.last_observed_read,
                arena::PACKETS,
            )?;
        }
        self.staged = Some(match self.reusable.take() {
            Some((proof, until)) => proof.prepare_currentness(
                self.group,
                prepared,
                self.generation,
                until,
                &mut self.currentness,
            )?,
            None => match &self.currentness {
                scoped_currentness::Currentness::Full => {
                    arena::Staged::prepare(self.group, prepared)?
                }
                scoped_currentness::Currentness::Scoped(_) => {
                    return Err("scoped warm layer cannot allocate a fresh arena".into());
                }
            },
        });
        self.currentness.idle_group(self.group)?;
        Ok(())
    }

    fn consume(&mut self, rank: usize) -> Result<()> {
        self.owners[rank].submit_currentness(self.group, &mut self.currentness)
    }

    fn reserve(&mut self, rank: usize) -> Result<()> {
        self.staged
            .as_mut()
            .ok_or("paired guarded MLP staging missing")?
            .reserve(self.group, rank)
    }

    fn fence(&mut self) -> Result<()> {
        self.staged
            .as_ref()
            .ok_or("paired guarded MLP staging missing")?
            .check(self.group)?;
        self.currentness.publication(self.group)
    }

    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.staged
            .as_mut()
            .ok_or("paired guarded MLP staging missing")?
            .publish_currentness(self.group, rank, deadline, &mut self.currentness)
    }

    fn poll(&mut self, deadline: Instant) -> Result<bool> {
        self.staged
            .as_mut()
            .ok_or("paired guarded MLP staging missing")?
            .complete(self.group, deadline)
    }

    fn retire(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.staged
            .as_mut()
            .ok_or("paired guarded MLP staging missing")?
            .retire(self.group, rank, deadline)
    }

    fn terminal(&mut self) -> Result<()> {
        let staged = self
            .staged
            .as_ref()
            .ok_or("paired guarded MLP staging missing")?;
        staged.require_retired()?;
        self.currentness.idle_group(self.group)?;
        for owner in &*self.owners {
            require_activation(owner.activation, Activation::Submitted)?;
            if owner.generation != self.generation {
                return Err("paired guarded MLP terminal generation drift".into());
            }
        }
        let states = [
            self.owners[0].observe(self.group)?,
            self.owners[1].observe(self.group)?,
        ];
        for state in &states {
            require_terminal(state, self.generation)?;
        }
        self.currentness.idle_group(self.group)?;
        self.validated_terminal = Some(states);
        Ok(())
    }

    fn complete(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1> {
        let staged = self
            .staged
            .as_ref()
            .ok_or("paired guarded MLP staging missing")?;
        staged.require_retired()?;
        let expected = &self
            .validated_terminal
            .as_ref()
            .ok_or("paired guarded MLP terminal pair not validated")?[rank];
        // SAFETY: both batches were published and every actual completion signal
        // acquired as zero, with current queue/fault checks. Both terminal guards
        // were validated under the selected private route before completion.
        // A Scoped caller retains all owners until its mandatory full exit. The
        // exclusive group borrow prevents intervening publication. Both logical
        // work frontiers retired without invented ring credit; arenas are retained.
        let observed = unsafe {
            self.owners[rank].complete_quiescent_currentness(self.group, &mut self.currentness)?
        };
        if &observed != expected {
            return Err("paired guarded MLP terminal snapshot changed".into());
        }
        Ok(observed)
    }

    fn finish(
        &mut self,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<[(u64, u64); 2]> {
        let staged = self
            .staged
            .as_mut()
            .ok_or("paired guarded MLP staging missing")?;
        staged.require_retired()?;
        self.currentness.idle_group(self.group)?;
        if !staged.complete(self.group, deadline)? {
            return Err("paired guarded MLP completion changed".into());
        }
        for rank in 0..2 {
            require_activation(self.owners[rank].activation, Activation::Completed)?;
            if self.owners[rank].generation != self.generation
                || self.owners[rank].observe(self.group)? != states[rank]
            {
                return Err("paired guarded MLP final generation/state drift".into());
            }
            require_terminal(&states[rank], self.generation)?;
        }
        self.currentness.idle_group(self.group)?;
        Ok(staged
            .last
            .ok_or("paired guarded MLP final observation missing")?
            .1)
    }

    fn pause(&mut self) {
        std::thread::sleep(Duration::from_micros(50));
    }

    fn complete_terminal_pair_until(
        &mut self,
        deadline: Instant,
    ) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
        if matches!(
            &self.currentness,
            scoped_currentness::Currentness::Scoped(_)
        ) {
            return Err("scoped layer preserves legacy terminal cadence".into());
        }
        terminal_pair::complete(self, deadline)
    }

    fn poison(&mut self) {
        for owner in &mut *self.owners {
            owner.poison(self.group);
        }
        for context in &mut self.group.contexts {
            context.ordered_batch_poisoned = true;
        }
        if let Some(staged) = &mut self.staged {
            staged.poisoned = true;
        }
    }
}

/// One paired segment with fresh private arenas, retained until queue-first Close.
/// No worker entry or generic SharedAtomic admission is changed by this module.
///
/// # Safety
/// Dedicated disposable process, reviewed exact R1/MLP images and model-role
/// joins, completed/validated attention producers, and established system-scope
/// payload coherence are required. Equal-size weight roles are not authenticated
/// by geometry. Every error is terminal; no retry, rollback or uncertain rearm.
pub(super) unsafe fn dispatch(
    group: &mut Gfx950EngineeringPeerGroupV1,
    owners: &mut [CombinedMlpStateV1; 2],
    inputs: Inputs<'_>,
    timeout_ms: u32,
) -> Result<Completion> {
    coordinate(
        &mut Native {
            group,
            owners,
            inputs,
            output_policy: profiles::OutputPolicy::Strict,
            timeout_ms,
            generation: 0,
            staged: None,
            reusable: None,
            validated_terminal: None,
            currentness: scoped_currentness::Currentness::Full,
        },
        timeout_ms,
    )
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_v1_tests.rs"]
mod tests;
