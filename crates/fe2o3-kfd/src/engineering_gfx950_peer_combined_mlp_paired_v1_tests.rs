use super::*;
use fe2o3_aql::{AqlPeerBarrierAndPacketV1, AqlPeerPacketBatchPublicationTargetV1};

fn terminal_state() -> CombinedMlpSnapshotV1 {
    let mut prefix = [0; PREFIX_WORDS];
    prefix[0] = 1;
    prefix[3] = 31;
    prefix[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[14..22].fill(u32::MAX);
    prefix[22] = 3;
    prefix[23..31].fill(u32::MAX);
    prefix[31] = 3;
    prefix[32..].fill(64);
    CombinedMlpSnapshotV1 {
        prefix,
        guard: guard_words(1, 1),
    }
}

struct Fake {
    clock: Instant,
    events: Vec<String>,
    operations: usize,
    fail_at: usize,
    after_effect: bool,
    expire_at: usize,
    activation: [Activation; 2],
    reserved: [bool; 2],
    published: [bool; 2],
    retired: [bool; 2],
    terminal_validated: bool,
    terminal_states: [CombinedMlpSnapshotV1; 2],
    polls: usize,
    poisoned: [bool; 4],
    poison_calls: usize,
}

impl Fake {
    fn new() -> Self {
        Self {
            clock: Instant::now(),
            events: vec![],
            operations: 0,
            fail_at: 0,
            after_effect: false,
            expire_at: 0,
            activation: [Activation::Ready; 2],
            reserved: [false; 2],
            published: [false; 2],
            retired: [false; 2],
            terminal_validated: false,
            terminal_states: [terminal_state(), terminal_state()],
            polls: 0,
            poisoned: [false; 4],
            poison_calls: 0,
        }
    }

    // Inject at every invocation, including repeated fences/polls and the second
    // rank's transitions. An after-effect failure models partial side effects.
    fn step(&mut self, name: String, effect: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        self.operations += 1;
        self.events.push(name);
        if self.operations == self.fail_at && !self.after_effect {
            return Err("before effect".into());
        }
        effect(self)?;
        if self.operations == self.expire_at {
            self.clock += Duration::from_millis(10);
        }
        if self.operations == self.fail_at && self.after_effect {
            return Err("after effect".into());
        }
        Ok(())
    }
}

impl CoordinatorBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }
    fn preflight(&mut self) -> Result<()> {
        self.step("preflight".into(), |_| Ok(()))
    }
    fn consume(&mut self, rank: usize) -> Result<()> {
        self.step(format!("consume{rank}"), |s| {
            assert_eq!(s.reserved, [false; 2]);
            require_activation(s.activation[rank], Activation::Ready)?;
            s.activation[rank] = Activation::Submitted;
            Ok(())
        })
    }
    fn reserve(&mut self, rank: usize) -> Result<()> {
        self.step(format!("reserve{rank}"), |s| {
            assert_eq!(s.activation, [Activation::Submitted; 2]);
            assert_eq!(s.published, [false; 2]);
            s.reserved[rank] = true;
            Ok(())
        })
    }
    fn fence(&mut self) -> Result<()> {
        self.step("fence".into(), |_| Ok(()))
    }
    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.step(format!("publish{rank}"), |s| {
            deadline_check(s.clock, deadline)?;
            assert_eq!(s.reserved, [true; 2]);
            assert_eq!(
                s.published,
                if rank == 0 { [false; 2] } else { [true, false] }
            );
            s.published[rank] = true;
            Ok(())
        })
    }
    fn poll(&mut self, deadline: Instant) -> Result<bool> {
        self.step("poll".into(), |s| {
            deadline_check(s.clock, deadline)?;
            assert_eq!(s.published, [true; 2]);
            s.polls += 1;
            Ok(())
        })?;
        Ok(self.polls >= 2)
    }
    fn retire(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.step(format!("retire{rank}"), |s| {
            deadline_check(s.clock, deadline)?;
            assert!(s.polls >= 2);
            s.retired[rank] = true;
            Ok(())
        })
    }
    fn terminal(&mut self) -> Result<()> {
        self.step("terminal".into(), |s| {
            assert_eq!(s.retired, [true; 2]);
            assert_eq!(s.activation, [Activation::Submitted; 2]);
            for state in &s.terminal_states {
                require_terminal(state, 1)?;
            }
            s.terminal_validated = true;
            Ok(())
        })
    }
    fn complete(&mut self, rank: usize) -> Result<CombinedMlpSnapshotV1> {
        self.step(format!("complete{rank}"), |s| {
            assert!(s.terminal_validated);
            s.activation[rank] = Activation::Completed;
            Ok(())
        })?;
        Ok(self.terminal_states[rank].clone())
    }
    fn finish(
        &mut self,
        states: &[CombinedMlpSnapshotV1; 2],
        deadline: Instant,
    ) -> Result<[(u64, u64); 2]> {
        self.step("finish".into(), |s| {
            deadline_check(s.clock, deadline)?;
            assert_eq!(s.activation, [Activation::Completed; 2]);
            assert_eq!(states, &s.terminal_states);
            Ok(())
        })?;
        Ok([(5, 0); 2])
    }
    fn pause(&mut self) {
        self.events.push("pause".into());
    }
    fn poison(&mut self) {
        self.poison_calls += 1;
        self.activation.fill(Activation::Poisoned);
        self.poisoned.fill(true);
        self.events.push("poison".into());
    }
}

#[test]
fn paired_outer_deadline_is_shared_before_preflight_and_publication() {
    let mut fake = Fake::new();
    let expired = fake.clock;
    assert!(coordinate_until(&mut fake, 100, Some(expired)).is_err());
    assert_eq!(fake.operations, 0);
    assert_eq!(fake.poison_calls, 1);
    assert_eq!(fake.published, [false; 2]);
    let mut success = Fake::new();
    coordinate(&mut success, 100).unwrap();
    for ordinal in 1..=success.operations {
        let mut fake = Fake::new();
        fake.expire_at = ordinal;
        let until = fake.clock + Duration::from_millis(10);
        assert!(coordinate_until(&mut fake, 100, Some(until)).is_err());
        assert_eq!(fake.operations, ordinal);
        assert_eq!(fake.poison_calls, 1);
        if ordinal < 7 {
            assert_eq!(fake.published, [false; 2]);
        }
    }
}

#[test]
fn paired_coordinator_exposes_both_batches_before_waiting() {
    let mut fake = Fake::new();
    let result = coordinate(&mut fake, 10).unwrap();
    assert_eq!(
        fake.events,
        [
            "preflight",
            "consume0",
            "consume1",
            "reserve0",
            "reserve1",
            "fence",
            "publish0",
            "fence",
            "publish1",
            "fence",
            "poll",
            "pause",
            "fence",
            "poll",
            "retire0",
            "retire1",
            "terminal",
            "complete0",
            "complete1",
            "finish"
        ]
    );
    assert_eq!(fake.poison_calls, 0);
    assert_eq!(result.states, [terminal_state(), terminal_state()]);
    assert_eq!(result.observed_queue_frontiers, [(5, 0); 2]);
    assert_eq!(result.segment_host_ns, 0);
}

#[test]
fn paired_every_failure_boundary_poisoned_before_and_after_effect() {
    let mut success = Fake::new();
    coordinate(&mut success, 10).unwrap();
    for fail_at in 1..=success.operations {
        for after_effect in [false, true] {
            let mut fake = Fake::new();
            fake.fail_at = fail_at;
            fake.after_effect = after_effect;
            assert!(
                coordinate(&mut fake, 10).is_err(),
                "{fail_at}/{after_effect}"
            );
            assert_eq!(fake.operations, fail_at);
            assert_eq!(fake.activation, [Activation::Poisoned; 2]);
            assert_eq!(fake.poisoned, [true; 4]);
            assert_eq!(fake.poison_calls, 1);
            assert_eq!(fake.events.last().unwrap(), "poison");
        }
    }
}

#[test]
fn paired_every_deadline_boundary_and_invalid_timeout_are_terminal() {
    let mut success = Fake::new();
    coordinate(&mut success, 10).unwrap();
    for expire_at in 1..=success.operations {
        let mut fake = Fake::new();
        fake.expire_at = expire_at;
        assert!(coordinate(&mut fake, 10).is_err(), "{expire_at}");
        assert_eq!(fake.poisoned, [true; 4]);
        assert_eq!(fake.poison_calls, 1);
        assert!(!fake.events.iter().any(|s| s == "complete0") || expire_at >= 17);
    }
    for timeout in [0, 10_001, u32::MAX] {
        let mut fake = Fake::new();
        assert!(coordinate(&mut fake, timeout).is_err());
        assert_eq!(fake.operations, 0);
        assert_eq!(fake.poison_calls, 1);
    }
}

#[test]
fn paired_both_terminal_guards_precede_either_completion() {
    for rank in 0..2 {
        for word in 0..4 {
            let mut fake = Fake::new();
            fake.terminal_states[rank].guard[word] ^= 1;
            assert!(coordinate(&mut fake, 10).is_err());
            assert!(!fake.events.iter().any(|s| s.starts_with("complete")));
            assert!(!fake.terminal_validated);
            assert_eq!(fake.poisoned, [true; 4]);
        }
        let mut fake = Fake::new();
        fake.terminal_states[rank].prefix[547] = 0;
        assert!(coordinate(&mut fake, 10).is_err());
        assert!(!fake.events.iter().any(|s| s.starts_with("complete")));
    }
}

#[test]
fn paired_publication_accepts_early_rank_zero_producers_only() {
    assert!(arena::before_publication([[1; 5]; 2], [false; 2], 0).is_ok());
    for pattern in 0..8 {
        let mut values = [[1; 5]; 2];
        for slot in 0..3 {
            values[0][slot] = (pattern >> slot) & 1;
        }
        assert!(arena::before_publication(values, [true, false], 1).is_ok());
        for (rank, slot) in [(0, 3), (0, 4), (1, 0), (1, 1), (1, 2), (1, 3), (1, 4)] {
            let mut wrong = values;
            wrong[rank][slot] = 0;
            assert!(arena::before_publication(wrong, [true, false], 1).is_err());
        }
    }
    for published in [[false; 2], [false, true], [true; 2]] {
        assert!(arena::before_publication([[1; 5]; 2], published, 1).is_err());
    }
    assert!(arena::before_publication([[1; 5]; 2], [false; 2], 2).is_err());
}

#[test]
fn paired_all_ten_signals_required_without_fabricated_read_credit() {
    let done = [[0; 5]; 2];
    assert!(arena::completion_gate(done, [true; 2], [(5, 0); 2], [5; 2], [0; 2]).unwrap());
    for rank in 0..2 {
        for slot in 0..5 {
            let mut pending = done;
            pending[rank][slot] = 1;
            assert!(
                !arena::completion_gate(pending, [true; 2], [(5, 0); 2], [5; 2], [0; 2]).unwrap()
            );
            for bad in [-1, 2, i64::MIN, i64::MAX] {
                pending[rank][slot] = bad;
                assert!(
                    arena::completion_gate(pending, [true; 2], [(5, 0); 2], [5; 2], [0; 2])
                        .is_err()
                );
                assert!(arena::signal_value(1, bad).is_err());
            }
        }
    }
    for kind in [-1, 0, 2, i64::MAX] {
        assert!(arena::signal_value(kind, 0).is_err());
    }
    for value in [0, 1] {
        assert_eq!(arena::signal_value(1, value).unwrap(), value);
    }
    for counters in [[(4, 0); 2], [(5, 6); 2], [(5, 4); 2]] {
        assert!(arena::completion_gate(done, [true; 2], counters, [5; 2], [5; 2]).is_err());
    }
    assert!(arena::completion_gate(done, [true, false], [(5, 0); 2], [5; 2], [0; 2]).is_err());
    assert!(require_sequence_capacity(MAX_UNRETIRED_RING_PACKETS_V1, 0, 5).is_err());
    assert!(require_sequence_capacity(MAX_UNRETIRED_RING_PACKETS_V1 - 5, 0, 5).is_ok());
    assert!(require_sequence_capacity(MAX_UNRETIRED_RING_PACKETS_V1 - 4, 0, 5).is_err());
}

fn prepared() -> [PreparedDispatch; 4] {
    std::array::from_fn(|index| PreparedDispatch {
        bytes: vec![0; if index == 2 { 280 } else { 360 }],
        geometry: AqlDispatchGeometryV1::new(
            [if index == 2 { 64 } else { 4096 }, 1, 1],
            [64, 1, 1],
        )
        .unwrap(),
        descriptor: 0x400_0000 + index as u64 * 64,
        alignment: 8,
        group_bytes: if index == 1 { 512 } else { 0 },
    })
}

fn signals() -> [[ObservedGpuAddressV1; 5]; 2] {
    std::array::from_fn(|rank| {
        std::array::from_fn(|slot| {
            ObservedGpuAddressV1::new((rank as u64 + 1) * 0x100_0000 + slot as u64 * 64).unwrap()
        })
    })
}

fn kernargs(rank: usize) -> [ObservedGpuAddressV1; 4] {
    std::array::from_fn(|index| {
        ObservedGpuAddressV1::new(
            (rank as u64 + 1) * 0x100_0000
                + PAGE_BYTES as u64
                + index as u64 * u64::from(MAX_KERNARG_BYTES_V1),
        )
        .unwrap()
    })
}

#[derive(Default)]
struct Capture {
    bodies: [Option<[u8; 64]>; 5],
    headers: Vec<(u32, u16)>,
}
impl Capture {
    fn body(&mut self, index: u32, bytes: [u8; 64]) -> Result<()> {
        assert!(self.headers.is_empty());
        assert!(self.bodies[index as usize].replace(bytes).is_none());
        Ok(())
    }
}
impl AqlPeerPacketBatchPublicationTargetV1 for Capture {
    type Error = String;
    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }
    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }
    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        assert!(self.bodies.iter().all(Option::is_some));
        assert_eq!(index as usize, self.headers.len());
        self.headers.push((index, header));
        Ok(())
    }
}
fn graph() -> [Capture; 2] {
    std::array::from_fn(|rank| {
        let mut capture = Capture::default();
        arena::make_batch(rank, prepared(), signals(), kernargs(rank))
            .unwrap()
            .publish_with(&mut capture)
            .unwrap();
        capture
    })
}
fn word(body: &[u8; 64], offset: usize) -> u64 {
    u64::from_le_bytes(body[offset..offset + 8].try_into().unwrap())
}

#[test]
fn paired_real_packet_bytes_encode_five_packet_dependency_graph() {
    let graph = graph();
    let mut completions = BTreeSet::new();
    let mut arguments = BTreeSet::new();
    for rank in 0..2 {
        assert_eq!(
            graph[rank].headers,
            [
                (0, 0x1502),
                (1, 0x1502),
                (2, 0x1502),
                (3, 0x1503),
                (4, 0x1502)
            ]
        );
        for slot in 0..5 {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(&body[..2], &1_u16.to_le_bytes());
            assert_eq!(word(&body, 56), signals()[rank][slot].raw());
            assert!(completions.insert(word(&body, 56)));
        }
        let barrier = graph[rank].bodies[3].unwrap();
        assert_eq!(word(&barrier, 8), signals()[0][2].raw());
        assert_eq!(word(&barrier, 16), signals()[1][2].raw());
        assert_eq!(&barrier[24..56], &[0; 32]);
        for (index, slot) in arena::KERNEL_SLOTS.into_iter().enumerate() {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(word(&body, 32), 0x400_0000 + index as u64 * 64);
            assert_eq!(word(&body, 40), kernargs(rank)[index].raw());
            assert!(arguments.insert(word(&body, 40)));
            assert_eq!(
                u32::from_le_bytes(body[12..16].try_into().unwrap()),
                if index == 2 { 64 } else { 4096 }
            );
        }
    }
    assert_eq!(completions.len(), 10);
    assert_eq!(arguments.len(), 8);
}

#[test]
fn paired_packet_constructor_refuses_duplicate_completion_addresses() {
    for left in 0..10 {
        for right in left + 1..10 {
            let mut value = signals();
            value[right / 5][right % 5] = value[left / 5][left % 5];
            for rank in 0..2 {
                assert!(arena::make_batch(rank, prepared(), value, kernargs(rank)).is_err());
            }
        }
    }
    assert!(arena::make_batch(2, prepared(), signals(), kernargs(0)).is_err());
}

// Enumerate all rank interleavings against the handles in the real packet
// bytes. This checks the encoded graph, not GPU memory-coherence behavior.
fn schedules(
    graph: &[Capture; 2],
    next: [usize; 2],
    done: BTreeSet<u64>,
    unsafe_paths: &mut usize,
) -> usize {
    if next == [5; 2] {
        return 1;
    }
    let mut paths = 0;
    for rank in 0..2 {
        let slot = next[rank];
        if slot == 5 {
            continue;
        }
        let body = graph[rank].bodies[slot].unwrap();
        if slot == 3
            && [word(&body, 8), word(&body, 16)]
                .iter()
                .any(|handle| !done.contains(handle))
        {
            continue;
        }
        if slot == 4 && !signals().iter().all(|s| done.contains(&s[2].raw())) {
            *unsafe_paths += 1;
        }
        let mut advanced = next;
        advanced[rank] += 1;
        let mut completed = done.clone();
        completed.insert(word(&body, 56));
        paths += schedules(graph, advanced, completed, unsafe_paths);
    }
    paths
}

#[test]
fn paired_actual_barriers_block_r2_until_both_validators_complete() {
    let mut graph = graph();
    let mut unsafe_paths = 0;
    assert!(schedules(&graph, [0; 2], BTreeSet::new(), &mut unsafe_paths) > 0);
    assert_eq!(unsafe_paths, 0);
    // Depending on MLP completion alone is insufficient: a peer validator may
    // still be pending. The same enumerator must discover that counterexample.
    for rank in 0..2 {
        let body = graph[rank].bodies[3].as_mut().unwrap();
        for peer in 0..2 {
            body[8 + peer * 8..16 + peer * 8]
                .copy_from_slice(&signals()[peer][1].raw().to_le_bytes());
        }
    }
    schedules(&graph, [0; 2], BTreeSet::new(), &mut unsafe_paths);
    assert!(unsafe_paths > 0);
}

#[test]
fn paired_guard_arguments_preserve_generation_lengths_and_zero_padding() {
    for generation in [1, 0x1_0000_0003, u64::MAX] {
        for validator in [false, true] {
            let bytes = profiles::guarded_arguments(generation, validator);
            let lengths: &[u64] = if validator {
                &[552]
            } else {
                &[4096, 4096, 4096, 4096, 4, 4]
            };
            for (i, length) in lengths.iter().enumerate() {
                assert_eq!(&bytes[i * 16..i * 16 + 8], &[0; 8]);
                assert_eq!(&bytes[i * 16 + 8..i * 16 + 16], &length.to_le_bytes());
            }
            let at = lengths.len() * 16;
            assert_eq!(&bytes[at..at + 8], &generation.to_le_bytes());
            assert_eq!(&bytes[at + 8..], &[0; 256]);
        }
    }
}

fn metadata(validator: bool) -> KernelMetadataV1 {
    let slices: usize = if validator { 1 } else { 6 };
    let explicit = if validator { 24 } else { 104 };
    KernelMetadataV1 {
        symbol: if validator {
            profiles::GUARD_SYMBOL
        } else {
            profiles::R2_SYMBOL
        }
        .into(),
        object_sha256: profiles::GUARDED_IMAGE,
        kernarg_bytes: explicit + 256,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(explicit),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..slices * 2 + 2)
            .map(|i| {
                let slice = i < slices * 2;
                crate::engineering_wire::ExplicitArgumentV1 {
                    offset: if slice {
                        (i * 8) as u32
                    } else {
                        (slices * 16 + (i - slices * 2) * 4) as u32
                    },
                    bytes: if slice { 8 } else { 4 },
                    global_buffer: slice && i % 2 == 0,
                    pointee_alignment: None,
                    access: None,
                }
            })
            .collect(),
    }
}

#[test]
fn paired_guard_metadata_is_exact_not_an_annotation_wildcard() {
    for validator in [false, true] {
        let base = metadata(validator);
        profiles::guarded_metadata(&base, validator).unwrap();
        assert!(profiles::guarded_metadata(&base, !validator).is_err());
        for mutation in 0..14 {
            let mut value = base.clone();
            match mutation {
                0 => value.symbol.push('x'),
                1 => value.object_sha256[31] ^= 1,
                2 => value.kernarg_bytes += 1,
                3 => value.kernarg_bytes -= 1,
                4 => value.kernarg_alignment = 16,
                5 => value.group_segment_bytes = 1,
                6 => value.private_segment_bytes = 1,
                7 => value.wavefront_size = 32,
                8 => value.implicit_argument_offset = None,
                9 => value.implicit_argument_offset = Some(0),
                10 => value.implicit_argument_bytes = 255,
                11 => value.implicit_argument_bytes = 257,
                12 => {
                    value.explicit_arguments.pop();
                }
                _ => value
                    .explicit_arguments
                    .push(value.explicit_arguments[0].clone()),
            }
            assert!(
                profiles::guarded_metadata(&value, validator).is_err(),
                "{validator}/{mutation}"
            );
        }
        for index in 0..base.explicit_arguments.len() {
            for mutation in 0..9 {
                let mut value = base.clone();
                let arg = &mut value.explicit_arguments[index];
                match mutation {
                    0 => arg.offset += 1,
                    1 => arg.bytes ^= 4,
                    2 => arg.global_buffer = !arg.global_buffer,
                    3..=5 => arg.pointee_alignment = Some(1 << (mutation - 3)),
                    6 => arg.access = Some(BufferAccessV1::Read),
                    7 => arg.access = Some(BufferAccessV1::Write),
                    _ => arg.access = Some(BufferAccessV1::ReadWrite),
                }
                assert!(profiles::guarded_metadata(&value, validator).is_err());
            }
        }
    }
}

fn owned(id: u64, requested: usize) -> profile::OwnedRegion {
    profile::OwnedRegion {
        buffer: id,
        base: id * 0x1000_0000,
        requested,
        backing: requested.div_ceil(PAGE_BYTES) * PAGE_BYTES,
    }
}
fn roots() -> [[profile::OwnedRegion; 11]; 2] {
    std::array::from_fn(|rank| {
        std::array::from_fn(|i| {
            owned(
                (rank * 11 + i + 1) as u64,
                if i == 10 {
                    COMBINED_BYTES
                } else {
                    profile::EXTENTS[i]
                },
            )
        })
    })
}

#[test]
fn paired_genuine_extents_and_allocation_backings_are_checked() {
    let partials = [owned(30, 16384), owned(31, 16384)];
    let residuals = [owned(32, 8192), owned(33, 8192)];
    let outputs = [owned(34, 8192), owned(35, 8192)];
    profiles::validate_regions(&roots(), &partials, &residuals, &outputs).unwrap();
    for row in 0..28 {
        for mutation in 0..8 {
            let mut roots = roots();
            let mut partials = partials;
            let mut residuals = residuals;
            let mut outputs = outputs;
            let value = match row {
                0..=21 => &mut roots[row / 11][row % 11],
                22..=23 => &mut partials[row - 22],
                24..=25 => &mut residuals[row - 24],
                _ => &mut outputs[row - 26],
            };
            match mutation {
                0 => value.buffer = 0,
                1 => value.base = 0,
                2 => value.base += 1,
                3 => value.requested -= 1,
                4 => value.requested += 1,
                5 => value.backing = value.requested - 1,
                6 => value.backing += 1,
                _ => value.base = u64::MAX - PAGE_BYTES as u64 + 1,
            }
            assert!(profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err());
        }
    }
    for rank in 0..2 {
        let mut roots = roots();
        roots[rank][10].requested = 2192;
        assert!(profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err());
    }
    // Disjoint payloads can still occupy overlapping padded allocations.
    let mut roots = roots();
    roots[0][10].backing = 8192;
    roots[1][10].base = roots[0][10].base + 4096;
    assert!(profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err());
}

#[test]
fn paired_cross_stage_writer_aliases_and_output_aliases_are_refused() {
    let partials = [owned(30, 16384), owned(31, 16384)];
    let residuals = [owned(32, 8192), owned(33, 8192)];
    let outputs = [owned(34, 8192), owned(35, 8192)];
    for left in 0..22 {
        for right in left + 1..22 {
            for id_alias in [false, true] {
                let mut roots = roots();
                if id_alias {
                    roots[right / 11][right % 11].buffer = roots[left / 11][left % 11].buffer;
                } else {
                    roots[right / 11][right % 11].base = roots[left / 11][left % 11].base;
                }
                assert!(
                    profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err()
                );
            }
        }
    }
    let roots = roots();
    for row in &roots {
        for index in [0, 5, 6, 7, 8, 9, 10] {
            for read in 0..4 {
                let mut partials = partials;
                let mut residuals = residuals;
                let value = if read < 2 {
                    &mut partials[read]
                } else {
                    &mut residuals[read - 2]
                };
                value.base = row[index].base;
                assert!(
                    profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err()
                );
            }
        }
    }
    let retained: Vec<_> = roots
        .iter()
        .flatten()
        .chain(&partials)
        .chain(&residuals)
        .chain(&outputs)
        .copied()
        .collect();
    for rank in 0..2 {
        for (index, prior) in retained.iter().enumerate() {
            if index == 26 + rank {
                continue;
            }
            let mut outputs = outputs;
            outputs[rank].base = prior.base;
            assert!(profiles::validate_regions(&roots, &partials, &residuals, &outputs).is_err());
        }
    }
    let mut shared_reads = residuals;
    shared_reads[0] = roots[0][1];
    profiles::validate_regions(&roots, &partials, &shared_reads, &outputs).unwrap();
    assert!(profiles::validate_regions(&roots, &[partials[0]; 2], &residuals, &outputs).is_err());
}

#[test]
fn paired_native_entry_refuses_wrong_world_before_gpu_operations() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 5,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    let mut owners = std::array::from_fn(|rank| CombinedMlpStateV1 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3 + rank as u64,
            owner: rank,
            bytes: 2208,
        },
        activation: Activation::Ready,
        generation: 1,
    });
    let kernels: [Gfx950EngineeringPeerKernelV1; 2] =
        std::array::from_fn(|rank| Gfx950EngineeringPeerKernelV1 {
            group: 7,
            rank,
            id: 1,
            metadata: metadata(true),
        });
    let inputs = Inputs {
        ranks: std::array::from_fn(|rank| RankInputs {
            kernels: [&kernels[rank]; 4],
            mlp_roots: [owners[rank].buffer; 10],
            residual_input: owners[rank].buffer,
            output: owners[rank].buffer,
        }),
        partials: [owners[0].buffer, owners[1].buffer],
        projection_sha256: [1; 32],
        mlp_sha256: [1; 32],
    };
    // No backend exists, so the checked world-size refusal must precede every
    // allocation/publication operation and terminate both owners.
    assert!(unsafe { dispatch(&mut group, &mut owners, inputs, 10) }.is_err());
    assert!(group.poisoned);
    assert!(
        owners
            .iter()
            .all(|owner| owner.activation == Activation::Poisoned)
    );
}

#[test]
fn paired_exact_residual_policy_requires_both_full_identities() {
    let mlp = roots();
    let partials = [owned(30, 16384), owned(31, 16384)];
    let residuals = [owned(32, 8192), owned(33, 8192)];
    let separate = [owned(34, 8192), owned(35, 8192)];
    let check = |outputs: &[profile::OwnedRegion; 2]| {
        profiles::validate_regions_with_policy(
            &mlp,
            &partials,
            &residuals,
            outputs,
            profiles::OutputPolicy::ExactOwnResidual,
        )
    };
    for mask in 0..4 {
        let outputs = std::array::from_fn(|rank| {
            if mask & (1 << rank) == 0 {
                separate[rank]
            } else {
                residuals[rank]
            }
        });
        assert_eq!(check(&outputs).is_ok(), mask == 3);
        assert_eq!(
            profiles::validate_regions(&mlp, &partials, &residuals, &outputs).is_ok(),
            mask == 0
        );
    }
    assert!(check(&[residuals[1], residuals[0]]).is_err());
    for rank in 0..2 {
        for field in 0..5 {
            let mut outputs = residuals;
            match field {
                0 => outputs[rank].buffer += 100,
                1 => outputs[rank].base += PAGE_BYTES as u64,
                2 => outputs[rank].requested -= 1,
                3 => outputs[rank].backing += PAGE_BYTES,
                _ => outputs[rank].base += outputs[rank].backing as u64,
            }
            assert!(check(&outputs).is_err(), "{rank}/{field}");
        }
    }
}

#[test]
fn paired_exact_residual_policy_refuses_all_live_root_and_peer_aliases() {
    let mlp = roots();
    let partials = [owned(30, 16384), owned(31, 16384)];
    let residuals = [owned(32, 8192), owned(33, 8192)];
    for rank in 0..2 {
        let live: Vec<_> = mlp
            .iter()
            .flatten()
            .chain(&partials)
            .chain(std::iter::once(&residuals[1 - rank]))
            .copied()
            .collect();
        assert_eq!(live.len(), 25);
        for (slot, prior) in live.iter().enumerate() {
            for collision in 0..3 {
                let mut reads = residuals;
                match collision {
                    0 => reads[rank].buffer = prior.buffer,
                    1 => reads[rank].base = prior.base,
                    _ => reads[rank].base = prior.base + prior.backing as u64 - PAGE_BYTES as u64,
                }
                // Exact own identity must not mask transitive root/partial/peer overlap.
                assert!(
                    profiles::validate_regions_with_policy(
                        &mlp,
                        &partials,
                        &reads,
                        &reads,
                        profiles::OutputPolicy::ExactOwnResidual,
                    )
                    .is_err(),
                    "{rank}/{slot}/{collision}"
                );
            }
        }
        for peer in 0..2 {
            let mut reads = residuals;
            reads[rank] = mlp[peer][1];
            let separate = [owned(34, 8192), owned(35, 8192)];
            // The strict policy permits this shared read; it cannot become an R2 write.
            profiles::validate_regions(&mlp, &partials, &reads, &separate).unwrap();
            assert!(
                profiles::validate_regions_with_policy(
                    &mlp,
                    &partials,
                    &reads,
                    &reads,
                    profiles::OutputPolicy::ExactOwnResidual,
                )
                .is_err()
            );
        }
    }
    for id_alias in [false, true] {
        let mut reads = residuals;
        if id_alias {
            reads[1].buffer = reads[0].buffer;
        } else {
            reads[1].base = reads[0].base + PAGE_BYTES as u64;
        }
        assert!(
            profiles::validate_regions_with_policy(
                &mlp,
                &partials,
                &reads,
                &reads,
                profiles::OutputPolicy::ExactOwnResidual,
            )
            .is_err()
        );
    }
}

#[test]
fn paired_exact_residual_policy_preserves_extents_and_backing_checks() {
    let partials = [owned(30, 16384), owned(31, 16384)];
    let residuals = [owned(32, 8192), owned(33, 8192)];
    for row in 0..26 {
        for field in 0..8 {
            let mut mlp = roots();
            let mut partials = partials;
            let mut reads = residuals;
            let value = match row {
                0..=21 => &mut mlp[row / 11][row % 11],
                22..=23 => &mut partials[row - 22],
                _ => &mut reads[row - 24],
            };
            match field {
                0 => value.buffer = 0,
                1 => value.base = 0,
                2 => value.base += 1,
                3 => value.requested -= 1,
                4 => value.requested += 1,
                5 => value.backing = value.requested - 1,
                6 => value.backing += 1,
                _ => value.base = u64::MAX - PAGE_BYTES as u64 + 1,
            }
            assert!(
                profiles::validate_regions_with_policy(
                    &mlp,
                    &partials,
                    &reads,
                    &reads,
                    profiles::OutputPolicy::ExactOwnResidual,
                )
                .is_err(),
                "{row}/{field}"
            );
        }
    }
    for rank in 0..2 {
        let mut mlp = roots();
        mlp[rank][10].requested = 2192;
        assert!(
            profiles::validate_regions_with_policy(
                &mlp,
                &partials,
                &residuals,
                &residuals,
                profiles::OutputPolicy::ExactOwnResidual,
            )
            .is_err()
        );
    }
    let mut reads = residuals;
    reads[1].base = reads[0].base + reads[0].backing as u64;
    profiles::validate_regions_with_policy(
        &roots(),
        &partials,
        &reads,
        &reads,
        profiles::OutputPolicy::ExactOwnResidual,
    )
    .unwrap();
    // A padded backing extends the live interval even when requested bytes do not overlap.
    reads[0].backing += PAGE_BYTES;
    assert!(
        profiles::validate_regions_with_policy(
            &roots(),
            &partials,
            &reads,
            &reads,
            profiles::OutputPolicy::ExactOwnResidual,
        )
        .is_err()
    );
}
