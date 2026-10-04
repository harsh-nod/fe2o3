use super::*;
use wave_mlp_tiles_state_v2::Activation as MlpActivation;
use wave_qkv_attention_output_tiles_state_v6::Activation as PrefixActivation;

fn prefix(
    id: u64,
    activation: PrefixActivation,
) -> Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
    Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id,
            owner: 0,
            bytes: 1136,
        },
        activation,
    }
}

fn mlp(id: u64, activation: MlpActivation) -> Gfx950EngineeringPeerWaveMlpTilesStateV2 {
    Gfx950EngineeringPeerWaveMlpTilesStateV2 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id,
            owner: 0,
            bytes: 2192,
        },
        activation,
    }
}

fn owner() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: Default::default(),
        next_buffer: 200,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
    }
}

fn insert_record(
    group: &mut Gfx950EngineeringPeerGroupV1,
    token: Gfx950EngineeringPeerBufferV1,
    kind: BufferKind,
) {
    group.buffers.insert(
        token.id,
        BufferRecord {
            token,
            local_id: token.id,
            mapping: PeerMapping {
                peers: vec![],
                mapped: 0,
                unmapped: 0,
                phase: Phase::PeersMapped,
            },
            kind,
        },
    );
}

#[derive(Default)]
struct Fake {
    calls: Vec<String>,
    fail_at: Option<usize>,
}

impl Fake {
    fn step(&mut self, label: String) -> Result<()> {
        self.calls.push(label);
        if self.fail_at == Some(self.calls.len() - 1) {
            Err("injected bank failure".into())
        } else {
            Ok(())
        }
    }
}

impl BankBackend for Fake {
    fn check(&mut self) -> Result<()> {
        self.step("check".into())
    }

    fn validate(&mut self, entry: Gfx950EngineeringPeerStateBankEntryV1<'_>) -> Result<()> {
        self.step(format!("validate:{}", entry.token().id))
    }

    fn observe(
        &mut self,
        entry: Gfx950EngineeringPeerStateBankEntryV1<'_>,
    ) -> Result<Gfx950EngineeringPeerStateBankSnapshotV1> {
        self.step(format!("observe:{}", entry.token().id))?;
        let word = entry.token().id as u32;
        Ok(match entry {
            Gfx950EngineeringPeerStateBankEntryV1::Prefix(_) => {
                Gfx950EngineeringPeerStateBankSnapshotV1::Prefix([word; 284])
            }
            Gfx950EngineeringPeerStateBankEntryV1::Mlp(_) => {
                Gfx950EngineeringPeerStateBankSnapshotV1::Mlp([word; 548])
            }
        })
    }
}

#[test]
fn bank_mixed_order_whole_validation_then_reads_and_two_fresh_checks() {
    let p = prefix(1, PrefixActivation::Completed);
    let m = mlp(2, MlpActivation::Ready);
    let entries = [
        Gfx950EngineeringPeerStateBankEntryV1::Mlp(&m),
        Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p),
    ];
    let mut fake = Fake::default();
    let snapshots = observe_bank(&mut fake, &entries).unwrap();
    assert_eq!(
        snapshots,
        vec![
            Gfx950EngineeringPeerStateBankSnapshotV1::Mlp([2; 548]),
            Gfx950EngineeringPeerStateBankSnapshotV1::Prefix([1; 284])
        ]
    );
    assert_eq!(
        fake.calls,
        [
            "check",
            "validate:2",
            "validate:1",
            "observe:2",
            "observe:1",
            "check"
        ]
    );
    assert_eq!(p.activation, PrefixActivation::Completed);
    assert_eq!(m.activation, MlpActivation::Ready);
}

#[test]
fn bank_bounds_one_and144_accept_empty_and145_refuse_before_backend() {
    let states: Vec<_> = (1..=145)
        .map(|id| prefix(id, PrefixActivation::Ready))
        .collect();
    let entries: Vec<_> = states
        .iter()
        .map(Gfx950EngineeringPeerStateBankEntryV1::Prefix)
        .collect();
    for count in [1, 144] {
        let mut fake = Fake::default();
        assert_eq!(
            observe_bank(&mut fake, &entries[..count]).unwrap().len(),
            count
        );
        assert_eq!(fake.calls.iter().filter(|call| *call == "check").count(), 2);
        assert_eq!(fake.calls.len(), count * 2 + 2);
    }
    for count in [0, 145] {
        let mut fake = Fake::default();
        assert!(observe_bank(&mut fake, &entries[..count]).is_err());
        assert!(fake.calls.is_empty());
    }
}

#[test]
fn bank_duplicates_refuse_before_any_load_and_quarantine() {
    let p = prefix(1, PrefixActivation::Ready);
    let m = mlp(1, MlpActivation::Completed);
    for entry in [
        Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p),
        Gfx950EngineeringPeerStateBankEntryV1::Mlp(&m),
    ] {
        let mut fake = Fake::default();
        let mut group = owner();
        let entries = [Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p), entry];
        assert_eq!(
            group.finish(observe_bank(&mut fake, &entries)).unwrap_err(),
            "duplicate state bank identity"
        );
        assert_eq!(fake.calls, ["check", "validate:1", "validate:1"]);
        assert_eq!(
            group.require_active().unwrap_err(),
            "peer group is closed or quarantined"
        );
    }
}

#[test]
fn bank_first_middle_last_validation_read_and_both_fence_failures_return_no_vector() {
    let states: Vec<_> = (1..=144)
        .map(|id| prefix(id, PrefixActivation::Ready))
        .collect();
    let entries: Vec<_> = states
        .iter()
        .map(Gfx950EngineeringPeerStateBankEntryV1::Prefix)
        .collect();
    let mut success = Fake::default();
    observe_bank(&mut success, &entries).unwrap();
    for at in [0, 1, 72, 144, 145, 216, 288, 289] {
        let mut fake = Fake {
            fail_at: Some(at),
            ..Fake::default()
        };
        let mut group = owner();
        assert_eq!(
            group.finish(observe_bank(&mut fake, &entries)).unwrap_err(),
            "injected bank failure"
        );
        assert_eq!(fake.calls, success.calls[..=at]);
        assert!(group.poisoned);
        assert_eq!(
            group.observe_state_bank_v1(&entries).unwrap_err(),
            "peer group is closed or quarantined"
        );
        assert_eq!(
            group.require_active().unwrap_err(),
            "peer group is closed or quarantined"
        );
        assert!(
            states
                .iter()
                .all(|state| state.activation == PrefixActivation::Ready)
        );
    }
}

#[test]
fn bank_repeated_calls_take_new_fences_and_later_failure_quarantines() {
    let state = mlp(1, MlpActivation::Completed);
    let entries = [Gfx950EngineeringPeerStateBankEntryV1::Mlp(&state)];
    let mut fake = Fake::default();
    let mut group = owner();
    assert!(group.finish(observe_bank(&mut fake, &entries)).is_ok());
    assert!(group.finish(observe_bank(&mut fake, &entries)).is_ok());
    assert_eq!(
        fake.calls,
        [
            "check",
            "validate:1",
            "observe:1",
            "check",
            "check",
            "validate:1",
            "observe:1",
            "check"
        ]
    );
    fake.fail_at = Some(fake.calls.len());
    assert!(group.finish(observe_bank(&mut fake, &entries)).is_err());
    assert_eq!(
        group.require_active().unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(state.activation, MlpActivation::Completed);
}

#[test]
fn bank_public_bounds_error_poison_and_public_single_fences_remain_guarded() {
    let mut group = owner();
    let p = prefix(1, PrefixActivation::Ready);
    let m = mlp(2, MlpActivation::Ready);
    assert_eq!(
        group.observe_state_bank_v1(&[]).unwrap_err(),
        "state bank requires 1..=144 entries"
    );
    assert_eq!(
        group
            .observe_wave_qkv_attention_output_tiles_state_v6(&p)
            .unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(
        group.observe_wave_mlp_tiles_state_v2(&m).unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert!(group.close().is_err());
}

#[test]
fn bank_native_routing_accepts_completed_lifetime_but_refuses_missing_owner_storage() {
    for activation in [
        PrefixActivation::Ready,
        PrefixActivation::Submitted,
        PrefixActivation::Completed,
    ] {
        let p = prefix(1, activation);
        let mut group = owner();
        insert_record(
            &mut group,
            p.buffer,
            BufferKind::WaveQkvAttentionOutputTilesStateV6,
        );
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p);
        let result = NativeBank { group: &mut group }.validate(entry);
        assert_eq!(
            result.unwrap_err(),
            "prefix tiles V6 state owner outside group"
        );
        // This fixture cannot reach mapped storage: native routing must reject
        // its missing owner before an atomic load or any hardware interaction.
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_eq!(
            group.finish(result).unwrap_err(),
            "prefix tiles V6 state owner outside group"
        );
        assert!(group.poisoned);
        assert_eq!(p.activation, activation);
    }
    for activation in [
        MlpActivation::Ready,
        MlpActivation::Submitted,
        MlpActivation::Completed,
    ] {
        let m = mlp(2, activation);
        let mut group = owner();
        insert_record(&mut group, m.buffer, BufferKind::WaveMlpTilesStateV2);
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Mlp(&m);
        let result = NativeBank { group: &mut group }.validate(entry);
        assert_eq!(
            result.unwrap_err(),
            "MLP tiles V2 state owner outside group"
        );
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_eq!(
            group.finish(result).unwrap_err(),
            "MLP tiles V2 state owner outside group"
        );
        assert!(group.poisoned);
        assert_eq!(m.activation, activation);
    }
}

#[test]
fn bank_native_rejects_unconstructed_lifetimes_without_storage_access() {
    for activation in [PrefixActivation::Allocated, PrefixActivation::Initialized] {
        let p = prefix(1, activation);
        let mut group = owner();
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p);
        assert_eq!(
            NativeBank { group: &mut group }
                .validate(entry)
                .unwrap_err(),
            "prefix tiles V6 idle bank observation activation"
        );
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_eq!(
            group.finish(result).unwrap_err(),
            "prefix tiles V6 idle bank observation activation"
        );
        assert!(group.poisoned);
    }
    for activation in [MlpActivation::Allocated, MlpActivation::Initialized] {
        let m = mlp(2, activation);
        let mut group = owner();
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Mlp(&m);
        assert_eq!(
            NativeBank { group: &mut group }
                .validate(entry)
                .unwrap_err(),
            "MLP tiles V2 idle bank observation activation"
        );
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_eq!(
            group.finish(result).unwrap_err(),
            "MLP tiles V2 idle bank observation activation"
        );
        assert!(group.poisoned);
    }
}

#[test]
fn bank_native_prefix_foreign_stale_kind_extent_and_mapping_checks_precede_storage() {
    for change in 0..10 {
        let mut p = prefix(1, PrefixActivation::Completed);
        let mut group = owner();
        insert_record(
            &mut group,
            p.buffer,
            BufferKind::WaveQkvAttentionOutputTilesStateV6,
        );
        match change {
            0 => group.incarnation += 1,
            1 => p.buffer.id += 1,
            2 => p.buffer.owner += 1,
            3 => {
                p.buffer.bytes += 4;
                group.buffers.get_mut(&1).unwrap().token = p.buffer;
            }
            4 => group.buffers.get_mut(&1).unwrap().kind = BufferKind::WaveMlpTilesStateV2,
            5 => group.buffers.get_mut(&1).unwrap().mapping.peers.push(1),
            6 => group.buffers.get_mut(&1).unwrap().mapping.mapped = 1,
            7 => group.buffers.get_mut(&1).unwrap().mapping.unmapped = 1,
            8 => group.buffers.get_mut(&1).unwrap().mapping.phase = Phase::Released,
            _ => group.buffers.clear(),
        }
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Prefix(&p);
        let result = NativeBank { group: &mut group }.validate(entry);
        assert_ne!(
            result.unwrap_err(),
            "prefix tiles V6 state owner outside group",
            "change {change}"
        );
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_ne!(
            group.finish(result).unwrap_err(),
            "prefix tiles V6 state owner outside group",
            "change {change}"
        );
        assert_eq!(
            group.require_active().unwrap_err(),
            "peer group is closed or quarantined"
        );
        assert_eq!(p.activation, PrefixActivation::Completed);
    }
}

#[test]
fn bank_native_mlp_foreign_stale_kind_extent_and_mapping_checks_precede_storage() {
    for change in 0..10 {
        let mut m = mlp(2, MlpActivation::Completed);
        let mut group = owner();
        insert_record(&mut group, m.buffer, BufferKind::WaveMlpTilesStateV2);
        match change {
            0 => group.incarnation += 1,
            1 => m.buffer.id += 1,
            2 => m.buffer.owner += 1,
            3 => {
                m.buffer.bytes += 4;
                group.buffers.get_mut(&2).unwrap().token = m.buffer;
            }
            4 => {
                group.buffers.get_mut(&2).unwrap().kind =
                    BufferKind::WaveQkvAttentionOutputTilesStateV6
            }
            5 => group.buffers.get_mut(&2).unwrap().mapping.peers.push(1),
            6 => group.buffers.get_mut(&2).unwrap().mapping.mapped = 1,
            7 => group.buffers.get_mut(&2).unwrap().mapping.unmapped = 1,
            8 => group.buffers.get_mut(&2).unwrap().mapping.phase = Phase::Released,
            _ => group.buffers.clear(),
        }
        let entry = Gfx950EngineeringPeerStateBankEntryV1::Mlp(&m);
        let result = NativeBank { group: &mut group }.validate(entry);
        assert_ne!(
            result.unwrap_err(),
            "MLP tiles V2 state owner outside group",
            "change {change}"
        );
        let result = NativeBank { group: &mut group }.observe(entry);
        assert_ne!(
            group.finish(result).unwrap_err(),
            "MLP tiles V2 state owner outside group",
            "change {change}"
        );
        assert_eq!(
            group.require_active().unwrap_err(),
            "peer group is closed or quarantined"
        );
        assert_eq!(m.activation, MlpActivation::Completed);
    }
}
