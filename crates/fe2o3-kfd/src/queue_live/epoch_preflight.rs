//! Borrowed lane admission reserves storage without moving any native owner.

use super::*;

fn preallocate_next_binding_v1<const N: usize>(
    capacity: &Gfx942FixedDispatchCapacityV1,
    key: QueueKeyV1,
    dispatch: Option<&DispatchResourceOwnerV1>,
    unpublished: &UnpublishedDispatchStateV1,
    detached: Option<u64>,
    completion: &CompletionSignalArenaOwnerV1,
) -> Result<Option<Gfx942FixedDispatchPreallocationV1>, ComputeAqlQueueSessionErrorV1> {
    capacity.validate_batch::<N>()?;
    completion.ensure_releasable()?;
    let prepared = if let Some(dispatch) = dispatch {
        if !unpublished.is_clear() || detached.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        PreparedDispatchGenerationV1::preallocate::<N>(
            capacity,
            DispatchGenerationSeedV1::Recycled(dispatch.ensure_returnable()?),
        )?
    } else if let Some(continuation) = unpublished.continuation.as_ref() {
        if !unpublished.is_detached()
            || detached.is_some()
            || !continuation.matches_capacity(capacity)
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        continuation.preallocate_resume::<N>()?
    } else if unpublished.is_clear() {
        PreparedDispatchGenerationV1::preallocate::<N>(
            capacity,
            DispatchGenerationSeedV1::Detached(
                detached.ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?,
            ),
        )?
    } else {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
    };
    Ok(prepared.map(|prepared| prepared.for_queue(key)))
}

impl ComputeAqlQueueSessionV1 {
    /// Reserves the next recipe's vacant epoch storage for an exact live lane.
    /// This only borrows queue state: no detach, DATA move, continuation take,
    /// currentness callback or native operation occurs. An attached recipe must
    /// already be recycled. Temporary old/new table overlap needs extra credit.
    /// The reservation is storage, not a receipt or execution authorization.
    pub fn preallocate_next_fixed_dispatch_v1<const N: usize>(
        &self,
        lane: ComputeAqlQueueLaneV1,
    ) -> Result<Option<Gfx942FixedDispatchPreallocationV1>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        match admit_compute_lane_v1(
            self.compute_lane_session,
            &self.auxiliary_compute_lanes,
            lane,
        )? {
            AdmittedComputeLaneV1::Primary => preallocate_next_binding_v1::<N>(
                &self.dispatch_capacity,
                self.key,
                self.dispatch.as_ref(),
                &self.unpublished_dispatch,
                self.detached_dispatch_generation,
                &self.completion_owner,
            ),
            AdmittedComputeLaneV1::Auxiliary(index) => {
                let lane = self.auxiliary_compute_lanes[index]
                    .state
                    .as_ref()
                    .expect("admitted lane");
                preallocate_next_binding_v1::<N>(
                    &self.dispatch_capacity,
                    lane.key,
                    lane.dispatch.as_ref(),
                    &lane.unpublished_dispatch,
                    lane.detached_dispatch_generation,
                    &lane.completion_owner,
                )
            }
        }
    }
}
