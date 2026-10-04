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
