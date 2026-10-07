//! Private opt-in terminal transaction. No retained currentness proof escapes.
use super::*;

trait TerminalBackend {
    fn now(&mut self) -> Instant;
    fn generation(&self) -> u64;
    fn custody(&mut self) -> Result<()>;
    fn full_fence(&mut self) -> Result<()>;
    fn queues(&mut self) -> Result<()>;
    fn observe(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1>;
    fn signals(&mut self, deadline: Instant) -> Result<[(u64, u64); 2]>;
    fn commit(&mut self) -> Result<()>;
    fn quarantine(&mut self);
}

struct Transaction<'a, B: TerminalBackend> {
    backend: &'a mut B,
    committed: bool,
}

impl<B: TerminalBackend> Drop for Transaction<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}

impl<B: TerminalBackend> Transaction<'_, B> {
    fn step<T>(
        &mut self,
        deadline: Instant,
        operation: impl FnOnce(&mut B) -> Result<T>,
    ) -> Result<T> {
        deadline_check(self.backend.now(), deadline)?;
        let result = operation(self.backend)?;
        deadline_check(self.backend.now(), deadline)?;
        Ok(result)
    }
}

fn run(
    backend: &mut impl TerminalBackend,
    deadline: Instant,
) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
    let mut transaction = Transaction {
        backend,
        committed: false,
    };
    transaction.step(deadline, TerminalBackend::custody)?;
    transaction.step(deadline, TerminalBackend::full_fence)?;
    let generation = transaction.backend.generation();
    let states = [
        transaction.step(deadline, |b| b.observe(0))?,
        transaction.step(deadline, |b| b.observe(1))?,
    ];
    for state in &states {
        require_terminal(state, generation)?;
    }
    // The six replaced fences retain their queue/fault probes at the same
    // boundaries. No allocation, mapping, publication, callback or wait occurs.
    transaction.step(deadline, TerminalBackend::queues)?;
    for (rank, expected) in states.iter().enumerate() {
        transaction.step(deadline, TerminalBackend::queues)?;
        let observed = transaction.step(deadline, |b| b.observe(rank))?;
        require_terminal(&observed, generation)?;
        if observed != *expected {
            return Err("paired terminal snapshot changed before completion".into());
        }
        transaction.step(deadline, TerminalBackend::queues)?;
    }
    transaction.step(deadline, TerminalBackend::queues)?;
    let frontiers = transaction.step(deadline, |b| b.signals(deadline))?;
    for (rank, expected) in states.iter().enumerate() {
        let observed = transaction.step(deadline, |b| b.observe(rank))?;
        require_terminal(&observed, generation)?;
        if observed != *expected {
            return Err("paired terminal final generation/state drift".into());
        }
    }
    transaction.step(deadline, TerminalBackend::full_fence)?;
    transaction.step(deadline, TerminalBackend::commit)?;
    transaction.committed = true;
    Ok((states, frontiers))
}

struct NativeTerminal<'a, 'group, 'kernel>(&'a mut Native<'group, 'kernel>);

impl NativeTerminal<'_, '_, '_> {
    fn custody(&self) -> Result<()> {
        let native = &self.0;
        native.group.require_active()?;
        native
            .staged
            .as_ref()
            .ok_or("paired terminal staging absent")?
            .require_retired()?;
        if native.group.contexts.len() != 2 || native.generation == 0 {
            return Err("paired terminal rank count/generation".into());
        }
        for (rank, owner) in native.owners.iter().enumerate() {
            require_activation(owner.activation, Activation::Submitted)?;
            if owner.generation != native.generation || owner.owner_rank() != rank {
                return Err("paired terminal owner generation/rank drift".into());
            }
        }
        Ok(())
    }
}

impl TerminalBackend for NativeTerminal<'_, '_, '_> {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn generation(&self) -> u64 {
        self.0.generation
    }
    fn custody(&mut self) -> Result<()> {
        NativeTerminal::custody(self)
    }
    fn full_fence(&mut self) -> Result<()> {
        self.custody()?;
        check_contexts(
            &mut self.0.group.contexts,
            self.0.group.shared_full_currentness,
        )
    }
    fn queues(&mut self) -> Result<()> {
        self.custody()?;
        // Deliberately method-local queue checks inside this exclusive retired
        // transaction. Context::check_idle_after_currentness keeps its stricter
        // immediate-fence contract; this does not make it a reusable proof API.
        for context in &mut self.0.group.contexts {
            if context.ordered_batch_poisoned {
                return Err("ordered batch context is terminally poisoned".into());
            }
            let (write, read) =
                Backend::observe_aql_counters(&mut context.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            require_completed_frontier(context.completed_write, context.ring.write())?;
            validate_counters(
                context.ring.write(),
                context.last_observed_read,
                (write, read),
            )?;
            context.last_observed_read = read;
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("queue exception payload is nonzero".into());
            }
        }
        Ok(())
    }
    fn observe(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1> {
        self.custody()?;
        self.0.owners[rank].observe(self.0.group)
    }
    fn signals(&mut self, deadline: Instant) -> Result<[(u64, u64); 2]> {
        self.custody()?;
        let staged = self
            .0
            .staged
            .as_mut()
            .ok_or("paired terminal staging absent")?;
        if !staged.complete(self.0.group, deadline)? {
            return Err("paired guarded MLP completion changed".into());
        }
        Ok(staged
            .last
            .ok_or("paired guarded MLP final observation missing")?
            .1)
    }
    fn commit(&mut self) -> Result<()> {
        self.custody()?;
        // Both full fences, all three snapshots/rank and the ten real signals
        // precede either transition. There is no external access between stores.
        for owner in &mut *self.0.owners {
            owner.activation = Activation::Completed;
        }
        Ok(())
    }
    fn quarantine(&mut self) {
        CoordinatorBackend::poison(self.0);
    }
}

pub(super) fn complete(
    native: &mut Native<'_, '_>,
    deadline: Instant,
) -> Result<([CombinedMlpSnapshotV1; 2], [(u64, u64); 2])> {
    run(&mut NativeTerminal(native), deadline)
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_combined_mlp_paired_terminal_v1_tests.rs"]
mod tests;
