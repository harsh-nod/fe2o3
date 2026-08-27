use crate::{
    ObservedContext,
    generated_argument_plan::{
        GeneratedArgumentInputV1, GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1,
        GeneratedDeviceScalarV1,
    },
};
use fe2o3_core::{
    DeviceBuffer, DeviceBufferIdentity, DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut,
    DeviceCopy, DevicePtr,
};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Opaque identity for one device allocation in one exact HIP context.
///
/// This is a symbolic comparison key. It exposes neither the allocation's raw
/// address nor a constructor that could assign two identities to one address.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct AllocationIdentity {
    context: usize,
    allocation: usize,
}

impl fmt::Debug for AllocationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AllocationIdentity(..)")
    }
}

/// Provenance and extent for a live device allocation.
///
/// A value borrowed from a [`DeviceBuffer`] cannot outlive that buffer. Creating
/// this value does not grant permission to launch or to access the allocation.
pub struct AllocationProvenance<'allocation> {
    identity: AllocationIdentity,
    context: ObservedContext,
    byte_length: usize,
    marker: PhantomData<&'allocation ()>,
}

impl fmt::Debug for AllocationProvenance<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AllocationProvenance")
            .field("identity", &self.identity)
            .field("byte_length", &self.byte_length)
            .finish_non_exhaustive()
    }
}

impl<'allocation> AllocationProvenance<'allocation> {
    /// Derives provenance from a live, owning `DeviceBuffer` in the observed
    /// context.
    pub fn from_device_buffer<T: DeviceCopy>(
        observed: &ObservedContext,
        buffer: &'allocation DeviceBuffer<T>,
    ) -> Result<Self, RegionError> {
        if !observed.is_for_context(buffer.context()) {
            return Err(RegionError::WrongContext);
        }

        let byte_length = buffer
            .len()
            .checked_mul(size_of::<T>())
            .ok_or(RegionError::AllocationSizeOverflow)?;
        let allocation = buffer.as_device_ptr().as_raw().addr();
        allocation
            .checked_add(byte_length)
            .ok_or(RegionError::AllocationAddressOverflow)?;

        Ok(Self::from_parts(observed, allocation, byte_length))
    }

    /// Declares provenance for a raw device allocation.
    ///
    /// The `owner` borrow makes the declared allocation lifetime explicit but
    /// is otherwise not inspected.
    ///
    /// # Safety
    ///
    /// `address..address + byte_length` must name exactly one live allocation
    /// owned by `owner` in `observed`. Repeated declarations for the same live
    /// allocation must use the same base address and context. The allocation
    /// must not be freed or repurposed for the returned value's lifetime.
    pub unsafe fn from_raw_parts<Owner: ?Sized>(
        observed: &ObservedContext,
        owner: &'allocation Owner,
        address: *mut u8,
        byte_length: usize,
    ) -> Result<Self, RegionError> {
        let _ = owner;
        let allocation = address.addr();
        allocation
            .checked_add(byte_length)
            .ok_or(RegionError::AllocationAddressOverflow)?;
        Ok(Self::from_parts(observed, allocation, byte_length))
    }

    fn from_parts(observed: &ObservedContext, allocation: usize, byte_length: usize) -> Self {
        Self {
            identity: AllocationIdentity {
                context: observed.context_key(),
                allocation,
            },
            context: observed.clone(),
            byte_length,
            marker: PhantomData,
        }
    }

    pub const fn identity(&self) -> AllocationIdentity {
        self.identity
    }

    pub const fn byte_length(&self) -> usize {
        self.byte_length
    }

    /// Checks and describes a byte subregion of this allocation.
    pub fn region(
        &self,
        byte_offset: usize,
        byte_length: usize,
    ) -> Result<CheckedByteRegion<'allocation>, RegionError> {
        let byte_end =
            byte_offset
                .checked_add(byte_length)
                .ok_or(RegionError::OffsetLengthOverflow {
                    byte_offset,
                    byte_length,
                })?;
        if byte_end > self.byte_length {
            return Err(RegionError::OutOfBounds {
                allocation_length: self.byte_length,
                byte_offset,
                byte_length,
            });
        }

        Ok(CheckedByteRegion {
            identity: self.identity,
            context: self.context.clone(),
            byte_offset,
            byte_length,
            byte_end,
            marker: PhantomData,
        })
    }
}

/// A checked half-open byte range within one live allocation.
pub struct CheckedByteRegion<'allocation> {
    identity: AllocationIdentity,
    context: ObservedContext,
    byte_offset: usize,
    byte_length: usize,
    byte_end: usize,
    marker: PhantomData<&'allocation ()>,
}

impl fmt::Debug for CheckedByteRegion<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedByteRegion")
            .field("identity", &self.identity)
            .field("byte_offset", &self.byte_offset)
            .field("byte_length", &self.byte_length)
            .finish_non_exhaustive()
    }
}

impl CheckedByteRegion<'_> {
    pub const fn allocation(&self) -> AllocationIdentity {
        self.identity
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn byte_length(&self) -> usize {
        self.byte_length
    }

    pub const fn byte_end(&self) -> usize {
        self.byte_end
    }

    pub const fn is_empty(&self) -> bool {
        self.byte_length == 0
    }
}

pub(super) struct GeneratedDeviceSliceMetadata {
    identity: AllocationIdentity,
    _device_buffer_identity: DeviceBufferIdentity,
    context: ObservedContext,
    byte_offset: usize,
    byte_length: usize,
}

/// Crate-private proof that a generated argument remains tied to an original
/// allocation borrow even after its capability wrapper is dropped.
pub(super) struct GeneratedArgumentBorrowV1<'allocation>(PhantomData<&'allocation ()>);

impl GeneratedArgumentBorrowV1<'_> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

#[cfg(test)]
pub(super) const fn generated_argument_borrow_for_test() -> GeneratedArgumentBorrowV1<'static> {
    GeneratedArgumentBorrowV1::new()
}

impl GeneratedDeviceSliceMetadata {
    fn from_region<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(
        observed: &ObservedContext,
        region: &R,
    ) -> Result<Self, RegionError> {
        Self::from_region_with_empty_policy(observed, region, true)
    }

    fn from_region_with_empty_policy<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(
        observed: &ObservedContext,
        region: &R,
        allow_empty: bool,
    ) -> Result<Self, RegionError> {
        if !observed.is_for_context(region.context()) {
            return Err(RegionError::WrongContext);
        }

        let allocation_address = region.allocation_device_ptr().as_raw().addr();
        let allocation_len = region.allocation_len();
        let region_address = region.region_device_ptr().as_raw().addr();
        let region_len = region.region_len();
        let checked = if allow_empty {
            checked_device_region_allow_empty::<T>(
                allocation_address,
                allocation_len,
                region_address,
                region_len,
            )
        } else {
            checked_device_region::<T>(
                allocation_address,
                allocation_len,
                region_address,
                region_len,
            )
        }?;

        Ok(Self {
            identity: AllocationIdentity {
                context: observed.context_key(),
                allocation: region.allocation_device_ptr().as_raw().addr(),
            },
            _device_buffer_identity: region.allocation_identity(),
            context: observed.clone(),
            byte_offset: checked.byte_offset,
            byte_length: checked.byte_length,
        })
    }

    pub(super) fn checked_region<'allocation>(&self) -> CheckedByteRegion<'allocation> {
        let byte_end = self
            .byte_offset
            .checked_add(self.byte_length)
            .expect("generated device region was checked at construction");
        CheckedByteRegion {
            identity: self.identity,
            context: self.context.clone(),
            byte_offset: self.byte_offset,
            byte_length: self.byte_length,
            byte_end,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDeviceRegion {
    byte_offset: usize,
    byte_length: usize,
}

fn checked_device_region<T>(
    allocation_address: usize,
    allocation_len: usize,
    region_address: usize,
    region_len: usize,
) -> Result<CheckedDeviceRegion, RegionError> {
    checked_device_region_impl::<T>(
        allocation_address,
        allocation_len,
        region_address,
        region_len,
        false,
    )
}

fn checked_device_region_allow_empty<T>(
    allocation_address: usize,
    allocation_len: usize,
    region_address: usize,
    region_len: usize,
) -> Result<CheckedDeviceRegion, RegionError> {
    checked_device_region_impl::<T>(
        allocation_address,
        allocation_len,
        region_address,
        region_len,
        true,
    )
}

fn checked_device_region_impl<T>(
    allocation_address: usize,
    allocation_len: usize,
    region_address: usize,
    region_len: usize,
    allow_empty: bool,
) -> Result<CheckedDeviceRegion, RegionError> {
    if size_of::<T>() == 0 {
        return Err(RegionError::ZeroSizedElement);
    }
    if region_len == 0 && !allow_empty {
        return Err(RegionError::EmptyRegion);
    }

    let allocation_byte_length = allocation_len
        .checked_mul(size_of::<T>())
        .ok_or(RegionError::AllocationSizeOverflow)?;
    allocation_address
        .checked_add(allocation_byte_length)
        .ok_or(RegionError::AllocationAddressOverflow)?;
    let byte_length = region_len
        .checked_mul(size_of::<T>())
        .ok_or(RegionError::AllocationSizeOverflow)?;
    let byte_offset = region_address.checked_sub(allocation_address).ok_or(
        RegionError::RegionAddressOutsideAllocation {
            allocation_address,
            region_address,
        },
    )?;
    let byte_end =
        byte_offset
            .checked_add(byte_length)
            .ok_or(RegionError::OffsetLengthOverflow {
                byte_offset,
                byte_length,
            })?;
    if byte_end > allocation_byte_length {
        return Err(RegionError::OutOfBounds {
            allocation_length: allocation_byte_length,
            byte_offset,
            byte_length,
        });
    }

    Ok(CheckedDeviceRegion {
        byte_offset,
        byte_length,
    })
}

/// Generated read-only capability for one checked typed device-buffer region.
///
/// This doc-hidden SPI owns the actual shared buffer borrow. Its admission
/// access is fixed to [`ArgumentAccessMode::SharedRead`], and its packing helper
/// emits the pointer and element count from that same retained buffer. Empty
/// regions retain their allocation provenance and describe no memory effects.
#[doc(hidden)]
pub struct GeneratedReadDeviceSlice<'allocation, T: DeviceCopy> {
    pointer: DevicePtr<T>,
    len: usize,
    metadata: GeneratedDeviceSliceMetadata,
    marker: PhantomData<&'allocation T>,
}

impl<'allocation, T: DeviceCopy> GeneratedReadDeviceSlice<'allocation, T> {
    pub fn new(
        observed: &ObservedContext,
        buffer: &'allocation DeviceBuffer<T>,
    ) -> Result<Self, RegionError> {
        Self::from_region(observed, buffer)
    }

    /// Retains one checked immutable subregion as a generated shared argument.
    pub fn from_view(
        observed: &ObservedContext,
        view: DeviceBufferView<'allocation, T>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region(observed, &view)?;
        Ok(Self {
            pointer: view.region_device_ptr(),
            len: view.region_len(),
            metadata,
            marker: PhantomData,
        })
    }

    fn from_region<R: DeviceBufferRegion<T> + ?Sized>(
        observed: &ObservedContext,
        region: &'allocation R,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region(observed, region)?;
        Ok(Self {
            pointer: region.region_device_ptr(),
            len: region.region_len(),
            metadata,
            marker: PhantomData,
        })
    }

    pub fn argument_access(&self) -> ArgumentAccess<'allocation> {
        ArgumentAccess::new(
            self.metadata.checked_region(),
            ArgumentAccessMode::SharedRead,
        )
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Binds this retained shared slice to one exact generated argument plan.
    pub fn bind_argument(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedArgumentInputV1<'allocation>, GeneratedArgumentPackError>
    where
        T: GeneratedDeviceScalarV1,
    {
        plan.bind_generated_read_slice_v1::<T>(
            argument_index,
            self.pointer.as_raw().addr(),
            self.len,
            GeneratedArgumentBorrowV1::new(),
        )
    }

    /// Binds this capability and its exact access record as one opaque unit.
    #[doc(hidden)]
    pub fn bind_argument_pair(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError>
    where
        T: GeneratedDeviceScalarV1,
    {
        Ok(GeneratedSliceArgumentPairV1 {
            input: self.bind_argument(plan, argument_index)?,
            access: self.argument_access(),
        })
    }
}

/// Generated initialized read-write capability for one exclusive typed region.
///
/// This doc-hidden SPI owns the actual exclusive buffer borrow. Its admission
/// access is distinct from write-only output and its safe packing helper binds
/// the same retained pointer and length to an exact canonical `DisjointSlice`
/// argument. Empty regions retain their allocation provenance and describe no
/// memory effects.
#[doc(hidden)]
pub struct GeneratedReadWriteDeviceSlice<'allocation, T: DeviceCopy> {
    pointer: DevicePtr<T>,
    len: usize,
    metadata: GeneratedDeviceSliceMetadata,
    marker: PhantomData<&'allocation mut T>,
}

impl<'allocation, T: DeviceCopy> GeneratedReadWriteDeviceSlice<'allocation, T> {
    pub fn new(
        observed: &ObservedContext,
        buffer: &'allocation mut DeviceBuffer<T>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region(observed, buffer)?;
        Ok(Self {
            pointer: buffer.region_device_ptr(),
            len: buffer.region_len(),
            metadata,
            marker: PhantomData,
        })
    }

    /// Consumes one checked exclusive subregion as a generated read-write argument.
    pub fn from_view_mut(
        observed: &ObservedContext,
        view: DeviceBufferViewMut<'allocation, T>,
    ) -> Result<Self, RegionError> {
        let metadata = GeneratedDeviceSliceMetadata::from_region(observed, &view)?;
        Ok(Self {
            pointer: view.region_device_ptr(),
            len: view.region_len(),
            metadata,
            marker: PhantomData,
        })
    }

    pub fn argument_access(&self) -> ArgumentAccess<'allocation> {
        ArgumentAccess::new(
            self.metadata.checked_region(),
            ArgumentAccessMode::ExclusiveReadWrite,
        )
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Binds this retained exclusive slice to one exact generated argument plan.
    pub fn bind_argument(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedArgumentInputV1<'allocation>, GeneratedArgumentPackError>
    where
        T: GeneratedDeviceScalarV1,
    {
        plan.bind_generated_read_write_slice_v1::<T>(
            argument_index,
            self.pointer.as_raw().addr(),
            self.len,
            GeneratedArgumentBorrowV1::new(),
        )
    }

    /// Binds this capability and its exact access record as one opaque unit.
    #[doc(hidden)]
    pub fn bind_argument_pair(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError>
    where
        T: GeneratedDeviceScalarV1,
    {
        Ok(GeneratedSliceArgumentPairV1 {
            input: self.bind_argument(plan, argument_index)?,
            access: self.argument_access(),
        })
    }
}

/// Device atomic operation category used by host alias admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AtomicOperation {
    Load,
    Store,
    ReadModifyWrite,
}

/// Device atomic ordering used by host alias admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

/// Synchronization scope of a device atomic operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AtomicScope {
    Workgroup,
    Device,
    System,
}

/// Atomic access details retained for future coverage-aware admission.
///
/// This first admission slice validates and records the descriptor but rejects
/// every overlapping atomic region. Accepting such overlap requires
/// unforgeable evidence that the atomic scope covers every concurrent accessor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicAccess {
    operation: AtomicOperation,
    ordering: AtomicOrdering,
    scope: AtomicScope,
}

impl AtomicAccess {
    pub fn new(
        operation: AtomicOperation,
        ordering: AtomicOrdering,
        scope: AtomicScope,
    ) -> Result<Self, InvalidAtomicOrdering> {
        if ordering_supported(operation, ordering) {
            Ok(Self {
                operation,
                ordering,
                scope,
            })
        } else {
            Err(InvalidAtomicOrdering {
                operation,
                ordering,
            })
        }
    }

    pub const fn operation(self) -> AtomicOperation {
        self.operation
    }

    pub const fn ordering(self) -> AtomicOrdering {
        self.ordering
    }

    pub const fn scope(self) -> AtomicScope {
        self.scope
    }
}

const fn ordering_supported(operation: AtomicOperation, ordering: AtomicOrdering) -> bool {
    match operation {
        AtomicOperation::Load => matches!(
            ordering,
            AtomicOrdering::Relaxed
                | AtomicOrdering::Acquire
                | AtomicOrdering::SequentiallyConsistent
        ),
        AtomicOperation::Store => matches!(
            ordering,
            AtomicOrdering::Relaxed
                | AtomicOrdering::Release
                | AtomicOrdering::SequentiallyConsistent
        ),
        AtomicOperation::ReadModifyWrite => true,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidAtomicOrdering {
    pub operation: AtomicOperation,
    pub ordering: AtomicOrdering,
}

impl fmt::Display for InvalidAtomicOrdering {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "atomic ordering {:?} is invalid for {:?}",
            self.ordering, self.operation
        )
    }
}

impl std::error::Error for InvalidAtomicOrdering {}

/// Executable permission requested for one kernel argument region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArgumentAccessMode {
    SharedRead,
    ExclusiveWrite,
    ExclusiveReadWrite,
    Atomic(AtomicAccess),
}

enum ArgumentRegion<'allocation> {
    Known(CheckedByteRegion<'allocation>),
    Unknown,
}

/// One memory effect declared for a launch argument.
pub struct ArgumentAccess<'allocation> {
    region: ArgumentRegion<'allocation>,
    mode: ArgumentAccessMode,
}

/// Opaque pairing of one branded slice input with the access record emitted by
/// the same retained generated capability.
///
/// There is no public constructor and this value is neither `Clone` nor `Copy`.
/// Generated adapters can obtain it only from a generated slice capability's
/// `bind_argument_pair` method.
#[doc(hidden)]
pub struct GeneratedSliceArgumentPairV1<'allocation> {
    input: GeneratedArgumentInputV1<'allocation>,
    access: ArgumentAccess<'allocation>,
}

impl<'allocation> GeneratedSliceArgumentPairV1<'allocation> {
    #[cfg(feature = "hardware-test-hooks")]
    pub(crate) const fn new(
        input: GeneratedArgumentInputV1<'allocation>,
        access: ArgumentAccess<'allocation>,
    ) -> Self {
        Self { input, access }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedArgumentInputV1<'allocation>,
        ArgumentAccess<'allocation>,
    ) {
        (self.input, self.access)
    }
}

impl fmt::Debug for ArgumentAccess<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = formatter.debug_struct("ArgumentAccess");
        match &self.region {
            ArgumentRegion::Known(region) => output.field("region", region),
            ArgumentRegion::Unknown => output.field("region", &"Unknown"),
        };
        output.field("mode", &self.mode).finish()
    }
}

impl<'allocation> ArgumentAccess<'allocation> {
    pub const fn new(region: CheckedByteRegion<'allocation>, mode: ArgumentAccessMode) -> Self {
        Self {
            region: ArgumentRegion::Known(region),
            mode,
        }
    }

    /// Describes an effect whose allocation provenance could not be retained.
    /// Alias admission always rejects this value.
    pub const fn unknown(mode: ArgumentAccessMode) -> Self {
        Self {
            region: ArgumentRegion::Unknown,
            mode,
        }
    }

    pub const fn mode(&self) -> ArgumentAccessMode {
        self.mode
    }

    pub(crate) fn matches_generated_slice_v1(
        &self,
        address: u64,
        length: u64,
        element_size: u64,
        expected_mode: ArgumentAccessMode,
    ) -> bool {
        let ArgumentRegion::Known(region) = &self.region else {
            return false;
        };
        if self.mode != expected_mode {
            return false;
        }
        let Ok(address) = usize::try_from(address) else {
            return false;
        };
        let Ok(length) = usize::try_from(length) else {
            return false;
        };
        let Ok(element_size) = usize::try_from(element_size) else {
            return false;
        };
        let Some(region_address) = region.identity.allocation.checked_add(region.byte_offset)
        else {
            return false;
        };
        let Some(byte_length) = length.checked_mul(element_size) else {
            return false;
        };
        address == region_address && byte_length == region.byte_length
    }
}

/// Inert evidence that one set of argument regions passed host alias checks.
///
/// This value cannot authorize or enqueue a launch, and it carries no static
/// race-freedom or verification claim.
pub struct ArgumentAliasAdmission<'allocation> {
    context: ObservedContext,
    accesses: Vec<AdmittedAccess<'allocation>>,
}

impl fmt::Debug for ArgumentAliasAdmission<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArgumentAliasAdmission")
            .field("context", &self.context)
            .field("access_count", &self.accesses.len())
            .finish_non_exhaustive()
    }
}

impl ArgumentAliasAdmission<'_> {
    pub fn len(&self) -> usize {
        self.accesses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accesses.is_empty()
    }
}

struct AdmittedAccess<'allocation> {
    region: CheckedByteRegion<'allocation>,
    mode: ArgumentAccessMode,
}

#[derive(Clone, Copy)]
struct AccessDescriptor {
    identity: AllocationIdentity,
    byte_offset: usize,
    byte_end: usize,
    mode: ArgumentAccessMode,
}

impl AccessDescriptor {
    fn from_admitted(access: &AdmittedAccess<'_>) -> Self {
        Self {
            identity: access.region.identity,
            byte_offset: access.region.byte_offset,
            byte_end: access.region.byte_end,
            mode: access.mode,
        }
    }

    const fn is_empty(self) -> bool {
        self.byte_offset == self.byte_end
    }
}

struct RegisteredLaunch {
    seal: Arc<()>,
    accesses: Vec<AccessDescriptor>,
}

#[derive(Default)]
struct AliasAdmissionRegistryState {
    launches: Vec<RegisteredLaunch>,
}

pub(crate) struct AliasAdmissionRegistry {
    context: usize,
    state: Mutex<AliasAdmissionRegistryState>,
}

impl AliasAdmissionRegistry {
    fn new(context: usize) -> Self {
        Self {
            context,
            state: Mutex::new(AliasAdmissionRegistryState::default()),
        }
    }

    fn register<'allocation>(
        self: &Arc<Self>,
        context: &ObservedContext,
        admission: &ArgumentAliasAdmission<'allocation>,
    ) -> Result<InFlightRegionRegistration<'allocation>, AliasAdmissionError> {
        debug_assert_eq!(self.context, context.context_key());
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        for (argument_index, candidate) in admission.accesses.iter().enumerate() {
            let candidate = AccessDescriptor::from_admitted(candidate);
            for (launch_index, launch) in state.launches.iter().enumerate() {
                for (in_flight_argument, existing) in launch.accesses.iter().enumerate() {
                    if descriptors_conflict(candidate, *existing) {
                        return Err(AliasAdmissionError::Conflict {
                            argument_index,
                            conflicting_with: ConflictSource::InFlight {
                                launch_index,
                                argument_index: in_flight_argument,
                            },
                        });
                    }
                }
            }
        }

        let seal = Arc::new(());
        state.launches.push(RegisteredLaunch {
            seal: seal.clone(),
            accesses: admission
                .accesses
                .iter()
                .map(AccessDescriptor::from_admitted)
                .collect(),
        });
        Ok(InFlightRegionRegistration {
            registry: self.clone(),
            seal,
            marker: PhantomData,
        })
    }
}

pub(crate) struct InFlightRegionRegistration<'allocation> {
    registry: Arc<AliasAdmissionRegistry>,
    seal: Arc<()>,
    marker: PhantomData<&'allocation ()>,
}

impl Drop for InFlightRegionRegistration<'_> {
    fn drop(&mut self) {
        let mut state = self
            .registry
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .launches
            .retain(|launch| !Arc::ptr_eq(&launch.seal, &self.seal));
    }
}

pub(crate) fn shared_alias_registry(context: usize) -> Arc<AliasAdmissionRegistry> {
    static REGISTRIES: OnceLock<Mutex<HashMap<usize, Weak<AliasAdmissionRegistry>>>> =
        OnceLock::new();
    let registries = REGISTRIES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut registries = registries.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(registry) = registries.get(&context).and_then(Weak::upgrade) {
        return registry;
    }

    let registry = Arc::new(AliasAdmissionRegistry::new(context));
    registries.insert(context, Arc::downgrade(&registry));
    registry
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
pub(crate) fn fresh_alias_registry(context: usize) -> Arc<AliasAdmissionRegistry> {
    Arc::new(AliasAdmissionRegistry::new(context))
}

pub(crate) fn admit_and_register<'allocation>(
    registry: &Arc<AliasAdmissionRegistry>,
    context: &ObservedContext,
    arguments: impl IntoIterator<Item = ArgumentAccess<'allocation>>,
) -> Result<
    (
        ArgumentAliasAdmission<'allocation>,
        InFlightRegionRegistration<'allocation>,
    ),
    AliasAdmissionError,
> {
    let admission = ArgumentAliasValidator::new().admit(context, arguments, &[])?;
    let registration = registry.register(context, &admission)?;
    Ok((admission, registration))
}

/// Stateless validator for one launch's argument regions.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArgumentAliasValidator;

impl ArgumentAliasValidator {
    pub const fn new() -> Self {
        Self
    }

    /// Checks aliases within `arguments` and against every supplied in-flight
    /// admission. Overlapping atomics fail closed because this validator has no
    /// participant-domain evidence with which to establish scope coverage. The
    /// result remains inert and is not consumed by any launch method.
    pub fn admit<'allocation>(
        &self,
        context: &ObservedContext,
        arguments: impl IntoIterator<Item = ArgumentAccess<'allocation>>,
        in_flight: &[&ArgumentAliasAdmission<'_>],
    ) -> Result<ArgumentAliasAdmission<'allocation>, AliasAdmissionError> {
        for (launch_index, admission) in in_flight.iter().enumerate() {
            if !context.same_context(&admission.context) {
                return Err(AliasAdmissionError::InFlightContextMismatch { launch_index });
            }
        }

        let mut accesses = Vec::new();
        for (argument_index, argument) in arguments.into_iter().enumerate() {
            let region = match argument.region {
                ArgumentRegion::Known(region) => region,
                ArgumentRegion::Unknown => {
                    return Err(AliasAdmissionError::UnknownProvenance { argument_index });
                }
            };
            if !context.same_context(&region.context) {
                return Err(AliasAdmissionError::ArgumentContextMismatch { argument_index });
            }

            let candidate = AdmittedAccess {
                region,
                mode: argument.mode,
            };
            for (earlier_argument, earlier) in accesses.iter().enumerate() {
                if accesses_conflict(&candidate, earlier) {
                    return Err(AliasAdmissionError::Conflict {
                        argument_index,
                        conflicting_with: ConflictSource::Argument { earlier_argument },
                    });
                }
            }
            for (launch_index, admission) in in_flight.iter().enumerate() {
                for (in_flight_argument, existing) in admission.accesses.iter().enumerate() {
                    if accesses_conflict(&candidate, existing) {
                        return Err(AliasAdmissionError::Conflict {
                            argument_index,
                            conflicting_with: ConflictSource::InFlight {
                                launch_index,
                                argument_index: in_flight_argument,
                            },
                        });
                    }
                }
            }
            accesses.push(candidate);
        }

        Ok(ArgumentAliasAdmission {
            context: context.clone(),
            accesses,
        })
    }
}

fn accesses_conflict(left: &AdmittedAccess<'_>, right: &AdmittedAccess<'_>) -> bool {
    descriptors_conflict(
        AccessDescriptor::from_admitted(left),
        AccessDescriptor::from_admitted(right),
    )
}

fn descriptors_conflict(left: AccessDescriptor, right: AccessDescriptor) -> bool {
    left.identity == right.identity
        && descriptors_overlap(left, right)
        && !matches!(
            (left.mode, right.mode),
            (
                ArgumentAccessMode::SharedRead,
                ArgumentAccessMode::SharedRead
            )
        )
}

fn descriptors_overlap(left: AccessDescriptor, right: AccessDescriptor) -> bool {
    !left.is_empty()
        && !right.is_empty()
        && left.byte_offset < right.byte_end
        && right.byte_offset < left.byte_end
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictSource {
    Argument {
        earlier_argument: usize,
    },
    InFlight {
        launch_index: usize,
        argument_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AliasAdmissionError {
    UnknownProvenance {
        argument_index: usize,
    },
    ArgumentContextMismatch {
        argument_index: usize,
    },
    InFlightContextMismatch {
        launch_index: usize,
    },
    Conflict {
        argument_index: usize,
        conflicting_with: ConflictSource,
    },
}

impl fmt::Display for AliasAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProvenance { argument_index } => write!(
                formatter,
                "argument {argument_index} has unknown allocation provenance"
            ),
            Self::ArgumentContextMismatch { argument_index } => write!(
                formatter,
                "argument {argument_index} belongs to a different context"
            ),
            Self::InFlightContextMismatch { launch_index } => write!(
                formatter,
                "in-flight launch {launch_index} belongs to a different context"
            ),
            Self::Conflict {
                argument_index,
                conflicting_with,
            } => write!(
                formatter,
                "argument {argument_index} conflicts with {conflicting_with:?}"
            ),
        }
    }
}

impl std::error::Error for AliasAdmissionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RegionError {
    WrongContext,
    ZeroSizedElement,
    EmptyRegion,
    AllocationSizeOverflow,
    AllocationAddressOverflow,
    RegionAddressOutsideAllocation {
        allocation_address: usize,
        region_address: usize,
    },
    OffsetLengthOverflow {
        byte_offset: usize,
        byte_length: usize,
    },
    OutOfBounds {
        allocation_length: usize,
        byte_offset: usize,
        byte_length: usize,
    },
}

impl fmt::Display for RegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongContext => formatter.write_str("allocation belongs to a different context"),
            Self::ZeroSizedElement => {
                formatter.write_str("generated device regions require non-zero-sized elements")
            }
            Self::EmptyRegion => {
                formatter.write_str("generated device regions must contain at least one element")
            }
            Self::AllocationSizeOverflow => {
                formatter.write_str("allocation element count overflows its byte length")
            }
            Self::AllocationAddressOverflow => {
                formatter.write_str("allocation address plus byte length overflows")
            }
            Self::RegionAddressOutsideAllocation {
                allocation_address,
                region_address,
            } => write!(
                formatter,
                "region address {region_address:#x} is before allocation address {allocation_address:#x}"
            ),
            Self::OffsetLengthOverflow {
                byte_offset,
                byte_length,
            } => write!(
                formatter,
                "byte region {byte_offset} + {byte_length} overflows"
            ),
            Self::OutOfBounds {
                allocation_length,
                byte_offset,
                byte_length,
            } => write!(
                formatter,
                "byte region {byte_offset}..+{byte_length} exceeds allocation length {allocation_length}"
            ),
        }
    }
}

impl std::error::Error for RegionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(identity: usize) -> ObservedContext {
        ObservedContext::for_test(identity, 0, "gfx942", 1_024, 65_536)
    }

    unsafe fn allocation<'a>(
        context: &ObservedContext,
        owner: &'a (),
        address: usize,
        byte_length: usize,
    ) -> AllocationProvenance<'a> {
        // SAFETY: tests use inert numeric addresses only for admission logic;
        // no returned value can access memory or authorize launch.
        unsafe {
            AllocationProvenance::from_raw_parts(context, owner, address as *mut u8, byte_length)
                .unwrap()
        }
    }

    fn read(region: CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(region, ArgumentAccessMode::SharedRead)
    }

    fn write(region: CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(region, ArgumentAccessMode::ExclusiveWrite)
    }

    fn read_write(region: CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        ArgumentAccess::new(region, ArgumentAccessMode::ExclusiveReadWrite)
    }

    fn selected_region<'allocation>(
        context: &ObservedContext,
        allocation: usize,
        checked: CheckedDeviceRegion,
    ) -> CheckedByteRegion<'allocation> {
        CheckedByteRegion {
            identity: AllocationIdentity {
                context: context.context_key(),
                allocation,
            },
            context: context.clone(),
            byte_offset: checked.byte_offset,
            byte_length: checked.byte_length,
            byte_end: checked.byte_offset + checked.byte_length,
            marker: PhantomData,
        }
    }

    fn atomic(region: CheckedByteRegion<'_>) -> ArgumentAccess<'_> {
        atomic_at_scope(region, AtomicScope::Device)
    }

    fn atomic_at_scope(region: CheckedByteRegion<'_>, scope: AtomicScope) -> ArgumentAccess<'_> {
        ArgumentAccess::new(
            region,
            ArgumentAccessMode::Atomic(
                AtomicAccess::new(
                    AtomicOperation::ReadModifyWrite,
                    AtomicOrdering::AcquireRelease,
                    scope,
                )
                .unwrap(),
            ),
        )
    }

    #[test]
    fn region_checks_overflow_bounds_and_end_boundary() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 32) };

        let whole = allocation.region(0, 32).unwrap();
        assert_eq!(whole.byte_end(), 32);
        assert_eq!(allocation.region(32, 0).unwrap().byte_end(), 32);
        assert_eq!(
            allocation.region(33, 0).unwrap_err(),
            RegionError::OutOfBounds {
                allocation_length: 32,
                byte_offset: 33,
                byte_length: 0,
            }
        );
        assert_eq!(
            allocation.region(usize::MAX, 2).unwrap_err(),
            RegionError::OffsetLengthOverflow {
                byte_offset: usize::MAX,
                byte_length: 2,
            }
        );
    }

    #[test]
    fn raw_allocation_extent_rejects_address_overflow() {
        let context = context(1);
        let owner = ();
        // SAFETY: deliberately malformed inert declaration tests validation.
        let error = unsafe {
            AllocationProvenance::from_raw_parts(&context, &owner, usize::MAX as *mut u8, 2)
        }
        .unwrap_err();
        assert_eq!(error, RegionError::AllocationAddressOverflow);
    }

    #[test]
    fn generated_region_geometry_keeps_allocation_relative_offsets() {
        let selected = checked_device_region::<u32>(0x1000, 12, 0x100c, 4).unwrap();
        assert_eq!(
            selected,
            CheckedDeviceRegion {
                byte_offset: 12,
                byte_length: 16,
            }
        );

        assert_eq!(
            checked_device_region::<u32>(0x1000, 12, 0x0ffc, 4).unwrap_err(),
            RegionError::RegionAddressOutsideAllocation {
                allocation_address: 0x1000,
                region_address: 0x0ffc,
            }
        );
        assert_eq!(
            checked_device_region::<u32>(0x1000, 12, 0x1028, 4).unwrap_err(),
            RegionError::OutOfBounds {
                allocation_length: 48,
                byte_offset: 40,
                byte_length: 16,
            }
        );
    }

    #[test]
    fn generated_regions_reject_empty_and_zero_sized_elements() {
        assert_eq!(
            checked_device_region::<u32>(0x1000, 4, 0x1000, 0).unwrap_err(),
            RegionError::EmptyRegion
        );
        assert_eq!(
            checked_device_region::<[u8; 0]>(0, usize::MAX, 0, 1).unwrap_err(),
            RegionError::ZeroSizedElement
        );
    }

    #[test]
    fn generic_generated_regions_admit_checked_empty_boundaries() {
        assert_eq!(
            checked_device_region_allow_empty::<u32>(0x1000, 4, 0x1010, 0).unwrap(),
            CheckedDeviceRegion {
                byte_offset: 16,
                byte_length: 0,
            }
        );
        assert_eq!(
            checked_device_region_allow_empty::<u32>(0x1000, 4, 0x1011, 0).unwrap_err(),
            RegionError::OutOfBounds {
                allocation_length: 16,
                byte_offset: 17,
                byte_length: 0,
            }
        );
    }

    #[test]
    fn generated_mutable_subregions_preserve_canaries_and_alias_identity() {
        let context = context(1);
        let first = checked_device_region::<u32>(0x1000, 12, 0x1004, 4).unwrap();
        let second = checked_device_region::<u32>(0x1000, 12, 0x1014, 4).unwrap();
        let first = selected_region(&context, 0x1000, first);
        let second = selected_region(&context, 0x1000, second);

        assert_eq!(first.allocation(), second.allocation());
        assert_eq!((first.byte_offset(), first.byte_end()), (4, 20));
        assert_eq!((second.byte_offset(), second.byte_end()), (20, 36));
        ArgumentAliasValidator::new()
            .admit(&context, [write(first), write(second)], &[])
            .unwrap();

        let first = selected_region(
            &context,
            0x1000,
            checked_device_region::<u32>(0x1000, 12, 0x1004, 4).unwrap(),
        );
        let overlapping = selected_region(
            &context,
            0x1000,
            checked_device_region::<u32>(0x1000, 12, 0x1010, 4).unwrap(),
        );
        assert!(matches!(
            ArgumentAliasValidator::new().admit(&context, [write(first), write(overlapping)], &[],),
            Err(AliasAdmissionError::Conflict { .. })
        ));
    }

    #[test]
    fn partial_write_overlap_conflicts_but_touching_boundaries_do_not() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 64) };
        let validator = ArgumentAliasValidator::new();

        let error = validator
            .admit(
                &context,
                [
                    write(allocation.region(0, 16).unwrap()),
                    read(allocation.region(8, 16).unwrap()),
                ],
                &[],
            )
            .unwrap_err();
        assert_eq!(
            error,
            AliasAdmissionError::Conflict {
                argument_index: 1,
                conflicting_with: ConflictSource::Argument {
                    earlier_argument: 0,
                },
            }
        );

        validator
            .admit(
                &context,
                [
                    write(allocation.region(0, 16).unwrap()),
                    write(allocation.region(16, 16).unwrap()),
                ],
                &[],
            )
            .unwrap();
    }

    #[test]
    fn shared_reads_may_overlap_and_zero_length_never_conflicts() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 32) };
        let validator = ArgumentAliasValidator::new();

        validator
            .admit(
                &context,
                [
                    read(allocation.region(0, 24).unwrap()),
                    read(allocation.region(8, 24).unwrap()),
                    write(allocation.region(4, 0).unwrap()),
                ],
                &[],
            )
            .unwrap();
    }

    #[test]
    fn read_write_is_exclusive_against_reads_writes_and_other_read_write_accesses() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 32) };
        let validator = ArgumentAliasValidator::new();

        for second in [
            read(allocation.region(0, 32).unwrap()),
            write(allocation.region(0, 32).unwrap()),
            read_write(allocation.region(0, 32).unwrap()),
        ] {
            assert!(matches!(
                validator.admit(
                    &context,
                    [read_write(allocation.region(0, 32).unwrap()), second],
                    &[],
                ),
                Err(AliasAdmissionError::Conflict { .. })
            ));
        }

        validator
            .admit(
                &context,
                [
                    read_write(allocation.region(0, 16).unwrap()),
                    read_write(allocation.region(16, 16).unwrap()),
                ],
                &[],
            )
            .unwrap();
    }

    #[test]
    fn distinct_allocations_accept_overlapping_offsets() {
        let context = context(1);
        let first_owner = ();
        let second_owner = ();
        // SAFETY: see `allocation`; the addresses model distinct allocations.
        let first = unsafe { allocation(&context, &first_owner, 0x1000, 32) };
        // SAFETY: see above.
        let second = unsafe { allocation(&context, &second_owner, 0x2000, 32) };

        ArgumentAliasValidator::new()
            .admit(
                &context,
                [
                    write(first.region(0, 32).unwrap()),
                    write(second.region(0, 32).unwrap()),
                ],
                &[],
            )
            .unwrap();
    }

    #[test]
    fn repeated_provenance_for_same_address_has_one_symbolic_identity() {
        let context = context(1);
        let first_owner = ();
        let second_owner = ();
        // SAFETY: this test checks deterministic identity issuance for an
        // address the caller claims is the same allocation.
        let first = unsafe { allocation(&context, &first_owner, 0x1000, 32) };
        // SAFETY: see above.
        let second = unsafe { allocation(&context, &second_owner, 0x1000, 32) };

        assert_eq!(first.identity(), second.identity());
        assert!(matches!(
            ArgumentAliasValidator::new().admit(
                &context,
                [
                    write(first.region(0, 32).unwrap()),
                    write(second.region(0, 32).unwrap()),
                ],
                &[],
            ),
            Err(AliasAdmissionError::Conflict { .. })
        ));
    }

    #[test]
    fn conflicts_with_in_flight_launch_regions() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 64) };
        let validator = ArgumentAliasValidator::new();
        let in_flight = validator
            .admit(&context, [write(allocation.region(16, 16).unwrap())], &[])
            .unwrap();

        assert_eq!(
            validator
                .admit(
                    &context,
                    [read(allocation.region(8, 16).unwrap())],
                    &[&in_flight],
                )
                .unwrap_err(),
            AliasAdmissionError::Conflict {
                argument_index: 0,
                conflicting_with: ConflictSource::InFlight {
                    launch_index: 0,
                    argument_index: 0,
                },
            }
        );
    }

    #[test]
    fn atomic_and_non_atomic_overlap_conflicts() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 8) };

        assert!(matches!(
            ArgumentAliasValidator::new().admit(
                &context,
                [
                    atomic(allocation.region(0, 4).unwrap()),
                    read(allocation.region(0, 4).unwrap()),
                ],
                &[],
            ),
            Err(AliasAdmissionError::Conflict { .. })
        ));
    }

    #[test]
    fn identical_workgroup_atomic_overlap_fails_closed_within_and_across_launches() {
        let context = context(1);
        let owner = ();
        // SAFETY: see `allocation`.
        let allocation = unsafe { allocation(&context, &owner, 0x1000, 8) };
        let region = || allocation.region(0, 4).unwrap();
        let workgroup_atomic = || atomic_at_scope(region(), AtomicScope::Workgroup);
        let validator = ArgumentAliasValidator::new();

        assert_eq!(
            validator
                .admit(&context, [workgroup_atomic(), workgroup_atomic()], &[],)
                .unwrap_err(),
            AliasAdmissionError::Conflict {
                argument_index: 1,
                conflicting_with: ConflictSource::Argument {
                    earlier_argument: 0,
                },
            }
        );

        let in_flight = validator
            .admit(&context, [workgroup_atomic()], &[])
            .unwrap();
        assert_eq!(
            validator
                .admit(&context, [workgroup_atomic()], &[&in_flight])
                .unwrap_err(),
            AliasAdmissionError::Conflict {
                argument_index: 0,
                conflicting_with: ConflictSource::InFlight {
                    launch_index: 0,
                    argument_index: 0,
                },
            }
        );
    }

    #[test]
    fn invalid_atomic_orderings_are_rejected() {
        assert_eq!(
            AtomicAccess::new(
                AtomicOperation::Load,
                AtomicOrdering::Release,
                AtomicScope::Device,
            ),
            Err(InvalidAtomicOrdering {
                operation: AtomicOperation::Load,
                ordering: AtomicOrdering::Release,
            })
        );
        assert!(
            AtomicAccess::new(
                AtomicOperation::Store,
                AtomicOrdering::Acquire,
                AtomicScope::Device,
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_provenance_fails_closed() {
        let context = context(1);
        assert_eq!(
            ArgumentAliasValidator::new()
                .admit(
                    &context,
                    [ArgumentAccess::unknown(ArgumentAccessMode::SharedRead)],
                    &[],
                )
                .unwrap_err(),
            AliasAdmissionError::UnknownProvenance { argument_index: 0 }
        );
    }

    #[test]
    fn argument_and_in_flight_context_mismatches_are_rejected() {
        let first_context = context(1);
        let second_context = context(2);
        let first_owner = ();
        let second_owner = ();
        // SAFETY: see `allocation`.
        let first = unsafe { allocation(&first_context, &first_owner, 0x1000, 8) };
        // SAFETY: see above.
        let second = unsafe { allocation(&second_context, &second_owner, 0x2000, 8) };
        let validator = ArgumentAliasValidator::new();

        assert_eq!(
            validator
                .admit(&first_context, [read(second.region(0, 8).unwrap())], &[],)
                .unwrap_err(),
            AliasAdmissionError::ArgumentContextMismatch { argument_index: 0 }
        );
        assert_eq!(
            validator
                .admit(
                    &first_context,
                    [read_write(second.region(0, 8).unwrap())],
                    &[],
                )
                .unwrap_err(),
            AliasAdmissionError::ArgumentContextMismatch { argument_index: 0 }
        );

        let in_flight = validator
            .admit(&first_context, [read(first.region(0, 8).unwrap())], &[])
            .unwrap();
        assert_eq!(
            validator
                .admit(&second_context, std::iter::empty(), &[&in_flight])
                .unwrap_err(),
            AliasAdmissionError::InFlightContextMismatch { launch_index: 0 }
        );
    }
}
