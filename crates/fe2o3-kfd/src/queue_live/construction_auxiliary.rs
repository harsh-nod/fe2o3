//! Auxiliary construction retains the original shared session on terminal failure.

use super::construction_primary::{
    ExecutablePrefixV1, LinuxPrimaryEnvironmentV1 as Platform, MutablePrefixV1,
    PrimaryEnvironmentV1, PrimaryMemoryV1, UserptrConstructionEntryV1, map_executable_prefix,
    map_mutable_prefix, retain_executable_prefix, retain_mutable_prefix,
    run_rooted_construction_with_v1,
};
use super::*;
use crate::queue::dispatch_binding::preparation::PreparationMemoryV1;

pub(super) struct AuxiliaryQueueTargetV1<'a, E: PrimaryEnvironmentV1> {
    pub(super) engine: &'a mut NativeQueueEngineV1<PrimaryQueueBackendV1<E::Memory>>,
    pub(super) primary: &'a ComputeAqlQueueObservationV1,
    pub(super) lanes: &'a mut Vec<AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1<E>>>,
    pub(super) sdma: Option<&'a Gfx942SdmaQueueSetV1>,
    pub(super) striped_sdma: Option<&'a Gfx942SdmaQueueSetV1>,
}

impl<E: PrimaryEnvironmentV1> AuxiliaryQueueTargetV1<'_, E> {
    fn session_owned_queue_id_is_retained_v1(&self, queue_id: u32) -> bool {
        let auxiliary_collision = self
            .lanes
            .iter()
            .filter_map(|slot| slot.state.as_ref())
            .any(|lane| lane.observation.queue_id == queue_id);
        let sdma_collision = self
            .sdma
            .is_some_and(|owner| owner.contains_confirmed_queue_id(queue_id))
            || self
                .striped_sdma
                .is_some_and(|owner| owner.contains_confirmed_queue_id(queue_id));
        queue_id_collides_with_session_owned_roster_v1(
            queue_id,
            self.primary.queue_id,
            auxiliary_collision,
            sdma_collision,
        )
    }
}

pub(super) struct QueueOwnerSlotV1<T>(pub(super) Option<T>);

impl<T> core::ops::Deref for QueueOwnerSlotV1<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
            .as_ref()
            .expect("queue owner retained in terminal custody")
    }
}

impl<T> core::ops::DerefMut for QueueOwnerSlotV1<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0
            .as_mut()
            .expect("queue owner retained in terminal custody")
    }
}

impl ComputeAqlQueueSessionV1 {
    fn take_for_terminal_auxiliary_construction_v1(&mut self) -> Self {
        self.poison_terminal();
        let shell = Self {
            engine: None,
            key: self.key,
            compute_lane_session: self.compute_lane_session,
            doorbell: None,
            submission: None,
            completion_signals: None,
            completion_owner: QueueOwnerSlotV1(None),
            dependency_owner: QueueOwnerSlotV1(None),
            terminal_dependency: None,
            dispatch: None,
            unpublished_dispatch: UnpublishedDispatchStateV1::default(),
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            persistent_compute: None,
            #[cfg(test)]
            persistent_compute_test_release: None,
            next_persistent_compute_generation: self.next_persistent_compute_generation,
            exception: None,
            sdma: None,
            striped_sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
            sdma_device_pool: SdmaDevicePoolConfigurationV1 {
                limits: self.sdma_device_pool.limits,
                activity_started: self.sdma_device_pool.activity_started,
            },
            sdma_host_pool_limits: self.sdma_host_pool_limits,
            terminal_poisoned: true,
            observation: self.observation,
            auxiliary_compute_lanes: Vec::new(),
        };
        core::mem::replace(self, shell)
    }
}

pub(super) struct AuxiliaryConstructionV1<const N: usize, E: PrimaryEnvironmentV1 = Platform> {
    packets: Option<[Gfx942FixedDispatchPacketV1; N]>,
    pub(super) data: Option<Vec<Gfx942FixedDispatchDataV1>>,
    pub(super) preparation: Option<FixedDispatchPreparationCustodyV1<N>>,
    pub(super) dispatch: Option<DispatchResourceOwnerV1>,
    pub(super) ring: Option<RingConstructionV1>,
    pub(super) control: MutablePrefixV1<UserptrAqlControlGttV1, ControlAuthority>,
    pub(super) completion: MutablePrefixV1<HostVisibleCoherentGttV1, CompletionSignalAuthority>,
    pub(super) eop: ExecutablePrefixV1<EopAuthority>,
    pub(super) context: ExecutablePrefixV1<ContextSaveAuthority>,
    pub(super) runtime: Option<E::Runtime>,
    pub(super) event: Option<E::Event>,
    pub(super) unpublished: Option<E::Unpublished>,
    pub(super) published: Option<E::Published>,
    pub(super) resource_prefix: Option<QueueResourcePrefixV1>,
    pub(super) authority: Option<QueueResourceAuthorityV1>,
    pub(super) completion_owner: Option<CompletionSignalArenaOwnerV1>,
    pub(super) submission: Option<NativeAqlSubmissionOwnerV1>,
    pub(super) key: Option<QueueKeyV1>,
    pub(super) outputs: Option<fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs>,
    pub(super) completed: Option<ComputeAqlQueueLaneStateV1<E>>,
    pub(super) creation_arm: Option<E::CreationArm>,
}

struct AuxiliaryConstructionScopeV1<'a, const N: usize> {
    parent: &'a mut ComputeAqlQueueSessionV1,
    construction: AuxiliaryConstructionV1<N>,
    terminal_parent: Option<ComputeAqlQueueSessionV1>,
}

impl<const N: usize, E: PrimaryEnvironmentV1> AuxiliaryConstructionV1<N, E> {
    pub(super) fn new(packets: [Gfx942FixedDispatchPacketV1; N]) -> Self {
        Self {
            packets: Some(packets),
            data: None,
            preparation: None,
            dispatch: None,
            ring: None,
            control: MutablePrefixV1::new(),
            completion: MutablePrefixV1::new(),
            eop: ExecutablePrefixV1::new(),
            context: ExecutablePrefixV1::new(),
            runtime: None,
            event: None,
            unpublished: None,
            published: None,
            resource_prefix: None,
            authority: None,
            completion_owner: None,
            submission: None,
            key: None,
            outputs: None,
            completed: None,
            creation_arm: None,
        }
    }

    pub(super) fn prepare_dispatch(
        &mut self,
        memory: &mut E::Memory,
        entry: &mut UserptrConstructionEntryV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
        programs: &[fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>],
        prepare_data: impl FnOnce(
            &mut E::Memory,
        ) -> Result<
            Vec<Gfx942FixedDispatchDataV1>,
            ComputeAqlQueueSessionErrorV1,
        >,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>
    where
        E::Memory: PreparationMemoryV1,
    {
        capture_returned_preparation_v1(memory, &mut self.data, prepare_data)?;
        self.preparation = Some(FixedDispatchPreparationCustodyV1::new(
            self.packets.take().expect("fixed packets"),
            self.data.take().expect("returned data"),
        ));
        let preparation = self.preparation.as_mut().expect("preparation custody");
        super::super::dispatch_binding::prepare_public_fixed_dispatch_resources_in_place(
            memory,
            programs,
            preparation,
        )?;
        self.dispatch = Some(preparation.take_completed()?);
        self.prepare(memory, entry, geometry, ring_bytes)
    }

    fn prepare(
        &mut self,
        memory: &mut E::Memory,
        entry: &mut UserptrConstructionEntryV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.ring = Some(RingConstructionV1::Cpu(
            memory.allocate_ring(
                QueueRingBackingV1::AqlSpecial,
                usize::try_from(ring_bytes)
                    .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring size conversion"))?,
            )?,
        ));
        entry.enter("USERPTR auxiliary queue-control creation");
        self.control.cpu = Some(memory.allocate_userptr_aql_control()?);
        self.completion.cpu =
            Some(memory.allocate_host_visible_coherent(COMPLETION_SIGNAL_ARENA_BYTES_V1)?);
        self.eop.cpu = Some(
            memory.allocate_executable(
                usize::try_from(geometry.end_of_pipe().mapping_bytes())
                    .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("EOP size conversion"))?,
            )?,
        );
        self.context.cpu = Some(memory.allocate_executable(
            usize::try_from(geometry.context_save().mapping_bytes()).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("context-save size conversion")
            })?,
        )?);
        let RingConstructionV1::Cpu(ring) = self.ring.as_mut().expect("allocated ring") else {
            unreachable!("initial CPU ring")
        };
        memory
            .initialize_ring(ring)?
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("INVALID ring initialization"))?;
        memory
            .with_bytes_mut(
                self.control.cpu.as_mut().expect("control"),
                initialize_amd_aql_control,
            )?
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("AMD AQL control initialization")
            })?;
        memory.with_bytes_mut(
            self.completion.cpu.as_mut().expect("completion"),
            initialize_pending_completion_signal_arena,
        )??;
        memory.with_bytes_mut(self.eop.cpu.as_mut().expect("EOP"), |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(self.context.cpu.as_mut().expect("context-save"), |bytes| {
            bytes.fill(0)
        })?;
        memory.check_queue_currentness()?;

        self.runtime = Some(E::enable_runtime(memory)?);
        E::validate_runtime(self.runtime.as_ref().expect("runtime"), None, memory)?;
        memory.check_queue_currentness()?;
        self.creation_arm = Some(E::arm_creation(memory)?);
        self.event = Some(E::create_event(memory)?);
        memory.check_queue_currentness()?;
        self.unpublished = Some(E::install_shadows(
            memory,
            self.context.cpu.as_ref().expect("context-save"),
            self.event.as_ref().expect("event"),
        )?);
        let unpublished = self.unpublished.as_ref().expect("unpublished shadows");
        E::initialize_shadows(
            memory,
            self.context.cpu.as_mut().expect("context-save"),
            unpublished,
        )?;
        E::validate_runtime(self.runtime.as_ref().expect("runtime"), None, memory)?;
        E::validate_event(memory, self.event.as_ref().expect("event"), unpublished)?;
        memory.check_queue_currentness()?;

        self.eop.seal(memory)?;
        self.context.seal(memory)?;
        E::restore_shadow_write(unpublished)?;
        let ring = self.ring.as_mut().expect("ring");
        ring.map_in_place(memory)?;
        ring.retain_in_place(memory)?;
        map_mutable_prefix(memory, &mut self.control)?;
        map_mutable_prefix(memory, &mut self.completion)?;
        map_executable_prefix(memory, &mut self.eop)?;
        map_executable_prefix(memory, &mut self.context)?;
        retain_mutable_prefix(
            memory,
            &mut self.control,
            E::Memory::retain_aql_control_resource,
        )?;
        retain_mutable_prefix(
            memory,
            &mut self.completion,
            E::Memory::retain_aql_completion_signal_resource,
        )?;
        retain_executable_prefix(memory, &mut self.eop, E::Memory::retain_aql_eop_resource)?;
        retain_executable_prefix(
            memory,
            &mut self.context,
            E::Memory::retain_aql_context_save_resource,
        )?;
        self.resource_prefix = Some(QueueResourcePrefixV1::new(
            ring.take_retained()?,
            self.control.retained.take().expect("retained control"),
            self.eop.retained.take().expect("retained EOP"),
            self.context.retained.take().expect("retained context-save"),
        ));
        let prefix = self.resource_prefix.as_mut().expect("resource prefix");
        prefix.build_in_place(memory.queue_model_device(), geometry)?;
        self.authority = Some(prefix.take_complete()?);
        self.completion_owner = Some(CompletionSignalArenaOwnerV1::new(
            self.authority
                .as_ref()
                .expect("resource authority")
                .view
                .plan
                .queue,
            self.completion
                .retained
                .as_ref()
                .expect("completion authority")
                .facts(),
        )?);
        self.submission =
            Some(NativeAqlSubmissionOwnerV1::new(ring_bytes).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("AQL ring submission model")
            })?);
        Ok(())
    }

    pub(super) fn create_and_install(
        &mut self,
        parent: AuxiliaryQueueTargetV1<'_, E>,
        ring_bytes: u32,
        slot: PreparedAuxiliaryComputeLaneSlotV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = &mut *parent.engine;
        self.key = Some(
            engine
                .admit_in_place(&mut self.authority)
                .map_err(|error| {
                    if matches!(error, NativeQueueAdapterErrorV1::AuthorityPoisoned) {
                        terminal_creation("auxiliary queue model admission", map_native(error))
                    } else {
                        map_native(error)
                    }
                })?,
        );
        let key = self.key.expect("admitted auxiliary key");
        engine
            .create_at_native_boundary(key, || {
                self.published = Some(E::publish_shadows(
                    self.unpublished.take().expect("unpublished shadows"),
                ));
            })
            .map_err(map_create)?;
        E::mark_queue_created(self.runtime.as_mut().expect("runtime"))?;
        self.outputs = Some(E::recover_create_outputs(engine, key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE output recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs"),
            )
        })?);
        let queue_id = E::recover_native_queue_id(engine, key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE identity recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing queue id"),
            )
        })?;
        if parent.session_owned_queue_id_is_retained_v1(queue_id) {
            return Err(terminal_creation(
                "auxiliary compute queue ID admission",
                ComputeAqlQueueSessionErrorV1::Contract(
                    "auxiliary compute queue ID collides with a session-owned queue",
                ),
            ));
        }
        let observation = ComputeAqlQueueObservationV1 {
            queue_id,
            ring_bytes,
            doorbell_slice_bytes: 0,
            doorbell_byte_offset: 0,
            event_id: E::event_id(self.event.as_ref().expect("event")),
            cwsr_shadow_pages: 8,
        };
        self.completed = Some(ComputeAqlQueueLaneStateV1 {
            key,
            doorbell: None,
            submission: self.submission.take(),
            completion_signals: self.completion.retained.take(),
            completion_owner: QueueOwnerSlotV1(self.completion_owner.take()),
            dispatch: self.dispatch.take(),
            unpublished_dispatch: UnpublishedDispatchStateV1::default(),
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            exception: Some(QueueExceptionStateV1 {
                runtime: self.runtime.take().expect("runtime"),
                runtime_control: None,
                event: self.event.take().expect("event"),
                shadows: self.published.take().expect("published shadows"),
            }),
            observation,
        });
        parent.engine.prepare_operation().map_err(map_native)?;
        let engine = &*parent.engine;
        let pid = engine.opener_pid;
        let completed = self.completed.as_mut().expect("completed auxiliary lane");
        completed.doorbell = Some(E::map_doorbell(
            &engine.backend.session,
            self.outputs.expect("CREATE outputs"),
            pid,
        )?);
        let (bytes, offset) =
            E::doorbell_observation(completed.doorbell.as_ref().expect("doorbell"));
        completed.observation.doorbell_slice_bytes = bytes;
        completed.observation.doorbell_byte_offset = offset;
        parent.engine.prepare_operation().map_err(map_native)?;
        let destination = check_auxiliary_compute_lane_slot_v1(parent.lanes, slot)?;
        E::finish_creation(self.creation_arm.as_mut().expect("creation arm"), pid)?;
        install_auxiliary_compute_lane_slot_v1(
            destination,
            self.completed.take().expect("completed auxiliary lane"),
        );
        Ok(())
    }
}

pub(super) fn construct_auxiliary_compute_lane_v1<const N: usize>(
    parent: &mut ComputeAqlQueueSessionV1,
    ring_bytes: u32,
    programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
    packets: [Gfx942FixedDispatchPacketV1; N],
    slot: PreparedAuxiliaryComputeLaneSlotV1,
    prepare_data: impl FnOnce(
        &mut SharedGttMemorySessionV1,
    )
        -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1>,
) -> Result<ComputeAqlQueueLaneV1, ComputeAqlQueueSessionErrorV1> {
    let scope = Box::new(AuxiliaryConstructionScopeV1 {
        parent,
        construction: AuxiliaryConstructionV1::new(packets),
        terminal_parent: None,
    });
    let scope = settle_auxiliary_construction_with_v1(
        scope,
        ComputeAqlQueueSessionV1::check_currentness,
        |scope, entry| {
            let root = &mut scope.construction;
            let (result, retake) = scope
                .parent
                .with_live_queue_memory_model_custody(|memory| {
                    let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
                    root.prepare_dispatch(
                        memory,
                        entry,
                        geometry,
                        ring_bytes,
                        &programs,
                        prepare_data,
                    )
                })?;
            retake?;
            result?;
            root.create_and_install(
                AuxiliaryQueueTargetV1 {
                    engine: scope.parent.engine.as_mut().expect("checked queue engine"),
                    primary: &scope.parent.observation,
                    lanes: &mut scope.parent.auxiliary_compute_lanes,
                    sdma: scope.parent.sdma.as_ref(),
                    striped_sdma: scope.parent.striped_sdma.as_ref(),
                },
                ring_bytes,
                slot,
            )
        },
        &Platform::poison,
        |root| {
            let _retained = Box::into_raw(root);
        },
    )?;
    Ok(ComputeAqlQueueLaneV1 {
        session: scope.parent.compute_lane_session,
        ordinal: slot.index + 1,
        generation: slot.generation,
    })
}

fn settle_auxiliary_construction_with_v1<'a, const N: usize>(
    scope: Box<AuxiliaryConstructionScopeV1<'a, N>>,
    opening: impl FnOnce(&mut ComputeAqlQueueSessionV1) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    work: impl FnOnce(
        &mut AuxiliaryConstructionScopeV1<'a, N>,
        &mut UserptrConstructionEntryV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    poison: &dyn Fn(),
    retain: impl FnOnce(Box<AuxiliaryConstructionScopeV1<'a, N>>),
) -> Result<Box<AuxiliaryConstructionScopeV1<'a, N>>, ComputeAqlQueueSessionErrorV1> {
    run_rooted_construction_with_v1(
        scope,
        |scope, entry| {
            opening(scope.parent)?;
            work(scope, entry)
        },
        |scope| {
            scope.terminal_parent =
                Some(scope.parent.take_for_terminal_auxiliary_construction_v1());
            if let Some(unpublished) = scope.construction.unpublished.as_mut() {
                Platform::cleanup_unpublished(unpublished);
            }
        },
        poison,
        retain,
    )
}

#[cfg(test)]
#[path = "construction_auxiliary/tests.rs"]
mod tests;
