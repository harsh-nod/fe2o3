//! Synthetic adapter tests only; no launch, MI command, or hardware evidence.
use super::*;
use fe2o3_debug_protocol::*;

fn id(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn projection_fixture() -> RocgdbHardwareStopResourcesV1 {
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

fn missing<T>() -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Unavailable {
        reason: LiveGpuUnavailableReasonV3::NotCaptured,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Unavailable,
            evidence: vec![],
        },
    }
}

fn native_fixture() -> (RocgdbMiNativeCliResponseV5, RocgdbHardwareStopResourcesV1) {
    let projection = projection_fixture();
    let stopped = RocgdbMiNativeStoppedStateV4 {
        association_identity: projection.association_identity,
        queue_occurrence_identity: projection.queue_occurrence_identity,
        process_instance_identity: projection.process_instance_identity,
        dispatch_identity: projection.dispatch_identity,
        artifact: projection.artifact,
        grid: projection.grid,
        workgroup: projection.workgroup,
        workgroup_coordinate: projection.workgroup_coordinate,
        wave_identity: projection.scope.wave.identity,
        wave_in_workgroup: projection.wave_in_workgroup,
        lanes: (0..64_u16)
            .map(|lane| RocgdbMiNativeLaneV4 {
                lane_identity: id(40 + u8::try_from(lane).unwrap()),
                lane_index: lane,
                workitem: RocgdbMiWorkitemCoordinateV4 {
                    x: 64 + u32::from(lane),
                    y: 0,
                    z: 0,
                },
                active: missing(),
            })
            .collect(),
        relative_pc: missing(),
        source: missing(),
        registers: missing(),
        memory: missing(),
        origins: vec![
            RocgdbMiNativeCorrelationOriginV4::TargetKfdPublicationObservation,
            RocgdbMiNativeCorrelationOriginV4::RocgdbStructuredObservation,
            RocgdbMiNativeCorrelationOriginV4::ExplicitCodeObjectAdmission,
            RocgdbMiNativeCorrelationOriginV4::Correlated,
        ],
    };
    let inspection = RocgdbMiNativeInspectionV5 {
        association_identity: projection.association_identity,
        scope: projection.scope,
        registers: RocgdbMiNativeCapturedV5::Captured {
            evidence_identity: projection.register_evidence_identity,
            value: projection.registers.clone(),
        },
        locals: RocgdbMiNativeCapturedV5::Captured {
            evidence_identity: id(12),
            value: RocgdbMiValueSnapshotV3 {
                scope: projection.scope,
                values: vec![],
            },
        },
        source: projection.source,
        isa: projection.isa,
        memory: projection.memory,
    };
    let native = RocgdbMiNativeCliResponseV5 {
        schema: RocgdbMiNativeCliResponseSchemaV5::V5,
        result: RocgdbMiNativeCliResultV5::Available {
            probe: RocgdbMiNativeProbeV4 {
                structured_mi_commands: true,
                direct_kfd_device_admitted: true,
                cooperative_v2_declaration: true,
                cooperative_v2_publication: true,
            },
            inspection_probe: RocgdbMiNativeInspectionProbeV5 {
                register_names: true,
                register_values: true,
                simple_locals: true,
                disassembly: false,
                memory_bytes: false,
            },
            stopped_state: Box::new(stopped),
            inspection: Box::new(inspection),
        },
    };
    native.validate().unwrap();
    projection.validate().unwrap();
    (native, projection)
}

#[test]
fn synthetic_exact_capture_moves_inert_projection_unchanged() {
    let (native, projection) = native_fixture();
    let expected = projection.clone();
    let response = historical_response(native, Some(projection)).unwrap();
    assert_eq!(
        response.observation_lifetime,
        RocgdbHardwareCaptureLifetimeV1::HistoricalSameStopCapture
    );
    let RocgdbHardwareCaptureResultV1::Captured {
        locals_completion,
        projection,
        ..
    } = response.result
    else {
        panic!()
    };
    assert_eq!(
        locals_completion,
        RocgdbHardwareLocalsCompletionV1::Captured
    );
    assert_eq!(projection, expected);
}

#[test]
fn every_native_failure_retains_exact_reason_and_probe_history() {
    use RocgdbMiNativeUnavailableReasonV4 as R;
    for reason in [
        R::RocgdbSpawnFailed,
        R::StructuredCommandsUnavailable,
        R::DirectKfdDeviceUnavailable,
        R::TargetLaunchFailed,
        R::CooperativeTelemetryUnavailable,
        R::TargetExitedBeforePublication,
        R::NativePublicationNotObserved,
        R::GpuStoppedStateUnavailable,
        R::CorrelationRejected,
    ] {
        let probe = RocgdbMiNativeProbeV4 {
            structured_mi_commands: true,
            direct_kfd_device_admitted: false,
            cooperative_v2_declaration: false,
            cooperative_v2_publication: false,
        };
        let inspection_probe = RocgdbMiNativeInspectionProbeV5 {
            register_names: true,
            ..RocgdbMiNativeInspectionProbeV5::default()
        };
        let native = RocgdbMiNativeCliResponseV5 {
            schema: RocgdbMiNativeCliResponseSchemaV5::V5,
            result: RocgdbMiNativeCliResultV5::Unavailable {
                probe,
                inspection_probe,
                reason,
            },
        };
        assert_eq!(
            historical_response(native.clone(), Some(projection_fixture())),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
        );
        assert_eq!(
            historical_response(native, None).unwrap().result,
            RocgdbHardwareCaptureResultV1::Unavailable {
                probe,
                inspection_probe,
                reason: RocgdbHardwareCaptureUnavailableV1::NativeCapture { reason },
            }
        );
    }
}

#[test]
fn register_unavailable_and_rejected_are_not_successful_captures() {
    use RocgdbMiNativeInspectionUnavailableReasonV5 as R;
    for (reason, supported) in [
        (R::MachineCommandUnavailable, false),
        (R::BackendRejected, true),
        (R::NotCaptured, true),
    ] {
        let (mut native, projection) = native_fixture();
        let RocgdbMiNativeCliResultV5::Available {
            inspection_probe,
            inspection,
            ..
        } = &mut native.result
        else {
            panic!()
        };
        inspection_probe.register_values = supported;
        inspection.registers = RocgdbMiNativeCapturedV5::Unavailable { reason };
        native.validate().unwrap();
        assert_eq!(
            historical_response(native.clone(), Some(projection)),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
        );
        let response = historical_response(native, None).unwrap();
        assert!(
            matches!(response.result, RocgdbHardwareCaptureResultV1::Unavailable {
            reason: RocgdbHardwareCaptureUnavailableV1::RegisterInspection { reason: actual }, ..
        } if actual == reason)
        );
    }
}

#[test]
fn unavailable_locals_command_is_explicit_but_failed_locals_are_refused() {
    use RocgdbMiNativeInspectionUnavailableReasonV5 as R;
    let (mut native, projection) = native_fixture();
    let RocgdbMiNativeCliResultV5::Available {
        inspection_probe,
        inspection,
        ..
    } = &mut native.result
    else {
        panic!()
    };
    inspection_probe.simple_locals = false;
    inspection.locals = RocgdbMiNativeCapturedV5::Unavailable {
        reason: R::MachineCommandUnavailable,
    };
    let response = historical_response(native, Some(projection)).unwrap();
    assert!(matches!(
        response.result,
        RocgdbHardwareCaptureResultV1::Captured {
            locals_completion: RocgdbHardwareLocalsCompletionV1::CommandUnavailable,
            ..
        }
    ));
    for reason in [R::BackendRejected, R::NotCaptured] {
        let (mut native, projection) = native_fixture();
        let RocgdbMiNativeCliResultV5::Available { inspection, .. } = &mut native.result else {
            panic!()
        };
        inspection.locals = RocgdbMiNativeCapturedV5::Unavailable { reason };
        assert_eq!(
            historical_response(native.clone(), Some(projection)),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
        );
        assert!(matches!(historical_response(native, None).unwrap().result,
            RocgdbHardwareCaptureResultV1::Unavailable {
                reason: RocgdbHardwareCaptureUnavailableV1::LocalsInspection { reason: actual }, ..
            } if actual == reason));
    }
}

#[test]
fn missing_retained_projection_is_explicit_not_reconstructed_from_v5() {
    let (native, _) = native_fixture();
    assert!(matches!(
        historical_response(native, None).unwrap().result,
        RocgdbHardwareCaptureResultV1::Unavailable {
            reason: RocgdbHardwareCaptureUnavailableV1::ProjectionNotRetained {},
            ..
        }
    ));
}

#[test]
fn exact_cross_record_binding_mutations_are_refused() {
    for axis in 0..11 {
        let (native, mut projection) = native_fixture();
        match axis {
            0 => projection.association_identity = id(20),
            1 => projection.queue_occurrence_identity = id(20),
            2 => projection.process_instance_identity = id(20),
            3 => projection.dispatch_identity = id(20),
            4 => projection.artifact.digest = id(20),
            5 => projection.artifact.canonical_bytes += 1,
            6 => projection.grid[0] = 192,
            7 => {
                projection.workgroup[0] = 128;
                projection.workgroup_coordinate.x = 0;
            }
            8 => projection.workgroup_coordinate.x = 0,
            9 => {
                projection.scope.stop_identity = id(20);
                projection.registers.scope = projection.scope;
            }
            _ => {
                projection.register_evidence_identity = id(20);
                let LiveGpuAvailabilityV3::Available { truth, .. } =
                    &mut projection.registers.registers[0].value
                else {
                    panic!()
                };
                truth.evidence[0].identity = id(20);
            }
        }
        projection.validate().unwrap();
        assert_eq!(
            historical_response(native, Some(projection)),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
        );
    }
}

#[test]
fn exact_register_value_and_row_identity_mutations_are_refused() {
    for axis in 0..3 {
        let (native, mut projection) = native_fixture();
        let row = &mut projection.registers.registers[0];
        match axis {
            0 => row.register_identity = id(20),
            1 => row.name = "s5".into(),
            _ => {
                let LiveGpuAvailabilityV3::Available {
                    value: LiveGpuValueEncodingV3::Bits { bits, .. },
                    ..
                } = &mut row.value
                else {
                    panic!()
                };
                *bits = "0000002b".into();
            }
        }
        projection.validate().unwrap();
        assert_eq!(
            historical_response(native, Some(projection)),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
        );
    }
}

#[test]
fn unsupported_vector_never_becomes_wave_scalar_bits() {
    let (mut native, mut projection) = native_fixture();
    let RocgdbMiNativeCliResultV5::Available { inspection, .. } = &mut native.result else {
        panic!()
    };
    let RocgdbMiNativeCapturedV5::Captured { value, .. } = &mut inspection.registers else {
        panic!()
    };
    value.registers[0].class = LiveGpuRegisterClassV3::Vector;
    value.registers[0].name = "v4".into();
    projection.registers.registers[0].class = LiveGpuRegisterClassV3::Vector;
    projection.registers.registers[0].name = "v4".into();
    projection.registers.registers[0].value = LiveGpuAvailabilityV3::Unavailable {
        reason: LiveGpuUnavailableReasonV3::Unsupported,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Unavailable,
            evidence: vec![],
        },
    };
    let response = historical_response(native, Some(projection)).unwrap();
    let RocgdbHardwareCaptureResultV1::Captured { projection, .. } = response.result else {
        panic!()
    };
    assert!(matches!(
        projection.registers.registers[0].value,
        LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::Unsupported,
            ..
        }
    ));
}

#[test]
fn inconsistent_probe_does_not_turn_projection_into_success() {
    let (mut native, projection) = native_fixture();
    let RocgdbMiNativeCliResultV5::Available {
        inspection_probe, ..
    } = &mut native.result
    else {
        panic!()
    };
    inspection_probe.simple_locals = false;
    assert_eq!(
        historical_response(native, Some(projection)),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
    );
}

#[test]
fn explicit_command_selects_new_output_without_changing_old_defaults() {
    for (command, expected) in [
        ("live-rocgdb-kfd-v4", OutputVersion::V4),
        ("live-rocgdb-kfd-v5", OutputVersion::V5),
        (
            "capture-rocgdb-kfd-resources-v1",
            OutputVersion::HistoricalResourcesV1,
        ),
    ] {
        let args = [
            command,
            "--rocgdb",
            "/usr/bin/rocgdb",
            "--authorization",
            "0101010101010101010101010101010101010101010101010101010101010101",
            "--hsaco",
            "/tmp/kernel.hsaco",
            "--load-base",
            "0x1000",
            "--kernel",
            "kernel",
            "--",
            "/bin/true",
        ];
        let options = parse_options(args.into_iter().map(OsString::from).collect()).unwrap();
        assert_eq!(options.output, expected);
        assert_eq!(options.wave_width, 64);
        assert_eq!(options.timeout, Duration::from_secs(10));
        assert!(options.arguments.is_empty());
    }
}

#[test]
fn new_command_does_not_accept_target_or_attach_authority_flags() {
    for flag in ["--target", "--attach", "--captured-projection"] {
        let args = [
            "capture-rocgdb-kfd-resources-v1",
            "--rocgdb",
            "/usr/bin/rocgdb",
            "--authorization",
            "0101010101010101010101010101010101010101010101010101010101010101",
            "--hsaco",
            "/tmp/kernel.hsaco",
            "--load-base",
            "0x1000",
            "--kernel",
            "kernel",
            flag,
            "gfx950",
            "--",
            "/bin/true",
        ];
        assert!(parse_options(args.into_iter().map(OsString::from).collect()).is_err());
    }
}

#[test]
fn pc_redaction_is_preserved_and_not_reused_after_value_unavailability() {
    let (mut native, mut projection) = native_fixture();
    let row = &mut projection.registers.registers[0];
    row.name = "pc".into();
    row.class = LiveGpuRegisterClassV3::Special;
    let truth = match &row.value {
        LiveGpuAvailabilityV3::Available { truth, .. } => truth.clone(),
        _ => panic!(),
    };
    row.value = LiveGpuAvailabilityV3::Redacted {
        reason: LiveGpuRedactionReasonV3::AbsoluteTargetLocation,
        truth,
    };
    let expected = row.value.clone();
    let RocgdbMiNativeCliResultV5::Available { inspection, .. } = &mut native.result else {
        panic!()
    };
    let RocgdbMiNativeCapturedV5::Captured { value, .. } = &mut inspection.registers else {
        panic!()
    };
    value.registers = projection.registers.registers.clone();
    native.validate().unwrap();
    projection.validate().unwrap();
    let response = historical_response(native.clone(), Some(projection.clone())).unwrap();
    let RocgdbHardwareCaptureResultV1::Captured {
        projection: actual, ..
    } = response.result
    else {
        panic!()
    };
    assert_eq!(actual.registers.registers[0].value, expected);
    let wire = serde_json::to_string(&actual).unwrap();
    assert!(wire.contains("absolute_target_location"));
    assert!(!wire.contains("0000002a"));

    let RocgdbMiNativeCliResultV5::Available { inspection, .. } = &mut native.result else {
        panic!()
    };
    let RocgdbMiNativeCapturedV5::Captured { value, .. } = &mut inspection.registers else {
        panic!()
    };
    value.registers[0].value = missing();
    native.validate().unwrap();
    assert_eq!(
        historical_response(native, Some(projection)),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
    );
}

#[test]
fn supported_unavailable_reason_is_preserved_not_substituted() {
    let (mut native, mut projection) = native_fixture();
    projection.registers.registers[0].value = missing();
    let RocgdbMiNativeCliResultV5::Available { inspection, .. } = &mut native.result else {
        panic!()
    };
    let RocgdbMiNativeCapturedV5::Captured { value, .. } = &mut inspection.registers else {
        panic!()
    };
    value.registers[0].value = projection.registers.registers[0].value.clone();
    native.validate().unwrap();
    projection.validate().unwrap();
    let response = historical_response(native.clone(), Some(projection.clone())).unwrap();
    let RocgdbHardwareCaptureResultV1::Captured {
        projection: actual, ..
    } = response.result
    else {
        panic!()
    };
    assert_eq!(
        actual.registers.registers[0].value,
        projection.registers.registers[0].value
    );
    let LiveGpuAvailabilityV3::Unavailable { reason, .. } =
        &mut projection.registers.registers[0].value
    else {
        panic!()
    };
    *reason = LiveGpuUnavailableReasonV3::OptimizedOut;
    projection.validate().unwrap();
    assert_eq!(
        historical_response(native, Some(projection)),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
    );
}
