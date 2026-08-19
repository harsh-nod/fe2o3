#![no_std]
#![forbid(unsafe_code)]

//! Inert, source-pinned AMDHSA AQL packet and busy-wait signal contracts.
//!
//! This crate validates values and lays out bytes. It does not own GPU virtual
//! addresses, publish packets, map a doorbell, create a queue, or establish
//! that firmware consumed a packet.

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
source.amd_aql_queue.cpp=291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14
packet=size:64,align:8,header:0,setup:2,workgroup:4,grid:12,private:24,group:28,kernel-object:32,kernarg:40,reserved2:48,completion-signal:56
publication=single-release-u32-at-offset-0,type:2,barrier:0,acquire:system-2,release:system-2,header:0x1402,setup-dimensions:1..3
ring-reservation=packet-bytes:64,ring-bytes:4096..2147483648-power-of-two,capacity:64..33554432,monotonic-u64-no-wrap,read<=write,distance<=capacity,slot:packet-id&(capacity-1)
signal=size:64,align:64,kind-offset:0,value-offset:8,kind:user-1,pending:1,complete:0,event-fields:zero,busy-poll-only
address-observations=nonzero,kernel-object-align:64,completion-signal-align:64,kernarg-align:caller-supplied-power-of-two-1..4096
authority=inert-wire-values-only,no-address-provenance,no-allocation,no-queue,no-publication,no-doorbell,no-execution
"#;

/// SHA-256 of [`AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1`].
pub const AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1: &str =
    "fac47d309307e25ea464b9a0ae1253b4cf49c653df8958d1ee6f0cee5df05e7e";

/// Typed SHA-256 bytes of [`AQL_DISPATCH_ABI_SCHEMA_MANIFEST_V1`].
pub const AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_BYTES_V1: [u8; 32] = [
    0xfa, 0xc4, 0x7d, 0x30, 0x93, 0x07, 0xe2, 0x5e, 0xa4, 0x64, 0xb9, 0xa0, 0xae, 0x12, 0x53, 0xb4,
    0xcf, 0x49, 0xc6, 0x53, 0xdf, 0x89, 0x58, 0xd1, 0xee, 0x6f, 0x0c, 0xee, 0x5d, 0xf0, 0x5e, 0x7e,
];

pub const AQL_KERNEL_DISPATCH_PACKET_BYTES_V1: usize = 64;
pub const AMD_SIGNAL_BYTES_V1: usize = 64;
pub const AMD_SIGNAL_ALIGNMENT_V1: usize = 64;
pub const AMD_SIGNAL_KIND_USER_V1: i64 = 1;
pub const AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1: u16 = 0x1402;
pub const AQL_MIN_RING_BYTES_V1: u32 = 4096;
pub const AQL_MAX_RING_BYTES_V1: u32 = 1 << 31;

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
    ReadAfterWrite,
    CounterDistanceExceedsCapacity,
    Full,
    WriteCounterExhausted,
}

/// One observed pair of monotonic queue counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlRingCounterSnapshotV1 {
    write: u64,
    read: u64,
}

impl AqlRingCounterSnapshotV1 {
    pub const fn new(write: u64, read: u64) -> Self {
        Self { write, read }
    }

    pub const fn write(self) -> u64 {
        self.write
    }

    pub const fn read(self) -> u64 {
        self.read
    }

    pub const fn reserve_one(
        self,
        capacity: AqlRingCapacityV1,
    ) -> Result<AqlRingReservationV1, AqlRingReservationError> {
        let Some(distance) = self.write.checked_sub(self.read) else {
            return Err(AqlRingReservationError::ReadAfterWrite);
        };
        let capacity_u64 = capacity.packets as u64;
        if distance > capacity_u64 {
            return Err(AqlRingReservationError::CounterDistanceExceedsCapacity);
        }
        if distance == capacity_u64 {
            return Err(AqlRingReservationError::Full);
        }
        let Some(next_write) = self.write.checked_add(1) else {
            return Err(AqlRingReservationError::WriteCounterExhausted);
        };
        Ok(AqlRingReservationV1 {
            packet_id: self.write,
            slot_index: (self.write & capacity.mask()) as u32,
            observed_read: self.read,
            next_write,
        })
    }
}

/// Unique slot selected from one admitted counter snapshot.
///
/// This remains an inert arithmetic result. It is not a lease on native ring
/// memory and cannot publish or advance a write pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlRingReservationV1 {
    packet_id: u64,
    slot_index: u32,
    observed_read: u64,
    next_write: u64,
}

impl AqlRingReservationV1 {
    pub const fn packet_id(self) -> u64 {
        self.packet_id
    }

    pub const fn slot_index(self) -> u32 {
        self.slot_index
    }

    pub const fn observed_read(self) -> u64 {
        self.observed_read
    }

    pub const fn next_write(self) -> u64 {
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
/// `full_header` is always zero. A queue implementation must first copy this
/// complete value into an exclusively owned slot and only then publish
/// [`AqlPreparedKernelDispatchV1::publication_word`] with one release atomic
/// store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            full_header: 0,
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
        Ok(AqlPreparedKernelDispatchV1 {
            packet,
            publication_word: (AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1 as u32)
                | ((geometry.dimensions() as u32) << 16),
        })
    }

    pub const fn is_unpublished(self) -> bool {
        self.full_header == 0
    }

    pub const fn kernel_object(self) -> u64 {
        self.kernel_object
    }

    pub const fn kernarg_address(self) -> u64 {
        self.kernarg_address
    }

    pub const fn completion_signal(self) -> u64 {
        self.completion_signal
    }
}

/// An unpublished packet paired with its non-substitutable release word.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AqlPreparedKernelDispatchV1 {
    packet: AqlKernelDispatchPacketV1,
    publication_word: u32,
}

impl AqlPreparedKernelDispatchV1 {
    pub const fn packet(self) -> AqlKernelDispatchPacketV1 {
        self.packet
    }

    pub const fn publication_word(self) -> u32 {
        self.publication_word
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
            value: AtomicI64::new(1),
            event_mailbox_ptr: 0,
            event_id: 0,
            reserved1: 0,
            start_ts: 0,
            end_ts: 0,
            reserved2: 0,
            reserved3: [0; 2],
        }
    }

    pub fn observe_acquire(&self) -> AqlCompletionObservationV1 {
        match self.value.load(Ordering::Acquire) {
            1 => AqlCompletionObservationV1::Pending,
            0 => AqlCompletionObservationV1::Completed,
            value => AqlCompletionObservationV1::Unexpected(value),
        }
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
