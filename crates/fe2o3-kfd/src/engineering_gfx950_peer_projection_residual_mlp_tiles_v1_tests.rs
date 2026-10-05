use super::*;

fn metadata() -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: PROJECTION_SYMBOL.into(),
        object_sha256: [7; 32],
        kernarg_bytes: 424,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(168),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..22)
            .map(|index| {
                let pointer = index < 20 && index % 2 == 0;
                crate::engineering_wire::ExplicitArgumentV1 {
                    offset: if index < 20 {
                        index * 8
                    } else {
                        160 + (index - 20) * 4
                    },
                    bytes: if index < 20 { 8 } else { 4 },
                    global_buffer: pointer,
                    pointee_alignment: pointer.then_some(if index < 16 { 4 } else { 2 }),
                    access: pointer.then_some(if index == 18 {
                        BufferAccessV1::Write
                    } else {
                        BufferAccessV1::Read
                    }),
                }
            })
            .collect(),
    }
}

fn terminal() -> [u32; 548] {
    let mut words = [0; 548];
    words[0] = 1;
    words[3] = 31;
    words[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    words[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    words[14..22].fill(u32::MAX);
    words[22] = 3;
    words[23..31].fill(u32::MAX);
    words[31] = 3;
    words[32..290].fill(1);
    words[290..548].fill(64);
    words
}

fn region(id: u64, bytes: usize) -> profile::OwnedRegion {
    profile::OwnedRegion {
        buffer: id,
        base: id * 0x1000_0000,
        requested: bytes,
        backing: bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES,
    }
}

fn regions() -> [[profile::OwnedRegion; 11]; 2] {
    core::array::from_fn(|rank| {
        core::array::from_fn(|index| {
            region((rank * 11 + index + 1) as u64, profile::EXTENTS[index])
        })
    })
}

fn reads() -> ([profile::OwnedRegion; 2], [profile::OwnedRegion; 2]) {
    (
        [region(30, 16_384), region(31, 16_384)],
        [region(32, 8192), region(33, 8192)],
    )
}

fn identities() -> [RankIdentity; 2] {
    core::array::from_fn(|rank| RankIdentity {
        rank,
        unique_id: rank as u64 + 9,
        gpu_id: rank as u32 + 5,
        queue_id: Some(rank as u32 + 1),
        epoch: 7,
        storage: ordered::PairStorageIdentity {
            arena: (
                0x100000 + rank as u64 * 0x200000,
                20 + rank as u64,
                1 << 20,
                1 << 20,
            ),
            signal: (
                0x1000 + rank as u64 * 0x1000,
                30 + rank as u64,
                PAGE_BYTES,
                PAGE_BYTES,
            ),
        },
    })
}

fn scratch() -> Scratch {
    Scratch {
        incarnation: 17,
        ranks: identities(),
        phase: ScratchPhase::Idle {
            retired_frontiers: [4, 8],
        },
    }
}

struct Fake {
    clock: Instant,
    calls: Vec<String>,
    fail: Option<&'static str>,
    consume_delay: Duration,
    terminal_delay: Duration,
    complete_delay: Duration,
    published: [bool; 2],
    polls: [u32; 2],
    signals: [[bool; 2]; 2],
    final_states: [[u32; 548]; 2],
    activation: [Activation; 2],
    phase: ScratchPhase,
    deadlines: Vec<Instant>,
    poisoned: bool,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            clock: Instant::now(),
            calls: vec![],
            fail: None,
            consume_delay: Duration::ZERO,
            terminal_delay: Duration::ZERO,
            complete_delay: Duration::ZERO,
            published: [false; 2],
            polls: [0; 2],
            signals: [[true; 2]; 2],
            final_states: [terminal(); 2],
            activation: [Activation::Ready; 2],
            phase: ScratchPhase::Idle {
                retired_frontiers: [0; 2],
            },
            deadlines: vec![],
            poisoned: false,
        }
    }
}

impl Fake {
    fn step(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        self.calls.push(name.clone());
        self.clock += Duration::from_micros(1);
        if self.fail == Some(name.as_str()) {
            return Err("injected failure".into());
        }
        Ok(())
    }
}

impl SegmentBackend for Fake {
    fn now(&mut self) -> Instant {
        self.clock
    }
    fn preflight(&mut self) -> Result<()> {
        self.step("preflight")?;
        if self.activation != [Activation::Ready; 2] {
            return Err("not Ready".into());
        }
        Ok(())
    }
    fn consume(&mut self) -> Result<()> {
        self.step("consume")?;
        self.activation = [Activation::Submitted; 2];
        self.phase = ScratchPhase::Busy;
        self.clock += self.consume_delay;
        Ok(())
    }
    fn stage(&mut self) -> Result<()> {
        assert_eq!(self.activation, [Activation::Submitted; 2]);
        assert_eq!(self.phase, ScratchPhase::Busy);
        assert_eq!(self.published, [false; 2]);
        self.step("stage-both")
    }
    fn publication_fence(&mut self) -> Result<()> {
        self.step("publication-fence")
    }
    fn publish(&mut self, rank: usize, deadline: Instant) -> Result<()> {
        self.step(format!("publish{rank}"))?;
        self.published[rank] = true;
        self.deadlines.push(deadline);
        Ok(())
    }
    fn poll(&mut self, rank: usize) -> Result<bool> {
        assert_eq!(self.published, [true; 2]);
        self.step(format!("poll{rank}"))?;
        self.polls[rank] += 1;
        Ok(self.polls[rank] >= 2)
    }
    fn validate_signals(&mut self, rank: usize) -> Result<()> {
        self.step(format!("signals{rank}"))?;
        if self.signals[rank] != [true; 2] {
            return Err("earlier signal incomplete".into());
        }
        Ok(())
    }
    fn retire_queues(&mut self) -> Result<()> {
        self.step("retire-queues")
    }
    fn terminal(&mut self) -> Result<[[u32; 548]; 2]> {
        self.step("terminal-fence-read-read-fence")?;
        for state in self.final_states {
            profile::validate_final_state(state)?;
        }
        self.clock += self.terminal_delay;
        Ok(self.final_states)
    }
    fn complete(&mut self) -> Result<()> {
        self.step("complete")?;
        self.clock += self.complete_delay;
        self.activation = [Activation::Completed; 2];
        self.phase = ScratchPhase::Idle {
            retired_frontiers: [2; 2],
        };
        Ok(())
    }
    fn pause(&mut self) {
        self.clock += Duration::from_micros(50);
    }
    fn poison(&mut self) {
        self.poisoned = true;
        self.phase = ScratchPhase::Poisoned;
    }
}

#[test]
fn ordered_pair_publishes_both_before_any_poll() {
    let mut fake = Fake::default();
    let result = coordinate(&mut fake, 10_000).unwrap();
    assert_eq!(
        &fake.calls[..7],
        [
            "preflight",
            "consume",
            "stage-both",
            "publication-fence",
            "publish0",
            "publication-fence",
            "publish1"
        ]
    );
    assert_eq!(fake.polls, [2, 2]);
    assert_eq!(fake.activation, [Activation::Completed; 2]);
    assert_eq!(result.final_states, [terminal(); 2]);
    assert!(result.segment_host_ns > 0);
    assert!(!fake.poisoned);
}

#[test]
fn ordered_pair_preflight_rejects_before_consumption_or_publication() {
    let mut fake = Fake {
        fail: Some("preflight"),
        ..Fake::default()
    };
    assert!(coordinate(&mut fake, 100).is_err());
    assert_eq!(fake.activation, [Activation::Ready; 2]);
    assert_eq!(fake.published, [false; 2]);
    assert!(fake.poisoned);
}

#[test]
fn ordered_pair_partial_publication_is_terminal() {
    let mut fake = Fake {
        fail: Some("publish1"),
        ..Fake::default()
    };
    assert!(coordinate(&mut fake, 100).is_err());
    assert_eq!(fake.published, [true, false]);
    assert_eq!(fake.activation, [Activation::Submitted; 2]);
    assert_eq!(fake.polls, [0, 0]);
    assert_eq!(fake.phase, ScratchPhase::Poisoned);
}

#[test]
fn ordered_pair_every_uncertain_stage_poisoned_without_completion() {
    for fail in [
        "consume",
        "stage-both",
        "publication-fence",
        "publish0",
        "poll0",
        "poll1",
        "signals0",
        "signals1",
        "retire-queues",
        "terminal-fence-read-read-fence",
        "complete",
    ] {
        let mut fake = Fake {
            fail: Some(fail),
            ..Fake::default()
        };
        assert!(coordinate(&mut fake, 100).is_err(), "{fail}");
        assert!(fake.poisoned, "{fail}");
        assert_ne!(fake.activation, [Activation::Completed; 2], "{fail}");
    }
}

#[test]
fn ordered_pair_missing_earlier_completion_rejects_final_signal_only() {
    for rank in 0..2 {
        let mut fake = Fake::default();
        fake.signals[rank][0] = false;
        assert!(coordinate(&mut fake, 100).is_err());
        assert!(!fake.calls.iter().any(|v| v == "retire-queues"));
        assert_eq!(fake.activation, [Activation::Submitted; 2]);
        assert!(fake.poisoned);
    }
}

#[test]
fn ordered_pair_one_deadline_covers_staging_and_both_publications() {
    let mut fake = Fake::default();
    let start = fake.clock;
    coordinate(&mut fake, 100).unwrap();
    assert_eq!(fake.deadlines, vec![start + Duration::from_millis(100); 2]);
    let mut late = Fake {
        consume_delay: Duration::from_millis(100),
        ..Fake::default()
    };
    assert!(coordinate(&mut late, 100).is_err());
    assert_eq!(late.published, [false; 2]);
    assert!(late.poisoned);
}

#[test]
fn ordered_pair_timeout_includes_terminal_readback_and_fences() {
    let mut fake = Fake {
        terminal_delay: Duration::from_millis(100),
        ..Fake::default()
    };
    assert!(coordinate(&mut fake, 100).is_err());
    assert_eq!(fake.activation, [Activation::Submitted; 2]);
    assert_eq!(fake.phase, ScratchPhase::Poisoned);
}

#[test]
fn ordered_pair_timeout_after_retirement_poison_blocks_completed_state_reuse() {
    let mut fake = Fake {
        complete_delay: Duration::from_millis(100),
        ..Fake::default()
    };
    assert!(coordinate(&mut fake, 100).is_err());
    assert_eq!(fake.activation, [Activation::Completed; 2]);
    assert!(fake.poisoned);
    assert_eq!(fake.phase, ScratchPhase::Poisoned);
}

#[test]
fn ordered_pair_common_timeout_rejects_split_or_unbounded_values() {
    assert_eq!(timeout(1, 1).unwrap(), 1);
    assert_eq!(timeout(10_000, 10_000).unwrap(), 10_000);
    for (left, right) in [(0, 0), (1, 2), (10_001, 10_001), (u32::MAX, 1)] {
        assert!(timeout(left, right).is_err());
    }
    let mut fake = Fake::default();
    assert!(coordinate(&mut fake, 0).is_err());
    assert!(fake.calls.is_empty());
    assert!(fake.poisoned);
}

#[test]
fn ordered_pair_genuine_typed_terminal_refusal_keeps_submitted() {
    let mut fake = Fake::default();
    fake.final_states[1][289] = 0;
    assert!(coordinate(&mut fake, 100).is_err());
    assert_eq!(fake.activation, [Activation::Submitted; 2]);
    assert!(fake.poisoned);
}

#[test]
fn ordered_pair_repeated_activation_is_rejected_before_stage() {
    for activation in [
        Activation::Allocated,
        Activation::Initialized,
        Activation::Submitted,
        Activation::Completed,
    ] {
        let mut fake = Fake::default();
        fake.activation[1] = activation;
        assert!(coordinate(&mut fake, 100).is_err());
        assert_eq!(fake.published, [false; 2]);
        assert!(fake.poisoned);
    }
}

#[test]
fn ordered_pair_distinct_down_roots_accept_and_old_alias_rejects() {
    let mut mlp = regions();
    let (partials, inputs) = reads();
    validate_cross_stage(&mlp, &partials, &inputs).unwrap();
    mlp[0][9] = partials[0];
    assert!(validate_cross_stage(&mlp, &partials, &inputs).is_err());
}

#[test]
fn ordered_pair_checks_every_cross_rank_write_and_backing_alias() {
    let mlp = regions();
    let (partials, inputs) = reads();
    for rank in 0..2 {
        for index in [0, 5, 6, 7, 8, 9, 10] {
            let mut bad = partials;
            bad[1] = mlp[rank][index];
            assert!(validate_cross_stage(&mlp, &bad, &inputs).is_err());
            let mut bad_inputs = inputs;
            bad_inputs[0] = mlp[rank][index];
            assert!(validate_cross_stage(&mlp, &partials, &bad_inputs).is_err());
        }
    }
    let mut bad = partials;
    bad[0].base = mlp[1][9].base + PAGE_BYTES as u64;
    assert!(validate_cross_stage(&mlp, &bad, &inputs).is_err());
}

#[test]
fn ordered_pair_read_only_sharing_does_not_create_a_writer() {
    let mlp = regions();
    let (partials, mut inputs) = reads();
    inputs[0] = mlp[0][1];
    inputs[1] = mlp[1][1];
    validate_cross_stage(&mlp, &partials, &inputs).unwrap();
}

#[test]
fn ordered_pair_projection_metadata_is_closed_and_digest_bound() {
    let base = metadata();
    validate_projection_metadata(&base, [7; 32]).unwrap();
    assert!(validate_projection_metadata(&base, [8; 32]).is_err());
    let mut zero = base.clone();
    zero.object_sha256 = [0; 32];
    assert!(validate_projection_metadata(&zero, [0; 32]).is_err());
    for field in 0..8 {
        let mut bad = base.clone();
        match field {
            0 => bad.symbol.push('x'),
            1 => bad.kernarg_bytes = 168,
            2 => bad.kernarg_alignment = 4,
            3 => bad.group_segment_bytes = 4,
            4 => bad.private_segment_bytes = 4,
            5 => bad.wavefront_size = 32,
            6 => bad.implicit_argument_offset = Some(160),
            _ => bad.implicit_argument_bytes = 0,
        }
        assert!(validate_projection_metadata(&bad, [7; 32]).is_err());
    }
}

#[test]
fn ordered_pair_projection_metadata_checks_all_twenty_two_roles() {
    for index in 0..22 {
        let mut bad = metadata();
        bad.explicit_arguments[index].offset += 4;
        assert!(validate_projection_metadata(&bad, [7; 32]).is_err());
        let mut bad = metadata();
        bad.explicit_arguments[index].global_buffer ^= true;
        assert!(validate_projection_metadata(&bad, [7; 32]).is_err());
    }
    let mut bad = metadata();
    bad.explicit_arguments[18].access = Some(BufferAccessV1::Read);
    assert!(validate_projection_metadata(&bad, [7; 32]).is_err());
}

#[test]
fn ordered_pair_projection_kernarg_matches_existing_consumer_bytes() {
    let bytes = projection_bytes();
    assert_eq!(bytes.len(), 424);
    for slot in 0..10 {
        assert_eq!(&bytes[slot * 16..slot * 16 + 8], &[0; 8]);
        assert_eq!(
            u64::from_le_bytes(bytes[slot * 16 + 8..slot * 16 + 16].try_into().unwrap()),
            if [0, 1, 8, 9].contains(&slot) {
                4096
            } else {
                0
            }
        );
    }
    assert_eq!(&bytes[160..168], &[1, 0, 0, 0, 2, 0, 0, 0]);
    assert!(bytes[168..].iter().all(|&v| v == 0));
}

fn token(rank: usize, id: u64, bytes: usize) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 17,
        owner: rank,
        id,
        bytes: bytes as u64,
    }
}

fn command<'a>(
    rank: usize,
    kernel: &'a Gfx950EngineeringPeerKernelV1,
    state: &'a mut Gfx950EngineeringPeerWaveMlpTilesStateV2,
) -> Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1<'a> {
    Gfx950EngineeringPeerProjectionResidualMlpTilesDispatchV1 {
        projection_kernel: kernel,
        projection_object_sha256: [7; 32],
        partials: [token(0, 30, 16_384), token(1, 31, 16_384)],
        residual_input: token(rank, 32 + rank as u64, 8192),
        mlp: Gfx950EngineeringPeerWaveMlpTilesDispatchV2 {
            kernel,
            object_sha256: [7; 32],
            roots: core::array::from_fn(|index| {
                token(
                    rank,
                    (rank * 11 + index + 1) as u64,
                    profile::EXTENTS[index],
                )
            }),
            state,
            timeout_ms: 10_000,
        },
    }
}

#[test]
fn ordered_pair_projection_fixups_have_only_two_live_partial_slots() {
    let kernel = Gfx950EngineeringPeerKernelV1 {
        group: 17,
        rank: 0,
        id: 1,
        metadata: metadata(),
    };
    let mut state = Gfx950EngineeringPeerWaveMlpTilesStateV2 {
        buffer: token(0, 11, 2192),
        activation: Activation::Ready,
    };
    let command = command(0, &kernel, &mut state);
    let pointers = projection_pointers(&command);
    assert_eq!(pointers.len(), 10);
    for (slot, pointer) in pointers.iter().enumerate() {
        assert_eq!(pointer.kernarg_offset, slot as u32 * 16);
        assert_eq!(pointer.buffer_offset, 0);
        assert_eq!(
            pointer.extent_bytes,
            match slot {
                0 | 1 => 16_384,
                8 | 9 => 8192,
                _ => 0,
            }
        );
        assert_eq!(
            pointer.access,
            if slot == 9 {
                BufferAccessV1::Write
            } else {
                BufferAccessV1::Read
            }
        );
        if slot < 8 {
            assert_eq!(
                pointer.buffer,
                command.partials[if slot < 2 { slot } else { 0 }]
            );
        }
    }
    assert_eq!(pointers[8].buffer, command.residual_input);
    assert_eq!(pointers[9].buffer, command.mlp.roots[0]);
}

#[test]
fn ordered_pair_public_entry_failure_poison_blocks_legacy_group_reuse() {
    let kernels = core::array::from_fn::<_, 2, _>(|rank| Gfx950EngineeringPeerKernelV1 {
        group: 17,
        rank,
        id: 1,
        metadata: metadata(),
    });
    let mut states =
        core::array::from_fn::<_, 2, _>(|rank| Gfx950EngineeringPeerWaveMlpTilesStateV2 {
            buffer: token(rank, (rank * 11 + 11) as u64, 2192),
            activation: Activation::Ready,
        });
    let [left, right] = &mut states;
    let commands = [
        command(0, &kernels[0], left),
        command(1, &kernels[1], right),
    ];
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 17,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: true,
        projection_mlp_scratch: None,
    };
    // SAFETY: empty contexts force rejection before any native access.
    assert!(
        unsafe { group.dispatch_projection_residual_mlp_tiles_round_unchecked_v1(commands) }
            .is_err()
    );
    assert!(group.poisoned);
    assert!(group.require_active().is_err());
    assert!(states.iter().all(|s| s.activation == Activation::Ready));
}

#[test]
fn ordered_pair_scratch_accepts_intervening_legacy_frontier_advances() {
    let scratch = scratch();
    scratch
        .require_idle(17, identities(), [4, 8], [4, 8])
        .unwrap();
    scratch
        .require_idle(17, identities(), [17, 29], [17, 29])
        .unwrap();
    assert!(
        scratch
            .require_idle(17, identities(), [3, 8], [3, 8])
            .is_err()
    );
    assert!(
        scratch
            .require_idle(17, identities(), [5, 8], [4, 8])
            .is_err()
    );
}

#[test]
fn ordered_pair_scratch_rejects_busy_poisoned_and_identity_drift() {
    let mut scratch = scratch();
    assert!(
        scratch
            .require_idle(18, identities(), [4, 8], [4, 8])
            .is_err()
    );
    for field in 0..13 {
        let mut bad = identities();
        match field {
            0 => bad[1].rank = 0,
            1 => bad[1].unique_id += 1,
            2 => bad[1].gpu_id += 1,
            3 => bad[1].queue_id = None,
            4 => bad[1].epoch += 1,
            5 => bad[1].storage.arena.0 += 4096,
            6 => bad[1].storage.arena.1 += 1,
            7 => bad[1].storage.arena.2 += 4096,
            8 => bad[1].storage.arena.3 += 4096,
            9 => bad[1].storage.signal.0 += 4096,
            10 => bad[1].storage.signal.1 += 1,
            11 => bad[1].storage.signal.2 += 4096,
            _ => bad[1].storage.signal.3 += 4096,
        }
        assert!(scratch.require_idle(17, bad, [4, 8], [4, 8]).is_err());
    }
    for phase in [ScratchPhase::Busy, ScratchPhase::Poisoned] {
        scratch.phase = phase;
        assert!(
            scratch
                .require_idle(17, identities(), [4, 8], [4, 8])
                .is_err()
        );
    }
}

#[test]
fn ordered_pair_82908_segments_keep_fixed_storage_and_retirement_metadata() {
    let mut scratch = scratch();
    let ranks = scratch.ranks;
    let bytes = core::mem::size_of_val(&scratch);
    let mut writes = [4, 8];
    for _ in 0..82_908 {
        // Legacy Prefix/final-residual packets advance both queues between uses.
        for value in &mut writes {
            *value += 3;
        }
        scratch.require_idle(17, ranks, writes, writes).unwrap();
        scratch.phase = ScratchPhase::Busy;
        for value in &mut writes {
            *value += 2;
        }
        scratch.retire(writes).unwrap();
        assert_eq!(scratch.ranks, ranks);
        assert_eq!(core::mem::size_of_val(&scratch), bytes);
    }
    assert_eq!(
        scratch.ranks[0].storage.arena.2 + scratch.ranks[1].storage.arena.2,
        2 << 20
    );
}

#[test]
fn ordered_pair_scratch_retirement_requires_busy_and_capacity_is_bounded() {
    let mut scratch = scratch();
    assert!(scratch.retire([6, 10]).is_err());
    assert!(require_sequence_capacity(u64::MAX, u64::MAX, 2).is_err());
    assert!(require_sequence_capacity(0, 0, 2).is_ok());
}
