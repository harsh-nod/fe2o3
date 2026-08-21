//! Private, single-producer AQL reservation and publication boundary.
//!
//! This module is the only bridge from inert `fe2o3-aql` packet values to a
//! retained native queue. It deliberately provides no public submission,
//! address, pointer, or MMIO API. GPU participation in the shared counters and
//! packet bytes is an external, source-pinned contract rather than a Rust
//! atomic-memory-model proof.

use core::sync::atomic::{AtomicU32, AtomicU64};

#[cfg(test)]
use core::sync::atomic::{Ordering, fence};

use fe2o3_aql::{
    AQL_INVALID_PACKET_HEADER_V1, AQL_KERNEL_DISPATCH_PACKET_BYTES_V1, AqlKernelDispatchPacketV1,
    AqlPacketBatchPublicationTargetV1, AqlPreparedKernelDispatchBatchV1, AqlRingBatchReservationV1,
    AqlRingCapacityV1, AqlRingReservationError, AqlSingleProducerRingModelV1,
};
use fe2o3_kfd_uapi::{
    KfdContextSaveAreaHeaderV1, KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1,
};

#[cfg(test)]
use fe2o3_aql::{AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, AqlPreparedKernelDispatchV1};

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
    pub(super) fn new(ring_bytes: u32) -> Result<Self, NativeAqlSubmissionErrorV1> {
        Self::from_counters(ring_bytes, 0, 0)
    }

    pub(super) fn poison(&mut self) {
        self.phase = SubmissionPhaseV1::Poisoned;
    }

    fn from_counters(
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
        self.submit_batch(AqlPreparedKernelDispatchBatchV1::one(packet), backend)
    }

    pub(super) fn submit_batch<const N: usize, B: NativeAqlSubmissionBackendV1>(
        &mut self,
        batch: AqlPreparedKernelDispatchBatchV1<N>,
        backend: &mut B,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        if self.phase != SubmissionPhaseV1::Ready {
            return Err(NativeAqlSubmissionErrorV1::Poisoned);
        }

        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(error);
        }
        let (observed_write, observed_read) = match backend.observe_counters_acquire() {
            Ok(observation) => observation,
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(error);
            }
        };
        let expected_write = self.ring.write();
        if observed_write != expected_write {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(NativeAqlSubmissionErrorV1::WriteCounterReplay {
                expected: expected_write,
                observed: observed_write,
            });
        }

        // This is the final check after bounded read/capacity preparation and
        // before the first shared-memory side effect.
        if let Err(error) = backend.check_currentness() {
            self.phase = SubmissionPhaseV1::Poisoned;
            return Err(error);
        }
        let reservation = match self.ring.reserve_batch(observed_read, batch.packet_count()) {
            Ok(reservation) => reservation,
            Err(error) if retryable_occupancy(&error) => {
                return Err(NativeAqlSubmissionErrorV1::Ring(error));
            }
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeAqlSubmissionErrorV1::Ring(error));
            }
        };

        // From here on, even a reported error may follow a native side effect.
        self.phase = SubmissionPhaseV1::Poisoned;
        let old_write = backend.fetch_add_write_acq_rel(u64::from(batch.packet_count()))?;
        if old_write != reservation.first_packet_id() {
            return Err(NativeAqlSubmissionErrorV1::WriteCounterRace {
                expected: reservation.first_packet_id(),
                observed: old_write,
            });
        }

        let mut target = NativePacketBatchTargetV1 {
            backend,
            reservation: &reservation,
        };
        batch.publish_with(&mut target)?;

        // Every packet is already published here. Failure prevents MMIO but
        // is not recoverable or retryable by this owner.
        backend.check_currentness()?;
        backend.ring_doorbell_release(reservation.last_packet_id())?;
        self.phase = SubmissionPhaseV1::Ready;
        Ok(reservation.last_packet_id())
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
        packet: &AqlKernelDispatchPacketV1,
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
        self.backend.write_unpublished(entry.slot_index(), packet)
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

pub(super) fn initialize_control_atomics(
    bytes: &mut [u8],
) -> Result<(), NativeAqlSubmissionErrorV1> {
    if bytes.len() != 4096 {
        return Err(NativeAqlSubmissionErrorV1::CounterObservation);
    }
    bytes.fill(0);
    for offset in [0, 8] {
        let pointer = bytes[offset..].as_mut_ptr().cast::<AtomicU64>();
        if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU64>()) {
            return Err(NativeAqlSubmissionErrorV1::CounterObservation);
        }
        // SAFETY: each exact aligned 8-byte range is exclusively borrowed and
        // initialized as AtomicU64 before the mapping becomes GPU accessible.
        unsafe { pointer.write(AtomicU64::new(0)) };
    }
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
    packet: &AqlKernelDispatchPacketV1,
) -> Result<(), NativeAqlSubmissionErrorV1> {
    let slot = packet_slot(bytes, slot_index)?;
    let encoded = packet.encode_unpublished_le();
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
    if header != AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1 {
        return Err(NativeAqlSubmissionErrorV1::PacketHeader);
    }
    let slot = packet_slot(bytes, slot_index)?;
    let pointer = slot.as_mut_ptr().cast::<AtomicU32>();
    if !(pointer as usize).is_multiple_of(core::mem::align_of::<AtomicU32>()) {
        return Err(NativeAqlSubmissionErrorV1::PacketHeader);
    }
    // SAFETY: fake storage initialized this exact header AtomicU32 before use.
    let atomic = unsafe { &*pointer };
    let unpublished = u32::from_le(atomic.load(Ordering::Relaxed));
    if unpublished & 0xffff != u32::from(AQL_INVALID_PACKET_HEADER_V1) {
        return Err(NativeAqlSubmissionErrorV1::PacketHeader);
    }
    let setup = unpublished >> 16;
    if !(1..=3).contains(&setup) {
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
        AqlDispatchGeometryV1, AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchBatchV1,
        ObservedGpuAddressV1,
    };

    #[repr(align(64))]
    struct AlignedRing([u8; 4_224]);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FailureAfterV1 {
        FetchAdd,
        Body(usize),
        Header(usize),
        Doorbell,
    }

    struct FakeBackend {
        ring: AlignedRing,
        write: AtomicU64,
        read: AtomicU64,
        checks: usize,
        fail_check: Option<usize>,
        fail_after: Option<FailureAfterV1>,
        fetch_return_override: Option<u64>,
        body_calls: usize,
        header_calls: usize,
        trace: Vec<&'static str>,
        doorbells: Vec<u64>,
    }

    impl FakeBackend {
        fn new(write: u64, read: u64) -> Self {
            let mut ring = AlignedRing([0xa5; 4_224]);
            initialize_invalid_ring(&mut ring.0[64..4_160]).unwrap();
            Self {
                ring,
                write: AtomicU64::new(write),
                read: AtomicU64::new(read),
                checks: 0,
                fail_check: None,
                fail_after: None,
                fetch_return_override: None,
                body_calls: 0,
                header_calls: 0,
                trace: Vec::new(),
                doorbells: Vec::new(),
            }
        }

        fn logical_ring(&mut self) -> &mut [u8] {
            &mut self.ring.0[64..4_160]
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
            if self.fail_check == Some(self.checks) {
                Err(NativeAqlSubmissionErrorV1::Currentness)
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
            let observed = self.write.fetch_add(increment, Ordering::AcqRel);
            if self.fail_after == Some(FailureAfterV1::FetchAdd) {
                return Err(NativeAqlSubmissionErrorV1::Currentness);
            }
            Ok(self.fetch_return_override.unwrap_or(observed))
        }

        fn write_unpublished(
            &mut self,
            slot: u32,
            packet: &AqlKernelDispatchPacketV1,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("body");
            let call = self.body_calls;
            self.body_calls += 1;
            write_unpublished_slot(self.logical_ring(), slot, packet)?;
            if self.fail_after == Some(FailureAfterV1::Body(call)) {
                return Err(NativeAqlSubmissionErrorV1::PacketBody);
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
            publish_slot_header_release(self.logical_ring(), slot, header)?;
            if self.fail_after == Some(FailureAfterV1::Header(call)) {
                return Err(NativeAqlSubmissionErrorV1::PacketHeader);
            }
            Ok(())
        }

        fn ring_doorbell_release(
            &mut self,
            packet_id: u64,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("doorbell");
            release_fence_before_mmio();
            self.doorbells.push(packet_id);
            if self.fail_after == Some(FailureAfterV1::Doorbell) {
                return Err(NativeAqlSubmissionErrorV1::Doorbell);
            }
            Ok(())
        }
    }

    fn packet() -> AqlPreparedKernelDispatchV1 {
        indexed_packet(0)
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

    fn batch<const N: usize>() -> AqlPreparedKernelDispatchBatchV1<N> {
        AqlPreparedKernelDispatchBatchV1::try_from_packets(core::array::from_fn(|index| {
            indexed_packet(index as u32)
        }))
        .unwrap()
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
            full.submit_batch(batch::<2>(), &mut full_backend),
            Err(NativeAqlSubmissionErrorV1::Ring(
                AqlRingReservationError::Full
            ))
        );
        assert_eq!(full_backend.trace, ["check", "observe", "check"]);
        assert_eq!(full_backend.write.load(Ordering::Relaxed), 64);
        assert!(full_backend.doorbells.is_empty());
        full_backend.read.store(2, Ordering::Release);
        assert_eq!(full.submit_batch(batch::<2>(), &mut full_backend), Ok(65));

        let mut insufficient = NativeAqlSubmissionOwnerV1::from_counters(4_096, 63, 0).unwrap();
        let mut insufficient_backend = FakeBackend::new(63, 0);
        assert_eq!(
            insufficient.submit_batch(batch::<2>(), &mut insufficient_backend),
            Err(NativeAqlSubmissionErrorV1::Ring(
                AqlRingReservationError::InsufficientSpace {
                    requested: 2,
                    available: 1,
                }
            ))
        );
        assert_eq!(insufficient_backend.trace, ["check", "observe", "check"]);
        assert_eq!(insufficient_backend.write.load(Ordering::Relaxed), 63);
        assert!(insufficient_backend.doorbells.is_empty());
        insufficient_backend.read.store(2, Ordering::Release);
        assert_eq!(
            insufficient.submit_batch(batch::<2>(), &mut insufficient_backend),
            Ok(64)
        );
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
                owner.submit_batch(batch::<4>(), &mut backend),
                Err(expected)
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
                owner.submit_batch(batch::<4>(), &mut backend),
                Err(NativeAqlSubmissionErrorV1::Poisoned),
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
