//! Session-owned native outputs before a usable allocation record exists.

use super::*;
use fe2o3_kfd_uapi::KfdIoctlAllocMemoryOfGpuArgs;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingAllocationStageV1 {
    ReserveVa,
    ValidateReservation,
    PrepareUserptr,
    CheckUserptr,
    Allocate,
    ValidateAllocation,
    CheckAllocation,
    MapCpu,
    PrepareCpuMapping,
    ValidateMapping,
    CheckMapping,
}

pub(super) struct PendingSharedAllocationV1<B: MemoryBackend> {
    pub(super) id: u64,
    pub(super) profile: SharedGttProfileV1,
    pub(super) layout: SharedGttAllocationLayoutV1,
    pub(super) record_slot: usize,
    pub(super) stage: PendingAllocationStageV1,
    pub(super) reservation: Option<B::Reservation>,
    pub(super) allocation_output: Option<KfdIoctlAllocMemoryOfGpuArgs>,
    pub(super) mapping: Option<B::Mapping>,
    pub(super) host_backing_charge: Option<HostBackingChargeV1>,
}

pub(super) fn allocate_v1<B: MemoryBackend, P: GttProfileV1>(
    engine: &mut SharedMemoryEngine<B>,
    requested_bytes: usize,
) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
    let configured_host = engine.host_backing_account.is_some() && is_host_backing_profile::<P>();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.allocate_inner::<P>(requested_bytes)
    }));
    match outcome {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(error)) => {
            engine.quarantine_pending_allocation();
            Err(error)
        }
        Err(payload) => {
            engine.quarantine_pending_allocation();
            if configured_host {
                engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            }
            std::panic::resume_unwind(payload)
        }
    }
}

impl<B: MemoryBackend> SharedMemoryEngine<B> {
    fn quarantine_pending_allocation(&mut self) {
        if let Some(pending) = &mut self.pending_allocation {
            self.phase = SharedMemorySessionPhaseV1::Quarantined;
            if let Some(charge) = pending.host_backing_charge.take() {
                charge.quarantine();
            }
        }
    }

    fn pending_allocation_mut(&mut self) -> &mut PendingSharedAllocationV1<B> {
        self.pending_allocation
            .as_mut()
            .expect("rooted native attempt")
    }

    fn allocate_inner<P: GttProfileV1>(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        self.require_active()?;
        if self.pending_allocation.is_some() {
            return self.quarantine(MemorySessionError::SharedSessionQuarantined);
        }
        if is_host_backing_profile::<P>() {
            self.host_backing_activity_started = true;
        }
        let layout = profile_layout::<P>(requested_bytes)?;
        let record_slot = if self.allocations.len() < MAX_SHARED_GTT_ALLOCATIONS_V1 {
            self.allocations.len()
        } else {
            self.allocations
                .iter()
                .position(SharedAllocationRecord::is_fully_released)
                .ok_or(MemorySessionError::SharedAllocationCapacity {
                    maximum: MAX_SHARED_GTT_ALLOCATIONS_V1,
                })?
        };
        let new_total = self
            .retained_gpu_va_bytes
            .checked_add(layout.gpu_va_bytes)
            .ok_or(MemorySessionError::SizeOverflow)?;
        if new_total > MAX_SHARED_GTT_GPU_VA_BYTES_V1 {
            return Err(MemorySessionError::SharedVaCapacity {
                maximum_bytes: MAX_SHARED_GTT_GPU_VA_BYTES_V1,
            });
        }
        let id = self.next_id;
        let next_id = id.checked_add(1).ok_or(MemorySessionError::SizeOverflow)?;
        let reservation = if is_host_backing_profile::<P>() {
            self.host_backing_account
                .as_ref()
                .map(|account| {
                    let (device, vm) = account.domain();
                    account.reserve(self.session_id, device, vm, id, 1, layout)
                })
                .transpose()
                .map_err(host_backing_accounting_error)?
        } else {
            None
        };
        self.check_currentness()?;
        let reservation_bytes =
            usize::try_from(layout.gpu_va_bytes).map_err(|_| MemorySessionError::SizeOverflow)?;

        // Reserve the existing bounded slot and VA budget before the first
        // native attempt. Failure retains this slot; no second attempt may run.
        self.pending_allocation = Some(PendingSharedAllocationV1 {
            id,
            profile: P::PROFILE,
            layout,
            record_slot,
            stage: PendingAllocationStageV1::ReserveVa,
            reservation: None,
            allocation_output: None,
            mapping: None,
            host_backing_charge: reservation.map(|reservation| reservation.retain()),
        });
        self.next_id = next_id;
        self.retained_gpu_va_bytes = new_total;

        let reservation = self.backend.reserve_va(reservation_bytes)?;
        self.pending_allocation_mut().reservation = Some(reservation);
        self.pending_allocation_mut().stage = PendingAllocationStageV1::ValidateReservation;
        let gpu_va = B::reservation_address(
            self.pending_allocation_mut()
                .reservation
                .as_ref()
                .expect("returned reservation"),
        );
        validate_gpu_va_range(gpu_va, layout.gpu_va_bytes, self.backend.gpuvm_aperture())?;
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.gpu_va_bytes,
                    record.gpu_va,
                    record.layout.gpu_va_bytes,
                )
        }) || self.device_memory.iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.gpu_va_bytes,
                    record.gpu_va,
                    record.layout.backing_bytes,
                )
        }) {
            return Err(MemorySessionError::KernelResultMalformed(
                "overlapping GPU VA reservation",
            ));
        }
        if P::IS_USERPTR {
            self.pending_allocation_mut().stage = PendingAllocationStageV1::PrepareUserptr;
            let pending = self
                .pending_allocation
                .as_mut()
                .expect("rooted native attempt");
            let mapping = self.backend.prepare_userptr(
                pending.reservation.as_mut().expect("returned reservation"),
                layout.cpu_mapping_bytes,
            )?;
            pending.mapping = Some(mapping);
            pending.stage = PendingAllocationStageV1::CheckUserptr;
            self.check_currentness()?;
        }
        self.pending_allocation_mut().stage = PendingAllocationStageV1::Allocate;
        let outcome = if P::IS_USERPTR {
            self.backend
                .alloc_userptr(gpu_va, layout.gpu_va_bytes, P::FLAGS)
        } else {
            self.backend.alloc(gpu_va, layout.gpu_va_bytes, P::FLAGS)
        };
        let args = outcome.value;
        self.pending_allocation_mut().allocation_output = Some(args);
        outcome.result?;
        self.pending_allocation_mut().stage = PendingAllocationStageV1::ValidateAllocation;
        if args.va_addr != gpu_va
            || args.size != layout.gpu_va_bytes
            || args.gpu_id != self.backend.gpu_id()
            || args.flags != P::FLAGS.bits()
            || args.handle == 0
            || (!P::IS_USERPTR
                && (args.mmap_offset == 0
                    || !args
                        .mmap_offset
                        .is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)))
        {
            return Err(MemorySessionError::KernelResultMalformed(
                "shared ALLOC_MEMORY_OF_GPU output",
            ));
        }
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && (record.handle == Some(args.handle)
                    || (!P::IS_USERPTR
                        && !record.userptr
                        && record.mmap_offset == args.mmap_offset))
        }) || self.device_memory.iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
                && (record.handle == Some(args.handle)
                    || (!P::IS_USERPTR && record.mmap_offset == args.mmap_offset))
        }) {
            return Err(MemorySessionError::KernelResultMalformed(
                "shared allocation handle or mmap-offset collision",
            ));
        }
        self.pending_allocation_mut().stage = PendingAllocationStageV1::CheckAllocation;
        self.check_currentness()?;
        if !P::IS_USERPTR {
            self.pending_allocation_mut().stage = PendingAllocationStageV1::MapCpu;
            let pending = self
                .pending_allocation
                .as_mut()
                .expect("rooted native attempt");
            let mapping = self.backend.map_cpu(
                pending.reservation.as_mut().expect("returned reservation"),
                args.mmap_offset,
                layout.cpu_mapping_bytes,
            )?;
            pending.mapping = Some(mapping);
            pending.stage = PendingAllocationStageV1::PrepareCpuMapping;
            self.backend
                .prepare_cpu_mapping(pending.mapping.as_mut().expect("returned mapping"))?;
        }
        self.pending_allocation_mut().stage = PendingAllocationStageV1::ValidateMapping;
        if B::mapping_address(
            self.pending_allocation_mut()
                .mapping
                .as_ref()
                .expect("returned mapping"),
        ) != gpu_va
        {
            return Err(MemorySessionError::KernelResultMalformed(
                "shared identity CPU/GPU VA mapping",
            ));
        }
        self.pending_allocation_mut().stage = PendingAllocationStageV1::CheckMapping;
        self.check_currentness()?;

        // All capacity is preallocated and every check/native call is complete.
        // Move the same returned owners into the reserved record slot.
        let pending = self
            .pending_allocation
            .take()
            .expect("validated native attempt");
        let record = SharedAllocationRecord {
            id: pending.id,
            generation: 1,
            profile: pending.profile,
            layout: pending.layout,
            gpu_va,
            mmap_offset: args.mmap_offset,
            userptr: P::IS_USERPTR,
            reservation: pending.reservation,
            mapping: pending.mapping,
            handle: Some(args.handle),
            free_attempted: false,
            phase: SharedAllocationPhaseV1::CpuWritable,
            host_backing_charge: pending.host_backing_charge,
        };
        if pending.record_slot == self.allocations.len() {
            self.allocations.push(record);
        } else {
            self.allocations[pending.record_slot] = record;
        }
        self.allocation_record_slots.insert(id, pending.record_slot);
        Ok(SharedGttAllocationV1 {
            session_id: self.session_id,
            id,
            generation: 1,
            layout,
            marker: PhantomData,
        })
    }
}
