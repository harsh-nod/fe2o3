//! Addressless fixed-dispatch batch descriptions for a service queue.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use fe2o3_amdhsa_loader::ValidatedKernelEnvelope;
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{Gfx942DispatchBufferBindingV1, Gfx942FixedDispatchPacketV1};

use crate::allocation::{
    ServiceAllocationErrorV1, ServiceAllocationSessionV1, ServiceDeviceDispatchRangeV1,
    ServiceQueueAllocationLedgerV1,
};

/// One inert explicit kernarg-buffer binding to a checked service allocation range.
///
/// This value exposes no native allocation identity or device address. The
/// explicit argument index is checked against inspected executable metadata by
/// the retained KFD queue owner before it substitutes a private address.
///
/// ```compile_fail
/// use fe2o3_service_host::{ServiceDeviceDispatchRangeV1, ServiceFixedDispatchBufferV1};
///
/// fn forge(range: ServiceDeviceDispatchRangeV1) -> ServiceFixedDispatchBufferV1 {
///     ServiceFixedDispatchBufferV1 {
///         explicit_argument_index: 0,
///         range,
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceFixedDispatchBufferV1 {
    explicit_argument_index: usize,
    range: ServiceDeviceDispatchRangeV1,
}

impl ServiceFixedDispatchBufferV1 {
    /// Describes one explicit argument and checked addressless device range.
    pub const fn new(explicit_argument_index: usize, range: ServiceDeviceDispatchRangeV1) -> Self {
        Self {
            explicit_argument_index,
            range,
        }
    }

    /// Returns the explicit argument ordinal.
    pub const fn explicit_argument_index(&self) -> usize {
        self.explicit_argument_index
    }

    /// Returns the addressless service allocation range.
    pub const fn range(&self) -> ServiceDeviceDispatchRangeV1 {
        self.range
    }

    fn into_kfd(self) -> Gfx942DispatchBufferBindingV1 {
        Gfx942DispatchBufferBindingV1::new(
            self.explicit_argument_index,
            self.range.data_index,
            self.range.offset_bytes,
            self.range.extent_bytes,
        )
    }
}

/// One inert packet description in a fixed service batch.
///
/// `kernarg_bytes` must be the complete inspected kernarg image with zero bytes
/// in every device-pointer field. The private KFD owner checks that condition
/// and performs all pointer substitution after consuming allocation custody.
pub struct ServiceFixedDispatchPacketV1 {
    program_index: usize,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    kernarg_bytes: Box<[u8]>,
    buffers: Box<[ServiceFixedDispatchBufferV1]>,
}

impl ServiceFixedDispatchPacketV1 {
    /// Creates an inert packet description without granting dispatch authority.
    pub fn new(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[ServiceFixedDispatchBufferV1]>,
    ) -> Self {
        Self {
            program_index,
            geometry,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        }
    }

    /// Returns the selected program ordinal.
    pub const fn program_index(&self) -> usize {
        self.program_index
    }

    /// Returns the checked AQL geometry description.
    pub const fn geometry(&self) -> AqlDispatchGeometryV1 {
        self.geometry
    }

    /// Returns the requested dynamic group-segment bytes.
    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.dynamic_group_segment_bytes
    }

    /// Returns the number of explicit buffer bindings.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    fn validate(
        &self,
        ledger: &ServiceQueueAllocationLedgerV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        for buffer in &self.buffers {
            ledger.validate_range(buffer.range)?;
        }
        Ok(())
    }

    fn into_kfd(self) -> Gfx942FixedDispatchPacketV1 {
        let buffers = self
            .buffers
            .into_vec()
            .into_iter()
            .map(ServiceFixedDispatchBufferV1::into_kfd)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Gfx942FixedDispatchPacketV1::new(
            self.program_index,
            self.geometry,
            self.dynamic_group_segment_bytes,
            self.kernarg_bytes,
            buffers,
        )
    }
}

impl fmt::Debug for ServiceFixedDispatchPacketV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceFixedDispatchPacketV1")
            .field("program_index", &self.program_index)
            .field("geometry", &self.geometry)
            .field(
                "dynamic_group_segment_bytes",
                &self.dynamic_group_segment_bytes,
            )
            .field("kernarg_bytes", &self.kernarg_bytes.len())
            .field("buffer_count", &self.buffers.len())
            .finish_non_exhaustive()
    }
}

/// A move-only fixed-cardinality batch and its inspected executable custody.
///
/// Construction is descriptive. Only a compatible service queue owner can
/// consume this value, validate every retained allocation binding, and ask KFD
/// to prepare native resources.
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceFixedBatchV1;
///
/// fn cannot_clone(batch: ServiceFixedBatchV1<'_, 1>) {
///     let _ = batch.clone();
/// }
/// ```
#[must_use = "an inspected fixed batch must be consumed by a service queue owner"]
pub struct ServiceFixedBatchV1<'a, const N: usize> {
    programs: Vec<ValidatedKernelEnvelope<'a>>,
    packets: [ServiceFixedDispatchPacketV1; N],
}

impl<'a, const N: usize> ServiceFixedBatchV1<'a, N> {
    /// Retains inspected programs and exactly `N` inert packet descriptions.
    pub fn new(
        programs: Vec<ValidatedKernelEnvelope<'a>>,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Self {
        Self { programs, packets }
    }

    /// Returns the compile-time packet count.
    pub const fn packet_count(&self) -> usize {
        N
    }

    /// Returns the retained inspected-program count.
    pub fn program_count(&self) -> usize {
        self.programs.len()
    }

    pub(crate) fn validate(
        &self,
        ledger: &ServiceQueueAllocationLedgerV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        for packet in &self.packets {
            packet.validate(ledger)?;
        }
        Ok(())
    }

    pub(crate) fn validate_for_allocation(
        &self,
        allocation: &ServiceAllocationSessionV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        for packet in &self.packets {
            for buffer in &packet.buffers {
                allocation.validate_device_dispatch_range(buffer.range)?;
            }
        }
        Ok(())
    }

    pub(crate) fn into_kfd(
        self,
    ) -> (
        Vec<ValidatedKernelEnvelope<'a>>,
        [Gfx942FixedDispatchPacketV1; N],
    ) {
        (self.programs, self.packets.map(|packet| packet.into_kfd()))
    }
}

impl<const N: usize> fmt::Debug for ServiceFixedBatchV1<'_, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceFixedBatchV1")
            .field("program_count", &self.programs.len())
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}
