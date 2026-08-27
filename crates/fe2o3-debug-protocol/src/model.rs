use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

pub const REQUEST_SCHEMA_V1: &str = "fe2o3-debug-request-v1";
pub const RESPONSE_SCHEMA_V1: &str = "fe2o3-debug-response-v1";

pub const MAX_REQUEST_LINE_BYTES_V1: usize = 1024 * 1024;
pub const DEFAULT_MAX_RESPONSE_LINE_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_RESPONSE_LINE_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_BREAKPOINTS_V1: usize = 4_096;
pub const MAX_WATCHPOINTS_V1: usize = 4_096;
pub const MAX_PREDICATE_NODES_V1: usize = 64;
pub const MAX_PREDICATE_DEPTH_V1: usize = 16;
pub const MAX_TEXT_BYTES_V1: usize = 256;
pub const MAX_VALUE_PATH_COMPONENTS_V1: usize = 32;
pub const MAX_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_STEP_COUNT_V1: u32 = 1_000_000;
pub const DEFAULT_MEMORY_READ_BYTES_V1: u64 = 64 * 1024;
pub const MAX_MEMORY_READ_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_BIT_VECTOR_HEX_DIGITS_V1: usize = 1_024;
pub const MAX_RESPONSE_ITEMS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolLimitsV1 {
    pub max_request_line_bytes: usize,
    pub max_response_line_bytes: usize,
    pub max_breakpoints: usize,
    pub max_watchpoints: usize,
    pub max_response_items: usize,
}

impl ProtocolLimitsV1 {
    pub fn new(
        max_request_line_bytes: usize,
        max_response_line_bytes: usize,
        max_breakpoints: usize,
        max_watchpoints: usize,
        max_response_items: usize,
    ) -> Result<Self, ProtocolValidationErrorV1> {
        let limits = Self {
            max_request_line_bytes,
            max_response_line_bytes,
            max_breakpoints,
            max_watchpoints,
            max_response_items,
        };
        limits.validate()?;
        Ok(limits)
    }

    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.max_request_line_bytes == 0
            || self.max_request_line_bytes > MAX_REQUEST_LINE_BYTES_V1
        {
            return Err(ProtocolValidationErrorV1::LimitOutOfRange(
                "max_request_line_bytes",
            ));
        }
        if self.max_response_line_bytes == 0
            || self.max_response_line_bytes > MAX_RESPONSE_LINE_BYTES_V1
        {
            return Err(ProtocolValidationErrorV1::LimitOutOfRange(
                "max_response_line_bytes",
            ));
        }
        if self.max_breakpoints == 0 || self.max_breakpoints > MAX_BREAKPOINTS_V1 {
            return Err(ProtocolValidationErrorV1::LimitOutOfRange(
                "max_breakpoints",
            ));
        }
        if self.max_watchpoints == 0 || self.max_watchpoints > MAX_WATCHPOINTS_V1 {
            return Err(ProtocolValidationErrorV1::LimitOutOfRange(
                "max_watchpoints",
            ));
        }
        if self.max_response_items == 0 || self.max_response_items > MAX_RESPONSE_ITEMS_V1 {
            return Err(ProtocolValidationErrorV1::LimitOutOfRange(
                "max_response_items",
            ));
        }
        Ok(())
    }
}

impl Default for ProtocolLimitsV1 {
    fn default() -> Self {
        Self {
            max_request_line_bytes: MAX_REQUEST_LINE_BYTES_V1,
            max_response_line_bytes: DEFAULT_MAX_RESPONSE_LINE_BYTES_V1,
            max_breakpoints: MAX_BREAKPOINTS_V1,
            max_watchpoints: MAX_WATCHPOINTS_V1,
            max_response_items: MAX_RESPONSE_ITEMS_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RequestSchemaV1 {
    #[serde(rename = "fe2o3-debug-request-v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResponseSchemaV1 {
    #[serde(rename = "fe2o3-debug-response-v1")]
    V1,
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueIdentityV1([u8; 32]);

impl OpaqueIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, ProtocolValidationErrorV1> {
        if bytes == [0; 32] {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for OpaqueIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueIdentityV1(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

impl Serialize for OpaqueIdentityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&encode_hex(&self.0))
    }
}

impl<'de> Deserialize<'de> for OpaqueIdentityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() != 64
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(serde::de::Error::custom(
                "opaque identity must be exactly 64 lowercase hex digits",
            ));
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]).expect("validated lowercase hex") << 4)
                | hex_nibble(pair[1]).expect("validated lowercase hex");
        }
        Self::new(bytes).map_err(serde::de::Error::custom)
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes.iter().copied() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugRequestV1 {
    DiscoverCapabilities {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
    },
    GetState {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
    },
    SetBreakpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        breakpoints: Vec<BreakpointSpecV1>,
    },
    RemoveBreakpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        breakpoint_ids: Vec<u64>,
    },
    ListBreakpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        page: PageRequestV1,
    },
    SetWatchpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        watchpoints: Vec<WatchpointSpecV1>,
    },
    RemoveWatchpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        watchpoint_ids: Vec<u64>,
    },
    ListWatchpoints {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        page: PageRequestV1,
    },
    Continue {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        max_events: u64,
    },
    Pause {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
    },
    Step {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        direction: StepDirectionV1,
        granularity: StepGranularityV1,
        count: u32,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        focus: Option<ExecutionScopeSelectorV1>,
    },
    Seek {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        cursor: DebugCursorV1,
    },
    InspectScope {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        scope: ExecutionScopeSelectorV1,
        include_children: bool,
        page: PageRequestV1,
    },
    ResolveSource {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        site: KirSiteV1,
    },
    InspectStack {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        scope: ExecutionScopeSelectorV1,
        page: PageRequestV1,
    },
    InspectValues {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        scope: ExecutionScopeSelectorV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        frame: Option<u64>,
        selector: ValueSelectorV1,
        page: PageRequestV1,
    },
    ReadMemory {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        allocation: AllocationIdentityV1,
        byte_offset: u64,
        byte_len: u64,
    },
    QueryEvents {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        filter: EventFilterV1,
        page: PageRequestV1,
    },
    ExportTrace {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        max_bytes: u64,
    },
    Terminate {
        schema: RequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
    },
}

impl DebugRequestV1 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::GetState { request_id, .. }
            | Self::SetBreakpoints { request_id, .. }
            | Self::RemoveBreakpoints { request_id, .. }
            | Self::ListBreakpoints { request_id, .. }
            | Self::SetWatchpoints { request_id, .. }
            | Self::RemoveWatchpoints { request_id, .. }
            | Self::ListWatchpoints { request_id, .. }
            | Self::Continue { request_id, .. }
            | Self::Pause { request_id, .. }
            | Self::Step { request_id, .. }
            | Self::Seek { request_id, .. }
            | Self::InspectScope { request_id, .. }
            | Self::ResolveSource { request_id, .. }
            | Self::InspectStack { request_id, .. }
            | Self::InspectValues { request_id, .. }
            | Self::ReadMemory { request_id, .. }
            | Self::QueryEvents { request_id, .. }
            | Self::ExportTrace { request_id, .. }
            | Self::Terminate { request_id, .. } => *request_id,
        }
    }

    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::GetState {
                expected_revision, ..
            }
            | Self::SetBreakpoints {
                expected_revision, ..
            }
            | Self::RemoveBreakpoints {
                expected_revision, ..
            }
            | Self::ListBreakpoints {
                expected_revision, ..
            }
            | Self::SetWatchpoints {
                expected_revision, ..
            }
            | Self::RemoveWatchpoints {
                expected_revision, ..
            }
            | Self::ListWatchpoints {
                expected_revision, ..
            }
            | Self::Continue {
                expected_revision, ..
            }
            | Self::Pause {
                expected_revision, ..
            }
            | Self::Step {
                expected_revision, ..
            }
            | Self::Seek {
                expected_revision, ..
            }
            | Self::InspectScope {
                expected_revision, ..
            }
            | Self::ResolveSource {
                expected_revision, ..
            }
            | Self::InspectStack {
                expected_revision, ..
            }
            | Self::InspectValues {
                expected_revision, ..
            }
            | Self::ReadMemory {
                expected_revision, ..
            }
            | Self::QueryEvents {
                expected_revision, ..
            }
            | Self::ExportTrace {
                expected_revision, ..
            }
            | Self::Terminate {
                expected_revision, ..
            } => *expected_revision,
        }
    }

    pub const fn operation(&self) -> DebugOperationNameV1 {
        match self {
            Self::DiscoverCapabilities { .. } => DebugOperationNameV1::DiscoverCapabilities,
            Self::GetState { .. } => DebugOperationNameV1::GetState,
            Self::SetBreakpoints { .. } => DebugOperationNameV1::SetBreakpoints,
            Self::RemoveBreakpoints { .. } => DebugOperationNameV1::RemoveBreakpoints,
            Self::ListBreakpoints { .. } => DebugOperationNameV1::ListBreakpoints,
            Self::SetWatchpoints { .. } => DebugOperationNameV1::SetWatchpoints,
            Self::RemoveWatchpoints { .. } => DebugOperationNameV1::RemoveWatchpoints,
            Self::ListWatchpoints { .. } => DebugOperationNameV1::ListWatchpoints,
            Self::Continue { .. } => DebugOperationNameV1::Continue,
            Self::Pause { .. } => DebugOperationNameV1::Pause,
            Self::Step { .. } => DebugOperationNameV1::Step,
            Self::Seek { .. } => DebugOperationNameV1::Seek,
            Self::InspectScope { .. } => DebugOperationNameV1::InspectScope,
            Self::ResolveSource { .. } => DebugOperationNameV1::ResolveSource,
            Self::InspectStack { .. } => DebugOperationNameV1::InspectStack,
            Self::InspectValues { .. } => DebugOperationNameV1::InspectValues,
            Self::ReadMemory { .. } => DebugOperationNameV1::ReadMemory,
            Self::QueryEvents { .. } => DebugOperationNameV1::QueryEvents,
            Self::ExportTrace { .. } => DebugOperationNameV1::ExportTrace,
            Self::Terminate { .. } => DebugOperationNameV1::Terminate,
        }
    }

    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        match self {
            Self::SetBreakpoints { breakpoints, .. } => {
                validate_nonempty_count(breakpoints.len(), limits.max_breakpoints, "breakpoints")?;
                for breakpoint in breakpoints {
                    breakpoint.validate()?;
                }
            }
            Self::RemoveBreakpoints { breakpoint_ids, .. } => {
                validate_ids(breakpoint_ids, limits.max_breakpoints, "breakpoint_ids")?;
            }
            Self::SetWatchpoints { watchpoints, .. } => {
                validate_nonempty_count(watchpoints.len(), limits.max_watchpoints, "watchpoints")?;
                for watchpoint in watchpoints {
                    watchpoint.validate()?;
                }
            }
            Self::RemoveWatchpoints { watchpoint_ids, .. } => {
                validate_ids(watchpoint_ids, limits.max_watchpoints, "watchpoint_ids")?;
            }
            Self::ListBreakpoints { page, .. } | Self::ListWatchpoints { page, .. } => {
                page.validate()?;
            }
            Self::Continue { max_events, .. } if *max_events == 0 => {
                return Err(ProtocolValidationErrorV1::ZeroCount("max_events"));
            }
            Self::Step { count, focus, .. } => {
                if *count == 0 || *count > MAX_STEP_COUNT_V1 {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange("step count"));
                }
                if let Some(focus) = focus {
                    focus.validate()?;
                }
            }
            Self::InspectScope { scope, page, .. } => {
                scope.validate()?;
                page.validate()?;
            }
            Self::InspectStack { scope, page, .. } => {
                scope.validate()?;
                page.validate()?;
            }
            Self::InspectValues {
                scope,
                selector,
                page,
                ..
            } => {
                scope.validate()?;
                selector.validate()?;
                page.validate()?;
            }
            Self::ReadMemory {
                allocation,
                byte_offset,
                byte_len,
                ..
            } => {
                allocation.validate()?;
                if *byte_len == 0 || *byte_len > MAX_MEMORY_READ_BYTES_V1 {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "memory byte_len",
                    ));
                }
                byte_offset
                    .checked_add(*byte_len)
                    .ok_or(ProtocolValidationErrorV1::RangeOverflow("memory read"))?;
            }
            Self::QueryEvents { filter, page, .. } => {
                filter.validate()?;
                page.validate()?;
            }
            Self::ExportTrace { max_bytes, .. }
                if *max_bytes == 0 || *max_bytes > MAX_RESPONSE_LINE_BYTES_V1 as u64 =>
            {
                return Err(ProtocolValidationErrorV1::CountOutOfRange(
                    "trace max_bytes",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_nonempty_count(
    actual: usize,
    max: usize,
    field: &'static str,
) -> Result<(), ProtocolValidationErrorV1> {
    if actual == 0 || actual > max {
        return Err(ProtocolValidationErrorV1::CountOutOfRange(field));
    }
    Ok(())
}

fn validate_ids(
    ids: &[u64],
    max: usize,
    field: &'static str,
) -> Result<(), ProtocolValidationErrorV1> {
    validate_nonempty_count(ids.len(), max, field)?;
    let mut unique = BTreeSet::new();
    for id in ids.iter().copied() {
        if id == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if !unique.insert(id) {
            return Err(ProtocolValidationErrorV1::DuplicateIdentity(field));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugOperationNameV1 {
    DiscoverCapabilities,
    GetState,
    SetBreakpoints,
    RemoveBreakpoints,
    ListBreakpoints,
    SetWatchpoints,
    RemoveWatchpoints,
    ListWatchpoints,
    Continue,
    Pause,
    Step,
    Seek,
    InspectScope,
    ResolveSource,
    InspectStack,
    InspectValues,
    ReadMemory,
    QueryEvents,
    ExportTrace,
    Terminate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepDirectionV1 {
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepGranularityV1 {
    Event,
    Operation,
    Over,
    Out,
    MemoryAccess,
    BarrierPhase,
    Lane,
    Wave,
    Workgroup,
    Source,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "level", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionScopeSelectorV1 {
    Dispatch,
    Workgroup {
        workgroup: [u32; 3],
    },
    Wave {
        workgroup: [u32; 3],
        wave: u32,
    },
    Lane {
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
    },
}

impl ExecutionScopeSelectorV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if let Self::Lane { lane, .. } = self
            && lane >= 64
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange("lane"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "level", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionScopeV1 {
    Dispatch,
    Workgroup {
        workgroup: [u32; 3],
    },
    Wave {
        workgroup: [u32; 3],
        wave: u32,
        active_mask: u64,
        wave_width: u16,
        interpretation: WaveInterpretationV1,
    },
    Lane {
        workgroup: [u32; 3],
        wave: u32,
        lane: u16,
        logical_workitem: [u64; 3],
        active_mask: u64,
        wave_width: u16,
        interpretation: WaveInterpretationV1,
    },
}

impl ExecutionScopeV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        let (lane, mask, width) = match self {
            Self::Wave {
                active_mask,
                wave_width,
                ..
            } => (None, active_mask, wave_width),
            Self::Lane {
                lane,
                active_mask,
                wave_width,
                ..
            } => (Some(lane), active_mask, wave_width),
            Self::Dispatch | Self::Workgroup { .. } => return Ok(()),
        };
        if !matches!(width, 32 | 64) || (width == 32 && mask > u64::from(u32::MAX)) {
            return Err(ProtocolValidationErrorV1::InvalidActiveMask);
        }
        if let Some(lane) = lane
            && (lane >= width || mask & (1_u64 << lane) == 0)
        {
            return Err(ProtocolValidationErrorV1::InvalidActiveMask);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveInterpretationV1 {
    LogicalVisualization,
    HardwareObserved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KirSiteV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub point: KirSitePointV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum KirSitePointV1 {
    BlockEntry,
    Operation { operation_ordinal: u64 },
    Terminator,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocationV1 {
    pub map_identity: OpaqueIdentityV1,
    pub provenance: SourceMapProvenanceV1,
    pub file_identity: OpaqueIdentityV1,
    pub byte_start: u64,
    pub byte_end: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMapProvenanceV1 {
    CallerBound,
    CompilerBundleAuthenticated,
}

impl SourceLocationV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.byte_start >= self.byte_end {
            return Err(ProtocolValidationErrorV1::InvalidRange("source location"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BreakpointSpecV1 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub client_label: Option<String>,
    pub enabled: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub scope: Option<ExecutionScopeSelectorV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub hit_condition: Option<HitConditionV1>,
    pub kind: BreakpointKindV1,
}

impl BreakpointSpecV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if let Some(label) = &self.client_label {
            validate_text(label, "client_label")?;
        }
        if let Some(scope) = self.scope {
            scope.validate()?;
        }
        if let Some(hit) = self.hit_condition {
            hit.validate()?;
        }
        self.kind.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BreakpointKindV1 {
    Site {
        site: KirSiteV1,
        phase: OperationStopPhaseV1,
    },
    Source {
        source: SourceLocationV1,
    },
    Barrier {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        barrier_id: Option<u32>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        phase: Option<u64>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        action: Option<BarrierActionV1>,
    },
    Diagnostic {
        class: DiagnosticClassV1,
    },
    Value {
        predicate: PredicateV1,
    },
}

impl BreakpointKindV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Source { source } => source.validate(),
            Self::Value { predicate } => predicate.validate(),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStopPhaseV1 {
    BeforeOperation,
    AfterOperation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BarrierActionV1 {
    Arrive,
    Release,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticClassV1 {
    Any,
    Trap,
    Assert,
    Fault,
    ResourceExhaustion,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HitConditionV1 {
    pub comparison: IntegerComparisonV1,
    pub count: u64,
}

impl HitConditionV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.count == 0 {
            return Err(ProtocolValidationErrorV1::ZeroCount("hit count"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegerComparisonV1 {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WatchpointSpecV1 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub client_label: Option<String>,
    pub enabled: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub scope: Option<ExecutionScopeSelectorV1>,
    pub allocation: AllocationIdentityV1,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub access: WatchAccessV1,
    pub timing: MemoryStopPhaseV1,
}

impl WatchpointSpecV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if let Some(label) = &self.client_label {
            validate_text(label, "client_label")?;
        }
        if let Some(scope) = self.scope {
            scope.validate()?;
        }
        self.allocation.validate()?;
        if self.byte_len == 0 {
            return Err(ProtocolValidationErrorV1::ZeroCount("watchpoint byte_len"));
        }
        self.byte_offset
            .checked_add(self.byte_len)
            .ok_or(ProtocolValidationErrorV1::RangeOverflow("watchpoint"))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchAccessV1 {
    Read,
    Write,
    Atomic,
    Any,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStopPhaseV1 {
    BeforeCommit,
    AfterCommit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "predicate", rename_all = "snake_case", deny_unknown_fields)]
pub enum PredicateV1 {
    Compare {
        left: PredicateOperandV1,
        comparison: IntegerComparisonV1,
        right: PredicateOperandV1,
    },
    All {
        predicates: Vec<PredicateV1>,
    },
    Any {
        predicates: Vec<PredicateV1>,
    },
    Not {
        predicate_value: Box<PredicateV1>,
    },
}

impl PredicateV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        let mut nodes = 0_usize;
        self.validate_at_depth(1, &mut nodes)
    }

    fn validate_at_depth(
        &self,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), ProtocolValidationErrorV1> {
        if depth > MAX_PREDICATE_DEPTH_V1 {
            return Err(ProtocolValidationErrorV1::PredicateDepthExceeded);
        }
        *nodes = nodes
            .checked_add(1)
            .ok_or(ProtocolValidationErrorV1::PredicateNodeLimitExceeded)?;
        if *nodes > MAX_PREDICATE_NODES_V1 {
            return Err(ProtocolValidationErrorV1::PredicateNodeLimitExceeded);
        }
        match self {
            Self::Compare { left, right, .. } => {
                left.validate()?;
                right.validate()
            }
            Self::All { predicates } | Self::Any { predicates } => {
                validate_nonempty_count(
                    predicates.len(),
                    MAX_PREDICATE_NODES_V1,
                    "predicate operands",
                )?;
                for predicate in predicates {
                    predicate.validate_at_depth(depth + 1, nodes)?;
                }
                Ok(())
            }
            Self::Not { predicate_value } => predicate_value.validate_at_depth(depth + 1, nodes),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PredicateOperandV1 {
    Value {
        path: ValuePathV1,
    },
    Bool {
        value: bool,
    },
    Integer {
        signed: bool,
        bits: u16,
        value: String,
    },
}

impl PredicateOperandV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Value { path } => path.validate(),
            Self::Bool { .. } => Ok(()),
            Self::Integer { bits, value, .. } => validate_bit_vector(value, *bits),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValuePathV1 {
    pub root: ValueRootV1,
    pub components: Vec<ValuePathComponentV1>,
}

impl ValuePathV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        self.root.validate()?;
        if self.components.len() > MAX_VALUE_PATH_COMPONENTS_V1 {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "value path components",
            ));
        }
        for component in &self.components {
            component.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValueRootV1 {
    Argument {
        ordinal: u32,
    },
    Ssa {
        function_ordinal: u64,
        frame: u64,
        value_ordinal: u64,
    },
    SourceVariable {
        name: String,
    },
    Register {
        name: String,
    },
}

impl ValueRootV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Ssa { frame, .. } if *frame == 0 => Err(ProtocolValidationErrorV1::ZeroIdentity),
            Self::SourceVariable { name } | Self::Register { name } => {
                validate_text(name, "value root name")
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValuePathComponentV1 {
    Field { name: String },
    Tuple { index: u32 },
    Array { index: u64 },
}

impl ValuePathComponentV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Field { name } => validate_text(name, "field name"),
            Self::Tuple { .. } | Self::Array { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "selector", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValueSelectorV1 {
    All,
    Roots { roots: Vec<ValueRootClassV1> },
    Paths { paths: Vec<ValuePathV1> },
}

impl ValueSelectorV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::All => Ok(()),
            Self::Roots { roots } => validate_nonempty_count(roots.len(), 4, "value root classes"),
            Self::Paths { paths } => {
                validate_nonempty_count(paths.len(), MAX_RESPONSE_ITEMS_V1, "value paths")?;
                for path in paths {
                    path.validate()?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueRootClassV1 {
    Argument,
    Ssa,
    SourceVariable,
    Register,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationIdentityV1 {
    pub ordinal: u64,
    pub generation: u64,
}

impl AllocationIdentityV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.ordinal == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugCursorV1 {
    pub configuration_identity: OpaqueIdentityV1,
    pub event_sequence: u64,
    pub state_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageCursorV1 {
    pub query_identity: OpaqueIdentityV1,
    pub position: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageRequestV1 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub cursor: Option<PageCursorV1>,
    pub limit: u16,
}

impl PageRequestV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.limit == 0 || self.limit > MAX_PAGE_ITEMS_V1 {
            return Err(ProtocolValidationErrorV1::CountOutOfRange("page limit"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventFilterV1 {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub sequence_start: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub sequence_end: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub scope: Option<ExecutionScopeSelectorV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub site: Option<KirSiteV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub allocation: Option<AllocationIdentityV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub category: Option<EventCategoryV1>,
}

impl EventFilterV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if matches!((self.sequence_start, self.sequence_end), (Some(start), Some(end)) if start > end)
        {
            return Err(ProtocolValidationErrorV1::InvalidRange("event sequence"));
        }
        if let Some(scope) = self.scope {
            scope.validate()?;
        }
        if let Some(allocation) = self.allocation {
            allocation.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategoryV1 {
    Dispatch,
    Invocation,
    Block,
    Operation,
    Branch,
    Memory,
    Barrier,
    Allocation,
    Diagnostic,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugResponseV1 {
    Ok {
        schema: ResponseSchemaV1,
        request_id: u64,
        operation: DebugOperationNameV1,
        session: SessionViewV1,
        result: Box<DebugResultV1>,
    },
    Unavailable {
        schema: ResponseSchemaV1,
        request_id: u64,
        operation: DebugOperationNameV1,
        session: SessionViewV1,
        unavailable: CapabilityUnavailableV1,
    },
    Error {
        schema: ResponseSchemaV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        request_id: Option<u64>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        operation: Option<DebugOperationNameV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        session: Option<SessionViewV1>,
        error: DebugErrorV1,
    },
}

impl DebugResponseV1 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        match self {
            Self::Ok {
                request_id,
                operation,
                session,
                result,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                session.validate()?;
                result.validate(limits)?;
                if !result.matches_operation(*operation) {
                    return Err(ProtocolValidationErrorV1::OperationResultMismatch);
                }
                Ok(())
            }
            Self::Unavailable {
                request_id,
                session,
                unavailable,
                ..
            } => {
                if *request_id == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroRequestId);
                }
                session.validate()?;
                unavailable.validate()
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
                if let Some(session) = session {
                    session.validate()?;
                }
                error.validate()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugBackendV1 {
    CpuKirSimulator,
    KfdHardware,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKindV1 {
    CpuKirSimulation,
    KfdHardware,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStateV1 {
    Created,
    Running,
    Stopped,
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionViewV1 {
    pub backend: DebugBackendV1,
    pub execution_kind: ExecutionKindV1,
    pub state: SessionStateV1,
    pub revision: u64,
    pub configuration_identity: OpaqueIdentityV1,
    pub cursor: DebugCursorV1,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
}

impl SessionViewV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.configuration_identity != self.cursor.configuration_identity {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "session cursor",
            ));
        }
        if self.revision != self.cursor.state_revision {
            return Err(ProtocolValidationErrorV1::RevisionMismatch);
        }
        match (self.backend, self.execution_kind) {
            (DebugBackendV1::CpuKirSimulator, ExecutionKindV1::CpuKirSimulation)
                if self.simulated && !self.hardware_observed && !self.performance_prediction => {}
            (DebugBackendV1::KfdHardware, ExecutionKindV1::KfdHardware)
                if !self.simulated && self.hardware_observed && !self.performance_prediction => {}
            _ => return Err(ProtocolValidationErrorV1::InvalidTruthClassification),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugResultV1 {
    Capabilities {
        capabilities: Vec<CapabilityViewV1>,
    },
    State {
        snapshot: SnapshotAvailabilityV1,
    },
    Breakpoints {
        breakpoints: Vec<BreakpointViewV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Watchpoints {
        watchpoints: Vec<WatchpointViewV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Control {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        stop: Option<StopViewV1>,
        snapshot: SnapshotAvailabilityV1,
        events_advanced: u64,
    },
    Scopes {
        scopes: Vec<ScopeViewV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Source {
        site: SemanticSiteViewV1,
    },
    Stack {
        snapshot: DebugSnapshotAnchorV1,
        frames: Vec<StackFrameV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Values {
        snapshot: DebugSnapshotAnchorV1,
        values: Vec<DebugValueV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Memory {
        snapshot: DebugSnapshotAnchorV1,
        memory: MemoryReadV1,
    },
    Events {
        events: Vec<DebugEventViewV1>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        next_cursor: Option<PageCursorV1>,
    },
    Trace {
        trace_identity: OpaqueIdentityV1,
        canonical_bytes: u64,
        bytes: String,
        completeness: CaptureCompletenessV1,
    },
    Acknowledged {
        accepted: u32,
    },
    Terminated,
}

impl DebugResultV1 {
    fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Capabilities { capabilities } => {
                validate_response_count(capabilities.len(), limits)?;
                for capability in capabilities {
                    capability.validate()?;
                }
            }
            Self::State { snapshot } => snapshot.validate(limits)?,
            Self::Breakpoints { breakpoints, .. } => {
                if breakpoints.len() > limits.max_breakpoints {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "breakpoint response",
                    ));
                }
                for breakpoint in breakpoints {
                    breakpoint.validate()?;
                }
            }
            Self::Watchpoints { watchpoints, .. } => {
                if watchpoints.len() > limits.max_watchpoints {
                    return Err(ProtocolValidationErrorV1::CountOutOfRange(
                        "watchpoint response",
                    ));
                }
                for watchpoint in watchpoints {
                    watchpoint.validate()?;
                }
            }
            Self::Control { stop, snapshot, .. } => {
                if let Some(stop) = stop {
                    stop.validate()?;
                }
                snapshot.validate(limits)?;
            }
            Self::Scopes { scopes, .. } => {
                validate_response_count(scopes.len(), limits)?;
                for scope in scopes {
                    scope.validate()?;
                }
            }
            Self::Source { site } => site.source.validate()?,
            Self::Stack {
                snapshot, frames, ..
            } => {
                snapshot.validate()?;
                validate_response_count(frames.len(), limits)?;
                let mut identities = BTreeSet::new();
                for frame in frames {
                    frame.validate()?;
                    if !identities.insert(frame.frame) {
                        return Err(ProtocolValidationErrorV1::DuplicateIdentity("stack frames"));
                    }
                }
            }
            Self::Values {
                snapshot, values, ..
            } => {
                snapshot.validate()?;
                validate_response_count(values.len(), limits)?;
                for value in values {
                    value.validate()?;
                }
            }
            Self::Memory { snapshot, memory } => {
                snapshot.validate()?;
                memory.validate()?;
            }
            Self::Events { events, .. } => {
                validate_response_count(events.len(), limits)?;
                for event in events {
                    event.validate()?;
                }
            }
            Self::Trace {
                canonical_bytes,
                bytes,
                ..
            } => validate_hex_bytes_exact(bytes, *canonical_bytes)?,
            Self::Acknowledged { accepted } if *accepted == 0 => {
                return Err(ProtocolValidationErrorV1::ZeroCount("accepted"));
            }
            Self::Acknowledged { .. } | Self::Terminated => {}
        }
        Ok(())
    }

    const fn matches_operation(&self, operation: DebugOperationNameV1) -> bool {
        matches!(
            (operation, self),
            (
                DebugOperationNameV1::DiscoverCapabilities,
                Self::Capabilities { .. }
            ) | (DebugOperationNameV1::GetState, Self::State { .. })
                | (
                    DebugOperationNameV1::SetBreakpoints
                        | DebugOperationNameV1::RemoveBreakpoints
                        | DebugOperationNameV1::SetWatchpoints
                        | DebugOperationNameV1::RemoveWatchpoints,
                    Self::Acknowledged { .. }
                )
                | (
                    DebugOperationNameV1::ListBreakpoints,
                    Self::Breakpoints { .. }
                )
                | (
                    DebugOperationNameV1::ListWatchpoints,
                    Self::Watchpoints { .. }
                )
                | (
                    DebugOperationNameV1::Continue
                        | DebugOperationNameV1::Pause
                        | DebugOperationNameV1::Step
                        | DebugOperationNameV1::Seek,
                    Self::Control { .. }
                )
                | (DebugOperationNameV1::InspectScope, Self::Scopes { .. })
                | (DebugOperationNameV1::ResolveSource, Self::Source { .. })
                | (DebugOperationNameV1::InspectStack, Self::Stack { .. })
                | (DebugOperationNameV1::InspectValues, Self::Values { .. })
                | (DebugOperationNameV1::ReadMemory, Self::Memory { .. })
                | (DebugOperationNameV1::QueryEvents, Self::Events { .. })
                | (DebugOperationNameV1::ExportTrace, Self::Trace { .. })
                | (DebugOperationNameV1::Terminate, Self::Terminated)
        )
    }
}

fn validate_response_count(
    actual: usize,
    limits: ProtocolLimitsV1,
) -> Result<(), ProtocolValidationErrorV1> {
    if actual > limits.max_response_items {
        return Err(ProtocolValidationErrorV1::CountOutOfRange("response items"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugCapabilityNameV1 {
    HierarchyInspection,
    KirSites,
    SourceSites,
    CallStack,
    Breakpoints,
    Watchpoints,
    ForwardStep,
    ReverseStep,
    Pause,
    DeterministicReplay,
    KirSsaValues,
    SourceVariableValues,
    RegisterValues,
    AllocationRelativeMemory,
    SemanticTrace,
    HardwareWaveState,
    KfdDispatchControl,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailabilityV1 {
    Available,
    Unavailable,
    AuthorizationRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityUnavailableReasonV1 {
    Absent,
    ManyToOne,
    NotRepresented,
    NotCaptured,
    RequiresAuthenticatedMap,
    NotExposedByBackend,
    LogicalVisualizationOnly,
    ReadOnlyBackend,
    OptimizedOut,
    OutsideCaptureScope,
    Truncated,
    AuthorizationRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityViewV1 {
    pub name: DebugCapabilityNameV1,
    pub availability: CapabilityAvailabilityV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub reason: Option<CapabilityUnavailableReasonV1>,
}

impl CapabilityViewV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        let valid = match self.availability {
            CapabilityAvailabilityV1::Available => self.reason.is_none(),
            CapabilityAvailabilityV1::Unavailable => self.reason.is_some(),
            CapabilityAvailabilityV1::AuthorizationRequired => {
                self.reason == Some(CapabilityUnavailableReasonV1::AuthorizationRequired)
            }
        };
        if !valid {
            return Err(ProtocolValidationErrorV1::InvalidAvailability);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityUnavailableV1 {
    pub capability: DebugCapabilityNameV1,
    pub reason: CapabilityUnavailableReasonV1,
    pub state_changed: bool,
    pub detail: String,
}

impl CapabilityUnavailableV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.state_changed {
            return Err(ProtocolValidationErrorV1::UnavailableChangedState);
        }
        validate_text(&self.detail, "unavailable detail")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugErrorV1 {
    pub stage: DebugErrorStageV1,
    pub code: DebugErrorCodeV1,
    pub message: String,
    pub state_changed: bool,
}

impl DebugErrorV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.state_changed {
            return Err(ProtocolValidationErrorV1::ErrorChangedState);
        }
        validate_text(&self.message, "error message")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugErrorStageV1 {
    Framing,
    Protocol,
    Session,
    Backend,
    Output,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugErrorCodeV1 {
    InvalidJson,
    InvalidRequest,
    UnsupportedSchema,
    StaleRevision,
    InvalidState,
    InvalidCursor,
    ResourceLimit,
    BackendFailure,
    ResponseTooLarge,
    OutputFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BreakpointViewV1 {
    pub breakpoint_id: u64,
    pub spec: BreakpointSpecV1,
    pub hit_count: u64,
}

impl BreakpointViewV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.breakpoint_id == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        self.spec.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WatchpointViewV1 {
    pub watchpoint_id: u64,
    pub spec: WatchpointSpecV1,
    pub hit_count: u64,
}

impl WatchpointViewV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.watchpoint_id == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        self.spec.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSnapshotV1 {
    pub anchor: DebugSnapshotAnchorV1,
    pub stop: StopViewV1,
    pub values: Vec<DebugValueV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SnapshotAvailabilityV1 {
    Captured { snapshot: Box<DebugSnapshotV1> },
    Unavailable { reason: SnapshotUnavailableReasonV1 },
}

impl SnapshotAvailabilityV1 {
    fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Captured { snapshot } => snapshot.validate(limits),
            Self::Unavailable { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotUnavailableReasonV1 {
    SessionNotStopped,
    SessionTerminatedWithoutCapture,
    NotCaptured,
    UnsupportedByBackend,
    CaptureBudgetExhausted,
}

impl DebugSnapshotV1 {
    fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        self.anchor.validate()?;
        self.stop.validate()?;
        validate_response_count(self.values.len(), limits)?;
        for value in &self.values {
            value.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugSnapshotAnchorV1 {
    pub cursor: DebugCursorV1,
    pub scope: ExecutionScopeV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub site: Option<SemanticSiteViewV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub frame: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub occurrence: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSiteViewV1 {
    pub kir: KirSiteV1,
    pub source: SourceSiteAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceSiteAvailabilityV1 {
    Resolved {
        location: SourceLocationV1,
    },
    Unavailable {
        reason: SourceSiteUnavailableReasonV1,
    },
}

impl SourceSiteAvailabilityV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Resolved { location } => location.validate(),
            Self::Unavailable { .. } => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSiteUnavailableReasonV1 {
    Absent,
    ManyToOne,
    RequiresAuthenticatedMap,
    NotRepresented,
    OptimizedOut,
    OutsideCaptureScope,
    Truncated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StackFrameV1 {
    pub frame: u64,
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub next_operation: Option<u64>,
    pub values: StackValuesAvailabilityV1,
}

impl StackFrameV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.frame == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum StackValuesAvailabilityV1 {
    Captured { value_count: u64 },
    Unavailable { reason: ValueUnavailableReasonV1 },
}

impl DebugSnapshotAnchorV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.scope.validate()?;
        if let Some(site) = self.site {
            site.source.validate()?;
        }
        if self.frame == Some(0) || self.occurrence == Some(0) {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if self.occurrence.is_some() != self.frame.is_some() {
            return Err(ProtocolValidationErrorV1::OccurrenceWithoutFrame);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StopViewV1 {
    pub reason: StopReasonV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub breakpoint_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub watchpoint_id: Option<u64>,
    pub outcome: ExecutionOutcomeV1,
    pub exact: bool,
}

impl StopViewV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if self.breakpoint_id == Some(0) || self.watchpoint_id == Some(0) {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        if matches!(self.reason, StopReasonV1::Breakpoint) != self.breakpoint_id.is_some()
            || matches!(self.reason, StopReasonV1::Watchpoint) != self.watchpoint_id.is_some()
        {
            return Err(ProtocolValidationErrorV1::StopIdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReasonV1 {
    Entry,
    Step,
    Breakpoint,
    Watchpoint,
    PauseRequested,
    Barrier,
    Fault,
    ResourceExhaustion,
    Completed,
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcomeV1 {
    Active,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeViewV1 {
    pub scope: ExecutionScopeV1,
    pub state: ScopeStateV1,
}

impl ScopeViewV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.scope.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeStateV1 {
    NotStarted,
    Runnable,
    Running,
    BarrierBlocked,
    Completed,
    Failed,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugValueV1 {
    pub path: ValuePathV1,
    pub availability: ValueAvailabilityV1,
}

impl DebugValueV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        self.path.validate()?;
        self.availability.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValueAvailabilityV1 {
    Captured {
        value_type: DebugValueTypeV1,
        value: CapturedValueV1,
        provenance: ValueProvenanceV1,
    },
    Redacted {
        reason: RedactionReasonV1,
    },
    Unavailable {
        reason: ValueUnavailableReasonV1,
    },
}

impl ValueAvailabilityV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        if let Self::Captured {
            value_type, value, ..
        } = self
        {
            value_type.validate()?;
            value.validate_for_type(value_type)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DebugValueTypeV1 {
    Bool,
    Integer {
        signed: bool,
        bits: u16,
    },
    Index {
        bits: u16,
    },
    Float {
        bits: u16,
    },
    Pointer {
        address_space: AddressSpaceV1,
    },
    Bytes {
        byte_len: u64,
    },
    Aggregate {
        aggregate: AggregateKindV1,
        elements: u64,
        byte_len: u64,
    },
}

impl DebugValueTypeV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Integer { bits, .. } if bits == 0 || usize::from(bits) > 4_096 => {
                Err(ProtocolValidationErrorV1::InvalidValueType)
            }
            Self::Index { bits } if !matches!(bits, 32 | 64) => {
                Err(ProtocolValidationErrorV1::InvalidValueType)
            }
            Self::Float { bits } if !matches!(bits, 16 | 32 | 64) => {
                Err(ProtocolValidationErrorV1::InvalidValueType)
            }
            Self::Bytes { byte_len } | Self::Aggregate { byte_len, .. } if byte_len == 0 => {
                Err(ProtocolValidationErrorV1::InvalidValueType)
            }
            _ => Ok(()),
        }
    }

    const fn bit_width(self) -> Option<u16> {
        match self {
            Self::Bool => Some(1),
            Self::Integer { bits, .. } | Self::Index { bits } | Self::Float { bits } => Some(bits),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateKindV1 {
    Struct,
    Tuple,
    Array,
    Slice,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapturedValueV1 {
    Bits {
        bits: String,
    },
    AllocationRelativePointer {
        allocation: AllocationIdentityV1,
        byte_offset: u64,
    },
    Bytes {
        bytes: String,
        initialized: String,
    },
}

impl CapturedValueV1 {
    fn validate_for_type(
        &self,
        value_type: &DebugValueTypeV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        match (self, *value_type) {
            (Self::Bits { bits }, ty) if ty.bit_width().is_some() => {
                validate_bit_vector(bits, ty.bit_width().expect("guarded"))
            }
            (
                Self::AllocationRelativePointer { allocation, .. },
                DebugValueTypeV1::Pointer { .. },
            ) => allocation.validate(),
            (
                Self::Bytes { bytes, initialized },
                DebugValueTypeV1::Bytes { byte_len } | DebugValueTypeV1::Aggregate { byte_len, .. },
            ) => {
                validate_hex_bytes_exact(bytes, byte_len)?;
                validate_initialization_bits(initialized, byte_len)
            }
            _ => Err(ProtocolValidationErrorV1::ValueEncodingTypeMismatch),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueProvenanceV1 {
    SimulatedObservation,
    HardwareObservation,
    Reconstructed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionReasonV1 {
    NativeAddress,
    RuntimeHandle,
    Policy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueUnavailableReasonV1 {
    NotRepresented,
    NotCaptured,
    OptimizedOut,
    OutsideCaptureScope,
    NotInScope,
    NotLive,
    Uninitialized,
    Truncated,
    UnsupportedByBackend,
    RequiresAuthenticatedMap,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryReadV1 {
    pub allocation: AllocationIdentityV1,
    pub byte_offset: u64,
    pub requested_bytes: u64,
    pub returned_bytes: u64,
    pub availability: MemoryAvailabilityV1,
}

impl MemoryReadV1 {
    fn validate(&self) -> Result<(), ProtocolValidationErrorV1> {
        self.allocation.validate()?;
        if self.requested_bytes == 0
            || self.requested_bytes > MAX_MEMORY_READ_BYTES_V1
            || self.returned_bytes > self.requested_bytes
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange(
                "memory response bytes",
            ));
        }
        self.byte_offset
            .checked_add(self.returned_bytes)
            .ok_or(ProtocolValidationErrorV1::RangeOverflow("memory response"))?;
        self.availability.validate(self.returned_bytes)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryAvailabilityV1 {
    Captured {
        address_space: AddressSpaceV1,
        bytes: String,
        initialized: String,
        truncated: bool,
    },
    Redacted {
        reason: RedactionReasonV1,
    },
    Unavailable {
        reason: ValueUnavailableReasonV1,
    },
}

impl MemoryAvailabilityV1 {
    fn validate(&self, returned_bytes: u64) -> Result<(), ProtocolValidationErrorV1> {
        match self {
            Self::Captured {
                bytes, initialized, ..
            } => {
                validate_hex_bytes_exact(bytes, returned_bytes)?;
                validate_initialization_bits(initialized, returned_bytes)
            }
            Self::Redacted { .. } | Self::Unavailable { .. } if returned_bytes == 0 => Ok(()),
            Self::Redacted { .. } | Self::Unavailable { .. } => {
                Err(ProtocolValidationErrorV1::UnavailableCarriesBytes)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressSpaceV1 {
    Private,
    Workgroup,
    Global,
    Constant,
    Generic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugEventViewV1 {
    pub sequence: u64,
    pub scope: ExecutionScopeV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_non_null"
    )]
    pub site: Option<KirSiteV1>,
    pub category: EventCategoryV1,
    pub provenance: EventProvenanceV1,
}

impl DebugEventViewV1 {
    fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.scope.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventProvenanceV1 {
    Declared,
    Proved,
    SimulatedObservation,
    HardwareObservation,
    Inferred,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureCompletenessV1 {
    Complete,
    Truncated {
        reason: CaptureTruncationReasonV1,
        emitted_events: u64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_optional_non_null"
        )]
        dropped_events: Option<u64>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureTruncationReasonV1 {
    EventLimit,
    ByteLimit,
    ResidentLimit,
    ProducerFailure,
    UserStopped,
}

fn validate_text(value: &str, field: &'static str) -> Result<(), ProtocolValidationErrorV1> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES_V1 || value.chars().any(char::is_control) {
        return Err(ProtocolValidationErrorV1::InvalidText(field));
    }
    Ok(())
}

fn validate_bit_vector(value: &str, bits: u16) -> Result<(), ProtocolValidationErrorV1> {
    if bits == 0 || usize::from(bits) > MAX_BIT_VECTOR_HEX_DIGITS_V1 * 4 {
        return Err(ProtocolValidationErrorV1::InvalidBitVector);
    }
    let digits = usize::from(bits).div_ceil(4);
    let Some(encoded) = value.strip_prefix("0x") else {
        return Err(ProtocolValidationErrorV1::InvalidBitVector);
    };
    if encoded.len() != digits
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProtocolValidationErrorV1::InvalidBitVector);
    }
    if !bits.is_multiple_of(4) {
        let high =
            hex_nibble(encoded.as_bytes()[0]).ok_or(ProtocolValidationErrorV1::InvalidBitVector)?;
        if high >= (1_u8 << (bits % 4)) {
            return Err(ProtocolValidationErrorV1::InvalidBitVector);
        }
    }
    Ok(())
}

fn validate_hex_bytes_exact(
    value: &str,
    expected_bytes: u64,
) -> Result<(), ProtocolValidationErrorV1> {
    let expected_digits = usize::try_from(expected_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(ProtocolValidationErrorV1::RangeOverflow("hex bytes"))?;
    let Some(encoded) = value.strip_prefix("0x") else {
        return Err(ProtocolValidationErrorV1::InvalidHexBytes);
    };
    if encoded.len() != expected_digits
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProtocolValidationErrorV1::InvalidHexBytes);
    }
    Ok(())
}

fn validate_initialization_bits(value: &str, bytes: u64) -> Result<(), ProtocolValidationErrorV1> {
    let encoded_bytes = bytes.checked_add(7).map(|bits| bits / 8).ok_or(
        ProtocolValidationErrorV1::RangeOverflow("initialization bits"),
    )?;
    validate_hex_bytes_exact(value, encoded_bytes)?;
    if !bytes.is_multiple_of(8) && encoded_bytes != 0 {
        let encoded = value.strip_prefix("0x").expect("validated prefix");
        let final_pair = &encoded.as_bytes()[encoded.len() - 2..];
        let final_byte = (hex_nibble(final_pair[0]).expect("validated hex") << 4)
            | hex_nibble(final_pair[1]).expect("validated hex");
        if final_byte >> (bytes % 8) != 0 {
            return Err(ProtocolValidationErrorV1::InvalidInitializationBits);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolValidationErrorV1 {
    LimitOutOfRange(&'static str),
    ZeroRequestId,
    ZeroIdentity,
    ZeroCount(&'static str),
    CountOutOfRange(&'static str),
    DuplicateIdentity(&'static str),
    RangeOverflow(&'static str),
    InvalidRange(&'static str),
    InvalidText(&'static str),
    PredicateDepthExceeded,
    PredicateNodeLimitExceeded,
    InvalidActiveMask,
    IdentityMismatch(&'static str),
    RevisionMismatch,
    InvalidTruthClassification,
    InvalidAvailability,
    OperationResultMismatch,
    UnavailableChangedState,
    ErrorChangedState,
    OccurrenceWithoutFrame,
    StopIdentityMismatch,
    InvalidValueType,
    InvalidBitVector,
    InvalidHexBytes,
    InvalidInitializationBits,
    ValueEncodingTypeMismatch,
    UnavailableCarriesBytes,
}

impl fmt::Display for ProtocolValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid fe2o3 debug protocol value: {self:?}")
    }
}

impl std::error::Error for ProtocolValidationErrorV1 {}
