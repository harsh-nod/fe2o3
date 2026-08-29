//! Separately versioned, read-only semantic diagnosis protocol.

use std::io::{self, Write};

use serde::{Deserialize, Serialize};

use crate::{
    AllocationIdentityV1, CaptureCompletenessV1, DebugBackendV1, DebugErrorV1, ExecutionKindV1,
    ExecutionScopeSelectorV1, KirSiteV1, PageCursorV1, PageRequestV1, ProtocolCodecErrorV1,
    ProtocolLimitsV1, ProtocolValidationErrorV1, SessionViewV1,
};

pub const DIAGNOSIS_REQUEST_SCHEMA_V2: &str = "fe2o3-debug-diagnosis-request-v2";
pub const DIAGNOSIS_RESPONSE_SCHEMA_V2: &str = "fe2o3-debug-diagnosis-response-v2";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DiagnosisRequestSchemaV2 {
    #[serde(rename = "fe2o3-debug-diagnosis-request-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DiagnosisResponseSchemaV2 {
    #[serde(rename = "fe2o3-debug-diagnosis-response-v2")]
    V2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisOperationV2 {
    Diagnose,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisClassV2 {
    MemoryOutOfBounds,
    WorkgroupBarrierDivergence,
    WorkgroupBarrierMismatch,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisFilterV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<DiagnosisClassV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ExecutionScopeSelectorV1>,
}

impl DiagnosisFilterV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if let Some(scope) = self.scope {
            scope.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisRequestV2 {
    Diagnose {
        schema: DiagnosisRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        #[serde(default)]
        filter: DiagnosisFilterV2,
        page: PageRequestV1,
    },
}

impl DiagnosisRequestV2 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::Diagnose { request_id, .. } => *request_id,
        }
    }

    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::Diagnose {
                expected_revision, ..
            } => *expected_revision,
        }
    }

    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        match self {
            Self::Diagnose {
                request_id,
                filter,
                page,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                filter.validate()?;
                page.validate()
            }
        }
    }
}

/// Origin carried by each individual diagnosis fact.
///
/// `Observed` means observed by the deterministic CPU semantic interpreter. It
/// never means hardware-observed. `Inferred` facts name their exact derivation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisFactV2<T> {
    Declared {
        value: T,
    },
    Observed {
        value: T,
    },
    Inferred {
        value: T,
        basis: DiagnosisInferenceBasisV2,
    },
    Unavailable {
        reason: DiagnosisUnavailableReasonV2,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisInferenceBasisV2 {
    LaunchGeometry,
    LogicalWavePartition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisUnavailableReasonV2 {
    NotApplicable,
    MissingInvocation,
    SiteNotRepresented,
    TranscriptTruncated,
    NotCaptured,
    NotRepresentable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisDispatchV2 {
    pub launch_extent: [u64; 3],
    pub workgroup_size: [u32; 3],
}

impl DiagnosisDispatchV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.launch_extent.contains(&0) || self.workgroup_size.contains(&0) {
            return Err(ProtocolValidationErrorV1::ZeroCount(
                "diagnosis dispatch dimension",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisWorkitemV2 {
    pub global: [u64; 3],
    pub local: [u32; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisLogicalWaveV2 {
    pub wave: u32,
    pub width: u16,
    pub active_mask: u64,
}

impl DiagnosisLogicalWaveV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if !matches!(self.width, 32 | 64)
            || self.active_mask == 0
            || (self.width == 32 && self.active_mask > u64::from(u32::MAX))
        {
            return Err(ProtocolValidationErrorV1::InvalidActiveMask);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisExecutionContextV2 {
    pub dispatch: DiagnosisFactV2<DiagnosisDispatchV2>,
    pub workgroup: DiagnosisFactV2<[u64; 3]>,
    pub workitem: DiagnosisFactV2<DiagnosisWorkitemV2>,
    pub wave: DiagnosisFactV2<DiagnosisLogicalWaveV2>,
    pub lane: DiagnosisFactV2<u16>,
}

impl DiagnosisExecutionContextV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        let DiagnosisFactV2::Declared { value: dispatch } = &self.dispatch else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        dispatch.validate()?;

        let invocation_available = matches!(&self.workgroup, DiagnosisFactV2::Observed { .. })
            && matches!(&self.workitem, DiagnosisFactV2::Observed { .. });
        let invocation_unavailable = matches!(
            &self.workgroup,
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::MissingInvocation
            }
        ) && matches!(
            &self.workitem,
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::MissingInvocation
            }
        );
        if !invocation_available && !invocation_unavailable {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }

        if invocation_available {
            let DiagnosisFactV2::Observed { value: workgroup } = &self.workgroup else {
                return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
            };
            let DiagnosisFactV2::Observed { value: workitem } = &self.workitem else {
                return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
            };
            validate_workitem(*dispatch, *workgroup, *workitem)?;

            let DiagnosisFactV2::Inferred {
                value: wave,
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            } = &self.wave
            else {
                return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
            };
            wave.validate()?;
            let DiagnosisFactV2::Inferred {
                value: lane,
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            } = &self.lane
            else {
                return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
            };
            let (expected_wave, expected_lane, expected_mask) =
                logical_hierarchy(*dispatch, *workgroup, workitem.local, wave.width)?;
            if wave.wave != expected_wave
                || *lane != expected_lane
                || wave.active_mask != expected_mask
            {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis logical hierarchy",
                ));
            }
        } else if !matches!(
            (&self.wave, &self.lane),
            (
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation
                }
            )
        ) {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        Ok(())
    }
}

fn logical_hierarchy(
    dispatch: DiagnosisDispatchV2,
    workgroup: [u64; 3],
    local: [u32; 3],
    width: u16,
) -> Result<(u32, u16, u64), ProtocolValidationErrorV1> {
    if !matches!(width, 32 | 64) {
        return Err(ProtocolValidationErrorV1::InvalidActiveMask);
    }
    let linear = u64::from(local[2])
        .checked_mul(u64::from(dispatch.workgroup_size[1]))
        .and_then(|value| value.checked_mul(u64::from(dispatch.workgroup_size[0])))
        .and_then(|value| {
            u64::from(local[1])
                .checked_mul(u64::from(dispatch.workgroup_size[0]))
                .and_then(|middle| value.checked_add(middle))
        })
        .and_then(|value| value.checked_add(u64::from(local[0])))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow(
            "diagnosis logical hierarchy",
        ))?;
    let lanes = u64::from(width);
    let wave = u32::try_from(linear / lanes)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis logical wave"))?;
    let lane = u16::try_from(linear % lanes)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis logical lane"))?;
    let first =
        u64::from(wave)
            .checked_mul(lanes)
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis logical hierarchy",
            ))?;
    let volume = dispatch
        .workgroup_size
        .iter()
        .try_fold(1_u64, |volume, size| volume.checked_mul(u64::from(*size)))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow(
            "diagnosis workgroup volume",
        ))?;
    let mut active_mask = 0_u64;
    for candidate in 0..lanes {
        let candidate_linear =
            first
                .checked_add(candidate)
                .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                    "diagnosis logical hierarchy",
                ))?;
        if candidate_linear >= volume {
            break;
        }
        let x = candidate_linear % u64::from(dispatch.workgroup_size[0]);
        let yz = candidate_linear / u64::from(dispatch.workgroup_size[0]);
        let y = yz % u64::from(dispatch.workgroup_size[1]);
        let z = yz / u64::from(dispatch.workgroup_size[1]);
        let active = [x, y, z]
            .into_iter()
            .enumerate()
            .try_fold(true, |active, (axis, coordinate)| {
                workgroup[axis]
                    .checked_mul(u64::from(dispatch.workgroup_size[axis]))
                    .and_then(|base| base.checked_add(coordinate))
                    .map(|global| active && global < dispatch.launch_extent[axis])
            })
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis logical hierarchy",
            ))?;
        if active {
            active_mask |= 1_u64 << candidate;
        }
    }
    if active_mask & (1_u64 << lane) == 0 {
        return Err(ProtocolValidationErrorV1::InvalidActiveMask);
    }
    Ok((wave, lane, active_mask))
}

fn active_workgroup_participants(
    dispatch: DiagnosisDispatchV2,
    workgroup: [u64; 3],
) -> Result<u32, ProtocolValidationErrorV1> {
    let mut participants = 1_u64;
    for (axis, coordinate) in workgroup.into_iter().enumerate() {
        let start = coordinate
            .checked_mul(u64::from(dispatch.workgroup_size[axis]))
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis workgroup extent",
            ))?;
        let remaining = dispatch.launch_extent[axis].checked_sub(start).ok_or(
            ProtocolValidationErrorV1::CountOutOfRange("diagnosis workgroup coordinate"),
        )?;
        let active = remaining.min(u64::from(dispatch.workgroup_size[axis]));
        if active == 0 {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis workgroup coordinate",
            ));
        }
        participants =
            participants
                .checked_mul(active)
                .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                    "diagnosis workgroup participants",
                ))?;
    }
    u32::try_from(participants)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis workgroup participants"))
}

fn validate_workitem(
    dispatch: DiagnosisDispatchV2,
    workgroup: [u64; 3],
    workitem: DiagnosisWorkitemV2,
) -> Result<(), ProtocolValidationErrorV1> {
    for (axis, coordinate) in workgroup.into_iter().enumerate() {
        if workitem.local[axis] >= dispatch.workgroup_size[axis]
            || workitem.global[axis] >= dispatch.launch_extent[axis]
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis workitem coordinate",
            ));
        }
        let expected = coordinate
            .checked_mul(u64::from(dispatch.workgroup_size[axis]))
            .and_then(|base| base.checked_add(u64::from(workitem.local[axis])))
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis workitem coordinate",
            ))?;
        if expected != workitem.global[axis] {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis workitem hierarchy",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisMemoryRegionV2 {
    pub allocation: AllocationIdentityV1,
    pub requested_offset: u64,
    pub requested_bytes: u64,
    pub allocation_bytes: u64,
}

impl DiagnosisMemoryRegionV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.allocation.ordinal == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if self.requested_bytes == 0 {
            return Err(ProtocolValidationErrorV1::ZeroCount(
                "diagnosis requested bytes",
            ));
        }
        let end = self
            .requested_offset
            .checked_add(self.requested_bytes)
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis memory region",
            ))?;
        if end <= self.allocation_bytes {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis out-of-bounds region",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisBarrierParticipantV2 {
    pub local_workitem: DiagnosisFactV2<[u32; 3]>,
    pub global_workitem: DiagnosisFactV2<[u64; 3]>,
    pub wave: DiagnosisFactV2<u32>,
    pub lane: DiagnosisFactV2<u16>,
}

impl DiagnosisBarrierParticipantV2 {
    fn validate(
        &self,
        dispatch: DiagnosisDispatchV2,
        workgroup: [u64; 3],
        wave_width: u16,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let DiagnosisFactV2::Observed { value: local } = &self.local_workitem else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred {
            value: global,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = &self.global_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        validate_workitem(
            dispatch,
            workgroup,
            DiagnosisWorkitemV2 {
                global: *global,
                local: *local,
            },
        )?;
        let DiagnosisFactV2::Inferred {
            value: wave,
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        } = &self.wave
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred {
            value: lane,
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
        } = &self.lane
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if *lane >= wave_width {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis barrier lane",
            ));
        }
        let linear = u64::from(local[0])
            .checked_add(
                u64::from(dispatch.workgroup_size[0])
                    .checked_mul(
                        u64::from(local[1])
                            .checked_add(
                                u64::from(dispatch.workgroup_size[1])
                                    .checked_mul(u64::from(local[2]))
                                    .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                                        "diagnosis barrier participant",
                                    ))?,
                            )
                            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                                "diagnosis barrier participant",
                            ))?,
                    )
                    .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                        "diagnosis barrier participant",
                    ))?,
            )
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis barrier participant",
            ))?;
        let expected_wave = u32::try_from(linear / u64::from(wave_width))
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis barrier wave"))?;
        if *wave != expected_wave || u64::from(*lane) != linear % u64::from(wave_width) {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis logical barrier participant",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisBarrierMismatchV2 {
    Site,
    Semantics,
    SiteAndSemantics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisBarrierV2 {
    Divergence {
        phase: DiagnosisFactV2<u64>,
        observed_arrivals: DiagnosisFactV2<u32>,
        expected_participants: DiagnosisFactV2<u32>,
        waiting: DiagnosisBarrierParticipantV2,
        exited: DiagnosisBarrierParticipantV2,
    },
    Mismatch {
        phase: DiagnosisFactV2<u64>,
        mismatch: DiagnosisFactV2<DiagnosisBarrierMismatchV2>,
        expected_site: DiagnosisFactV2<KirSiteV1>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisViewV2 {
    pub sequence: u64,
    pub class: DiagnosisClassV2,
    pub context: DiagnosisExecutionContextV2,
    pub site: DiagnosisFactV2<KirSiteV1>,
    pub memory_region: DiagnosisFactV2<DiagnosisMemoryRegionV2>,
    pub barrier: DiagnosisFactV2<DiagnosisBarrierV2>,
}

impl DiagnosisViewV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.sequence == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        self.context.validate()?;
        if !matches!(
            &self.site,
            DiagnosisFactV2::Observed { .. }
                | DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::SiteNotRepresented
                }
        ) {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        match (&self.class, &self.memory_region, &self.barrier) {
            (
                DiagnosisClassV2::MemoryOutOfBounds,
                DiagnosisFactV2::Observed { value: region },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
            ) => region.validate(),
            (
                DiagnosisClassV2::MemoryOutOfBounds,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
            ) => Ok(()),
            (
                DiagnosisClassV2::WorkgroupBarrierDivergence,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Observed {
                    value:
                        DiagnosisBarrierV2::Divergence {
                            phase,
                            observed_arrivals,
                            expected_participants,
                            waiting,
                            exited,
                        },
                },
            ) => self.validate_divergence(
                phase,
                observed_arrivals,
                expected_participants,
                waiting,
                exited,
            ),
            (
                DiagnosisClassV2::WorkgroupBarrierDivergence,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                },
            ) => Ok(()),
            (
                DiagnosisClassV2::WorkgroupBarrierMismatch,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Observed {
                    value:
                        DiagnosisBarrierV2::Mismatch {
                            phase,
                            mismatch,
                            expected_site,
                        },
                },
            ) => {
                if !matches!(phase, DiagnosisFactV2::Observed { .. })
                    || !matches!(mismatch, DiagnosisFactV2::Observed { .. })
                    || !matches!(
                        expected_site,
                        DiagnosisFactV2::Observed { .. }
                            | DiagnosisFactV2::Unavailable {
                                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented
                            }
                    )
                {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                }
                Ok(())
            }
            _ => Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
    }

    fn validate_divergence(
        &self,
        phase: &DiagnosisFactV2<u64>,
        observed_arrivals: &DiagnosisFactV2<u32>,
        expected_participants: &DiagnosisFactV2<u32>,
        waiting: &DiagnosisBarrierParticipantV2,
        exited: &DiagnosisBarrierParticipantV2,
    ) -> Result<(), ProtocolValidationErrorV1> {
        if !matches!(phase, DiagnosisFactV2::Observed { .. }) {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        let arrivals = match observed_arrivals {
            DiagnosisFactV2::Observed { value } if *value > 0 => Some(*value),
            DiagnosisFactV2::Unavailable {
                reason:
                    DiagnosisUnavailableReasonV2::TranscriptTruncated
                    | DiagnosisUnavailableReasonV2::NotCaptured,
            } => None,
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        };
        let DiagnosisFactV2::Inferred {
            value: expected,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = expected_participants
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if *expected == 0 || arrivals.is_some_and(|arrivals| arrivals >= *expected) {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis barrier participation",
            ));
        }
        let DiagnosisFactV2::Declared { value: dispatch } = &self.context.dispatch else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Observed { value: workgroup } = &self.context.workgroup else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred { value: wave, .. } = &self.context.wave else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if *expected != active_workgroup_participants(*dispatch, *workgroup)? {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier participant count",
            ));
        }
        waiting.validate(*dispatch, *workgroup, wave.width)?;
        exited.validate(*dispatch, *workgroup, wave.width)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisResponseV2 {
    Ok {
        schema: DiagnosisResponseSchemaV2,
        request_id: u64,
        operation: DiagnosisOperationV2,
        session: SessionViewV1,
        completeness: CaptureCompletenessV1,
        diagnoses: Vec<DiagnosisViewV2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<PageCursorV1>,
    },
    Error {
        schema: DiagnosisResponseSchemaV2,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
        operation: DiagnosisOperationV2,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}

impl DiagnosisResponseV2 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        match self {
            Self::Ok {
                request_id,
                session,
                completeness,
                diagnoses,
                next_cursor,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                validate_simulator_session(*session)?;
                if diagnoses.len() > limits.max_response_items {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "diagnosis response",
                    ));
                }
                for diagnosis in diagnoses {
                    diagnosis.validate()?;
                }
                if next_cursor.is_some_and(|cursor| cursor.position == 0) {
                    return Err(ProtocolValidationErrorV1::ZeroIdentity);
                }
                let _capture_completeness = completeness;
                Ok(())
            }
            Self::Error {
                request_id,
                session,
                error,
                ..
            } => {
                if request_id == &Some(0) {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                validate_simulator_session(*session)?;
                error.validate()
            }
        }
    }
}

fn validate_simulator_session(session: SessionViewV1) -> Result<(), ProtocolValidationErrorV1> {
    session.validate()?;
    if session.backend == DebugBackendV1::CpuKirSimulator
        && session.execution_kind == ExecutionKindV1::CpuKirSimulation
        && session.simulated
        && !session.hardware_observed
        && !session.performance_prediction
    {
        Ok(())
    } else {
        Err(ProtocolValidationErrorV1::InvalidTruthClassification)
    }
}

pub fn decode_diagnosis_request_line_v2(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DiagnosisRequestV2, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_request_line_bytes)?;
    let request: DiagnosisRequestV2 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    request
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(request)
}

pub fn decode_diagnosis_response_line_v2(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<DiagnosisResponseV2, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_response_line_bytes)?;
    let response: DiagnosisResponseV2 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(response)
}

pub fn encode_diagnosis_response_line_v2(
    response: &DiagnosisResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload_limit = limits
        .max_response_line_bytes
        .checked_sub(1)
        .ok_or(ProtocolCodecErrorV1::ResponseTooLarge)?;
    let mut output = Vec::new();
    let mut writer = BoundedWriter {
        output: &mut output,
        max: payload_limit,
        limit_exceeded: false,
        allocation_failed: false,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(if writer.limit_exceeded {
            ProtocolCodecErrorV1::ResponseTooLarge
        } else if writer.allocation_failed {
            ProtocolCodecErrorV1::AllocationFailure
        } else {
            ProtocolCodecErrorV1::JsonEncode
        });
    }
    output
        .try_reserve_exact(1)
        .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
    output.push(b'\n');
    Ok(output)
}

fn validate_line(line: &[u8], max: usize) -> Result<&[u8], ProtocolCodecErrorV1> {
    if line.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if line.len() > max {
        return Err(ProtocolCodecErrorV1::LineTooLarge);
    }
    let payload = line
        .strip_suffix(b"\n")
        .ok_or(ProtocolCodecErrorV1::MissingLineTerminator)?;
    if payload.is_empty() {
        return Err(ProtocolCodecErrorV1::EmptyLine);
    }
    if payload.iter().any(|byte| matches!(*byte, b'\n' | b'\r')) {
        return Err(ProtocolCodecErrorV1::EmbeddedLineBreak);
    }
    Ok(payload)
}

struct BoundedWriter<'a> {
    output: &'a mut Vec<u8>,
    max: usize,
    limit_exceeded: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let required = match self.output.len().checked_add(bytes.len()) {
            Some(required) if required <= self.max => required,
            _ => {
                self.limit_exceeded = true;
                return Err(io::Error::other("bounded diagnosis response exceeded"));
            }
        };
        if required > self.output.capacity()
            && self
                .output
                .try_reserve_exact(required - self.output.capacity())
                .is_err()
        {
            self.allocation_failed = true;
            return Err(io::Error::other(
                "bounded diagnosis response allocation failed",
            ));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
