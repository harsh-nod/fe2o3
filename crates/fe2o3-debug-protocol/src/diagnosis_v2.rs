//! Separately versioned, read-only semantic diagnosis protocol.

use std::{
    fmt::Write as _,
    io::{self, Write},
};

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
    AbiViewBounds,
    BarrierPhase,
    BarrierParticipantSet,
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
    pub envelope_version: u16,
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
    pub operation_membership_root: OpaqueIdentityV1,
    pub operation_members: u32,
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
            DiagnosisFactV2::Declared { value } if matches!(value.envelope_version, 1 | 2) => {
                Some(value.subject_identity)
            }
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
                if value.operation_members > 4_000_000 {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "diagnosis source map members",
                    ));
                }
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisSourceOperationV2 {
    pub bundle_subject_identity: OpaqueIdentityV1,
    pub kir_site: KirSiteV1,
    pub location: SourceLocationV1,
    pub membership: DiagnosisSourceMapMembershipProofV2,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisSourceMapMembershipProofV2 {
    pub member_identity: OpaqueIdentityV1,
    pub member_index: u32,
    pub member_count: u32,
    pub siblings: Vec<OpaqueIdentityV1>,
}

/// Stable leaf identity for one exact admitted Source Map V2 operation/span member.
pub fn diagnosis_source_map_member_identity_v2(
    bundle_subject_identity: OpaqueIdentityV1,
    kir_site: KirSiteV1,
    location: SourceLocationV1,
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    if location.byte_start >= location.byte_end {
        return Err(ProtocolValidationErrorV1::InvalidRange(
            "diagnosis source map member",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-source-map-operation-member-v2\0");
    digest.update(bundle_subject_identity.as_bytes());
    digest.update(location.map_identity.as_bytes());
    digest.update([match location.provenance {
        SourceMapProvenanceV1::CallerBound => 0,
        SourceMapProvenanceV1::CompilerBundleBound => 1,
    }]);
    digest.update(kir_site.function_ordinal.to_le_bytes());
    digest.update(kir_site.block_ordinal.to_le_bytes());
    match kir_site.point {
        crate::KirSitePointV1::BlockEntry => digest.update([0]),
        crate::KirSitePointV1::Operation { operation_ordinal } => {
            digest.update([1]);
            digest.update(operation_ordinal.to_le_bytes());
        }
        crate::KirSitePointV1::Terminator => digest.update([2]),
    }
    digest.update(location.file_identity.as_bytes());
    digest.update(location.byte_start.to_le_bytes());
    digest.update(location.byte_end.to_le_bytes());
    OpaqueIdentityV1::new(digest.finalize().into())
}

fn diagnosis_merkle_parent_v2(
    left: OpaqueIdentityV1,
    right: OpaqueIdentityV1,
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-source-map-operation-node-v2\0");
    digest.update(left.as_bytes());
    digest.update(right.as_bytes());
    OpaqueIdentityV1::new(digest.finalize().into())
}

/// Computes the committed operation-membership root for an exact ordered leaf inventory.
pub fn diagnosis_source_map_membership_root_v2(
    members: &[OpaqueIdentityV1],
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    if members.is_empty() {
        let digest = Sha256::digest(b"fe2o3-debug-source-map-operation-empty-v2\0");
        return OpaqueIdentityV1::new(digest.into());
    }
    let mut level = Vec::new();
    level
        .try_reserve_exact(members.len())
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("source map membership"))?;
    level.extend_from_slice(members);
    while level.len() > 1 {
        let mut next = Vec::new();
        next.try_reserve_exact(level.len().div_ceil(2))
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("source map membership"))?;
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = pair.get(1).copied().unwrap_or(left);
            next.push(diagnosis_merkle_parent_v2(left, right)?);
        }
        level = next;
    }
    level
        .into_iter()
        .next()
        .ok_or(ProtocolValidationErrorV1::ZeroIdentity)
}

/// Builds a bounded proof for one member of the exact ordered operation inventory.
pub fn diagnosis_source_map_membership_proof_v2(
    members: &[OpaqueIdentityV1],
    member_index: usize,
) -> Result<DiagnosisSourceMapMembershipProofV2, ProtocolValidationErrorV1> {
    let member_count = u32::try_from(members.len()).map_err(|_| {
        ProtocolValidationErrorV1::CountOutOfRange("diagnosis source map membership")
    })?;
    let wire_index = u32::try_from(member_index).map_err(|_| {
        ProtocolValidationErrorV1::CountOutOfRange("diagnosis source map membership")
    })?;
    let member_identity =
        members
            .get(member_index)
            .copied()
            .ok_or(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis source map membership",
            ))?;
    let mut level = Vec::new();
    level
        .try_reserve_exact(members.len())
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("source map proof"))?;
    level.extend_from_slice(members);
    let mut index = member_index;
    let mut siblings = Vec::new();
    while level.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) {
            (index + 1).min(level.len() - 1)
        } else {
            index - 1
        };
        siblings
            .try_reserve_exact(1)
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("source map proof"))?;
        siblings.push(level[sibling_index]);
        let mut next = Vec::new();
        next.try_reserve_exact(level.len().div_ceil(2))
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("source map proof"))?;
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = pair.get(1).copied().unwrap_or(left);
            next.push(diagnosis_merkle_parent_v2(left, right)?);
        }
        level = next;
        index /= 2;
    }
    Ok(DiagnosisSourceMapMembershipProofV2 {
        member_identity,
        member_index: wire_index,
        member_count,
        siblings,
    })
}

fn validate_source_map_membership_v2(
    map: DiagnosisSourceMapReferenceV2,
    source: &DiagnosisSourceOperationV2,
) -> Result<(), ProtocolValidationErrorV1> {
    if map.operation_members == 0
        || source.membership.member_count != map.operation_members
        || source.membership.member_index >= map.operation_members
        || source.membership.siblings.len() > 32
    {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis source map membership",
        ));
    }
    let expected_leaf = diagnosis_source_map_member_identity_v2(
        source.bundle_subject_identity,
        source.kir_site,
        source.location,
    )?;
    if expected_leaf != source.membership.member_identity {
        return Err(ProtocolValidationErrorV1::IdentityMismatch(
            "diagnosis source map member",
        ));
    }
    let mut node = expected_leaf;
    let mut index = source.membership.member_index;
    let mut count = source.membership.member_count;
    let mut proof = source.membership.siblings.iter();
    while count > 1 {
        let sibling = proof
            .next()
            .ok_or(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis source map proof",
            ))?;
        node = if index.is_multiple_of(2) {
            diagnosis_merkle_parent_v2(node, *sibling)?
        } else {
            diagnosis_merkle_parent_v2(*sibling, node)?
        };
        index /= 2;
        count = count.div_ceil(2);
    }
    if proof.next().is_some() {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis source map proof",
        ));
    }
    if node != map.operation_membership_root {
        return Err(ProtocolValidationErrorV1::IdentityMismatch(
            "diagnosis source map membership root",
        ));
    }
    Ok(())
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
    let declared_volume = dispatch
        .workgroup_size
        .into_iter()
        .try_fold(1_u64, |volume, size| volume.checked_mul(u64::from(size)))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow(
            "diagnosis workgroup volume",
        ))?;
    if declared_volume > MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2 as u64 {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis workgroup volume",
        ));
    }
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
    pub legal_offset: u64,
    pub legal_bytes: u64,
    pub allocation_bytes: u64,
    pub allocation_contract: DiagnosisFactV2<DiagnosisAllocationContractV2>,
    pub abi_argument: DiagnosisFactV2<DiagnosisAbiArgumentV2>,
    pub logical_element: DiagnosisFactV2<DiagnosisLogicalElementV2>,
    pub legal_bounds: DiagnosisFactV2<DiagnosisLegalBoundsPropertyV2>,
}

impl DiagnosisMemoryRegionV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.allocation.ordinal == 0 || self.allocation.generation != 0 {
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
        let legal_end = self.legal_offset.checked_add(self.legal_bytes).ok_or(
            ProtocolValidationErrorV1::RangeOverflow("diagnosis legal memory view"),
        )?;
        if self.legal_bytes == 0 || legal_end > self.allocation_bytes {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis legal memory view",
            ));
        }
        if self.requested_offset >= self.legal_offset && end <= legal_end {
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
        let DiagnosisFactV2::Declared {
            value: abi_argument,
        } = &self.abi_argument
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Declared { value: contract } = &self.allocation_contract else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        abi_argument.validate(self.allocation_bytes)?;
        if abi_argument.view_offset != self.legal_offset
            || abi_argument.view_bytes != self.legal_bytes
            || abi_argument.address_space != contract.address_space
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis faulting ABI view",
            ));
        }
        if contract
            .abi_arguments
            .binary_search_by_key(&abi_argument.ordinal, |argument| argument.ordinal)
            .ok()
            .and_then(|index| contract.abi_arguments.get(index))
            != Some(abi_argument)
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis faulting ABI argument",
            ));
        }
        let DiagnosisFactV2::Inferred {
            value: logical,
            basis: DiagnosisInferenceBasisV2::AbiViewBounds,
        } = &self.logical_element
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if logical.argument_ordinal != abi_argument.ordinal
            || logical.element != abi_argument.element
            || logical.element_bytes != diagnosis_scalar_bytes_v2(logical.element)
            || self.requested_bytes != u64::from(logical.element_bytes)
            || self.requested_offset < self.legal_offset
            || !(self.requested_offset - self.legal_offset)
                .is_multiple_of(u64::from(logical.element_bytes))
            || logical.element_index
                != (self.requested_offset - self.legal_offset) / u64::from(logical.element_bytes)
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis logical element",
            ));
        }
        let DiagnosisFactV2::Inferred {
            value: property,
            basis: DiagnosisInferenceBasisV2::AbiViewBounds,
        } = &self.legal_bounds
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        if property.argument_ordinal != abi_argument.ordinal
            || property.legal_offset != self.legal_offset
            || property.legal_bytes != self.legal_bytes
            || property.requested_offset != self.requested_offset
            || property.requested_bytes != self.requested_bytes
            || property.satisfied
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis legal bounds property",
            ));
        }
        Ok(())
    }
}

const fn diagnosis_scalar_bytes_v2(scalar: DiagnosisScalarTypeV2) -> u16 {
    match scalar {
        DiagnosisScalarTypeV2::Bool | DiagnosisScalarTypeV2::I8 | DiagnosisScalarTypeV2::U8 => 1,
        DiagnosisScalarTypeV2::I16
        | DiagnosisScalarTypeV2::U16
        | DiagnosisScalarTypeV2::F16
        | DiagnosisScalarTypeV2::Bf16 => 2,
        DiagnosisScalarTypeV2::I32
        | DiagnosisScalarTypeV2::U32
        | DiagnosisScalarTypeV2::Index32
        | DiagnosisScalarTypeV2::F32 => 4,
        DiagnosisScalarTypeV2::I64
        | DiagnosisScalarTypeV2::U64
        | DiagnosisScalarTypeV2::Index64
        | DiagnosisScalarTypeV2::F64 => 8,
        DiagnosisScalarTypeV2::I128 | DiagnosisScalarTypeV2::U128 => 16,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisAccessModeV2 {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl DiagnosisAccessModeV2 {
    /// Reports whether this supplied capability includes every required effect.
    #[must_use]
    pub const fn satisfies(self, required: Self) -> bool {
        matches!(
            (required, self),
            (Self::ReadOnly, Self::ReadOnly | Self::ReadWrite)
                | (Self::WriteOnly, Self::WriteOnly | Self::ReadWrite)
                | (Self::ReadWrite, Self::ReadWrite)
        )
    }
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
    pub backing: Option<u32>,
    pub kind: DiagnosisAbiArgumentKindV2,
    pub element: DiagnosisScalarTypeV2,
    pub address_space: AddressSpaceV1,
    /// Access capability required by the kernel ABI.
    pub access: DiagnosisAccessModeV2,
    /// Access capability supplied by this request argument or buffer view.
    pub supplied_access: DiagnosisAccessModeV2,
    pub view_offset: u64,
    pub view_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisLogicalElementV2 {
    pub argument_ordinal: u32,
    pub element: DiagnosisScalarTypeV2,
    pub element_bytes: u16,
    pub element_index: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisLegalBoundsPropertyV2 {
    pub argument_ordinal: u32,
    pub legal_offset: u64,
    pub legal_bytes: u64,
    pub requested_offset: u64,
    pub requested_bytes: u64,
    pub satisfied: bool,
}

impl DiagnosisAbiArgumentV2 {
    fn validate(self, allocation_bytes: u64) -> Result<(), ProtocolValidationErrorV1> {
        let element_bytes = u64::from(diagnosis_scalar_bytes_v2(self.element));
        let end = self.view_offset.checked_add(self.view_bytes).ok_or(
            ProtocolValidationErrorV1::RangeOverflow("diagnosis ABI view"),
        )?;
        if self.view_bytes == 0
            || !self.view_offset.is_multiple_of(element_bytes)
            || !self.view_bytes.is_multiple_of(element_bytes)
            || end > allocation_bytes
        {
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
    /// Access capability supplied by the backing allocation.
    pub access: DiagnosisAccessModeV2,
    pub alignment: u32,
    pub allocation_bytes: u64,
    pub abi_arguments: Vec<DiagnosisAbiArgumentV2>,
}

impl DiagnosisAllocationContractV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.alignment == 0 || !self.alignment.is_power_of_two() || self.allocation_bytes == 0 {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis allocation alignment",
            ));
        }
        if self.abi_arguments.len() > 256
            || self
                .abi_arguments
                .windows(2)
                .any(|pair| pair[0].ordinal >= pair[1].ordinal)
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis ABI arguments",
            ));
        }
        for argument in &self.abi_arguments {
            argument.validate(self.allocation_bytes)?;
            if argument.address_space != self.address_space
                || !argument.supplied_access.satisfies(argument.access)
                || (argument.backing.is_none() && argument.supplied_access != self.access)
                || (argument.backing.is_some() && !self.access.satisfies(argument.supplied_access))
            {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis ABI allocation contract",
                ));
            }
        }
        if self.abi_arguments.len() > 1 {
            let Some(backing) = self
                .abi_arguments
                .first()
                .and_then(|argument| argument.backing)
            else {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis ABI shared backing",
                ));
            };
            if self
                .abi_arguments
                .iter()
                .any(|argument| argument.backing != Some(backing))
            {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis ABI shared backing",
                ));
            }
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
        inferred_local: bool,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let local = match (&self.local_workitem, inferred_local) {
            (
                DiagnosisFactV2::Inferred {
                    value,
                    basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                },
                true,
            )
            | (DiagnosisFactV2::Observed { value }, false) => value,
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
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

const MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2: usize = 1_024;

fn observed_participant_set(
    fact: &DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
) -> Result<&[DiagnosisBarrierParticipantV2], ProtocolValidationErrorV1> {
    let DiagnosisFactV2::Observed { value } = fact else {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    };
    Ok(value)
}

fn validate_participant_set(
    participants: &[DiagnosisBarrierParticipantV2],
    dispatch: DiagnosisDispatchV2,
    workgroup: [u64; 3],
    wave_width: u16,
    inferred_local: bool,
) -> Result<(), ProtocolValidationErrorV1> {
    if participants.len() > MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2 {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis barrier participant set",
        ));
    }
    let mut previous = None;
    for participant in participants {
        participant.validate(dispatch, workgroup, wave_width, inferred_local)?;
        let local = match &participant.local_workitem {
            DiagnosisFactV2::Observed { value } | DiagnosisFactV2::Inferred { value, .. } => value,
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        };
        if previous.is_some_and(|previous| previous >= *local) {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "diagnosis barrier participant order",
            ));
        }
        previous = Some(*local);
    }
    Ok(())
}

fn participant_locals(
    participants: &[DiagnosisBarrierParticipantV2],
) -> Result<Vec<[u32; 3]>, ProtocolValidationErrorV1> {
    let mut locals = Vec::new();
    locals
        .try_reserve_exact(participants.len())
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis participants"))?;
    for participant in participants {
        let value = match &participant.local_workitem {
            DiagnosisFactV2::Observed { value } | DiagnosisFactV2::Inferred { value, .. } => value,
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        };
        locals.push(*value);
    }
    Ok(locals)
}

fn expected_participant_locals(
    dispatch: DiagnosisDispatchV2,
    workgroup: [u64; 3],
) -> Result<Vec<[u32; 3]>, ProtocolValidationErrorV1> {
    let count = usize::try_from(active_workgroup_participants(dispatch, workgroup)?)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis participants"))?;
    if count > MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2 {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis barrier participant set",
        ));
    }
    let mut locals = Vec::new();
    locals
        .try_reserve_exact(count)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis participants"))?;
    let mut active_extent = [0_u32; 3];
    for axis in 0..3 {
        let start = workgroup[axis]
            .checked_mul(u64::from(dispatch.workgroup_size[axis]))
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "diagnosis participant set",
            ))?;
        let remaining = dispatch.launch_extent[axis].checked_sub(start).ok_or(
            ProtocolValidationErrorV1::CountOutOfRange("diagnosis workgroup coordinate"),
        )?;
        active_extent[axis] = u32::try_from(
            remaining.min(u64::from(dispatch.workgroup_size[axis])),
        )
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis participants"))?;
    }
    for z in 0..active_extent[2] {
        for y in 0..active_extent[1] {
            for x in 0..active_extent[0] {
                locals.push([x, y, z]);
            }
        }
    }
    locals.sort_unstable();
    if locals.len() != count {
        return Err(ProtocolValidationErrorV1::IdentityMismatch(
            "diagnosis participant set count",
        ));
    }
    Ok(locals)
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
        if !matches!(
            &self.current,
            DiagnosisFactV2::Inferred {
                value,
                basis: DiagnosisInferenceBasisV2::BarrierPhase
            } if *value == phase
        ) || !matches!(
            &self.after_release,
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::BarrierNotReleased
            }
        ) {
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
        expected_participant_set: DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
        arrived_participants: DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
        waiting_participants: DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
        exited_participants: DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
    },
    Mismatch {
        phase: DiagnosisFactV2<u64>,
        semantics: DiagnosisFactV2<DiagnosisBarrierSemanticsV2>,
        expected_semantics: DiagnosisFactV2<DiagnosisBarrierSemanticsV2>,
        lds_epoch: DiagnosisLdsEpochV2,
        expected_participants: DiagnosisFactV2<u32>,
        expected_participant_set: DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
        mismatch: DiagnosisFactV2<DiagnosisBarrierMismatchV2>,
        expected_site: DiagnosisFactV2<KirSiteV1>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisEvidenceSourceV2 {
    AdmittedInputRecord,
    SimulatorTerminalRecord,
    SimulatorTranscriptRecord,
    CanonicalKirOperationRecord,
    KernelAbiRecord,
    SourceMapOperationRecord,
    DerivedRecord,
    AvailabilityRecord,
}

/// Canonical logical invocation retained from the deterministic simulator fault.
///
/// This record contains no host/native pointer or device address.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisTerminalInvocationRecordV2 {
    pub global: [u64; 3],
    pub workgroup: [u64; 3],
    pub local: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub launch_extent: [u64; 3],
}

impl DiagnosisTerminalInvocationRecordV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        let dispatch = DiagnosisDispatchV2 {
            launch_extent: self.launch_extent,
            workgroup_size: self.workgroup_size,
        };
        dispatch.validate()?;
        active_workgroup_participants(dispatch, self.workgroup)?;
        validate_workitem(
            dispatch,
            self.workgroup,
            DiagnosisWorkitemV2 {
                global: self.global,
                local: self.local,
            },
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisTerminalAbiViewRecordV2 {
    pub allocation_contract: DiagnosisAllocationContractV2,
    pub abi_argument: DiagnosisAbiArgumentV2,
    pub legal_offset: u64,
    pub legal_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosisTerminalPayloadRecordV2 {
    MemoryOutOfBounds {
        allocation: AllocationIdentityV1,
        requested_offset: u64,
        requested_bytes: u64,
        allocation_bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        abi_view: Option<DiagnosisTerminalAbiViewRecordV2>,
    },
    WorkgroupBarrierDivergence {
        phase: u64,
        waiting_representative: [u32; 3],
        exited_representative: [u32; 3],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        waiting: Option<Vec<[u32; 3]>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exited: Option<Vec<[u32; 3]>>,
    },
    WorkgroupBarrierMismatch {
        phase: u64,
        mismatch: DiagnosisBarrierMismatchV2,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_site: Option<KirSiteV1>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisTerminalEvidenceRecordV2 {
    pub sequence: u64,
    pub class: DiagnosisClassV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<DiagnosisTerminalInvocationRecordV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<KirSiteV1>,
    pub payload: DiagnosisTerminalPayloadRecordV2,
}

impl DiagnosisTerminalEvidenceRecordV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.sequence == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if let Some(invocation) = self.invocation {
            invocation.validate()?;
        }
        match (&self.class, &self.payload) {
            (
                DiagnosisClassV2::MemoryOutOfBounds,
                DiagnosisTerminalPayloadRecordV2::MemoryOutOfBounds {
                    allocation,
                    requested_offset,
                    requested_bytes,
                    allocation_bytes,
                    abi_view,
                },
            ) => {
                if allocation.ordinal == 0
                    || allocation.generation != 0
                    || *requested_bytes == 0
                    || requested_offset.checked_add(*requested_bytes).is_none()
                    || *allocation_bytes == 0
                {
                    return Err(ProtocolValidationErrorV1::InvalidRange(
                        "diagnosis terminal memory record",
                    ));
                }
                if let Some(view) = abi_view {
                    view.allocation_contract.validate()?;
                    view.abi_argument.validate(*allocation_bytes)?;
                    let legal_end = view.legal_offset.checked_add(view.legal_bytes).ok_or(
                        ProtocolValidationErrorV1::RangeOverflow("diagnosis terminal ABI view"),
                    )?;
                    if view.legal_bytes == 0 || legal_end > *allocation_bytes {
                        return Err(ProtocolValidationErrorV1::InvalidRange(
                            "diagnosis terminal ABI view",
                        ));
                    }
                    if view.allocation_contract.allocation_bytes != *allocation_bytes
                        || view.abi_argument.view_offset != view.legal_offset
                        || view.abi_argument.view_bytes != view.legal_bytes
                        || view.abi_argument.address_space != view.allocation_contract.address_space
                        || view
                            .allocation_contract
                            .abi_arguments
                            .binary_search_by_key(&view.abi_argument.ordinal, |argument| {
                                argument.ordinal
                            })
                            .ok()
                            .and_then(|index| view.allocation_contract.abi_arguments.get(index))
                            != Some(&view.abi_argument)
                    {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "diagnosis terminal ABI contract",
                        ));
                    }
                }
            }
            (
                DiagnosisClassV2::WorkgroupBarrierDivergence,
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierDivergence {
                    waiting_representative,
                    exited_representative,
                    waiting,
                    exited,
                    ..
                },
            ) => match (waiting, exited) {
                (Some(waiting), Some(exited)) => {
                    validate_terminal_participant_locals_v2(waiting)?;
                    validate_terminal_participant_locals_v2(exited)?;
                    if waiting.is_empty()
                        || exited.is_empty()
                        || waiting.binary_search(waiting_representative).is_err()
                        || exited.binary_search(exited_representative).is_err()
                        || waiting
                            .iter()
                            .any(|participant| exited.binary_search(participant).is_ok())
                    {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "diagnosis terminal divergence inventory",
                        ));
                    }
                }
                (None, None) => {}
                _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
            },
            (
                DiagnosisClassV2::WorkgroupBarrierMismatch,
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierMismatch { .. },
            ) => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        Ok(())
    }
}

fn validate_terminal_participant_locals_v2(
    participants: &[[u32; 3]],
) -> Result<(), ProtocolValidationErrorV1> {
    if participants.len() > MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2
        || participants.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis terminal participant inventory",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisBarrierArrivalEvidenceRecordV2 {
    pub sequence: u64,
    pub local: [u32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<KirSiteV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisBarrierTranscriptEvidenceV2 {
    pub phase: u64,
    pub workgroup: [u64; 3],
    pub arrivals: Vec<DiagnosisBarrierArrivalEvidenceRecordV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisTranscriptEvidenceRecordV2 {
    pub completeness: CaptureCompletenessV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barrier: Option<DiagnosisBarrierTranscriptEvidenceV2>,
}

impl DiagnosisTranscriptEvidenceRecordV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if let Some(barrier) = &self.barrier {
            if barrier.arrivals.len() > MAX_DIAGNOSIS_BARRIER_PARTICIPANTS_V2
                || barrier.arrivals.windows(2).any(|pair| {
                    pair[0].sequence >= pair[1].sequence || pair[0].local == pair[1].local
                })
                || barrier.arrivals.iter().any(|arrival| arrival.sequence == 0)
            {
                return Err(ProtocolValidationErrorV1::CountOutOfRange(
                    "diagnosis transcript barrier arrivals",
                ));
            }
            let mut locals = Vec::new();
            locals
                .try_reserve_exact(barrier.arrivals.len())
                .map_err(|_| {
                    ProtocolValidationErrorV1::CountOutOfRange(
                        "diagnosis transcript barrier arrivals",
                    )
                })?;
            locals.extend(barrier.arrivals.iter().map(|arrival| arrival.local));
            locals.sort_unstable();
            if locals.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis transcript barrier participant",
                ));
            }
        }
        Ok(())
    }
}

/// Canonical bounded evidence copied from one owned deterministic simulator result.
///
/// Hash validation proves content integrity only. A consumer needs an independently
/// retained `DiagnosisCaptureBindingV2` to authenticate it to a particular capture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisRetainedEvidenceV2 {
    pub terminal: DiagnosisTerminalEvidenceRecordV2,
    pub transcript: DiagnosisTranscriptEvidenceRecordV2,
}

impl DiagnosisRetainedEvidenceV2 {
    /// Derives the capture-owner side admission token from independently owned
    /// simulator input, session, and response-envelope state.
    ///
    /// Deriving this from an untrusted response only proves that response's own
    /// content integrity; the caller must retain the returned value separately.
    pub fn capture_binding_v2(
        &self,
        input: &DiagnosisInputEvidenceV2,
        session: SessionViewV1,
        completeness: CaptureCompletenessV1,
        response: DiagnosisResponseEnvelopeBindingV2,
    ) -> Result<DiagnosisCaptureBindingV2, ProtocolValidationErrorV1> {
        self.validate()?;
        validate_simulator_session(session)?;
        input.validate(session)?;
        response.validate()?;
        if self.transcript.completeness != completeness {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis capture completeness",
            ));
        }
        Ok(DiagnosisCaptureBindingV2 {
            session,
            completeness,
            dispatch_identity: input.dispatch_identity,
            simulation_bundle: match &input.simulation_bundle {
                DiagnosisFactV2::Declared { value } => Some(*value),
                DiagnosisFactV2::Unavailable { .. } => None,
                _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
            },
            input_manifest_identity: diagnosis_json_hash_v2(
                b"fe2o3-debug-diagnosis-input-manifest-v2\0",
                &[],
                input,
            )?,
            response_binding_identity: diagnosis_json_hash_v2(
                b"fe2o3-debug-diagnosis-response-binding-v2\0",
                &[],
                &(
                    session,
                    completeness,
                    self.terminal.sequence,
                    self.terminal.class,
                ),
            )?,
            response_envelope_identity: diagnosis_json_hash_v2(
                b"fe2o3-debug-diagnosis-response-envelope-binding-v2\0",
                &[],
                &response,
            )?,
            terminal_record_identity: diagnosis_json_hash_v2(
                b"fe2o3-debug-simulator-terminal-record-v2\0",
                &[&input.dispatch_identity.as_bytes()],
                &self.terminal,
            )?,
            transcript_record_identity: diagnosis_json_hash_v2(
                b"fe2o3-debug-simulator-transcript-record-v2\0",
                &[&input.dispatch_identity.as_bytes()],
                &self.transcript,
            )?,
        })
    }

    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        self.terminal.validate()?;
        self.transcript.validate()
    }
}

/// Exact non-circular binding for the response wrapper owned by the query producer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisResponseEnvelopeBindingV2 {
    pub schema: DiagnosisResponseSchemaV2,
    pub request_id: u64,
    pub operation: DiagnosisOperationV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<PageCursorV1>,
}

impl DiagnosisResponseEnvelopeBindingV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.request_id == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        if self.next_cursor.is_some_and(|cursor| cursor.position == 0) {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}

/// Independently retained admission token for one exact simulator response.
///
/// The token authenticates a response only when its inputs came from the capture
/// owner's trusted simulator session. It is not a producer signature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisCaptureBindingV2 {
    pub session: SessionViewV1,
    pub completeness: CaptureCompletenessV1,
    pub dispatch_identity: OpaqueIdentityV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation_bundle: Option<DiagnosisBundleReferenceV2>,
    pub input_manifest_identity: OpaqueIdentityV1,
    pub response_binding_identity: OpaqueIdentityV1,
    pub response_envelope_identity: OpaqueIdentityV1,
    pub terminal_record_identity: OpaqueIdentityV1,
    pub transcript_record_identity: OpaqueIdentityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisEvidenceCitationV2 {
    pub field: String,
    pub source: DiagnosisEvidenceSourceV2,
    pub source_record_identity: OpaqueIdentityV1,
    pub claim_identity: OpaqueIdentityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisEvidenceManifestV2 {
    pub response_binding_identity: OpaqueIdentityV1,
    pub input_manifest_identity: OpaqueIdentityV1,
    pub terminal_record_identity: OpaqueIdentityV1,
    pub transcript_record_identity: OpaqueIdentityV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained: Option<DiagnosisRetainedEvidenceV2>,
    pub citations: Vec<DiagnosisEvidenceCitationV2>,
    pub manifest_identity: OpaqueIdentityV1,
}

impl DiagnosisEvidenceManifestV2 {
    /// Placeholder accepted only while a trusted producer is assembling a diagnosis.
    /// Response validation rejects it because it cannot match the completed claim inventory.
    pub fn unsealed() -> Result<Self, ProtocolValidationErrorV1> {
        let identity = OpaqueIdentityV1::new(
            Sha256::digest(b"fe2o3-debug-unsealed-diagnosis-evidence-v2\0").into(),
        )?;
        Ok(Self {
            response_binding_identity: identity,
            input_manifest_identity: identity,
            terminal_record_identity: identity,
            transcript_record_identity: identity,
            retained: None,
            citations: Vec::new(),
            manifest_identity: identity,
        })
    }
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
    pub evidence: DiagnosisEvidenceManifestV2,
}

struct DiagnosisEvidenceBuilderV2<'a> {
    input: &'a DiagnosisInputEvidenceV2,
    site: &'a DiagnosisFactV2<KirSiteV1>,
    source_operation: &'a DiagnosisFactV2<DiagnosisSourceOperationV2>,
    input_manifest_identity: OpaqueIdentityV1,
    terminal_record_identity: OpaqueIdentityV1,
    transcript_record_identity: OpaqueIdentityV1,
    citations: Vec<DiagnosisEvidenceCitationV2>,
}

const MAX_DIAGNOSIS_EVIDENCE_CITATIONS_V2: usize = 16_384;

impl DiagnosisEvidenceBuilderV2<'_> {
    fn push<T: Serialize>(
        &mut self,
        field: &str,
        source: DiagnosisEvidenceSourceV2,
        value: &T,
    ) -> Result<(), ProtocolValidationErrorV1> {
        self.push_with_kir_site(field, source, value, None)
    }

    fn push_with_kir_site<T: Serialize>(
        &mut self,
        field: &str,
        source: DiagnosisEvidenceSourceV2,
        value: &T,
        kir_site: Option<KirSiteV1>,
    ) -> Result<(), ProtocolValidationErrorV1> {
        if field.is_empty()
            || field.len() > 128
            || self.citations.len() >= MAX_DIAGNOSIS_EVIDENCE_CITATIONS_V2
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis evidence citation",
            ));
        }
        let source_binding = match source {
            DiagnosisEvidenceSourceV2::AdmittedInputRecord => {
                diagnosis_json_hash_v2(b"fe2o3-debug-admitted-input-record-v2\0", &[], value)?
            }
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord => self.terminal_record_identity,
            DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord => self.transcript_record_identity,
            DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord => {
                let kir = diagnosis_declared_content(&self.input.canonical_kir_v7)?;
                let site = match (kir_site, self.site) {
                    (Some(site), _) => site,
                    (None, DiagnosisFactV2::Observed { value }) => *value,
                    (None, _) => {
                        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                    }
                };
                diagnosis_json_hash_v2(
                    b"fe2o3-debug-canonical-kir-record-v2\0",
                    &[&kir.sha256.as_bytes(), &kir.canonical_bytes.to_le_bytes()],
                    &site,
                )?
            }
            DiagnosisEvidenceSourceV2::KernelAbiRecord => {
                let binding = match &self.input.kernel_abi_identity {
                    DiagnosisFactV2::Declared { value } => *value,
                    _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
                };
                diagnosis_json_hash_v2(
                    b"fe2o3-debug-kernel-abi-record-v2\0",
                    &[&binding.as_bytes()],
                    value,
                )?
            }
            DiagnosisEvidenceSourceV2::SourceMapOperationRecord => {
                let DiagnosisFactV2::Declared { value } = self.source_operation else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                value.membership.member_identity
            }
            DiagnosisEvidenceSourceV2::DerivedRecord => diagnosis_hash_v2(
                b"fe2o3-debug-derived-evidence-record-v2\0",
                &[
                    &self.input_manifest_identity.as_bytes(),
                    &self.terminal_record_identity.as_bytes(),
                    field.as_bytes(),
                ],
            )?,
            DiagnosisEvidenceSourceV2::AvailabilityRecord => diagnosis_json_hash_v2(
                b"fe2o3-debug-availability-record-v2\0",
                &[&self.input_manifest_identity.as_bytes(), field.as_bytes()],
                value,
            )?,
        };
        let claim_identity = diagnosis_json_hash_v2(
            b"fe2o3-debug-diagnosis-claim-v2\0",
            &[&source_binding.as_bytes(), field.as_bytes()],
            value,
        )?;
        self.citations
            .try_reserve_exact(1)
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis evidence"))?;
        let mut owned_field = String::new();
        owned_field
            .try_reserve_exact(field.len())
            .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis evidence field"))?;
        owned_field.push_str(field);
        self.citations.push(DiagnosisEvidenceCitationV2 {
            field: owned_field,
            source,
            source_record_identity: source_binding,
            claim_identity,
        });
        Ok(())
    }
}

fn diagnosis_hash_v2(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    OpaqueIdentityV1::new(digest.finalize().into())
}

struct DiagnosisHashWriterV2<'a>(&'a mut Sha256);

impl Write for DiagnosisHashWriterV2<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn diagnosis_json_hash_v2<T: Serialize>(
    domain: &[u8],
    fields: &[&[u8]],
    value: &T,
) -> Result<OpaqueIdentityV1, ProtocolValidationErrorV1> {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update(
            u64::try_from(field.len())
                .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis hash field"))?
                .to_le_bytes(),
        );
        digest.update(field);
    }
    digest.update(b"json\0");
    serde_json::to_writer(DiagnosisHashWriterV2(&mut digest), value)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis evidence claim"))?;
    OpaqueIdentityV1::new(digest.finalize().into())
}

fn diagnosis_declared_content(
    fact: &DiagnosisFactV2<DiagnosisContentReferenceV2>,
) -> Result<DiagnosisContentReferenceV2, ProtocolValidationErrorV1> {
    match fact {
        DiagnosisFactV2::Declared { value } => Ok(*value),
        _ => Err(ProtocolValidationErrorV1::InvalidTruthClassification),
    }
}

fn push_diagnosis_fact_v2<T: Serialize>(
    builder: &mut DiagnosisEvidenceBuilderV2<'_>,
    field: &str,
    preferred_source: DiagnosisEvidenceSourceV2,
    fact: &DiagnosisFactV2<T>,
) -> Result<(), ProtocolValidationErrorV1> {
    let source = match fact {
        DiagnosisFactV2::Declared { .. } => preferred_source,
        DiagnosisFactV2::Observed { .. } => DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
        DiagnosisFactV2::Inferred { .. } => DiagnosisEvidenceSourceV2::DerivedRecord,
        DiagnosisFactV2::Unavailable { .. } => DiagnosisEvidenceSourceV2::AvailabilityRecord,
    };
    builder.push(field, source, fact)
}

fn push_kir_fact_at_site_v2<T: Serialize>(
    builder: &mut DiagnosisEvidenceBuilderV2<'_>,
    field: &str,
    fact: &DiagnosisFactV2<T>,
    site: &DiagnosisFactV2<KirSiteV1>,
) -> Result<(), ProtocolValidationErrorV1> {
    let DiagnosisFactV2::Declared { .. } = fact else {
        return push_diagnosis_fact_v2(
            builder,
            field,
            DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord,
            fact,
        );
    };
    let DiagnosisFactV2::Observed { value: site } = site else {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    };
    builder.push_with_kir_site(
        field,
        DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord,
        fact,
        Some(*site),
    )
}

fn push_participant_set_claims_v2(
    builder: &mut DiagnosisEvidenceBuilderV2<'_>,
    field: &str,
    preferred_source: DiagnosisEvidenceSourceV2,
    fact: &DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
) -> Result<(), ProtocolValidationErrorV1> {
    if preferred_source == DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord {
        builder.push(field, preferred_source, fact)?;
    } else {
        push_diagnosis_fact_v2(builder, field, preferred_source, fact)?;
    }
    let participants = match fact {
        DiagnosisFactV2::Observed { value } | DiagnosisFactV2::Inferred { value, .. } => value,
        DiagnosisFactV2::Declared { .. } | DiagnosisFactV2::Unavailable { .. } => return Ok(()),
    };
    for (index, participant) in participants.iter().enumerate() {
        let path = diagnosis_participant_field_v2(field, index, "")?;
        builder.push(&path, preferred_source, participant)?;
        let local_field = diagnosis_participant_field_v2(field, index, ".local_workitem")?;
        if preferred_source == DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord {
            builder.push(
                &local_field,
                DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord,
                &participant.local_workitem,
            )?;
        } else {
            push_diagnosis_fact_v2(
                builder,
                &local_field,
                preferred_source,
                &participant.local_workitem,
            )?;
        }
        push_diagnosis_fact_v2(
            builder,
            &diagnosis_participant_field_v2(field, index, ".global_workitem")?,
            DiagnosisEvidenceSourceV2::DerivedRecord,
            &participant.global_workitem,
        )?;
        push_diagnosis_fact_v2(
            builder,
            &diagnosis_participant_field_v2(field, index, ".wave")?,
            DiagnosisEvidenceSourceV2::DerivedRecord,
            &participant.wave,
        )?;
        push_diagnosis_fact_v2(
            builder,
            &diagnosis_participant_field_v2(field, index, ".lane")?,
            DiagnosisEvidenceSourceV2::DerivedRecord,
            &participant.lane,
        )?;
    }
    Ok(())
}

fn diagnosis_participant_field_v2(
    field: &str,
    index: usize,
    suffix: &str,
) -> Result<String, ProtocolValidationErrorV1> {
    let capacity = field
        .len()
        .checked_add(suffix.len())
        .and_then(|length| length.checked_add(22))
        .ok_or(ProtocolValidationErrorV1::CountOutOfRange(
            "diagnosis participant evidence field",
        ))?;
    let mut path = String::new();
    path.try_reserve_exact(capacity)
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis evidence field"))?;
    write!(&mut path, "{field}[{index}]{suffix}")
        .map_err(|_| ProtocolValidationErrorV1::CountOutOfRange("diagnosis evidence field"))?;
    Ok(path)
}

const fn diagnosis_evidence_source_tag_v2(source: DiagnosisEvidenceSourceV2) -> u8 {
    match source {
        DiagnosisEvidenceSourceV2::AdmittedInputRecord => 0,
        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord => 1,
        DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord => 2,
        DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord => 3,
        DiagnosisEvidenceSourceV2::KernelAbiRecord => 4,
        DiagnosisEvidenceSourceV2::SourceMapOperationRecord => 5,
        DiagnosisEvidenceSourceV2::DerivedRecord => 6,
        DiagnosisEvidenceSourceV2::AvailabilityRecord => 7,
    }
}

impl DiagnosisViewV2 {
    /// Seals every material diagnosis claim against exact retained content records.
    ///
    /// This establishes canonical content integrity, not producer signing or capture
    /// authenticity. Use `DiagnosisResponseV2::validate_against_capture_v2` with an
    /// independently retained binding for the latter.
    pub fn seal_evidence_v2(
        &mut self,
        session: SessionViewV1,
        completeness: CaptureCompletenessV1,
        retained: DiagnosisRetainedEvidenceV2,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let mut evidence = self.build_evidence_manifest_v2(session, completeness, &retained)?;
        evidence.retained = Some(retained);
        self.evidence = evidence;
        Ok(())
    }

    fn build_evidence_manifest_v2(
        &self,
        session: SessionViewV1,
        completeness: CaptureCompletenessV1,
        retained: &DiagnosisRetainedEvidenceV2,
    ) -> Result<DiagnosisEvidenceManifestV2, ProtocolValidationErrorV1> {
        retained.validate()?;
        let terminal_record_identity = diagnosis_json_hash_v2(
            b"fe2o3-debug-simulator-terminal-record-v2\0",
            &[&self.input.dispatch_identity.as_bytes()],
            &retained.terminal,
        )?;
        let transcript_record_identity = diagnosis_json_hash_v2(
            b"fe2o3-debug-simulator-transcript-record-v2\0",
            &[&self.input.dispatch_identity.as_bytes()],
            &retained.transcript,
        )?;
        let response_binding_identity = diagnosis_json_hash_v2(
            b"fe2o3-debug-diagnosis-response-binding-v2\0",
            &[],
            &(session, completeness, self.sequence, self.class),
        )?;
        let input_manifest_identity = diagnosis_json_hash_v2(
            b"fe2o3-debug-diagnosis-input-manifest-v2\0",
            &[],
            &self.input,
        )?;
        let mut builder = DiagnosisEvidenceBuilderV2 {
            input: &self.input,
            site: &self.site,
            source_operation: &self.source_operation,
            input_manifest_identity,
            terminal_record_identity,
            transcript_record_identity,
            citations: Vec::new(),
        };

        push_diagnosis_fact_v2(
            &mut builder,
            "input.dispatch_request",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.dispatch_request,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.canonical_kir_v7",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.canonical_kir_v7,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.simulation_bundle",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.simulation_bundle,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.production_kir",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.production_kir,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.kernel_abi_identity",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.kernel_abi_identity,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.source_lineage",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.source_lineage,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.source_map_v2",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.source_map_v2,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.finalized_artifact",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.finalized_artifact,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "input.property_proof",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.input.property_proof,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "context.dispatch",
            DiagnosisEvidenceSourceV2::AdmittedInputRecord,
            &self.context.dispatch,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "context.workgroup",
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
            &self.context.workgroup,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "context.workitem",
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
            &self.context.workitem,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "context.wave",
            DiagnosisEvidenceSourceV2::DerivedRecord,
            &self.context.wave,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "context.lane",
            DiagnosisEvidenceSourceV2::DerivedRecord,
            &self.context.lane,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "site",
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
            &self.site,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "source_operation",
            DiagnosisEvidenceSourceV2::SourceMapOperationRecord,
            &self.source_operation,
        )?;
        push_diagnosis_fact_v2(
            &mut builder,
            "memory_region",
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
            &self.memory_region,
        )?;
        if let DiagnosisFactV2::Observed { value: memory } = &self.memory_region {
            let abi_source = if matches!(
                &self.input.kernel_abi_identity,
                DiagnosisFactV2::Declared { .. }
            ) {
                DiagnosisEvidenceSourceV2::KernelAbiRecord
            } else {
                DiagnosisEvidenceSourceV2::AdmittedInputRecord
            };
            push_diagnosis_fact_v2(
                &mut builder,
                "memory_region.allocation_contract",
                abi_source,
                &memory.allocation_contract,
            )?;
            push_diagnosis_fact_v2(
                &mut builder,
                "memory_region.abi_argument",
                abi_source,
                &memory.abi_argument,
            )?;
            push_diagnosis_fact_v2(
                &mut builder,
                "memory_region.logical_element",
                DiagnosisEvidenceSourceV2::DerivedRecord,
                &memory.logical_element,
            )?;
            push_diagnosis_fact_v2(
                &mut builder,
                "memory_region.legal_bounds",
                DiagnosisEvidenceSourceV2::DerivedRecord,
                &memory.legal_bounds,
            )?;
        }
        push_diagnosis_fact_v2(
            &mut builder,
            "barrier",
            DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
            &self.barrier,
        )?;
        if let DiagnosisFactV2::Observed { value: barrier } = &self.barrier {
            match barrier {
                DiagnosisBarrierV2::Divergence {
                    phase,
                    semantics,
                    lds_epoch,
                    observed_arrivals,
                    expected_participants,
                    expected_participant_set,
                    arrived_participants,
                    waiting_participants,
                    exited_participants,
                } => {
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.phase",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        phase,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.semantics",
                        DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord,
                        semantics,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.lds_epoch.current",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        &lds_epoch.current,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.lds_epoch.after_release",
                        DiagnosisEvidenceSourceV2::AvailabilityRecord,
                        &lds_epoch.after_release,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.observed_arrivals",
                        DiagnosisEvidenceSourceV2::SimulatorTranscriptRecord,
                        observed_arrivals,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.expected_participants",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        expected_participants,
                    )?;
                    push_participant_set_claims_v2(
                        &mut builder,
                        "barrier.expected_participant_set",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        expected_participant_set,
                    )?;
                    push_participant_set_claims_v2(
                        &mut builder,
                        "barrier.arrived_participants",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        arrived_participants,
                    )?;
                    push_participant_set_claims_v2(
                        &mut builder,
                        "barrier.waiting_participants",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        waiting_participants,
                    )?;
                    push_participant_set_claims_v2(
                        &mut builder,
                        "barrier.exited_participants",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        exited_participants,
                    )?;
                }
                DiagnosisBarrierV2::Mismatch {
                    phase,
                    semantics,
                    expected_semantics,
                    lds_epoch,
                    expected_participants,
                    expected_participant_set,
                    mismatch,
                    expected_site,
                } => {
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.phase",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        phase,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.semantics",
                        DiagnosisEvidenceSourceV2::CanonicalKirOperationRecord,
                        semantics,
                    )?;
                    push_kir_fact_at_site_v2(
                        &mut builder,
                        "barrier.expected_semantics",
                        expected_semantics,
                        expected_site,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.lds_epoch.current",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        &lds_epoch.current,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.lds_epoch.after_release",
                        DiagnosisEvidenceSourceV2::AvailabilityRecord,
                        &lds_epoch.after_release,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.expected_participants",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        expected_participants,
                    )?;
                    push_participant_set_claims_v2(
                        &mut builder,
                        "barrier.expected_participant_set",
                        DiagnosisEvidenceSourceV2::DerivedRecord,
                        expected_participant_set,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.mismatch",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        mismatch,
                    )?;
                    push_diagnosis_fact_v2(
                        &mut builder,
                        "barrier.expected_site",
                        DiagnosisEvidenceSourceV2::SimulatorTerminalRecord,
                        expected_site,
                    )?;
                }
            }
        }
        let citations = builder.citations;
        let mut digest = Sha256::new();
        digest.update(b"fe2o3-debug-diagnosis-evidence-manifest-v2\0");
        digest.update(response_binding_identity.as_bytes());
        digest.update(input_manifest_identity.as_bytes());
        digest.update(terminal_record_identity.as_bytes());
        digest.update(transcript_record_identity.as_bytes());
        for citation in &citations {
            digest.update((citation.field.len() as u64).to_le_bytes());
            digest.update(citation.field.as_bytes());
            digest.update([diagnosis_evidence_source_tag_v2(citation.source)]);
            digest.update(citation.source_record_identity.as_bytes());
            digest.update(citation.claim_identity.as_bytes());
        }
        let manifest_identity = OpaqueIdentityV1::new(digest.finalize().into())?;
        Ok(DiagnosisEvidenceManifestV2 {
            response_binding_identity,
            input_manifest_identity,
            terminal_record_identity,
            transcript_record_identity,
            retained: None,
            citations,
            manifest_identity,
        })
    }

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
                validate_source_map_membership_v2(*map, source)?;
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
                            expected_participant_set,
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
                self.validate_expected_participant_set(expected_participant_set)?;
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
        }?;
        let retained = self
            .evidence
            .retained
            .as_ref()
            .ok_or(ProtocolValidationErrorV1::InvalidTruthClassification)?;
        retained.validate()?;
        self.validate_retained_evidence_v2(completeness, retained)?;
        let expected = self.build_evidence_manifest_v2(session, completeness, retained)?;
        if expected.response_binding_identity != self.evidence.response_binding_identity
            || expected.input_manifest_identity != self.evidence.input_manifest_identity
            || expected.terminal_record_identity != self.evidence.terminal_record_identity
            || expected.transcript_record_identity != self.evidence.transcript_record_identity
            || expected.citations != self.evidence.citations
            || expected.manifest_identity != self.evidence.manifest_identity
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis evidence manifest",
            ));
        }
        Ok(())
    }

    fn validate_retained_evidence_v2(
        &self,
        completeness: CaptureCompletenessV1,
        retained: &DiagnosisRetainedEvidenceV2,
    ) -> Result<(), ProtocolValidationErrorV1> {
        if retained.terminal.sequence != self.sequence
            || retained.terminal.class != self.class
            || retained.transcript.completeness != completeness
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis retained evidence envelope",
            ));
        }
        match (
            retained.terminal.invocation,
            &self.context.workgroup,
            &self.context.workitem,
        ) {
            (
                Some(invocation),
                DiagnosisFactV2::Observed { value: workgroup },
                DiagnosisFactV2::Observed { value: workitem },
            ) => {
                let DiagnosisFactV2::Declared { value: dispatch } = &self.context.dispatch else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                if invocation.workgroup != *workgroup
                    || invocation.global != workitem.global
                    || invocation.local != workitem.local
                    || invocation.workgroup_size != dispatch.workgroup_size
                    || invocation.launch_extent != dispatch.launch_extent
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained terminal invocation",
                    ));
                }
            }
            (
                None,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
            ) => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        match (retained.terminal.site, &self.site) {
            (Some(record), DiagnosisFactV2::Observed { value }) if record == *value => {}
            (
                None,
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
                },
            ) => {}
            _ => {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "diagnosis retained terminal site",
                ));
            }
        }
        match (
            &retained.terminal.payload,
            &self.memory_region,
            &self.barrier,
        ) {
            (
                DiagnosisTerminalPayloadRecordV2::MemoryOutOfBounds {
                    allocation,
                    requested_offset,
                    requested_bytes,
                    allocation_bytes,
                    abi_view,
                },
                DiagnosisFactV2::Observed { value: memory },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
            ) => {
                let DiagnosisFactV2::Declared { value: argument } = &memory.abi_argument else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                let DiagnosisFactV2::Declared { value: contract } = &memory.allocation_contract
                else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                let Some(view) = abi_view else {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained terminal memory",
                    ));
                };
                if *allocation != memory.allocation
                    || *requested_offset != memory.requested_offset
                    || *requested_bytes != memory.requested_bytes
                    || *allocation_bytes != memory.allocation_bytes
                    || view.allocation_contract != *contract
                    || view.abi_argument != *argument
                    || view.legal_offset != memory.legal_offset
                    || view.legal_bytes != memory.legal_bytes
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained terminal memory",
                    ));
                }
                if retained.transcript.barrier.is_some() {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                }
            }
            (
                DiagnosisTerminalPayloadRecordV2::MemoryOutOfBounds { .. },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
            ) if retained.transcript.barrier.is_none() => {}
            (
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierDivergence {
                    phase,
                    waiting_representative,
                    exited_representative,
                    waiting: Some(waiting),
                    exited: Some(exited),
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Observed {
                    value:
                        DiagnosisBarrierV2::Divergence {
                            phase: DiagnosisFactV2::Observed { value: claim_phase },
                            observed_arrivals,
                            arrived_participants,
                            waiting_participants,
                            exited_participants,
                            ..
                        },
                },
            ) => {
                let waiting_claim = observed_participant_set(waiting_participants)?;
                let exited_claim = observed_participant_set(exited_participants)?;
                if *phase != *claim_phase
                    || participant_locals(waiting_claim)? != *waiting
                    || participant_locals(exited_claim)? != *exited
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained terminal divergence",
                    ));
                }
                let Some(invocation) = retained.terminal.invocation else {
                    return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                };
                if invocation.local != *waiting_representative
                    || waiting.binary_search(waiting_representative).is_err()
                    || exited.binary_search(exited_representative).is_err()
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained divergence representative",
                    ));
                }
                let transcript = retained
                    .transcript
                    .barrier
                    .as_ref()
                    .ok_or(ProtocolValidationErrorV1::InvalidTruthClassification)?;
                if transcript.phase != *phase || transcript.workgroup != invocation.workgroup {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained barrier transcript",
                    ));
                }
                match (completeness, observed_arrivals, arrived_participants) {
                    (
                        CaptureCompletenessV1::Complete,
                        DiagnosisFactV2::Observed { value: arrivals },
                        DiagnosisFactV2::Observed { value: arrived },
                    ) => {
                        let count = u32::try_from(transcript.arrivals.len()).map_err(|_| {
                            ProtocolValidationErrorV1::CountOutOfRange(
                                "diagnosis transcript arrivals",
                            )
                        })?;
                        let mut transcript_locals = Vec::new();
                        transcript_locals
                            .try_reserve_exact(transcript.arrivals.len())
                            .map_err(|_| {
                                ProtocolValidationErrorV1::CountOutOfRange(
                                    "diagnosis transcript arrivals",
                                )
                            })?;
                        transcript_locals
                            .extend(transcript.arrivals.iter().map(|arrival| arrival.local));
                        transcript_locals.sort_unstable();
                        if *arrivals != count || participant_locals(arrived)? != transcript_locals {
                            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                                "diagnosis transcript arrival claims",
                            ));
                        }
                    }
                    (
                        CaptureCompletenessV1::Truncated { .. },
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::TranscriptTruncated,
                        },
                        DiagnosisFactV2::Observed { .. },
                    ) => {}
                    _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
                }
            }
            (
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierDivergence { .. },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                },
            ) => {}
            (
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierMismatch {
                    phase,
                    mismatch,
                    expected_site,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Observed {
                    value:
                        DiagnosisBarrierV2::Mismatch {
                            phase: DiagnosisFactV2::Observed { value: claim_phase },
                            mismatch:
                                DiagnosisFactV2::Observed {
                                    value: claim_mismatch,
                                },
                            expected_site: claim_expected_site,
                            ..
                        },
                },
            ) => {
                let claim_expected_site = match claim_expected_site {
                    DiagnosisFactV2::Observed { value } => Some(*value),
                    DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
                    } => None,
                    _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
                };
                if *phase != *claim_phase
                    || *mismatch != *claim_mismatch
                    || *expected_site != claim_expected_site
                    || retained.transcript.barrier.is_some()
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "diagnosis retained terminal mismatch",
                    ));
                }
            }
            (
                DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierMismatch { .. },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotApplicable,
                },
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                },
            ) if retained.transcript.barrier.is_none() => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        Ok(())
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
            expected_participant_set,
            arrived_participants,
            waiting_participants,
            exited_participants,
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
        let expected_set = self.validate_expected_participant_set(expected_participant_set)?;
        let arrived = observed_participant_set(arrived_participants)?;
        let waiting = observed_participant_set(waiting_participants)?;
        let exited = observed_participant_set(exited_participants)?;
        validate_participant_set(arrived, *dispatch, *workgroup, wave.width, false)?;
        validate_participant_set(waiting, *dispatch, *workgroup, wave.width, false)?;
        validate_participant_set(exited, *dispatch, *workgroup, wave.width, false)?;
        if arrivals.is_some_and(|arrivals| u32::try_from(arrived.len()) != Ok(arrivals))
            || arrived != waiting
            || waiting.is_empty()
            || exited.is_empty()
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier observed participant sets",
            ));
        }
        let waiting_locals = participant_locals(waiting)?;
        let exited_locals = participant_locals(exited)?;
        if waiting_locals
            .iter()
            .any(|local| exited_locals.binary_search(local).is_ok())
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier participants",
            ));
        }
        let mut combined = waiting_locals;
        combined.extend(exited_locals);
        combined.sort_unstable();
        if combined != participant_locals(expected_set)? {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis barrier participant partition",
            ));
        }
        let first_waiting = waiting
            .first()
            .ok_or(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis barrier waiting participants",
            ))?;
        let DiagnosisFactV2::Observed {
            value: waiting_local,
        } = &first_waiting.local_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred {
            value: waiting_global,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = &first_waiting.global_workitem
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
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

    fn validate_expected_participant_set<'a>(
        &self,
        expected_participant_set: &'a DiagnosisFactV2<Vec<DiagnosisBarrierParticipantV2>>,
    ) -> Result<&'a [DiagnosisBarrierParticipantV2], ProtocolValidationErrorV1> {
        let DiagnosisFactV2::Inferred {
            value: participants,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
        } = expected_participant_set
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Declared { value: dispatch } = &self.context.dispatch else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Observed { value: workgroup } = &self.context.workgroup else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let DiagnosisFactV2::Inferred { value: wave, .. } = &self.context.wave else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        validate_participant_set(participants, *dispatch, *workgroup, wave.width, true)?;
        let expected = expected_participant_locals(*dispatch, *workgroup)?;
        if participant_locals(participants)? != expected {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis expected participant set",
            ));
        }
        Ok(participants)
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

    /// Validates one diagnosis against evidence identities independently retained by
    /// the capture owner.
    ///
    /// Ordinary decoding validates canonical content integrity only. This additional
    /// admission step binds the self-contained response to an exact deterministic
    /// simulator result (and Bundle envelope/subject when one was supplied).
    pub fn validate_against_capture_v2(
        &self,
        limits: ProtocolLimitsV1,
        expected: DiagnosisCaptureBindingV2,
    ) -> Result<(), ProtocolValidationErrorV1> {
        self.validate(limits)?;
        let Self::Ok {
            schema,
            request_id,
            operation,
            session,
            completeness,
            diagnoses,
            next_cursor,
            ..
        } = self
        else {
            return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
        };
        let [diagnosis] = diagnoses.as_slice() else {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "diagnosis capture admission",
            ));
        };
        let retained = diagnosis
            .evidence
            .retained
            .as_ref()
            .ok_or(ProtocolValidationErrorV1::InvalidTruthClassification)?;
        let response = DiagnosisResponseEnvelopeBindingV2 {
            schema: *schema,
            request_id: *request_id,
            operation: *operation,
            next_cursor: *next_cursor,
        };
        if retained.capture_binding_v2(&diagnosis.input, *session, *completeness, response)?
            != expected
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "diagnosis capture binding",
            ));
        }
        Ok(())
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
