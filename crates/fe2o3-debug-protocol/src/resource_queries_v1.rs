//! Separately versioned read-only CPU resource projections, not a trace format.
//!
//! A required expected snapshot is a comparison value, never admission evidence.
//! The backend must compare it with its independently produced full anchor before
//! binding the immutable projection to its own session/revision. Pagination tokens
//! name bounded backend-local cursor entries; their text carries no authority.
//! New byte extents use canonical decimal strings so JavaScript loses no bits.

use std::collections::BTreeSet;
use std::io::{self, Write};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    AddressSpaceV1, AllocationIdentityV1, CaptureCompletenessV1, DebugBackendV1, DebugErrorV1,
    DebugSnapshotAnchorV1, ExecutionKindV1, ExecutionScopeSelectorV1, ExecutionScopeV1,
    KirSitePointV1, KirSiteV1, ProtocolCodecErrorV1, ProtocolLimitsV1, ProtocolValidationErrorV1,
    SessionViewV1, WaveInterpretationV1,
};

pub const RESOURCE_REQUEST_SCHEMA_V1: &str = "fe2o3-debug-resource-request-v1";
pub const RESOURCE_RESPONSE_SCHEMA_V1: &str = "fe2o3-debug-resource-response-v1";
pub const MAX_RESOURCE_QUERY_ITEMS_V1: u16 = 256;
pub const MAX_RESOURCE_QUERY_SCANS_V1: u16 = 256;
pub const MAX_RESOURCE_PAGE_TOKEN_BYTES_V1: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResourceRequestSchemaV1 {
    #[serde(rename = "fe2o3-debug-resource-request-v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResourceResponseSchemaV1 {
    #[serde(rename = "fe2o3-debug-resource-response-v1")]
    V1,
}

/// Lossless unsigned byte extent, always encoded as a canonical decimal string.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceDecimalU64V1(u64);

impl ResourceDecimalU64V1 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Serialize for ResourceDecimalU64V1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ResourceDecimalU64V1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || value.len() > 20
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(serde::de::Error::custom(
                "noncanonical unsigned byte extent",
            ));
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Opaque lookup key only; never decode this into trusted cursor state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ResourcePageTokenV1(String);

impl ResourcePageTokenV1 {
    pub fn new(value: String) -> Result<Self, ProtocolValidationErrorV1> {
        if value.is_empty()
            || value.len() > MAX_RESOURCE_PAGE_TOKEN_BYTES_V1
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(ProtocolValidationErrorV1::InvalidText(
                "resource page token",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ResourcePageTokenV1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

fn optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceQueryPageV1 {
    pub max_items: u16,
    pub max_scanned: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub token: Option<ResourcePageTokenV1>,
}

impl ResourceQueryPageV1 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        if self.max_items == 0
            || self.max_items > MAX_RESOURCE_QUERY_ITEMS_V1
            || usize::from(self.max_items) > limits.max_response_items
            || self.max_scanned == 0
            || self.max_scanned > MAX_RESOURCE_QUERY_SCANS_V1
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange("resource page"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceOperationV1 {
    QueryAllocations,
    QueryMemoryAccesses,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceMemoryAccessKindV1 {
    Read,
    WriteCommitted,
    AtomicRead,
    AtomicWriteCommitted,
    AtomicReadWriteCommitted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceMemoryRangeV1 {
    pub byte_offset: ResourceDecimalU64V1,
    pub byte_len: ResourceDecimalU64V1,
}

impl ResourceMemoryRangeV1 {
    pub fn end(self) -> Result<u64, ProtocolValidationErrorV1> {
        if self.byte_len.get() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroCount("resource byte length"));
        }
        self.byte_offset
            .get()
            .checked_add(self.byte_len.get())
            .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                "resource byte range",
            ))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceMemoryAccessFilterV1 {
    pub scope: ExecutionScopeSelectorV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub allocation: Option<AllocationIdentityV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub address_space: Option<AddressSpaceV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub access: Option<ResourceMemoryAccessKindV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub range: Option<ResourceMemoryRangeV1>,
}

impl ResourceMemoryAccessFilterV1 {
    fn validate(&self, snapshot: DebugSnapshotAnchorV1) -> Result<(), ProtocolValidationErrorV1> {
        self.scope.validate()?;
        if let Some(allocation) = self.allocation {
            validate_allocation(allocation)?;
        }
        if let Some(range) = self.range {
            range.end()?;
        }
        if let (
            ExecutionScopeSelectorV1::Lane { lane, .. },
            ExecutionScopeV1::Lane { wave_width, .. },
        ) = (self.scope, snapshot.scope)
            && lane >= wave_width
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "resource filter lane",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceRequestV1 {
    QueryAllocations {
        schema: ResourceRequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        expected_snapshot: Box<DebugSnapshotAnchorV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional_non_null"
        )]
        address_space: Option<AddressSpaceV1>,
        page: ResourceQueryPageV1,
    },
    QueryMemoryAccesses {
        schema: ResourceRequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        expected_snapshot: Box<DebugSnapshotAnchorV1>,
        filter: ResourceMemoryAccessFilterV1,
        page: ResourceQueryPageV1,
    },
}

impl ResourceRequestV1 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::QueryAllocations { request_id, .. }
            | Self::QueryMemoryAccesses { request_id, .. } => *request_id,
        }
    }
    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::QueryAllocations {
                expected_revision, ..
            }
            | Self::QueryMemoryAccesses {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub const fn operation(&self) -> ResourceOperationV1 {
        match self {
            Self::QueryAllocations { .. } => ResourceOperationV1::QueryAllocations,
            Self::QueryMemoryAccesses { .. } => ResourceOperationV1::QueryMemoryAccesses,
        }
    }
    pub fn expected_snapshot(&self) -> &DebugSnapshotAnchorV1 {
        match self {
            Self::QueryAllocations {
                expected_snapshot, ..
            }
            | Self::QueryMemoryAccesses {
                expected_snapshot, ..
            } => expected_snapshot,
        }
    }
    pub const fn page(&self) -> &ResourceQueryPageV1 {
        match self {
            Self::QueryAllocations { page, .. } | Self::QueryMemoryAccesses { page, .. } => page,
        }
    }
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        validate_snapshot(*self.expected_snapshot())?;
        if self.expected_revision() != self.expected_snapshot().cursor.state_revision {
            return Err(ProtocolValidationErrorV1::RevisionMismatch);
        }
        self.page().validate(limits)?;
        if let Self::QueryMemoryAccesses { filter, .. } = self {
            filter.validate(*self.expected_snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFactUnavailableV1 {
    NotRepresented,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAllocationAccessV1 {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAllocationV1 {
    pub allocation: AllocationIdentityV1,
    pub address_space: AddressSpaceV1,
    pub access: ResourceAllocationAccessV1,
    pub alignment: u32,
    pub capacity_bytes: ResourceDecimalU64V1,
    pub snapshot_bytes_available: bool,
    pub initialization_available: bool,
    pub owning_scope: ResourceFactUnavailableV1,
    pub lifetime: ResourceFactUnavailableV1,
    pub physical_base: ResourceFactUnavailableV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAccessScheduleV1 {
    pub identity: ResourceScheduleIdentityV1,
    pub decision_ordinal: u64,
}

/// Exact simulator ordering profile; not a digest or hardware schedule.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceScheduleIdentityV1 {
    WorkgroupMajorLocalZyxSerialV1,
    WorkgroupMajorLocalZyxCooperativeV1,
    WorkgroupMajorSeededRunnableCooperativeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAccessOccurrenceV1 {
    pub record_ordinal: u64,
    pub event_sequence: u64,
    pub scope: ExecutionScopeV1,
    pub site: KirSiteV1,
    pub schedule: ResourceAccessScheduleV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceMemoryAccessV1 {
    pub occurrence: ResourceAccessOccurrenceV1,
    pub allocation: AllocationIdentityV1,
    pub range: ResourceMemoryRangeV1,
    pub address_space: AddressSpaceV1,
    pub access: ResourceMemoryAccessKindV1,
    pub call_frame: ResourceFactUnavailableV1,
    pub operation_occurrence: ResourceFactUnavailableV1,
    pub source_association: ResourceFactUnavailableV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePageInfoV1 {
    /// Raw entries in the selected source, not the number matching the filter.
    pub source_count: u64,
    pub scanned: u16,
    pub completeness: CaptureCompletenessV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_null"
    )]
    pub next_token: Option<ResourcePageTokenV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceQueryResultV1 {
    Allocations {
        allocations: Vec<ResourceAllocationV1>,
    },
    MemoryAccesses {
        accesses: Vec<ResourceMemoryAccessV1>,
    },
}

impl ResourceQueryResultV1 {
    pub fn item_count(&self) -> usize {
        match self {
            Self::Allocations { allocations } => allocations.len(),
            Self::MemoryAccesses { accesses } => accesses.len(),
        }
    }
    pub const fn operation(&self) -> ResourceOperationV1 {
        match self {
            Self::Allocations { .. } => ResourceOperationV1::QueryAllocations,
            Self::MemoryAccesses { .. } => ResourceOperationV1::QueryMemoryAccesses,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceQueryUnavailableReasonV1 {
    NoSelectedRecord,
    NotCheckpoint,
    FrameLimit,
    ValueLimit,
    AllocationLimit,
    MemoryByteLimit,
    AllocationFailure,
    NotCaptured,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceResponseV1 {
    Ok {
        schema: ResourceResponseSchemaV1,
        request_id: u64,
        operation: ResourceOperationV1,
        session: SessionViewV1,
        snapshot: Box<DebugSnapshotAnchorV1>,
        page: ResourcePageInfoV1,
        result: ResourceQueryResultV1,
        physical_registers: ResourceFactUnavailableV1,
    },
    Unavailable {
        schema: ResourceResponseSchemaV1,
        request_id: u64,
        operation: ResourceOperationV1,
        session: SessionViewV1,
        reason: ResourceQueryUnavailableReasonV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional_non_null"
        )]
        required: Option<ResourceDecimalU64V1>,
        completeness: CaptureCompletenessV1,
    },
    Error {
        schema: ResourceResponseSchemaV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional_non_null"
        )]
        request_id: Option<u64>,
        operation: ResourceOperationV1,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}

impl ResourceResponseV1 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        let (request_id, session) = match self {
            Self::Ok {
                request_id,
                session,
                ..
            }
            | Self::Unavailable {
                request_id,
                session,
                ..
            } => (Some(*request_id), *session),
            Self::Error {
                request_id,
                session,
                ..
            } => (*request_id, *session),
        };
        if request_id == Some(0) {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        validate_session(session)?;
        match self {
            Self::Ok {
                operation,
                snapshot,
                page,
                result,
                ..
            } => {
                validate_snapshot(**snapshot)?;
                if snapshot.cursor != session.cursor {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "resource snapshot cursor",
                    ));
                }
                if *operation != result.operation() {
                    return Err(ProtocolValidationErrorV1::OperationResultMismatch);
                }
                let count = result.item_count();
                if count > usize::from(MAX_RESOURCE_QUERY_ITEMS_V1)
                    || count > limits.max_response_items
                    || page.scanned > MAX_RESOURCE_QUERY_SCANS_V1
                    || count > usize::from(page.scanned)
                    || u64::from(page.scanned) > page.source_count
                    || (page.scanned == 0 && page.source_count != 0)
                    || (page.next_token.is_some()
                        && (page.scanned == 0 || page.source_count <= u64::from(page.scanned)))
                {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "resource result page",
                    ));
                }
                match result {
                    ResourceQueryResultV1::Allocations { allocations } => {
                        let mut identities = BTreeSet::new();
                        for row in allocations {
                            validate_allocation(row.allocation)?;
                            if !identities.insert(row.allocation.ordinal) {
                                return Err(ProtocolValidationErrorV1::DuplicateIdentity(
                                    "resource allocation",
                                ));
                            }
                            if !row.alignment.is_power_of_two()
                                || !row.snapshot_bytes_available
                                || !row.initialization_available
                            {
                                return Err(ProtocolValidationErrorV1::InvalidAvailability);
                            }
                        }
                    }
                    ResourceQueryResultV1::MemoryAccesses { accesses } => {
                        if page.source_count > snapshot.cursor.event_sequence {
                            return Err(ProtocolValidationErrorV1::InvalidRange(
                                "resource source prefix",
                            ));
                        }
                        if let CaptureCompletenessV1::Truncated { emitted_events, .. } =
                            page.completeness
                            && emitted_events < page.source_count
                        {
                            return Err(ProtocolValidationErrorV1::InvalidAvailability);
                        }
                        let mut previous = None;
                        for row in accesses {
                            validate_allocation(row.allocation)?;
                            row.range.end()?;
                            let occurrence = row.occurrence;
                            if occurrence.record_ordinal.checked_add(1)
                                != Some(occurrence.event_sequence)
                                || occurrence.event_sequence > page.source_count
                                || previous
                                    .is_some_and(|sequence| sequence >= occurrence.event_sequence)
                            {
                                return Err(ProtocolValidationErrorV1::InvalidRange(
                                    "resource access occurrence",
                                ));
                            }
                            previous = Some(occurrence.event_sequence);
                            validate_snapshot(DebugSnapshotAnchorV1 {
                                scope: occurrence.scope,
                                site: None,
                                frame: None,
                                occurrence: None,
                                ..**snapshot
                            })?;
                            if let (
                                ExecutionScopeV1::Lane {
                                    wave_width: actual, ..
                                },
                                ExecutionScopeV1::Lane {
                                    wave_width: expected,
                                    ..
                                },
                            ) = (occurrence.scope, snapshot.scope)
                                && actual != expected
                            {
                                return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
                            }
                            if !matches!(occurrence.site.point, KirSitePointV1::Operation { .. }) {
                                return Err(ProtocolValidationErrorV1::InvalidAvailability);
                            }
                        }
                    }
                }
                Ok(())
            }
            Self::Unavailable {
                operation,
                reason,
                required,
                ..
            } => {
                let capture_reason = !matches!(
                    reason,
                    ResourceQueryUnavailableReasonV1::NoSelectedRecord
                        | ResourceQueryUnavailableReasonV1::NotCheckpoint
                );
                if capture_reason != required.is_some()
                    || (*operation == ResourceOperationV1::QueryMemoryAccesses
                        && *reason != ResourceQueryUnavailableReasonV1::NoSelectedRecord)
                {
                    return Err(ProtocolValidationErrorV1::InvalidAvailability);
                }
                Ok(())
            }
            Self::Error { error, .. } => error.validate(),
        }
    }

    /// Additional client-side matching. The backend still checks its own anchor.
    pub fn validate_for_request(
        &self,
        request: &ResourceRequestV1,
        limits: ProtocolLimitsV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        request.validate(limits)?;
        self.validate(limits)?;
        let (request_id, operation) = match self {
            Self::Ok {
                request_id,
                operation,
                ..
            }
            | Self::Unavailable {
                request_id,
                operation,
                ..
            } => (Some(*request_id), *operation),
            Self::Error {
                request_id,
                operation,
                ..
            } => (*request_id, *operation),
        };
        if request_id != Some(request.request_id()) || operation != request.operation() {
            return Err(ProtocolValidationErrorV1::OperationResultMismatch);
        }
        if let Self::Unavailable { session, .. } = self
            && session.cursor != request.expected_snapshot().cursor
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "resource unavailable cursor",
            ));
        }
        if let Self::Ok {
            snapshot,
            page,
            result,
            ..
        } = self
        {
            if snapshot.as_ref() != request.expected_snapshot() {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "resource expected snapshot",
                ));
            }
            if result.item_count() > usize::from(request.page().max_items)
                || page.scanned > request.page().max_scanned
                || (page.next_token.is_some() && page.next_token == request.page().token)
            {
                return Err(ProtocolValidationErrorV1::CountOutOfRange(
                    "resource requested page",
                ));
            }
            match (request, result) {
                (
                    ResourceRequestV1::QueryAllocations { address_space, .. },
                    ResourceQueryResultV1::Allocations { allocations },
                ) => {
                    if allocations
                        .iter()
                        .any(|row| address_space.is_some_and(|space| space != row.address_space))
                    {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "resource allocation filter",
                        ));
                    }
                }
                (
                    ResourceRequestV1::QueryMemoryAccesses { filter, .. },
                    ResourceQueryResultV1::MemoryAccesses { accesses },
                ) => {
                    for row in accesses {
                        if !access_matches(filter, row)? {
                            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                                "resource access filter",
                            ));
                        }
                    }
                }
                _ => return Err(ProtocolValidationErrorV1::OperationResultMismatch),
            }
        }
        Ok(())
    }
}

fn validate_allocation(allocation: AllocationIdentityV1) -> Result<(), ProtocolValidationErrorV1> {
    if allocation.ordinal == 0 {
        return Err(ProtocolValidationErrorV1::ZeroIdentity);
    }
    if allocation.generation != 0 {
        return Err(ProtocolValidationErrorV1::InvalidAvailability);
    }
    Ok(())
}

fn validate_snapshot(snapshot: DebugSnapshotAnchorV1) -> Result<(), ProtocolValidationErrorV1> {
    snapshot.validate()?;
    if snapshot.cursor.event_sequence == 0 {
        return Err(ProtocolValidationErrorV1::ZeroIdentity);
    }
    if !matches!(
        snapshot.scope,
        ExecutionScopeV1::Lane {
            interpretation: WaveInterpretationV1::LogicalVisualization,
            ..
        }
    ) {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    }
    Ok(())
}

fn validate_session(session: SessionViewV1) -> Result<(), ProtocolValidationErrorV1> {
    session.validate()?;
    if session.backend != DebugBackendV1::CpuKirSimulator
        || session.execution_kind != ExecutionKindV1::CpuKirSimulation
    {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    }
    Ok(())
}

fn access_matches(
    filter: &ResourceMemoryAccessFilterV1,
    row: &ResourceMemoryAccessV1,
) -> Result<bool, ProtocolValidationErrorV1> {
    let ExecutionScopeV1::Lane {
        workgroup,
        wave,
        lane,
        ..
    } = row.occurrence.scope
    else {
        return Ok(false);
    };
    let scope_matches = match filter.scope {
        ExecutionScopeSelectorV1::Dispatch => true,
        ExecutionScopeSelectorV1::Workgroup {
            workgroup: expected,
        } => expected == workgroup,
        ExecutionScopeSelectorV1::Wave {
            workgroup: expected,
            wave: expected_wave,
        } => expected == workgroup && expected_wave == wave,
        ExecutionScopeSelectorV1::Lane {
            workgroup: expected,
            wave: expected_wave,
            lane: expected_lane,
        } => expected == workgroup && expected_wave == wave && expected_lane == lane,
    };
    Ok(scope_matches
        && filter
            .allocation
            .is_none_or(|allocation| allocation == row.allocation)
        && filter
            .address_space
            .is_none_or(|space| space == row.address_space)
        && filter.access.is_none_or(|access| access == row.access)
        && match filter.range {
            Some(range) => {
                row.range.byte_offset.get() < range.end()?
                    && range.byte_offset.get() < row.range.end()?
            }
            None => true,
        })
}

pub fn decode_resource_request_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<ResourceRequestV1, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_request_line_bytes)?;
    let value: ResourceRequestV1 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    value
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(value)
}

pub fn decode_resource_response_line_v1(
    line: &[u8],
    limits: ProtocolLimitsV1,
) -> Result<ResourceResponseV1, ProtocolCodecErrorV1> {
    limits
        .validate()
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let payload = validate_line(line, limits.max_response_line_bytes)?;
    let value: ResourceResponseV1 =
        serde_json::from_slice(payload).map_err(|_| ProtocolCodecErrorV1::InvalidJson)?;
    value
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    Ok(value)
}

pub fn encode_resource_response_line_v1(
    response: &ResourceResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<Vec<u8>, ProtocolCodecErrorV1> {
    response
        .validate(limits)
        .map_err(ProtocolCodecErrorV1::Validation)?;
    let mut writer = ResourceBoundedWriter {
        bytes: Vec::new(),
        max: limits.max_response_line_bytes - 1,
        error: None,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(writer.error.unwrap_or(ProtocolCodecErrorV1::JsonEncode));
    }
    writer
        .bytes
        .try_reserve_exact(1)
        .map_err(|_| ProtocolCodecErrorV1::AllocationFailure)?;
    writer.bytes.push(b'\n');
    Ok(writer.bytes)
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
    if payload.iter().any(|byte| matches!(byte, b'\n' | b'\r')) {
        return Err(ProtocolCodecErrorV1::EmbeddedLineBreak);
    }
    Ok(payload)
}

struct ResourceBoundedWriter {
    bytes: Vec<u8>,
    max: usize,
    error: Option<ProtocolCodecErrorV1>,
}

impl Write for ResourceBoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(required) = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|required| *required <= self.max)
        else {
            self.error = Some(ProtocolCodecErrorV1::ResponseTooLarge);
            return Err(io::Error::other("resource response size limit"));
        };
        if required > self.bytes.capacity()
            && self
                .bytes
                .try_reserve_exact(required - self.bytes.len())
                .is_err()
        {
            self.error = Some(ProtocolCodecErrorV1::AllocationFailure);
            return Err(io::Error::other("resource response allocation failed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
