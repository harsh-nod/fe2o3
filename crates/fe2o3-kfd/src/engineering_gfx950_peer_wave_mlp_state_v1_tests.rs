use super::*;

fn token() -> Gfx950EngineeringPeerWaveMlpStateV1 {
    Gfx950EngineeringPeerWaveMlpStateV1 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3,
            owner: 0,
            bytes: 44,
        },
    }
}

fn record() -> BufferRecord {
    BufferRecord {
        token: token().buffer,
        local_id: 1,
        mapping: PeerMapping {
            peers: vec![],
            mapped: 0,
            unmapped: 0,
            phase: Phase::PeersMapped,
        },
        kind: BufferKind::WaveMlpStateV1,
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
        projection_mlp_scratch: None,
    }
}

#[derive(Default)]
struct Fake {
    calls: Vec<&'static str>,
    fail_at: Option<usize>,
    initialized: bool,
    corrupt_initialization: bool,
    words: [u32; 11],
}

impl Fake {
    fn step(&mut self, call: &'static str) -> Result<()> {
        self.calls.push(call);
        if self.fail_at == Some(self.calls.len() - 1) {
            return Err("injected state uncertainty".into());
        }
        Ok(())
    }
}

impl StateBackend for Fake {
    fn check(&mut self) -> Result<()> {
        self.step("check")
    }

    fn allocate(&mut self) -> Result<Gfx950EngineeringPeerWaveMlpStateV1> {
        self.step("allocate")?;
        Ok(token())
    }

    fn initialize(&mut self, _: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<()> {
        self.step("initialize")?;
        assert!(!self.initialized);
        self.initialized = true;
        self.words = initial_state();
        if self.corrupt_initialization {
            self.words[10] = 1;
        }
        Ok(())
    }

    fn observe(&mut self, _: &Gfx950EngineeringPeerWaveMlpStateV1) -> Result<[u32; 11]> {
        self.step("observe")?;
        assert!(self.initialized);
        Ok(self.words)
    }
}

#[test]
fn state_initialization_is_checked_before_token_publication() {
    let mut backend = Fake::default();
    let state = allocate_initialized(&mut backend).unwrap();
    assert_eq!(state.buffer, token().buffer);
    assert_eq!(
        backend.calls,
        ["check", "allocate", "initialize", "observe", "check"]
    );
    assert_eq!(backend.words, [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn every_allocation_stage_error_is_terminal_and_does_not_publish_a_state() {
    let order = ["check", "allocate", "initialize", "observe", "check"];
    for fail_at in 0..order.len() {
        let mut backend = Fake {
            fail_at: Some(fail_at),
            ..Fake::default()
        };
        let mut owner = group();
        assert!(owner.finish(allocate_initialized(&mut backend)).is_err());
        assert_eq!(backend.calls, order[..=fail_at]);
        assert!(owner.poisoned);
        assert!(owner.allocate_wave_mlp_state_v1(0).is_err());
        assert!(owner.observe_wave_mlp_state_v1(&token()).is_err());
        assert!(owner.close().is_err());
    }
}

#[test]
fn mismatched_initial_words_are_not_accepted_as_fresh() {
    let mut backend = Fake {
        corrupt_initialization: true,
        ..Fake::default()
    };
    assert!(allocate_initialized(&mut backend).is_err());
    assert_eq!(
        backend.calls,
        ["check", "allocate", "initialize", "observe"]
    );
}

#[test]
fn idle_observation_preserves_all_words_and_never_reinitializes() {
    let words = core::array::from_fn(|index| 0x1000 + index as u32);
    let mut backend = Fake {
        initialized: true,
        words,
        ..Fake::default()
    };
    assert_eq!(observe_idle(&mut backend, &token()).unwrap(), words);
    assert_eq!(backend.calls, ["check", "observe", "check"]);
    assert_eq!(backend.words, words);
    for fail_at in 0..3 {
        let mut backend = Fake {
            initialized: true,
            words,
            fail_at: Some(fail_at),
            ..Fake::default()
        };
        let mut owner = group();
        assert!(owner.finish(observe_idle(&mut backend, &token())).is_err());
        assert_eq!(backend.calls, ["check", "observe", "check"][..=fail_at]);
        assert!(owner.poisoned);
    }
}

#[test]
fn state_has_only_the_fixed_owner_root_ten_binding() {
    let state = token();
    let pointer = state.pointer_mlp_v1();
    assert_eq!(state.owner_rank(), 0);
    assert_eq!(pointer.buffer, state.buffer);
    assert_eq!(pointer.kernarg_offset, 80);
    assert_eq!(pointer.buffer_offset, 0);
    assert_eq!(pointer.extent_bytes, 44);
    assert_eq!(pointer.access, BufferAccessV1::ReadWrite);
    for access in [
        BufferAccessV1::Read,
        BufferAccessV1::Write,
        BufferAccessV1::ReadWrite,
    ] {
        assert!(require_peer_access(0, 1, 9, &[], access).is_err());
    }
}

#[test]
fn state_provenance_rejects_public_vram_extent_mapping_phase_and_identity_changes() {
    validate_state_record(&token(), &record()).unwrap();
    for mutation in 0..11 {
        let mut changed = record();
        match mutation {
            0 => changed.kind = BufferKind::PublicVram,
            1 => changed.token.bytes = 80,
            2 => changed.token.group = 8,
            3 => changed.token.id = 4,
            4 => changed.token.owner = 1,
            5 => changed.mapping.peers.push(9),
            6 => changed.mapping.mapped = 1,
            7 => changed.mapping.unmapped = 1,
            8 => changed.mapping.phase = Phase::OwnerMapped,
            9 => changed.mapping.phase = Phase::Quarantined,
            10 => changed.kind = BufferKind::WaveOutputStateV5,
            _ => unreachable!(),
        }
        assert!(
            validate_state_record(&token(), &changed).is_err(),
            "mutation {mutation}"
        );
    }
    let mut ordinary = record();
    ordinary.kind = BufferKind::PublicVram;
    require_public_vram(&ordinary).unwrap();
    assert!(require_public_vram(&record()).is_err());
}

#[test]
fn generic_host_access_cannot_touch_state_even_with_an_internal_token() {
    let mut owner = group();
    assert!(owner.write(token().buffer, 0, &[0; 44]).is_err());
    assert!(owner.poisoned);
    let mut owner = group();
    assert!(owner.read(token().buffer, 0, 44).is_err());
    assert!(owner.poisoned);
}

#[test]
fn invalid_owner_and_foreign_state_fail_without_native_access() {
    let mut owner = group();
    assert!(owner.allocate_wave_mlp_state_v1(usize::MAX).is_err());
    assert!(owner.poisoned);
    let mut owner = group();
    let mut foreign = token();
    foreign.buffer.group += 1;
    assert!(owner.validate_token(foreign.buffer).is_err());
    assert!(owner.observe_wave_mlp_state_v1(&foreign).is_err());
    assert!(owner.poisoned);
}
