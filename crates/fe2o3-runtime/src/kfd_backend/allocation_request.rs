//! Shared allocation implementation after request-policy authentication.

use super::*;

impl KfdRuntimeBackendV1 {
    pub(super) fn allocate_request_backing_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<
        RuntimeBackendAllocationOutcomeV1<KfdRuntimeBackendErrorV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_live()?;
        self.require_device(device)?;
        if kind == RuntimeMemoryKindV1::DeviceLocal {
            self.require_default_dispatch_capacity_v1()?;
        }
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "allocation length and power-of-two alignment must be nonzero",
            ));
        }
        if kind == RuntimeMemoryKindV1::DeviceLocal && alignment > HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "device-local KFD allocation alignment exceeds 4096 bytes",
            ));
        }
        if kind == RuntimeMemoryKindV1::HostVisible && alignment > HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "host-visible KFD allocation alignment exceeds the admitted page alignment",
            ));
        }
        if byte_len > self.staging_budgets.max_allocation_bytes {
            return Err(Self::capacity(
                "allocation exceeds the direct-KFD per-allocation staging budget",
            ));
        }
        let next_staged_context_bytes = self
            .staged_context_bytes
            .checked_add(byte_len)
            .filter(|total| *total <= self.staging_budgets.max_context_bytes)
            .ok_or_else(|| {
                Self::capacity("allocation exceeds the direct-KFD context staging budget")
            })?;
        let len = usize::try_from(byte_len)
            .map_err(|_| Self::capacity("allocation does not fit host staging address space"))?;
        self.allocations
            .try_reserve(1)
            .map_err(|_| Self::capacity("KFD allocation-table growth failed"))?;
        let bytes = try_zeroed_staging_v1(len)?;
        let id = self.next_id()?;
        let sdma_storage = if self.native_available {
            let ready_before = self.sdma_allocation_ready_v1();
            self.ensure_sdma_queue_v1()?;
            let buffer = match self.allocate_sdma_owner_v1(
                kind,
                len,
                alignment,
                ready_before,
                "KFD persistent SDMA allocation",
            ) {
                Ok(buffer) => buffer,
                // This helper reports cold Quiescent only after the lower typed
                // RetryableCapacity disposition established settled empty custody.
                // Queue creation and later initialization failures cannot mint it.
                Err(RuntimeBackendFailureV1::Quiescent(error)) if !ready_before => {
                    return Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error));
                }
                Err(failure) => return Err(failure),
            };
            match kind {
                RuntimeMemoryKindV1::HostVisible => {
                    let buffer = self.initialize_sdma_host_v1(
                        buffer,
                        &bytes,
                        "KFD persistent host allocation initialization",
                    )?;
                    KfdRuntimeSdmaStorageV1::Host(buffer)
                }
                RuntimeMemoryKindV1::DeviceLocal => KfdRuntimeSdmaStorageV1::Device(Box::new(
                    self.promote_sdma_buffer_v1(buffer)
                        .map_err(Self::after_possible_host_mutation)?,
                )),
            }
        } else {
            KfdRuntimeSdmaStorageV1::Synthetic
        };
        let sdma_initialized = !self.native_available || kind == RuntimeMemoryKindV1::HostVisible;
        self.allocations.insert(
            id,
            AllocationRecordV1 {
                device,
                kind,
                alignment,
                bytes: bytes.into(),
                content_sha256: None,
                last_full_host_write: None,
                native_dirty: Vec::new(),
                sdma_storage,
                sdma_backed: self.native_available,
                sdma_initialized,
                sdma_shadow_dirty: false,
                #[cfg(test)]
                scripted_three_binding_replay: false,
            },
        );
        self.staged_context_bytes = next_staged_context_bytes;
        if self.native_available && kind == RuntimeMemoryKindV1::DeviceLocal {
            if let Err(failure) = self.zero_sdma_range_v1(id, byte_len) {
                if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
                    return Err(failure);
                }
                if let Err(cleanup) = self.discard_hidden_sdma_allocation_v1(id) {
                    return match cleanup {
                        failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                        RuntimeBackendFailureV1::Rejected(_)
                        | RuntimeBackendFailureV1::Quiescent(_) => Err(self.terminal_error(
                            "hidden KFD allocation cleanup retained unreachable native custody",
                        )),
                    };
                }
                return Err(Self::after_possible_host_mutation(failure));
            }
            self.allocations
                .get_mut(&id)
                .expect("initialized device allocation remains indexed")
                .sdma_initialized = true;
        }
        let allocation = self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, id);
        self.observe_profile_v1(allocation.map(|allocation| {
            KfdRuntimeProfileEventKindV1::AllocationCreated {
                allocation,
                memory_kind: match kind {
                    RuntimeMemoryKindV1::HostVisible => KfdProfileMemoryKindV1::HostVisible,
                    RuntimeMemoryKindV1::DeviceLocal => {
                        KfdProfileMemoryKindV1::DeviceLocalHostStaged
                    }
                },
                byte_len,
                alignment,
            }
        }));
        Ok(RuntimeBackendAllocationOutcomeV1::Allocated(id))
    }
}
