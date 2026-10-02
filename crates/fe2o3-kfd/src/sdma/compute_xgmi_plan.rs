//! Bounded, allocation-free packet arithmetic; no mapping or DMA authority.

use super::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
use fe2o3_runtime_model::MAX_ORDERED_PEER_COPY_SEGMENTS_V1;

include!("compute_xgmi_plan_body.rs");

/// One exact subrange of an admitted full-extent compute/XGMI copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gfx942ComputeXgmiPacketV1 {
    pub offset: u64,
    pub bytes: u32,
}

/// Immutable contiguous coverage using the existing packet and segment limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gfx942ComputeXgmiPacketPlanV1 {
    total_bytes: u64,
    count: usize,
}

impl Gfx942ComputeXgmiPacketPlanV1 {
    pub fn new(total_bytes: u64) -> Option<Self> {
        let count = compute_xgmi_packet_count_body_v1!(
            total_bytes,
            GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64,
            MAX_ORDERED_PEER_COPY_SEGMENTS_V1 as u64
        )?;
        Some(Self { total_bytes, count })
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn packet(&self, index: usize) -> Option<Gfx942ComputeXgmiPacketV1> {
        let total = self.total_bytes;
        let count = self.count;
        compute_xgmi_packet_at_body_v1!(
            total,
            count,
            index,
            GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64
        )
        .map(|(offset, bytes)| Gfx942ComputeXgmiPacketV1 { offset, bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: u64 = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64;
    const LIMIT: usize = MAX_ORDERED_PEER_COPY_SEGMENTS_V1;

    fn check(total: u64, count: usize) {
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(total).unwrap();
        assert_eq!(plan.total_bytes(), total);
        assert_eq!(plan.count(), count);
        let mut cursor = 0;
        for index in 0..count {
            let packet = plan.packet(index).unwrap();
            assert_eq!(packet.offset, cursor);
            assert!(packet.bytes > 0);
            assert!(packet.bytes <= GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
            if index + 1 != count {
                assert_eq!(packet.bytes, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
            }
            cursor = packet.offset.checked_add(u64::from(packet.bytes)).unwrap();
            assert!(cursor <= total);
        }
        assert_eq!(cursor, total);
        assert_eq!(plan.packet(count), None);
        assert_eq!(plan.packet(usize::MAX), None);
    }

    #[test]
    fn compute_xgmi_packet_plan_rejects_zero_and_over_bound_extents() {
        for total in [0, CAP * LIMIT as u64 + 1, u64::MAX - CAP, u64::MAX] {
            assert_eq!(Gfx942ComputeXgmiPacketPlanV1::new(total), None);
        }
    }

    #[test]
    fn compute_xgmi_packet_plan_exact_and_tail_boundaries_cover_every_byte() {
        for (total, count) in [
            (1, 1),
            (CAP - 1, 1),
            (CAP, 1),
            (CAP + 1, 2),
            (2 * CAP - 1, 2),
            (2 * CAP, 2),
            (2 * CAP + 1, 3),
            (CAP * LIMIT as u64 - 1, LIMIT),
            (CAP * LIMIT as u64, LIMIT),
        ] {
            check(total, count);
        }
    }

    #[test]
    fn compute_xgmi_packet_plan_every_supported_count_has_exact_final_tail() {
        for count in 1..=LIMIT {
            for tail in [1, CAP - 1, CAP] {
                let total = (count as u64 - 1) * CAP + tail;
                let plan = Gfx942ComputeXgmiPacketPlanV1::new(total).unwrap();
                assert_eq!(plan.count(), count);
                let last = plan.packet(count - 1).unwrap();
                assert_eq!(last.offset, (count as u64 - 1) * CAP);
                assert_eq!(u64::from(last.bytes), tail);
                assert_eq!(last.offset + u64::from(last.bytes), total);
                assert_eq!(plan.packet(count), None);
            }
        }
    }
}
