//! Checked logical subranges over complete retained allocation owners.

use super::Gfx942ComputeXgmiPacketPlanV1;

include!("compute_xgmi_window_body.rs");

/// Independent allocation-relative offsets of one admitted packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gfx942ComputeXgmiCopyPacketV1 {
    pub source_offset: u64,
    pub destination_offset: u64,
    pub bytes: u32,
}

/// Immutable checked ranges, not allocation, mapping, or DMA authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gfx942ComputeXgmiCopyWindowV1 {
    source_logical_bytes: u64,
    destination_logical_bytes: u64,
    source_offset: u64,
    destination_offset: u64,
    plan: Gfx942ComputeXgmiPacketPlanV1,
}

impl Gfx942ComputeXgmiCopyWindowV1 {
    pub fn new(
        source_logical_bytes: u64,
        destination_logical_bytes: u64,
        source_offset: u64,
        destination_offset: u64,
        bytes: u64,
    ) -> Option<Self> {
        if !gfx942_compute_xgmi_window_bounds_v1!(
            source_logical_bytes,
            destination_logical_bytes,
            source_offset,
            destination_offset,
            bytes
        ) {
            return None;
        }
        Some(Self {
            source_logical_bytes,
            destination_logical_bytes,
            source_offset,
            destination_offset,
            plan: Gfx942ComputeXgmiPacketPlanV1::new(bytes)?,
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

    pub fn destination_offset(&self) -> u64 {
        self.destination_offset
    }

    pub fn bytes(&self) -> u64 {
        self.plan.total_bytes()
    }

    pub fn plan(&self) -> Gfx942ComputeXgmiPacketPlanV1 {
        self.plan
    }

    pub fn packet(&self, index: usize) -> Option<Gfx942ComputeXgmiCopyPacketV1> {
        let packet = self.plan.packet(index)?;
        let (source_offset, destination_offset) = gfx942_compute_xgmi_window_packet_v1!(
            self.source_offset,
            self.destination_offset,
            self.bytes(),
            packet.offset,
            packet.bytes
        )?;
        Some(Gfx942ComputeXgmiCopyPacketV1 {
            source_offset,
            destination_offset,
            bytes: packet.bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    use fe2o3_runtime_model::MAX_ORDERED_PEER_COPY_SEGMENTS_V1;

    #[test]
    fn compute_xgmi_window_rejects_each_logical_overrun_and_overflow() {
        for args in [
            (64, 128, 0, 0, 0),
            (64, 128, 65, 0, 1),
            (64, 128, 0, 129, 1),
            (64, 128, 1, 0, 64),
            (64, 128, 0, 65, 64),
            (u64::MAX, u64::MAX, u64::MAX, 0, 1),
            (u64::MAX, u64::MAX, 0, u64::MAX, 1),
            (u64::MAX, u64::MAX, 1, 0, u64::MAX),
        ] {
            assert_eq!(
                Gfx942ComputeXgmiCopyWindowV1::new(args.0, args.1, args.2, args.3, args.4),
                None
            );
        }
        let over_bound = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
            * MAX_ORDERED_PEER_COPY_SEGMENTS_V1 as u64
            + 1;
        assert_eq!(
            Gfx942ComputeXgmiCopyWindowV1::new(u64::MAX, u64::MAX, 0, 0, over_bound),
            None
        );
    }

    #[test]
    fn compute_xgmi_window_packets_cover_independent_offsets_and_final_tail() {
        let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        for bytes in [1, cap - 1, cap, cap + 1, 2 * cap + 37] {
            for (source, destination) in [(1, 67), (u64::MAX - bytes, 0), (0, u64::MAX - bytes)] {
                let window = Gfx942ComputeXgmiCopyWindowV1::new(
                    source + bytes,
                    destination + bytes,
                    source,
                    destination,
                    bytes,
                )
                .unwrap();
                let mut covered = 0;
                for index in 0..window.plan().count() {
                    let packet = window.packet(index).unwrap();
                    assert_eq!(packet.source_offset, source + covered);
                    assert_eq!(packet.destination_offset, destination + covered);
                    covered += u64::from(packet.bytes);
                }
                assert_eq!(covered, bytes);
                assert_eq!(window.packet(window.plan().count()), None);
                assert_eq!(window.packet(usize::MAX), None);
            }
        }
    }

    #[test]
    fn compute_xgmi_window_packet_projection_rejects_independent_address_overflow() {
        for (source, destination, total, relative, bytes) in [
            (0, 0, 1, 0, 0_u32),
            (0, 0, 1, 2, 1),
            (0, 0, 1, 1, 1),
            (u64::MAX, 0, 2, 1, 1),
            (0, u64::MAX, 2, 1, 1),
            (u64::MAX, 0, 1, 0, 1),
            (0, u64::MAX, 1, 0, 1),
        ] {
            assert_eq!(
                gfx942_compute_xgmi_window_packet_v1!(source, destination, total, relative, bytes),
                None
            );
        }
        assert_eq!(
            gfx942_compute_xgmi_window_packet_v1!(u64::MAX - 1, u64::MAX - 1, 1, 0, 1_u32),
            Some((u64::MAX - 1, u64::MAX - 1))
        );
    }
}
