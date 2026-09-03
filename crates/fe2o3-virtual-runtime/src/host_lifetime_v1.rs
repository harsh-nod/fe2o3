//! Canonical, bounded evidence for virtual host-lifetime conflicts.

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{AccessMode, ScalarType};
use fe2o3_kir_sim::SimulationKernelIrIdentityV1;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::{
    DispatchRecordLocalV1, VirtualArgumentV1, VirtualBufferAccessV1, VirtualBufferHandleV1,
    VirtualCompletionAmbiguityV1, VirtualCompletionStateV1, VirtualRuntimeV1, virtual_scalar_bytes,
};

pub const VIRTUAL_HOST_LIFETIME_EVIDENCE_SCHEMA_V1: &str =
    "fe2o3-virtual-host-lifetime-evidence-v1";
pub const MAX_VIRTUAL_HOST_LIFETIME_BLOCKERS_V1: usize = 256;
pub const MAX_VIRTUAL_HOST_LIFETIME_INPUT_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_VIRTUAL_HOST_LIFETIME_EVIDENCE_BYTES_V1: usize = 4 * 1024 * 1024;

const DISPATCH_INPUT_DOMAIN_V1: &[u8] = b"FE2O3/VIRTUAL/HOST-LIFETIME/DISPATCH-INPUT/V1\0";
const BLOCKER_DOMAIN_V1: &[u8] = b"FE2O3/VIRTUAL/HOST-LIFETIME/BLOCKER/V1\0";
const INCIDENT_DOMAIN_V1: &[u8] = b"FE2O3/VIRTUAL/HOST-LIFETIME/INCIDENT/V1\0";

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtualEvidenceIdentityV1([u8; 32]);

impl VirtualEvidenceIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, VirtualHostLifetimeCodecErrorV1> {
        if bytes == [0; 32] {
            return Err(VirtualHostLifetimeCodecErrorV1::InvalidIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for VirtualEvidenceIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VirtualEvidenceIdentityV1(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

impl Serialize for VirtualEvidenceIdentityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut encoded = [0_u8; 64];
        encode_hex(&self.0, &mut encoded);
        let encoded = std::str::from_utf8(&encoded).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(encoded)
    }
}

impl<'de> Deserialize<'de> for VirtualEvidenceIdentityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex_identity(&encoded)
            .and_then(Self::new)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualHostLifetimeEvidenceLimitsV1 {
    max_blockers: usize,
    max_input_bytes_hashed: usize,
}

impl VirtualHostLifetimeEvidenceLimitsV1 {
    pub fn new(
        max_blockers: usize,
        max_input_bytes_hashed: usize,
    ) -> Result<Self, VirtualHostLifetimeCaptureErrorV1> {
        if max_blockers == 0 || max_blockers > MAX_VIRTUAL_HOST_LIFETIME_BLOCKERS_V1 {
            return Err(VirtualHostLifetimeCaptureErrorV1::InvalidLimit(
                "max_blockers",
            ));
        }
        if max_input_bytes_hashed > MAX_VIRTUAL_HOST_LIFETIME_INPUT_BYTES_V1 {
            return Err(VirtualHostLifetimeCaptureErrorV1::InvalidLimit(
                "max_input_bytes_hashed",
            ));
        }
        Ok(Self {
            max_blockers,
            max_input_bytes_hashed,
        })
    }

    pub const fn max_blockers(self) -> usize {
        self.max_blockers
    }

    pub const fn max_input_bytes_hashed(self) -> usize {
        self.max_input_bytes_hashed
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualHostLifetimeOperationV1 {
    ReleaseBuffer,
    CopyFromHost,
    CopyToHost,
    SnapshotBuffer,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualHostLifetimeFindingV1 {
    ReleaseWhileRetained,
    HostAccessWhileRetained,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualHostLifetimeTruthV1 {
    DeclaredAttemptAgainstObservedVirtualState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualHostLifetimeAuthorityV1 {
    AdvisorySimulationEvidenceNoExecutionOrHardwareAuthority,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualBufferAccessEvidenceV1 {
    ReadOnly,
    ReadWrite,
}

impl From<VirtualBufferAccessV1> for VirtualBufferAccessEvidenceV1 {
    fn from(value: VirtualBufferAccessV1) -> Self {
        match value {
            VirtualBufferAccessV1::ReadOnly => Self::ReadOnly,
            VirtualBufferAccessV1::ReadWrite => Self::ReadWrite,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualBlockingCompletionStateV1 {
    Prepared,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualBlockingCompletionAmbiguityV1 {
    PublicationOutcomeUnknown,
    WaitDeadlineExpired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualKirEvidenceReferenceV1 {
    pub wire_version: u16,
    pub sha256: VirtualEvidenceIdentityV1,
    pub canonical_bytes: u64,
}

impl TryFrom<SimulationKernelIrIdentityV1> for VirtualKirEvidenceReferenceV1 {
    type Error = VirtualHostLifetimeCodecErrorV1;

    fn try_from(value: SimulationKernelIrIdentityV1) -> Result<Self, Self::Error> {
        Ok(Self {
            wire_version: value.wire_version(),
            sha256: VirtualEvidenceIdentityV1::new(*value.digest())?,
            canonical_bytes: value.canonical_length(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum VirtualDispatchInputBindingV1 {
    Exact {
        identity: VirtualEvidenceIdentityV1,
        snapshot_bytes_hashed: u64,
    },
    Unavailable {
        reason: VirtualDispatchInputUnavailableReasonV1,
        required_snapshot_bytes: u64,
        limit: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualDispatchInputUnavailableReasonV1 {
    SnapshotByteLimit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualHostLifetimeBlockerV1 {
    pub completion_ordinal: u64,
    pub queue_ordinal: u64,
    pub module_ordinal: u64,
    pub state: VirtualBlockingCompletionStateV1,
    pub ambiguity: Option<VirtualBlockingCompletionAmbiguityV1>,
    pub kir: VirtualKirEvidenceReferenceV1,
    pub dispatch_input: VirtualDispatchInputBindingV1,
    pub blocker_identity: VirtualEvidenceIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum VirtualHostLifetimeCompletenessV1 {
    Complete,
    PartialBlockerLimit {
        total_blockers: u64,
        retained_blockers: u64,
    },
    PartialInputIdentity {
        total_blockers: u64,
        retained_blockers: u64,
    },
    PartialBlockerAndInputIdentity {
        total_blockers: u64,
        retained_blockers: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualHostLifetimeEvidenceV1 {
    pub schema: String,
    pub runtime_identity: VirtualEvidenceIdentityV1,
    pub buffer_ordinal: u64,
    pub operation: VirtualHostLifetimeOperationV1,
    pub finding: VirtualHostLifetimeFindingV1,
    pub truth: VirtualHostLifetimeTruthV1,
    pub authority: VirtualHostLifetimeAuthorityV1,
    pub buffer_access: VirtualBufferAccessEvidenceV1,
    pub retained_dispatches: u64,
    pub blockers: Vec<VirtualHostLifetimeBlockerV1>,
    pub completeness: VirtualHostLifetimeCompletenessV1,
    pub incident_identity: VirtualEvidenceIdentityV1,
}

impl VirtualHostLifetimeEvidenceV1 {
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }

    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, VirtualHostLifetimeCodecErrorV1> {
        validate_evidence(self)?;
        let bytes = serde_json::to_vec(self)
            .map_err(|_| VirtualHostLifetimeCodecErrorV1::EncodingFailure)?;
        if bytes.len() > MAX_VIRTUAL_HOST_LIFETIME_EVIDENCE_BYTES_V1 {
            return Err(VirtualHostLifetimeCodecErrorV1::ByteLimit);
        }
        Ok(bytes)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, VirtualHostLifetimeCodecErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_VIRTUAL_HOST_LIFETIME_EVIDENCE_BYTES_V1 {
            return Err(VirtualHostLifetimeCodecErrorV1::ByteLimit);
        }
        let evidence: Self = serde_json::from_slice(bytes)
            .map_err(|_| VirtualHostLifetimeCodecErrorV1::InvalidJson)?;
        validate_evidence(&evidence)?;
        if evidence.to_canonical_bytes()? != bytes {
            return Err(VirtualHostLifetimeCodecErrorV1::NonCanonical);
        }
        Ok(evidence)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualHostLifetimeCaptureErrorV1 {
    InvalidLimit(&'static str),
    ForeignBuffer,
    UnknownBuffer,
    ReleasedBuffer,
    BufferNotRetained,
    InconsistentRetention,
    SizeOverflow,
    Identity,
}

impl fmt::Display for VirtualHostLifetimeCaptureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "virtual host-lifetime evidence unavailable: {self:?}"
        )
    }
}

impl Error for VirtualHostLifetimeCaptureErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualHostLifetimeCodecErrorV1 {
    ByteLimit,
    InvalidJson,
    InvalidSchema,
    InvalidIdentity,
    InvalidFinding,
    InvalidBlocker,
    InvalidCompleteness,
    NonCanonical,
    EncodingFailure,
}

impl fmt::Display for VirtualHostLifetimeCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid virtual host-lifetime evidence: {self:?}"
        )
    }
}

impl Error for VirtualHostLifetimeCodecErrorV1 {}

impl VirtualRuntimeV1 {
    /// Captures a read-only explanation of why one host operation conflicts
    /// with outstanding virtual dispatch ownership.
    pub fn capture_host_lifetime_evidence_v1(
        &self,
        buffer: VirtualBufferHandleV1,
        operation: VirtualHostLifetimeOperationV1,
        limits: VirtualHostLifetimeEvidenceLimitsV1,
    ) -> Result<VirtualHostLifetimeEvidenceV1, VirtualHostLifetimeCaptureErrorV1> {
        if buffer.runtime_identity() != self.config.runtime_identity {
            return Err(VirtualHostLifetimeCaptureErrorV1::ForeignBuffer);
        }
        let buffer_index = self
            .buffers
            .iter()
            .position(|record| record.handle == buffer)
            .ok_or(VirtualHostLifetimeCaptureErrorV1::UnknownBuffer)?;
        let record = &self.buffers[buffer_index];
        if record.released {
            return Err(VirtualHostLifetimeCaptureErrorV1::ReleasedBuffer);
        }
        if record.retained_dispatches == 0 {
            return Err(VirtualHostLifetimeCaptureErrorV1::BufferNotRetained);
        }

        let mut matching = self
            .dispatches
            .iter()
            .filter(|dispatch| {
                dispatch.retained_buffers.contains(&buffer_index)
                    && matches!(
                        dispatch.state,
                        VirtualCompletionStateV1::Prepared | VirtualCompletionStateV1::Ambiguous
                    )
            })
            .collect::<Vec<_>>();
        matching.sort_by_key(|dispatch| dispatch.completion.ordinal());
        if matching.len() != record.retained_dispatches {
            return Err(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention);
        }

        let total_blockers = u64::try_from(matching.len())
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
        let runtime_identity = identity_from_digest(*self.config.runtime_identity.as_bytes())
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?;
        let mut remaining_input_bytes = limits.max_input_bytes_hashed;
        let mut input_incomplete = false;
        let mut blockers = Vec::new();
        blockers
            .try_reserve_exact(matching.len().min(limits.max_blockers))
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
        for dispatch in matching.iter().take(limits.max_blockers) {
            let (dispatch_input, consumed) =
                self.dispatch_input_binding_v1(dispatch, remaining_input_bytes)?;
            if matches!(
                dispatch_input,
                VirtualDispatchInputBindingV1::Unavailable { .. }
            ) {
                input_incomplete = true;
            }
            remaining_input_bytes = remaining_input_bytes.saturating_sub(consumed);
            let state = match dispatch.state {
                VirtualCompletionStateV1::Prepared => VirtualBlockingCompletionStateV1::Prepared,
                VirtualCompletionStateV1::Ambiguous => VirtualBlockingCompletionStateV1::Ambiguous,
                _ => return Err(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention),
            };
            let ambiguity = dispatch.ambiguity.map(|value| match value {
                VirtualCompletionAmbiguityV1::PublicationOutcomeUnknown => {
                    VirtualBlockingCompletionAmbiguityV1::PublicationOutcomeUnknown
                }
                VirtualCompletionAmbiguityV1::WaitDeadlineExpired => {
                    VirtualBlockingCompletionAmbiguityV1::WaitDeadlineExpired
                }
            });
            let module = self
                .modules
                .iter()
                .find(|module| module.handle == dispatch.module)
                .ok_or(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
            let mut blocker = VirtualHostLifetimeBlockerV1 {
                completion_ordinal: dispatch.completion.ordinal(),
                queue_ordinal: dispatch.queue.ordinal(),
                module_ordinal: dispatch.module.ordinal(),
                state,
                ambiguity,
                kir: (*module.module.identity())
                    .try_into()
                    .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?,
                dispatch_input,
                blocker_identity: identity_from_digest([1; 32])
                    .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?,
            };
            blocker.blocker_identity =
                blocker_identity(runtime_identity, buffer.ordinal(), &blocker)
                    .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?;
            blockers.push(blocker);
        }

        let blocker_incomplete = matching.len() > blockers.len();
        let retained_blockers = u64::try_from(blockers.len())
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
        let completeness = match (blocker_incomplete, input_incomplete) {
            (false, false) => VirtualHostLifetimeCompletenessV1::Complete,
            (true, false) => VirtualHostLifetimeCompletenessV1::PartialBlockerLimit {
                total_blockers,
                retained_blockers,
            },
            (false, true) => VirtualHostLifetimeCompletenessV1::PartialInputIdentity {
                total_blockers,
                retained_blockers,
            },
            (true, true) => VirtualHostLifetimeCompletenessV1::PartialBlockerAndInputIdentity {
                total_blockers,
                retained_blockers,
            },
        };
        let finding = match operation {
            VirtualHostLifetimeOperationV1::ReleaseBuffer => {
                VirtualHostLifetimeFindingV1::ReleaseWhileRetained
            }
            VirtualHostLifetimeOperationV1::CopyFromHost
            | VirtualHostLifetimeOperationV1::CopyToHost
            | VirtualHostLifetimeOperationV1::SnapshotBuffer => {
                VirtualHostLifetimeFindingV1::HostAccessWhileRetained
            }
        };
        let mut evidence = VirtualHostLifetimeEvidenceV1 {
            schema: VIRTUAL_HOST_LIFETIME_EVIDENCE_SCHEMA_V1.to_owned(),
            runtime_identity,
            buffer_ordinal: buffer.ordinal(),
            operation,
            finding,
            truth: VirtualHostLifetimeTruthV1::DeclaredAttemptAgainstObservedVirtualState,
            authority:
                VirtualHostLifetimeAuthorityV1::AdvisorySimulationEvidenceNoExecutionOrHardwareAuthority,
            buffer_access: record.access.into(),
            retained_dispatches: u64::try_from(record.retained_dispatches)
                .map_err(|_| VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?,
            blockers,
            completeness,
            incident_identity: identity_from_digest([1; 32])
                .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?,
        };
        evidence.incident_identity = incident_identity(&evidence)
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?;
        validate_evidence(&evidence)
            .map_err(|_| VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
        Ok(evidence)
    }

    fn dispatch_input_binding_v1(
        &self,
        dispatch: &DispatchRecordLocalV1,
        remaining_limit: usize,
    ) -> Result<(VirtualDispatchInputBindingV1, usize), VirtualHostLifetimeCaptureErrorV1> {
        let mut required = 0_usize;
        for argument in &dispatch.request.arguments {
            if let VirtualArgumentV1::Buffer {
                element, elements, ..
            } = argument
            {
                let bytes = virtual_scalar_bytes(*element)
                    .and_then(|width| width.checked_mul(*elements))
                    .and_then(|bytes| bytes.checked_mul(2))
                    .ok_or(VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
                required = required
                    .checked_add(bytes)
                    .ok_or(VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
            }
        }
        if required > remaining_limit {
            return Ok((
                VirtualDispatchInputBindingV1::Unavailable {
                    reason: VirtualDispatchInputUnavailableReasonV1::SnapshotByteLimit,
                    required_snapshot_bytes: required as u64,
                    limit: remaining_limit as u64,
                },
                0,
            ));
        }

        let mut hash = Sha256::new();
        hash.update(DISPATCH_INPUT_DOMAIN_V1);
        hash.update(self.config.runtime_identity.as_bytes());
        hash.update(dispatch.completion.ordinal().to_le_bytes());
        hash.update(dispatch.queue.ordinal().to_le_bytes());
        hash.update(dispatch.module.ordinal().to_le_bytes());
        hash_bytes(&mut hash, dispatch.request.kernel.as_str().as_bytes());
        for value in dispatch.request.grid {
            hash.update(value.to_le_bytes());
        }
        for value in dispatch.request.workgroup {
            hash.update(value.to_le_bytes());
        }
        hash.update((dispatch.request.arguments.len() as u64).to_le_bytes());
        for argument in &dispatch.request.arguments {
            match argument {
                VirtualArgumentV1::Scalar(value) => {
                    hash.update([0, scalar_type_tag(value.ty())]);
                    hash.update(value.bits().to_le_bytes());
                }
                VirtualArgumentV1::Buffer {
                    buffer,
                    element,
                    access,
                    alignment,
                    byte_offset,
                    elements,
                } => {
                    hash.update([1, scalar_type_tag(*element), access_mode_tag(*access)]);
                    hash.update(buffer.runtime_identity().as_bytes());
                    hash.update(buffer.ordinal().to_le_bytes());
                    hash.update(alignment.to_le_bytes());
                    hash.update((*byte_offset as u64).to_le_bytes());
                    hash.update((*elements as u64).to_le_bytes());
                    let width = virtual_scalar_bytes(*element)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
                    let byte_len = width
                        .checked_mul(*elements)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
                    let end = byte_offset
                        .checked_add(byte_len)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::SizeOverflow)?;
                    let buffer_index = self
                        .buffers
                        .iter()
                        .position(|record| record.handle == *buffer)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
                    let buffer_record = &self.buffers[buffer_index];
                    let bytes = buffer_record
                        .bytes
                        .get(*byte_offset..end)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
                    let initialized = buffer_record
                        .initialized
                        .get(*byte_offset..end)
                        .ok_or(VirtualHostLifetimeCaptureErrorV1::InconsistentRetention)?;
                    hash_bytes(&mut hash, bytes);
                    hash.update((initialized.len() as u64).to_le_bytes());
                    for initialized in initialized {
                        hash.update([u8::from(*initialized)]);
                    }
                }
            }
        }
        hash.update((dispatch.request.dependencies.len() as u64).to_le_bytes());
        for dependency in &dispatch.request.dependencies {
            hash.update(dependency.runtime_identity().as_bytes());
            hash.update(dependency.ordinal().to_le_bytes());
        }
        match dispatch.dynamic_workgroup_memory {
            Some(dynamic) => {
                hash.update([1]);
                hash.update(dynamic.byte_extent().to_le_bytes());
            }
            None => hash.update([0]),
        }
        Ok((
            VirtualDispatchInputBindingV1::Exact {
                identity: identity_from_digest(hash.finalize().into())
                    .map_err(|_| VirtualHostLifetimeCaptureErrorV1::Identity)?,
                snapshot_bytes_hashed: required as u64,
            },
            required,
        ))
    }
}

fn blocker_identity(
    runtime_identity: VirtualEvidenceIdentityV1,
    buffer_ordinal: u64,
    blocker: &VirtualHostLifetimeBlockerV1,
) -> Result<VirtualEvidenceIdentityV1, VirtualHostLifetimeCodecErrorV1> {
    #[derive(Serialize)]
    struct Preimage<'a> {
        runtime_identity: VirtualEvidenceIdentityV1,
        buffer_ordinal: u64,
        completion_ordinal: u64,
        queue_ordinal: u64,
        module_ordinal: u64,
        state: VirtualBlockingCompletionStateV1,
        ambiguity: Option<VirtualBlockingCompletionAmbiguityV1>,
        kir: VirtualKirEvidenceReferenceV1,
        dispatch_input: &'a VirtualDispatchInputBindingV1,
    }
    content_identity(
        BLOCKER_DOMAIN_V1,
        &Preimage {
            runtime_identity,
            buffer_ordinal,
            completion_ordinal: blocker.completion_ordinal,
            queue_ordinal: blocker.queue_ordinal,
            module_ordinal: blocker.module_ordinal,
            state: blocker.state,
            ambiguity: blocker.ambiguity,
            kir: blocker.kir,
            dispatch_input: &blocker.dispatch_input,
        },
    )
}

fn incident_identity(
    evidence: &VirtualHostLifetimeEvidenceV1,
) -> Result<VirtualEvidenceIdentityV1, VirtualHostLifetimeCodecErrorV1> {
    #[derive(Serialize)]
    struct Preimage<'a> {
        schema: &'a str,
        runtime_identity: VirtualEvidenceIdentityV1,
        buffer_ordinal: u64,
        operation: VirtualHostLifetimeOperationV1,
        finding: VirtualHostLifetimeFindingV1,
        truth: VirtualHostLifetimeTruthV1,
        authority: VirtualHostLifetimeAuthorityV1,
        buffer_access: VirtualBufferAccessEvidenceV1,
        retained_dispatches: u64,
        blockers: &'a [VirtualHostLifetimeBlockerV1],
        completeness: VirtualHostLifetimeCompletenessV1,
    }
    content_identity(
        INCIDENT_DOMAIN_V1,
        &Preimage {
            schema: &evidence.schema,
            runtime_identity: evidence.runtime_identity,
            buffer_ordinal: evidence.buffer_ordinal,
            operation: evidence.operation,
            finding: evidence.finding,
            truth: evidence.truth,
            authority: evidence.authority,
            buffer_access: evidence.buffer_access,
            retained_dispatches: evidence.retained_dispatches,
            blockers: &evidence.blockers,
            completeness: evidence.completeness,
        },
    )
}

fn validate_evidence(
    evidence: &VirtualHostLifetimeEvidenceV1,
) -> Result<(), VirtualHostLifetimeCodecErrorV1> {
    if evidence.schema != VIRTUAL_HOST_LIFETIME_EVIDENCE_SCHEMA_V1
        || evidence.buffer_ordinal == 0
        || evidence.retained_dispatches == 0
        || evidence.blockers.is_empty()
        || evidence.blockers.len() > MAX_VIRTUAL_HOST_LIFETIME_BLOCKERS_V1
    {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidSchema);
    }
    let expected_finding = match evidence.operation {
        VirtualHostLifetimeOperationV1::ReleaseBuffer => {
            VirtualHostLifetimeFindingV1::ReleaseWhileRetained
        }
        VirtualHostLifetimeOperationV1::CopyFromHost
        | VirtualHostLifetimeOperationV1::CopyToHost
        | VirtualHostLifetimeOperationV1::SnapshotBuffer => {
            VirtualHostLifetimeFindingV1::HostAccessWhileRetained
        }
    };
    if evidence.finding != expected_finding {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidFinding);
    }
    if evidence
        .blockers
        .windows(2)
        .any(|pair| pair[0].completion_ordinal >= pair[1].completion_ordinal)
    {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidBlocker);
    }
    for blocker in &evidence.blockers {
        if blocker.completion_ordinal == 0
            || blocker.queue_ordinal == 0
            || blocker.module_ordinal == 0
            || blocker.kir.wire_version == 0
            || blocker.kir.canonical_bytes == 0
            || blocker.blocker_identity
                != blocker_identity(evidence.runtime_identity, evidence.buffer_ordinal, blocker)?
            || !matches!(
                (blocker.state, blocker.ambiguity),
                (VirtualBlockingCompletionStateV1::Prepared, None)
                    | (VirtualBlockingCompletionStateV1::Ambiguous, Some(_))
            )
        {
            return Err(VirtualHostLifetimeCodecErrorV1::InvalidBlocker);
        }
        match blocker.dispatch_input {
            VirtualDispatchInputBindingV1::Exact {
                snapshot_bytes_hashed,
                ..
            } if snapshot_bytes_hashed <= MAX_VIRTUAL_HOST_LIFETIME_INPUT_BYTES_V1 as u64 => {}
            VirtualDispatchInputBindingV1::Unavailable {
                required_snapshot_bytes,
                limit,
                ..
            } if required_snapshot_bytes > limit => {}
            _ => return Err(VirtualHostLifetimeCodecErrorV1::InvalidBlocker),
        }
    }
    let retained = evidence.blockers.len() as u64;
    let input_incomplete = evidence.blockers.iter().any(|blocker| {
        matches!(
            blocker.dispatch_input,
            VirtualDispatchInputBindingV1::Unavailable { .. }
        )
    });
    let completeness_valid = match evidence.completeness {
        VirtualHostLifetimeCompletenessV1::Complete => {
            retained == evidence.retained_dispatches && !input_incomplete
        }
        VirtualHostLifetimeCompletenessV1::PartialBlockerLimit {
            total_blockers,
            retained_blockers,
        } => {
            total_blockers == evidence.retained_dispatches
                && retained_blockers == retained
                && retained < total_blockers
                && !input_incomplete
        }
        VirtualHostLifetimeCompletenessV1::PartialInputIdentity {
            total_blockers,
            retained_blockers,
        } => {
            total_blockers == evidence.retained_dispatches
                && retained_blockers == retained
                && retained == total_blockers
                && input_incomplete
        }
        VirtualHostLifetimeCompletenessV1::PartialBlockerAndInputIdentity {
            total_blockers,
            retained_blockers,
        } => {
            total_blockers == evidence.retained_dispatches
                && retained_blockers == retained
                && retained < total_blockers
                && input_incomplete
        }
    };
    if !completeness_valid {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidCompleteness);
    }
    if evidence.incident_identity != incident_identity(evidence)? {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidIdentity);
    }
    Ok(())
}

fn content_identity<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<VirtualEvidenceIdentityV1, VirtualHostLifetimeCodecErrorV1> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| VirtualHostLifetimeCodecErrorV1::EncodingFailure)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    identity_from_digest(hash.finalize().into())
}

fn identity_from_digest(
    digest: [u8; 32],
) -> Result<VirtualEvidenceIdentityV1, VirtualHostLifetimeCodecErrorV1> {
    VirtualEvidenceIdentityV1::new(digest)
}

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

const fn scalar_type_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}

const fn access_mode_tag(value: AccessMode) -> u8 {
    match value {
        AccessMode::ReadOnly => 0,
        AccessMode::WriteOnly => 1,
        AccessMode::ReadWrite => 2,
    }
}

fn encode_hex(input: &[u8], output: &mut [u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in input.iter().copied().enumerate() {
        output[index * 2] = DIGITS[(byte >> 4) as usize];
        output[index * 2 + 1] = DIGITS[(byte & 0x0f) as usize];
    }
}

fn decode_hex_identity(value: &str) -> Result<[u8; 32], VirtualHostLifetimeCodecErrorV1> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(VirtualHostLifetimeCodecErrorV1::InvalidIdentity);
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?;
    }
    Ok(decoded)
}

fn decode_nibble(value: u8) -> Result<u8, VirtualHostLifetimeCodecErrorV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(VirtualHostLifetimeCodecErrorV1::InvalidIdentity),
    }
}
