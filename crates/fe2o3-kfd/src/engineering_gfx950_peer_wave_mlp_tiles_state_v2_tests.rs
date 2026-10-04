use super::*;

fn state(activation: Activation) -> Gfx950EngineeringPeerWaveMlpTilesStateV2 {
    Gfx950EngineeringPeerWaveMlpTilesStateV2 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3,
            owner: 0,
            bytes: 2192,
        },
        activation,
    }
}
fn record() -> BufferRecord {
    BufferRecord {
        token: state(Activation::Ready).buffer,
        local_id: 1,
        mapping: PeerMapping {
            peers: vec![],
            mapped: 0,
            unmapped: 0,
            phase: Phase::PeersMapped,
        },
        kind: BufferKind::WaveMlpTilesStateV2,
    }
}
fn group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: [(3, record())].into(),
        next_buffer: 4,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
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
    words[32..290].fill(64);
    words[290..].fill(64);
    words
}

struct Fake {
    calls: Vec<&'static str>,
    fail_at: Option<usize>,
    words: [u32; 548],
    corrupt: bool,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            calls: vec![],
            fail_at: None,
            words: [0; 548],
            corrupt: false,
        }
    }
}
impl Fake {
    fn step(&mut self, name: &'static str) -> Result<()> {
        self.calls.push(name);
        if self.fail_at == Some(self.calls.len() - 1) {
            Err("injected state failure".into())
        } else {
            Ok(())
        }
    }
}
impl StateBackend for Fake {
    fn check(&mut self) -> Result<()> {
        self.step("check")
    }
    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpTilesStateV2> {
        self.step("allocate")?;
        Ok(state(Activation::Allocated))
    }
    fn initialize(&mut self, state: &mut Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()> {
        self.step("initialize")?;
        begin_initialization(state)?;
        self.words = profile::INITIAL_STATE;
        if self.corrupt {
            self.words[547] = 1;
        }
        Ok(())
    }
    fn observe(&mut self, _: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<[u32; 548]> {
        self.step("observe")?;
        Ok(self.words)
    }
    fn rearm(&mut self, _: &Gfx950EngineeringPeerWaveMlpTilesStateV2) -> Result<()> {
        self.step("rearm")?;
        self.words = profile::INITIAL_STATE;
        if self.corrupt {
            self.words[547] = 1;
        }
        Ok(())
    }
}

#[test]
fn v2_initialization_checks_every_word_before_ready() {
    let mut fake = Fake::default();
    let token = allocate_initialized(&mut fake).unwrap();
    assert_eq!(token.activation, Activation::Ready);
    assert_eq!(fake.words, profile::INITIAL_STATE);
    assert_eq!(
        fake.calls,
        ["check", "allocate", "initialize", "observe", "check"]
    );
    let pointer = token.pointer();
    assert_eq!(
        (
            pointer.kernarg_offset,
            pointer.buffer_offset,
            pointer.extent_bytes
        ),
        (80, 0, 2192)
    );
    assert_eq!(pointer.access, BufferAccessV1::ReadWrite);
    let mut fake = Fake {
        corrupt: true,
        ..Fake::default()
    };
    assert!(allocate_initialized(&mut fake).is_err());
    assert_eq!(fake.calls.last(), Some(&"observe"));
}

#[test]
fn v2_allocation_failures_poison_and_keep_retained_records() {
    let order = ["check", "allocate", "initialize", "observe", "check"];
    for at in 0..order.len() {
        let mut fake = Fake {
            fail_at: Some(at),
            ..Fake::default()
        };
        let mut owner = group();
        assert!(owner.finish(allocate_initialized(&mut fake)).is_err());
        assert_eq!(fake.calls, order[..=at]);
        assert!(owner.poisoned);
        assert_eq!(owner.buffers.len(), 1);
        assert!(owner.allocate_wave_mlp_tiles_state_v2(0).is_err());
        assert!(
            owner
                .observe_wave_mlp_tiles_state_v2(&state(Activation::Ready))
                .is_err()
        );
        assert!(owner.close().is_err());
    }
}

#[test]
fn v2_record_refuses_old_states_foreign_identity_and_nonowner_mappings() {
    validate_state_record(&state(Activation::Ready), &record()).unwrap();
    for change in 0..14 {
        let mut value = record();
        match change {
            0 => value.kind = BufferKind::PublicVram,
            1 => value.kind = BufferKind::WaveMlpStateV1,
            2 => value.kind = BufferKind::WaveOutputStateV5,
            3 => value.token.bytes = 44,
            4 => value.token.bytes = 4096,
            5 => value.token.group += 1,
            6 => value.token.owner = 1,
            7 => value.token.id += 1,
            8 => value.mapping.peers.push(1),
            9 => value.mapping.mapped = 1,
            10 => value.mapping.unmapped = 1,
            11 => value.mapping.phase = Phase::OwnerMapped,
            12 => value.mapping.phase = Phase::Released,
            _ => value.mapping.phase = Phase::Quarantined,
        }
        assert!(
            validate_state_record(&state(Activation::Ready), &value).is_err(),
            "change {change}"
        );
    }
    assert!(require_public_vram(&record()).is_err());
    let mut owner = group();
    assert!(
        owner
            .write(state(Activation::Ready).buffer, 0, &[0; 2192])
            .is_err()
    );
    assert!(owner.poisoned);
    let mut owner = group();
    assert!(
        owner
            .read(state(Activation::Ready).buffer, 0, 2192)
            .is_err()
    );
    assert!(owner.poisoned);
}

#[test]
fn v2_allocator_enforces_group_local_bounds_and_checked_ids() {
    assert_eq!(allocation_ids(2, 1, 1, 1, 3, 4).unwrap(), (4, 5));
    for (world, owner, total, local, id, local_id) in [
        (1, 0, 0, 0, 1, 1),
        (2, 2, 0, 0, 1, 1),
        (2, 0, MAX_ALLOCATIONS * 2, 0, 1, 1),
        (2, 0, 0, MAX_ALLOCATIONS, 1, 1),
        (2, 0, 0, 0, u64::MAX, 1),
        (2, 0, 0, 0, 1, u64::MAX),
    ] {
        assert!(allocation_ids(world, owner, total, local, id, local_id).is_err());
    }
}

#[test]
fn v2_late_double_construction_and_foreign_owner_are_terminal() {
    for phase in [
        Activation::Initialized,
        Activation::Ready,
        Activation::Submitted,
        Activation::Completed,
    ] {
        assert!(begin_initialization(&mut state(phase)).is_err());
    }
    let mut fresh = state(Activation::Allocated);
    begin_initialization(&mut fresh).unwrap();
    assert!(begin_initialization(&mut fresh).is_err());
    let mut owner = group();
    assert!(owner.allocate_wave_mlp_tiles_state_v2(usize::MAX).is_err());
    assert!(owner.poisoned);
    let mut owner = group();
    let mut foreign = state(Activation::Ready);
    foreign.buffer.group += 1;
    assert!(owner.validate_token(foreign.buffer).is_err());
    assert!(owner.observe_wave_mlp_tiles_state_v2(&foreign).is_err());
    assert!(owner.poisoned);
    let mut owner = group();
    owner.buffers.clear();
    assert!(
        owner
            .validate_token(state(Activation::Ready).buffer)
            .is_err()
    );
}

#[test]
fn v2_idle_acquire_is_not_reinitialization_or_activation() {
    let mut fake = Fake {
        words: terminal(),
        ..Fake::default()
    };
    let token = state(Activation::Submitted);
    assert_eq!(observe_idle(&mut fake, &token).unwrap(), terminal());
    assert_eq!(fake.calls, ["check", "observe", "check"]);
    assert_eq!(token.activation, Activation::Submitted);
    for at in 0..3 {
        let mut fake = Fake {
            fail_at: Some(at),
            ..Fake::default()
        };
        let mut owner = group();
        assert!(owner.finish(observe_idle(&mut fake, &token)).is_err());
        assert!(owner.poisoned);
    }
}

#[test]
fn v2_rearm_needs_completed_activation_and_exact_terminal_snapshot() {
    let expected = terminal();
    for phase in [
        Activation::Allocated,
        Activation::Initialized,
        Activation::Ready,
        Activation::Submitted,
    ] {
        let mut fake = Fake {
            words: expected,
            ..Fake::default()
        };
        assert!(rearm_terminal(&mut fake, &mut state(phase), &expected).is_err());
        assert!(fake.calls.is_empty());
    }
    for index in 0..548 {
        let mut wrong = expected;
        wrong[index] = if index >= 32 && index < 290 {
            0
        } else {
            wrong[index] ^ 1
        };
        let mut fake = Fake {
            words: expected,
            ..Fake::default()
        };
        assert!(
            rearm_terminal(&mut fake, &mut state(Activation::Completed), &wrong).is_err(),
            "word {index}"
        );
        assert!(fake.calls.is_empty());
    }
    let mut stale = expected;
    stale[32] = 1;
    let mut fake = Fake {
        words: stale,
        ..Fake::default()
    };
    assert!(rearm_terminal(&mut fake, &mut state(Activation::Completed), &expected).is_err());
    assert_eq!(fake.calls, ["check", "observe"]);
}

#[test]
fn v2_rearm_uses_stores_and_publishes_ready_only_after_readback_and_fence() {
    let expected = terminal();
    let mut fake = Fake {
        words: expected,
        ..Fake::default()
    };
    let mut token = state(Activation::Completed);
    rearm_terminal(&mut fake, &mut token, &expected).unwrap();
    assert_eq!(
        fake.calls,
        ["check", "observe", "rearm", "observe", "check"]
    );
    assert_eq!(token.activation, Activation::Ready);
    assert_eq!(fake.words, profile::INITIAL_STATE);
    for at in 0..5 {
        let mut fake = Fake {
            words: expected,
            fail_at: Some(at),
            ..Fake::default()
        };
        let mut token = state(Activation::Completed);
        let mut owner = group();
        assert!(
            owner
                .finish(rearm_terminal(&mut fake, &mut token, &expected))
                .is_err()
        );
        assert!(owner.poisoned);
        assert_ne!(token.activation, Activation::Ready);
        assert_eq!(owner.buffers.len(), 1);
    }
    let mut fake = Fake {
        words: expected,
        corrupt: true,
        ..Fake::default()
    };
    let mut token = state(Activation::Completed);
    assert!(rearm_terminal(&mut fake, &mut token, &expected).is_err());
    assert_eq!(token.activation, Activation::Submitted);
}
