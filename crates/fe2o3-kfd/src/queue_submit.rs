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

    pub(super) fn is_pristine_v1(&self) -> bool {
        self.phase == SubmissionPhaseV1::Ready
            && self.ring.write() == 0
            && self.ring.last_read() == 0
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

pub(crate) fn initialize_invalid_ring(bytes: &mut [u8]) -> Result<(), NativeAqlSubmissionErrorV1> {
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
#[path = "queue_submit/tests.rs"]
mod tests;
