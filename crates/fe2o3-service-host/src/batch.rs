//! Addressless fixed-dispatch batch descriptions for a service queue.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use fe2o3_amdhsa_loader::ValidatedKernelEnvelope;
use fe2o3_aql::{AqlDispatchGeometryV1, AqlDispatchOrderingV1};
use fe2o3_kfd::{Gfx942DispatchBufferBindingV1, Gfx942FixedDispatchPacketV1};

use crate::allocation::{
    ServiceAllocationErrorV1, ServiceAllocationSessionV1, ServiceDeviceDispatchRangeV1,
    ServiceDispatchRangeV1, ServiceHostDispatchRangeV1, ServiceHostDispatchSnapshotRangeV1,
    ServiceQueueAllocationLedgerV1, validate_host_dispatch_snapshot,
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
    range: ServiceDispatchRangeV1,
    completed_snapshot: Option<ServiceHostDispatchSnapshotRangeV1>,
}

impl ServiceFixedDispatchBufferV1 {
    /// Describes one explicit argument and checked addressless device range.
    pub const fn new(explicit_argument_index: usize, range: ServiceDeviceDispatchRangeV1) -> Self {
        Self {
            explicit_argument_index,
            range: ServiceDispatchRangeV1::Device(range),
            completed_snapshot: None,
        }
    }

    /// Describes one explicit argument and checked coherent host-visible range.
    pub const fn new_host_visible(
        explicit_argument_index: usize,
        range: ServiceHostDispatchRangeV1,
    ) -> Self {
        Self {
            explicit_argument_index,
            range: ServiceDispatchRangeV1::HostVisible(range),
            completed_snapshot: None,
        }
    }

    /// Describes one host-visible interior pointer and initialized enclosing snapshot.
    ///
    /// This association is inert. Inspected metadata must independently
    /// establish a write or read-write effect for the interior, and only a
    /// matching completed and recycled generation can copy the exact snapshot.
    ///
    /// # Errors
    ///
    /// Rejects ranges from different allocation identities, generations,
    /// ordinals, or subleases, and snapshots that do not strictly enclose the
    /// interior on both sides.
    pub fn new_host_visible_with_completed_snapshot(
        explicit_argument_index: usize,
        interior: ServiceHostDispatchRangeV1,
        snapshot: ServiceHostDispatchSnapshotRangeV1,
    ) -> Result<Self, ServiceAllocationErrorV1> {
        validate_host_dispatch_snapshot(interior, snapshot.dispatch_range())?;
        Ok(Self {
            explicit_argument_index,
            range: ServiceDispatchRangeV1::HostVisible(interior),
            completed_snapshot: Some(snapshot),
        })
    }

    /// Returns the explicit argument ordinal.
    pub const fn explicit_argument_index(&self) -> usize {
        self.explicit_argument_index
    }

    /// Returns the addressless service allocation range.
    pub const fn range(&self) -> ServiceDispatchRangeV1 {
        self.range
    }

    /// Returns the optional enclosing completed-snapshot descriptor.
    pub const fn completed_snapshot(&self) -> Option<ServiceHostDispatchSnapshotRangeV1> {
        self.completed_snapshot
    }

    fn into_kfd(self) -> Gfx942DispatchBufferBindingV1 {
        match self.completed_snapshot {
            Some(snapshot) => Gfx942DispatchBufferBindingV1::new_with_completed_snapshot(
                self.explicit_argument_index,
                self.range.data_index(),
                self.range.offset_bytes(),
                self.range.extent_bytes(),
                snapshot.offset_bytes(),
                snapshot.extent_bytes(),
            ),
            None => Gfx942DispatchBufferBindingV1::new(
                self.explicit_argument_index,
                self.range.data_index(),
                self.range.offset_bytes(),
                self.range.extent_bytes(),
            ),
        }
    }
}

/// One inert packet description in a fixed service batch.
///
/// `kernarg_bytes` must be the complete inspected kernarg image with zero bytes
/// in every device-pointer field. If supported COV6 implicit fields are
/// declared, its exact trailing 256-byte implicit suffix must also be entirely
/// zero. The private KFD owner checks those conditions and performs admitted
/// pointer and implicit-value substitution after consuming allocation custody.
pub struct ServiceFixedDispatchPacketV1 {
    program_index: usize,
    geometry: AqlDispatchGeometryV1,
    ordering: AqlDispatchOrderingV1,
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
        Self::with_ordering(
            program_index,
            geometry,
            AqlDispatchOrderingV1::WaitForPrior,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        )
    }

    /// Creates a packet that may execute independently of earlier queue packets.
    pub fn new_independent(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[ServiceFixedDispatchBufferV1]>,
    ) -> Self {
        Self::with_ordering(
            program_index,
            geometry,
            AqlDispatchOrderingV1::Independent,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        )
    }

    fn with_ordering(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        ordering: AqlDispatchOrderingV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[ServiceFixedDispatchBufferV1]>,
    ) -> Self {
        Self {
            program_index,
            geometry,
            ordering,
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

    /// Returns the explicit AQL execution-order policy.
    pub const fn ordering(&self) -> AqlDispatchOrderingV1 {
        self.ordering
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
            match (buffer.range, buffer.completed_snapshot) {
                (ServiceDispatchRangeV1::HostVisible(interior), Some(snapshot)) => {
                    ledger.validate_host_dispatch_snapshot(interior, snapshot)?;
                }
                (ServiceDispatchRangeV1::Device(_), Some(_)) => {
                    return Err(ServiceAllocationErrorV1::KindMismatch);
                }
                (_, None) => {}
            }
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
        Gfx942FixedDispatchPacketV1::new_with_ordering(
            self.program_index,
            self.geometry,
            self.ordering,
            self.dynamic_group_segment_bytes,
            self.kernarg_bytes,
            buffers,
        )
    }

    fn to_kfd_preflight(&self) -> Gfx942FixedDispatchPacketV1 {
        let buffers = self
            .buffers
            .iter()
            .copied()
            .map(ServiceFixedDispatchBufferV1::into_kfd)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Gfx942FixedDispatchPacketV1::new_with_ordering(
            self.program_index,
            self.geometry,
            self.ordering,
            self.dynamic_group_segment_bytes,
            self.kernarg_bytes.clone(),
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
            .field("ordering", &self.ordering)
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
                match buffer.range {
                    ServiceDispatchRangeV1::Device(range) => {
                        allocation.validate_device_dispatch_range(range)?
                    }
                    ServiceDispatchRangeV1::HostVisible(range) => {
                        allocation.validate_host_dispatch_range(range)?;
                        if let Some(snapshot) = buffer.completed_snapshot {
                            allocation.validate_host_dispatch_snapshot(range, snapshot)?;
                        }
                    }
                }
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

    pub(crate) fn preflight_replacement(
        &self,
        ring_bytes: u32,
        data: &[fe2o3_kfd::Gfx942FixedDispatchDataV1],
        predecessor_generation: u64,
    ) -> Result<u64, fe2o3_kfd::Gfx942DispatchBindingErrorV1> {
        let packets: [Gfx942FixedDispatchPacketV1; N] =
            core::array::from_fn(|index| self.packets[index].to_kfd_preflight());
        fe2o3_kfd::preflight_gfx942_fixed_dispatch_replacement(
            ring_bytes,
            &self.programs,
            &packets,
            data,
            predecessor_generation,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_packet_default_is_ordered_and_independent_is_explicit() {
        let geometry = AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap();
        let ordered = ServiceFixedDispatchPacketV1::new(
            0,
            geometry,
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        assert_eq!(ordered.ordering(), AqlDispatchOrderingV1::WaitForPrior);
        assert_eq!(
            ordered.into_kfd().ordering(),
            AqlDispatchOrderingV1::WaitForPrior
        );

        let independent = ServiceFixedDispatchPacketV1::new_independent(
            0,
            geometry,
            0,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        assert_eq!(independent.ordering(), AqlDispatchOrderingV1::Independent);
        assert_eq!(
            independent.into_kfd().ordering(),
            AqlDispatchOrderingV1::Independent
        );
    }
}
