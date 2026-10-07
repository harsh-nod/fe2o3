//! Private dispatch binding for the retained gfx942 compute-AQL queue.
//!
//! This module closes host-side identity, layout, ownership, and lifetime
//! composition. It deliberately has no public constructor or submission
//! method: device-data initialization/effect premises and native execution
//! semantics remain reviewed integration obligations.

#![allow(dead_code)]

include!("queue_dispatch_binding/epoch_cancel_body.rs");
include!("queue_dispatch_binding/epoch_reserve_body.rs");
include!("queue_dispatch_binding/cancel_binding_body.rs");
include!("queue_dispatch_binding/template_prepare_body.rs");
include!("queue_dispatch_binding/template_preflight_body.rs");
include!("queue_dispatch_binding/template_bind_body.rs");

macro_rules! dispatch_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[path = "queue_dispatch_binding/control_release.rs"]
pub(crate) mod control_release;

#[path = "queue_dispatch_binding/preparation.rs"]
pub(crate) mod preparation;
pub(crate) use preparation::FixedDispatchPreparationCustodyV1;

#[path = "queue_dispatch_binding/conditional_fill.rs"]
mod conditional_fill;
#[path = "queue_dispatch_binding/native_fill_arena.rs"]
mod native_fill_arena;
#[path = "queue_dispatch_binding/native_fill_cohort.rs"]
mod native_fill_cohort;
#[path = "queue_dispatch_binding/native_fill_registry.rs"]
mod native_fill_registry;
pub(super) use native_fill_arena::{ArenaOrderV1, ArenaRecipeV1};
pub use native_fill_arena::{
    GFX942_NATIVE_FILL_ARENA_SLOTS_V1, Gfx942IndependentFillArenaInputsV1,
    Gfx942NativeFillArenaFailureV1, Gfx942NativeFillArenaInputsV1, Gfx942NativeFillArenaPacketsV1,
    Gfx942NativeFillArenaStorageV1,
};
pub use native_fill_cohort::{
    Gfx942NativeFillCohortFailureV1, Gfx942NativeFillCohortMemberV1, Gfx942NativeFillCohortV1,
};
pub(super) use native_fill_registry::RegistryRecipeV1;
pub use native_fill_registry::{
    Gfx942NativeFillRegistryInputsV1, Gfx942NativeFillRegistryRepeat2StorageV1,
    Gfx942NativeFillRegistryStorageV1, Gfx942NativeFillResidentRegistryInputsV1,
    Gfx942NativeFillResidentRegistryStorageV1,
};

#[path = "queue_dispatch_binding/generation_preflight.rs"]
mod generation_preflight;
pub use generation_preflight::Gfx942FixedDispatchPreallocationV1;
pub(in crate::queue) use generation_preflight::{
    DispatchGenerationSeedV1, Gfx942FixedDispatchPreallocationV1 as PreparedDispatchGenerationV1,
};

#[path = "queue_dispatch_binding/pristine_abort.rs"]
pub(crate) mod pristine_abort;
pub(super) use pristine_abort::{
    UnpublishedDispatchAbortV1, UnpublishedDispatchContinuationV1,
    prepare_public_fixed_dispatch_resources_after_unpublished_abort_in_place_v1,
};

#[cfg(test)]
#[path = "queue_dispatch_binding/capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
#[path = "queue_dispatch_binding/epoch_cancel_tests.rs"]
mod epoch_cancel_tests;

#[cfg(test)]
#[path = "queue_dispatch_binding/epoch_reserve_tests.rs"]
mod epoch_reserve_tests;

use core::fmt;
use fe2o3_resource_accounting::{HostMetadataTableV1, ResourceCreditAccountV1};
use std::sync::atomic::{AtomicU64, Ordering};

use arrayvec::ArrayVec;
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelIdentityInputsV1, ValidatedKernelEnvelope};
use fe2o3_aql::{
    AQL_MAX_FIXED_BATCH_PACKETS_V2, AqlDispatchGeometryV1, AqlDispatchOrderingV1,
    AqlRingCapacityV1, Cov6ImplicitDispatchShapeV1, ObservedGpuAddressV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, COV6_IMPLICIT_ARGUMENT_BYTES, ExplicitValueKind,
    HiddenValueKind, InspectedKernel,
};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};
use sha2::{Digest, Sha256};

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
use super::completion::completion_dispatch_roster_v1;
use super::completion::{
    CompletionBatchOccurrenceV1, CompletionDispatchGenerationBindingV1, CompletionDispatchRosterV1,
    CompletionPacketTemplateV1, Gfx942CompletedBatchV1, Gfx942CompletionBatchV1,
    Gfx942CompletionErrorV1, Gfx942CompletionPollV1, Gfx942CompletionPollWithProgressV1,
    Gfx942CompletionProgressV1, completion_template_dispatch_roster_v1,
};
use super::device_content::{Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1};
use crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;
use crate::MemorySessionError;
use crate::sdma::{Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferStorageV1};
use crate::shared_memory::{
    AqlDispatchCodeResourceRoleV1, AqlDispatchHostDataResourceRoleV1,
    AqlDispatchKernargResourceRoleV1, ExecutableGttV1, Gfx942DeviceMemoryDispatchAuthorityV1,
    Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    Gfx942InitializedDeviceMemoryV1, Gfx942InitializedHostVisibleMemoryV1,
    GttGpuAccessibleExecutableV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1,
    KernargGttV1, SharedGttAllocationIdentityV1, SharedGttAllocationV1, SharedGttMemorySessionV1,
    SharedGttQueueResourceAuthorityV1,
};

/// Maximum retained data allocations in one fixed-dispatch queue owner.
pub const GFX942_MAX_FIXED_DISPATCH_DATA_V1: usize = 16;
pub(crate) const MAX_DISPATCH_DATA_LEASES_V1: usize = GFX942_MAX_FIXED_DISPATCH_DATA_V1;
pub(crate) const MAX_DISPATCH_KERNARG_BYTES_V1: usize = 65_536;
pub const GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1: usize = 32;
pub const GFX942_MAX_FIXED_DISPATCH_PACKETS_V1: usize = AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize;
/// Default-profile maximum simultaneous accepted epochs over one immutable lane recipe.
pub const GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1: usize = 64;
/// Closed epoch-table sizing profiles, not execution or admission authority.
///
/// Existing public queue constructors continue to select `Default64`.
/// `Qualification1024` describes the separately accounted development profile;
/// selecting this value alone cannot configure a queue or authorize a launch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Gfx942FixedDispatchCapacityProfileV1 {
    #[default]
    Default64,
    Qualification1024,
}

pub(crate) use Gfx942FixedDispatchCapacityProfileV1 as FixedDispatchCapacityProfileV1;

impl Gfx942FixedDispatchCapacityProfileV1 {
    pub const fn slots(self) -> usize {
        dispatch_capacity_slots_body!(dispatch_rust_expr, self)
    }
}

/// Immutable queue-family metadata capacity. Cloning shares the account but
/// never duplicates a table or a debit. Each table reserves its own payload.
/// This is not native-memory accounting or evidence of hardware concurrency.
#[derive(Clone, Default)]
pub struct Gfx942FixedDispatchCapacityV1 {
    profile: Gfx942FixedDispatchCapacityProfileV1,
    account: Option<ResourceCreditAccountV1>,
}

impl Gfx942FixedDispatchCapacityV1 {
    /// Reserves the actual scaled table before a caller consumes native inputs.
    /// Default64 returns `None`, preserving its existing allocation timing.
    /// This fresh reservation can serve primary, initial or auxiliary binding,
    /// but not an existing lane's rebind.
    pub fn preallocate_fresh_v1<const N: usize>(
        &self,
    ) -> Result<Option<Gfx942FixedDispatchPreallocationV1>, Gfx942DispatchBindingErrorV1> {
        PreparedDispatchGenerationV1::preallocate::<N>(self, DispatchGenerationSeedV1::Fresh)
    }

    /// Opts into the one-packet, 1024-epoch qualification profile. Admission
    /// charges ControlResidentBytes before allocating each epoch table.
    /// Requires `scale-qualification`; aggregate/native qualification is separate.
    #[cfg(any(test, feature = "scale-qualification"))]
    pub fn qualification_1024(account: ResourceCreditAccountV1) -> Self {
        Self {
            profile: Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
            account: Some(account),
        }
    }

    pub const fn profile(&self) -> Gfx942FixedDispatchCapacityProfileV1 {
        self.profile
    }

    pub(super) fn validate_batch<const N: usize>(
        &self,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.profile == Gfx942FixedDispatchCapacityProfileV1::Qualification1024 && N != 1 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        Ok(())
    }

    fn matches(
        &self,
        profile: Gfx942FixedDispatchCapacityProfileV1,
        account: Option<&ResourceCreditAccountV1>,
    ) -> bool {
        self.profile == profile
            && match (self.account.as_ref(), account) {
                (None, None) => true,
                (Some(left), Some(right)) => left.shares_ledger_with(right),
                _ => false,
            }
    }
}

static NEXT_DISPATCH_RECIPE_OCCURRENCE_V1: AtomicU64 = AtomicU64::new(1);
const KERNEL_DESCRIPTOR_BYTES_V1: u64 = 64;
const COV6_IMPLICIT_ARGUMENT_BYTES_V1: usize = COV6_IMPLICIT_ARGUMENT_BYTES as usize;

const COV6_BLOCK_COUNT_X_OFFSET_V1: usize = 0;
const COV6_BLOCK_COUNT_Y_OFFSET_V1: usize = 4;
const COV6_BLOCK_COUNT_Z_OFFSET_V1: usize = 8;
const COV6_GROUP_SIZE_X_OFFSET_V1: usize = 12;
const COV6_GROUP_SIZE_Y_OFFSET_V1: usize = 14;
const COV6_GROUP_SIZE_Z_OFFSET_V1: usize = 16;
const COV6_REMAINDER_X_OFFSET_V1: usize = 18;
const COV6_REMAINDER_Y_OFFSET_V1: usize = 20;
const COV6_REMAINDER_Z_OFFSET_V1: usize = 22;
const COV6_GLOBAL_OFFSET_X_OFFSET_V1: usize = 40;
const COV6_GLOBAL_OFFSET_Y_OFFSET_V1: usize = 48;
const COV6_GLOBAL_OFFSET_Z_OFFSET_V1: usize = 56;
const COV6_GRID_DIMENSIONS_OFFSET_V1: usize = 64;
const COV6_DYNAMIC_LDS_SIZE_OFFSET_V1: usize = 120;

/// One inert global-buffer field in a fixed dispatch kernarg image.
///
/// This value identifies an inspected explicit-argument ordinal and a bounded
/// subrange of separately owned dispatch data. It contains no native
/// address and grants no initialization, access-effect, or dispatch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DispatchBufferBindingV1 {
    explicit_argument_index: usize,
    data_index: usize,
    data_byte_offset: u64,
    byte_len: u64,
    completed_snapshot: Option<CompletedSnapshotRangeV1>,
}

impl Gfx942DispatchBufferBindingV1 {
    pub const fn new(
        explicit_argument_index: usize,
        data_index: usize,
        data_byte_offset: u64,
        byte_len: u64,
    ) -> Self {
        Self {
            explicit_argument_index,
            data_index,
            data_byte_offset,
            byte_len,
            completed_snapshot: None,
        }
    }

    /// Associates an initialized enclosing range with one interior buffer pointer.
    ///
    /// This constructor is descriptive. Queue composition validates exact
    /// bounds, strict containment, initialization, inspected write access, and
    /// isolation from every other writable binding before retaining the range.
    pub const fn new_with_completed_snapshot(
        explicit_argument_index: usize,
        data_index: usize,
        data_byte_offset: u64,
        byte_len: u64,
        snapshot_byte_offset: u64,
        snapshot_byte_len: u64,
    ) -> Self {
        Self {
            explicit_argument_index,
            data_index,
            data_byte_offset,
            byte_len,
            completed_snapshot: Some(CompletedSnapshotRangeV1 {
                offset: snapshot_byte_offset,
                byte_len: snapshot_byte_len,
                interior_offset: data_byte_offset,
                interior_byte_len: byte_len,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletedSnapshotRangeV1 {
    offset: u64,
    byte_len: u64,
    interior_offset: u64,
    interior_byte_len: u64,
}

/// One inert packet description for a checked fixed dispatch batch.
///
/// The caller supplies scalar bytes, geometry, and buffer indices. Every
/// device-pointer field must be zero. When the inspected COV6 metadata declares
/// the supported implicit geometry or dynamic-LDS fields, the caller must also
/// provide the exact trailing 256-byte implicit suffix entirely zero. Queue
/// construction derives pointer locations, alignments, access effects, and the
/// admitted implicit values from inspected metadata and checked geometry, then
/// performs substitution only inside the retained native owner.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942FixedDispatchPacketV1;
///
/// fn cannot_extract_kernarg(packet: Gfx942FixedDispatchPacketV1) {
///     let _ = packet.kernarg_bytes;
/// }
/// ```
pub struct Gfx942FixedDispatchPacketV1 {
    program_index: usize,
    geometry: AqlDispatchGeometryV1,
    ordering: AqlDispatchOrderingV1,
    dynamic_group_segment_bytes: u32,
    kernarg_bytes: Box<[u8]>,
    buffers: Box<[Gfx942DispatchBufferBindingV1]>,
    conditional_fill: bool,
}

impl Gfx942FixedDispatchPacketV1 {}

impl fmt::Debug for Gfx942FixedDispatchPacketV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942FixedDispatchPacketV1")
            .field("program_index", &self.program_index)
            .field("geometry", &self.geometry)
            .field("ordering", &self.ordering)
            .field(
                "dynamic_group_segment_bytes",
                &self.dynamic_group_segment_bytes,
            )
            .field("kernarg_bytes", &self.kernarg_bytes.len())
            .field("buffer_count", &self.buffers.len())
            .field("conditional_fill", &self.conditional_fill)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942FixedDispatchDataKindV1 {
    DeviceLocal,
    HostVisibleCoherent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942FixedDispatchDataLayoutV1 {
    kind: Gfx942FixedDispatchDataKindV1,
    requested_bytes: u64,
    alignment: u64,
}

impl Gfx942FixedDispatchDataLayoutV1 {
    pub(super) const fn device_local(requested_bytes: u64, alignment: u64) -> Self {
        Self {
            kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
            requested_bytes,
            alignment,
        }
    }

    pub const fn kind(self) -> Gfx942FixedDispatchDataKindV1 {
        self.kind
    }

    pub const fn requested_bytes(self) -> u64 {
        self.requested_bytes
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

enum DispatchDataStorageV1 {
    Uninitialized(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    InitializedContent(Gfx942InitializedDeviceMemoryV1),
    InitializedStorage(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    HostVisibleUninitialized(
        SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
    ),
    HostVisibleInitialized(Gfx942InitializedHostVisibleMemoryV1),
}

/// Ordered non-authority identity retained by an unbound queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Gfx942FixedDispatchStorageIdentityV1 {
    DeviceUninitialized(Gfx942DeviceMemoryIdentityV1),
    DeviceInitializedContent(Gfx942DeviceMemoryIdentityV1),
    DeviceInitializedStorage(Gfx942DeviceMemoryIdentityV1),
    HostVisibleUninitialized(SharedGttAllocationIdentityV1),
    HostVisibleInitialized(SharedGttAllocationIdentityV1),
}

/// Move-only device-local or coherent host-visible input for fixed dispatch.
///
/// Uninitialized storage is admitted only for inspected write-only arguments.
/// Read-only and read-write arguments require the sealed initialized variant.
/// Neither variant exposes a native address, allocation handle, or generation.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942FixedDispatchDataV1;
///
/// fn cannot_clone(data: Gfx942FixedDispatchDataV1) {
///     let _ = data.clone();
/// }
/// ```
pub struct Gfx942FixedDispatchDataV1 {
    storage: DispatchDataStorageV1,
}

impl Gfx942FixedDispatchDataV1 {}

pub(crate) enum DispatchDataStorageRefV1<'a> {
    Device(&'a Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    HostVisible(&'a SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>),
}

pub(crate) enum DispatchDataInputStorageV1 {
    Device(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    HostVisible(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>),
}

pub(crate) struct DispatchDataInputV1 {
    pub(crate) layout: Gfx942FixedDispatchDataLayoutV1,
    pub(crate) storage: DispatchDataInputStorageV1,
    pub(crate) initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    pub(crate) fully_initialized: bool,
}

impl fmt::Debug for Gfx942FixedDispatchDataV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942FixedDispatchDataV1")
            .field("layout", &self.layout())
            .field("fully_initialized", &self.is_fully_initialized())
            .finish_non_exhaustive()
    }
}

/// Inert request for one owned copy from a recycled coherent dispatch range.
///
/// The request contains no address or allocation authority. The retained queue
/// revalidates its dispatch generation, data ordinal, inspected write effect,
/// coherent storage kind, and exact byte bounds before copying.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompletedDispatchReadRequestV1 {
    dispatch_generation: u64,
    data_index: usize,
    offset: u64,
    byte_len: u64,
}

/// Inert request to overwrite an initialized coherent range after exact recycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942RecycledDispatchWriteRequestV1 {
    dispatch_generation: u64,
    data_index: usize,
    offset: u64,
}

impl Gfx942RecycledDispatchWriteRequestV1 {
    pub const fn new(dispatch_generation: u64, data_index: usize, offset: u64) -> Self {
        Self {
            dispatch_generation,
            data_index,
            offset,
        }
    }
}

/// Inert request for one exact admitted enclosing snapshot after recycle.
///
/// The request contains no address or allocation authority. The retained
/// queue revalidates the dispatch generation and the exact snapshot declared
/// beside one initialized coherent writable binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompletedDispatchSnapshotRequestV1 {
    dispatch_generation: u64,
    data_index: usize,
    offset: u64,
    byte_len: u64,
}

impl Gfx942CompletedDispatchSnapshotRequestV1 {
    /// Describes one generation, data ordinal, and exact snapshot range.
    pub const fn new(
        dispatch_generation: u64,
        data_index: usize,
        offset: u64,
        byte_len: u64,
    ) -> Self {
        Self {
            dispatch_generation,
            data_index,
            offset,
            byte_len,
        }
    }
}

impl Gfx942CompletedDispatchReadRequestV1 {
    pub const fn new(
        dispatch_generation: u64,
        data_index: usize,
        offset: u64,
        byte_len: u64,
    ) -> Self {
        Self {
            dispatch_generation,
            data_index,
            offset,
            byte_len,
        }
    }
}

/// Owned bytes copied from one exact completed-and-recycled coherent range.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942CompletedDispatchReadbackV1 {
    dispatch_generation: u64,
    data_index: usize,
    offset: u64,
    bytes: Box<[u8]>,
}

impl Gfx942CompletedDispatchReadbackV1 {}

/// Frozen claim boundary for the addressless fixed-dispatch binding slice.
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-dispatch-binding-r52-v1\n",
    "target=gfx942:xnack-,COV6,one-selected-current-device-vm-and-queue-generation\n",
    "code=1-through-32-validated-amdhsa-kernel-envelopes,content-and-selected-descriptor-identity,exact-zero-then-copy-materialization-into-owned-gtt,read-only-seal-before-map,per-packet-program-selection,unused-inspected-programs-retained-without-publication,descriptor-resolution-with-checked-relative-arithmetic\n",
    "kernarg=public-inert-complete-byte-images,exact-inspected-size-and-power-of-two-alignment,optional-exact-trailing-256-byte-COV6-implicit-suffix-must-be-caller-zero,metadata-declared-block-count-group-size-remainder-zero-global-offset-grid-dimensions-and-dynamic-lds-only,queue-pointer-and-runtime-service-or-address-fields-rejected,all-global-buffer-fields-zero,checked-nonoverlapping-8-byte-internal-device-pointer-patches,one-owned-kernarg-gtt-arena-with-N-distinct-checked-aligned-slices,private-initialization-before-map\n",
    "geometry=block-count-floor-grid-div-workgroup,remainder-grid-mod-workgroup,inactive-dimensions-count-and-group-one-remainder-zero,uniform-workgroup-rejects-any-nonzero-remainder\n",
    "data=1-through-16-actual-linear-mapped-device-local-or-host-visible-coherent-authorities,exact-device-vm-and-allocation-generation,complete-device-local-live-set,all-authorities-retained-even-when-no-packet-references-them,checked-bounded-referenced-subranges,inspected-actual-access-derived-internally-only-for-referenced-authorities,read-or-readwrite-requires-sealed-full-extent-initialization,write-only-admits-uninitialized-exclusive-storage,optional-enclosing-snapshot-requires-coherent-full-initialization\n",
    "batch=1-through-8192,aql-fixed-batch-v2,minimum-ring-packet-capacity-checked,all-program-code-owners,N-distinct-kernarg-slices,every-public-fixed-recipe-packet-must-wait-for-prior,one-immutable-recipe-with-one-globally-minted-nonzero-occurrence-and-one-preallocated-64-slot-epoch-table,each-accepted-epoch-binds-exact-queue-recipe-slot-slot-generation-dispatch-generation-roster-completion-occurrence-and-packet-interval,one-reservation-one-write-counter-fetch-add-one-final-doorbell-and-one-signal-per-packet-composition\n",
    "retention=queue-owns-all-code-kernarg-and-data-authorities-through-every-exact-ready-and-recycle,unreferenced-data-has-no-inspected-effect-or-inspected-writable-readback-authority,ordinary-and-returning-destroy-require-all-64-epoch-slots-vacant,returning-destroy-uses-the-monotonic-maximum-exact-recycled-generation-and-returns-actual-mapped-authorities-with-owning-memory-session,replacement-owner-seeded-from-exact-recycled-predecessor-and-strictly-advances-before-publication,one-full-range-persistent-owner-may-retain-immutable-code-mapped-kernarg-packet-premise-and-recycled-generation-while-detaching-only-its-exact-data-authority,fully-initialized-state-preserved-without-stale-current-content-digest,initially-uninitialized-remains-uninitialized\n",
    "readback=owned-byte-copy-only-when-all-64-epoch-slots-are-vacant-after-exact-completion-and-signal-recycle,exact-dispatch-generation-and-retained-host-visible-allocation-authority,ordinary-request-must-be-contained-in-exactly-one-metadata-inspected-write-or-readwrite-binding;optional-snapshot-request-must-exactly-match-one-retained-strictly-enclosing-initialized-range-with-one-isolated-inspected-writable-interior;device-local-readonly-unwritten-out-of-range-overlapping-subrange-and-stale-requests-rejected,no-initialization-promotion\n",
    "initialized-observation=distinct-caller-owned-destination-copy-after-exact-recycled-generation-and-all-epoch-slots-vacant,retained-coherent-host-authority-and-sealed-fully-initialized-extent,checked-nonempty-bounds-and-exact-destination-length,includes-unreferenced-and-readonly-data,no-write-coverage-or-initialization-or-reuse-authority\n",
    "queue-transfer=ordinary-path-still-rejects-device-memory,dispatch-path-requires-exact-complete-distinct-set-of-every-live-mapped-c3-lease-before-model-mutation\n",
    "failure=all-layout-ordering-capacity-and-identity-validation-before-native-preparation,65th-live-epoch-rejects-without-mutation,completion-signal-or-ring-capacity-rejection-cancels-only-the-exact-reserved-epoch-and-burns-global-and-slot-generations,persistent-control-replay-requires-exact-queue-code-abi-packet-kernarg-role-layout-storage-and-recycled-predecessor-identity;post-side-effect-failure,currentness,publication,completion,timeout,recycle-release-or-unwind-ambiguity-poisons-and-requires-teardown\n",
    "authority=public-linear-addressless-construction-submit-poll-wait-recycle-and-returning-destroy,no-address-handle-pointer-fd-packet-template-signal-or-mmio-export\n",
    "proof=bounded-host-state-machine-and-mock-fault-tests-only,no-concrete-verus-or-machine-refinement-of-ordered-shared-recipe-epochs\n",
    "contracted=code-segment-permission-refinement,cpu-gpu-coherence,firmware-dispatch-effects-and-quiescence,acquire-observed-device-write-visibility\n",
    "excluded=queue-pointer,printf-hostcall-heap-default-queue-completion-action-multigrid-private-base-shared-base-and-unknown-implicit-fields,caller-effect-assertion,caller-initialization-assertion,public-packet-template,async-copy,device-address-export,peer-map,full-write-coverage-or-initialization-promotion,numerical-correctness,hardware-execution,general-multi-recipe-or-shared-buffer-dag,concurrent-kernel-execution,performance-or-hip-hsa-parity\n",
);

/// SHA-256 of [`GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1`].
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1: &str =
    "d4265552e99fcfefcfdcb094b0927647edd0f50a948a8a970d91e3636ae7b694";

type CodeAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchCodeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type KernargAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchKernargResourceRoleV1,
    KernargGttV1,
    GttGpuAccessibleMutableV1,
>;
type HostDataAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchHostDataResourceRoleV1,
    HostVisibleCoherentGttV1,
    GttGpuAccessibleMutableV1,
>;

pub(crate) enum DispatchDataAuthorityV1 {
    Device(Gfx942DeviceMemoryDispatchAuthorityV1),
    HostVisible(HostDataAuthority),
}

impl DispatchDataAuthorityV1 {
    const fn kind(&self) -> Gfx942FixedDispatchDataKindV1 {
        match self {
            Self::Device(_) => Gfx942FixedDispatchDataKindV1::DeviceLocal,
            Self::HostVisible(_) => Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
        }
    }

    const fn vm(&self) -> fe2o3_runtime_model::VmKeyV1 {
        dispatch_template_data_vm_body!(dispatch_rust_expr, self)
    }

    fn checked_gpu_subrange(&self, offset: u64, byte_len: u64, alignment: u64) -> Option<u64> {
        match self {
            Self::Device(authority) => authority
                .facts()
                .checked_gpu_subrange(offset, byte_len, alignment),
            Self::HostVisible(authority) => authority
                .facts()
                .checked_gpu_subrange(offset, byte_len, alignment),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceDataEffectV1 {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl DeviceDataEffectV1 {
    pub(crate) const fn reads(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }
}

/// Validates the exact R25 one-packet, one-data, full-allocation contract and
/// returns only the access derived from authenticated kernel metadata.
pub(super) fn preflight_gfx942_persistent_compute_dispatch_v1(
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; 1],
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
) -> Result<DeviceDataEffectV1, Gfx942DispatchBindingErrorV1> {
    if packets[0].conditional_fill
        || layout.kind != Gfx942FixedDispatchDataKindV1::DeviceLocal
        || packets[0].buffers.len() != 1
        || packets[0].buffers[0].data_index != 0
        || packets[0].buffers[0].data_byte_offset != 0
        || packets[0].buffers[0].byte_len != layout.requested_bytes
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent compute requires one full-allocation device binding",
        });
    }
    let plan = plan_public_fixed_dispatch_resources(
        programs,
        packets,
        core::slice::from_ref(&layout),
        core::slice::from_ref(&initialized),
    )?;
    plan.data
        .into_iter()
        .next()
        .and_then(|data| data.effect)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent compute data must be referenced",
        })
}

/// Addressless identity for one prepare-once persistent dispatch control.
///
/// The semantic digest binds the authenticated code/ABI roster and every inert
/// packet field. Storage is kept separate so replay can reject substitution
/// without serializing or exposing a private allocation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PersistentFixedDispatchControlIdentityV1 {
    queue: QueueKeyV1,
    semantic_sha256: [u8; 32],
    content_role: Gfx942DeviceContentRoleV1,
    data_layout: Gfx942FixedDispatchDataLayoutV1,
    data_storage: Gfx942SdmaBufferStorageIdentityV1,
    effect: DeviceDataEffectV1,
}

impl PersistentFixedDispatchControlIdentityV1 {
    pub(super) const fn effect(self) -> DeviceDataEffectV1 {
        self.effect
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistentFixedDispatchControlStateV1 {
    Ordinary,
    Attached(BoundedPersistentFixedDispatchControlIdentityV1),
    DataDetached(BoundedPersistentFixedDispatchControlIdentityV1),
}

/// Immutable identity for the exact two-read/one-write persistent control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreeBindingPersistentFixedDispatchControlIdentityV1 {
    queue: QueueKeyV1,
    semantic_sha256: [u8; 32],
    content_roles: [Gfx942DeviceContentRoleV1; 3],
    data_layouts: [Gfx942FixedDispatchDataLayoutV1; 3],
    data_storage: [Gfx942SdmaBufferStorageIdentityV1; 3],
    effects: [DeviceDataEffectV1; 3],
}

impl ThreeBindingPersistentFixedDispatchControlIdentityV1 {
    pub(super) const fn effects(self) -> [DeviceDataEffectV1; 3] {
        self.effects
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundedPersistentFixedDispatchControlIdentityV1 {
    queue: QueueKeyV1,
    semantic_sha256: [u8; 32],
    binding_count: u8,
    content_roles: [Option<Gfx942DeviceContentRoleV1>; MAX_DISPATCH_DATA_LEASES_V1],
    data_layouts: [Option<Gfx942FixedDispatchDataLayoutV1>; MAX_DISPATCH_DATA_LEASES_V1],
    data_storage: [Option<Gfx942SdmaBufferStorageIdentityV1>; MAX_DISPATCH_DATA_LEASES_V1],
    effects: [Option<DeviceDataEffectV1>; MAX_DISPATCH_DATA_LEASES_V1],
}

impl BoundedPersistentFixedDispatchControlIdentityV1 {
    fn from_single(identity: PersistentFixedDispatchControlIdentityV1) -> Self {
        let mut bounded = Self {
            queue: identity.queue,
            semantic_sha256: identity.semantic_sha256,
            binding_count: 1,
            content_roles: [None; MAX_DISPATCH_DATA_LEASES_V1],
            data_layouts: [None; MAX_DISPATCH_DATA_LEASES_V1],
            data_storage: [None; MAX_DISPATCH_DATA_LEASES_V1],
            effects: [None; MAX_DISPATCH_DATA_LEASES_V1],
        };
        bounded.content_roles[0] = Some(identity.content_role);
        bounded.data_layouts[0] = Some(identity.data_layout);
        bounded.data_storage[0] = Some(identity.data_storage);
        bounded.effects[0] = Some(identity.effect);
        bounded
    }

    fn from_three(identity: ThreeBindingPersistentFixedDispatchControlIdentityV1) -> Self {
        let mut bounded = Self {
            queue: identity.queue,
            semantic_sha256: identity.semantic_sha256,
            binding_count: 3,
            content_roles: [None; MAX_DISPATCH_DATA_LEASES_V1],
            data_layouts: [None; MAX_DISPATCH_DATA_LEASES_V1],
            data_storage: [None; MAX_DISPATCH_DATA_LEASES_V1],
            effects: [None; MAX_DISPATCH_DATA_LEASES_V1],
        };
        for index in 0..3 {
            bounded.content_roles[index] = Some(identity.content_roles[index]);
            bounded.data_layouts[index] = Some(identity.data_layouts[index]);
            bounded.data_storage[index] = Some(identity.data_storage[index]);
            bounded.effects[index] = Some(identity.effects[index]);
        }
        bounded
    }

    const fn binding_count(self) -> usize {
        self.binding_count as usize
    }

    fn as_single(self) -> Option<PersistentFixedDispatchControlIdentityV1> {
        (self.binding_count == 1).then(|| PersistentFixedDispatchControlIdentityV1 {
            queue: self.queue,
            semantic_sha256: self.semantic_sha256,
            content_role: self.content_roles[0].expect("single persistent content role"),
            data_layout: self.data_layouts[0].expect("single persistent data layout"),
            data_storage: self.data_storage[0].expect("single persistent storage identity"),
            effect: self.effects[0].expect("single persistent effect"),
        })
    }
}

pub(super) fn three_binding_persistent_fixed_dispatch_control_identity_v1(
    queue: QueueKeyV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; 1],
    data_layouts: [Gfx942FixedDispatchDataLayoutV1; 3],
    initialized: [bool; 3],
    content_roles: [Gfx942DeviceContentRoleV1; 3],
    data_storage: [Gfx942SdmaBufferStorageIdentityV1; 3],
) -> Result<ThreeBindingPersistentFixedDispatchControlIdentityV1, Gfx942DispatchBindingErrorV1> {
    if packets[0].conditional_fill
        || data_layouts
            .iter()
            .any(|layout| layout.kind != Gfx942FixedDispatchDataKindV1::DeviceLocal)
        || !initialized[0]
        || !initialized[1]
        || !initialized[2]
        || data_layouts[0].requested_bytes != data_layouts[1].requested_bytes
        || data_layouts[0].requested_bytes != data_layouts[2].requested_bytes
        || packets[0].buffers.len() != 3
        || packets[0]
            .buffers
            .iter()
            .enumerate()
            .any(|(index, buffer)| {
                buffer.data_index != index
                    || buffer.data_byte_offset != 0
                    || buffer.byte_len != data_layouts[index].requested_bytes
            })
        || data_storage[0] == data_storage[1]
        || data_storage[0] == data_storage[2]
        || data_storage[1] == data_storage[2]
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "three-binding persistent compute requires distinct full-extent device data in A/B/C order",
        });
    }
    let plan =
        plan_public_fixed_dispatch_resources(programs, packets, &data_layouts, &initialized)?;
    let effects = [
        DeviceDataEffectV1::ReadOnly,
        DeviceDataEffectV1::ReadOnly,
        DeviceDataEffectV1::WriteOnly,
    ];
    if plan.data.len() != effects.len()
        || !plan
            .data
            .iter()
            .zip(effects)
            .all(|(data, expected)| data.effect == Some(expected))
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "three-binding persistent compute requires metadata effects Read/Read/Write",
        });
    }
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-gfx942-three-binding-persistent-fixed-dispatch-control-v1\0");
    digest.update((programs.len() as u64).to_le_bytes());
    for program in programs {
        let identity = program.identity_inputs();
        digest.update(identity.object_sha256());
        digest.update(identity.metadata_sha256());
        digest.update(identity.descriptor_sha256());
        digest.update(identity.entry_sha256());
        digest.update(identity.closure_sha256());
        match program.dispatch_abi_identity() {
            Some(identity) => {
                digest.update([1]);
                digest.update(identity);
            }
            None => digest.update([0]),
        }
    }
    for packet in packets {
        digest.update((packet.program_index as u64).to_le_bytes());
        for value in packet.geometry.grid() {
            digest.update(value.to_le_bytes());
        }
        for value in packet.geometry.workgroup() {
            digest.update(value.to_le_bytes());
        }
        digest.update(packet.geometry.dimensions().to_le_bytes());
        digest.update(packet.ordering.header().to_le_bytes());
        digest.update(packet.dynamic_group_segment_bytes.to_le_bytes());
        digest.update((packet.kernarg_bytes.len() as u64).to_le_bytes());
        digest.update(&packet.kernarg_bytes);
        digest.update((packet.buffers.len() as u64).to_le_bytes());
        for buffer in &packet.buffers {
            digest.update((buffer.explicit_argument_index as u64).to_le_bytes());
            digest.update((buffer.data_index as u64).to_le_bytes());
            digest.update(buffer.data_byte_offset.to_le_bytes());
            digest.update(buffer.byte_len.to_le_bytes());
            match buffer.completed_snapshot {
                Some(snapshot) => {
                    digest.update([1]);
                    digest.update(snapshot.offset.to_le_bytes());
                    digest.update(snapshot.byte_len.to_le_bytes());
                    digest.update(snapshot.interior_offset.to_le_bytes());
                    digest.update(snapshot.interior_byte_len.to_le_bytes());
                }
                None => digest.update([0]),
            }
        }
    }
    for index in 0..3 {
        digest.update(content_roles[index].identity());
        digest.update(content_roles[index].ordinal().to_le_bytes());
        digest.update(data_layouts[index].requested_bytes().to_le_bytes());
        digest.update(data_layouts[index].alignment().to_le_bytes());
        digest.update([u8::from(initialized[index] && effects[index].reads())]);
    }
    Ok(ThreeBindingPersistentFixedDispatchControlIdentityV1 {
        queue,
        semantic_sha256: digest.finalize().into(),
        content_roles,
        data_layouts,
        data_storage,
        effects,
    })
}

pub(super) fn persistent_fixed_dispatch_control_identity_v1(
    queue: QueueKeyV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; 1],
    data_layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content_role: Gfx942DeviceContentRoleV1,
    data_storage: Gfx942SdmaBufferStorageIdentityV1,
) -> Result<PersistentFixedDispatchControlIdentityV1, Gfx942DispatchBindingErrorV1> {
    let effect = preflight_gfx942_persistent_compute_dispatch_v1(
        programs,
        packets,
        data_layout,
        initialized,
    )?;
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-gfx942-persistent-fixed-dispatch-control-v1\0");
    digest.update((programs.len() as u64).to_le_bytes());
    for program in programs {
        let identity = program.identity_inputs();
        digest.update(identity.object_sha256());
        digest.update(identity.metadata_sha256());
        digest.update(identity.descriptor_sha256());
        digest.update(identity.entry_sha256());
        digest.update(identity.closure_sha256());
        match program.dispatch_abi_identity() {
            Some(identity) => {
                digest.update([1]);
                digest.update(identity);
            }
            None => digest.update([0]),
        }
    }
    for packet in packets {
        digest.update((packet.program_index as u64).to_le_bytes());
        for value in packet.geometry.grid() {
            digest.update(value.to_le_bytes());
        }
        for value in packet.geometry.workgroup() {
            digest.update(value.to_le_bytes());
        }
        digest.update(packet.geometry.dimensions().to_le_bytes());
        digest.update(packet.ordering.header().to_le_bytes());
        digest.update(packet.dynamic_group_segment_bytes.to_le_bytes());
        digest.update((packet.kernarg_bytes.len() as u64).to_le_bytes());
        digest.update(&packet.kernarg_bytes);
        digest.update((packet.buffers.len() as u64).to_le_bytes());
        for buffer in &packet.buffers {
            digest.update((buffer.explicit_argument_index as u64).to_le_bytes());
            digest.update((buffer.data_index as u64).to_le_bytes());
            digest.update(buffer.data_byte_offset.to_le_bytes());
            digest.update(buffer.byte_len.to_le_bytes());
            match buffer.completed_snapshot {
                Some(snapshot) => {
                    digest.update([1]);
                    digest.update(snapshot.offset.to_le_bytes());
                    digest.update(snapshot.byte_len.to_le_bytes());
                    digest.update(snapshot.interior_offset.to_le_bytes());
                    digest.update(snapshot.interior_byte_len.to_le_bytes());
                }
                None => digest.update([0]),
            }
        }
    }
    digest.update(content_role.identity());
    digest.update(content_role.ordinal().to_le_bytes());
    digest.update([match data_layout.kind() {
        Gfx942FixedDispatchDataKindV1::DeviceLocal => 1,
        Gfx942FixedDispatchDataKindV1::HostVisibleCoherent => 2,
    }]);
    digest.update(data_layout.requested_bytes().to_le_bytes());
    digest.update(data_layout.alignment().to_le_bytes());
    // Initialization is an admission premise, not immutable control. A
    // write-only first use may establish the premise consumed by exact replay.
    digest.update([u8::from(initialized && effect.reads())]);
    digest.update([match effect {
        DeviceDataEffectV1::ReadOnly => 1,
        DeviceDataEffectV1::WriteOnly => 2,
        DeviceDataEffectV1::ReadWrite => 3,
    }]);
    Ok(PersistentFixedDispatchControlIdentityV1 {
        queue,
        semantic_sha256: digest.finalize().into(),
        content_role,
        data_layout,
        data_storage,
        effect,
    })
}

/// Write-only premise for one whole uninitialized C3 allocation.
///
/// Read effects remain rejected until the device-content foundation can
/// consume an authenticated copy-kernel completion and the queue can return
/// its retained destination lease. There is intentionally no caller boolean.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceDataPremiseV1 {
    role_identity: [u8; 32],
    valid_bytes: u64,
    effect: DeviceDataEffectV1,
}

impl DeviceDataPremiseV1 {
    pub(crate) const fn new(
        role_identity: [u8; 32],
        valid_bytes: u64,
        effect: DeviceDataEffectV1,
    ) -> Self {
        Self {
            role_identity,
            valid_bytes,
            effect,
        }
    }
}

pub(crate) struct DeviceDataAllocationInputV1 {
    requested_bytes: u64,
    alignment: u64,
    premise: DeviceDataPremiseV1,
}

impl DeviceDataAllocationInputV1 {
    pub(crate) const fn new(
        requested_bytes: u64,
        alignment: u64,
        premise: DeviceDataPremiseV1,
    ) -> Self {
        Self {
            requested_bytes,
            alignment,
            premise,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DevicePointerPatchV1 {
    byte_offset: usize,
    data_index: usize,
    data_byte_offset: u64,
    required_bytes: u64,
    required_alignment: u64,
}

impl DevicePointerPatchV1 {
    pub(crate) const fn new(
        byte_offset: usize,
        data_index: usize,
        data_byte_offset: u64,
        required_bytes: u64,
        required_alignment: u64,
    ) -> Self {
        Self {
            byte_offset,
            data_index,
            data_byte_offset,
            required_bytes,
            required_alignment,
        }
    }
}

/// Complete typed kernarg image before private device-pointer substitution.
///
/// Bytes and patch locations have no public accessor or constructor.
pub(crate) struct TypedKernargImageV1 {
    layout_identity: [u8; 32],
    bytes: Box<[u8]>,
    device_pointers: Box<[DevicePointerPatchV1]>,
}

impl TypedKernargImageV1 {
    pub(crate) fn new(
        layout_identity: [u8; 32],
        bytes: Box<[u8]>,
        device_pointers: Box<[DevicePointerPatchV1]>,
    ) -> Self {
        Self {
            layout_identity,
            bytes,
            device_pointers,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DispatchGeometryV1 {
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
}

impl DispatchGeometryV1 {
    pub(crate) const fn new(
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
    ) -> Self {
        Self {
            geometry,
            dynamic_group_segment_bytes,
        }
    }
}

#[derive(Debug)]
pub enum Gfx942DispatchBindingErrorV1 {
    ZeroPacketCount,
    PacketCountExceedsMaximum { requested: usize, maximum: usize },
    RingCapacity { requested: usize, capacity: u32 },
    ProgramCount { requested: usize, maximum: usize },
    DataLeaseCount { requested: usize, maximum: usize },
    InvalidCode(&'static str),
    InvalidKernarg { packet: usize, detail: &'static str },
    InvalidData { index: usize, detail: &'static str },
    Geometry { packet: usize, detail: &'static str },
    Memory(MemorySessionError),
    Completion(Gfx942CompletionErrorV1),
    WrongQueueGeneration,
    StaleDispatchGeneration,
    ResourcePhase,
    DispatchEpochCapacity { maximum: usize },
    HostAllocationCapacity { operation: &'static str },
    GenerationExhausted,
    Poisoned,
}

impl fmt::Display for Gfx942DispatchBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942DispatchBindingErrorV1 {}

impl From<MemorySessionError> for Gfx942DispatchBindingErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

impl From<Gfx942CompletionErrorV1> for Gfx942DispatchBindingErrorV1 {
    fn from(value: Gfx942CompletionErrorV1) -> Self {
        Self::Completion(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchEpochPhaseV1 {
    Vacant,
    Reserved {
        dispatch_generation: u64,
        expected_roster: CompletionDispatchRosterV1,
    },
    Published {
        dispatch_generation: u64,
        completion: CompletionBatchOccurrenceV1,
    },
    Completed {
        dispatch_generation: u64,
        completion: CompletionBatchOccurrenceV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DispatchEpochSlotV1 {
    slot_generation: u64,
    phase: DispatchEpochPhaseV1,
}

impl DispatchEpochSlotV1 {
    const VACANT: Self = Self {
        slot_generation: 0,
        phase: DispatchEpochPhaseV1::Vacant,
    };
}

/// Sealed identity for one accepted epoch of one immutable lane recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DispatchEpochIdentityV1 {
    queue: QueueKeyV1,
    recipe_occurrence: u64,
    slot_index: u16,
    slot_generation: u64,
    dispatch_generation: u64,
}

impl DispatchEpochIdentityV1 {
    pub(super) const fn dispatch_generation(self) -> u64 {
        self.dispatch_generation
    }

    #[cfg(test)]
    pub(super) const fn for_test(queue: QueueKeyV1, dispatch_generation: u64) -> Self {
        Self {
            queue,
            recipe_occurrence: 1,
            slot_index: 0,
            slot_generation: dispatch_generation,
            dispatch_generation,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DispatchGenerationOwnerV1 {
    next_generation: u64,
    recipe_occurrence: u64,
    recipe_queue: Option<QueueKeyV1>,
    capacity_profile: FixedDispatchCapacityProfileV1,
    slots: HostMetadataTableV1<DispatchEpochSlotV1>,
    recycled_generation: Option<u64>,
    predecessor_detached_generation: Option<u64>,
    poisoned: bool,
}

#[cfg(test)]
impl Clone for DispatchGenerationOwnerV1 {
    fn clone(&self) -> Self {
        Self {
            next_generation: self.next_generation,
            recipe_occurrence: self.recipe_occurrence,
            recipe_queue: self.recipe_queue,
            capacity_profile: self.capacity_profile,
            slots: self
                .slots
                .try_clone()
                .expect("independently reserved test snapshot"),
            recycled_generation: self.recycled_generation,
            predecessor_detached_generation: self.predecessor_detached_generation,
            poisoned: self.poisoned,
        }
    }
}

fn mint_dispatch_recipe_occurrence_v1() -> Result<u64, Gfx942DispatchBindingErrorV1> {
    NEXT_DISPATCH_RECIPE_OCCURRENCE_V1
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current != 0).then(|| current.checked_add(1)).flatten()
        })
        .map_err(|_| Gfx942DispatchBindingErrorV1::GenerationExhausted)
}

impl DispatchGenerationOwnerV1 {
    fn new() -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::with_next_generation(1)
    }

    fn with_next_generation(next_generation: u64) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::with_capacity(
            next_generation,
            FixedDispatchCapacityProfileV1::Default64,
            None,
        )
    }

    fn with_capacity(
        next_generation: u64,
        capacity_profile: FixedDispatchCapacityProfileV1,
        account: Option<&ResourceCreditAccountV1>,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::with_capacity_and_occurrence(
            next_generation,
            capacity_profile,
            account,
            mint_dispatch_recipe_occurrence_v1,
        )
    }

    fn with_capacity_and_occurrence(
        next_generation: u64,
        capacity_profile: FixedDispatchCapacityProfileV1,
        account: Option<&ResourceCreditAccountV1>,
        mint_occurrence: impl FnOnce() -> Result<u64, Gfx942DispatchBindingErrorV1>,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        if next_generation == 0
            || (capacity_profile == FixedDispatchCapacityProfileV1::Qualification1024
                && account.is_none())
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        next_generation
            .checked_add(1)
            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
        let slots = HostMetadataTableV1::try_new(capacity_profile.slots(), account, || {
            DispatchEpochSlotV1::VACANT
        })
        .map_err(|_| Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
            operation: "fixed dispatch epoch table",
        })?;
        Ok(Self {
            next_generation,
            recipe_occurrence: mint_occurrence()?,
            recipe_queue: None,
            capacity_profile,
            slots,
            recycled_generation: None,
            predecessor_detached_generation: None,
            poisoned: false,
        })
    }

    fn after_recycled(predecessor: u64) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::after_recycled_with_capacity(
            predecessor,
            FixedDispatchCapacityProfileV1::Default64,
            None,
        )
    }

    fn after_recycled_with_capacity(
        predecessor: u64,
        capacity_profile: FixedDispatchCapacityProfileV1,
        account: Option<&ResourceCreditAccountV1>,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let next_generation = next_dispatch_generation_after_recycled_v1(predecessor)?;
        Self::with_capacity(next_generation, capacity_profile, account)
    }

    fn after_detached(predecessor: u64) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::after_detached_with_capacity(
            predecessor,
            FixedDispatchCapacityProfileV1::Default64,
            None,
        )
    }

    fn after_detached_with_capacity(
        predecessor: u64,
        capacity_profile: FixedDispatchCapacityProfileV1,
        account: Option<&ResourceCreditAccountV1>,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        if predecessor == 0 {
            Self::with_capacity(1, capacity_profile, account)
        } else {
            Self::after_recycled_with_capacity(predecessor, capacity_profile, account)
        }
    }

    fn persistent_after_detached(
        predecessor: Option<u64>,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let mut owner = match predecessor {
            Some(predecessor) => Self::after_detached(predecessor),
            None => Self::new(),
        }?;
        owner.predecessor_detached_generation = predecessor;
        Ok(owner)
    }

    fn reserve(
        &mut self,
        queue: QueueKeyV1,
        expected_roster: CompletionDispatchRosterV1,
    ) -> Result<DispatchEpochIdentityV1, Gfx942DispatchBindingErrorV1> {
        dispatch_reserve_epoch_body!(dispatch_rust_expr, self, queue, expected_roster)
    }

    fn preflight_reservation(
        &self,
        queue: QueueKeyV1,
    ) -> Result<(usize, u64, u64), Gfx942DispatchBindingErrorV1> {
        dispatch_preflight_reservation_body!(dispatch_rust_expr, self, queue)
    }

    fn mark_published(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_identity(
            identity,
            DispatchEpochPhaseV1::Reserved {
                dispatch_generation: identity.dispatch_generation,
                expected_roster: completion.dispatch_roster,
            },
        )?;
        if completion.queue != identity.queue {
            return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
        }
        self.slots[identity.slot_index as usize].phase = DispatchEpochPhaseV1::Published {
            dispatch_generation: identity.dispatch_generation,
            completion,
        };
        Ok(())
    }

    fn validate_published(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_identity(
            identity,
            DispatchEpochPhaseV1::Published {
                dispatch_generation: identity.dispatch_generation,
                completion,
            },
        )
    }

    fn cancel_epoch(
        &mut self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        dispatch_cancel_epoch_body!(dispatch_rust_expr, self, identity)
    }

    fn complete_epoch(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.validate_published(identity, completion)?;
        self.slots[identity.slot_index as usize].phase = DispatchEpochPhaseV1::Completed {
            dispatch_generation: identity.dispatch_generation,
            completion,
        };
        Ok(())
    }

    fn validate_completed(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_identity(
            identity,
            DispatchEpochPhaseV1::Completed {
                dispatch_generation: identity.dispatch_generation,
                completion,
            },
        )
    }

    fn recycle_epoch(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.validate_completed(identity, completion)?;
        self.slots[identity.slot_index as usize].phase = DispatchEpochPhaseV1::Vacant;
        self.recycled_generation = Some(
            self.recycled_generation
                .map_or(identity.dispatch_generation, |prior| {
                    prior.max(identity.dispatch_generation)
                }),
        );
        Ok(())
    }

    fn returned_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        self.recycled_generation
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }

    fn returning_destroy_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        Ok(self.recycled_generation.unwrap_or(0))
    }

    fn persistent_cancellation_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        // Cancellation preserves a detached continuation, not a completion on this owner.
        Ok(self
            .recycled_generation
            .or(self.predecessor_detached_generation)
            .unwrap_or(0))
    }

    fn poison(&mut self) {
        self.poisoned = true;
    }

    fn ensure_not_poisoned(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        dispatch_not_poisoned_body!(dispatch_rust_expr, self)
    }

    fn ensure_prepared(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.ensure_not_poisoned()?;
        if self
            .slots
            .iter()
            .all(|slot| slot.phase == DispatchEpochPhaseV1::Vacant)
        {
            Ok(())
        } else {
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        }
    }

    fn require_identity(
        &self,
        identity: DispatchEpochIdentityV1,
        expected: DispatchEpochPhaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        dispatch_require_identity_body!(dispatch_rust_expr, self, identity, expected)
    }

    fn expected_roster(
        &self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<CompletionDispatchRosterV1, Gfx942DispatchBindingErrorV1> {
        dispatch_expected_roster_body!(dispatch_rust_expr, self, identity)
    }

    #[cfg(test)]
    fn next(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        self.next_generation
            .checked_add(1)
            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
        Ok(self.next_generation)
    }

    #[cfg(test)]
    fn commit_begin(&mut self, generation: u64) {
        debug_assert_eq!(generation, self.next_generation);
        self.next_generation = generation + 1;
        self.slots[0].slot_generation += 1;
        self.slots[0].phase = DispatchEpochPhaseV1::Reserved {
            dispatch_generation: generation,
            expected_roster: test_completion_roster_v1(generation),
        };
    }

    #[cfg(test)]
    fn active(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_not_poisoned()?;
        self.slots
            .iter()
            .find_map(|slot| match slot.phase {
                DispatchEpochPhaseV1::Reserved {
                    dispatch_generation,
                    ..
                }
                | DispatchEpochPhaseV1::Published {
                    dispatch_generation,
                    ..
                }
                | DispatchEpochPhaseV1::Completed {
                    dispatch_generation,
                    ..
                } => Some(dispatch_generation),
                DispatchEpochPhaseV1::Vacant => None,
            })
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }

    #[cfg(test)]
    fn cancel(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.test_require_generation(generation, false)?;
        self.slots[0].phase = DispatchEpochPhaseV1::Vacant;
        Ok(())
    }

    #[cfg(test)]
    fn complete(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.test_require_generation(generation, false)?;
        self.slots[0].phase = DispatchEpochPhaseV1::Completed {
            dispatch_generation: generation,
            completion: test_completion_occurrence_v1(generation),
        };
        Ok(())
    }

    #[cfg(test)]
    fn recycle(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.test_require_generation(generation, true)?;
        self.slots[0].phase = DispatchEpochPhaseV1::Vacant;
        self.recycled_generation = Some(
            self.recycled_generation
                .map_or(generation, |prior| prior.max(generation)),
        );
        Ok(())
    }

    #[cfg(test)]
    fn test_require_generation(
        &self,
        generation: u64,
        completed: bool,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.ensure_not_poisoned()?;
        let matches = match self.slots[0].phase {
            DispatchEpochPhaseV1::Reserved {
                dispatch_generation,
                ..
            } => !completed && dispatch_generation == generation,
            DispatchEpochPhaseV1::Completed {
                dispatch_generation,
                ..
            } => completed && dispatch_generation == generation,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
        }
    }
}

fn next_dispatch_generation_after_recycled_v1(
    predecessor: u64,
) -> Result<u64, Gfx942DispatchBindingErrorV1> {
    if predecessor == 0 {
        return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
    }
    predecessor
        .checked_add(1)
        .filter(|generation| generation.checked_add(1).is_some())
        .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)
}

#[cfg(test)]
fn test_dispatch_queue_v1() -> QueueKeyV1 {
    QueueKeyV1 {
        vm: fe2o3_runtime_model::VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(1),
                generation: fe2o3_runtime_model::DeviceGenerationV1(1),
            },
            id: fe2o3_runtime_model::VmIdV1(1),
        },
        id: fe2o3_runtime_model::QueueInstanceIdV1(1),
        generation: fe2o3_runtime_model::QueueGenerationV1(1),
    }
}

#[cfg(test)]
fn test_completion_roster_v1(generation: u64) -> CompletionDispatchRosterV1 {
    CompletionDispatchRosterV1 {
        queue: test_dispatch_queue_v1(),
        packet_count: 1,
        dispatch_generation: generation,
        roster_sha256: [generation as u8; 32],
    }
}

#[cfg(test)]
fn test_completion_occurrence_v1(generation: u64) -> CompletionBatchOccurrenceV1 {
    let queue = test_dispatch_queue_v1();
    CompletionBatchOccurrenceV1 {
        batch_id: generation,
        queue,
        signal_mapping: MemoryMappingKeyV1 {
            allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
                vm: queue.vm,
                id: fe2o3_runtime_model::AllocationIdV1(1),
                generation: fe2o3_runtime_model::AllocationGenerationV1(1),
            },
            id: fe2o3_runtime_model::MappingIdV1(1),
        },
        packet_count: 1,
        first_packet_id: generation,
        last_packet_id: generation,
        roster_sha256: [generation as u8; 32],
        dispatch_roster: test_completion_roster_v1(generation),
    }
}

/// Test-only logical dispatch owner for exercising queue-live orchestration
/// without fabricating code, kernarg, data, or native memory authority.
#[cfg(test)]
pub(super) struct TestOnlyDispatchGenerationOwnerV1 {
    next_generation: u64,
    active_generation: Option<u64>,
    predecessor_generation: u64,
    last_cancelled_generation: Option<u64>,
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
pub(super) struct TestOnlyMultiInflightDispatchOwnerV1 {
    owner: DispatchGenerationOwnerV1,
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CpuDispatchOwnerSnapshotV1 {
    next_generation: u64,
    recipe_occurrence: u64,
    recipe_queue: Option<QueueKeyV1>,
    capacity_profile: FixedDispatchCapacityProfileV1,
    slot_storage: usize,
    slots: Vec<DispatchEpochSlotV1>,
    recycled_generation: Option<u64>,
    predecessor_detached_generation: Option<u64>,
    poisoned: bool,
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
impl CpuDispatchOwnerSnapshotV1 {
    #[cfg(test)]
    pub(super) fn expect_cancel_for_test(&mut self, identity: DispatchEpochIdentityV1) {
        assert!(!self.poisoned);
        assert_eq!(self.recipe_queue, Some(identity.queue));
        assert_eq!(self.recipe_occurrence, identity.recipe_occurrence);
        let slot = &mut self.slots[usize::from(identity.slot_index)];
        assert_eq!(slot.slot_generation, identity.slot_generation);
        assert!(
            matches!(slot.phase, DispatchEpochPhaseV1::Reserved { dispatch_generation, .. }
            if dispatch_generation == identity.dispatch_generation)
        );
        slot.phase = DispatchEpochPhaseV1::Vacant;
    }

    #[cfg(test)]
    pub(super) fn expect_poison_for_test(&mut self) {
        self.poisoned = true;
    }

    pub(super) fn same_custody(&self, other: &Self) -> bool {
        self.next_generation == other.next_generation
            && self.recipe_occurrence == other.recipe_occurrence
            && self.recipe_queue == other.recipe_queue
            && self.capacity_profile == other.capacity_profile
            && self.slot_storage == other.slot_storage
            && self.slots == other.slots
            && self.recycled_generation == other.recycled_generation
            && self.predecessor_detached_generation == other.predecessor_detached_generation
    }
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
impl TestOnlyMultiInflightDispatchOwnerV1 {
    pub(super) fn cpu_snapshot(&self) -> CpuDispatchOwnerSnapshotV1 {
        CpuDispatchOwnerSnapshotV1 {
            next_generation: self.owner.next_generation,
            recipe_occurrence: self.owner.recipe_occurrence,
            recipe_queue: self.owner.recipe_queue,
            capacity_profile: self.owner.capacity_profile,
            slot_storage: self.owner.slots.as_ptr() as usize,
            slots: self.owner.slots.iter().copied().collect(),
            recycled_generation: self.owner.recycled_generation,
            predecessor_detached_generation: self.owner.predecessor_detached_generation,
            poisoned: self.owner.poisoned,
        }
    }

    pub(super) fn new() -> Self {
        Self {
            owner: DispatchGenerationOwnerV1::new().unwrap(),
        }
    }

    pub(super) const fn next_generation(&self) -> u64 {
        self.owner.next_generation
    }

    pub(super) fn reserve_one(
        &mut self,
        queue: QueueKeyV1,
        template: CompletionPacketTemplateV1,
    ) -> Result<DispatchEpochIdentityV1, Gfx942DispatchBindingErrorV1> {
        self.reserve_batch(queue, &[template])
    }

    pub(super) fn reserve_batch<const N: usize>(
        &mut self,
        queue: QueueKeyV1,
        templates: &[CompletionPacketTemplateV1; N],
    ) -> Result<DispatchEpochIdentityV1, Gfx942DispatchBindingErrorV1> {
        let dispatches: Vec<_> = templates
            .iter()
            .map(|template| template.generations())
            .collect();
        let roster = completion_dispatch_roster_v1(&dispatches)?;
        self.owner.reserve(queue, roster)
    }

    pub(super) fn mark_published<const N: usize>(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let occurrence = completion
            .occurrence_v1()
            .map_err(Gfx942DispatchBindingErrorV1::Completion)?;
        self.owner.mark_published(identity, occurrence)
    }

    pub(super) fn cancel(
        &mut self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.owner.cancel_epoch(identity)
    }

    pub(super) fn validate_published<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<CompletionBatchOccurrenceV1, Gfx942DispatchBindingErrorV1> {
        let occurrence = completion
            .occurrence_v1()
            .map_err(Gfx942DispatchBindingErrorV1::Completion)?;
        self.owner.validate_published(identity, occurrence)?;
        Ok(occurrence)
    }

    pub(super) fn mark_completed(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.owner.complete_epoch(identity, completion)
    }

    pub(super) fn validate_completed<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completed: &Gfx942CompletedBatchV1<N>,
    ) -> Result<CompletionBatchOccurrenceV1, Gfx942DispatchBindingErrorV1> {
        let occurrence = completed
            .occurrence_v1()
            .map_err(Gfx942DispatchBindingErrorV1::Completion)?;
        self.owner.validate_completed(identity, occurrence)?;
        Ok(occurrence)
    }

    pub(super) fn mark_recycled(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.owner.recycle_epoch(identity, completion)
    }

    pub(super) fn live_epoch_count(&self) -> usize {
        self.owner
            .slots
            .iter()
            .filter(|slot| slot.phase != DispatchEpochPhaseV1::Vacant)
            .count()
    }

    pub(super) fn poison(&mut self) {
        self.owner.poison();
    }

    pub(super) fn ensure_releasable(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.owner.ensure_prepared()
    }
}

#[cfg(test)]
impl TestOnlyDispatchGenerationOwnerV1 {
    pub(super) fn after_recycled(
        predecessor_generation: u64,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let next_generation = predecessor_generation
            .checked_add(1)
            .filter(|generation| generation.checked_add(1).is_some())
            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
        Ok(Self {
            next_generation,
            active_generation: None,
            predecessor_generation,
            last_cancelled_generation: None,
        })
    }

    pub(super) fn bind_one(&mut self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        if self.active_generation.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let generation = self.next_generation;
        self.next_generation = generation
            .checked_add(1)
            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
        self.active_generation = Some(generation);
        Ok(generation)
    }

    pub(super) fn cancel_binding(
        &mut self,
        generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.active_generation != Some(generation) {
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
        }
        self.active_generation = None;
        self.last_cancelled_generation = Some(generation);
        Ok(())
    }

    pub(super) const fn predecessor_generation(&self) -> u64 {
        self.predecessor_generation
    }

    pub(super) const fn last_cancelled_generation(&self) -> Option<u64> {
        self.last_cancelled_generation
    }

    pub(super) fn active_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.active_generation
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedCodeIdentityV1 {
    authenticated: KernelIdentityInputsV1,
    dispatch_abi_identity: [u8; 32],
    materialized_sha256: [u8; 32],
    mapping: MemoryMappingKeyV1,
    descriptor_address: ObservedGpuAddressV1,
}

#[derive(Debug, Eq, PartialEq)]
struct RetainedDataPremiseV1 {
    layout: Gfx942FixedDispatchDataLayoutV1,
    role_identity: [u8; 32],
    valid_bytes: u64,
    effect: Option<DeviceDataEffectV1>,
    initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    fully_initialized: bool,
    writable_ranges: Box<[CompletedWritableRangeV1]>,
    completed_snapshots: Box<[CompletedSnapshotRangeV1]>,
}

#[derive(Debug, Eq, PartialEq)]
struct PublicRetainedDataPlanV1 {
    layout: Gfx942FixedDispatchDataLayoutV1,
    effect: Option<DeviceDataEffectV1>,
    fully_initialized: bool,
    writable_ranges: Box<[CompletedWritableRangeV1]>,
    completed_snapshots: Box<[CompletedSnapshotRangeV1]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletedWritableRangeV1 {
    offset: u64,
    byte_len: u64,
}

/// One actual mapped C3 authority returned only after exact C4 recycle.
pub(super) struct ReturnedDispatchDataLeaseV1 {
    authority: DispatchDataAuthorityV1,
    premise: RetainedDataPremiseV1,
}

impl ReturnedDispatchDataLeaseV1 {
    pub(super) const fn role_identity(&self) -> [u8; 32] {
        self.premise.role_identity
    }

    pub(super) const fn valid_bytes(&self) -> u64 {
        self.premise.valid_bytes
    }

    pub(super) const fn effect(&self) -> Option<DeviceDataEffectV1> {
        self.premise.effect
    }

    pub(super) const fn is_fully_initialized(&self) -> bool {
        self.premise.fully_initialized
    }

    pub(super) fn into_data(self) -> Gfx942FixedDispatchDataV1 {
        dispatch_data_from_authority_v1(self.authority, self.premise.fully_initialized)
    }
}

fn dispatch_data_from_authority_v1(
    authority: DispatchDataAuthorityV1,
    fully_initialized: bool,
) -> Gfx942FixedDispatchDataV1 {
    match (authority, fully_initialized) {
        (DispatchDataAuthorityV1::Device(authority), true) => {
            Gfx942FixedDispatchDataV1::initialized_storage(authority.into_lease())
        }
        (DispatchDataAuthorityV1::Device(authority), false) => {
            Gfx942FixedDispatchDataV1::uninitialized(authority.into_lease())
        }
        (DispatchDataAuthorityV1::HostVisible(authority), true) => {
            Gfx942FixedDispatchDataV1::host_visible_initialized(
                Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(
                    authority.into_token(),
                ),
            )
        }
        (DispatchDataAuthorityV1::HostVisible(authority), false) => {
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(authority.into_token())
        }
    }
}

/// Exact returned C3 set from a never-published or recycled dispatch owner.
pub(super) struct ReturnedDispatchDataV1 {
    generation: u64,
    data: Vec<ReturnedDispatchDataLeaseV1>,
}

impl ReturnedDispatchDataV1 {
    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn data(&self) -> &[ReturnedDispatchDataLeaseV1] {
        &self.data
    }

    pub(super) fn into_data(self) -> Vec<ReturnedDispatchDataLeaseV1> {
        self.data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedDispatchPacketV1 {
    geometry: AqlDispatchGeometryV1,
    ordering: AqlDispatchOrderingV1,
    private_segment_size: u32,
    group_segment_size: u32,
    kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64,
    kernarg_mapping: MemoryMappingKeyV1,
    kernarg_layout_identity: [u8; 32],
    code_bound_kernarg_layout: bool,
    code_index: usize,
    conditional_fill: bool,
}

fn prepared_kernarg_layout_matches_code(
    code_bound: bool,
    kernarg_layout_identity: [u8; 32],
    dispatch_abi_identity: [u8; 32],
) -> bool {
    dispatch_template_abi_matches_body!(
        dispatch_rust_expr,
        code_bound,
        kernarg_layout_identity,
        dispatch_abi_identity
    )
}

fn prepare_dispatch_templates_v1(
    packets: &[PreparedDispatchPacketV1],
    code_identity: &[ResolvedCodeIdentityV1],
    queue: QueueKeyV1,
    generation: u64,
) -> Result<Vec<CompletionPacketTemplateV1>, Gfx942DispatchBindingErrorV1> {
    dispatch_prepare_templates_body!(
        dispatch_rust_expr,
        packets,
        code_identity,
        queue,
        generation
    )
}

/// Queue-retained real resource owner for one prepared batch shape.
pub(super) struct DispatchResourceOwnerV1 {
    code: Vec<CodeAuthority>,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    kernarg: KernargAuthority,
    packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<DispatchDataAuthorityV1>,
    data_premises: Vec<RetainedDataPremiseV1>,
    generation: DispatchGenerationOwnerV1,
    persistent_control: PersistentFixedDispatchControlStateV1,
    conditional_fill: Option<Box<conditional_fill::ConditionalFillStorageV1>>,
}

impl DispatchResourceOwnerV1 {
    #[cfg(feature = "hardware-qualification")]
    pub(super) fn qualification_single_write_host_data_identity_v1(
        &self,
        vm: fe2o3_runtime_model::VmKeyV1,
    ) -> Option<crate::shared_memory::SharedGttAllocationIdentityV1> {
        let [DispatchDataAuthorityV1::HostVisible(data)] = self.data.as_slice() else {
            return None;
        };
        let [premise] = self.data_premises.as_slice() else {
            return None;
        };
        if self.data[0].vm() != vm
            || premise.layout.requested_bytes == 0
            || premise.effect != Some(DeviceDataEffectV1::WriteOnly)
        {
            return None;
        }
        Some(data.storage_identity())
    }

    pub(super) fn retained_published_batch_observation_v1<const N: usize>(
        &self,
        batch: &Gfx942DispatchBatchV1<N>,
    ) -> Option<[u8; 32]> {
        let occurrence = batch.completion.occurrence_v1().ok()?;
        r66_retained_published_occurrence_observation_v1(
            &self.generation,
            batch.identity,
            occurrence,
        )
    }

    pub(super) fn validate_persistent_replay_v1(
        &self,
        identity: PersistentFixedDispatchControlIdentityV1,
        predecessor_generation: u64,
    ) -> Result<DeviceDataEffectV1, Gfx942DispatchBindingErrorV1> {
        validate_persistent_control_replay_v1(
            self.persistent_control,
            self.data.len(),
            self.data_premises.len(),
            &self.generation,
            identity,
            predecessor_generation,
        )
    }

    pub(super) fn retain_persistent_replay_data_in_place_v1(
        &mut self,
        memory: &SharedGttMemorySessionV1,
        identity: PersistentFixedDispatchControlIdentityV1,
        data: &mut Option<Gfx942FixedDispatchDataV1>,
        predecessor_generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.retain_persistent_replay_data_with_v1(identity, data, predecessor_generation, |data| {
            memory.retain_persistent_replay_data_in_place_v1(data)
        })
    }

    fn retain_persistent_replay_data_with_v1(
        &mut self,
        identity: PersistentFixedDispatchControlIdentityV1,
        data: &mut Option<Gfx942FixedDispatchDataV1>,
        predecessor_generation: u64,
        retain: impl FnOnce(
            &mut Option<Gfx942FixedDispatchDataV1>,
        )
            -> Result<crate::shared_memory::RetainedDispatchDataV1, MemorySessionError>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.validate_persistent_replay_v1(identity, predecessor_generation)?;
        if self.data.capacity() == 0 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let input = data
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let retained_initialized = self
            .data_premises
            .first()
            .is_some_and(|premise| premise.fully_initialized);
        if input.layout() != identity.data_layout
            || input.sdma_storage_identity() != identity.data_storage
            || !persistent_replay_initialization_is_admitted_v1(
                identity.effect,
                input.is_fully_initialized(),
                retained_initialized,
            )
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: 0,
                detail: "persistent control replay storage, extent, or initialization",
            });
        }
        let premise = self
            .data_premises
            .first_mut()
            .expect("validated persistent replay premise");
        let input = retain(data)?;
        self.data.push(input.authority);
        premise.initialized_content = input.initialized_content;
        // Detached storage may have been overwritten since the retained dispatch.
        premise.fully_initialized = input.fully_initialized;
        self.persistent_control = PersistentFixedDispatchControlStateV1::Attached(
            BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity),
        );
        Ok(())
    }

    pub(super) const fn persistent_data_is_detached_v1(&self) -> bool {
        matches!(
            self.persistent_control,
            PersistentFixedDispatchControlStateV1::DataDetached(_)
        )
    }

    pub(super) fn validate_detached_persistent_control_release_v1(
        &self,
        expected_generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        validate_detached_persistent_control_release_state_v1(
            self.persistent_control,
            self.data.len(),
            self.data_premises.len(),
            self.generation.returned_generation()?,
            expected_generation,
        )
    }

    /// Consumes only retained immutable dispatch control after its exact data
    /// authority was already detached and restored to the SDMA owner.
    #[cfg(test)]
    pub(super) fn release_detached_persistent_control_v1(
        self,
        memory: &mut SharedGttMemorySessionV1,
        expected_generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        control_release::release_detached_persistent_with_v1(
            control_release::ReturningControlCleanupCustodyV1::new(
                self,
                control_release::ReturningControlModeV1::DetachedPersistent {
                    expected_generation,
                },
            ),
            memory,
            core::mem::forget,
        )
    }

    pub(super) fn detach_persistent_replay_data_after_recycle_v1(
        &mut self,
    ) -> Result<(u64, Vec<Gfx942FixedDispatchDataV1>), Gfx942DispatchBindingErrorV1> {
        let generation = self.generation.returned_generation()?;
        let expected_count = match self.persistent_control {
            PersistentFixedDispatchControlStateV1::Attached(identity) => identity.binding_count(),
            _ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        };
        if self.data.len() != expected_count || self.data_premises.len() != expected_count {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let mut data = Vec::new();
        data.try_reserve_exact(expected_count).map_err(|_| {
            Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                operation: "persistent dispatch data detach",
            }
        })?;
        while let Some(authority) = self.data.pop() {
            let premise = &self.data_premises[self.data.len()];
            data.push(dispatch_data_from_authority_v1(
                authority,
                premise.fully_initialized,
            ));
        }
        data.reverse();
        self.persistent_control = match self.persistent_control {
            PersistentFixedDispatchControlStateV1::Attached(identity) => {
                PersistentFixedDispatchControlStateV1::DataDetached(identity)
            }
            _ => unreachable!("validated persistent control state"),
        };
        Ok((generation, data))
    }

    pub(super) fn device_authorities_inline_v1(
        &self,
    ) -> ArrayVec<&Gfx942DeviceMemoryDispatchAuthorityV1, MAX_DISPATCH_DATA_LEASES_V1> {
        let mut authorities = ArrayVec::new();
        for authority in &self.data {
            if let DispatchDataAuthorityV1::Device(authority) = authority {
                authorities.push(authority);
            }
        }
        authorities
    }

    pub(super) fn persistent_device_roster_matches_v1(
        &self,
        identities: &[crate::shared_memory::Gfx942DeviceMemoryIdentityV1],
    ) -> bool {
        let PersistentFixedDispatchControlStateV1::Attached(control) = self.persistent_control
        else {
            return false;
        };
        self.generation.ensure_not_poisoned().is_ok()
            && !identities.is_empty()
            && control.binding_count() == identities.len()
            && self.data.len() == identities.len()
            && self.data_premises.len() == identities.len()
            && self
                .data
                .iter()
                .zip(identities)
                .enumerate()
                .all(|(index, (data, identity))| {
                    control.data_storage[index]
                        == Some(Gfx942SdmaBufferStorageIdentityV1::Device(*identity))
                        && matches!(data, DispatchDataAuthorityV1::Device(authority)
                        if authority.storage_identity() == *identity)
                })
    }

    fn preflight_templates<const N: usize>(
        &self,
        queue: QueueKeyV1,
    ) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        dispatch_template_preflight_body!(dispatch_rust_expr, self, N, queue)
    }

    pub(super) fn bind_templates<const N: usize>(
        &mut self,
        queue: QueueKeyV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        match (
            &self.conditional_fill,
            self.packets.iter().any(|packet| packet.conditional_fill),
        ) {
            (Some(premises), true) => premises.revalidate(self)?,
            (None, false) => {}
            _ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        }
        dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)
    }

    pub(super) fn cancel_binding(
        &mut self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        dispatch_cancel_binding_body!(dispatch_rust_expr, self, identity)
    }

    pub(super) fn mark_published<const N: usize>(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .mark_published(identity, completion.occurrence_v1()?)
    }

    pub(super) fn mark_published_occurrence(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.mark_published(identity, completion)
    }

    pub(super) fn validate_published<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .validate_published(identity, completion.occurrence_v1()?)
    }

    pub(super) fn validate_published_occurrence(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.validate_published(identity, completion)
    }

    pub(super) fn mark_completed<const N: usize>(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .complete_epoch(identity, completion.occurrence_v1()?)
    }

    pub(super) fn mark_completed_occurrence(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.complete_epoch(identity, completion)
    }

    pub(super) fn validate_completed<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .validate_completed(identity, completion.occurrence_v1()?)
    }

    pub(super) fn mark_recycled_occurrence(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.recycle_epoch(identity, completion)
    }

    pub(super) fn poison(&mut self) {
        self.generation.poison();
    }

    pub(super) fn ensure_releasable(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_prepared()
    }

    pub(super) fn ensure_returnable(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.generation.returned_generation()
    }

    pub(super) fn recycled_data_shape_v1(
        &self,
    ) -> Result<(u64, usize), Gfx942DispatchBindingErrorV1> {
        let generation = self.ensure_returnable()?;
        if self.data.len() != self.data_premises.len() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.data.len().min(self.data_premises.len()),
                detail: "retained data/premise cardinality",
            });
        }
        Ok((generation, self.data.len()))
    }

    #[cfg(test)]
    pub(in crate::queue) fn set_recycled_generation_for_test(&mut self, generation: Option<u64>) {
        self.generation.recycled_generation = generation;
    }

    #[cfg(test)]
    pub(in crate::queue) fn remove_recycled_premise_for_test(&mut self) {
        self.data_premises
            .pop()
            .expect("fixture retains data premises");
    }

    pub(super) fn ensure_returnable_for_destroy(
        &self,
    ) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        validate_returning_destroy_control_state_v1(
            self.persistent_control,
            self.data.len(),
            self.data_premises.len(),
        )?;
        self.generation.returning_destroy_generation()
    }

    pub(super) fn read_completed_host_visible(
        &self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942CompletedDispatchReadRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, Gfx942DispatchBindingErrorV1> {
        let generation =
            validate_completed_read_request(&self.generation, &self.data_premises, request)?;
        let authority =
            self.data
                .get(request.data_index)
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                    index: request.data_index,
                    detail: "completed read authority ordinal",
                })?;
        if authority.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed read requires coherent host-visible storage",
            });
        }
        let DispatchDataAuthorityV1::HostVisible(authority) = authority else {
            unreachable!("coherent kind check matched retained authority");
        };
        let bytes = memory.copy_completed_dispatch_host_data_subrange(
            authority,
            request.offset,
            request.byte_len,
        )?;
        Ok(Gfx942CompletedDispatchReadbackV1 {
            dispatch_generation: generation,
            data_index: request.data_index,
            offset: request.offset,
            bytes,
        })
    }

    pub(super) fn read_completed_host_visible_into(
        &self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        validate_completed_read_request(&self.generation, &self.data_premises, request)?;
        validate_completed_read_destination(request, destination.len())?;
        let authority =
            self.data
                .get(request.data_index)
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                    index: request.data_index,
                    detail: "completed read authority ordinal",
                })?;
        let DispatchDataAuthorityV1::HostVisible(authority) = authority else {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed read requires coherent host-visible storage",
            });
        };
        memory.copy_completed_dispatch_host_data_subrange_into(
            authority,
            request.offset,
            destination,
        )?;
        Ok(())
    }

    pub(super) fn read_completed_host_visible_snapshot(
        &self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942CompletedDispatchSnapshotRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, Gfx942DispatchBindingErrorV1> {
        let generation =
            validate_completed_snapshot_request(&self.generation, &self.data_premises, request)?;
        let authority =
            self.data
                .get(request.data_index)
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                    index: request.data_index,
                    detail: "completed snapshot authority ordinal",
                })?;
        if authority.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed snapshot requires coherent host-visible storage",
            });
        }
        let DispatchDataAuthorityV1::HostVisible(authority) = authority else {
            unreachable!("coherent kind check matched retained authority");
        };
        let bytes = memory.copy_completed_dispatch_host_data_subrange(
            authority,
            request.offset,
            request.byte_len,
        )?;
        Ok(Gfx942CompletedDispatchReadbackV1 {
            dispatch_generation: generation,
            data_index: request.data_index,
            offset: request.offset,
            bytes,
        })
    }

    pub(super) fn read_completed_initialized_host_visible_into(
        &self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        validate_completed_initialized_read_request(
            &self.generation,
            &self.data_premises,
            request,
        )?;
        validate_completed_read_destination(request, destination.len())?;
        let Some(DispatchDataAuthorityV1::HostVisible(authority)) =
            self.data.get(request.data_index)
        else {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "initialized read requires retained coherent host-visible storage",
            });
        };
        memory.copy_completed_dispatch_host_data_subrange_into(
            authority,
            request.offset,
            destination,
        )?;
        Ok(())
    }

    pub(super) fn overwrite_recycled_host_visible(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        request: Gfx942RecycledDispatchWriteRequestV1,
        source: &[u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let generation = self.generation.returned_generation()?;
        if request.dispatch_generation != generation {
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
        }
        let premise = self.data_premises.get(request.data_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite premise ordinal",
            },
        )?;
        let source_len =
            u64::try_from(source.len()).map_err(|_| Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite source length",
            })?;
        let end = request.offset.checked_add(source_len).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite range overflow",
            },
        )?;
        if source.is_empty() || end > premise.valid_bytes || !premise.fully_initialized {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite range or initialization",
            });
        }
        let authority = self.data.get_mut(request.data_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite authority ordinal",
            },
        )?;
        let DispatchDataAuthorityV1::HostVisible(authority) = authority else {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "recycled overwrite requires coherent host-visible storage",
            });
        };
        memory.overwrite_recycled_dispatch_host_data_subrange(authority, request.offset, source)?;
        Ok(())
    }

    pub(super) fn release(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        control_release::release_ordinary_with_v1(
            control_release::ReturningControlCleanupCustodyV1::new(
                self,
                control_release::ReturningControlModeV1::Ordinary,
            ),
            memory,
            core::mem::forget,
        )
    }

    /// Releases code and kernarg while returning the exact mapped C3 set.
    ///
    /// The generation owner admits this transition only after a matching C4
    /// completion was observed and its signal was recycled. The returned
    /// authorities retain no public address, handle, pointer, or descriptor.
    pub(super) fn release_non_data_after_recycle(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
        self.release_non_data(
            memory,
            control_release::ReturningControlModeV1::AfterRecycle,
        )
    }

    /// Releases code and kernarg while returning never-published or recycled data.
    pub(super) fn release_non_data_for_returning_destroy(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
        self.release_non_data(
            memory,
            control_release::ReturningControlModeV1::ReturningDestroy,
        )
    }

    /// Releases code and kernarg while returning the persistent data owner on
    /// both success and terminal cleanup failure.
    pub(super) fn release_persistent_data_before_publication(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<
        (u64, Vec<Gfx942FixedDispatchDataV1>),
        (Gfx942DispatchBindingErrorV1, Vec<Gfx942FixedDispatchDataV1>),
    > {
        self.release_persistent_data(
            memory,
            control_release::ReturningControlModeV1::PersistentBeforePublication,
        )
    }

    /// Releases code and kernarg after exact recycle while retaining the
    /// persistent data owner on every later failure.
    pub(super) fn release_persistent_data_after_recycle(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<
        (u64, Vec<Gfx942FixedDispatchDataV1>),
        (Gfx942DispatchBindingErrorV1, Vec<Gfx942FixedDispatchDataV1>),
    > {
        self.release_persistent_data(
            memory,
            control_release::ReturningControlModeV1::PersistentAfterRecycle,
        )
    }

    fn release_persistent_data(
        self,
        memory: &mut SharedGttMemorySessionV1,
        mode: control_release::ReturningControlModeV1,
    ) -> Result<
        (u64, Vec<Gfx942FixedDispatchDataV1>),
        (Gfx942DispatchBindingErrorV1, Vec<Gfx942FixedDispatchDataV1>),
    > {
        control_release::release_persistent_with_v1(
            control_release::ReturningControlCleanupCustodyV1::new(self, mode),
            memory,
            core::mem::forget,
        )
    }

    fn release_non_data(
        self,
        memory: &mut SharedGttMemorySessionV1,
        mode: control_release::ReturningControlModeV1,
    ) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
        control_release::release_returning_with_v1(
            control_release::ReturningControlCleanupCustodyV1::new(self, mode),
            memory,
            core::mem::forget,
        )
    }

    fn require_prepared(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.ensure_prepared()
    }
}

fn r66_retained_published_occurrence_observation_v1(
    generation: &DispatchGenerationOwnerV1,
    identity: DispatchEpochIdentityV1,
    occurrence: CompletionBatchOccurrenceV1,
) -> Option<[u8; 32]> {
    generation.validate_published(identity, occurrence).ok()?;
    let mut hash = Sha256::new();
    match generation.capacity_profile {
        FixedDispatchCapacityProfileV1::Default64 => {
            hash.update(b"fe2o3.r66.retained-persistent-dispatch.v1\0");
        }
        FixedDispatchCapacityProfileV1::Qualification1024 => {
            hash.update(b"fe2o3.qualification1024.retained-dispatch.v1\0");
            hash.update(1024_u16.to_le_bytes());
        }
    }
    hash.update(occurrence.roster_sha256);
    hash.update(identity.recipe_occurrence.to_le_bytes());
    match generation.capacity_profile {
        FixedDispatchCapacityProfileV1::Default64 => {
            hash.update([u8::try_from(identity.slot_index).ok()?]);
        }
        FixedDispatchCapacityProfileV1::Qualification1024 => {
            hash.update(identity.slot_index.to_le_bytes());
        }
    }
    hash.update(identity.slot_generation.to_le_bytes());
    hash.update(identity.dispatch_generation.to_le_bytes());
    Some(hash.finalize().into())
}

const fn persistent_replay_initialization_is_admitted_v1(
    effect: DeviceDataEffectV1,
    incoming_initialized: bool,
    _retained_initialized: bool,
) -> bool {
    !effect.reads() || incoming_initialized
}

fn validate_detached_persistent_control_release_state_v1(
    state: PersistentFixedDispatchControlStateV1,
    data_authority_count: usize,
    data_premise_count: usize,
    returned_generation: u64,
    expected_generation: u64,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let expected_premises = match state {
        PersistentFixedDispatchControlStateV1::DataDetached(identity) => identity.binding_count(),
        _ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
    };
    if data_authority_count != 0
        || data_premise_count != expected_premises
        || returned_generation != expected_generation
    {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
    }
    Ok(())
}

fn validate_persistent_control_replay_v1(
    state: PersistentFixedDispatchControlStateV1,
    data_authority_count: usize,
    data_premise_count: usize,
    generation: &DispatchGenerationOwnerV1,
    requested: PersistentFixedDispatchControlIdentityV1,
    predecessor_generation: u64,
) -> Result<DeviceDataEffectV1, Gfx942DispatchBindingErrorV1> {
    let PersistentFixedDispatchControlStateV1::DataDetached(retained) = state else {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
    };
    let Some(retained) = retained.as_single() else {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
    };
    if retained.queue != requested.queue {
        return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
    }
    if retained.semantic_sha256 != requested.semantic_sha256 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "persistent control code, ABI, packet, geometry, or kernarg identity",
        ));
    }
    if retained.content_role != requested.content_role {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent control content role",
        });
    }
    if retained.data_layout != requested.data_layout {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent control storage layout or extent",
        });
    }
    if retained.data_storage != requested.data_storage {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent control mapped storage identity",
        });
    }
    if retained.effect != requested.effect {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 0,
            detail: "persistent control metadata effect",
        });
    }
    if data_authority_count != 0 || data_premise_count != 1 {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
    }
    if generation.returned_generation()? != predecessor_generation {
        return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
    }
    Ok(requested.effect)
}

fn validate_returning_destroy_control_state_v1(
    state: PersistentFixedDispatchControlStateV1,
    data_authority_count: usize,
    data_premise_count: usize,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if data_authority_count != data_premise_count
        || matches!(
            state,
            PersistentFixedDispatchControlStateV1::DataDetached(_)
        )
    {
        return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
    }
    Ok(())
}

fn validate_completed_read_request(
    generation_owner: &DispatchGenerationOwnerV1,
    premises: &[RetainedDataPremiseV1],
    request: Gfx942CompletedDispatchReadRequestV1,
) -> Result<u64, Gfx942DispatchBindingErrorV1> {
    let generation = generation_owner.returned_generation()?;
    if request.dispatch_generation == 0 || request.dispatch_generation != generation {
        return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
    }
    let premise =
        premises
            .get(request.data_index)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed read data ordinal",
            })?;
    if premise.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read requires coherent host-visible storage",
        });
    }
    if !matches!(
        premise.effect,
        Some(DeviceDataEffectV1::WriteOnly | DeviceDataEffectV1::ReadWrite)
    ) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read requires inspected write access",
        });
    }
    let end = request.offset.checked_add(request.byte_len).ok_or(
        Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read range overflow",
        },
    )?;
    if request.byte_len == 0 || end > premise.valid_bytes {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read range",
        });
    }
    let mut matching_range = None;
    for range in &premise.writable_ranges {
        if ranges_overlap_u64(
            range.offset,
            range.byte_len,
            request.offset,
            request.byte_len,
        ) && matching_range.replace(range).is_some()
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed read requires one inspected writable range",
            });
        }
    }
    let Some(matching_range) = matching_range else {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read requires one inspected writable range",
        });
    };
    let writable_end = matching_range.offset.checked_add(matching_range.byte_len);
    if request.offset < matching_range.offset
        || writable_end.is_none_or(|writable_end| end > writable_end)
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read requires one inspected writable range",
        });
    }
    Ok(generation)
}

fn validate_completed_read_destination(
    request: Gfx942CompletedDispatchReadRequestV1,
    destination_len: usize,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if u64::try_from(destination_len).ok() != Some(request.byte_len) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed read destination length",
        });
    }
    Ok(())
}

fn validate_completed_initialized_read_request(
    generation_owner: &DispatchGenerationOwnerV1,
    premises: &[RetainedDataPremiseV1],
    request: Gfx942CompletedDispatchReadRequestV1,
) -> Result<u64, Gfx942DispatchBindingErrorV1> {
    let generation = generation_owner.returned_generation()?;
    if request.dispatch_generation == 0 || request.dispatch_generation != generation {
        return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
    }
    let premise =
        premises
            .get(request.data_index)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "initialized read data ordinal",
            })?;
    // This distinct observation uses sealed initialization, not a writable
    // effect. Unreferenced initialized inputs remain part of the retained roster.
    if premise.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
        || !premise.fully_initialized
        || request.byte_len == 0
        || request
            .offset
            .checked_add(request.byte_len)
            .is_none_or(|end| end > premise.valid_bytes)
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "initialized read requires a bounded sealed coherent extent",
        });
    }
    Ok(generation)
}

fn validate_completed_snapshot_request(
    generation_owner: &DispatchGenerationOwnerV1,
    premises: &[RetainedDataPremiseV1],
    request: Gfx942CompletedDispatchSnapshotRequestV1,
) -> Result<u64, Gfx942DispatchBindingErrorV1> {
    let generation = generation_owner.returned_generation()?;
    if request.dispatch_generation == 0 || request.dispatch_generation != generation {
        return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
    }
    let premise =
        premises
            .get(request.data_index)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: request.data_index,
                detail: "completed snapshot data ordinal",
            })?;
    if premise.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed snapshot requires coherent host-visible storage",
        });
    }
    if !premise.fully_initialized {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed snapshot requires full initialization",
        });
    }
    let end = request.offset.checked_add(request.byte_len).ok_or(
        Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed snapshot range overflow",
        },
    )?;
    if request.byte_len == 0 || end > premise.valid_bytes {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed snapshot range",
        });
    }
    let exact_matches = premise
        .completed_snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.offset == request.offset && snapshot.byte_len == request.byte_len
        })
        .count();
    if exact_matches != 1 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: request.data_index,
            detail: "completed snapshot requires one exact admitted range",
        });
    }
    Ok(generation)
}

/// Linear published dispatch batch retaining one exact resource generation.
///
/// It has no public constructor or operation, and is neither `Clone` nor
/// `Copy`.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn consume<const N: usize>(_: Gfx942DispatchBatchV1<N>) {}
/// fn cannot_use_twice<const N: usize>(batch: Gfx942DispatchBatchV1<N>) {
///     consume(batch);
///     consume(batch);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn cannot_clone<const N: usize>(batch: Gfx942DispatchBatchV1<N>) {
///     let _ = batch.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn cannot_extract_addresses<const N: usize>(batch: &Gfx942DispatchBatchV1<N>) {
///     let _ = batch.kernel_object();
///     let _ = batch.kernarg_address();
///     let _ = batch.device_addresses();
/// }
/// ```
#[must_use = "a published dispatch batch must remain bound through completion"]
pub struct Gfx942DispatchBatchV1<const N: usize> {
    completion: Gfx942CompletionBatchV1<N>,
    identity: DispatchEpochIdentityV1,
}

#[cfg(feature = "cpu-runtime-fixtures")]
impl Gfx942DispatchBatchV1<1> {
    pub(super) fn cpu_fixture_parts(
        &self,
    ) -> (&Gfx942CompletionBatchV1<1>, DispatchEpochIdentityV1) {
        (&self.completion, self.identity)
    }
}

impl<const N: usize> fmt::Debug for Gfx942DispatchBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DispatchBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

/// Linear exact-batch completion before signal recycle.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942CompletedDispatchBatchV1;
///
/// fn consume<const N: usize>(_: Gfx942CompletedDispatchBatchV1<N>) {}
/// fn cannot_recycle_twice<const N: usize>(batch: Gfx942CompletedDispatchBatchV1<N>) {
///     consume(batch);
///     consume(batch);
/// }
/// ```
#[must_use = "completed dispatch resources remain retained until signal recycle"]
pub struct Gfx942CompletedDispatchBatchV1<const N: usize> {
    completion: Gfx942CompletedBatchV1<N>,
    identity: DispatchEpochIdentityV1,
}

#[cfg(feature = "cpu-runtime-fixtures")]
impl Gfx942CompletedDispatchBatchV1<1> {
    pub(super) fn cpu_fixture_parts(
        &self,
    ) -> (&Gfx942CompletedBatchV1<1>, DispatchEpochIdentityV1) {
        (&self.completion, self.identity)
    }
}

impl<const N: usize> fmt::Debug for Gfx942CompletedDispatchBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedDispatchBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Gfx942DispatchPollV1<const N: usize> {
    Pending(Gfx942DispatchBatchV1<N>),
    Ready(Gfx942CompletedDispatchBatchV1<N>),
}

/// Addressless progress from one exact fixed-dispatch completion scan.
///
/// Signal loads occur sequentially, not as one atomic snapshot. Counts record
/// what that scan observed, and the first pending index can already be stale by
/// the time this value is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DispatchProgressV1 {
    packet_count: u16,
    completed_count: u16,
    pending_count: u16,
    first_pending_batch_index: Option<u16>,
}

impl Gfx942DispatchProgressV1 {
    pub const fn packet_count(self) -> u16 {
        self.packet_count
    }

    pub const fn completed_count(self) -> u16 {
        self.completed_count
    }

    pub const fn pending_count(self) -> u16 {
        self.pending_count
    }

    /// Returns the earliest batch-local index observed pending in this scan.
    pub const fn first_pending_batch_index(self) -> Option<u16> {
        self.first_pending_batch_index
    }

    fn from_completion(progress: Gfx942CompletionProgressV1) -> Self {
        Self {
            packet_count: progress.packet_count(),
            completed_count: progress.completed_count(),
            pending_count: progress.pending_count(),
            first_pending_batch_index: progress.first_pending_batch_index(),
        }
    }
}

/// Linear dispatch custody paired with progress from the same completion scan.
#[derive(Debug)]
pub enum Gfx942DispatchPollWithProgressV1<const N: usize> {
    Pending {
        batch: Gfx942DispatchBatchV1<N>,
        progress: Gfx942DispatchProgressV1,
    },
    Ready {
        completed: Gfx942CompletedDispatchBatchV1<N>,
        progress: Gfx942DispatchProgressV1,
    },
}

pub(super) fn wrap_published<const N: usize>(
    completion: Gfx942CompletionBatchV1<N>,
    identity: DispatchEpochIdentityV1,
) -> Gfx942DispatchBatchV1<N> {
    Gfx942DispatchBatchV1 {
        completion,
        identity,
    }
}

pub(super) fn unwrap_published<const N: usize>(
    batch: Gfx942DispatchBatchV1<N>,
) -> (Gfx942CompletionBatchV1<N>, DispatchEpochIdentityV1) {
    (batch.completion, batch.identity)
}

pub(super) fn wrap_poll<const N: usize>(
    poll: Gfx942CompletionPollV1<N>,
    identity: DispatchEpochIdentityV1,
) -> Gfx942DispatchPollV1<N> {
    match poll {
        Gfx942CompletionPollV1::Pending(completion) => {
            Gfx942DispatchPollV1::Pending(wrap_published(completion, identity))
        }
        Gfx942CompletionPollV1::Ready(completion) => {
            Gfx942DispatchPollV1::Ready(Gfx942CompletedDispatchBatchV1 {
                completion,
                identity,
            })
        }
    }
}

pub(super) fn wrap_poll_with_progress<const N: usize>(
    poll: Gfx942CompletionPollWithProgressV1<N>,
    identity: DispatchEpochIdentityV1,
) -> Gfx942DispatchPollWithProgressV1<N> {
    match poll {
        Gfx942CompletionPollWithProgressV1::Pending { batch, progress } => {
            Gfx942DispatchPollWithProgressV1::Pending {
                batch: wrap_published(batch, identity),
                progress: Gfx942DispatchProgressV1::from_completion(progress),
            }
        }
        Gfx942CompletionPollWithProgressV1::Ready {
            completed,
            progress,
        } => Gfx942DispatchPollWithProgressV1::Ready {
            completed: Gfx942CompletedDispatchBatchV1 {
                completion: completed,
                identity,
            },
            progress: Gfx942DispatchProgressV1::from_completion(progress),
        },
    }
}

pub(super) fn unwrap_completed<const N: usize>(
    batch: Gfx942CompletedDispatchBatchV1<N>,
) -> (Gfx942CompletedBatchV1<N>, DispatchEpochIdentityV1) {
    (batch.completion, batch.identity)
}

pub(super) fn wrap_completed<const N: usize>(
    completion: Gfx942CompletedBatchV1<N>,
    identity: DispatchEpochIdentityV1,
) -> Gfx942CompletedDispatchBatchV1<N> {
    Gfx942CompletedDispatchBatchV1 {
        completion,
        identity,
    }
}

fn validate_gfx942_executable_profile(
    profile: AdmittedProfile,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if profile != AdmittedProfile::Gfx942XnackOffCov6 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "executable target is not gfx942:xnack-",
        ));
    }
    Ok(())
}

/// Builds real owned code, kernarg, and C3 data authorities without publishing.
pub(super) fn prepare_dispatch_resources<const N: usize>(
    memory: &mut SharedGttMemorySessionV1,
    kernel: ValidatedKernelEnvelope<'_>,
    geometry: [DispatchGeometryV1; N],
    kernargs: [TypedKernargImageV1; N],
    data: Vec<DeviceDataAllocationInputV1>,
) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    validate_gfx942_kernel_profile(&kernel)?;
    let generation = DispatchGenerationOwnerV1::new()?;
    let resources = kernel.resources();
    let plan = *kernel.envelope().plan();
    let image_len_u64 = plan
        .image_end()
        .checked_sub(plan.image_start())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("image range"))?;
    let image_len = usize::try_from(image_len_u64)
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("image size conversion"))?;
    if image_len == 0 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode("empty image"));
    }
    let descriptor_offset = kernel
        .selected_binding()
        .descriptor_address()
        .checked_sub(plan.image_start())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor precedes image",
        ))?;
    descriptor_offset
        .checked_add(KERNEL_DESCRIPTOR_BYTES_V1)
        .filter(|end| *end <= image_len_u64)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor outside image",
        ))?;
    if !descriptor_offset.is_multiple_of(KERNEL_DESCRIPTOR_BYTES_V1) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor alignment",
        ));
    }
    let kernarg_size = usize::try_from(resources.kernarg_segment_size())
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("kernarg size conversion"))?;
    if kernarg_size == 0 || kernarg_size > MAX_DISPATCH_KERNARG_BYTES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg size bound",
        ));
    }
    let kernarg_alignment = resources.kernarg_segment_alignment();
    if kernarg_alignment == 0 || !kernarg_alignment.is_power_of_two() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg alignment",
        ));
    }
    let kernarg_alignment = usize::try_from(kernarg_alignment)
        .ok()
        .filter(|alignment| *alignment <= 4096)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg alignment bound",
        ))?;
    let kernarg_stride = kernarg_size
        .checked_add(kernarg_alignment - 1)
        .map(|bytes| bytes & !(kernarg_alignment - 1))
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("kernarg stride"))?;
    let kernarg_arena_bytes =
        kernarg_stride
            .checked_mul(N)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                "kernarg arena size",
            ))?;
    validate_geometry(
        resources,
        kernel.selected_kernel().uniform_work_group_size(),
        &geometry,
    )?;
    validate_data_inputs(&data)?;
    validate_kernargs(&kernargs, kernarg_size, &data)?;
    let requests: Vec<_> = data
        .iter()
        .map(|input| (input.requested_bytes, input.alignment))
        .collect();
    memory.validate_gfx942_dispatch_allocation_requests(&requests)?;
    let mut data_authorities = Vec::with_capacity(data.len());
    let mut data_premises = Vec::with_capacity(data.len());
    for input in data {
        let premise = input.premise;
        let lease = memory.allocate_gfx942_device_memory(input.requested_bytes, input.alignment)?;
        let lease = memory.map_gfx942_device_memory(lease)?;
        let authority = memory.retain_gfx942_device_memory_for_dispatch(lease)?;
        data_premises.push(RetainedDataPremiseV1 {
            layout: Gfx942FixedDispatchDataLayoutV1 {
                kind: Gfx942FixedDispatchDataKindV1::DeviceLocal,
                requested_bytes: input.requested_bytes,
                alignment: input.alignment,
            },
            role_identity: premise.role_identity,
            valid_bytes: premise.valid_bytes,
            effect: Some(premise.effect),
            initialized_content: None,
            fully_initialized: false,
            writable_ranges: Box::new([]),
            completed_snapshots: Box::new([]),
        });
        data_authorities.push(DispatchDataAuthorityV1::Device(authority));
    }

    let mut code = memory.allocate_executable(image_len)?;
    let materialized_sha256 = memory.with_bytes_mut(&mut code, |bytes| {
        kernel
            .materialize_into(bytes)
            .map(|()| Sha256::digest(bytes).into())
    })?;
    let materialized_sha256 = match materialized_sha256 {
        Ok(digest) => digest,
        Err(_) => {
            let _ = memory.quarantine_queue_composition("dispatch code materialization failure");
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode("materialization"));
        }
    };
    let code = memory.seal_executable(code)?;
    let code = memory.map_executable_to_gpu(code)?;
    let code = memory.retain_aql_dispatch_code_resource(code)?;
    let descriptor_address = code
        .facts()
        .checked_gpu_subrange(descriptor_offset, KERNEL_DESCRIPTOR_BYTES_V1, 64)
        .and_then(|address| ObservedGpuAddressV1::new(address).ok())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "resolved descriptor address",
        ))?;
    let code_identity = ResolvedCodeIdentityV1 {
        authenticated: kernel.identity_inputs(),
        dispatch_abi_identity: kernel
            .dispatch_abi_identity()
            .unwrap_or_else(|| kernel.identity_inputs().closure_sha256()),
        materialized_sha256,
        mapping: code.facts().mapping(),
        descriptor_address,
    };

    let private_segment_size = u32::try_from(resources.private_segment_fixed_size())
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("private segment size"))?;
    let mut kernarg = memory.allocate_kernarg(kernarg_arena_bytes)?;
    memory.with_bytes_mut(&mut kernarg, |bytes| {
        bytes.fill(0);
        for (packet_index, typed) in kernargs.iter().enumerate() {
            let start = packet_index * kernarg_stride;
            let packet_bytes = &mut bytes[start..start + kernarg_size];
            packet_bytes.copy_from_slice(&typed.bytes);
            for patch in &typed.device_pointers {
                let address = data_authorities[patch.data_index]
                    .checked_gpu_subrange(
                        patch.data_byte_offset,
                        patch.required_bytes,
                        patch.required_alignment,
                    )
                    .expect("dispatch preflight checked device pointer range");
                packet_bytes[patch.byte_offset..patch.byte_offset + 8]
                    .copy_from_slice(&address.to_le_bytes());
            }
        }
    })?;
    let kernarg = memory.map_to_gpu(kernarg)?;
    let kernarg = memory.retain_aql_dispatch_kernarg_resource(kernarg)?;
    let mut packets = Vec::with_capacity(N);
    for (packet_index, (typed, dispatch_geometry)) in kernargs.into_iter().zip(geometry).enumerate()
    {
        let kernarg_offset = packet_index
            .checked_mul(kernarg_stride)
            .and_then(|offset| u64::try_from(offset).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg slice offset",
            })?;
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(
                kernarg_offset,
                kernarg_size as u64,
                kernarg_alignment as u64,
            )
            .and_then(|address| ObservedGpuAddressV1::new(address).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "mapped kernarg address",
            })?;
        let group_segment_size = u64::from(dispatch_geometry.dynamic_group_segment_bytes)
            .checked_add(resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet: packet_index,
                detail: "group segment size",
            })?;
        packets.push(PreparedDispatchPacketV1 {
            geometry: dispatch_geometry.geometry,
            ordering: AqlDispatchOrderingV1::WaitForPrior,
            private_segment_size,
            group_segment_size,
            kernarg_address,
            kernarg_alignment: kernarg_alignment as u64,
            kernarg_mapping: kernarg.facts().mapping(),
            kernarg_layout_identity: typed.layout_identity,
            code_bound_kernarg_layout: false,
            code_index: 0,
            conditional_fill: false,
        });
    }

    Ok(DispatchResourceOwnerV1 {
        code: vec![code],
        code_identity: vec![code_identity],
        kernarg,
        packets,
        data: data_authorities,
        data_premises,
        generation,
        persistent_control: PersistentFixedDispatchControlStateV1::Ordinary,
        conditional_fill: None,
    })
}

struct FixedDispatchProgramPlanV1 {
    image_len: usize,
    descriptor_offset: u64,
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
    implicit_kernarg: Option<Cov6ImplicitKernargPlanV1>,
}

struct FixedDispatchPacketPlanV1 {
    patches: Box<[DevicePointerPatchV1]>,
    implicit_kernarg: Option<Cov6ImplicitKernargValuesV1>,
    kernarg_offset: usize,
    kernarg_alignment: usize,
    private_segment_size: u32,
    group_segment_size: u32,
}

struct FixedDispatchPreparationPlanV1 {
    programs: Vec<FixedDispatchProgramPlanV1>,
    packets: Vec<FixedDispatchPacketPlanV1>,
    data: Vec<PublicRetainedDataPlanV1>,
    kernarg_arena_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cov6ImplicitKernargFieldKindV1 {
    BlockCount(usize),
    GroupSize(usize),
    Remainder(usize),
    GlobalOffset(usize),
    GridDimensions,
    DynamicLdsSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Cov6ImplicitKernargFieldV1 {
    kind: Cov6ImplicitKernargFieldKindV1,
    relative_offset: usize,
    byte_len: usize,
}

struct Cov6ImplicitKernargPlanV1 {
    byte_offset: usize,
    fields: Box<[Cov6ImplicitKernargFieldV1]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Cov6ImplicitKernargValuesV1 {
    dispatch_shape: Cov6ImplicitDispatchShapeV1,
    dynamic_lds_size: u32,
}

fn validate_packet_program_indices<const N: usize>(
    program_count: usize,
    packets: &[Gfx942FixedDispatchPacketV1; N],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    for (packet, input) in packets.iter().enumerate() {
        if input.program_index >= program_count {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "program index",
            });
        }
    }
    Ok(())
}

fn validate_gfx942_kernel_profile(
    kernel: &ValidatedKernelEnvelope<'_>,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    validate_gfx942_executable_profile(kernel.envelope().plan().profile())
}

pub(super) fn validate_gfx942_kernel_profiles(
    programs: &[ValidatedKernelEnvelope<'_>],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    programs.iter().try_for_each(validate_gfx942_kernel_profile)
}

fn plan_public_fixed_dispatch_resources<const N: usize>(
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    data_layouts: &[Gfx942FixedDispatchDataLayoutV1],
    data_initialized: &[bool],
) -> Result<FixedDispatchPreparationPlanV1, Gfx942DispatchBindingErrorV1> {
    plan_fixed_dispatch_resources_with_order(
        programs,
        packets,
        data_layouts,
        data_initialized,
        AqlDispatchOrderingV1::WaitForPrior,
    )
}

fn plan_fixed_dispatch_resources_with_order<const N: usize>(
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    data_layouts: &[Gfx942FixedDispatchDataLayoutV1],
    data_initialized: &[bool],
    ordering: AqlDispatchOrderingV1,
) -> Result<FixedDispatchPreparationPlanV1, Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    if programs.is_empty() || programs.len() > GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1 {
        return Err(Gfx942DispatchBindingErrorV1::ProgramCount {
            requested: programs.len(),
            maximum: GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1,
        });
    }
    validate_gfx942_kernel_profiles(programs)?;
    if data_layouts.is_empty() || data_layouts.len() > MAX_DISPATCH_DATA_LEASES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
            requested: data_layouts.len(),
            maximum: MAX_DISPATCH_DATA_LEASES_V1,
        });
    }
    if data_layouts.len() != data_initialized.len() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: data_layouts.len().min(data_initialized.len()),
            detail: "initialization/layout cardinality",
        });
    }
    if let Some(packet) = packets
        .iter()
        .position(|packet| packet.ordering != ordering)
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "multi-inflight recipe requires wait-for-prior ordering",
        });
    }
    validate_packet_program_indices(programs.len(), packets)?;

    let mut program_plans = Vec::with_capacity(programs.len());
    for kernel in programs {
        let resources = kernel.resources();
        let plan = *kernel.envelope().plan();
        let image_len_u64 = plan
            .image_end()
            .checked_sub(plan.image_start())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("image range"))?;
        let image_len = usize::try_from(image_len_u64)
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("image size conversion"))?;
        let descriptor_offset = kernel
            .selected_binding()
            .descriptor_address()
            .checked_sub(plan.image_start())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                "descriptor precedes image",
            ))?;
        if image_len == 0
            || descriptor_offset
                .checked_add(KERNEL_DESCRIPTOR_BYTES_V1)
                .is_none_or(|end| end > image_len_u64)
            || !descriptor_offset.is_multiple_of(KERNEL_DESCRIPTOR_BYTES_V1)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "descriptor image range",
            ));
        }
        let implicit_kernarg = validate_cov6_implicit_kernarg_layout(kernel.selected_kernel())?;
        validate_kernarg_resource_shape(resources)?;
        program_plans.push(FixedDispatchProgramPlanV1 {
            image_len,
            descriptor_offset,
            resources,
            implicit_kernarg,
        });
    }

    let mut data_effects = vec![None; data_layouts.len()];
    let mut data_writable_ranges = vec![Vec::new(); data_layouts.len()];
    let mut data_completed_snapshots = vec![Vec::new(); data_layouts.len()];
    let mut packet_plans = Vec::with_capacity(N);
    let mut kernarg_arena_bytes = 0usize;
    for (packet_index, input) in packets.iter().enumerate() {
        let program_plan = program_plans.get(input.program_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "program index",
            },
        )?;
        let kernel = &programs[input.program_index];
        let kernarg_size = usize::try_from(program_plan.resources.kernarg_segment_size())
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("kernarg size conversion"))?;
        if input.kernarg_bytes.len() != kernarg_size {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "exact kernarg byte extent",
            });
        }
        let kernarg_alignment = usize::try_from(program_plan.resources.kernarg_segment_alignment())
            .map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidCode("kernarg alignment conversion")
            })?;
        let kernarg_offset = align_up(kernarg_arena_bytes, kernarg_alignment).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg arena offset",
            },
        )?;
        kernarg_arena_bytes = kernarg_offset.checked_add(kernarg_size).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg arena extent",
            },
        )?;
        let patches = validate_public_packet_bindings(
            packet_index,
            kernel,
            input,
            data_layouts,
            PublicDataBindingStateV1 {
                effects: &mut data_effects,
                writable_ranges: &mut data_writable_ranges,
                completed_snapshots: &mut data_completed_snapshots,
            },
        )?;
        let geometry = DispatchGeometryV1::new(input.geometry, input.dynamic_group_segment_bytes);
        validate_geometry(
            program_plan.resources,
            kernel.selected_kernel().uniform_work_group_size(),
            &[geometry],
        )?;
        let implicit_kernarg = validate_and_derive_cov6_implicit_kernarg(
            packet_index,
            kernel.selected_kernel(),
            input,
            program_plan.implicit_kernarg.as_ref(),
        )?;
        let private_segment_size =
            u32::try_from(program_plan.resources.private_segment_fixed_size())
                .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("private segment size"))?;
        let group_segment_size = u64::from(input.dynamic_group_segment_bytes)
            .checked_add(program_plan.resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet: packet_index,
                detail: "group segment size",
            })?;
        packet_plans.push(FixedDispatchPacketPlanV1 {
            patches,
            implicit_kernarg,
            kernarg_offset,
            kernarg_alignment,
            private_segment_size,
            group_segment_size,
        });
    }
    let data = plan_public_retained_data(
        data_layouts,
        data_initialized,
        data_effects,
        data_writable_ranges,
        data_completed_snapshots,
    )?;

    Ok(FixedDispatchPreparationPlanV1 {
        programs: program_plans,
        packets: packet_plans,
        data,
        kernarg_arena_bytes,
    })
}

/// Converts initialized hidden arguments to the fixed queue's zero template.
///
/// The entire hidden suffix must equal the independently derived native values,
/// including padding. The existing fixed-dispatch planner then checks the exact
/// ABI, geometry, aliases and complete coherent-host roster. Lengths describe
/// future initialized storage; success creates no native initialization witness,
/// allocation, queue or execution authority. Unsupported fixed profiles reject.
pub fn project_gfx942_fixed_host_packet_v1(
    program: &ValidatedKernelEnvelope<'_>,
    packet: Gfx942FixedDispatchPacketV1,
    buffer_lengths: &[usize],
) -> Result<Gfx942FixedDispatchPacketV1, Gfx942DispatchBindingErrorV1> {
    project_fixed_host_packet_with_order_v1(
        program,
        packet,
        buffer_lengths,
        AqlDispatchOrderingV1::WaitForPrior,
    )
}

/// Inert independent fill transport only. Actual disjoint-range and original
/// native custody admission remains required by the separate arena constructor.
/// The ordinary fixed-packet projection continues to reject Independent.
pub fn project_gfx942_independent_fill_host_packet_v1(
    program: &ValidatedKernelEnvelope<'_>,
    packet: Gfx942FixedDispatchPacketV1,
    buffer_lengths: &[usize],
) -> Result<Gfx942FixedDispatchPacketV1, Gfx942DispatchBindingErrorV1> {
    if !packet.conditional_fill {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet: 0,
            detail: "independent arena member requires closed fill",
        });
    }
    project_fixed_host_packet_with_order_v1(
        program,
        packet,
        buffer_lengths,
        AqlDispatchOrderingV1::Independent,
    )
}

fn project_fixed_host_packet_with_order_v1(
    program: &ValidatedKernelEnvelope<'_>,
    mut packet: Gfx942FixedDispatchPacketV1,
    buffer_lengths: &[usize],
    ordering: AqlDispatchOrderingV1,
) -> Result<Gfx942FixedDispatchPacketV1, Gfx942DispatchBindingErrorV1> {
    validate_gfx942_kernel_profile(program)?;
    if buffer_lengths.is_empty() || buffer_lengths.len() > MAX_DISPATCH_DATA_LEASES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
            requested: buffer_lengths.len(),
            maximum: MAX_DISPATCH_DATA_LEASES_V1,
        });
    }
    let kernel = program.selected_kernel();
    if let Some(plan) = validate_cov6_implicit_kernarg_layout(kernel)? {
        let values = derive_cov6_implicit_kernarg_values(
            packet.geometry,
            packet.dynamic_group_segment_bytes,
            kernel.uniform_work_group_size(),
        )
        .map_err(|detail| Gfx942DispatchBindingErrorV1::Geometry { packet: 0, detail })?;
        let mut expected = [0; COV6_IMPLICIT_ARGUMENT_BYTES_V1];
        initialize_cov6_implicit_suffix(&mut expected, &plan.fields, values);
        let end = plan.byte_offset.checked_add(expected.len()).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: 0,
                detail: "initialized implicit-kernarg extent",
            },
        )?;
        let suffix = packet.kernarg_bytes.get_mut(plan.byte_offset..end).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: 0,
                detail: "initialized implicit-kernarg extent",
            },
        )?;
        if suffix != expected {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: 0,
                detail: "initialized implicit-kernarg bytes differ from native derivation",
            });
        }
        suffix.fill(0);
    }
    let layouts = buffer_lengths
        .iter()
        .enumerate()
        .map(|(index, &bytes)| {
            let layout = crate::shared_memory::coherent_host_layout_v1(bytes).map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index,
                    detail: "invalid future coherent-host layout",
                }
            })?;
            Ok(Gfx942FixedDispatchDataLayoutV1 {
                kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
                requested_bytes: layout.requested_bytes() as u64,
                alignment: crate::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
            })
        })
        .collect::<Result<Vec<_>, Gfx942DispatchBindingErrorV1>>()?;
    let plan = plan_fixed_dispatch_resources_with_order(
        core::slice::from_ref(program),
        core::array::from_ref(&packet),
        &layouts,
        &vec![true; layouts.len()],
        ordering,
    )?;
    let _ = conditional_fill::check_plan(
        core::slice::from_ref(program),
        core::array::from_ref(&packet),
        &plan,
        PersistentFixedDispatchControlStateV1::Ordinary,
    )?;
    Ok(packet)
}

/// Validates every deterministic replacement fixed-dispatch property without
/// consuming allocation, executable, packet, or queue custody.
///
/// The returned generation is the first generation the replacement queue will
/// publish. Success reserves one further counter value so a later begin cannot
/// fail immediately after confirmed predecessor destruction.
pub fn preflight_gfx942_fixed_dispatch_replacement<const N: usize>(
    ring_bytes: u32,
    programs: &[ValidatedKernelEnvelope<'_>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    data: &[Gfx942FixedDispatchDataV1],
    predecessor_generation: u64,
) -> Result<u64, Gfx942DispatchBindingErrorV1> {
    validate_fixed_batch_ring::<N>(ring_bytes)?;
    next_dispatch_generation_after_recycled_v1(predecessor_generation)?;
    let data_layouts: Vec<_> = data.iter().map(Gfx942FixedDispatchDataV1::layout).collect();
    let data_initialized: Vec<_> = data
        .iter()
        .map(Gfx942FixedDispatchDataV1::is_fully_initialized)
        .collect();
    let plan =
        plan_public_fixed_dispatch_resources(programs, packets, &data_layouts, &data_initialized)?;
    let _ = conditional_fill::check_plan(
        programs,
        packets,
        &plan,
        PersistentFixedDispatchControlStateV1::Ordinary,
    )?;
    Ok(predecessor_generation + 1)
}

/// Consumes inspected executable custody and exact mapped data authorities,
/// then prepares one addressless fixed batch without publishing it.
pub(super) fn prepare_public_fixed_dispatch_resources<const N: usize>(
    memory: &mut SharedGttMemorySessionV1,
    programs: Vec<ValidatedKernelEnvelope<'_>>,
    packets: [Gfx942FixedDispatchPacketV1; N],
    data: Vec<Gfx942FixedDispatchDataV1>,
) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
    prepare_public_fixed_dispatch_resources_with_generation(
        memory,
        programs,
        packets,
        data,
        DispatchGenerationOwnerV1::new()?,
    )
}

pub(super) fn prepare_public_fixed_dispatch_resources_after_recycle_with_capacity_in_place<
    const N: usize,
>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
    predecessor_generation: u64,
    capacity: &Gfx942FixedDispatchCapacityV1,
    prepared: &mut Option<PreparedDispatchGenerationV1>,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    capacity.validate_batch::<N>()?;
    custody.prepare_in_place(
        memory,
        programs,
        PreparedDispatchGenerationV1::take_for(
            prepared,
            capacity,
            DispatchGenerationSeedV1::Recycled(predecessor_generation),
        ),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

#[cfg(test)]
pub(super) fn prepare_public_fixed_dispatch_resources_after_detach_in_place<const N: usize>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
    predecessor_generation: u64,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    custody.prepare_in_place(
        memory,
        programs,
        DispatchGenerationOwnerV1::after_detached(predecessor_generation),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

pub(super) fn prepare_public_fixed_dispatch_resources_after_detach_with_capacity_in_place<
    const N: usize,
>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
    predecessor_generation: u64,
    capacity: &Gfx942FixedDispatchCapacityV1,
    prepared: &mut Option<PreparedDispatchGenerationV1>,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    capacity.validate_batch::<N>()?;
    custody.prepare_in_place(
        memory,
        programs,
        PreparedDispatchGenerationV1::take_for(
            prepared,
            capacity,
            DispatchGenerationSeedV1::Detached(predecessor_generation),
        ),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

pub(super) fn prepare_persistent_fixed_dispatch_resources_v1(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<1>,
    predecessor_generation: Option<u64>,
    control_identity: PersistentFixedDispatchControlIdentityV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let generation = DispatchGenerationOwnerV1::persistent_after_detached(predecessor_generation);
    custody.prepare_in_place(
        memory,
        programs,
        generation,
        PersistentFixedDispatchControlStateV1::Attached(
            BoundedPersistentFixedDispatchControlIdentityV1::from_single(control_identity),
        ),
    )
}

pub(super) fn prepare_three_binding_persistent_fixed_dispatch_resources_v1(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<1>,
    predecessor_generation: Option<u64>,
    control_identity: ThreeBindingPersistentFixedDispatchControlIdentityV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let generation = DispatchGenerationOwnerV1::persistent_after_detached(predecessor_generation);
    custody.prepare_in_place(
        memory,
        programs,
        generation,
        PersistentFixedDispatchControlStateV1::Attached(
            BoundedPersistentFixedDispatchControlIdentityV1::from_three(control_identity),
        ),
    )
}

fn recover_dispatch_input_v1(input: DispatchDataInputV1) -> Gfx942FixedDispatchDataV1 {
    match input.storage {
        DispatchDataInputStorageV1::Device(lease) => {
            if let Some(content) = input.initialized_content {
                match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                    lease, content,
                ) {
                    Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                    Err(lease) => Gfx942FixedDispatchDataV1::initialized_storage(lease),
                }
            } else if input.fully_initialized {
                Gfx942FixedDispatchDataV1::initialized_storage(lease)
            } else {
                Gfx942FixedDispatchDataV1::uninitialized(lease)
            }
        }
        DispatchDataInputStorageV1::HostVisible(token) if input.fully_initialized => {
            Gfx942FixedDispatchDataV1::host_visible_initialized(
                Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(token),
            )
        }
        DispatchDataInputStorageV1::HostVisible(token) => {
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(token)
        }
    }
}

pub(super) fn prepare_public_fixed_dispatch_resources_in_place<const N: usize>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    custody.prepare_in_place(
        memory,
        programs,
        DispatchGenerationOwnerV1::new(),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

pub(super) fn prepare_public_fixed_dispatch_resources_with_capacity_in_place<const N: usize>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
    capacity: &Gfx942FixedDispatchCapacityV1,
    prepared: &mut Option<PreparedDispatchGenerationV1>,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    capacity.validate_batch::<N>()?;
    custody.prepare_in_place(
        memory,
        programs,
        PreparedDispatchGenerationV1::take_for(prepared, capacity, DispatchGenerationSeedV1::Fresh),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

fn prepare_public_fixed_dispatch_resources_with_generation<const N: usize>(
    memory: &mut SharedGttMemorySessionV1,
    programs: Vec<ValidatedKernelEnvelope<'_>>,
    packets: [Gfx942FixedDispatchPacketV1; N],
    data: Vec<Gfx942FixedDispatchDataV1>,
    generation: DispatchGenerationOwnerV1,
) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
    // Legacy constructors share this sequencer; their outer custody is NATIVE-2.
    let mut custody = FixedDispatchPreparationCustodyV1::new(packets, data);
    custody.prepare_in_place(
        memory,
        &programs,
        Ok(generation),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )?;
    custody.take_completed()
}

fn validate_kernarg_resource_shape(
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let size = resources.kernarg_segment_size();
    let alignment = resources.kernarg_segment_alignment();
    if size == 0
        || size > MAX_DISPATCH_KERNARG_BYTES_V1 as u64
        || alignment == 0
        || !alignment.is_power_of_two()
        || alignment > 4096
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg resource shape",
        ));
    }
    Ok(())
}

#[cfg(feature = "engineering-gfx950")]
pub(crate) fn initialize_engineering_cov6_kernarg(
    kernel: &InspectedKernel,
    geometry: AqlDispatchGeometryV1,
    bytes: &mut [u8],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if bytes.len() as u64 != kernel.kernarg_segment_size() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "engineering kernarg size",
        ));
    }
    if let Some(plan) = validate_cov6_implicit_kernarg_layout(kernel)? {
        validate_caller_zero_cov6_implicit_suffix(0, bytes, &plan)?;
        let values =
            derive_cov6_implicit_kernarg_values(geometry, 0, kernel.uniform_work_group_size())
                .map_err(|detail| Gfx942DispatchBindingErrorV1::Geometry { packet: 0, detail })?;
        initialize_cov6_implicit_kernarg(bytes, &plan, values);
    }
    Ok(())
}

fn validate_cov6_implicit_kernarg_layout(
    kernel: &InspectedKernel,
) -> Result<Option<Cov6ImplicitKernargPlanV1>, Gfx942DispatchBindingErrorV1> {
    let hidden = kernel.hidden_arguments();
    if hidden.is_empty() {
        if kernel.implicit_argument_offset().is_some() || kernel.implicit_argument_size() != 0 {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "implicit-kernarg metadata without hidden fields",
            ));
        }
        return Ok(None);
    }

    let byte_offset = exact_cov6_implicit_kernarg_offset(
        kernel.implicit_argument_offset(),
        kernel.implicit_argument_size(),
        kernel.kernarg_segment_size(),
    )
    .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
        "exact trailing COV6 implicit-kernarg extent",
    ))?;

    let mut fields = Vec::with_capacity(hidden.len());
    for argument in hidden {
        let field = admitted_cov6_implicit_kernarg_field(argument.value_kind()).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidCode(
                "unsupported runtime or address implicit-kernarg field",
            ),
        )?;
        let observed_offset = argument
            .offset()
            .checked_sub(byte_offset as u64)
            .and_then(|offset| usize::try_from(offset).ok());
        if observed_offset != Some(field.relative_offset)
            || usize::try_from(argument.size()).ok() != Some(field.byte_len)
            || field
                .relative_offset
                .checked_add(field.byte_len)
                .is_none_or(|end| end > COV6_IMPLICIT_ARGUMENT_BYTES_V1)
            || fields
                .iter()
                .any(|prior: &Cov6ImplicitKernargFieldV1| prior.kind == field.kind)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "noncanonical COV6 implicit-kernarg field",
            ));
        }
        fields.push(field);
    }

    Ok(Some(Cov6ImplicitKernargPlanV1 {
        byte_offset,
        fields: fields.into_boxed_slice(),
    }))
}

fn exact_cov6_implicit_kernarg_offset(
    byte_offset: Option<u64>,
    byte_len: u64,
    kernarg_byte_len: u64,
) -> Option<usize> {
    let byte_offset = byte_offset?;
    if byte_len != COV6_IMPLICIT_ARGUMENT_BYTES
        || byte_offset.checked_add(byte_len) != Some(kernarg_byte_len)
    {
        return None;
    }
    usize::try_from(byte_offset).ok()
}

fn admitted_cov6_implicit_kernarg_field(
    kind: HiddenValueKind,
) -> Option<Cov6ImplicitKernargFieldV1> {
    let (kind, relative_offset, byte_len) = match kind {
        HiddenValueKind::BlockCountX => (
            Cov6ImplicitKernargFieldKindV1::BlockCount(0),
            COV6_BLOCK_COUNT_X_OFFSET_V1,
            4,
        ),
        HiddenValueKind::BlockCountY => (
            Cov6ImplicitKernargFieldKindV1::BlockCount(1),
            COV6_BLOCK_COUNT_Y_OFFSET_V1,
            4,
        ),
        HiddenValueKind::BlockCountZ => (
            Cov6ImplicitKernargFieldKindV1::BlockCount(2),
            COV6_BLOCK_COUNT_Z_OFFSET_V1,
            4,
        ),
        HiddenValueKind::GroupSizeX => (
            Cov6ImplicitKernargFieldKindV1::GroupSize(0),
            COV6_GROUP_SIZE_X_OFFSET_V1,
            2,
        ),
        HiddenValueKind::GroupSizeY => (
            Cov6ImplicitKernargFieldKindV1::GroupSize(1),
            COV6_GROUP_SIZE_Y_OFFSET_V1,
            2,
        ),
        HiddenValueKind::GroupSizeZ => (
            Cov6ImplicitKernargFieldKindV1::GroupSize(2),
            COV6_GROUP_SIZE_Z_OFFSET_V1,
            2,
        ),
        HiddenValueKind::RemainderX => (
            Cov6ImplicitKernargFieldKindV1::Remainder(0),
            COV6_REMAINDER_X_OFFSET_V1,
            2,
        ),
        HiddenValueKind::RemainderY => (
            Cov6ImplicitKernargFieldKindV1::Remainder(1),
            COV6_REMAINDER_Y_OFFSET_V1,
            2,
        ),
        HiddenValueKind::RemainderZ => (
            Cov6ImplicitKernargFieldKindV1::Remainder(2),
            COV6_REMAINDER_Z_OFFSET_V1,
            2,
        ),
        HiddenValueKind::GlobalOffsetX => (
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(0),
            COV6_GLOBAL_OFFSET_X_OFFSET_V1,
            8,
        ),
        HiddenValueKind::GlobalOffsetY => (
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(1),
            COV6_GLOBAL_OFFSET_Y_OFFSET_V1,
            8,
        ),
        HiddenValueKind::GlobalOffsetZ => (
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(2),
            COV6_GLOBAL_OFFSET_Z_OFFSET_V1,
            8,
        ),
        HiddenValueKind::GridDimensions => (
            Cov6ImplicitKernargFieldKindV1::GridDimensions,
            COV6_GRID_DIMENSIONS_OFFSET_V1,
            2,
        ),
        HiddenValueKind::DynamicLdsSize => (
            Cov6ImplicitKernargFieldKindV1::DynamicLdsSize,
            COV6_DYNAMIC_LDS_SIZE_OFFSET_V1,
            4,
        ),
        HiddenValueKind::None
        | HiddenValueKind::PrintfBuffer
        | HiddenValueKind::HostcallBuffer
        | HiddenValueKind::HeapV1
        | HiddenValueKind::DefaultQueue
        | HiddenValueKind::CompletionAction
        | HiddenValueKind::MultigridSyncArgument
        | HiddenValueKind::PrivateBase
        | HiddenValueKind::SharedBase
        | HiddenValueKind::QueuePointer => return None,
    };
    Some(Cov6ImplicitKernargFieldV1 {
        kind,
        relative_offset,
        byte_len,
    })
}

fn validate_and_derive_cov6_implicit_kernarg(
    packet: usize,
    kernel: &InspectedKernel,
    input: &Gfx942FixedDispatchPacketV1,
    plan: Option<&Cov6ImplicitKernargPlanV1>,
) -> Result<Option<Cov6ImplicitKernargValuesV1>, Gfx942DispatchBindingErrorV1> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    validate_caller_zero_cov6_implicit_suffix(packet, &input.kernarg_bytes, plan)?;
    derive_cov6_implicit_kernarg_values(
        input.geometry,
        input.dynamic_group_segment_bytes,
        kernel.uniform_work_group_size(),
    )
    .map(Some)
    .map_err(|detail| Gfx942DispatchBindingErrorV1::Geometry { packet, detail })
}

fn validate_caller_zero_cov6_implicit_suffix(
    packet: usize,
    kernarg: &[u8],
    plan: &Cov6ImplicitKernargPlanV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let suffix_end = plan
        .byte_offset
        .checked_add(COV6_IMPLICIT_ARGUMENT_BYTES_V1)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "implicit-kernarg suffix overflow",
        })?;
    if kernarg
        .get(plan.byte_offset..suffix_end)
        .is_none_or(|suffix| suffix.iter().any(|byte| *byte != 0))
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "caller must zero the complete COV6 implicit-kernarg suffix",
        });
    }
    Ok(())
}

fn derive_cov6_implicit_kernarg_values(
    geometry: AqlDispatchGeometryV1,
    dynamic_lds_size: u32,
    uniform_workgroup: bool,
) -> Result<Cov6ImplicitKernargValuesV1, &'static str> {
    let dispatch_shape = geometry.cov6_implicit_dispatch_shape();
    if uniform_workgroup && dispatch_shape.remainder() != [0; 3] {
        return Err("uniform workgroup has a partial remainder");
    }
    Ok(Cov6ImplicitKernargValuesV1 {
        dispatch_shape,
        dynamic_lds_size,
    })
}

fn initialize_cov6_implicit_kernarg(
    kernarg: &mut [u8],
    plan: &Cov6ImplicitKernargPlanV1,
    values: Cov6ImplicitKernargValuesV1,
) {
    initialize_cov6_implicit_suffix(
        &mut kernarg[plan.byte_offset..plan.byte_offset + COV6_IMPLICIT_ARGUMENT_BYTES_V1],
        &plan.fields,
        values,
    );
}

fn initialize_cov6_implicit_suffix(
    kernarg: &mut [u8],
    fields: &[Cov6ImplicitKernargFieldV1],
    values: Cov6ImplicitKernargValuesV1,
) {
    let block_count = values.dispatch_shape.block_count();
    let group_size = values.dispatch_shape.group_size();
    let remainder = values.dispatch_shape.remainder();
    debug_assert!(kernarg.iter().all(|byte| *byte == 0));
    for field in fields {
        let offset = field.relative_offset;
        match field.kind {
            Cov6ImplicitKernargFieldKindV1::BlockCount(axis) => {
                put_u32(kernarg, offset, block_count[axis]);
            }
            Cov6ImplicitKernargFieldKindV1::GroupSize(axis) => {
                put_u16(kernarg, offset, group_size[axis]);
            }
            Cov6ImplicitKernargFieldKindV1::Remainder(axis) => {
                put_u16(kernarg, offset, remainder[axis]);
            }
            Cov6ImplicitKernargFieldKindV1::GlobalOffset(axis) => {
                let _ = axis;
                put_u64(kernarg, offset, 0);
            }
            Cov6ImplicitKernargFieldKindV1::GridDimensions => {
                put_u16(kernarg, offset, values.dispatch_shape.grid_dimensions());
            }
            Cov6ImplicitKernargFieldKindV1::DynamicLdsSize => {
                put_u32(kernarg, offset, values.dynamic_lds_size);
            }
        }
    }
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

struct PublicDataBindingStateV1<'a> {
    effects: &'a mut [Option<DeviceDataEffectV1>],
    writable_ranges: &'a mut [Vec<CompletedWritableRangeV1>],
    completed_snapshots: &'a mut [Vec<CompletedSnapshotRangeV1>],
}

fn validate_public_packet_bindings(
    packet: usize,
    kernel: &ValidatedKernelEnvelope<'_>,
    input: &Gfx942FixedDispatchPacketV1,
    data: &[Gfx942FixedDispatchDataLayoutV1],
    state: PublicDataBindingStateV1<'_>,
) -> Result<Box<[DevicePointerPatchV1]>, Gfx942DispatchBindingErrorV1> {
    let arguments = kernel.selected_kernel().explicit_arguments();
    let global_count = arguments
        .iter()
        .filter(|argument| argument.value_kind() == ExplicitValueKind::GlobalBuffer)
        .count();
    if input.buffers.len() != global_count {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "global-buffer binding cardinality",
        });
    }
    let mut seen_arguments = vec![false; arguments.len()];
    let mut patches = Vec::with_capacity(input.buffers.len());
    for binding in &input.buffers {
        let argument = arguments.get(binding.explicit_argument_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "explicit argument index",
            },
        )?;
        if seen_arguments[binding.explicit_argument_index]
            || argument.value_kind() != ExplicitValueKind::GlobalBuffer
            || argument.size() != 8
            || argument.address_space() != Some(ArgumentAddressSpace::Global)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "inspected global-buffer argument",
            });
        }
        seen_arguments[binding.explicit_argument_index] = true;
        let layout =
            data.get(binding.data_index)
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "dispatch data index",
                })?;
        let (patch, effect, completed_snapshot) = validate_inspected_buffer_contract(
            packet,
            &input.kernarg_bytes,
            binding,
            layout.requested_bytes(),
            layout.alignment(),
            &input.buffers,
            InspectedBufferContractV1 {
                pointer_offset: argument.offset(),
                declared_access: argument.access(),
                actual_access: kernel.dispatch_actual_access(binding.explicit_argument_index),
                pointee_alignment: kernel
                    .dispatch_pointee_alignment(binding.explicit_argument_index),
            },
        )?;
        state.effects[binding.data_index] =
            Some(merge_effect(state.effects[binding.data_index], effect));
        if matches!(
            effect,
            DeviceDataEffectV1::WriteOnly | DeviceDataEffectV1::ReadWrite
        ) {
            state.writable_ranges[binding.data_index].push(CompletedWritableRangeV1 {
                offset: binding.data_byte_offset,
                byte_len: binding.byte_len,
            });
        }
        if let Some(snapshot) = completed_snapshot {
            state.completed_snapshots[binding.data_index].push(snapshot);
        }
        if patches.iter().any(|prior: &DevicePointerPatchV1| {
            ranges_overlap_usize(prior.byte_offset, 8, patch.byte_offset, 8)
        }) {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "overlapping pointer fields",
            });
        }
        patches.push(patch);
    }
    if arguments.iter().enumerate().any(|(index, argument)| {
        argument.value_kind() == ExplicitValueKind::GlobalBuffer && !seen_arguments[index]
    }) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "missing inspected global-buffer binding",
        });
    }
    Ok(patches.into_boxed_slice())
}

#[derive(Clone, Copy)]
struct InspectedBufferContractV1 {
    pointer_offset: u64,
    declared_access: Option<ArgumentAccess>,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
}

fn validate_inspected_buffer_contract(
    packet: usize,
    kernarg_bytes: &[u8],
    binding: &Gfx942DispatchBufferBindingV1,
    allocation_bytes: u64,
    allocation_alignment: u64,
    all_bindings: &[Gfx942DispatchBufferBindingV1],
    contract: InspectedBufferContractV1,
) -> Result<
    (
        DevicePointerPatchV1,
        DeviceDataEffectV1,
        Option<CompletedSnapshotRangeV1>,
    ),
    Gfx942DispatchBindingErrorV1,
> {
    let access = contract
        .actual_access
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "missing inspected actual access",
        })?;
    if contract
        .declared_access
        .is_some_and(|declared| declared != access)
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "declared/actual access contradiction",
        });
    }
    let required_alignment =
        contract
            .pointee_alignment
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "missing inspected pointee alignment",
            })?;
    let pointer_offset = usize::try_from(contract.pointer_offset).map_err(|_| {
        Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "pointer field offset conversion",
        }
    })?;
    let pointer_end =
        pointer_offset
            .checked_add(8)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "pointer field overflow",
            })?;
    if !pointer_offset.is_multiple_of(8)
        || pointer_end > kernarg_bytes.len()
        || kernarg_bytes[pointer_offset..pointer_end] != [0; 8]
        || binding.byte_len == 0
        || required_alignment == 0
        || !required_alignment.is_power_of_two()
        || required_alignment > allocation_alignment
        || !binding.data_byte_offset.is_multiple_of(required_alignment)
        || binding
            .data_byte_offset
            .checked_add(binding.byte_len)
            .is_none_or(|end| end > allocation_bytes)
        || all_bindings.iter().any(|other| {
            other != binding
                && other.data_index == binding.data_index
                && ranges_overlap_u64(
                    other.data_byte_offset,
                    other.byte_len,
                    binding.data_byte_offset,
                    binding.byte_len,
                )
        })
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "dispatch buffer range or alias",
        });
    }
    let effect = match access {
        ArgumentAccess::ReadOnly => DeviceDataEffectV1::ReadOnly,
        ArgumentAccess::WriteOnly => DeviceDataEffectV1::WriteOnly,
        ArgumentAccess::ReadWrite => DeviceDataEffectV1::ReadWrite,
    };
    if let Some(snapshot) = binding.completed_snapshot {
        validate_completed_snapshot_binding(
            packet,
            binding,
            snapshot,
            allocation_bytes,
            all_bindings,
            effect,
        )?;
    }
    Ok((
        DevicePointerPatchV1::new(
            pointer_offset,
            binding.data_index,
            binding.data_byte_offset,
            binding.byte_len,
            required_alignment,
        ),
        effect,
        binding.completed_snapshot,
    ))
}

fn validate_completed_snapshot_binding(
    packet: usize,
    binding: &Gfx942DispatchBufferBindingV1,
    snapshot: CompletedSnapshotRangeV1,
    allocation_bytes: u64,
    all_bindings: &[Gfx942DispatchBufferBindingV1],
    effect: DeviceDataEffectV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let interior_end = binding
        .data_byte_offset
        .checked_add(binding.byte_len)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "completed snapshot interior overflow",
        })?;
    let snapshot_end = snapshot.offset.checked_add(snapshot.byte_len).ok_or(
        Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "completed snapshot range overflow",
        },
    )?;
    if !matches!(
        effect,
        DeviceDataEffectV1::WriteOnly | DeviceDataEffectV1::ReadWrite
    ) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "completed snapshot requires inspected write access",
        });
    }
    if snapshot.interior_offset != binding.data_byte_offset
        || snapshot.interior_byte_len != binding.byte_len
        || snapshot.byte_len == 0
        || snapshot.offset >= binding.data_byte_offset
        || interior_end >= snapshot_end
        || snapshot_end > allocation_bytes
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "completed snapshot strict enclosing range",
        });
    }
    if all_bindings.iter().any(|other| {
        other != binding
            && other.data_index == binding.data_index
            && ranges_overlap_u64(
                snapshot.offset,
                snapshot.byte_len,
                other.data_byte_offset,
                other.byte_len,
            )
    }) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "completed snapshot overlaps another buffer binding",
        });
    }
    Ok(())
}

fn merge_effect(
    existing: Option<DeviceDataEffectV1>,
    next: DeviceDataEffectV1,
) -> DeviceDataEffectV1 {
    match existing {
        None => next,
        Some(existing) if existing == next => next,
        Some(_) => DeviceDataEffectV1::ReadWrite,
    }
}

fn validate_initialization_premises(
    effects: &[Option<DeviceDataEffectV1>],
    initialized: &[bool],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if effects.len() != initialized.len() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: effects.len().min(initialized.len()),
            detail: "initialization/effect cardinality",
        });
    }
    for (index, effect) in effects.iter().enumerate() {
        if effect.is_some_and(DeviceDataEffectV1::reads) && !initialized[index] {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "inspected read requires sealed initialized storage",
            });
        }
    }
    Ok(())
}

fn plan_public_retained_data(
    layouts: &[Gfx942FixedDispatchDataLayoutV1],
    initialized: &[bool],
    effects: Vec<Option<DeviceDataEffectV1>>,
    writable_ranges: Vec<Vec<CompletedWritableRangeV1>>,
    completed_snapshots: Vec<Vec<CompletedSnapshotRangeV1>>,
) -> Result<Vec<PublicRetainedDataPlanV1>, Gfx942DispatchBindingErrorV1> {
    let expected = layouts.len();
    let observed = [
        initialized.len(),
        effects.len(),
        writable_ranges.len(),
        completed_snapshots.len(),
    ];
    if observed.iter().any(|&len| len != expected) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: observed
                .into_iter()
                .chain(core::iter::once(expected))
                .min()
                .unwrap_or(0),
            detail: "public retained data cardinality",
        });
    }
    validate_initialization_premises(&effects, initialized)?;
    validate_completed_snapshot_premises(
        layouts,
        initialized,
        &writable_ranges,
        &completed_snapshots,
    )?;
    Ok(layouts
        .iter()
        .copied()
        .zip(initialized.iter().copied())
        .zip(effects)
        .zip(writable_ranges)
        .zip(completed_snapshots)
        .map(
            |((((layout, fully_initialized), effect), writable_ranges), completed_snapshots)| {
                PublicRetainedDataPlanV1 {
                    layout,
                    effect,
                    fully_initialized,
                    writable_ranges: writable_ranges.into_boxed_slice(),
                    completed_snapshots: completed_snapshots.into_boxed_slice(),
                }
            },
        )
        .collect())
}

fn validate_completed_snapshot_premises(
    layouts: &[Gfx942FixedDispatchDataLayoutV1],
    initialized: &[bool],
    writable_ranges: &[Vec<CompletedWritableRangeV1>],
    snapshots: &[Vec<CompletedSnapshotRangeV1>],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if layouts.len() != initialized.len()
        || layouts.len() != writable_ranges.len()
        || layouts.len() != snapshots.len()
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: layouts
                .len()
                .min(initialized.len())
                .min(writable_ranges.len())
                .min(snapshots.len()),
            detail: "completed snapshot premise cardinality",
        });
    }
    for (index, declared) in snapshots.iter().enumerate() {
        if declared.is_empty() {
            continue;
        }
        if layouts[index].kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "completed snapshot requires coherent host-visible storage",
            });
        }
        if !initialized[index] {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "completed snapshot requires full initialization",
            });
        }
        for (position, snapshot) in declared.iter().enumerate() {
            if declared[..position].iter().any(|prior| {
                ranges_overlap_u64(
                    prior.offset,
                    prior.byte_len,
                    snapshot.offset,
                    snapshot.byte_len,
                )
            }) {
                return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    index,
                    detail: "completed snapshot ranges overlap",
                });
            }
            let exact_interior_count = writable_ranges[index]
                .iter()
                .filter(|range| {
                    range.offset == snapshot.interior_offset
                        && range.byte_len == snapshot.interior_byte_len
                })
                .count();
            let overlapping_write_count = writable_ranges[index]
                .iter()
                .filter(|range| {
                    ranges_overlap_u64(
                        range.offset,
                        range.byte_len,
                        snapshot.offset,
                        snapshot.byte_len,
                    )
                })
                .count();
            if exact_interior_count != 1 || overlapping_write_count != 1 {
                return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    index,
                    detail: "completed snapshot writable effect isolation",
                });
            }
        }
    }
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|end| end & !(alignment - 1))
}

fn ranges_overlap_u64(left: u64, left_len: u64, right: u64, right_len: u64) -> bool {
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

fn validate_packet_count<const N: usize>() -> Result<(), Gfx942DispatchBindingErrorV1> {
    dispatch_template_packet_count_body!(dispatch_rust_expr, N)
}

pub(super) fn validate_fixed_batch_ring<const N: usize>(
    ring_bytes: u32,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    let capacity = AqlRingCapacityV1::from_ring_bytes(ring_bytes)
        .map_err(|_| Gfx942DispatchBindingErrorV1::RingCapacity {
            requested: N,
            capacity: 0,
        })?
        .packets();
    if N > capacity as usize {
        return Err(Gfx942DispatchBindingErrorV1::RingCapacity {
            requested: N,
            capacity,
        });
    }
    Ok(())
}

fn validate_data_inputs(
    data: &[DeviceDataAllocationInputV1],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if data.is_empty() || data.len() > MAX_DISPATCH_DATA_LEASES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
            requested: data.len(),
            maximum: MAX_DISPATCH_DATA_LEASES_V1,
        });
    }
    for (index, input) in data.iter().enumerate() {
        let premise = input.premise;
        if premise.role_identity == [0; 32] {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "zero role identity",
            });
        }
        if input.requested_bytes == 0
            || input.alignment == 0
            || !input.alignment.is_power_of_two()
            || input.alignment > 4096
            || premise.valid_bytes == 0
            || premise.valid_bytes > input.requested_bytes
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "valid byte extent",
            });
        }
        if premise.effect.reads() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "read requires authenticated initialized-content authority",
            });
        }
        if data[..index]
            .iter()
            .any(|prior| prior.premise.role_identity == premise.role_identity)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "role identity alias",
            });
        }
    }
    Ok(())
}

fn validate_kernargs<const N: usize>(
    kernargs: &[TypedKernargImageV1; N],
    expected_bytes: usize,
    data: &[DeviceDataAllocationInputV1],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let mut referenced = vec![false; data.len()];
    for (packet, kernarg) in kernargs.iter().enumerate() {
        if kernarg.layout_identity == [0; 32] || kernarg.bytes.len() != expected_bytes {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "typed layout identity or size",
            });
        }
        for (patch_index, patch) in kernarg.device_pointers.iter().enumerate() {
            let end = patch.byte_offset.checked_add(8).ok_or(
                Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "pointer field overflow",
                },
            )?;
            if !patch.byte_offset.is_multiple_of(8)
                || end > kernarg.bytes.len()
                || kernarg.bytes[patch.byte_offset..end] != [0; 8]
                || patch.required_bytes == 0
                || patch.required_alignment == 0
                || !patch.required_alignment.is_power_of_two()
                || patch.data_index >= data.len()
                || !patch
                    .data_byte_offset
                    .is_multiple_of(patch.required_alignment)
                || patch.required_alignment > data[patch.data_index].alignment
                || patch
                    .data_byte_offset
                    .checked_add(patch.required_bytes)
                    .is_none_or(|end| end > data[patch.data_index].premise.valid_bytes)
                || kernarg.device_pointers[..patch_index]
                    .iter()
                    .any(|prior| ranges_overlap_usize(prior.byte_offset, 8, patch.byte_offset, 8))
            {
                return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "device pointer patch",
                });
            }
            referenced[patch.data_index] = true;
        }
    }
    if let Some(index) = referenced.iter().position(|referenced| !referenced) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index,
            detail: "lease not referenced by kernarg",
        });
    }
    Ok(())
}

fn validate_geometry<const N: usize>(
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
    uniform_workgroup: bool,
    geometry: &[DispatchGeometryV1; N],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    for (packet, dispatch) in geometry.iter().enumerate() {
        let observed_workgroup = dispatch.geometry.workgroup();
        let workgroup = observed_workgroup.map(u32::from);
        let grid = dispatch.geometry.grid();
        let flat = u64::from(workgroup[0])
            .checked_mul(u64::from(workgroup[1]))
            .and_then(|xy| xy.checked_mul(u64::from(workgroup[2])))
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "workgroup product",
            })?;
        if flat > u64::from(resources.max_flat_workgroup_size())
            || resources
                .required_workgroup_size()
                .is_some_and(|required| required != workgroup)
        {
            return Err(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "workgroup resource contract",
            });
        }
        if uniform_workgroup
            && grid
                .iter()
                .zip(workgroup)
                .any(|(grid, workgroup)| !grid.is_multiple_of(workgroup))
        {
            return Err(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "uniform workgroup has a partial remainder",
            });
        }
        for dimension in 0..3 {
            if resources.max_workgroups()[dimension]
                .is_some_and(|maximum| grid[dimension].div_ceil(workgroup[dimension]) > maximum)
            {
                return Err(Gfx942DispatchBindingErrorV1::Geometry {
                    packet,
                    detail: "workgroup count",
                });
            }
        }
        u64::from(dispatch.dynamic_group_segment_bytes)
            .checked_add(resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "group segment size",
            })?;
    }
    Ok(())
}

fn ranges_overlap_usize(left: usize, left_len: usize, right: usize, right_len: usize) -> bool {
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

#[cfg(test)]
pub(super) use tests::actual_persistent_control_test_program;

#[cfg(test)]
impl DispatchResourceOwnerV1 {
    pub(super) fn source_failure_snapshot_v1(&self) -> CpuDispatchOwnerSnapshotV1 {
        CpuDispatchOwnerSnapshotV1 {
            next_generation: self.generation.next_generation,
            recipe_occurrence: self.generation.recipe_occurrence,
            recipe_queue: self.generation.recipe_queue,
            capacity_profile: self.generation.capacity_profile,
            slot_storage: self.generation.slots.as_ptr() as usize,
            slots: self.generation.slots.iter().copied().collect(),
            recycled_generation: self.generation.recycled_generation,
            predecessor_detached_generation: self.generation.predecessor_detached_generation,
            poisoned: self.generation.poisoned,
        }
    }

    // Exercises the real epoch table with fixture occurrences, not GPU publication.
    pub(super) fn primary_fixture_exercise_capacity_v1(
        &mut self,
        capacity: &Gfx942FixedDispatchCapacityV1,
    ) {
        assert!(capacity.matches(
            self.generation.capacity_profile,
            self.generation.slots.account().as_ref()
        ));
        let mut identities = Vec::new();
        for slot in 0..capacity.profile().slots() {
            let generation = self.generation.next_generation;
            let identity = self
                .generation
                .reserve(
                    test_dispatch_queue_v1(),
                    test_completion_roster_v1(generation),
                )
                .unwrap();
            assert_eq!(usize::from(identity.slot_index), slot);
            identities.push(identity);
        }
        assert!(
            matches!(self.generation.preflight_reservation(test_dispatch_queue_v1()), Err(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity { maximum }) if maximum == capacity.profile().slots())
        );
        for identity in identities {
            self.generation.cancel_epoch(identity).unwrap();
        }
    }

    pub(super) fn primary_fixture_recipe_occurrence_v1(&self) -> u64 {
        self.generation.recipe_occurrence
    }

    pub(super) fn primary_fixture_next_generation_v1(&self) -> u64 {
        self.generation.next_generation
    }

    pub(super) fn primary_fixture_identities_v1(
        &self,
    ) -> Vec<crate::shared_memory::SharedGttAllocationIdentityV1> {
        use crate::shared_memory::PreparationMemoryFixtureV1 as Memory;
        let mut identities: Vec<_> = self.code.iter().map(Memory::code_identity).collect();
        identities.push(Memory::kernarg_identity(&self.kernarg));
        identities.extend(
            self.data
                .iter()
                .filter_map(|data| match Memory::data_storage(data) {
                    Gfx942SdmaBufferStorageIdentityV1::Host(identity) => Some(identity),
                    Gfx942SdmaBufferStorageIdentityV1::Device(_) => None,
                }),
        );
        identities
    }
}

#[cfg(test)]
#[path = "queue_dispatch_binding/tests.rs"]
mod tests;

#[path = "queue_dispatch_binding/data_api.rs"]
mod data_api;
