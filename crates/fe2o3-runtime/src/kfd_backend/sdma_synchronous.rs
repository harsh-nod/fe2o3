//! Runtime custody spans normalization, the fused lower copy and retirement.

#![forbid(unsafe_code)]

use super::kfd_backend_sdma_seam::{
    DirectionalSdmaOpsV1, SdmaOwnerDiagnosticV1, SdmaTerminalCustodyV1,
    retire_native_directional_completed_v1,
};
use super::sdma_host_write::resume_sdma_owner_panic_v1;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[allow(dead_code, clippy::large_enum_variant)]
pub(super) enum SynchronousSdmaPhaseV1 {
    Host(SdmaBufferOwnerV1),
    Pair(DirectionalSdmaPairOwnerV1),
    LowerOwned,
    Pending(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
    RetiredPair(DirectionalSdmaPairOwnerV1),
    Terminal(SdmaTerminalCustodyV1),
}

pub(super) struct SynchronousSdmaCustodyV1 {
    pub(super) allocation: u64,
    pub(super) direction: Gfx942PersistentSdmaDirectionV1,
    pub(super) request: DirectionalSdmaCopyRequestV1,
    pub(super) operation: &'static str,
    pub(super) shell: Option<Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>>,
    pub(super) phase: SynchronousSdmaPhaseV1,
}

impl SynchronousSdmaCustodyV1 {
    fn take_pair(&mut self) -> DirectionalSdmaPairOwnerV1 {
        assert!(matches!(
            self.phase,
            SynchronousSdmaPhaseV1::Pair(_) | SynchronousSdmaPhaseV1::RetiredPair(_)
        ));
        match std::mem::replace(&mut self.phase, SynchronousSdmaPhaseV1::LowerOwned) {
            SynchronousSdmaPhaseV1::Pair(pair) | SynchronousSdmaPhaseV1::RetiredPair(pair) => pair,
            _ => unreachable!("checked synchronous pair"),
        }
    }

    fn take_completed(&mut self) -> DirectionalSdmaCompletedOwnerV1 {
        assert!(matches!(self.phase, SynchronousSdmaPhaseV1::Completed(_)));
        let SynchronousSdmaPhaseV1::Completed(completed) =
            std::mem::replace(&mut self.phase, SynchronousSdmaPhaseV1::LowerOwned)
        else {
            unreachable!("checked synchronous completion");
        };
        completed
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn synchronous_copy_root_v1(&mut self) -> &mut SynchronousSdmaCustodyV1 {
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(root)) =
            &mut self.terminal_sdma_custody
        else {
            unreachable!("synchronous copy retains its runtime root");
        };
        root
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_synchronous_directional_sdma_v1(
        &mut self,
        allocation: u64,
        direction: Gfx942PersistentSdmaDirectionV1,
        host: SdmaBufferOwnerV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        operation: &'static str,
    ) -> Result<SdmaBufferOwnerV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(
            SynchronousSdmaCustodyV1 {
                allocation,
                direction,
                request: DirectionalSdmaCopyRequestV1 {
                    host_offset,
                    device_offset,
                    copy_bytes,
                },
                operation,
                shell: None,
                phase: SynchronousSdmaPhaseV1::Host(host),
            },
        ));
        let result = catch_unwind(AssertUnwindSafe(|| {
            if let Err(device) = self.try_normalize_h2d_ready_v1(allocation) {
                let root = self.synchronous_copy_root_v1();
                let SynchronousSdmaPhaseV1::Host(host) =
                    std::mem::replace(&mut root.phase, SynchronousSdmaPhaseV1::LowerOwned)
                else {
                    unreachable!("normalization retains the host input");
                };
                root.phase =
                    SynchronousSdmaPhaseV1::Pair(DirectionalSdmaPairOwnerV1 { device, host });
                return Err(self.terminal_error(
                    "persistent-compute ready normalization slot changed unexpectedly",
                ));
            }
            if !self.allocations.get(&allocation).is_some_and(|record| {
                record.kind == RuntimeMemoryKindV1::DeviceLocal
                    && matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Device(_))
            }) {
                let host = self.take_synchronous_host_v1();
                self.recycle_transient_sdma_buffer_v1(host, operation)?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "persistent device allocation is retained by pending work",
                ));
            }
            self.extract_synchronous_device_v1();
            let result = self.run_synchronous_lower_v1();
            self.settle_synchronous_execution_v1(result, |detail| detail.to_string())?;
            self.retire_synchronous_completion_v1()?;
            self.restore_synchronous_pair_v1()?;
            Ok(self.take_synchronous_host_v1())
        }));
        match result {
            Ok(result) => result,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || {
                if !self.terminal {
                    self.poison_terminal_v1();
                }
            }),
        }
    }

    pub(super) fn extract_synchronous_device_v1(&mut self) {
        let root = self.synchronous_copy_root_v1();
        assert!(matches!(root.phase, SynchronousSdmaPhaseV1::Host(_)));
        assert!(root.shell.is_none());
        let allocation = root.allocation;
        let storage = &mut self.allocations.get_mut(&allocation).unwrap().sdma_storage;
        assert!(matches!(storage, KfdRuntimeSdmaStorageV1::Device(_)));
        let KfdRuntimeSdmaStorageV1::Device(device) = std::mem::replace(
            storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
        ) else {
            unreachable!("checked synchronous device");
        };
        let (device, shell) = take_restore_shell_v1(device);
        let root = self.synchronous_copy_root_v1();
        let SynchronousSdmaPhaseV1::Host(host) =
            std::mem::replace(&mut root.phase, SynchronousSdmaPhaseV1::LowerOwned)
        else {
            unreachable!("checked synchronous host");
        };
        root.shell = Some(shell);
        root.phase = SynchronousSdmaPhaseV1::Pair(DirectionalSdmaPairOwnerV1 { device, host });
    }

    #[allow(
        clippy::result_large_err,
        reason = "returned owners reach runtime custody before diagnostics or allocation"
    )]
    pub(super) fn run_synchronous_lower_v1(
        &mut self,
    ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaSynchronousExecutionFailureV1> {
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(root)) =
            &mut self.terminal_sdma_custody
        else {
            unreachable!("installed synchronous input");
        };
        assert!(matches!(root.phase, SynchronousSdmaPhaseV1::Pair(_)));
        #[cfg(test)]
        let mut ops = if let Some(driver) = self.scripted_sdma.as_mut() {
            DirectionalSdmaOpsV1::Scripted(driver)
        } else {
            DirectionalSdmaOpsV1::Native(
                self.queue.as_mut().expect("synchronous copy retains queue"),
            )
        };
        #[cfg(not(test))]
        let mut ops = DirectionalSdmaOpsV1::Native(
            self.queue.as_mut().expect("synchronous copy retains queue"),
        );
        let pair = root.take_pair();
        ops.execute_synchronous_single(pair, root.direction, root.request, Duration::from_secs(30))
    }

    pub(super) fn settle_synchronous_execution_v1(
        &mut self,
        result: Result<
            DirectionalSdmaCompletedOwnerV1,
            DirectionalSdmaSynchronousExecutionFailureV1,
        >,
        diagnostic: impl FnOnce(&SdmaOwnerDiagnosticV1) -> String,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let root = self.synchronous_copy_root_v1();
            let operation = root.operation;
            match result {
                Ok(completed) => root.phase = SynchronousSdmaPhaseV1::Completed(completed),
                Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                    detail,
                    pair,
                }) => {
                    root.phase = SynchronousSdmaPhaseV1::Pair(pair);
                    self.restore_synchronous_pair_v1()?;
                    let host = self.take_synchronous_host_v1();
                    self.recycle_transient_sdma_buffer_v1(host, operation)?;
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("KFD {operation} publication: {}", diagnostic(&detail)),
                    ));
                }
                Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                    detail,
                    submission,
                }) => {
                    root.phase = SynchronousSdmaPhaseV1::Pending(submission);
                    return Err(self.terminal_error(format!(
                        "KFD {operation} completion became ambiguous: {}",
                        diagnostic(&detail)
                    )));
                }
                Err(DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
                    detail,
                    custody,
                }) => {
                    root.phase = SynchronousSdmaPhaseV1::Terminal(custody);
                    return Err(self.terminal_error(format!(
                        "KFD {operation} execution became ambiguous: {}",
                        diagnostic(&detail)
                    )));
                }
            }
            let root = self.synchronous_copy_root_v1();
            let SynchronousSdmaPhaseV1::Completed(completed) = &root.phase else {
                unreachable!("installed synchronous completion");
            };
            if completed.direction() != root.direction
                || completed.host_offset() != root.request.host_offset
                || completed.device_offset() != root.request.device_offset
                || completed.copy_bytes() != root.request.copy_bytes
                || completed.packet_count() != 1
            {
                return Err(self.terminal_error(format!(
                    "KFD {operation} completion metadata changed unexpectedly"
                )));
            }
            Ok(())
        }));
        match outcome {
            Ok(result) => result,
            Err(payload) => resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1()),
        }
    }

    pub(super) fn retire_synchronous_completion_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(root)) =
            &mut self.terminal_sdma_custody
        else {
            unreachable!("installed synchronous completion");
        };
        #[cfg(test)]
        if matches!(
            root.phase,
            SynchronousSdmaPhaseV1::Completed(DirectionalSdmaCompletedOwnerV1::Scripted(_))
        ) {
            let driver = self
                .scripted_sdma
                .as_mut()
                .expect("synchronous retirement retains driver");
            let result = DirectionalSdmaOpsV1::Scripted(driver).retire(root.take_completed());
            return self.settle_synchronous_retirement_v1(result);
        }
        let parts = match root.take_completed() {
            DirectionalSdmaCompletedOwnerV1::NativeSingle { completed, .. } => {
                completed.into_parts()
            }
            DirectionalSdmaCompletedOwnerV1::NativeWindow { completed } => completed.into_parts(),
            #[cfg(test)]
            DirectionalSdmaCompletedOwnerV1::Scripted(_) => {
                unreachable!("handled scripted completion")
            }
        };
        // Frontier retirement is a pure owner transition, independent of the queue driver.
        let result = retire_native_directional_completed_v1(parts);
        self.settle_synchronous_retirement_v1(result)
    }

    pub(super) fn settle_synchronous_retirement_v1(
        &mut self,
        result: Result<
            DirectionalSdmaPairOwnerV1,
            SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1, SdmaOwnerDiagnosticV1>,
        >,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.synchronous_copy_root_v1();
        let operation = root.operation;
        match result {
            Ok(pair) => {
                root.phase = SynchronousSdmaPhaseV1::RetiredPair(pair);
                Ok(())
            }
            Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => {
                root.phase = SynchronousSdmaPhaseV1::Completed(custody);
                Err(self.terminal_error(format!("KFD {operation} frontier retirement failed")))
            }
            Err(SdmaTransitionFailureV1::ProcessTeardown { custody, .. }) => {
                root.phase = SynchronousSdmaPhaseV1::Terminal(custody);
                Err(self.terminal_error(format!("KFD {operation} frontier retirement failed")))
            }
        }
    }

    pub(super) fn restore_synchronous_pair_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let allocation = self.synchronous_copy_root_v1().allocation;
        if !self.allocations.get(&allocation).is_some_and(|record| {
            record.kind == RuntimeMemoryKindV1::DeviceLocal
                && matches!(
                    record.sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
                )
        }) {
            return Err(self.terminal_error(
                "synchronous directional SDMA restoration slot changed unexpectedly",
            ));
        }
        let root = self.synchronous_copy_root_v1();
        assert!(root.shell.is_some());
        let pair = root.take_pair();
        let shell = root.shell.take().unwrap();
        root.phase = SynchronousSdmaPhaseV1::Host(pair.host);
        self.allocations.get_mut(&allocation).unwrap().sdma_storage =
            KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(shell, pair.device));
        Ok(())
    }

    pub(super) fn take_synchronous_host_v1(&mut self) -> SdmaBufferOwnerV1 {
        let root = self.synchronous_copy_root_v1();
        assert!(matches!(root.phase, SynchronousSdmaPhaseV1::Host(_)));
        assert!(root.shell.is_none());
        let Some(KfdRuntimeTerminalSdmaCustodyV1::Synchronous(SynchronousSdmaCustodyV1 {
            phase: SynchronousSdmaPhaseV1::Host(host),
            ..
        })) = self.terminal_sdma_custody.take()
        else {
            unreachable!("checked synchronous host return");
        };
        host
    }
}
