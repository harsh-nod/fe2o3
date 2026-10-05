//! Inert two-producer barriers and ordered mixed-packet batches.
//!
//! Address shape is not signal ownership or a dependency-graph proof. The
//! retained queue owner must bind every signal to a live allocation/epoch,
//! reject cycles, and keep producers and consumers alive until retirement.

use super::*;

/// System acquire/release BARRIER_AND, also waiting for prior local packets.
pub const AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_BARRIER_AND_HEADER_V1: u16 = 0x1503;

/// The peer path is separate from the frozen zero-dependency publication gate.
pub const fn is_reviewed_aql_peer_publication_v1(header: u16, setup: u16) -> bool {
    (header == AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1
        && setup >= 1
        && setup <= 3)
        || (header == AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_BARRIER_AND_HEADER_V1 && setup == 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlPeerBarrierAndPacketErrorV1 {
    DependencySignal {
        index: usize,
        error: AqlAddressObservationError,
    },
    CompletionSignal(AqlAddressObservationError),
    DuplicateDependencies,
    SelfDependency,
}

/// Exact 64-byte unpublished packet with two distinct nonzero dependencies.
/// The remaining three dependency handles and all reserved fields are zero.
#[derive(Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AqlPeerBarrierAndPacketV1 {
    full_header: u32,
    reserved1: u32,
    dep_signals: [u64; 5],
    reserved2: u64,
    completion_signal: u64,
}

impl AqlPeerBarrierAndPacketV1 {
    pub fn new_unpublished(
        dependencies: [ObservedGpuAddressV1; 2],
        completion: ObservedGpuAddressV1,
    ) -> Result<AqlPreparedPeerBarrierAndV1, AqlPeerBarrierAndPacketErrorV1> {
        let mut signals = [0; 5];
        for (index, signal) in dependencies.into_iter().enumerate() {
            signals[index] = signal
                .require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)
                .map_err(|error| AqlPeerBarrierAndPacketErrorV1::DependencySignal { index, error })?
                .raw();
        }
        let completion = completion
            .require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)
            .map_err(AqlPeerBarrierAndPacketErrorV1::CompletionSignal)?
            .raw();
        if signals[0] == signals[1] {
            return Err(AqlPeerBarrierAndPacketErrorV1::DuplicateDependencies);
        }
        if signals[..2].contains(&completion) {
            return Err(AqlPeerBarrierAndPacketErrorV1::SelfDependency);
        }
        Ok(AqlPreparedPeerBarrierAndV1 {
            packet: Self {
                full_header: u32::from(AQL_INVALID_PACKET_HEADER_V1),
                reserved1: 0,
                dep_signals: signals,
                reserved2: 0,
                completion_signal: completion,
            },
        })
    }

    pub const fn completion_signal(&self) -> u64 {
        self.completion_signal
    }

    pub const fn dependency_signals(&self) -> [u64; 2] {
        [self.dep_signals[0], self.dep_signals[1]]
    }

    pub fn encode_unpublished_le(&self) -> [u8; AQL_BARRIER_AND_PACKET_BYTES_V1] {
        let mut bytes = [0; AQL_BARRIER_AND_PACKET_BYTES_V1];
        bytes[..4].copy_from_slice(&self.full_header.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.reserved1.to_le_bytes());
        for (index, signal) in self.dep_signals.iter().enumerate() {
            let offset = 8 + index * size_of::<u64>();
            bytes[offset..offset + size_of::<u64>()].copy_from_slice(&signal.to_le_bytes());
        }
        bytes[48..56].copy_from_slice(&self.reserved2.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.completion_signal.to_le_bytes());
        bytes
    }
}

/// Linear packet; publication is possible only through the mixed batch below.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedPeerBarrierAndV1 {
    packet: AqlPeerBarrierAndPacketV1,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AqlPreparedPeerPacketV1 {
    Kernel(AqlPreparedKernelDispatchV1),
    Barrier(AqlPreparedPeerBarrierAndV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlPeerPacketBatchErrorV1 {
    Count(AqlPreparedKernelDispatchBatchErrorV1),
    UnorderedKernel { index: usize },
}

/// Ordered kernel/barrier values, not a native queue reservation or graph proof.
/// Every INVALID body is written before any release header is published.
/// On failure the native owner must quarantine all partially exposed queues.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedPeerPacketBatchV1<const N: usize> {
    packets: Box<[AqlPreparedPeerPacketV1; N]>,
}

impl<const N: usize> AqlPreparedPeerPacketBatchV1<N> {
    pub fn try_from_packets(
        packets: [AqlPreparedPeerPacketV1; N],
    ) -> Result<Self, AqlPeerPacketBatchErrorV1> {
        Self::try_from_boxed_packets(Box::new(packets))
    }

    pub fn try_from_boxed_packets(
        packets: Box<[AqlPreparedPeerPacketV1; N]>,
    ) -> Result<Self, AqlPeerPacketBatchErrorV1> {
        if N == 0 {
            return Err(AqlPeerPacketBatchErrorV1::Count(
                AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount,
            ));
        }
        if N > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
            return Err(AqlPeerPacketBatchErrorV1::Count(
                AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                    requested: N,
                    maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2,
                },
            ));
        }
        for (index, packet) in packets.iter().enumerate() {
            if let AqlPreparedPeerPacketV1::Kernel(kernel) = packet {
                if kernel.ordering() != AqlDispatchOrderingV1::WaitForPrior {
                    return Err(AqlPeerPacketBatchErrorV1::UnorderedKernel { index });
                }
            }
        }
        Ok(Self { packets })
    }

    pub const fn packet_count(&self) -> u32 {
        N as u32
    }

    pub fn publish_with<T: AqlPeerPacketBatchPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        for (index, packet) in self.packets.iter().enumerate() {
            match packet {
                AqlPreparedPeerPacketV1::Kernel(kernel) => {
                    target.write_unpublished_kernel(index as u32, &kernel.packet)?;
                }
                AqlPreparedPeerPacketV1::Barrier(barrier) => {
                    target.write_unpublished_barrier(index as u32, &barrier.packet)?;
                }
            }
        }
        for (index, packet) in self.packets.iter().enumerate() {
            let header = match packet {
                AqlPreparedPeerPacketV1::Kernel(kernel) => kernel.ordering.header(),
                AqlPreparedPeerPacketV1::Barrier(_) => {
                    AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_BARRIER_AND_HEADER_V1
                }
            };
            target.publish_release_header(index as u32, header)?;
        }
        Ok(())
    }
}

/// Inert publication interface; implementing it grants no native authority.
/// The queue owner must bind each index to its exclusive slot and copied setup.
pub trait AqlPeerPacketBatchPublicationTargetV1 {
    type Error;
    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error>;
    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<(), Self::Error>;
    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<(), Self::Error>;
}

const _: () = {
    assert!(size_of::<AqlPeerBarrierAndPacketV1>() == 64);
    assert!(align_of::<AqlPeerBarrierAndPacketV1>() == 8);
    assert!(offset_of!(AqlPeerBarrierAndPacketV1, full_header) == 0);
    assert!(offset_of!(AqlPeerBarrierAndPacketV1, reserved1) == 4);
    assert!(offset_of!(AqlPeerBarrierAndPacketV1, dep_signals) == 8);
    assert!(offset_of!(AqlPeerBarrierAndPacketV1, reserved2) == 48);
    assert!(offset_of!(AqlPeerBarrierAndPacketV1, completion_signal) == 56);
};
