use super::*;

fn tokens() -> [Gfx950EngineeringPeerBufferV1; 2] {
    core::array::from_fn(|rank| Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id: rank as u64 + 1,
        owner: rank,
        bytes: 8192,
    })
}

struct Fake {
    events: Vec<&'static str>,
    fail: Option<usize>,
    panic: Option<usize>,
    poisoned: bool,
    bad_second: bool,
    copies: usize,
}
impl Fake {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            fail: None,
            panic: None,
            poisoned: false,
            bad_second: false,
            copies: 0,
        }
    }
    fn event(&mut self, name: &'static str) -> Result<()> {
        let index = self.events.len();
        self.events.push(name);
        assert_ne!(self.panic, Some(index), "injected pair-read unwind");
        if self.fail == Some(index) {
            Err("injected pair-read failure".into())
        } else {
            Ok(())
        }
    }
}
impl ReadPairBackend for Fake {
    fn validate(&mut self) -> Result<()> {
        self.event("validate-both")?;
        let mut input = tokens();
        if self.bad_second {
            input[1].bytes = 8191;
        }
        validate_headers(7, 2, input, 0, 8192)
    }
    fn fence(&mut self) -> Result<()> {
        self.event("fresh-full-group-and-both-queues")
    }
    fn copy(&mut self, rank: usize) -> Result<Vec<u8>> {
        self.event(if rank == 0 { "copy0" } else { "copy1" })?;
        self.copies += 1;
        Ok(vec![rank as u8 + 10; 8])
    }
    fn quarantine(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn pair_read_orders_each_fresh_fence_before_publication() {
    let expected = [
        "validate-both",
        "fresh-full-group-and-both-queues",
        "copy0",
        "copy1",
        "fresh-full-group-and-both-queues",
    ];
    let mut fake = Fake::new();
    for _ in 0..2 {
        assert_eq!(read_pair(&mut fake).unwrap(), [vec![10; 8], vec![11; 8]]);
        assert!(!fake.poisoned);
    }
    assert_eq!(fake.events, expected.repeat(2));
    assert_eq!(fake.copies, 4);
}

#[test]
fn pair_read_each_boundary_failure_quarantines_without_data() {
    let expected = [
        "validate-both",
        "fresh-full-group-and-both-queues",
        "copy0",
        "copy1",
        "fresh-full-group-and-both-queues",
    ];
    for index in 0..expected.len() {
        let mut fake = Fake::new();
        fake.fail = Some(index);
        assert!(read_pair(&mut fake).is_err());
        assert_eq!(fake.events, expected[..=index]);
        assert_eq!(fake.copies, index.saturating_sub(2).min(2));
        assert!(fake.poisoned);
    }
}

#[test]
fn pair_read_unwind_quarantines_after_partial_copy() {
    for index in 0..5 {
        let mut fake = Fake::new();
        fake.panic = Some(index);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_pair(&mut fake)))
                .is_err()
        );
        assert_eq!(fake.events.len(), index + 1);
        assert_eq!(fake.copies, index.saturating_sub(2).min(2));
        assert!(fake.poisoned);
    }
}

#[test]
fn pair_read_headers_reject_foreign_rank_alias_extent_and_caps() {
    validate_headers(7, 2, tokens(), 0, 8192).unwrap();
    validate_headers(7, 2, tokens(), 4096, 4096).unwrap();
    for world in [0, 1, 3, 8, usize::MAX] {
        assert!(validate_headers(7, world, tokens(), 0, 8192).is_err());
    }
    for rank in 0..2 {
        for field in 0..5 {
            let mut input = tokens();
            match field {
                0 => input[rank].group = 8,
                1 => input[rank].owner = 1 - rank,
                2 => input[rank].id = 0,
                3 => input[rank].id = input[1 - rank].id,
                _ => input[rank].bytes = 8191,
            }
            assert!(validate_headers(7, 2, input, 0, 8192).is_err());
        }
    }
    for (offset, bytes) in [(0, 0), (8192, 1), (u64::MAX, 2), (0, u32::MAX)] {
        assert!(validate_headers(7, 2, tokens(), offset, bytes).is_err());
    }
    let mut large = tokens();
    for buffer in &mut large {
        buffer.bytes = u64::from(MAX_TRANSFER_BYTES_V1);
    }
    validate_headers(7, 2, large, 0, MAX_TRANSFER_BYTES_V1 / 2).unwrap();
    assert!(validate_headers(7, 2, large, 0, MAX_TRANSFER_BYTES_V1 / 2 + 1).is_err());
    assert!(validate_headers(0, 2, tokens(), 0, 1).is_err());
}

#[test]
fn pair_read_second_bad_header_precedes_any_queue_or_copy() {
    let mut fake = Fake::new();
    fake.bad_second = true;
    assert!(read_pair(&mut fake).is_err());
    assert_eq!(fake.events, ["validate-both"]);
    assert_eq!(fake.copies, 0);
    assert!(fake.poisoned);
}

fn empty_group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    }
}

#[test]
fn pair_read_native_invalid_roster_quarantines_and_ordinary_reads_stay_closed() {
    let mut group = empty_group();
    assert!(group.read_pair_v1(tokens(), 0, 8192).is_err());
    assert!(group.poisoned);
    assert!(group.read(tokens()[0], 0, 1).is_err());
    assert!(group.read_pair_v1(tokens(), 0, 1).is_err());
    assert!(group.close().is_err());
    let mut group = empty_group();
    group.closed = true;
    assert!(group.read_pair_v1(tokens(), 0, 1).is_err());
    assert!(!group.poisoned);
}

#[test]
fn pair_read_public_signature_returns_bytes_not_a_currentness_capability() {
    let _: fn(
        &mut Gfx950EngineeringPeerGroupV1,
        [Gfx950EngineeringPeerBufferV1; 2],
        u64,
        u32,
    ) -> Result<[Vec<u8>; 2]> = Gfx950EngineeringPeerGroupV1::read_pair_v1;
    let mut group = empty_group();
    for (rank, token) in tokens().into_iter().enumerate() {
        group.buffers.insert(
            token.id,
            BufferRecord {
                token,
                local_id: rank as u64 + 1,
                mapping: PeerMapping {
                    peers: vec![],
                    mapped: 0,
                    unmapped: 0,
                    phase: Phase::PeersMapped,
                },
                kind: BufferKind::PublicVram,
            },
        );
        require_public_vram(group.validate_token(token).unwrap()).unwrap();
        group.buffers.get_mut(&token.id).unwrap().kind = BufferKind::CombinedMlpStateV1;
        assert!(require_public_vram(group.validate_token(token).unwrap()).is_err());
        group.buffers.get_mut(&token.id).unwrap().mapping.phase = Phase::Quarantined;
        assert!(group.validate_token(token).is_err());
    }
}
