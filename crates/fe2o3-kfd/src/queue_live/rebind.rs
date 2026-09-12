//! Retain consumed rebind inputs independently of deferred whole-session custody.

use super::*;

pub(in crate::queue) struct LiveRebindRootV1<'a, const N: usize> {
    pub(in crate::queue) programs: Option<Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>>,
    pub(in crate::queue) packets: Option<[Gfx942FixedDispatchPacketV1; N]>,
    pub(in crate::queue) data: Option<Vec<Gfx942FixedDispatchDataV1>>,
    pub(in crate::queue) preparation: Option<FixedDispatchPreparationCustodyV1<N>>,
    pub(in crate::queue) predecessor: Option<u64>,
    pub(in crate::queue) continuation: Option<PristineDispatchContinuationV1>,
    ordinary_entered: bool,
    pristine_entered: bool,
}

impl<'a, const N: usize> LiveRebindRootV1<'a, N> {
    pub(in crate::queue) fn new(
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
        predecessor: Option<u64>,
    ) -> Box<Self> {
        Box::new(Self {
            programs: Some(programs),
            packets: Some(packets),
            data: Some(data),
            preparation: None,
            predecessor,
            continuation: None,
            ordinary_entered: false,
            pristine_entered: false,
        })
    }
}

pub(in crate::queue) struct SettledLiveRebindV1 {
    pub(in crate::queue) result: std::thread::Result<Result<(), ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledLiveRebindV1 {
    pub(super) fn into_result(self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn bind_fixed_dispatch_settled_v1<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> SettledLiveRebindV1 {
        let root =
            LiveRebindRootV1::new(programs, packets, data, self.detached_dispatch_generation);
        self.settle_fixed_dispatch_rebind_with_v1(
            root,
            |session, programs, preparation, predecessor, continuation| {
                session.with_live_queue_memory_model(|memory| {
                    match predecessor {
                        Some(predecessor) => super::super::dispatch_binding::prepare_public_fixed_dispatch_resources_after_detach_in_place(
                            memory, programs, preparation, predecessor,
                        ),
                        None => prepare_public_fixed_dispatch_resources_after_pristine_abort_in_place_v1(
                            memory, programs, preparation,
                            continuation.take().expect("one unpublished continuation"),
                        ),
                    }.map_err(Into::into)
                })
            },
            Self::validate_persistent_bind_preparation_v1,
            core::mem::forget,
        )
    }

    pub(in crate::queue) fn settle_fixed_dispatch_rebind_with_v1<'a, const N: usize>(
        &mut self,
        mut root: Box<LiveRebindRootV1<'a, N>>,
        prepare: impl FnOnce(
            &mut Self,
            &[fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>],
            &mut FixedDispatchPreparationCustodyV1<N>,
            Option<u64>,
            &mut Option<PristineDispatchContinuationV1>,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
        validate: impl FnOnce(
            &mut Self,
            &FixedDispatchPreparationCustodyV1<N>,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
        retain: impl FnOnce(Box<LiveRebindRootV1<'a, N>>),
    ) -> SettledLiveRebindV1 {
        let was_terminal = self.terminal_poisoned;
        let result = settle_persistent_bind_preparation_v1(
            self,
            &mut root,
            |session, root| {
                session.preflight_fixed_dispatch_rebind_v1::<N>(
                    root.data.as_ref().expect("rooted rebind inputs"),
                )?;
                if session.unpublished_dispatch.is_detached() {
                    root.pristine_entered = true;
                    root.continuation = session.unpublished_dispatch.continuation.take();
                } else {
                    root.ordinary_entered = true;
                }
                root.preparation = Some(FixedDispatchPreparationCustodyV1::new(
                    root.packets.take().expect("rooted packets"),
                    root.data.take().expect("rooted data"),
                ));
                prepare(
                    session,
                    root.programs.as_ref().expect("rooted programs"),
                    root.preparation.as_mut().expect("rooted preparation"),
                    root.predecessor,
                    &mut root.continuation,
                )
            },
            |session, root| match &root.preparation {
                Some(preparation) => validate(session, preparation),
                None => Ok(()),
            },
        );
        let result = result.map(|result| {
            result.and_then(|()| {
                if let Some(preparation) = root.preparation.as_mut() {
                    let prepared = preparation.take_completed()?;
                    self.dispatch = Some(prepared);
                    self.detached_data_count = 0;
                    self.detached_dispatch_generation = None;
                    self.detached_data_identities.clear();
                    self.detached_next_insertion_index = None;
                }
                Ok(())
            })
        });
        let failed = !matches!(result, Ok(Ok(())));
        let transport = result.is_err()
            || failed
                && (root.ordinary_entered
                    || root.pristine_entered
                    || !was_terminal && self.terminal_poisoned);
        if transport {
            self.poison_terminal();
            // Admitted pristine rebind is process-terminal even on opening error.
            // Ordinary returned errors keep their existing local classification.
            if result.is_err() || root.pristine_entered {
                poison_process_global_after_dispatch_terminal_v1();
            }
        }
        if failed {
            retain(root);
        }
        SettledLiveRebindV1 { result, transport }
    }

    fn preflight_fixed_dispatch_rebind_v1<const N: usize>(
        &mut self,
        data: &[Gfx942FixedDispatchDataV1],
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
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let data_identities = fixed_dispatch_storage_identities(data);
        if self.detached_data_identities.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger cardinality",
            ));
        }
        if data.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data.len().min(self.detached_data_count),
                detail: "detached dispatch-data cardinality",
            }
            .into());
        }
        if let Some(index) =
            first_ordered_identity_mismatch(&self.detached_data_identities, &data_identities)
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "detached rebind storage identity",
            }
            .into());
        }
        let preflight = self
            .completion_owner
            .ensure_releasable()
            .map_err(ComputeAqlQueueSessionErrorV1::from)
            .and_then(|()| {
                validate_fixed_batch_ring::<N>(self.observation.ring_bytes).map_err(Into::into)
            });
        if preflight.is_err() && self.unpublished_dispatch.is_detached() {
            self.poison_terminal();
        }
        preflight
    }

    pub(super) fn retain_terminal_rebind_parent_v1(
        &mut self,
        retain: impl FnOnce(Box<Option<Self>>),
    ) {
        let mut root = Box::new(None);
        *root = Some(self.take_for_terminal_auxiliary_construction_v1());
        retain(root);
    }
}
