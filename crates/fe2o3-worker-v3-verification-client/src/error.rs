use std::{error::Error, fmt, io};

use fe2o3_worker_v3_verification_protocol::{
    WorkerV3VerificationFdPayloadKindV1, WorkerV3VerificationProtocolErrorV1,
};

/// Admission, immutable-custody, transport, or response-correlation failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationClientErrorV1 {
    /// The one-shot client timeout was zero.
    InvalidTimeout,
    /// The absolute session deadline could not be represented.
    DeadlineOverflow,
    /// Snapshot admission received other than the protocol's exact two descriptors.
    DescriptorCount {
        /// Required descriptor count.
        expected: usize,
        /// Observed descriptor count.
        actual: usize,
    },
    /// A descriptor was not a regular file.
    PayloadNotRegular {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
    },
    /// A descriptor was linked into a filesystem namespace.
    PayloadLinked {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed hard-link count.
        actual: u64,
    },
    /// A descriptor was not owned by the effective client uid.
    PayloadOwnerMismatch {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Effective uid required by admission.
        expected: u32,
        /// Observed inode owner uid.
        actual: u32,
    },
    /// A descriptor was not owned by the effective client gid.
    PayloadGroupMismatch {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Effective gid required by admission.
        expected: u32,
        /// Observed inode owner gid.
        actual: u32,
    },
    /// A descriptor was not backed by anonymous shmem/memfd storage.
    PayloadNotMemfd {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
    },
    /// A descriptor lacked one of the two admitted immutable seal sets.
    PayloadNotImmutable {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed kernel seal bits.
        actual_seal_bits: u32,
    },
    /// A retained payload descriptor was not exact read-only custody.
    PayloadNotReadOnly {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed open-file status bits.
        actual_status_bits: u32,
    },
    /// A retained payload did not have exact close-on-exec descriptor flags.
    PayloadDescriptorFlags {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed descriptor-flag bits.
        actual_bits: u32,
    },
    /// A payload was shorter than its canonical descriptor.
    PayloadLengthMismatch {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Canonical byte length.
        expected: u64,
        /// Observed byte length.
        actual: u64,
    },
    /// A payload carried bytes after its canonical descriptor length.
    TrailingPayloadBytes {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Canonical byte length.
        declared: u64,
        /// Observed byte length.
        actual: u64,
    },
    /// A payload digest did not match the exact request descriptor.
    PayloadDigestMismatch {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
    },
    /// The two protocol positions aliased one inode instead of independent snapshots.
    DuplicatePayloadInode,
    /// A payload identity changed during or after admission.
    PayloadChanged {
        /// Protocol payload position.
        kind: WorkerV3VerificationFdPayloadKindV1,
    },
    /// A snapshot bundle was used with a different canonical request.
    SnapshotRequestMismatch,
    /// A descriptor operation failed.
    Descriptor {
        /// Stable operation context.
        operation: &'static str,
        /// Kernel or standard-I/O failure.
        source: io::Error,
    },
    /// The service endpoint was not `SOCK_SEQPACKET`.
    NotSeqpacket,
    /// The service endpoint was not a connected unnamed Unix socket.
    NamedOrNonUnixPeer,
    /// Polling the service endpoint failed.
    Poll(io::Error),
    /// Sending the one canonical request failed.
    Send(io::Error),
    /// Half-closing the one-shot request direction failed.
    Shutdown(io::Error),
    /// Receiving the framing response failed.
    Receive(io::Error),
    /// The absolute session deadline expired.
    Timeout,
    /// The service endpoint descriptor became invalid.
    InvalidPeer,
    /// The service endpoint reported an asynchronous error.
    PeerFailed,
    /// The service endpoint closed before returning a response.
    PeerClosed,
    /// The request plus its two descriptors was not sent atomically and completely.
    PartialSend {
        /// Canonical request byte length.
        expected: usize,
        /// Observed sent byte count.
        actual: usize,
    },
    /// The service response was shorter than the exact framing response.
    ResponseTruncated {
        /// Exact response length.
        expected: usize,
        /// Observed packet length.
        actual: usize,
    },
    /// The service response exceeded the exact framing response length.
    ResponseOversize {
        /// Exact maximum response length.
        maximum: usize,
        /// Kernel-reported packet length.
        actual: usize,
    },
    /// The service response carried forbidden ancillary data.
    ResponseAncillaryData,
    /// The canonical framing response named different request coordinates.
    ResponseRequestMismatch,
    /// Canonical request or response framing failed.
    Protocol(WorkerV3VerificationProtocolErrorV1),
}

impl fmt::Display for WorkerV3VerificationClientErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeout => {
                formatter.write_str("Worker V3 verification client timeout must be nonzero")
            }
            Self::DeadlineOverflow => {
                formatter.write_str("Worker V3 verification client deadline overflowed")
            }
            Self::DescriptorCount { expected, actual } => write!(
                formatter,
                "Worker V3 verification snapshot descriptor count mismatch: expected {expected}, got {actual}"
            ),
            Self::PayloadNotRegular { kind } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload is not a regular file"
            ),
            Self::PayloadLinked { kind, actual } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload has {actual} filesystem links"
            ),
            Self::PayloadOwnerMismatch {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload owner mismatch: expected uid {expected}, got {actual}"
            ),
            Self::PayloadGroupMismatch {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload group mismatch: expected gid {expected}, got {actual}"
            ),
            Self::PayloadNotMemfd { kind } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload is not an anonymous shmem/memfd inode"
            ),
            Self::PayloadNotImmutable {
                kind,
                actual_seal_bits,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload has an inadmissible seal set 0x{actual_seal_bits:08x}"
            ),
            Self::PayloadNotReadOnly {
                kind,
                actual_status_bits,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} retained payload is not read-only: status 0x{actual_status_bits:08x}"
            ),
            Self::PayloadDescriptorFlags { kind, actual_bits } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload has noncanonical descriptor flags 0x{actual_bits:08x}"
            ),
            Self::PayloadLengthMismatch {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload length mismatch: expected {expected}, got {actual}"
            ),
            Self::TrailingPayloadBytes {
                kind,
                declared,
                actual,
            } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload has trailing bytes: declared {declared}, got {actual}"
            ),
            Self::PayloadDigestMismatch { kind } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload digest mismatch"
            ),
            Self::DuplicatePayloadInode => {
                formatter.write_str("Worker V3 verification payload positions alias one inode")
            }
            Self::PayloadChanged { kind } => write!(
                formatter,
                "Worker V3 verification {kind:?} payload changed during custody validation"
            ),
            Self::SnapshotRequestMismatch => {
                formatter.write_str("Worker V3 verification snapshots name another request")
            }
            Self::Descriptor { operation, source } => write!(
                formatter,
                "Worker V3 verification descriptor operation `{operation}` failed: {source}"
            ),
            Self::NotSeqpacket => {
                formatter.write_str("Worker V3 verification peer is not SOCK_SEQPACKET")
            }
            Self::NamedOrNonUnixPeer => formatter
                .write_str("Worker V3 verification peer is not a connected unnamed Unix socket"),
            Self::Poll(source) => write!(
                formatter,
                "Worker V3 verification peer poll failed: {source}"
            ),
            Self::Send(source) => write!(
                formatter,
                "Worker V3 verification request send failed: {source}"
            ),
            Self::Shutdown(source) => write!(
                formatter,
                "Worker V3 verification request half-close failed: {source}"
            ),
            Self::Receive(source) => write!(
                formatter,
                "Worker V3 verification response receive failed: {source}"
            ),
            Self::Timeout => {
                formatter.write_str("Worker V3 verification absolute deadline expired")
            }
            Self::InvalidPeer => {
                formatter.write_str("Worker V3 verification peer descriptor became invalid")
            }
            Self::PeerFailed => {
                formatter.write_str("Worker V3 verification peer reported an error")
            }
            Self::PeerClosed => {
                formatter.write_str("Worker V3 verification peer closed before responding")
            }
            Self::PartialSend { expected, actual } => write!(
                formatter,
                "Worker V3 verification request send was partial: expected {expected}, got {actual}"
            ),
            Self::ResponseTruncated { expected, actual } => write!(
                formatter,
                "Worker V3 verification response was truncated: expected {expected}, got {actual}"
            ),
            Self::ResponseOversize { maximum, actual } => write!(
                formatter,
                "Worker V3 verification response exceeded {maximum} bytes: got {actual}"
            ),
            Self::ResponseAncillaryData => {
                formatter.write_str("Worker V3 verification response carried ancillary data")
            }
            Self::ResponseRequestMismatch => {
                formatter.write_str("Worker V3 verification response names another request")
            }
            Self::Protocol(source) => {
                write!(formatter, "Worker V3 verification framing failed: {source}")
            }
        }
    }
}

impl Error for WorkerV3VerificationClientErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor { source, .. }
            | Self::Poll(source)
            | Self::Send(source)
            | Self::Shutdown(source)
            | Self::Receive(source) => Some(source),
            Self::Protocol(source) => Some(source),
            _ => None,
        }
    }
}

impl From<WorkerV3VerificationProtocolErrorV1> for WorkerV3VerificationClientErrorV1 {
    fn from(source: WorkerV3VerificationProtocolErrorV1) -> Self {
        Self::Protocol(source)
    }
}
