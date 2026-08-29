//! Separately versioned, read-only semantic diagnosis protocol.

use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AddressSpaceV1, AllocationIdentityV1, CaptureCompletenessV1, DebugBackendV1, DebugErrorV1,
    ExecutionKindV1, ExecutionScopeSelectorV1, KirSiteV1, OpaqueIdentityV1, PageCursorV1,
    PageRequestV1, ProtocolCodecErrorV1, ProtocolLimitsV1, ProtocolValidationErrorV1,
    SessionViewV1, SourceLocationV1, SourceMapProvenanceV1,
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
    InputNotProvided,
    RequiresSourceMapV2,
    SourceSiteAbsent,
    SourceSiteAmbiguous,
    NoArtifactAuthority,
    NoProofAuthority,
    AmbiguousAbiBinding,
    BarrierNotReleased,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisContentReferenceV2 {
    pub sha256: OpaqueIdentityV1,
    pub canonical_bytes: u64,
}

impl DiagnosisContentReferenceV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.canonical_bytes == 0 {
            Err(ProtocolValidationErrorV1::ZeroCount(
                "diagnosis content bytes",
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisVersionedContentReferenceV2 {
    pub version: u16,
    pub content: DiagnosisContentReferenceV2,
}

impl DiagnosisVersionedContentReferenceV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.version == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        self.content.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisBundleReferenceV2 {
    pub identity: OpaqueIdentityV1,
    pub subject_identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisSourceLineageV2 {
    pub identity_inventory_receipt: DiagnosisContentReferenceV2,
    pub preflight_plan_receipt: DiagnosisContentReferenceV2,
}

impl DiagnosisSourceLineageV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.identity_inventory_receipt.validate()?;
        self.preflight_plan_receipt.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisSourceMapReferenceV2 {
    pub identity: OpaqueIdentityV1,
    pub bundle_subject_identity: OpaqueIdentityV1,
    pub provenance: SourceMapProvenanceV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisInputEvidenceV2 {
    pub configuration_identity: OpaqueIdentityV1,
    pub dispatch_identity: OpaqueIdentityV1,
    pub dispatch_request: DiagnosisFactV2<DiagnosisContentReferenceV2>,
    pub canonical_kir_v7: DiagnosisFactV2<DiagnosisContentReferenceV2>,
    pub simulation_bundle: DiagnosisFactV2<DiagnosisBundleReferenceV2>,
    pub production_kir: DiagnosisFactV2<DiagnosisVersionedContentReferenceV2>,
    pub kernel_abi_identity: DiagnosisFactV2<OpaqueIdentityV1>,
    pub source_lineage: DiagnosisFactV2<DiagnosisSourceLineageV2>,
    pub source_map_v2: DiagnosisFactV2<DiagnosisSourceMapReferenceV2>,
    pub finalized_artifact: DiagnosisFactV2<DiagnosisContentReferenceV2>,
    pub property_proof: DiagnosisFactV2<OpaqueIdentityV1>,
}

impl DiagnosisInputEvidenceV2 {
    fn validate(&self, session: SessionViewV1) -> Result<(), ProtocolValidationErrorV1> {
        if self.configuration_identity != session.configuration_identity {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis configuration",
            ));
        }
        let DiagnosisFactV2::Declared { value: request } = &self.dispatch_request else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        request.validate()?;
        let DiagnosisFactV2::Declared { value: kir } = &self.canonical_kir_v7 else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        kir.validate()?;
        if diagnosis_dispatch_input_identity_v2(*request, *kir) != Ok(self.dispatch_identity) {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis dispatch input",
            ));
        }

        let bundle_subject = match &self.simulation_bundle {
            DiagnosisFactV2::Declared { value } => Some(value.subject_identity),
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::InputNotProvided,
            } => None,
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        };
        if !bundle_property_available(&self.production_kir, bundle_subject.is_some())
            || !bundle_property_available(&self.kernel_abi_identity, bundle_subject.is_some())
            || !bundle_property_available(&self.source_lineage, bundle_subject.is_some())
        {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        if let DiagnosisFactV2::Declared { value } = &self.production_kir {
            value.validate()?;
        }
        if let DiagnosisFactV2::Declared { value } = &self.source_lineage {
            value.validate()?;
        }
        match &self.source_map_v2 {
            DiagnosisFactV2::Declared { value } => {
                if bundle_subject.is_some_and(|subject| subject != value.bundle_subject_identity) {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis source map bundle subject",
                    ));
                }
            }
            DiagnosisFactV2::Unavailable {
                reason:
                    DiagnosisUnavailableReasonV2::InputNotProvided
                    | DiagnosisUnavailableReasonV2::RequiresSourceMapV2,
            } => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        if !matches!(
            &self.finalized_artifact,
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
            }
        ) || !matches!(
            &self.property_proof,
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NoProofAuthority
            }
        ) {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        Ok(())
    }
}

fn bundle_property_available<T>(fact: &DiagnosisFactV2<T>, bundle_available: bool) -> bool {
    matches!(fact, DiagnosisFactV2::Declared { .. }) == bundle_available
        && (bundle_available
            || matches!(
                fact,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::InputNotProvided
                }
            ))
}

/// Derives the separately versioned identity of one exact CPU-simulation dispatch input.
/// It identifies admitted KIR plus request content, never a native or hardware dispatch.
pub fn diagnosis_dispatch_input_identity_v2(
    request: DiagnosisContentReferenceV2,
    kir: DiagnosisContentReferenceV2,
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-sim-dispatch-input-v2\0");
    digest.update(request.sha256.as_bytes());
    digest.update(request.canonical_bytes.to_le_bytes());
    digest.update(kir.sha256.as_bytes());
    digest.update(kir.canonical_bytes.to_le_bytes());
    OpaqueIdentityV1::new(digest.finalize().into())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisSourceOperationV2 {
    pub bundle_subject_identity: OpaqueIdentityV1,
    pub kir_site: KirSiteV1,
    pub location: SourceLocationV1,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisMemoryRegionV2 {
    pub allocation: AllocationIdentityV1,
    pub requested_offset: u64,
    pub requested_bytes: u64,
    pub allocation_bytes: u64,
    pub allocation_contract: DiagnosisFactV2<DiagnosisAllocationContractV2>,
}

impl DiagnosisMemoryRegionV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
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
        match &self.allocation_contract {
            DiagnosisFactV2::Declared { value } | DiagnosisFactV2::Observed { value } => {
                value.validate()?;
                if value.allocation_bytes != self.allocation_bytes {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis allocation byte contract",
                    ));
                }
            }
            DiagnosisFactV2::Unavailable {
                reason:
                    DiagnosisUnavailableReasonV2::NotCaptured
                    | DiagnosisUnavailableReasonV2::TranscriptTruncated
                    | DiagnosisUnavailableReasonV2::NotRepresentable,
            } => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisAccessModeV2 {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisScalarTypeV2 {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Index32,
    Index64,
    F16,
    Bf16,
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisAbiArgumentKindV2 {
    Pointer,
    Slice,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisAbiArgumentV2 {
    pub ordinal: u32,
    pub kind: DiagnosisAbiArgumentKindV2,
    pub element: DiagnosisScalarTypeV2,
    pub address_space: AddressSpaceV1,
    pub access: DiagnosisAccessModeV2,
    pub view_offset: u64,
    pub view_bytes: u64,
}

impl DiagnosisAbiArgumentV2 {
    fn validate(self, allocation_bytes: u64) -> Result<(), ProtocolValidationErrorV1> {
        let end = self.view_offset.checked_add(self.view_bytes).ok_or(
            ProtocolValidationErrorV1::RangeOverflow("diagnosis ABI view"),
        )?;
        if end > allocation_bytes {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis ABI view",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisAllocationContractV2 {
    pub address_space: AddressSpaceV1,
    pub access: DiagnosisAccessModeV2,
    pub alignment: u32,
    pub allocation_bytes: u64,
    pub abi_argument: DiagnosisFactV2<DiagnosisAbiArgumentV2>,
}

impl DiagnosisAllocationContractV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.alignment == 0 || !self.alignment.is_power_of_two() {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis allocation alignment",
            ));
        }
        match &self.abi_argument {
            DiagnosisFactV2::Declared { value } => value.validate(self.allocation_bytes),
            DiagnosisFactV2::Unavailable {
                reason:
                    DiagnosisUnavailableReasonV2::NotApplicable
                    | DiagnosisUnavailableReasonV2::AmbiguousAbiBinding,
            } => Ok(()),
            _ => Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisSynchronizationScopeV2 {
    Invocation,
    Subgroup,
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisMemoryOrderingV2 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisBarrierSemanticsV2 {
    pub memory_scope: DiagnosisSynchronizationScopeV2,
    pub ordering: DiagnosisMemoryOrderingV2,
    pub address_spaces: Vec<AddressSpaceV1>,
}

impl DiagnosisBarrierSemanticsV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.address_spaces.is_empty() || self.address_spaces.len() > 5 {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis barrier address spaces",
            ));
        }
        if self
            .address_spaces
            .windows(2)
            .any(|pair| address_space_rank(pair[0]) >= address_space_rank(pair[1]))
        {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis barrier address spaces",
            ));
        }
        Ok(())
    }
}

const fn address_space_rank(space: AddressSpaceV1) -> u8 {
    match space {
        AddressSpaceV1::Private => 0,
        AddressSpaceV1::Workgroup => 1,
        AddressSpaceV1::Global => 2,
        AddressSpaceV1::Constant => 3,
        AddressSpaceV1::Generic => 4,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisLdsEpochV2 {
    pub current: DiagnosisFactV2<u64>,
    pub after_release: DiagnosisFactV2<u64>,
}

impl DiagnosisLdsEpochV2 {
    fn validate(&self, phase: u64) -> Result<(), ProtocolValidationErrorV1> {
        if !matches!(&self.current, DiagnosisFactV2::Observed { value } if *value == phase)
            || !matches!(
                &self.after_release,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::BarrierNotReleased
                }
            )
        {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisBarrierV2 {
    Divergence {
        phase: DiagnosisFactV2<u64>,
        semantics: DiagnosisFactV2<DiagnosisBarrierSemanticsV2>,
        lds_epoch: DiagnosisLdsEpochV2,
        observed_arrivals: DiagnosisFactV2<u32>,
        expected_participants: DiagnosisFactV2<u32>,
        waiting: DiagnosisBarrierParticipantV2,
        exited: DiagnosisBarrierParticipantV2,
    },
    Mismatch {
        phase: DiagnosisFactV2<u64>,
        semantics: DiagnosisFactV2<DiagnosisBarrierSemanticsV2>,
        expected_semantics: DiagnosisFactV2<DiagnosisBarrierSemanticsV2>,
        lds_epoch: DiagnosisLdsEpochV2,
        expected_participants: DiagnosisFactV2<u32>,
        mismatch: DiagnosisFactV2<DiagnosisBarrierMismatchV2>,
        expected_site: DiagnosisFactV2<KirSiteV1>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisViewV2 {
    pub sequence: u64,
    pub class: DiagnosisClassV2,
    pub input: DiagnosisInputEvidenceV2,
    pub context: DiagnosisExecutionContextV2,
    pub site: DiagnosisFactV2<KirSiteV1>,
    pub source_operation: DiagnosisFactV2<DiagnosisSourceOperationV2>,
    pub memory_region: DiagnosisFactV2<DiagnosisMemoryRegionV2>,
    pub barrier: DiagnosisFactV2<DiagnosisBarrierV2>,
}

impl DiagnosisViewV2 {
    fn validate(
        &self,
        session: SessionViewV1,
        completeness: CaptureCompletenessV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        if self.sequence == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        self.input.validate(session)?;
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
        match (
            &self.input.source_map_v2,
            &self.site,
            &self.source_operation,
        ) {
            (
                DiagnosisFactV2::Declared { value: map },
                DiagnosisFactV2::Observed { value: site },
                DiagnosisFactV2::Declared { value: source },
            ) => {
                if source.kir_site != *site
                    || source.location.map_identity != map.identity
                    || source.location.provenance != map.provenance
                    || source.bundle_subject_identity != map.bundle_subject_identity
                    || source.location.byte_start >= source.location.byte_end
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis source operation",
                    ));
                }
            }
            (
                DiagnosisFactV2::Declared { .. },
                _,
                DiagnosisFactV2::Unavailable {
                    reason:
                        DiagnosisUnavailableReasonV2::SourceSiteAbsent
                        | DiagnosisUnavailableReasonV2::SourceSiteAmbiguous
                        | DiagnosisUnavailableReasonV2::SiteNotRepresented,
                },
            ) => {}
            (
                DiagnosisFactV2::Unavailable { reason: map_reason },
                _,
                DiagnosisFactV2::Unavailable {
                    reason: source_reason,
                },
            ) if map_reason == source_reason => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
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
                    value: divergence @ DiagnosisBarrierV2::Divergence { .. },
                },
            ) => self.validate_divergence(completeness, divergence),
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
                            semantics,
                            expected_semantics,
                            lds_epoch,
                            expected_participants,
                            mismatch,
                            expected_site,
                        },
                },
            ) => {
                let DiagnosisFactV2::Observed {
                    value: mismatch_kind,
                } = mismatch
                else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                let DiagnosisFactV2::Observed { value: phase } = phase else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                let DiagnosisFactV2::Declared {
                    value: actual_semantics,
                } = semantics
                else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                let DiagnosisFactV2::Declared {
                    value: expected_semantics,
                } = expected_semantics
                else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                actual_semantics.validate()?;
                expected_semantics.validate()?;
                lds_epoch.validate(*phase)?;
                self.validate_expected_participants(expected_participants)?;
                if !matches!(
                    expected_site,
                    DiagnosisFactV2::Observed { .. }
                        | DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::SiteNotRepresented
                        }
                ) {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                }
                if let (
                    DiagnosisFactV2::Observed { value: actual },
                    DiagnosisFactV2::Observed { value: expected },
                ) = (&self.site, expected_site)
                {
                    let sites_match = actual == expected;
                    let mismatch_matches_sites = match mismatch_kind {
                        DiagnosisBarrierMismatchV2::Semantics => sites_match,
                        DiagnosisBarrierMismatchV2::Site
                        | DiagnosisBarrierMismatchV2::SiteAndSemantics => !sites_match,
                    };
                    if !mismatch_matches_sites {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "diagnosis barrier mismatch site",
                        ));
                    }
                }
                let semantics_match = actual_semantics == expected_semantics;
                let mismatch_matches_semantics = match mismatch_kind {
                    DiagnosisBarrierMismatchV2::Site => semantics_match,
                    DiagnosisBarrierMismatchV2::Semantics
                    | DiagnosisBarrierMismatchV2::SiteAndSemantics => !semantics_match,
                };
                if !mismatch_matches_semantics {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis barrier mismatch semantics",
                    ));
                }
                Ok(())
            }
            _ => Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
    }

    fn validate_divergence(
        &self,
        completeness: CaptureCompletenessV1,
        divergence: &DiagnosisBarrierV2,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let DiagnosisBarrierV2::Divergence {
            phase,
            semantics,
            lds_epoch,
            observed_arrivals,
            expected_participants,
            waiting,
            exited,
        } = divergence
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Observed { value: phase } = phase else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Declared { value: semantics } = semantics else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        semantics.validate()?;
        lds_epoch.validate(*phase)?;
        let arrivals = match (completeness, observed_arrivals) {
            (CaptureCompletenessV1::Complete, DiagnosisFactV2::Observed { value })
                if *value > 0 =>
            {
                Some(*value)
            }
            (
                CaptureCompletenessV1::Truncated { .. },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::TranscriptTruncated,
                },
            ) => None,
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
        exited.validate(*dispatch, *workgroup, wave.width)?;

        let DiagnosisFactV2::Observed {
            value: waiting_local,
        } = &waiting.local_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred {
            value: waiting_global,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = &waiting.global_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Observed {
            value: exited_local,
        } = &exited.local_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if waiting_local == exited_local {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier participants",
            ));
        }
        let DiagnosisFactV2::Observed {
            value: context_workitem,
        } = &self.context.workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if context_workitem.local != *waiting_local || context_workitem.global != *waiting_global {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier waiting context",
            ));
        }
        Ok(())
    }

    fn validate_expected_participants(
        &self,
        expected_participants: &DiagnosisFactV2<u32>,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let DiagnosisFactV2::Inferred {
            value: expected,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = expected_participants
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Declared { value: dispatch } = &self.context.dispatch else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Observed { value: workgroup } = &self.context.workgroup else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if *expected != active_workgroup_participants(*dispatch, *workgroup)? {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier participant count",
            ));
        }
        Ok(())
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
                    diagnosis.validate(*session, *completeness)?;
                }
                if next_cursor.is_some_and(|cursor| cursor.position == 0) {
                    return Err(ProtocolValidationErrorV1::ZeroIdentity);
                }
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
