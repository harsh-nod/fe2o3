use super::*;
use crate::{
    LiveGpuEvidenceRefV3, LiveGpuRegisterValueV3, LiveGpuTruthV3, RocgdbMiThreadIdentityV3,
    RocgdbMiWaveIdentityV3,
};

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

#[test]
fn synthetic_scalar_projection_is_inert_and_serializable() {
    let value = fixture();
    value.validate().unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert!(json.contains("gfx942_xnack_minus_wave64"));
    assert!(json.contains("requires_allocation_relative_authority"));
    assert!(!json.contains("load_base"));
    assert!(!json.contains("lane\":"));
}

#[test]
fn rejects_lane_bound_and_substituted_register_scope() {
    let mut value = fixture();
    value.registers.registers[0].lane = Some(0);
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
    let mut value = fixture();
    value.registers.scope.stop_identity = id(12);
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidBinding)
    );
    let mut value = fixture();
    value.scope.wave.thread.identity = id(12);
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidBinding)
    );
}

#[test]
fn rejects_vector_and_unknown_special_bits_as_wave_scalar_values() {
    for class in [
        LiveGpuRegisterClassV3::Vector,
        LiveGpuRegisterClassV3::Special,
    ] {
        let mut value = fixture();
        value.registers.registers[0].class = class;
        assert_eq!(
            value.validate(),
            Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
        );
    }
    let mut value = fixture();
    value.registers.registers[0].name = "s".into();
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
}

#[test]
fn refuses_register_evidence_and_origin_substitution() {
    let mut value = fixture();
    value.register_evidence_identity = id(13);
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
    let mut value = fixture();
    let LiveGpuAvailabilityV3::Available { truth, .. } = &mut value.registers.registers[0].value
    else {
        panic!()
    };
    truth.origin = LiveGpuTruthOriginV3::Declared;
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
}

#[test]
fn refuses_zero_revision_empty_artifact_and_bad_geometry() {
    let mut value = fixture();
    value.stop_revision = 0;
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidBinding)
    );
    let mut value = fixture();
    value.artifact.canonical_bytes = 0;
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidBinding)
    );
    for (grid, workgroup, coordinate, wave) in [
        ([128, 1, 1], [64, 1, 1], [2, 0, 0], 0),
        ([128, 1, 1], [64, 1, 1], [1, 0, 0], 1),
        (
            [u32::MAX, u32::MAX, 1],
            [u32::MAX, u32::MAX, 1],
            [0, 0, 0],
            0,
        ),
        ([0, 1, 1], [64, 1, 1], [0, 0, 0], 0),
    ] {
        let mut value = fixture();
        value.grid = grid;
        value.workgroup = workgroup;
        value.wave_in_workgroup = wave;
        value.workgroup_coordinate = RocgdbMiWorkgroupCoordinateV4 {
            x: coordinate[0],
            y: coordinate[1],
            z: coordinate[2],
        };
        assert_eq!(
            value.validate(),
            Err(RocgdbHardwareStopResourceErrorV1::InvalidGeometry)
        );
    }
}

#[test]
fn rejects_wide_values_and_duplicate_registers() {
    let mut value = fixture();
    let LiveGpuAvailabilityV3::Available { value: bits, .. } =
        &mut value.registers.registers[0].value
    else {
        panic!()
    };
    *bits = LiveGpuValueEncodingV3::Bits {
        bit_width: 68,
        bits: "00000000000000000".into(),
    };
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
    let mut value = fixture();
    value
        .registers
        .registers
        .push(value.registers.registers[0].clone());
    assert_eq!(
        value.validate(),
        Err(RocgdbHardwareStopResourceErrorV1::InvalidRegister)
    );
}

#[test]
fn unsupported_fields_never_become_captured_or_reclassified() {
    for field in 0..3 {
        let mut value = fixture();
        let replacement = RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::NotCaptured,
        };
        match field {
            0 => value.source = replacement,
            1 => value.isa = replacement,
            _ => value.memory = replacement,
        }
        assert_eq!(
            value.validate(),
            Err(RocgdbHardwareStopResourceErrorV1::InvalidUnavailableBoundary)
        );
    }
}

#[test]
fn structural_validation_does_not_authenticate_copied_session_or_artifact() {
    let mut historical = fixture();
    historical.session_identity = id(99);
    historical.artifact.digest = id(98);
    // There is deliberately no DTO -> process owner import or authorization API.
    historical.validate().unwrap();
}
