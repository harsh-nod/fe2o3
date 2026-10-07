use super::*;
use crate::engineering_gfx950::peer::scoped_currentness::capacity_fence::{
    FenceBackend, RankBackend, fence,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Recording {
    events: Vec<String>,
    fail: Option<usize>,
    unwind: Option<usize>,
    poisoned: [bool; 5],
    published: bool,
    snapshot: [u64; 6],
    census_calls: usize,
    drift: Option<usize>,
    malformed: Option<(usize, u32)>,
    counts: Gfx950EngineeringPeerScopedCurrentnessCountsV1,
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            fail: None,
            unwind: None,
            poisoned: [false; 5],
            published: false,
            snapshot: [3, 10, 9, 20, 12, 30],
            census_calls: 0,
            drift: None,
            malformed: None,
            counts: Gfx950EngineeringPeerScopedCurrentnessCountsV1 {
                full_discoveries: 0,
                local_checkpoints: 0,
                before_calls: 0,
                after_calls: 0,
                generation_probes: 0,
            },
        }
    }
}
impl Recording {
    fn event(&mut self, name: impl Into<String>) -> Result<()> {
        self.events.push(name.into());
        let index = self.events.len() - 1;
        assert_ne!(self.unwind, Some(index), "injected census unwind");
        if self.fail == Some(index) {
            return Err("injected census failure".into());
        }
        Ok(())
    }
    fn checkpoint(&mut self, rank: usize, name: &str) -> Result<()> {
        self.event(format!("rank{rank}.{name}"))?;
        self.counts.local_checkpoints += 1;
        self.counts.before_calls += 1;
        self.counts.after_calls += 1;
        self.counts.generation_probes += 2;
        Ok(())
    }
}
struct RecordingRank<'a> {
    backend: &'a mut Recording,
    rank: usize,
}
impl RankBackend for RecordingRank<'_> {
    fn check(&mut self) -> Result<()> {
        self.backend.checkpoint(self.rank, "check")
    }
    fn idle(&mut self) -> Result<()> {
        self.backend.checkpoint(self.rank, "idle.check")?;
        self.backend.event(format!("rank{}.queue", self.rank))
    }
}
impl FenceBackend for Recording {
    type Rank<'rank>
        = RecordingRank<'rank>
    where
        Self: 'rank;
    fn rank(&mut self, rank: usize) -> Result<Self::Rank<'_>> {
        Ok(RecordingRank {
            backend: self,
            rank,
        })
    }
}
impl ClosedBackend for Recording {
    type Prefix = u8;
    type Pending = u8;
    type Hidden = u8;
    type Counts = Gfx950EngineeringPeerScopedCurrentnessCountsV1;
    type Output = ([u8; 3], Gfx950EngineeringPeerScopedCurrentnessCountsV1);
    fn enter(&mut self) -> Result<()> {
        self.event("entry")?;
        self.counts.full_discoveries += 1;
        self.counts.before_calls += 2;
        self.counts.after_calls += 2;
        self.counts.generation_probes += 1;
        Ok(())
    }
    fn prefix(&mut self) -> Result<u8> {
        self.event("prefix")?;
        Ok(1)
    }
    fn mlp(&mut self) -> Result<u8> {
        self.event("mlp_retired")?;
        Ok(2)
    }
    fn hidden(&mut self) -> Result<u8> {
        self.event("two_hidden_reads_validated")?;
        Ok(3)
    }
    fn exit(&mut self) -> Result<Self::Counts> {
        self.event("full_exit")?;
        self.counts.full_discoveries += 1;
        self.counts.before_calls += 2;
        self.counts.after_calls += 2;
        self.counts.generation_probes += 2;
        Ok(self.counts)
    }
    fn commit(
        &mut self,
        prefix: u8,
        pending: u8,
        hidden: u8,
        counts: Self::Counts,
    ) -> Result<Self::Output> {
        self.event("commit_head_deadline")?;
        self.event("commit_tail_deadline")?;
        self.published = true;
        Ok(([prefix, pending, hidden], counts))
    }
    fn quarantine(&mut self) {
        self.events.push("quarantine".into());
        self.poisoned.fill(true);
    }
}
impl CensusBackend for Recording {
    type Snapshot = [u64; 6];
    fn census(&mut self) -> Result<Sample<Self::Snapshot>> {
        self.census_calls += 1;
        let first = fence(self)?;
        let second = fence(self)?;
        if self.census_calls == 2 {
            if let Some(field) = self.drift {
                self.snapshot[field] += 1;
            }
        }
        let mut observed = first.checked_add(second).unwrap();
        if let Some((call, value)) = self.malformed {
            if call == self.census_calls {
                observed = value;
            }
        }
        Ok(Sample {
            snapshot: self.snapshot,
            owner_counts: [self.snapshot[0], self.snapshot[2]],
            rank_checkpoints: observed,
        })
    }
}
fn wrapped(inner: Recording) -> CensusLayer<Recording> {
    CensusLayer {
        inner,
        expected: [3, 9],
        before: None,
        observation: None,
    }
}
fn expected_trace() -> Vec<String> {
    let boundary = [
        "rank0.check",
        "rank0.idle.check",
        "rank0.queue",
        "rank1.check",
        "rank1.idle.check",
        "rank1.queue",
    ];
    let mut trace = vec!["entry".into()];
    trace.extend(boundary.repeat(2).into_iter().map(str::to_owned));
    trace.extend(["prefix", "mlp_retired", "two_hidden_reads_validated"].map(str::to_owned));
    trace.extend(boundary.repeat(2).into_iter().map(str::to_owned));
    trace.extend(["full_exit", "commit_head_deadline", "commit_tail_deadline"].map(str::to_owned));
    trace
}

#[test]
fn scoped_census_closed_engine_preserves_all_four_rank_ordered_fences() {
    let mut backend = wrapped(Recording::default());
    let ((layer, counts), census) = closed_layer(&mut backend).unwrap();
    assert_eq!(layer, [1, 2, 3]);
    assert_eq!(backend.inner.events, expected_trace());
    assert_eq!(backend.inner.poisoned, [false; 5]);
    assert!(backend.inner.published);
    assert_eq!(census.owner_counts, [3, 9]);
    assert_eq!((census.preflights, census.rank_checkpoints), (2, 16));
    assert_eq!((counts.full_discoveries, counts.local_checkpoints), (2, 16));
    assert_eq!(
        (
            counts.before_calls,
            counts.after_calls,
            counts.generation_probes
        ),
        (20, 20, 35)
    );
}
#[test]
fn scoped_census_every_rank_queue_and_commit_boundary_failure_is_terminal() {
    let trace = expected_trace();
    assert_eq!(trace.iter().filter(|s| s.ends_with("check")).count(), 16);
    assert_eq!(trace.iter().filter(|s| s.ends_with("queue")).count(), 8);
    for index in 0..trace.len() {
        let mut backend = wrapped(Recording {
            fail: Some(index),
            ..Recording::default()
        });
        assert!(closed_layer(&mut backend).is_err(), "boundary {index}");
        assert_eq!(&backend.inner.events[..=index], &trace[..=index]);
        assert_eq!(backend.inner.events.len(), index + 2);
        assert_eq!(backend.inner.events.last().unwrap(), "quarantine");
        assert_eq!(backend.inner.poisoned, [true; 5]);
        assert!(!backend.inner.published);
        assert!(backend.observation.is_none() || index >= trace.len() - 3);
    }
}
#[test]
fn scoped_census_every_rank_queue_and_commit_boundary_unwind_is_terminal() {
    let trace = expected_trace();
    for index in 0..trace.len() {
        let mut backend = wrapped(Recording {
            unwind: Some(index),
            ..Recording::default()
        });
        assert!(catch_unwind(AssertUnwindSafe(|| closed_layer(&mut backend))).is_err());
        assert_eq!(&backend.inner.events[..=index], &trace[..=index]);
        assert_eq!(backend.inner.events.len(), index + 2);
        assert_eq!(backend.inner.poisoned, [true; 5]);
        assert!(!backend.inner.published);
    }
}
#[test]
fn scoped_census_complete_snapshot_drift_refuses_before_exit_even_with_equal_lengths() {
    for field in 0..6 {
        let mut backend = wrapped(Recording {
            drift: Some(field),
            ..Recording::default()
        });
        assert!(
            closed_layer(&mut backend).is_err(),
            "snapshot field {field}"
        );
        assert!(!backend.inner.events.iter().any(|s| s == "full_exit"));
        assert_eq!(backend.inner.poisoned, [true; 5]);
        assert!(!backend.inner.published);
    }
}
#[test]
fn scoped_census_ledger_assertion_is_reject_only_and_owner_order_is_exact() {
    for expected in [[9, 3], [0, 9], [3, 0], [usize::MAX, 9]] {
        let mut backend = wrapped(Recording::default());
        backend.expected = expected;
        assert!(closed_layer(&mut backend).is_err());
        assert_eq!(backend.inner.snapshot, [3, 10, 9, 20, 12, 30]);
        assert!(!backend.inner.events.iter().any(|s| s == "prefix"));
        assert_eq!(backend.inner.poisoned, [true; 5]);
    }
}
#[test]
fn scoped_census_malformed_observed_checkpoint_counts_never_reach_commit() {
    for call in [1, 2] {
        for value in [0, 7, 9, u32::MAX] {
            let mut backend = wrapped(Recording {
                malformed: Some((call, value)),
                ..Recording::default()
            });
            assert!(closed_layer(&mut backend).is_err());
            assert!(!backend.inner.events.iter().any(|s| s == "full_exit"));
            assert_eq!(backend.inner.poisoned, [true; 5]);
            assert!(!backend.inner.published);
        }
    }
}
#[test]
fn scoped_census_old_closed_backend_keeps_its_original_trace_and_zero_subset() {
    let mut old = Recording::default();
    let (_, counts) = closed_layer(&mut old).unwrap();
    assert_eq!(
        old.events,
        [
            "entry",
            "prefix",
            "mlp_retired",
            "two_hidden_reads_validated",
            "full_exit",
            "commit_head_deadline",
            "commit_tail_deadline",
        ]
    );
    assert_eq!(old.census_calls, 0);
    assert_eq!(counts.local_checkpoints, 0);
    assert_eq!(counts.full_discoveries, 2);
}
