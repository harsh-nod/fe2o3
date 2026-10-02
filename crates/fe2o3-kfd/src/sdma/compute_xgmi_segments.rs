//! Checked ordered logical windows; these values grant no native authority.

use super::Gfx942ComputeXgmiCopyWindowV1;
use fe2o3_runtime_model::{
    OrderedPeerCopyAdmissionErrorV1, OrderedPeerCopySegmentV1,
    validate_ordered_peer_copy_segments_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ComputeXgmiSegmentsPlanErrorV1 {
    SourceExtent,
    DestinationExtent,
    Segments(OrderedPeerCopyAdmissionErrorV1),
    PacketExtent,
    Capacity,
}

/// Fully checked, immutable descriptors in caller order, including duplicates.
///
/// Descriptor offsets are relative to the two bounding regions. Each retained
/// window is allocation-relative and checked against the original logical
/// extent, never pool padding. Overlapping destination windows require serial
/// completion in this order; this plan is not an atomic transaction or a grant
/// to map or use either allocation. There are at most 4096 descriptors, each
/// subject to the existing 4096-packet window bound, without a flattened roster.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942ComputeXgmiSegmentsPlanV1 {
    source_logical_bytes: u64,
    destination_logical_bytes: u64,
    source_offset: u64,
    source_len: u64,
    destination_offset: u64,
    destination_len: u64,
    total_bytes: u64,
    packet_count: usize,
    windows: Box<[Gfx942ComputeXgmiCopyWindowV1]>,
}

impl Gfx942ComputeXgmiSegmentsPlanV1 {
    pub fn new(
        source_logical_bytes: u64,
        destination_logical_bytes: u64,
        source_offset: u64,
        source_len: u64,
        destination_offset: u64,
        destination_len: u64,
        segments: &[OrderedPeerCopySegmentV1],
    ) -> Result<Self, Gfx942ComputeXgmiSegmentsPlanErrorV1> {
        use Gfx942ComputeXgmiSegmentsPlanErrorV1 as E;
        let total_bytes = validate_ordered_peer_copy_segments_v1(
            source_offset,
            source_len,
            destination_offset,
            destination_len,
            segments,
        )
        .map_err(E::Segments)?;
        if source_offset
            .checked_add(source_len)
            .is_none_or(|end| end > source_logical_bytes)
        {
            return Err(E::SourceExtent);
        }
        if destination_offset
            .checked_add(destination_len)
            .is_none_or(|end| end > destination_logical_bytes)
        {
            return Err(E::DestinationExtent);
        }
        let window = |segment: &OrderedPeerCopySegmentV1| {
            Gfx942ComputeXgmiCopyWindowV1::new(
                source_logical_bytes,
                destination_logical_bytes,
                source_offset.checked_add(segment.source_offset)?,
                destination_offset.checked_add(segment.destination_offset)?,
                segment.byte_len,
            )
        };
        // Validate the complete packet roster before even allocating its retained copy.
        let mut packet_count = 0_usize;
        for segment in segments {
            packet_count = packet_count
                .checked_add(window(segment).ok_or(E::PacketExtent)?.plan().count())
                .ok_or(E::PacketExtent)?;
        }
        let mut windows = Vec::new();
        windows
            .try_reserve_exact(segments.len())
            .map_err(|_| E::Capacity)?;
        for segment in segments {
            windows.push(window(segment).ok_or(E::PacketExtent)?);
        }
        Ok(Self {
            source_logical_bytes,
            destination_logical_bytes,
            source_offset,
            source_len,
            destination_offset,
            destination_len,
            total_bytes,
            packet_count,
            windows: windows.into_boxed_slice(),
        })
    }

    pub fn source_logical_bytes(&self) -> u64 {
        self.source_logical_bytes
    }

    pub fn destination_logical_bytes(&self) -> u64 {
        self.destination_logical_bytes
    }

    pub fn source_offset(&self) -> u64 {
        self.source_offset
    }

    pub fn source_len(&self) -> u64 {
        self.source_len
    }

    pub fn destination_offset(&self) -> u64 {
        self.destination_offset
    }

    pub fn destination_len(&self) -> u64 {
        self.destination_len
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn packet_count(&self) -> usize {
        self.packet_count
    }

    pub fn windows(&self) -> &[Gfx942ComputeXgmiCopyWindowV1] {
        &self.windows
    }
}

#[cfg(test)]
#[path = "compute_xgmi_segments_tests.rs"]
mod tests;
