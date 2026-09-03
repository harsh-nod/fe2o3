//! Fail-closed native compute-AQL queue adapter foundation.
//!
//! The executable engine is crate-private because the current memory adapter
//! cannot mint its backend-specific resource authority. Scripted tests execute
//! every transition. A future Linux backend must own the retained checked
//! device and exact mapped resource capabilities before this can become a
//! public queue API.

use core::fmt;

use fe2o3_kfd_uapi::{
    KfdAqlComputeQueueBuffers, KfdAqlQueueRingSize, KfdGfx942CreateQueueOutputs,
    KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs, KfdIoctlUpdateQueueArgs, KfdQueuePercentage,
    KfdQueuePriority, admit_kfd_aql_queue_ring_address, admit_kfd_gfx942_create_queue_outputs,
};
use fe2o3_runtime_model::{
    CREATE_QUEUE_ID_SENTINEL_V1, ComputeAqlQueuePhaseV1, ComputeAqlQueuePlanV1,
    CreateQueueIdFieldObservationV1, DeviceIdentityStateV1, MAX_QUEUE_HISTORY_ENTRIES_V1,
    MemoryLifecycleStateV1, QueueConfigurationIdV1, QueueCreateObservationV1, QueueKeyV1,
    QueueLifecycleStateV1, QueueSyscallStatusV1, QueueTransitionErrorV1, QueueTransitionV1,
    UntrustedQueueIdObservationV1,
};

#[path = "queue_live.rs"]
mod live;

#[allow(unsafe_code)]
#[path = "queue_submit.rs"]
pub(crate) mod submit;

#[allow(unsafe_code)]
#[path = "queue_completion.rs"]
pub(crate) mod completion;

#[path = "queue_dispatch_binding.rs"]
pub(crate) mod dispatch_binding;

#[path = "queue_device_content.rs"]
pub(crate) mod device_content;

pub use completion::{
    GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1, GFX942_AQL_COMPLETION_MANIFEST_V1,
    Gfx942CompletedBatchV1, Gfx942CompletionBatchV1, Gfx942CompletionErrorV1,
    Gfx942CompletionPollV1, Gfx942CompletionPollWithProgressV1, Gfx942CompletionProgressV1,
    Gfx942CompletionRecycleObservationV1, Gfx942TimeoutExecutionObservationV1,
    Gfx942TimeoutSignalObservationV1,
};

pub use dispatch_binding::{
    GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1, GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1,
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, GFX942_MAX_FIXED_DISPATCH_PACKETS_V1,
    GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1, Gfx942CompletedDispatchBatchV1,
    Gfx942CompletedDispatchReadRequestV1, Gfx942CompletedDispatchReadbackV1,
    Gfx942CompletedDispatchSnapshotRequestV1, Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1,
    Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1, Gfx942DispatchPollWithProgressV1,
    Gfx942DispatchProgressV1, Gfx942FixedDispatchDataKindV1, Gfx942FixedDispatchDataLayoutV1,
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1, Gfx942RecycledDispatchWriteRequestV1,
    preflight_gfx942_fixed_dispatch_replacement,
};

pub use device_content::{
    GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_SHA256_V1,
    GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_V1, Gfx942DeviceContentDescriptorErrorV1,
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, Gfx942RepeatedByteContentV1,
};

pub use live::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueObservationV1, ComputeAqlQueueSessionErrorV1,
    ComputeAqlQueueSessionV1, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1,
    GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1, GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_V1, Gfx942BarrierProbeExecutionObservationV1,
    Gfx942BarrierProbeFailureV1, Gfx942BarrierProbePollBoundErrorV1, Gfx942BarrierProbePollBoundV1,
    Gfx942BarrierProbeRingBackingV1, Gfx942BarrierProbeSuccessV1, Gfx942DetachedFixedDispatchV1,
    Gfx942KfdDebugTargetDispatchErrorV2, Gfx942KfdDebugTargetDispatchResultV2,
    Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchErrorV1, Gfx942KfdDispatchPointerFixupV1,
    Gfx942KfdDispatchRequestErrorV1, Gfx942KfdDispatchRequestV1, Gfx942KfdDispatchResultV1,
    Gfx942KfdQueueExceptionObservationV1, Gfx942RecycledDispatchResourcesV1,
    KfdTargetRuntimeDebugQueueTeardownV1, KfdTargetRuntimeDebugQueueV1,
    QuarantinedGfx942BarrierProbeV1, execute_gfx942_kfd_debug_target_dispatch_unchecked_v1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v2,
    execute_gfx942_kfd_dispatch_unchecked_v1,
};

/// Canonical claim boundary for the executable native-queue foundation.
pub const NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-native-queue-adapter-foundation-r22-v1\n",
    "compute_session_sha256=ab5d1fc0b2fcdc4f3918d772f03156b14cb524da7c7660c39f29b539e6186289\n",
    "operations=create,update,disable,destroy\n",
    "projection=existing-bounded-queue-lifecycle-model,pending-before-ioctl,append-only-history\n",
    "resources=backend-specific-private-capability,linearly-retained,exact-ring-control-eop-cwsr-mappings-required\n",
    "currentness=opener-pid-and-contracted-device-check-before-and-after-every-lifecycle-ioctl\n",
    "failure=linux-errno-must-map-indeterminate,malformed-output-global-poison,post-call-projection-failure-global-poison\n",
    "release=explicit-only-after-confirmed-destroy,no-drop-ioctl\n",
    "linux-boundary=private-create-update-destroy-ioctl-shims,production-create-destroy-composition\n",
    "composition=shared-memory-linear-role-authorities,exact-one-page-same-va-userptr-writable-coherent-control,exact-set-device-memory-dispatch-transfer,transferred-model-foundation,live-allocation-lifecycle-mutation-foundation-loan-and-reclaim,whole-slice-doorbell-mmap\n",
    "creation=every-error-from-userptr-control-allocation-attempt-through-live-session-return-recovers-no-authority-permanently-poisons-process-global-runtime-gate-and-requires-process-termination\n",
    "submission=crate-private-single-producer-aql-fixed-batch-v2-through-8192,ring-capacity-checked,one-actual-write-counter-fetch-add-by-count,all-invalid-bodies-before-release-headers,one-final-doorbell-store\n",
    "completion=separate-linear-8192-signal-host-coherent-arena,heap-owned-fixed-cardinality-retention,unique-signal-per-packet,crate-private-generation-binding,bounded-acquire-poll,addressless-timeout-execution-observation-before-terminal-poison,release-reset-after-exact-batch-completion\n",
    "barrier-probe=three-consuming-fresh-queue-entries,gfx942-production-executable-one-span-or-plain-executable-one-span-or-selected-gpu-userptr-final-rocr-derived-flags-one-span-ring-with-no-full-rocr-order-parity,typed-poll-bound-before-device-consumption,zero-dependency-system-scope-packet,isolated-one-signal-lease,no-code-kernarg-or-dispatch-generation,success-after-completion-reset-and-confirmed-destroy-release-only,every-error-at-or-after-userptr-control-registration-entry-permanently-poisons-process-global-runtime-gate-and-is-terminal,execution-failure-opaque-quarantine-until-process-teardown,process-global-runtime-gate-poison-armed-before-destroy-and-cleared-only-after-confirmed-success,terminal-teardown-or-panic-retains-permanent-gate-poison-recovers-no-authority-native-resource-disposition-indeterminate-process-termination-required-no-retry-reopen-or-confirmed-cleanup\n",
    "dispatch-binding=public-addressless-inspected-code-zero-pointer-and-caller-zero-implicit-kernarg-private-substitution,mapped-device-lease-fixed-batch-completion-generation-composition,metadata-derived-COV6-geometry-and-dynamic-lds-only,queue-pointer-and-runtime-address-fields-rejected,real-resource-retention-through-recycle,recycled-only-detach-and-rebind-on-one-live-queue,actual-mapped-authority-return-only-after-exact-recycle\n",
    "dispatch-generation=rebind-is-seeded-from-exact-detached-predecessor-and-strictly-advances-before-publication\n",
    "missing=kernel-dispatch-hardware-completion-and-exception-refinement,live-kernel-batch-evidence,kernel-memory-effect-refinement,kernel-numerical-correctness,machine-proof\n",
    "proof=model-projection-and-hostile-tests-only,cpu-gpu-atomic-coherence-and-mmio-refinement-contracted\n",
    "authority=redacted-live-session,queue-id-observation-only,no-fd-gpu-address-mmio-pointer-or-dispatch-export\n",
);

/// SHA-256 of [`NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1`].
pub const NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_SHA256_V1: &str =
    "275ca7f6b5c78542b1ed82fdd62f43e59708ef01c057d94f9369c2bf4f9d411c";

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeQueueOperationV1 {
    Create,
    Update,
    Disable,
    Destroy,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeQueueJournalSummaryV1 {
    queues: usize,
    history: usize,
    live_publications: usize,
    ambiguous: usize,
    authority_poisoned: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum NativeQueueAdapterErrorV1 {
    ProcessChanged,
    Currentness(&'static str),
    InvalidResource(&'static str),
    InvalidPhase,
    JournalCapacity,
    BackendFailedNoEffect(NativeQueueOperationV1),
    BackendIndeterminate(NativeQueueOperationV1),
    MalformedKernelResult(NativeQueueOperationV1, &'static str),
    ModelProjection,
    AuthorityPoisoned,
}

impl fmt::Display for NativeQueueAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeQueueAdapterErrorV1 {}

/// Model foundation supplied by the backend that owns the checked device/VM.
/// These model values are not concrete authority; the private backend resource
/// type is what prevents callers from presenting numeric addresses directly.
struct QueueModelFoundationV1 {
    identity: DeviceIdentityStateV1,
    memory: MemoryLifecycleStateV1,
}

#[derive(Clone, Copy)]
struct NativeQueueResourceViewV1 {
    plan: ComputeAqlQueuePlanV1,
    buffers: KfdAqlComputeQueueBuffers,
    ring_size: KfdAqlQueueRingSize,
    initial_percentage: KfdQueuePercentage,
    priority: KfdQueuePriority,
}

#[derive(Debug)]
struct QueueKernelOutcomeV1<T> {
    value: T,
    status: QueueSyscallStatusV1,
}

/// Private substitution point. Its associated authority type is retained by
/// the engine and cannot be manufactured through the public crate API.
#[allow(dead_code)]
trait NativeQueueBackendV1 {
    type ResourceAuthority;

    fn opener_pid(&self) -> u32;
    fn take_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, NativeQueueAdapterErrorV1>;
    fn resource_view(
        &self,
        authority: &Self::ResourceAuthority,
    ) -> Result<NativeQueueResourceViewV1, NativeQueueAdapterErrorV1>;
    fn check_currentness(&mut self) -> Result<(), &'static str>;
    fn create(
        &mut self,
        args: KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlCreateQueueArgs>;
    fn update(
        &mut self,
        args: KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlUpdateQueueArgs>;
    fn destroy(
        &mut self,
        args: KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlDestroyQueueArgs>;
}

struct RetainedQueueResourcesV1<A> {
    key: QueueKeyV1,
    authority: Option<A>,
    view: NativeQueueResourceViewV1,
    create_outputs: Option<KfdGfx942CreateQueueOutputs>,
}

struct NativeQueueEngineV1<B: NativeQueueBackendV1> {
    backend: B,
    opener_pid: u32,
    identity: DeviceIdentityStateV1,
    memory: MemoryLifecycleStateV1,
    model: QueueLifecycleStateV1,
    resources: Vec<RetainedQueueResourcesV1<B::ResourceAuthority>>,
    authority_poisoned: bool,
}

#[allow(dead_code)]
impl<B: NativeQueueBackendV1> NativeQueueEngineV1<B> {
    fn new(mut backend: B) -> Result<Self, NativeQueueAdapterErrorV1> {
        let opener_pid = backend.opener_pid();
        if opener_pid != std::process::id() {
            return Err(NativeQueueAdapterErrorV1::ProcessChanged);
        }
        let foundation = backend.take_model_foundation()?;
        let domain = foundation.identity.domain_id();
        if foundation.memory.domain_id() != domain {
            return Err(NativeQueueAdapterErrorV1::InvalidResource(
                "model observation domain",
            ));
        }
        Ok(Self {
            backend,
            opener_pid,
            identity: foundation.identity,
            memory: foundation.memory,
            model: QueueLifecycleStateV1::new(domain),
            resources: Vec::new(),
            authority_poisoned: false,
        })
    }

    fn admit(
        &mut self,
        authority: B::ResourceAuthority,
    ) -> Result<QueueKeyV1, NativeQueueAdapterErrorV1> {
        if self.authority_poisoned {
            return Err(NativeQueueAdapterErrorV1::AuthorityPoisoned);
        }
        let view = self.backend.resource_view(&authority)?;
        if view.buffers.ring_base_address == 0
            || view.buffers.write_pointer_address == 0
            || view.buffers.read_pointer_address == 0
            || view.buffers.eop_buffer_address == 0
            || view.buffers.ctx_save_restore_address == 0
            || view.buffers.eop_buffer_size == 0
            || view.buffers.ctx_save_restore_size == 0
            || view.buffers.ctl_stack_size == 0
            || admit_kfd_aql_queue_ring_address(view.buffers.ring_base_address).is_err()
        {
            return Err(NativeQueueAdapterErrorV1::InvalidResource(
                "nonzero queue buffer contract",
            ));
        }
        let admission = self
            .model
            .admit_compute_aql_plan(&self.identity, &self.memory, view.plan)
            .map_err(|_| NativeQueueAdapterErrorV1::ModelProjection)?;
        let key = view.plan.queue;
        (self.model, self.memory) = admission.into_states();
        self.resources.push(RetainedQueueResourcesV1 {
            key,
            authority: Some(authority),
            view,
            create_outputs: None,
        });
        Ok(key)
    }

    fn journal_summary(&self) -> NativeQueueJournalSummaryV1 {
        NativeQueueJournalSummaryV1 {
            queues: self.model.queues().len(),
            history: self.model.history().len(),
            live_publications: self
                .memory
                .publications()
                .iter()
                .filter(|publication| {
                    publication.state == fe2o3_runtime_model::MemoryPublicationStateV1::Live
                })
                .count(),
            ambiguous: self
                .model
                .queues()
                .iter()
                .filter(|queue| queue.phase == ComputeAqlQueuePhaseV1::Ambiguous)
                .count(),
            authority_poisoned: self.authority_poisoned,
        }
    }

    fn phase(&self, key: QueueKeyV1) -> Option<ComputeAqlQueuePhaseV1> {
        self.model
            .queues()
            .iter()
            .find(|queue| queue.plan.queue == key)
            .map(|queue| queue.phase)
    }

    fn native_queue_id(&self, key: QueueKeyV1) -> Option<u32> {
        self.model
            .queues()
            .iter()
            .find(|queue| queue.plan.queue == key)
            .and_then(|queue| queue.queue_id)
            .map(|queue_id| queue_id.0)
    }

    fn create_outputs(&self, key: QueueKeyV1) -> Option<KfdGfx942CreateQueueOutputs> {
        self.resources
            .iter()
            .find(|resource| resource.key == key && resource.authority.is_some())
            .and_then(|resource| resource.create_outputs)
    }

    fn create(&mut self, key: QueueKeyV1) -> Result<(), NativeQueueAdapterErrorV1> {
        self.prepare_operation()?;
        let view = self.resource(key)?.view;
        let gpu_id = view.plan.current_device.correlation().kfd_gpu_id();
        let args = KfdIoctlCreateQueueArgs::new_compute_aql(
            view.buffers,
            view.ring_size,
            gpu_id,
            view.initial_percentage,
            view.priority,
        );
        self.begin(QueueTransitionV1::BeginCreate { queue: key })?;
        let outcome = self.backend.create(args);
        let queue_id_field = queue_id_field(outcome.value.queue_id);
        let mut status = outcome.status;
        let mut malformed = None;
        if !create_inputs_unchanged(args, outcome.value) {
            status = QueueSyscallStatusV1::Indeterminate;
            malformed = Some("CREATE_QUEUE immutable inputs");
        }
        let admitted_outputs = if status == QueueSyscallStatusV1::Succeeded {
            match admit_kfd_gfx942_create_queue_outputs(
                outcome.value.queue_id,
                outcome.value.doorbell_offset,
                gpu_id,
            ) {
                Ok(outputs) => Some(outputs),
                Err(_) => {
                    status = QueueSyscallStatusV1::Indeterminate;
                    malformed = Some("CREATE_QUEUE outputs");
                    None
                }
            }
        } else {
            if status == QueueSyscallStatusV1::FailedNoEffect
                && (outcome.value.queue_id != CREATE_QUEUE_ID_SENTINEL_V1
                    || outcome.value.doorbell_offset != u64::MAX)
            {
                status = QueueSyscallStatusV1::Indeterminate;
                malformed = Some("CREATE_QUEUE failed-no-effect outputs");
            }
            None
        };
        self.observe(QueueTransitionV1::ObserveCreate {
            queue: key,
            observation: QueueCreateObservationV1 {
                status,
                queue_id_field,
            },
        })?;
        let phase = self.phase(key);
        if phase == Some(ComputeAqlQueuePhaseV1::Active) {
            self.resource_mut(key)?.create_outputs = admitted_outputs;
        }
        self.finish_operation()?;
        if let Some(field) = malformed {
            self.authority_poisoned = true;
            return Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                field,
            ));
        }
        self.classify_completion(NativeQueueOperationV1::Create, status, phase)
    }

    fn update(
        &mut self,
        key: QueueKeyV1,
        configuration: QueueConfigurationIdV1,
        percentage: KfdQueuePercentage,
        priority: KfdQueuePriority,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        self.prepare_operation()?;
        let view = self.resource(key)?.view;
        let queue_id = self
            .native_queue_id(key)
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)?;
        let ring = admit_kfd_aql_queue_ring_address(view.buffers.ring_base_address)
            .map_err(|_| NativeQueueAdapterErrorV1::InvalidResource("ring address"))?;
        let args = KfdIoctlUpdateQueueArgs::reconfigure_compute_aql(
            queue_id,
            ring,
            view.ring_size,
            percentage,
            priority,
        );
        self.begin(QueueTransitionV1::BeginUpdate {
            queue: key,
            configuration,
        })?;
        self.complete_plain_operation(
            NativeQueueOperationV1::Update,
            key,
            args,
            |backend, args| backend.update(args),
        )
    }

    fn disable(&mut self, key: QueueKeyV1) -> Result<(), NativeQueueAdapterErrorV1> {
        self.prepare_operation()?;
        let view = self.resource(key)?.view;
        let queue_id = self
            .native_queue_id(key)
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)?;
        let args =
            KfdIoctlUpdateQueueArgs::disable_compute_aql(queue_id, view.ring_size, view.priority);
        self.begin(QueueTransitionV1::BeginDisable { queue: key })?;
        self.complete_plain_operation(
            NativeQueueOperationV1::Disable,
            key,
            args,
            |backend, args| backend.update(args),
        )
    }

    fn destroy(&mut self, key: QueueKeyV1) -> Result<(), NativeQueueAdapterErrorV1> {
        self.prepare_operation()?;
        let queue_id = self
            .native_queue_id(key)
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)?;
        let args = KfdIoctlDestroyQueueArgs::new(queue_id);
        self.begin(QueueTransitionV1::BeginDestroy { queue: key })?;
        let outcome = self.backend.destroy(args);
        let mut status = outcome.status;
        let malformed = outcome.value != args;
        if malformed {
            status = QueueSyscallStatusV1::Indeterminate;
        }
        self.observe(QueueTransitionV1::ObserveDestroy { queue: key, status })?;
        let phase = self.phase(key);
        self.finish_operation()?;
        if malformed {
            self.authority_poisoned = true;
            return Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Destroy,
                "DESTROY_QUEUE immutable inputs",
            ));
        }
        self.classify_completion(NativeQueueOperationV1::Destroy, status, phase)
    }

    fn release_destroyed_resources(
        &mut self,
        key: QueueKeyV1,
    ) -> Result<B::ResourceAuthority, NativeQueueAdapterErrorV1> {
        if self.authority_poisoned {
            return Err(NativeQueueAdapterErrorV1::AuthorityPoisoned);
        }
        if self.phase(key) != Some(ComputeAqlQueuePhaseV1::Destroyed) {
            return Err(NativeQueueAdapterErrorV1::InvalidPhase);
        }
        self.memory = self
            .model
            .release_resource_publications(&self.memory, key)
            .map_err(|_| NativeQueueAdapterErrorV1::ModelProjection)?;
        self.resource_mut(key)?
            .authority
            .take()
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)
    }

    fn complete_plain_operation<T: Copy + Eq>(
        &mut self,
        operation: NativeQueueOperationV1,
        key: QueueKeyV1,
        args: T,
        call: impl FnOnce(&mut B, T) -> QueueKernelOutcomeV1<T>,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        let outcome = call(&mut self.backend, args);
        let mut status = outcome.status;
        let malformed = outcome.value != args;
        if malformed {
            status = QueueSyscallStatusV1::Indeterminate;
        }
        let transition = match operation {
            NativeQueueOperationV1::Update => {
                QueueTransitionV1::ObserveUpdate { queue: key, status }
            }
            NativeQueueOperationV1::Disable => {
                QueueTransitionV1::ObserveDisable { queue: key, status }
            }
            _ => return Err(NativeQueueAdapterErrorV1::ModelProjection),
        };
        self.observe(transition)?;
        let phase = self.phase(key);
        self.finish_operation()?;
        if malformed {
            self.authority_poisoned = true;
            return Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
                operation,
                "UPDATE_QUEUE immutable inputs",
            ));
        }
        self.classify_completion(operation, status, phase)
    }

    fn classify_completion(
        &self,
        operation: NativeQueueOperationV1,
        status: QueueSyscallStatusV1,
        phase: Option<ComputeAqlQueuePhaseV1>,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        match status {
            QueueSyscallStatusV1::Succeeded
                if !matches!(phase, Some(ComputeAqlQueuePhaseV1::Ambiguous)) =>
            {
                Ok(())
            }
            QueueSyscallStatusV1::FailedNoEffect => {
                Err(NativeQueueAdapterErrorV1::BackendFailedNoEffect(operation))
            }
            _ => Err(NativeQueueAdapterErrorV1::BackendIndeterminate(operation)),
        }
    }

    fn prepare_operation(&mut self) -> Result<(), NativeQueueAdapterErrorV1> {
        if self.authority_poisoned {
            return Err(NativeQueueAdapterErrorV1::AuthorityPoisoned);
        }
        let retained = self
            .model
            .queues()
            .iter()
            .filter(|queue| queue.phase.retains_resources())
            .count();
        if self
            .model
            .history()
            .len()
            .checked_add(2 + retained)
            .is_none_or(|needed| needed > MAX_QUEUE_HISTORY_ENTRIES_V1)
        {
            return Err(NativeQueueAdapterErrorV1::JournalCapacity);
        }
        if self.opener_pid != std::process::id() || self.backend.opener_pid() != self.opener_pid {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::ProcessChanged);
        }
        if let Err(detail) = self.backend.check_currentness() {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::Currentness(detail));
        }
        Ok(())
    }

    fn finish_operation(&mut self) -> Result<(), NativeQueueAdapterErrorV1> {
        if self.opener_pid != std::process::id() || self.backend.opener_pid() != self.opener_pid {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::ProcessChanged);
        }
        if let Err(detail) = self.backend.check_currentness() {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::Currentness(detail));
        }
        Ok(())
    }

    fn begin(&mut self, transition: QueueTransitionV1) -> Result<(), NativeQueueAdapterErrorV1> {
        self.model = self
            .model
            .next(&self.identity, &self.memory, transition)
            .map_err(map_model_error)?;
        Ok(())
    }

    fn observe(&mut self, transition: QueueTransitionV1) -> Result<(), NativeQueueAdapterErrorV1> {
        match self.model.next(&self.identity, &self.memory, transition) {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => {
                // The syscall may have changed native state, while the exact
                // model observation could not be committed. Pending phases
                // already retain resources; poison the concrete adapter
                // without inventing a CurrentnessLost history edge.
                self.authority_poisoned = true;
                Err(NativeQueueAdapterErrorV1::ModelProjection)
            }
        }
    }

    fn quarantine_all(&mut self) -> Result<(), NativeQueueAdapterErrorV1> {
        let keys: Vec<_> = self
            .model
            .queues()
            .iter()
            .filter(|queue| queue.phase.retains_resources())
            .map(|queue| queue.plan.queue)
            .collect();
        for key in keys {
            self.model = match self.model.quarantine_currentness_loss(key) {
                Ok(model) => model,
                Err(_) => {
                    self.authority_poisoned = true;
                    return Err(NativeQueueAdapterErrorV1::ModelProjection);
                }
            };
        }
        self.authority_poisoned = true;
        Ok(())
    }

    fn resource(
        &self,
        key: QueueKeyV1,
    ) -> Result<&RetainedQueueResourcesV1<B::ResourceAuthority>, NativeQueueAdapterErrorV1> {
        self.resources
            .iter()
            .find(|resource| resource.key == key && resource.authority.is_some())
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)
    }

    fn resource_mut(
        &mut self,
        key: QueueKeyV1,
    ) -> Result<&mut RetainedQueueResourcesV1<B::ResourceAuthority>, NativeQueueAdapterErrorV1>
    {
        self.resources
            .iter_mut()
            .find(|resource| resource.key == key && resource.authority.is_some())
            .ok_or(NativeQueueAdapterErrorV1::InvalidPhase)
    }

    /// Returns the private backend after an exact consuming teardown path.
    ///
    /// This method performs no native operation. Every contained capability
    /// type also has a no-effect `Drop`; callers must complete the explicit
    /// queue and resource transitions before consuming the engine here.
    fn into_backend(self) -> Result<B, NativeQueueAdapterErrorV1> {
        if self.authority_poisoned
            || self.resources.is_empty()
            || self.resources.iter().any(|resource| {
                resource.authority.is_some()
                    || self.phase(resource.key) != Some(ComputeAqlQueuePhaseV1::Destroyed)
            })
        {
            return Err(NativeQueueAdapterErrorV1::InvalidPhase);
        }
        Ok(self.backend)
    }
}

fn map_model_error(_: QueueTransitionErrorV1) -> NativeQueueAdapterErrorV1 {
    NativeQueueAdapterErrorV1::InvalidPhase
}

fn queue_id_field(queue_id: u32) -> CreateQueueIdFieldObservationV1 {
    if queue_id == CREATE_QUEUE_ID_SENTINEL_V1 {
        CreateQueueIdFieldObservationV1::SentinelUnchanged
    } else {
        CreateQueueIdFieldObservationV1::Returned(UntrustedQueueIdObservationV1(queue_id))
    }
}

fn create_inputs_unchanged(
    before: KfdIoctlCreateQueueArgs,
    after: KfdIoctlCreateQueueArgs,
) -> bool {
    before.ring_base_address == after.ring_base_address
        && before.write_pointer_address == after.write_pointer_address
        && before.read_pointer_address == after.read_pointer_address
        && before.ring_size == after.ring_size
        && before.gpu_id == after.gpu_id
        && before.queue_type == after.queue_type
        && before.queue_percentage == after.queue_percentage
        && before.queue_priority == after.queue_priority
        && before.eop_buffer_address == after.eop_buffer_address
        && before.eop_buffer_size == after.eop_buffer_size
        && before.ctx_save_restore_address == after.ctx_save_restore_address
        && before.ctx_save_restore_size == after.ctx_save_restore_size
        && before.ctl_stack_size == after.ctl_stack_size
        && before.sdma_engine_id == after.sdma_engine_id
        && before.pad == after.pad
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
