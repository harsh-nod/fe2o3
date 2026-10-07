//! Inert AMDHSA dependency-barrier and final-dispatch planning.
//!
//! This module packs a bounded signal roster into five-wide BARRIER_AND
//! packets followed by one kernel dispatch. It drives a generic publication
//! target in the required order, but owns no native ring, counter, signal,
//! doorbell, or execution authority. As a `no_std` layer it classifies callback
//! `Err` values but cannot catch callback panics; a production `std` adapter
//! must catch unwind and retain and poison its exact native owners as terminal.

use alloc::{boxed::Box, vec::Vec};
use core::mem::{align_of, offset_of, size_of};

use crate::{
    AMD_SIGNAL_ALIGNMENT_V1, AQL_BARRIER_AND_PACKET_BYTES_V1, AQL_INVALID_PACKET_HEADER_V1,
    AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1, AqlAddressObservationError, AqlDispatchOrderingV1,
    AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchV1, AqlRingBatchReservationEntryV1,
    AqlRingBatchReservationV1, AqlRingReservationError, AqlSingleProducerRingModelV1,
    ObservedGpuAddressV1,
};

/// Maximum dependency signals admitted by one V1 dependency dispatch.
pub const AQL_MAX_DEPENDENCY_SIGNALS_V1: usize = 256;
/// Number of dependency signal handles carried by one BARRIER_AND packet.
pub const AQL_BARRIER_AND_FAN_IN_V1: usize = 5;
/// Maximum barrier packets needed for one V1 dependency dispatch.
pub const AQL_MAX_DEPENDENCY_BARRIER_PACKETS_V1: usize =
    AQL_MAX_DEPENDENCY_SIGNALS_V1.div_ceil(AQL_BARRIER_AND_FAN_IN_V1);
/// Maximum total packets in one V1 dependency dispatch publication.
pub const AQL_MAX_DEPENDENCY_DISPATCH_PACKETS_V1: u32 =
    AQL_MAX_DEPENDENCY_BARRIER_PACKETS_V1 as u32 + 1;

/// Canonical contract for inert dependency-dispatch planning and callbacks.
pub const AQL_DEPENDENCY_DISPATCH_MANIFEST_V1: &str = r#"schema=fe2o3-aql-dependency-dispatch-v1
wire-schema=rocr-7.2.4-amdhsa-aql-barrier-and-plus-kernel-dispatch
source.hsa.h=51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5
dependency-count=0..256
barrier-fan-in=5
lowering=zero:one-independent-kernel|nonzero:ceil(dependencies/5)-system-scope-barrier-and-0x1403-plus-one-wait-for-prior-kernel-0x1502
barrier-packet=size:64,align:8,invalid-full-header:1,reserved-zero,unused-dependencies-zero,completion-signal-zero
packet-count=1..53
reservation=existing-v2-fixed-batch-ring-model,whole-range-before-target-callback,retryable-only-for-full-or-insufficient-space,other-ring-errors-definite-pre-effect-rejection
publication=claim-write-index,all-invalid-barrier-bodies,invalid-final-dispatch-body,ordered-barrier-release-headers,final-dispatch-release-header,one-final-doorbell
failure=reservation-retry-or-rejection-returns-prepared-owner,any-target-callback-error-is-terminal-ambiguous-and-retains-nonretryable-inert-custody
unwind=no-std-planner-cannot-catch-callback-panic,production-std-adapter-must-catch-unwind-and-retain-poison-exact-reservation-and-native-owners-as-terminal
authority=inert-packet-reservation-and-callback-orchestration-only,no-address-provenance,no-native-ring-or-counter,no-release-atomic-proof,no-signal-lifetime,no-doorbell-authority,no-firmware-or-execution-claim
"#;

/// SHA-256 of [`AQL_DEPENDENCY_DISPATCH_MANIFEST_V1`].
pub const AQL_DEPENDENCY_DISPATCH_MANIFEST_SHA256_V1: &str =
    "b0fa3e74de7d5f395d679e0eace95cb8e0bdece640595c0c23323308b5e6d841";

/// One aligned, nonzero numeric dependency-signal observation.
///
/// This is an inert number. It proves no allocation, provenance, mapping,
/// lifetime, device identity, or signal ABI placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct AqlDependencySignalObservationV1(ObservedGpuAddressV1);

impl AqlDependencySignalObservationV1 {
    /// Admits one nonzero, 64-byte-aligned numeric signal observation.
    pub const fn new(raw: u64) -> Result<Self, AqlAddressObservationError> {
        let observed = match ObservedGpuAddressV1::new(raw) {
            Ok(observed) => observed,
            Err(error) => return Err(error),
        };
        match observed.require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64) {
            Ok(observed) => Ok(Self(observed)),
            Err(error) => Err(error),
        }
    }

    /// Returns the inert numeric observation.
    pub const fn raw(self) -> u64 {
        self.0.raw()
    }
}

/// Exact unpublished 64-byte BARRIER_AND packet with up to five dependencies.
///
/// Unused dependency positions and every reserved field are zero. The
/// completion signal is zero because the following barrier-ordered kernel
/// dispatch is the sole completion boundary for this packet sequence.
#[derive(Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AqlDependencyBarrierPacketV1 {
    full_header: u32,
    reserved1: u32,
    dependency_signals: [u64; AQL_BARRIER_AND_FAN_IN_V1],
    reserved2: u64,
    completion_signal: u64,
}

impl AqlDependencyBarrierPacketV1 {
    fn from_signals(signals: &[AqlDependencySignalObservationV1]) -> Self {
        debug_assert!(!signals.is_empty());
        debug_assert!(signals.len() <= AQL_BARRIER_AND_FAN_IN_V1);
        let mut dependency_signals = [0; AQL_BARRIER_AND_FAN_IN_V1];
        for (index, signal) in signals.iter().enumerate() {
            dependency_signals[index] = signal.raw();
        }
        Self {
            full_header: u32::from(AQL_INVALID_PACKET_HEADER_V1),
            reserved1: 0,
            dependency_signals,
            reserved2: 0,
            completion_signal: 0,
        }
    }

    /// Returns whether the packet still carries the INVALID unpublished type.
    pub const fn is_unpublished(&self) -> bool {
        self.full_header == AQL_INVALID_PACKET_HEADER_V1 as u32
    }

    /// Returns the exact five dependency slots, including zero padding.
    pub const fn dependency_signals(&self) -> [u64; AQL_BARRIER_AND_FAN_IN_V1] {
        self.dependency_signals
    }

    /// Returns the required zero completion-signal field.
    pub const fn completion_signal(&self) -> u64 {
        self.completion_signal
    }

    /// Encodes the exact unpublished little-endian packet image.
    pub fn encode_unpublished_le(&self) -> [u8; AQL_BARRIER_AND_PACKET_BYTES_V1] {
        let mut bytes = [0_u8; AQL_BARRIER_AND_PACKET_BYTES_V1];
        bytes[0..4].copy_from_slice(&self.full_header.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.reserved1.to_le_bytes());
        for (index, signal) in self.dependency_signals.iter().enumerate() {
            let offset = 8 + index * size_of::<u64>();
            bytes[offset..offset + size_of::<u64>()].copy_from_slice(&signal.to_le_bytes());
        }
        bytes[48..56].copy_from_slice(&self.reserved2.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.completion_signal.to_le_bytes());
        bytes
    }
}

/// Definite preparation rejection before a packet roster exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlDependencyDispatchPlanErrorV1 {
    /// The caller supplied more than the reviewed dependency bound.
    TooManyDependencies { requested: usize, maximum: usize },
    /// Two roster entries name the same numeric signal observation.
    DuplicateDependencySignal {
        first_index: usize,
        duplicate_index: usize,
    },
    /// Heap allocation for the exact barrier roster failed.
    Allocation,
}

/// Preparation failure that returns the caller's final dispatch owner.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlDependencyDispatchPlanFailureV1 {
    error: AqlDependencyDispatchPlanErrorV1,
    final_dispatch: AqlPreparedKernelDispatchV1,
}

impl AqlDependencyDispatchPlanFailureV1 {
    /// Returns the definite preparation error.
    pub const fn error(&self) -> AqlDependencyDispatchPlanErrorV1 {
        self.error
    }

    /// Recovers the unchanged final dispatch after definite rejection.
    pub fn into_final_dispatch(self) -> AqlPreparedKernelDispatchV1 {
        self.final_dispatch
    }
}

/// Complete inert packet roster for one dependency-ordered dispatch.
///
/// Construction performs every allocation and validation needed by
/// publication. Zero dependencies retain an independent final dispatch.
/// Nonzero dependencies retain a wait-for-prior final dispatch.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedDependencyDispatchV1 {
    inner: Box<[AqlDependencyDispatchPlanV1; 1]>,
}

#[derive(Debug, Eq, PartialEq)]
struct AqlDependencyDispatchPlanV1 {
    dependency_count: u16,
    barriers: Box<[AqlDependencyBarrierPacketV1]>,
    final_dispatch: AqlPreparedKernelDispatchV1,
}

impl AqlPreparedDependencyDispatchV1 {
    /// Packs an exact dependency roster and normalizes final dispatch ordering.
    pub fn new(
        dependencies: &[AqlDependencySignalObservationV1],
        mut final_dispatch: AqlPreparedKernelDispatchV1,
    ) -> Result<Self, AqlDependencyDispatchPlanFailureV1> {
        if dependencies.len() > AQL_MAX_DEPENDENCY_SIGNALS_V1 {
            return Err(AqlDependencyDispatchPlanFailureV1 {
                error: AqlDependencyDispatchPlanErrorV1::TooManyDependencies {
                    requested: dependencies.len(),
                    maximum: AQL_MAX_DEPENDENCY_SIGNALS_V1,
                },
                final_dispatch,
            });
        }
        for (duplicate_index, duplicate) in dependencies.iter().enumerate() {
            if let Some(first_index) = dependencies[..duplicate_index]
                .iter()
                .position(|candidate| candidate == duplicate)
            {
                return Err(AqlDependencyDispatchPlanFailureV1 {
                    error: AqlDependencyDispatchPlanErrorV1::DuplicateDependencySignal {
                        first_index,
                        duplicate_index,
                    },
                    final_dispatch,
                });
            }
        }

        let barrier_count = dependencies.len().div_ceil(AQL_BARRIER_AND_FAN_IN_V1);
        let mut inner = Vec::new();
        if inner.try_reserve_exact(1).is_err() {
            return Err(AqlDependencyDispatchPlanFailureV1 {
                error: AqlDependencyDispatchPlanErrorV1::Allocation,
                final_dispatch,
            });
        }
        let mut barriers = Vec::new();
        if barriers.try_reserve_exact(barrier_count).is_err() {
            return Err(AqlDependencyDispatchPlanFailureV1 {
                error: AqlDependencyDispatchPlanErrorV1::Allocation,
                final_dispatch,
            });
        }
        for signals in dependencies.chunks(AQL_BARRIER_AND_FAN_IN_V1) {
            barriers.push(AqlDependencyBarrierPacketV1::from_signals(signals));
        }
        final_dispatch.ordering = if dependencies.is_empty() {
            AqlDispatchOrderingV1::Independent
        } else {
            AqlDispatchOrderingV1::WaitForPrior
        };
        inner.push(AqlDependencyDispatchPlanV1 {
            dependency_count: dependencies.len() as u16,
            barriers: barriers.into_boxed_slice(),
            final_dispatch,
        });
        let inner = inner
            .into_boxed_slice()
            .try_into()
            .expect("one exact plan was inserted into one reserved slot");
        Ok(Self { inner })
    }

    /// Returns the exact number of dependency signals.
    pub const fn dependency_count(&self) -> u16 {
        self.inner[0].dependency_count
    }

    /// Returns the exact number of BARRIER_AND packets.
    pub fn barrier_count(&self) -> u32 {
        self.inner[0].barriers.len() as u32
    }

    /// Returns the exact total packet count, including the final dispatch.
    pub fn packet_count(&self) -> u32 {
        self.barrier_count() + 1
    }

    /// Returns the normalized final dispatch ordering.
    pub const fn final_dispatch_ordering(&self) -> AqlDispatchOrderingV1 {
        self.inner[0].final_dispatch.ordering
    }

    /// Returns one exact barrier packet by its publication index.
    pub fn barrier(&self, barrier_index: u32) -> Option<&AqlDependencyBarrierPacketV1> {
        self.inner[0].barriers.get(barrier_index as usize)
    }

    /// Reserves and publishes this complete inert plan through one target.
    ///
    /// Ring occupancy failures are retryable and return the prepared owner.
    /// Other ring errors are definite pre-effect rejections and also return the
    /// owner. Once the ring model reserves the complete range, any target
    /// callback error is terminal ambiguity and the plan becomes nonretryable
    /// terminal custody.
    pub fn publish_with<T: AqlDependencyDispatchPublicationTargetV1>(
        self,
        ring: &mut AqlSingleProducerRingModelV1,
        observed_read: u64,
        target: &mut T,
    ) -> Result<
        AqlDependencyDispatchPublicationV1,
        AqlDependencyDispatchPublicationFailureV1<T::Error>,
    > {
        let reservation = match ring.reserve_fixed_batch_v2(observed_read, self.packet_count()) {
            Ok(reservation) => reservation,
            Err(
                error @ (AqlRingReservationError::Full
                | AqlRingReservationError::InsufficientSpace { .. }),
            ) => {
                return Err(
                    AqlDependencyDispatchPublicationFailureV1::RetryableBeforeSideEffect {
                        error,
                        prepared: self,
                    },
                );
            }
            Err(error) => {
                return Err(
                    AqlDependencyDispatchPublicationFailureV1::RejectedBeforeSideEffect {
                        error,
                        prepared: self,
                    },
                );
            }
        };

        if let Err(error) = target
            .claim_write_index_acq_rel(reservation.first_packet_id(), reservation.packet_count())
        {
            return Err(terminal_failure(
                self,
                reservation,
                AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
                error,
            ));
        }

        for (barrier_index, barrier) in self.inner[0].barriers.iter().enumerate() {
            let barrier_index = barrier_index as u32;
            let entry = reservation
                .entry(barrier_index)
                .expect("planned barrier index is inside the exact reservation");
            if let Err(error) = target.write_unpublished_barrier(entry, barrier) {
                return Err(terminal_failure(
                    self,
                    reservation,
                    AqlDependencyDispatchPublicationBoundaryV1::BarrierBody { barrier_index },
                    error,
                ));
            }
        }

        let final_index = self.barrier_count();
        let final_entry = reservation
            .entry(final_index)
            .expect("planned final dispatch is inside the exact reservation");
        if let Err(error) =
            target.write_unpublished_dispatch(final_entry, &self.inner[0].final_dispatch.packet)
        {
            return Err(terminal_failure(
                self,
                reservation,
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchBody,
                error,
            ));
        }

        for barrier_index in 0..self.barrier_count() {
            let entry = reservation
                .entry(barrier_index)
                .expect("planned barrier index is inside the exact reservation");
            if let Err(error) = target
                .publish_barrier_release_header(entry, AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1)
            {
                return Err(terminal_failure(
                    self,
                    reservation,
                    AqlDependencyDispatchPublicationBoundaryV1::BarrierHeader { barrier_index },
                    error,
                ));
            }
        }

        if let Err(error) = target.publish_dispatch_release_header(
            final_entry,
            self.inner[0].final_dispatch.ordering.header(),
        ) {
            return Err(terminal_failure(
                self,
                reservation,
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader,
                error,
            ));
        }

        if let Err(error) = target.ring_doorbell_release(reservation.last_packet_id()) {
            return Err(terminal_failure(
                self,
                reservation,
                AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
                error,
            ));
        }

        Ok(AqlDependencyDispatchPublicationV1 {
            dependency_count: self.dependency_count(),
            barrier_count: self.barrier_count() as u16,
            first_packet_id: reservation.first_packet_id(),
            last_packet_id: reservation.last_packet_id(),
            packet_count: reservation.packet_count(),
        })
    }
}

/// Exact target callback at which publication became ambiguous.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlDependencyDispatchPublicationBoundaryV1 {
    ClaimWriteIndex,
    BarrierBody { barrier_index: u32 },
    FinalDispatchBody,
    BarrierHeader { barrier_index: u32 },
    FinalDispatchHeader,
    Doorbell,
}

/// Nonretryable inert custody retained after a target callback error.
///
/// This type has inspection only. It deliberately has no conversion back to a
/// prepared publication owner.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlTerminalDependencyDispatchCustodyV1 {
    prepared: AqlPreparedDependencyDispatchV1,
    reservation: AqlRingBatchReservationV1,
}

impl AqlTerminalDependencyDispatchCustodyV1 {
    pub const fn dependency_count(&self) -> u16 {
        self.prepared.dependency_count()
    }

    pub fn barrier_count(&self) -> u32 {
        self.prepared.barrier_count()
    }

    pub const fn reservation(&self) -> &AqlRingBatchReservationV1 {
        &self.reservation
    }
}

/// Classified failure from reservation or target publication.
#[derive(Debug, Eq, PartialEq)]
pub enum AqlDependencyDispatchPublicationFailureV1<E> {
    /// Capacity pressure caused no retained model or target side effect.
    RetryableBeforeSideEffect {
        error: AqlRingReservationError,
        prepared: AqlPreparedDependencyDispatchV1,
    },
    /// A ring invariant rejected without invoking the target.
    RejectedBeforeSideEffect {
        error: AqlRingReservationError,
        prepared: AqlPreparedDependencyDispatchV1,
    },
    /// A target callback may have performed its named side effect.
    TerminalAmbiguous {
        error: E,
        boundary: AqlDependencyDispatchPublicationBoundaryV1,
        custody: AqlTerminalDependencyDispatchCustodyV1,
    },
}

/// Redacted result of one fully successful callback sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlDependencyDispatchPublicationV1 {
    dependency_count: u16,
    barrier_count: u16,
    first_packet_id: u64,
    last_packet_id: u64,
    packet_count: u32,
}

impl AqlDependencyDispatchPublicationV1 {
    pub const fn dependency_count(self) -> u16 {
        self.dependency_count
    }

    pub const fn barrier_count(self) -> u16 {
        self.barrier_count
    }

    pub const fn first_packet_id(self) -> u64 {
        self.first_packet_id
    }

    pub const fn last_packet_id(self) -> u64 {
        self.last_packet_id
    }

    pub const fn packet_count(self) -> u32 {
        self.packet_count
    }
}

/// Generic callback boundary for one reserved dependency-dispatch sequence.
///
/// Implementing this trait grants no native authority. A production adapter
/// must bind every entry to the exact exclusive native slot, implement the
/// named atomic/MMIO order, and retain real terminal custody on any error. This
/// `no_std` planner cannot catch callback panics; a `std` adapter must catch
/// unwind around the whole call and retain and poison the exact reservation
/// and native owners as terminal.
pub trait AqlDependencyDispatchPublicationTargetV1 {
    type Error;

    fn claim_write_index_acq_rel(
        &mut self,
        first_packet_id: u64,
        packet_count: u32,
    ) -> Result<(), Self::Error>;

    fn write_unpublished_barrier(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        packet: &AqlDependencyBarrierPacketV1,
    ) -> Result<(), Self::Error>;

    fn write_unpublished_dispatch(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error>;

    fn publish_barrier_release_header(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        header: u16,
    ) -> Result<(), Self::Error>;

    fn publish_dispatch_release_header(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        header: u16,
    ) -> Result<(), Self::Error>;

    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), Self::Error>;
}

fn terminal_failure<E>(
    prepared: AqlPreparedDependencyDispatchV1,
    reservation: AqlRingBatchReservationV1,
    boundary: AqlDependencyDispatchPublicationBoundaryV1,
    error: E,
) -> AqlDependencyDispatchPublicationFailureV1<E> {
    AqlDependencyDispatchPublicationFailureV1::TerminalAmbiguous {
        error,
        boundary,
        custody: AqlTerminalDependencyDispatchCustodyV1 {
            prepared,
            reservation,
        },
    }
}

const _: () = {
    assert!(AQL_MAX_DEPENDENCY_BARRIER_PACKETS_V1 == 52);
    assert!(AQL_MAX_DEPENDENCY_DISPATCH_PACKETS_V1 == 53);
    assert!(size_of::<AqlDependencyBarrierPacketV1>() == AQL_BARRIER_AND_PACKET_BYTES_V1);
    assert!(align_of::<AqlDependencyBarrierPacketV1>() == 8);
    assert!(offset_of!(AqlDependencyBarrierPacketV1, full_header) == 0);
    assert!(offset_of!(AqlDependencyBarrierPacketV1, reserved1) == 4);
    assert!(offset_of!(AqlDependencyBarrierPacketV1, dependency_signals) == 8);
    assert!(offset_of!(AqlDependencyBarrierPacketV1, reserved2) == 48);
    assert!(offset_of!(AqlDependencyBarrierPacketV1, completion_signal) == 56);
};

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use sha2::{Digest, Sha256};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Event {
        Claim { first: u64, count: u32 },
        BarrierBody { packet_id: u64, slot: u32 },
        DispatchBody { packet_id: u64, slot: u32 },
        BarrierHeader { packet_id: u64, header: u16 },
        DispatchHeader { packet_id: u64, header: u16 },
        Doorbell { packet_id: u64 },
    }

    #[derive(Default)]
    struct CaptureTarget {
        events: Vec<Event>,
        barrier_bodies: Vec<[u8; AQL_BARRIER_AND_PACKET_BYTES_V1]>,
        dispatch_body: Option<[u8; crate::AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]>,
        fail_at: Option<usize>,
    }

    impl CaptureTarget {
        fn record(&mut self, event: Event) -> Result<(), &'static str> {
            let index = self.events.len();
            self.events.push(event);
            if self.fail_at == Some(index) {
                Err("injected publication failure")
            } else {
                Ok(())
            }
        }
    }

    impl AqlDependencyDispatchPublicationTargetV1 for CaptureTarget {
        type Error = &'static str;

        fn claim_write_index_acq_rel(
            &mut self,
            first_packet_id: u64,
            packet_count: u32,
        ) -> Result<(), Self::Error> {
            self.record(Event::Claim {
                first: first_packet_id,
                count: packet_count,
            })
        }

        fn write_unpublished_barrier(
            &mut self,
            entry: AqlRingBatchReservationEntryV1,
            packet: &AqlDependencyBarrierPacketV1,
        ) -> Result<(), Self::Error> {
            assert!(packet.is_unpublished());
            self.barrier_bodies.push(packet.encode_unpublished_le());
            self.record(Event::BarrierBody {
                packet_id: entry.packet_id(),
                slot: entry.slot_index(),
            })
        }

        fn write_unpublished_dispatch(
            &mut self,
            entry: AqlRingBatchReservationEntryV1,
            packet: &AqlKernelDispatchPacketV1,
        ) -> Result<(), Self::Error> {
            assert!(packet.is_unpublished());
            self.dispatch_body = Some(packet.encode_unpublished_le());
            self.record(Event::DispatchBody {
                packet_id: entry.packet_id(),
                slot: entry.slot_index(),
            })
        }

        fn publish_barrier_release_header(
            &mut self,
            entry: AqlRingBatchReservationEntryV1,
            header: u16,
        ) -> Result<(), Self::Error> {
            self.record(Event::BarrierHeader {
                packet_id: entry.packet_id(),
                header,
            })
        }

        fn publish_dispatch_release_header(
            &mut self,
            entry: AqlRingBatchReservationEntryV1,
            header: u16,
        ) -> Result<(), Self::Error> {
            self.record(Event::DispatchHeader {
                packet_id: entry.packet_id(),
                header,
            })
        }

        fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), Self::Error> {
            self.record(Event::Doorbell { packet_id })
        }
    }

    #[test]
    fn manifest_digest_is_frozen() {
        let digest = Sha256::digest(AQL_DEPENDENCY_DISPATCH_MANIFEST_V1);
        assert_eq!(hex(&digest), AQL_DEPENDENCY_DISPATCH_MANIFEST_SHA256_V1);
    }

    #[test]
    fn dependency_signal_observation_rejects_zero_and_misalignment() {
        assert_eq!(
            AqlDependencySignalObservationV1::new(0),
            Err(AqlAddressObservationError::Zero)
        );
        assert_eq!(
            AqlDependencySignalObservationV1::new(0x1008),
            Err(AqlAddressObservationError::Misaligned)
        );
        assert_eq!(
            AqlDependencySignalObservationV1::new(0x1040).unwrap().raw(),
            0x1040
        );
    }

    #[test]
    fn boundary_counts_and_final_ordering_are_exact() {
        for (dependency_count, barriers, packets, ordering) in [
            (0, 0, 1, AqlDispatchOrderingV1::Independent),
            (1, 1, 2, AqlDispatchOrderingV1::WaitForPrior),
            (5, 1, 2, AqlDispatchOrderingV1::WaitForPrior),
            (6, 2, 3, AqlDispatchOrderingV1::WaitForPrior),
            (255, 51, 52, AqlDispatchOrderingV1::WaitForPrior),
            (256, 52, 53, AqlDispatchOrderingV1::WaitForPrior),
        ] {
            let dependencies = dependencies(dependency_count);
            let plan = plan(&dependencies);
            assert_eq!(plan.dependency_count(), dependency_count as u16);
            assert_eq!(plan.barrier_count(), barriers);
            assert_eq!(plan.packet_count(), packets);
            assert_eq!(plan.final_dispatch_ordering(), ordering);
        }
        assert_eq!(AQL_MAX_DEPENDENCY_BARRIER_PACKETS_V1, 52);
        assert_eq!(AQL_MAX_DEPENDENCY_DISPATCH_PACKETS_V1, 53);
    }

    #[test]
    fn every_admitted_count_packs_each_signal_once_and_zero_pads_tail() {
        for dependency_count in 0..=AQL_MAX_DEPENDENCY_SIGNALS_V1 {
            let dependencies = dependencies(dependency_count);
            let plan = plan(&dependencies);
            let flattened = (0..plan.barrier_count())
                .flat_map(|index| plan.barrier(index).unwrap().dependency_signals())
                .collect::<Vec<_>>();
            assert_eq!(
                &flattened[..dependency_count],
                dependencies
                    .iter()
                    .map(|dependency| dependency.raw())
                    .collect::<Vec<_>>()
            );
            assert!(
                flattened[dependency_count..]
                    .iter()
                    .all(|signal| *signal == 0)
            );
            assert_eq!(
                plan.barrier_count() as usize,
                dependency_count.div_ceil(AQL_BARRIER_AND_FAN_IN_V1)
            );
            for barrier in plan.inner[0].barriers.iter() {
                let bytes = barrier.encode_unpublished_le();
                assert_eq!(&bytes[0..4], &1_u32.to_le_bytes());
                assert_eq!(&bytes[4..8], &[0; 4]);
                assert_eq!(&bytes[48..64], &[0; 16]);
                assert_eq!(barrier.completion_signal(), 0);
            }
        }
    }

    #[test]
    fn rejects_257_and_duplicate_signals_without_losing_final_dispatch() {
        let too_many = dependencies(257);
        let failure = AqlPreparedDependencyDispatchV1::new(&too_many, dispatch()).unwrap_err();
        assert_eq!(
            failure.error(),
            AqlDependencyDispatchPlanErrorV1::TooManyDependencies {
                requested: 257,
                maximum: 256,
            }
        );
        assert_eq!(
            failure.into_final_dispatch().ordering(),
            AqlDispatchOrderingV1::Independent
        );

        let duplicate = vec![signal(0), signal(1), signal(0)];
        let failure = AqlPreparedDependencyDispatchV1::new(&duplicate, dispatch()).unwrap_err();
        assert_eq!(
            failure.error(),
            AqlDependencyDispatchPlanErrorV1::DuplicateDependencySignal {
                first_index: 0,
                duplicate_index: 2,
            }
        );
        assert_eq!(
            failure.into_final_dispatch().ordering(),
            AqlDispatchOrderingV1::Independent
        );
    }

    #[test]
    fn zero_one_and_six_publish_exact_body_header_and_doorbell_order() {
        for dependency_count in [0, 1, 6] {
            let dependencies = dependencies(dependency_count);
            let mut ring = ring(62, 62);
            let mut target = CaptureTarget::default();
            let publication = plan(&dependencies)
                .publish_with(&mut ring, 62, &mut target)
                .unwrap();
            let barriers = dependency_count.div_ceil(AQL_BARRIER_AND_FAN_IN_V1);
            assert_eq!(publication.dependency_count(), dependency_count as u16);
            assert_eq!(publication.barrier_count(), barriers as u16);
            assert_eq!(publication.packet_count(), barriers as u32 + 1);
            assert_eq!(publication.first_packet_id(), 62);
            assert_eq!(publication.last_packet_id(), 62 + barriers as u64);
            assert_eq!(ring.write(), 63 + barriers as u64);

            assert!(matches!(target.events[0], Event::Claim { .. }));
            assert!(
                target.events[1..1 + barriers]
                    .iter()
                    .all(|event| matches!(event, Event::BarrierBody { .. }))
            );
            assert!(matches!(
                target.events[1 + barriers],
                Event::DispatchBody { .. }
            ));
            assert!(
                target.events[2 + barriers..2 + 2 * barriers]
                    .iter()
                    .all(|event| matches!(event, Event::BarrierHeader { header: 0x1403, .. }))
            );
            assert_eq!(
                target.events[2 + 2 * barriers],
                Event::DispatchHeader {
                    packet_id: 62 + barriers as u64,
                    header: if dependency_count == 0 {
                        0x1402
                    } else {
                        0x1502
                    },
                }
            );
            assert_eq!(
                target.events[3 + 2 * barriers],
                Event::Doorbell {
                    packet_id: 62 + barriers as u64,
                }
            );
        }
    }

    #[test]
    fn maximum_plan_reserves_53_distinct_wrap_aware_slots() {
        let dependencies = dependencies(256);
        let mut ring = ring(40, 40);
        let mut target = CaptureTarget::default();
        let publication = plan(&dependencies)
            .publish_with(&mut ring, 40, &mut target)
            .unwrap();
        assert_eq!(publication.packet_count(), 53);
        assert_eq!(publication.last_packet_id(), 92);
        let body_slots = target.events[1..54]
            .iter()
            .map(|event| match event {
                Event::BarrierBody { slot, .. } | Event::DispatchBody { slot, .. } => *slot,
                other => panic!("unexpected body event: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(&body_slots[..24], &(40_u32..64).collect::<Vec<_>>());
        assert_eq!(&body_slots[24..], &(0_u32..29).collect::<Vec<_>>());
        let mut distinct = body_slots.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 53);
    }

    #[test]
    fn occupancy_retry_returns_owner_and_leaves_ring_and_target_unchanged() {
        for (write, expected) in [
            (64, AqlRingReservationError::Full),
            (
                63,
                AqlRingReservationError::InsufficientSpace {
                    requested: 3,
                    available: 1,
                },
            ),
        ] {
            let dependencies = dependencies(6);
            let mut blocked_ring = ring(write, 0);
            let before = (blocked_ring.write(), blocked_ring.last_read());
            let mut target = CaptureTarget::default();
            let failure = plan(&dependencies)
                .publish_with(&mut blocked_ring, 0, &mut target)
                .unwrap_err();
            let prepared = match failure {
                AqlDependencyDispatchPublicationFailureV1::RetryableBeforeSideEffect {
                    error,
                    prepared,
                } => {
                    assert_eq!(error, expected);
                    prepared
                }
                other => panic!("unexpected failure: {other:?}"),
            };
            assert_eq!((blocked_ring.write(), blocked_ring.last_read()), before);
            assert!(target.events.is_empty());
            assert_eq!(prepared.dependency_count(), 6);

            let mut retry_ring = ring(0, 0);
            let publication = prepared
                .publish_with(&mut retry_ring, 0, &mut target)
                .unwrap();
            assert_eq!(publication.packet_count(), 3);
        }
    }

    #[test]
    fn every_dependency_count_obeys_exact_minimum_ring_capacity() {
        for dependency_count in 0..=AQL_MAX_DEPENDENCY_SIGNALS_V1 {
            let dependencies = dependencies(dependency_count);
            let packet_count = dependency_count.div_ceil(AQL_BARRIER_AND_FAN_IN_V1) as u32 + 1;
            for occupancy in 0_u64..=64 {
                let mut candidate = ring(1_000 + occupancy, 1_000);
                let before = (candidate.write(), candidate.last_read());
                let mut target = CaptureTarget::default();
                let result = plan(&dependencies).publish_with(&mut candidate, 1_000, &mut target);
                let available = 64 - occupancy;
                if u64::from(packet_count) <= available {
                    let publication = result.unwrap();
                    assert_eq!(publication.packet_count(), packet_count);
                    assert_eq!(candidate.write(), before.0 + u64::from(packet_count));
                    assert_eq!(candidate.last_read(), 1_000);
                    assert_eq!(target.events.len(), 2 * packet_count as usize + 2);
                } else {
                    let failure = result.unwrap_err();
                    match failure {
                        AqlDependencyDispatchPublicationFailureV1::RetryableBeforeSideEffect {
                            error,
                            prepared,
                        } => {
                            let expected = if available == 0 {
                                AqlRingReservationError::Full
                            } else {
                                AqlRingReservationError::InsufficientSpace {
                                    requested: packet_count,
                                    available: available as u32,
                                }
                            };
                            assert_eq!(error, expected);
                            assert_eq!(prepared.dependency_count(), dependency_count as u16);
                        }
                        other => panic!("unexpected failure: {other:?}"),
                    }
                    assert_eq!((candidate.write(), candidate.last_read()), before);
                    assert!(target.events.is_empty());
                }
            }
        }
    }

    #[test]
    fn invariant_rejection_is_definite_pre_effect_and_returns_owner() {
        let dependencies = dependencies(1);
        let mut ring = ring(10, 5);
        let before = (ring.write(), ring.last_read());
        let mut target = CaptureTarget::default();
        let failure = plan(&dependencies)
            .publish_with(&mut ring, 4, &mut target)
            .unwrap_err();
        match failure {
            AqlDependencyDispatchPublicationFailureV1::RejectedBeforeSideEffect {
                error,
                prepared,
            } => {
                assert_eq!(error, AqlRingReservationError::ReadRegressed);
                assert_eq!(prepared.dependency_count(), 1);
            }
            other => panic!("unexpected failure: {other:?}"),
        }
        assert_eq!((ring.write(), ring.last_read()), before);
        assert!(target.events.is_empty());
    }

    #[test]
    fn every_target_fault_is_terminal_with_exact_nonretryable_custody() {
        let dependencies = dependencies(256);
        let successful_event_count = 2 * AQL_MAX_DEPENDENCY_BARRIER_PACKETS_V1 + 4;
        for fail_at in 0..successful_event_count {
            let mut ring = ring(40, 40);
            let mut target = CaptureTarget {
                fail_at: Some(fail_at),
                ..CaptureTarget::default()
            };
            let failure = plan(&dependencies)
                .publish_with(&mut ring, 40, &mut target)
                .unwrap_err();
            let expected_boundary = boundary_for_event(*target.events.last().unwrap());
            match failure {
                AqlDependencyDispatchPublicationFailureV1::TerminalAmbiguous {
                    error,
                    boundary,
                    custody,
                } => {
                    assert_eq!(error, "injected publication failure");
                    assert_eq!(boundary, expected_boundary);
                    assert_eq!(custody.dependency_count(), 256);
                    assert_eq!(custody.barrier_count(), 52);
                    assert_eq!(custody.reservation().first_packet_id(), 40);
                    assert_eq!(custody.reservation().last_packet_id(), 92);
                }
                other => panic!("unexpected failure: {other:?}"),
            }
            assert_eq!(target.events.len(), fail_at + 1);
            assert_eq!(ring.write(), 93);
            assert_eq!(ring.last_read(), 40);
        }
    }

    fn boundary_for_event(event: Event) -> AqlDependencyDispatchPublicationBoundaryV1 {
        match event {
            Event::Claim { .. } => AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
            Event::BarrierBody { packet_id, .. } => {
                AqlDependencyDispatchPublicationBoundaryV1::BarrierBody {
                    barrier_index: (packet_id - 40) as u32,
                }
            }
            Event::DispatchBody { .. } => {
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchBody
            }
            Event::BarrierHeader { packet_id, .. } => {
                AqlDependencyDispatchPublicationBoundaryV1::BarrierHeader {
                    barrier_index: (packet_id - 40) as u32,
                }
            }
            Event::DispatchHeader { .. } => {
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader
            }
            Event::Doorbell { .. } => AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
        }
    }

    fn signal(index: usize) -> AqlDependencySignalObservationV1 {
        AqlDependencySignalObservationV1::new(0x10_0000 + index as u64 * 64).unwrap()
    }

    fn dependencies(count: usize) -> Vec<AqlDependencySignalObservationV1> {
        (0..count).map(signal).collect()
    }

    fn dispatch() -> AqlPreparedKernelDispatchV1 {
        crate::AqlKernelDispatchPacketV1::new_unpublished(
            crate::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            0,
            ObservedGpuAddressV1::new(0x1000).unwrap(),
            ObservedGpuAddressV1::new(0x2000).unwrap(),
            16,
            ObservedGpuAddressV1::new(0x3000).unwrap(),
        )
        .unwrap()
    }

    fn plan(dependencies: &[AqlDependencySignalObservationV1]) -> AqlPreparedDependencyDispatchV1 {
        AqlPreparedDependencyDispatchV1::new(dependencies, dispatch()).unwrap()
    }

    fn ring(write: u64, read: u64) -> AqlSingleProducerRingModelV1 {
        AqlSingleProducerRingModelV1::new(
            crate::AqlRingCapacityV1::from_ring_bytes(4096).unwrap(),
            write,
            read,
        )
        .unwrap()
    }

    fn hex(bytes: &[u8]) -> alloc::string::String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut value = alloc::string::String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            value.push(char::from(DIGITS[usize::from(*byte >> 4)]));
            value.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
        }
        value
    }
}
