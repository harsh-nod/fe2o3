//! Fixed-dispatch DATA binding, insertion, and detachment.

use super::*;

impl ComputeAqlQueueSessionV1 {
    pub fn detach_recycled_fixed_dispatch(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.detach_recycled_fixed_dispatch_inner()
    }

    /// Consumes a detached persistent dispatch's immutable code/kernarg
    /// control without changing the queue's detached-generation ledger.
    pub fn release_retained_persistent_fixed_dispatch_control_v1(
        &mut self,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        let settled = self.release_retained_control_settled_v1();
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    pub(in super::super) fn detach_recycled_fixed_dispatch_inner(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.detach_recycled_settled_v1();
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Binds a new fixed batch to the same live native queue.
    ///
    /// The queue must have no attached batch. The complete detached data set is
    /// rebound, and its device-local subset is revalidated against the retained
    /// KFD session before the new owner is installed. Every mapped storage input
    /// and inspected program is retained even when no packet in this batch
    /// selects it. This does not publish.
    /// Rejected inputs and incomplete ordinary preparation remain retained;
    /// terminal failures retain the whole session until process teardown.
    pub fn bind_fixed_dispatch<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.bind_fixed_dispatch_with_preallocation_v1(programs, packets, data, None)
    }

    /// Binds using optional queue-bound vacant storage. Consumed inputs retain
    /// the ordinary rebind failure-custody contract, including on rejection.
    pub fn bind_fixed_dispatch_with_preallocation_v1<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
        preallocation: Option<Gfx942FixedDispatchPreallocationV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let settled = self.bind_fixed_dispatch_settled_v1(programs, packets, data, preallocation);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates and maps one uninitialized device-local extent while no fixed
    /// batch is attached.
    pub fn allocate_uninitialized_fixed_dispatch_data(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.allocate_device_data_settled_v1(requested_bytes, alignment);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates, writes, verifies, CPU-unmaps, and GPU-maps one fully
    /// initialized device-local extent while no fixed batch is attached.
    pub fn initialize_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.initialize_device_data_settled_v1(None, bytes, alignment, content);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates and inserts one initialized device-local extent at an exact
    /// detached data ordinal without replacing an existing allocation.
    ///
    /// The insertion ordinal is validated before allocation. It is intended
    /// for service ledgers that keep device-local entries before coherent host
    /// entries while changing the detached allocation cardinality.
    pub fn insert_initialized_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled =
            self.initialize_device_data_settled_v1(Some(data_index), bytes, alignment, content);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Validates an exact detached-data insertion without allocating, mapping,
    /// or changing queue state.
    ///
    /// Service layers can use this before mutating their own allocation ledger,
    /// so a full lower data roster or an invalid ordinal remains a retry-safe
    /// rejection with unchanged custody.
    pub fn preflight_fixed_dispatch_data_insertion(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)
    }

    /// Overwrites one initialized coherent extent retained from the immediately
    /// preceding completed dispatch while the queue is unbound.
    ///
    /// The exact detached storage identity and bounds are checked before the
    /// mapped bytes are changed. Native handles and GPU addresses remain private.
    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        data: &mut Gfx942FixedDispatchDataV1,
        offset: u64,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        let expected_identity = *self.detached_data_identities.get(data_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite ordinal",
            },
        )?;
        if data.storage_identity() != expected_identity {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite storage identity",
            }
            .into());
        }
        let end = offset
            .checked_add(u64::try_from(source.len()).map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index: data_index,
                    detail: "detached overwrite source length",
                }
            })?)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range overflow",
            })?;
        if source.is_empty() || end > data.layout().requested_bytes() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range",
            }
            .into());
        }
        let token = data.initialized_host_visible_token_mut().ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite requires initialized coherent storage",
            },
        )?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .overwrite_mapped_host_visible_subrange(token, offset, source)
                .map_err(Into::into)
        });
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    /// Allocates, initializes, and inserts one coherent host-visible extent at
    /// an exact detached data ordinal.
    pub fn insert_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(data_index, &bytes)
    }

    /// Synchronously copies a complete borrowed extent at an exact detached
    /// ordinal. The source borrow does not escape; this does not publish work.
    pub fn insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        data_index: usize,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.initialize_coherent_data_settled_v1(Some(data_index), bytes);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates and initializes one coherent host-visible extent at the exact
    /// ordinal vacated by the immediately preceding detached release.
    pub fn initialize_host_visible_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.initialize_host_visible_fixed_dispatch_data_from_slice_v1(&bytes)
    }

    /// Synchronously initializes the exact vacated detached ordinal from a
    /// borrowed extent, preserving the existing release/replacement ledger.
    pub fn initialize_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.initialize_coherent_data_settled_v1(None, bytes);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates, maps, and inserts one uninitialized coherent host-visible
    /// extent at an exact detached data ordinal.
    pub fn insert_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.allocate_coherent_data_settled_v1(Some(data_index), requested_bytes);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Allocates and maps one uninitialized coherent host-visible extent at the
    /// exact ordinal vacated by the immediately preceding detached release.
    pub fn allocate_host_visible_fixed_dispatch_data(
        &mut self,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        let settled = self.allocate_coherent_data_settled_v1(None, requested_bytes);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    /// Unmaps and releases detached fixed-dispatch storage exactly once.
    pub fn release_detached_fixed_dispatch_data(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let settled = self.release_data_settled_v1(data);
        if settled.transport {
            self.retain_terminal_rebind_parent_v1(core::mem::forget);
        }
        settled.into_result()
    }

    pub(in super::super) fn require_unbound_fixed_dispatch(
        &self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.dispatch.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if !(self.unpublished_dispatch.is_clear() && self.detached_dispatch_generation.is_some()
            || self.unpublished_dispatch.is_detached()
                && self.detached_dispatch_generation.is_none())
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_data_count > super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger bound",
            ));
        }
        if self.detached_data_identities.len() != self.detached_data_count
            || self
                .detached_next_insertion_index
                .is_some_and(|index| index > self.detached_data_identities.len())
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        Ok(())
    }

    pub(in super::super) fn require_new_detached_data_index(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        validate_new_detached_data_index(self.detached_data_count, data_index).map_err(Into::into)
    }

    pub(in super::super) fn require_detached_allocation_capacity(
        &self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.detached_data_count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
                requested: self.detached_data_count + 1,
                maximum: super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1,
            }
            .into());
        }
        Ok(())
    }
}
