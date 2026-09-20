//! One logical peer transfer with an immutable, ordered descriptor snapshot.

use super::*;
use fe2o3_runtime_model::{
    OrderedPeerCopyAdmissionErrorV1, validate_ordered_peer_copy_segments_v1,
};

pub use fe2o3_runtime_model::{
    MAX_ORDERED_PEER_COPY_SEGMENTS_V1 as MAX_RUNTIME_PEER_COPY_SEGMENTS_V1,
    OrderedPeerCopySegmentV1 as RuntimePeerCopySegmentV1,
};

/// Typed completion for the entire ordered segment list, not one native packet.
pub enum RuntimePeerCopySegmentsV1 {}

/// Optional ordered peer-copy SPI; not encoded by Worker V1-V5.
///
/// Offsets are relative to the bounding regions, which need not have equal
/// lengths. Validate and retain the entire descriptor list before accepting
/// work. Preserve order and duplicates; overlapping destination writes have
/// serial, last-writer-wins semantics. A failure may leave an applied prefix.
/// This is not an atomic transaction.
///
/// One returned submission owns both allocation leases until all segments and
/// required closing checks complete. An intermediate completion must not wake
/// dependents. Cancellation is allowed only before the first publication.
/// Generic poll, wait and event completion must apply to the whole list. A
/// serial adapter may publish only the current segment during flush: later
/// segments become eligible after their predecessor completes. First-segment
/// publication is the logical operation's irreversible publication point.
pub trait RuntimePeerCopySegmentsBackendV1: RuntimeBackendV1 {
    fn peer_copy_segments_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;
}

pub(super) struct PreparedPeerCopyV1 {
    pub(super) stream_record: StreamRecordV1,
    pub(super) journal_source: ContextReadSourceV1,
    pub(super) source: BackendMemoryRegionV1,
    pub(super) destination: BackendMemoryRegionV1,
    pub(super) dependencies: Vec<u64>,
}

pub(super) fn contract_identity(
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
    segments: &[RuntimePeerCopySegmentV1],
) -> IdentityDigestV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.peer-copy-segments.v1\0");
    digest.update(peer_copy_contract_identity(stream, source, destination).as_bytes());
    digest.update((segments.len() as u64).to_le_bytes());
    for segment in segments {
        digest.update(segment.source_offset.to_le_bytes());
        digest.update(segment.destination_offset.to_le_bytes());
        digest.update(segment.byte_len.to_le_bytes());
    }
    IdentityDigestV1::from_untrusted_bytes(digest.finalize().into())
}

impl<B: RuntimePeerCopySegmentsBackendV1> RuntimeContextV1<B> {
    /// Submit up to 4096 ordered segments as one journal writer and source lease.
    ///
    /// The caller may change or discard its descriptor slice after return.
    /// Source/destination regions are bounding envelopes, not equal-size copies.
    pub fn peer_copy_segments(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>, RuntimeErrorV1<B::Error>> {
        let prepared =
            self.prepare_context_peer_copy_v1(stream, source, destination, dependencies, false)?;
        validate_ordered_peer_copy_segments_v1(
            source.byte_offset,
            source.byte_len,
            destination.byte_offset,
            destination.byte_len,
            segments,
        )
        .map_err(|error| match error {
            OrderedPeerCopyAdmissionErrorV1::Count => RuntimeValidationErrorV1::Capacity,
            _ => RuntimeValidationErrorV1::InvalidRange,
        })?;
        let mut snapshot = Vec::new();
        snapshot
            .try_reserve_exact(segments.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        snapshot.extend_from_slice(segments);
        let identity = contract_identity(stream, source, destination, &snapshot);
        self.submit_context_operation_v1(
            stream,
            prepared.stream_record,
            &[destination.allocation],
            Some(PeerTransferMechanismV1::DeclaredPeerCopy {
                contract_identity: identity,
            }),
            &[prepared.journal_source],
            |backend| {
                backend.peer_copy_segments_v1(
                    prepared.stream_record.backend_stream,
                    prepared.source,
                    prepared.destination,
                    &snapshot,
                    &prepared.dependencies,
                )
            },
        )
    }
}
