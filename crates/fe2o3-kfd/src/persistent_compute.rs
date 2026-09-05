//! Single-allocation bridge between directional persistent SDMA and fixed compute.
//!
//! Public values in this module are addressless, move-only custody. The queue
//! retains the exact mapped device lease while a compute attachment is active;
//! the persistent owner retains its incarnation and use ledger with the native
//! lease temporarily absent.

use core::fmt;
use core::marker::PhantomData;
use std::rc::Rc;

use fe2o3_runtime_model::QueueKeyV1;

use crate::persistent_allocation::{
    Gfx942PersistentCompletedV1, Gfx942PersistentDependencyFrontierV1, Gfx942PersistentPreparedV1,
    Gfx942PersistentPublishedV1, Gfx942PersistentUseLeaseV1,
};
use crate::persistent_directional_sdma::{
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaWindowCompletedV1, Gfx942DirectionalQueuePersistentAllocationV1,
};
use crate::queue::{
    ComputeAqlQueueSessionErrorV1, Gfx942CompletedDispatchBatchV1,
    Gfx942CompletionRecycleObservationV1, Gfx942DispatchBatchV1, Gfx942FixedDispatchDataV1,
};
use crate::sdma::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1};
use crate::shared_memory::Gfx942DeviceMemoryIdentityV1;

/// Claim boundary for the first persistent directional-SDMA/compute bridge.
pub const GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-gfx942-kfd-persistent-local-compute-r25-v2\n",
    "target=gfx942:xnack-,one-primary-compute-queue-and-one-directional-sdma-pair\n",
    "admission=one-local-fresh-or-exact-size-pooled-logical-equals-physical-allocation,one-fixed-compute-packet,one-full-allocation-device-local-binding,complete-live-device-set-of-one,metadata-derived-access\n",
    "binding=exact-parent-and-compute-queue-occurrence,attachment-generation,pool-generation,logical-and-physical-extent,mapped-storage-identity,persistent-owner-incarnation,dispatch-generation,and-prepare-once-control-identity-over-code-abi-packet-geometry-kernarg-content-role-and-storage-layout\n",
    "initialization=read-or-readwrite-requires-one-exact-full-h2d-completion-and-exact-pre-h2d-host-certificate-content-descriptor-match,certificate-is-minted-only-by-userspace-hash-while-copy-and-bound-to-queue-storage-identity-pool-generation-full-extents-and-range,h2d-source-linear-custody-preserves-certificate,d2h-or-any-other-host-mutation-invalidates-before-publication,promotion-retains-pre-post-operational-currentness,h2d-source-host-owner-is-returned-separately,ready-retains-only-device-owner-and-sealed-byte-digest,bind-relabels-the-digest-to-the-final-kernel-role-without-copy,write-only-may-use-quiescent-uninitialized-custody,no-compute-write-initialization-promotion\n",
    "lifecycle=first-launch-preflight-reserve-prepare-detach-and-allocate-map-retain-control,confirmed-only-publish,pending-retains-all-custody,exact-completion-then-signal-recycle-then-data-only-detach-native-restore-and-settle,subsequent-exact-replay-reattaches-only-data-to-the-retained-control-and-advances-generation,explicit-frontier-retirement,ordinary-destroy-releases-retained-control-exactly-once\n",
    "ledger=the-existing-directional-sdma-outstanding-buffer-debit-and-pool-generation-are-preserved\n",
    "failure=pre-retention-rejection-returns-input,confirmed-no-effect-ring-occupancy-retains-prepared-retry,post-retention-publication-completion-recycle-detach-or-restore-ambiguity-quarantines-and-requires-process-teardown\n",
    "partial-prepare=after-first-code-or-kernarg-allocation,the-terminal-queue-owned-shared-memory-session-registry-retains-every-native-record,linear-token-drop-performs-no-native-cleanup,and-process-teardown-is-required\n",
    "terminal-custody=queue-or-returned-failure-retains-the-exact-attached-published-completed-recycled-data-storage-or-restored-owner-stage-with-address-free-observation\n",
    "authority=no-native-address-handle-pointer-fd-packet-signal-or-storage-identity-export\n",
    "limits=no-auxiliary-lane,no-padded-pooled-or-partial-range,no-second-device-allocation,no-concurrent-sdma,no-xgmi,no-generic-fixed-dispatch-escape-while-attached,certificate-is-userspace-write-evidence-not-kernel-attestation-or-loaded-kernel-proof,sha256-collision-resistance-and-cpu-gpu-coherence-contracted\n",
    "evidence=native-neutral-transition-and-failure-tests-only,no-rust-to-model-refinement,no-hardware-execution-or-performance-evidence\n",
);

/// SHA-256 of [`GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1`].
pub const GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_SHA256_V1: &str =
    "f7c0d2b4ccf7d1f7369928ee8631b21fd1ba35bc8b02a8ab88eb164b8f962197";

/// Metadata-derived aggregate access of the exact persistent compute binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentComputeEffectV1 {
    Read,
    Write,
    ReadWrite,
}

impl Gfx942PersistentComputeEffectV1 {
    pub const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

const fn replay_authenticated_sha256_v1(
    effect: Gfx942PersistentComputeEffectV1,
    authenticated_sha256: Option<[u8; 32]>,
) -> Option<[u8; 32]> {
    if effect.writes() {
        None
    } else {
        authenticated_sha256
    }
}

/// Authenticated full-allocation H2D result that may satisfy a compute read.
#[must_use = "ready persistent compute custody must be bound or normalized"]
pub struct Gfx942PersistentComputeReadyV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) authenticated_sha256: [u8; 32],
}

impl Gfx942PersistentComputeReadyV1 {
    pub const fn byte_len(&self) -> u64 {
        self.allocation.byte_len()
    }

    pub const fn physical_byte_len(&self) -> u64 {
        self.allocation.physical_byte_len()
    }

    pub const fn authenticated_sha256(&self) -> [u8; 32] {
        self.authenticated_sha256
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        self.allocation
    }
}

impl fmt::Debug for Gfx942PersistentComputeReadyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeReadyV1")
            .field("byte_len", &self.byte_len())
            .field("physical_byte_len", &self.physical_byte_len())
            .finish_non_exhaustive()
    }
}

pub fn normalize_persistent_compute_ready_v1(
    ready: Gfx942PersistentComputeReadyV1,
) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    ready.into_allocation()
}

pub type Gfx942PersistentComputeReadyPartsV1 = (
    Gfx942DirectionalQueuePersistentAllocationV1,
    Gfx942SdmaBufferV1,
    Gfx942PersistentDependencyFrontierV1,
);

/// Linear H2D custody returned by a failed ready transition.
#[must_use = "retryable custody may be restored; terminal custody requires process teardown"]
pub enum Gfx942PersistentComputeReadyFailureCustodyV1 {
    Retryable(Gfx942PersistentComputeReadyPartsV1),
    ForeignQueue(Gfx942DirectionalPersistentSdmaWindowCompletedV1),
    ProcessTeardown(Gfx942PersistentComputeReadyTerminalCustodyV1),
}

#[must_use = "terminal H2D completion custody must remain opaque until process teardown"]
pub struct Gfx942PersistentComputeReadyTerminalCustodyV1 {
    pub(crate) completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
}

impl Gfx942PersistentComputeReadyTerminalCustodyV1 {
    pub fn direction(&self) -> crate::persistent_sdma::Gfx942PersistentSdmaDirectionV1 {
        self.completed.direction()
    }

    pub const fn copy_bytes(&self) -> u32 {
        self.completed.copy_bytes()
    }

    pub const fn packet_count(&self) -> usize {
        self.completed.packet_count()
    }
}

#[must_use = "inspect the error and retain the returned H2D custody"]
pub struct Gfx942PersistentComputeReadyFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942PersistentComputeReadyFailureCustodyV1,
}

impl Gfx942PersistentComputeReadyFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942PersistentComputeReadyFailureCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

impl fmt::Debug for Gfx942PersistentComputeReadyFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeReadyFailureV1")
            .field("error", &self.error)
            .field(
                "custody",
                &match self.custody {
                    Gfx942PersistentComputeReadyFailureCustodyV1::Retryable(_) => "retryable",
                    Gfx942PersistentComputeReadyFailureCustodyV1::ForeignQueue(_) => {
                        "foreign-queue"
                    }
                    Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(_) => {
                        "process-teardown"
                    }
                },
            )
            .finish()
    }
}

/// Quiescent input for one persistent compute attachment.
#[must_use = "persistent compute input must be bound or normalized"]
pub enum Gfx942PersistentComputeInputV1 {
    /// No sealed initialization premise; only inspected write-only access is admitted.
    Uninitialized(Gfx942DirectionalQueuePersistentAllocationV1),
    /// Exact full-H2D initialization was authenticated before frontier retirement.
    Initialized(Gfx942PersistentComputeReadyV1),
    /// Exact predecessor compute completed, recycled, detached, restored, and
    /// retired its dependency frontier while proving the full extent initialized.
    InitializedAfterDispatch(Gfx942DirectionalQueuePersistentAllocationV1),
}

impl Gfx942PersistentComputeInputV1 {
    pub const fn is_fully_initialized(&self) -> bool {
        !matches!(self, Self::Uninitialized(_))
    }

    pub(crate) fn belongs_to(&self, queue: QueueKeyV1) -> bool {
        match self {
            Self::Uninitialized(allocation) => allocation.attachment.queue == queue,
            Self::Initialized(ready) => ready.allocation.attachment.queue == queue,
            Self::InitializedAfterDispatch(allocation) => allocation.attachment.queue == queue,
        }
    }

    pub const fn byte_len(&self) -> u64 {
        match self {
            Self::Uninitialized(allocation) => allocation.byte_len(),
            Self::Initialized(ready) => ready.byte_len(),
            Self::InitializedAfterDispatch(allocation) => allocation.byte_len(),
        }
    }

    pub const fn physical_byte_len(&self) -> u64 {
        match self {
            Self::Uninitialized(allocation) => allocation.physical_byte_len(),
            Self::Initialized(ready) => ready.physical_byte_len(),
            Self::InitializedAfterDispatch(allocation) => allocation.physical_byte_len(),
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Option<[u8; 32]>,
        bool,
    ) {
        match self {
            Self::Uninitialized(allocation) => (allocation, None, false),
            Self::Initialized(ready) => (ready.allocation, Some(ready.authenticated_sha256), true),
            Self::InitializedAfterDispatch(allocation) => (allocation, None, true),
        }
    }

    pub(crate) fn from_parts(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        authenticated_sha256: Option<[u8; 32]>,
        fully_initialized: bool,
    ) -> Self {
        match (authenticated_sha256, fully_initialized) {
            (Some(authenticated_sha256), true) => {
                Self::Initialized(Gfx942PersistentComputeReadyV1 {
                    allocation,
                    authenticated_sha256,
                })
            }
            (None, true) => Self::InitializedAfterDispatch(allocation),
            (None, false) => Self::Uninitialized(allocation),
            (Some(_), false) => {
                debug_assert!(false, "authenticated persistent input must be initialized");
                Self::Uninitialized(allocation)
            }
        }
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        match self {
            Self::Uninitialized(allocation) | Self::InitializedAfterDispatch(allocation) => {
                allocation
            }
            Self::Initialized(ready) => ready.allocation,
        }
    }
}

impl fmt::Debug for Gfx942PersistentComputeInputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeInputV1")
            .field(
                "initialized",
                &matches!(
                    self,
                    Self::Initialized(_) | Self::InitializedAfterDispatch(_)
                ),
            )
            .field("byte_len", &self.byte_len())
            .field("physical_byte_len", &self.physical_byte_len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistentComputeBindingKeyV1 {
    pub(crate) queue: QueueKeyV1,
    pub(crate) attachment_generation: u64,
}

macro_rules! receipt {
    ($name:ident) => {
        #[must_use = "persistent compute custody must be advanced or retained"]
        pub struct $name {
            pub(crate) binding: PersistentComputeBindingKeyV1,
            pub(crate) thread_affinity: PhantomData<Rc<()>>,
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("attachment_generation", &self.binding.attachment_generation)
                    .finish_non_exhaustive()
            }
        }
    };
}

receipt!(Gfx942PreparedPersistentComputeDispatchV1);

#[must_use = "published persistent compute custody must be polled"]
pub struct Gfx942PersistentComputeDispatchV1 {
    pub(crate) binding: PersistentComputeBindingKeyV1,
    pub(crate) batch: Gfx942DispatchBatchV1<1>,
    pub(crate) thread_affinity: PhantomData<Rc<()>>,
}

impl fmt::Debug for Gfx942PersistentComputeDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeDispatchV1")
            .field("attachment_generation", &self.binding.attachment_generation)
            .finish_non_exhaustive()
    }
}

#[must_use = "completed persistent compute custody must be recycled"]
pub struct Gfx942CompletedPersistentComputeDispatchV1 {
    pub(crate) binding: PersistentComputeBindingKeyV1,
    pub(crate) completed: Gfx942CompletedDispatchBatchV1<1>,
    pub(crate) thread_affinity: PhantomData<Rc<()>>,
}

impl fmt::Debug for Gfx942CompletedPersistentComputeDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedPersistentComputeDispatchV1")
            .field("attachment_generation", &self.binding.attachment_generation)
            .finish_non_exhaustive()
    }
}

#[must_use = "recycled persistent compute custody must be detached and restored"]
pub struct Gfx942RecycledPersistentComputeDispatchV1 {
    pub(crate) binding: PersistentComputeBindingKeyV1,
    pub(crate) recycle: Gfx942CompletionRecycleObservationV1,
    pub(crate) thread_affinity: PhantomData<Rc<()>>,
}

impl fmt::Debug for Gfx942RecycledPersistentComputeDispatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942RecycledPersistentComputeDispatchV1")
            .field("attachment_generation", &self.binding.attachment_generation)
            .finish_non_exhaustive()
    }
}

impl Gfx942RecycledPersistentComputeDispatchV1 {
    pub const fn recycle_observation(&self) -> Gfx942CompletionRecycleObservationV1 {
        self.recycle
    }
}

#[must_use = "pending persistent compute custody must be polled again"]
pub enum Gfx942PersistentComputePollV1 {
    Pending(Gfx942PersistentComputeDispatchV1),
    Ready(Gfx942CompletedPersistentComputeDispatchV1),
}

/// Restored persistent ownership after exact compute completion and detach.
#[must_use = "completed persistent compute ownership must be retired or reused"]
pub struct Gfx942PersistentComputeCompletedV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) frontier: Gfx942PersistentDependencyFrontierV1,
    pub(crate) effect: Gfx942PersistentComputeEffectV1,
    pub(crate) authenticated_sha256: Option<[u8; 32]>,
    pub(crate) fully_initialized: bool,
}

impl Gfx942PersistentComputeCompletedV1 {
    pub const fn effect(&self) -> Gfx942PersistentComputeEffectV1 {
        self.effect
    }

    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentComputeEffectV1,
    ) {
        (self.allocation, self.frontier, self.effect)
    }

    /// Retires the exact completed frontier and preserves the strongest valid
    /// initialization proof for an exact persistent-control replay.
    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontier_for_replay_v1(
        self,
    ) -> Result<
        (
            Gfx942PersistentComputeInputV1,
            Gfx942PersistentComputeEffectV1,
        ),
        Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    > {
        let allocation = self.allocation.retire_settled_frontier_v1(self.frontier)?;
        let authenticated_sha256 =
            replay_authenticated_sha256_v1(self.effect, self.authenticated_sha256);
        let input = match (authenticated_sha256, self.fully_initialized) {
            (Some(authenticated_sha256), true) => {
                Gfx942PersistentComputeInputV1::Initialized(Gfx942PersistentComputeReadyV1 {
                    allocation,
                    authenticated_sha256,
                })
            }
            (None, true) => Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation),
            (_, false) => Gfx942PersistentComputeInputV1::Uninitialized(allocation),
        };
        Ok((input, self.effect))
    }
}

pub(crate) enum PersistentComputeUseStateV1 {
    Prepared(Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>),
    Published(Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>),
    Completed(Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>),
    Recycled(Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>),
    Quarantined,
}

pub(crate) enum PersistentComputeTerminalNativeCustodyV1 {
    Attached,
    Published(Gfx942DispatchBatchV1<1>),
    Completed(Gfx942CompletedDispatchBatchV1<1>),
    Recycled(Gfx942CompletionRecycleObservationV1),
    Data(Vec<Gfx942FixedDispatchDataV1>),
    Storage(Gfx942SdmaBufferStorageV1),
    Restored,
}

/// Address-free observation of native custody retained after a terminal fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentComputeTerminalStageV1 {
    Attached,
    Published,
    Completed,
    Recycled,
    DataDetached,
    StorageDetached,
    Restored,
}

impl PersistentComputeTerminalNativeCustodyV1 {
    pub(crate) const fn stage(&self) -> Gfx942PersistentComputeTerminalStageV1 {
        match self {
            Self::Attached => Gfx942PersistentComputeTerminalStageV1::Attached,
            Self::Published(batch) => {
                let _ = core::mem::size_of_val(batch);
                Gfx942PersistentComputeTerminalStageV1::Published
            }
            Self::Completed(completed) => {
                let _ = core::mem::size_of_val(completed);
                Gfx942PersistentComputeTerminalStageV1::Completed
            }
            Self::Recycled(observation) => {
                let _ = observation.packet_count();
                Gfx942PersistentComputeTerminalStageV1::Recycled
            }
            Self::Data(data) => {
                let _ = data.len();
                Gfx942PersistentComputeTerminalStageV1::DataDetached
            }
            Self::Storage(storage) => {
                let _ = core::mem::size_of_val(storage);
                Gfx942PersistentComputeTerminalStageV1::StorageDetached
            }
            Self::Restored => Gfx942PersistentComputeTerminalStageV1::Restored,
        }
    }
}

pub(crate) struct PersistentComputeAttachmentV1 {
    pub(crate) allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    pub(crate) authenticated_sha256: Option<[u8; 32]>,
    pub(crate) state: PersistentComputeUseStateV1,
    pub(crate) binding: PersistentComputeBindingKeyV1,
    pub(crate) storage_identity: Gfx942DeviceMemoryIdentityV1,
    pub(crate) effect: Gfx942PersistentComputeEffectV1,
    pub(crate) predecessor_dispatch_generation: Option<u64>,
    pub(crate) terminal_custody: Option<PersistentComputeTerminalNativeCustodyV1>,
}

#[must_use = "retryable input must be recovered; terminal input requires process teardown"]
pub enum Gfx942PersistentComputeBindFailureCustodyV1 {
    Retryable(Gfx942PersistentComputeInputV1),
    ProcessTeardown(Gfx942PersistentComputeBindTerminalCustodyV1),
}

#[must_use = "terminal persistent-compute input must remain opaque until process teardown"]
pub struct Gfx942PersistentComputeBindTerminalCustodyV1 {
    pub(crate) input: Option<Gfx942PersistentComputeInputV1>,
}

impl Gfx942PersistentComputeBindTerminalCustodyV1 {
    pub const fn retains_prebinding_input(&self) -> bool {
        self.input.is_some()
    }
}

#[must_use = "inspect the error and retain the returned bind custody"]
pub struct Gfx942PersistentComputeBindFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) custody: Gfx942PersistentComputeBindFailureCustodyV1,
}

impl Gfx942PersistentComputeBindFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942PersistentComputeBindFailureCustodyV1,
    ) {
        (self.error, self.custody)
    }
}

impl fmt::Debug for Gfx942PersistentComputeBindFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeBindFailureV1")
            .field("error", &self.error)
            .field(
                "custody",
                &match self.custody {
                    Gfx942PersistentComputeBindFailureCustodyV1::Retryable(_) => "retryable",
                    Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(_) => {
                        "process-teardown"
                    }
                },
            )
            .finish()
    }
}

#[must_use = "retryable prepared custody must be retried; terminal failure requires teardown"]
pub struct Gfx942PersistentComputeExecutionFailureV1 {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) retryable: Option<Gfx942PreparedPersistentComputeDispatchV1>,
}

impl Gfx942PersistentComputeExecutionFailureV1 {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Option<Gfx942PreparedPersistentComputeDispatchV1>,
    ) {
        (self.error, self.retryable)
    }
}

impl fmt::Debug for Gfx942PersistentComputeExecutionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeExecutionFailureV1")
            .field("error", &self.error)
            .field("retryable", &self.retryable.is_some())
            .finish()
    }
}

#[must_use = "recover foreign custody or retain terminal native custody until process teardown"]
pub struct Gfx942PersistentComputeTransitionFailureV1<T> {
    pub(crate) error: ComputeAqlQueueSessionErrorV1,
    pub(crate) recovered: Option<T>,
    pub(crate) retained: Option<PersistentComputeTerminalNativeCustodyV1>,
}

#[must_use = "terminal persistent-compute custody must remain retained until process teardown"]
pub struct Gfx942PersistentComputeTerminalCustodyV1 {
    native: Option<PersistentComputeTerminalNativeCustodyV1>,
}

impl Gfx942PersistentComputeTerminalCustodyV1 {
    pub fn stage(&self) -> Option<Gfx942PersistentComputeTerminalStageV1> {
        self.native
            .as_ref()
            .map(PersistentComputeTerminalNativeCustodyV1::stage)
    }
}

#[must_use = "retryable custody must be retried; terminal custody requires process teardown"]
pub enum Gfx942PersistentComputeTransitionFailureCustodyV1<T> {
    Retryable(T),
    ProcessTeardown(Gfx942PersistentComputeTerminalCustodyV1),
}

impl<T> fmt::Debug for Gfx942PersistentComputeTransitionFailureV1<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentComputeTransitionFailureV1")
            .field("error", &self.error)
            .field("recovered", &self.recovered.is_some())
            .field("retained_stage", &self.retained_stage())
            .finish()
    }
}

impl<T> Gfx942PersistentComputeTransitionFailureV1<T> {
    pub const fn error(&self) -> &ComputeAqlQueueSessionErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        ComputeAqlQueueSessionErrorV1,
        Gfx942PersistentComputeTransitionFailureCustodyV1<T>,
    ) {
        let custody = match self.recovered {
            Some(recovered) => {
                Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(recovered)
            }
            None => Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeTerminalCustodyV1 {
                    native: self.retained,
                },
            ),
        };
        (self.error, custody)
    }

    pub fn retained_stage(&self) -> Option<Gfx942PersistentComputeTerminalStageV1> {
        self.retained
            .as_ref()
            .map(PersistentComputeTerminalNativeCustodyV1::stage)
    }
}

pub type Gfx942PersistentComputeCancelFailureV1 =
    Gfx942PersistentComputeTransitionFailureV1<Gfx942PreparedPersistentComputeDispatchV1>;
pub type Gfx942PersistentComputePollFailureV1 =
    Gfx942PersistentComputeTransitionFailureV1<Gfx942PersistentComputeDispatchV1>;
pub type Gfx942PersistentComputeRecycleFailureV1 =
    Gfx942PersistentComputeTransitionFailureV1<Gfx942CompletedPersistentComputeDispatchV1>;
pub type Gfx942PersistentComputeDetachFailureV1 =
    Gfx942PersistentComputeTransitionFailureV1<Gfx942RecycledPersistentComputeDispatchV1>;

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn metadata_effect_reports_only_write_capability() {
        assert!(!Gfx942PersistentComputeEffectV1::Read.writes());
        assert!(Gfx942PersistentComputeEffectV1::Write.writes());
        assert!(Gfx942PersistentComputeEffectV1::ReadWrite.writes());
    }

    #[test]
    fn readwrite_completion_never_reuses_predispatch_authenticated_digest() {
        let stale = Some([0x5a; 32]);
        assert_eq!(
            replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::Read, stale),
            stale
        );
        assert_eq!(
            replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::Write, stale),
            None
        );
        assert_eq!(
            replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::ReadWrite, stale),
            None
        );
    }

    #[test]
    fn terminal_custody_observation_is_address_free_and_stage_exact() {
        assert_eq!(
            PersistentComputeTerminalNativeCustodyV1::Attached.stage(),
            Gfx942PersistentComputeTerminalStageV1::Attached
        );
        assert_eq!(
            PersistentComputeTerminalNativeCustodyV1::Data(Vec::new()).stage(),
            Gfx942PersistentComputeTerminalStageV1::DataDetached
        );
        assert_eq!(
            PersistentComputeTerminalNativeCustodyV1::Restored.stage(),
            Gfx942PersistentComputeTerminalStageV1::Restored
        );
    }

    #[test]
    fn transition_failure_into_parts_preserves_retryable_or_terminal_custody() {
        let retryable = Gfx942PersistentComputeTransitionFailureV1 {
            error: ComputeAqlQueueSessionErrorV1::Contract("foreign receipt"),
            recovered: Some(37_u64),
            retained: None,
        };
        let (_, custody) = retryable.into_parts();
        assert!(matches!(
            custody,
            Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(37)
        ));

        let terminal = Gfx942PersistentComputeTransitionFailureV1::<u64> {
            error: ComputeAqlQueueSessionErrorV1::Contract("terminal receipt"),
            recovered: None,
            retained: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
        };
        let (_, custody) = terminal.into_parts();
        let Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) = custody
        else {
            panic!("terminal native custody must not be dropped by into_parts")
        };
        assert_eq!(
            custody.stage(),
            Some(Gfx942PersistentComputeTerminalStageV1::Attached)
        );
    }
}
