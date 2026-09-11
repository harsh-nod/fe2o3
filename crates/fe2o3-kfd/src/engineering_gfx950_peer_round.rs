//! Bounded independent-rank publication; existing serial APIs are unchanged.

use super::*;

trait ConcurrentRoundBackend {
    type Prepared;
    type Pending;
    fn full_fence(&mut self) -> Result<()>;
    fn prepare(&mut self, index: usize) -> Result<Self::Prepared>;
    fn publication_fence(&mut self, pending: &[Self::Pending]) -> Result<()>;
    fn publish(&mut self, prepared: Self::Prepared) -> Result<Self::Pending>;
    fn poll(&mut self, pending: &mut Self::Pending) -> Result<Option<u64>>;
    fn wait_checkpoint(&mut self) -> Result<()>;
}

fn require_round_ranks(world: usize, ranks: &[usize]) -> Result<()> {
    if !matches!(world, 2 | 8) || ranks.is_empty() || ranks.len() > world {
        return Err("peer round world or count is outside its bound".into());
    }
    let mut seen = 0_u8;
    for &rank in ranks {
        if rank >= world || seen & (1 << rank) != 0 {
            return Err("peer round requires distinct retained ranks".into());
        }
        seen |= 1 << rank;
    }
    Ok(())
}

fn require_round_timeout(timeouts: impl Iterator<Item = u32>) -> Result<()> {
    let mut sum = 0_u32;
    for timeout in timeouts {
        sum = sum
            .checked_add(timeout)
            .filter(|&total| timeout != 0 && total <= 600_000)
            .ok_or("peer round aggregate timeout is outside 1..600000 ms")?;
    }
    if sum == 0 {
        return Err("peer round has no timeout".into());
    }
    Ok(())
}

fn require_round_independence(arguments: &[&[Gfx950EngineeringPeerPointerV1]]) -> Result<()> {
    if arguments.is_empty()
        || arguments.len() > 8
        || arguments
            .iter()
            .any(|pointers| pointers.len() > MAX_POINTER_FIXUPS_V1)
    {
        return Err("peer round argument roster is outside its bound".into());
    }
    let mut ranges = Vec::new();
    for (command, pointers) in arguments.iter().enumerate() {
        for pointer in *pointers {
            let end = pointer
                .buffer_offset
                .checked_add(pointer.extent_bytes)
                .filter(|&end| end <= pointer.buffer.bytes)
                .ok_or("peer round argument range is outside its buffer")?;
            if pointer.extent_bytes == 0 {
                continue;
            }
            for &(other_command, group, buffer, start, other_end, access) in &ranges {
                if command != other_command
                    && pointer.buffer.group == group
                    && pointer.buffer.id == buffer
                    && pointer.buffer_offset < other_end
                    && start < end
                    && (pointer.access != BufferAccessV1::Read || access != BufferAccessV1::Read)
                {
                    return Err("peer round has overlapping cross-command read/write ranges".into());
                }
            }
            ranges.push((
                command,
                pointer.buffer.group,
                pointer.buffer.id,
                pointer.buffer_offset,
                end,
                pointer.access,
            ));
        }
    }
    Ok(())
}

fn require_round_deadline(now: Instant, deadline: Instant) -> Result<()> {
    if now >= deadline {
        return Err("peer round dispatch deadline exceeded; process teardown required".into());
    }
    Ok(())
}

fn run_round(backend: &mut impl ConcurrentRoundBackend, count: usize) -> Result<Vec<u64>> {
    if !(1..=8).contains(&count) {
        return Err("peer round count is outside 1..8".into());
    }
    backend.full_fence()?;
    let prepared = (0..count)
        .map(|index| backend.prepare(index))
        .collect::<Result<Vec<_>>>()?;
    let mut submitted = Vec::with_capacity(count);
    for command in prepared {
        backend.publication_fence(&submitted)?;
        submitted.push(backend.publish(command)?);
    }

    // No completion polling or wait occurs until every command was published.
    let mut pending = submitted.into_iter().map(Some).collect::<Vec<_>>();
    let mut elapsed = vec![0; count];
    let mut remaining = count;
    while remaining != 0 {
        for (index, slot) in pending.iter_mut().enumerate() {
            let Some(command) = slot.as_mut() else {
                continue;
            };
            if let Some(duration) = backend.poll(command)? {
                elapsed[index] = duration;
                *slot = None;
                remaining -= 1;
            }
        }
        if remaining != 0 {
            backend.wait_checkpoint()?;
        }
    }
    backend.full_fence()?;
    Ok(elapsed)
}

struct NativeRound<'group, 'kernel> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    commands: Vec<Option<Gfx950EngineeringPeerDispatchV1<'kernel>>>,
    next_currentness: Instant,
}

impl NativeRound<'_, '_> {
    fn operational_fence(&mut self) -> Result<()> {
        for context in &mut self.group.contexts {
            context.check_currentness(false)?;
            // In-flight queues are not idle. Observe faults without inventing
            // completion or advancing their retained hardware-read frontiers.
            if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
                .map_err(explain)?
                != 0
            {
                return Err("peer round participant queue exception".into());
            }
        }
        Ok(())
    }
}

impl ConcurrentRoundBackend for NativeRound<'_, '_> {
    type Prepared = (usize, PreparedDispatch, u32);
    type Pending = (usize, PendingDispatch);

    fn full_fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts)
    }

    fn prepare(&mut self, index: usize) -> Result<Self::Prepared> {
        let command = self
            .commands
            .get_mut(index)
            .and_then(Option::take)
            .ok_or("peer round command unavailable")?;
        let rank = command.kernel.rank;
        let prepared = self.group.prepare_peer_dispatch(
            command.kernel,
            command.bytes,
            command.workgroup,
            command.grid,
            &command.pointers,
            command.timeout_ms,
        )?;
        let context = &self.group.contexts[rank];
        require_sequence_capacity(context.ring.write(), context.last_observed_read, 1)?;
        Ok((rank, prepared, command.timeout_ms))
    }

    fn publication_fence(&mut self, pending: &[Self::Pending]) -> Result<()> {
        self.operational_fence()?;
        let now = Instant::now();
        for (_, dispatch) in pending {
            require_round_deadline(now, dispatch.deadline)?;
        }
        Ok(())
    }

    fn publish(&mut self, (rank, prepared, timeout): Self::Prepared) -> Result<Self::Pending> {
        // SAFETY: the enclosing unsafe round entry supplies the trusted-code
        // contract. All commands/ranges were prevalidated, ranks are unique,
        // and the exclusive group borrow retains every mapping and queue.
        let pending =
            unsafe { self.group.contexts[rank].publish_prepared_dispatch(prepared, timeout) }?;
        Ok((rank, pending))
    }

    fn poll(&mut self, (rank, pending): &mut Self::Pending) -> Result<Option<u64>> {
        require_round_deadline(Instant::now(), pending.deadline)?;
        self.group.contexts[*rank].poll_pending_dispatch(pending)
    }

    fn wait_checkpoint(&mut self) -> Result<()> {
        let now = Instant::now();
        if now >= self.next_currentness {
            self.operational_fence()?;
            self.next_currentness = Instant::now()
                .checked_add(Duration::from_millis(100))
                .ok_or("peer round currentness deadline")?;
        }
        std::thread::sleep(Duration::from_micros(50));
        Ok(())
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Submits independent retained ranks before waiting for any completion.
    ///
    /// A round contains 1..world commands with distinct ranks, where world is
    /// two or eight. All scopes, kernel/argument bindings, queue capacities and
    /// cross-command ranges are checked before the first publication. Read/read
    /// sharing and disjoint ranges are permitted; an overlapping range with any
    /// writer rejects. Timeout sums are bounded to 600000 ms, with each command's
    /// deadline starting at its publication phase, not when it is first polled.
    ///
    /// Full currentness and idle checks bracket the whole round. Every context
    /// receives currentness and exception checks before each publication and
    /// periodically while waiting. Real completion/frontier checks still apply
    /// to each queue. No host access, map, free, load, rollover or unrelated GPU
    /// operation can interleave with this borrow. One context has at most one
    /// outstanding dispatch, retaining its private kernarg and signal storage.
    ///
    /// Timings are submission-to-observed-completion host nanoseconds in input
    /// order, including intervening rank publication/polling; not GPU timestamps.
    /// Success is returned only after every completion and the full exit fence.
    /// Any failure poisons the entire group, even after partial publication or
    /// completion. Uncertain native resources remain owned until process exit;
    /// there is no partial-success result or retry/cleanup on the poisoned group.
    ///
    /// # Safety
    /// The disposable exclusive-process and trusted-machine-code obligations of
    /// `dispatch_unchecked` apply to every command. Kernels must honor their
    /// complete declared access ranges and terminate independently; undeclared
    /// accesses or synchronization with another command are not supported.
    pub unsafe fn dispatch_round_unchecked(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>> {
        self.require_active()?;
        let result = (|| {
            if commands.len() > self.contexts.len() {
                return Err("peer round count exceeds retained ranks".into());
            }
            require_round_ranks(
                self.contexts.len(),
                &commands
                    .iter()
                    .map(|command| command.kernel.rank)
                    .collect::<Vec<_>>(),
            )?;
            require_round_timeout(commands.iter().map(|command| command.timeout_ms))?;
            require_round_independence(
                &commands
                    .iter()
                    .map(|command| command.pointers.as_slice())
                    .collect::<Vec<_>>(),
            )?;
            let count = commands.len();
            let mut native = NativeRound {
                group: self,
                commands: commands.into_iter().map(Some).collect(),
                next_currentness: Instant::now(),
            };
            run_round(&mut native, count)
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_round_tests.rs"]
mod tests;
