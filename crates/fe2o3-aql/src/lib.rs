#![no_std]
#![forbid(unsafe_code)]

//! Inert, source-pinned AMDHSA AQL packet and busy-wait signal contracts.
//!
//! This crate validates values and lays out bytes. It does not own GPU virtual
//! addresses, publish packets, map a doorbell, create a queue, or establish
//! that firmware consumed a packet.

extern crate alloc;

use alloc::boxed::Box;
use core::{
    mem::{align_of, offset_of, size_of},
    sync::atomic::{AtomicI64, Ordering},
};

/// Stable name of the reviewed packet/signal contract.
pub const AQL_DISPATCH_ABI_SCHEMA_ID_V1: &str =
    "rocr-7.2.4-amdhsa-gfx942-aql-dispatch-busy-signal-v1";

/// Canonical source and wire-format manifest for the reviewed contract.
pub const AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1: &str = r#"schema=rocr-7.2.4-amdhsa-gfx942-aql-dispatch-busy-signal-v1
platform=linux-x86_64,little-endian,pointer-width:64
rocr_commit=97f5574fe2fdc7bef44fb01545347912ee9f1779,tag:rocm-7.2.4
source.hsa.h=51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5
source.amd_hsa_signal.h=ba429b422e91fe370e4241ce8c8d934738b6e3c59b10c1eefd2370d76afe5020
source.signal.h=615199b8f8321de9f766d3be4d17caaec58e5057c6113767f6181c455fb7667a
source.signal.cpp=2faa5a0a554a4c15d9a83991f02717afb0436eceedaf51040b74defbb61c5c73
source.default_signal.h=440c98decfaa80db761ca5ec0add6f8956e4bbbaeff1ff9b185096c9852629ce
source.default_signal.cpp=8bc963899a366b4de8745c88939b9f9ab2d779cecffe42f4171a8e6a796a3cd2
source.amd_aql_queue.cpp=291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14
source.amd_blit_kernel.cpp=e6ce094b32e4f300bd574db1a056fdd91740dbcae722da0824eb119a7e6490a2
source.queue.h=aa1cd1acea3405e8c18076b406dd91b5433438792f7cbe8ac5bc3d46df25a9ca
source.amd_hsa_kernel_code.h=2f48b1fff5432fb96aa460d3c5ac0bccb2e8996adfa5ecdb508722f3911ff9d0
legacy_release_u32_reference.fe2o3_hsa_runtime.native.runtime.c=99dc188ad8b12561b66ac4a156fdbcfec068c1797fad75afa43a45d3a830554f,not-invalid-body-evidence
packet=size:64,align:8,header:0,setup:2,workgroup:4,grid:12,private:24,group:28,kernel-object:32,kernarg:40,reserved2:48,completion-signal:56
publication=initial-type:invalid-1,initial-setup-dimensions:1..3,prepared-retains-exact-final-header,ordering:independent-barrier0-header0x1402|wait-for-prior-barrier1-header0x1502,backend-preserves-copied-setup,single-release-u32-at-offset-0,type:2,acquire:system-2,release:system-2
ring-reservation=mutable-single-producer-model,packet-bytes:64,ring-bytes:4096..2147483648-power-of-two,capacity:64..33554432,monotonic-u64-no-wrap,nondecreasing-read,read<=write,distance<=capacity,slot:packet-id&(capacity-1)
signal=size:64,align:64,kind-offset:0,value-offset:8,kind:user-1,pending:1,complete:0,event-fields:zero,byte-encoder:exact-64,classifier:1-pending|0-completed|other-unexpected-preserved,busy-poll-only
address-observations=nonzero,kernel-object-align:64,completion-signal-align:64,kernarg-align:caller-supplied-power-of-two-1..4096
authority=inert-wire-values-only,no-address-provenance,no-allocation,no-typed-object-placement,no-queue,no-publication,no-doorbell,no-execution
"#;

/// SHA-256 of [`AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1`].
pub const AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1: &str =
    "82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0";

/// Typed SHA-256 bytes of [`AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1`].
pub const AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1: [u8; 32] = [
    0x82, 0xfb, 0xd7, 0xcf, 0x0b, 0x6c, 0x86, 0x47, 0xdc, 0xe3, 0xf9, 0xb1, 0x1e, 0x4f, 0x13, 0xa2,
    0xda, 0xdf, 0xe3, 0x42, 0x35, 0x09, 0xf7, 0x69, 0xa4, 0xbc, 0x6c, 0xc8, 0x7b, 0xb7, 0xac, 0xd0,
];

pub const AQL_KERNEL_DISPATCH_PACKET_BYTES_V1: usize = 64;
pub const AMD_SIGNAL_BYTES_V1: usize = 64;
pub const AMD_SIGNAL_ALIGNMENT_V1: usize = 64;
pub const AMD_SIGNAL_KIND_USER_V1: i64 = 1;
pub const AMD_SIGNAL_VALUE_PENDING_V1: i64 = 1;
pub const AMD_SIGNAL_VALUE_COMPLETE_V1: i64 = 0;
pub const AQL_INVALID_PACKET_HEADER_V1: u16 = 1;
pub const AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1: u16 = 0x1402;
pub const AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1: u16 = 0x1502;
pub const AQL_MIN_RING_BYTES_V1: u32 = 4096;
pub const AQL_MAX_RING_BYTES_V1: u32 = 1 << 31;
/// Maximum packets admitted by one V1 arithmetic batch reservation.
///
/// At 64 bytes per packet this bounds one reservation to 16 KiB of logical
/// ring slots. It does not size a native queue or claim that one batch is a
/// complete command schedule.
pub const AQL_MAX_BATCH_PACKETS_V1: u32 = 256;

/// Maximum packets admitted by one V2 fixed-capacity publication.
///
/// At 64 bytes per packet this requires at least a 512 KiB ring for the
/// maximum batch. The bound is a host resource policy, not a hardware queue
/// limit.
pub const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 8192;

/// Execution-order policy encoded in one kernel-dispatch packet header.
///
/// System-scoped acquire and release fences govern memory visibility. They do
/// not by themselves make a dispatch wait for preceding queue packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum AqlDispatchOrderingV1 {
    /// The dispatch may become eligible independently of prior queue packets.
    Independent = AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1,
    /// The dispatch waits until all preceding queue packets have completed.
    WaitForPrior = AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1,
}

impl AqlDispatchOrderingV1 {
    /// Returns the exact system-scoped kernel-dispatch header.
    pub const fn header(self) -> u16 {
        self as u16
    }

    /// Admits exactly one reviewed system-scoped dispatch header.
    pub const fn from_header(header: u16) -> Option<Self> {
        match header {
            AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1 => Some(Self::Independent),
            AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1 => Some(Self::WaitForPrior),
            _ => None,
        }
    }
}

/// Stable name of the inert V1 batch-reservation model.
pub const AQL_BATCH_RESERVATION_MODEL_SCHEMA_ID_V1: &str =
    "fe2o3-aql-single-producer-batch-reservation-v1";

/// Canonical arithmetic and authority boundary of the V1 batch model.
pub const AQL_BATCH_RESERVATION_MODEL_MANIFEST_V1: &str = r#"schema=fe2o3-aql-single-producer-batch-reservation-v1
packet-count=1..256
state=single-producer-write,last-observed-read,power-of-two-ring-capacity
admission=nondecreasing-read,read<=write,distance<=capacity,count<=capacity,count<=available,checked-u64-next-write
slots=packet-id&(capacity-1),ordered,distinct-within-admitted-batch,wrap-aware
transition=all-checks-before-write-or-last-read-mutation
authority=inert-arithmetic-only,no-native-reservation,no-counter-access,no-packet-write,no-publication,no-doorbell,no-completion
"#;

/// SHA-256 of [`AQL_BATCH_RESERVATION_MODEL_MANIFEST_V1`].
pub const AQL_BATCH_RESERVATION_MODEL_MANIFEST_SHA256_V1: &str =
    "0734191a1975f1bfc66bbcdbfd47f907656963b35c97a6d3f4cd2e04d2f59a83";

/// Additive fixed-capacity reservation/publication profile retaining V1.
pub const AQL_FIXED_BATCH_MODEL_MANIFEST_V2: &str = r#"schema=fe2o3-aql-single-producer-fixed-batch-v2
v1_schema_sha256=0734191a1975f1bfc66bbcdbfd47f907656963b35c97a6d3f4cd2e04d2f59a83
packet-count=1..8192
packet-bytes=64
minimum-ring-for-maximum-batch=524288
state=single-producer-write,last-observed-read,power-of-two-ring-capacity
admission=nondecreasing-read,read<=write,distance<=capacity,count<=capacity,count<=available,checked-u64-next-write
slots=packet-id&(capacity-1),ordered,distinct-within-admitted-batch,wrap-aware
transition=all-checks-before-write-or-last-read-mutation
publication=all-invalid-bodies-before-any-per-packet-retained-exact-release-header,independent:0x1402,wait-for-prior:0x1502,one-final-doorbell-required-by-later-native-owner
authority=inert-arithmetic-and-packet-values-only,no-native-reservation,no-counter-access,no-packet-write,no-publication,no-doorbell,no-completion
"#;

/// SHA-256 of [`AQL_FIXED_BATCH_MODEL_MANIFEST_V2`].
pub const AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2: &str =
    "a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlAddressObservationError {
    Zero,
    InvalidRequiredAlignment,
    Misaligned,
}

/// A nonzero numeric GPU address observation without ownership or provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct ObservedGpuAddressV1(u64);

impl ObservedGpuAddressV1 {
    pub const fn new(raw: u64) -> Result<Self, AqlAddressObservationError> {
        if raw == 0 {
            return Err(AqlAddressObservationError::Zero);
        }
        Ok(Self(raw))
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn require_alignment(
        self,
        alignment: u64,
    ) -> Result<Self, AqlAddressObservationError> {
        if alignment == 0 || alignment > 4096 || !alignment.is_power_of_two() {
            return Err(AqlAddressObservationError::InvalidRequiredAlignment);
        }
        if self.0 & (alignment - 1) != 0 {
            return Err(AqlAddressObservationError::Misaligned);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlRingCapacityError {
    BelowMinimum,
    AboveMaximum,
    NotPowerOfTwo,
}

/// Packet capacity derived from an exact admitted AQL ring byte length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlRingCapacityV1 {
    packets: u32,
}

impl AqlRingCapacityV1 {
    pub const fn from_ring_bytes(ring_bytes: u32) -> Result<Self, AqlRingCapacityError> {
        if ring_bytes < AQL_MIN_RING_BYTES_V1 {
            return Err(AqlRingCapacityError::BelowMinimum);
        }
        if ring_bytes > AQL_MAX_RING_BYTES_V1 {
            return Err(AqlRingCapacityError::AboveMaximum);
        }
        if !ring_bytes.is_power_of_two() {
            return Err(AqlRingCapacityError::NotPowerOfTwo);
        }
        Ok(Self {
            packets: ring_bytes / AQL_KERNEL_DISPATCH_PACKET_BYTES_V1 as u32,
        })
    }

    pub const fn packets(self) -> u32 {
        self.packets
    }

    pub const fn mask(self) -> u64 {
        self.packets as u64 - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlRingReservationError {
    ZeroPacketCount,
    PacketCountExceedsReviewedMaximum { requested: u32, maximum: u32 },
    PacketCountExceedsRingCapacity { requested: u32, capacity: u32 },
    ReadAfterWrite,
    ReadRegressed,
    CounterDistanceExceedsCapacity,
    Full,
    InsufficientSpace { requested: u32, available: u32 },
    WriteCounterExhausted,
}

/// Mutable single-producer reservation model for one logical queue.
///
/// The object prevents duplicate reservations within this model instance. It
/// is not native queue authority; a later sealed queue owner must ensure that
/// exactly one instance is bound to the retained native queue.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlSingleProducerRingModelV1 {
    capacity: AqlRingCapacityV1,
    write: u64,
    last_read: u64,
}

impl AqlSingleProducerRingModelV1 {
    pub const fn new(
        capacity: AqlRingCapacityV1,
        write: u64,
        read: u64,
    ) -> Result<Self, AqlRingReservationError> {
        let Some(distance) = write.checked_sub(read) else {
            return Err(AqlRingReservationError::ReadAfterWrite);
        };
        if distance > capacity.packets as u64 {
            return Err(AqlRingReservationError::CounterDistanceExceedsCapacity);
        }
        Ok(Self {
            capacity,
            write,
            last_read: read,
        })
    }

    pub const fn write(&self) -> u64 {
        self.write
    }

    pub const fn last_read(&self) -> u64 {
        self.last_read
    }

    pub const fn reserve_one(
        &mut self,
        observed_read: u64,
    ) -> Result<AqlRingReservationV1, AqlRingReservationError> {
        let batch = match self.reserve_batch(observed_read, 1) {
            Ok(batch) => batch,
            Err(error) => return Err(error),
        };
        Ok(AqlRingReservationV1 {
            packet_id: batch.first_packet_id,
            slot_index: (batch.first_packet_id & batch.slot_mask) as u32,
            observed_read: batch.observed_read,
            next_write: batch.next_write,
        })
    }

    /// Reserves one bounded ordered batch in this arithmetic model.
    ///
    /// All validation and checked arithmetic complete before either retained
    /// counter changes. Success advances both counters once as one model
    /// transition. The result remains inert and cannot reserve native storage
    /// or publish packets.
    pub const fn reserve_batch(
        &mut self,
        observed_read: u64,
        packet_count: u32,
    ) -> Result<AqlRingBatchReservationV1, AqlRingReservationError> {
        self.reserve_batch_with_maximum(observed_read, packet_count, AQL_MAX_BATCH_PACKETS_V1)
    }

    /// Reserves one additive V2 fixed batch without changing the V1 bound.
    pub const fn reserve_fixed_batch_v2(
        &mut self,
        observed_read: u64,
        packet_count: u32,
    ) -> Result<AqlRingBatchReservationV1, AqlRingReservationError> {
        self.reserve_batch_with_maximum(observed_read, packet_count, AQL_MAX_FIXED_BATCH_PACKETS_V2)
    }

    const fn reserve_batch_with_maximum(
        &mut self,
        observed_read: u64,
        packet_count: u32,
        maximum: u32,
    ) -> Result<AqlRingBatchReservationV1, AqlRingReservationError> {
        if packet_count == 0 {
            return Err(AqlRingReservationError::ZeroPacketCount);
        }
        if packet_count > maximum {
            return Err(AqlRingReservationError::PacketCountExceedsReviewedMaximum {
                requested: packet_count,
                maximum,
            });
        }
        if packet_count > self.capacity.packets {
            return Err(AqlRingReservationError::PacketCountExceedsRingCapacity {
                requested: packet_count,
                capacity: self.capacity.packets,
            });
        }
        if observed_read < self.last_read {
            return Err(AqlRingReservationError::ReadRegressed);
        }
        let Some(distance) = self.write.checked_sub(observed_read) else {
            return Err(AqlRingReservationError::ReadAfterWrite);
        };
        let capacity_u64 = self.capacity.packets as u64;
        if distance > capacity_u64 {
            return Err(AqlRingReservationError::CounterDistanceExceedsCapacity);
        }
        if distance == capacity_u64 {
            return Err(AqlRingReservationError::Full);
        }
        let available = capacity_u64 - distance;
        if packet_count as u64 > available {
            return Err(AqlRingReservationError::InsufficientSpace {
                requested: packet_count,
                available: available as u32,
            });
        }
        let Some(next_write) = self.write.checked_add(packet_count as u64) else {
            return Err(AqlRingReservationError::WriteCounterExhausted);
        };
        let reservation = AqlRingBatchReservationV1 {
            first_packet_id: self.write,
            packet_count,
            slot_mask: self.capacity.mask(),
            observed_read,
            next_write,
        };
        self.write = next_write;
        self.last_read = observed_read;
        Ok(reservation)
    }
}

/// One ordered, bounded arithmetic reservation over distinct logical slots.
///
/// This value owns no native counter or memory. Its entries only describe the
/// packet IDs and wrapped slot indices selected by the successful model
/// transition.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlRingBatchReservationV1 {
    first_packet_id: u64,
    packet_count: u32,
    slot_mask: u64,
    observed_read: u64,
    next_write: u64,
}

impl AqlRingBatchReservationV1 {
    pub const fn first_packet_id(&self) -> u64 {
        self.first_packet_id
    }

    pub const fn packet_count(&self) -> u32 {
        self.packet_count
    }

    pub const fn observed_read(&self) -> u64 {
        self.observed_read
    }

    pub const fn next_write(&self) -> u64 {
        self.next_write
    }

    pub const fn last_packet_id(&self) -> u64 {
        self.next_write - 1
    }

    pub const fn entry(&self, batch_index: u32) -> Option<AqlRingBatchReservationEntryV1> {
        if batch_index >= self.packet_count {
            return None;
        }
        let Some(packet_id) = self.first_packet_id.checked_add(batch_index as u64) else {
            return None;
        };
        Some(AqlRingBatchReservationEntryV1 {
            packet_id,
            slot_index: (packet_id & self.slot_mask) as u32,
        })
    }

    pub const fn entries(&self) -> AqlRingBatchReservationEntriesV1 {
        AqlRingBatchReservationEntriesV1 {
            first_packet_id: self.first_packet_id,
            packet_count: self.packet_count,
            slot_mask: self.slot_mask,
            next_index: 0,
        }
    }
}

/// One inert packet-ID/slot pair within a batch reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlRingBatchReservationEntryV1 {
    packet_id: u64,
    slot_index: u32,
}

impl AqlRingBatchReservationEntryV1 {
    pub const fn packet_id(self) -> u64 {
        self.packet_id
    }

    pub const fn slot_index(self) -> u32 {
        self.slot_index
    }
}

/// Exact-size iterator over one inert batch reservation.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlRingBatchReservationEntriesV1 {
    first_packet_id: u64,
    packet_count: u32,
    slot_mask: u64,
    next_index: u32,
}

impl Iterator for AqlRingBatchReservationEntriesV1 {
    type Item = AqlRingBatchReservationEntryV1;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.packet_count {
            return None;
        }
        let packet_id = self
            .first_packet_id
            .checked_add(u64::from(self.next_index))?;
        self.next_index += 1;
        Some(AqlRingBatchReservationEntryV1 {
            packet_id,
            slot_index: (packet_id & self.slot_mask) as u32,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.packet_count - self.next_index) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for AqlRingBatchReservationEntriesV1 {}
impl core::iter::FusedIterator for AqlRingBatchReservationEntriesV1 {}

/// One slot selected by one mutable arithmetic-model transition.
///
/// This remains an inert arithmetic result. It is not a lease on native ring
/// memory and cannot publish or advance a write pointer.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlRingReservationV1 {
    packet_id: u64,
    slot_index: u32,
    observed_read: u64,
    next_write: u64,
}

impl AqlRingReservationV1 {
    pub const fn packet_id(&self) -> u64 {
        self.packet_id
    }

    pub const fn slot_index(&self) -> u32 {
        self.slot_index
    }

    pub const fn observed_read(&self) -> u64 {
        self.observed_read
    }

    pub const fn next_write(&self) -> u64 {
        self.next_write
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlGeometryError {
    ZeroGrid,
    ZeroWorkgroup,
    WorkgroupTooLarge,
    GridSmallerThanWorkgroup,
    InconsistentTrailingDimension,
}

/// Checked one-, two-, or three-dimensional AQL dispatch geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlDispatchGeometryV1 {
    grid: [u32; 3],
    workgroup: [u16; 3],
    dimensions: u16,
}

impl AqlDispatchGeometryV1 {
    pub fn new(grid: [u32; 3], workgroup: [u32; 3]) -> Result<Self, AqlGeometryError> {
        if grid.contains(&0) {
            return Err(AqlGeometryError::ZeroGrid);
        }
        if workgroup.contains(&0) {
            return Err(AqlGeometryError::ZeroWorkgroup);
        }
        if workgroup.iter().any(|value| *value > u16::MAX.into()) {
            return Err(AqlGeometryError::WorkgroupTooLarge);
        }
        if grid
            .iter()
            .zip(workgroup)
            .any(|(grid_value, workgroup_value)| *grid_value < workgroup_value)
        {
            return Err(AqlGeometryError::GridSmallerThanWorkgroup);
        }

        let dimensions = if grid[2] > 1 || workgroup[2] > 1 {
            3
        } else if grid[1] > 1 || workgroup[1] > 1 {
            2
        } else {
            1
        };
        if (dimensions < 3 && (grid[2] != 1 || workgroup[2] != 1))
            || (dimensions < 2 && (grid[1] != 1 || workgroup[1] != 1))
        {
            return Err(AqlGeometryError::InconsistentTrailingDimension);
        }

        Ok(Self {
            grid,
            workgroup: [
                workgroup[0] as u16,
                workgroup[1] as u16,
                workgroup[2] as u16,
            ],
            dimensions,
        })
    }

    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    pub const fn workgroup(self) -> [u16; 3] {
        self.workgroup
    }

    pub const fn dimensions(self) -> u16 {
        self.dimensions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlDispatchPacketError {
    KernelObject(AqlAddressObservationError),
    Kernarg(AqlAddressObservationError),
    CompletionSignal(AqlAddressObservationError),
}

/// Exact unpublished 64-byte AMDHSA kernel-dispatch packet.
///
/// The packet starts with type `INVALID` and its exact setup dimensions. A
/// queue implementation must copy the complete value into an exclusively
/// owned slot before the paired release publication.
#[derive(Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AqlKernelDispatchPacketV1 {
    full_header: u32,
    workgroup_size_x: u16,
    workgroup_size_y: u16,
    workgroup_size_z: u16,
    reserved0: u16,
    grid_size_x: u32,
    grid_size_y: u32,
    grid_size_z: u32,
    private_segment_size: u32,
    group_segment_size: u32,
    kernel_object: u64,
    kernarg_address: u64,
    reserved2: u64,
    completion_signal: u64,
}

impl AqlKernelDispatchPacketV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new_unpublished(
        geometry: AqlDispatchGeometryV1,
        private_segment_size: u32,
        group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1,
        kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64,
        completion_signal: ObservedGpuAddressV1,
    ) -> Result<AqlPreparedKernelDispatchV1, AqlDispatchPacketError> {
        Self::new_unpublished_with_ordering(
            geometry,
            private_segment_size,
            group_segment_size,
            kernel_object,
            kernarg_address,
            kernarg_alignment,
            completion_signal,
            AqlDispatchOrderingV1::Independent,
        )
    }

    /// Constructs an unpublished packet with an explicit execution-order policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new_unpublished_with_ordering(
        geometry: AqlDispatchGeometryV1,
        private_segment_size: u32,
        group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1,
        kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64,
        completion_signal: ObservedGpuAddressV1,
        ordering: AqlDispatchOrderingV1,
    ) -> Result<AqlPreparedKernelDispatchV1, AqlDispatchPacketError> {
        let kernel_object = kernel_object
            .require_alignment(64)
            .map_err(AqlDispatchPacketError::KernelObject)?;
        let kernarg_address = kernarg_address
            .require_alignment(kernarg_alignment)
            .map_err(AqlDispatchPacketError::Kernarg)?;
        let completion_signal = completion_signal
            .require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)
            .map_err(AqlDispatchPacketError::CompletionSignal)?;
        let workgroup = geometry.workgroup();
        let grid = geometry.grid();

        let packet = Self {
            full_header: (u32::from(geometry.dimensions()) << 16)
                | u32::from(AQL_INVALID_PACKET_HEADER_V1),
            workgroup_size_x: workgroup[0],
            workgroup_size_y: workgroup[1],
            workgroup_size_z: workgroup[2],
            reserved0: 0,
            grid_size_x: grid[0],
            grid_size_y: grid[1],
            grid_size_z: grid[2],
            private_segment_size,
            group_segment_size,
            kernel_object: kernel_object.raw(),
            kernarg_address: kernarg_address.raw(),
            reserved2: 0,
            completion_signal: completion_signal.raw(),
        };
        Ok(AqlPreparedKernelDispatchV1 { packet, ordering })
    }

    pub const fn is_unpublished(&self) -> bool {
        self.full_header & 0xff == AQL_INVALID_PACKET_HEADER_V1 as u32
    }

    pub const fn setup_dimensions(&self) -> u16 {
        (self.full_header >> 16) as u16
    }

    pub const fn kernel_object(&self) -> u64 {
        self.kernel_object
    }

    pub const fn kernarg_address(&self) -> u64 {
        self.kernarg_address
    }

    pub const fn completion_signal(&self) -> u64 {
        self.completion_signal
    }

    pub fn encode_unpublished_le(&self) -> [u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1] {
        let mut bytes = [0_u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1];
        bytes[0..4].copy_from_slice(&self.full_header.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.workgroup_size_x.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.workgroup_size_y.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.workgroup_size_z.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.reserved0.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.grid_size_x.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.grid_size_y.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.grid_size_z.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.private_segment_size.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.group_segment_size.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.kernel_object.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.kernarg_address.to_le_bytes());
        bytes[48..56].copy_from_slice(&self.reserved2.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.completion_signal.to_le_bytes());
        bytes
    }
}

/// A linear unpublished packet paired with the invariant release header.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedKernelDispatchV1 {
    packet: AqlKernelDispatchPacketV1,
    ordering: AqlDispatchOrderingV1,
}

impl AqlPreparedKernelDispatchV1 {
    /// Returns the explicit execution-order policy retained for publication.
    pub const fn ordering(&self) -> AqlDispatchOrderingV1 {
        self.ordering
    }

    pub fn publish_with<T: AqlPacketPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        target.write_unpublished(&self.packet)?;
        target.publish_release_header(self.ordering.header())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlPreparedKernelDispatchBatchErrorV1 {
    ZeroPacketCount,
    PacketCountExceedsReviewedMaximum { requested: usize, maximum: u32 },
}

/// A fixed, inert batch of prepared kernel-dispatch packet values.
///
/// Construction only checks the reviewed packet-count bound. Publication
/// through [`Self::publish_with`] preserves a two-phase ordering: every exact
/// INVALID body is written before any release header is exposed to the target.
/// This value owns no queue, slot, counter, address, or completion authority.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedKernelDispatchBatchV1<const N: usize> {
    packets: [AqlPreparedKernelDispatchV1; N],
}

impl<const N: usize> AqlPreparedKernelDispatchBatchV1<N> {
    pub fn try_from_packets(
        packets: [AqlPreparedKernelDispatchV1; N],
    ) -> Result<Self, AqlPreparedKernelDispatchBatchErrorV1> {
        if N == 0 {
            return Err(AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount);
        }
        if N > AQL_MAX_BATCH_PACKETS_V1 as usize {
            return Err(
                AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                    requested: N,
                    maximum: AQL_MAX_BATCH_PACKETS_V1,
                },
            );
        }
        Ok(Self { packets })
    }

    pub const fn packet_count(&self) -> u32 {
        N as u32
    }

    /// Writes all INVALID bodies before release-publishing any header.
    pub fn publish_with<T: AqlPacketBatchPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        for (batch_index, packet) in self.packets.iter().enumerate() {
            target.write_unpublished(batch_index as u32, &packet.packet)?;
        }
        for (batch_index, packet) in self.packets.iter().enumerate() {
            target.publish_release_header(batch_index as u32, packet.ordering.header())?;
        }
        Ok(())
    }
}

impl AqlPreparedKernelDispatchBatchV1<1> {
    pub const fn one(packet: AqlPreparedKernelDispatchV1) -> Self {
        Self { packets: [packet] }
    }
}

/// A fixed inert V2 batch supporting one through 8192 packet values.
///
/// Like V1, this owns no queue, slot, address, counter, completion signal, or
/// publication authority. The separate type preserves the frozen V1 bound.
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedKernelDispatchBatchV2<const N: usize> {
    packets: Box<[AqlPreparedKernelDispatchV1; N]>,
}

impl<const N: usize> AqlPreparedKernelDispatchBatchV2<N> {
    pub fn try_from_packets(
        packets: [AqlPreparedKernelDispatchV1; N],
    ) -> Result<Self, AqlPreparedKernelDispatchBatchErrorV1> {
        Self::try_from_boxed_packets(Box::new(packets))
    }

    /// Admits an already heap-owned exact-cardinality packet set.
    pub fn try_from_boxed_packets(
        packets: Box<[AqlPreparedKernelDispatchV1; N]>,
    ) -> Result<Self, AqlPreparedKernelDispatchBatchErrorV1> {
        if N == 0 {
            return Err(AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount);
        }
        if N > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
            return Err(
                AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                    requested: N,
                    maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2,
                },
            );
        }
        Ok(Self { packets })
    }

    pub const fn packet_count(&self) -> u32 {
        N as u32
    }

    /// Preserves the V1 all-body-before-any-header publication order.
    pub fn publish_with<T: AqlPacketBatchPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        for (batch_index, packet) in self.packets.iter().enumerate() {
            target.write_unpublished(batch_index as u32, &packet.packet)?;
        }
        for (batch_index, packet) in self.packets.iter().enumerate() {
            target.publish_release_header(batch_index as u32, packet.ordering.header())?;
        }
        Ok(())
    }
}

impl AqlPreparedKernelDispatchBatchV2<1> {
    pub fn one(packet: AqlPreparedKernelDispatchV1) -> Self {
        Self {
            packets: Box::new([packet]),
        }
    }
}

/// Backend boundary used to keep one packet body and final header paired.
///
/// Implementing this trait grants no ring or doorbell authority. A production
/// implementation must remain private to the sealed native queue owner. It
/// must construct one release `u32` from this invariant header and the setup
/// halfword already copied into the selected slot.
pub trait AqlPacketPublicationTargetV1 {
    type Error;

    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<(), Self::Error>;

    fn publish_release_header(&mut self, header: u16) -> Result<(), Self::Error>;
}

/// Inert two-phase target boundary for one prepared packet batch.
///
/// Implementing this trait grants no native authority. A production target
/// must remain private to a queue owner that binds each batch index to the
/// matching exclusive native slot and poisons itself after ambiguous effects.
pub trait AqlPacketBatchPublicationTargetV1 {
    type Error;

    fn write_unpublished(
        &mut self,
        batch_index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error>;

    fn publish_release_header(&mut self, batch_index: u32, header: u16) -> Result<(), Self::Error>;
}

/// Encode the exact inert 64-byte image of a pending ROCr user signal.
///
/// The user kind occupies bytes 0 through 7 and the pending value occupies
/// bytes 8 through 15, both little-endian. Every other byte is zero. The
/// returned array is only a wire image; it does not construct an atomic Rust
/// object, validate storage, or grant memory or GPU authority.
pub const fn encode_pending_completion_signal_bytes_v1() -> [u8; AMD_SIGNAL_BYTES_V1] {
    let kind = AMD_SIGNAL_KIND_USER_V1.to_le_bytes();
    let value = AMD_SIGNAL_VALUE_PENDING_V1.to_le_bytes();
    let mut bytes = [0_u8; AMD_SIGNAL_BYTES_V1];
    let mut index = 0;
    while index < size_of::<i64>() {
        bytes[index] = kind[index];
        bytes[8 + index] = value[index];
        index += 1;
    }
    bytes
}

/// Replace an exact-size caller-provided byte array with the pending image.
///
/// This initializes bytes only. In particular, it does not start the lifetime
/// of [`AmdBusyCompletionSignalV1`] in that storage.
pub fn initialize_pending_completion_signal_bytes_v1(destination: &mut [u8; AMD_SIGNAL_BYTES_V1]) {
    *destination = encode_pending_completion_signal_bytes_v1();
}

/// Classify one completion value already obtained by an acquiring observer.
///
/// This pure function performs no memory access and makes no claim that its
/// argument came from an atomic load, a GPU, or a retained completion signal.
pub const fn classify_acquired_completion_value_v1(value: i64) -> AqlCompletionObservationV1 {
    match value {
        AMD_SIGNAL_VALUE_PENDING_V1 => AqlCompletionObservationV1::Pending,
        AMD_SIGNAL_VALUE_COMPLETE_V1 => AqlCompletionObservationV1::Completed,
        unexpected => AqlCompletionObservationV1::Unexpected(unexpected),
    }
}

/// Exact 64-byte ROCr user-signal prefix used by AQL completion packets.
///
/// Event fields stay zero, so this value only supports bounded busy polling.
/// The enclosing allocation and its GPU address must be supplied by a later
/// runtime authority layer.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct AmdBusyCompletionSignalV1 {
    kind: i64,
    value: AtomicI64,
    event_mailbox_ptr: u64,
    event_id: u32,
    reserved1: u32,
    start_ts: u64,
    end_ts: u64,
    reserved2: u64,
    reserved3: [u32; 2],
}

impl AmdBusyCompletionSignalV1 {
    pub const fn new_pending() -> Self {
        Self {
            kind: AMD_SIGNAL_KIND_USER_V1,
            value: AtomicI64::new(AMD_SIGNAL_VALUE_PENDING_V1),
            event_mailbox_ptr: 0,
            event_id: 0,
            reserved1: 0,
            start_ts: 0,
            end_ts: 0,
            reserved2: 0,
            reserved3: [0; 2],
        }
    }

    /// Acquire one value and delegate only its pure classification.
    pub fn observe_acquire(&self) -> AqlCompletionObservationV1 {
        classify_acquired_completion_value_v1(self.value.load(Ordering::Acquire))
    }
}

impl Default for AmdBusyCompletionSignalV1 {
    fn default() -> Self {
        Self::new_pending()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlCompletionObservationV1 {
    Pending,
    Completed,
    Unexpected(i64),
}

const _: () = {
    assert!(size_of::<AqlKernelDispatchPacketV1>() == AQL_KERNEL_DISPATCH_PACKET_BYTES_V1);
    assert!(align_of::<AqlKernelDispatchPacketV1>() == 8);
    assert!(offset_of!(AqlKernelDispatchPacketV1, full_header) == 0);
    assert!(offset_of!(AqlKernelDispatchPacketV1, workgroup_size_x) == 4);
    assert!(offset_of!(AqlKernelDispatchPacketV1, grid_size_x) == 12);
    assert!(offset_of!(AqlKernelDispatchPacketV1, private_segment_size) == 24);
    assert!(offset_of!(AqlKernelDispatchPacketV1, group_segment_size) == 28);
    assert!(offset_of!(AqlKernelDispatchPacketV1, kernel_object) == 32);
    assert!(offset_of!(AqlKernelDispatchPacketV1, kernarg_address) == 40);
    assert!(offset_of!(AqlKernelDispatchPacketV1, reserved2) == 48);
    assert!(offset_of!(AqlKernelDispatchPacketV1, completion_signal) == 56);

    assert!(size_of::<AmdBusyCompletionSignalV1>() == AMD_SIGNAL_BYTES_V1);
    assert!(align_of::<AmdBusyCompletionSignalV1>() == AMD_SIGNAL_ALIGNMENT_V1);
    assert!(offset_of!(AmdBusyCompletionSignalV1, kind) == 0);
    assert!(offset_of!(AmdBusyCompletionSignalV1, value) == 8);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_observation_is_exact() {
        let signal = AmdBusyCompletionSignalV1::new_pending();
        assert_eq!(
            signal.observe_acquire(),
            AqlCompletionObservationV1::Pending
        );
        signal.value.store(0, Ordering::Release);
        assert_eq!(
            signal.observe_acquire(),
            AqlCompletionObservationV1::Completed
        );
        signal.value.store(-7, Ordering::Release);
        assert_eq!(
            signal.observe_acquire(),
            AqlCompletionObservationV1::Unexpected(-7)
        );
    }
}
