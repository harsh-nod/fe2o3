//! Native initialization conversion preserves indexed ownership on every outcome.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::DirectionalSdmaOpsV1;
use super::*;
use fe2o3_kfd::{
    Gfx942PersistentComputeInitializedStorageV1, Gfx942PersistentComputeStorageIneligibilityV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug)]
pub(super) enum InitializedStorageOwnerV1 {
    Native(Gfx942PersistentComputeInitializedStorageV1),
    #[cfg(test)]
    Scripted(DirectionalSdmaDeviceOwnerV1),
}

impl InitializedStorageOwnerV1 {
    pub(super) fn extents(&self) -> (u64, u64) {
        match self {
            Self::Native(ready) => (ready.byte_len(), ready.physical_byte_len()),
            #[cfg(test)]
            Self::Scripted(device) => {
                let bytes = device
                    .scripted_bytes()
                    .expect("scripted storage owner")
                    .len() as u64;
                (bytes, bytes)
            }
        }
    }

    pub(super) fn normalize(self) -> DirectionalSdmaDeviceOwnerV1 {
        match self {
            Self::Native(ready) => DirectionalSdmaDeviceOwnerV1::Native(ready.into_allocation()),
            #[cfg(test)]
            Self::Scripted(device) => device,
        }
    }

    pub(super) fn into_input(self) -> KfdRuntimePersistentComputeInputV1 {
        match self {
            Self::Native(ready) => KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::InitializedStorage(ready),
            ),
            #[cfg(test)]
            Self::Scripted(device) => KfdRuntimePersistentComputeInputV1::ScriptedStorage(device),
        }
    }
}

pub(super) enum InitializedStorageAttemptV1 {
    Promoted(InitializedStorageOwnerV1),
    NotEligible {
        device: DirectionalSdmaDeviceOwnerV1,
        reason: Gfx942PersistentComputeStorageIneligibilityV1,
    },
}

fn full_storage_binding_shape_v1(
    binding: &BackendBindingV1,
    stream_device: u64,
    allocation: &AllocationRecordV1,
) -> bool {
    allocation.device == stream_device
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.sdma_backed
        && allocation.sdma_initialized
        && allocation.native_dirty.is_empty()
        && !allocation.bytes.is_empty()
        && binding.region.byte_offset == 0
        && binding.region.byte_len == allocation.bytes.len() as u64
}

pub(super) fn initialized_storage_full_range_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocation: Option<&AllocationRecordV1>,
) -> Option<PersistentFullRangeComputeAdmissionV1> {
    let [binding] = bindings else {
        return None;
    };
    let allocation = allocation?;
    let KfdRuntimeSdmaStorageV1::InitializedStorage(ready) = &allocation.sdma_storage else {
        return None;
    };
    let logical = allocation.bytes.len() as u64;
    (semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && full_storage_binding_shape_v1(binding, stream_device, allocation)
        && ready.extents() == (logical, logical))
        .then_some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source: PersistentFullRangeComputeSourceV1::InitializedStorage,
        })
}

impl KfdRuntimeBackendV1 {
    pub(super) fn three_binding_storage_candidates_admissible_v1(
        &self,
        launch: BackendLaunchV1<'_>,
    ) -> bool {
        let Some(&device) = self.streams.get(&launch.stream) else {
            return false;
        };
        if !compute_dispatch::three_binding_persistent_compute_shape_v1(
            launch.semantic_launch,
            launch.bindings,
            device,
            &self.allocations,
        ) {
            return false;
        }
        let candidates = self.initialized_storage_candidates_v1(launch);
        launch.bindings.iter().all(|binding| {
            candidates.contains(&Some(binding.region.allocation))
                || self
                    .allocations
                    .get(&binding.region.allocation)
                    .is_some_and(|allocation| {
                        compute_dispatch::three_binding_persistent_ready_source_v1(allocation)
                            .is_some()
                    })
        })
    }

    pub(super) fn initialized_storage_candidates_v1(
        &self,
        launch: BackendLaunchV1<'_>,
    ) -> [Option<u64>; 3] {
        let Some(&device) = self.streams.get(&launch.stream) else {
            return [None; 3];
        };
        if launch.semantic_launch != KfdRuntimeSemanticLaunchV1::Ordinary
            || !(launch.bindings.len() == 1
                || compute_dispatch::three_binding_persistent_compute_shape_v1(
                    launch.semantic_launch,
                    launch.bindings,
                    device,
                    &self.allocations,
                ))
        {
            return [None; 3];
        }
        std::array::from_fn(|index| {
            let binding = launch.bindings.get(index)?;
            let allocation = self.allocations.get(&binding.region.allocation)?;
            if !full_storage_binding_shape_v1(binding, device, allocation) {
                return None;
            }
            let KfdRuntimeSdmaStorageV1::Device(owner) = &allocation.sdma_storage else {
                return None;
            };
            let extents = match owner.as_ref() {
                DirectionalSdmaDeviceOwnerV1::Native(owner) => {
                    (owner.byte_len(), owner.physical_byte_len())
                }
                #[cfg(test)]
                owner @ DirectionalSdmaDeviceOwnerV1::Scripted(_) => {
                    let bytes = owner.scripted_bytes().unwrap().len() as u64;
                    (bytes, bytes)
                }
            };
            let logical = allocation.bytes.len() as u64;
            (extents == (logical, logical)).then_some(binding.region.allocation)
        })
    }

    pub(super) fn restore_initialized_storage_input_v1(
        &mut self,
        allocation: u64,
        submission: u64,
        input: KfdRuntimePersistentComputeInputV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let valid = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == submission)
                && record.persistent_storage_restore.as_ref().is_some_and(|shell| match &input {
                    KfdRuntimePersistentComputeInputV1::Native(Gfx942PersistentComputeInputV1::InitializedStorage(_)) => shell.initialized.is_some(),
                    KfdRuntimePersistentComputeInputV1::Native(Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_)) => shell.replay.is_some(),
                    #[cfg(test)]
                    KfdRuntimePersistentComputeInputV1::ScriptedStorage(_) => shell.initialized.is_some(),
                    #[cfg(test)]
                    KfdRuntimePersistentComputeInputV1::ScriptedReplay(_) => shell.device.is_some(),
                    _ => false,
                })
        });
        if !valid {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(input),
            );
            return Err(
                self.terminal_error("initialized-storage restore slot/input/shell mismatch")
            );
        }
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("preflighted storage restoration");
        let shell = record.persistent_storage_restore.take().unwrap();
        record.sdma_storage = match input {
            KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::InitializedStorage(ready),
            ) => KfdRuntimeSdmaStorageV1::InitializedStorage(fill_restore_shell_v1(
                shell.initialized.unwrap(),
                InitializedStorageOwnerV1::Native(ready),
            )),
            KfdRuntimePersistentComputeInputV1::Native(
                input @ Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_),
            ) => KfdRuntimeSdmaStorageV1::PersistentReplay(fill_restore_shell_v1(
                shell.replay.unwrap(),
                input,
            )),
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) => {
                KfdRuntimeSdmaStorageV1::InitializedStorage(fill_restore_shell_v1(
                    shell.initialized.unwrap(),
                    InitializedStorageOwnerV1::Scripted(device),
                ))
            }
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => {
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(
                    shell.device.unwrap(),
                    device,
                ))
            }
            _ => unreachable!("preflighted initialized-storage input"),
        };
        Ok(())
    }

    /// Some(reason) is only the native API's typed, clean ineligibility outcome.
    // Keep failure custody inline; recovery must not require another allocation.
    #[allow(clippy::result_large_err)]
    pub(super) fn convert_initialized_storage_v1(
        &mut self,
        allocation: u64,
    ) -> Result<
        Option<Gfx942PersistentComputeStorageIneligibilityV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let ready_shell = try_uninit_box_v1()
            .map_err(|_| Self::capacity("KFD initialized-storage input allocation failed"))?;
        let record = self.allocations.get_mut(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "initialized-storage allocation disappeared",
            )
        })?;
        let storage = core::mem::replace(
            &mut record.sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
        );
        let KfdRuntimeSdmaStorageV1::Device(device) = storage else {
            record.sdma_storage = storage;
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "initialized-storage conversion lost its device owner",
            ));
        };
        let (device, device_shell) = take_restore_shell_v1(device);
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(device));
        let result = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
                DirectionalSdmaOpsV1::Scripted(driver)
            } else {
                DirectionalSdmaOpsV1::Native(
                    self.queue
                        .as_mut()
                        .expect("storage conversion retains queue"),
                )
            };
            #[cfg(not(test))]
            let mut ops = DirectionalSdmaOpsV1::Native(
                self.queue
                    .as_mut()
                    .expect("storage conversion retains queue"),
            );
            let Some(KfdRuntimeTerminalSdmaCustodyV1::Device(device)) =
                self.terminal_sdma_custody.take()
            else {
                unreachable!("installed storage conversion input")
            };
            ops.promote_initialized_storage(device)
        }));
        let result = match result {
            Ok(result) => result,
            Err(payload) => super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.poison_terminal_v1()
            }),
        };
        let (storage, ineligible, diagnostic) = match result {
            Ok(InitializedStorageAttemptV1::Promoted(ready)) => (
                KfdRuntimeSdmaStorageV1::InitializedStorage(fill_restore_shell_v1(
                    ready_shell,
                    ready,
                )),
                None,
                None,
            ),
            Ok(InitializedStorageAttemptV1::NotEligible { device, reason }) => (
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(device_shell, device)),
                Some(reason),
                None,
            ),
            Err(SdmaTransitionFailureV1::Retryable { detail, custody }) => (
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(device_shell, custody)),
                None,
                Some(detail),
            ),
            Err(SdmaTransitionFailureV1::ProcessTeardown { detail, custody }) => {
                self.retain_sdma_seam_terminal_v1(custody);
                self.poison_terminal_v1();
                return Err(
                    self.terminal_error(format!("KFD initialized-storage conversion: {detail}"))
                );
            }
        };
        let slot_matches = self.allocations.get(&allocation).is_some_and(|record| {
            matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
            )
        });
        if !slot_matches {
            match storage {
                KfdRuntimeSdmaStorageV1::Device(device) => self.retain_terminal_sdma_custody_v1(
                    KfdRuntimeTerminalSdmaCustodyV1::Device(*device),
                ),
                KfdRuntimeSdmaStorageV1::InitializedStorage(ready) => self
                    .retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::InitializedStorage(*ready),
                    ),
                _ => unreachable!("conversion returns exact typed storage"),
            }
            return Err(
                self.terminal_error("initialized-storage conversion restoration slot changed")
            );
        }
        let record = self
            .allocations
            .get_mut(&allocation)
            .expect("preflighted storage slot");
        if matches!(storage, KfdRuntimeSdmaStorageV1::InitializedStorage(_)) {
            record.content_sha256 = None;
            record.last_full_host_write = None;
        }
        record.sdma_storage = storage;
        // Restore ownership before diagnostic formatting, which can unwind.
        if let Some(detail) = diagnostic {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Native,
                format!("KFD initialized-storage conversion: {detail}"),
            ));
        }
        Ok(ineligible)
    }
}
