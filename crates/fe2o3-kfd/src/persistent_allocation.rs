//! Addressless lifecycle core for one persistent native device allocation.
//!
//! This module deliberately stops before queue integration. Its typed tokens
//! record a checked host-side custody protocol; they do not publish AQL or SDMA
//! packets and are not evidence that firmware observed a dependency.

use fe2o3_runtime_model::QueueKeyV1;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::queue::dispatch_binding::{DispatchDataStorageRefV1, Gfx942FixedDispatchDataV1};
use crate::sdma::{
    Gfx942SdmaBufferStorageIdentityV1, Gfx942SdmaBufferV1, Gfx942SdmaDeviceBackingV1,
};

use crate::shared_memory::{
    Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    Gfx942XgmiMappedDeviceMemoryV1,
};

/// Maximum number of live or retained-settled uses of one persistent owner.
pub const GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1: usize = 64;

/// The concrete native mapping retained by a persistent owner.
///
/// Both variants remain addressless. The peer form is admitted only for an
/// existing complete, canonical two-device XGMI mapping.
#[must_use = "native device-memory authority must be explicitly released"]
pub enum Gfx942PersistentNativeAllocationV1 {
    Local(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    ExactTwoDevicePeer(Gfx942XgmiMappedDeviceMemoryV1),
}

enum PersistentBackingV1 {
    Local(Gfx942SdmaDeviceBackingV1),
    ExactTwoDevicePeer(Gfx942XgmiMappedDeviceMemoryV1),
}

/// Minted only after exact never-published compute restoration preflight.
pub(crate) struct PersistentComputeCancellationPermitV1 {
    _private: (),
}

/// Minted only after exact completed-use and original typed-data preflight.
pub(crate) struct PersistentComputeCompletionPermitV1 {
    _private: (),
}

impl fmt::Debug for Gfx942PersistentNativeAllocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Gfx942PersistentNativeAllocationV1")
            .field(&match self {
                Self::Local(_) => Gfx942PersistentMappingFormV1::Local,
                Self::ExactTwoDevicePeer(mapping) => {
                    let gpu_ids = <[u32; 2]>::try_from(mapping.gpu_ids())
                        .expect("peer owner was admitted with exactly two devices");
                    Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { gpu_ids }
                }
            })
            .finish()
    }
}

/// Public, non-authoritative description of the retained mapping form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentMappingFormV1 {
    Local,
    ExactTwoDevicePeer { gpu_ids: [u32; 2] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentUseOwnerV1 {
    Compute,
    LocalSdma,
    PeerMapped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentAccessV1 {
    Read,
    Write,
    ReadWrite,
}

impl Gfx942PersistentAccessV1 {
    const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

/// Closed roster of operations understood by the initial ledger.
///
/// Access is derived from the operation rather than accepted independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentOperationV1 {
    ComputeRead,
    ComputeWrite,
    ComputeReadWrite,
    LocalSdmaSource,
    LocalSdmaDestination,
    /// Classification only; it grants no directional XGMI route or engine.
    PeerMappedSource,
    /// Classification only; it grants no directional XGMI route or engine.
    PeerMappedDestination,
}

impl Gfx942PersistentOperationV1 {
    pub const fn owner(self) -> Gfx942PersistentUseOwnerV1 {
        match self {
            Self::ComputeRead | Self::ComputeWrite | Self::ComputeReadWrite => {
                Gfx942PersistentUseOwnerV1::Compute
            }
            Self::LocalSdmaSource | Self::LocalSdmaDestination => {
                Gfx942PersistentUseOwnerV1::LocalSdma
            }
            Self::PeerMappedSource | Self::PeerMappedDestination => {
                Gfx942PersistentUseOwnerV1::PeerMapped
            }
        }
    }

    pub const fn access(self) -> Gfx942PersistentAccessV1 {
        match self {
            Self::ComputeRead | Self::LocalSdmaSource | Self::PeerMappedSource => {
                Gfx942PersistentAccessV1::Read
            }
            Self::ComputeWrite | Self::LocalSdmaDestination | Self::PeerMappedDestination => {
                Gfx942PersistentAccessV1::Write
            }
            Self::ComputeReadWrite => Gfx942PersistentAccessV1::ReadWrite,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PersistentRangeV1 {
    offset: u64,
    byte_len: u64,
}

impl Gfx942PersistentRangeV1 {
    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn end(self) -> Option<u64> {
        self.offset.checked_add(self.byte_len)
    }

    fn overlaps(self, other: Self) -> bool {
        let Some(self_end) = self.end() else {
            return true;
        };
        let Some(other_end) = other.end() else {
            return true;
        };
        self.offset < other_end && other.offset < self_end
    }
}

/// Data-only request. Construction checks nonzero and nonoverflowing extent;
/// reservation additionally checks the allocation bound and mapping form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PersistentUseRequestV1 {
    operation: Gfx942PersistentOperationV1,
    range: Gfx942PersistentRangeV1,
}

impl Gfx942PersistentUseRequestV1 {
    pub fn new(
        operation: Gfx942PersistentOperationV1,
        offset: u64,
        byte_len: u64,
    ) -> Result<Self, Gfx942PersistentUseErrorV1> {
        let range = Gfx942PersistentRangeV1 { offset, byte_len };
        if byte_len == 0 || range.end().is_none() {
            return Err(Gfx942PersistentUseErrorV1::InvalidRange);
        }
        Ok(Self { operation, range })
    }

    pub const fn operation(self) -> Gfx942PersistentOperationV1 {
        self.operation
    }

    pub const fn owner(self) -> Gfx942PersistentUseOwnerV1 {
        self.operation.owner()
    }

    pub const fn access(self) -> Gfx942PersistentAccessV1 {
        self.operation.access()
    }

    pub const fn range(self) -> Gfx942PersistentRangeV1 {
        self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentUseErrorV1 {
    InvalidRange,
    OperationRequiresPeerMapping,
    Capacity,
    GenerationExhausted,
    WrongOwnerOrGeneration,
    WrongState,
    OverlappingWriterActive,
    DependencyRequired,
    DependencyNotRequired,
    StaleOrSubstitutedDependency,
    EarlierUseNotSettled,
    Quarantined,
    OutstandingUses,
}

impl fmt::Display for Gfx942PersistentUseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRange => "the use range is empty, overflowing, or outside the allocation",
            Self::OperationRequiresPeerMapping => {
                "the operation requires an exact complete two-device peer mapping"
            }
            Self::Capacity => "the persistent-allocation use ledger is full",
            Self::GenerationExhausted => "the persistent-allocation generation is exhausted",
            Self::WrongOwnerOrGeneration => "the use belongs to another allocation or generation",
            Self::WrongState => "the use is in the wrong lifecycle state",
            Self::OverlappingWriterActive => "an overlapping active use includes a writer",
            Self::DependencyRequired => "the current successful dependency frontier is required",
            Self::DependencyNotRequired => "no successful dependency frontier is required",
            Self::StaleOrSubstitutedDependency => {
                "the dependency frontier is stale or belongs to another allocation"
            }
            Self::EarlierUseNotSettled => "an earlier reserved use is not settled",
            Self::Quarantined => "the persistent allocation is quarantined",
            Self::OutstandingUses => "the persistent allocation still has outstanding uses",
        })
    }
}

impl std::error::Error for Gfx942PersistentUseErrorV1 {}

mod state {
    pub trait Sealed {}
}

pub trait Gfx942PersistentUseStateV1: state::Sealed + 'static {}
pub enum Gfx942PersistentReservedV1 {}
pub enum Gfx942PersistentPreparedV1 {}
pub enum Gfx942PersistentPublishedV1 {}
pub enum Gfx942PersistentCompletedV1 {}

impl state::Sealed for Gfx942PersistentReservedV1 {}
impl state::Sealed for Gfx942PersistentPreparedV1 {}
impl state::Sealed for Gfx942PersistentPublishedV1 {}
impl state::Sealed for Gfx942PersistentCompletedV1 {}
impl Gfx942PersistentUseStateV1 for Gfx942PersistentReservedV1 {}
impl Gfx942PersistentUseStateV1 for Gfx942PersistentPreparedV1 {}
impl Gfx942PersistentUseStateV1 for Gfx942PersistentPublishedV1 {}
impl Gfx942PersistentUseStateV1 for Gfx942PersistentCompletedV1 {}

/// Move-only, addressless custody for one exact ledger use.
///
/// The token binds the private native allocation/device/VM generation and the
/// exact mapped-state authority retained by its owner. It is deliberately
/// thread-affine and cannot expose those native identities.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942PersistentReservedV1, Gfx942PersistentUseLeaseV1};
/// fn cannot_clone(value: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942PersistentReservedV1, Gfx942PersistentUseLeaseV1};
/// fn require_send<T: Send>(_: T) {}
/// fn cannot_send(value: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>) {
///     require_send(value);
/// }
/// ```
#[must_use = "use custody must be transitioned, cancelled, or quarantined"]
pub struct Gfx942PersistentUseLeaseV1<S: Gfx942PersistentUseStateV1> {
    incarnation: Rc<()>,
    binding: Gfx942DeviceMemoryIdentityV1,
    slot: u8,
    generation: u64,
    sequence: u64,
    request: Gfx942PersistentUseRequestV1,
    marker: PhantomData<S>,
    thread_affinity: PhantomData<Rc<()>>,
}

impl<S: Gfx942PersistentUseStateV1> Gfx942PersistentUseLeaseV1<S> {
    #[cfg(test)]
    pub(crate) fn cancellation_identity_for_test(&self) -> PersistentUseIdentityForTestV1 {
        PersistentUseIdentityForTestV1 {
            incarnation: Rc::as_ptr(&self.incarnation) as usize,
            binding: self.binding,
            slot: self.slot,
            generation: self.generation,
            sequence: self.sequence,
            request: self.request,
        }
    }

    pub const fn request(&self) -> Gfx942PersistentUseRequestV1 {
        self.request
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    fn retag<T: Gfx942PersistentUseStateV1>(self) -> Gfx942PersistentUseLeaseV1<T> {
        Gfx942PersistentUseLeaseV1 {
            incarnation: self.incarnation,
            binding: self.binding,
            slot: self.slot,
            generation: self.generation,
            sequence: self.sequence,
            request: self.request,
            marker: PhantomData,
            thread_affinity: PhantomData,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PersistentUseIdentityForTestV1 {
    incarnation: usize,
    binding: Gfx942DeviceMemoryIdentityV1,
    slot: u8,
    generation: u64,
    sequence: u64,
    request: Gfx942PersistentUseRequestV1,
}

#[allow(dead_code)]
pub(crate) struct Gfx942PersistentLocalSdmaPairTransitionFailureV1<S: Gfx942PersistentUseStateV1> {
    pub(crate) error: Gfx942PersistentUseErrorV1,
    pub(crate) source: Gfx942PersistentUseLeaseV1<S>,
    pub(crate) destination: Gfx942PersistentUseLeaseV1<S>,
}

impl<S: Gfx942PersistentUseStateV1> fmt::Debug for Gfx942PersistentUseLeaseV1<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentUseLeaseV1")
            .field("sequence", &self.sequence)
            .field("request", &self.request)
            .finish_non_exhaustive()
    }
}

/// Latest host-confirmed successful frontier for this exact owner.
///
/// A frontier may be borrowed by several compatible reservations. It is
/// invalidated when a later use settles, and never represents a device packet,
/// signal, barrier, or firmware observation.
#[must_use = "retain the dependency frontier while it may order later uses"]
pub struct Gfx942PersistentDependencyFrontierV1 {
    incarnation: Rc<()>,
    binding: Gfx942DeviceMemoryIdentityV1,
    generation: u64,
    through_sequence: u64,
    thread_affinity: PhantomData<Rc<()>>,
}

impl Gfx942PersistentDependencyFrontierV1 {
    #[cfg(test)]
    pub(crate) fn identity_for_test_v1(&self) -> (usize, Gfx942DeviceMemoryIdentityV1, u64, u64) {
        (
            Rc::as_ptr(&self.incarnation) as usize,
            self.binding,
            self.generation,
            self.through_sequence,
        )
    }

    pub const fn through_sequence(&self) -> u64 {
        self.through_sequence
    }
}

impl fmt::Debug for Gfx942PersistentDependencyFrontierV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentDependencyFrontierV1")
            .field("through_sequence", &self.through_sequence)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Gfx942PersistentReservationFailureV1 {
    error: Gfx942PersistentUseErrorV1,
    request: Gfx942PersistentUseRequestV1,
}

impl Gfx942PersistentReservationFailureV1 {
    pub const fn error(&self) -> Gfx942PersistentUseErrorV1 {
        self.error
    }

    pub const fn into_parts(self) -> (Gfx942PersistentUseErrorV1, Gfx942PersistentUseRequestV1) {
        (self.error, self.request)
    }
}

pub struct Gfx942PersistentTransitionFailureV1<S: Gfx942PersistentUseStateV1> {
    error: Gfx942PersistentUseErrorV1,
    lease: Gfx942PersistentUseLeaseV1<S>,
}

impl<S: Gfx942PersistentUseStateV1> fmt::Debug for Gfx942PersistentTransitionFailureV1<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentTransitionFailureV1")
            .field("error", &self.error)
            .field("lease", &self.lease)
            .finish()
    }
}

impl<S: Gfx942PersistentUseStateV1> Gfx942PersistentTransitionFailureV1<S> {
    pub const fn error(&self) -> Gfx942PersistentUseErrorV1 {
        self.error
    }

    pub fn into_parts(self) -> (Gfx942PersistentUseErrorV1, Gfx942PersistentUseLeaseV1<S>) {
        (self.error, self.lease)
    }
}

/// Timeout custody retains the published use unchanged for later observation.
#[must_use = "a timeout is not completion; published custody must be retained"]
pub struct Gfx942PersistentTimeoutV1 {
    published: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
}

impl Gfx942PersistentTimeoutV1 {
    pub const fn request(&self) -> Gfx942PersistentUseRequestV1 {
        self.published.request
    }

    pub fn into_published(self) -> Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1> {
        self.published
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PersistentQuarantineReasonV1 {
    CallerReportedPublicationIndeterminate,
    CallerReportedCurrentnessLoss,
    CallerReportedCompletionIndeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LedgerStateV1 {
    Reserved,
    Prepared,
    Published,
    Completed,
    Settled,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LedgerRecordV1 {
    generation: u64,
    sequence: u64,
    request: Gfx942PersistentUseRequestV1,
    state: LedgerStateV1,
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PersistentOwnerSnapshotForTestV1 {
    incarnation: usize,
    local_native: Option<Gfx942DeviceMemoryIdentityV1>,
    binding: Gfx942DeviceMemoryIdentityV1,
    mapping: Gfx942PersistentMappingFormV1,
    byte_len: u64,
    initialization: Option<(Gfx942DeviceMemoryIdentityV1, QueueKeyV1, u64, u64, u64, u64)>,
    detached_compute: Option<(u8, u64)>,
    ledger_address: usize,
    ledger: [Option<LedgerRecordV1>; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1],
    next_generation: u64,
    next_sequence: u64,
    frontier_generation: u64,
    frontier_sequence: Option<u64>,
    quarantine: Option<Gfx942PersistentQuarantineReasonV1>,
}

#[cfg(test)]
impl PersistentOwnerSnapshotForTestV1 {
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        self.incarnation == other.incarnation
            && self.binding == other.binding
            && self.mapping == other.mapping
            && self.byte_len == other.byte_len
            && self.ledger_address == other.ledger_address
    }
    pub(crate) fn reservation_history(&self) -> (u64, u64) {
        (self.next_generation, self.next_sequence)
    }
    pub(crate) fn local_native(&self) -> Option<Gfx942DeviceMemoryIdentityV1> {
        self.local_native
    }
}

/// Persistent owner for exactly one native mapped device-memory authority.
///
/// The owner is non-cloneable and thread-affine. Dropping it performs no KFD
/// operation. Normal extraction of the native authority is possible only when
/// no use is active and the owner has not been quarantined.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942PersistentDeviceAllocationV1;
/// fn cannot_clone(value: Gfx942PersistentDeviceAllocationV1) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942PersistentDeviceAllocationV1;
/// fn require_send<T: Send>(_: T) {}
/// fn cannot_send(value: Gfx942PersistentDeviceAllocationV1) {
///     require_send(value);
/// }
/// ```
#[must_use = "persistent native authority must be explicitly released or retained"]
pub struct Gfx942PersistentDeviceAllocationV1 {
    incarnation: Rc<()>,
    binding: Gfx942DeviceMemoryIdentityV1,
    mapping: Gfx942PersistentMappingFormV1,
    byte_len: u64,
    // One allocation keeps both the bounded ledger and backing out of inline custody.
    state: Box<PersistentOwnerStateV1>,
    next_generation: u64,
    next_sequence: u64,
    frontier_generation: u64,
    frontier_sequence: Option<u64>,
    quarantine: Option<Gfx942PersistentQuarantineReasonV1>,
    thread_affinity: PhantomData<Rc<()>>,
}

struct PersistentOwnerStateV1 {
    native: Option<PersistentBackingV1>,
    detached_compute: Option<(u8, u64)>,
    ledger: [Option<LedgerRecordV1>; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1],
}

impl fmt::Debug for Gfx942PersistentDeviceAllocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942PersistentDeviceAllocationV1")
            .field("mapping", &self.mapping)
            .field("byte_len", &self.byte_len)
            .field("live_use_count", &self.live_use_count())
            .field(
                "retained_settled_use_count",
                &self.retained_settled_use_count(),
            )
            .field("quarantine", &self.quarantine)
            .finish_non_exhaustive()
    }
}

impl Gfx942PersistentDeviceAllocationV1 {}

fn validate_local_sdma_pair<S: Gfx942PersistentUseStateV1>(
    source_owner: &Gfx942PersistentDeviceAllocationV1,
    source: &Gfx942PersistentUseLeaseV1<S>,
    destination_owner: &Gfx942PersistentDeviceAllocationV1,
    destination: &Gfx942PersistentUseLeaseV1<S>,
    expected: LedgerStateV1,
) -> Result<(), Gfx942PersistentUseErrorV1> {
    if Rc::ptr_eq(&source_owner.incarnation, &destination_owner.incarnation)
        || source_owner.binding == destination_owner.binding
        || source.request.operation() != Gfx942PersistentOperationV1::LocalSdmaSource
        || destination.request.operation() != Gfx942PersistentOperationV1::LocalSdmaDestination
        || source.request.range().byte_len() != destination.request.range().byte_len()
    {
        return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
    }
    source_owner.validate_lease(source, expected)?;
    destination_owner.validate_lease(destination, expected)
}

fn detach_local_backing_pair_for_sdma_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
) -> Result<(Gfx942SdmaDeviceBackingV1, Gfx942SdmaDeviceBackingV1), Gfx942PersistentUseErrorV1> {
    if source_owner.quarantine.is_some() || destination_owner.quarantine.is_some() {
        return Err(Gfx942PersistentUseErrorV1::Quarantined);
    }
    if Rc::ptr_eq(&source_owner.incarnation, &destination_owner.incarnation)
        || source_owner.binding == destination_owner.binding
        || !source_owner.local_native_is_attached_for_sdma()
        || !destination_owner.local_native_is_attached_for_sdma()
    {
        return Err(Gfx942PersistentUseErrorV1::WrongState);
    }
    let Some(PersistentBackingV1::Local(source)) = source_owner.state.native.take() else {
        unreachable!("prevalidated local source native custody")
    };
    let Some(PersistentBackingV1::Local(destination)) = destination_owner.state.native.take()
    else {
        unreachable!("prevalidated local destination native custody")
    };
    Ok((source, destination))
}

#[cfg(test)]
pub(crate) fn detach_local_native_pair_for_sdma_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
) -> Result<
    (
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ),
    Gfx942PersistentUseErrorV1,
> {
    let (source, destination) =
        detach_local_backing_pair_for_sdma_v1(source_owner, destination_owner)?;
    Ok((source.into_native(), destination.into_native()))
}

pub(crate) fn detach_sdma_buffer_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source_scope: (QueueKeyV1, u64, u64),
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination_scope: (QueueKeyV1, u64, u64),
) -> Result<(Gfx942SdmaBufferV1, Gfx942SdmaBufferV1), Gfx942PersistentUseErrorV1> {
    if !source_owner.can_detach_sdma_buffer(source_scope.0, source_scope.1, source_scope.2)
        || !destination_owner.can_detach_sdma_buffer(
            destination_scope.0,
            destination_scope.1,
            destination_scope.2,
        )
    {
        return Err(Gfx942PersistentUseErrorV1::WrongState);
    }
    let (source, destination) =
        detach_local_backing_pair_for_sdma_v1(source_owner, destination_owner)?;
    Ok((
        source.into_buffer(source_scope.0, source_scope.1, source_scope.2),
        destination.into_buffer(
            destination_scope.0,
            destination_scope.1,
            destination_scope.2,
        ),
    ))
}

#[allow(clippy::result_large_err)]
pub(crate) fn restore_sdma_buffer_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942SdmaBufferV1,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942SdmaBufferV1,
) -> Result<
    (),
    (
        Gfx942PersistentUseErrorV1,
        Gfx942SdmaBufferV1,
        Gfx942SdmaBufferV1,
    ),
> {
    if source_owner.quarantine.is_some() || destination_owner.quarantine.is_some() {
        return Err((Gfx942PersistentUseErrorV1::Quarantined, source, destination));
    }
    if Rc::ptr_eq(&source_owner.incarnation, &destination_owner.incarnation)
        || source_owner.binding == destination_owner.binding
        || !source_owner.can_restore_sdma_buffer(&source)
        || !destination_owner.can_restore_sdma_buffer(&destination)
        || source.storage_identity() == destination.storage_identity()
    {
        return Err((
            Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration,
            source,
            destination,
        ));
    }
    source_owner
        .restore_sdma_buffer(source)
        .expect("prevalidated source backing");
    destination_owner
        .restore_sdma_buffer(destination)
        .expect("prevalidated destination backing");
    Ok(())
}

#[allow(clippy::result_large_err)]
fn transition_local_sdma_pair<S: Gfx942PersistentUseStateV1, T: Gfx942PersistentUseStateV1>(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<S>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<S>,
    expected: LedgerStateV1,
    next: LedgerStateV1,
) -> Result<
    (Gfx942PersistentUseLeaseV1<T>, Gfx942PersistentUseLeaseV1<T>),
    Gfx942PersistentLocalSdmaPairTransitionFailureV1<S>,
> {
    if let Err(error) = validate_local_sdma_pair(
        source_owner,
        &source,
        destination_owner,
        &destination,
        expected,
    ) {
        return Err(Gfx942PersistentLocalSdmaPairTransitionFailureV1 {
            error,
            source,
            destination,
        });
    }
    source_owner.state.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = next;
    destination_owner.state.ledger[usize::from(destination.slot)]
        .as_mut()
        .expect("validated destination ledger slot")
        .state = next;
    Ok((source.retag(), destination.retag()))
}

#[allow(clippy::result_large_err)]
pub(crate) fn publish_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
) -> Result<
    (
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    ),
    Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentPreparedV1>,
> {
    transition_local_sdma_pair(
        source_owner,
        source,
        destination_owner,
        destination,
        LedgerStateV1::Prepared,
        LedgerStateV1::Published,
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn complete_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
) -> Result<
    (
        Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    ),
    Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentPublishedV1>,
> {
    transition_local_sdma_pair(
        source_owner,
        source,
        destination_owner,
        destination,
        LedgerStateV1::Published,
        LedgerStateV1::Completed,
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn cancel_prepared_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
) -> Result<(), Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentPreparedV1>> {
    if let Err(error) = validate_local_sdma_pair(
        source_owner,
        &source,
        destination_owner,
        &destination,
        LedgerStateV1::Prepared,
    ) {
        return Err(Gfx942PersistentLocalSdmaPairTransitionFailureV1 {
            error,
            source,
            destination,
        });
    }
    source_owner.state.ledger[usize::from(source.slot)] = None;
    destination_owner.state.ledger[usize::from(destination.slot)] = None;
    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn quarantine_prepared_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Result<(), Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentPreparedV1>> {
    if let Err(error) = validate_local_sdma_pair(
        source_owner,
        &source,
        destination_owner,
        &destination,
        LedgerStateV1::Prepared,
    ) {
        return Err(Gfx942PersistentLocalSdmaPairTransitionFailureV1 {
            error,
            source,
            destination,
        });
    }
    source_owner.state.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Quarantined;
    destination_owner.state.ledger[usize::from(destination.slot)]
        .as_mut()
        .expect("validated destination ledger slot")
        .state = LedgerStateV1::Quarantined;
    source_owner.quarantine = Some(reason);
    destination_owner.quarantine = Some(reason);
    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn quarantine_published_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    reason: Gfx942PersistentQuarantineReasonV1,
) -> Result<(), Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentPublishedV1>> {
    if let Err(error) = validate_local_sdma_pair(
        source_owner,
        &source,
        destination_owner,
        &destination,
        LedgerStateV1::Published,
    ) {
        return Err(Gfx942PersistentLocalSdmaPairTransitionFailureV1 {
            error,
            source,
            destination,
        });
    }
    source_owner.state.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Quarantined;
    destination_owner.state.ledger[usize::from(destination.slot)]
        .as_mut()
        .expect("validated destination ledger slot")
        .state = LedgerStateV1::Quarantined;
    source_owner.quarantine = Some(reason);
    destination_owner.quarantine = Some(reason);
    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn settle_completed_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
) -> Result<
    (
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentDependencyFrontierV1,
    ),
    Gfx942PersistentLocalSdmaPairTransitionFailureV1<Gfx942PersistentCompletedV1>,
> {
    let validation = validate_local_sdma_pair(
        source_owner,
        &source,
        destination_owner,
        &destination,
        LedgerStateV1::Completed,
    )
    .and_then(|()| {
        if source_owner.state.ledger.iter().flatten().any(|record| {
            record.sequence < source.sequence && record.state != LedgerStateV1::Settled
        }) || destination_owner
            .state
            .ledger
            .iter()
            .flatten()
            .any(|record| {
                record.sequence < destination.sequence && record.state != LedgerStateV1::Settled
            })
        {
            return Err(Gfx942PersistentUseErrorV1::EarlierUseNotSettled);
        }
        if source_owner.frontier_generation.checked_add(1).is_none()
            || destination_owner
                .frontier_generation
                .checked_add(1)
                .is_none()
        {
            return Err(Gfx942PersistentUseErrorV1::GenerationExhausted);
        }
        Ok(())
    });
    if let Err(error) = validation {
        return Err(Gfx942PersistentLocalSdmaPairTransitionFailureV1 {
            error,
            source,
            destination,
        });
    }
    let source_generation = source_owner.frontier_generation + 1;
    let destination_generation = destination_owner.frontier_generation + 1;
    source_owner.state.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Settled;
    destination_owner.state.ledger[usize::from(destination.slot)]
        .as_mut()
        .expect("validated destination ledger slot")
        .state = LedgerStateV1::Settled;
    source_owner.frontier_generation = source_generation;
    destination_owner.frontier_generation = destination_generation;
    source_owner.frontier_sequence = Some(source.sequence);
    destination_owner.frontier_sequence = Some(destination.sequence);
    Ok((
        Gfx942PersistentDependencyFrontierV1 {
            incarnation: Rc::clone(&source_owner.incarnation),
            binding: source_owner.binding,
            generation: source_generation,
            through_sequence: source.sequence,
            thread_affinity: PhantomData,
        },
        Gfx942PersistentDependencyFrontierV1 {
            incarnation: Rc::clone(&destination_owner.incarnation),
            binding: destination_owner.binding,
            generation: destination_generation,
            through_sequence: destination.sequence,
            thread_affinity: PhantomData,
        },
    ))
}

#[allow(clippy::result_large_err)]
pub(crate) fn retire_settled_local_sdma_pair_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942PersistentDependencyFrontierV1,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942PersistentDependencyFrontierV1,
) -> Result<
    (),
    (
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentDependencyFrontierV1,
    ),
> {
    let current = |owner: &Gfx942PersistentDeviceAllocationV1,
                   frontier: &Gfx942PersistentDependencyFrontierV1| {
        Rc::ptr_eq(&frontier.incarnation, &owner.incarnation)
            && frontier.binding == owner.binding
            && frontier.generation == owner.frontier_generation
            && Some(frontier.through_sequence) == owner.frontier_sequence
            && owner
                .state
                .ledger
                .iter()
                .flatten()
                .all(|record| record.state == LedgerStateV1::Settled)
            && owner.quarantine.is_none()
    };
    if !current(source_owner, &source) || !current(destination_owner, &destination) {
        return Err((source, destination));
    }
    for slot in source_owner.state.ledger.iter_mut() {
        if slot
            .as_ref()
            .is_some_and(|record| record.state == LedgerStateV1::Settled)
        {
            *slot = None;
        }
    }
    for slot in destination_owner.state.ledger.iter_mut() {
        if slot
            .as_ref()
            .is_some_and(|record| record.state == LedgerStateV1::Settled)
        {
            *slot = None;
        }
    }
    source_owner.frontier_sequence = None;
    destination_owner.frontier_sequence = None;
    Ok(())
}

#[cfg(test)]
#[path = "persistent_allocation/tests.rs"]
mod tests;

#[path = "persistent_allocation/owner.rs"]
mod owner;
