//! Synthetic structural and writer tests; not a hardware qualification.
use super::*;
use crate::*;

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

fn probes() -> (RocgdbMiNativeProbeV4, RocgdbMiNativeInspectionProbeV5) {
    (
        RocgdbMiNativeProbeV4 {
            structured_mi_commands: true,
            direct_kfd_device_admitted: true,
            cooperative_v2_declaration: true,
            cooperative_v2_publication: true,
        },
        RocgdbMiNativeInspectionProbeV5 {
            register_names: true,
            register_values: true,
            simple_locals: true,
            disassembly: false,
            memory_bytes: false,
        },
    )
}

fn captured() -> RocgdbHardwareCaptureResponseV1 {
    let (probe, inspection_probe) = probes();
    RocgdbHardwareCaptureResponseV1 {
        schema: RocgdbHardwareCaptureSchemaV1::V1,
        observation_lifetime: RocgdbHardwareCaptureLifetimeV1::HistoricalSameStopCapture,
        result: RocgdbHardwareCaptureResultV1::Captured {
            probe,
            inspection_probe,
            locals_completion: RocgdbHardwareLocalsCompletionV1::Captured,
            projection: projection_fixture(),
        },
    }
}

fn unavailable(reason: RocgdbHardwareCaptureUnavailableV1) -> RocgdbHardwareCaptureResponseV1 {
    let (probe, inspection_probe) = probes();
    RocgdbHardwareCaptureResponseV1 {
        schema: RocgdbHardwareCaptureSchemaV1::V1,
        observation_lifetime: RocgdbHardwareCaptureLifetimeV1::HistoricalSameStopCapture,
        result: RocgdbHardwareCaptureResultV1::Unavailable {
            probe,
            inspection_probe,
            reason,
        },
    }
}

#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    flushes: usize,
    fail_flush: bool,
    fail_write: bool,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::other("synthetic write failure"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            Err(io::Error::other("synthetic flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn exact_historical_envelope_single_lf_and_flush() {
    let mut sink = Sink::default();
    write_rocgdb_hardware_capture_v1(&mut sink, &captured()).unwrap();
    assert_eq!(sink.flushes, 1);
    assert_eq!(sink.bytes.last(), Some(&b'\n'));
    assert_eq!(sink.bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    let value: serde_json::Value = serde_json::from_slice(&sink.bytes).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 3);
    assert_eq!(value["schema"], ROCGDB_HARDWARE_CAPTURE_SCHEMA_V1);
    assert_eq!(
        value["observation_lifetime"],
        "historical_same_stop_capture"
    );
    assert_eq!(value["result"].as_object().unwrap().len(), 5);
    assert_eq!(value["result"]["status"], "captured");
    assert_eq!(value["result"]["locals_completion"], "captured");
    assert!(value["result"]["projection"]["scope"].get("lane").is_none());
    assert!(
        value["result"]["projection"]["registers"]["registers"][0]
            .get("lane")
            .is_none()
    );
}

#[test]
fn invalid_projection_is_rejected_before_output_and_flush() {
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured { projection, .. } = &mut response.result else {
        panic!()
    };
    projection.stop_revision = 0;
    let mut sink = Sink::default();
    assert_eq!(
        write_rocgdb_hardware_capture_v1(&mut sink, &response),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
    );
    assert!(sink.bytes.is_empty());
    assert_eq!(sink.flushes, 0);
}

#[test]
fn captured_probe_and_completion_mutations_refuse_before_output() {
    for axis in 0..7 {
        let mut response = captured();
        let RocgdbHardwareCaptureResultV1::Captured {
            probe,
            inspection_probe,
            ..
        } = &mut response.result
        else {
            panic!()
        };
        match axis {
            0 => probe.structured_mi_commands = false,
            1 => probe.direct_kfd_device_admitted = false,
            2 => probe.cooperative_v2_declaration = false,
            3 => probe.cooperative_v2_publication = false,
            4 => inspection_probe.register_names = false,
            5 => inspection_probe.register_values = false,
            _ => inspection_probe.simple_locals = false,
        }
        let mut sink = Sink::default();
        assert_eq!(
            write_rocgdb_hardware_capture_v1(&mut sink, &response),
            Err(RocgdbHardwareCaptureErrorV1::InvalidProbe)
        );
        assert!(sink.bytes.is_empty());
        assert_eq!(sink.flushes, 0);
    }
}

#[test]
fn unsupported_locals_are_explicit_and_not_failed_locals() {
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured {
        inspection_probe,
        locals_completion,
        ..
    } = &mut response.result
    else {
        panic!()
    };
    inspection_probe.simple_locals = false;
    *locals_completion = RocgdbHardwareLocalsCompletionV1::CommandUnavailable;
    assert_eq!(response.validate(), Ok(()));
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured {
        locals_completion, ..
    } = &mut response.result
    else {
        panic!()
    };
    *locals_completion = RocgdbHardwareLocalsCompletionV1::CommandUnavailable;
    assert_eq!(
        response.validate(),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProbe)
    );
}

#[test]
fn registry_disassembly_and_memory_are_not_observations() {
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured {
        inspection_probe,
        projection,
        ..
    } = &mut response.result
    else {
        panic!()
    };
    inspection_probe.disassembly = true;
    inspection_probe.memory_bytes = true;
    assert!(matches!(
        projection.isa,
        RocgdbMiNativeUnavailableFieldV5::Unavailable { .. }
    ));
    assert!(matches!(
        projection.memory,
        RocgdbMiNativeUnavailableFieldV5::Unavailable { .. }
    ));
    response.validate().unwrap();
}

#[test]
fn unavailable_probe_relations_are_closed() {
    use RocgdbHardwareCaptureUnavailableV1 as U;
    use RocgdbMiNativeInspectionUnavailableReasonV5 as R;
    for reason in [
        R::MachineCommandUnavailable,
        R::BackendRejected,
        R::NotCaptured,
        R::RequiresAuthenticatedSourceMap,
        R::RequiresArtifactRelativeInstructionBinding,
        R::RequiresAllocationRelativeAuthority,
    ] {
        for supported in [false, true] {
            let expected = matches!(reason, R::NotCaptured)
                || (reason == R::MachineCommandUnavailable && !supported)
                || (reason == R::BackendRejected && supported);
            let mut registers = unavailable(U::RegisterInspection { reason });
            let RocgdbHardwareCaptureResultV1::Unavailable {
                inspection_probe, ..
            } = &mut registers.result
            else {
                panic!()
            };
            inspection_probe.register_values = supported;
            assert_eq!(registers.validate().is_ok(), expected);
            let mut locals = unavailable(U::LocalsInspection { reason });
            let RocgdbHardwareCaptureResultV1::Unavailable {
                inspection_probe, ..
            } = &mut locals.result
            else {
                panic!()
            };
            inspection_probe.simple_locals = supported;
            assert_eq!(locals.validate().is_ok(), expected);
        }
    }
    for reason in [
        U::RegisterInspection {
            reason: R::NotCaptured,
        },
        U::LocalsInspection {
            reason: R::NotCaptured,
        },
        U::ProjectionNotRetained {},
    ] {
        let mut response = unavailable(reason);
        let RocgdbHardwareCaptureResultV1::Unavailable { probe, .. } = &mut response.result else {
            panic!()
        };
        probe.cooperative_v2_publication = false;
        assert_eq!(
            response.validate(),
            Err(RocgdbHardwareCaptureErrorV1::InvalidUnavailable)
        );
    }
}

#[test]
fn native_failure_keeps_probe_history_and_closed_reason() {
    let mut response = unavailable(RocgdbHardwareCaptureUnavailableV1::NativeCapture {
        reason: RocgdbMiNativeUnavailableReasonV4::GpuStoppedStateUnavailable,
    });
    let RocgdbHardwareCaptureResultV1::Unavailable { probe, .. } = &mut response.result else {
        panic!()
    };
    probe.direct_kfd_device_admitted = false;
    let mut sink = Sink::default();
    write_rocgdb_hardware_capture_v1(&mut sink, &response).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&sink.bytes).unwrap();
    assert_eq!(value["result"]["reason"]["stage"], "native_capture");
    assert_eq!(
        value["result"]["reason"]["reason"],
        "gpu_stopped_state_unavailable"
    );
    assert_eq!(
        value["result"]["probe"]["direct_kfd_device_admitted"],
        false
    );
    assert!(value["result"].get("projection").is_none());
}

#[test]
fn full_u64_stop_revision_and_artifact_length_are_not_truncated() {
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured { projection, .. } = &mut response.result else {
        panic!()
    };
    projection.stop_revision = u64::MAX;
    projection.artifact.canonical_bytes = u64::MAX;
    let mut sink = Sink::default();
    write_rocgdb_hardware_capture_v1(&mut sink, &response).unwrap();
    let wire = std::str::from_utf8(&sink.bytes).unwrap();
    assert!(wire.contains("\"stop_revision\":18446744073709551615"));
    assert!(wire.contains("\"canonical_bytes\":18446744073709551615"));
}

#[test]
fn buffer_cap_counts_final_lf_and_does_not_mutate_on_refusal() {
    let mut buffer = CaptureBufferV1 {
        bytes: Vec::new(),
        limit_hit: false,
    };
    buffer
        .write_all(&vec![b'x'; MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1 - 1])
        .unwrap();
    buffer.write_all(b"\n").unwrap();
    assert_eq!(buffer.bytes.len(), MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1);
    let capacity = buffer.bytes.capacity();
    assert!(buffer.write_all(b"x").is_err());
    assert!(buffer.limit_hit);
    assert_eq!(buffer.bytes.len(), MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1);
    assert_eq!(buffer.bytes.capacity(), capacity);
    assert_eq!(buffer.bytes.last(), Some(&b'\n'));
}

#[test]
fn small_writes_use_geometric_not_per_token_growth() {
    let mut buffer = CaptureBufferV1 {
        bytes: Vec::new(),
        limit_hit: false,
    };
    let mut growths = 0;
    let mut prior = buffer.bytes.capacity();
    for _ in 0..65_536 {
        buffer.write_all(b"x").unwrap();
        let capacity = buffer.bytes.capacity();
        if capacity != prior {
            growths += 1;
            assert!(capacity >= prior.saturating_mul(2).max(256));
            prior = capacity;
        }
    }
    assert!(growths <= 9);
    assert_eq!(buffer.bytes.len(), 65_536);
}

#[test]
fn over_limit_row_count_is_refused_before_writer_allocation() {
    let mut response = captured();
    let RocgdbHardwareCaptureResultV1::Captured { projection, .. } = &mut response.result else {
        panic!()
    };
    projection.registers.registers = vec![projection.registers.registers[0].clone(); 1025];
    let mut sink = Sink::default();
    assert_eq!(
        write_rocgdb_hardware_capture_v1(&mut sink, &response),
        Err(RocgdbHardwareCaptureErrorV1::InvalidProjection)
    );
    assert!(sink.bytes.is_empty());
    assert_eq!(sink.flushes, 0);
}

#[test]
fn actual_writer_write_and_flush_failures_are_reported() {
    let mut sink = Sink {
        fail_write: true,
        ..Sink::default()
    };
    assert_eq!(
        write_rocgdb_hardware_capture_v1(&mut sink, &captured()),
        Err(RocgdbHardwareCaptureErrorV1::OutputIo)
    );
    assert_eq!(sink.flushes, 0);
    let mut sink = Sink {
        fail_flush: true,
        ..Sink::default()
    };
    assert_eq!(
        write_rocgdb_hardware_capture_v1(&mut sink, &captured()),
        Err(RocgdbHardwareCaptureErrorV1::OutputIo)
    );
    assert!(!sink.bytes.is_empty());
    assert_eq!(sink.flushes, 1);
}
