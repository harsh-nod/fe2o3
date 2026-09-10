//! Explicit serial performance options; every group lifecycle fence stays full.

use super::*;

/// One owned argument record borrowing a retained group kernel token.
pub struct Gfx950EngineeringPeerDispatchV1<'a> {
    pub kernel: &'a Gfx950EngineeringPeerKernelV1,
    pub bytes: Vec<u8>,
    pub workgroup: [u16; 3],
    pub grid: [u32; 3],
    pub pointers: Vec<Gfx950EngineeringPeerPointerV1>,
    pub timeout_ms: u32,
}

trait SerialSequenceBackend {
    type Prepared;
    fn full_fence(&mut self) -> Result<()>;
    fn operational_fence(&mut self) -> Result<()>;
    fn prepare(&mut self, index: usize) -> Result<Self::Prepared>;
    fn execute(&mut self, prepared: Self::Prepared) -> Result<u64>;
}

fn sequence_rank(world: usize, ranks: &[usize]) -> Result<usize> {
    if !matches!(world, 2 | 8) || !(1..=MAX_SEQUENCE_DISPATCHES_V1).contains(&ranks.len()) {
        return Err("peer sequence world or count is outside its bound".into());
    }
    let rank = ranks[0];
    if rank >= world || ranks.iter().any(|&other| other != rank) {
        return Err("peer sequence must belong to one retained rank".into());
    }
    Ok(rank)
}

fn require_sequence_timeout(mut timeouts: impl Iterator<Item = u32>) -> Result<()> {
    timeouts
        .try_fold(0_u32, |sum, timeout| {
            if timeout == 0 {
                return None;
            }
            sum.checked_add(timeout).filter(|sum| *sum <= 600_000)
        })
        .ok_or_else(|| "peer sequence aggregate timeout is outside 1..600000 ms".into())
        .map(|_| ())
}

fn run_sequence(backend: &mut impl SerialSequenceBackend, count: usize) -> Result<Vec<u64>> {
    if !(1..=MAX_SEQUENCE_DISPATCHES_V1).contains(&count) {
        return Err("peer sequence count is outside 1..16".into());
    }
    backend.full_fence()?;
    let prepared = (0..count)
        .map(|index| backend.prepare(index))
        .collect::<Result<Vec<_>>>()?;
    let mut elapsed = Vec::with_capacity(count);
    for command in prepared {
        backend.operational_fence()?;
        elapsed.push(backend.execute(command)?);
        backend.operational_fence()?;
    }
    backend.full_fence()?;
    Ok(elapsed)
}

struct NativeSequence<'group, 'kernel> {
    group: &'group mut Gfx950EngineeringPeerGroupV1,
    rank: usize,
    commands: Vec<Option<Gfx950EngineeringPeerDispatchV1<'kernel>>>,
}

impl SerialSequenceBackend for NativeSequence<'_, '_> {
    type Prepared = (PreparedDispatch, u32);
    fn full_fence(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts)
    }
    fn operational_fence(&mut self) -> Result<()> {
        for context in &mut self.group.contexts {
            context.check_currentness(false)?;
            context.check_idle()?;
        }
        Ok(())
    }
    fn prepare(&mut self, index: usize) -> Result<Self::Prepared> {
        let command = self
            .commands
            .get_mut(index)
            .and_then(Option::take)
            .ok_or("peer sequence command unavailable")?;
        let prepared = self.group.prepare_peer_dispatch(
            command.kernel,
            command.bytes,
            command.workgroup,
            command.grid,
            &command.pointers,
            command.timeout_ms,
        )?;
        Ok((prepared, command.timeout_ms))
    }
    fn execute(&mut self, (prepared, timeout): Self::Prepared) -> Result<u64> {
        // SAFETY: the enclosing unsafe entry authorizes this finite sequence;
        // all bindings were checked before publication, all owners are retained,
        // and preceding completion plus all-participant idle fences were observed.
        unsafe { self.group.contexts[self.rank].execute_prepared_dispatch(prepared, timeout) }
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Configures every participant exactly once, before any user buffer/kernel.
    /// Immutable admission caching does not skip argument, ownership or ABI checks.
    /// Operational checks apply inside bounded sequences and kernel wait loops;
    /// mapping, host access, single-dispatch boundaries and lifecycle checks stay full.
    pub fn configure_performance(
        &mut self,
        cache_kernel_admission: bool,
        operational_currentness: bool,
    ) -> Result<()> {
        self.require_active()?;
        let result = (|| {
            for context in &self.contexts {
                require_fresh_configuration(
                    context.performance.is_some(),
                    context.next_buffer,
                    context.next_kernel,
                    context.ring.write(),
                )?;
            }
            check_contexts(&mut self.contexts)?;
            let options = PerformanceOptions {
                cache_kernel_admission,
                operational_currentness,
                profile: false,
            };
            for context in &mut self.contexts {
                context.configure_performance(options)?;
            }
            check_contexts(&mut self.contexts)
        })();
        self.finish(result)
    }

    /// Prevalidates a bounded same-rank sequence, then runs every kernel serially.
    /// No free, map, load, write or queue rollover can interleave with this borrow.
    /// Full checks on every participant bracket the sequence. Any failure poisons
    /// the entire group, including a partially completed sequence; no partial
    /// result or guessed retirement is returned as a successful completion.
    ///
    /// # Safety
    /// Every unauthenticated kernel must obey its declared buffer access, bounds
    /// and termination contract, as required by `dispatch_unchecked`.
    pub unsafe fn dispatch_sequence_unchecked(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>> {
        self.require_active()?;
        let result = (|| {
            let count = commands.len();
            if !(1..=MAX_SEQUENCE_DISPATCHES_V1).contains(&count) {
                return Err("peer sequence count is outside 1..16".into());
            }
            let rank = sequence_rank(
                self.contexts.len(),
                &commands
                    .iter()
                    .map(|command| command.kernel.rank)
                    .collect::<Vec<_>>(),
            )?;
            require_sequence_timeout(commands.iter().map(|command| command.timeout_ms))?;
            require_sequence_capacity(
                self.contexts[rank].ring.write(),
                self.contexts[rank].last_observed_read,
                count,
            )?;
            let mut native = NativeSequence {
                group: self,
                rank,
                commands: commands.into_iter().map(Some).collect(),
            };
            run_sequence(&mut native, count)
        })();
        self.finish(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_timeout_rejects_zero_overflow_and_excess_aggregate() {
        assert!(require_sequence_timeout([300_000, 300_000].into_iter()).is_ok());
        assert!(require_sequence_timeout([37_500; 16].into_iter()).is_ok());
        for timeouts in [
            vec![0],
            vec![600_001],
            vec![300_001, 300_000],
            vec![u32::MAX, 1],
            vec![600_000; 16],
        ] {
            assert!(require_sequence_timeout(timeouts.into_iter()).is_err());
        }
    }

    #[test]
    fn sequence_roster_rejects_mixed_foreign_or_unbounded_ranks() {
        assert_eq!(sequence_rank(2, &[1, 1]).unwrap(), 1);
        assert_eq!(sequence_rank(8, &[7; 16]).unwrap(), 7);
        for (world, ranks) in [
            (0, vec![0]),
            (1, vec![0]),
            (3, vec![0]),
            (2, vec![]),
            (2, vec![0; 17]),
            (2, vec![2]),
            (8, vec![8]),
            (8, vec![0, 1]),
        ] {
            assert!(sequence_rank(world, &ranks).is_err());
        }
    }

    #[derive(Default)]
    struct Recording {
        events: Vec<String>,
        fail: Option<String>,
        executed: usize,
    }
    impl Recording {
        fn event(&mut self, name: String) -> Result<()> {
            self.events.push(name.clone());
            if self.fail.as_ref() == Some(&name) {
                Err("injected sequence failure".into())
            } else {
                Ok(())
            }
        }
    }
    impl SerialSequenceBackend for Recording {
        type Prepared = usize;
        fn full_fence(&mut self) -> Result<()> {
            self.event(format!("full:{}", self.executed))
        }
        fn operational_fence(&mut self) -> Result<()> {
            self.event(format!("operational:{}", self.executed))
        }
        fn prepare(&mut self, index: usize) -> Result<usize> {
            self.event(format!("prepare:{index}"))?;
            Ok(index)
        }
        fn execute(&mut self, index: usize) -> Result<u64> {
            self.event(format!("execute:{index}"))?;
            self.executed += 1;
            Ok(index as u64)
        }
    }

    #[test]
    fn full_group_fences_bracket_prevalidation_and_every_completion() {
        let mut backend = Recording::default();
        assert_eq!(run_sequence(&mut backend, 2).unwrap(), [0, 1]);
        assert_eq!(
            backend.events,
            [
                "full:0",
                "prepare:0",
                "prepare:1",
                "operational:0",
                "execute:0",
                "operational:1",
                "operational:1",
                "execute:1",
                "operational:2",
                "full:2"
            ]
        );
    }

    #[test]
    fn malformed_later_command_prevents_every_publication() {
        for failure in ["full:0", "prepare:0", "prepare:1", "operational:0"] {
            let mut backend = Recording {
                fail: Some(failure.into()),
                ..Recording::default()
            };
            assert!(run_sequence(&mut backend, 2).is_err());
            assert_eq!(backend.executed, 0);
        }
        for count in [0, 17, usize::MAX] {
            let mut backend = Recording::default();
            assert!(run_sequence(&mut backend, count).is_err());
            assert!(backend.events.is_empty());
        }
    }

    #[test]
    fn completion_or_postcheck_failure_never_publishes_later_commands_or_success() {
        for (failure, completed) in [
            ("execute:0", 0),
            ("operational:1", 1),
            ("execute:1", 1),
            ("full:2", 2),
        ] {
            let mut backend = Recording {
                fail: Some(failure.into()),
                ..Recording::default()
            };
            assert!(run_sequence(&mut backend, 2).is_err());
            assert_eq!(backend.executed, completed);
        }
        let mut backend = Recording::default();
        assert_eq!(run_sequence(&mut backend, 16).unwrap().len(), 16);
    }
}
