//! Linear ownership of one long-lived KFD queue and replaceable fixed batches.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use fe2o3_aql::{AQL_MAX_FIXED_BATCH_PACKETS_V2, AqlRingCapacityV1};
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueObservationV1, ComputeAqlQueueSessionErrorV1,
    ComputeAqlQueueSessionV1, Gfx942CompletedDispatchBatchV1, Gfx942CompletedDispatchReadRequestV1,
    Gfx942CompletedDispatchReadbackV1, Gfx942CompletedDispatchSnapshotRequestV1,
    Gfx942CompletionErrorV1, Gfx942CompletionRecycleObservationV1, Gfx942DeviceContentDescriptorV1,
    Gfx942DispatchBatchV1, Gfx942DispatchPollWithProgressV1, Gfx942DispatchProgressV1,
    Gfx942FixedDispatchDataV1, Gfx942RecycledDispatchResourcesV1,
    Gfx942TimeoutExecutionObservationV1,
};

use crate::allocation::{
    DeviceAllocationRoleMarkerV1, DeviceLocalAllocationV1, QuarantinedServiceAllocationsV1,
    ServiceAllocationErrorV1, ServiceAllocationReleaseFailureV1,
    ServiceAllocationReleaseObservationV1, ServiceAllocationSessionV1,
    ServiceAllocationSubleaseSetV1, ServiceDeviceDispatchRangeV1, ServiceDispatchRangeV1,
    ServiceHostDispatchRangeV1, ServiceHostDispatchSnapshotRangeV1, ServiceQueueAllocationLedgerV1,
    ServiceQueueAllocationRestoreFailureV1,
};
use crate::batch::ServiceFixedBatchV1;

/// Frozen claim boundary for the reusable service queue composition layer.
pub const SERVICE_QUEUE_OWNERSHIP_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-service-addressless-fixed-queue-r14-v1\n",
    "source.compute_aql_session_sha256=cad16095d8b14f73fae906ebaa2ab21b0763d46f95e45622eb75f3f33c535fcf\n",
    "queue=one-live-kfd-compute-aql-owner,ring-event-doorbell-and-signal-resources-retained-across-live-rebind,quiescent-rollover-may-confirm-destroy-and-create-one-replacement-queue\n",
    "batch=1-through-8192-fixed-packets,conservative-wait-for-prior-ordering-default-with-explicit-independent-opt-in,exact-ring-capacity,inspected-programs,complete-kernarg-images,addressless-checked-device-local-or-host-visible-ranges,optional-initialized-enclosing-host-snapshot-associated-with-one-strict-interior\n",
    "implicit-kernarg=exact-trailing-256-byte-COV6-caller-zero-suffix,lower-owner-privately-populates-metadata-derived-block-count-group-size-remainder-zero-global-offset-grid-dimensions-and-dynamic-lds,queue-pointer-and-runtime-service-or-address-fields-rejected\n",
    "publication=one-reservation-one-write-counter-fetch-add,one-retained-final-ordering-header-per-packet,one-final-doorbell-per-fixed-batch\n",
    "custody=prepared-published-completed-recycled-unbound-linear-service-types,consuming-poll-with-progress-returns-pending-or-completed-custody-plus-same-scan-redacted-counts-and-first-pending-index,terminal-timeout-failure-borrows-addressless-currentness-enveloped-execution-observation,exact-completion-and-signal-recycle-before-detach-rebind-or-attached-or-unbound-returning-destroy\n",
    "data=read-and-readwrite-require-sealed-full-initialization,write-only-may-consume-uninitialized-exclusive-storage,initialized-state-retained-after-generic-completion-without-stale-content-digest\n",
    "subleases=whole-native-allocation-owner-retained,partition-registry-transfers-with-ledger,partitioned-bindings-require-member-index-and-contained-offset-extent,detached-initialized-replacement-preflights-and-atomically-installs-an-exact-new-partition,replacement-denies-old-allocation-generation\n",
    "readback=caller-can-mint-only-from-current-recycled-owner,request-binds-exact-dispatch-generation-and-owner-checked-host-allocation-generation,lower-owner-allows-an-ordinary-range-within-one-inspected-write-or-readwrite-binding-or-one-exact-declared-initialized-enclosing-snapshot-with-an-isolated-writable-interior-and-returns-owned-bytes,no-address-or-initialization-promotion\n",
    "rebind=same-native-queue-may-consume-a-different-fixed-cardinality-program-geometry-kernarg-and-addressless-data-binding-after-exact-recycle,unbound-device-partition-insertion-removal-or-replacement-and-host-visible-replacement-advance-private-ledgers-and-reissue-shifted-addressless-ranges,rollover-may-consume-a-new-ring-size-only-after-exact-detach-and-confirmed-old-native-destroy,dispatch-generation-strictly-advances-from-the-detached-predecessor-across-either-route,lower-owner-reclaims-authoritative-model-foundation-after-every-live-allocation-lifecycle-mutation\n",
    "release=return-attached-or-exact-ordered-detached-data-custody-after-exact-recycle,destroy-native-queue,restore-service-ledger,reverse-order-unmap-and-free\n",
    "qualification-fault-injection=feature-gated-post-recycle-before-completed-read-attempt-terminal-typestate,prior-attempt-rejects-and-returns-recycled-owner,ordinary-native-teardown-only,no-synthetic-kfd-error-or-hardware-fault-claim\n",
    "failure=pure-rejection-recovers-input-owners,ambiguous-native-side-effect-is-terminal-and-denies-retry,opaque-quarantine-retains-available-owner-state,timeout-observation-grants-no-live-introspection-or-authority\n",
    "authority=no-native-address-handle-pointer-fd-mmio-signal-or-packet-template-export,no-caller-initialization-or-effect-assertion\n",
    "excluded=executable-correctness,effect-correctness-beyond-inspected-metadata,full-write-coverage,content-interpretation,numerical-correctness,hardware-execution,performance\n",
);

/// SHA-256 of [`SERVICE_QUEUE_OWNERSHIP_MANIFEST_V1`].
pub const SERVICE_QUEUE_OWNERSHIP_MANIFEST_SHA256_V1: &str =
    "545168583aa95b944985e850adb31902e6d57a980cac645f426e7d26cc295d7a";

/// Feature-bound contract for deliberate service queue-transition faults.
#[cfg(feature = "qualification-fault-injection")]
pub const SERVICE_QUALIFICATION_QUEUE_FAULT_CONTRACT_V1: &str = concat!(
    "profile=fe2o3-service-qualification-queue-fault-r1-v1\n",
    "availability=cargo-feature:qualification-fault-injection\n",
    "injection=post-recycle-before-any-completed-read-attempt,consumes-recycled-owner\n",
    "terminal=readback-reuse-detach-denied-by-type,ordinary-returning-teardown-only\n",
    "authority=no-synthetic-kfd-error-native-fault-device-fault-or-reset-claim\n",
);

/// SHA-256 of [`SERVICE_QUALIFICATION_QUEUE_FAULT_CONTRACT_V1`].
#[cfg(feature = "qualification-fault-injection")]
pub const SERVICE_QUALIFICATION_QUEUE_FAULT_CONTRACT_SHA256_V1: &str =
    "8a83bbdef6745b1eb13090e2c2e3e933734e02ee7755ab35764d9b59700c90fa";

/// Queue composition, transition, or teardown error.
#[derive(Debug)]
pub enum ServiceQueueErrorV1 {
    /// A service allocation binding or ledger invariant was rejected.
    Allocation(ServiceAllocationErrorV1),
    /// The packet count or ring capacity was rejected before KFD ownership transfer.
    BatchContract(&'static str),
    /// The retained KFD queue operation failed.
    Kfd(ComputeAqlQueueSessionErrorV1),
}

impl fmt::Display for ServiceQueueErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for ServiceQueueErrorV1 {}

struct ServiceQueueOwnerV1 {
    queue: ComputeAqlQueueSessionV1,
    ledger: ServiceQueueAllocationLedgerV1,
}

impl ServiceQueueOwnerV1 {
    const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.queue.observation()
    }
}

/// Opaque retained queue owner after an ambiguous or consuming transition failed.
///
/// No retry, publication, destruction, or allocation accessor is exposed.
/// Native process teardown is required when the lower layer cannot prove which
/// effects occurred.
#[must_use = "quarantined queue ownership must remain retained"]
pub struct QuarantinedServiceQueueV1 {
    owner: ServiceQueueOwnerV1,
    detached_data: Option<Vec<Gfx942FixedDispatchDataV1>>,
}

impl fmt::Debug for QuarantinedServiceQueueV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuarantinedServiceQueueV1")
            .field("queue", &self.owner.observation())
            .field(
                "detached_data_lease_count",
                &self.detached_data.as_ref().map_or(0, Vec::len),
            )
            .finish_non_exhaustive()
    }
}

/// A consuming queue-operation failure paired with opaque retained ownership.
#[must_use = "the failure retains a quarantined queue owner"]
pub struct ServiceQueueOperationFailureV1 {
    error: ServiceQueueErrorV1,
    retained: Box<QuarantinedServiceQueueV1>,
}

impl ServiceQueueOperationFailureV1 {
    /// Returns the operation error without discarding retained ownership.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        &self.error
    }

    /// Returns addressless execution state only for a terminal completion timeout.
    pub fn timeout_observation(&self) -> Option<&Gfx942TimeoutExecutionObservationV1> {
        timeout_observation(&self.error)
    }

    /// Consumes the failure and returns the opaque quarantined owner.
    pub fn into_quarantined(self) -> QuarantinedServiceQueueV1 {
        *self.retained
    }
}

fn timeout_observation(
    error: &ServiceQueueErrorV1,
) -> Option<&Gfx942TimeoutExecutionObservationV1> {
    match error {
        ServiceQueueErrorV1::Kfd(ComputeAqlQueueSessionErrorV1::Completion(
            Gfx942CompletionErrorV1::Timeout { observation, .. },
        )) => Some(observation.as_ref()),
        _ => None,
    }
}

impl fmt::Debug for ServiceQueueOperationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueueOperationFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

/// Queue creation failure before a public queue owner exists.
///
/// A pure service rejection retains both inputs. Once the KFD composition path
/// consumes them, an error is terminal because partial native or mapping effects
/// may be ambiguous and no typed input owner can honestly be reconstructed.
#[must_use = "pure creation rejection may retain the allocation owner and batch"]
pub enum ServiceQueueCreateFailureV1<'a, const N: usize> {
    /// Validation rejected the inputs before KFD ownership transfer.
    Rejected {
        /// Exact rejection.
        error: ServiceQueueErrorV1,
        /// Unchanged allocation owner.
        allocations: Box<ServiceAllocationSessionV1>,
        /// Unchanged batch description.
        batch: Box<ServiceFixedBatchV1<'a, N>>,
    },
    /// KFD consumed the inputs before reporting a terminal failure.
    Terminal {
        /// Exact lower-layer error.
        error: ServiceQueueErrorV1,
    },
}

impl<const N: usize> fmt::Debug for ServiceQueueCreateFailureV1<'_, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { error, .. } => formatter
                .debug_struct("Rejected")
                .field("error", error)
                .finish_non_exhaustive(),
            Self::Terminal { error } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .finish(),
        }
    }
}

impl<'a, const N: usize> ServiceQueueCreateFailureV1<'a, N> {
    /// Returns the exact error.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        match self {
            Self::Rejected { error, .. } | Self::Terminal { error } => error,
        }
    }

    /// Recovers unchanged inputs only for a pure pre-transfer rejection.
    pub fn into_rejected_inputs(
        self,
    ) -> Option<(ServiceAllocationSessionV1, ServiceFixedBatchV1<'a, N>)> {
        match self {
            Self::Rejected {
                allocations, batch, ..
            } => Some((*allocations, *batch)),
            Self::Terminal { .. } => None,
        }
    }
}

/// Prepared custody of one attached fixed batch and one live native queue.
///
/// The packet count is part of the type. Submission consumes this owner, so a
/// published generation cannot be duplicated or submitted twice.
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQueueSessionV1;
///
/// fn cannot_clone(queue: ServiceQueueSessionV1<1>) {
///     let _ = queue.clone();
/// }
/// ```
#[must_use = "the live KFD queue requires an explicit linear transition"]
pub struct ServiceQueueSessionV1<const N: usize> {
    owner: ServiceQueueOwnerV1,
}

impl<const N: usize> fmt::Debug for ServiceQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.owner.observation())
            .finish_non_exhaustive()
    }
}

impl<const N: usize> ServiceQueueSessionV1<N> {
    /// Consumes a service allocation owner and one fixed batch into a new KFD queue.
    pub fn create<'a>(
        allocations: ServiceAllocationSessionV1,
        ring_bytes: u32,
        batch: ServiceFixedBatchV1<'a, N>,
    ) -> Result<Self, ServiceQueueCreateFailureV1<'a, N>> {
        if let Err(error) = validate_ring::<N>(ring_bytes) {
            return Err(ServiceQueueCreateFailureV1::Rejected {
                error,
                allocations: Box::new(allocations),
                batch: Box::new(batch),
            });
        }
        if let Err(error) = batch.validate_for_allocation(&allocations) {
            return Err(ServiceQueueCreateFailureV1::Rejected {
                error: ServiceQueueErrorV1::Allocation(error),
                allocations: Box::new(allocations),
                batch: Box::new(batch),
            });
        }
        let transfer = match allocations.into_queue_transfer() {
            Ok(transfer) => transfer,
            Err((allocations, error)) => {
                return Err(ServiceQueueCreateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    allocations: Box::new(allocations),
                    batch: Box::new(batch),
                });
            }
        };
        let (programs, packets) = batch.into_kfd();
        let queue = transfer
            .session
            .create_compute_aql_queue_with_fixed_dispatch(
                ring_bytes,
                programs,
                packets,
                transfer.data,
            )
            .map_err(|error| ServiceQueueCreateFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
            })?;
        Ok(Self {
            owner: ServiceQueueOwnerV1 {
                queue,
                ledger: transfer.ledger,
            },
        })
    }

    /// Returns a redacted native queue observation, not queue authority.
    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.owner.observation()
    }

    /// Publishes the complete fixed batch as one KFD reservation and one doorbell store.
    pub fn submit(
        mut self,
    ) -> Result<ServicePublishedQueueSessionV1<N>, ServiceQueueOperationFailureV1> {
        match self.owner.queue.submit_fixed_dispatch::<N>() {
            Ok(batch) => Ok(ServicePublishedQueueSessionV1 {
                owner: self.owner,
                batch,
            }),
            Err(error) => Err(quarantine(self.owner, error)),
        }
    }
}

/// Published custody of one exact queue generation.
#[must_use = "published queue custody must be polled or waited exactly once"]
pub struct ServicePublishedQueueSessionV1<const N: usize> {
    owner: ServiceQueueOwnerV1,
    batch: Gfx942DispatchBatchV1<N>,
}

impl<const N: usize> fmt::Debug for ServicePublishedQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServicePublishedQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.owner.observation())
            .finish_non_exhaustive()
    }
}

impl<const N: usize> ServicePublishedQueueSessionV1<N> {
    /// Polls every exact completion signal once while preserving linear custody.
    pub fn poll(self) -> Result<ServiceQueuePollV1<N>, ServiceQueueOperationFailureV1> {
        match self.poll_with_progress()? {
            ServiceQueuePollWithProgressV1::Pending { session, .. } => {
                Ok(ServiceQueuePollV1::Pending(session))
            }
            ServiceQueuePollWithProgressV1::Ready { session, .. } => {
                Ok(ServiceQueuePollV1::Ready(session))
            }
        }
    }

    /// Polls once and returns custody with progress from the same sequential,
    /// non-atomic signal scan.
    pub fn poll_with_progress(
        mut self,
    ) -> Result<ServiceQueuePollWithProgressV1<N>, ServiceQueueOperationFailureV1> {
        match self
            .owner
            .queue
            .poll_fixed_dispatch_with_progress(self.batch)
        {
            Ok(Gfx942DispatchPollWithProgressV1::Pending { batch, progress }) => {
                Ok(ServiceQueuePollWithProgressV1::Pending {
                    session: Self {
                        owner: self.owner,
                        batch,
                    },
                    progress: ServiceQueueProgressV1::from_kfd(progress),
                })
            }
            Ok(Gfx942DispatchPollWithProgressV1::Ready {
                completed,
                progress,
            }) => Ok(ServiceQueuePollWithProgressV1::Ready {
                session: ServiceCompletedQueueSessionV1 {
                    owner: self.owner,
                    completed,
                },
                progress: ServiceQueueProgressV1::from_kfd(progress),
            }),
            Err(error) => Err(quarantine(self.owner, error)),
        }
    }

    /// Waits for every exact completion signal with the supplied bounded poll count.
    pub fn wait(
        mut self,
        polls: u32,
    ) -> Result<ServiceCompletedQueueSessionV1<N>, ServiceQueueOperationFailureV1> {
        match self.owner.queue.wait_fixed_dispatch(self.batch, polls) {
            Ok(completed) => Ok(ServiceCompletedQueueSessionV1 {
                owner: self.owner,
                completed,
            }),
            Err(error) => Err(quarantine(self.owner, error)),
        }
    }
}

/// Result of one nonblocking exact-batch completion poll.
#[derive(Debug)]
pub enum ServiceQueuePollV1<const N: usize> {
    /// Completion remains pending and published custody is returned.
    Pending(ServicePublishedQueueSessionV1<N>),
    /// Every signal was observed ready and completed custody is returned.
    Ready(ServiceCompletedQueueSessionV1<N>),
}

/// Addressless progress observed for one exact service queue batch.
///
/// Signal loads occur sequentially, not as one atomic snapshot. Counts record
/// what that scan observed, and the first pending index can already be stale by
/// the time this value is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceQueueProgressV1 {
    packet_count: u16,
    completed_count: u16,
    pending_count: u16,
    first_pending_batch_index: Option<u16>,
}

impl ServiceQueueProgressV1 {
    fn from_kfd(progress: Gfx942DispatchProgressV1) -> Self {
        Self {
            packet_count: progress.packet_count(),
            completed_count: progress.completed_count(),
            pending_count: progress.pending_count(),
            first_pending_batch_index: progress.first_pending_batch_index(),
        }
    }

    /// Returns the exact fixed-batch packet count.
    pub const fn packet_count(self) -> u16 {
        self.packet_count
    }

    /// Returns the number of signals observed completed in this scan.
    pub const fn completed_count(self) -> u16 {
        self.completed_count
    }

    /// Returns the number of signals observed pending in this scan.
    pub const fn pending_count(self) -> u16 {
        self.pending_count
    }

    /// Returns the earliest batch-local index observed pending in this scan.
    pub const fn first_pending_batch_index(self) -> Option<u16> {
        self.first_pending_batch_index
    }
}

/// Linear service custody paired with progress from the same completion scan.
#[derive(Debug)]
pub enum ServiceQueuePollWithProgressV1<const N: usize> {
    /// Completion remains pending.
    Pending {
        /// Returned published custody.
        session: ServicePublishedQueueSessionV1<N>,
        /// Progress observed in the consuming poll.
        progress: ServiceQueueProgressV1,
    },
    /// Every signal was observed completed.
    Ready {
        /// Returned completed custody.
        session: ServiceCompletedQueueSessionV1<N>,
        /// Progress observed in the consuming poll.
        progress: ServiceQueueProgressV1,
    },
}

/// Completed custody before exact signal recycle.
#[must_use = "completed signals must be recycled before reuse, detach, or release"]
pub struct ServiceCompletedQueueSessionV1<const N: usize> {
    owner: ServiceQueueOwnerV1,
    completed: Gfx942CompletedDispatchBatchV1<N>,
}

impl<const N: usize> fmt::Debug for ServiceCompletedQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceCompletedQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.owner.observation())
            .finish_non_exhaustive()
    }
}

impl<const N: usize> ServiceCompletedQueueSessionV1<N> {
    /// Recycles every completed signal and returns exact quiescent custody.
    pub fn recycle(
        mut self,
    ) -> Result<ServiceRecycledQueueSessionV1<N>, ServiceQueueOperationFailureV1> {
        match self.owner.queue.recycle_fixed_dispatch(self.completed) {
            Ok(observation) => match self.owner.queue.recycled_fixed_dispatch_generation() {
                Ok(dispatch_generation) => Ok(ServiceRecycledQueueSessionV1 {
                    owner: self.owner,
                    recycle: observation,
                    dispatch_generation,
                    completed_read_attempted: false,
                }),
                Err(error) => Err(quarantine(self.owner, error)),
            },
            Err(error) => Err(quarantine(self.owner, error)),
        }
    }
}

/// Inert coherent read request bound to one exact recycled dispatch generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceCompletedReadRequestV1 {
    dispatch_generation: u64,
    range: ServiceHostDispatchRangeV1,
}

/// Inert exact enclosing-snapshot request bound to one recycled generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceCompletedSnapshotRequestV1 {
    dispatch_generation: u64,
    range: ServiceHostDispatchSnapshotRangeV1,
}

/// Owned bytes copied from one generation-checked coherent dispatch range.
#[derive(Debug, Eq, PartialEq)]
pub struct ServiceCompletedReadbackV1 {
    inner: Gfx942CompletedDispatchReadbackV1,
}

impl ServiceCompletedReadbackV1 {
    /// Returns the exact dispatch generation that authorized the copy.
    pub const fn dispatch_generation(&self) -> u64 {
        self.inner.dispatch_generation()
    }

    /// Returns the addressless dispatch-data ordinal.
    pub const fn data_index(&self) -> usize {
        self.inner.data_index()
    }

    /// Returns the byte offset within the retained allocation.
    pub const fn offset_bytes(&self) -> u64 {
        self.inner.offset()
    }

    /// Returns the owned byte copy.
    pub fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }
}

/// Exact completed-and-recycled custody of an attached fixed batch.
#[must_use = "recycled custody must be reused, detached, or explicitly released"]
pub struct ServiceRecycledQueueSessionV1<const N: usize> {
    owner: ServiceQueueOwnerV1,
    recycle: Gfx942CompletionRecycleObservationV1,
    dispatch_generation: u64,
    completed_read_attempted: bool,
}

/// Exact qualification-only transition at which a deliberate service fault is injected.
///
/// This is a service-host typestate boundary, not evidence that KFD or the GPU
/// produced a native fault. The API exists only with the
/// `qualification-fault-injection` feature.
#[cfg(feature = "qualification-fault-injection")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceQualificationQueueFaultPointV1 {
    /// Completion signals were recycled before any completed-read attempt.
    PostRecycleBeforeCompletedReadAttempt,
}

/// Terminal custody after a deliberate qualification queue-transition fault.
///
/// The ordinary recycled owner is consumed. This state exposes no readback,
/// reuse, or detach transition; the only native operation is exact queue and
/// allocation teardown. It does not claim that KFD or hardware faulted.
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQualificationFaultedQueueSessionV1;
/// fn read<const N: usize>(mut queue: ServiceQualificationFaultedQueueSessionV1<N>) {
///     let _ = queue.read_completed(todo!());
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQualificationFaultedQueueSessionV1;
/// fn reuse<const N: usize>(queue: ServiceQualificationFaultedQueueSessionV1<N>) {
///     let _ = queue.reuse();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQualificationFaultedQueueSessionV1;
/// fn detach<const N: usize>(queue: ServiceQualificationFaultedQueueSessionV1<N>) {
///     let _ = queue.detach();
/// }
/// ```
#[cfg(feature = "qualification-fault-injection")]
#[must_use = "the deliberately faulted queue must be destroyed or retained"]
pub struct ServiceQualificationFaultedQueueSessionV1<const N: usize> {
    owner: ServiceQueueOwnerV1,
    recycle: Gfx942CompletionRecycleObservationV1,
    dispatch_generation: u64,
    point: ServiceQualificationQueueFaultPointV1,
}

#[cfg(feature = "qualification-fault-injection")]
impl<const N: usize> fmt::Debug for ServiceQualificationFaultedQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQualificationFaultedQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.owner.observation())
            .field("recycle", &self.recycle)
            .field("dispatch_generation", &self.dispatch_generation)
            .field("point", &self.point)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "qualification-fault-injection")]
impl<const N: usize> ServiceQualificationFaultedQueueSessionV1<N> {
    /// Returns the deliberate service transition that consumed recycled custody.
    pub const fn point(&self) -> ServiceQualificationQueueFaultPointV1 {
        self.point
    }

    /// Returns the exact recycled dispatch generation retained by the faulted state.
    pub const fn dispatch_generation(&self) -> u64 {
        self.dispatch_generation
    }

    /// Returns the exact lower-layer recycle observation preceding injection.
    pub const fn recycle_observation(&self) -> Gfx942CompletionRecycleObservationV1 {
        self.recycle
    }

    /// Destroys the real native queue and releases its exact allocation roster.
    ///
    /// This is ordinary returning teardown after a deliberate service-layer
    /// transition fault. It does not synthesize a KFD failure result.
    pub fn destroy_and_release(
        self,
    ) -> Result<ServiceQueueReleaseObservationV1, ServiceQueueReleaseFailureV1> {
        let ServiceQueueOwnerV1 { queue, ledger } = self.owner;
        let resources = queue
            .destroy_returning_fixed_dispatch_resources()
            .map_err(|error| {
                ServiceQueueReleaseFailureV1::Queue(ServiceQueueErrorV1::Kfd(error))
            })?;
        restore_and_release_queue_resources(ledger, resources)
    }
}

impl<const N: usize> fmt::Debug for ServiceRecycledQueueSessionV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceRecycledQueueSessionV1")
            .field("packet_count", &N)
            .field("queue", &self.owner.observation())
            .field("recycle", &self.recycle)
            .field("dispatch_generation", &self.dispatch_generation)
            .field("completed_read_attempted", &self.completed_read_attempted)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> ServiceRecycledQueueSessionV1<N> {
    /// Returns the exact lower-layer recycle observation.
    pub const fn recycle_observation(&self) -> Gfx942CompletionRecycleObservationV1 {
        self.recycle
    }

    /// Consumes recycled custody into a qualification-only terminal fault state.
    ///
    /// No KFD operation is performed and no native or hardware fault is claimed.
    /// The returned state denies readback, reuse, and detach by construction and
    /// permits only exact returning teardown. A prior completed-read attempt
    /// rejects injection and returns the unchanged recycled owner.
    ///
    /// ```compile_fail
    /// use fe2o3_service_host::{
    ///     ServiceQualificationQueueFaultPointV1, ServiceRecycledQueueSessionV1,
    /// };
    /// fn inject_twice<const N: usize>(queue: ServiceRecycledQueueSessionV1<N>) {
    ///     let _faulted = queue.inject_qualification_fault(
    ///         ServiceQualificationQueueFaultPointV1::PostRecycleBeforeCompletedReadAttempt,
    ///     );
    ///     let _again = queue.reuse();
    /// }
    /// ```
    #[cfg(feature = "qualification-fault-injection")]
    pub fn inject_qualification_fault(
        self,
        point: ServiceQualificationQueueFaultPointV1,
    ) -> Result<ServiceQualificationFaultedQueueSessionV1<N>, Box<Self>> {
        if self.completed_read_attempted {
            return Err(Box::new(self));
        }
        Ok(ServiceQualificationFaultedQueueSessionV1 {
            owner: self.owner,
            recycle: self.recycle,
            dispatch_generation: self.dispatch_generation,
            point,
        })
    }

    /// Creates a generation-bound inert request for one coherent allocation range.
    pub const fn completed_read_request(
        &self,
        range: ServiceHostDispatchRangeV1,
    ) -> ServiceCompletedReadRequestV1 {
        ServiceCompletedReadRequestV1 {
            dispatch_generation: self.dispatch_generation,
            range,
        }
    }

    /// Creates a generation-bound request for one admitted enclosing snapshot.
    pub const fn completed_snapshot_request(
        &self,
        range: ServiceHostDispatchSnapshotRangeV1,
    ) -> ServiceCompletedSnapshotRequestV1 {
        ServiceCompletedSnapshotRequestV1 {
            dispatch_generation: self.dispatch_generation,
            range,
        }
    }

    /// Copies one exact inspected writable range after completion and recycle.
    pub fn read_completed(
        &mut self,
        request: ServiceCompletedReadRequestV1,
    ) -> Result<ServiceCompletedReadbackV1, ServiceQueueErrorV1> {
        self.completed_read_attempted = true;
        if request.dispatch_generation != self.dispatch_generation {
            return Err(ServiceQueueErrorV1::Kfd(
                fe2o3_kfd::Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
            ));
        }
        self.owner
            .ledger
            .validate_range(ServiceDispatchRangeV1::HostVisible(request.range))
            .map_err(ServiceQueueErrorV1::Allocation)?;
        let readback = self
            .owner
            .queue
            .read_recycled_fixed_dispatch_data(Gfx942CompletedDispatchReadRequestV1::new(
                request.dispatch_generation,
                request.range.data_index,
                request.range.offset_bytes,
                request.range.extent_bytes,
            ))
            .map_err(ServiceQueueErrorV1::Kfd)?;
        Ok(ServiceCompletedReadbackV1 { inner: readback })
    }

    /// Copies one exact admitted enclosing snapshot after completion and recycle.
    pub fn read_completed_snapshot(
        &mut self,
        request: ServiceCompletedSnapshotRequestV1,
    ) -> Result<ServiceCompletedReadbackV1, ServiceQueueErrorV1> {
        self.completed_read_attempted = true;
        if request.dispatch_generation != self.dispatch_generation {
            return Err(ServiceQueueErrorV1::Kfd(
                fe2o3_kfd::Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
            ));
        }
        let range = request.range.dispatch_range();
        self.owner
            .ledger
            .validate_range(ServiceDispatchRangeV1::HostVisible(range))
            .map_err(ServiceQueueErrorV1::Allocation)?;
        let readback = self
            .owner
            .queue
            .read_recycled_fixed_dispatch_snapshot(Gfx942CompletedDispatchSnapshotRequestV1::new(
                request.dispatch_generation,
                range.data_index,
                range.offset_bytes,
                range.extent_bytes,
            ))
            .map_err(ServiceQueueErrorV1::Kfd)?;
        Ok(ServiceCompletedReadbackV1 { inner: readback })
    }

    /// Reuses the same attached batch without rebuilding queue resources.
    pub fn reuse(self) -> ServiceQueueSessionV1<N> {
        ServiceQueueSessionV1 { owner: self.owner }
    }

    /// Detaches data custody after exact completion and recycle while keeping the queue live.
    pub fn detach(
        mut self,
    ) -> Result<ServiceQueueUnboundSessionV1, ServiceQueueOperationFailureV1> {
        match self.owner.queue.detach_recycled_fixed_dispatch() {
            Ok(detached) => Ok(ServiceQueueUnboundSessionV1 {
                owner: self.owner,
                dispatch_generation: detached.dispatch_generation(),
                data: detached.into_data(),
            }),
            Err(error) => Err(quarantine(self.owner, error)),
        }
    }

    /// Destroys the queue, restores exact allocation custody, and releases all storage.
    pub fn destroy_and_release(
        self,
    ) -> Result<ServiceQueueReleaseObservationV1, ServiceQueueReleaseFailureV1> {
        let ServiceQueueOwnerV1 { queue, ledger } = self.owner;
        let resources = queue
            .destroy_returning_fixed_dispatch_resources()
            .map_err(|error| {
                ServiceQueueReleaseFailureV1::Queue(ServiceQueueErrorV1::Kfd(error))
            })?;
        restore_and_release_queue_resources(ledger, resources)
    }
}

/// A live native queue with no attached executable, kernarg, or packet batch.
///
/// Queue ring, completion-signal, event, runtime, and doorbell resources remain
/// owned and live. A compatible replacement batch may consume the detached
/// dispatch-data allocations and reattach them, or exact returning teardown may
/// destroy the queue and release the allocations.
///
/// The unbound state cannot be fabricated from a pre-recycle queue:
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQueueSessionV1;
///
/// fn release_before_recycle(queue: ServiceQueueSessionV1<1>) {
///     let _ = queue.destroy_and_release();
/// }
/// ```
///
/// Its private detached-data and generation ledgers cannot be fabricated:
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceQueueUnboundSessionV1;
///
/// fn fabricate() -> ServiceQueueUnboundSessionV1 {
///     ServiceQueueUnboundSessionV1 {
///         owner: todo!(),
///         dispatch_generation: 1,
///         data: Vec::new(),
///     }
/// }
/// ```
#[must_use = "the unbound live queue must be rebound, destroyed, or quarantined"]
pub struct ServiceQueueUnboundSessionV1 {
    owner: ServiceQueueOwnerV1,
    dispatch_generation: u64,
    data: Vec<Gfx942FixedDispatchDataV1>,
}

/// Fresh queue, logical-partition, and member-range custody after one detached
/// initialized device allocation is replaced.
///
/// This owner intentionally does not implement `Clone`. The queue and new
/// partition generation must move together until the caller explicitly
/// separates them for replacement-batch construction.
#[must_use = "the live queue and replacement partition must remain retained"]
pub struct ServiceQueuePartitionedDataUpdateV1<R, const N: usize>
where
    R: DeviceAllocationRoleMarkerV1,
{
    queue: ServiceQueueUnboundSessionV1,
    subleases: ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
    ranges: [ServiceDeviceDispatchRangeV1; N],
}

impl<R, const N: usize> ServiceQueuePartitionedDataUpdateV1<R, N>
where
    R: DeviceAllocationRoleMarkerV1,
{
    /// Separates the still-live queue, fresh partition witness, and exact
    /// addressless member ranges.
    pub fn into_parts(
        self,
    ) -> (
        ServiceQueueUnboundSessionV1,
        ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
        [ServiceDeviceDispatchRangeV1; N],
    ) {
        (self.queue, self.subleases, self.ranges)
    }
}

impl<R, const N: usize> fmt::Debug for ServiceQueuePartitionedDataUpdateV1<R, N>
where
    R: DeviceAllocationRoleMarkerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueuePartitionedDataUpdateV1")
            .field("queue", &self.queue)
            .field("subleases", &self.subleases)
            .field("member_count", &N)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ServiceQueueUnboundSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueueUnboundSessionV1")
            .field("queue", &self.owner.observation())
            .field("dispatch_generation", &self.dispatch_generation)
            .field("data_lease_count", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl ServiceQueueUnboundSessionV1 {
    /// Returns the completed dispatch generation that authorized detachment.
    pub const fn detached_dispatch_generation(&self) -> u64 {
        self.dispatch_generation
    }

    /// Returns a redacted observation of the still-live native queue.
    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.owner.observation()
    }

    /// Destroys the live queue and releases its exact detached allocation set.
    ///
    /// The detached data vector and private KFD generation, cardinality, and
    /// ordered storage-identity ledgers are reunited by this consuming
    /// transition. A mismatch is terminal and does not return retry custody.
    ///
    /// ```compile_fail
    /// use fe2o3_service_host::ServiceQueueUnboundSessionV1;
    ///
    /// fn destroy_twice(queue: ServiceQueueUnboundSessionV1) {
    ///     let _first = queue.destroy_and_release();
    ///     let _second = queue.destroy_and_release();
    /// }
    /// ```
    pub fn destroy_and_release(
        self,
    ) -> Result<ServiceQueueReleaseObservationV1, ServiceQueueReleaseFailureV1> {
        let Self {
            owner,
            dispatch_generation: _,
            data,
        } = self;
        let ServiceQueueOwnerV1 { queue, ledger } = owner;
        let resources = queue
            .destroy_returning_detached_fixed_dispatch_resources(data)
            .map_err(|error| {
                ServiceQueueReleaseFailureV1::Queue(ServiceQueueErrorV1::Kfd(error))
            })?;
        restore_and_release_queue_resources(ledger, resources)
    }

    /// Rebinds a replacement fixed batch to the same live native queue.
    pub fn bind<'a, const M: usize>(
        mut self,
        batch: ServiceFixedBatchV1<'a, M>,
    ) -> Result<ServiceQueueSessionV1<M>, ServiceQueueBindFailureV1<'a, M>> {
        if let Err(error) = validate_ring::<M>(self.owner.observation().ring_bytes()) {
            return Err(ServiceQueueBindFailureV1::Rejected {
                error,
                queue: Box::new(self),
                batch: Box::new(batch),
            });
        }
        if let Err(error) = batch.validate(&self.owner.ledger) {
            return Err(ServiceQueueBindFailureV1::Rejected {
                error: ServiceQueueErrorV1::Allocation(error),
                queue: Box::new(self),
                batch: Box::new(batch),
            });
        }
        let (programs, packets) = batch.into_kfd();
        match self
            .owner
            .queue
            .bind_fixed_dispatch(programs, packets, self.data)
        {
            Ok(()) => Ok(ServiceQueueSessionV1 { owner: self.owner }),
            Err(error) => Err(ServiceQueueBindFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                retained: Box::new(QuarantinedServiceQueueV1 {
                    owner: self.owner,
                    detached_data: None,
                }),
            }),
        }
    }

    /// Destroys the quiescent native queue and creates a replacement queue
    /// with a newly admitted ring and fixed batch while retaining the exact
    /// mapped dispatch-data allocation set.
    ///
    /// Ring and batch validation complete before native destruction. A pure
    /// rejection therefore returns both unchanged inputs. Once destruction
    /// begins, any failure is terminal because native creation may have taken
    /// effect and no public owner can be reconstructed honestly.
    pub fn rollover<'a, const M: usize>(
        self,
        ring_bytes: u32,
        batch: ServiceFixedBatchV1<'a, M>,
    ) -> Result<ServiceQueueRolloverSuccessV1<M>, ServiceQueueRolloverFailureV1<'a, M>> {
        if let Err(error) = validate_ring::<M>(ring_bytes) {
            return Err(ServiceQueueRolloverFailureV1::Rejected {
                error,
                queue: Box::new(self),
                batch: Box::new(batch),
            });
        }
        if let Err(error) = batch.validate(&self.owner.ledger) {
            return Err(ServiceQueueRolloverFailureV1::Rejected {
                error: ServiceQueueErrorV1::Allocation(error),
                queue: Box::new(self),
                batch: Box::new(batch),
            });
        }
        let replacement_dispatch_generation = match self.dispatch_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                return Err(ServiceQueueRolloverFailureV1::Rejected {
                    error: ServiceQueueErrorV1::BatchContract(
                        "rollover dispatch generation exhaustion",
                    ),
                    queue: Box::new(self),
                    batch: Box::new(batch),
                });
            }
        };
        let Self {
            owner,
            dispatch_generation,
            data,
        } = self;
        let ServiceQueueOwnerV1 { queue, ledger } = owner;
        let resources = queue
            .destroy_returning_detached_fixed_dispatch_resources(data)
            .map_err(|error| ServiceQueueRolloverFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                previous_queue_destroyed: None,
                previous_dispatch_generation: dispatch_generation,
            })?;
        if resources.dispatch_generation() != dispatch_generation {
            return Err(ServiceQueueRolloverFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(ComputeAqlQueueSessionErrorV1::Contract(
                    "rollover dispatch generation",
                )),
                previous_queue_destroyed: Some(resources.destroyed()),
                previous_dispatch_generation: dispatch_generation,
            });
        }
        let previous_queue_destroyed = resources.destroyed();
        let (programs, packets) = batch.into_kfd();
        let queue = resources
            .recreate_compute_aql_queue_with_fixed_dispatch(ring_bytes, programs, packets)
            .map_err(|error| ServiceQueueRolloverFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                previous_queue_destroyed: Some(previous_queue_destroyed),
                previous_dispatch_generation: dispatch_generation,
            })?;
        let replacement_queue_observation = queue.observation();
        Ok(ServiceQueueRolloverSuccessV1 {
            queue: ServiceQueueSessionV1 {
                owner: ServiceQueueOwnerV1 { queue, ledger },
            },
            previous_queue_destroyed,
            previous_dispatch_generation: dispatch_generation,
            replacement_queue_observation,
            replacement_dispatch_generation,
        })
    }

    /// Replaces one complete detached allocation with newly verified device-local bytes.
    ///
    /// The old allocation is validated and released before the new allocation is
    /// installed at the same addressless data ordinal. The content descriptor is
    /// recomputed from the owned source bytes before any KFD transition. Success
    /// returns a new allocation generation; the supplied old range is stale.
    pub fn replace_initialized_device_local<R>(
        mut self,
        old: ServiceDeviceDispatchRangeV1,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<(Self, ServiceDeviceDispatchRangeV1), ServiceQueueDataUpdateFailureV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let observed = Gfx942DeviceContentDescriptorV1::from_bytes(content.role(), &bytes);
        if observed.is_err() || observed.as_ref().is_ok_and(|actual| actual != &content) {
            return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                error: ServiceQueueErrorV1::BatchContract("device content descriptor"),
                queue: Box::new(self),
            });
        }
        if old.data_index >= self.data.len() {
            return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                error: ServiceQueueErrorV1::Allocation(
                    ServiceAllocationErrorV1::AllocationGenerationMismatch,
                ),
                queue: Box::new(self),
            });
        }
        let extent_bytes = match u64::try_from(bytes.len()) {
            Ok(extent_bytes) => extent_bytes,
            Err(_) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(ServiceAllocationErrorV1::InvalidExtent),
                    queue: Box::new(self),
                });
            }
        };
        let replacement = match self.owner.ledger.prepare_initialized_replacement::<R>(
            old,
            extent_bytes,
            alignment,
        ) {
            Ok(replacement) => replacement,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    queue: Box::new(self),
                });
            }
        };
        let old_data = self.data.remove(old.data_index);
        if let Err(error) = self
            .owner
            .queue
            .release_detached_fixed_dispatch_data(old_data)
        {
            return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                retained: Box::new(QuarantinedServiceQueueV1 {
                    owner: self.owner,
                    detached_data: Some(self.data),
                }),
            });
        }
        self.owner.ledger.commit_replacement_release(&replacement);
        let data = match self
            .owner
            .queue
            .initialize_fixed_dispatch_data(bytes, alignment, content)
        {
            Ok(data) => data,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                    error: ServiceQueueErrorV1::Kfd(error),
                    retained: Box::new(QuarantinedServiceQueueV1 {
                        owner: self.owner,
                        detached_data: Some(self.data),
                    }),
                });
            }
        };
        self.data.insert(old.data_index, data);
        let range = self
            .owner
            .ledger
            .commit_initialized_replacement(replacement);
        Ok((self, range))
    }

    /// Replaces one detached partitioned device allocation with verified bytes
    /// and atomically installs a new exact logical partition at the same data
    /// ordinal.
    ///
    /// The borrowed old sublease witness is checked against the retained queue
    /// ledger before any native transition. The new layout is validated in full
    /// before the old allocation is released. Success returns a fresh move-only
    /// partition witness plus all exact addressless member ranges; every range
    /// from the old allocation generation is stale. The member cardinality and
    /// allocation extent may change across a long-lived queue rebind.
    pub fn replace_initialized_partitioned_device_local<R, const OLD_N: usize, const NEW_N: usize>(
        mut self,
        old: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, OLD_N>,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
        new_members: [(u64, u64, u64); NEW_N],
    ) -> Result<ServiceQueuePartitionedDataUpdateV1<R, NEW_N>, ServiceQueueDataUpdateFailureV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let observed = Gfx942DeviceContentDescriptorV1::from_bytes(content.role(), &bytes);
        if observed.is_err() || observed.as_ref().is_ok_and(|actual| actual != &content) {
            return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                error: ServiceQueueErrorV1::BatchContract("device content descriptor"),
                queue: Box::new(self),
            });
        }
        let extent_bytes = match u64::try_from(bytes.len()) {
            Ok(extent_bytes) => extent_bytes,
            Err(_) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(ServiceAllocationErrorV1::InvalidExtent),
                    queue: Box::new(self),
                });
            }
        };
        let (replacement, ranges) = match self
            .owner
            .ledger
            .prepare_initialized_partition_replacement::<R, OLD_N, NEW_N>(
                old,
                extent_bytes,
                alignment,
                new_members,
            ) {
            Ok(replacement) => replacement,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    queue: Box::new(self),
                });
            }
        };
        let data_index = replacement.data_index();
        let old_data = self.data.remove(data_index);
        if let Err(error) = self
            .owner
            .queue
            .release_detached_fixed_dispatch_data(old_data)
        {
            return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                retained: Box::new(QuarantinedServiceQueueV1 {
                    owner: self.owner,
                    detached_data: Some(self.data),
                }),
            });
        }
        self.owner.ledger.commit_replacement_release(&replacement);
        let data = match self
            .owner
            .queue
            .initialize_fixed_dispatch_data(bytes, alignment, content)
        {
            Ok(data) => data,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                    error: ServiceQueueErrorV1::Kfd(error),
                    retained: Box::new(QuarantinedServiceQueueV1 {
                        owner: self.owner,
                        detached_data: Some(self.data),
                    }),
                });
            }
        };
        self.data.insert(data_index, data);
        let (subleases, dispatch_ranges) = self
            .owner
            .ledger
            .commit_initialized_partitioned_replacement::<R, NEW_N>(replacement, ranges);
        Ok(ServiceQueuePartitionedDataUpdateV1 {
            queue: self,
            subleases,
            ranges: dispatch_ranges,
        })
    }

    /// Inserts one initialized partitioned device-local allocation before the
    /// retained host-visible data suffix.
    pub fn insert_initialized_partitioned_device_local<R, const N: usize>(
        mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
        members: [(u64, u64, u64); N],
    ) -> Result<ServiceQueuePartitionedDataUpdateV1<R, N>, ServiceQueueDataUpdateFailureV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let observed = Gfx942DeviceContentDescriptorV1::from_bytes(content.role(), &bytes);
        if observed.is_err() || observed.as_ref().is_ok_and(|actual| actual != &content) {
            return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                error: ServiceQueueErrorV1::BatchContract("device content descriptor"),
                queue: Box::new(self),
            });
        }
        let extent_bytes = match u64::try_from(bytes.len()) {
            Ok(extent_bytes) => extent_bytes,
            Err(_) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(ServiceAllocationErrorV1::InvalidExtent),
                    queue: Box::new(self),
                });
            }
        };
        if self.data.try_reserve(1).is_err() {
            return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                error: ServiceQueueErrorV1::Allocation(
                    ServiceAllocationErrorV1::AllocationRegistryReservation,
                ),
                queue: Box::new(self),
            });
        }
        let insertion = match self
            .owner
            .ledger
            .prepare_initialized_partition_insertion::<R, N>(extent_bytes, alignment, members)
        {
            Ok(insertion) => insertion,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    queue: Box::new(self),
                });
            }
        };
        let data_index = insertion.data_index();
        let data = match self
            .owner
            .queue
            .insert_initialized_fixed_dispatch_data(data_index, bytes, alignment, content)
        {
            Ok(data) => data,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                    error: ServiceQueueErrorV1::Kfd(error),
                    retained: Box::new(QuarantinedServiceQueueV1 {
                        owner: self.owner,
                        detached_data: Some(self.data),
                    }),
                });
            }
        };
        self.data.insert(data_index, data);
        let (subleases, ranges) = self
            .owner
            .ledger
            .commit_initialized_partition_insertion::<R, N>(insertion);
        Ok(ServiceQueuePartitionedDataUpdateV1 {
            queue: self,
            subleases,
            ranges,
        })
    }

    /// Removes and releases one complete partitioned device-local allocation.
    ///
    /// The supplied witness is borrowed for validation. On success it is stale
    /// and every future use is rejected by the advanced private ledger.
    pub fn remove_partitioned_device_local<R, const N: usize>(
        mut self,
        old: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
    ) -> Result<Self, ServiceQueueDataUpdateFailureV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let removal = match self.owner.ledger.prepare_partitioned_removal(old) {
            Ok(removal) => removal,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    queue: Box::new(self),
                });
            }
        };
        let data = self.data.remove(removal.data_index());
        if let Err(error) = self.owner.queue.release_detached_fixed_dispatch_data(data) {
            return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                retained: Box::new(QuarantinedServiceQueueV1 {
                    owner: self.owner,
                    detached_data: Some(self.data),
                }),
            });
        }
        self.owner.ledger.commit_partitioned_removal(removal);
        Ok(self)
    }

    /// Replaces one complete host-visible allocation with a fresh uninitialized
    /// mapped extent at the same detached data ordinal.
    pub fn replace_host_visible<R>(
        self,
        old: ServiceHostDispatchRangeV1,
        requested_bytes: usize,
    ) -> Result<ServiceQueueHostDataUpdateV1, ServiceQueueDataUpdateFailureV1>
    where
        R: crate::HostAllocationRoleMarkerV1,
    {
        self.replace_host_visible_inner::<R>(old, requested_bytes, None)
    }

    /// Replaces one complete host-visible allocation with exact initialized
    /// bytes and returns a fresh full-range snapshot witness.
    pub fn replace_initialized_host_visible<R>(
        self,
        old: ServiceHostDispatchRangeV1,
        bytes: Box<[u8]>,
    ) -> Result<ServiceQueueHostDataUpdateV1, ServiceQueueDataUpdateFailureV1>
    where
        R: crate::HostAllocationRoleMarkerV1,
    {
        self.replace_host_visible_inner::<R>(old, bytes.len(), Some(bytes))
    }

    fn replace_host_visible_inner<R>(
        mut self,
        old: ServiceHostDispatchRangeV1,
        requested_bytes: usize,
        initialized: Option<Box<[u8]>>,
    ) -> Result<ServiceQueueHostDataUpdateV1, ServiceQueueDataUpdateFailureV1>
    where
        R: crate::HostAllocationRoleMarkerV1,
    {
        let extent_bytes = match u64::try_from(requested_bytes) {
            Ok(extent_bytes) => extent_bytes,
            Err(_) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(ServiceAllocationErrorV1::InvalidExtent),
                    queue: Box::new(self),
                });
            }
        };
        let replacement = match self
            .owner
            .ledger
            .prepare_host_replacement::<R>(old, extent_bytes)
        {
            Ok(replacement) => replacement,
            Err(error) => {
                return Err(ServiceQueueDataUpdateFailureV1::Rejected {
                    error: ServiceQueueErrorV1::Allocation(error),
                    queue: Box::new(self),
                });
            }
        };
        let data_index = replacement.data_index();
        let old_data = self.data.remove(data_index);
        if let Err(error) = self
            .owner
            .queue
            .release_detached_fixed_dispatch_data(old_data)
        {
            return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                error: ServiceQueueErrorV1::Kfd(error),
                retained: Box::new(QuarantinedServiceQueueV1 {
                    owner: self.owner,
                    detached_data: Some(self.data),
                }),
            });
        }
        self.owner
            .ledger
            .commit_host_replacement_release(&replacement);
        let (data, initialized) = match initialized {
            Some(bytes) => match self
                .owner
                .queue
                .initialize_host_visible_fixed_dispatch_data(bytes)
            {
                Ok(data) => (data, true),
                Err(error) => {
                    return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                        error: ServiceQueueErrorV1::Kfd(error),
                        retained: Box::new(QuarantinedServiceQueueV1 {
                            owner: self.owner,
                            detached_data: Some(self.data),
                        }),
                    });
                }
            },
            None => match self
                .owner
                .queue
                .allocate_host_visible_fixed_dispatch_data(requested_bytes)
            {
                Ok(data) => (data, false),
                Err(error) => {
                    return Err(ServiceQueueDataUpdateFailureV1::Terminal {
                        error: ServiceQueueErrorV1::Kfd(error),
                        retained: Box::new(QuarantinedServiceQueueV1 {
                            owner: self.owner,
                            detached_data: Some(self.data),
                        }),
                    });
                }
            },
        };
        self.data.insert(data_index, data);
        let range = self.owner.ledger.commit_host_replacement(replacement);
        let snapshot =
            initialized.then(|| ServiceHostDispatchSnapshotRangeV1::from_initialized_range(range));
        Ok(ServiceQueueHostDataUpdateV1 {
            queue: self,
            range,
            snapshot,
        })
    }
}

/// Fresh queue and host-visible range custody after detached replacement.
#[must_use = "the live queue and fresh host allocation range must remain retained"]
pub struct ServiceQueueHostDataUpdateV1 {
    queue: ServiceQueueUnboundSessionV1,
    range: ServiceHostDispatchRangeV1,
    snapshot: Option<ServiceHostDispatchSnapshotRangeV1>,
}

impl ServiceQueueHostDataUpdateV1 {
    /// Separates the live queue, fresh complete range, and optional initialized snapshot.
    pub fn into_parts(
        self,
    ) -> (
        ServiceQueueUnboundSessionV1,
        ServiceHostDispatchRangeV1,
        Option<ServiceHostDispatchSnapshotRangeV1>,
    ) {
        (self.queue, self.range, self.snapshot)
    }
}

impl fmt::Debug for ServiceQueueHostDataUpdateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueueHostDataUpdateV1")
            .field("queue", &self.queue)
            .field("range", &self.range)
            .field("has_snapshot", &self.snapshot.is_some())
            .finish_non_exhaustive()
    }
}

/// Successful quiescent queue rollover with confirmed predecessor teardown.
#[must_use = "the replacement live queue requires an explicit linear transition"]
pub struct ServiceQueueRolloverSuccessV1<const N: usize> {
    queue: ServiceQueueSessionV1<N>,
    previous_queue_destroyed: ComputeAqlQueueDestroyedV1,
    previous_dispatch_generation: u64,
    replacement_queue_observation: ComputeAqlQueueObservationV1,
    replacement_dispatch_generation: u64,
}

impl<const N: usize> ServiceQueueRolloverSuccessV1<N> {
    /// Returns confirmed destruction of the predecessor native queue.
    pub const fn previous_queue_destroyed(&self) -> ComputeAqlQueueDestroyedV1 {
        self.previous_queue_destroyed
    }

    /// Returns the exact recycled generation that authorized rollover.
    pub const fn previous_dispatch_generation(&self) -> u64 {
        self.previous_dispatch_generation
    }

    /// Returns the prepared replacement native queue observation.
    pub const fn replacement_queue_observation(&self) -> ComputeAqlQueueObservationV1 {
        self.replacement_queue_observation
    }

    /// Returns the dispatch generation prepared for the replacement queue.
    pub const fn replacement_dispatch_generation(&self) -> u64 {
        self.replacement_dispatch_generation
    }

    /// Consumes rollover evidence into the prepared replacement queue.
    pub fn into_queue(self) -> ServiceQueueSessionV1<N> {
        self.queue
    }
}

impl<const N: usize> fmt::Debug for ServiceQueueRolloverSuccessV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceQueueRolloverSuccessV1")
            .field("previous_queue_destroyed", &self.previous_queue_destroyed)
            .field(
                "previous_dispatch_generation",
                &self.previous_dispatch_generation,
            )
            .field(
                "replacement_queue_observation",
                &self.replacement_queue_observation,
            )
            .field(
                "replacement_dispatch_generation",
                &self.replacement_dispatch_generation,
            )
            .field("replacement_queue", &self.queue)
            .finish()
    }
}

/// Quiescent queue-rollover rejection or terminal native transition failure.
#[must_use = "pure rejection retains both inputs; terminal failure requires process teardown"]
pub enum ServiceQueueRolloverFailureV1<'a, const N: usize> {
    /// Validation rejected the replacement before native queue destruction.
    Rejected {
        /// Exact rejection.
        error: ServiceQueueErrorV1,
        /// Unchanged detached queue owner.
        queue: Box<ServiceQueueUnboundSessionV1>,
        /// Unchanged replacement batch.
        batch: Box<ServiceFixedBatchV1<'a, N>>,
    },
    /// Native destruction or replacement creation consumed the inputs.
    Terminal {
        /// Exact lower-layer error.
        error: ServiceQueueErrorV1,
        /// Confirmed predecessor destruction, when rollover reached that boundary.
        previous_queue_destroyed: Option<ComputeAqlQueueDestroyedV1>,
        /// Exact recycled predecessor dispatch generation.
        previous_dispatch_generation: u64,
    },
}

impl<const N: usize> fmt::Debug for ServiceQueueRolloverFailureV1<'_, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { error, .. } => formatter
                .debug_struct("Rejected")
                .field("error", error)
                .finish_non_exhaustive(),
            Self::Terminal {
                error,
                previous_queue_destroyed,
                previous_dispatch_generation,
            } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .field("previous_queue_destroyed", previous_queue_destroyed)
                .field("previous_dispatch_generation", previous_dispatch_generation)
                .finish(),
        }
    }
}

impl<'a, const N: usize> ServiceQueueRolloverFailureV1<'a, N> {
    /// Returns the exact error without discarding retained rejection inputs.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        match self {
            Self::Rejected { error, .. } | Self::Terminal { error, .. } => error,
        }
    }

    /// Returns confirmed predecessor destruction for a post-destroy terminal failure.
    pub const fn previous_queue_destroyed(&self) -> Option<ComputeAqlQueueDestroyedV1> {
        match self {
            Self::Rejected { .. } => None,
            Self::Terminal {
                previous_queue_destroyed,
                ..
            } => *previous_queue_destroyed,
        }
    }

    /// Returns the recycled predecessor generation once native rollover began.
    pub const fn previous_dispatch_generation(&self) -> Option<u64> {
        match self {
            Self::Rejected { .. } => None,
            Self::Terminal {
                previous_dispatch_generation,
                ..
            } => Some(*previous_dispatch_generation),
        }
    }

    /// Recovers both unchanged inputs only after pure preflight rejection.
    pub fn into_rejected_inputs(
        self,
    ) -> Option<(ServiceQueueUnboundSessionV1, ServiceFixedBatchV1<'a, N>)> {
        match self {
            Self::Rejected { queue, batch, .. } => Some((*queue, *batch)),
            Self::Terminal { .. } => None,
        }
    }
}

/// Detached-data replacement rejection or terminal native transition failure.
#[must_use = "pure rejection retains the unbound queue; terminal failure retains quarantine"]
pub enum ServiceQueueDataUpdateFailureV1 {
    /// Validation rejected the replacement before releasing the old allocation.
    Rejected {
        /// Exact rejection.
        error: ServiceQueueErrorV1,
        /// Unchanged unbound queue owner and detached data.
        queue: Box<ServiceQueueUnboundSessionV1>,
    },
    /// KFD consumed an allocation transition and retry is forbidden.
    Terminal {
        /// Exact lower-layer failure.
        error: ServiceQueueErrorV1,
        /// Opaque retained queue and remaining detached data custody.
        retained: Box<QuarantinedServiceQueueV1>,
    },
}

impl fmt::Debug for ServiceQueueDataUpdateFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { error, .. } => formatter
                .debug_struct("Rejected")
                .field("error", error)
                .finish_non_exhaustive(),
            Self::Terminal { error, retained } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .field("retained", retained)
                .finish(),
        }
    }
}

impl ServiceQueueDataUpdateFailureV1 {
    /// Returns the exact failure without discarding retained ownership.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        match self {
            Self::Rejected { error, .. } | Self::Terminal { error, .. } => error,
        }
    }

    /// Recovers the unchanged queue only after pure validation rejection.
    pub fn into_rejected_queue(self) -> Option<ServiceQueueUnboundSessionV1> {
        match self {
            Self::Rejected { queue, .. } => Some(*queue),
            Self::Terminal { .. } => None,
        }
    }

    /// Recovers opaque quarantine only after a terminal KFD transition.
    pub fn into_quarantined(self) -> Option<QuarantinedServiceQueueV1> {
        match self {
            Self::Rejected { .. } => None,
            Self::Terminal { retained, .. } => Some(*retained),
        }
    }
}

/// Replacement-batch rejection or terminal KFD rebind failure.
#[must_use = "pure rejection retains the queue and batch; terminal failure retains quarantine"]
pub enum ServiceQueueBindFailureV1<'a, const N: usize> {
    /// Pure validation rejected the replacement before consuming detached data.
    Rejected {
        /// Exact rejection.
        error: ServiceQueueErrorV1,
        /// Unchanged unbound queue owner.
        queue: Box<ServiceQueueUnboundSessionV1>,
        /// Unchanged replacement batch.
        batch: Box<ServiceFixedBatchV1<'a, N>>,
    },
    /// KFD consumed detached data while attempting the replacement binding.
    Terminal {
        /// Exact lower-layer failure.
        error: ServiceQueueErrorV1,
        /// Opaque retained queue owner with retry denied.
        retained: Box<QuarantinedServiceQueueV1>,
    },
}

impl<const N: usize> fmt::Debug for ServiceQueueBindFailureV1<'_, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { error, .. } => formatter
                .debug_struct("Rejected")
                .field("error", error)
                .finish_non_exhaustive(),
            Self::Terminal { error, retained } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .field("retained", retained)
                .finish(),
        }
    }
}

impl<'a, const N: usize> ServiceQueueBindFailureV1<'a, N> {
    /// Returns the exact error without discarding retained ownership.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        match self {
            Self::Rejected { error, .. } | Self::Terminal { error, .. } => error,
        }
    }

    /// Recovers the unchanged queue and batch only for pure validation rejection.
    pub fn into_rejected_inputs(
        self,
    ) -> Option<(ServiceQueueUnboundSessionV1, ServiceFixedBatchV1<'a, N>)> {
        match self {
            Self::Rejected { queue, batch, .. } => Some((*queue, *batch)),
            Self::Terminal { .. } => None,
        }
    }

    /// Recovers opaque quarantine only after a terminal KFD failure.
    pub fn into_quarantined(self) -> Option<QuarantinedServiceQueueV1> {
        match self {
            Self::Rejected { .. } => None,
            Self::Terminal { retained, .. } => Some(*retained),
        }
    }
}

/// Redacted evidence of queue destruction and allocation release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceQueueReleaseObservationV1 {
    destroyed: ComputeAqlQueueDestroyedV1,
    dispatch_generation: u64,
    allocations: ServiceAllocationReleaseObservationV1,
}

impl ServiceQueueReleaseObservationV1 {
    /// Returns confirmed native queue-destruction evidence.
    pub const fn queue_destroyed(self) -> ComputeAqlQueueDestroyedV1 {
        self.destroyed
    }

    /// Returns the exact recycled generation authorizing data return.
    pub const fn dispatch_generation(self) -> u64 {
        self.dispatch_generation
    }

    /// Returns confirmed service allocation-release evidence.
    pub const fn allocations_released(self) -> ServiceAllocationReleaseObservationV1 {
        self.allocations
    }
}

/// Opaque device and session custody retained after ledger restoration failed.
#[must_use = "ambiguous restored resources must remain quarantined"]
pub struct QuarantinedServiceQueueResourcesV1 {
    failure: ServiceQueueAllocationRestoreFailureV1,
}

impl fmt::Debug for QuarantinedServiceQueueResourcesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuarantinedServiceQueueResourcesV1")
            .field("error", &self.failure.error)
            .field("data_lease_count", &self.failure.data.len())
            .field(
                "ledger_data_count",
                &self.failure.ledger.device_allocation_count(),
            )
            .field("session_phase", &self.failure.session.phase())
            .finish_non_exhaustive()
    }
}

/// Consuming queue teardown failure.
#[must_use = "teardown failure may retain opaque allocation or queue resources"]
pub enum ServiceQueueReleaseFailureV1 {
    /// Native queue destruction failed after consuming the public queue owner.
    Queue(ServiceQueueErrorV1),
    /// Returned KFD data did not match the retained private service ledger.
    Restore(Box<QuarantinedServiceQueueResourcesV1>),
    /// Queue destruction succeeded, but allocation release failed and retained quarantine.
    Allocation(Box<ServiceAllocationReleaseFailureV1>),
}

impl fmt::Debug for ServiceQueueReleaseFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queue(error) => formatter.debug_tuple("Queue").field(error).finish(),
            Self::Restore(retained) => formatter.debug_tuple("Restore").field(retained).finish(),
            Self::Allocation(failure) => {
                formatter.debug_tuple("Allocation").field(failure).finish()
            }
        }
    }
}

impl ServiceQueueReleaseFailureV1 {
    /// Recovers allocation quarantine when native destruction succeeded but release failed.
    pub fn into_quarantined_allocations(self) -> Option<QuarantinedServiceAllocationsV1> {
        match self {
            Self::Allocation(failure) => Some(failure.into_retained()),
            Self::Queue(_) | Self::Restore(_) => None,
        }
    }
}

fn restore_and_release_queue_resources(
    ledger: ServiceQueueAllocationLedgerV1,
    resources: Gfx942RecycledDispatchResourcesV1,
) -> Result<ServiceQueueReleaseObservationV1, ServiceQueueReleaseFailureV1> {
    let destroyed = resources.destroyed();
    let dispatch_generation = resources.dispatch_generation();
    let (session, data) = resources.into_session_and_data();
    let allocations = ledger.restore(session, data).map_err(|failure| {
        ServiceQueueReleaseFailureV1::Restore(Box::new(QuarantinedServiceQueueResourcesV1 {
            failure,
        }))
    })?;
    let allocations = allocations
        .release_quiescent()
        .map_err(|failure| ServiceQueueReleaseFailureV1::Allocation(Box::new(failure)))?;
    Ok(ServiceQueueReleaseObservationV1 {
        destroyed,
        dispatch_generation,
        allocations,
    })
}

fn validate_ring<const N: usize>(ring_bytes: u32) -> Result<(), ServiceQueueErrorV1> {
    if N == 0 || N > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
        return Err(ServiceQueueErrorV1::BatchContract(
            "fixed batch packet count",
        ));
    }
    let capacity = AqlRingCapacityV1::from_ring_bytes(ring_bytes)
        .map_err(|_| ServiceQueueErrorV1::BatchContract("AQL ring byte capacity"))?;
    if N > capacity.packets() as usize {
        return Err(ServiceQueueErrorV1::BatchContract(
            "fixed batch exceeds AQL ring capacity",
        ));
    }
    Ok(())
}

fn quarantine(
    owner: ServiceQueueOwnerV1,
    error: ComputeAqlQueueSessionErrorV1,
) -> ServiceQueueOperationFailureV1 {
    ServiceQueueOperationFailureV1 {
        error: ServiceQueueErrorV1::Kfd(error),
        retained: Box::new(QuarantinedServiceQueueV1 {
            owner,
            detached_data: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;
    use core::fmt::Write as _;
    use sha2::{Digest, Sha256};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestQueuePhaseV1 {
        Prepared,
        Published(u64),
        Recycled(u64),
        Unbound(u64),
        Destroyed,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct TestBatchDescriptionV1 {
        packet_count: usize,
        program_identity: [u8; 32],
        geometry: fe2o3_aql::AqlDispatchGeometryV1,
        kernarg_scalar_bytes: Box<[u8]>,
        data_generation: u64,
    }

    struct TestLongLivedQueueV1 {
        queue_identity: u64,
        phase: TestQueuePhaseV1,
        next_generation: u64,
        publication_count: u64,
        batch: Option<TestBatchDescriptionV1>,
    }

    impl TestLongLivedQueueV1 {
        fn submit(&mut self) -> u64 {
            assert_eq!(self.phase, TestQueuePhaseV1::Prepared);
            assert!(self.batch.is_some());
            let generation = self.next_generation;
            self.next_generation += 1;
            self.publication_count += 1;
            self.phase = TestQueuePhaseV1::Published(generation);
            generation
        }

        fn recycle(&mut self, generation: u64) {
            assert_eq!(self.phase, TestQueuePhaseV1::Published(generation));
            self.phase = TestQueuePhaseV1::Recycled(generation);
        }

        fn detach(&mut self) -> Option<TestBatchDescriptionV1> {
            let TestQueuePhaseV1::Recycled(generation) = self.phase else {
                return None;
            };
            self.phase = TestQueuePhaseV1::Unbound(generation);
            self.batch.take()
        }

        fn bind(&mut self, batch: TestBatchDescriptionV1) {
            assert!(matches!(self.phase, TestQueuePhaseV1::Unbound(_)));
            self.batch = Some(batch);
            self.phase = TestQueuePhaseV1::Prepared;
        }

        fn destroy_unbound(&mut self, data: TestBatchDescriptionV1) -> Option<u64> {
            let TestQueuePhaseV1::Unbound(generation) = self.phase else {
                return None;
            };
            assert!(self.batch.is_none());
            let _returned_data = data;
            self.phase = TestQueuePhaseV1::Destroyed;
            Some(generation)
        }
    }

    #[test]
    fn fixed_batch_ring_preflight_covers_large_single_publications() {
        assert!(validate_ring::<1>(4_096).is_ok());
        assert!(validate_ring::<1024>(65_536).is_ok());
        assert!(validate_ring::<8192>(524_288).is_ok());
        assert!(matches!(
            validate_ring::<0>(524_288),
            Err(ServiceQueueErrorV1::BatchContract(_))
        ));
        assert!(matches!(
            validate_ring::<8193>(1_048_576),
            Err(ServiceQueueErrorV1::BatchContract(_))
        ));
        assert!(matches!(
            validate_ring::<65>(4_096),
            Err(ServiceQueueErrorV1::BatchContract(_))
        ));
        assert!(matches!(
            validate_ring::<8192>(262_144),
            Err(ServiceQueueErrorV1::BatchContract(_))
        ));
    }

    #[test]
    fn queue_manifest_hash_is_frozen() {
        assert!(
            SERVICE_QUEUE_OWNERSHIP_MANIFEST_V1.contains(&alloc::format!(
                "source.compute_aql_session_sha256={}\n",
                fe2o3_kfd::GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1
            ))
        );
        let mut actual = String::new();
        for byte in Sha256::digest(SERVICE_QUEUE_OWNERSHIP_MANIFEST_V1) {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, SERVICE_QUEUE_OWNERSHIP_MANIFEST_SHA256_V1);
    }

    #[cfg(feature = "qualification-fault-injection")]
    #[test]
    fn qualification_fault_contract_hash_is_frozen() {
        let mut actual = String::new();
        for byte in Sha256::digest(SERVICE_QUALIFICATION_QUEUE_FAULT_CONTRACT_V1) {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, SERVICE_QUALIFICATION_QUEUE_FAULT_CONTRACT_SHA256_V1);
    }

    #[test]
    fn timeout_observation_is_absent_from_non_timeout_errors() {
        assert_eq!(
            timeout_observation(&ServiceQueueErrorV1::BatchContract("not timeout")),
            None
        );
    }

    #[test]
    fn two_generations_rebind_every_batch_input_without_recreating_the_queue() {
        let first = TestBatchDescriptionV1 {
            packet_count: 3,
            program_identity: [1; 32],
            geometry: fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            kernarg_scalar_bytes: vec![1, 2, 3].into_boxed_slice(),
            data_generation: 11,
        };
        let second = TestBatchDescriptionV1 {
            packet_count: 5,
            program_identity: [2; 32],
            geometry: fe2o3_aql::AqlDispatchGeometryV1::new([128, 2, 1], [64, 1, 1]).unwrap(),
            kernarg_scalar_bytes: vec![9, 8, 7, 6].into_boxed_slice(),
            data_generation: 12,
        };
        assert_ne!(first, second);

        let mut queue = TestLongLivedQueueV1 {
            queue_identity: 41,
            phase: TestQueuePhaseV1::Prepared,
            next_generation: 1,
            publication_count: 0,
            batch: Some(first),
        };
        assert!(queue.detach().is_none());
        let first_generation = queue.submit();
        assert!(queue.detach().is_none());
        queue.recycle(first_generation);
        let detached_first = queue.detach().unwrap();
        queue.bind(second);
        let second_generation = queue.submit();
        queue.recycle(second_generation);

        assert_eq!(queue.queue_identity, 41);
        assert_eq!(first_generation, 1);
        assert_eq!(second_generation, 2);
        assert_eq!(queue.publication_count, 2);
        assert_eq!(detached_first.packet_count, 3);
        let current = queue.batch.as_ref().unwrap();
        assert_eq!(current.packet_count, 5);
        assert_eq!(current.program_identity, [2; 32]);
        assert_eq!(current.geometry.grid(), [128, 2, 1]);
        assert_eq!(&*current.kernarg_scalar_bytes, &[9, 8, 7, 6]);
        assert_eq!(current.data_generation, 12);
    }

    #[test]
    fn unbound_teardown_returns_the_detached_generation_once() {
        let batch = TestBatchDescriptionV1 {
            packet_count: 1,
            program_identity: [7; 32],
            geometry: fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            kernarg_scalar_bytes: vec![3, 1, 4].into_boxed_slice(),
            data_generation: 23,
        };
        let mut queue = TestLongLivedQueueV1 {
            queue_identity: 52,
            phase: TestQueuePhaseV1::Prepared,
            next_generation: 9,
            publication_count: 0,
            batch: Some(batch),
        };

        let generation = queue.submit();
        assert_eq!(generation, 9);
        assert_eq!(
            queue.destroy_unbound(TestBatchDescriptionV1 {
                packet_count: 0,
                program_identity: [0; 32],
                geometry: fe2o3_aql::AqlDispatchGeometryV1::new([1, 1, 1], [1, 1, 1]).unwrap(),
                kernarg_scalar_bytes: Box::new([]),
                data_generation: 0,
            }),
            None
        );
        queue.recycle(generation);
        let detached = queue.detach().unwrap();
        assert_eq!(queue.destroy_unbound(detached), Some(generation));
        assert!(matches!(queue.phase, TestQueuePhaseV1::Destroyed));
    }
}
