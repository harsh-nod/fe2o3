//! Cooperative target telemetry for exact native dispatch publication.
//!
//! V2 is a distinct fixed-size wire contract. V1 bytes and lifecycle remain
//! unchanged. Native KFD identifiers in `NativeDispatchPublished` are private
//! debugger correlation inputs, never durable identity or operation authority.

use std::fmt;
use std::io::IoSliceMut;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;

use rustix::net::sockopt::{set_socket_passcred, socket_passcred, socket_peercred};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendFlags,
    SocketFlags, SocketType, recvmsg, send, socketpair,
};
use sha2::{Digest, Sha256};

use super::target_debug_telemetry_v1::{
    KfdTargetDebugArtifactIdentityV1, KfdTargetDebugSessionNonceV1,
    KfdTargetDebugTelemetryDigestV1, KfdTargetDebugTelemetryProcessErrorV1,
    KfdTargetDebugTelemetryProcessV1, decode_canonical_decimal_v1, decode_nonce_hex_v1,
    duplicate_raw_descriptor_cloexec_v1, protect_raw_descriptor_v1, validate_connected_seqpacket,
};

pub const KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2: usize = 384;
pub const KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V2: &str = "FE2O3_KFD_DEBUG_TELEMETRY_FD_V2";
pub const KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V2: &str = "FE2O3_KFD_DEBUG_TELEMETRY_NONCE_V2";
pub const KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V2: &str =
    "FE2O3_KFD_DEBUG_TELEMETRY_DEBUGGER_PID_V2";

const MAGIC_V2: [u8; 8] = *b"F2KDTL2\0";
const VERSION_V2: u16 = 2;
const HEADER_LEN_V2: usize = 56;
const CHECKSUM_OFFSET_V2: usize = 352;
const PAYLOAD_CAPACITY_V2: usize = CHECKSUM_OFFSET_V2 - HEADER_LEN_V2;
const CHECKSUM_DOMAIN_V2: &[u8] = b"fe2o3-kfd-target-debug-telemetry-record-v2\0";

/// Nonzero generation derived independently by both ends of one V2 session.
pub fn derive_kfd_target_debug_generation_v2(nonce: KfdTargetDebugSessionNonceV1) -> u64 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-kfd-target-debug-generation-v2\0");
    digest.update(nonce.as_bytes());
    let bytes: [u8; 32] = digest.finalize().into();
    let generation = u64::from_le_bytes(bytes[..8].try_into().expect("fixed digest prefix"));
    generation.max(1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum KfdTargetDebugSessionOutcomeV2 {
    Completed = 1,
    Failed = 2,
    Cancelled = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum KfdTargetDebugTelemetryPayloadV2 {
    /// Target declaration derived from an independently authorized prepared request.
    DispatchDeclared {
        process_instance: KfdTargetDebugTelemetryDigestV1,
        executable: KfdTargetDebugArtifactIdentityV1,
        artifact: KfdTargetDebugArtifactIdentityV1,
        dispatch: KfdTargetDebugTelemetryDigestV1,
        kernel: KfdTargetDebugTelemetryDigestV1,
        logical_queue: KfdTargetDebugTelemetryDigestV1,
        grid: [u32; 3],
        workgroup: [u32; 3],
        dynamic_shared_memory_bytes: u32,
        generation: u64,
    },
    /// KFD operation observed inside the target immediately after AQL publication.
    NativeDispatchPublished {
        process_instance: KfdTargetDebugTelemetryDigestV1,
        queue_occurrence: KfdTargetDebugTelemetryDigestV1,
        dispatch: KfdTargetDebugTelemetryDigestV1,
        artifact: KfdTargetDebugTelemetryDigestV1,
        generation: u64,
        target_kfd_gpu_id_observation: u32,
        target_kfd_queue_id_observation: u32,
        target_aql_packet_id_observation: u64,
        grid: [u32; 3],
        workgroup: [u32; 3],
    },
    SessionEnded {
        outcome: KfdTargetDebugSessionOutcomeV2,
    },
}

/// Exact logical declaration supplied by the safe authorized runtime composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KfdTargetDebugDispatchDeclarationV2 {
    process_instance: KfdTargetDebugTelemetryDigestV1,
    executable: KfdTargetDebugArtifactIdentityV1,
    artifact: KfdTargetDebugArtifactIdentityV1,
    dispatch: KfdTargetDebugTelemetryDigestV1,
    kernel: KfdTargetDebugTelemetryDigestV1,
    logical_queue: KfdTargetDebugTelemetryDigestV1,
    grid: [u32; 3],
    workgroup: [u32; 3],
    dynamic_shared_memory_bytes: u32,
    generation: u64,
}

impl KfdTargetDebugDispatchDeclarationV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        process_instance: KfdTargetDebugTelemetryDigestV1,
        executable: KfdTargetDebugArtifactIdentityV1,
        artifact: KfdTargetDebugArtifactIdentityV1,
        dispatch: KfdTargetDebugTelemetryDigestV1,
        kernel: KfdTargetDebugTelemetryDigestV1,
        logical_queue: KfdTargetDebugTelemetryDigestV1,
        grid: [u32; 3],
        workgroup: [u32; 3],
        dynamic_shared_memory_bytes: u32,
        generation: u64,
    ) -> Result<Self, KfdTargetDebugTelemetryDataErrorV2> {
        let value = Self {
            process_instance,
            executable,
            artifact,
            dispatch,
            kernel,
            logical_queue,
            grid,
            workgroup,
            dynamic_shared_memory_bytes,
            generation,
        };
        value.payload().validate()?;
        Ok(value)
    }

    fn payload(&self) -> KfdTargetDebugTelemetryPayloadV2 {
        KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance: self.process_instance,
            executable: self.executable,
            artifact: self.artifact,
            dispatch: self.dispatch,
            kernel: self.kernel,
            logical_queue: self.logical_queue,
            grid: self.grid,
            workgroup: self.workgroup,
            dynamic_shared_memory_bytes: self.dynamic_shared_memory_bytes,
            generation: self.generation,
        }
    }
}

/// Linear target-side sink consumed by one direct-KFD debug dispatch.
///
/// The queue owner invokes the publication method exactly once. No callback,
/// native descriptor, address, queue operation, or packet publication
/// authority is exposed to the safe runtime caller.
#[must_use = "native telemetry must be consumed by one direct-KFD debug dispatch"]
pub struct KfdNativeDispatchTelemetrySinkV2 {
    endpoint: KfdCooperativeTargetTelemetryEndpointV2,
    declaration: KfdTargetDebugDispatchDeclarationV2,
    declared: bool,
    published: bool,
}

/// One-shot terminal record authority returned only after exact native publication.
///
/// It carries no queue, packet, descriptor, address, or stop-control authority.
#[must_use = "a published native dispatch requires one terminal record"]
pub struct KfdNativeDispatchTerminalV2 {
    sink: KfdNativeDispatchTelemetrySinkV2,
}

impl fmt::Debug for KfdNativeDispatchTerminalV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdNativeDispatchTerminalV2")
            .field("native_authority", &"NONE")
            .finish_non_exhaustive()
    }
}

impl KfdNativeDispatchTerminalV2 {
    pub(crate) fn from_published(
        sink: KfdNativeDispatchTelemetrySinkV2,
    ) -> Result<Self, KfdTargetDebugTelemetryTransportErrorV2> {
        if !sink.declared || !sink.published {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::InvalidSinkState);
        }
        Ok(Self { sink })
    }

    pub fn finish_completed(mut self) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        self.sink.emit_completed()
    }

    pub fn finish_failed(mut self) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        self.sink.emit_failed()
    }
}

impl fmt::Debug for KfdNativeDispatchTelemetrySinkV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdNativeDispatchTelemetrySinkV2")
            .field("declaration", &self.declaration)
            .field("declared", &self.declared)
            .field("published", &self.published)
            .field("native_authority", &"REDACTED")
            .finish_non_exhaustive()
    }
}

impl KfdNativeDispatchTelemetrySinkV2 {
    pub const fn new(
        endpoint: KfdCooperativeTargetTelemetryEndpointV2,
        declaration: KfdTargetDebugDispatchDeclarationV2,
    ) -> Self {
        Self {
            endpoint,
            declaration,
            declared: false,
            published: false,
        }
    }

    pub(crate) fn emit_declaration(
        &mut self,
    ) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        if self.declared || self.published {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::InvalidSinkState);
        }
        self.endpoint.send(self.declaration.payload())?;
        self.declared = true;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_native_publication(
        &mut self,
        queue_occurrence: KfdTargetDebugTelemetryDigestV1,
        target_kfd_gpu_id_observation: u32,
        target_kfd_queue_id_observation: u32,
        target_aql_packet_id_observation: u64,
    ) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        if !self.declared || self.published {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::InvalidSinkState);
        }
        self.endpoint
            .send(KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
                process_instance: self.declaration.process_instance,
                queue_occurrence,
                dispatch: self.declaration.dispatch,
                artifact: self.declaration.artifact.digest(),
                generation: self.declaration.generation,
                target_kfd_gpu_id_observation,
                target_kfd_queue_id_observation,
                target_aql_packet_id_observation,
                grid: self.declaration.grid,
                workgroup: self.declaration.workgroup,
            })?;
        self.published = true;
        Ok(())
    }

    pub(crate) fn emit_completed(&mut self) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        if !self.published {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::InvalidSinkState);
        }
        self.endpoint
            .send(KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Completed,
            })?;
        Ok(())
    }

    pub(crate) fn emit_failed(&mut self) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        if !self.declared {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::InvalidSinkState);
        }
        self.endpoint
            .send(KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Failed,
            })?;
        Ok(())
    }
}

impl KfdTargetDebugTelemetryPayloadV2 {
    pub fn validate(&self) -> Result<(), KfdTargetDebugTelemetryDataErrorV2> {
        match self {
            Self::DispatchDeclared {
                grid,
                workgroup,
                generation,
                ..
            }
            | Self::NativeDispatchPublished {
                grid,
                workgroup,
                generation,
                ..
            } => {
                if *generation == 0 {
                    return Err(KfdTargetDebugTelemetryDataErrorV2::ZeroGeneration);
                }
                validate_geometry(*grid, *workgroup)?;
            }
            Self::SessionEnded { .. } => {}
        }
        Ok(())
    }

    fn kind(&self) -> u16 {
        match self {
            Self::DispatchDeclared { .. } => 1,
            Self::NativeDispatchPublished { .. } => 2,
            Self::SessionEnded { .. } => 3,
        }
    }

    fn encode(&self, output: &mut [u8; PAYLOAD_CAPACITY_V2]) -> usize {
        match self {
            Self::DispatchDeclared {
                process_instance,
                executable,
                artifact,
                dispatch,
                kernel,
                logical_queue,
                grid,
                workgroup,
                dynamic_shared_memory_bytes,
                generation,
            } => {
                output[..32].copy_from_slice(process_instance.as_bytes());
                put_artifact(&mut output[32..72], *executable);
                put_artifact(&mut output[72..112], *artifact);
                output[112..144].copy_from_slice(dispatch.as_bytes());
                output[144..176].copy_from_slice(kernel.as_bytes());
                output[176..208].copy_from_slice(logical_queue.as_bytes());
                put_triplet(&mut output[208..220], *grid);
                put_triplet(&mut output[220..232], *workgroup);
                output[232..236].copy_from_slice(&dynamic_shared_memory_bytes.to_le_bytes());
                output[240..248].copy_from_slice(&generation.to_le_bytes());
                248
            }
            Self::NativeDispatchPublished {
                process_instance,
                queue_occurrence,
                dispatch,
                artifact,
                generation,
                target_kfd_gpu_id_observation,
                target_kfd_queue_id_observation,
                target_aql_packet_id_observation,
                grid,
                workgroup,
            } => {
                output[..32].copy_from_slice(process_instance.as_bytes());
                output[32..64].copy_from_slice(queue_occurrence.as_bytes());
                output[64..96].copy_from_slice(dispatch.as_bytes());
                output[96..128].copy_from_slice(artifact.as_bytes());
                output[128..136].copy_from_slice(&generation.to_le_bytes());
                output[136..140].copy_from_slice(&target_kfd_gpu_id_observation.to_le_bytes());
                output[140..144].copy_from_slice(&target_kfd_queue_id_observation.to_le_bytes());
                output[144..152].copy_from_slice(&target_aql_packet_id_observation.to_le_bytes());
                put_triplet(&mut output[152..164], *grid);
                put_triplet(&mut output[164..176], *workgroup);
                176
            }
            Self::SessionEnded { outcome } => {
                output[..2].copy_from_slice(&(*outcome as u16).to_le_bytes());
                2
            }
        }
    }

    fn decode(kind: u16, bytes: &[u8]) -> Result<Self, KfdTargetDebugTelemetryProtocolErrorV2> {
        let payload = match kind {
            1 => {
                require_len(bytes, 248)?;
                require_zero(&bytes[236..240])?;
                Self::DispatchDeclared {
                    process_instance: digest(&bytes[..32])?,
                    executable: artifact(&bytes[32..72])?,
                    artifact: artifact(&bytes[72..112])?,
                    dispatch: digest(&bytes[112..144])?,
                    kernel: digest(&bytes[144..176])?,
                    logical_queue: digest(&bytes[176..208])?,
                    grid: triplet(&bytes[208..220]),
                    workgroup: triplet(&bytes[220..232]),
                    dynamic_shared_memory_bytes: u32_value(&bytes[232..236]),
                    generation: u64_value(&bytes[240..248]),
                }
            }
            2 => {
                require_len(bytes, 176)?;
                Self::NativeDispatchPublished {
                    process_instance: digest(&bytes[..32])?,
                    queue_occurrence: digest(&bytes[32..64])?,
                    dispatch: digest(&bytes[64..96])?,
                    artifact: digest(&bytes[96..128])?,
                    generation: u64_value(&bytes[128..136]),
                    target_kfd_gpu_id_observation: u32_value(&bytes[136..140]),
                    target_kfd_queue_id_observation: u32_value(&bytes[140..144]),
                    target_aql_packet_id_observation: u64_value(&bytes[144..152]),
                    grid: triplet(&bytes[152..164]),
                    workgroup: triplet(&bytes[164..176]),
                }
            }
            3 => {
                require_len(bytes, 2)?;
                let outcome = match u16_value(bytes) {
                    1 => KfdTargetDebugSessionOutcomeV2::Completed,
                    2 => KfdTargetDebugSessionOutcomeV2::Failed,
                    3 => KfdTargetDebugSessionOutcomeV2::Cancelled,
                    _ => return Err(KfdTargetDebugTelemetryProtocolErrorV2::InvalidEnum),
                };
                Self::SessionEnded { outcome }
            }
            _ => return Err(KfdTargetDebugTelemetryProtocolErrorV2::UnknownKind),
        };
        payload
            .validate()
            .map_err(KfdTargetDebugTelemetryProtocolErrorV2::InvalidPayload)?;
        Ok(payload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KfdTargetDebugTelemetryRecordV2 {
    sequence: u64,
    nonce: KfdTargetDebugSessionNonceV1,
    payload: KfdTargetDebugTelemetryPayloadV2,
}

impl KfdTargetDebugTelemetryRecordV2 {
    pub fn new(
        sequence: u64,
        nonce: KfdTargetDebugSessionNonceV1,
        payload: KfdTargetDebugTelemetryPayloadV2,
    ) -> Result<Self, KfdTargetDebugTelemetryDataErrorV2> {
        payload.validate()?;
        Ok(Self {
            sequence,
            nonce,
            payload,
        })
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn session_nonce(&self) -> KfdTargetDebugSessionNonceV1 {
        self.nonce
    }

    pub const fn payload(&self) -> &KfdTargetDebugTelemetryPayloadV2 {
        &self.payload
    }

    pub fn to_wire_bytes(&self) -> [u8; KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2] {
        let mut bytes = [0_u8; KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2];
        bytes[..8].copy_from_slice(&MAGIC_V2);
        bytes[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.payload.kind().to_le_bytes());
        bytes[16..24].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[24..56].copy_from_slice(self.nonce.as_bytes());
        let mut payload = [0_u8; PAYLOAD_CAPACITY_V2];
        let len = self.payload.encode(&mut payload);
        bytes[12..16].copy_from_slice(&(len as u32).to_le_bytes());
        bytes[HEADER_LEN_V2..CHECKSUM_OFFSET_V2].copy_from_slice(&payload);
        let checksum = checksum(&bytes[..CHECKSUM_OFFSET_V2]);
        bytes[CHECKSUM_OFFSET_V2..].copy_from_slice(&checksum);
        bytes
    }

    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, KfdTargetDebugTelemetryProtocolErrorV2> {
        if bytes.len() != KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2 {
            return Err(KfdTargetDebugTelemetryProtocolErrorV2::WireLength);
        }
        if bytes[..8] != MAGIC_V2 {
            return Err(KfdTargetDebugTelemetryProtocolErrorV2::Magic);
        }
        if u16_value(&bytes[8..10]) != VERSION_V2 {
            return Err(KfdTargetDebugTelemetryProtocolErrorV2::Version);
        }
        require_zero(
            &bytes[56
                + usize::try_from(u32_value(&bytes[12..16]))
                    .unwrap_or(usize::MAX)
                    .min(PAYLOAD_CAPACITY_V2)..CHECKSUM_OFFSET_V2],
        )?;
        if bytes[CHECKSUM_OFFSET_V2..] != checksum(&bytes[..CHECKSUM_OFFSET_V2]) {
            return Err(KfdTargetDebugTelemetryProtocolErrorV2::Checksum);
        }
        let payload_len = usize::try_from(u32_value(&bytes[12..16]))
            .map_err(|_| KfdTargetDebugTelemetryProtocolErrorV2::PayloadLength)?;
        if payload_len > PAYLOAD_CAPACITY_V2 {
            return Err(KfdTargetDebugTelemetryProtocolErrorV2::PayloadLength);
        }
        let payload = KfdTargetDebugTelemetryPayloadV2::decode(
            u16_value(&bytes[10..12]),
            &bytes[HEADER_LEN_V2..HEADER_LEN_V2 + payload_len],
        )?;
        Ok(Self {
            sequence: u64_value(&bytes[16..24]),
            nonce: KfdTargetDebugSessionNonceV1::from_bytes(
                bytes[24..56].try_into().expect("fixed nonce"),
            )
            .map_err(|_| KfdTargetDebugTelemetryProtocolErrorV2::InvalidNonce)?,
            payload,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleV2 {
    AwaitingDeclaration,
    AwaitingPublication,
    Published,
    Finished,
}

pub struct KfdDebuggerTelemetryEndpointV2 {
    endpoint: OwnedFd,
    nonce: KfdTargetDebugSessionNonceV1,
    target: KfdTargetDebugTelemetryProcessV1,
    next_sequence: u64,
    lifecycle: LifecycleV2,
    poisoned: bool,
}

impl KfdDebuggerTelemetryEndpointV2 {
    pub fn admit(
        endpoint: OwnedFd,
        nonce: KfdTargetDebugSessionNonceV1,
        target: KfdTargetDebugTelemetryProcessV1,
    ) -> Result<Self, KfdTargetDebugTelemetryTransportErrorV2> {
        target.validate_current()?;
        validate_connected_seqpacket(&endpoint)
            .map_err(|_| KfdTargetDebugTelemetryTransportErrorV2::SocketAdmission)?;
        set_socket_passcred(&endpoint, true)
            .map_err(KfdTargetDebugTelemetryTransportErrorV2::ConfigureCredentials)?;
        if !socket_passcred(&endpoint)
            .map_err(KfdTargetDebugTelemetryTransportErrorV2::InspectSocket)?
        {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::CredentialsDisabled);
        }
        Ok(Self {
            endpoint,
            nonce,
            target,
            next_sequence: 0,
            lifecycle: LifecycleV2::AwaitingDeclaration,
            poisoned: false,
        })
    }

    pub fn receive(
        &mut self,
    ) -> Result<KfdTargetDebugTelemetryRecordV2, KfdTargetDebugTelemetryTransportErrorV2> {
        self.receive_inner(RecvFlags::CMSG_CLOEXEC)?
            .ok_or(KfdTargetDebugTelemetryTransportErrorV2::EndpointNotReady)
    }

    pub fn try_receive(
        &mut self,
    ) -> Result<Option<KfdTargetDebugTelemetryRecordV2>, KfdTargetDebugTelemetryTransportErrorV2>
    {
        self.receive_inner(RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT)
    }

    fn receive_inner(
        &mut self,
        flags: RecvFlags,
    ) -> Result<Option<KfdTargetDebugTelemetryRecordV2>, KfdTargetDebugTelemetryTransportErrorV2>
    {
        if self.poisoned {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::Poisoned);
        }
        let bytes = match receive_packet(&self.endpoint, flags, self.target) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None),
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let record = KfdTargetDebugTelemetryRecordV2::from_wire_bytes(&bytes).map_err(|error| {
            self.poisoned = true;
            KfdTargetDebugTelemetryTransportErrorV2::Protocol(error)
        })?;
        if record.nonce != self.nonce {
            self.poisoned = true;
            return Err(KfdTargetDebugTelemetryTransportErrorV2::NonceMismatch);
        }
        self.admit_record(&record)?;
        Ok(Some(record))
    }

    fn admit_record(
        &mut self,
        record: &KfdTargetDebugTelemetryRecordV2,
    ) -> Result<(), KfdTargetDebugTelemetryTransportErrorV2> {
        if record.sequence != self.next_sequence {
            self.poisoned = true;
            return Err(KfdTargetDebugTelemetryTransportErrorV2::SequenceMismatch);
        }
        let next = next_lifecycle(self.lifecycle, &record.payload).map_err(|error| {
            self.poisoned = true;
            KfdTargetDebugTelemetryTransportErrorV2::Protocol(error)
        })?;
        self.next_sequence = self.next_sequence.checked_add(1).ok_or_else(|| {
            self.poisoned = true;
            KfdTargetDebugTelemetryTransportErrorV2::SequenceExhausted
        })?;
        self.lifecycle = next;
        Ok(())
    }
}

pub struct KfdCooperativeTargetTelemetryEndpointV2 {
    endpoint: OwnedFd,
    nonce: KfdTargetDebugSessionNonceV1,
    debugger: KfdTargetDebugTelemetryProcessV1,
    next_sequence: u64,
    lifecycle: LifecycleV2,
    poisoned: bool,
}

impl KfdCooperativeTargetTelemetryEndpointV2 {
    pub fn admit(
        endpoint: OwnedFd,
        nonce: KfdTargetDebugSessionNonceV1,
        debugger: KfdTargetDebugTelemetryProcessV1,
    ) -> Result<Self, KfdTargetDebugTelemetryTransportErrorV2> {
        debugger.validate_current()?;
        validate_connected_seqpacket(&endpoint)
            .map_err(|_| KfdTargetDebugTelemetryTransportErrorV2::SocketAdmission)?;
        let peer = socket_peercred(&endpoint)
            .map_err(KfdTargetDebugTelemetryTransportErrorV2::InspectSocket)?;
        if !debugger.matches_credentials(
            peer.pid.as_raw_pid(),
            peer.uid.as_raw(),
            peer.gid.as_raw(),
        ) {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::PeerCredentialMismatch);
        }
        Ok(Self {
            endpoint,
            nonce,
            debugger,
            next_sequence: 0,
            lifecycle: LifecycleV2::AwaitingDeclaration,
            poisoned: false,
        })
    }

    /// Session generation without exposing the retained correlation nonce.
    pub fn session_generation(&self) -> u64 {
        derive_kfd_target_debug_generation_v2(self.nonce)
    }

    pub fn send(
        &mut self,
        payload: KfdTargetDebugTelemetryPayloadV2,
    ) -> Result<KfdTargetDebugTelemetryRecordV2, KfdTargetDebugTelemetryTransportErrorV2> {
        if self.poisoned {
            return Err(KfdTargetDebugTelemetryTransportErrorV2::Poisoned);
        }
        payload.validate()?;
        let next = next_lifecycle(self.lifecycle, &payload)?;
        self.debugger.validate_current()?;
        let record = KfdTargetDebugTelemetryRecordV2::new(self.next_sequence, self.nonce, payload)?;
        let bytes = record.to_wire_bytes();
        loop {
            match send(&self.endpoint, &bytes, SendFlags::NOSIGNAL) {
                Ok(actual) if actual == bytes.len() => break,
                Ok(_) => {
                    self.poisoned = true;
                    return Err(KfdTargetDebugTelemetryTransportErrorV2::PartialSend);
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => {
                    self.poisoned = true;
                    return Err(KfdTargetDebugTelemetryTransportErrorV2::Send(error));
                }
            }
        }
        self.next_sequence = self.next_sequence.checked_add(1).ok_or_else(|| {
            self.poisoned = true;
            KfdTargetDebugTelemetryTransportErrorV2::SequenceExhausted
        })?;
        self.lifecycle = next;
        Ok(record)
    }
}

pub fn create_kfd_target_debug_telemetry_channel_v2()
-> Result<(OwnedFd, OwnedFd), KfdTargetDebugTelemetryTransportErrorV2> {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(KfdTargetDebugTelemetryTransportErrorV2::CreateChannel)
}

pub fn admit_inherited_kfd_target_debug_telemetry_v2()
-> Result<Option<KfdCooperativeTargetTelemetryEndpointV2>, KfdInheritedTargetDebugTelemetryErrorV2>
{
    let fd = std::env::var_os(KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V2);
    let nonce = std::env::var_os(KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V2);
    let debugger = std::env::var_os(KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V2);
    if fd.is_none() && nonce.is_none() && debugger.is_none() {
        return Ok(None);
    }
    let (fd, nonce, debugger) = match (fd, nonce, debugger) {
        (Some(fd), Some(nonce), Some(debugger)) => (fd, nonce, debugger),
        _ => return Err(KfdInheritedTargetDebugTelemetryErrorV2::IncompleteEnvironment),
    };
    let fd = fd
        .to_str()
        .and_then(decode_canonical_decimal_v1::<i32>)
        .filter(|value| *value >= 3)
        .ok_or(KfdInheritedTargetDebugTelemetryErrorV2::InvalidDescriptor)?;
    let nonce = nonce
        .to_str()
        .and_then(decode_nonce_hex_v1)
        .ok_or(KfdInheritedTargetDebugTelemetryErrorV2::InvalidNonce)
        .and_then(|value| {
            KfdTargetDebugSessionNonceV1::from_bytes(value)
                .map_err(|_| KfdInheritedTargetDebugTelemetryErrorV2::InvalidNonce)
        })?;
    let debugger = debugger
        .to_str()
        .and_then(decode_canonical_decimal_v1::<u32>)
        .ok_or(KfdInheritedTargetDebugTelemetryErrorV2::InvalidDebugger)
        .and_then(|pid| {
            KfdTargetDebugTelemetryProcessV1::capture(pid)
                .map_err(KfdInheritedTargetDebugTelemetryErrorV2::Process)
        })?;
    let endpoint = duplicate_raw_descriptor_cloexec_v1(fd)
        .map_err(KfdInheritedTargetDebugTelemetryErrorV2::Descriptor)?;
    let endpoint = KfdCooperativeTargetTelemetryEndpointV2::admit(endpoint, nonce, debugger)
        .map_err(KfdInheritedTargetDebugTelemetryErrorV2::Admit)?;
    protect_raw_descriptor_v1(fd).map_err(KfdInheritedTargetDebugTelemetryErrorV2::Descriptor)?;
    Ok(Some(endpoint))
}

fn next_lifecycle(
    current: LifecycleV2,
    payload: &KfdTargetDebugTelemetryPayloadV2,
) -> Result<LifecycleV2, KfdTargetDebugTelemetryProtocolErrorV2> {
    match (current, payload) {
        (
            LifecycleV2::AwaitingDeclaration,
            KfdTargetDebugTelemetryPayloadV2::DispatchDeclared { .. },
        ) => Ok(LifecycleV2::AwaitingPublication),
        (
            LifecycleV2::AwaitingPublication,
            KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished { .. },
        ) => Ok(LifecycleV2::Published),
        (
            LifecycleV2::Published,
            KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Completed,
            },
        )
        | (
            LifecycleV2::AwaitingPublication | LifecycleV2::Published,
            KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome:
                    KfdTargetDebugSessionOutcomeV2::Failed | KfdTargetDebugSessionOutcomeV2::Cancelled,
            },
        ) => Ok(LifecycleV2::Finished),
        (LifecycleV2::Finished, _) => Err(KfdTargetDebugTelemetryProtocolErrorV2::RecordAfterEnd),
        _ => Err(KfdTargetDebugTelemetryProtocolErrorV2::InvalidLifecycle),
    }
}

fn receive_packet(
    endpoint: &OwnedFd,
    flags: RecvFlags,
    target: KfdTargetDebugTelemetryProcessV1,
) -> Result<
    Option<[u8; KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2]>,
    KfdTargetDebugTelemetryTransportErrorV2,
> {
    let mut bytes = [0_u8; KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2];
    let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(4))];
    let mut control = RecvAncillaryBuffer::new(&mut storage);
    let mut vectors = [IoSliceMut::new(&mut bytes)];
    let message = loop {
        match recvmsg(endpoint, &mut vectors, &mut control, flags) {
            Ok(message) => break message,
            Err(rustix::io::Errno::INTR) => continue,
            Err(rustix::io::Errno::AGAIN) if flags.contains(RecvFlags::DONTWAIT) => {
                return Ok(None);
            }
            Err(error) => return Err(KfdTargetDebugTelemetryTransportErrorV2::Receive(error)),
        }
    };
    let mut credentials = None;
    let mut forbidden = false;
    for item in control.drain() {
        match item {
            RecvAncillaryMessage::ScmCredentials(value) => {
                if credentials.replace(value).is_some() {
                    forbidden = true;
                }
            }
            RecvAncillaryMessage::ScmRights(rights) => {
                drop(rights);
                forbidden = true;
            }
            _ => forbidden = true,
        }
    }
    if message
        .flags
        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    {
        return Err(KfdTargetDebugTelemetryTransportErrorV2::Truncated);
    }
    if forbidden {
        return Err(KfdTargetDebugTelemetryTransportErrorV2::ForbiddenAncillary);
    }
    if message.bytes != bytes.len() {
        return Err(KfdTargetDebugTelemetryTransportErrorV2::PacketLength);
    }
    let credentials =
        credentials.ok_or(KfdTargetDebugTelemetryTransportErrorV2::MissingCredentials)?;
    if !target.matches_credentials(
        credentials.pid.as_raw_pid(),
        credentials.uid.as_raw(),
        credentials.gid.as_raw(),
    ) {
        return Err(KfdTargetDebugTelemetryTransportErrorV2::PeerCredentialMismatch);
    }
    target.validate_after_authenticated_packet()?;
    Ok(Some(bytes))
}

fn validate_geometry(
    grid: [u32; 3],
    workgroup: [u32; 3],
) -> Result<(), KfdTargetDebugTelemetryDataErrorV2> {
    for axis in 0..3 {
        if grid[axis] == 0 || workgroup[axis] == 0 || workgroup[axis] > grid[axis] {
            return Err(KfdTargetDebugTelemetryDataErrorV2::InvalidGeometry);
        }
    }
    workgroup
        .into_iter()
        .try_fold(1_u32, u32::checked_mul)
        .filter(|volume| *volume <= 1_024)
        .ok_or(KfdTargetDebugTelemetryDataErrorV2::InvalidGeometry)?;
    Ok(())
}

fn put_artifact(output: &mut [u8], value: KfdTargetDebugArtifactIdentityV1) {
    output[..32].copy_from_slice(value.digest().as_bytes());
    output[32..40].copy_from_slice(&value.byte_length().to_le_bytes());
}

fn artifact(
    bytes: &[u8],
) -> Result<KfdTargetDebugArtifactIdentityV1, KfdTargetDebugTelemetryProtocolErrorV2> {
    KfdTargetDebugArtifactIdentityV1::new(digest(&bytes[..32])?, u64_value(&bytes[32..40]))
        .map_err(|_| KfdTargetDebugTelemetryProtocolErrorV2::InvalidArtifact)
}

fn digest(
    bytes: &[u8],
) -> Result<KfdTargetDebugTelemetryDigestV1, KfdTargetDebugTelemetryProtocolErrorV2> {
    KfdTargetDebugTelemetryDigestV1::from_bytes(bytes.try_into().expect("fixed digest"))
        .map_err(|_| KfdTargetDebugTelemetryProtocolErrorV2::InvalidDigest)
}

fn put_triplet(output: &mut [u8], values: [u32; 3]) {
    for (index, value) in values.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn triplet(bytes: &[u8]) -> [u32; 3] {
    [
        u32_value(&bytes[..4]),
        u32_value(&bytes[4..8]),
        u32_value(&bytes[8..12]),
    ]
}

fn require_len(
    bytes: &[u8],
    expected: usize,
) -> Result<(), KfdTargetDebugTelemetryProtocolErrorV2> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(KfdTargetDebugTelemetryProtocolErrorV2::PayloadLength)
    }
}

fn require_zero(bytes: &[u8]) -> Result<(), KfdTargetDebugTelemetryProtocolErrorV2> {
    if bytes.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(KfdTargetDebugTelemetryProtocolErrorV2::NonCanonical)
    }
}

fn u16_value(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().expect("fixed u16"))
}
fn u32_value(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("fixed u32"))
}
fn u64_value(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("fixed u64"))
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN_V2);
    digest.update(bytes);
    digest.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdTargetDebugTelemetryDataErrorV2 {
    ZeroGeneration,
    InvalidGeometry,
}

impl fmt::Display for KfdTargetDebugTelemetryDataErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid native telemetry V2 data: {self:?}")
    }
}
impl std::error::Error for KfdTargetDebugTelemetryDataErrorV2 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdTargetDebugTelemetryProtocolErrorV2 {
    WireLength,
    Magic,
    Version,
    UnknownKind,
    PayloadLength,
    Checksum,
    NonCanonical,
    InvalidNonce,
    InvalidDigest,
    InvalidArtifact,
    InvalidEnum,
    InvalidPayload(KfdTargetDebugTelemetryDataErrorV2),
    InvalidLifecycle,
    RecordAfterEnd,
}

impl fmt::Display for KfdTargetDebugTelemetryProtocolErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid native telemetry V2 protocol: {self:?}")
    }
}
impl std::error::Error for KfdTargetDebugTelemetryProtocolErrorV2 {}

#[derive(Debug)]
pub enum KfdTargetDebugTelemetryTransportErrorV2 {
    CreateChannel(rustix::io::Errno),
    ConfigureCredentials(rustix::io::Errno),
    InspectSocket(rustix::io::Errno),
    Send(rustix::io::Errno),
    Receive(rustix::io::Errno),
    Process(KfdTargetDebugTelemetryProcessErrorV1),
    Protocol(KfdTargetDebugTelemetryProtocolErrorV2),
    Data(KfdTargetDebugTelemetryDataErrorV2),
    SocketAdmission,
    CredentialsDisabled,
    PeerCredentialMismatch,
    MissingCredentials,
    ForbiddenAncillary,
    Truncated,
    PacketLength,
    PartialSend,
    EndpointNotReady,
    NonceMismatch,
    SequenceMismatch,
    SequenceExhausted,
    Poisoned,
    InvalidSinkState,
}

impl fmt::Display for KfdTargetDebugTelemetryTransportErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native telemetry V2 transport failed: {self:?}")
    }
}
impl std::error::Error for KfdTargetDebugTelemetryTransportErrorV2 {}

impl From<KfdTargetDebugTelemetryProcessErrorV1> for KfdTargetDebugTelemetryTransportErrorV2 {
    fn from(value: KfdTargetDebugTelemetryProcessErrorV1) -> Self {
        Self::Process(value)
    }
}
impl From<KfdTargetDebugTelemetryProtocolErrorV2> for KfdTargetDebugTelemetryTransportErrorV2 {
    fn from(value: KfdTargetDebugTelemetryProtocolErrorV2) -> Self {
        Self::Protocol(value)
    }
}
impl From<KfdTargetDebugTelemetryDataErrorV2> for KfdTargetDebugTelemetryTransportErrorV2 {
    fn from(value: KfdTargetDebugTelemetryDataErrorV2) -> Self {
        Self::Data(value)
    }
}

#[derive(Debug)]
pub enum KfdInheritedTargetDebugTelemetryErrorV2 {
    IncompleteEnvironment,
    InvalidDescriptor,
    InvalidNonce,
    InvalidDebugger,
    Descriptor(rustix::io::Errno),
    Process(KfdTargetDebugTelemetryProcessErrorV1),
    Admit(KfdTargetDebugTelemetryTransportErrorV2),
}

impl fmt::Display for KfdInheritedTargetDebugTelemetryErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "inherited native telemetry V2 failed: {self:?}")
    }
}
impl std::error::Error for KfdInheritedTargetDebugTelemetryErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_value(seed: u8) -> KfdTargetDebugTelemetryDigestV1 {
        KfdTargetDebugTelemetryDigestV1::from_bytes([seed; 32]).unwrap()
    }

    #[test]
    fn generation_is_nonce_bound_nonzero_and_stable() {
        let first = KfdTargetDebugSessionNonceV1::from_bytes([9; 32]).unwrap();
        let second = KfdTargetDebugSessionNonceV1::from_bytes([10; 32]).unwrap();
        assert_ne!(derive_kfd_target_debug_generation_v2(first), 0);
        assert_eq!(
            derive_kfd_target_debug_generation_v2(first),
            derive_kfd_target_debug_generation_v2(first)
        );
        assert_ne!(
            derive_kfd_target_debug_generation_v2(first),
            derive_kfd_target_debug_generation_v2(second)
        );
    }

    fn declaration() -> KfdTargetDebugTelemetryPayloadV2 {
        KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance: digest_value(1),
            executable: KfdTargetDebugArtifactIdentityV1::new(digest_value(2), 64).unwrap(),
            artifact: KfdTargetDebugArtifactIdentityV1::new(digest_value(3), 128).unwrap(),
            dispatch: digest_value(4),
            kernel: digest_value(5),
            logical_queue: digest_value(6),
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_memory_bytes: 0,
            generation: 7,
        }
    }

    fn published() -> KfdTargetDebugTelemetryPayloadV2 {
        KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
            process_instance: digest_value(1),
            queue_occurrence: digest_value(7),
            dispatch: digest_value(4),
            artifact: digest_value(3),
            generation: 7,
            target_kfd_gpu_id_observation: 35_090,
            target_kfd_queue_id_observation: 9,
            target_aql_packet_id_observation: 41,
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
        }
    }

    #[test]
    fn v2_wire_round_trips_and_v1_length_is_unchanged() {
        assert_eq!(
            super::super::target_debug_telemetry_v1::KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V1,
            256
        );
        let nonce = KfdTargetDebugSessionNonceV1::from_bytes([9; 32]).unwrap();
        for (sequence, payload) in [
            declaration(),
            published(),
            KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Completed,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let record =
                KfdTargetDebugTelemetryRecordV2::new(sequence as u64, nonce, payload).unwrap();
            let wire = record.to_wire_bytes();
            assert_eq!(wire.len(), KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V2);
            assert_eq!(
                KfdTargetDebugTelemetryRecordV2::from_wire_bytes(&wire).unwrap(),
                record
            );
        }
    }

    #[test]
    fn rejects_padding_checksum_geometry_and_generation_mutations() {
        let nonce = KfdTargetDebugSessionNonceV1::from_bytes([9; 32]).unwrap();
        let record = KfdTargetDebugTelemetryRecordV2::new(0, nonce, declaration()).unwrap();
        let mut padding = record.to_wire_bytes();
        padding[HEADER_LEN_V2 + 250] = 1;
        assert_eq!(
            KfdTargetDebugTelemetryRecordV2::from_wire_bytes(&padding),
            Err(KfdTargetDebugTelemetryProtocolErrorV2::NonCanonical)
        );
        let mut checksum = record.to_wire_bytes();
        checksum[CHECKSUM_OFFSET_V2] ^= 1;
        assert_eq!(
            KfdTargetDebugTelemetryRecordV2::from_wire_bytes(&checksum),
            Err(KfdTargetDebugTelemetryProtocolErrorV2::Checksum)
        );
        let KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
            process_instance,
            executable,
            artifact,
            dispatch,
            kernel,
            logical_queue,
            dynamic_shared_memory_bytes,
            ..
        } = declaration()
        else {
            unreachable!()
        };
        assert_eq!(
            KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
                process_instance,
                executable,
                artifact,
                dispatch,
                kernel,
                logical_queue,
                grid: [u32::MAX, 1, 1],
                workgroup: [2, 1, 1],
                dynamic_shared_memory_bytes,
                generation: 0,
            }
            .validate(),
            Err(KfdTargetDebugTelemetryDataErrorV2::ZeroGeneration)
        );
    }

    #[test]
    fn credential_bound_channel_accepts_typed_failure_before_publication() {
        let (debugger_fd, target_fd) = create_kfd_target_debug_telemetry_channel_v2().unwrap();
        let process = KfdTargetDebugTelemetryProcessV1::capture(std::process::id()).unwrap();
        let nonce = KfdTargetDebugSessionNonceV1::from_bytes([9; 32]).unwrap();
        let mut debugger =
            KfdDebuggerTelemetryEndpointV2::admit(debugger_fd, nonce, process).unwrap();
        let mut target =
            KfdCooperativeTargetTelemetryEndpointV2::admit(target_fd, nonce, process).unwrap();
        target.send(declaration()).unwrap();
        target
            .send(KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Failed,
            })
            .unwrap();
        assert!(matches!(
            debugger.receive().unwrap().payload(),
            KfdTargetDebugTelemetryPayloadV2::DispatchDeclared { .. }
        ));
        assert!(matches!(
            debugger.receive().unwrap().payload(),
            KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                outcome: KfdTargetDebugSessionOutcomeV2::Failed
            }
        ));
    }

    #[test]
    fn completed_terminal_requires_an_exact_publication() {
        assert_eq!(
            next_lifecycle(
                LifecycleV2::AwaitingPublication,
                &KfdTargetDebugTelemetryPayloadV2::SessionEnded {
                    outcome: KfdTargetDebugSessionOutcomeV2::Completed,
                },
            ),
            Err(KfdTargetDebugTelemetryProtocolErrorV2::InvalidLifecycle)
        );
        assert_eq!(
            next_lifecycle(LifecycleV2::AwaitingPublication, &published()),
            Ok(LifecycleV2::Published)
        );
    }
}
