use super::*;

fn metadata() -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: profile::SYMBOL.into(),
        object_sha256: [7; 32],
        kernarg_bytes: 376,
        kernarg_alignment: 8,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(120),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..15)
            .map(|index| crate::engineering_wire::ExplicitArgumentV1 {
                offset: index * 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(profile::role_alignment(index as usize)),
                access: Some(profile::role_access(index as usize)),
            })
            .collect(),
    }
}
fn kernels() -> [Gfx950EngineeringPeerKernelV1; 2] {
    core::array::from_fn(|rank| Gfx950EngineeringPeerKernelV1 {
        group: 7,
        rank,
        id: 1,
        metadata: metadata(),
    })
}
fn root(rank: usize, index: usize) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id: (rank * 15 + index + 1) as u64,
        owner: rank,
        bytes: profile::EXTENTS[index] as u64,
    }
}
fn make_states() -> [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2] {
    core::array::from_fn(
        |rank| Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6 {
            buffer: root(rank, 14),
            activation: Activation::Ready,
        },
    )
}
fn commands<'a>(
    kernels: &'a [Gfx950EngineeringPeerKernelV1; 2],
    states: &'a mut [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6; 2],
) -> [Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'a>; 2] {
    let [left, right] = states;
    [
        Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6 {
            kernel: &kernels[0],
            object_sha256: [7; 32],
            roots: core::array::from_fn(|index| root(0, index)),
            state: left,
            timeout_ms: 10_000,
        },
        Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6 {
            kernel: &kernels[1],
            object_sha256: [7; 32],
            roots: core::array::from_fn(|index| root(1, index)),
            state: right,
            timeout_ms: 10_000,
        },
    ]
}
fn regions(rank: usize) -> [profile::OwnedRegion; 15] {
    core::array::from_fn(|index| profile::OwnedRegion {
        buffer: root(rank, index).id,
        base: 0x1000_0000 + rank as u64 * 0x1_0000_0000 + index as u64 * 0x1000_0000,
        requested: profile::EXTENTS[index],
        backing: profile::EXTENTS[index].div_ceil(4096) * 4096,
    })
}
fn terminal() -> [u32; 284] {
    let mut words = [0; 284];
    words[0] = 1;
    words[3] = 31;
    words[4..9].copy_from_slice(&[1, 48, 1, 16, 64]);
    words[9..14].copy_from_slice(&[1, 48, 1, 16, 64]);
    words[14..18].fill(u32::MAX);
    words[18] = 3;
    words[19..23].fill(u32::MAX);
    words[23] = 3;
    words[24..154].fill(1);
    words[154..284].fill(64);
    words
}
fn group() -> Gfx950EngineeringPeerGroupV1 {
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

struct Fake {
    calls: Vec<String>,
    fail_at: Option<usize>,
    completed: bool,
    world: usize,
    initial: [[u32; 284]; 2],
    final_words: [[u32; 284]; 2],
    regions: [[profile::OwnedRegion; 15]; 2],
    durations: Vec<u64>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            calls: vec![],
            fail_at: None,
            completed: false,
            world: 2,
            initial: [profile::INITIAL_STATE; 2],
            final_words: [terminal(); 2],
            regions: [regions(0), regions(1)],
            durations: vec![123, 456],
        }
    }
}
impl Fake {
    fn step(&mut self, name: impl Into<String>) -> Result<()> {
        self.calls.push(name.into());
        if self.fail_at == Some(self.calls.len() - 1) {
            Err("injected resident uncertainty".into())
        } else {
            Ok(())
        }
    }
}
impl ResidentBackend for Fake {
    fn check(&mut self) -> Result<()> {
        self.step("check")
    }
    fn validate(
        &mut self,
        rank: usize,
        command: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesDispatchV6<'_>,
    ) -> Result<[profile::OwnedRegion; 15]> {
        self.step(format!("validate{rank}"))?;
        validate_command_identity(7, self.world, rank, command, &metadata())?;
        Ok(self.regions[rank])
    }
    fn observe(
        &mut self,
        state: &Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6,
    ) -> Result<[u32; 284]> {
        self.step(format!("observe{}", state.owner_rank()))?;
        Ok(if self.completed {
            self.final_words[state.owner_rank()]
        } else {
            self.initial[state.owner_rank()]
        })
    }
    fn submit_and_complete(
        &mut self,
        commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
    ) -> Result<Vec<u64>> {
        assert_eq!(commands.len(), 2);
        for (rank, command) in commands.iter().enumerate() {
            assert_eq!(command.kernel.rank, rank);
            assert_eq!(command.grid, [4096, 1, 1]);
            assert_eq!(command.workgroup, [64, 1, 1]);
            assert_eq!(command.bytes, vec![0; 376]);
            assert_eq!(command.pointers.len(), 15);
            for (index, pointer) in command.pointers.iter().enumerate() {
                assert_eq!(pointer.buffer, root(rank, index));
                assert_eq!(
                    (
                        pointer.kernarg_offset,
                        pointer.buffer_offset,
                        pointer.extent_bytes
                    ),
                    (index as u32 * 8, 0, profile::EXTENTS[index] as u64)
                );
                assert_eq!(pointer.access, profile::role_access(index));
            }
        }
        // A faultable adapter at the existing publisher boundary. The original
        // peer_round tests independently cover its actual prepare/publish loop.
        for name in [
            "prepare0",
            "prepare1",
            "publish0",
            "publish1",
            "complete0",
            "complete1",
        ] {
            self.step(name)?;
        }
        self.completed = true;
        Ok(self.durations.clone())
    }
}

#[test]
fn resident_v6_dispatch_binds_exact_geometry_roots_and_completion_before_state() {
    let kernels = kernels();
    let mut states = make_states();
    let mut fake = Fake::default();
    let observed = run_resident(&mut fake, commands(&kernels, &mut states)).unwrap();
    assert_eq!(observed.final_states, [terminal(); 2]);
    assert_eq!(observed.dispatch_elapsed_ns, [123, 456]);
    assert!(
        states
            .iter()
            .all(|state| state.activation == Activation::Completed)
    );
    assert_eq!(
        fake.calls,
        [
            "check",
            "validate0",
            "validate1",
            "observe0",
            "observe1",
            "check",
            "prepare0",
            "prepare1",
            "publish0",
            "publish1",
            "complete0",
            "complete1",
            "check",
            "observe0",
            "observe1",
            "check"
        ]
    );
    assert!(run_resident(&mut Fake::default(), commands(&kernels, &mut states)).is_err());
}

#[test]
fn resident_v6_every_failure_poison_blocks_rearm_close_and_second_dispatch() {
    let kernels = kernels();
    for at in 0..16 {
        let mut states = make_states();
        let mut fake = Fake {
            fail_at: Some(at),
            ..Fake::default()
        };
        let mut owner = group();
        assert!(
            owner
                .finish(run_resident(&mut fake, commands(&kernels, &mut states)))
                .is_err(),
            "step {at}"
        );
        assert_eq!(fake.calls.len(), at + 1);
        assert!(owner.poisoned);
        assert!(
            states
                .iter()
                .all(|state| state.activation != Activation::Completed)
        );
        if at >= 6 {
            assert!(
                states
                    .iter()
                    .all(|state| state.activation == Activation::Submitted)
            );
        }
        assert!(
            owner
                .observe_wave_qkv_attention_output_tiles_state_v6(&states[0])
                .is_err()
        );
        // SAFETY: poisoned-group tests refuse before any backend access.
        assert!(
            unsafe {
                owner.rearm_wave_qkv_attention_output_tiles_state_v6(&mut states[0], &terminal())
            }
            .is_err()
        );
        assert!(
            unsafe {
                owner.dispatch_wave_qkv_attention_output_tiles_round_v6(commands(
                    &kernels,
                    &mut states,
                ))
            }
            .is_err()
        );
        assert!(owner.close().is_err());
    }
}

#[test]
fn resident_v6_pending_or_wrong_completion_census_never_yields_completed_tokens() {
    let kernels = kernels();
    for count in [0, 1, 3] {
        let mut states = make_states();
        let mut fake = Fake {
            durations: vec![1; count],
            ..Fake::default()
        };
        assert!(run_resident(&mut fake, commands(&kernels, &mut states)).is_err());
        assert!(
            states
                .iter()
                .all(|state| state.activation == Activation::Submitted)
        );
    }
    let mut states = make_states();
    let mut fake = Fake {
        fail_at: Some(10),
        ..Fake::default()
    };
    assert!(run_resident(&mut fake, commands(&kernels, &mut states)).is_err());
    assert!(!fake.completed);
    assert_eq!(
        fake.calls
            .iter()
            .filter(|name| name.starts_with("observe"))
            .count(),
        2
    );
}

#[test]
fn resident_v6_refuses_each_initial_and_terminal_word_corruption() {
    let kernels = kernels();
    for rank in 0..2 {
        for index in 0..284 {
            let mut states = make_states();
            let mut fake = Fake::default();
            fake.initial[rank][index] ^= 1;
            assert!(run_resident(&mut fake, commands(&kernels, &mut states)).is_err());
            assert!(!fake.calls.iter().any(|name| name == "publish0"));
            let mut states = make_states();
            let mut fake = Fake::default();
            fake.final_words[rank][index] = if (24..154).contains(&index) {
                65
            } else {
                fake.final_words[rank][index] ^ 1
            };
            assert!(
                run_resident(&mut fake, commands(&kernels, &mut states)).is_err(),
                "rank {rank} word {index}"
            );
            assert!(
                states
                    .iter()
                    .all(|state| state.activation == Activation::Submitted)
            );
        }
    }
}

#[test]
fn resident_v6_refuses_foreign_swapped_repeated_ranks_states_and_kernel_identity() {
    for change in 0..8 {
        let mut kernels = kernels();
        let mut states = make_states();
        let mut fake = Fake::default();
        match change {
            0 => kernels[0].group = 8,
            1 => kernels[1].rank = 0,
            2 => kernels.swap(0, 1),
            3 => states.swap(0, 1),
            4 => states[1].buffer.group = 8,
            5 => states[1].activation = Activation::Submitted,
            6 => kernels[0].metadata.object_sha256 = [8; 32],
            _ => fake.world = 8,
        }
        assert!(
            run_resident(&mut fake, commands(&kernels, &mut states)).is_err(),
            "change {change}"
        );
        assert!(!fake.calls.iter().any(|name| name == "publish0"));
    }
}

#[test]
fn resident_v6_refuses_digest_timeout_and_foreign_data_before_publication() {
    let kernels = kernels();
    for change in 0..5 {
        let mut states = make_states();
        let mut commands = commands(&kernels, &mut states);
        match change {
            0 => commands[0].object_sha256 = [9; 32],
            1 => commands[0].timeout_ms = 0,
            2 => commands[1].timeout_ms = 600_000,
            3 => commands[0].roots[0].group += 1,
            _ => commands[0].roots[0].owner = 1,
        }
        let mut fake = Fake::default();
        assert!(run_resident(&mut fake, commands).is_err());
        assert!(!fake.calls.iter().any(|name| name == "publish0"));
    }
}

#[test]
fn resident_v6_exact_metadata_cannot_be_replaced_by_same_width_v5_or_mlp_abi() {
    let kernels = kernels();
    let mut states = make_states();
    let commands = commands(&kernels, &mut states);
    validate_command_identity(7, 2, 0, &commands[0], &metadata()).unwrap();
    for change in 0..13 {
        let mut value = metadata();
        match change {
            0 => {
                value.symbol =
                    "ferric_qwen3_claimed_rmsnorm_qkv_attention_output_tasks_bf16_f32_v5".into()
            }
            1 => value.kernarg_bytes = 344,
            2 => value.kernarg_alignment = 16,
            3 => value.group_segment_bytes = 0,
            4 => value.private_segment_bytes = 4,
            5 => value.wavefront_size = 32,
            6 => value.implicit_argument_offset = Some(88),
            7 => value.implicit_argument_bytes = 0,
            8 => value.explicit_arguments[14].offset = 104,
            9 => value.explicit_arguments[14].access = Some(BufferAccessV1::Read),
            10 => value.explicit_arguments[14].pointee_alignment = Some(2),
            11 => value.explicit_arguments[0].global_buffer = false,
            _ => {
                value.explicit_arguments.pop();
            }
        }
        assert!(
            profile::validate_metadata(&value, [7; 32], profile::SYMBOL).is_err(),
            "change {change}"
        );
    }
}

#[test]
fn resident_v6_data_roots_require_exact_requested_extent_kind_and_current_record() {
    for index in 0..14 {
        let token = root(0, index);
        let fresh = || BufferRecord {
            token,
            local_id: index as u64 + 1,
            mapping: PeerMapping {
                peers: vec![],
                mapped: 0,
                unmapped: 0,
                phase: Phase::PeersMapped,
            },
            kind: BufferKind::PublicVram,
        };
        validate_data_record(0, index, token, &fresh()).unwrap();
        for change in 0..5 {
            let mut record = fresh();
            match change {
                0 => record.token.bytes += 1,
                1 => record.token.id += 1,
                2 => record.kind = BufferKind::WaveQkvAttentionOutputTilesStateV6,
                3 => record.mapping.phase = Phase::Released,
                _ => record.token.owner = 1,
            }
            assert!(validate_data_record(0, index, token, &record).is_err());
        }
        let mut wrong = token;
        wrong.bytes += 1;
        let mut record = fresh();
        record.token = wrong;
        assert!(validate_data_record(0, index, wrong, &record).is_err());
    }
}

#[test]
fn resident_v6_rejects_aliases_even_readonly_and_wrong_physical_extents() {
    validate_pair_regions(&[regions(0), regions(1)]).unwrap();
    for rank in 0..2 {
        for index in 0..15 {
            for change in 0..6 {
                let mut values = [regions(0), regions(1)];
                let region = &mut values[rank][index];
                match change {
                    0 => region.requested += 1,
                    1 => region.backing = region.requested - 1,
                    2 => region.base += 2,
                    3 => region.base = 0,
                    4 => region.buffer = 0,
                    _ => region.base = u64::MAX & !4095,
                }
                assert!(
                    validate_pair_regions(&values).is_err(),
                    "rank {rank} root {index} change {change}"
                );
            }
        }
    }
    for other in 0..2 {
        let mut values = [regions(0), regions(1)];
        values[other][1].base = values[0][0].base;
        assert!(validate_pair_regions(&values).is_err());
        let mut values = [regions(0), regions(1)];
        values[other][1].buffer = values[0][0].buffer;
        assert!(validate_pair_regions(&values).is_err());
    }
}

#[test]
fn resident_v6_native_adapter_routes_to_private_guard_not_public_observer() {
    for capture in [false, true] {
        let mut owner = group();
        owner.shared_full_currentness = true;
        let mut states = make_states();
        states[0].activation = Activation::Initialized;
        // Both modes use the same private observation guard, which refuses
        // this phase before a public fence could reach the empty native roster.
        let raw = capture.then(Vec::new);
        let error = NativeResident(&mut owner, raw)
            .observe(&states[0])
            .unwrap_err();
        assert_eq!(
            error,
            "prefix tiles V6 private resident observation activation"
        );
        assert!(owner.poisoned);
        assert_eq!(
            owner.require_active().unwrap_err(),
            "peer group is closed or quarantined"
        );
    }
}

#[test]
fn resident_v6_raw_entry_invalid_deadline_poison_blocks_both_modes_and_rearm() {
    let kernels = kernels();
    let mut states = make_states();
    let mut owner = group();
    let mut invalid = commands(&kernels, &mut states);
    invalid[0].timeout_ms = 0;
    // SAFETY: the real entry rejects the malformed deadline before native I/O.
    let error = unsafe {
        owner.dispatch_wave_qkv_attention_output_tiles_round_with_raw_timestamps_unchecked_v1(
            invalid,
        )
    }
    .unwrap_err();
    assert_eq!(
        error,
        "prefix tiles V6 aggregate timeout outside 1..600000 ms"
    );
    assert!(
        states
            .iter()
            .all(|state| state.activation == Activation::Ready)
    );
    assert!(owner.poisoned);
    // SAFETY: every following public entry must refuse the quarantined group.
    let raw_error = unsafe {
        owner.dispatch_wave_qkv_attention_output_tiles_round_with_raw_timestamps_unchecked_v1(
            commands(&kernels, &mut states),
        )
    }
    .unwrap_err();
    assert_eq!(raw_error, "peer group is closed or quarantined");
    let ordinary_error = unsafe {
        owner.dispatch_wave_qkv_attention_output_tiles_round_v6(commands(&kernels, &mut states))
    }
    .unwrap_err();
    assert_eq!(ordinary_error, "peer group is closed or quarantined");
    assert_eq!(
        unsafe {
            owner.rearm_wave_qkv_attention_output_tiles_state_v6(&mut states[0], &terminal())
        }
        .unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(
        owner.close().unwrap_err(),
        "peer group is closed or quarantined"
    );
}

#[test]
fn resident_v6_raw_entry_routes_to_native_validation_before_activation() {
    let kernels = kernels();
    let mut states = make_states();
    let mut owner = group();
    // SAFETY: this real native adapter has no contexts; validation must refuse
    // the missing rank without a mapping load or any queue publication.
    let error = unsafe {
        owner.dispatch_wave_qkv_attention_output_tiles_round_with_raw_timestamps_unchecked_v1(
            commands(&kernels, &mut states),
        )
    }
    .unwrap_err();
    assert_eq!(error, "prefix tiles V6 missing rank");
    assert!(
        states
            .iter()
            .all(|state| state.activation == Activation::Ready)
    );
    assert!(owner.poisoned);
    assert_eq!(
        owner
            .observe_wave_qkv_attention_output_tiles_state_v6(&states[0])
            .unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(
        owner.close().unwrap_err(),
        "peer group is closed or quarantined"
    );
}
