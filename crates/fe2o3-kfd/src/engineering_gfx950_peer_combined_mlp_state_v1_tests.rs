use super::*;

#[path = "engineering_gfx950_peer_combined_mlp_native_v1_tests.rs"]
mod native;

fn state(activation: Activation, generation: u64) -> CombinedMlpStateV1 {
    CombinedMlpStateV1 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            group: 7,
            id: 3,
            owner: 0,
            bytes: 2208,
        },
        activation,
        generation,
    }
}

fn record() -> BufferRecord {
    BufferRecord {
        token: state(Activation::Ready, 1).buffer,
        local_id: 1,
        mapping: PeerMapping {
            peers: vec![22],
            mapped: 1,
            unmapped: 0,
            phase: Phase::PeersMapped,
        },
        kind: BufferKind::CombinedMlpStateV1,
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

fn terminal(generation: u64) -> CombinedMlpSnapshotV1 {
    let mut prefix = [0; 548];
    prefix[0] = 1;
    prefix[3] = 31;
    prefix[4..9].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[9..14].copy_from_slice(&[1, 96, 96, 1, 64]);
    prefix[14..22].fill(u32::MAX);
    prefix[22] = 3;
    prefix[23..31].fill(u32::MAX);
    prefix[31] = 3;
    prefix[32..548].fill(64);
    CombinedMlpSnapshotV1 {
        prefix,
        guard: guard_words(generation, 1),
    }
}

const PHASES: [Activation; 7] = [
    Activation::Allocated,
    Activation::Initialized,
    Activation::Ready,
    Activation::Submitted,
    Activation::Completed,
    Activation::Rearming,
    Activation::Poisoned,
];

#[test]
fn combined_binding_matrix_admits_only_exact_owner_and_suffix_regions() {
    let record = record();
    let mut accepted = 0;
    for rank in [0, 1, 2, usize::MAX] {
        for arg in [0, 16, 64, 80, 96] {
            for offset in [0, 1, 2191, 2192, 2193, u64::MAX] {
                for extent in [0, 4, 15, 16, 2192, 2208, 2209, u64::MAX] {
                    for access in [
                        BufferAccessV1::Read,
                        BufferAccessV1::Write,
                        BufferAccessV1::ReadWrite,
                    ] {
                        let pointer = record.token.pointer(arg, offset, extent, access);
                        let expected = rank < 2
                            && ((offset == 2192
                                && extent == 16
                                && access == BufferAccessV1::Read
                                && matches!(arg, 64 | 80))
                                || (rank == 0
                                    && offset == 0
                                    && access == BufferAccessV1::ReadWrite
                                    && ((arg == 80 && extent == 2192)
                                        || (arg == 0 && extent == 2208))));
                        assert_eq!(validate_pointer(&record, &pointer, rank).is_ok(), expected);
                        accepted += usize::from(expected);
                    }
                }
            }
        }
    }
    assert_eq!(accepted, 6);
    let mut wrong_extent = record;
    wrong_extent.token.bytes = 2192;
    let pointer = wrong_extent
        .token
        .pointer(0, 0, 2208, BufferAccessV1::ReadWrite);
    assert!(validate_pointer(&wrong_extent, &pointer, 0).is_err());
}

#[test]
fn combined_borrowed_regions_preserve_one_real_allocation_identity() {
    for owner in 0..2 {
        let mut state = state(Activation::Ready, 1);
        state.buffer.owner = owner;
        let regions = CombinedMlpRegionsV1 { state: &state };
        let prefix = regions.mlp_prefix(owner).unwrap();
        let whole = regions.validator(owner).unwrap();
        assert!(regions.mlp_prefix(1 - owner).is_err());
        assert!(regions.validator(1 - owner).is_err());
        for rank in 0..2 {
            for arg in [64, 80] {
                let guard = regions.r2_guard(rank, arg).unwrap();
                for pointer in [&prefix, &whole, &guard] {
                    assert_eq!(pointer.buffer, state.buffer);
                    assert_eq!(pointer.buffer.bytes, 2208);
                }
                assert_eq!(
                    (guard.buffer_offset, guard.extent_bytes, guard.access),
                    (2192, 16, BufferAccessV1::Read)
                );
            }
        }
        assert!(regions.r2_guard(2, 64).is_err());
        assert!(regions.r2_guard(0, 0).is_err());
    }
}

#[test]
fn combined_owner_does_not_change_legacy_token_or_public_byte_policy() {
    let record = record();
    assert!(require_public_vram(&record).is_err());
    assert!(require_peer_access(0, 1, 22, &[22], BufferAccessV1::Read).is_ok());
    assert!(require_peer_access(0, 1, 22, &[22], BufferAccessV1::ReadWrite).is_err());
    assert!(require_peer_access(0, 1, 23, &[22], BufferAccessV1::Read).is_err());
    let legacy = wave_mlp_tiles_state_v2::Gfx950EngineeringPeerWaveMlpTilesStateV2 {
        buffer: Gfx950EngineeringPeerBufferV1 {
            bytes: 2192,
            ..record.token
        },
        activation: wave_mlp_tiles_state_v2::Activation::Ready,
    };
    let pointer = legacy.pointer();
    assert_eq!(
        (
            pointer.buffer.bytes,
            pointer.buffer_offset,
            pointer.extent_bytes
        ),
        (2192, 0, 2192)
    );
    assert_eq!(pointer.access, BufferAccessV1::ReadWrite);
    assert_eq!(wave_mlp_tiles_state_v2::STATE_BYTES, 2192);
    assert_eq!(profile::EXTENTS[10], 2192);
}

#[test]
fn combined_snapshot_predicates_check_every_prefix_word_and_guard_component() {
    let generation = 0x1234_5678_9abc_def0;
    let initial = CombinedMlpSnapshotV1 {
        prefix: profile::INITIAL_STATE,
        guard: guard_words(generation, 0),
    };
    let valid = terminal(generation);
    require_initial(&initial, generation).unwrap();
    require_terminal(&valid, generation).unwrap();
    for index in 0..548 {
        let mut changed = initial.clone();
        changed.prefix[index] ^= 1;
        assert!(require_initial(&changed, generation).is_err());
        let mut changed = valid.clone();
        changed.prefix[index] ^= 1;
        assert!(require_terminal(&changed, generation).is_err());
    }
    for index in 0..4 {
        let mut changed = initial.clone();
        changed.guard[index] ^= 1;
        assert!(require_initial(&changed, generation).is_err());
        let mut changed = valid.clone();
        changed.guard[index] ^= 1;
        assert!(require_terminal(&changed, generation).is_err());
    }
    for verdict in [0, 2, 3, u32::MAX] {
        let mut changed = valid.clone();
        changed.guard[2] = verdict;
        assert!(require_terminal(&changed, generation).is_err());
    }
    assert!(require_terminal(&valid, generation + (1 << 32)).is_err());
    assert!(require_terminal(&valid, generation + 1).is_err());
    assert!(require_initial(&initial, 0).is_err());
    assert!(require_terminal(&terminal(0), 0).is_err());
}

#[test]
fn combined_generation_and_activation_rules_refuse_replay_skip_and_overflow() {
    for phase in PHASES {
        for previous in [
            1,
            u32::MAX as u64,
            (u32::MAX as u64) + 1,
            u64::MAX - 1,
            u64::MAX,
        ] {
            for next in [
                0,
                1,
                previous,
                previous.wrapping_add(1),
                previous.wrapping_add(2),
            ] {
                let expected =
                    phase == Activation::Completed && previous.checked_add(1) == Some(next);
                assert_eq!(require_rearm(phase, previous, next).is_ok(), expected);
            }
        }
        for expected in PHASES {
            assert_eq!(
                require_activation(phase, expected).is_ok(),
                phase == expected
            );
        }
    }
    assert!(require_rearm(Activation::Completed, 0, 1).is_err());
    assert_eq!(guard_words(1 << 32, 0), [0, 1, 0, 0]);
    assert_eq!(guard_words(u64::MAX, 1), [u32::MAX, u32::MAX, 1, 0]);
}

#[test]
fn combined_native_submit_refuses_bad_phase_before_any_context_operation() {
    for phase in PHASES
        .into_iter()
        .filter(|phase| *phase != Activation::Ready)
    {
        let mut state = state(phase, 1);
        let mut group = group();
        let before = group.buffers[&3].token;
        assert_eq!(
            state.submit(&mut group).unwrap_err(),
            "combined MLP activation mismatch"
        );
        assert_eq!(state.activation, Activation::Poisoned);
        assert!(group.poisoned);
        assert_eq!(group.buffers[&3].token, before);
        assert!(group.contexts.is_empty());
    }
}

#[test]
fn combined_terminal_failures_poison_without_publishing_generation_or_losing_custody() {
    for phase in PHASES {
        let mut state = state(phase, 7);
        let mut group = group();
        assert!(
            state
                .outcome::<()>(&mut group, Err("injected backend failure".into()))
                .is_err()
        );
        assert_eq!(state.activation, Activation::Poisoned);
        assert_eq!(state.generation(), 7);
        assert_eq!(state.owner_rank(), 0);
        assert!(group.poisoned);
        assert!(group.require_active().is_err());
        assert_eq!(group.buffers.len(), 1);
        assert_eq!(group.buffers[&3].token, state.buffer);
        drop(state);
        assert_eq!(group.buffers.len(), 1);
    }
    let mut state = state(Activation::Completed, 7);
    let mut group = group();
    state.outcome(&mut group, Ok(())).unwrap();
    assert!(!group.poisoned);
    assert_eq!(state.activation, Activation::Completed);
    assert_eq!(state.generation(), 7);
}

#[test]
fn combined_cleanup_selects_queue_first_and_keeps_existing_kinds_unchanged() {
    let mut group = group();
    assert!(group.has_peer_dependency_arena());
    for kind in [
        BufferKind::PublicVram,
        BufferKind::WaveMlpTilesStateV2,
        BufferKind::PeerDependencyArena,
    ] {
        group.buffers.get_mut(&3).unwrap().kind = kind;
        assert_eq!(
            group.has_peer_dependency_arena(),
            kind == BufferKind::PeerDependencyArena
        );
    }
    group.buffers.get_mut(&3).unwrap().kind = BufferKind::CombinedMlpStateV1;
    assert!(group.has_peer_dependency_arena());
}
