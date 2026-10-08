//! An opt-in whole-destination copy attached to one original producer ticket.

use super::*;

pub(super) struct Request {
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) destination: RuntimeMemoryRegionV1,
    pub(super) submission: Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
}

pub(super) type Step<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut RuntimeGfx942PreparedV1<P>,
    &GeneratedHostRosterV1,
    &ContextUnpublishedHoldV1,
    RuntimeStreamIdV1,
    RuntimeMemoryRegionV1,
    &mut Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
) -> Result<bool, NativeError>;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    pub(super) fn generated_data_copy_hook_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>()
    -> Option<Step<KfdRuntimeBackendV1, P>> {
        Some(Self::advance_gfx942_data_copy_v1::<P>)
    }
}
impl RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1> {
    pub(super) fn generated_data_copy_hook_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>()
    -> Option<Step<KfdMultiDeviceRuntimeBackendV1, P>> {
        None
    }
}

impl<'scope, P: RuntimeGfx942GeneratedCompletionCarrierV1>
    RuntimeGfx942GeneratedScopeV1<'scope, '_, KfdRuntimeBackendV1, P>
{
    /// Attaches an actual DATA-to-SDMA copy to one not-yet-adopted producer.
    ///
    /// The producer must use opt-in Singleton SDMA storage on the primary
    /// backend. The complete ordinary DeviceLocal destination and distinct copy
    /// stream must belong to its admitted device. Graphs and generated consumers
    /// are unsupported. No generated DATA is cast into an ordinary allocation.
    ///
    /// The existing producer result remains gated until copy completion, original
    /// frontier and source release, destination journal settlement, producer
    /// settlement and decoding. Refusal or ambiguity after entry retains owners;
    /// dropping the observer is not cancellation.
    pub fn copy_generated_output_to_v1(
        &mut self,
        producer: &RuntimeGfx942ScopedTicketV1<'scope>,
        stream: RuntimeStreamIdV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        let _permit = self
            .epoch
            .enter()
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        // Reject graph admission before touching a destination writer or owner.
        self.context
            .require_graph_access(None)
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        if self.graph.is_some() || !Rc::ptr_eq(&producer.scope, &self.identity) {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        let slot = self
            .slots
            .get(producer.index)
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)?;
        let refused =
            || RuntimeGfx942ScopeErrorV1::Context(RuntimeValidationErrorV1::ContextReserved.into());
        if self.hooks.data_copy.is_none()
            || self.context.versions.is_none()
            || !slot.sdma_backed
            || slot.lifecycle.phase != Phase::Adopting
            || slot.data_copy.is_some()
            || slot.roster.count != 1
            || slot.roster.source_identity.profile()
                != crate::generated_source::GeneratedProfileV1::Singleton
        {
            return Err(refused());
        }
        let validate = || -> Result<(), RuntimeValidationErrorV1> {
            let transfer = self.context.unheld_stream_v1(stream)?;
            let original = self
                .context
                .streams
                .get(&slot.hold.stream())
                .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
            let allocation = self
                .context
                .allocations
                .get(&destination.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            if stream == slot.hold.stream()
                || transfer.device != original.device
                || allocation.device != original.device
            {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            if allocation.kind != RuntimeMemoryKindV1::DeviceLocal
                || destination.byte_offset != 0
                || destination.byte_len != allocation.byte_len
                || destination.byte_len == 0
                || u32::try_from(destination.byte_len).is_err()
                || slot.roster.buffers[0].is_none_or(|b| b.bytes != destination.byte_len)
            {
                return Err(RuntimeValidationErrorV1::InvalidRange);
            }
            if !matches!(
                destination.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            ) {
                return Err(RuntimeValidationErrorV1::InvalidAccess);
            }
            if self
                .context
                .submissions
                .values()
                .any(|r| r.stream == stream && !r.quiescent)
                || self
                    .copies
                    .iter()
                    .any(|copy| copy.conflicts_with_generated(stream, destination.allocation))
                || self
                    .slots
                    .iter()
                    .filter(|slot| slot.lifecycle.unsettled())
                    .filter_map(|s| s.data_copy.as_ref())
                    .any(|r| {
                        r.stream == stream || r.destination.allocation == destination.allocation
                    })
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            Ok(())
        };
        validate().map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        self.slots[producer.index].data_copy = Some(Request {
            stream,
            destination,
            submission: None,
        });
        Ok(())
    }
}
