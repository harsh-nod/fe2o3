use crate::{
    AllocationIdentity, AllocationKind, Error, GpuContext, MemoryTopologyObservation,
    PhysicalDeviceIdentity, check,
};
use core::ffi::c_void;
use core::fmt;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Exact native access established for one physical device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmmAccess {
    Read,
    ReadWrite,
}

impl VmmAccess {
    const fn native(self) -> u32 {
        match self {
            Self::Read => fe2o3_hip_sys::HIP_VMM_ACCESS_READ,
            Self::ReadWrite => fe2o3_hip_sys::HIP_VMM_ACCESS_READ_WRITE,
        }
    }
}

/// Rounded allocation layout accepted by the native VMM granularity contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmmLayout {
    requested_byte_len: usize,
    byte_len: usize,
    granularity: usize,
}

impl VmmLayout {
    pub const fn requested_byte_len(self) -> usize {
        self.requested_byte_len
    }

    pub const fn byte_len(self) -> usize {
        self.byte_len
    }

    pub const fn granularity(self) -> usize {
        self.granularity
    }
}

trait VmmBackend: Send + Sync {
    fn bind(&self, context: &GpuContext) -> Result<(), Error>;
    fn granularity(&self, device_id: i32) -> Result<usize, Error>;
    fn reserve(&self, size: usize, alignment: usize) -> Result<*mut c_void, Error>;
    fn address_free(&self, pointer: *mut c_void, size: usize) -> Result<(), Error>;
    fn create(&self, size: usize, device_id: i32) -> Result<usize, Error>;
    fn map(&self, pointer: *mut c_void, size: usize, handle: usize) -> Result<(), Error>;
    fn set_access(
        &self,
        pointer: *mut c_void,
        size: usize,
        device_id: i32,
        access: VmmAccess,
    ) -> Result<(), Error>;
    fn get_access(&self, pointer: *mut c_void, device_id: i32) -> Result<u32, Error>;
    fn unmap(&self, pointer: *mut c_void, size: usize) -> Result<(), Error>;
    fn release(&self, handle: usize) -> Result<(), Error>;
}

struct HipVmmBackend;

impl VmmBackend for HipVmmBackend {
    fn bind(&self, context: &GpuContext) -> Result<(), Error> {
        context.bind_to_thread()
    }

    fn granularity(&self, device_id: i32) -> Result<usize, Error> {
        let mut granularity = 0;
        check(unsafe {
            fe2o3_hip_sys::fe2o3HipMemGetAllocationGranularity(&mut granularity, device_id)
        })?;
        Ok(granularity)
    }

    fn reserve(&self, size: usize, alignment: usize) -> Result<*mut c_void, Error> {
        let mut pointer = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemAddressReserve(&mut pointer, size, alignment) })?;
        Ok(pointer)
    }

    fn address_free(&self, pointer: *mut c_void, size: usize) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemAddressFree(pointer, size) })
    }

    fn create(&self, size: usize, device_id: i32) -> Result<usize, Error> {
        let mut handle = 0;
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemCreate(&mut handle, size, device_id) })?;
        Ok(handle)
    }

    fn map(&self, pointer: *mut c_void, size: usize, handle: usize) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemMap(pointer, size, handle) })
    }

    fn set_access(
        &self,
        pointer: *mut c_void,
        size: usize,
        device_id: i32,
        access: VmmAccess,
    ) -> Result<(), Error> {
        check(unsafe {
            fe2o3_hip_sys::fe2o3HipMemSetAccess(pointer, size, device_id, access.native())
        })
    }

    fn get_access(&self, pointer: *mut c_void, device_id: i32) -> Result<u32, Error> {
        let mut access = 0;
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemGetAccess(&mut access, pointer, device_id) })?;
        Ok(access)
    }

    fn unmap(&self, pointer: *mut c_void, size: usize) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemUnmap(pointer, size) })
    }

    fn release(&self, handle: usize) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemRelease(handle) })
    }
}

struct VmmResources {
    pointer: *mut c_void,
    handle: usize,
    layout: VmmLayout,
    reservation: AllocationIdentity,
    physical: AllocationIdentity,
    topology: MemoryTopologyObservation,
    context: Arc<GpuContext>,
    backend: Arc<dyn VmmBackend>,
    active: bool,
}

impl VmmResources {
    fn abandon(&mut self) {
        self.active = false;
        core::mem::forget(self.context.clone());
    }

    fn successor(&mut self) -> Self {
        self.active = false;
        Self {
            pointer: self.pointer,
            handle: self.handle,
            layout: self.layout,
            reservation: self.reservation,
            physical: self.physical,
            topology: self.topology,
            context: self.context.clone(),
            backend: self.backend.clone(),
            active: true,
        }
    }

    fn cleanup_unmapped(&mut self) -> Result<VmmReclamationReceipt, VmmCleanupError> {
        debug_assert!(self.active);
        self.active = false;
        if let Err(error) = self.backend.bind(&self.context) {
            self.retain_context();
            return Err(self.cleanup_error(VmmCleanupStage::Bind, error));
        }
        let release = self.backend.release(self.handle).err();
        let address_free = self
            .backend
            .address_free(self.pointer, self.layout.byte_len)
            .err();
        if release.is_some() || address_free.is_some() {
            self.retain_context();
            return Err(VmmCleanupError {
                resources: Box::new(VmmCleanupResources {
                    reservation: self.reservation,
                    physical: self.physical,
                }),
                stage: VmmCleanupStage::UnmappedResources,
                operation: release.map(Box::new),
                secondary: address_free.map(Box::new),
            });
        }
        Ok(self.receipt())
    }

    fn cleanup_mapped(&mut self) -> Result<VmmReclamationReceipt, VmmCleanupError> {
        debug_assert!(self.active);
        self.active = false;
        if let Err(error) = self.backend.bind(&self.context) {
            self.retain_context();
            return Err(self.cleanup_error(VmmCleanupStage::Bind, error));
        }
        if let Err(error) = self.backend.unmap(self.pointer, self.layout.byte_len) {
            self.retain_context();
            return Err(self.cleanup_error(VmmCleanupStage::Unmap, error));
        }
        if let Err(error) = self.backend.release(self.handle) {
            self.retain_context();
            return Err(self.cleanup_error(VmmCleanupStage::Release, error));
        }
        if let Err(error) = self
            .backend
            .address_free(self.pointer, self.layout.byte_len)
        {
            self.retain_context();
            return Err(self.cleanup_error(VmmCleanupStage::AddressFree, error));
        }
        Ok(self.receipt())
    }

    fn cleanup_error(&self, stage: VmmCleanupStage, operation: Error) -> VmmCleanupError {
        VmmCleanupError {
            resources: Box::new(VmmCleanupResources {
                reservation: self.reservation,
                physical: self.physical,
            }),
            stage,
            operation: Some(Box::new(operation)),
            secondary: None,
        }
    }

    fn retain_context(&self) {
        core::mem::forget(self.context.clone());
    }

    fn receipt(&self) -> VmmReclamationReceipt {
        VmmReclamationReceipt {
            reservation: self.reservation,
            physical: self.physical,
        }
    }
}

/// Reserved virtual address and physical allocation before mapping.
///
/// This owner is linear and exposes no native address. Dropping it attempts to
/// release both resources exactly once.
#[must_use = "dropping an unmapped VMM allocation attempts native reclamation"]
pub struct VmmUnmappedAllocation {
    resources: VmmResources,
}

impl fmt::Debug for VmmUnmappedAllocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VmmUnmappedAllocation")
            .field("layout", &self.resources.layout)
            .field("reservation", &self.resources.reservation)
            .field("physical", &self.resources.physical)
            .finish_non_exhaustive()
    }
}

impl VmmUnmappedAllocation {
    pub fn create(
        context: &Arc<GpuContext>,
        topology: MemoryTopologyObservation,
        requested_byte_len: usize,
    ) -> Result<Self, VmmError> {
        Self::create_with_backend(
            context,
            topology,
            requested_byte_len,
            Arc::new(HipVmmBackend),
        )
    }

    fn create_with_backend(
        context: &Arc<GpuContext>,
        topology: MemoryTopologyObservation,
        requested_byte_len: usize,
        backend: Arc<dyn VmmBackend>,
    ) -> Result<Self, VmmError> {
        if !topology.is_for_context(context) {
            return Err(VmmError::ForeignTopologyObservation);
        }
        if !topology.capabilities().virtual_memory_management() {
            return Err(VmmError::Unsupported);
        }
        if requested_byte_len == 0 {
            return Err(VmmError::EmptyAllocation);
        }
        backend.bind(context).map_err(VmmError::Hip)?;
        let granularity = backend
            .granularity(topology.physical_device().ordinal())
            .map_err(VmmError::Hip)?;
        if granularity == 0 {
            return Err(VmmError::InvalidGranularity);
        }
        let byte_len = requested_byte_len
            .checked_add(granularity - 1)
            .ok_or(VmmError::SizeOverflow)?
            / granularity
            * granularity;
        let layout = VmmLayout {
            requested_byte_len,
            byte_len,
            granularity,
        };
        let pointer = backend
            .reserve(byte_len, granularity)
            .map_err(VmmError::Hip)?;
        if pointer.is_null() {
            return Err(VmmError::NullReservation);
        }
        let handle = match backend.create(byte_len, topology.physical_device().ordinal()) {
            Ok(handle) => handle,
            Err(operation) => {
                return match backend.address_free(pointer, byte_len) {
                    Ok(()) => Err(VmmError::Hip(operation)),
                    Err(cleanup) => Err(VmmError::ConstructionRecoveryFailed {
                        operation: Box::new(operation),
                        cleanup: Box::new(cleanup),
                    }),
                };
            }
        };
        if handle == 0 {
            return match backend.address_free(pointer, byte_len) {
                Ok(()) => Err(VmmError::NullPhysicalHandle),
                Err(cleanup) => Err(VmmError::NullHandleRecoveryFailed(Box::new(cleanup))),
            };
        }
        Ok(Self {
            resources: VmmResources {
                pointer,
                handle,
                layout,
                reservation: topology
                    .new_allocation_identity(AllocationKind::VmmVirtualRange, byte_len),
                physical: topology.new_allocation_identity(AllocationKind::VmmPhysical, byte_len),
                topology,
                context: context.clone(),
                backend,
                active: true,
            },
        })
    }

    pub const fn layout(&self) -> VmmLayout {
        self.resources.layout
    }

    pub const fn reservation_identity(&self) -> AllocationIdentity {
        self.resources.reservation
    }

    pub const fn physical_identity(&self) -> AllocationIdentity {
        self.resources.physical
    }

    pub fn map(mut self) -> Result<VmmMappedAllocation, VmmError> {
        self.resources
            .backend
            .bind(&self.resources.context)
            .map_err(VmmError::Hip)?;
        if let Err(operation) = self.resources.backend.map(
            self.resources.pointer,
            self.resources.layout.byte_len,
            self.resources.handle,
        ) {
            let reservation = self.resources.reservation;
            let physical = self.resources.physical;
            self.resources.abandon();
            return Err(VmmError::MapAmbiguous(Box::new(VmmMapAmbiguity {
                reservation,
                physical,
                operation,
            })));
        }
        Ok(VmmMappedAllocation {
            resources: self.resources.successor(),
        })
    }

    pub fn reclaim(mut self) -> Result<VmmReclamationReceipt, VmmCleanupError> {
        self.resources.cleanup_unmapped()
    }
}

impl Drop for VmmUnmappedAllocation {
    fn drop(&mut self) {
        if self.resources.active {
            let _ = self.resources.cleanup_unmapped();
        }
    }
}

/// Successfully mapped VMM allocation without verified device access.
#[must_use = "dropping a mapped VMM allocation attempts native reclamation"]
pub struct VmmMappedAllocation {
    resources: VmmResources,
}

impl fmt::Debug for VmmMappedAllocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VmmMappedAllocation")
            .field("layout", &self.resources.layout)
            .field("reservation", &self.resources.reservation)
            .field("physical", &self.resources.physical)
            .finish_non_exhaustive()
    }
}

impl VmmMappedAllocation {
    pub const fn layout(&self) -> VmmLayout {
        self.resources.layout
    }

    pub fn grant_access(
        mut self,
        destination: MemoryTopologyObservation,
        access: VmmAccess,
    ) -> Result<VmmAccessibleAllocation, VmmError> {
        establish_access(&self.resources, destination, access)?;
        let mut accesses = BTreeMap::new();
        accesses.insert(destination.physical_device(), access);
        Ok(VmmAccessibleAllocation {
            resources: self.resources.successor(),
            accesses,
        })
    }

    pub fn reclaim(mut self) -> Result<VmmReclamationReceipt, VmmCleanupError> {
        self.resources.cleanup_mapped()
    }
}

impl Drop for VmmMappedAllocation {
    fn drop(&mut self) {
        if self.resources.active {
            let _ = self.resources.cleanup_mapped();
        }
    }
}

/// Mapped VMM allocation with at least one verified device access grant.
#[must_use = "dropping an accessible VMM allocation attempts native reclamation"]
pub struct VmmAccessibleAllocation {
    resources: VmmResources,
    accesses: BTreeMap<PhysicalDeviceIdentity, VmmAccess>,
}

impl fmt::Debug for VmmAccessibleAllocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VmmAccessibleAllocation")
            .field("layout", &self.resources.layout)
            .field("reservation", &self.resources.reservation)
            .field("physical", &self.resources.physical)
            .field("accesses", &self.accesses)
            .finish_non_exhaustive()
    }
}

impl VmmAccessibleAllocation {
    pub const fn layout(&self) -> VmmLayout {
        self.resources.layout
    }

    pub const fn reservation_identity(&self) -> AllocationIdentity {
        self.resources.reservation
    }

    pub const fn physical_identity(&self) -> AllocationIdentity {
        self.resources.physical
    }

    pub fn access_for(&self, device: PhysicalDeviceIdentity) -> Option<VmmAccess> {
        self.accesses.get(&device).copied()
    }

    pub fn grant_access(
        &mut self,
        destination: MemoryTopologyObservation,
        access: VmmAccess,
    ) -> Result<VmmAccessReceipt, VmmError> {
        establish_access(&self.resources, destination, access)?;
        self.accesses.insert(destination.physical_device(), access);
        Ok(VmmAccessReceipt {
            reservation: self.resources.reservation,
            destination: destination.physical_device(),
            access,
        })
    }

    pub fn query_access(
        &self,
        destination: MemoryTopologyObservation,
    ) -> Result<VmmAccessReceipt, VmmError> {
        let expected = self
            .accesses
            .get(&destination.physical_device())
            .copied()
            .ok_or(VmmError::AccessNotGranted {
                device: destination.physical_device(),
            })?;
        verify_access(&self.resources, destination, expected)?;
        Ok(VmmAccessReceipt {
            reservation: self.resources.reservation,
            destination: destination.physical_device(),
            access: expected,
        })
    }

    /// Returns the mapped address without creating dereference or launch authority.
    ///
    /// # Safety
    ///
    /// The caller must retain this owner, use only a device with a matching
    /// access receipt, enforce bounds and aliasing, establish synchronization,
    /// and never use the address after reclamation.
    pub unsafe fn raw_pointer(&self) -> *mut c_void {
        self.resources.pointer
    }

    pub fn reclaim(mut self) -> Result<VmmReclamationReceipt, VmmCleanupError> {
        self.resources.cleanup_mapped()
    }
}

impl Drop for VmmAccessibleAllocation {
    fn drop(&mut self) {
        if self.resources.active {
            let _ = self.resources.cleanup_mapped();
        }
    }
}

fn establish_access(
    resources: &VmmResources,
    destination: MemoryTopologyObservation,
    access: VmmAccess,
) -> Result<(), VmmError> {
    if !destination.capabilities().virtual_memory_management() {
        return Err(VmmError::DestinationUnsupported {
            device: destination.physical_device(),
        });
    }
    resources
        .backend
        .bind(&resources.context)
        .map_err(VmmError::Hip)?;
    resources
        .backend
        .set_access(
            resources.pointer,
            resources.layout.byte_len,
            destination.physical_device().ordinal(),
            access,
        )
        .map_err(VmmError::Hip)?;
    verify_access(resources, destination, access)
}

fn verify_access(
    resources: &VmmResources,
    destination: MemoryTopologyObservation,
    expected: VmmAccess,
) -> Result<(), VmmError> {
    resources
        .backend
        .bind(&resources.context)
        .map_err(VmmError::Hip)?;
    let observed = resources
        .backend
        .get_access(resources.pointer, destination.physical_device().ordinal())
        .map_err(VmmError::Hip)?;
    if observed != expected.native() {
        return Err(VmmError::UnexpectedAccess {
            device: destination.physical_device(),
            expected,
            observed,
        });
    }
    Ok(())
}

/// Linear evidence that exact access was observed for an exact mapping/device.
#[derive(Debug)]
pub struct VmmAccessReceipt {
    reservation: AllocationIdentity,
    destination: PhysicalDeviceIdentity,
    access: VmmAccess,
}

impl VmmAccessReceipt {
    pub const fn reservation(&self) -> AllocationIdentity {
        self.reservation
    }

    pub const fn destination(&self) -> PhysicalDeviceIdentity {
        self.destination
    }

    pub const fn access(&self) -> VmmAccess {
        self.access
    }
}

/// Linear evidence that a VMM mapping, handle, and reservation were reclaimed.
#[derive(Debug)]
pub struct VmmReclamationReceipt {
    reservation: AllocationIdentity,
    physical: AllocationIdentity,
}

impl VmmReclamationReceipt {
    pub const fn reservation(&self) -> AllocationIdentity {
        self.reservation
    }

    pub const fn physical(&self) -> AllocationIdentity {
        self.physical
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum VmmError {
    ForeignTopologyObservation,
    Unsupported,
    DestinationUnsupported {
        device: PhysicalDeviceIdentity,
    },
    EmptyAllocation,
    InvalidGranularity,
    SizeOverflow,
    NullReservation,
    NullPhysicalHandle,
    NullHandleRecoveryFailed(Box<Error>),
    ConstructionRecoveryFailed {
        operation: Box<Error>,
        cleanup: Box<Error>,
    },
    MapAmbiguous(Box<VmmMapAmbiguity>),
    AccessNotGranted {
        device: PhysicalDeviceIdentity,
    },
    UnexpectedAccess {
        device: PhysicalDeviceIdentity,
        expected: VmmAccess,
        observed: u32,
    },
    Hip(Error),
}

impl fmt::Display for VmmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignTopologyObservation => {
                write!(
                    formatter,
                    "memory topology observation belongs to another context"
                )
            }
            Self::Unsupported => write!(formatter, "virtual memory management is not supported"),
            Self::DestinationUnsupported { device } => write!(
                formatter,
                "VMM access destination {} does not support virtual memory management",
                device.ordinal()
            ),
            Self::EmptyAllocation => write!(formatter, "VMM allocation must be non-empty"),
            Self::InvalidGranularity => write!(formatter, "HIP returned zero VMM granularity"),
            Self::SizeOverflow => write!(formatter, "VMM allocation size rounding overflowed"),
            Self::NullReservation => write!(formatter, "HIP returned a null VMM reservation"),
            Self::NullPhysicalHandle => write!(formatter, "HIP returned a null VMM handle"),
            Self::NullHandleRecoveryFailed(error) => write!(
                formatter,
                "HIP returned a null VMM handle and reservation cleanup failed: {error}"
            ),
            Self::ConstructionRecoveryFailed { operation, cleanup } => write!(
                formatter,
                "VMM physical allocation failed ({operation}) and reservation cleanup failed ({cleanup})"
            ),
            Self::MapAmbiguous(ambiguity) => write!(
                formatter,
                "VMM map outcome is ambiguous; native resources were retained: {}",
                ambiguity.operation
            ),
            Self::AccessNotGranted { device } => write!(
                formatter,
                "VMM mapping has no retained access grant for device {}",
                device.ordinal()
            ),
            Self::UnexpectedAccess {
                device,
                expected,
                observed,
            } => write!(
                formatter,
                "HIP reported VMM access {observed} for device {}, expected {expected:?}",
                device.ordinal()
            ),
            Self::Hip(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for VmmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NullHandleRecoveryFailed(error) => Some(error),
            Self::ConstructionRecoveryFailed { operation, .. } => Some(operation),
            Self::MapAmbiguous(ambiguity) => Some(&ambiguity.operation),
            Self::Hip(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct VmmMapAmbiguity {
    reservation: AllocationIdentity,
    physical: AllocationIdentity,
    operation: Error,
}

impl VmmMapAmbiguity {
    pub const fn reservation(&self) -> AllocationIdentity {
        self.reservation
    }

    pub const fn physical(&self) -> AllocationIdentity {
        self.physical
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmmCleanupStage {
    Bind,
    Unmap,
    Release,
    AddressFree,
    UnmappedResources,
}

#[derive(Debug)]
pub struct VmmCleanupError {
    resources: Box<VmmCleanupResources>,
    stage: VmmCleanupStage,
    operation: Option<Box<Error>>,
    secondary: Option<Box<Error>>,
}

impl VmmCleanupError {
    pub fn reservation(&self) -> AllocationIdentity {
        self.resources.reservation
    }

    pub fn physical(&self) -> AllocationIdentity {
        self.resources.physical
    }

    pub const fn stage(&self) -> VmmCleanupStage {
        self.stage
    }
}

#[derive(Debug)]
struct VmmCleanupResources {
    reservation: AllocationIdentity,
    physical: AllocationIdentity,
}

impl fmt::Display for VmmCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "VMM cleanup stopped at {:?}; native ownership is ambiguous",
            self.stage
        )?;
        if let Some(operation) = &self.operation {
            write!(formatter, ": {operation}")?;
        }
        if let Some(secondary) = &self.secondary {
            write!(formatter, "; secondary cleanup failed: {secondary}")?;
        }
        Ok(())
    }
}

impl std::error::Error for VmmCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.operation
            .as_deref()
            .or(self.secondary.as_deref())
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HipError;
    use std::sync::Mutex;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Bind,
        Granularity,
        Reserve(usize, usize),
        Create(usize, i32),
        Map,
        SetAccess(i32, VmmAccess),
        GetAccess(i32),
        Unmap,
        Release,
        AddressFree,
    }

    struct MockBackend {
        calls: Mutex<Vec<Call>>,
        granularity: usize,
        access: Mutex<u32>,
        fail: Mutex<Option<Call>>,
    }

    impl MockBackend {
        fn new(granularity: usize) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                granularity,
                access: Mutex::new(0),
                fail: Mutex::new(None),
            }
        }

        fn error() -> Error {
            HipError::new(999).into()
        }

        fn record(&self, call: Call) -> Result<(), Error> {
            self.calls.lock().unwrap().push(call.clone());
            if self.fail.lock().unwrap().as_ref() == Some(&call) {
                Err(Self::error())
            } else {
                Ok(())
            }
        }
    }

    impl VmmBackend for MockBackend {
        fn bind(&self, _: &GpuContext) -> Result<(), Error> {
            self.record(Call::Bind)
        }

        fn granularity(&self, _: i32) -> Result<usize, Error> {
            self.record(Call::Granularity)?;
            Ok(self.granularity)
        }

        fn reserve(&self, size: usize, alignment: usize) -> Result<*mut c_void, Error> {
            self.record(Call::Reserve(size, alignment))?;
            Ok(0x10000_usize as *mut c_void)
        }

        fn address_free(&self, _: *mut c_void, _: usize) -> Result<(), Error> {
            self.record(Call::AddressFree)
        }

        fn create(&self, size: usize, device_id: i32) -> Result<usize, Error> {
            self.record(Call::Create(size, device_id))?;
            Ok(0x20000)
        }

        fn map(&self, _: *mut c_void, _: usize, _: usize) -> Result<(), Error> {
            self.record(Call::Map)
        }

        fn set_access(
            &self,
            _: *mut c_void,
            _: usize,
            device_id: i32,
            access: VmmAccess,
        ) -> Result<(), Error> {
            self.record(Call::SetAccess(device_id, access))?;
            *self.access.lock().unwrap() = access.native();
            Ok(())
        }

        fn get_access(&self, _: *mut c_void, device_id: i32) -> Result<u32, Error> {
            self.record(Call::GetAccess(device_id))?;
            Ok(*self.access.lock().unwrap())
        }

        fn unmap(&self, _: *mut c_void, _: usize) -> Result<(), Error> {
            self.record(Call::Unmap)
        }

        fn release(&self, _: usize) -> Result<(), Error> {
            self.record(Call::Release)
        }
    }

    fn setup(backend: Arc<MockBackend>) -> (Arc<GpuContext>, MemoryTopologyObservation) {
        let context = Arc::new(GpuContext::for_test(0));
        let topology = MemoryTopologyObservation::for_test(&context, 0);
        let _ = backend;
        (context, topology)
    }

    #[test]
    fn successful_lifecycle_rounds_grants_verifies_and_reclaims() {
        let backend = Arc::new(MockBackend::new(4096));
        let (context, topology) = setup(backend.clone());
        let unmapped =
            VmmUnmappedAllocation::create_with_backend(&context, topology, 4097, backend.clone())
                .unwrap();
        assert_eq!(unmapped.layout().byte_len(), 8192);
        assert_eq!(unmapped.layout().granularity(), 4096);
        assert_eq!(
            unmapped.reservation_identity().kind(),
            AllocationKind::VmmVirtualRange
        );
        assert_eq!(
            unmapped.physical_identity().kind(),
            AllocationKind::VmmPhysical
        );

        let mut accessible = unmapped
            .map()
            .unwrap()
            .grant_access(topology, VmmAccess::ReadWrite)
            .unwrap();
        assert_eq!(
            accessible.access_for(topology.physical_device()),
            Some(VmmAccess::ReadWrite)
        );
        assert_eq!(
            accessible.query_access(topology).unwrap().access(),
            VmmAccess::ReadWrite
        );
        assert_eq!(
            accessible
                .grant_access(topology, VmmAccess::Read)
                .unwrap()
                .access(),
            VmmAccess::Read
        );
        let receipt = accessible.reclaim().unwrap();
        assert_eq!(
            receipt.reservation().kind(),
            AllocationKind::VmmVirtualRange
        );
        assert_eq!(receipt.physical().kind(), AllocationKind::VmmPhysical);

        let calls = backend.calls.lock().unwrap();
        assert!(calls.ends_with(&[Call::Bind, Call::Unmap, Call::Release, Call::AddressFree]));
    }

    #[test]
    fn construction_failure_reclaims_the_independent_reservation() {
        let backend = Arc::new(MockBackend::new(4096));
        *backend.fail.lock().unwrap() = Some(Call::Create(4096, 0));
        let (context, topology) = setup(backend.clone());
        assert!(matches!(
            VmmUnmappedAllocation::create_with_backend(&context, topology, 1, backend.clone()),
            Err(VmmError::Hip(_))
        ));
        assert!(
            backend
                .calls
                .lock()
                .unwrap()
                .ends_with(&[Call::Create(4096, 0), Call::AddressFree])
        );
    }

    #[test]
    fn ambiguous_map_leaks_instead_of_guessing_cleanup() {
        let backend = Arc::new(MockBackend::new(4096));
        *backend.fail.lock().unwrap() = Some(Call::Map);
        let (context, topology) = setup(backend.clone());
        let unmapped =
            VmmUnmappedAllocation::create_with_backend(&context, topology, 4096, backend.clone())
                .unwrap();
        assert!(matches!(unmapped.map(), Err(VmmError::MapAmbiguous(_))));
        let calls = backend.calls.lock().unwrap();
        assert!(!calls.contains(&Call::Unmap));
        assert!(!calls.contains(&Call::Release));
        assert!(!calls.contains(&Call::AddressFree));
    }

    #[test]
    fn failed_unmap_never_releases_live_backing_or_address() {
        let backend = Arc::new(MockBackend::new(4096));
        let (context, topology) = setup(backend.clone());
        let mapped =
            VmmUnmappedAllocation::create_with_backend(&context, topology, 4096, backend.clone())
                .unwrap()
                .map()
                .unwrap();
        *backend.fail.lock().unwrap() = Some(Call::Unmap);
        let error = mapped.reclaim().unwrap_err();
        assert_eq!(error.stage(), VmmCleanupStage::Unmap);
        let calls = backend.calls.lock().unwrap();
        assert!(!calls.contains(&Call::Release));
        assert!(!calls.contains(&Call::AddressFree));
    }
}
