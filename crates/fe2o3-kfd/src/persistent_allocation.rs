//! Addressless lifecycle core for one persistent native device allocation.
//!
//! This module deliberately stops before queue integration. Its typed tokens
//! record a checked host-side custody protocol; they do not publish AQL or SDMA
//! packets and are not evidence that firmware observed a dependency.

use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

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
    native: Option<Gfx942PersistentNativeAllocationV1>,
    binding: Gfx942DeviceMemoryIdentityV1,
    mapping: Gfx942PersistentMappingFormV1,
    byte_len: u64,
    ledger: [Option<LedgerRecordV1>; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1],
    next_generation: u64,
    next_sequence: u64,
    frontier_generation: u64,
    frontier_sequence: Option<u64>,
    quarantine: Option<Gfx942PersistentQuarantineReasonV1>,
    thread_affinity: PhantomData<Rc<()>>,
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

impl Gfx942PersistentDeviceAllocationV1 {
    pub fn from_local_mapping(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Self {
        let binding = lease.storage_identity();
        let byte_len = lease.layout().requested_bytes();
        Self::new(
            Gfx942PersistentNativeAllocationV1::Local(lease),
            binding,
            Gfx942PersistentMappingFormV1::Local,
            byte_len,
        )
    }

    #[allow(clippy::result_large_err)]
    pub fn from_exact_two_device_peer_mapping(
        mapping: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<Self, Gfx942XgmiMappedDeviceMemoryV1> {
        let gpu_ids = mapping.gpu_ids();
        let Ok(gpu_ids) = <[u32; 2]>::try_from(gpu_ids) else {
            return Err(mapping);
        };
        if !mapping.is_fully_mapped() || gpu_ids[0] >= gpu_ids[1] {
            return Err(mapping);
        }
        let binding = mapping.lease().storage_identity();
        let byte_len = mapping.lease().layout().requested_bytes();
        Ok(Self::new(
            Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(mapping),
            binding,
            Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { gpu_ids },
            byte_len,
        ))
    }

    fn new(
        native: Gfx942PersistentNativeAllocationV1,
        binding: Gfx942DeviceMemoryIdentityV1,
        mapping: Gfx942PersistentMappingFormV1,
        byte_len: u64,
    ) -> Self {
        Self {
            incarnation: Rc::new(()),
            native: Some(native),
            binding,
            mapping,
            byte_len,
            ledger: [None; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1],
            next_generation: 1,
            next_sequence: 1,
            frontier_generation: 0,
            frontier_sequence: None,
            quarantine: None,
            thread_affinity: PhantomData,
        }
    }

    pub const fn mapping_form(&self) -> Gfx942PersistentMappingFormV1 {
        self.mapping
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn quarantine_reason(&self) -> Option<Gfx942PersistentQuarantineReasonV1> {
        self.quarantine
    }

    pub fn live_use_count(&self) -> usize {
        self.ledger
            .iter()
            .flatten()
            .filter(|record| record.state != LedgerStateV1::Settled)
            .count()
    }

    pub fn retained_settled_use_count(&self) -> usize {
        self.ledger
            .iter()
            .flatten()
            .filter(|record| record.state == LedgerStateV1::Settled)
            .count()
    }

    pub fn reserve(
        &mut self,
        request: Gfx942PersistentUseRequestV1,
        dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
        Gfx942PersistentReservationFailureV1,
    > {
        let fail = |error| Gfx942PersistentReservationFailureV1 { error, request };
        if self.quarantine.is_some() {
            return Err(fail(Gfx942PersistentUseErrorV1::Quarantined));
        }
        let Some(end) = request.range.end() else {
            return Err(fail(Gfx942PersistentUseErrorV1::InvalidRange));
        };
        if request.range.byte_len == 0 || end > self.byte_len {
            return Err(fail(Gfx942PersistentUseErrorV1::InvalidRange));
        }
        if request.owner() == Gfx942PersistentUseOwnerV1::PeerMapped
            && !matches!(
                self.mapping,
                Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { .. }
            )
        {
            return Err(fail(
                Gfx942PersistentUseErrorV1::OperationRequiresPeerMapping,
            ));
        }

        let mut needs_dependency = false;
        for record in self.ledger.iter().flatten() {
            if !request.range.overlaps(record.request.range) {
                continue;
            }
            let hazard = request.access().writes() || record.request.access().writes();
            if !hazard {
                continue;
            }
            if record.state == LedgerStateV1::Settled {
                needs_dependency = true;
            } else {
                return Err(fail(Gfx942PersistentUseErrorV1::OverlappingWriterActive));
            }
        }
        match (needs_dependency, dependency) {
            (true, None) => return Err(fail(Gfx942PersistentUseErrorV1::DependencyRequired)),
            (false, Some(_)) => {
                return Err(fail(Gfx942PersistentUseErrorV1::DependencyNotRequired));
            }
            (true, Some(dependency))
                if !Rc::ptr_eq(&dependency.incarnation, &self.incarnation)
                    || dependency.binding != self.binding
                    || dependency.generation != self.frontier_generation
                    || Some(dependency.through_sequence) != self.frontier_sequence =>
            {
                return Err(fail(
                    Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency,
                ));
            }
            _ => {}
        }

        let Some(slot) = self.ledger.iter().position(Option::is_none) else {
            return Err(fail(Gfx942PersistentUseErrorV1::Capacity));
        };
        let generation = self.next_generation;
        let sequence = self.next_sequence;
        let Some(next_generation) = generation.checked_add(1) else {
            return Err(fail(Gfx942PersistentUseErrorV1::GenerationExhausted));
        };
        let Some(next_sequence) = sequence.checked_add(1) else {
            return Err(fail(Gfx942PersistentUseErrorV1::GenerationExhausted));
        };
        self.next_generation = next_generation;
        self.next_sequence = next_sequence;
        self.ledger[slot] = Some(LedgerRecordV1 {
            generation,
            sequence,
            request,
            state: LedgerStateV1::Reserved,
        });
        Ok(Gfx942PersistentUseLeaseV1 {
            incarnation: Rc::clone(&self.incarnation),
            binding: self.binding,
            slot: u8::try_from(slot).expect("ledger bound fits u8"),
            generation,
            sequence,
            request,
            marker: PhantomData,
            thread_affinity: PhantomData,
        })
    }

    pub fn prepare(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentReservedV1>,
    > {
        self.transition(lease, LedgerStateV1::Reserved, LedgerStateV1::Prepared)
    }

    pub fn publish(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>,
    > {
        self.transition(lease, LedgerStateV1::Prepared, LedgerStateV1::Published)
    }

    pub fn complete(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>,
    > {
        self.transition(lease, LedgerStateV1::Published, LedgerStateV1::Completed)
    }

    fn transition<S: Gfx942PersistentUseStateV1, T: Gfx942PersistentUseStateV1>(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
        next: LedgerStateV1,
    ) -> Result<Gfx942PersistentUseLeaseV1<T>, Gfx942PersistentTransitionFailureV1<S>> {
        let result = self.validate_lease(&lease, expected);
        if let Err(error) = result {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = next;
        Ok(lease.retag())
    }

    pub fn cancel_reserved(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentReservedV1>> {
        self.cancel(lease, LedgerStateV1::Reserved)
    }

    pub fn cancel_prepared(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>> {
        self.cancel(lease, LedgerStateV1::Prepared)
    }

    fn cancel<S: Gfx942PersistentUseStateV1>(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<S>> {
        if let Err(error) = self.validate_lease(&lease, expected) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.ledger[usize::from(lease.slot)] = None;
        Ok(())
    }

    pub fn observe_timeout(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    ) -> Result<
        Gfx942PersistentTimeoutV1,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>,
    > {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Published) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        Ok(Gfx942PersistentTimeoutV1 { published: lease })
    }

    pub fn settle(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    ) -> Result<
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentCompletedV1>,
    > {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Completed) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        if self.ledger.iter().flatten().any(|record| {
            record.sequence < lease.sequence && record.state != LedgerStateV1::Settled
        }) {
            return Err(Gfx942PersistentTransitionFailureV1 {
                error: Gfx942PersistentUseErrorV1::EarlierUseNotSettled,
                lease,
            });
        }
        let Some(frontier_generation) = self.frontier_generation.checked_add(1) else {
            return Err(Gfx942PersistentTransitionFailureV1 {
                error: Gfx942PersistentUseErrorV1::GenerationExhausted,
                lease,
            });
        };
        self.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Settled;
        self.frontier_generation = frontier_generation;
        self.frontier_sequence = Some(lease.sequence);
        Ok(Gfx942PersistentDependencyFrontierV1 {
            incarnation: Rc::clone(&self.incarnation),
            binding: self.binding,
            generation: frontier_generation,
            through_sequence: lease.sequence,
            thread_affinity: PhantomData,
        })
    }

    /// Retires settled history after the caller has established quiescence.
    /// This is a ledger transition only and performs no device operation.
    pub fn retire_settled_frontier(
        &mut self,
        frontier: Gfx942PersistentDependencyFrontierV1,
    ) -> Result<(), Gfx942PersistentDependencyFrontierV1> {
        let current = Rc::ptr_eq(&frontier.incarnation, &self.incarnation)
            && frontier.binding == self.binding
            && frontier.generation == self.frontier_generation
            && Some(frontier.through_sequence) == self.frontier_sequence;
        let has_active = self
            .ledger
            .iter()
            .flatten()
            .any(|record| record.state != LedgerStateV1::Settled);
        if !current || has_active || self.quarantine.is_some() {
            return Err(frontier);
        }
        for slot in &mut self.ledger {
            if slot
                .as_ref()
                .is_some_and(|record| record.state == LedgerStateV1::Settled)
            {
                *slot = None;
            }
        }
        self.frontier_sequence = None;
        Ok(())
    }

    pub fn quarantine_published(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>> {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Published) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Quarantined;
        self.quarantine = Some(reason);
        Ok(())
    }

    /// Quarantines a prepared use after the native adapter crossed its point
    /// of no return without confirming publication. A prepared use must not be
    /// relabeled published merely because lower-layer custody was retained.
    pub(crate) fn quarantine_prepared(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>> {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Prepared) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Quarantined;
        self.quarantine = Some(reason);
        Ok(())
    }

    /// Temporarily moves the exact local mapping into a queue record. The
    /// queue adapter retains this owner while the native authority is absent.
    pub(crate) fn detach_local_native_for_sdma(
        &mut self,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, Gfx942PersistentUseErrorV1>
    {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        match self.native.take() {
            Some(Gfx942PersistentNativeAllocationV1::Local(lease)) => Ok(lease),
            Some(native @ Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(_)) => {
                self.native = Some(native);
                Err(Gfx942PersistentUseErrorV1::WrongState)
            }
            None => Err(Gfx942PersistentUseErrorV1::WrongState),
        }
    }

    /// Restores only the exact local mapping detached from this owner.
    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_local_native_from_sdma(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<
        (),
        (
            Gfx942PersistentUseErrorV1,
            Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        ),
    > {
        if self.native.is_some() {
            return Err((Gfx942PersistentUseErrorV1::WrongState, lease));
        }
        if lease.storage_identity() != self.binding
            || lease.layout().requested_bytes() != self.byte_len
        {
            return Err((Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration, lease));
        }
        self.native = Some(Gfx942PersistentNativeAllocationV1::Local(lease));
        Ok(())
    }

    pub(crate) fn local_native_is_attached_for_sdma(&self) -> bool {
        matches!(
            self.native,
            Some(Gfx942PersistentNativeAllocationV1::Local(_))
        )
    }

    pub(crate) fn local_native_for_sdma(
        &self,
    ) -> Option<&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>> {
        match self.native.as_ref()? {
            Gfx942PersistentNativeAllocationV1::Local(lease) => Some(lease),
            Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(_) => None,
        }
    }

    /// Records caller-reported currentness loss even when no use is published.
    /// This core does not itself observe KFD, DRM, topology, or queue state.
    pub fn quarantine_for_caller_reported_currentness_loss(&mut self) {
        self.quarantine = Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss);
    }

    /// Returns native cleanup custody only after every use is settled or
    /// cancelled. A quarantined owner is deliberately returned intact.
    #[allow(clippy::result_large_err)]
    pub fn try_into_native(
        mut self,
    ) -> Result<Gfx942PersistentNativeAllocationV1, (Gfx942PersistentUseErrorV1, Self)> {
        if self.quarantine.is_some() {
            return Err((Gfx942PersistentUseErrorV1::Quarantined, self));
        }
        if self
            .ledger
            .iter()
            .flatten()
            .any(|record| record.state != LedgerStateV1::Settled)
        {
            return Err((Gfx942PersistentUseErrorV1::OutstandingUses, self));
        }
        Ok(self
            .native
            .take()
            .expect("persistent owner retains native authority"))
    }

    fn validate_lease<S: Gfx942PersistentUseStateV1>(
        &self,
        lease: &Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        if !Rc::ptr_eq(&lease.incarnation, &self.incarnation) || lease.binding != self.binding {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        let Some(record) = self
            .ledger
            .get(usize::from(lease.slot))
            .and_then(Option::as_ref)
        else {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        };
        if record.generation != lease.generation
            || record.sequence != lease.sequence
            || record.request != lease.request
        {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        if record.state != expected {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        Ok(())
    }
}

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
    if source_owner.quarantine.is_some() || destination_owner.quarantine.is_some() {
        return Err(Gfx942PersistentUseErrorV1::Quarantined);
    }
    if Rc::ptr_eq(&source_owner.incarnation, &destination_owner.incarnation)
        || source_owner.binding == destination_owner.binding
        || !matches!(
            source_owner.native,
            Some(Gfx942PersistentNativeAllocationV1::Local(_))
        )
        || !matches!(
            destination_owner.native,
            Some(Gfx942PersistentNativeAllocationV1::Local(_))
        )
    {
        return Err(Gfx942PersistentUseErrorV1::WrongState);
    }
    let Some(Gfx942PersistentNativeAllocationV1::Local(source)) = source_owner.native.take() else {
        unreachable!("prevalidated local source native custody")
    };
    let Some(Gfx942PersistentNativeAllocationV1::Local(destination)) =
        destination_owner.native.take()
    else {
        unreachable!("prevalidated local destination native custody")
    };
    Ok((source, destination))
}

#[allow(clippy::result_large_err)]
pub(crate) fn restore_local_native_pair_from_sdma_v1(
    source_owner: &mut Gfx942PersistentDeviceAllocationV1,
    source: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    destination_owner: &mut Gfx942PersistentDeviceAllocationV1,
    destination: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
) -> Result<
    (),
    (
        Gfx942PersistentUseErrorV1,
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ),
> {
    if source_owner.quarantine.is_some() || destination_owner.quarantine.is_some() {
        return Err((Gfx942PersistentUseErrorV1::Quarantined, source, destination));
    }
    if Rc::ptr_eq(&source_owner.incarnation, &destination_owner.incarnation)
        || source_owner.binding == destination_owner.binding
        || source_owner.native.is_some()
        || destination_owner.native.is_some()
        || source.storage_identity() != source_owner.binding
        || destination.storage_identity() != destination_owner.binding
        || source.storage_identity() == destination.storage_identity()
        || source.layout().requested_bytes() != source_owner.byte_len
        || destination.layout().requested_bytes() != destination_owner.byte_len
    {
        return Err((
            Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration,
            source,
            destination,
        ));
    }
    source_owner.native = Some(Gfx942PersistentNativeAllocationV1::Local(source));
    destination_owner.native = Some(Gfx942PersistentNativeAllocationV1::Local(destination));
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
    source_owner.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = next;
    destination_owner.ledger[usize::from(destination.slot)]
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
    source_owner.ledger[usize::from(source.slot)] = None;
    destination_owner.ledger[usize::from(destination.slot)] = None;
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
    source_owner.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Quarantined;
    destination_owner.ledger[usize::from(destination.slot)]
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
    source_owner.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Quarantined;
    destination_owner.ledger[usize::from(destination.slot)]
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
        if source_owner.ledger.iter().flatten().any(|record| {
            record.sequence < source.sequence && record.state != LedgerStateV1::Settled
        }) || destination_owner.ledger.iter().flatten().any(|record| {
            record.sequence < destination.sequence && record.state != LedgerStateV1::Settled
        }) {
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
    source_owner.ledger[usize::from(source.slot)]
        .as_mut()
        .expect("validated source ledger slot")
        .state = LedgerStateV1::Settled;
    destination_owner.ledger[usize::from(destination.slot)]
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
                .ledger
                .iter()
                .flatten()
                .all(|record| record.state == LedgerStateV1::Settled)
            && owner.quarantine.is_none()
    };
    if !current(source_owner, &source) || !current(destination_owner, &destination) {
        return Err((source, destination));
    }
    for slot in &mut source_owner.ledger {
        if slot
            .as_ref()
            .is_some_and(|record| record.state == LedgerStateV1::Settled)
        {
            *slot = None;
        }
    }
    for slot in &mut destination_owner.ledger {
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
mod tests {
    use super::*;
    use crate::shared_memory::{
        local_mapping_for_persistent_sdma_test, xgmi_mapping_for_sdma_test,
    };

    fn owner(id: u64) -> Gfx942PersistentDeviceAllocationV1 {
        match Gfx942PersistentDeviceAllocationV1::from_exact_two_device_peer_mapping(
            xgmi_mapping_for_sdma_test(id),
        ) {
            Ok(owner) => owner,
            Err(_) => panic!("test fixture is an exact complete peer mapping"),
        }
    }

    fn request(
        operation: Gfx942PersistentOperationV1,
        offset: u64,
        byte_len: u64,
    ) -> Gfx942PersistentUseRequestV1 {
        Gfx942PersistentUseRequestV1::new(operation, offset, byte_len).unwrap()
    }

    fn settle(
        owner: &mut Gfx942PersistentDeviceAllocationV1,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
    ) -> Gfx942PersistentDependencyFrontierV1 {
        let lease = owner.prepare(lease).unwrap();
        let lease = owner.publish(lease).unwrap();
        let lease = owner.complete(lease).unwrap();
        owner.settle(lease).unwrap()
    }

    #[test]
    fn exact_peer_form_and_operation_access_are_checked() {
        let mut owner = owner(1);
        assert_eq!(
            owner.mapping_form(),
            Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { gpu_ids: [7, 9] }
        );
        let request = request(Gfx942PersistentOperationV1::PeerMappedSource, 0, 8);
        assert_eq!(request.owner(), Gfx942PersistentUseOwnerV1::PeerMapped);
        assert_eq!(request.access(), Gfx942PersistentAccessV1::Read);
        assert!(owner.reserve(request, None).is_ok());
        assert!(
            Gfx942PersistentUseRequestV1::new(
                Gfx942PersistentOperationV1::ComputeRead,
                u64::MAX,
                2
            )
            .is_err()
        );
    }

    #[test]
    fn capacity_and_allocation_bounds_are_preflighted() {
        let mut owner = owner(2);
        let outside = request(Gfx942PersistentOperationV1::ComputeRead, 4095, 2);
        assert_eq!(
            owner.reserve(outside, None).unwrap_err().error(),
            Gfx942PersistentUseErrorV1::InvalidRange
        );
        let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 1);
        let mut leases = Vec::new();
        for _ in 0..GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 {
            leases.push(owner.reserve(read, None).unwrap());
        }
        assert_eq!(
            owner.reserve(read, None).unwrap_err().error(),
            Gfx942PersistentUseErrorV1::Capacity
        );
        assert_eq!(leases.len(), GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1);
    }

    #[test]
    fn owner_and_generation_substitution_recover_move_only_custody() {
        let mut first = owner(3);
        let mut second = owner(4);
        let lease = first
            .reserve(
                request(Gfx942PersistentOperationV1::ComputeRead, 0, 8),
                None,
            )
            .unwrap();
        let failure = second.prepare(lease).unwrap_err();
        assert_eq!(
            failure.error(),
            Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
        );
        let (_, mut lease) = failure.into_parts();
        lease.generation += 1;
        let failure = first.prepare(lease).unwrap_err();
        assert_eq!(
            failure.error(),
            Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
        );
    }

    #[test]
    fn reads_coexist_writers_conflict_and_disjoint_writers_coexist() {
        let mut owner = owner(5);
        let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 16);
        let first = owner.reserve(read, None).unwrap();
        let second = owner.reserve(read, None).unwrap();
        let writer = request(Gfx942PersistentOperationV1::LocalSdmaDestination, 8, 4);
        assert_eq!(
            owner.reserve(writer, None).unwrap_err().error(),
            Gfx942PersistentUseErrorV1::OverlappingWriterActive
        );
        let disjoint = request(Gfx942PersistentOperationV1::LocalSdmaDestination, 32, 4);
        let disjoint = owner.reserve(disjoint, None).unwrap();
        owner.cancel_reserved(first).unwrap();
        owner.cancel_reserved(second).unwrap();
        owner.cancel_reserved(disjoint).unwrap();
    }

    #[test]
    fn overlapping_writer_requires_exact_current_successful_frontier() {
        let mut owner = owner(6);
        let first = owner
            .reserve(
                request(Gfx942PersistentOperationV1::ComputeRead, 0, 16),
                None,
            )
            .unwrap();
        let frontier = settle(&mut owner, first);
        let writer = request(Gfx942PersistentOperationV1::ComputeWrite, 0, 16);
        assert_eq!(
            owner.reserve(writer, None).unwrap_err().error(),
            Gfx942PersistentUseErrorV1::DependencyRequired
        );
        let mut other = self::owner(7);
        let other_lease = other
            .reserve(
                request(Gfx942PersistentOperationV1::ComputeRead, 0, 16),
                None,
            )
            .unwrap();
        let other_frontier = settle(&mut other, other_lease);
        assert_eq!(
            owner
                .reserve(writer, Some(&other_frontier))
                .unwrap_err()
                .error(),
            Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
        );
        let writer = owner.reserve(writer, Some(&frontier)).unwrap();
        let newer = settle(&mut owner, writer);
        assert!(newer.through_sequence() > frontier.through_sequence());
        let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 16);
        assert_eq!(
            owner.reserve(read, Some(&frontier)).unwrap_err().error(),
            Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
        );
        assert!(owner.reserve(read, Some(&newer)).is_ok());
    }

    #[test]
    fn prepublication_cancel_reclaims_slots() {
        let mut owner = owner(8);
        let request = request(Gfx942PersistentOperationV1::ComputeRead, 0, 8);
        let reserved = owner.reserve(request, None).unwrap();
        owner.cancel_reserved(reserved).unwrap();
        let reserved = owner.reserve(request, None).unwrap();
        let prepared = owner.prepare(reserved).unwrap();
        owner.cancel_prepared(prepared).unwrap();
        assert_eq!(owner.live_use_count(), 0);
        assert_eq!(owner.retained_settled_use_count(), 0);
    }

    #[test]
    fn timeout_retains_exact_published_custody() {
        let mut owner = owner(9);
        let request = request(Gfx942PersistentOperationV1::LocalSdmaSource, 12, 8);
        let lease = owner.reserve(request, None).unwrap();
        let lease = owner.prepare(lease).unwrap();
        let lease = owner.publish(lease).unwrap();
        let timeout = owner.observe_timeout(lease).unwrap();
        assert_eq!(timeout.request(), request);
        assert_eq!(owner.live_use_count(), 1);
        let lease = owner.complete(timeout.into_published()).unwrap();
        let _frontier = owner.settle(lease).unwrap();
    }

    #[test]
    fn indeterminate_publication_quarantines_and_blocks_release() {
        let mut owner = owner(10);
        let lease = owner
            .reserve(
                request(Gfx942PersistentOperationV1::PeerMappedDestination, 0, 8),
                None,
            )
            .unwrap();
        let lease = owner.prepare(lease).unwrap();
        let lease = owner.publish(lease).unwrap();
        owner
            .quarantine_published(
                lease,
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            )
            .unwrap();
        assert_eq!(
            owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
        );
        let (error, owner) = owner.try_into_native().unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
        assert_eq!(owner.live_use_count(), 1);
    }

    #[test]
    fn caller_reported_currentness_loss_quarantines_without_claiming_observation() {
        let mut owner = owner(12);
        owner.quarantine_for_caller_reported_currentness_loss();
        assert_eq!(
            owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
        );
        let (error, _) = owner.try_into_native().unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
    }

    #[test]
    fn prepared_indeterminate_use_is_quarantined_without_fake_publication() {
        let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            local_mapping_for_persistent_sdma_test(13),
        );
        let reserved = owner
            .reserve(
                request(Gfx942PersistentOperationV1::LocalSdmaDestination, 0, 8),
                None,
            )
            .unwrap();
        let prepared = owner.prepare(reserved).unwrap();
        owner
            .quarantine_prepared(
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            )
            .unwrap();
        assert_eq!(
            owner.quarantine_reason(),
            Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
        );
        let (error, _) = owner.try_into_native().unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
    }

    #[test]
    fn sdma_detach_restore_requires_exact_local_native_identity() {
        let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            local_mapping_for_persistent_sdma_test(14),
        );
        let lease = owner.detach_local_native_for_sdma().unwrap();
        assert!(!owner.local_native_is_attached_for_sdma());
        let foreign = local_mapping_for_persistent_sdma_test(15);
        let (error, foreign) = owner.restore_local_native_from_sdma(foreign).unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        assert!(!owner.local_native_is_attached_for_sdma());
        owner.restore_local_native_from_sdma(lease).unwrap();
        assert!(owner.local_native_is_attached_for_sdma());
        assert_eq!(foreign.layout().requested_bytes(), owner.byte_len());
    }

    #[test]
    fn release_requires_settlement_and_returns_exact_native_owner() {
        let mut owner = owner(11);
        let lease = owner
            .reserve(
                request(Gfx942PersistentOperationV1::ComputeRead, 0, 8),
                None,
            )
            .unwrap();
        let (error, mut owner) = owner.try_into_native().unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::OutstandingUses);
        let frontier = settle(&mut owner, lease);
        assert_eq!(owner.retained_settled_use_count(), 1);
        owner.retire_settled_frontier(frontier).unwrap();
        assert_eq!(owner.retained_settled_use_count(), 0);
        assert!(matches!(
            owner.try_into_native().unwrap(),
            Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(_)
        ));
    }
}
