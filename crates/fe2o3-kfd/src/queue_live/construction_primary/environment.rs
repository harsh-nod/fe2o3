//! Platform operations only; ordering and retained transfers stay in the constructor.

use super::*;

pub(in crate::queue::live) trait PrimaryEnvironmentV1 {
    type Memory: PrimaryMemoryV1;
    type Runtime;
    type RuntimeControl;
    type Event;
    type Unpublished;
    type Published;
    type Doorbell;
    type CreationArm;

    fn enable_runtime(
        memory: &mut Self::Memory,
    ) -> Result<Self::Runtime, ComputeAqlQueueSessionErrorV1>;
    fn validate_runtime(
        runtime: &Self::Runtime,
        control: Option<&Self::RuntimeControl>,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn arm_creation(
        memory: &Self::Memory,
    ) -> Result<Self::CreationArm, ComputeAqlQueueSessionErrorV1>;
    fn create_event(
        memory: &mut Self::Memory,
    ) -> Result<Self::Event, ComputeAqlQueueSessionErrorV1>;
    fn install_shadows(
        memory: &mut Self::Memory,
        context: &Cpu<ExecutableGttV1>,
        event: &Self::Event,
    ) -> Result<Self::Unpublished, ComputeAqlQueueSessionErrorV1>;
    fn initialize_shadows(
        memory: &mut Self::Memory,
        context: &mut Cpu<ExecutableGttV1>,
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn validate_event(
        memory: &Self::Memory,
        event: &Self::Event,
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn restore_shadow_write(
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    // These moves are nonfallible after the shared constructor's phase preflight.
    fn publish_shadows(shadows: Self::Unpublished) -> Self::Published;
    fn cleanup_unpublished(shadows: &mut Self::Unpublished);
    fn mark_queue_created(runtime: &mut Self::Runtime)
    -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn recover_create_outputs(
        engine: &NativeQueueEngineV1<PrimaryQueueBackendV1<Self::Memory>>,
        key: QueueKeyV1,
    ) -> Option<fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs> {
        engine.create_outputs(key)
    }
    fn recover_native_queue_id(
        engine: &NativeQueueEngineV1<PrimaryQueueBackendV1<Self::Memory>>,
        key: QueueKeyV1,
    ) -> Option<u32> {
        engine.native_queue_id(key)
    }
    fn create_dependency_owner(
        key: QueueKeyV1,
    ) -> Result<ComputeDependencySessionOwnerV1, ComputeDependencyTargetUseErrorV1> {
        ComputeDependencySessionOwnerV1::new(key.id.0)
    }
    fn event_id(event: &Self::Event) -> u32;
    fn map_doorbell(
        memory: &Self::Memory,
        outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
        pid: u32,
    ) -> Result<Self::Doorbell, ComputeAqlQueueSessionErrorV1>;
    fn doorbell_observation(doorbell: &Self::Doorbell) -> (usize, u64);
    fn finish_creation(
        arm: &mut Self::CreationArm,
        pid: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison();
}

pub(in crate::queue::live) struct LinuxPrimaryEnvironmentV1;

impl PrimaryEnvironmentV1 for LinuxPrimaryEnvironmentV1 {
    type Memory = SharedGttMemorySessionV1;
    type Runtime = LinuxKfdRuntimeEnabledV1;
    type RuntimeControl = KfdWithAdmittedUapi;
    type Event = LinuxQueueExceptionEventV1;
    type Unpublished = LinuxUnpublishedCwsrShadowPagesV1;
    type Published = LinuxCwsrShadowPagesV1;
    type Doorbell = LinuxDoorbellSliceV1;
    type CreationArm = ProcessGlobalKfdRuntimeCreationArmV1;

    fn enable_runtime(
        memory: &mut Self::Memory,
    ) -> Result<Self::Runtime, ComputeAqlQueueSessionErrorV1> {
        LinuxKfdRuntimeEnabledV1::enable(memory.kfd_fd(), memory.opener_pid()).map_err(|error| {
            let _ = memory.quarantine_queue_composition("RUNTIME_ENABLE enable ambiguous failure");
            error.into()
        })
    }
    fn validate_runtime(
        runtime: &Self::Runtime,
        control: Option<&Self::RuntimeControl>,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match control {
            Some(control) => {
                runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?
            }
            None => runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?,
        }
        Ok(())
    }
    fn arm_creation(_: &Self::Memory) -> Result<Self::CreationArm, ComputeAqlQueueSessionErrorV1> {
        Ok(arm_process_global_kfd_runtime_gate_for_creation_v1()?)
    }
    fn create_event(
        memory: &mut Self::Memory,
    ) -> Result<Self::Event, ComputeAqlQueueSessionErrorV1> {
        LinuxQueueExceptionEventV1::create(memory.kfd_fd(), memory.opener_pid()).map_err(|error| {
            let _ = memory.quarantine_queue_composition("CREATE_EVENT ambiguous failure");
            error.into()
        })
    }
    fn install_shadows(
        memory: &mut Self::Memory,
        context: &Cpu<ExecutableGttV1>,
        event: &Self::Event,
    ) -> Result<Self::Unpublished, ComputeAqlQueueSessionErrorV1> {
        let plan = memory.cwsr_shadow_plan(context)?;
        LinuxCwsrShadowPagesV1::install(plan, event).map_err(|error| {
            let _ = memory.quarantine_queue_composition("CWSR shadow setup failure");
            error.into()
        })
    }
    fn initialize_shadows(
        memory: &mut Self::Memory,
        context: &mut Cpu<ExecutableGttV1>,
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let initialization = memory
            .with_bytes_mut(context, |bytes| {
                shadows.shadows().initialize_and_validate_bo_headers(bytes)
            })
            .inspect_err(|_| {
                let _ = memory.quarantine_queue_composition("CWSR BO initialization failure");
            })?;
        if initialization.is_err() {
            let _ = memory.quarantine_queue_composition("CWSR header readback failure");
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "gfx942 CWSR header initialization",
            ));
        }
        Ok(())
    }
    fn validate_event(
        memory: &Self::Memory,
        event: &Self::Event,
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        event.validate_live_with_shadows(
            memory.kfd_fd(),
            memory.opener_pid(),
            shadows.shadows(),
        )?;
        Ok(())
    }
    fn restore_shadow_write(
        shadows: &Self::Unpublished,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        shadows
            .shadows()
            .restore_kernel_write_access_after_bo_seal()?;
        Ok(())
    }
    fn publish_shadows(shadows: Self::Unpublished) -> Self::Published {
        shadows.publish_for_native_queue_creation()
    }
    fn cleanup_unpublished(shadows: &mut Self::Unpublished) {
        shadows.cleanup_payload_for_terminal_retention();
    }
    fn mark_queue_created(
        runtime: &mut Self::Runtime,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        runtime
            .mark_queue_created()
            .map_err(|error| terminal_creation("runtime queue-live transition", error.into()))
    }
    fn event_id(event: &Self::Event) -> u32 {
        event.event_id_observation()
    }
    fn map_doorbell(
        memory: &Self::Memory,
        outputs: fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs,
        pid: u32,
    ) -> Result<Self::Doorbell, ComputeAqlQueueSessionErrorV1> {
        LinuxDoorbellSliceV1::map(memory.kfd_fd(), outputs, pid)
            .map_err(|error| terminal_creation("doorbell mapping", error.into()))
    }
    fn doorbell_observation(doorbell: &Self::Doorbell) -> (usize, u64) {
        (doorbell.slice_bytes(), doorbell.queue_byte_offset())
    }
    fn finish_creation(
        arm: &mut Self::CreationArm,
        pid: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(arm.finish_checked(pid)?)
    }
    fn poison() {
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

pub(in crate::queue::live) struct CompletedPrimaryV1<E: PrimaryEnvironmentV1> {
    pub(super) engine: NativeQueueEngineV1<PrimaryQueueBackendV1<E::Memory>>,
    pub(super) key: QueueKeyV1,
    pub(super) submission: NativeAqlSubmissionOwnerV1,
    pub(super) completion_signals: CompletionSignalAuthority,
    pub(super) completion_owner: CompletionSignalArenaOwnerV1,
    pub(super) dependency_owner: ComputeDependencySessionOwnerV1,
    pub(super) dispatch: Option<DispatchResourceOwnerV1>,
    pub(super) runtime: E::Runtime,
    pub(super) runtime_control: Option<E::RuntimeControl>,
    pub(super) event: E::Event,
    pub(super) shadows: E::Published,
    pub(super) doorbell: Option<E::Doorbell>,
    pub(super) observation: ComputeAqlQueueObservationV1,
}

impl CompletedPrimaryV1<LinuxPrimaryEnvironmentV1> {
    pub(in crate::queue::live) fn into_session(self) -> ComputeAqlQueueSessionV1 {
        // Finalization has succeeded. Only nonallocating field moves follow.
        ComputeAqlQueueSessionV1 {
            engine: Some(self.engine),
            key: self.key,
            compute_lane_session: self.key,
            doorbell: self.doorbell,
            submission: Some(self.submission),
            completion_signals: Some(self.completion_signals),
            completion_owner: QueueOwnerSlotV1(Some(self.completion_owner)),
            dependency_owner: QueueOwnerSlotV1(Some(self.dependency_owner)),
            terminal_dependency: None,
            dispatch: self.dispatch,
            unpublished_dispatch: UnpublishedDispatchStateV1::default(),
            detached_data_count: 0,
            detached_dispatch_generation: None,
            detached_data_identities: Vec::new(),
            detached_next_insertion_index: None,
            persistent_compute: None,
            #[cfg(test)]
            persistent_compute_test_release: None,
            next_persistent_compute_generation: 1,
            exception: Some(QueueExceptionStateV1 {
                runtime: self.runtime,
                runtime_control: self.runtime_control,
                event: self.event,
                shadows: self.shadows,
            }),
            sdma: None,
            striped_sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
            sdma_device_pool: SdmaDevicePoolConfigurationV1::default(),
            sdma_host_pool_limits: None,
            terminal_poisoned: false,
            observation: self.observation,
            auxiliary_compute_lanes: Vec::new(),
        }
    }
}
