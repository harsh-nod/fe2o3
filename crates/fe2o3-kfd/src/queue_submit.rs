//! Private, single-producer AQL reservation and publication boundary.
//!
//! This module is the only bridge from inert `fe2o3-aql` packet values to a
//! retained native queue. It deliberately provides no public submission,
//! address, pointer, or MMIO API. GPU participation in the shared counters and
//! packet bytes is an external, source-pinned contract rather than a Rust
//! atomic-memory-model proof.

use core::sync::atomic::{AtomicU32, AtomicU64};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[cfg(test)]
use core::sync::atomic::{Ordering, fence};

use fe2o3_aql::{
    AQL_INVALID_PACKET_HEADER_V1, AQL_KERNEL_DISPATCH_PACKET_BYTES_V1, AqlBarrierAndPacketV1,
    AqlBarrierAndPublicationTargetV1, AqlDependencyBarrierPacketV1,
    AqlDependencyDispatchPublicationBoundaryV1, AqlDependencyDispatchPublicationFailureV1,
    AqlDependencyDispatchPublicationTargetV1, AqlDependencyDispatchPublicationV1,
    AqlKernelDispatchPacketV1, AqlPacketBatchPublicationTargetV1, AqlPreparedBarrierAndV1,
    AqlPreparedDependencyDispatchV1, AqlPreparedKernelDispatchBatchV2,
    AqlRingBatchReservationEntryV1, AqlRingBatchReservationV1, AqlRingCapacityV1,
    AqlRingReservationError, AqlSingleProducerRingModelV1, AqlTerminalDependencyDispatchCustodyV1,
};
use fe2o3_kfd_uapi::{
    KfdContextSaveAreaHeaderV1, KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1,
};

#[cfg(test)]
use fe2o3_aql::{AqlPreparedKernelDispatchV1, is_reviewed_aql_publication_v1};

pub(crate) const GFX942_CWSR_XCC_COUNT_V1: usize = 8;
pub(crate) const GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1: usize = 0x162_1000;
pub(crate) const GFX942_CWSR_TOTAL_BYTES_V1: usize = 0xb16_7000;
pub(crate) const GFX942_CWSR_DEBUG_BYTES_TOTAL_V1: u32 = 0x5_f000;
pub(crate) const CWSR_HEADER_BYTES: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubmissionPhaseV1 {
    Ready,
    Poisoned,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NativeAqlSubmissionErrorV1 {
    InvalidQueue(&'static str),
    InvalidRing(&'static str),
    InvalidCwsr(&'static str),
    Poisoned,
    Currentness,
    CounterObservation,
    WriteCounterReplay { expected: u64, observed: u64 },
    Ring(AqlRingReservationError),
    WriteCounterRace { expected: u64, observed: u64 },
    PacketBody,
    PacketHeader,
    Doorbell,
    CallbackPanic,
}

/// Side-effect classification for one kernel-dispatch batch submission.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum NativeAqlSubmissionFailureV1 {
    RetryableBeforeSideEffect(NativeAqlSubmissionErrorV1),
    Terminal(NativeAqlSubmissionErrorV1),
}

impl NativeAqlSubmissionFailureV1 {
    pub(super) fn into_error(self) -> NativeAqlSubmissionErrorV1 {
        match self {
            Self::RetryableBeforeSideEffect(error) | Self::Terminal(error) => error,
        }
    }
}

/// Side-effect classification for the isolated BARRIER_AND submission.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum NativeBarrierAndSubmissionFailureV1 {
    RetryableBeforeSideEffect(NativeAqlSubmissionErrorV1),
    Terminal(NativeAqlSubmissionErrorV1),
}

/// Side-effect classification for one dependency-ordered dispatch.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum NativeDependencyDispatchSubmissionFailureV1 {
    /// Ring occupancy rejected the complete plan before the claim callback.
    RetryableBeforeSideEffect {
        error: NativeAqlSubmissionErrorV1,
        prepared: AqlPreparedDependencyDispatchV1,
    },
    /// A fail-closed native precondition rejected before the claim callback.
    TerminalBeforeClaim {
        error: NativeAqlSubmissionErrorV1,
        prepared: AqlPreparedDependencyDispatchV1,
    },
    /// The write-index claim callback was attempted or a later callback failed.
    TerminalAmbiguous {
        error: NativeAqlSubmissionErrorV1,
        boundary: AqlDependencyDispatchPublicationBoundaryV1,
        custody: AqlTerminalDependencyDispatchCustodyV1,
    },
}

/// Linear state for one retained single-producer native queue.
///
/// This type is intentionally not `Clone`. Counter divergence, invalid
/// monotonic observations, currentness loss, and every possible native side
/// effect poison it. Only an ordinary full or insufficient-space observation
/// before the write-counter reservation is retryable.
pub(super) struct NativeAqlSubmissionOwnerV1 {
    ring: AqlSingleProducerRingModelV1,
    phase: SubmissionPhaseV1,
}

impl NativeAqlSubmissionOwnerV1 {
    #[cfg(test)]
    pub(super) fn is_poisoned_for_test(&self) -> bool {
        self.phase == SubmissionPhaseV1::Poisoned
    }

    pub(super) fn new(ring_bytes: u32) -> Result<Self, NativeAqlSubmissionErrorV1> {
        Self::from_counters(ring_bytes, 0, 0)
    }

    pub(super) fn poison(&mut self) {
        self.phase = SubmissionPhaseV1::Poisoned;
    }

    pub(super) fn from_counters(
        ring_bytes: u32,
        write: u64,
        read: u64,
    ) -> Result<Self, NativeAqlSubmissionErrorV1> {
        let capacity = AqlRingCapacityV1::from_ring_bytes(ring_bytes)
            .map_err(|_| NativeAqlSubmissionErrorV1::InvalidRing("capacity"))?;
        let ring = AqlSingleProducerRingModelV1::new(capacity, write, read)
            .map_err(NativeAqlSubmissionErrorV1::Ring)?;
        Ok(Self {
            ring,
            phase: SubmissionPhaseV1::Ready,
        })
    }

    #[cfg(test)]
    pub(super) fn submit<B: NativeAqlSubmissionBackendV1>(
        &mut self,
        packet: AqlPreparedKernelDispatchV1,
        backend: &mut B,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.submit_batch(AqlPreparedKernelDispatchBatchV2::one(packet), backend)
    }

    #[cfg(test)]
    pub(super) fn submit_batch<const N: usize, B: NativeAqlSubmissionBackendV1>(
        &mut self,
        batch: AqlPreparedKernelDispatchBatchV2<N>,
        backend: &mut B,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.submit_batch_classified(batch, backend)
            .map_err(NativeAqlSubmissionFailureV1::into_error)
    }

    pub(super) fn submit_batch_classified<const N: usize, B: NativeAqlSubmissionBackendV1>(
        &mut self,
        batch: AqlPreparedKernelDispatchBatchV2<N>,
        backend: &mut B,
    ) -> Result<u64, NativeAqlSubmissionFailureV1> {
        if self.phase != SubmissionPhaseV1::Ready {
            return Err(NativeAqlSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned,
            ));
        }

        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeAqlSubmissionFailureV1::Terminal(error));
        }
        let (observed_write, observed_read) = match backend.observe_counters_acquire() {
            Ok(observation) => observation,
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeAqlSubmissionFailureV1::Terminal(error));
            }
        };
        let expected_write = self.ring.write();
        if observed_write != expected_write {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeAqlSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::WriteCounterReplay {
                    expected: expected_write,
                    observed: observed_write,
                },
            ));
        }

        // This is the final check after bounded read/capacity preparation and
        // before the first shared-memory side effect.
        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeAqlSubmissionFailureV1::Terminal(error));
        }
        let reservation = match self
            .ring
            .reserve_fixed_batch_v2(observed_read, batch.packet_count())
        {
            Ok(reservation) => reservation,
            Err(error) if retryable_occupancy(&error) => {
                return Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                    NativeAqlSubmissionErrorV1::Ring(error),
                ));
            }
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Ring(error),
                ));
            }
        };

        // From here on, even a reported error may follow a native side effect.
        self.phase = SubmissionPhaseV1::Poisoned;
        let old_write = backend
            .fetch_add_write_acq_rel(u64::from(batch.packet_count()))
            .map_err(NativeAqlSubmissionFailureV1::Terminal)?;
        if old_write != reservation.first_packet_id() {
            return Err(NativeAqlSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::WriteCounterRace {
                    expected: reservation.first_packet_id(),
                    observed: old_write,
                },
            ));
        }

        let mut target = NativePacketBatchTargetV1 {
            backend,
            reservation: &reservation,
        };
        batch
            .publish_with(&mut target)
            .map_err(NativeAqlSubmissionFailureV1::Terminal)?;

        // Every packet is already published here. Failure prevents MMIO but
        // is not recoverable or retryable by this owner.
        backend
            .check_currentness()
            .map_err(NativeAqlSubmissionFailureV1::Terminal)?;
        backend
            .ring_doorbell_release(reservation.last_packet_id())
            .map_err(NativeAqlSubmissionFailureV1::Terminal)?;
        self.phase = SubmissionPhaseV1::Ready;
        Ok(reservation.last_packet_id())
    }

    /// Publishes one complete b37 dependency plan through the retained queue.
    ///
    /// Capacity rejection is the only retryable result. The owner is poisoned
    /// before `publish_with` can invoke its first callback; callback errors and
    /// panics therefore retain nonretryable planner custody.
    pub(super) fn submit_dependency_dispatch_classified<B: NativeAqlSubmissionBackendV1>(
        &mut self,
        prepared: AqlPreparedDependencyDispatchV1,
        backend: &mut B,
    ) -> Result<AqlDependencyDispatchPublicationV1, NativeDependencyDispatchSubmissionFailureV1>
    {
        if self.phase != SubmissionPhaseV1::Ready {
            return Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error: NativeAqlSubmissionErrorV1::Poisoned,
                    prepared,
                },
            );
        }
        if let Err(error) = catch_dependency_callback(|| backend.check_currentness()) {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error,
                    prepared,
                },
            );
        }
        let (observed_write, observed_read) =
            match catch_dependency_callback(|| backend.observe_counters_acquire()) {
                Ok(observation) => observation,
                Err(error) => {
                    self.phase = SubmissionPhaseV1::Poisoned;
                    return Err(
                        NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                            error,
                            prepared,
                        },
                    );
                }
            };
        let expected_write = self.ring.write();
        if observed_write != expected_write {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error: NativeAqlSubmissionErrorV1::WriteCounterReplay {
                        expected: expected_write,
                        observed: observed_write,
                    },
                    prepared,
                },
            );
        }
        if let Err(error) = catch_dependency_callback(|| backend.check_currentness()) {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error,
                    prepared,
                },
            );
        }

        self.phase = SubmissionPhaseV1::Poisoned;
        let mut target = NativeDependencyDispatchTargetV1 {
            backend,
            expected_first_packet_id: expected_write,
        };
        match prepared.publish_with(&mut self.ring, observed_read, &mut target) {
            Ok(publication) => {
                self.phase = SubmissionPhaseV1::Ready;
                Ok(publication)
            }
            Err(AqlDependencyDispatchPublicationFailureV1::RetryableBeforeSideEffect {
                error,
                prepared,
            }) => {
                self.phase = SubmissionPhaseV1::Ready;
                Err(
                    NativeDependencyDispatchSubmissionFailureV1::RetryableBeforeSideEffect {
                        error: NativeAqlSubmissionErrorV1::Ring(error),
                        prepared,
                    },
                )
            }
            Err(AqlDependencyDispatchPublicationFailureV1::RejectedBeforeSideEffect {
                error,
                prepared,
            }) => Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                    error: NativeAqlSubmissionErrorV1::Ring(error),
                    prepared,
                },
            ),
            Err(AqlDependencyDispatchPublicationFailureV1::TerminalAmbiguous {
                error,
                boundary,
                custody,
            }) => Err(
                NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                    error,
                    boundary,
                    custody,
                },
            ),
        }
    }

    /// Publishes exactly one zero-dependency BARRIER_AND packet.
    pub(super) fn submit_barrier_and<B: NativeAqlSubmissionBackendV1>(
        &mut self,
        packet: AqlPreparedBarrierAndV1,
        backend: &mut B,
    ) -> Result<u64, NativeBarrierAndSubmissionFailureV1> {
        if self.phase != SubmissionPhaseV1::Ready {
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned,
            ));
        }
        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(error));
        }
        let (observed_write, observed_read) = match backend.observe_counters_acquire() {
            Ok(observation) => observation,
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeBarrierAndSubmissionFailureV1::Terminal(error));
            }
        };
        let expected_write = self.ring.write();
        if observed_write != expected_write {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::WriteCounterReplay {
                    expected: expected_write,
                    observed: observed_write,
                },
            ));
        }
        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(error));
        }
        let reservation = match self.ring.reserve_one(observed_read) {
            Ok(reservation) => reservation,
            Err(error) if retryable_occupancy(&error) => {
                return Err(
                    NativeBarrierAndSubmissionFailureV1::RetryableBeforeSideEffect(
                        NativeAqlSubmissionErrorV1::Ring(error),
                    ),
                );
            }
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Ring(error),
                ));
            }
        };

        self.phase = SubmissionPhaseV1::Poisoned;
        let old_write = backend
            .fetch_add_write_acq_rel(1)
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        if old_write != reservation.packet_id() {
            return Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::WriteCounterRace {
                    expected: reservation.packet_id(),
                    observed: old_write,
                },
            ));
        }
        let mut target = NativeBarrierAndTargetV1 {
            backend,
            slot: reservation.slot_index(),
        };
        packet
            .publish_with(&mut target)
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        backend
            .check_currentness()
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        backend
            .ring_doorbell_release(reservation.packet_id())
            .map_err(NativeBarrierAndSubmissionFailureV1::Terminal)?;
        self.phase = SubmissionPhaseV1::Ready;
        Ok(reservation.packet_id())
    }
}

fn retryable_occupancy(error: &AqlRingReservationError) -> bool {
    matches!(
        error,
        AqlRingReservationError::Full | AqlRingReservationError::InsufficientSpace { .. }
    )
}

pub(super) trait NativeAqlSubmissionBackendV1 {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1>;
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1>;
    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1>;
    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
    ) -> Result<(), NativeAqlSubmissionErrorV1>;
    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1>;
    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1>;
}

struct NativePacketBatchTargetV1<'a, B> {
    backend: &'a mut B,
    reservation: &'a AqlRingBatchReservationV1,
}

impl<B: NativeAqlSubmissionBackendV1> AqlPacketBatchPublicationTargetV1
    for NativePacketBatchTargetV1<'_, B>
{
    type Error = NativeAqlSubmissionErrorV1;

    fn write_unpublished(
        &mut self,
        batch_index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error> {
        let entry = self
            .reservation
            .entry(batch_index)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidRing("batch body index"))?;
        self.backend
            .write_unpublished(entry.slot_index(), &packet.encode_unpublished_le())
    }

    fn publish_release_header(&mut self, batch_index: u32, header: u16) -> Result<(), Self::Error> {
        let entry =
            self.reservation
                .entry(batch_index)
                .ok_or(NativeAqlSubmissionErrorV1::InvalidRing(
                    "batch header index",
                ))?;
        self.backend
            .publish_release_header(entry.slot_index(), header)
    }
}

struct NativeBarrierAndTargetV1<'a, B> {
    backend: &'a mut B,
    slot: u32,
}

struct NativeDependencyDispatchTargetV1<'a, B> {
    backend: &'a mut B,
    expected_first_packet_id: u64,
}

fn catch_dependency_callback<T>(
    operation: impl FnOnce() -> Result<T, NativeAqlSubmissionErrorV1>,
) -> Result<T, NativeAqlSubmissionErrorV1> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(_) => Err(NativeAqlSubmissionErrorV1::CallbackPanic),
    }
}

impl<B: NativeAqlSubmissionBackendV1> AqlDependencyDispatchPublicationTargetV1
    for NativeDependencyDispatchTargetV1<'_, B>
{
    type Error = NativeAqlSubmissionErrorV1;

    fn claim_write_index_acq_rel(
        &mut self,
        first_packet_id: u64,
        packet_count: u32,
    ) -> Result<(), Self::Error> {
        if first_packet_id != self.expected_first_packet_id {
            return Err(NativeAqlSubmissionErrorV1::InvalidRing(
                "dependency reservation replay",
            ));
        }
        catch_dependency_callback(|| self.backend.check_currentness())?;
        let observed = catch_dependency_callback(|| {
            self.backend
                .fetch_add_write_acq_rel(u64::from(packet_count))
        })?;
        if observed != first_packet_id {
            return Err(NativeAqlSubmissionErrorV1::WriteCounterRace {
                expected: first_packet_id,
                observed,
            });
        }
        Ok(())
    }

    fn write_unpublished_barrier(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        packet: &AqlDependencyBarrierPacketV1,
    ) -> Result<(), Self::Error> {
        catch_dependency_callback(|| {
            self.backend
                .write_unpublished(entry.slot_index(), &packet.encode_unpublished_le())
        })
    }

    fn write_unpublished_dispatch(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error> {
        catch_dependency_callback(|| {
            self.backend
                .write_unpublished(entry.slot_index(), &packet.encode_unpublished_le())
        })
    }

    fn publish_barrier_release_header(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        header: u16,
    ) -> Result<(), Self::Error> {
        catch_dependency_callback(|| {
            self.backend
                .publish_release_header(entry.slot_index(), header)
        })
    }

    fn publish_dispatch_release_header(
        &mut self,
        entry: AqlRingBatchReservationEntryV1,
        header: u16,
    ) -> Result<(), Self::Error> {
        catch_dependency_callback(|| {
            self.backend
                .publish_release_header(entry.slot_index(), header)
        })
    }

    fn ring_doorbell_release(&mut self, packet_id: u64) -> Result<(), Self::Error> {
        catch_dependency_callback(|| self.backend.check_currentness())?;
        catch_dependency_callback(|| self.backend.ring_doorbell_release(packet_id))
    }
}

impl<B: NativeAqlSubmissionBackendV1> AqlBarrierAndPublicationTargetV1
    for NativeBarrierAndTargetV1<'_, B>
{
    type Error = NativeAqlSubmissionErrorV1;

    fn write_unpublished_barrier(
        &mut self,
        packet: &AqlBarrierAndPacketV1,
    ) -> Result<(), Self::Error> {
        self.backend
            .write_unpublished(self.slot, &packet.encode_unpublished_le())
    }

    fn publish_barrier_release_header(&mut self, header: u16) -> Result<(), Self::Error> {
        self.backend.publish_release_header(self.slot, header)
    }
}

pub(super) fn initialize_invalid_ring(bytes: &mut [u8]) -> Result<(), NativeAqlSubmissionErrorV1> {
    let ring_bytes = u32::try_from(bytes.len())
        .map_err(|_| NativeAqlSubmissionErrorV1::InvalidRing("length"))?;
    AqlRingCapacityV1::from_ring_bytes(ring_bytes)
        .map_err(|_| NativeAqlSubmissionErrorV1::InvalidRing("capacity"))?;
    for slot in bytes.chunks_exact_mut(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1) {
        slot.fill(0);
        let pointer = slot.as_mut_ptr().cast::<AtomicU32>();
        if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU32>()) {
            return Err(NativeAqlSubmissionErrorV1::InvalidRing("header alignment"));
        }
        // SAFETY: this exclusively borrowed, aligned 64-byte slot has room
        // for the AtomicU32 header object, which is created before GPU map.
        unsafe { pointer.write(AtomicU32::new(u32::from(AQL_INVALID_PACKET_HEADER_V1))) };
    }
    Ok(())
}

pub(crate) fn initialize_amd_aql_control(
    bytes: &mut [u8],
) -> Result<(), NativeAqlSubmissionErrorV1> {
    if bytes.len() != 4096 {
        return Err(NativeAqlSubmissionErrorV1::CounterObservation);
    }
    bytes.fill(0);
    for offset in [
        crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
        crate::queue_resources::AMD_AQL_READ_DISPATCH_ID_OFFSET_V1,
    ] {
        let pointer = bytes[offset..].as_mut_ptr().cast::<AtomicU64>();
        if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU64>()) {
            return Err(NativeAqlSubmissionErrorV1::CounterObservation);
        }
        // SAFETY: each exact aligned 8-byte range is exclusively borrowed and
        // initialized as AtomicU64 before the mapping becomes GPU accessible.
        unsafe { pointer.write(AtomicU64::new(0)) };
    }
    let field_offset = crate::queue_resources::AMD_AQL_READ_BASE_OFFSET_FIELD_V1;
    bytes[field_offset..field_offset + core::mem::size_of::<u32>()]
        .copy_from_slice(&crate::queue_resources::AMD_AQL_READ_BASE_OFFSET_VALUE_V1.to_le_bytes());
    Ok(())
}

pub(crate) fn gfx942_cwsr_header_bytes(
    xcc: usize,
    payload: KfdQueueExceptionPayloadAddressV1,
    event_id: KfdSignalEventIdV1,
) -> Result<[u8; CWSR_HEADER_BYTES], NativeAqlSubmissionErrorV1> {
    if xcc >= GFX942_CWSR_XCC_COUNT_V1 {
        return Err(NativeAqlSubmissionErrorV1::InvalidCwsr("XCC index"));
    }
    let debug_offset = u32::try_from(
        (GFX942_CWSR_XCC_COUNT_V1 - xcc)
            .checked_mul(GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("debug offset"))?,
    )
    .map_err(|_| NativeAqlSubmissionErrorV1::InvalidCwsr("debug offset width"))?;
    let header = KfdContextSaveAreaHeaderV1::new_queue_exception(
        debug_offset,
        GFX942_CWSR_DEBUG_BYTES_TOTAL_V1,
        payload,
        event_id,
    )
    .map_err(|_| NativeAqlSubmissionErrorV1::InvalidCwsr("typed header"))?;
    let mut bytes = [0_u8; CWSR_HEADER_BYTES];
    for (index, word) in header.wave_state_words().iter().enumerate() {
        let offset = index * 4;
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes[16..20].copy_from_slice(&header.debug_offset().to_le_bytes());
    bytes[20..24].copy_from_slice(&header.debug_size().to_le_bytes());
    bytes[24..32].copy_from_slice(&header.error_payload_address().to_le_bytes());
    bytes[32..36].copy_from_slice(&header.error_event_id().to_le_bytes());
    bytes[36..40].copy_from_slice(&header.reserved().to_le_bytes());
    Ok(bytes)
}

/// Reproduces the pinned ROCr `fill_cwsr_header` layout with one exact event.
pub(crate) fn initialize_gfx942_cwsr_headers(
    bytes: &mut [u8],
    payload: KfdQueueExceptionPayloadAddressV1,
    event_id: KfdSignalEventIdV1,
) -> Result<(), NativeAqlSubmissionErrorV1> {
    if bytes.len() != GFX942_CWSR_TOTAL_BYTES_V1 {
        return Err(NativeAqlSubmissionErrorV1::InvalidCwsr("mapping length"));
    }
    for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
        let offset = xcc
            .checked_mul(GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header offset"))?;
        let end = offset
            .checked_add(CWSR_HEADER_BYTES)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header end"))?;
        let destination = bytes
            .get_mut(offset..end)
            .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header range"))?;
        destination.copy_from_slice(&gfx942_cwsr_header_bytes(xcc, payload, event_id)?);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn write_unpublished_slot(
    bytes: &mut [u8],
    slot_index: u32,
    encoded: &[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
) -> Result<(), NativeAqlSubmissionErrorV1> {
    let slot = packet_slot(bytes, slot_index)?;
    if u32::from_le_bytes(encoded[..4].try_into().expect("four header bytes")) & 0xffff
        != u32::from(AQL_INVALID_PACKET_HEADER_V1)
    {
        return Err(NativeAqlSubmissionErrorV1::PacketBody);
    }
    let pointer = slot.as_mut_ptr().cast::<AtomicU32>();
    if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU32>()) {
        return Err(NativeAqlSubmissionErrorV1::PacketBody);
    }
    // SAFETY: fake storage initialized this exact header AtomicU32 before use.
    unsafe { &*pointer }.store(
        u32::from_le_bytes(encoded[..4].try_into().expect("four header bytes")).to_le(),
        Ordering::Relaxed,
    );
    slot[4..].copy_from_slice(&encoded[4..]);
    Ok(())
}

#[cfg(test)]
pub(super) fn publish_slot_header_release(
    bytes: &mut [u8],
    slot_index: u32,
    header: u16,
) -> Result<(), NativeAqlSubmissionErrorV1> {
    let slot = packet_slot(bytes, slot_index)?;
    let pointer = slot.as_mut_ptr().cast::<AtomicU32>();
    if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU32>()) {
        return Err(NativeAqlSubmissionErrorV1::PacketHeader);
    }
    // SAFETY: fake storage initialized this exact header AtomicU32 before use.
    let atomic = unsafe { &*pointer };
    let unpublished = u32::from_le(atomic.load(Ordering::Relaxed));
    let setup = unpublished >> 16;
    if unpublished & 0xffff != u32::from(AQL_INVALID_PACKET_HEADER_V1)
        || !is_reviewed_aql_publication_v1(header, setup as u16)
    {
        return Err(NativeAqlSubmissionErrorV1::PacketHeader);
    }
    let final_header = (setup << 16) | u32::from(header);
    // SAFETY: `slot` is an exclusive 64-byte slice, the checked pointer is
    // aligned, and an AtomicU32 fits at offset zero. The x86_64 little-endian
    // target makes this the exact LE full-header publication.
    atomic.store(final_header.to_le(), Ordering::Release);
    Ok(())
}

#[cfg(test)]
fn packet_slot(bytes: &mut [u8], slot_index: u32) -> Result<&mut [u8], NativeAqlSubmissionErrorV1> {
    let offset = usize::try_from(slot_index)
        .ok()
        .and_then(|index| index.checked_mul(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1))
        .ok_or(NativeAqlSubmissionErrorV1::PacketBody)?;
    let end = offset
        .checked_add(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1)
        .ok_or(NativeAqlSubmissionErrorV1::PacketBody)?;
    bytes
        .get_mut(offset..end)
        .ok_or(NativeAqlSubmissionErrorV1::PacketBody)
}

#[cfg(test)]
fn release_fence_before_mmio() {
    fence(Ordering::Release);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SFENCE has no memory operand and is available on every x86_64
    // CPU admitted by this crate's platform profile.
    unsafe {
        core::arch::x86_64::_mm_sfence();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_aql::{
        AqlBarrierAndPacketV1, AqlDependencySignalObservationV1, AqlDispatchGeometryV1,
        AqlKernelDispatchPacketV1, AqlPreparedDependencyDispatchV1,
        AqlPreparedKernelDispatchBatchV2, ObservedGpuAddressV1,
    };

    #[repr(align(64))]
    struct AlignedRing([u8; 524_416]);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FailureAfterV1 {
        FetchAdd,
        Body(usize),
        Header(usize),
        Doorbell,
    }

    struct FakeBackend {
        ring: Box<AlignedRing>,
        logical_bytes: usize,
        write: AtomicU64,
        read: AtomicU64,
        checks: usize,
        fail_check: Option<usize>,
        fail_check_error: Option<NativeAqlSubmissionErrorV1>,
        fail_after: Option<FailureAfterV1>,
        fail_error: Option<NativeAqlSubmissionErrorV1>,
        panic_check: Option<usize>,
        panic_after: Option<FailureAfterV1>,
        fetch_return_override: Option<u64>,
        body_calls: usize,
        header_calls: usize,
        trace: Vec<&'static str>,
        doorbells: Vec<u64>,
    }

    impl FakeBackend {
        fn new(write: u64, read: u64) -> Self {
            Self::with_ring_bytes(4_096, write, read)
        }

        fn with_ring_bytes(logical_bytes: usize, write: u64, read: u64) -> Self {
            let mut ring = Box::<AlignedRing>::new_uninit();
            // SAFETY: every byte pattern is valid for the wrapped byte array.
            // Initializing the allocation in place avoids a maximum-ring-sized
            // temporary on the test thread's stack.
            unsafe {
                ring.as_mut_ptr().cast::<u8>().write_bytes(0xa5, 524_416);
            }
            // SAFETY: the complete allocation was initialized above.
            let mut ring = unsafe { ring.assume_init() };
            initialize_invalid_ring(&mut ring.0[64..64 + logical_bytes]).unwrap();
            Self {
                ring,
                logical_bytes,
                write: AtomicU64::new(write),
                read: AtomicU64::new(read),
                checks: 0,
                fail_check: None,
                fail_check_error: None,
                fail_after: None,
                fail_error: None,
                panic_check: None,
                panic_after: None,
                fetch_return_override: None,
                body_calls: 0,
                header_calls: 0,
                trace: Vec::new(),
                doorbells: Vec::new(),
            }
        }

        fn logical_ring(&mut self) -> &mut [u8] {
            &mut self.ring.0[64..64 + self.logical_bytes]
        }

        fn slot_word(&mut self, slot: u32, byte_offset: usize) -> u32 {
            let start = slot as usize * AQL_KERNEL_DISPATCH_PACKET_BYTES_V1 + byte_offset;
            u32::from_le_bytes(self.logical_ring()[start..start + 4].try_into().unwrap())
        }
    }

    impl NativeAqlSubmissionBackendV1 for FakeBackend {
        fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.checks += 1;
            self.trace.push("check");
            assert_ne!(
                self.panic_check,
                Some(self.checks),
                "injected currentness panic"
            );
            if self.fail_check == Some(self.checks) {
                Err(self
                    .fail_check_error
                    .take()
                    .unwrap_or(NativeAqlSubmissionErrorV1::Currentness))
            } else {
                Ok(())
            }
        }

        fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
            self.trace.push("observe");
            Ok((
                self.write.load(Ordering::Acquire),
                self.read.load(Ordering::Acquire),
            ))
        }

        fn fetch_add_write_acq_rel(
            &mut self,
            increment: u64,
        ) -> Result<u64, NativeAqlSubmissionErrorV1> {
            self.trace.push("fetch-add");
            assert_ne!(
                self.panic_after,
                Some(FailureAfterV1::FetchAdd),
                "injected fetch-add panic"
            );
            let observed = self.write.fetch_add(increment, Ordering::AcqRel);
            if self.fail_after == Some(FailureAfterV1::FetchAdd) {
                return Err(self
                    .fail_error
                    .take()
                    .unwrap_or(NativeAqlSubmissionErrorV1::Currentness));
            }
            Ok(self.fetch_return_override.unwrap_or(observed))
        }

        fn write_unpublished(
            &mut self,
            slot: u32,
            packet: &[u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1],
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("body");
            let call = self.body_calls;
            self.body_calls += 1;
            assert_ne!(
                self.panic_after,
                Some(FailureAfterV1::Body(call)),
                "injected body panic"
            );
            write_unpublished_slot(self.logical_ring(), slot, packet)?;
            if self.fail_after == Some(FailureAfterV1::Body(call)) {
                return Err(self
                    .fail_error
                    .take()
                    .unwrap_or(NativeAqlSubmissionErrorV1::PacketBody));
            }
            Ok(())
        }

        fn publish_release_header(
            &mut self,
            slot: u32,
            header: u16,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("header");
            let call = self.header_calls;
            self.header_calls += 1;
            assert_ne!(
                self.panic_after,
                Some(FailureAfterV1::Header(call)),
                "injected header panic"
            );
            publish_slot_header_release(self.logical_ring(), slot, header)?;
            if self.fail_after == Some(FailureAfterV1::Header(call)) {
                return Err(self
                    .fail_error
                    .take()
                    .unwrap_or(NativeAqlSubmissionErrorV1::PacketHeader));
            }
            Ok(())
        }

        fn ring_doorbell_release(
            &mut self,
            packet_id: u64,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("doorbell");
            assert_ne!(
                self.panic_after,
                Some(FailureAfterV1::Doorbell),
                "injected doorbell panic"
            );
            release_fence_before_mmio();
            self.doorbells.push(packet_id);
            if self.fail_after == Some(FailureAfterV1::Doorbell) {
                return Err(self
                    .fail_error
                    .take()
                    .unwrap_or(NativeAqlSubmissionErrorV1::Doorbell));
            }
            Ok(())
        }
    }

    fn packet() -> AqlPreparedKernelDispatchV1 {
        indexed_packet(0)
    }

    fn barrier() -> AqlPreparedBarrierAndV1 {
        AqlBarrierAndPacketV1::new_unpublished(ObservedGpuAddressV1::new(0x30_040).unwrap())
            .unwrap()
    }

    fn indexed_packet(index: u32) -> AqlPreparedKernelDispatchV1 {
        AqlKernelDispatchPacketV1::new_unpublished(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            index,
            ObservedGpuAddressV1::new(0x10_000).unwrap(),
            ObservedGpuAddressV1::new(0x20_000).unwrap(),
            16,
            ObservedGpuAddressV1::new(0x30_000).unwrap(),
        )
        .unwrap()
    }

    fn batch<const N: usize>() -> AqlPreparedKernelDispatchBatchV2<N> {
        let packets = (0..N)
            .map(|index| indexed_packet(index as u32))
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        AqlPreparedKernelDispatchBatchV2::try_from_boxed_packets(packets).unwrap()
    }

    fn dependency_plan(count: usize) -> AqlPreparedDependencyDispatchV1 {
        let signals = (0..count)
            .map(|index| {
                AqlDependencySignalObservationV1::new(0x40_000 + index as u64 * 64).unwrap()
            })
            .collect::<Vec<_>>();
        AqlPreparedDependencyDispatchV1::new(&signals, indexed_packet(7)).unwrap()
    }

    #[test]
    fn invalid_ring_initialization_covers_every_logical_slot() {
        let mut ring = [0xff; 4_096];
        initialize_invalid_ring(&mut ring).unwrap();
        for slot in ring.chunks_exact(64) {
            assert_eq!(&slot[..4], &1_u32.to_le_bytes());
            assert!(slot[4..].iter().all(|byte| *byte == 0));
        }
        assert!(initialize_invalid_ring(&mut [0; 4_095]).is_err());
    }

    #[test]
    fn amd_aql_control_initialization_matches_the_reviewed_layout() {
        #[repr(align(64))]
        struct AlignedControl([u8; 4096]);

        let mut control = AlignedControl([0xff; 4096]);
        initialize_amd_aql_control(&mut control.0).unwrap();
        let mut expected = [0_u8; 4096];
        expected[0x88..0x8c].copy_from_slice(&0x80_u32.to_le_bytes());
        assert_eq!(control.0, expected);
        assert_eq!(
            u64::from_le_bytes(control.0[0x38..0x40].try_into().unwrap()),
            0
        );
        assert_eq!(
            u64::from_le_bytes(control.0[0x80..0x88].try_into().unwrap()),
            0
        );
        assert!(initialize_amd_aql_control(&mut [0; 4095]).is_err());
        assert!(initialize_amd_aql_control(&mut [0; 4097]).is_err());
    }

    #[test]
    fn first_packet_uses_id_zero_and_exact_release_header() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        let untouched_slots = backend.logical_ring()[64..].to_vec();
        assert_eq!(owner.submit(packet(), &mut backend), Ok(0));
        assert_eq!(backend.write.load(Ordering::Relaxed), 1);
        assert_eq!(backend.doorbells, [0]);
        assert_eq!(
            backend.trace,
            [
                "check",
                "observe",
                "check",
                "fetch-add",
                "body",
                "header",
                "check",
                "doorbell"
            ]
        );
        assert_eq!(
            u32::from_le_bytes(backend.logical_ring()[..4].try_into().unwrap()),
            0x0001_1402
        );
        assert!(backend.ring.0[..64].iter().all(|byte| *byte == 0xa5));
        assert!(backend.ring.0[4_160..].iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            &backend.logical_ring()[64..68],
            &u32::from(AQL_INVALID_PACKET_HEADER_V1).to_le_bytes()
        );
        assert_eq!(&backend.logical_ring()[64..], untouched_slots);
    }

    #[test]
    fn zero_dependency_barrier_uses_one_slot_and_exact_release_header() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        let packet =
            AqlBarrierAndPacketV1::new_unpublished(ObservedGpuAddressV1::new(0x30_040).unwrap())
                .unwrap();

        assert_eq!(owner.submit_barrier_and(packet, &mut backend), Ok(0));
        assert_eq!(backend.write.load(Ordering::Relaxed), 1);
        assert_eq!(backend.doorbells, [0]);
        assert_eq!(backend.slot_word(0, 0), 0x0000_1403);
        assert!(backend.logical_ring()[8..48].iter().all(|byte| *byte == 0));
        assert_eq!(
            u64::from_le_bytes(backend.logical_ring()[56..64].try_into().unwrap()),
            0x30_040
        );
    }

    #[test]
    fn dependency_dispatch_uses_one_claim_all_bodies_all_headers_and_one_doorbell() {
        for dependency_count in [0, 1, 5, 6, 256] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            let publication = owner
                .submit_dependency_dispatch_classified(
                    dependency_plan(dependency_count),
                    &mut backend,
                )
                .unwrap();
            let barriers = dependency_count.div_ceil(5);
            assert_eq!(publication.dependency_count(), dependency_count as u16);
            assert_eq!(publication.barrier_count(), barriers as u16);
            assert_eq!(publication.packet_count(), barriers as u32 + 1);
            assert_eq!(
                backend.write.load(Ordering::Relaxed),
                u64::from(publication.packet_count())
            );
            assert_eq!(backend.body_calls, barriers + 1);
            assert_eq!(backend.header_calls, barriers + 1);
            assert_eq!(backend.doorbells, [publication.last_packet_id()]);
            assert_eq!(
                backend
                    .trace
                    .iter()
                    .filter(|event| **event == "fetch-add")
                    .count(),
                1
            );
            assert_eq!(
                backend
                    .trace
                    .iter()
                    .filter(|event| **event == "doorbell")
                    .count(),
                1
            );
            let first_header = backend.slot_word(0, 0) & 0xffff;
            if dependency_count == 0 {
                assert_eq!(first_header, 0x1402);
            } else {
                assert_eq!(first_header, 0x1403);
                let final_slot = barriers as u32;
                assert_eq!(backend.slot_word(final_slot, 0) & 0xffff, 0x1502);
            }
        }
    }

    #[test]
    fn dependency_ring_pressure_is_retryable_and_preserves_the_plan() {
        let mut owner = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
        let mut backend = FakeBackend::new(64, 0);
        let prepared = match owner
            .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
            .unwrap_err()
        {
            NativeDependencyDispatchSubmissionFailureV1::RetryableBeforeSideEffect {
                error: NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full),
                prepared,
            } => prepared,
            failure => panic!("unexpected failure: {failure:?}"),
        };
        assert_eq!(backend.trace, ["check", "observe", "check"]);
        assert_eq!(backend.write.load(Ordering::Relaxed), 64);
        backend.read.store(3, Ordering::Release);
        let publication = owner
            .submit_dependency_dispatch_classified(prepared, &mut backend)
            .unwrap();
        assert_eq!(publication.first_packet_id(), 64);
        assert_eq!(publication.last_packet_id(), 66);
    }

    #[test]
    fn dependency_callback_failures_report_exact_terminal_boundaries() {
        let cases = [
            (
                FailureAfterV1::FetchAdd,
                AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
            ),
            (
                FailureAfterV1::Body(0),
                AqlDependencyDispatchPublicationBoundaryV1::BarrierBody { barrier_index: 0 },
            ),
            (
                FailureAfterV1::Body(2),
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchBody,
            ),
            (
                FailureAfterV1::Header(0),
                AqlDependencyDispatchPublicationBoundaryV1::BarrierHeader { barrier_index: 0 },
            ),
            (
                FailureAfterV1::Header(2),
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader,
            ),
            (
                FailureAfterV1::Doorbell,
                AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
            ),
        ];
        for (stage, expected_boundary) in cases {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_after = Some(stage);
            let failure = owner
                .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
                .unwrap_err();
            assert!(matches!(
                failure,
                NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                    boundary,
                    ..
                } if boundary == expected_boundary
            ));
            assert!(matches!(
                owner.submit_dependency_dispatch_classified(dependency_plan(1), &mut backend),
                Err(
                    NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                        error: NativeAqlSubmissionErrorV1::Poisoned,
                        ..
                    }
                )
            ));
        }
    }

    #[test]
    fn dependency_callback_panics_become_terminal_custody() {
        let cases = [
            (
                None,
                Some(3),
                AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
            ),
            (
                Some(FailureAfterV1::FetchAdd),
                None,
                AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex,
            ),
            (
                Some(FailureAfterV1::Body(0)),
                None,
                AqlDependencyDispatchPublicationBoundaryV1::BarrierBody { barrier_index: 0 },
            ),
            (
                Some(FailureAfterV1::Header(2)),
                None,
                AqlDependencyDispatchPublicationBoundaryV1::FinalDispatchHeader,
            ),
            (
                Some(FailureAfterV1::Doorbell),
                None,
                AqlDependencyDispatchPublicationBoundaryV1::Doorbell,
            ),
        ];
        for (panic_after, panic_check, expected_boundary) in cases {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.panic_after = panic_after;
            backend.panic_check = panic_check;
            let failure = owner
                .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
                .unwrap_err();
            assert!(matches!(
                failure,
                NativeDependencyDispatchSubmissionFailureV1::TerminalAmbiguous {
                    error: NativeAqlSubmissionErrorV1::CallbackPanic,
                    boundary,
                    ..
                } if boundary == expected_boundary
            ));
        }
    }

    #[test]
    fn dependency_currentness_rejection_before_claim_is_terminal_without_ring_effect() {
        for failed_check in [1, 2] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_check = Some(failed_check);
            let before = backend.logical_ring().to_vec();
            assert!(matches!(
                owner.submit_dependency_dispatch_classified(dependency_plan(6), &mut backend),
                Err(
                    NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                        error: NativeAqlSubmissionErrorV1::Currentness,
                        ..
                    }
                )
            ));
            assert_eq!(backend.write.load(Ordering::Relaxed), 0);
            assert!(backend.doorbells.is_empty());
            assert_eq!(backend.logical_ring(), before);
        }
    }

    #[test]
    fn dependency_preclaim_panics_are_terminal_with_exact_plan_custody() {
        for panic_check in [1, 2] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.panic_check = Some(panic_check);
            let before = backend.logical_ring().to_vec();
            let failure = owner
                .submit_dependency_dispatch_classified(dependency_plan(6), &mut backend)
                .unwrap_err();
            let NativeDependencyDispatchSubmissionFailureV1::TerminalBeforeClaim {
                error,
                prepared,
            } = failure
            else {
                panic!("preclaim panic returned nonterminal custody")
            };
            assert_eq!(error, NativeAqlSubmissionErrorV1::CallbackPanic);
            assert_eq!(prepared.dependency_count(), 6);
            assert_eq!(backend.write.load(Ordering::Relaxed), 0);
            assert!(backend.doorbells.is_empty());
            assert_eq!(backend.logical_ring(), before);
        }
    }

    #[test]
    fn barrier_full_is_retryable_but_post_reservation_failure_is_terminal() {
        let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
        let mut full_backend = FakeBackend::new(64, 0);
        assert_eq!(
            full.submit_barrier_and(barrier(), &mut full_backend),
            Err(
                NativeBarrierAndSubmissionFailureV1::RetryableBeforeSideEffect(
                    NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                )
            )
        );
        assert_eq!(full_backend.trace, ["check", "observe", "check"]);
        assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
        assert!(full_backend.doorbells.is_empty());
        full_backend.read.store(1, Ordering::Release);
        assert_eq!(
            full.submit_barrier_and(barrier(), &mut full_backend),
            Ok(64)
        );

        let mut ambiguous = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut ambiguous_backend = FakeBackend::new(0, 0);
        ambiguous_backend.fail_after = Some(FailureAfterV1::Body(0));
        assert_eq!(
            ambiguous.submit_barrier_and(barrier(), &mut ambiguous_backend),
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::PacketBody
            ))
        );
        assert_eq!(ambiguous_backend.write.load(Ordering::Relaxed), 1);
        assert_eq!(
            ambiguous.submit_barrier_and(barrier(), &mut ambiguous_backend),
            Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                NativeAqlSubmissionErrorV1::Poisoned
            ))
        );
    }

    #[test]
    fn barrier_backend_occupancy_shaped_errors_are_terminal_after_reservation() {
        let stages = [
            FailureAfterV1::FetchAdd,
            FailureAfterV1::Body(0),
            FailureAfterV1::Header(0),
            FailureAfterV1::Doorbell,
        ];
        for stage in stages {
            for insufficient in [false, true] {
                let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
                let mut backend = FakeBackend::new(0, 0);
                backend.fail_after = Some(stage);
                backend.fail_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                    AqlRingReservationError::InsufficientSpace {
                        requested: 1,
                        available: 0,
                    }
                } else {
                    AqlRingReservationError::Full
                }));

                let result = owner.submit_barrier_and(barrier(), &mut backend);
                assert!(
                    matches!(
                        &result,
                        Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                            NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                                | NativeAqlSubmissionErrorV1::Ring(
                                    AqlRingReservationError::InsufficientSpace { .. }
                                )
                        ))
                    ),
                    "{stage:?}: {result:?}"
                );
                let trace = backend.trace.clone();
                assert_eq!(
                    owner.submit_barrier_and(barrier(), &mut backend),
                    Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Poisoned
                    )),
                    "{stage:?}"
                );
                assert_eq!(backend.trace, trace, "{stage:?}");
            }
        }

        for insufficient in [false, true] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_check = Some(3);
            backend.fail_check_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                AqlRingReservationError::InsufficientSpace {
                    requested: 1,
                    available: 0,
                }
            } else {
                AqlRingReservationError::Full
            }));

            let result = owner.submit_barrier_and(barrier(), &mut backend);
            assert!(matches!(
                &result,
                Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                        | NativeAqlSubmissionErrorV1::Ring(
                            AqlRingReservationError::InsufficientSpace { .. }
                        )
                ))
            ));
            assert_eq!(backend.write.load(Ordering::Relaxed), 1);
            assert!(backend.doorbells.is_empty());
            let trace = backend.trace.clone();
            assert_eq!(
                owner.submit_barrier_and(barrier(), &mut backend),
                Err(NativeBarrierAndSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned
                ))
            );
            assert_eq!(backend.trace, trace);
        }
    }

    #[test]
    fn full_regressed_and_replayed_counters_have_no_side_effect() {
        let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
        let mut full_backend = FakeBackend::new(64, 0);
        assert_eq!(
            full.submit(packet(), &mut full_backend),
            Err(NativeAqlSubmissionErrorV1::Ring(
                AqlRingReservationError::Full
            ))
        );
        assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
        assert!(full_backend.doorbells.is_empty());
        full_backend.read.store(1, Ordering::Release);
        assert_eq!(full.submit(packet(), &mut full_backend), Ok(64));
        assert_eq!(full_backend.doorbells, [64]);

        let mut regressed = NativeAqlSubmissionOwnerV1::from_counters(4_096, 5, 5).unwrap();
        let mut regressed_backend = FakeBackend::new(5, 4);
        assert_eq!(
            regressed.submit(packet(), &mut regressed_backend),
            Err(NativeAqlSubmissionErrorV1::Ring(
                AqlRingReservationError::ReadRegressed
            ))
        );
        assert_eq!(regressed_backend.write.load(Ordering::Relaxed), 5);
        assert!(regressed_backend.doorbells.is_empty());
        assert_eq!(
            regressed.submit(packet(), &mut regressed_backend),
            Err(NativeAqlSubmissionErrorV1::Poisoned)
        );

        let mut replay = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut replay_backend = FakeBackend::new(1, 0);
        assert_eq!(
            replay.submit(packet(), &mut replay_backend),
            Err(NativeAqlSubmissionErrorV1::WriteCounterReplay {
                expected: 0,
                observed: 1,
            })
        );
        assert_eq!(replay_backend.write.load(Ordering::Relaxed), 1);
        assert!(replay_backend.doorbells.is_empty());
        assert_eq!(
            replay.submit(packet(), &mut replay_backend),
            Err(NativeAqlSubmissionErrorV1::Poisoned)
        );
    }

    #[test]
    fn prepublication_currentness_failure_performs_no_store() {
        for failed_check in [1, 2] {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            let before = backend.logical_ring().to_vec();
            backend.fail_check = Some(failed_check);
            assert_eq!(
                owner.submit(packet(), &mut backend),
                Err(NativeAqlSubmissionErrorV1::Currentness)
            );
            assert_eq!(backend.write.load(Ordering::Relaxed), 0);
            assert!(backend.doorbells.is_empty());
            assert_eq!(
                &backend.logical_ring()[..4],
                &u32::from(AQL_INVALID_PACKET_HEADER_V1).to_le_bytes()
            );
            assert_eq!(backend.logical_ring(), before);
            assert_eq!(
                owner.submit(packet(), &mut backend),
                Err(NativeAqlSubmissionErrorV1::Poisoned)
            );
        }
    }

    #[test]
    fn postpublication_failure_poison_is_not_retryable() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fail_check = Some(3);
        assert_eq!(
            owner.submit(packet(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::Currentness)
        );
        assert_eq!(backend.write.load(Ordering::Relaxed), 1);
        assert!(backend.doorbells.is_empty());
        assert_eq!(
            owner.submit(packet(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::Poisoned)
        );
    }

    #[test]
    fn batches_of_two_four_and_sixteen_use_one_reservation_and_one_doorbell() {
        assert_successful_batch::<2>();
        assert_successful_batch::<4>();
        assert_successful_batch::<16>();
    }

    #[test]
    fn fixed_batch_of_8192_uses_one_fetch_add_and_one_final_doorbell() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(524_288).unwrap();
        let mut backend = FakeBackend::with_ring_bytes(524_288, 0, 0);
        assert_eq!(owner.submit_batch(batch::<8192>(), &mut backend), Ok(8191));
        assert_eq!(backend.write.load(Ordering::Relaxed), 8192);
        assert_eq!(backend.body_calls, 8192);
        assert_eq!(backend.header_calls, 8192);
        assert_eq!(backend.doorbells, [8191]);
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "fetch-add")
                .count(),
            1
        );
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "doorbell")
                .count(),
            1
        );
        assert!(backend.trace[4..8196].iter().all(|event| *event == "body"));
        assert!(
            backend.trace[8196..16388]
                .iter()
                .all(|event| *event == "header")
        );
    }

    #[test]
    fn batch_wrap_uses_exact_ordered_slots_and_last_packet_doorbell() {
        let mut owner = NativeAqlSubmissionOwnerV1::from_counters(4_096, 62, 62).unwrap();
        let mut backend = FakeBackend::new(62, 62);
        assert_eq!(owner.submit_batch(batch::<4>(), &mut backend), Ok(65));
        assert_eq!(backend.write.load(Ordering::Relaxed), 66);
        assert_eq!(backend.doorbells, [65]);

        for (batch_index, slot) in [62_u32, 63, 0, 1].into_iter().enumerate() {
            assert_eq!(backend.slot_word(slot, 0), 0x0001_1402);
            assert_eq!(backend.slot_word(slot, 28), batch_index as u32);
        }
        assert_eq!(backend.slot_word(2, 0), 1);
    }

    #[test]
    fn full_and_insufficient_batch_space_are_retryable_before_side_effects() {
        let mut full = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
        let mut full_backend = FakeBackend::new(64, 0);
        assert_eq!(
            full.submit_batch_classified(batch::<2>(), &mut full_backend),
            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
            ))
        );
        assert_eq!(full_backend.trace, ["check", "observe", "check"]);
        assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
        assert!(full_backend.doorbells.is_empty());
        full_backend.read.store(2, Ordering::Release);
        assert_eq!(
            full.submit_batch_classified(batch::<2>(), &mut full_backend),
            Ok(65)
        );

        let mut insufficient = NativeAqlSubmissionOwnerV1::from_counters(4_096, 63, 0).unwrap();
        let mut insufficient_backend = FakeBackend::new(63, 0);
        assert_eq!(
            insufficient.submit_batch_classified(batch::<2>(), &mut insufficient_backend),
            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::InsufficientSpace {
                    requested: 2,
                    available: 1,
                })
            ))
        );
        assert_eq!(insufficient_backend.trace, ["check", "observe", "check"]);
        assert_eq!(insufficient_backend.write.load(Ordering::Relaxed), 63);
        assert!(insufficient_backend.doorbells.is_empty());
        insufficient_backend.read.store(2, Ordering::Release);
        assert_eq!(
            insufficient.submit_batch_classified(batch::<2>(), &mut insufficient_backend),
            Ok(64)
        );
    }

    #[test]
    fn occupancy_shaped_backend_errors_are_terminal_at_every_checkpoint() {
        for failed_check in [1, 2, 3] {
            for insufficient in [false, true] {
                let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
                let mut backend = FakeBackend::new(0, 0);
                backend.fail_check = Some(failed_check);
                backend.fail_check_error =
                    Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                        AqlRingReservationError::InsufficientSpace {
                            requested: 1,
                            available: 0,
                        }
                    } else {
                        AqlRingReservationError::Full
                    }));

                let result = owner.submit_batch_classified(batch::<1>(), &mut backend);
                assert!(matches!(
                    result,
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                            | NativeAqlSubmissionErrorV1::Ring(
                                AqlRingReservationError::InsufficientSpace { .. }
                            )
                    ))
                ));
                let trace = backend.trace.clone();
                assert_eq!(
                    owner.submit_batch_classified(batch::<1>(), &mut backend),
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Poisoned
                    ))
                );
                assert_eq!(backend.trace, trace);
            }
        }
    }

    #[test]
    fn write_counter_divergence_after_reservation_is_terminal() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        backend.fetch_return_override = Some(9);
        assert_eq!(
            owner.submit_batch(batch::<4>(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::WriteCounterRace {
                expected: 0,
                observed: 9,
            })
        );
        assert_eq!(backend.write.load(Ordering::Relaxed), 4);
        assert_eq!(backend.trace, ["check", "observe", "check", "fetch-add"]);
        assert!(backend.doorbells.is_empty());
        let trace = backend.trace.clone();
        assert_eq!(
            owner.submit_batch(batch::<4>(), &mut backend),
            Err(NativeAqlSubmissionErrorV1::Poisoned)
        );
        assert_eq!(backend.trace, trace);
    }

    #[test]
    fn every_batch_side_effect_failure_is_terminal_without_cleanup_or_retry() {
        let mut cases = vec![(
            FailureAfterV1::FetchAdd,
            NativeAqlSubmissionErrorV1::Currentness,
        )];
        for index in 0..4 {
            cases.push((
                FailureAfterV1::Body(index),
                NativeAqlSubmissionErrorV1::PacketBody,
            ));
        }
        for index in 0..4 {
            cases.push((
                FailureAfterV1::Header(index),
                NativeAqlSubmissionErrorV1::PacketHeader,
            ));
        }
        cases.push((
            FailureAfterV1::Doorbell,
            NativeAqlSubmissionErrorV1::Doorbell,
        ));

        for (failure, expected) in cases {
            let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
            let mut backend = FakeBackend::new(0, 0);
            backend.fail_after = Some(failure);
            assert_eq!(
                owner.submit_batch_classified(batch::<4>(), &mut backend),
                Err(NativeAqlSubmissionFailureV1::Terminal(expected))
            );
            assert_eq!(backend.write.load(Ordering::Relaxed), 4, "{failure:?}");
            assert!(backend.doorbells.len() <= 1, "{failure:?}");
            if failure == FailureAfterV1::Doorbell {
                assert_eq!(backend.doorbells, [3]);
            } else {
                assert!(backend.doorbells.is_empty(), "{failure:?}");
            }

            let trace = backend.trace.clone();
            assert_eq!(
                owner.submit_batch_classified(batch::<4>(), &mut backend),
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::Poisoned
                )),
                "{failure:?}"
            );
            assert_eq!(backend.trace, trace, "{failure:?}");
            {
                let _terminal_owner = owner;
            }
            assert_eq!(backend.trace, trace, "Drop must not clean up {failure:?}");
        }
    }

    #[test]
    fn occupancy_shaped_batch_side_effect_errors_are_terminal_and_sticky() {
        for failure in [
            FailureAfterV1::FetchAdd,
            FailureAfterV1::Body(0),
            FailureAfterV1::Header(0),
            FailureAfterV1::Doorbell,
        ] {
            for insufficient in [false, true] {
                let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
                let mut backend = FakeBackend::new(0, 0);
                backend.fail_after = Some(failure);
                backend.fail_error = Some(NativeAqlSubmissionErrorV1::Ring(if insufficient {
                    AqlRingReservationError::InsufficientSpace {
                        requested: 1,
                        available: 0,
                    }
                } else {
                    AqlRingReservationError::Full
                }));

                let result = owner.submit_batch_classified(batch::<1>(), &mut backend);
                assert!(matches!(
                    result,
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Ring(AqlRingReservationError::Full)
                            | NativeAqlSubmissionErrorV1::Ring(
                                AqlRingReservationError::InsufficientSpace { .. }
                            )
                    ))
                ));
                let trace = backend.trace.clone();
                assert_eq!(
                    owner.submit_batch_classified(batch::<1>(), &mut backend),
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::Poisoned
                    ))
                );
                assert_eq!(backend.trace, trace);
            }
        }
    }

    #[test]
    fn cwsr_headers_match_pinned_rocr_layout() {
        let mut bytes = vec![0xff; GFX942_CWSR_TOTAL_BYTES_V1];
        let payload = KfdQueueExceptionPayloadAddressV1::new(0x1000).unwrap();
        let event = KfdSignalEventIdV1::new(7).unwrap();
        initialize_gfx942_cwsr_headers(&mut bytes, payload, event).unwrap();
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            let offset = xcc * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1;
            let header = &bytes[offset..offset + CWSR_HEADER_BYTES];
            assert_eq!(&header[..16], &[0; 16]);
            assert_eq!(
                u32::from_le_bytes(header[16..20].try_into().unwrap()),
                ((GFX942_CWSR_XCC_COUNT_V1 - xcc) * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1) as u32
            );
            assert_eq!(
                u32::from_le_bytes(header[20..24].try_into().unwrap()),
                GFX942_CWSR_DEBUG_BYTES_TOTAL_V1
            );
            assert_eq!(
                u64::from_le_bytes(header[24..32].try_into().unwrap()),
                0x1000
            );
            assert_eq!(u32::from_le_bytes(header[32..36].try_into().unwrap()), 7);
            assert_eq!(&header[36..40], &[0; 4]);
        }
        assert!(
            bytes[CWSR_HEADER_BYTES..GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1]
                .iter()
                .all(|byte| *byte == 0xff)
        );
        assert!(initialize_gfx942_cwsr_headers(&mut [0; 4096], payload, event).is_err());
        assert!(gfx942_cwsr_header_bytes(8, payload, event).is_err());
    }

    fn assert_successful_batch<const N: usize>() {
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        assert_eq!(
            owner.submit_batch(batch::<N>(), &mut backend),
            Ok(N as u64 - 1)
        );
        assert_eq!(backend.write.load(Ordering::Relaxed), N as u64);
        assert_eq!(backend.doorbells, [N as u64 - 1]);
        assert_eq!(
            backend.trace[..4],
            ["check", "observe", "check", "fetch-add"]
        );
        assert!(backend.trace[4..4 + N].iter().all(|event| *event == "body"));
        assert!(
            backend.trace[4 + N..4 + 2 * N]
                .iter()
                .all(|event| *event == "header")
        );
        assert_eq!(backend.trace[4 + 2 * N..], ["check", "doorbell"]);
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "fetch-add")
                .count(),
            1
        );
        assert_eq!(
            backend
                .trace
                .iter()
                .filter(|event| **event == "doorbell")
                .count(),
            1
        );
        for index in 0..N {
            assert_eq!(backend.slot_word(index as u32, 0), 0x0001_1402);
            assert_eq!(backend.slot_word(index as u32, 28), index as u32);
        }
    }
}
