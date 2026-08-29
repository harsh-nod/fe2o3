use fe2o3_debug_protocol::*;

fn identity(seed: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([seed; 32]).unwrap()
}

fn evidence(seed: u8, kind: LiveGpuEvidenceKindV3) -> LiveGpuEvidenceRefV3 {
    LiveGpuEvidenceRefV3 {
        kind,
        identity: identity(seed),
    }
}

fn observed(seed: u8) -> LiveGpuTruthV3 {
    LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Observed,
        evidence: vec![evidence(seed, LiveGpuEvidenceKindV3::RuntimeObservation)],
    }
}

fn unavailable() -> LiveGpuTruthV3 {
    LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Unavailable,
        evidence: vec![],
    }
}

fn content(seed: u8, canonical_bytes: u64) -> LiveGpuContentIdentityV3 {
    LiveGpuContentIdentityV3 {
        digest: identity(seed),
        canonical_bytes,
    }
}

fn binding() -> LiveGpuArtifactBindingV3 {
    LiveGpuArtifactBindingV3 {
        binding_identity: identity(1),
        code_object_version: 6,
        declared_code_object: content(2, 4_096),
        declaration: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Declared,
            evidence: vec![evidence(22, LiveGpuEvidenceKindV3::Declaration)],
        },
        target_declared_code_object: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable(),
        },
        target_telemetry: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable(),
        },
        execution_code_object: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable(),
        },
        kernel_ir_v7: content(3, 2_048),
        source_map_v2: content(4, 1_024),
        isa_map_v1: Some(content(5, 512)),
        cpu_reference: LiveGpuCpuReferenceBindingV3 {
            bundle_identity: identity(23),
            request_identity: identity(24),
            configuration_identity: identity(25),
            deterministic_evidence: LiveGpuCpuReferenceEvidenceV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotCaptured,
            },
        },
    }
}

fn dispatch() -> LiveGpuDispatchIdentityV3 {
    LiveGpuDispatchIdentityV3 {
        domain: LiveGpuDispatchIdentityDomainV3::RuntimeModel,
        identity: identity(6),
    }
}

fn session() -> LiveGpuSessionViewV3 {
    LiveGpuSessionViewV3 {
        backend: LiveGpuBackendV3::DirectKfd,
        state: LiveGpuSessionStateV3::Stopped,
        revision: 9,
        commands_processed: 3,
        observation_sequence: 11,
        identity_generation: 2,
        runtime_enabled: true,
        binding_identity: binding().binding_identity,
    }
}

fn anchor() -> LiveGpuStoppedAnchorV3 {
    LiveGpuStoppedAnchorV3 {
        snapshot_identity: identity(7),
        stop_identity: identity(8),
        observation_sequence: 11,
        binding: binding(),
        dispatch: dispatch(),
        queue: HardwareQueueIdV2 {
            generation: 2,
            ordinal: 1,
        },
        reason: LiveGpuStopReasonV3::Breakpoint,
        truth: observed(9),
    }
}

#[test]
fn request_is_strict_bounded_and_stale_safe() {
    let line = format!(
        "{{\"schema\":\"{LIVE_GPU_REQUEST_SCHEMA_V3}\",\"operation\":\"inspect_stopped_scopes\",\"request_id\":7,\"expected_revision\":9,\"binding_identity\":\"{}\",\"stop_identity\":\"{}\",\"scope\":{{\"level\":\"wave\",\"dispatch\":{{\"domain\":\"runtime_model\",\"identity\":\"{}\"}},\"workgroup\":[1,2,3],\"wave\":4}},\"page\":{{\"snapshot_identity\":\"{}\",\"start\":0,\"limit\":16}}}}\n",
        "01".repeat(32),
        "08".repeat(32),
        "06".repeat(32),
        "07".repeat(32),
    );
    let request =
        decode_live_gpu_request_line_v3(line.as_bytes(), LiveGpuProtocolLimitsV3::default())
            .unwrap();
    assert_eq!(request.request_id(), 7);
    assert_eq!(request.expected_revision(), 9);
    assert_eq!(
        request.operation(),
        LiveGpuOperationV3::InspectStoppedScopes
    );

    for hostile in [
        format!(
            "{{\"schema\":\"{LIVE_GPU_REQUEST_SCHEMA_V3}\",\"operation\":\"get_state\",\"request_id\":1,\"expected_revision\":0,\"pid\":44}}\n"
        ),
        format!(
            "{{\"schema\":\"{LIVE_GPU_REQUEST_SCHEMA_V3}\",\"operation\":\"get_state\",\"request_id\":1,\"request_id\":2,\"expected_revision\":0}}\n"
        ),
        format!(
            "{{\"schema\":\"{LIVE_GPU_REQUEST_SCHEMA_V3}\",\"operation\":\"get_state\",\"request_id\":0,\"expected_revision\":0}}\n"
        ),
    ] {
        assert!(
            decode_live_gpu_request_line_v3(hostile.as_bytes(), LiveGpuProtocolLimitsV3::default())
                .is_err(),
            "hostile request admitted: {hostile}"
        );
    }

    let limits = LiveGpuProtocolLimitsV3 {
        max_request_line_bytes: 64,
        ..LiveGpuProtocolLimitsV3::default()
    };
    assert!(matches!(
        decode_live_gpu_request_line_v3(&[b'x'; 65], limits),
        Err(LiveGpuCodecErrorV3::LineTooLarge)
    ));
    assert!(matches!(
        decode_live_gpu_request_line_v3(b"{}", limits),
        Err(LiveGpuCodecErrorV3::MissingLineTerminator)
    ));
}

#[test]
fn running_session_wraps_kfd_lifecycle_without_claiming_a_stop() {
    let running = LiveGpuSessionViewV3 {
        state: LiveGpuSessionStateV3::Running,
        ..session()
    };
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 8,
        operation: LiveGpuOperationV3::InspectHardwareDevices,
        session: running,
        result: Box::new(LiveGpuDebugResultV3::Hardware {
            hardware: HardwareDebugResultV2::Devices {
                generation: 2,
                items: vec![HardwareDeviceViewV2 {
                    id: HardwareDeviceIdV2 {
                        generation: 2,
                        ordinal: 1,
                    },
                    gfx_target_version: 942,
                    xcc_count: 8,
                    trap_debug_supported: true,
                    debug_firmware_supported: true,
                    launch_mode_supported: true,
                    launch_override_supported: false,
                    precise_memory_supported: true,
                    precise_alu_supported: true,
                }],
                next_start: 0,
            },
        }),
    };
    let line =
        encode_live_gpu_response_line_v3(&response, LiveGpuProtocolLimitsV3::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("hardware"));
    assert!(!text.contains("stopped_state"));

    let state = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 9,
        operation: LiveGpuOperationV3::GetState,
        session: running,
        result: Box::new(LiveGpuDebugResultV3::State {
            stopped: LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::SessionNotStopped,
                truth: unavailable(),
            },
        }),
    };
    encode_live_gpu_response_line_v3(&state, LiveGpuProtocolLimitsV3::default()).unwrap();

    let binding_response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 10,
        operation: LiveGpuOperationV3::GetSessionBinding,
        session: running,
        result: Box::new(LiveGpuDebugResultV3::SessionBinding { binding: binding() }),
    };
    let line =
        encode_live_gpu_response_line_v3(&binding_response, LiveGpuProtocolLimitsV3::default())
            .unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("declared_code_object"));
    assert!(text.contains("target_declared_code_object"));
    assert!(text.contains("execution_code_object"));
    assert!(text.contains("cpu_reference"));
}

#[test]
fn stateful_errors_preserve_effect_and_terminal_state() {
    let poisoned = LiveGpuSessionViewV3 {
        state: LiveGpuSessionStateV3::Poisoned,
        ..session()
    };
    let response = LiveGpuDebugResponseV3::Error {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: Some(11),
        operation: Some(LiveGpuOperationV3::SuspendQueues),
        session: poisoned,
        error: LiveGpuErrorV3 {
            stage: LiveGpuErrorStageV3::Observation,
            code: LiveGpuErrorCodeV3::BackendFailure,
            effect: HardwareEffectV2::Indeterminate,
            terminal: true,
        },
    };
    let line =
        encode_live_gpu_response_line_v3(&response, LiveGpuProtocolLimitsV3::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("\"effect\":\"indeterminate\""));
    assert!(text.contains("\"terminal\":true"));

    let inconsistent = LiveGpuDebugResponseV3::Error {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: Some(12),
        operation: Some(LiveGpuOperationV3::SuspendQueues),
        session: poisoned,
        error: LiveGpuErrorV3 {
            stage: LiveGpuErrorStageV3::Observation,
            code: LiveGpuErrorCodeV3::BackendFailure,
            effect: HardwareEffectV2::None,
            terminal: false,
        },
    };
    assert!(
        encode_live_gpu_response_line_v3(&inconsistent, LiveGpuProtocolLimitsV3::default())
            .is_err()
    );
}

#[test]
fn stopped_scope_page_is_an_observed_subset_and_round_trips() {
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 12,
        operation: LiveGpuOperationV3::InspectStoppedScopes,
        session: session(),
        result: Box::new(LiveGpuDebugResultV3::Scopes {
            anchor: anchor(),
            coverage: LiveGpuStoppedCoverageV3::ObservedSubset,
            page: LiveGpuPageViewV3 {
                snapshot_identity: anchor().snapshot_identity,
                start: 0,
                limit: 16,
                returned: 2,
                next_start: Some(2),
            },
            items: vec![
                LiveGpuStoppedScopeV3::Wave {
                    dispatch: dispatch(),
                    workgroup: [1, 2, 3],
                    wave: 4,
                    wave_width: 64,
                    active_mask: LiveGpuAvailabilityV3::Available {
                        value: 0xff,
                        truth: observed(10),
                    },
                    truth: observed(11),
                },
                LiveGpuStoppedScopeV3::Lane {
                    dispatch: dispatch(),
                    workgroup: [1, 2, 3],
                    wave: 4,
                    lane: 9,
                    wave_width: 64,
                    active: LiveGpuAvailabilityV3::Unavailable {
                        reason: LiveGpuUnavailableReasonV3::NotObserved,
                        truth: unavailable(),
                    },
                    logical_workitem: LiveGpuAvailabilityV3::Unavailable {
                        reason: LiveGpuUnavailableReasonV3::NotObserved,
                        truth: unavailable(),
                    },
                    truth: observed(12),
                },
            ],
        }),
    };

    let line =
        encode_live_gpu_response_line_v3(&response, LiveGpuProtocolLimitsV3::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("observed_subset"));
    for forbidden in [
        "\"pid\"",
        "native_address",
        "queue_descriptor",
        "raw_pointer",
    ] {
        assert!(!text.contains(forbidden), "leaked field: {forbidden}");
    }
    assert_eq!(
        decode_live_gpu_response_line_v3(&line, LiveGpuProtocolLimitsV3::default()).unwrap(),
        response
    );
}

#[test]
fn truth_evidence_cardinality_and_unavailable_state_are_enforced() {
    let bad_anchor = LiveGpuStoppedAnchorV3 {
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Observed,
            evidence: vec![],
        },
        ..anchor()
    };
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 1,
        operation: LiveGpuOperationV3::GetState,
        session: session(),
        result: Box::new(LiveGpuDebugResultV3::State {
            stopped: LiveGpuAvailabilityV3::Available {
                value: Box::new(bad_anchor),
                truth: observed(26),
            },
        }),
    };
    assert!(matches!(
        response.validate(LiveGpuProtocolLimitsV3::default()),
        Err(LiveGpuValidationErrorV3::InvalidTruthEvidence)
    ));

    let bad_value = LiveGpuAvailabilityV3::<LiveGpuValueEncodingV3>::Unavailable {
        reason: LiveGpuUnavailableReasonV3::NotObserved,
        truth: observed(13),
    };
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 1,
        operation: LiveGpuOperationV3::InspectValues,
        session: session(),
        result: Box::new(LiveGpuDebugResultV3::Values {
            anchor: anchor(),
            scope: LiveGpuScopeSelectorV3::Dispatch {
                dispatch: dispatch(),
            },
            page: LiveGpuPageViewV3 {
                snapshot_identity: anchor().snapshot_identity,
                start: 0,
                limit: 1,
                returned: 1,
                next_start: None,
            },
            items: vec![LiveGpuSemanticValueV3 {
                value_identity: identity(14),
                name: "sum".to_string(),
                kind: LiveGpuValueKindV3::UnsignedInteger,
                value: bad_value,
            }],
        }),
    };
    assert!(matches!(
        response.validate(LiveGpuProtocolLimitsV3::default()),
        Err(LiveGpuValidationErrorV3::InvalidAvailability)
    ));
}

#[test]
fn program_sites_are_relative_and_source_map_bound() {
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 2,
        operation: LiveGpuOperationV3::ResolveProgramSite,
        session: session(),
        result: Box::new(LiveGpuDebugResultV3::ProgramSite {
            anchor: anchor(),
            scope: LiveGpuScopeSelectorV3::Lane {
                dispatch: dispatch(),
                workgroup: [0, 0, 0],
                wave: 0,
                lane: 3,
            },
            site: LiveGpuProgramSiteV3 {
                relative_pc: LiveGpuAvailabilityV3::Available {
                    value: LiveGpuRelativePcV3 {
                        kernel_entry_byte_offset: 48,
                    },
                    truth: observed(15),
                },
                isa: LiveGpuAvailabilityV3::Available {
                    value: LiveGpuIsaSiteV3 {
                        instruction_ordinal: 9,
                        kernel_entry_byte_offset: 48,
                        instruction_bytes: 4,
                    },
                    truth: observed(16),
                },
                kir: LiveGpuAvailabilityV3::Available {
                    value: KirSiteV1 {
                        function_ordinal: 0,
                        block_ordinal: 2,
                        point: KirSitePointV1::Operation {
                            operation_ordinal: 5,
                        },
                    },
                    truth: LiveGpuTruthV3 {
                        origin: LiveGpuTruthOriginV3::Inferred,
                        evidence: vec![evidence(17, LiveGpuEvidenceKindV3::InferenceRule)],
                    },
                },
                source: LiveGpuAvailabilityV3::Available {
                    value: LiveGpuSourceSpanV3 {
                        source_map_identity: binding().source_map_v2.digest,
                        file_identity: identity(18),
                        byte_start: 100,
                        byte_end: 120,
                    },
                    truth: LiveGpuTruthV3 {
                        origin: LiveGpuTruthOriginV3::Declared,
                        evidence: vec![evidence(19, LiveGpuEvidenceKindV3::Declaration)],
                    },
                },
            },
        }),
    };
    let line =
        encode_live_gpu_response_line_v3(&response, LiveGpuProtocolLimitsV3::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("kernel_entry_byte_offset"));
    assert!(!text.contains("absolute"));

    let mut wrong = binding();
    wrong.source_map_v2 = content(20, 1_024);
    let bad_session = LiveGpuSessionViewV3 {
        binding_identity: wrong.binding_identity,
        ..session()
    };
    let bad_anchor = LiveGpuStoppedAnchorV3 {
        binding: wrong,
        ..anchor()
    };
    let bad = match response {
        LiveGpuDebugResponseV3::Ok {
            schema,
            request_id,
            operation,
            result,
            ..
        } => match *result {
            LiveGpuDebugResultV3::ProgramSite { scope, site, .. } => LiveGpuDebugResponseV3::Ok {
                schema,
                request_id,
                operation,
                session: bad_session,
                result: Box::new(LiveGpuDebugResultV3::ProgramSite {
                    anchor: bad_anchor,
                    scope,
                    site,
                }),
            },
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
    assert!(matches!(
        bad.validate(LiveGpuProtocolLimitsV3::default()),
        Err(LiveGpuValidationErrorV3::IdentityMismatch("source map"))
    ));
}

#[test]
fn memory_identity_is_allocation_relative_and_unavailable_carries_no_bytes() {
    let response = LiveGpuDebugResponseV3::Ok {
        schema: LiveGpuResponseSchemaV3::V3,
        request_id: 3,
        operation: LiveGpuOperationV3::ReadMemory,
        session: session(),
        result: Box::new(LiveGpuDebugResultV3::Memory {
            anchor: anchor(),
            scope: LiveGpuScopeSelectorV3::Dispatch {
                dispatch: dispatch(),
            },
            memory: LiveGpuMemoryReadV3 {
                allocation: AllocationIdentityV1 {
                    ordinal: 4,
                    generation: 2,
                },
                byte_offset: 64,
                requested_bytes: 16,
                returned_bytes: 0,
                value: LiveGpuAvailabilityV3::Redacted {
                    reason: LiveGpuRedactionReasonV3::Policy,
                    truth: observed(21),
                },
            },
        }),
    };
    let line =
        encode_live_gpu_response_line_v3(&response, LiveGpuProtocolLimitsV3::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    assert!(text.contains("allocation"));
    assert!(text.contains("byte_offset"));
    assert!(!text.contains("raw_pointer"));
}

#[test]
fn public_v3_schema_has_no_raw_authority_fields() {
    let source = include_str!("../src/live_gpu_v3.rs");
    for forbidden in [
        "pub pid",
        "pub fd",
        "pub gpu_id",
        "pub queue_id",
        "pub address",
        "pub descriptor",
        "pub pointer",
        "pub argv",
    ] {
        assert!(
            !source.contains(forbidden),
            "live-GPU V3 exposed forbidden field fragment: {forbidden}"
        );
    }
}
