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
    AqlPacketPublicationTargetV1, AqlPreparedKernelDispatchV1, AqlRingCapacityV1,
    AqlRingReservationError, AqlSingleProducerRingModelV1,
};
use fe2o3_kfd_uapi::{
    KfdContextSaveAreaHeaderV1, KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1,
};

#[cfg(test)]
use fe2o3_aql::AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1;

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
/// effect poison it. Only an ordinary full-ring observation is retryable.
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

    pub(super) fn submit<B: NativeAqlSubmissionBackendV1>(
        &mut self,
        packet: AqlPreparedKernelDispatchV1,
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
        let reservation = match self.ring.reserve_one(observed_read) {
            Ok(reservation) => reservation,
            Err(AqlRingReservationError::Full) => {
                return Err(NativeAqlSubmissionErrorV1::Ring(
                    AqlRingReservationError::Full,
                ));
            }
            Err(error) => {
                self.phase = SubmissionPhaseV1::Poisoned;
                return Err(NativeAqlSubmissionErrorV1::Ring(error));
            }
        };

        // From here on, even a reported error may follow a native side effect.
        self.phase = SubmissionPhaseV1::Poisoned;
        let old_write = backend.fetch_add_write_acq_rel(1)?;
        if old_write != reservation.packet_id() {
            return Err(NativeAqlSubmissionErrorV1::WriteCounterRace {
                expected: reservation.packet_id(),
                observed: old_write,
            });
        }

        let mut target = NativePacketTargetV1 {
            backend,
            slot: reservation.slot_index(),
        };
        packet.publish_with(&mut target)?;

        // The packet is already published here. Failure prevents MMIO but is
        // not recoverable or retryable by this owner.
        backend.check_currentness()?;
        backend.ring_doorbell_release(reservation.packet_id())?;
        self.phase = SubmissionPhaseV1::Ready;
        Ok(reservation.packet_id())
    }
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

struct NativePacketTargetV1<'a, B> {
    backend: &'a mut B,
    slot: u32,
}

impl<B: NativeAqlSubmissionBackendV1> AqlPacketPublicationTargetV1 for NativePacketTargetV1<'_, B> {
    type Error = NativeAqlSubmissionErrorV1;

    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<(), Self::Error> {
        self.backend.write_unpublished(self.slot, packet)
    }

    fn publish_release_header(&mut self, header: u16) -> Result<(), Self::Error> {
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
    use fe2o3_aql::{AqlDispatchGeometryV1, AqlKernelDispatchPacketV1, ObservedGpuAddressV1};

    #[repr(align(64))]
    struct AlignedRing([u8; 4_224]);

    struct FakeBackend {
        ring: AlignedRing,
        write: AtomicU64,
        read: AtomicU64,
        checks: usize,
        fail_check: Option<usize>,
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
                trace: Vec::new(),
                doorbells: Vec::new(),
            }
        }

        fn logical_ring(&mut self) -> &mut [u8] {
            &mut self.ring.0[64..4_160]
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
            Ok(self.write.fetch_add(increment, Ordering::AcqRel))
        }

        fn write_unpublished(
            &mut self,
            slot: u32,
            packet: &AqlKernelDispatchPacketV1,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("body");
            write_unpublished_slot(self.logical_ring(), slot, packet)
        }

        fn publish_release_header(
            &mut self,
            slot: u32,
            header: u16,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("header");
            publish_slot_header_release(self.logical_ring(), slot, header)
        }

        fn ring_doorbell_release(
            &mut self,
            packet_id: u64,
        ) -> Result<(), NativeAqlSubmissionErrorV1> {
            self.trace.push("doorbell");
            release_fence_before_mmio();
            self.doorbells.push(packet_id);
            Ok(())
        }
    }

    fn packet() -> AqlPreparedKernelDispatchV1 {
        AqlKernelDispatchPacketV1::new_unpublished(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            0,
            ObservedGpuAddressV1::new(0x10_000).unwrap(),
            ObservedGpuAddressV1::new(0x20_000).unwrap(),
            16,
            ObservedGpuAddressV1::new(0x30_000).unwrap(),
        )
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
        let mut owner = NativeAqlSubmissionOwnerV1::new(4_096).unwrap();
        let mut backend = FakeBackend::new(0, 0);
        let before = backend.logical_ring().to_vec();
        backend.fail_check = Some(2);
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
}
