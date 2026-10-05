//! Inert closed single-agent programs with system fences only at the boundaries.

use super::*;
use alloc::collections::BTreeSet;

/// A position-bound header obtained only from an admitted closed program.
/// It grants no queue, address, ownership or native publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlClosedProgramHeaderV1 {
    index: u32,
    count: u32,
}

impl AqlClosedProgramHeaderV1 {
    /// All positions retain WaitForPrior; no fence scope is disabled.
    pub const fn header(self) -> u16 {
        let acquire = if self.index == 0 { 2 } else { 1 };
        let release = if self.index + 1 == self.count { 2 } else { 1 };
        0x0102 | (acquire << 9) | (release << 11)
    }

    pub const fn matches_position(self, index: u32, count: u32) -> bool {
        self.index == index && self.count == count
    }

    /// Binds the retained setup halfword and the full program cardinality.
    pub const fn admits(self, index: u32, count: u32, setup: u16) -> bool {
        self.matches_position(index, count) && setup >= 1 && setup <= 3
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlPreparedClosedProgramErrorV1 {
    PacketCount(AqlPreparedKernelDispatchBatchErrorV1),
    IndependentDispatch { index: usize },
    ReusedCompletionSignal { index: usize },
}

/// A closed sequence of ordered kernels on one agent, not execution authority.
///
/// A native owner must additionally exclude host/peer/other-queue communication
/// during execution, finish all host inputs before publication, retain every
/// packet resource and acquire the final system-releasing completion before
/// reading, reusing or releasing data. Kernargs still require release publication.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedClosedKernelDispatchProgramV1 {
    packets: Box<[AqlPreparedKernelDispatchV1]>,
}

impl AqlPreparedClosedKernelDispatchProgramV1 {
    pub fn try_from_packets(
        packets: Box<[AqlPreparedKernelDispatchV1]>,
    ) -> Result<Self, AqlPreparedClosedProgramErrorV1> {
        let program = AqlPreparedKernelDispatchProgramV1::try_from_packets(packets)
            .map_err(AqlPreparedClosedProgramErrorV1::PacketCount)?;
        let mut signals = BTreeSet::new();
        for (index, prepared) in program.packets.iter().enumerate() {
            if prepared.ordering != AqlDispatchOrderingV1::WaitForPrior {
                return Err(AqlPreparedClosedProgramErrorV1::IndependentDispatch { index });
            }
            if !signals.insert(prepared.packet.completion_signal()) {
                return Err(AqlPreparedClosedProgramErrorV1::ReusedCompletionSignal { index });
            }
        }
        Ok(Self {
            packets: program.packets,
        })
    }

    pub fn packet_count(&self) -> u32 {
        self.packets.len() as u32
    }

    pub fn header_for_packet(&self, index: u32) -> Option<AqlClosedProgramHeaderV1> {
        (index < self.packet_count()).then_some(AqlClosedProgramHeaderV1 {
            index,
            count: self.packet_count(),
        })
    }

    /// Writes every INVALID body before exposing any closed-program header.
    pub fn publish_with<T: AqlClosedProgramPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        for (index, packet) in self.packets.iter().enumerate() {
            target.write_unpublished(index as u32, &packet.packet)?;
        }
        for index in 0..self.packet_count() {
            target.publish_closed_release_header(
                index,
                AqlClosedProgramHeaderV1 {
                    index,
                    count: self.packet_count(),
                },
            )?;
        }
        Ok(())
    }
}

/// Separate inert publication boundary. Ordinary header validators stay closed.
pub trait AqlClosedProgramPublicationTargetV1: AqlPacketBatchPublicationTargetV1 {
    fn publish_closed_release_header(
        &mut self,
        index: u32,
        header: AqlClosedProgramHeaderV1,
    ) -> Result<(), Self::Error>;
}

#[cfg(test)]
#[path = "closed_program_v1_tests.rs"]
mod tests;
