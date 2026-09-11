//! Auxiliary construction retains the original shared session on terminal failure.

use super::construction_primary::{
    ExecutablePrefixV1, LinuxPrimaryEnvironmentV1 as Platform, MutablePrefixV1,
    PrimaryEnvironmentV1, UserptrConstructionEntryV1, map_executable_prefix, map_mutable_prefix,
    retain_executable_prefix, retain_mutable_prefix, run_rooted_construction_with_v1,
};
use super::*;
use crate::queue_linux::ProcessGlobalKfdRuntimeCreationArmV1;

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

struct AuxiliaryConstructionV1<const N: usize> {
    packets: Option<[Gfx942FixedDispatchPacketV1; N]>,
    data: Option<Vec<Gfx942FixedDispatchDataV1>>,
    preparation: Option<FixedDispatchPreparationCustodyV1<N>>,
    dispatch: Option<DispatchResourceOwnerV1>,
    ring: Option<RingConstructionV1>,
    control: MutablePrefixV1<UserptrAqlControlGttV1, ControlAuthority>,
    completion: MutablePrefixV1<HostVisibleCoherentGttV1, CompletionSignalAuthority>,
    eop: ExecutablePrefixV1<EopAuthority>,
    context: ExecutablePrefixV1<ContextSaveAuthority>,
    runtime: Option<LinuxKfdRuntimeEnabledV1>,
    event: Option<LinuxQueueExceptionEventV1>,
    unpublished: Option<LinuxUnpublishedCwsrShadowPagesV1>,
    published: Option<LinuxCwsrShadowPagesV1>,
    resource_prefix: Option<QueueResourcePrefixV1>,
    authority: Option<QueueResourceAuthorityV1>,
    completion_owner: Option<CompletionSignalArenaOwnerV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    key: Option<QueueKeyV1>,
    outputs: Option<fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs>,
    completed: Option<ComputeAqlQueueLaneStateV1>,
    creation_arm: Option<ProcessGlobalKfdRuntimeCreationArmV1>,
    terminal_parent: Option<ComputeAqlQueueSessionV1>,
}

struct AuxiliaryConstructionScopeV1<'a, const N: usize> {
    parent: &'a mut ComputeAqlQueueSessionV1,
    construction: AuxiliaryConstructionV1<N>,
}

impl<const N: usize> AuxiliaryConstructionV1<N> {
    fn new(packets: [Gfx942FixedDispatchPacketV1; N]) -> Self {
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
            terminal_parent: None,
        }
    }

    fn prepare(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        entry: &mut UserptrConstructionEntryV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.ring = Some(RingConstructionV1::Cpu(CpuRingAuthorityV1::allocate(
            memory,
            QueueRingBackingV1::AqlSpecial,
            usize::try_from(ring_bytes)
                .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring size conversion"))?,
        )?));
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
        ring.initialize_invalid(memory)?
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

        self.runtime = Some(Platform::enable_runtime(memory)?);
        Platform::validate_runtime(self.runtime.as_ref().expect("runtime"), None, memory)?;
        memory.check_queue_currentness()?;
        self.creation_arm = Some(Platform::arm_creation(memory)?);
        self.event = Some(Platform::create_event(memory)?);
        memory.check_queue_currentness()?;
        self.unpublished = Some(Platform::install_shadows(
            memory,
            self.context.cpu.as_ref().expect("context-save"),
            self.event.as_ref().expect("event"),
        )?);
        let unpublished = self.unpublished.as_ref().expect("unpublished shadows");
        Platform::initialize_shadows(
            memory,
            self.context.cpu.as_mut().expect("context-save"),
            unpublished,
        )?;
        Platform::validate_runtime(self.runtime.as_ref().expect("runtime"), None, memory)?;
        Platform::validate_event(memory, self.event.as_ref().expect("event"), unpublished)?;
        memory.check_queue_currentness()?;

        self.eop.seal(memory)?;
        self.context.seal(memory)?;
        Platform::restore_shadow_write(unpublished)?;
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
            SharedGttMemorySessionV1::retain_aql_control_resource,
        )?;
        retain_mutable_prefix(
            memory,
            &mut self.completion,
            SharedGttMemorySessionV1::retain_aql_completion_signal_resource,
        )?;
        retain_executable_prefix(
            memory,
            &mut self.eop,
            SharedGttMemorySessionV1::retain_aql_eop_resource,
        )?;
        retain_executable_prefix(
            memory,
            &mut self.context,
            SharedGttMemorySessionV1::retain_aql_context_save_resource,
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

    fn create_and_install(
        &mut self,
        parent: &mut ComputeAqlQueueSessionV1,
        ring_bytes: u32,
        slot: PreparedAuxiliaryComputeLaneSlotV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = parent
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
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
                self.published = Some(Platform::publish_shadows(
                    self.unpublished.take().expect("unpublished shadows"),
                ));
            })
            .map_err(map_create)?;
        Platform::mark_queue_created(self.runtime.as_mut().expect("runtime"))?;
        self.outputs = Some(
            Platform::recover_create_outputs(engine, key).ok_or_else(|| {
                terminal_creation(
                    "CREATE_QUEUE output recovery",
                    ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs"),
                )
            })?,
        );
        let queue_id = Platform::recover_native_queue_id(engine, key).ok_or_else(|| {
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
            event_id: Platform::event_id(self.event.as_ref().expect("event")),
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
        parent.check_currentness()?;
        let engine = parent.engine.as_ref().expect("checked queue engine");
        let pid = engine.opener_pid;
        let completed = self.completed.as_mut().expect("completed auxiliary lane");
        completed.doorbell = Some(Platform::map_doorbell(
            &engine.backend.session,
            self.outputs.expect("CREATE outputs"),
            pid,
        )?);
        let (bytes, offset) =
            Platform::doorbell_observation(completed.doorbell.as_ref().expect("doorbell"));
        completed.observation.doorbell_slice_bytes = bytes;
        completed.observation.doorbell_byte_offset = offset;
        parent.check_currentness()?;
        let destination =
            check_auxiliary_compute_lane_slot_v1(&mut parent.auxiliary_compute_lanes, slot)?;
        Platform::finish_creation(self.creation_arm.as_mut().expect("creation arm"), pid)?;
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
    });
    let scope = settle_auxiliary_construction_with_v1(
        scope,
        ComputeAqlQueueSessionV1::check_currentness,
        |scope, entry| {
            let root = &mut scope.construction;
            let (result, retake) = scope.parent.with_live_queue_memory_model_custody(|memory| {
                let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
                capture_returned_preparation_v1(memory, &mut root.data, prepare_data)?;
                root.preparation = Some(FixedDispatchPreparationCustodyV1::new(
                    root.packets.take().expect("fixed packets"), root.data.take().expect("returned data"),
                ));
                let preparation = root.preparation.as_mut().expect("preparation custody");
                super::super::dispatch_binding::prepare_public_fixed_dispatch_resources_in_place(memory, &programs, preparation)?;
                root.dispatch = Some(preparation.take_completed()?);
                root.prepare(memory, entry, geometry, ring_bytes)
            })?;
            retake?;
            result?;
            root.create_and_install(scope.parent, ring_bytes, slot)
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
            scope.construction.terminal_parent =
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
