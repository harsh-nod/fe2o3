use super::*;

// These are deterministic fixture observations, not GPU qualification.
fn id(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn fixture() -> RocgdbHardwareStopResourcesV1 {
    let thread = RocgdbMiThreadIdentityV3 { identity: id(2) };
    let scope = RocgdbMiStoppedScopeV3 {
        stop_identity: id(3),
        thread,
        wave: RocgdbMiWaveIdentityV3 {
            identity: id(4),
            thread,
        },
        lane: None,
    };
    RocgdbHardwareStopResourcesV1 {
        target: RocgdbHardwareCheckedTargetV1::Gfx942XnackMinusWave64,
        session_identity: id(1), stop_revision: 7, association_identity: id(5),
        queue_occurrence_identity: id(6), process_instance_identity: id(7),
        dispatch_identity: id(8),
        artifact: LiveGpuContentIdentityV3 { digest: id(9), canonical_bytes: 128 },
        grid: [128,1,1], workgroup: [64,1,1],
        workgroup_coordinate: RocgdbMiWorkgroupCoordinateV4 { x: 1, y: 0, z: 0 },
        wave_in_workgroup: 0, scope, register_evidence_identity: id(10),
        registers: RocgdbMiRegisterSnapshotV3 { scope, registers: vec![
            LiveGpuRegisterValueV3 {
                register_identity: id(11), name: "s4".into(),
                class: LiveGpuRegisterClassV3::Scalar, kind: LiveGpuValueKindV3::UnsignedInteger,
                lane: None, value: LiveGpuAvailabilityV3::Available {
                    value: LiveGpuValueEncodingV3::Bits { bit_width: 32, bits: "0000002a".into() },
                    truth: LiveGpuTruthV3 { origin: LiveGpuTruthOriginV3::Observed,
                        evidence: vec![LiveGpuEvidenceRefV3 {
                            kind: LiveGpuEvidenceKindV3::RuntimeObservation, identity: id(10),
                        }],
                    },
                },
            },
        ] },
        source: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAuthenticatedSourceMap,
        },
        isa: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresArtifactRelativeInstructionBinding,
        },
        memory: RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAllocationRelativeAuthority,
        },
    }
}

fn stopped(value: &RocgdbHardwareStopResourcesV1) -> RocgdbMiNativeStoppedStateV4 {
    RocgdbMiNativeStoppedStateV4 {
        association_identity: value.association_identity,
        queue_occurrence_identity: value.queue_occurrence_identity,
        process_instance_identity: value.process_instance_identity,
        dispatch_identity: value.dispatch_identity,
        artifact: value.artifact,
        grid: value.grid,
        workgroup: value.workgroup,
        workgroup_coordinate: value.workgroup_coordinate,
        wave_identity: value.scope.wave.identity,
        wave_in_workgroup: value.wave_in_workgroup,
        lanes: (0_u16..64)
            .map(|lane| RocgdbMiNativeLaneV4 {
                lane_identity: id(u8::try_from(lane + 100).unwrap()),
                lane_index: lane,
                workitem: RocgdbMiWorkitemCoordinateV4 {
                    x: 64 + u32::from(lane),
                    y: 0,
                    z: 0,
                },
                active: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
            })
            .collect(),
        relative_pc: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
        source: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
        registers: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
        memory: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
        origins: vec![
            RocgdbMiNativeCorrelationOriginV4::TargetKfdPublicationObservation,
            RocgdbMiNativeCorrelationOriginV4::RocgdbStructuredObservation,
            RocgdbMiNativeCorrelationOriginV4::ExplicitCodeObjectAdmission,
            RocgdbMiNativeCorrelationOriginV4::Correlated,
        ],
    }
}

fn seeded_process() -> RocgdbMiProcessV3 {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fake_rocgdb_mi_v3.py");
    let mut process =
        RocgdbMiProcessV3::spawn(&path, id(1), id(2), 64, RocgdbMiAdapterLimitsV3::default())
            .unwrap();
    assert!(process.native_hardware_stop_resources_v1().is_none());
    process
        .adapter
        .admit_threads_from_thread_info(b"1^done,threads=[{id=\"9\"}]\n", &[0])
        .unwrap();
    process
        .adapter
        .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\"\n")
        .unwrap();
    let mut projection = fixture();
    projection.stop_revision = process.adapter.revision();
    process.native_stop_v4 = Some(RocgdbMiNativeStopPinV4 {
        revision: projection.stop_revision,
        identity: projection.scope.stop_identity,
    });
    process.hardware_stop_resources_v1 = Some(RocgdbHardwareStopResourceOwnerV1 { projection });
    assert!(process.native_hardware_stop_resources_v1().is_some());
    process
}

#[test]
fn bounded_projection_copies_exact_scalar_bits_and_binding() {
    let value = fixture();
    let stop = RocgdbMiNativeStopPinV4 {
        revision: value.stop_revision,
        identity: value.scope.stop_identity,
    };
    let actual = project_actual_capture(
        value.target,
        value.session_identity,
        stop,
        &stopped(&value),
        &value.registers,
        value.register_evidence_identity,
    )
    .unwrap();
    assert_eq!(actual, value);
}

#[test]
fn projection_rejects_foreign_stop_wave_and_32_lane_target() {
    let value = fixture();
    let mut stop = RocgdbMiNativeStopPinV4 {
        revision: value.stop_revision,
        identity: id(77),
    };
    assert!(
        project_actual_capture(
            value.target,
            value.session_identity,
            stop,
            &stopped(&value),
            &value.registers,
            value.register_evidence_identity
        )
        .is_err()
    );
    stop.identity = value.scope.stop_identity;
    let mut foreign = stopped(&value);
    foreign.wave_identity = id(78);
    assert!(
        project_actual_capture(
            value.target,
            value.session_identity,
            stop,
            &foreign,
            &value.registers,
            value.register_evidence_identity
        )
        .is_err()
    );
    let mut foreign = stopped(&value);
    foreign.lanes.truncate(32);
    assert!(
        project_actual_capture(
            value.target,
            value.session_identity,
            stop,
            &foreign,
            &value.registers,
            value.register_evidence_identity
        )
        .is_err()
    );
}

#[test]
fn vector_special_and_wide_rows_are_not_reinterpreted_as_scalar_lanes() {
    let value = fixture();
    for (name, class, width, bits) in [
        ("v0", LiveGpuRegisterClassV3::Vector, 32, "0000002a"),
        (
            "flat_scratch",
            LiveGpuRegisterClassV3::Special,
            32,
            "0000002a",
        ),
        (
            "s4",
            LiveGpuRegisterClassV3::Scalar,
            68,
            "0000000000000002a0",
        ),
    ] {
        let mut snapshot = value.registers.clone();
        let row = &mut snapshot.registers[0];
        row.name = name.into();
        row.class = class;
        let LiveGpuAvailabilityV3::Available { value, .. } = &mut row.value else {
            panic!()
        };
        *value = LiveGpuValueEncodingV3::Bits {
            bit_width: width,
            bits: bits.into(),
        };
        let projected = project_registers(&snapshot, id(10)).unwrap();
        assert!(matches!(
            projected.registers[0].value,
            LiveGpuAvailabilityV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::Unsupported,
                ..
            }
        ));
        assert_eq!(projected.registers[0].lane, None);
    }
}

#[test]
fn wrong_evidence_lane_and_unbounded_names_are_refused() {
    let value = fixture();
    assert!(project_registers(&value.registers, id(88)).is_err());
    let mut snapshot = value.registers.clone();
    snapshot.registers[0].lane = Some(0);
    assert!(project_registers(&snapshot, id(10)).is_err());
    let mut snapshot = value.registers.clone();
    snapshot.registers[0].name = "s".repeat(MAX_LIVE_GPU_TEXT_BYTES_V3 + 1);
    assert!(project_registers(&snapshot, id(10)).is_err());
    let mut snapshot = value.registers.clone();
    snapshot.registers = vec![snapshot.registers[0].clone(); MAX_ROCGDB_MI_REGISTERS_V3 + 1];
    assert!(project_registers(&snapshot, id(10)).is_err());
}

#[test]
fn pc_remains_redacted_and_cannot_be_promoted_to_value() {
    let value = fixture();
    let mut snapshot = value.registers.clone();
    let row = &mut snapshot.registers[0];
    row.name = "pc".into();
    row.class = LiveGpuRegisterClassV3::Special;
    row.value = LiveGpuAvailabilityV3::Redacted {
        reason: LiveGpuRedactionReasonV3::AbsoluteTargetLocation,
        truth: one_observation(id(10)).unwrap(),
    };
    let projected = project_registers(&snapshot, id(10)).unwrap();
    assert!(matches!(
        projected.registers[0].value,
        LiveGpuAvailabilityV3::Redacted { .. }
    ));
    assert!(
        !serde_json::to_string(&projected)
            .unwrap()
            .contains("0000002a")
    );
}

#[test]
fn accessor_requires_exact_session_stop_revision_without_queries() {
    let mut process = seeded_process();
    let token = process.next_token;
    let historical = process.native_hardware_stop_resources_v1().unwrap().clone();
    assert_eq!(process.next_token, token);
    let owner = process.hardware_stop_resources_v1.as_ref().unwrap();
    let stop = process.native_stop_v4.unwrap();
    assert!(!owner.matches(id(90), stop));
    assert!(!owner.matches(
        id(1),
        RocgdbMiNativeStopPinV4 {
            identity: id(91),
            ..stop
        }
    ));
    assert!(!owner.matches(
        id(1),
        RocgdbMiNativeStopPinV4 {
            revision: stop.revision + 1,
            ..stop
        }
    ));
    let mut copied = historical.clone();
    copied.artifact.digest = id(92);
    assert_eq!(
        process.native_hardware_stop_resources_v1().unwrap(),
        &historical
    );
    process.native_stop_v4 = None;
    assert!(process.native_hardware_stop_resources_v1().is_none());
}

#[test]
fn successful_same_stop_locals_preserve_projection_but_refusal_clears() {
    let mut process = seeded_process();
    let original = process.native_hardware_stop_resources_v1().unwrap().clone();
    process
        .inspect_native_locals_v5(b"9", original.scope, Duration::from_secs(5))
        .unwrap();
    assert_eq!(process.native_hardware_stop_resources_v1(), Some(&original));
    assert!(
        process
            .inspect_native_locals_v5(b"10", original.scope, Duration::from_secs(5))
            .is_err()
    );
    assert!(process.hardware_stop_resources_v1.is_none());
}

#[test]
fn early_inspection_scope_refusal_clears_without_dispatch() {
    let mut process = seeded_process();
    let mut scope = process.native_hardware_stop_resources_v1().unwrap().scope;
    scope.stop_identity = id(89);
    let token = process.next_token;
    assert!(
        process
            .inspect_native_locals_v5(b"9", scope, Duration::from_secs(5))
            .is_err()
    );
    assert_eq!(process.next_token, token);
    assert!(process.hardware_stop_resources_v1.is_none());
}

#[test]
fn ordinary_command_and_invalid_command_clear_before_dispatch() {
    for command in [
        b"-info-gdb-mi-command thread-info".as_slice(),
        b"".as_slice(),
    ] {
        let mut process = seeded_process();
        let _ = process.send_command(command, Instant::now() + Duration::from_secs(5));
        assert!(process.hardware_stop_resources_v1.is_none());
    }
}

#[test]
fn even_rejected_control_and_mutable_adapter_escape_clear() {
    let mut process = seeded_process();
    let request = RocgdbMiControlRequestV3::Launch {
        request_id: 1,
        authorization: RocgdbMiControlAuthorizationV3 {
            authorization_identity: id(2),
            expected_revision: process.adapter.revision(),
        },
    };
    assert_eq!(
        process.control(request, Duration::from_secs(5)),
        Err(RocgdbMiAdapterErrorV3::InvalidCommand)
    );
    assert!(process.hardware_stop_resources_v1.is_none());
    let mut process = seeded_process();
    let _ = process.adapter_mut();
    assert!(process.hardware_stop_resources_v1.is_none());
}

#[test]
fn all_native_stop_transitions_and_poll_uncertainty_clear() {
    for transition in [
        RocgdbMiNativeStopTransitionV4::Clear,
        RocgdbMiNativeStopTransitionV4::Stopped,
    ] {
        let mut process = seeded_process();
        process
            .apply_native_stop_transition_v4(transition, b"synthetic-next-stop")
            .unwrap();
        assert!(process.hardware_stop_resources_v1.is_none());
    }
    let mut process = seeded_process();
    assert!(process.next_event(Duration::ZERO).is_err());
    assert!(process.hardware_stop_resources_v1.is_none());
}

#[test]
fn transport_timeout_clears_projection_and_does_not_change_legacy_wire() {
    let mut process = seeded_process();
    assert!(
        process
            .send_native_projection_inspection_v1(
                b"-stack-list-variables --thread \"9\" --simple-values",
                Instant::now(),
            )
            .is_err()
    );
    assert!(process.hardware_stop_resources_v1.is_none());
    let legacy = RocgdbMiNativeCliResponseV5 {
        schema: RocgdbMiNativeCliResponseSchemaV5::V5,
        result: RocgdbMiNativeCliResultV5::Unavailable {
            probe: RocgdbMiNativeProbeV4 {
                structured_mi_commands: false,
                direct_kfd_device_admitted: false,
                cooperative_v2_declaration: false,
                cooperative_v2_publication: false,
            },
            inspection_probe: RocgdbMiNativeInspectionProbeV5::default(),
            reason: RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
        },
    };
    let wire = serde_json::to_string(&legacy).unwrap();
    assert!(!wire.contains("hardware_stop"));
    assert!(!wire.contains("checked_target"));
}

#[test]
fn historical_moveout_is_exact_once_without_commands() {
    let mut process = seeded_process();
    let expected = process.native_hardware_stop_resources_v1().unwrap().clone();
    let token = process.next_token;
    assert_eq!(
        process.take_historical_hardware_stop_resources_v1(),
        Some(expected)
    );
    assert_eq!(process.next_token, token);
    assert!(process.native_hardware_stop_resources_v1().is_none());
    assert!(
        process
            .take_historical_hardware_stop_resources_v1()
            .is_none()
    );
}

#[test]
fn historical_moveout_clears_stale_owner_without_exposure() {
    let mut process = seeded_process();
    process.native_stop_v4 = None;
    assert!(
        process
            .take_historical_hardware_stop_resources_v1()
            .is_none()
    );
    assert!(process.hardware_stop_resources_v1.is_none());
}
