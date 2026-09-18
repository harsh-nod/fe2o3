use super::*;

fn identity() -> Gfx950EngineeringTp2DependencyIdentityV1 {
    Gfx950EngineeringTp2DependencyIdentityV1 {
        request_id: 1,
        generation: 1,
        group_id: 0,
        epoch: 0,
        layer: 0,
        operation: Gfx950EngineeringTp2DependencyOperationV1::AttentionOutputSum,
    }
}

fn metadata(producer: bool) -> KernelMetadataV1 {
    let slices: u32 = if producer { 3 } else { 10 };
    let scalars = if producer { 5 } else { 2 };
    let implicit = if producer { 72 } else { 168 };
    KernelMetadataV1 {
        symbol: if producer { PRODUCER } else { CONSUMER }.into(),
        object_sha256: [if producer { 1 } else { 2 }; 32],
        kernarg_bytes: implicit + 256,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(implicit),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..slices * 2)
            .map(|index| ExplicitArgumentV1 {
                offset: index * 8,
                bytes: 8,
                global_buffer: index % 2 == 0,
                pointee_alignment: None,
                access: None,
            })
            .chain((0..scalars).map(|index| ExplicitArgumentV1 {
                offset: slices * 16 + index * 4,
                bytes: 4,
                global_buffer: false,
                pointee_alignment: None,
                access: None,
            }))
            .collect(),
    }
}

fn kernels() -> [Gfx950EngineeringPeerKernelV1; 4] {
    std::array::from_fn(|index| Gfx950EngineeringPeerKernelV1 {
        group: 7,
        rank: index % 2,
        id: index as u64 + 1,
        metadata: metadata(index < 2),
    })
}

fn buffer(id: u64, owner: usize, bytes: u64) -> Gfx950EngineeringPeerBufferV1 {
    Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id,
        owner,
        bytes,
    }
}

fn request(
    kernels: &[Gfx950EngineeringPeerKernelV1; 4],
    operation: Gfx950EngineeringTp2DependencyOperationV1,
) -> Gfx950EngineeringTp2DependencyRequestV1<'_> {
    let (k, tag) = shape(operation);
    let partials = [
        buffer(1, 0, 16 * ELEMENTS * 4),
        buffer(2, 1, 16 * ELEMENTS * 4),
    ];
    let producers = std::array::from_fn(|rank| {
        let mut bytes = vec![0; 328];
        for (index, length) in [16 * k, ELEMENTS * k, 16 * ELEMENTS]
            .into_iter()
            .enumerate()
        {
            bytes[index * 16 + 8..index * 16 + 16].copy_from_slice(&length.to_le_bytes());
        }
        for (index, value) in [1, 4096, k as u32, 2, tag].into_iter().enumerate() {
            bytes[48 + index * 4..52 + index * 4].copy_from_slice(&value.to_le_bytes());
        }
        Gfx950EngineeringPeerDispatchV1 {
            kernel: &kernels[rank],
            bytes,
            workgroup: [64, 1, 1],
            grid: [16384, 1, 1],
            timeout_ms: 2000,
            pointers: vec![
                buffer(10 + rank as u64, rank, 16 * k * 2).pointer(
                    0,
                    0,
                    16 * k * 2,
                    BufferAccessV1::Read,
                ),
                buffer(12 + rank as u64, rank, ELEMENTS * k * 2).pointer(
                    16,
                    0,
                    ELEMENTS * k * 2,
                    BufferAccessV1::Read,
                ),
                partials[rank].pointer(32, 0, 16 * ELEMENTS * 4, BufferAccessV1::Write),
            ],
        }
    });
    let consumers = std::array::from_fn(|rank| {
        let mut bytes = vec![0; 424];
        let mut pointers = Vec::new();
        for index in 0..10 {
            let length = if (2..8).contains(&index) { 0 } else { ELEMENTS };
            bytes[index * 16 + 8..index * 16 + 16].copy_from_slice(&length.to_le_bytes());
            let target = match index {
                0 | 1 => partials[index],
                2..=7 => partials[0],
                8 => buffer(3 + rank as u64, rank, ELEMENTS * 2),
                _ => buffer(5 + rank as u64, rank, ELEMENTS * 2),
            };
            pointers.push(target.pointer(
                (index * 16) as u32,
                0,
                length * if index < 8 { 4 } else { 2 },
                if index == 9 {
                    BufferAccessV1::Write
                } else {
                    BufferAccessV1::Read
                },
            ));
        }
        bytes[160..164].copy_from_slice(&1_u32.to_le_bytes());
        bytes[164..168].copy_from_slice(&2_u32.to_le_bytes());
        Gfx950EngineeringPeerDispatchV1 {
            kernel: &kernels[rank + 2],
            bytes,
            workgroup: [64, 1, 1],
            grid: [4096, 1, 1],
            pointers,
            timeout_ms: 2000,
        }
    });
    Gfx950EngineeringTp2DependencyRequestV1 {
        identity: Gfx950EngineeringTp2DependencyIdentityV1 {
            operation,
            ..identity()
        },
        producers,
        consumers,
    }
}

#[test]
fn identity_allows_zero_collective_labels_but_requires_strict_transaction_sequence() {
    assert_eq!(next_identity(identity(), 1, 0).unwrap(), 2);
    let mut value = identity();
    value.request_id = 41;
    value.generation = 2;
    value.layer = 35;
    assert_eq!(next_identity(value, 2, 5).unwrap(), 3);
    for mutation in 0..7 {
        let mut value = identity();
        match mutation {
            0 => value.request_id = 0,
            1 => value.generation = 0,
            2 => value.generation = 2,
            3 => value.layer = 36,
            4 => value.generation = u64::MAX,
            5 => value.request_id = 7,
            _ => value.request_id = 6,
        }
        let expected = if mutation == 4 { u64::MAX } else { 1 };
        let last = if mutation >= 5 { 7 } else { 0 };
        assert!(
            next_identity(value, expected, last).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn both_exact_local_tp2_shapes_admit_without_fallback() {
    let kernels = kernels();
    for operation in [
        Gfx950EngineeringTp2DependencyOperationV1::AttentionOutputSum,
        Gfx950EngineeringTp2DependencyOperationV1::FeedForwardDownSum,
    ] {
        graph_contract(&request(&kernels, operation)).unwrap();
    }
}

#[test]
fn producer_capacity_is_exactly_sixteen_while_consumers_read_one_active_row() {
    let kernels = kernels();
    for operation in [
        Gfx950EngineeringTp2DependencyOperationV1::AttentionOutputSum,
        Gfx950EngineeringTp2DependencyOperationV1::FeedForwardDownSum,
    ] {
        let valid = request(&kernels, operation);
        assert_eq!(word(&valid.producers[0].bytes, 40).unwrap(), 16 * 4096);
        assert_eq!(word(&valid.consumers[0].bytes, 8).unwrap(), 4096);
        assert_eq!(scalar(&valid.producers[0].bytes, 48).unwrap(), 1);
        for capacity in [1_u64, 32] {
            for index in [0, 2] {
                let mut value = request(&kernels, operation);
                let per_row = if index == 0 { shape(operation).0 } else { 4096 };
                let width = if index == 0 { 2 } else { 4 };
                let length = capacity * per_row;
                let producer = &mut value.producers[0];
                producer.bytes[index * 16 + 8..index * 16 + 16]
                    .copy_from_slice(&length.to_le_bytes());
                producer.pointers[index].extent_bytes = length * width;
                producer.pointers[index].buffer.bytes = length * width;
                assert!(command_contract(producer, 0, true, operation).is_err());
            }
        }
    }
}

#[test]
fn all_ten_role_ids_are_distinct_including_read_only_roles() {
    let kernels = kernels();
    let mut value = request(&kernels, identity().operation);
    value.producers[0].pointers[0].buffer = value.producers[0].pointers[1].buffer;
    assert!(graph_contract(&value).unwrap_err().contains("roles alias"));
    let mut value = request(&kernels, identity().operation);
    value.consumers[0].pointers[8].buffer = value.producers[0].pointers[0].buffer;
    assert!(graph_contract(&value).unwrap_err().contains("roles alias"));
}

#[test]
fn metadata_contract_closes_every_abi_field_and_root() {
    for producer in [false, true] {
        metadata_contract(&metadata(producer), producer).unwrap();
        for mutation in 0..15 {
            let mut value = metadata(producer);
            match mutation {
                0 => value.symbol.push('_'),
                1 => value.kernarg_bytes += 8,
                2 => value.kernarg_alignment = 16,
                3 => value.group_segment_bytes = 4,
                4 => value.private_segment_bytes = 4,
                5 => value.wavefront_size = 32,
                6 => value.implicit_argument_offset = None,
                7 => value.implicit_argument_bytes = 0,
                8 => {
                    value.explicit_arguments.pop();
                }
                9 => value.explicit_arguments[0].offset = 8,
                10 => value.explicit_arguments[0].bytes = 4,
                11 => value.explicit_arguments[0].global_buffer = false,
                12 => value.explicit_arguments[1].global_buffer = true,
                13 => value.explicit_arguments[0].pointee_alignment = Some(2),
                _ => value.explicit_arguments[0].access = Some(BufferAccessV1::Read),
            }
            assert!(
                metadata_contract(&value, producer).is_err(),
                "producer={producer} mutation={mutation}"
            );
        }
    }
}

#[test]
fn dedicated_consumer_rejects_the_legacy_v4_root() {
    let mut legacy = metadata(false);
    legacy.symbol = "ferric_qwen3_tp_peer_ordered_residual_bf16_v4".into();
    assert!(metadata_contract(&legacy, false).is_err());
    assert_eq!(
        CONSUMER,
        "ferric_qwen3_tp_peer_tp2_ordered_residual_bf16_v18"
    );
    metadata_contract(&metadata(false), false).unwrap();
}

#[test]
fn graph_rejects_wrong_rank_or_rank_image() {
    for mutation in 0..4 {
        let mut kernels = kernels();
        match mutation {
            0 => kernels[1].metadata.object_sha256 = [9; 32],
            1 => kernels[3].metadata.object_sha256 = [9; 32],
            2 => kernels[0].rank = 1,
            _ => kernels[2].rank = 1,
        }
        assert!(graph_contract(&request(&kernels, identity().operation)).is_err());
    }
}

#[test]
fn exact_producer_body_pointer_and_geometry_guards() {
    let kernels = kernels();
    for mutation in 0..17 {
        let mut value = request(&kernels, identity().operation);
        let command = &mut value.producers[0];
        match mutation {
            0 => command.timeout_ms = 1999,
            1 => command.timeout_ms = 2001,
            2 => command.workgroup[0] = 128,
            3 => command.grid[0] = 4096,
            4 => {
                command.bytes.pop();
            }
            5 => {
                command.pointers.pop();
            }
            6 => command.bytes[68] = 1,
            7 => command.bytes[327] = 1,
            8 => command.bytes[0] = 1,
            9 => command.bytes[8] = 1,
            10 => command.bytes[48] = 2,
            11 => command.bytes[60] = 8,
            12 => command.bytes[64] = 2,
            13 => command.pointers[0].buffer.owner = 1,
            14 => command.pointers[0].buffer_offset = 2,
            15 => command.pointers[2].access = BufferAccessV1::Read,
            _ => command.pointers[2].extent_bytes -= 4,
        }
        assert!(graph_contract(&value).is_err(), "mutation {mutation}");
    }
}

#[test]
fn ordered_consumers_close_aliases_fillers_and_extents() {
    let kernels = kernels();
    for mutation in 0..14 {
        let mut value = request(&kernels, identity().operation);
        match mutation {
            0 => value.consumers[0].pointers.swap(0, 1),
            1 => value.consumers[1].pointers[0].buffer = value.producers[1].pointers[2].buffer,
            2 => value.consumers[1].pointers[2].buffer = value.producers[1].pointers[2].buffer,
            3 => value.consumers[0].pointers[8].buffer.owner = 1,
            4 => value.consumers[0].pointers[9].buffer = value.consumers[0].pointers[8].buffer,
            5 => value.consumers[1].pointers[9].buffer = value.producers[0].pointers[2].buffer,
            6 => value.consumers[1].bytes[164] = 8,
            7 => value.consumers[0].bytes[40] = 1,
            8 => value.consumers[0].bytes[160] = 2,
            9 => value.consumers[0].pointers[1].access = BufferAccessV1::Write,
            10 => value.consumers[1].pointers[9].extent_bytes += 2,
            11 => value.consumers[0].pointers[8].buffer.bytes -= 2,
            12 => value.producers[0].pointers[0].buffer = value.producers[0].pointers[2].buffer,
            _ => value.producers[0].pointers[0].buffer = value.consumers[0].pointers[9].buffer,
        }
        assert!(graph_contract(&value).is_err(), "mutation {mutation}");
    }
}

struct Mock {
    diagnostic: Diagnostic,
    time: Duration,
    events: Vec<String>,
    fail_at: Option<usize>,
    published: [bool; 2],
    first: [u64; 2],
    stall: bool,
    bad: Option<Observation>,
    final_refresh_late: bool,
    observed_complete: bool,
    initial_refresh_late: bool,
    initial_observation_late: bool,
    retirement_late: bool,
}

impl Mock {
    fn new() -> Self {
        Self {
            diagnostic: Diagnostic::default(),
            time: Duration::ZERO,
            events: Vec::new(),
            fail_at: None,
            published: [false; 2],
            first: [8, 17],
            stall: false,
            bad: None,
            final_refresh_late: false,
            observed_complete: false,
            initial_refresh_late: false,
            initial_observation_late: false,
            retirement_late: false,
        }
    }
    fn event(&mut self, event: String) -> Result<()> {
        self.events.push(event);
        if self.fail_at == Some(self.events.len() - 1) {
            return Err("injected protocol failure".into());
        }
        Ok(())
    }
    fn run(&mut self) -> Result<Observation> {
        protocol(self, self.first, self.first.map(|first| first + 3))
    }
}

impl ProtocolBackend for Mock {
    fn diagnostic(&mut self) -> &mut Diagnostic {
        &mut self.diagnostic
    }
    fn now(&self) -> Duration {
        self.time
    }
    fn refresh(&mut self) -> Result<()> {
        self.event("refresh".into())?;
        if self.initial_refresh_late && self.events.len() == 1 {
            self.time += DEADLINE;
        }
        if self.final_refresh_late && self.observed_complete {
            self.time += DEADLINE;
        }
        Ok(())
    }
    fn observe(&mut self) -> Result<Observation> {
        self.event("observe".into())?;
        if self.initial_observation_late && self.events.len() == 2 {
            self.time += DEADLINE;
        }
        if let Some(bad) = self.bad {
            return Ok(bad);
        }
        let done = self.published == [true; 2];
        self.observed_complete |= done && !self.stall;
        Ok(Observation {
            values: if done { [[0; 3]; 2] } else { [[1; 3]; 2] },
            frontiers: self.first.map(|first| {
                [
                    first + 3,
                    if done && !self.stall {
                        first + 3
                    } else {
                        first
                    },
                ]
            }),
        })
    }
    fn publish(&mut self, rank: usize) -> Result<()> {
        self.event(format!("publish:{rank}"))?;
        self.published[rank] = true;
        Ok(())
    }
    fn retire(&mut self) -> Result<()> {
        self.event("retire".into())?;
        if self.retirement_late {
            self.time += DEADLINE;
        }
        Ok(())
    }
    fn pause(&mut self) {
        self.events.push("pause".into());
        self.time += Duration::from_millis(100);
    }
}

#[test]
fn six_packet_protocol_has_full_fences_and_no_time_in_receipt() {
    let mut backend = Mock::new();
    let result = backend.run().unwrap();
    assert_eq!(result.values, [[0; 3]; 2]);
    assert_eq!(result.frontiers, [[11, 11], [20, 20]]);
    assert_eq!(
        backend.events,
        [
            "refresh",
            "observe",
            "refresh",
            "publish:0",
            "refresh",
            "publish:1",
            "observe",
            "refresh",
            "observe",
            "retire"
        ]
    );
}

#[test]
fn failure_at_every_protocol_operation_is_terminal_without_followup() {
    let mut success = Mock::new();
    success.run().unwrap();
    for fail_at in 0..success.events.len() {
        let mut backend = Mock::new();
        backend.fail_at = Some(fail_at);
        assert!(backend.run().is_err());
        assert_eq!(backend.events, success.events[..=fail_at]);
    }
}

#[test]
fn six_zeros_never_replace_both_actual_read_frontiers() {
    let mut backend = Mock::new();
    backend.stall = true;
    assert!(backend.run().unwrap_err().contains("drain deadline"));
    assert_eq!(backend.diagnostic.last.unwrap().values, [[0; 3]; 2]);
    assert_eq!(
        backend.diagnostic.last.unwrap().frontiers,
        [[11, 8], [20, 17]]
    );
}

#[test]
fn invalid_initial_values_and_consumed_frontiers_reject_before_publication() {
    for mutation in 0..6 {
        let mut backend = Mock::new();
        let mut value = Observation {
            values: [[1; 3]; 2],
            frontiers: [[11, 8], [20, 17]],
        };
        match mutation {
            0 => value.values[0][0] = 0,
            1 => value.values[1][2] = 0,
            2 => value.values[0][1] = 2,
            3 => value.frontiers[0][1] += 1,
            4 => value.frontiers[1][0] += 1,
            _ => value.frontiers[0][1] -= 1,
        }
        backend.bad = Some(value);
        assert!(backend.run().is_err());
        assert!(
            !backend
                .events
                .iter()
                .any(|event| event.starts_with("publish:"))
        );
    }
}

#[test]
fn signal_regression_and_frontier_regression_reject() {
    for mutation in 0..4 {
        let mut backend = Mock::new();
        let mut previous = Observation {
            values: [[0; 3]; 2],
            frontiers: [[11, 11], [20, 20]],
        };
        let mut value = previous;
        match mutation {
            0 => value.values[1][0] = 1,
            1 => value.values[0][2] = -1,
            2 => value.frontiers[0][1] -= 1,
            _ => value.frontiers[1][1] += 1,
        }
        backend.bad = Some(value);
        assert!(checked_observation(&mut backend, &mut previous, [11, 20]).is_err());
    }
}

#[test]
fn final_currentness_cost_remains_inside_aggregate_deadline() {
    let mut backend = Mock::new();
    backend.final_refresh_late = true;
    assert!(backend.run().unwrap_err().contains("final currentness"));
}

#[test]
fn initial_full_fence_and_observation_are_inside_the_same_deadline() {
    let mut backend = Mock::new();
    backend.initial_refresh_late = true;
    assert!(
        backend
            .run()
            .unwrap_err()
            .contains("initial currentness deadline")
    );
    assert_eq!(backend.events, ["refresh"]);
    let mut backend = Mock::new();
    backend.initial_observation_late = true;
    assert!(
        backend
            .run()
            .unwrap_err()
            .contains("initial observation deadline")
    );
    assert_eq!(backend.events, ["refresh", "observe"]);
}

#[test]
fn final_retirement_is_inside_the_same_deadline() {
    let mut backend = Mock::new();
    backend.retirement_late = true;
    assert!(
        backend
            .run()
            .unwrap_err()
            .contains("retirement currentness deadline")
    );
}

#[test]
fn exact_three_packet_reservation_rejects_overflow_and_size_drift() {
    for (first, next) in [
        ([0, 0], [2, 3]),
        ([0, 0], [3, 4]),
        ([u64::MAX - 2, 0], [0, 3]),
    ] {
        let mut backend = Mock::new();
        assert!(protocol(&mut backend, first, next).is_err());
        assert!(backend.events.is_empty());
    }
}

#[test]
fn ordinary_group_cannot_enter_dependency_mode_and_failure_poisons_it() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        dependency_collective: None,
    };
    let kernels = kernels();
    // No contexts exist and the mode check precedes all device operations.
    let result = unsafe {
        group.dispatch_tp2_dependency_collective_unchecked(request(&kernels, identity().operation))
    };
    assert!(result.unwrap_err().contains("mode not enabled"));
    assert!(group.poisoned);
    assert!(group.close().is_err());
}

#[test]
fn packet_capture_is_single_body_then_single_header() {
    let mut capture = PacketCapture::default();
    assert!(capture.header(0x1503).is_err());
    let mut capture = PacketCapture::default();
    capture.body([0; 64]).unwrap();
    assert!(capture.body([0; 64]).is_err());
    let mut capture = PacketCapture::default();
    capture.body([0; 64]).unwrap();
    capture.header(0x1503).unwrap();
    assert!(capture.header(0x1503).is_err());
}

#[test]
fn fan_in_barrier_encodes_both_producers_and_private_completion() {
    let mut capture = PacketCapture::default();
    AqlDependencyBarrierAndPacketV1::new_unpublished(
        &[address(0x1000).unwrap(), address(0x2000).unwrap()],
        address(0x3040).unwrap(),
    )
    .unwrap()
    .publish_with(&mut capture)
    .unwrap();
    let packet = capture.finish().unwrap();
    assert_eq!(packet.header, 0x1503);
    assert_eq!(word(&packet.bytes, 8).unwrap(), 0x1000);
    assert_eq!(word(&packet.bytes, 16).unwrap(), 0x2000);
    assert_eq!(word(&packet.bytes, 56).unwrap(), 0x3040);
}

#[test]
fn retained_storage_and_stage_order_are_source_pinned() {
    let source = include_str!("engineering_gfx950_peer_dependency.rs");
    let initialize = source
        .split("fn initialize_storage(")
        .nth(1)
        .unwrap()
        .split("fn signals(")
        .next()
        .unwrap();
    assert!(
        initialize
            .find("dependency_internal.push(signals)")
            .unwrap()
            < initialize
                .find("initialize_engineering_signal_slots")
                .unwrap()
    );
    assert!(initialize.contains("dependency_internal.push(arguments)"));
    let stage = source
        .split("fn stage(")
        .nth(1)
        .unwrap()
        .split("struct Observation")
        .next()
        .unwrap();
    assert!(
        stage.find("require_quiescent").unwrap()
            < stage.find("reset_completion_signal_release").unwrap()
    );
    assert!(
        stage.find("reset_completion_signal_release").unwrap()
            < stage.find("kernel_packet").unwrap()
    );
    assert!(stage.find("kernel_packet").unwrap() < stage.find("write_aql_slot").unwrap());
    assert!(!stage.contains("publish_aql_header"));
    let publish = source
        .split("fn publish(&mut self, rank: usize) -> Result<()> {")
        .nth(1)
        .unwrap()
        .split("fn pause(")
        .next()
        .unwrap();
    assert!(!publish.contains("reset_completion"));
    assert!(!publish.contains("write_aql_slot"));
    assert!(!publish.contains("with_bytes_mut"));
    assert!(source.contains("ARGUMENT_SLOT_BYTES: usize = PAGE_BYTES / 2"));
    assert!(!source.contains("context.internal[KERNARG]"));
}

#[test]
fn default_paths_and_terminal_owner_lifecycle_remain_separate() {
    let peer = include_str!("engineering_gfx950_peer.rs");
    assert!(peer.contains("dependency_collective: None"));
    let close = peer.split("pub fn close(&mut self)").nth(1).unwrap();
    assert!(
        close.find("dependency::release_private(self)").unwrap()
            < close.find("context.close_inner()").unwrap()
    );
    assert!(peer.contains("std::mem::forget(std::mem::take(&mut self.contexts))"));
    let source = include_str!("engineering_gfx950_peer_dependency.rs");
    let release = source
        .split("pub(super) fn release_private(")
        .nth(1)
        .unwrap()
        .split("fn address(")
        .next()
        .unwrap();
    assert!(
        release.find("require_quiescent").unwrap()
            < release.find("mappings[rank].release").unwrap()
    );
    assert!(!release.contains("dependency_internal.pop"));
    let context = include_str!("engineering_gfx950.rs");
    let close = context.split("fn close_inner(").nth(1).unwrap();
    assert!(close.find("destroy_queue").unwrap() < close.find("dependency_internal.pop").unwrap());
    assert!(context.contains("if !self.dependency_internal.is_empty()"));
    let performance = include_str!("engineering_gfx950_peer_performance.rs");
    assert!(performance.contains("TP2 dependency mode requires unshared full currentness"));
    assert!(performance.contains("TP2 dependency mode excludes peer sequences"));
    let round = include_str!("engineering_gfx950_peer_round.rs");
    assert!(round.contains("TP2 dependency mode excludes independent peer rounds"));
}
