//! Synthetic canonical-owner tests, not ordinary-source or hardware qualification.
//! The fixture has declared synthetic source IDs; no source map is authenticated.
use super::*;
use fe2o3_kir_sim::{BufferArgumentV1, EventPolicyV1};
use fe2o3_kir_sim_cli::load_debug_simulation_input_bytes_v16;
use std::path::Path;

#[path = "../../fe2o3-kir-sim-cli/tests/fixtures/diagnostic_kir_v16.rs"]
pub(crate) mod fixture;

fn input(used: bool, arguments: [u32; 3]) -> AdmittedSimulationInputV1 {
    let owner = fixture::owner(&fixture::module(used));
    load_debug_simulation_input_bytes_v16(owner.canonical_bytes(), &fixture::request(arguments))
        .unwrap()
}

fn capture_limits() -> SimulationDebugCaptureLimitsV1 {
    SimulationDebugCaptureLimitsV1::new(64, 4096, 16384, 16 * 1024 * 1024).unwrap()
}

fn debugger_limits() -> DebuggerLimitsV1 {
    DebuggerLimitsV1::new(1_000_000, 16_000_000, 256 * 1024 * 1024).unwrap()
}

fn identity(input: &AdmittedSimulationInputV1) -> OpaqueIdentityV1 {
    configuration_identity(
        input,
        DebugWaveWidthV1::Wave64,
        capture_limits(),
        debugger_limits(),
    )
    .unwrap()
}

#[test]
fn diagnostic_options_select_only_explicit_v16_path_and_wave64() {
    let options = parse_options(
        [
            "sim",
            "--diagnostic-kir-v16",
            "/missing/region.kir",
            "--request",
            "/missing/request.json",
            "--protocol",
            "jsonl",
        ]
        .into_iter()
        .map(OsString::from),
    )
    .unwrap();
    assert!(
        matches!(options.program, ProgramInputV1::DiagnosticKirV16(path) if path == Path::new("/missing/region.kir"))
    );
    assert!(
        matches!(options.request, RequestInputV1::Path(path) if path == Path::new("/missing/request.json"))
    );
    assert_eq!(options.wave_width, DebugWaveWidthV1::Wave64);
    assert!(options.source_map.is_none());
    assert!(options.replay_schedule.is_none());
    for other in [
        "--kir-v7",
        "--kir-v7-fd",
        "--bundle",
        "--bundle-v2",
        "--bundle-v3",
        "--bundle-v4",
        "--bundle-v5",
        "--bundle-v6",
        "--diagnostic-kir-v16",
    ] {
        let value = if other == "--kir-v7-fd" {
            "7"
        } else {
            "/missing/second"
        };
        assert!(
            parse_options(
                [
                    "sim",
                    "--diagnostic-kir-v16",
                    "/missing/first",
                    other,
                    value,
                    "--request",
                    "/missing/request"
                ]
                .into_iter()
                .map(OsString::from)
            )
            .is_err(),
            "mixed or repeated flag {other}"
        );
    }
    for arguments in [
        vec!["sim", "--diagnostic-kir-v16", "/missing/first"],
        vec![
            "sim",
            "--diagnostic-kir-v16",
            "/missing/first",
            "--request-fd",
            "8",
        ],
        vec!["sim", "--diagnostic-kir-v16"],
    ] {
        assert!(parse_options(arguments.into_iter().map(OsString::from)).is_err());
    }
}

#[test]
fn unsupported_options_refuse_during_argument_parsing_before_path_admission() {
    let source_subject = "11".repeat(32);
    for extras in [
        vec!["--wave-width", "32"],
        vec!["--replay-schedule", "/missing/schedule"],
        vec![
            "--source-map",
            "/missing/source",
            "--source-bundle-subject",
            source_subject.as_str(),
        ],
    ] {
        let mut arguments = vec![
            "sim",
            "--diagnostic-kir-v16",
            "/missing/region",
            "--request",
            "/missing/request",
        ];
        arguments.extend(extras);
        let error = parse_options(arguments.into_iter().map(OsString::from)).unwrap_err();
        assert!(
            error.contains("diagnostic KIR V16 requires wave64"),
            "{error}"
        );
    }
    assert!(require_supported_options(DebugWaveWidthV1::Wave64, false, false).is_ok());
    for (wave, source_map, replay) in [
        (DebugWaveWidthV1::Wave32, false, false),
        (DebugWaveWidthV1::Wave64, true, false),
        (DebugWaveWidthV1::Wave64, false, true),
    ] {
        assert!(require_supported_options(wave, source_map, replay).is_err());
    }
    let result = SimulatorBackendV1::new(input(true, [19, 23, 42]), DebugWaveWidthV1::Wave32);
    assert!(matches!(result, Err(error) if error.contains("requires wave64")));
}

fn bindings(record: &SimulationDebugRecordV1) -> &[SimulationDebugBindingV1] {
    let SimulationDebugRecordKindV1::Checkpoint {
        stack: SimulationDebugCollectionV1::Captured(frames),
        ..
    } = &record.kind
    else {
        panic!("expected captured whole-region checkpoint");
    };
    assert_eq!(frames.len(), 1);
    let SimulationDebugCollectionV1::Captured(values) = &frames[0].values else {
        panic!("expected captured logical SSA values");
    };
    values
}

fn scalar(record: &SimulationDebugRecordV1, id: u32) -> Option<u32> {
    bindings(record)
        .iter()
        .find(|binding| binding.value == ValueId(id))
        .map(|binding| {
            let SimulationDebugValueV1::Scalar(value) = binding.observed else {
                panic!("expected scalar SSA binding");
            };
            assert_eq!(value.ty(), ScalarType::U32);
            u32::try_from(value.bits()).unwrap()
        })
}

fn last_checkpoint(backend: &mut SimulatorBackendV1) {
    let index = backend
        .session
        .transcript()
        .records()
        .iter()
        .rposition(|record| matches!(record.kind, SimulationDebugRecordKindV1::Checkpoint { .. }))
        .unwrap();
    assert!(matches!(
        backend.session.seek_record_index(index),
        DebugNavigationV1::Stopped(_)
    ));
    backend.revision += 1;
}

#[test]
fn canonical_owner_retains_logical_whole_region_pairs_without_source_or_physical_claims() {
    for used in [true, false] {
        for arguments in [[19, 23, 42], [u32::MAX, 0, 1], [0, 0, 0]] {
            let admitted = input(used, arguments);
            let expected_identity = *admitted.module.identity().digest();
            let mut backend = SimulatorBackendV1::new(admitted, DebugWaveWidthV1::Wave64).unwrap();
            assert_eq!(backend.module.identity().wire_version(), 16);
            assert_eq!(*backend.module.identity().digest(), expected_identity);
            assert!(!backend.module.grants_execution_authority());
            assert!(!backend.failed_execution);
            assert_eq!(
                backend.session.transcript().completeness(),
                DebugTranscriptCompletenessV1::Complete
            );
            assert!(backend.source_map_identity.is_none());
            assert!(backend.source_variables_v2.is_none());
            assert!(backend.diagnosis_input.is_none());
            let mut before = 0;
            let mut after = 0;
            let mut lanes = std::collections::BTreeSet::new();
            for record in backend.session.transcript().records() {
                if record.site.operation != 0 {
                    continue;
                }
                assert_eq!(record.site.block, fe2o3_kernel_ir::BlockId(7));
                assert!(matches!(
                    backend.source_availability(record.site),
                    SourceSiteAvailabilityV1::Unavailable {
                        reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap
                    }
                ));
                match record.kind {
                    SimulationDebugRecordKindV1::Checkpoint {
                        phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
                        ..
                    } => {
                        before += 1;
                        assert_eq!(
                            [scalar(record, 0), scalar(record, 1), scalar(record, 2)],
                            arguments.map(Some)
                        );
                        assert_eq!(scalar(record, 4), None);
                    }
                    SimulationDebugRecordKindV1::Checkpoint {
                        phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                        ..
                    } => {
                        after += 1;
                        lanes.insert(record.invocation.global);
                        assert_eq!(
                            scalar(record, 4),
                            Some((arguments[0] ^ arguments[1]).wrapping_add(arguments[2]))
                        );
                    }
                    _ => panic!(
                        "No memory event or instruction microstep belongs inside this NoMemory region"
                    ),
                }
                assert!(bindings(record).iter().all(|binding| binding.value.0 < 32));
            }
            assert_eq!((before, after, lanes.len()), (64, 64, 64));
            last_checkpoint(&mut backend);
            let record = &backend.session.transcript().records()
                [backend.session.cursor_record_index().unwrap()];
            let SimulationDebugRecordKindV1::Checkpoint {
                memory: SimulationDebugCollectionV1::Captured(memory),
                ..
            } = &record.kind
            else {
                panic!("expected final memory checkpoint");
            };
            assert_eq!(memory.len(), 1);
            assert_eq!(memory[0].bytes, fixture::expected_output(arguments, used));
            assert!(memory[0].initialized.iter().all(|value| *value));
        }
    }
}

#[test]
fn diagnosis_v2_refuses_v16_instead_of_serializing_a_v7_evidence_label() {
    let mut backend =
        SimulatorBackendV1::new(input(true, [19, 23, 42]), DebugWaveWidthV1::Wave64).unwrap();
    let before = backend.session_view();
    let response = backend.handle_diagnosis_v2(DiagnosisRequestV2::Diagnose {
        schema: DiagnosisRequestSchemaV2::V2,
        request_id: 1,
        expected_revision: backend.revision,
        filter: DiagnosisFilterV2::default(),
        page: PageRequestV1 {
            cursor: None,
            limit: 8,
        },
    });
    response.validate(backend.protocol_limits).unwrap();
    assert!(
        matches!(&response, DiagnosisResponseV2::Error { error: DebugErrorV1 { code: DebugErrorCodeV1::UnsupportedSchema, state_changed: false, message, .. }, .. } if message == DIAGNOSIS_UNAVAILABLE)
    );
    let encoded = serde_json::to_string(&response).unwrap();
    assert!(!encoded.contains("canonical_kir_v7"));
    assert!(!encoded.contains("source_lineage"));
    assert_eq!(backend.session_view(), before);
}

fn access_query(backend: &SimulatorBackendV1, scan: u16) -> ResourceRequestV1 {
    ResourceRequestV1::QueryMemoryAccesses {
        schema: ResourceRequestSchemaV1::V1,
        request_id: 1,
        expected_revision: backend.revision,
        expected_snapshot: Box::new(backend.current_anchor(None).unwrap()),
        filter: ResourceMemoryAccessFilterV1 {
            scope: ExecutionScopeSelectorV1::Dispatch,
            allocation: None,
            address_space: None,
            access: None,
            range: None,
        },
        page: ResourceQueryPageV1 {
            max_items: 16,
            max_scanned: scan,
            token: None,
        },
    }
}

fn invalid_cursor(response: ResourceResponseV1) {
    response.validate(ProtocolLimitsV1::default()).unwrap();
    assert!(matches!(
        response,
        ResourceResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::InvalidCursor,
                state_changed: false,
                ..
            },
            ..
        }
    ));
}

#[test]
fn resource_pages_are_real_bounded_history_and_reject_cross_request_anchors_and_tokens() {
    let mut first =
        SimulatorBackendV1::new(input(true, [19, 23, 42]), DebugWaveWidthV1::Wave64).unwrap();
    let mut second =
        SimulatorBackendV1::new(input(true, [7, 11, 13]), DebugWaveWidthV1::Wave64).unwrap();
    last_checkpoint(&mut first);
    last_checkpoint(&mut second);
    assert_ne!(first.configuration_identity, second.configuration_identity);
    assert_eq!(first.module.identity(), second.module.identity());
    assert_eq!(first.revision, second.revision);
    let before = first.session_view();
    let mut query = access_query(&first, 1);
    invalid_cursor(second.handle_resource_queries_v1(query.clone()));
    let response = first.handle_resource_queries_v1(query.clone());
    response
        .validate_for_request(&query, first.protocol_limits)
        .unwrap();
    let ResourceResponseV1::Ok {
        page: ResourcePageInfoV1 {
            next_token: Some(token),
            ..
        },
        ..
    } = response
    else {
        panic!("expected bounded continuation");
    };
    if let ResourceRequestV1::QueryMemoryAccesses {
        page, request_id, ..
    } = &mut query
    {
        page.token = Some(token);
        *request_id += 1;
    }
    let mut foreign = query.clone();
    if let ResourceRequestV1::QueryMemoryAccesses {
        expected_snapshot, ..
    } = &mut foreign
    {
        **expected_snapshot = second.current_anchor(None).unwrap();
    }
    invalid_cursor(second.handle_resource_queries_v1(foreign));
    first
        .handle_resource_queries_v1(query.clone())
        .validate_for_request(&query, first.protocol_limits)
        .unwrap();
    invalid_cursor(first.handle_resource_queries_v1(query));
    let mut query = access_query(&first, 256);
    let mut offsets = Vec::new();
    let mut page_count = 0;
    loop {
        page_count += 1;
        assert!(page_count < 128, "bounded fixture history must terminate");
        let response = first.handle_resource_queries_v1(query.clone());
        response
            .validate_for_request(&query, first.protocol_limits)
            .unwrap();
        let ResourceResponseV1::Ok {
            page,
            result: ResourceQueryResultV1::MemoryAccesses { accesses },
            ..
        } = response
        else {
            panic!("expected access page");
        };
        assert!(page.scanned <= 256);
        for access in accesses {
            assert_eq!(access.access, ResourceMemoryAccessKindV1::WriteCommitted);
            assert_eq!(access.range.byte_len.get(), 4);
            assert_eq!(
                access.source_association,
                ResourceFactUnavailableV1::NotRepresented
            );
            offsets.push(access.range.byte_offset.get());
        }
        match page.next_token {
            None => break,
            Some(token) => {
                if let ResourceRequestV1::QueryMemoryAccesses {
                    page, request_id, ..
                } = &mut query
                {
                    page.token = Some(token);
                    *request_id += 1;
                }
            }
        }
    }
    assert_eq!(
        offsets,
        (0_u64..64).map(|lane| lane * 4).collect::<Vec<_>>()
    );
    assert_eq!(first.session_view(), before);
}

#[test]
fn all_fifteen_simulation_four_capture_and_three_debugger_limit_fields_bind_configuration() {
    let mut admitted = input(true, [19, 23, 42]);
    let base = identity(&admitted);
    let original = admitted.simulation_limits;
    let mut distinct = std::collections::BTreeSet::new();
    macro_rules! changed {
        ($($field:ident),+ $(,)?) => { $(
            admitted.simulation_limits = original;
            admitted.simulation_limits.$field = if original.$field > 1 {
                original.$field - 1
            } else {
                2
            };
            let actual = identity(&admitted);
            assert_ne!(actual, base, stringify!($field));
            assert!(distinct.insert(actual.as_bytes()), stringify!($field));
        )+ };
    }
    changed!(
        max_canonical_bytes,
        max_reachable_functions,
        max_reachable_operations,
        max_invocations,
        max_workgroups,
        max_scheduled_slots,
        max_steps,
        max_call_depth,
        max_ssa_values,
        max_allocations,
        max_allocation_bytes,
        max_total_bytes,
        max_resident_bytes,
        max_events,
        max_memory_access_records
    );
    assert_eq!(distinct.len(), 15);
    admitted.simulation_limits = original;
    for field in 0..4 {
        let mut values = [64, 4096, 16384, 16 * 1024 * 1024];
        values[field] -= 1;
        let capture =
            SimulationDebugCaptureLimitsV1::new(values[0], values[1], values[2], values[3])
                .unwrap();
        let actual = configuration_identity(
            &admitted,
            DebugWaveWidthV1::Wave64,
            capture,
            debugger_limits(),
        )
        .unwrap();
        assert_ne!(actual, base);
        assert!(distinct.insert(actual.as_bytes()));
    }
    for field in 0..3 {
        let mut values = [1_000_000, 16_000_000, 256 * 1024 * 1024];
        values[field] -= 1;
        let debugger = DebuggerLimitsV1::new(values[0], values[1], values[2]).unwrap();
        let actual = configuration_identity(
            &admitted,
            DebugWaveWidthV1::Wave64,
            capture_limits(),
            debugger,
        )
        .unwrap();
        assert_ne!(actual, base);
        assert!(distinct.insert(actual.as_bytes()));
    }
    assert_eq!(distinct.len(), 22);
    admitted.simulation_limits.max_steps = 0;
    assert!(
        configuration_identity(
            &admitted,
            DebugWaveWidthV1::Wave64,
            capture_limits(),
            debugger_limits()
        )
        .is_err()
    );
}

#[test]
fn post_admission_typed_request_changes_bind_even_if_original_raw_hash_is_unchanged() {
    let mut admitted = input(true, [19, 23, 42]);
    let base = identity(&admitted);
    let original = admitted.request.clone();
    let raw_sha = admitted.request_sha256;
    let raw_bytes = admitted.request_bytes();
    for field in 0..10 {
        admitted.request = original.clone();
        match field {
            0 => {
                admitted.request.arguments[0] = SimulationArgumentV1::Scalar(ScalarBitsV1::u32(20))
            }
            1 => admitted.request.grid.0[0] += 64,
            2 => admitted.request.workgroup.0[0] = 32,
            3 => admitted.request.kernel = "other_region".into(),
            4 => admitted.request.events = EventPolicyV1::Enabled,
            5 => admitted.request.arguments.swap(0, 1),
            6..=9 => {
                let SimulationArgumentV1::Buffer(buffer) = &original.arguments[3] else {
                    panic!("fixture buffer");
                };
                let mut bytes = buffer.bytes().to_vec();
                let mut initialized = buffer.initialized().to_vec();
                if field == 6 {
                    bytes[0] ^= 1;
                }
                if field == 7 {
                    initialized[0] = !initialized[0];
                }
                let alignment = if field == 8 { 8 } else { buffer.alignment() };
                let access = if field == 9 {
                    AccessMode::ReadOnly
                } else {
                    buffer.access()
                };
                admitted.request.arguments[3] = SimulationArgumentV1::Buffer(
                    BufferArgumentV1::new(
                        buffer.element(),
                        access,
                        alignment,
                        bytes,
                        initialized,
                        admitted.simulation_target(),
                    )
                    .unwrap(),
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(admitted.request_sha256, raw_sha);
        assert_eq!(admitted.request_bytes(), raw_bytes);
        assert_ne!(identity(&admitted), base, "typed request field {field}");
    }
    admitted.request = original;
    admitted.kir_sha256 = [99; 32];
    assert_eq!(
        identity(&admitted),
        base,
        "mutable metadata must not replace the owner identity"
    );
    admitted.module = input(false, [19, 23, 42]).module;
    assert_ne!(
        identity(&admitted),
        base,
        "actual immutable canonical owner must bind"
    );
    let other_owner_identity = identity(&admitted);
    admitted.request_sha256 = [88; 32];
    assert_ne!(
        identity(&admitted),
        other_owner_identity,
        "retained raw request digest also binds"
    );
}

#[test]
fn mismatched_public_request_index_layout_is_refused_before_retaining_a_session() {
    use fe2o3_kir_sim::{BufferBackingIdV1, BufferViewArgumentV1, SharedBufferV1};

    // These requests are deliberately changed by an embedding caller after raw
    // admission. Hidden target-width state is checked by mandatory CPU preflight,
    // not authenticated by the configuration hash or original request bytes.
    for variant in 0..4 {
        for index_width in [IndexWidthV1::Bits64, IndexWidthV1::Bits32] {
            let mut admitted = input(true, [19, 23, 42]);
            let target = SimulationTargetV1::little_endian(index_width);
            if variant < 3 {
                let mut module = fixture::module(true);
                module.functions[0]
                    .signature
                    .parameters
                    .push(if variant == 0 {
                        Type::INDEX
                    } else {
                        Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::ReadOnly)
                    });
                module.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .parameters
                    .push(ValueId(7));
                admitted.module = AdmittedSimulationModuleV1::admit_v16(
                    &fixture::owner(&module),
                    admitted.simulation_limits,
                )
                .unwrap();
            }
            match variant {
                0 => admitted
                    .request
                    .arguments
                    .push(SimulationArgumentV1::Scalar(
                        ScalarBitsV1::index(0, target).unwrap(),
                    )),
                1 => admitted
                    .request
                    .arguments
                    .push(SimulationArgumentV1::Buffer(
                        BufferArgumentV1::new(
                            ScalarType::Index,
                            AccessMode::ReadOnly,
                            8,
                            vec![0; 8],
                            vec![true; 8],
                            target,
                        )
                        .unwrap(),
                    )),
                2 => {
                    admitted.request.shared_buffers.push(SharedBufferV1 {
                        id: BufferBackingIdV1(17),
                        buffer: BufferArgumentV1::new(
                            ScalarType::Index,
                            AccessMode::ReadOnly,
                            8,
                            vec![0; 8],
                            vec![true; 8],
                            admitted.simulation_target(),
                        )
                        .unwrap(),
                    });
                    admitted
                        .request
                        .arguments
                        .push(SimulationArgumentV1::BufferView(
                            BufferViewArgumentV1::new(
                                BufferBackingIdV1(17),
                                ScalarType::Index,
                                AccessMode::ReadOnly,
                                8,
                                0,
                                1,
                                target,
                            )
                            .unwrap(),
                        ));
                }
                3 => admitted.request.shared_buffers.push(SharedBufferV1 {
                    id: BufferBackingIdV1(17),
                    buffer: BufferArgumentV1::new(
                        ScalarType::Index,
                        AccessMode::ReadOnly,
                        8,
                        vec![0; 8],
                        vec![true; 8],
                        target,
                    )
                    .unwrap(),
                }),
                _ => unreachable!(),
            }
            let result = SimulatorBackendV1::new(admitted, DebugWaveWidthV1::Wave64);
            if index_width == IndexWidthV1::Bits64 {
                assert!(
                    result.is_ok(),
                    "same-layout positive control {variant} failed"
                );
            } else {
                let error = match result {
                    Ok(_) => panic!("mismatched index layout {variant} retained a session"),
                    Err(error) => error,
                };
                assert!(
                    error.contains("different index layout"),
                    "case {variant}: {error}"
                );
            }
        }
    }
}

#[test]
fn configuration_hash_scans_are_bounded_before_variable_input_and_length_delimited() {
    let mut state = ConfigurationHash {
        hash: Sha256::new(),
        work: MAX_CONFIGURATION_WORK - 8,
    };
    state.bytes(&[]).unwrap();
    assert_eq!(state.work, MAX_CONFIGURATION_WORK);
    let accepted = state.hash.clone().finalize();
    assert!(
        state
            .bytes(&[1])
            .unwrap_err()
            .contains("logical work bound")
    );
    assert_eq!(state.hash.clone().finalize(), accepted);
    assert_eq!(state.work, MAX_CONFIGURATION_WORK);
    assert!(state.charge(usize::MAX).is_err());
    assert_eq!(state.work, MAX_CONFIGURATION_WORK);
    let admitted = input(true, [19, 23, 42]);
    assert!(state.request(&admitted.request).is_err());
    assert_eq!(state.hash.clone().finalize(), accepted);
    let SimulationArgumentV1::Buffer(buffer) = &admitted.request.arguments[3] else {
        panic!("fixture buffer");
    };
    assert!(state.buffer(buffer).is_err());
    assert_eq!(state.hash.finalize(), accepted);
    let mut first = ConfigurationHash {
        hash: Sha256::new(),
        work: 0,
    };
    let mut second = ConfigurationHash {
        hash: Sha256::new(),
        work: 0,
    };
    first.bytes(b"a").unwrap();
    first.bytes(b"bc").unwrap();
    second.bytes(b"ab").unwrap();
    second.bytes(b"c").unwrap();
    assert_ne!(first.hash.finalize(), second.hash.finalize());
}
