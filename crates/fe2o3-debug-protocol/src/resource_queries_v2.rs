//! New storage-generation resource queries. Legacy ResourceV1 still means
//! generation zero / lifetime not represented; no implicit conversion exists.
use crate::runtime_observations_v1::{observed_optional_non_null, validate_observed_session};
use crate::{
    AddressSpaceV1, CaptureCompletenessV1, DebugErrorV1, MAX_RESOURCE_QUERY_ITEMS_V1,
    MAX_RESOURCE_QUERY_SCANS_V1, ProtocolLimitsV1, ProtocolValidationErrorV1,
    ResourceAccessOccurrenceV1, ResourceAllocationAccessV1, ResourceDecimalU64V1,
    ResourceMemoryAccessKindV1, ResourceMemoryRangeV1, ResourcePageTokenV1, ResourceQueryPageV1,
    RuntimeDecimalU64V1, RuntimeInvocationV1, RuntimeObservationBindingV1,
    RuntimeObservationUnavailableV1, RuntimeOperationObservationV1, SessionViewV1,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RESOURCE_REQUEST_SCHEMA_V2: &str = "fe2o3-debug-resource-request-v2";
pub const RESOURCE_RESPONSE_SCHEMA_V2: &str = "fe2o3-debug-resource-response-v2";
pub const MAX_RESOURCE_MEMORY_READ_BYTES_V2: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResourceRequestSchemaV2 {
    #[serde(rename = "fe2o3-debug-resource-request-v2")]
    V2,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResourceResponseSchemaV2 {
    #[serde(rename = "fe2o3-debug-resource-response-v2")]
    V2,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceStorageIdentityV2 {
    pub allocation: ResourceDecimalU64V1,
    pub storage_slot: ResourceDecimalU64V1,
    pub generation: ResourceDecimalU64V1,
}
impl ResourceStorageIdentityV2 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.allocation.get() == 0 || self.storage_slot.get() == 0 || self.generation.get() == 0
        {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceCreationSiteV2 {
    pub function_ordinal: RuntimeDecimalU64V1,
    pub block: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "observed_optional_non_null"
    )]
    pub operation: Option<u32>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceAllocationScopeV2 {
    Dispatch,
    Workgroup {
        coordinate: [RuntimeDecimalU64V1; 3],
        size: [u32; 3],
        count: [RuntimeDecimalU64V1; 3],
        launch: [RuntimeDecimalU64V1; 3],
    },
    Invocation {
        invocation: RuntimeInvocationV1,
    },
}
impl ResourceAllocationScopeV2 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Dispatch => Ok(()),
            Self::Invocation { invocation } => invocation.validate(),
            Self::Workgroup {
                coordinate,
                size,
                count,
                launch,
            } => {
                for axis in 0..3 {
                    let extent = launch[axis].get();
                    let width = u64::from(size[axis]);
                    if extent == 0
                        || width == 0
                        || count[axis].get() != extent.div_ceil(width)
                        || coordinate[axis].get() >= count[axis].get()
                    {
                        return Err(ProtocolValidationErrorV1::InvalidRange(
                            "allocation workgroup",
                        ));
                    }
                }
                Ok(())
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAllocationDescriptorV2 {
    pub identity: ResourceStorageIdentityV2,
    pub address_space: AddressSpaceV1,
    pub access: ResourceAllocationAccessV1,
    pub alignment: u32,
    pub byte_len: ResourceDecimalU64V1,
    pub owning_scope: ResourceAllocationScopeV2,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "observed_optional_non_null"
    )]
    pub creation_site: Option<ResourceCreationSiteV2>,
}
impl ResourceAllocationDescriptorV2 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.identity.validate()?;
        self.owning_scope.validate()?;
        if !self.alignment.is_power_of_two() {
            return Err(ProtocolValidationErrorV1::InvalidRange(
                "allocation alignment",
            ));
        }
        let valid_scope = matches!(
            (self.address_space, self.owning_scope),
            (
                AddressSpaceV1::Global | AddressSpaceV1::Constant,
                ResourceAllocationScopeV2::Dispatch
            ) | (
                AddressSpaceV1::Workgroup,
                ResourceAllocationScopeV2::Workgroup { .. }
            ) | (
                AddressSpaceV1::Private,
                ResourceAllocationScopeV2::Invocation { .. }
            )
        );
        if !valid_scope {
            return Err(ProtocolValidationErrorV1::InvalidAvailability);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAllocationV2 {
    pub descriptor: ResourceAllocationDescriptorV2,
    pub snapshot_bytes_available: bool,
    pub initialization_available: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "transition", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceAllocationTransitionKindV2 {
    Preexisting,
    Create {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "observed_optional_non_null"
        )]
        previous_allocation: Option<ResourceDecimalU64V1>,
    },
    Release,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceAllocationTransitionV2 {
    pub sequence: ResourceDecimalU64V1,
    pub descriptor: ResourceAllocationDescriptorV2,
    pub kind: ResourceAllocationTransitionKindV2,
}
impl ResourceAllocationTransitionV2 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.descriptor.validate()?;
        if self.sequence.get() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        match self.kind {
            ResourceAllocationTransitionKindV2::Preexisting => {
                if self.descriptor.creation_site.is_some()
                    || self.descriptor.owning_scope != ResourceAllocationScopeV2::Dispatch
                    || self.descriptor.identity.generation.get() != 1
                {
                    return Err(ProtocolValidationErrorV1::InvalidAvailability);
                }
            }
            ResourceAllocationTransitionKindV2::Create {
                previous_allocation,
            } => {
                if self.descriptor.owning_scope == ResourceAllocationScopeV2::Dispatch
                    || self.descriptor.creation_site.is_none()
                {
                    return Err(ProtocolValidationErrorV1::InvalidAvailability);
                }
                let identity = self.descriptor.identity;
                if let Some(previous) = previous_allocation {
                    if previous.get() == 0
                        || previous.get() >= identity.allocation.get()
                        || identity.generation.get() <= 1
                    {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "allocation predecessor",
                        ));
                    }
                } else if identity.generation.get() != 1 {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "fresh storage generation",
                    ));
                }
            }
            ResourceAllocationTransitionKindV2::Release => {}
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceOperationV2 {
    QueryAllocations,
    QueryAllocationLifecycle,
    QueryMemoryAccesses,
    ReadAllocationMemory,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceRequestV2 {
    QueryAllocations {
        schema: ResourceRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        expected_binding: RuntimeObservationBindingV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "observed_optional_non_null"
        )]
        address_space: Option<AddressSpaceV1>,
        page: ResourceQueryPageV1,
    },
    QueryAllocationLifecycle {
        schema: ResourceRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        expected_binding: RuntimeObservationBindingV1,
        page: ResourceQueryPageV1,
    },
    QueryMemoryAccesses {
        schema: ResourceRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        expected_binding: RuntimeObservationBindingV1,
        allocation: ResourceStorageIdentityV2,
        page: ResourceQueryPageV1,
    },
    ReadAllocationMemory {
        schema: ResourceRequestSchemaV2,
        request_id: u64,
        expected_revision: u64,
        expected_binding: RuntimeObservationBindingV1,
        allocation: ResourceStorageIdentityV2,
        range: ResourceMemoryRangeV1,
    },
}
impl ResourceRequestV2 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::QueryAllocations { request_id, .. }
            | Self::QueryAllocationLifecycle { request_id, .. }
            | Self::QueryMemoryAccesses { request_id, .. }
            | Self::ReadAllocationMemory { request_id, .. } => *request_id,
        }
    }
    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::QueryAllocations {
                expected_revision, ..
            }
            | Self::QueryAllocationLifecycle {
                expected_revision, ..
            }
            | Self::QueryMemoryAccesses {
                expected_revision, ..
            }
            | Self::ReadAllocationMemory {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub const fn expected_binding(&self) -> RuntimeObservationBindingV1 {
        match self {
            Self::QueryAllocations {
                expected_binding, ..
            }
            | Self::QueryAllocationLifecycle {
                expected_binding, ..
            }
            | Self::QueryMemoryAccesses {
                expected_binding, ..
            }
            | Self::ReadAllocationMemory {
                expected_binding, ..
            } => *expected_binding,
        }
    }
    pub const fn operation(&self) -> ResourceOperationV2 {
        match self {
            Self::QueryAllocations { .. } => ResourceOperationV2::QueryAllocations,
            Self::QueryAllocationLifecycle { .. } => ResourceOperationV2::QueryAllocationLifecycle,
            Self::QueryMemoryAccesses { .. } => ResourceOperationV2::QueryMemoryAccesses,
            Self::ReadAllocationMemory { .. } => ResourceOperationV2::ReadAllocationMemory,
        }
    }
    pub fn page(&self) -> Option<&ResourceQueryPageV1> {
        match self {
            Self::QueryAllocations { page, .. }
            | Self::QueryAllocationLifecycle { page, .. }
            | Self::QueryMemoryAccesses { page, .. } => Some(page),
            Self::ReadAllocationMemory { .. } => None,
        }
    }
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        self.expected_binding().validate()?;
        if self.expected_binding().cursor.event_sequence == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if self.expected_revision() != self.expected_binding().cursor.state_revision {
            return Err(ProtocolValidationErrorV1::RevisionMismatch);
        }
        if let Some(page) = self.page() {
            page.validate(limits)?;
        }
        if let Self::QueryMemoryAccesses { allocation, .. }
        | Self::ReadAllocationMemory { allocation, .. } = self
        {
            allocation.validate()?;
        }
        if let Self::ReadAllocationMemory { range, .. } = self {
            range.end()?;
            if range.byte_len.get() > MAX_RESOURCE_MEMORY_READ_BYTES_V2 {
                return Err(ProtocolValidationErrorV1::CountOutOfRange(
                    "observed memory read",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePageInfoV2 {
    pub source_count: ResourceDecimalU64V1,
    /// Zero-based examined-source start; informational, not continuation authority.
    pub source_start: ResourceDecimalU64V1,
    pub scanned: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "observed_optional_non_null"
    )]
    pub next_token: Option<ResourcePageTokenV1>,
}
impl ResourcePageInfoV2 {
    fn validate(
        &self,
        count: usize,
        limits: ProtocolLimitsV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        let end = self
            .source_start
            .get()
            .checked_add(u64::from(self.scanned))
            .ok_or(ProtocolValidationErrorV1::RangeOverflow("observed page"))?;
        if count > usize::from(MAX_RESOURCE_QUERY_ITEMS_V1)
            || count > limits.max_response_items
            || count > usize::from(self.scanned)
            || self.scanned > MAX_RESOURCE_QUERY_SCANS_V1
            || end > self.source_count.get()
            || (self.scanned == 0 && end != self.source_count.get())
            || (self.next_token.is_some() && end >= self.source_count.get())
            || (end < self.source_count.get() && self.next_token.is_none())
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange("observed page"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceMemoryAccessV2 {
    pub occurrence: ResourceAccessOccurrenceV1,
    pub invocation: RuntimeInvocationV1,
    pub allocation: ResourceStorageIdentityV2,
    pub range: ResourceMemoryRangeV1,
    pub address_space: AddressSpaceV1,
    pub access: ResourceMemoryAccessKindV1,
    pub origin: RuntimeOperationObservationV1,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceMemoryReadV2 {
    pub allocation: ResourceStorageIdentityV2,
    pub range: ResourceMemoryRangeV1,
    pub address_space: AddressSpaceV1,
    /// Exact lowercase 0x-prefixed bytes; no padding or partial success.
    pub bytes: String,
    /// Packed little-bit-order initialization mask, as in the legacy wire.
    pub initialized: String,
}
impl ResourceMemoryReadV2 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        self.allocation.validate()?;
        self.range.end()?;
        let len = self.range.byte_len.get();
        if len > MAX_RESOURCE_MEMORY_READ_BYTES_V2 {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "observed memory bytes",
            ));
        }
        validate_hex(&self.bytes, len)?;
        validate_hex(&self.initialized, len.div_ceil(8))?;
        if !len.is_multiple_of(8) {
            let end = self.initialized.len();
            let last = u8::from_str_radix(&self.initialized[end - 2..], 16)
                .map_err(|_| ProtocolValidationErrorV1::InvalidInitializationBits)?;
            if last >> (len % 8) != 0 {
                return Err(ProtocolValidationErrorV1::InvalidInitializationBits);
            }
        }
        Ok(())
    }
}
fn validate_hex(text: &str, bytes: u64) -> Result<(), ProtocolValidationErrorV1> {
    let size = usize::try_from(bytes)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow("observed hex"))?;
    let encoded = text
        .strip_prefix("0x")
        .ok_or(ProtocolValidationErrorV1::InvalidHexBytes)?;
    if encoded.len() != size
        || !encoded
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ProtocolValidationErrorV1::InvalidHexBytes);
    }
    Ok(())
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceQueryResultV2 {
    Allocations {
        allocations: Vec<ResourceAllocationV2>,
    },
    AllocationLifecycle {
        transitions: Vec<ResourceAllocationTransitionV2>,
    },
    MemoryAccesses {
        accesses: Vec<ResourceMemoryAccessV2>,
    },
    AllocationMemory {
        memory: ResourceMemoryReadV2,
    },
}
impl ResourceQueryResultV2 {
    pub const fn operation(&self) -> ResourceOperationV2 {
        match self {
            Self::Allocations { .. } => ResourceOperationV2::QueryAllocations,
            Self::AllocationLifecycle { .. } => ResourceOperationV2::QueryAllocationLifecycle,
            Self::MemoryAccesses { .. } => ResourceOperationV2::QueryMemoryAccesses,
            Self::AllocationMemory { .. } => ResourceOperationV2::ReadAllocationMemory,
        }
    }
    fn item_count(&self) -> usize {
        match self {
            Self::Allocations { allocations } => allocations.len(),
            Self::AllocationLifecycle { transitions } => transitions.len(),
            Self::MemoryAccesses { accesses } => accesses.len(),
            Self::AllocationMemory { .. } => 1,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceResponseV2 {
    Ok {
        schema: ResourceResponseSchemaV2,
        request_id: u64,
        operation: ResourceOperationV2,
        session: SessionViewV1,
        binding: RuntimeObservationBindingV1,
        through_sequence: ResourceDecimalU64V1,
        completeness: CaptureCompletenessV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "observed_optional_non_null"
        )]
        page: Option<ResourcePageInfoV2>,
        result: ResourceQueryResultV2,
    },
    Unavailable {
        schema: ResourceResponseSchemaV2,
        request_id: u64,
        operation: ResourceOperationV2,
        session: SessionViewV1,
        binding: RuntimeObservationBindingV1,
        completeness: CaptureCompletenessV1,
        reason: RuntimeObservationUnavailableV1,
    },
    Error {
        schema: ResourceResponseSchemaV2,
        request_id: u64,
        operation: ResourceOperationV2,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}
impl ResourceResponseV2 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::Ok { request_id, .. }
            | Self::Unavailable { request_id, .. }
            | Self::Error { request_id, .. } => *request_id,
        }
    }
    pub const fn operation(&self) -> ResourceOperationV2 {
        match self {
            Self::Ok { operation, .. }
            | Self::Unavailable { operation, .. }
            | Self::Error { operation, .. } => *operation,
        }
    }
    pub const fn session(&self) -> SessionViewV1 {
        match self {
            Self::Ok { session, .. }
            | Self::Unavailable { session, .. }
            | Self::Error { session, .. } => *session,
        }
    }
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        validate_observed_session(self.session())?;
        if let Self::Ok {
            completeness: CaptureCompletenessV1::Truncated { emitted_events, .. },
            ..
        } = self
            && *emitted_events < self.session().cursor.event_sequence
        {
            return Err(ProtocolValidationErrorV1::InvalidAvailability);
        }
        let (binding, watermark, page, result) = match self {
            Self::Ok {
                binding,
                through_sequence,
                page,
                result,
                ..
            } => (*binding, through_sequence.get(), page, result),
            Self::Unavailable { binding, .. } => {
                binding.validate()?;
                if binding.cursor != self.session().cursor {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "resource unavailable cursor",
                    ));
                }
                return Ok(());
            }
            Self::Error { error, .. } => return error.validate(),
        };
        binding.validate()?;
        if binding.cursor != self.session().cursor || binding.cursor.event_sequence == 0 {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "resource observed cursor",
            ));
        }
        if self.operation() != result.operation() {
            return Err(ProtocolValidationErrorV1::OperationResultMismatch);
        }
        if result.item_count() != 0 && watermark == 0 {
            return Err(ProtocolValidationErrorV1::InvalidAvailability);
        }
        if let ResourceQueryResultV2::AllocationMemory { memory } = result {
            if page.is_some() {
                return Err(ProtocolValidationErrorV1::InvalidAvailability);
            }
            return memory.validate();
        }
        let page = page
            .as_ref()
            .ok_or(ProtocolValidationErrorV1::InvalidAvailability)?;
        page.validate(result.item_count(), limits)?;
        match result {
            ResourceQueryResultV2::Allocations { allocations } => {
                let mut allocations_seen = BTreeSet::new();
                let mut slots_seen = BTreeSet::new();
                for row in allocations {
                    row.descriptor.validate()?;
                    if !allocations_seen.insert(row.descriptor.identity.allocation.get())
                        || !slots_seen.insert(row.descriptor.identity.storage_slot.get())
                    {
                        return Err(ProtocolValidationErrorV1::DuplicateIdentity(
                            "live resource storage",
                        ));
                    }
                    if row.snapshot_bytes_available != row.initialization_available {
                        return Err(ProtocolValidationErrorV1::InvalidAvailability);
                    }
                }
            }
            ResourceQueryResultV2::AllocationLifecycle { transitions } => {
                if page.source_count.get() != watermark
                    || transitions.len() != usize::from(page.scanned)
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "lifecycle literal prefix",
                    ));
                }
                for (index, row) in transitions.iter().enumerate() {
                    row.validate()?;
                    if page
                        .source_start
                        .get()
                        .checked_add(index as u64)
                        .and_then(|n| n.checked_add(1))
                        != Some(row.sequence.get())
                        || row.sequence.get() > watermark
                    {
                        return Err(ProtocolValidationErrorV1::InvalidRange("lifecycle prefix"));
                    }
                }
            }
            ResourceQueryResultV2::MemoryAccesses { accesses } => {
                if page.source_count.get() > binding.cursor.event_sequence {
                    return Err(ProtocolValidationErrorV1::InvalidRange(
                        "observed access prefix",
                    ));
                }
                let mut previous = None;
                for row in accesses {
                    row.allocation.validate()?;
                    row.invocation.validate()?;
                    row.range.end()?;
                    row.origin.validate()?;
                    validate_access_coordinates(row, binding)?;
                    let occurrence = row.occurrence;
                    if occurrence.record_ordinal.checked_add(1) != Some(occurrence.event_sequence)
                        || occurrence.record_ordinal < page.source_start.get()
                        || occurrence.record_ordinal
                            >= page.source_start.get() + u64::from(page.scanned)
                        || previous.is_some_and(|n| n >= occurrence.event_sequence)
                    {
                        return Err(ProtocolValidationErrorV1::InvalidRange(
                            "observed access occurrence",
                        ));
                    }
                    previous = Some(occurrence.event_sequence);
                }
            }
            ResourceQueryResultV2::AllocationMemory { .. } => unreachable!("handled above"),
        }
        Ok(())
    }
    pub fn validate_for_request(
        &self,
        request: &ResourceRequestV2,
        limits: ProtocolLimitsV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        request.validate(limits)?;
        self.validate(limits)?;
        if self.request_id() != request.request_id() || self.operation() != request.operation() {
            return Err(ProtocolValidationErrorV1::OperationResultMismatch);
        }
        let (binding, result, page) = match self {
            Self::Ok {
                binding,
                result,
                page,
                ..
            } => (*binding, Some(result), page.as_ref()),
            Self::Unavailable { binding, .. } => (*binding, None, None),
            Self::Error { .. } => return Ok(()),
        };
        request.expected_binding().matches(binding)?;
        if let Some(page) = page {
            let requested = request
                .page()
                .ok_or(ProtocolValidationErrorV1::OperationResultMismatch)?;
            if page.scanned > requested.max_scanned
                || result
                    .is_some_and(|result| result.item_count() > usize::from(requested.max_items))
                || (page.next_token.is_some() && page.next_token == requested.token)
                || (requested.token.is_none() && page.source_start.get() != 0)
            {
                return Err(ProtocolValidationErrorV1::CountOutOfRange(
                    "requested resource page",
                ));
            }
        }
        match (request, result) {
            (
                SelfRequest::QueryAllocations { address_space, .. },
                Some(ResourceQueryResultV2::Allocations { allocations }),
            ) if allocations
                .iter()
                .any(|row| address_space.is_some_and(|a| a != row.descriptor.address_space)) =>
            {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "resource address space",
                ));
            }
            (
                SelfRequest::QueryMemoryAccesses { allocation, .. },
                Some(ResourceQueryResultV2::MemoryAccesses { accesses }),
            ) if accesses.iter().any(|row| row.allocation != *allocation) => {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "resource access identity",
                ));
            }
            (
                SelfRequest::ReadAllocationMemory {
                    allocation, range, ..
                },
                Some(ResourceQueryResultV2::AllocationMemory { memory }),
            ) if memory.allocation != *allocation || memory.range != *range => {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "resource memory selection",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}
use ResourceRequestV2 as SelfRequest;

fn validate_access_coordinates(
    row: &ResourceMemoryAccessV2,
    binding: RuntimeObservationBindingV1,
) -> Result<(), ProtocolValidationErrorV1> {
    use crate::{DebugSnapshotAnchorV1, ExecutionScopeV1, KirSitePointV1, WaveInterpretationV1};
    DebugSnapshotAnchorV1 {
        cursor: binding.cursor,
        scope: row.occurrence.scope,
        site: None,
        frame: None,
        occurrence: None,
    }
    .validate()?;
    let ExecutionScopeV1::Lane {
        workgroup,
        wave,
        lane,
        logical_workitem,
        wave_width,
        interpretation: WaveInterpretationV1::LogicalVisualization,
        ..
    } = row.occurrence.scope
    else {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    };
    for axis in 0..3 {
        if u64::from(workgroup[axis]) != row.invocation.workgroup[axis].get()
            || logical_workitem[axis] != row.invocation.global[axis].get()
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "access invocation scope",
            ));
        }
    }
    let [x, y, z] = row.invocation.local.map(u64::from);
    let [sx, sy, _] = row.invocation.workgroup_size.map(u64::from);
    let flat = z
        .checked_mul(sy)
        .and_then(|v| v.checked_add(y))
        .and_then(|v| v.checked_mul(sx))
        .and_then(|v| v.checked_add(x))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow("access lane"))?;
    if flat / u64::from(wave_width) != u64::from(wave)
        || flat % u64::from(wave_width) != u64::from(lane)
    {
        return Err(ProtocolValidationErrorV1::IdentityMismatch(
            "access invocation lane",
        ));
    }
    let KirSitePointV1::Operation { operation_ordinal } = row.occurrence.site.point else {
        return Err(ProtocolValidationErrorV1::InvalidAvailability);
    };
    if let RuntimeOperationObservationV1::Available { identity } = row.origin
        && (identity.site.function_ordinal.get() != row.occurrence.site.function_ordinal
            || u64::from(identity.site.operation) != operation_ordinal)
    {
        return Err(ProtocolValidationErrorV1::IdentityMismatch(
            "access origin site",
        ));
    }
    Ok(())
}
