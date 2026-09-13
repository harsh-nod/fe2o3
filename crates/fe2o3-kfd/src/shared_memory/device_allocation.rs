//! Actual device-local allocation custody through borrowed GPU mapping.

use super::*;
use transitions::NativeTransitionProgressV1;

pub(super) enum AllocationLeaseV1 {
    None,
    Unmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    Mapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
}

pub(super) struct DeviceAllocationCustodyV1 {
    pub(super) lease: AllocationLeaseV1,
    pub(super) native_started: bool,
    pub(super) progress: NativeTransitionProgressV1,
    pub(super) failed: bool,
    pub(super) started: bool,
}

impl DeviceAllocationCustodyV1 {
    pub(super) fn new() -> Self {
        Self {
            lease: AllocationLeaseV1::None,
            native_started: false,
            progress: NativeTransitionProgressV1::default(),
            failed: false,
            started: false,
        }
    }

    pub(super) fn prepare_in_place<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        if self.started {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        self.started = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.require_active()?;
            if engine.terminal_device_initialization.is_some() {
                return engine.quarantine(MemorySessionError::SharedSessionQuarantined);
            }
            let lease = engine.with_device_backing_unwind_quarantine(|engine| {
                engine.allocate_device_memory_with_flags_inner(
                    device,
                    vm,
                    requested_bytes,
                    alignment,
                    KfdAllocMemoryFlags::DEVICE_LOCAL,
                    &mut self.native_started,
                )
            })?;
            self.lease = AllocationLeaseV1::Unmapped(lease);
            let AllocationLeaseV1::Unmapped(lease) = &self.lease else {
                unreachable!("retained device allocation");
            };
            engine.map_device_memory_borrowed(lease, &mut self.progress)?;
            let AllocationLeaseV1::Unmapped(lease) =
                std::mem::replace(&mut self.lease, AllocationLeaseV1::None)
            else {
                unreachable!("borrowed device allocation");
            };
            self.lease = AllocationLeaseV1::Mapped(lease.retag());
            Ok(())
        }));
        if !matches!(result, Ok(Ok(()))) {
            self.failed = true;
            if self.native_started {
                engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(super) fn completed(
        &self,
    ) -> Result<&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, MemorySessionError> {
        if !self.failed
            && let AllocationLeaseV1::Mapped(lease) = &self.lease
        {
            return Ok(lease);
        }
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    }

    pub(super) fn take_complete(
        &mut self,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, MemorySessionError> {
        self.completed()?;
        let AllocationLeaseV1::Mapped(lease) =
            std::mem::replace(&mut self.lease, AllocationLeaseV1::None)
        else {
            unreachable!("checked mapped device allocation");
        };
        Ok(lease)
    }

    pub(super) fn requires_retention(&self) -> bool {
        self.native_started
    }

    pub(super) fn retain_live_failure<B: MemoryBackend>(
        mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) {
        if !self.requires_retention() {
            return;
        }
        self.failed = true;
        engine.phase = SharedMemorySessionPhaseV1::Quarantined;
        if engine.terminal_device_initialization.is_some() {
            // Preserve the earlier owner and original failure on invariant breach.
            let _unplaced = core::mem::ManuallyDrop::new(self);
        } else {
            engine
                .terminal_device_initialization
                .retain_allocation(self);
        }
    }
}
