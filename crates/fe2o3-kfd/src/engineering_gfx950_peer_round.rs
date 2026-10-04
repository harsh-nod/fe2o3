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

trait PublicationCurrentnessBackend {
    fn participants(&self) -> usize;
    fn share_full_observation(&self) -> bool;
    fn fresh_group_currentness(&mut self) -> Result<()>;
    fn individual_currentness(&mut self, rank: usize) -> Result<()>;
    fn queue_exception(&mut self, rank: usize) -> Result<()>;
}

fn can_share_full_publication_observation(
    mut options: impl Iterator<Item = Option<PerformanceOptions>>,
) -> bool {
    options.all(|value| value.is_none_or(|value| !value.operational_currentness && !value.profile))
}

fn run_publication_currentness(backend: &mut impl PublicationCurrentnessBackend) -> Result<()> {
    let count = backend.participants();
    if !matches!(count, 2 | 8) {
        return Err("peer publication currentness participant count".into());
    }
    if backend.share_full_observation() {
        // The group helper brackets one fresh discovery with every device's
        // full mutable checks. No snapshot or authorization escapes this call.
        backend.fresh_group_currentness()?;
        for rank in 0..count {
            backend.queue_exception(rank)?;
        }
    } else {
        // Keep explicitly configured operational checks and profile counters
        // on their original per-context path.
        for rank in 0..count {
            backend.individual_currentness(rank)?;
            backend.queue_exception(rank)?;
        }
    }
    Ok(())
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
    capture: bool,
    observations: [Option<Gfx950EngineeringRawTimestampObservationV1>; 8],
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

impl PublicationCurrentnessBackend for NativeRound<'_, '_> {
    fn participants(&self) -> usize {
        self.group.contexts.len()
    }

    fn share_full_observation(&self) -> bool {
        can_share_full_publication_observation(
            self.group
                .contexts
                .iter()
                .map(|context| context.performance),
        )
    }

    fn fresh_group_currentness(&mut self) -> Result<()> {
        host_observation::shared_currentness(
            &mut self.group.contexts,
            host_observation::SharedScope::Publication,
        )
    }

    fn individual_currentness(&mut self, rank: usize) -> Result<()> {
        self.group.contexts[rank].check_currentness(false)
    }

    fn queue_exception(&mut self, rank: usize) -> Result<()> {
        let context = &mut self.group.contexts[rank];
        if Backend::observe_i64_acquire(&mut context.internal[CONTROL].mapping, PAGE_BYTES, 256)
            .map_err(explain)?
            != 0
        {
            return Err("peer round participant queue exception".into());
        }
        Ok(())
    }
}

impl ConcurrentRoundBackend for NativeRound<'_, '_> {
    type Prepared = (usize, PreparedDispatch, u32);
    type Pending = (usize, PendingDispatch);

    fn full_fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
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
        run_publication_currentness(self)?;
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
        let pending = unsafe {
            self.group.contexts[rank].publish_prepared_dispatch_with_raw_timestamps(
                prepared,
                timeout,
                self.capture,
            )
        }?;
        Ok((rank, pending))
    }

    fn poll(&mut self, (rank, pending): &mut Self::Pending) -> Result<Option<u64>> {
        require_round_deadline(Instant::now(), pending.deadline)?;
        let elapsed = self.group.contexts[*rank].poll_pending_dispatch(pending)?;
        if let Some(host_ns) = elapsed {
            if self.capture {
                if self.observations[*rank].is_some() {
                    return Err("duplicate raw timestamp rank completion".into());
                }
                let context = &self.group.contexts[*rank];
                self.observations[*rank] = Some(raw_timestamps::completed_observation(
                    self.group.incarnation,
                    *rank,
                    [
                        context.unique_id,
                        context.queue_epoch,
                        context.completed_write,
                    ],
                    pending,
                    host_ns,
                )?);
            }
        }
        Ok(elapsed)
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
    /// periodically while waiting. Unprofiled full publication checks share one
    /// fresh topology discovery, with every participant's before/after checks
    /// and a final generation recheck; no discovery is reused at another fence.
    /// Operational/profiled paths and periodic wait checks stay unchanged.
    /// Real completion/frontier checks still apply
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
        self.dispatch_round_with_timestamp_mode(commands, false)
            .map(|value| value.0)
    }

    /// Same independent-rank sequencing and terminal-failure contract as
    /// `dispatch_round_unchecked`, with one bounded raw tick record per command
    /// in input order. Requires `open_raw_timestamps_unchecked`. No conversion
    /// to nanoseconds or common clock is performed. Equal nonzero ticks are
    /// retained; zero/unwritten or reversed intervals reject the entire round.
    /// All records remain private until every completion and the exit fence.
    ///
    /// # Safety
    /// The trusted-machine-code and exclusive disposable-process obligations of
    /// `dispatch_round_unchecked` apply unchanged. Caller must obtain healthy
    /// close before publishing an external successful-run receipt.
    pub unsafe fn dispatch_round_with_raw_timestamps_unchecked(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<Gfx950EngineeringRawTimestampObservationV1>> {
        self.dispatch_round_with_timestamp_mode(commands, true)
            .map(|value| value.1)
    }

    fn dispatch_round_with_timestamp_mode(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
        capture: bool,
    ) -> Result<(Vec<u64>, Vec<Gfx950EngineeringRawTimestampObservationV1>)> {
        self.require_active()?;
        let result = (|| {
            for context in &self.contexts {
                raw_timestamps::require_capture_mode(context.raw_timestamps_enabled, capture)?;
            }
            if commands.len() > self.contexts.len() {
                return Err("peer round count exceeds retained ranks".into());
            }
            let ranks = commands
                .iter()
                .map(|command| command.kernel.rank)
                .collect::<Vec<_>>();
            require_round_ranks(self.contexts.len(), &ranks)?;
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
                capture,
                observations: [None; 8],
            };
            let elapsed = run_round(&mut native, count)?;
            let observations =
                raw_timestamps::finish_round(capture, &ranks, &elapsed, native.observations)?;
            Ok((elapsed, observations))
        })();
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_round_tests.rs"]
mod tests;
