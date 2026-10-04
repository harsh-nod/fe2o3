//! Additive structured inspection for one authenticated native ROCgdb stop.

use serde::{Deserialize, Serialize};

use crate::{
    LiveGpuAvailabilityV3, LiveGpuEvidenceKindV3, OpaqueIdentityV1, RocgdbMiNativeProbeV4,
    RocgdbMiNativeProtocolErrorV4, RocgdbMiNativeStoppedStateV4, RocgdbMiNativeUnavailableReasonV4,
    RocgdbMiRegisterSnapshotV3, RocgdbMiStoppedScopeV3, RocgdbMiValueSnapshotV3,
};

pub const ROCGDB_MI_NATIVE_CLI_RESPONSE_SCHEMA_V5: &str = "fe2o3-rocgdb-kfd-native-response-v5";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RocgdbMiNativeCliResponseSchemaV5 {
    #[serde(rename = "fe2o3-rocgdb-kfd-native-response-v5")]
    V5,
}

/// Registry discovery is not an observation and does not imply that a GPU
/// thread can be stopped or that a query will succeed for that thread.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeInspectionProbeV5 {
    pub register_names: bool,
    pub register_values: bool,
    pub simple_locals: bool,
    pub disassembly: bool,
    pub memory_bytes: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiNativeInspectionUnavailableReasonV5 {
    MachineCommandUnavailable,
    BackendRejected,
    NotCaptured,
    RequiresAuthenticatedSourceMap,
    RequiresArtifactRelativeInstructionBinding,
    RequiresAllocationRelativeAuthority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiNativeCapturedV5<T> {
    Captured {
        evidence_identity: OpaqueIdentityV1,
        value: T,
    },
    Unavailable {
        reason: RocgdbMiNativeInspectionUnavailableReasonV5,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiNativeUnavailableFieldV5 {
    Unavailable {
        reason: RocgdbMiNativeInspectionUnavailableReasonV5,
    },
}

/// Values collected after the V4 hierarchy, while its stop pin remained
/// current. Native selectors and MI address fields do not cross this boundary;
/// opaque register bits are never interpreted as pointer authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeInspectionV5 {
    pub association_identity: OpaqueIdentityV1,
    pub scope: RocgdbMiStoppedScopeV3,
    pub registers: RocgdbMiNativeCapturedV5<RocgdbMiRegisterSnapshotV3>,
    pub locals: RocgdbMiNativeCapturedV5<RocgdbMiValueSnapshotV3>,
    pub source: RocgdbMiNativeUnavailableFieldV5,
    pub isa: RocgdbMiNativeUnavailableFieldV5,
    pub memory: RocgdbMiNativeUnavailableFieldV5,
}

impl RocgdbMiNativeInspectionV5 {
    pub fn validate(
        &self,
        stopped: &RocgdbMiNativeStoppedStateV4,
    ) -> Result<(), RocgdbMiNativeProtocolErrorV5> {
        self.scope
            .validate()
            .map_err(|_| RocgdbMiNativeProtocolErrorV5::InvalidScope)?;
        if self.scope.lane.is_some() {
            return Err(RocgdbMiNativeProtocolErrorV5::InvalidScope);
        }
        if self.association_identity != stopped.association_identity
            || self.scope.wave.identity != stopped.wave_identity
        {
            return Err(RocgdbMiNativeProtocolErrorV5::IdentitySubstitution);
        }
        if !matches!(stopped.source, LiveGpuAvailabilityV3::Unavailable { .. })
            || !matches!(stopped.registers, LiveGpuAvailabilityV3::Unavailable { .. })
            || !matches!(stopped.memory, LiveGpuAvailabilityV3::Unavailable { .. })
        {
            return Err(RocgdbMiNativeProtocolErrorV5::InvalidUnavailableBoundary);
        }
        validate_snapshot(&self.registers, |snapshot, evidence| {
            snapshot.scope == self.scope
                && snapshot.validate().is_ok()
                && snapshot
                    .registers
                    .iter()
                    .all(|register| availability_is_bound_to(&register.value, evidence))
        })?;
        validate_snapshot(&self.locals, |snapshot, evidence| {
            snapshot.scope == self.scope
                && snapshot.validate().is_ok()
                && snapshot
                    .values
                    .iter()
                    .all(|value| availability_is_bound_to(&value.value, evidence))
        })?;
        if !matches!(
            self.source,
            RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAuthenticatedSourceMap
            }
        ) || !matches!(
            self.isa,
            RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresArtifactRelativeInstructionBinding
            }
        ) || !matches!(
            self.memory,
            RocgdbMiNativeUnavailableFieldV5::Unavailable {
                reason: RocgdbMiNativeInspectionUnavailableReasonV5::RequiresAllocationRelativeAuthority
            }
        )
        {
            return Err(RocgdbMiNativeProtocolErrorV5::InvalidUnavailableBoundary);
        }
        Ok(())
    }
}

fn validate_snapshot<T>(
    value: &RocgdbMiNativeCapturedV5<T>,
    validate: impl FnOnce(&T, OpaqueIdentityV1) -> bool,
) -> Result<(), RocgdbMiNativeProtocolErrorV5> {
    match value {
        RocgdbMiNativeCapturedV5::Captured {
            evidence_identity,
            value,
        } if validate(value, *evidence_identity) => Ok(()),
        RocgdbMiNativeCapturedV5::Captured { .. } => {
            Err(RocgdbMiNativeProtocolErrorV5::InvalidSnapshot)
        }
        RocgdbMiNativeCapturedV5::Unavailable {
            reason:
                RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable
                | RocgdbMiNativeInspectionUnavailableReasonV5::BackendRejected
                | RocgdbMiNativeInspectionUnavailableReasonV5::NotCaptured,
        } => Ok(()),
        RocgdbMiNativeCapturedV5::Unavailable { .. } => {
            Err(RocgdbMiNativeProtocolErrorV5::InvalidUnavailableBoundary)
        }
    }
}

fn availability_is_bound_to<T>(
    availability: &LiveGpuAvailabilityV3<T>,
    evidence_identity: OpaqueIdentityV1,
) -> bool {
    match availability {
        LiveGpuAvailabilityV3::Available { truth, .. }
        | LiveGpuAvailabilityV3::Redacted { truth, .. } => {
            truth.evidence.len() == 1
                && truth.evidence[0].kind == LiveGpuEvidenceKindV3::RuntimeObservation
                && truth.evidence[0].identity == evidence_identity
        }
        LiveGpuAvailabilityV3::Unavailable { .. } => true,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RocgdbMiNativeCliResultV5 {
    Available {
        probe: RocgdbMiNativeProbeV4,
        inspection_probe: RocgdbMiNativeInspectionProbeV5,
        stopped_state: Box<RocgdbMiNativeStoppedStateV4>,
        inspection: Box<RocgdbMiNativeInspectionV5>,
    },
    Unavailable {
        probe: RocgdbMiNativeProbeV4,
        inspection_probe: RocgdbMiNativeInspectionProbeV5,
        reason: RocgdbMiNativeUnavailableReasonV4,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RocgdbMiNativeCliResponseV5 {
    pub schema: RocgdbMiNativeCliResponseSchemaV5,
    pub result: RocgdbMiNativeCliResultV5,
}

impl RocgdbMiNativeCliResponseV5 {
    pub fn validate(&self) -> Result<(), RocgdbMiNativeProtocolErrorV5> {
        if let RocgdbMiNativeCliResultV5::Available {
            probe,
            inspection_probe,
            stopped_state,
            inspection,
        } = &self.result
        {
            if !probe.structured_mi_commands
                || !probe.direct_kfd_device_admitted
                || !probe.cooperative_v2_declaration
                || !probe.cooperative_v2_publication
            {
                return Err(RocgdbMiNativeProtocolErrorV5::InvalidProbe);
            }
            stopped_state
                .validate()
                .map_err(RocgdbMiNativeProtocolErrorV5::V4)?;
            inspection.validate(stopped_state)?;
            validate_probe_relation(inspection_probe, inspection)?;
        }
        Ok(())
    }
}

fn validate_probe_relation(
    probe: &RocgdbMiNativeInspectionProbeV5,
    inspection: &RocgdbMiNativeInspectionV5,
) -> Result<(), RocgdbMiNativeProtocolErrorV5> {
    validate_capture_probe(
        &inspection.registers,
        probe.register_names && probe.register_values,
    )?;
    validate_capture_probe(&inspection.locals, probe.simple_locals)
}

fn validate_capture_probe<T>(
    capture: &RocgdbMiNativeCapturedV5<T>,
    command_available: bool,
) -> Result<(), RocgdbMiNativeProtocolErrorV5> {
    let valid = match capture {
        RocgdbMiNativeCapturedV5::Captured { .. } => command_available,
        RocgdbMiNativeCapturedV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::MachineCommandUnavailable,
        } => !command_available,
        RocgdbMiNativeCapturedV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::BackendRejected,
        } => command_available,
        RocgdbMiNativeCapturedV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::NotCaptured,
        } => true,
        RocgdbMiNativeCapturedV5::Unavailable { .. } => false,
    };
    valid
        .then_some(())
        .ok_or(RocgdbMiNativeProtocolErrorV5::InvalidProbe)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbMiNativeProtocolErrorV5 {
    V4(RocgdbMiNativeProtocolErrorV4),
    InvalidProbe,
    InvalidScope,
    InvalidSnapshot,
    IdentitySubstitution,
    InvalidUnavailableBoundary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LiveGpuAvailabilityV3, LiveGpuContentIdentityV3, LiveGpuEvidenceRefV3,
        LiveGpuTruthOriginV3, LiveGpuTruthV3, LiveGpuUnavailableReasonV3, RocgdbMiLaneIdentityV3,
        RocgdbMiNativeCorrelationOriginV4, RocgdbMiNativeLaneV4, RocgdbMiThreadIdentityV3,
        RocgdbMiWaveIdentityV3, RocgdbMiWorkgroupCoordinateV4, RocgdbMiWorkitemCoordinateV4,
    };

    fn identity(seed: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([seed; 32]).unwrap()
    }

    fn unavailable<T>() -> LiveGpuAvailabilityV3<T> {
        LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotCaptured,
            truth: LiveGpuTruthV3 {
                origin: LiveGpuTruthOriginV3::Unavailable,
                evidence: Vec::<LiveGpuEvidenceRefV3>::new(),
            },
        }
    }

    fn stopped() -> RocgdbMiNativeStoppedStateV4 {
        RocgdbMiNativeStoppedStateV4 {
            association_identity: identity(1),
            queue_occurrence_identity: identity(2),
            process_instance_identity: identity(3),
            dispatch_identity: identity(4),
            artifact: LiveGpuContentIdentityV3 {
                digest: identity(5),
                canonical_bytes: 128,
            },
            grid: [32, 1, 1],
            workgroup: [32, 1, 1],
            workgroup_coordinate: RocgdbMiWorkgroupCoordinateV4 { x: 0, y: 0, z: 0 },
            wave_identity: identity(6),
            wave_in_workgroup: 0,
            lanes: (0_u16..32)
                .map(|lane| RocgdbMiNativeLaneV4 {
                    lane_identity: identity(u8::try_from(lane + 20).unwrap()),
                    lane_index: lane,
                    workitem: RocgdbMiWorkitemCoordinateV4 {
                        x: u32::from(lane),
                        y: 0,
                        z: 0,
                    },
                    active: unavailable(),
                })
                .collect(),
            relative_pc: unavailable(),
            source: unavailable(),
            registers: unavailable(),
            memory: unavailable(),
            origins: vec![
                RocgdbMiNativeCorrelationOriginV4::TargetKfdPublicationObservation,
                RocgdbMiNativeCorrelationOriginV4::RocgdbStructuredObservation,
                RocgdbMiNativeCorrelationOriginV4::ExplicitCodeObjectAdmission,
                RocgdbMiNativeCorrelationOriginV4::Correlated,
            ],
        }
    }

    fn inspection(stopped: &RocgdbMiNativeStoppedStateV4) -> RocgdbMiNativeInspectionV5 {
        let thread = RocgdbMiThreadIdentityV3 {
            identity: identity(7),
        };
        let scope = RocgdbMiStoppedScopeV3 {
            stop_identity: identity(8),
            thread,
            wave: RocgdbMiWaveIdentityV3 {
                identity: stopped.wave_identity,
                thread,
            },
            lane: None,
        };
        RocgdbMiNativeInspectionV5 {
            association_identity: stopped.association_identity,
            scope,
            registers: RocgdbMiNativeCapturedV5::Captured {
                evidence_identity: identity(9),
                value: RocgdbMiRegisterSnapshotV3 {
                    scope,
                    registers: Vec::new(),
                },
            },
            locals: RocgdbMiNativeCapturedV5::Captured {
                evidence_identity: identity(10),
                value: RocgdbMiValueSnapshotV3 {
                    scope,
                    values: Vec::new(),
                },
            },
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
    fn exact_scope_and_unavailable_boundaries_validate() {
        let stopped = stopped();
        let inspection = inspection(&stopped);
        inspection.validate(&stopped).unwrap();

        let mut wrong_wave = inspection.clone();
        wrong_wave.scope.wave.identity = identity(99);
        assert_eq!(
            wrong_wave.validate(&stopped),
            Err(RocgdbMiNativeProtocolErrorV5::IdentitySubstitution)
        );

        let mut false_lane = inspection.clone();
        false_lane.scope.lane = Some(RocgdbMiLaneIdentityV3 {
            identity: identity(97),
            wave: false_lane.scope.wave,
            lane: 0,
        });
        assert_eq!(
            false_lane.validate(&stopped),
            Err(RocgdbMiNativeProtocolErrorV5::InvalidScope)
        );

        let mut wrong_snapshot = inspection.clone();
        let RocgdbMiNativeCapturedV5::Captured { value, .. } = &mut wrong_snapshot.registers else {
            unreachable!()
        };
        value.scope.stop_identity = identity(98);
        assert_eq!(
            wrong_snapshot.validate(&stopped),
            Err(RocgdbMiNativeProtocolErrorV5::InvalidSnapshot)
        );

        let mut false_memory = inspection;
        false_memory.memory = RocgdbMiNativeUnavailableFieldV5::Unavailable {
            reason: RocgdbMiNativeInspectionUnavailableReasonV5::NotCaptured,
        };
        assert_eq!(
            false_memory.validate(&stopped),
            Err(RocgdbMiNativeProtocolErrorV5::InvalidUnavailableBoundary)
        );
    }

    #[test]
    fn unknown_wire_fields_and_available_probe_substitution_are_rejected() {
        let stopped = stopped();
        let response = RocgdbMiNativeCliResponseV5 {
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
                inspection: Box::new(inspection(&stopped)),
                stopped_state: Box::new(stopped),
            },
        };
        response.validate().unwrap();
        let mut value = serde_json::to_value(&response).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("native_thread_id".to_owned(), serde_json::json!("9"));
        assert!(serde_json::from_value::<RocgdbMiNativeCliResponseV5>(value).is_err());

        let mut bad_probe = response;
        let RocgdbMiNativeCliResultV5::Available { probe, .. } = &mut bad_probe.result else {
            unreachable!()
        };
        probe.cooperative_v2_publication = false;
        assert_eq!(
            bad_probe.validate(),
            Err(RocgdbMiNativeProtocolErrorV5::InvalidProbe)
        );

        let mut missing_command = serde_json::from_str::<RocgdbMiNativeCliResponseV5>(
            &serde_json::to_string(&bad_probe).unwrap(),
        )
        .unwrap();
        let RocgdbMiNativeCliResultV5::Available {
            probe,
            inspection_probe,
            ..
        } = &mut missing_command.result
        else {
            unreachable!()
        };
        probe.cooperative_v2_publication = true;
        inspection_probe.register_names = false;
        assert_eq!(
            missing_command.validate(),
            Err(RocgdbMiNativeProtocolErrorV5::InvalidProbe)
        );
    }
}
