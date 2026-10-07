//! Settled fixed-dispatch readback and host overwrites.

use super::*;

impl ComputeAqlQueueSessionV1 {
    /// Returns the exact dispatch generation only while its completion signals
    /// have been observed and recycled and the same batch remains attached.
    pub fn recycled_fixed_dispatch_generation(&self) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .ensure_returnable()
            .map_err(Into::into)
    }

    /// Copies one inspected writable subrange from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Device-local
    /// storage, read-only or unwritten bytes, stale generations, invalid bounds,
    /// and requests intersecting more than one admitted writable range fail
    /// before any mapped bytes are exposed.
    pub fn read_recycled_fixed_dispatch_data(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Read,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one inspected writable coherent subrange into caller-owned bytes.
    ///
    /// This has the same generation, effect, kind, and bounds checks as
    /// [`Self::read_recycled_fixed_dispatch_data`] but avoids an intermediate
    /// owned readback allocation when the caller already owns the destination.
    pub fn read_recycled_fixed_dispatch_data_into(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::ReadInto,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_into(memory, request, destination);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one exact admitted enclosing snapshot from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Admission
    /// requires a retained fully initialized range strictly enclosing one
    /// isolated inspected writable binding. Subranges, stale generations,
    /// device-local storage, and undeclared ranges fail before bytes are exposed.
    pub fn read_recycled_fixed_dispatch_snapshot(
        &mut self,
        request: Gfx942CompletedDispatchSnapshotRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Snapshot,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_snapshot(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies retained, fully initialized coherent data after exact recycle.
    ///
    /// Unlike writable-output readback, this also permits read-only and
    /// unreferenced initialized inputs. The sealed initialization premise, exact
    /// dispatch generation, vacant epoch slots, native allocation authority and
    /// bounds are required. No mapped borrow, initialization promotion, inspected
    /// write coverage or permission to reuse the input is returned.
    pub fn read_recycled_fixed_dispatch_initialized_data_into(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::InitializedReadInto,
        )?;
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result =
            dispatch.read_completed_initialized_host_visible_into(memory, request, destination);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Returns exact recycled generation and retained DATA cardinality.
    /// No storage authority or permission to publish is transferred.
    pub fn recycled_fixed_dispatch_data_shape_v1(
        &self,
    ) -> Result<(u64, usize), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::InitializedReadInto,
        )?;
        self.dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .recycled_data_shape_v1()
            .map_err(Into::into)
    }

    /// Overwrites one initialized coherent range while the attached dispatch
    /// is exactly completed, recycled, and ready for another generation.
    pub fn overwrite_recycled_fixed_dispatch_host_data(
        &mut self,
        request: Gfx942RecycledDispatchWriteRequestV1,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Overwrite,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.overwrite_recycled_host_visible(memory, request, source);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }
}
