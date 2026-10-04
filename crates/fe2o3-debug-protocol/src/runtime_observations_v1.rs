//! Additive CPU runtime observations. Tokens are process-local owner labels,
//! not authentication. A client MUST bind them to its accepted backend transport
//! lifetime; reconnect/relaunch clears every selection and continuation token.
use crate::{
    CaptureCompletenessV1, DebugBackendV1, DebugCursorV1, DebugErrorV1, ExecutionKindV1,
    ProtocolLimitsV1, ProtocolValidationErrorV1, ResourceDecimalU64V1, SessionViewV1,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RUNTIME_OBSERVATION_REQUEST_SCHEMA_V1: &str =
    "fe2o3-debug-runtime-observation-request-v1";
pub const RUNTIME_OBSERVATION_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-debug-runtime-observation-response-v1";
pub const MAX_RUNTIME_FRAMES_V1: usize = 4_096;
pub type RuntimeDecimalU64V1 = ResourceDecimalU64V1;

pub(crate) fn observed_optional_non_null<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeObservationRequestSchemaV1 {
    #[serde(rename = "fe2o3-debug-runtime-observation-request-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeObservationResponseSchemaV1 {
    #[serde(rename = "fe2o3-debug-runtime-observation-response-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeObservationOwnerV1 {
    pub backend_session: RuntimeDecimalU64V1,
    pub capture_instance: RuntimeDecimalU64V1,
}
impl RuntimeObservationOwnerV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.backend_session.get() == 0 || self.capture_instance.get() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeObservationBindingV1 {
    pub owner: RuntimeObservationOwnerV1,
    pub cursor: DebugCursorV1,
}
impl RuntimeObservationBindingV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        self.owner.validate()
    }
    /// Exact comparison is intentionally stronger than revision alone.
    pub fn matches(self, actual: Self) -> Result<(), ProtocolValidationErrorV1> {
        self.validate()?;
        actual.validate()?;
        if self != actual {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "runtime observation binding",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeInvocationV1 {
    pub global: [RuntimeDecimalU64V1; 3],
    pub workgroup: [RuntimeDecimalU64V1; 3],
    pub local: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub workgroup_count: [RuntimeDecimalU64V1; 3],
    pub launch_extent: [RuntimeDecimalU64V1; 3],
}
impl RuntimeInvocationV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        for axis in 0..3 {
            let size = u64::from(self.workgroup_size[axis]);
            let extent = self.launch_extent[axis].get();
            let count = self.workgroup_count[axis].get();
            let group = self.workgroup[axis].get();
            let local = u64::from(self.local[axis]);
            if size == 0
                || extent == 0
                || count != extent.div_ceil(size)
                || group >= count
                || local >= size
            {
                return Err(ProtocolValidationErrorV1::InvalidRange(
                    "runtime invocation",
                ));
            }
            let global = group
                .checked_mul(size)
                .and_then(|v| v.checked_add(local))
                .ok_or(ProtocolValidationErrorV1::RangeOverflow(
                    "runtime invocation",
                ))?;
            if self.global[axis].get() != global || global >= extent {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "runtime invocation",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOperationSiteV1 {
    /// Zero-based simulator function ordinal, not a source name or symbol ID.
    pub function_ordinal: RuntimeDecimalU64V1,
    pub block: u32,
    pub operation: u32,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOperationIdentityV1 {
    pub activation: RuntimeDecimalU64V1,
    pub attempt: RuntimeDecimalU64V1,
    pub site: RuntimeOperationSiteV1,
}
impl RuntimeOperationIdentityV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if self.activation.get() == 0 || self.attempt.get() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroIdentity);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeObservationUnavailableV1 {
    NotRequested,
    PolicyDisabled,
    NoSelectedRecord,
    NotCheckpoint,
    NoMatchingOperation,
    AggregateRecord,
    LegacyStackUnavailable,
    IdentityInvariant,
    PrefixTruncated,
    InvalidJoin,
    AllocationFailure,
    ObservationStopped,
    SequenceOverflow,
    NotCaptured,
    WorkLimit,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeOperationObservationV1 {
    Available {
        identity: RuntimeOperationIdentityV1,
    },
    Unavailable {
        reason: RuntimeObservationUnavailableV1,
    },
}
impl RuntimeOperationObservationV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if let Self::Available { identity } = self {
            identity.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFrameOperationV1 {
    Ready,
    ActiveOperation {
        attempt: RuntimeDecimalU64V1,
        site: RuntimeOperationSiteV1,
    },
    Suspended {
        attempt: RuntimeDecimalU64V1,
        site: RuntimeOperationSiteV1,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "parent", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFrameParentV1 {
    Root,
    Caller {
        activation: RuntimeDecimalU64V1,
        attempt: RuntimeDecimalU64V1,
        call_site: RuntimeOperationSiteV1,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFrameV1 {
    /// Join coordinate only; persisted selection must use the real activation.
    pub legacy_depth: u32,
    pub function_ordinal: RuntimeDecimalU64V1,
    pub block: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "observed_optional_non_null"
    )]
    pub next_operation: Option<u32>,
    pub activation: RuntimeDecimalU64V1,
    pub operation: RuntimeFrameOperationV1,
    pub parent: RuntimeFrameParentV1,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFramesV1 {
    Captured {
        frames: Vec<RuntimeFrameV1>,
    },
    Unavailable {
        reason: RuntimeObservationUnavailableV1,
    },
}
impl RuntimeFramesV1 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        let Self::Captured { frames } = self else {
            return Ok(());
        };
        if frames.is_empty()
            || frames.len() > MAX_RUNTIME_FRAMES_V1
            || frames.len() > limits.max_response_items
        {
            return Err(ProtocolValidationErrorV1::CountOutOfRange("runtime frames"));
        }
        let mut identities = BTreeSet::new();
        for (depth, row) in frames.iter().enumerate() {
            if row.legacy_depth as usize != depth || row.activation.get() == 0 {
                return Err(ProtocolValidationErrorV1::InvalidRange(
                    "runtime frame depth or activation",
                ));
            }
            if !identities.insert(row.activation.get()) {
                return Err(ProtocolValidationErrorV1::DuplicateIdentity(
                    "runtime activation",
                ));
            }
            if let RuntimeFrameOperationV1::ActiveOperation { attempt, site }
            | RuntimeFrameOperationV1::Suspended { attempt, site } = row.operation
                && (attempt.get() == 0 || site.function_ordinal != row.function_ordinal)
            {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "runtime frame operation",
                ));
            }
            match (depth, row.parent) {
                (0, RuntimeFrameParentV1::Root) => {}
                (0, _) | (_, RuntimeFrameParentV1::Root) => {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "runtime root parent",
                    ));
                }
                (
                    _,
                    RuntimeFrameParentV1::Caller {
                        activation,
                        attempt,
                        call_site,
                    },
                ) => {
                    let caller = frames[depth - 1];
                    if activation != caller.activation
                        || activation.get() >= row.activation.get()
                        || attempt.get() == 0
                        || caller.operation
                            != (RuntimeFrameOperationV1::Suspended {
                                attempt,
                                site: call_site,
                            })
                    {
                        return Err(ProtocolValidationErrorV1::IdentityMismatch(
                            "runtime caller",
                        ));
                    }
                }
            }
            if depth + 1 < frames.len()
                && !matches!(row.operation, RuntimeFrameOperationV1::Suspended { .. })
            {
                return Err(ProtocolValidationErrorV1::InvalidAvailability);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetadataCutoffV1 {
    RowLimit,
    FrameLimit,
    TransitionLimit,
    ValidationWorkLimit,
    ByteLimit,
    AllocationFailure,
    InvalidCapacity,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "coverage", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeMetadataCoverageV1 {
    Disabled,
    Complete,
    PrefixTruncated {
        retained_records: RuntimeDecimalU64V1,
        reason: RuntimeMetadataCutoffV1,
    },
    InvalidJoin,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeAllocationWatermarkV1 {
    Available {
        through_sequence: RuntimeDecimalU64V1,
    },
    Unavailable {
        reason: RuntimeObservationUnavailableV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeObservationRequestV1 {
    InspectCurrentRecord {
        schema: RuntimeObservationRequestSchemaV1,
        request_id: u64,
        expected_revision: u64,
        expected_cursor: DebugCursorV1,
        /// Omit only for owner discovery on an accepted backend connection.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "observed_optional_non_null"
        )]
        expected_owner: Option<RuntimeObservationOwnerV1>,
    },
}
impl RuntimeObservationRequestV1 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::InspectCurrentRecord { request_id, .. } => *request_id,
        }
    }
    pub const fn expected_revision(&self) -> u64 {
        match self {
            Self::InspectCurrentRecord {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub const fn expected_cursor(&self) -> DebugCursorV1 {
        match self {
            Self::InspectCurrentRecord {
                expected_cursor, ..
            } => *expected_cursor,
        }
    }
    pub const fn expected_owner(&self) -> Option<RuntimeObservationOwnerV1> {
        match self {
            Self::InspectCurrentRecord { expected_owner, .. } => *expected_owner,
        }
    }
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        if self.expected_revision() != self.expected_cursor().state_revision {
            return Err(ProtocolValidationErrorV1::RevisionMismatch);
        }
        if let Some(owner) = self.expected_owner() {
            owner.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeObservationResponseV1 {
    Ok {
        schema: RuntimeObservationResponseSchemaV1,
        request_id: u64,
        session: SessionViewV1,
        binding: RuntimeObservationBindingV1,
        invocation: Box<RuntimeInvocationV1>,
        origin: RuntimeOperationObservationV1,
        frames: RuntimeFramesV1,
        allocation_watermark: RuntimeAllocationWatermarkV1,
        completeness: CaptureCompletenessV1,
        origin_coverage: RuntimeMetadataCoverageV1,
        frame_coverage: RuntimeMetadataCoverageV1,
        lifecycle_coverage: RuntimeMetadataCoverageV1,
    },
    Unavailable {
        schema: RuntimeObservationResponseSchemaV1,
        request_id: u64,
        session: SessionViewV1,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "observed_optional_non_null"
        )]
        binding: Option<RuntimeObservationBindingV1>,
        completeness: CaptureCompletenessV1,
        reason: RuntimeObservationUnavailableV1,
    },
    Error {
        schema: RuntimeObservationResponseSchemaV1,
        request_id: u64,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}
pub(crate) fn validate_observed_session(
    session: SessionViewV1,
) -> Result<(), ProtocolValidationErrorV1> {
    session.validate()?;
    if session.backend != DebugBackendV1::CpuKirSimulator
        || session.execution_kind != ExecutionKindV1::CpuKirSimulation
    {
        return Err(ProtocolValidationErrorV1::InvalidTruthClassification);
    }
    Ok(())
}
impl RuntimeObservationResponseV1 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::Ok { request_id, .. }
            | Self::Unavailable { request_id, .. }
            | Self::Error { request_id, .. } => *request_id,
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
        let binding = match self {
            Self::Ok {
                binding,
                invocation,
                origin,
                frames,
                origin_coverage,
                frame_coverage,
                ..
            } => {
                if binding.cursor.event_sequence == 0 {
                    return Err(ProtocolValidationErrorV1::ZeroIdentity);
                }
                invocation.validate()?;
                origin.validate()?;
                frames.validate(limits)?;
                if (matches!(origin, RuntimeOperationObservationV1::Available { .. })
                    && matches!(
                        origin_coverage,
                        RuntimeMetadataCoverageV1::Disabled
                            | RuntimeMetadataCoverageV1::InvalidJoin
                    ))
                    || (matches!(frames, RuntimeFramesV1::Captured { .. })
                        && matches!(
                            frame_coverage,
                            RuntimeMetadataCoverageV1::Disabled
                                | RuntimeMetadataCoverageV1::InvalidJoin
                        ))
                {
                    return Err(ProtocolValidationErrorV1::InvalidAvailability);
                }
                // A complete checkpoint roster includes the current operation's
                // activation. Its next_operation is deliberately NOT compared:
                // after-operation capture can already point to the next site.
                if let (
                    RuntimeOperationObservationV1::Available { identity },
                    RuntimeFramesV1::Captured { frames },
                ) = (origin, frames)
                    && !frames.last().is_some_and(|frame| {
                        frame.activation == identity.activation
                            && matches!(frame.operation,
                            RuntimeFrameOperationV1::ActiveOperation { attempt, site }
                            if attempt == identity.attempt && site == identity.site)
                    })
                {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "runtime origin frame",
                    ));
                }
                if let RuntimeAllocationWatermarkV1::Available { .. } = self.allocation_watermark()
                {
                    let Self::Ok {
                        lifecycle_coverage, ..
                    } = self
                    else {
                        unreachable!()
                    };
                    validate_available_coverage(
                        *lifecycle_coverage,
                        binding.cursor.event_sequence,
                    )?;
                }
                if matches!(origin, RuntimeOperationObservationV1::Available { .. }) {
                    validate_available_coverage(*origin_coverage, binding.cursor.event_sequence)?;
                }
                if matches!(frames, RuntimeFramesV1::Captured { .. }) {
                    validate_available_coverage(*frame_coverage, binding.cursor.event_sequence)?;
                }
                Some(*binding)
            }
            Self::Unavailable { binding, .. } => *binding,
            Self::Error { error, .. } => {
                error.validate()?;
                None
            }
        };
        if let Some(binding) = binding {
            binding.validate()?;
            if binding.cursor != self.session().cursor {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "runtime session cursor",
                ));
            }
        }
        let completeness = match self {
            Self::Ok { completeness, .. } => Some(*completeness),
            Self::Unavailable { .. } | Self::Error { .. } => None,
        };
        if let Some(CaptureCompletenessV1::Truncated { emitted_events, .. }) = completeness
            && emitted_events < self.session().cursor.event_sequence
        {
            return Err(ProtocolValidationErrorV1::InvalidAvailability);
        }
        Ok(())
    }
    fn allocation_watermark(&self) -> RuntimeAllocationWatermarkV1 {
        match self {
            Self::Ok {
                allocation_watermark,
                ..
            } => *allocation_watermark,
            _ => RuntimeAllocationWatermarkV1::Unavailable {
                reason: RuntimeObservationUnavailableV1::NotCaptured,
            },
        }
    }
    pub fn validate_for_request(
        &self,
        request: &RuntimeObservationRequestV1,
        limits: ProtocolLimitsV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        request.validate(limits)?;
        self.validate(limits)?;
        if self.request_id() != request.request_id() {
            return Err(ProtocolValidationErrorV1::OperationResultMismatch);
        }
        if matches!(self, Self::Error { .. }) {
            return Ok(());
        }
        if self.session().cursor != request.expected_cursor() {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "runtime expected cursor",
            ));
        }
        let binding = match self {
            Self::Ok { binding, .. } => Some(*binding),
            Self::Unavailable { binding, .. } => *binding,
            Self::Error { .. } => None,
        };
        if let Some(expected) = request.expected_owner()
            && binding.is_none_or(|binding| binding.owner != expected)
        {
            return Err(ProtocolValidationErrorV1::IdentityMismatch(
                "runtime expected owner",
            ));
        }
        Ok(())
    }
}

fn validate_available_coverage(
    coverage: RuntimeMetadataCoverageV1,
    event_sequence: u64,
) -> Result<(), ProtocolValidationErrorV1> {
    match coverage {
        RuntimeMetadataCoverageV1::Complete => Ok(()),
        RuntimeMetadataCoverageV1::PrefixTruncated {
            retained_records, ..
        } if retained_records.get() >= event_sequence => Ok(()),
        _ => Err(ProtocolValidationErrorV1::InvalidAvailability),
    }
}
