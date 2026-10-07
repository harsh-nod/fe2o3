//! Exclusive custody across finite paired segments; caller snapshots grant no rights.
use super::*;

enum Custody {
    Ready,
    Busy,
    Idle {
        generation: u64,
        states: [CombinedMlpSnapshotV1; 2],
    },
    Poisoned,
}

impl Custody {
    fn begin_run(&mut self) -> Result<()> {
        match std::mem::replace(self, Self::Busy) {
            Self::Ready => Ok(()),
            _ => Err("paired session run requires Ready custody".into()),
        }
    }

    fn complete(&mut self, generation: u64, result: &Completion) -> Result<()> {
        if !matches!(self, Self::Busy) || generation == 0 {
            return Err("paired session completion custody/generation mismatch".into());
        }
        for state in &result.states {
            require_terminal(state, generation)?;
        }
        *self = Self::Idle {
            generation,
            states: result.states.clone(),
        };
        Ok(())
    }

    fn begin_rearm(&mut self) -> Result<(u64, [CombinedMlpSnapshotV1; 2])> {
        match std::mem::replace(self, Self::Busy) {
            Self::Idle { generation, states } => Ok((generation, states)),
            _ => Err("paired session rearm requires retained Idle custody".into()),
        }
    }

    fn transfer(&self, write: bool) -> Result<()> {
        if matches!(self, Self::Ready) || (!write && matches!(self, Self::Idle { .. })) {
            Ok(())
        } else {
            Err("paired session transfer requires quiescent custody (writes only Ready)".into())
        }
    }
}

trait RearmBackend {
    fn now(&mut self) -> Instant;
    fn quiescent(
        &mut self,
        old: u64,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<()>;
    fn reset(&mut self, rank: usize, expected: &CombinedMlpSnapshotV1, next: u64) -> Result<()>;
    fn initial(&mut self, next: u64) -> Result<()>;
    fn quarantine(&mut self);
}

fn rearm_pair(
    backend: &mut impl RearmBackend,
    old: u64,
    states: &[CombinedMlpSnapshotV1; 2],
    timeout_ms: u32,
) -> Result<u64> {
    let started = backend.now();
    let result = (|| {
        if !(1..=10_000).contains(&timeout_ms) || old == 0 {
            return Err("paired session invalid rearm timeout/generation".into());
        }
        let next = old
            .checked_add(1)
            .ok_or("paired session generation exhausted")?;
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(timeout_ms)))
            .ok_or("paired session rearm deadline overflow")?;
        // Recheck both actual terminal owners and all ten completion signals
        // before the first store. No caller-provided snapshot enters this path.
        backend.quiescent(old, states, deadline)?;
        deadline_check(backend.now(), deadline)?;
        for rank in 0..2 {
            backend.reset(rank, &states[rank], next)?;
            deadline_check(backend.now(), deadline)?;
        }
        backend.initial(next)?;
        deadline_check(backend.now(), deadline)?;
        Ok(next)
    })();
    if result.is_err() {
        backend.quarantine();
    }
    result
}

impl RearmBackend for Native<'_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn quiescent(
        &mut self,
        old: u64,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<()> {
        if old != self.generation {
            return Err("paired session retained/native generation drift".into());
        }
        // finish checks the actual arenas, queue identities/frontiers/faults,
        // all ten acquired zero signals and both Completed owners under fences.
        CoordinatorBackend::finish(self, states, deadline)?;
        Ok(())
    }

    fn reset(&mut self, rank: usize, expected: &CombinedMlpSnapshotV1, next: u64) -> Result<()> {
        // SAFETY: quiescent validated BOTH peers before either reset. Exclusive
        // group/owner custody prevents new publication; prior R2 peer reads have
        // completed. The owner rechecks its own identity, snapshot and fences.
        unsafe { self.owners[rank].rearm_quiescent(self.group, expected, next) }
    }

    fn initial(&mut self, next: u64) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)?;
        if profiles::validate_owners(self.group, self.owners)? != next {
            return Err("paired session rearmed generation/readback drift".into());
        }
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn quarantine(&mut self) {
        CoordinatorBackend::poison(self);
    }
}

/// Private, non-Clone session. Returned Completions are observations, not rearm authority.
pub(in super::super) struct Session<'group, 'kernel> {
    native: Native<'group, 'kernel>,
    custody: Custody,
}

impl<'group, 'kernel> Session<'group, 'kernel> {
    /// # Safety
    /// All dispatch safety obligations apply for the entire retained session:
    /// dedicated disposable process, exact reviewed images/model-role joins,
    /// coherent completed producers, and valid role-correct payloads before
    /// every run (including subsequent writes). Errors are terminal, never retried.
    pub(in super::super) unsafe fn new(
        group: &'group mut Gfx950EngineeringPeerGroupV1,
        owners: &'group mut [CombinedMlpStateV1; 2],
        inputs: Inputs<'kernel>,
    ) -> Self {
        Self {
            native: Native {
                group,
                owners,
                inputs,
                output_policy: profiles::OutputPolicy::Strict,
                timeout_ms: 0,
                generation: 0,
                staged: None,
                reusable: None,
                validated_terminal: None,
            },
            custody: Custody::Ready,
        }
    }

    fn outcome<T>(&mut self, result: Result<T>) -> Result<T> {
        if result.is_err() {
            self.custody = Custody::Poisoned;
            CoordinatorBackend::poison(&mut self.native);
        }
        result
    }

    pub(in super::super) fn run(&mut self, timeout_ms: u32) -> Result<Completion> {
        let result = (|| {
            self.custody.begin_run()?;
            self.native.timeout_ms = timeout_ms;
            let completion = coordinate(&mut self.native, timeout_ms)?;
            self.custody.complete(self.native.generation, &completion)?;
            Ok(completion)
        })();
        self.outcome(result)
    }

    pub(in super::super) fn rearm_next(&mut self, timeout_ms: u32) -> Result<u64> {
        let result = (|| {
            let (old, states) = self.custody.begin_rearm()?;
            let next = rearm_pair(&mut self.native, old, &states, timeout_ms)?;
            // Drop only host staging metadata. Group custody retains every old
            // signal/kernarg arena until queue-first Close. Never reset signals,
            // free early, or invent hardware read credit for the next batch.
            self.native.staged = None;
            self.native.validated_terminal = None;
            self.native.generation = next;
            self.custody = Custody::Ready;
            Ok(next)
        })();
        self.outcome(result)
    }

    fn bound_payload(&self, buffer: Gfx950EngineeringPeerBufferV1) -> Result<()> {
        let inputs = &self.native.inputs;
        if !inputs.partials.contains(&buffer)
            && !inputs.ranks.iter().any(|rank| {
                rank.mlp_roots.contains(&buffer)
                    || rank.residual_input == buffer
                    || rank.output == buffer
            })
        {
            return Err("paired session transfer outside retained payload roles".into());
        }
        require_public_vram(self.native.group.validate_token(buffer)?)
    }

    pub(in super::super) fn read(
        &mut self,
        buffer: Gfx950EngineeringPeerBufferV1,
        offset: u64,
        bytes: u32,
    ) -> Result<Vec<u8>> {
        let result = (|| {
            self.custody.transfer(false)?;
            let idle = std::mem::replace(&mut self.custody, Custody::Busy);
            self.bound_payload(buffer)?;
            let result = self.native.group.read(buffer, offset, bytes)?;
            self.custody = idle;
            Ok(result)
        })();
        self.outcome(result)
    }

    pub(in super::super) fn write(
        &mut self,
        buffer: Gfx950EngineeringPeerBufferV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<()> {
        let result = (|| {
            self.custody.transfer(true)?;
            self.custody = Custody::Busy;
            self.bound_payload(buffer)?;
            self.native.group.write(buffer, offset, bytes)?;
            self.custody = Custody::Ready;
            Ok(())
        })();
        self.outcome(result)
    }
}

impl Drop for Session<'_, '_> {
    fn drop(&mut self) {
        if matches!(self.custody, Custody::Busy | Custody::Poisoned) {
            CoordinatorBackend::poison(&mut self.native);
        }
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_session_v1_tests.rs"]
mod tests;
