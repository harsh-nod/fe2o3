use super::*;

#[test]
fn round_roster_admits_unique_partial_and_reordered_ranks_only() {
    for (world, ranks) in [
        (2, vec![0]),
        (2, vec![1, 0]),
        (8, vec![7]),
        (8, vec![7, 2, 0]),
        (8, (0..8).collect()),
    ] {
        require_round_ranks(world, &ranks).unwrap();
    }
    for (world, ranks) in [
        (0, vec![0]),
        (1, vec![0]),
        (3, vec![0]),
        (2, vec![]),
        (2, vec![0, 0]),
        (2, vec![0, 1, 2]),
        (2, vec![2]),
        (8, vec![8]),
        (8, vec![usize::MAX]),
        (8, vec![0; 9]),
    ] {
        assert!(require_round_ranks(world, &ranks).is_err());
    }
}

#[test]
fn round_timeouts_reject_zero_overflow_and_aggregate_excess() {
    for timeouts in [vec![1], vec![300_000; 2], vec![75_000; 8]] {
        require_round_timeout(timeouts.into_iter()).unwrap();
    }
    for timeouts in [
        vec![],
        vec![0],
        vec![600_001],
        vec![300_001; 2],
        vec![600_000, 1],
        vec![u32::MAX, 1],
        vec![75_000; 9],
    ] {
        assert!(require_round_timeout(timeouts.into_iter()).is_err());
    }
    let now = Instant::now();
    require_round_deadline(now, now + Duration::from_nanos(1)).unwrap();
    assert!(require_round_deadline(now, now).is_err());
    assert!(require_round_deadline(now + Duration::from_nanos(1), now).is_err());
}

fn buffer(id: u64) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id,
        owner: 0,
        bytes: 64,
    }
}

#[test]
fn cross_rank_aliases_reject_each_writer_and_allow_only_independent_ranges() {
    let source = buffer(1);
    for (first, second) in [
        (BufferAccessV1::Read, BufferAccessV1::Write),
        (BufferAccessV1::Write, BufferAccessV1::Read),
        (BufferAccessV1::Write, BufferAccessV1::Write),
        (BufferAccessV1::ReadWrite, BufferAccessV1::Read),
        (BufferAccessV1::Read, BufferAccessV1::ReadWrite),
    ] {
        assert!(
            require_round_independence(&[
                &[source.pointer(0, 0, 32, first)],
                &[source.pointer(0, 16, 32, second)],
            ])
            .is_err()
        );
    }
    for arguments in [
        vec![
            vec![source.pointer(0, 0, 64, BufferAccessV1::Read)],
            vec![source.pointer(0, 0, 64, BufferAccessV1::Read)],
        ],
        vec![
            vec![source.pointer(0, 0, 32, BufferAccessV1::Write)],
            vec![source.pointer(0, 32, 32, BufferAccessV1::Read)],
        ],
        vec![
            vec![source.pointer(0, 0, 64, BufferAccessV1::Write)],
            vec![source.pointer(0, 64, 0, BufferAccessV1::Write)],
        ],
        vec![
            vec![source.pointer(0, 0, 64, BufferAccessV1::Write)],
            vec![buffer(2).pointer(0, 0, 64, BufferAccessV1::Read)],
        ],
    ] {
        require_round_independence(&arguments.iter().map(Vec::as_slice).collect::<Vec<_>>())
            .unwrap();
    }
}

#[test]
fn round_alias_checker_bounds_empty_oversized_and_overflow_ranges() {
    assert!(require_round_independence(&[]).is_err());
    let empty: &[Gfx950EngineeringPeerPointerV1] = &[];
    assert!(require_round_independence(&[empty; 9]).is_err());
    let too_many =
        vec![buffer(1).pointer(0, 0, 0, BufferAccessV1::Read); MAX_POINTER_FIXUPS_V1 + 1];
    assert!(require_round_independence(&[&too_many]).is_err());
    for (offset, extent) in [(u64::MAX, 1), (65, 0), (63, 2), (0, u64::MAX)] {
        assert!(
            require_round_independence(&[&[buffer(1).pointer(
                0,
                offset,
                extent,
                BufferAccessV1::Read
            )]])
            .is_err()
        );
    }
}

struct Recording {
    events: Vec<String>,
    fail: Option<String>,
    published: Vec<usize>,
    completed: Vec<usize>,
    finish_after: Vec<usize>,
    fences: usize,
    waits: usize,
}

impl Recording {
    fn new(count: usize) -> Self {
        Self {
            events: Vec::new(),
            fail: None,
            published: Vec::new(),
            completed: Vec::new(),
            finish_after: vec![0; count],
            fences: 0,
            waits: 0,
        }
    }
    fn event(&mut self, event: String) -> Result<()> {
        self.events.push(event.clone());
        if self.fail.as_ref() == Some(&event) {
            Err(format!("injected {event}"))
        } else {
            Ok(())
        }
    }
}

impl ConcurrentRoundBackend for Recording {
    type Prepared = usize;
    type Pending = (usize, usize);
    fn full_fence(&mut self) -> Result<()> {
        let index = self.fences;
        self.fences += 1;
        self.event(format!("full:{index}"))
    }
    fn prepare(&mut self, index: usize) -> Result<usize> {
        self.event(format!("prepare:{index}"))?;
        Ok(index)
    }
    fn publication_fence(&mut self, pending: &[Self::Pending]) -> Result<()> {
        self.event(format!("fence:{}", pending.len()))
    }
    fn publish(&mut self, index: usize) -> Result<Self::Pending> {
        // The failed attempt is uncertain: publication may already have happened.
        self.published.push(index);
        self.event(format!("publish:{index}"))?;
        Ok((index, 0))
    }
    fn poll(&mut self, (index, polls): &mut Self::Pending) -> Result<Option<u64>> {
        self.event(format!("poll:{index}"))?;
        if *polls == self.finish_after[*index] {
            self.completed.push(*index);
            Ok(Some(100 + *index as u64))
        } else {
            *polls += 1;
            Ok(None)
        }
    }
    fn wait_checkpoint(&mut self) -> Result<()> {
        let index = self.waits;
        self.waits += 1;
        self.event(format!("wait:{index}"))
    }
}

#[test]
fn round_publishes_all_before_polling_and_preserves_input_order() {
    for count in [1, 2, 7, 8] {
        let mut backend = Recording::new(count);
        backend.finish_after = (0..count).rev().collect();
        let elapsed = run_round(&mut backend, count).unwrap();
        assert_eq!(
            elapsed,
            (0..count)
                .map(|index| 100 + index as u64)
                .collect::<Vec<_>>()
        );
        assert_eq!(backend.completed, (0..count).rev().collect::<Vec<_>>());
        let first_publish = backend
            .events
            .iter()
            .position(|event| event.starts_with("publish:"))
            .unwrap();
        let last_prepare = backend
            .events
            .iter()
            .rposition(|event| event.starts_with("prepare:"))
            .unwrap();
        let first_poll = backend
            .events
            .iter()
            .position(|event| event.starts_with("poll:"))
            .unwrap();
        let last_publish = backend
            .events
            .iter()
            .rposition(|event| event.starts_with("publish:"))
            .unwrap();
        assert!(last_prepare < first_publish && last_publish < first_poll);
        assert_eq!(backend.events.first().unwrap(), "full:0");
        assert_eq!(backend.events.last().unwrap(), "full:1");
        assert_eq!(backend.fences, 2);
    }
}

fn empty_group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
    }
}

#[test]
fn every_round_fault_is_terminal_without_partial_success_or_more_publication() {
    let mut faults = vec![
        "full:0".to_owned(),
        "full:1".to_owned(),
        "wait:0".to_owned(),
    ];
    for stage in ["prepare", "fence", "publish", "poll"] {
        faults.extend((0..8).map(|index| format!("{stage}:{index}")));
    }
    for failure in faults {
        let mut backend = Recording::new(8);
        backend.fail = Some(failure.clone());
        if failure == "wait:0" {
            backend.finish_after = vec![1; 8];
        }
        let result = run_round(&mut backend, 8);
        assert!(result.is_err(), "{failure}");
        assert_eq!(backend.events.last().unwrap(), &failure);
        if failure.starts_with("prepare:") || failure == "full:0" {
            assert!(backend.published.is_empty(), "{failure}");
        }
        if let Some(index) = failure.strip_prefix("publish:") {
            assert_eq!(backend.published.len(), index.parse::<usize>().unwrap() + 1);
            assert!(backend.completed.is_empty());
        }
        if let Some(index) = failure.strip_prefix("poll:") {
            assert_eq!(backend.completed.len(), index.parse::<usize>().unwrap());
        }
        let mut group = empty_group();
        assert!(group.finish(result).is_err());
        assert!(group.poisoned && !group.closed);
        assert!(group.require_active().is_err());
        assert!(group.close().is_err());
    }
}

#[test]
fn round_rejects_invalid_count_before_any_backend_operation() {
    for count in [0, 9, usize::MAX] {
        let mut backend = Recording::new(0);
        assert!(run_round(&mut backend, count).is_err());
        assert!(backend.events.is_empty());
    }
}

#[test]
fn round_entry_rejection_quarantines_without_native_resources() {
    let mut group = empty_group();
    // SAFETY: this empty request rejects before any hardware operation, and the
    // fixture contains no native context, mapping, kernel, or allocation.
    assert!(unsafe { group.dispatch_round_unchecked(Vec::new()) }.is_err());
    assert!(group.poisoned);
    assert!(group.require_active().is_err());
}

#[test]
fn publication_sharing_never_enables_operational_mode_or_changes_profile_accounting() {
    assert!(can_share_full_publication_observation(
        [None; 2].into_iter()
    ));
    let full = PerformanceOptions {
        cache_kernel_admission: false,
        operational_currentness: false,
        profile: false,
    };
    for count in [2, 8] {
        for cache_kernel_admission in [false, true] {
            let full = PerformanceOptions {
                cache_kernel_admission,
                ..full
            };
            assert!(can_share_full_publication_observation(
                vec![Some(full); count].into_iter()
            ));
            for rank in 0..count {
                for (operational_currentness, profile) in
                    [(true, false), (false, true), (true, true)]
                {
                    let mut mixed = vec![Some(full); count];
                    mixed[rank] = Some(PerformanceOptions {
                        operational_currentness,
                        profile,
                        ..full
                    });
                    assert!(!can_share_full_publication_observation(mixed.into_iter()));
                }
            }
        }
    }
}

struct PublicationRecording {
    round: Recording,
    participants: usize,
    share: bool,
    checkpoint: usize,
    fresh_observations: usize,
}

impl PublicationRecording {
    fn new(participants: usize, commands: usize, share: bool) -> Self {
        Self {
            round: Recording::new(commands),
            participants,
            share,
            checkpoint: 0,
            fresh_observations: 0,
        }
    }
}

impl PublicationCurrentnessBackend for PublicationRecording {
    fn participants(&self) -> usize {
        self.participants
    }
    fn share_full_observation(&self) -> bool {
        self.share
    }
    fn fresh_group_currentness(&mut self) -> Result<()> {
        self.fresh_observations += 1;
        self.round.event(format!("fresh:{}", self.checkpoint))
    }
    fn individual_currentness(&mut self, rank: usize) -> Result<()> {
        self.round
            .event(format!("individual:{}:{rank}", self.checkpoint))
    }
    fn queue_exception(&mut self, rank: usize) -> Result<()> {
        self.round
            .event(format!("exception:{}:{rank}", self.checkpoint))
    }
}

impl ConcurrentRoundBackend for PublicationRecording {
    type Prepared = usize;
    type Pending = (usize, usize);
    fn full_fence(&mut self) -> Result<()> {
        self.round.full_fence()
    }
    fn prepare(&mut self, index: usize) -> Result<usize> {
        self.round.prepare(index)
    }
    fn publication_fence(&mut self, pending: &[Self::Pending]) -> Result<()> {
        self.checkpoint = pending.len();
        self.round.event(format!("fence:{}", self.checkpoint))?;
        run_publication_currentness(self)
    }
    fn publish(&mut self, index: usize) -> Result<Self::Pending> {
        self.round.publish(index)
    }
    fn poll(&mut self, pending: &mut Self::Pending) -> Result<Option<u64>> {
        self.round.poll(pending)
    }
    fn wait_checkpoint(&mut self) -> Result<()> {
        self.round.wait_checkpoint()
    }
}

#[test]
fn every_publication_observes_a_fresh_full_group_before_all_queue_exceptions() {
    for (participants, commands) in [(2, 1), (2, 2), (8, 1), (8, 8)] {
        let mut backend = PublicationRecording::new(participants, commands, true);
        assert_eq!(
            run_round(&mut backend, commands).unwrap(),
            (0..commands).map(|i| 100 + i as u64).collect::<Vec<_>>()
        );
        assert_eq!(backend.fresh_observations, commands);
        for checkpoint in 0..commands {
            let start = backend
                .round
                .events
                .iter()
                .position(|e| e == &format!("fence:{checkpoint}"))
                .unwrap();
            let mut expected = vec![format!("fence:{checkpoint}"), format!("fresh:{checkpoint}")];
            expected.extend((0..participants).map(|rank| format!("exception:{checkpoint}:{rank}")));
            expected.push(format!("publish:{checkpoint}"));
            assert_eq!(
                &backend.round.events[start..start + expected.len()],
                expected.as_slice()
            );
        }
        assert!(
            !backend
                .round
                .events
                .iter()
                .any(|e| e.starts_with("individual:"))
        );
        assert_eq!(backend.round.fences, 2);
    }
}

#[test]
fn explicit_operational_or_profiled_path_keeps_individual_exception_order() {
    for participants in [2, 8] {
        let mut backend = PublicationRecording::new(participants, 2, false);
        run_round(&mut backend, 2).unwrap();
        assert_eq!(backend.fresh_observations, 0);
        for checkpoint in 0..2 {
            let start = backend
                .round
                .events
                .iter()
                .position(|e| e == &format!("fence:{checkpoint}"))
                .unwrap();
            let mut expected = vec![format!("fence:{checkpoint}")];
            for rank in 0..participants {
                expected.push(format!("individual:{checkpoint}:{rank}"));
                expected.push(format!("exception:{checkpoint}:{rank}"));
            }
            expected.push(format!("publish:{checkpoint}"));
            assert_eq!(
                &backend.round.events[start..start + expected.len()],
                expected.as_slice()
            );
        }
    }
}

#[test]
fn host_observation_timers_do_not_change_full_publication_routing_or_trace() {
    for count in [2, 8] {
        for cache in [false, true] {
            let options = PerformanceOptions {
                cache_kernel_admission: cache,
                operational_currentness: false,
                profile: false,
            };
            let mut traces = Vec::new();
            for observation in [false, true] {
                assert_eq!(
                    host_observation::timers_enabled(Some(options), observation),
                    observation
                );
                let share =
                    can_share_full_publication_observation(vec![Some(options); count].into_iter());
                assert!(share);
                let mut backend = PublicationRecording::new(count, 2, share);
                assert_eq!(run_round(&mut backend, 2).unwrap(), [100, 101]);
                traces.push(backend.round.events);
            }
            assert_eq!(traces[0], traces[1]);
        }
    }
}

#[test]
fn host_observation_keeps_publication_failure_cutoff_before_successful_result() {
    let options = PerformanceOptions {
        cache_kernel_admission: true,
        operational_currentness: false,
        profile: false,
    };
    for failure in ["fresh:0", "exception:0:1", "fresh:1", "exception:1:0"] {
        let mut traces = Vec::new();
        for observation in [false, true] {
            assert_eq!(
                host_observation::timers_enabled(Some(options), observation),
                observation
            );
            let share = can_share_full_publication_observation([Some(options); 2].into_iter());
            let mut backend = PublicationRecording::new(2, 2, share);
            backend.round.fail = Some(failure.into());
            assert!(run_round(&mut backend, 2).is_err());
            assert_eq!(
                backend.round.events.last().map(String::as_str),
                Some(failure)
            );
            traces.push(backend.round.events);
        }
        assert_eq!(traces[0], traces[1]);
    }
}

#[test]
fn every_publication_observation_fault_stops_and_poisons_even_after_prior_publish() {
    for participants in [2, 8] {
        for share in [false, true] {
            let mut good = PublicationRecording::new(participants, 2, share);
            run_round(&mut good, 2).unwrap();
            let failures = good
                .round
                .events
                .iter()
                .filter(|e| {
                    e.starts_with("fresh:")
                        || e.starts_with("individual:")
                        || e.starts_with("exception:")
                })
                .cloned()
                .collect::<Vec<_>>();
            for failure in failures {
                let checkpoint = failure.split(':').nth(1).unwrap().parse::<usize>().unwrap();
                let mut backend = PublicationRecording::new(participants, 2, share);
                backend.round.fail = Some(failure.clone());
                let result = run_round(&mut backend, 2);
                assert!(result.is_err(), "{failure}");
                assert_eq!(backend.round.events.last(), Some(&failure));
                assert_eq!(backend.round.published.len(), checkpoint);
                assert!(backend.round.completed.is_empty());
                let mut group = empty_group();
                assert!(group.finish(result).is_err());
                assert!(group.poisoned);
                assert!(group.require_active().is_err());
                assert!(group.close().is_err());
            }
        }
    }
}

#[test]
fn publication_sharing_does_not_move_wait_or_completion_checkpoints() {
    let mut backend = PublicationRecording::new(2, 2, true);
    backend.round.finish_after = vec![2, 1];
    assert_eq!(run_round(&mut backend, 2).unwrap(), [100, 101]);
    assert_eq!(backend.fresh_observations, 2);
    assert_eq!(backend.round.waits, 2);
    assert_eq!(backend.round.completed, [1, 0]);
    assert_eq!(backend.round.events.last().unwrap(), "full:1");
}

#[test]
fn publication_currentness_refuses_invalid_rosters_before_observation() {
    for count in [0, 1, 3, 7, 9] {
        for share in [false, true] {
            let mut backend = PublicationRecording::new(count, 0, share);
            assert!(run_publication_currentness(&mut backend).is_err());
            assert!(backend.round.events.is_empty());
            assert_eq!(backend.fresh_observations, 0);
        }
    }
}
