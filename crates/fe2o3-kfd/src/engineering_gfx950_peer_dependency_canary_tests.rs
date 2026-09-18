use super::*;

#[test]
fn dependency_control_policy_uses_two_exact_existing_allocation_profiles() {
    assert_eq!(
        QueueControlPolicy::UserptrCoherent.flags().bits(),
        0x8400_0004
    );
    assert_eq!(
        QueueControlPolicy::DependencyCanaryGttUncached
            .flags()
            .bits(),
        0x8600_0002
    );
    assert_eq!(
        QueueControlPolicy::UserptrCoherent.flags(),
        KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL
    );
    assert_eq!(
        QueueControlPolicy::DependencyCanaryGttUncached.flags(),
        KfdAllocMemoryFlags::KERNARG
    );
}

#[test]
fn dependency_control_policy_rejects_every_timestamp_mode_combination() {
    for timestamps in [false, true] {
        for full_forward in [false, true] {
            assert_eq!(
                QueueControlPolicy::DependencyCanaryGttUncached
                    .validate_modes(timestamps, full_forward)
                    .is_ok(),
                !timestamps && !full_forward
            );
        }
    }
}

#[test]
fn dependency_shared_constructor_preserves_original_timestamp_mode_admission() {
    for timestamps in [false, true] {
        for full_forward in [false, true] {
            let result =
                QueueControlPolicy::UserptrCoherent.validate_modes(timestamps, full_forward);
            assert_eq!(result.is_ok(), !(timestamps && full_forward));
            if timestamps && full_forward {
                assert_eq!(
                    result.unwrap_err(),
                    "timestamp modes are mutually exclusive"
                );
            }
        }
    }
}

#[test]
fn dependency_control_roster_is_exactly_two_without_widening_default_rosters() {
    let rosters: &[&[u64]] = &[
        &[],
        &[1],
        &[1, 2],
        &[2, 1],
        &[0, 1],
        &[1, 1],
        &[1, 2, 3],
        &[1, 2, 3, 4, 5, 6, 7, 8],
        &[1, 2, 3, 4, 5, 6, 7, 0],
        &[1, 2, 3, 4, 5, 6, 7, 7],
        &[1, 2, 3, 4, 5, 6, 7, 8, 9],
    ];
    for &ids in rosters {
        assert_eq!(
            checked_control_roster(ids, QueueControlPolicy::UserptrCoherent),
            checked_roster(ids)
        );
        assert_eq!(
            checked_control_roster(ids, QueueControlPolicy::DependencyCanaryGttUncached).is_ok(),
            ids.len() == 2 && checked_roster(ids).is_ok()
        );
    }
}

#[test]
fn dependency_control_policy_source_contract_preserves_all_default_entrypoints() {
    let context = include_str!("engineering_gfx950.rs");
    let constructors = context
        .split("impl Context {")
        .nth(1)
        .unwrap()
        .split("fn initialize(")
        .next()
        .unwrap();
    let normal = constructors
        .split("fn open(")
        .nth(1)
        .unwrap()
        .split("fn open_with_timestamp_canary(")
        .next()
        .unwrap();
    assert!(normal.contains("Self::open_with_timestamp_canary(device, None, None)"));
    let timestamps = constructors
        .split("fn open_with_timestamp_canary(")
        .nth(1)
        .unwrap()
        .split("fn open_with_queue_control(")
        .next()
        .unwrap();
    assert!(timestamps.contains("QueueControlPolicy::UserptrCoherent"));
    assert!(!timestamps.contains("DependencyCanaryGttUncached"));
    let shared = constructors
        .split("fn open_with_queue_control(")
        .nth(1)
        .unwrap();
    assert!(
        shared.find("queue_control_policy.validate_modes(").unwrap()
            < shared.find("Backend::new(device)").unwrap()
    );
    assert!(
        shared.find("queue_control_policy,").unwrap()
            < shared.find("context.initialize()").unwrap()
    );
    assert!(!context.contains("self.queue_control_policy ="));
    let peer = include_str!("engineering_gfx950_peer.rs");
    let normal = peer
        .split("pub unsafe fn open_unchecked(")
        .nth(1)
        .unwrap()
        .split("unsafe fn open_with_queue_control_unchecked(")
        .next()
        .unwrap();
    assert!(normal.contains("QueueControlPolicy::UserptrCoherent"));
    assert!(!normal.contains("DependencyCanaryGttUncached"));
    let shared = peer
        .split("unsafe fn open_with_queue_control_unchecked(")
        .nth(1)
        .unwrap()
        .split("fn require_active(")
        .next()
        .unwrap();
    assert!(
        shared
            .find("checked_control_roster(unique_ids, policy)?")
            .unwrap()
            < shared.find("OpenedKfd::open_default()").unwrap()
    );
    assert!(
        shared
            .split_whitespace()
            .collect::<String>()
            .contains("Context::open_with_queue_control(device,None,None,policy,)?")
    );
    let canary = include_str!("engineering_gfx950_peer_dependency_canary.rs");
    assert_eq!(
        canary
            .matches("QueueControlPolicy::DependencyCanaryGttUncached")
            .count(),
        1
    );
    assert!(canary.contains("Gfx950EngineeringPeerGroupV1::open_with_queue_control_unchecked("));
}

#[test]
fn dependency_control_allocation_source_contract_keeps_layout_and_create_order() {
    let context = include_str!("engineering_gfx950.rs");
    let initialize = context
        .split("fn initialize_queue(")
        .nth(1)
        .unwrap()
        .split("fn allocate_resource(")
        .next()
        .unwrap();
    let compact = initialize.split_whitespace().collect::<String>();
    assert!(compact.contains(
        "letcontrol=self.allocate_resource(PAGE_BYTES,self.queue_control_policy.flags(),"
    ));
    assert!(
        initialize
            .find("self.queue_control_policy.flags()")
            .unwrap()
            < initialize
                .find("crate::queue_linux::create_queue(")
                .unwrap()
    );
    assert!(
        initialize.find("self.internal.push(control)").unwrap()
            < initialize
                .find("crate::queue_linux::create_queue(")
                .unwrap()
    );
    assert!(compact.contains("write_pointer_address:self.internal[CONTROL].va+0x38,"));
    assert!(compact.contains("read_pointer_address:self.internal[CONTROL].va+0x80,"));
    assert!(initialize.contains("initialize_amd_aql_control(bytes)"));
    assert!(initialize.contains("KfdAllocMemoryFlags::USERPTR_EXECUTABLE"));
    assert_eq!(
        context.matches("self.queue_control_policy.flags()").count(),
        1
    );
}

#[derive(Default)]
struct FakeProtocol {
    time: Duration,
    events: Vec<&'static str>,
    fail_at: Option<usize>,
    phase: u8,
    observation_count: usize,
    corrupt: Option<(usize, usize, i64)>,
    frontier_lag: bool,
    witness_pending: bool,
    header_corrupt: bool,
    slow_final_refresh: bool,
    drain_values: Option<[i64; 4]>,
    diagnostic: Diagnostic,
}
impl FakeProtocol {
    fn event(&mut self, name: &'static str) -> Result<()> {
        self.events.push(name);
        if self.fail_at == Some(self.events.len()) {
            return Err("injected terminal failure".into());
        }
        Ok(())
    }
}
impl ProtocolBackend for FakeProtocol {
    fn diagnostic(&mut self) -> &mut Diagnostic {
        &mut self.diagnostic
    }
    fn now(&self) -> Duration {
        self.time
    }
    fn refresh(&mut self) -> Result<()> {
        self.event("refresh")?;
        if self.phase == 2 && self.slow_final_refresh {
            self.time += DEADLINE;
        }
        Ok(())
    }
    fn observe(&mut self) -> Result<Observation> {
        self.event("observe")?;
        self.observation_count += 1;
        let mut values = match self.phase {
            0 => [1; 4],
            1 => [1, i64::from(self.witness_pending), 1, 1],
            _ => self.drain_values.unwrap_or([0; 4]),
        };
        if let Some((count, index, value)) = self.corrupt
            && self.observation_count == count
        {
            values[index] = value;
        }
        Ok(Observation {
            values,
            frontiers: if self.phase == 2 && !self.frontier_lag {
                [[1, 1], [3, 3]]
            } else {
                [[1, 0], [3, 0]]
            },
            producer_header: (
                0,
                if self.header_corrupt || self.phase == 2 {
                    0x1402
                } else {
                    1
                },
                1,
            ),
        })
    }
    fn publish_consumer(&mut self) -> Result<()> {
        self.event("consumer")?;
        assert_eq!(self.phase, 0);
        self.phase = 1;
        Ok(())
    }
    fn publish_producer(&mut self) -> Result<()> {
        self.event("producer")?;
        assert_eq!(self.phase, 1);
        assert!(self.time >= HOLD);
        self.phase = 2;
        self.time += Duration::from_millis(7);
        Ok(())
    }
    fn pause(&mut self) {
        self.time += Duration::from_millis(1);
    }
}

#[test]
fn dependency_protocol_records_only_prepublication_hold_and_all_final_evidence() {
    let mut backend = FakeProtocol::default();
    let receipt = protocol(&mut backend, [1, 3], 0).unwrap();
    assert_eq!(receipt.hold, HOLD);
    assert_eq!(receipt.elapsed, HOLD + Duration::from_millis(7));
    assert_eq!(receipt.witness_after, Duration::ZERO);
    assert_eq!(receipt.held.values, [1, 0, 1, 1]);
    assert_eq!(receipt.held.producer_header, (0, 1, 1));
    assert_eq!(receipt.final_observation.values, [0; 4]);
    assert_eq!(receipt.final_observation.frontiers, [[1, 1], [3, 3]]);
    assert_eq!(
        backend
            .events
            .iter()
            .filter(|&&event| event == "producer")
            .count(),
        1
    );
}

#[test]
fn dependency_every_protocol_operation_failure_stops_without_retry_or_cleanup() {
    let mut passing = FakeProtocol::default();
    protocol(&mut passing, [1, 3], 0).unwrap();
    for fail_at in 1..=passing.events.len() {
        let mut backend = FakeProtocol {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert!(
            protocol(&mut backend, [1, 3], 0).is_err(),
            "operation {fail_at}"
        );
        assert_eq!(backend.events, passing.events[..fail_at]);
        assert!(
            !backend
                .events
                .iter()
                .any(|event| ["reset", "unmap", "free", "rollback"].contains(event))
        );
    }
}

#[test]
fn dependency_premature_completion_invalid_value_and_regression_reject() {
    for index in 0..4 {
        let mut initial = FakeProtocol {
            corrupt: Some((1, index, 0)),
            ..Default::default()
        };
        assert!(protocol(&mut initial, [1, 3], 0).is_err());
        assert!(!initial.events.contains(&"consumer"));
    }
    for index in [0, 2, 3] {
        let mut backend = FakeProtocol {
            corrupt: Some((3, index, 0)),
            ..Default::default()
        };
        assert!(protocol(&mut backend, [1, 3], 0).is_err());
        assert!(!backend.events.contains(&"producer"));
    }
    for value in [-1, 1, 2] {
        let mut backend = FakeProtocol {
            corrupt: Some((3, 1, value)),
            ..Default::default()
        };
        assert!(protocol(&mut backend, [1, 3], 0).is_err());
        assert!(!backend.events.contains(&"producer"));
    }
}

#[test]
fn dependency_frontier_signal_header_and_deadline_gates_are_not_interchangeable() {
    assert!(!drained([0; 4], [[1, 0], [3, 3]], [1, 3]));
    assert!(!drained([0; 4], [[1, 1], [3, 2]], [1, 3]));
    for index in 0..4 {
        let mut values = [0; 4];
        values[index] = 1;
        assert!(!drained(values, [[1, 1], [3, 3]], [1, 3]));
    }
    for mut backend in [
        FakeProtocol {
            frontier_lag: true,
            ..Default::default()
        },
        FakeProtocol {
            witness_pending: true,
            ..Default::default()
        },
        FakeProtocol {
            header_corrupt: true,
            ..Default::default()
        },
        FakeProtocol {
            slow_final_refresh: true,
            ..Default::default()
        },
    ] {
        assert!(protocol(&mut backend, [1, 3], 0).is_err());
    }
}

#[test]
fn dependency_terminal_diagnostic_distinguishes_last_observed_stall_states() {
    for values in [[1, 0, 1, 1], [0, 0, 1, 1], [0, 0, 0, 1], [0; 4]] {
        let mut backend = FakeProtocol {
            drain_values: Some(values),
            frontier_lag: true,
            ..Default::default()
        };
        backend.diagnostic.begin_generation(2);
        let error = protocol(&mut backend, [1, 3], 0).err().unwrap();
        assert!(error.contains("drain deadline"));
        backend.diagnostic.protocol_finished_after = Some(backend.now());
        let report = backend.diagnostic.report(&error);
        assert_eq!(report["generation"], 2);
        assert_eq!(report["phase"], "drain");
        assert_eq!(
            report["last_observation"]["completion_values"],
            serde_json::json!(values)
        );
        assert_eq!(
            report["last_observation"]["actual_frontiers"],
            serde_json::json!([[1, 0], [3, 0]])
        );
        assert_eq!(
            report["last_observation"]["producer_header"],
            serde_json::json!([0, 0x1402, 1])
        );
        assert_eq!(report["last_observation"]["age_at_protocol_return_ns"], 0);
        assert_eq!(report["last_observation_is_terminal_snapshot"], false);
        assert_eq!(report["additional_device_operations_after_failure"], false);
        assert_eq!(report["acceptance"], false);
    }
}

#[test]
fn dependency_failed_refresh_retains_only_the_last_complete_observation() {
    let mut backend = FakeProtocol {
        fail_at: Some(5),
        ..Default::default()
    };
    let error = protocol(&mut backend, [1, 3], 0).err().unwrap();
    assert_eq!(error, "injected terminal failure");
    assert_eq!(
        backend.events,
        ["refresh", "observe", "consumer", "observe", "refresh"]
    );
    assert_eq!(backend.diagnostic.operation, "refresh_currentness");
    assert_eq!(backend.diagnostic.last.unwrap().0.values, [1, 0, 1, 1]);
    assert_eq!(backend.diagnostic.observation_count, 2);
    let count = backend.events.len();
    let report = backend.diagnostic.report(&"x".repeat(20_000));
    assert_eq!(report["error"].as_str().unwrap().len(), 512);
    assert!(serde_json::to_vec(&report).unwrap().len() < 4096);
    assert_eq!(backend.events.len(), count);
}

#[test]
fn dependency_diagnostic_rollover_retains_only_explicitly_verified_generations() {
    let mut diagnostic = Diagnostic {
        outputs_verified: [true, false],
        header_attempted: [true; 4],
        header_completed: [true; 4],
        doorbell_attempted: [true; 2],
        doorbell_completed: [true; 2],
        last: Some((
            Observation {
                values: [0; 4],
                frontiers: [[1, 1], [3, 3]],
                producer_header: (0, 0x1402, 1),
            },
            Duration::from_millis(30),
        )),
        observation_count: 50,
        ..Diagnostic::default()
    };
    diagnostic.begin_generation(2);
    assert_eq!(diagnostic.outputs_verified, [true, false]);
    assert!(diagnostic.last.is_none());
    assert_eq!(diagnostic.observation_count, 0);
    assert_eq!(diagnostic.header_attempted, [false; 4]);
    assert_eq!(diagnostic.header_completed, [false; 4]);
    assert_eq!(diagnostic.doorbell_attempted, [false; 2]);
    assert_eq!(diagnostic.doorbell_completed, [false; 2]);
    assert!(diagnostic.report("stage failure")["last_observation"].is_null());
}

#[test]
fn dependency_terminal_diagnostics_do_not_read_device_or_change_publication_order() {
    let source = include_str!("engineering_gfx950_peer_dependency_canary.rs");
    let terminal = source
        .split("if let Err(error) = &result {")
        .nth(1)
        .unwrap()
        .split("group.finish(result)")
        .next()
        .unwrap();
    assert!(terminal.contains("diagnostic.report(error)"));
    for forbidden in [
        "Backend::",
        "observe(",
        "publish(",
        "reset_",
        "group.read",
        "release(",
        "close(",
    ] {
        assert!(
            !terminal.contains(forbidden),
            "terminal operation: {forbidden}"
        );
    }
    let publication = source
        .split("fn publish(")
        .nth(1)
        .unwrap()
        .split("fn require_held(")
        .next()
        .unwrap();
    assert!(
        publication.find("header_attempted[index] = true").unwrap()
            < publication
                .find("publish_engineering_dependency_header")
                .unwrap()
    );
    assert!(
        publication.find("publish_aql_header").unwrap()
            < publication.find("header_completed[index] = true").unwrap()
    );
    assert!(
        publication.find("doorbell_attempted[rank] = true").unwrap()
            < publication.find("store_packet_id_release").unwrap()
    );
    assert!(
        publication.find("store_packet_id_release").unwrap()
            < publication.find("doorbell_completed[rank] = true").unwrap()
    );
}

#[test]
fn dependency_typed_dependency_packet_remains_outside_generic_header_predicate() {
    let mut capture = PacketCapture::default();
    AqlDependencyBarrierAndPacketV1::new_unpublished(
        &[address(0x1000).unwrap()],
        address(0x2040).unwrap(),
    )
    .unwrap()
    .publish_with(&mut capture)
    .unwrap();
    let packet = capture.finish().unwrap();
    assert_eq!(packet.header, 0x1503);
    assert_eq!(&packet.bytes[..4], &1_u32.to_le_bytes());
    assert_eq!(&packet.bytes[8..16], &0x1000_u64.to_le_bytes());
    assert_eq!(&packet.bytes[56..64], &0x2040_u64.to_le_bytes());
    assert!(packet.bytes[16..56].iter().all(|byte| *byte == 0));
    assert!(!fe2o3_aql::is_reviewed_aql_publication_v1(0x1503, 0));
    let mut duplicate = PacketCapture::default();
    assert!(duplicate.header(0x1503).is_err());
    let mut duplicate = PacketCapture::default();
    duplicate.body(packet.bytes).unwrap();
    assert!(duplicate.body(packet.bytes).is_err());
}

#[test]
fn dependency_exact_residual_abi_and_fixed_dataflow_are_checked() {
    let hash: [u8; 32] = std::array::from_fn(|index| {
        u8::from_str_radix(&OBJECT_SHA[index * 2..index * 2 + 2], 16).unwrap()
    });
    let metadata = KernelMetadataV1 {
        symbol: SYMBOL.into(),
        object_sha256: hash,
        kernarg_bytes: 312,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(56),
        implicit_argument_bytes: 256,
        explicit_arguments: [
            (0, 8, true),
            (8, 8, false),
            (16, 8, true),
            (24, 8, false),
            (32, 8, true),
            (40, 8, false),
            (48, 4, false),
        ]
        .into_iter()
        .map(|(offset, bytes, global_buffer)| ExplicitArgumentV1 {
            offset,
            bytes,
            global_buffer,
            pointee_alignment: None,
            access: None,
        })
        .collect(),
    };
    check_metadata(&metadata).unwrap();
    let mut wrong = metadata.clone();
    wrong.kernarg_bytes = 56;
    assert!(check_metadata(&wrong).is_err());
    for index in 0..7 {
        let mut wrong = metadata.clone();
        wrong.explicit_arguments[index].offset += 4;
        assert!(check_metadata(&wrong).is_err());
        let mut wrong = metadata.clone();
        wrong.explicit_arguments[index].access = Some(BufferAccessV1::Read);
        assert!(check_metadata(&wrong).is_err());
    }
    let buffers = std::array::from_fn(|index| Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id: index as u64 + 1,
        owner: usize::from(index >= 3),
        bytes: if [0, 3].contains(&index) { 16384 } else { 8192 },
    });
    let [producer, consumer] = canary_bindings(buffers).unwrap();
    assert_eq!(producer[2].buffer, consumer[1].buffer);
    assert_eq!(producer[2].access, BufferAccessV1::Write);
    assert_eq!(consumer[1].access, BufferAccessV1::Read);
    for index in 0..5 {
        let mut wrong = buffers;
        wrong[index].group += 1;
        assert!(canary_bindings(wrong).is_err());
        let mut wrong = buffers;
        wrong[index].bytes -= 2;
        assert!(canary_bindings(wrong).is_err());
        let mut wrong = buffers;
        wrong[index].owner ^= 1;
        assert!(canary_bindings(wrong).is_err());
    }
    let mut wrong = buffers;
    wrong[4].id = wrong[2].id;
    assert!(canary_bindings(wrong).is_err());
    let arguments = arguments();
    assert_eq!(arguments.len(), 312);
    for offset in [8, 24, 40] {
        assert_eq!(&arguments[offset..offset + 8], &4096_u64.to_le_bytes());
    }
}

#[test]
fn dependency_two_generations_check_every_bf16_word_with_exact_finite_arithmetic() {
    let first = data(1);
    let second = data(2);
    for generation in 1..=2 {
        let GenerationData {
            partial: p,
            residual: r,
            consumer_partial: c,
            producer_expected: expected_p,
            consumer_expected: expected_c,
        } = data(generation);
        assert_eq!(
            (
                p.len(),
                r.len(),
                c.len(),
                expected_p.len(),
                expected_c.len()
            ),
            (16384, 8192, 16384, 8192, 8192)
        );
        for expected in [&expected_p, &expected_c] {
            assert_eq!(
                expected
                    .chunks_exact(2)
                    .map(|word| u16::from_le_bytes(word.try_into().unwrap()))
                    .collect::<BTreeSet<_>>()
                    .len(),
                ELEMENTS
            );
        }
        for index in 0..ELEMENTS {
            let partial = f32::from_le_bytes(p[index * 4..index * 4 + 4].try_into().unwrap());
            let residual = f32::from_bits(
                u32::from(u16::from_le_bytes(
                    r[index * 2..index * 2 + 2].try_into().unwrap(),
                )) << 16,
            );
            let own = f32::from_le_bytes(c[index * 4..index * 4 + 4].try_into().unwrap());
            let producer = (partial + residual).to_bits();
            let consumer = (partial + residual + own).to_bits();
            assert_eq!(producer & 0xffff, 0);
            assert_eq!(consumer & 0xffff, 0);
            assert_eq!(
                &expected_p[index * 2..index * 2 + 2],
                &((producer >> 16) as u16).to_le_bytes()
            );
            assert_eq!(
                &expected_c[index * 2..index * 2 + 2],
                &((consumer >> 16) as u16).to_le_bytes()
            );
            assert_ne!(
                &first.producer_expected[index * 2..index * 2 + 2],
                &second.producer_expected[index * 2..index * 2 + 2]
            );
            assert_ne!(
                &first.consumer_expected[index * 2..index * 2 + 2],
                &second.consumer_expected[index * 2..index * 2 + 2]
            );
        }
    }
}

// These source-order tripwires supplement the executable state-machine tests;
// they do not claim to simulate a native allocation, mapping, or queue.
#[test]
fn dependency_stage_and_publication_source_contract_has_no_late_reset() {
    let source = include_str!("engineering_gfx950_peer_dependency_canary.rs");
    let stage = source
        .split("fn stage(")
        .nth(1)
        .unwrap()
        .split("fn observe(")
        .next()
        .unwrap();
    assert!(
        stage.find("prior != [0; 4]").unwrap()
            < stage.find("reset_completion_signal_release").unwrap()
    );
    assert!(
        stage.find("reset_completion_signal_release").unwrap() < stage.find("let p =").unwrap()
    );
    assert!(stage.find("kernel_packet(").unwrap() < stage.find("reserve_batch").unwrap());
    assert!(stage.find("write_aql_slot").unwrap() < stage.find("fetch_add_aql_write").unwrap());
    let publication = source
        .split("fn publish(")
        .nth(1)
        .unwrap()
        .split("fn require_held(")
        .next()
        .unwrap();
    for forbidden in [
        "reset_completion",
        "allocate_resource",
        "kernel_packet",
        "with_bytes_mut",
        "write_aql_slot",
    ] {
        assert!(
            !publication.contains(forbidden),
            "late producer operation: {forbidden}"
        );
    }
    let entry = source
        .split("pub unsafe fn run_gfx950_tp2_dependency_canary_unchecked_v1(")
        .nth(1)
        .unwrap();
    assert!(
        entry
            .find("actual_p != expected_p || actual_c != expected_c")
            .unwrap()
            < entry.find("generations.push").unwrap()
    );
    assert!(entry.find("mapping.release").unwrap() < entry.find("group.close()").unwrap());
}

#[test]
fn dependency_owner_lifecycle_source_contract_retains_before_fallible_effects() {
    let source = include_str!("engineering_gfx950_peer_dependency_canary.rs");
    let initialize = source
        .split("fn initialize_signals(")
        .nth(1)
        .unwrap()
        .split("fn signals(")
        .next()
        .unwrap();
    assert!(
        initialize.find("dependency_internal.push").unwrap()
            < initialize
                .find("initialize_engineering_signal_slots")
                .unwrap()
    );
    let context = include_str!("engineering_gfx950.rs");
    let rollover = context
        .split("fn rollover_queue(")
        .nth(1)
        .unwrap()
        .split("fn close_inner(")
        .next()
        .unwrap();
    assert!(
        rollover
            .find("!self.dependency_internal.is_empty()")
            .unwrap()
            < rollover.find("destroy_queue()").unwrap()
    );
    let close = context
        .split("fn close_inner(")
        .nth(1)
        .unwrap()
        .split("struct Publication")
        .next()
        .unwrap();
    assert!(
        close.find("destroy_queue()").unwrap()
            < close.find("self.dependency_internal.pop()").unwrap()
    );
    let peer = include_str!("engineering_gfx950_peer.rs");
    let drop = peer
        .split("impl Drop for Gfx950EngineeringPeerGroupV1")
        .nth(1)
        .unwrap();
    assert!(drop.contains("std::mem::forget(std::mem::take(&mut self.contexts))"));
}
