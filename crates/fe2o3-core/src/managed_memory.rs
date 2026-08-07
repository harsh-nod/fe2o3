use crate::{
    AllocationIdentity, AllocationKind, Error, GpuContext, MemoryTopologyObservation, Stream, check,
};
use core::ffi::c_void;
use core::fmt;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> u64 {
    NEXT_REQUEST_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .unwrap_or_else(|_| panic!("process-local managed-memory request identity space exhausted"))
}

/// Exact destination of a managed-memory placement or advice request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ManagedMemoryLocation {
    Host,
    Device(crate::PhysicalDeviceIdentity),
}

impl ManagedMemoryLocation {
    pub const fn host() -> Self {
        Self::Host
    }

    pub const fn device(observation: MemoryTopologyObservation) -> Self {
        Self::Device(observation.physical_device())
    }

    const fn hip_device_id(self) -> i32 {
        match self {
            Self::Host => fe2o3_hip_sys::HIP_CPU_DEVICE_ID,
            Self::Device(device) => device.ordinal(),
        }
    }
}

/// Last placement fact established by this wrapper.
///
/// HIP prefetch completion and `LastPrefetchLocation` describe requests; they
/// do not prove that every page is physically resident at that location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedResidencyState {
    Unobserved,
    LastPrefetchCompleted {
        request_id: u64,
        location: ManagedMemoryLocation,
    },
    LastPrefetchQueried {
        request_id: u64,
        location: ManagedMemoryLocation,
    },
    AmbiguousInFlight {
        request_id: u64,
        requested: ManagedMemoryLocation,
    },
}

/// Canonical managed-memory advice transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedAdviceRequest {
    SetReadMostly,
    UnsetReadMostly,
    SetPreferredLocation(ManagedMemoryLocation),
    UnsetPreferredLocation(ManagedMemoryLocation),
    SetAccessedBy(crate::PhysicalDeviceIdentity),
    UnsetAccessedBy(crate::PhysicalDeviceIdentity),
    SetCoarseGrain,
    UnsetCoarseGrain,
}

/// Locally retained advice state after successful HIP calls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManagedAdviceState {
    read_mostly: bool,
    preferred_location: Option<ManagedMemoryLocation>,
    accessed_by: BTreeSet<crate::PhysicalDeviceIdentity>,
    coarse_grain: bool,
}

impl ManagedAdviceState {
    pub fn read_mostly(&self) -> bool {
        self.read_mostly
    }

    pub fn preferred_location(&self) -> Option<ManagedMemoryLocation> {
        self.preferred_location
    }

    pub fn accessed_by(&self, device: crate::PhysicalDeviceIdentity) -> bool {
        self.accessed_by.contains(&device)
    }

    pub fn coarse_grain(&self) -> bool {
        self.coarse_grain
    }
}

trait ManagedBackend: Send + Sync {
    fn bind(&self, context: &GpuContext) -> Result<(), Error>;
    fn allocate(&self, size: usize) -> Result<*mut c_void, Error>;
    fn prefetch(
        &self,
        pointer: *const c_void,
        size: usize,
        device_id: i32,
        stream: fe2o3_hip_sys::hipStream_t,
    ) -> Result<(), Error>;
    fn synchronize(&self, stream: &Stream) -> Result<(), Error>;
    fn advise(
        &self,
        pointer: *const c_void,
        size: usize,
        advice: u32,
        device_id: i32,
    ) -> Result<(), Error>;
    fn last_prefetch_location(&self, pointer: *const c_void, size: usize) -> Result<i32, Error>;
    fn free(&self, pointer: *mut c_void) -> Result<(), Error>;
}

struct HipManagedBackend;

impl ManagedBackend for HipManagedBackend {
    fn bind(&self, context: &GpuContext) -> Result<(), Error> {
        context.bind_to_thread()
    }

    fn allocate(&self, size: usize) -> Result<*mut c_void, Error> {
        let mut pointer = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::fe2o3HipMallocManaged(&mut pointer, size) })?;
        Ok(pointer)
    }

    fn prefetch(
        &self,
        pointer: *const c_void,
        size: usize,
        device_id: i32,
        stream: fe2o3_hip_sys::hipStream_t,
    ) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemPrefetchAsync(pointer, size, device_id, stream) })
    }

    fn synchronize(&self, stream: &Stream) -> Result<(), Error> {
        stream.synchronize()
    }

    fn advise(
        &self,
        pointer: *const c_void,
        size: usize,
        advice: u32,
        device_id: i32,
    ) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::fe2o3HipMemAdvise(pointer, size, advice, device_id) })
    }

    fn last_prefetch_location(&self, pointer: *const c_void, size: usize) -> Result<i32, Error> {
        let mut device_id = 0;
        check(unsafe {
            fe2o3_hip_sys::fe2o3HipMemRangeGetLastPrefetchLocation(pointer, size, &mut device_id)
        })?;
        Ok(device_id)
    }

    fn free(&self, pointer: *mut c_void) -> Result<(), Error> {
        check(unsafe { fe2o3_hip_sys::hipFree(pointer) })
    }
}

/// Exclusive ownership and cleanup responsibility for one HIP managed allocation.
///
/// This type is intentionally neither `Clone` nor `Copy`. It exposes no safe
/// host slice or device pointer. Placement and advice receipts grant no memory,
/// peer, or launch authority.
#[must_use = "dropping managed memory attempts native reclamation"]
pub struct ManagedAllocation {
    pointer: *mut c_void,
    identity: AllocationIdentity,
    topology: MemoryTopologyObservation,
    context: Arc<GpuContext>,
    residency: ManagedResidencyState,
    advice: ManagedAdviceState,
    reclaimable: bool,
    active: bool,
    backend: Arc<dyn ManagedBackend>,
}

impl fmt::Debug for ManagedAllocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedAllocation")
            .field("identity", &self.identity)
            .field("residency", &self.residency)
            .field("advice", &self.advice)
            .field("reclaimable", &self.reclaimable)
            .finish_non_exhaustive()
    }
}

impl ManagedAllocation {
    pub fn allocate(
        context: &Arc<GpuContext>,
        topology: MemoryTopologyObservation,
        byte_len: usize,
    ) -> Result<Self, ManagedMemoryError> {
        Self::allocate_with_backend(context, topology, byte_len, Arc::new(HipManagedBackend))
    }

    fn allocate_with_backend(
        context: &Arc<GpuContext>,
        topology: MemoryTopologyObservation,
        byte_len: usize,
        backend: Arc<dyn ManagedBackend>,
    ) -> Result<Self, ManagedMemoryError> {
        if !topology.is_for_context(context) {
            return Err(ManagedMemoryError::ForeignTopologyObservation);
        }
        if !topology.capabilities().managed_memory() {
            return Err(ManagedMemoryError::Unsupported);
        }
        if byte_len == 0 {
            return Err(ManagedMemoryError::EmptyAllocation);
        }
        backend.bind(context).map_err(ManagedMemoryError::Hip)?;
        let pointer = backend
            .allocate(byte_len)
            .map_err(ManagedMemoryError::Hip)?;
        if pointer.is_null() {
            return Err(ManagedMemoryError::NullAllocation);
        }
        Ok(Self {
            pointer,
            identity: topology.new_allocation_identity(AllocationKind::Managed, byte_len),
            topology,
            context: context.clone(),
            residency: ManagedResidencyState::Unobserved,
            advice: ManagedAdviceState::default(),
            reclaimable: true,
            active: true,
            backend,
        })
    }

    pub const fn identity(&self) -> AllocationIdentity {
        self.identity
    }

    pub const fn topology(&self) -> MemoryTopologyObservation {
        self.topology
    }

    pub const fn residency(&self) -> ManagedResidencyState {
        self.residency
    }

    pub fn advice_state(&self) -> &ManagedAdviceState {
        &self.advice
    }

    /// Returns the untyped managed pointer without creating dereference authority.
    ///
    /// # Safety
    ///
    /// The caller must retain this owner, establish host/device quiescence,
    /// enforce bounds and aliasing, and never free or use the pointer after
    /// reclamation. This API does not authorize a kernel launch.
    pub unsafe fn raw_pointer(&self) -> *mut c_void {
        self.pointer
    }

    pub fn prefetch_to_device(
        &mut self,
        stream: &Stream,
        destination: MemoryTopologyObservation,
    ) -> Result<ManagedMigrationReceipt, ManagedMemoryError> {
        if !destination.is_for_context(stream.context()) {
            return Err(ManagedMemoryError::StreamTopologyMismatch);
        }
        self.prefetch(stream, ManagedMemoryLocation::device(destination))
    }

    pub fn prefetch_to_host(
        &mut self,
        stream: &Stream,
    ) -> Result<ManagedMigrationReceipt, ManagedMemoryError> {
        if self.context.identity() != stream.context().identity() {
            return Err(ManagedMemoryError::StreamTopologyMismatch);
        }
        self.prefetch(stream, ManagedMemoryLocation::Host)
    }

    fn prefetch(
        &mut self,
        stream: &Stream,
        location: ManagedMemoryLocation,
    ) -> Result<ManagedMigrationReceipt, ManagedMemoryError> {
        if !self.reclaimable {
            return Err(ManagedMemoryError::AmbiguousInFlight);
        }
        let request_id = next_request_id();
        self.backend
            .bind(stream.context())
            .map_err(ManagedMemoryError::Hip)?;
        self.reclaimable = false;
        let operation = self.backend.prefetch(
            self.pointer,
            self.identity.byte_len(),
            location.hip_device_id(),
            unsafe { stream.raw() },
        );
        let synchronization = self.backend.synchronize(stream);
        match (operation, synchronization) {
            (Ok(()), Ok(())) => {
                self.reclaimable = true;
                self.residency = ManagedResidencyState::LastPrefetchCompleted {
                    request_id,
                    location,
                };
                Ok(ManagedMigrationReceipt {
                    allocation: self.identity,
                    request_id,
                    location,
                })
            }
            (Err(operation), Ok(())) => {
                self.reclaimable = true;
                Err(ManagedMemoryError::Hip(operation))
            }
            (operation, Err(synchronization)) => {
                self.residency = ManagedResidencyState::AmbiguousInFlight {
                    request_id,
                    requested: location,
                };
                Err(ManagedMemoryError::MigrationRecoveryFailed {
                    operation: operation.err().map(Box::new),
                    synchronization: Box::new(synchronization),
                })
            }
        }
    }

    /// Queries and verifies the exact last-prefetch location.
    ///
    /// The expected location supplies the physical identity corresponding to a
    /// returned HIP ordinal. A matching query still does not prove page residency.
    pub fn query_last_prefetch_location(
        &mut self,
        expected: ManagedMemoryLocation,
    ) -> Result<ManagedMigrationReceipt, ManagedMemoryError> {
        if !self.reclaimable {
            return Err(ManagedMemoryError::AmbiguousInFlight);
        }
        self.backend
            .bind(&self.context)
            .map_err(ManagedMemoryError::Hip)?;
        let observed = self
            .backend
            .last_prefetch_location(self.pointer, self.identity.byte_len())
            .map_err(ManagedMemoryError::Hip)?;
        if observed != expected.hip_device_id() {
            return Err(ManagedMemoryError::UnexpectedPrefetchLocation {
                expected: expected.hip_device_id(),
                observed,
            });
        }
        let request_id = next_request_id();
        self.residency = ManagedResidencyState::LastPrefetchQueried {
            request_id,
            location: expected,
        };
        Ok(ManagedMigrationReceipt {
            allocation: self.identity,
            request_id,
            location: expected,
        })
    }

    pub fn apply_advice(
        &mut self,
        request: ManagedAdviceRequest,
    ) -> Result<ManagedAdviceReceipt, ManagedMemoryError> {
        if !self.reclaimable {
            return Err(ManagedMemoryError::AmbiguousInFlight);
        }
        validate_advice_transition(&self.advice, request)?;
        let (native, device_id) = native_advice(request);
        self.backend
            .bind(&self.context)
            .map_err(ManagedMemoryError::Hip)?;
        self.backend
            .advise(self.pointer, self.identity.byte_len(), native, device_id)
            .map_err(ManagedMemoryError::Hip)?;
        apply_advice_transition(&mut self.advice, request);
        Ok(ManagedAdviceReceipt {
            allocation: self.identity,
            request_id: next_request_id(),
            request,
        })
    }

    pub fn reclaim(mut self) -> Result<ManagedReclamationReceipt, ManagedMemoryCleanupError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<ManagedReclamationReceipt, ManagedMemoryCleanupError> {
        debug_assert!(self.active);
        self.active = false;
        if !self.reclaimable {
            self.leak_context();
            return Err(ManagedMemoryCleanupError::AmbiguousInFlight {
                identity: self.identity,
            });
        }
        if let Err(error) = self.backend.bind(&self.context) {
            self.leak_context();
            return Err(ManagedMemoryCleanupError::Hip {
                identity: self.identity,
                error,
            });
        }
        if let Err(error) = self.backend.free(self.pointer) {
            self.leak_context();
            return Err(ManagedMemoryCleanupError::Hip {
                identity: self.identity,
                error,
            });
        }
        Ok(ManagedReclamationReceipt {
            identity: self.identity,
        })
    }

    fn leak_context(&self) {
        core::mem::forget(self.context.clone());
    }
}

impl Drop for ManagedAllocation {
    fn drop(&mut self) {
        if self.active {
            let _ = self.cleanup();
        }
    }
}

fn validate_advice_transition(
    state: &ManagedAdviceState,
    request: ManagedAdviceRequest,
) -> Result<(), ManagedMemoryError> {
    let valid = match request {
        ManagedAdviceRequest::SetReadMostly => !state.read_mostly,
        ManagedAdviceRequest::UnsetReadMostly => state.read_mostly,
        ManagedAdviceRequest::SetPreferredLocation(_) => state.preferred_location.is_none(),
        ManagedAdviceRequest::UnsetPreferredLocation(location) => {
            state.preferred_location == Some(location)
        }
        ManagedAdviceRequest::SetAccessedBy(device) => !state.accessed_by.contains(&device),
        ManagedAdviceRequest::UnsetAccessedBy(device) => state.accessed_by.contains(&device),
        ManagedAdviceRequest::SetCoarseGrain => !state.coarse_grain,
        ManagedAdviceRequest::UnsetCoarseGrain => state.coarse_grain,
    };
    if valid {
        Ok(())
    } else {
        Err(ManagedMemoryError::InvalidAdviceTransition { request })
    }
}

fn apply_advice_transition(state: &mut ManagedAdviceState, request: ManagedAdviceRequest) {
    match request {
        ManagedAdviceRequest::SetReadMostly => state.read_mostly = true,
        ManagedAdviceRequest::UnsetReadMostly => state.read_mostly = false,
        ManagedAdviceRequest::SetPreferredLocation(location) => {
            state.preferred_location = Some(location);
        }
        ManagedAdviceRequest::UnsetPreferredLocation(_) => state.preferred_location = None,
        ManagedAdviceRequest::SetAccessedBy(device) => {
            state.accessed_by.insert(device);
        }
        ManagedAdviceRequest::UnsetAccessedBy(device) => {
            state.accessed_by.remove(&device);
        }
        ManagedAdviceRequest::SetCoarseGrain => state.coarse_grain = true,
        ManagedAdviceRequest::UnsetCoarseGrain => state.coarse_grain = false,
    }
}

fn native_advice(request: ManagedAdviceRequest) -> (u32, i32) {
    match request {
        ManagedAdviceRequest::SetReadMostly => {
            (fe2o3_hip_sys::HIP_MEMORY_ADVISE_SET_READ_MOSTLY, 0)
        }
        ManagedAdviceRequest::UnsetReadMostly => {
            (fe2o3_hip_sys::HIP_MEMORY_ADVISE_UNSET_READ_MOSTLY, 0)
        }
        ManagedAdviceRequest::SetPreferredLocation(location) => (
            fe2o3_hip_sys::HIP_MEMORY_ADVISE_SET_PREFERRED_LOCATION,
            location.hip_device_id(),
        ),
        ManagedAdviceRequest::UnsetPreferredLocation(location) => (
            fe2o3_hip_sys::HIP_MEMORY_ADVISE_UNSET_PREFERRED_LOCATION,
            location.hip_device_id(),
        ),
        ManagedAdviceRequest::SetAccessedBy(device) => (
            fe2o3_hip_sys::HIP_MEMORY_ADVISE_SET_ACCESSED_BY,
            device.ordinal(),
        ),
        ManagedAdviceRequest::UnsetAccessedBy(device) => (
            fe2o3_hip_sys::HIP_MEMORY_ADVISE_UNSET_ACCESSED_BY,
            device.ordinal(),
        ),
        ManagedAdviceRequest::SetCoarseGrain => {
            (fe2o3_hip_sys::HIP_MEMORY_ADVISE_SET_COARSE_GRAIN, 0)
        }
        ManagedAdviceRequest::UnsetCoarseGrain => {
            (fe2o3_hip_sys::HIP_MEMORY_ADVISE_UNSET_COARSE_GRAIN, 0)
        }
    }
}

/// Linear evidence that one exact prefetch operation completed or was queried.
#[derive(Debug)]
pub struct ManagedMigrationReceipt {
    allocation: AllocationIdentity,
    request_id: u64,
    location: ManagedMemoryLocation,
}

impl ManagedMigrationReceipt {
    pub const fn allocation(&self) -> AllocationIdentity {
        self.allocation
    }

    pub const fn location(&self) -> ManagedMemoryLocation {
        self.location
    }

    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
}

/// Linear evidence that HIP accepted one exact advice transition.
#[derive(Debug)]
pub struct ManagedAdviceReceipt {
    allocation: AllocationIdentity,
    request_id: u64,
    request: ManagedAdviceRequest,
}

impl ManagedAdviceReceipt {
    pub const fn allocation(&self) -> AllocationIdentity {
        self.allocation
    }

    pub const fn request(&self) -> ManagedAdviceRequest {
        self.request
    }

    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
}

#[derive(Debug)]
pub struct ManagedReclamationReceipt {
    identity: AllocationIdentity,
}

impl ManagedReclamationReceipt {
    pub const fn identity(&self) -> AllocationIdentity {
        self.identity
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ManagedMemoryError {
    ForeignTopologyObservation,
    Unsupported,
    EmptyAllocation,
    NullAllocation,
    StreamTopologyMismatch,
    AmbiguousInFlight,
    UnexpectedPrefetchLocation {
        expected: i32,
        observed: i32,
    },
    InvalidAdviceTransition {
        request: ManagedAdviceRequest,
    },
    Hip(Error),
    MigrationRecoveryFailed {
        operation: Option<Box<Error>>,
        synchronization: Box<Error>,
    },
}

impl fmt::Display for ManagedMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignTopologyObservation => {
                write!(
                    formatter,
                    "memory topology observation belongs to another context"
                )
            }
            Self::Unsupported => write!(formatter, "managed memory is not supported"),
            Self::EmptyAllocation => write!(formatter, "managed allocation must be non-empty"),
            Self::NullAllocation => write!(formatter, "HIP returned a null managed allocation"),
            Self::StreamTopologyMismatch => {
                write!(
                    formatter,
                    "stream does not match the exact requested topology context"
                )
            }
            Self::AmbiguousInFlight => {
                write!(
                    formatter,
                    "managed allocation has an ambiguously in-flight migration"
                )
            }
            Self::UnexpectedPrefetchLocation { expected, observed } => write!(
                formatter,
                "HIP reported last prefetch location {observed}, expected {expected}"
            ),
            Self::InvalidAdviceTransition { request } => {
                write!(
                    formatter,
                    "invalid or duplicate managed advice transition: {request:?}"
                )
            }
            Self::Hip(error) => error.fmt(formatter),
            Self::MigrationRecoveryFailed {
                operation,
                synchronization,
            } => match operation {
                Some(operation) => write!(
                    formatter,
                    "managed migration failed ({operation}) and synchronization was ambiguous ({synchronization})"
                ),
                None => write!(
                    formatter,
                    "managed migration synchronization was ambiguous ({synchronization})"
                ),
            },
        }
    }
}

impl std::error::Error for ManagedMemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip(error) => Some(error),
            Self::MigrationRecoveryFailed {
                synchronization, ..
            } => Some(synchronization),
            _ => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ManagedMemoryCleanupError {
    AmbiguousInFlight {
        identity: AllocationIdentity,
    },
    Hip {
        identity: AllocationIdentity,
        error: Error,
    },
}

impl ManagedMemoryCleanupError {
    pub const fn identity(&self) -> AllocationIdentity {
        match self {
            Self::AmbiguousInFlight { identity } | Self::Hip { identity, .. } => *identity,
        }
    }
}

impl fmt::Display for ManagedMemoryCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmbiguousInFlight { .. } => write!(
                formatter,
                "managed memory was leaked because migration completion is ambiguous"
            ),
            Self::Hip { error, .. } => {
                write!(
                    formatter,
                    "managed memory reclamation is ambiguous: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ManagedMemoryCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hip { error, .. } => Some(error),
            Self::AmbiguousInFlight { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Bind(i32),
        Allocate(usize),
        Prefetch(usize, i32),
        Synchronize(i32),
        Advise(u32, i32),
        Query,
        Free,
    }

    struct MockBackend {
        pointer: usize,
        operations: Mutex<VecDeque<Result<(), Error>>>,
        synchronizations: Mutex<VecDeque<Result<(), Error>>>,
        query: Mutex<Result<i32, Error>>,
        calls: Mutex<Vec<Call>>,
    }

    impl MockBackend {
        fn successful() -> Arc<Self> {
            Arc::new(Self {
                pointer: 0x1000,
                operations: Mutex::new(VecDeque::new()),
                synchronizations: Mutex::new(VecDeque::new()),
                query: Mutex::new(Ok(0)),
                calls: Mutex::new(Vec::new()),
            })
        }

        fn operation(&self) -> Result<(), Error> {
            self.operations
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }
    }

    impl ManagedBackend for MockBackend {
        fn bind(&self, context: &GpuContext) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Bind(context.device_id()));
            self.operation()
        }

        fn allocate(&self, size: usize) -> Result<*mut c_void, Error> {
            self.calls.lock().unwrap().push(Call::Allocate(size));
            self.operation()?;
            Ok(self.pointer as *mut c_void)
        }

        fn prefetch(
            &self,
            _pointer: *const c_void,
            size: usize,
            device_id: i32,
            _stream: fe2o3_hip_sys::hipStream_t,
        ) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Prefetch(size, device_id));
            self.operation()
        }

        fn synchronize(&self, stream: &Stream) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Synchronize(stream.context().device_id()));
            self.synchronizations
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        fn advise(
            &self,
            _pointer: *const c_void,
            _size: usize,
            advice: u32,
            device_id: i32,
        ) -> Result<(), Error> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Advise(advice, device_id));
            self.operation()
        }

        fn last_prefetch_location(
            &self,
            _pointer: *const c_void,
            _size: usize,
        ) -> Result<i32, Error> {
            self.calls.lock().unwrap().push(Call::Query);
            match &*self.query.lock().unwrap() {
                Ok(value) => Ok(*value),
                Err(_) => Err(Error::SizeOverflow),
            }
        }

        fn free(&self, _pointer: *mut c_void) -> Result<(), Error> {
            self.calls.lock().unwrap().push(Call::Free);
            self.operation()
        }
    }

    fn allocation(
        backend: &Arc<MockBackend>,
        device: i32,
        size: usize,
    ) -> (
        ManagedAllocation,
        Arc<GpuContext>,
        MemoryTopologyObservation,
    ) {
        let context = Arc::new(GpuContext::for_test(device));
        let topology = MemoryTopologyObservation::for_test(&context, device);
        let allocation =
            ManagedAllocation::allocate_with_backend(&context, topology, size, backend.clone())
                .unwrap();
        (allocation, context, topology)
    }

    #[test]
    fn allocation_binds_exact_identity_and_reclaims_once() {
        let backend = MockBackend::successful();
        let (allocation, context, topology) = allocation(&backend, 2, 4096);
        let identity = allocation.identity();
        assert_eq!(identity.context(), context.identity());
        assert_eq!(identity.physical_device(), topology.physical_device());
        assert_eq!(identity.kind(), AllocationKind::Managed);
        assert_eq!(identity.byte_len(), 4096);

        let receipt = allocation.reclaim().unwrap();
        assert_eq!(receipt.identity(), identity);
        assert_eq!(
            *backend.calls.lock().unwrap(),
            [
                Call::Bind(2),
                Call::Allocate(4096),
                Call::Bind(2),
                Call::Free
            ]
        );
    }

    #[test]
    fn allocation_rejects_foreign_observation_empty_and_null_results() {
        let backend = MockBackend::successful();
        let left = Arc::new(GpuContext::for_test(0));
        let right = Arc::new(GpuContext::for_test(0));
        let topology = MemoryTopologyObservation::for_test(&left, 0);
        assert!(matches!(
            ManagedAllocation::allocate_with_backend(&right, topology, 1, backend.clone()),
            Err(ManagedMemoryError::ForeignTopologyObservation)
        ));
        assert!(matches!(
            ManagedAllocation::allocate_with_backend(&left, topology, 0, backend.clone()),
            Err(ManagedMemoryError::EmptyAllocation)
        ));

        let null = Arc::new(MockBackend {
            pointer: 0,
            operations: Mutex::new(VecDeque::new()),
            synchronizations: Mutex::new(VecDeque::new()),
            query: Mutex::new(Ok(0)),
            calls: Mutex::new(Vec::new()),
        });
        assert!(matches!(
            ManagedAllocation::allocate_with_backend(&left, topology, 1, null),
            Err(ManagedMemoryError::NullAllocation)
        ));
    }

    #[test]
    fn prefetch_completion_records_request_without_claiming_residency() {
        let backend = MockBackend::successful();
        let (mut allocation, context, topology) = allocation(&backend, 1, 64);
        let stream = context.default_stream();
        let receipt = allocation.prefetch_to_device(&stream, topology).unwrap();

        assert_eq!(receipt.allocation(), allocation.identity());
        assert_eq!(receipt.location(), ManagedMemoryLocation::device(topology));
        assert!(matches!(
            allocation.residency(),
            ManagedResidencyState::LastPrefetchCompleted { location, .. }
                if location == ManagedMemoryLocation::device(topology)
        ));
        drop(allocation);
        assert!(backend.calls.lock().unwrap().contains(&Call::Free));
    }

    #[test]
    fn enqueue_error_with_successful_recovery_remains_reclaimable() {
        let backend = MockBackend::successful();
        let (mut allocation, context, topology) = allocation(&backend, 0, 64);
        backend
            .operations
            .lock()
            .unwrap()
            .push_back(Err(Error::SizeOverflow));

        assert!(matches!(
            allocation.prefetch_to_device(&context.default_stream(), topology),
            Err(ManagedMemoryError::Hip(Error::SizeOverflow))
        ));
        allocation.reclaim().unwrap();
        assert!(backend.calls.lock().unwrap().contains(&Call::Free));
    }

    #[test]
    fn ambiguous_prefetch_suppresses_all_reclamation() {
        let backend = MockBackend::successful();
        let (mut allocation, context, topology) = allocation(&backend, 0, 64);
        backend
            .synchronizations
            .lock()
            .unwrap()
            .push_back(Err(Error::SizeOverflow));

        assert!(matches!(
            allocation.prefetch_to_device(&context.default_stream(), topology),
            Err(ManagedMemoryError::MigrationRecoveryFailed { .. })
        ));
        let identity = allocation.identity();
        assert!(matches!(
            allocation.reclaim(),
            Err(ManagedMemoryCleanupError::AmbiguousInFlight { identity: failed })
                if failed == identity
        ));
        assert!(!backend.calls.lock().unwrap().contains(&Call::Free));
    }

    #[test]
    fn advice_state_changes_only_after_success_and_rejects_duplicates() {
        let backend = MockBackend::successful();
        let (mut allocation, _, topology) = allocation(&backend, 0, 64);
        let device = topology.physical_device();

        for request in [
            ManagedAdviceRequest::SetReadMostly,
            ManagedAdviceRequest::SetPreferredLocation(ManagedMemoryLocation::device(topology)),
            ManagedAdviceRequest::SetAccessedBy(device),
            ManagedAdviceRequest::SetCoarseGrain,
        ] {
            let receipt = allocation.apply_advice(request).unwrap();
            assert_eq!(receipt.allocation(), allocation.identity());
            assert_eq!(receipt.request(), request);
        }
        assert!(allocation.advice_state().read_mostly());
        assert!(allocation.advice_state().accessed_by(device));
        assert!(allocation.advice_state().coarse_grain());
        assert!(matches!(
            allocation.apply_advice(ManagedAdviceRequest::SetReadMostly),
            Err(ManagedMemoryError::InvalidAdviceTransition { .. })
        ));
    }

    #[test]
    fn queried_location_requires_exact_expected_ordinal() {
        let backend = MockBackend::successful();
        let (mut allocation, _, topology) = allocation(&backend, 0, 64);
        assert!(
            allocation
                .query_last_prefetch_location(ManagedMemoryLocation::device(topology))
                .is_ok()
        );
        *backend.query.lock().unwrap() = Ok(7);
        assert!(matches!(
            allocation.query_last_prefetch_location(ManagedMemoryLocation::device(topology)),
            Err(ManagedMemoryError::UnexpectedPrefetchLocation {
                expected: 0,
                observed: 7
            })
        ));
    }
}
