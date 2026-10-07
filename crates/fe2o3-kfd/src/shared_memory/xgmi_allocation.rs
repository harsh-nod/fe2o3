//! Allocation-only classification from the original session's retained state.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942XgmiAllocationDispositionV1 {
    /// Exact capacity rejection before currentness/native allocation, with a
    /// still-active session and no new backing charge or native owner.
    RejectedCapacity,
    /// No retry authority is supplied. Preserve the session and its uncertainty.
    ProcessTeardown,
}

#[derive(Debug)]
pub struct Gfx942XgmiAllocationFailureV1 {
    error: MemorySessionError,
    disposition: Gfx942XgmiAllocationDispositionV1,
}

impl Gfx942XgmiAllocationFailureV1 {
    pub fn error(&self) -> &MemorySessionError {
        &self.error
    }

    pub fn disposition(&self) -> Gfx942XgmiAllocationDispositionV1 {
        self.disposition
    }

    pub fn into_error(self) -> MemorySessionError {
        self.error
    }
}

impl fmt::Display for Gfx942XgmiAllocationFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl std::error::Error for Gfx942XgmiAllocationFailureV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl<B: MemoryBackend> SharedMemoryEngine<B> {
    pub(super) fn allocate_xgmi_device_memory_classified_v1(
        &mut self,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
        Gfx942XgmiAllocationFailureV1,
    > {
        let mut native_started = false;
        self.with_device_backing_unwind_quarantine(|engine| {
            let result = engine.allocate_device_memory_with_flags_inner(
                device,
                vm,
                requested_bytes,
                alignment,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                &mut native_started,
            );
            result.map_err(|error| {
                // Currentness failure can precede native_started. Its quarantine
                // must prevent a coincidentally capacity-shaped error from retrying.
                let capacity = !native_started
                    && engine.phase() == SharedMemorySessionPhaseV1::Active
                    && matches!(
                        error,
                        MemorySessionError::DeviceMemoryAllocationCapacity { .. }
                            | MemorySessionError::DeviceMemoryByteCapacity { .. }
                            | MemorySessionError::DeviceBackingCredits(
                                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
                            )
                    );
                Gfx942XgmiAllocationFailureV1 {
                    error,
                    disposition: if capacity {
                        Gfx942XgmiAllocationDispositionV1::RejectedCapacity
                    } else {
                        Gfx942XgmiAllocationDispositionV1::ProcessTeardown
                    },
                }
            })
        })
    }
}
