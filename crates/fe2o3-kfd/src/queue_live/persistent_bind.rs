//! Borrowed preparation and validation before the infallible bind commit.

use super::*;

pub(super) fn validate_persistent_bind_inputs_v1<C, I, E>(
    context: &mut C,
    inputs: &[I],
    mut validate: impl FnMut(&mut C, &I) -> Result<(), E>,
) -> std::thread::Result<Result<(), E>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for input in inputs {
            validate(context, input)?;
        }
        Ok(())
    }))
}

pub(in crate::queue) fn settle_persistent_bind_preparation_v1<C, P, E>(
    context: &mut C,
    preparation: &mut P,
    prepare: impl FnOnce(&mut C, &mut P) -> Result<(), E>,
    validate: impl FnOnce(&mut C, &P) -> Result<(), E>,
) -> std::thread::Result<Result<(), E>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepare(context, preparation)?;
        validate(context, preparation)
    }))
}

impl PersistentRetainedControlReplayCustodyV1 {
    pub(super) fn mapped_facts(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Self::Input(request) = self else {
            unreachable!("replay input phase")
        };
        let allocation = persistent_compute_input_allocation_mut_v1(&mut request.input);
        let lease = allocation
            .owner
            .local_native_for_sdma()
            .expect("replay preflight validated attached local native custody");
        memory
            .mapped_gfx942_device_memory_facts(lease)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub(super) fn detach(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Self::Input(request) = self else {
            unreachable!("replay input phase")
        };
        let allocation = persistent_compute_input_allocation_mut_v1(&mut request.input);
        let lease = allocation
            .owner
            .detach_local_native_for_compute(&request.prepared)
            .map_err(map_directional_persistent_sdma_use_error_v1)?;
        let Self::Input(request) = core::mem::replace(self, Self::Empty) else {
            unreachable!("borrowed input phase")
        };
        let (allocation, authenticated_sha256, fully_initialized) = request.input.into_parts();
        *self = Self::Storage(PersistentRetainedControlReplayStorageV1 {
            replay: PersistentRetainedControlReplayDetachedV1 {
                allocation,
                prepared: request.prepared,
                dispatch: request.dispatch,
                authenticated_sha256,
                fully_initialized,
            },
            lease,
            initialized_content: request.initialized_content,
            control_identity: request.control_identity,
            predecessor_generation: request.predecessor_generation,
        });
        Ok(())
    }

    pub(super) fn construct(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Self::Storage(storage) = self else {
            unreachable!("replay storage phase")
        };
        if storage
            .initialized_content
            .is_some_and(|content| content.byte_len() != storage.lease.layout().requested_bytes())
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "persistent compute authenticated extent changed after preflight",
            ));
        }
        let Self::Storage(storage) = core::mem::replace(self, Self::Empty) else {
            unreachable!("borrowed storage phase")
        };
        let data = match storage.initialized_content {
            Some(content) => {
                match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                    storage.lease,
                    content,
                ) {
                    Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                    Err(lease) => {
                        *self = Self::Storage(PersistentRetainedControlReplayStorageV1 {
                            lease,
                            ..storage
                        });
                        return Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute authenticated extent changed after preflight",
                        ));
                    }
                }
            }
            None => Gfx942FixedDispatchDataV1::uninitialized(storage.lease),
        };
        *self = Self::Data(PersistentRetainedControlReplayDataV1 {
            replay: storage.replay,
            data: Some(data),
            control_identity: storage.control_identity,
            predecessor_generation: storage.predecessor_generation,
        });
        Ok(())
    }

    pub(super) fn retain(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Self::Data(data) = self else {
            unreachable!("replay data phase")
        };
        data.replay
            .dispatch
            .retain_persistent_replay_data_in_place_v1(
                memory,
                data.control_identity,
                &mut data.data,
                data.predecessor_generation,
            )?;
        let Self::Data(data) = core::mem::replace(self, Self::Empty) else {
            unreachable!("borrowed data phase")
        };
        *self = Self::Attached(data.replay);
        Ok(())
    }

    pub(super) fn audit(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Self::Attached(replay) = self else {
            unreachable!("replay attached phase")
        };
        let authorities = replay.dispatch.device_authorities_inline_v1();
        memory
            .validate_persistent_replay_dispatch_memory(&authorities)
            .map_err(Into::into)
    }

    pub(super) fn into_bind_outcome(
        self,
        result: Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> PersistentRetainedControlReplayOutcomeV1 {
        use PersistentRetainedControlReplayOutcomeV1 as Outcome;
        use PersistentRetainedControlReplayPipelineOutcomeV1 as Pipeline;
        match self.into_outcome(result) {
            Pipeline::BeforeDetach { request, error } => Outcome::BeforeDetach { request, error },
            Pipeline::Storage { storage, error } => Outcome::AfterDetach {
                replay: storage.replay,
                custody: PersistentComputeTerminalNativeCustodyV1::Storage(
                    Gfx942SdmaBufferStorageV1::Device(storage.lease),
                ),
                error,
            },
            Pipeline::Data { data, error } => Outcome::AfterDetach {
                replay: data.replay,
                custody: match data.data {
                    Some(data) => PersistentComputeTerminalNativeCustodyV1::Data(
                        PersistentComputeTerminalDataV1::from_one(data),
                    ),
                    None => PersistentComputeTerminalNativeCustodyV1::Attached,
                },
                error,
            },
            Pipeline::Attached { attached, error } => Outcome::AfterDetach {
                replay: attached,
                custody: PersistentComputeTerminalNativeCustodyV1::Attached,
                error,
            },
            Pipeline::Ready(replay) => Outcome::Ready(replay),
        }
    }
}
