//! One retained primary constructor owner, from admitted memory to final queue.

use super::*;
use crate::queue_linux::{
    ProcessGlobalKfdRuntimeCreationArmV1, arm_process_global_kfd_runtime_gate_for_creation_v1,
};
use crate::shared_memory::{GttExecutableImmutableV1, GttProfileV1, SharedGttAllocationIdentityV1};

#[cfg(test)]
#[path = "construction_primary/tests.rs"]
mod tests;

type Cpu<P> = SharedGttAllocationV1<P, GttCpuWritableV1>;
type Mapped<P> = SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>;
type Sealed = SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>;
type Executable = SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>;

pub(super) fn capture_returned_preparation_v1<M, T>(
    memory: &mut M,
    output: &mut Option<T>,
    prepare: impl FnOnce(&mut M) -> Result<T, ComputeAqlQueueSessionErrorV1>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if output.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "preparation output occupied",
        ));
    }
    *output = Some(prepare(memory)?);
    Ok(())
}

struct MutablePrefixV1<P: GttProfileV1, R> {
    cpu: Option<Cpu<P>>,
    mapped: Option<Mapped<P>>,
    retained: Option<R>,
    in_session: Option<SharedGttAllocationIdentityV1>,
}

impl<P: GttProfileV1, R> MutablePrefixV1<P, R> {
    fn new() -> Self {
        Self {
            cpu: None,
            mapped: None,
            retained: None,
            in_session: None,
        }
    }
}

struct ExecutablePrefixV1<R> {
    cpu: Option<Cpu<ExecutableGttV1>>,
    sealed: Option<Sealed>,
    mapped: Option<Executable>,
    retained: Option<R>,
    in_session: Option<SharedGttAllocationIdentityV1>,
}

impl<R> ExecutablePrefixV1<R> {
    fn new() -> Self {
        Self {
            cpu: None,
            sealed: None,
            mapped: None,
            retained: None,
            in_session: None,
        }
    }
}

// Preflight is borrowed. Once admitted, R88 owns the consumed input on failure.
fn advance_token_v1<M, I, O>(
    memory: &mut M,
    input: &mut Option<I>,
    output: &mut Option<O>,
    in_session: &mut Option<SharedGttAllocationIdentityV1>,
    preflight: impl FnOnce(&M, &I) -> Result<SharedGttAllocationIdentityV1, MemorySessionError>,
    advance: impl FnOnce(&mut M, I) -> Result<O, MemorySessionError>,
) -> Result<(), MemorySessionError> {
    if output.is_some() || in_session.is_some() {
        return Err(MemorySessionError::Model(
            "queue construction destination occupied",
        ));
    }
    let identity = preflight(
        memory,
        input.as_ref().ok_or(MemorySessionError::Model(
            "queue construction input missing",
        ))?,
    )?;
    *in_session = Some(identity);
    *output = Some(advance(
        memory,
        input.take().expect("preflighted construction input"),
    )?);
    *in_session = None;
    Ok(())
}

pub(super) struct PrimaryQueueConstructionV1<P> {
    pub(super) memory: Option<SharedGttMemorySessionV1>,
    pub(super) preparation: P,
    pub(super) dispatch: Option<DispatchResourceOwnerV1>,
    pub(super) completed: Option<ComputeAqlQueueSessionV1>,
    ring: Option<RingConstructionV1>,
    control: MutablePrefixV1<UserptrAqlControlGttV1, ControlAuthority>,
    completion: MutablePrefixV1<HostVisibleCoherentGttV1, CompletionSignalAuthority>,
    eop: ExecutablePrefixV1<EopAuthority>,
    context: ExecutablePrefixV1<ContextSaveAuthority>,
    runtime: Option<LinuxKfdRuntimeEnabledV1>,
    runtime_control: Option<KfdWithAdmittedUapi>,
    event: Option<LinuxQueueExceptionEventV1>,
    unpublished: Option<LinuxUnpublishedCwsrShadowPagesV1>,
    published: Option<LinuxCwsrShadowPagesV1>,
    resource_prefix: Option<QueueResourcePrefixV1>,
    authority: Option<QueueResourceAuthorityV1>,
    completion_owner: Option<CompletionSignalArenaOwnerV1>,
    submission: Option<NativeAqlSubmissionOwnerV1>,
    dependency_owner: Option<ComputeDependencySessionOwnerV1>,
    foundation: Option<QueueModelFoundationV1>,
    initialization: Option<NativeQueueEngineInitializationV1<LinuxNativeQueueBackendV1>>,
    engine: Option<NativeQueueEngineV1<LinuxNativeQueueBackendV1>>,
    outputs: Option<fe2o3_kfd_uapi::KfdGfx942CreateQueueOutputs>,
    creation_arm: Option<ProcessGlobalKfdRuntimeCreationArmV1>,
}

pub(super) struct UserptrConstructionEntryV1<'a> {
    stage: Option<&'static str>,
    poison: &'a dyn Fn(),
}

impl UserptrConstructionEntryV1<'_> {
    fn enter(&mut self, stage: &'static str) {
        self.stage.get_or_insert(stage);
    }

    fn poison_now(&mut self) {
        if self.stage.take().is_some() {
            (self.poison)();
        }
    }
}

impl Drop for UserptrConstructionEntryV1<'_> {
    fn drop(&mut self) {
        self.poison_now();
    }
}

impl<P> PrimaryQueueConstructionV1<P> {
    pub(super) fn new(memory: SharedGttMemorySessionV1, preparation: P) -> Box<Self> {
        Box::new(Self {
            memory: Some(memory),
            preparation,
            dispatch: None,
            completed: None,
            ring: None,
            control: MutablePrefixV1::new(),
            completion: MutablePrefixV1::new(),
            eop: ExecutablePrefixV1::new(),
            context: ExecutablePrefixV1::new(),
            runtime: None,
            runtime_control: None,
            event: None,
            unpublished: None,
            published: None,
            resource_prefix: None,
            authority: None,
            completion_owner: None,
            submission: None,
            dependency_owner: None,
            foundation: None,
            initialization: None,
            engine: None,
            outputs: None,
            creation_arm: None,
        })
    }

    pub(super) fn run(
        self: Box<Self>,
        work: impl FnOnce(
            &mut Self,
            &mut UserptrConstructionEntryV1,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> Result<Box<Self>, ComputeAqlQueueSessionErrorV1> {
        run_rooted_construction_v1(self, work, |root| {
            if let Some(unpublished) = root.unpublished.as_mut() {
                unpublished.cleanup_payload_for_terminal_retention();
            }
        })
    }

    pub(super) fn construct(
        &mut self,
        entry: &mut UserptrConstructionEntryV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        ring_bytes: u32,
        backing: QueueRingBackingV1,
        external_runtime: Option<ExternalRuntimeV1<'_>>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.ring.is_some() || self.completed.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "queue construction already started",
            ));
        }
        let memory = self.memory.as_mut().expect("initial construction memory");
        let bytes = usize::try_from(ring_bytes)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring size conversion"))?;
        if matches!(backing, QueueRingBackingV1::UserptrProbe) {
            entry.enter("USERPTR queue-ring creation");
        }
        self.ring = Some(RingConstructionV1::Cpu(CpuRingAuthorityV1::allocate(
            memory, backing, bytes,
        )?));
        entry.enter("USERPTR queue-control creation");
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
        self.prepare_runtime_and_shadows(external_runtime)?;
        self.map_and_retain_resources(geometry)?;
        self.create_and_assemble(ring_bytes)?;
        self.finish_doorbell()?;
        let pid = self
            .completed
            .as_ref()
            .expect("completed queue")
            .engine
            .as_ref()
            .expect("completed engine")
            .opener_pid;
        self.creation_arm
            .as_mut()
            .expect("creation arm")
            .finish_checked(pid)?;
        Ok(())
    }

    fn prepare_runtime_and_shadows(
        &mut self,
        mut external_runtime: Option<ExternalRuntimeV1<'_>>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let memory = self.memory.as_mut().expect("construction memory");
        match external_runtime.as_mut() {
            Some((runtime, control)) => {
                let runtime = runtime
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing debug runtime authority",
                    ))?;
                let control = control
                    .as_ref()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "missing debug runtime control descriptor",
                    ))?;
                runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
            }
            None => {
                self.runtime = Some(
                    match LinuxKfdRuntimeEnabledV1::enable(memory.kfd_fd(), memory.opener_pid()) {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = memory.quarantine_queue_composition(
                                "RUNTIME_ENABLE enable ambiguous failure",
                            );
                            return Err(error.into());
                        }
                    },
                );
            }
        }
        if let Some((runtime, control)) = external_runtime.as_ref() {
            let runtime = runtime.as_ref().expect("validated debug runtime authority");
            let control = control
                .as_ref()
                .expect("validated debug runtime control descriptor");
            runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
        } else {
            self.runtime
                .as_ref()
                .expect("enabled queue runtime")
                .validate_active(memory.kfd_fd(), memory.opener_pid())?;
        }
        memory.check_queue_currentness()?;
        if let Some((runtime, control)) = external_runtime.as_mut() {
            self.runtime = Some(runtime.take().expect("validated debug runtime authority"));
            self.runtime_control = Some(
                control
                    .take()
                    .expect("validated debug runtime control descriptor"),
            );
        }
        // Admission must precede this arm. The external token is already empty.
        self.creation_arm = Some(arm_process_global_kfd_runtime_gate_for_creation_v1()?);
        self.event = Some(
            match LinuxQueueExceptionEventV1::create(memory.kfd_fd(), memory.opener_pid()) {
                Ok(event) => event,
                Err(error) => {
                    let _ = memory.quarantine_queue_composition("CREATE_EVENT ambiguous failure");
                    return Err(error.into());
                }
            },
        );
        memory.check_queue_currentness()?;
        let shadow_plan =
            memory.cwsr_shadow_plan(self.context.cpu.as_ref().expect("context-save"))?;
        self.unpublished = Some(
            match LinuxCwsrShadowPagesV1::install(shadow_plan, self.event.as_ref().expect("event"))
            {
                Ok(shadows) => shadows,
                Err(error) => {
                    let _ = memory.quarantine_queue_composition("CWSR shadow setup failure");
                    return Err(error.into());
                }
            },
        );
        let unpublished = self.unpublished.as_ref().expect("unpublished shadows");
        let initialization =
            match memory.with_bytes_mut(self.context.cpu.as_mut().expect("context-save"), |bytes| {
                unpublished
                    .shadows()
                    .initialize_and_validate_bo_headers(bytes)
            }) {
                Ok(initialization) => initialization,
                Err(error) => {
                    let _ = memory.quarantine_queue_composition("CWSR BO initialization failure");
                    return Err(error.into());
                }
            };
        if initialization.is_err() {
            let _ = memory.quarantine_queue_composition("CWSR header readback failure");
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "gfx942 CWSR header initialization",
            ));
        }
        let runtime = self.runtime.as_ref().expect("runtime");
        if let Some(control) = self.runtime_control.as_ref() {
            runtime.validate_active(control.opened.fd.as_fd(), control.opened.opener_pid)?;
        } else {
            runtime.validate_active(memory.kfd_fd(), memory.opener_pid())?;
        }
        self.event
            .as_ref()
            .expect("event")
            .validate_live_with_shadows(
                memory.kfd_fd(),
                memory.opener_pid(),
                unpublished.shadows(),
            )?;
        memory.check_queue_currentness()?;
        Ok(())
    }

    fn map_and_retain_resources(
        &mut self,
        geometry: Gfx942AqlQueueResourcePlanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let memory = self.memory.as_mut().expect("construction memory");
        self.eop.seal(memory)?;
        self.context.seal(memory)?;
        self.unpublished
            .as_ref()
            .expect("unpublished shadows")
            .shadows()
            .restore_kernel_write_access_after_bo_seal()?;
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
        // All four role slots were established above, without intervening callbacks.
        self.resource_prefix = Some(QueueResourcePrefixV1::new(
            ring.take_retained()?,
            self.control.retained.take().expect("retained control"),
            self.eop.retained.take().expect("retained EOP"),
            self.context.retained.take().expect("retained context-save"),
        ));
        let prefix = self.resource_prefix.as_mut().expect("resource prefix");
        prefix.build_in_place(memory.queue_model_device(), geometry)?;
        self.authority = Some(prefix.take_complete()?);
        Ok(())
    }

    fn create_and_assemble(
        &mut self,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let memory = self.memory.as_mut().expect("construction memory");
        self.completion_owner = Some(CompletionSignalArenaOwnerV1::new(
            self.authority
                .as_ref()
                .expect("completed resources")
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
        self.foundation = Some(match self.dispatch.as_ref() {
            Some(dispatch) => memory.take_queue_model_foundation_with_dispatch_memory(
                &dispatch.device_authorities_inline_v1(),
            )?,
            None => memory.take_queue_model_foundation()?,
        });
        self.initialization = Some(NativeQueueEngineInitializationV1::new(
            LinuxNativeQueueBackendV1 {
                session: self.memory.take().expect("transferred memory"),
                foundation: self.foundation.take(),
                foundation_in_engine: false,
            },
        ));
        self.engine = Some(
            self.initialization
                .as_mut()
                .expect("engine initialization")
                .initialize()
                .map_err(map_native)?,
        );
        let engine = self.engine.as_mut().expect("initialized engine");
        let key = match engine.admit_in_place(&mut self.authority) {
            Ok(key) => key,
            Err(error @ NativeQueueAdapterErrorV1::AuthorityPoisoned) => {
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(terminal_creation(
                    "queue model admission",
                    map_native(error),
                ));
            }
            Err(error) => return Err(map_native(error)),
        };
        engine
            .create_at_native_boundary(key, || {
                self.published = Some(
                    self.unpublished
                        .take()
                        .expect("unpublished shadow owner")
                        .publish_for_native_queue_creation(),
                );
            })
            .map_err(map_create)?;
        self.runtime
            .as_mut()
            .expect("runtime")
            .mark_queue_created()
            .map_err(|error| terminal_creation("runtime queue-live transition", error.into()))?;
        self.outputs = Some(engine.create_outputs(key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE output recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing CREATE outputs"),
            )
        })?);
        let queue_id = engine.native_queue_id(key).ok_or_else(|| {
            terminal_creation(
                "CREATE_QUEUE identity recovery",
                ComputeAqlQueueSessionErrorV1::Contract("missing queue id"),
            )
        })?;
        self.dependency_owner =
            Some(ComputeDependencySessionOwnerV1::new(key.id.0).map_err(|_| {
                terminal_creation(
                    "compute dependency session owner",
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "invalid compute dependency session occurrence",
                    ),
                )
            })?);
        let cwsr_shadow_pages = u8::try_from(crate::queue_linux::GFX942_CWSR_SHADOW_PAGES_V1)
            .map_err(|_| {
                terminal_creation(
                    "CWSR shadow page count",
                    ComputeAqlQueueSessionErrorV1::Contract("CWSR shadow page count"),
                )
            })?;
        let event_id = self.event.as_ref().expect("event").event_id_observation();
        // Every fallible assembly value is rooted or computed before extracting owners.
        self.completed = Some(ComputeAqlQueueSessionV1 {
            engine: self.engine.take(),
            key,
            compute_lane_session: key,
            doorbell: None,
            submission: self.submission.take(),
            completion_signals: self.completion.retained.take(),
            completion_owner: self.completion_owner.take().expect("completion owner"),
            dependency_owner: self.dependency_owner.take().expect("dependency owner"),
            terminal_dependency: None,
            dispatch: self.dispatch.take(),
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
                runtime: self.runtime.take().expect("runtime"),
                runtime_control: self.runtime_control.take(),
                event: self.event.take().expect("event"),
                shadows: self.published.take().expect("published shadows"),
            }),
            sdma: None,
            striped_sdma: None,
            sdma_outstanding_buffers: 0,
            sdma_pool_free: Vec::new(),
            sdma_pool_reuse_count: 0,
            sdma_device_pool: SdmaDevicePoolConfigurationV1::default(),
            sdma_host_pool_limits: None,
            terminal_poisoned: false,
            observation: ComputeAqlQueueObservationV1 {
                queue_id,
                ring_bytes,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
                event_id,
                cwsr_shadow_pages,
            },
            auxiliary_compute_lanes: Vec::new(),
        });
        Ok(())
    }

    fn finish_doorbell(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let session = self.completed.as_mut().expect("completed session custody");
        let outputs = self.outputs.expect("retained CREATE outputs");
        session
            .check_currentness()
            .map_err(|error| terminal_creation("post-create currentness before doorbell", error))?;
        let engine = session.engine.as_ref().expect("session engine");
        session.doorbell = Some(
            LinuxDoorbellSliceV1::map(engine.backend.session.kfd_fd(), outputs, engine.opener_pid)
                .map_err(|error| terminal_creation("doorbell mapping", error.into()))?,
        );
        let doorbell = session.doorbell.as_ref().expect("rooted doorbell");
        session.observation.doorbell_slice_bytes = doorbell.slice_bytes();
        session.observation.doorbell_byte_offset = doorbell.queue_byte_offset();
        session
            .check_currentness()
            .map_err(|error| terminal_creation("post-doorbell currentness", error))?;
        Ok(())
    }
}

fn run_rooted_construction_v1<T>(
    root: Box<T>,
    work: impl FnOnce(
        &mut T,
        &mut UserptrConstructionEntryV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    cleanup: impl FnOnce(&mut T),
) -> Result<Box<T>, ComputeAqlQueueSessionErrorV1> {
    run_rooted_construction_with_v1(
        root,
        work,
        cleanup,
        &permanently_poison_process_global_kfd_runtime_gate_v1,
        |root| {
            let _retained = Box::into_raw(root);
        },
    )
}

fn run_rooted_construction_with_v1<T>(
    mut root: Box<T>,
    work: impl FnOnce(
        &mut T,
        &mut UserptrConstructionEntryV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    cleanup: impl FnOnce(&mut T),
    poison: &dyn Fn(),
    retain: impl FnOnce(Box<T>),
) -> Result<Box<T>, ComputeAqlQueueSessionErrorV1> {
    let mut entry = UserptrConstructionEntryV1 {
        stage: None,
        poison,
    };
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&mut root, &mut entry)));
    if matches!(result, Ok(Ok(()))) {
        entry.stage = None;
        return Ok(root);
    }
    let stage = entry.stage;
    entry.poison_now();
    let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup(&mut root)));
    // No public error carries native authority; the original allocation stays retained.
    retain(root);
    if let Err(payload) = cleanup {
        if let Err(original) = result {
            // A secondary payload may itself panic on Drop.
            core::mem::forget(payload);
            std::panic::resume_unwind(original);
        }
        std::panic::resume_unwind(payload);
    }
    match result {
        Ok(Err(error)) => Err(match stage {
            Some(stage) if !error.is_terminal_creation() => terminal_creation(stage, error),
            _ => error,
        }),
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(Ok(())) => unreachable!("handled successful construction"),
    }
}

#[cfg(test)]
pub(in crate::queue) fn settle_queue_constructor_fixture_v1<T>(
    root: Box<T>,
    work: impl FnOnce(&mut T) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    retain: impl FnOnce(Box<T>),
) -> Result<Box<T>, ComputeAqlQueueSessionErrorV1> {
    run_rooted_construction_with_v1(
        root,
        |root, _| work(root),
        |_| {},
        &|| panic!("pre-control fixture poisoned the gate"),
        retain,
    )
}

#[cfg(test)]
pub(super) fn terminal_control_failure_for_test(
    error: ComputeAqlQueueSessionErrorV1,
) -> ComputeAqlQueueSessionErrorV1 {
    run_rooted_construction_v1(
        Box::new(()),
        |_, entry| {
            entry.enter("USERPTR queue-control creation");
            Err(error)
        },
        |_| {},
    )
    .unwrap_err()
}

fn map_mutable_prefix<P: crate::shared_memory::MutableGpuGttProfileV1, R>(
    memory: &mut SharedGttMemorySessionV1,
    prefix: &mut MutablePrefixV1<P, R>,
) -> Result<(), MemorySessionError> {
    advance_token_v1(
        memory,
        &mut prefix.cpu,
        &mut prefix.mapped,
        &mut prefix.in_session,
        |memory, token| {
            memory.preflight_cpu_queue_token_v1(token)?;
            Ok(token.storage_identity())
        },
        SharedGttMemorySessionV1::map_to_gpu,
    )
}

fn retain_mutable_prefix<P: crate::shared_memory::MutableGpuGttProfileV1, R>(
    memory: &mut SharedGttMemorySessionV1,
    prefix: &mut MutablePrefixV1<P, R>,
    retain: impl FnOnce(&mut SharedGttMemorySessionV1, Mapped<P>) -> Result<R, MemorySessionError>,
) -> Result<(), MemorySessionError> {
    advance_token_v1(
        memory,
        &mut prefix.mapped,
        &mut prefix.retained,
        &mut prefix.in_session,
        |memory, token| {
            memory.preflight_mapped_queue_token_v1(token)?;
            Ok(token.storage_identity())
        },
        retain,
    )
}

impl<R> ExecutablePrefixV1<R> {
    fn seal(&mut self, memory: &mut SharedGttMemorySessionV1) -> Result<(), MemorySessionError> {
        advance_token_v1(
            memory,
            &mut self.cpu,
            &mut self.sealed,
            &mut self.in_session,
            |memory, token| {
                memory.preflight_cpu_queue_token_v1(token)?;
                Ok(token.storage_identity())
            },
            SharedGttMemorySessionV1::seal_executable,
        )
    }
}

fn map_executable_prefix<R>(
    memory: &mut SharedGttMemorySessionV1,
    prefix: &mut ExecutablePrefixV1<R>,
) -> Result<(), MemorySessionError> {
    advance_token_v1(
        memory,
        &mut prefix.sealed,
        &mut prefix.mapped,
        &mut prefix.in_session,
        |memory, token| {
            memory.preflight_immutable_queue_token_v1(token)?;
            Ok(token.storage_identity())
        },
        SharedGttMemorySessionV1::map_executable_to_gpu,
    )
}

fn retain_executable_prefix<R>(
    memory: &mut SharedGttMemorySessionV1,
    prefix: &mut ExecutablePrefixV1<R>,
    retain: impl FnOnce(&mut SharedGttMemorySessionV1, Executable) -> Result<R, MemorySessionError>,
) -> Result<(), MemorySessionError> {
    advance_token_v1(
        memory,
        &mut prefix.mapped,
        &mut prefix.retained,
        &mut prefix.in_session,
        |memory, token| {
            memory.preflight_executable_queue_token_v1(token)?;
            Ok(token.storage_identity())
        },
        retain,
    )
}
