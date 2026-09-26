//! Server-local request custody; no accounting authority crosses the wire.

#![forbid(unsafe_code)]

use super::*;
use crate::RuntimeWorkerErrorV1;

/// Persistent allocation owner for an explicitly accounted runtime worker.
///
/// A complete Required profile is installed in a private Context. The worker
/// mints and retains its own request credits, independently of any client-side
/// limits. Borrowed witnesses, accounts and native authority are never serialized.
/// Other worker resources remain owned directly by the backend, not this Context.
///
/// Keep this owner alive when a serving call fails. I/O loss, malformed requests
/// and unwind seal it and quarantine local credits without disposal. Native
/// backends may abort on Drop when ambiguous custody remains.
#[must_use = "worker request custody must survive serving errors"]
pub struct RuntimeWorkerRequestOwnerV1<B: RuntimeBackendV1> {
    context: RuntimeContextV1<B>,
    allocations: HashMap<u64, RuntimeAllocationIdV1>,
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeWorkerRequestOwnerV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeWorkerRequestOwnerV1")
            .field("allocations", &self.context.allocations.len())
            .field("terminal", &self.is_terminal_v1())
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeWorkerRequestOwnerV1<B> {
    /// Admits the complete backend roster, returning the original backend on
    /// failure. Legacy or empty profiles are unsupported. Existing allocations
    /// are not adopted; construct this owner before accepting worker requests.
    /// Context initialization unwind retains the backend until process exit
    /// and propagates the original panic. No recoverable owner is returned.
    /// The caller remains responsible for custody of returned failures.
    pub fn open(backend: B) -> Result<Self, RuntimeContextOpenFailureV1<B>> {
        let context = RuntimeContextV1::open_configured_v1(backend, None)?;
        if !context.allocation_admission.is_required() {
            return Err(RuntimeContextOpenFailureV1 {
                backend: context.backend,
                error: RuntimeValidationErrorV1::Unsupported.into(),
            });
        }
        Ok(Self {
            context,
            allocations: HashMap::new(),
        })
    }

    pub const fn is_terminal_v1(&self) -> bool {
        self.context.is_terminal()
    }

    /// Includes malformed successful allocations retained by a sealed Context.
    pub fn retained_allocations_v1(&self) -> usize {
        self.context.allocations.len()
    }

    /// Returns the backend only after every request was explicitly disposed.
    /// This does not clean up worker streams/modules or assert native quiescence.
    /// The caller must still perform the backend's explicit native teardown.
    /// An empty worker shutdown frame alone does not satisfy this condition.
    /// Quarantined Required sessions also prevent transfer; healthy external
    /// reservations or retained credits alone do not.
    pub fn try_into_backend(self) -> Result<B, Box<Self>> {
        if self.is_terminal_v1()
            || !self.allocations.is_empty()
            || !self.context.allocations.is_empty()
            || !self.context.backend_allocations.is_empty()
            || self.context.allocation_admission.has_local_retained()
            || !self
                .context
                .allocation_admission
                .required_sessions_are_live()
        {
            return Err(Box::new(self));
        }
        Ok(self.context.backend)
    }

    pub(crate) fn require_live_v1(&self) -> Result<(), RuntimeWorkerErrorV1> {
        if self.is_terminal_v1() {
            Err(RuntimeWorkerErrorV1::Protocol(
                "worker request owner is terminal",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn seal_v1(&mut self) {
        self.context.quarantine_after_async_command_panic_v1();
    }

    pub(crate) fn enumerate_v1(
        &self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeErrorV1<B::Error>> {
        self.context.require_live()?;
        let mut descriptions = Vec::new();
        descriptions
            .try_reserve_exact(self.context.devices.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        descriptions.extend(
            self.context
                .devices
                .iter()
                .map(|device| BackendDeviceDescriptionV1 {
                    backend_device: device.backend_device,
                    name: device.name.clone(),
                    target: device.target.clone(),
                    global_memory_bytes: device.global_memory_bytes,
                    capabilities: device.capabilities,
                }),
        );
        Ok(descriptions)
    }

    pub(crate) fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeErrorV1<B::Error>> {
        self.context.require_live()?;
        let device = self
            .context
            .devices
            .iter()
            .find(|entry| entry.backend_device == device)
            .ok_or(RuntimeValidationErrorV1::UnknownDevice)?
            .id;
        if self.allocations.len() >= MAX_RUNTIME_ALLOCATIONS_V1 {
            return Err(RuntimeValidationErrorV1::Capacity.into());
        }
        self.allocations
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let id = self.context.allocate(device, kind, bytes, alignment)?;
        // Context roots and seals invalid native handles itself. After success,
        // the raw-handle index is vacant and reserved; no fallible tail remains.
        let handle = self.context.allocations[&id].backend_allocation;
        self.allocations.insert(handle, id);
        Ok(handle)
    }

    pub(crate) fn release_v1(&mut self, handle: u64) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.context.require_live()?;
        let id = *self
            .allocations
            .get(&handle)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        self.context.release_allocation(id)?;
        self.allocations.remove(&handle);
        Ok(())
    }

    pub(crate) fn dispatch_backend_v1(
        &mut self,
        request: &[u8],
        dispatch: impl FnOnce(
            &mut B,
            &[u8],
            &dyn Fn(u64) -> bool,
        ) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
        self.require_live_v1()?;
        let owns = |handle| self.allocations.contains_key(&handle);
        dispatch(&mut self.context.backend, request, &owns)
    }
}
