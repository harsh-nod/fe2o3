//! Queue-rooted fresh SDMA allocation through native mapping and model retake.

#![forbid(unsafe_code)]

use super::*;
use crate::shared_memory::{
    CoherentAllocationCustodyV1, DeviceAllocationCustodyV1, SharedMemorySessionPhaseV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) enum SdmaAllocationRequestV1 {
    Host(usize),
    Device { bytes: u64, alignment: u64 },
}

pub(super) enum SdmaAllocationCustodyV1 {
    Host {
        bytes: usize,
        allocation: CoherentAllocationCustodyV1,
    },
    Device {
        logical_bytes: u64,
        alignment: u64,
        allocation: DeviceAllocationCustodyV1,
    },
}

impl SdmaAllocationCustodyV1 {
    fn new(request: SdmaAllocationRequestV1) -> Self {
        match request {
            SdmaAllocationRequestV1::Host(bytes) => Self::Host {
                bytes,
                allocation: CoherentAllocationCustodyV1::new(),
            },
            SdmaAllocationRequestV1::Device { bytes, alignment } => Self::Device {
                logical_bytes: bytes,
                alignment,
                allocation: DeviceAllocationCustodyV1::new(),
            },
        }
    }

    fn prepare(
        &mut self,
        memory: &mut impl SdmaAllocationMemoryV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Host { bytes, allocation } => {
                memory
                    .prepare_host(allocation, *bytes)
                    .map_err(Gfx942SdmaErrorV1::from)?;
            }
            Self::Device {
                logical_bytes,
                alignment,
                allocation,
            } => {
                // Preserve validation inside the original loan/retake boundary.
                let (_, physical_bytes) =
                    crate::sdma::device_buffer_allocation_extents_v1(*logical_bytes, *alignment)?;
                memory
                    .prepare_device(allocation, physical_bytes, *alignment)
                    .map_err(Gfx942SdmaErrorV1::from)?;
            }
        }
        Ok(())
    }

    fn requires_retention(&self) -> bool {
        match self {
            Self::Host { allocation, .. } => allocation.requires_retention(),
            Self::Device { allocation, .. } => allocation.requires_retention(),
        }
    }

    fn take_complete(
        &mut self,
        owner: QueueKeyV1,
    ) -> Result<Gfx942SdmaBufferV1, MemorySessionError> {
        let (storage, logical_bytes) = match self {
            Self::Host { bytes, allocation } => (
                Gfx942SdmaBufferStorageV1::Host(allocation.take_complete()?),
                *bytes as u64,
            ),
            Self::Device {
                logical_bytes,
                allocation,
                ..
            } => (
                Gfx942SdmaBufferStorageV1::Device(allocation.take_complete()?),
                *logical_bytes,
            ),
        };
        Ok(Gfx942SdmaBufferV1::from_bridge_parts(
            storage,
            owner,
            1,
            logical_bytes,
        ))
    }
}

pub(super) trait SdmaAllocationMemoryV1 {
    fn prepare_host(
        &mut self,
        root: &mut CoherentAllocationCustodyV1,
        bytes: usize,
    ) -> Result<(), MemorySessionError>;
    fn prepare_device(
        &mut self,
        root: &mut DeviceAllocationCustodyV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<(), MemorySessionError>;
    fn is_quarantined(&self) -> bool;
}

pub(super) struct SdmaAllocationPartsV1<'a, M> {
    pub(super) memory: &'a mut M,
    pub(super) custody: &'a mut Option<SdmaAllocationCustodyV1>,
    pub(super) outstanding: &'a mut usize,
    pub(super) owner: QueueKeyV1,
}

pub(super) trait SdmaAllocationContextV1 {
    type Memory: SdmaAllocationMemoryV1;
    fn preflight(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn parts(
        &mut self,
    ) -> Result<SdmaAllocationPartsV1<'_, Self::Memory>, ComputeAqlQueueSessionErrorV1>;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn is_terminal(&self) -> bool;
    fn poison(&mut self);
}

fn poison_without_replacing_failure<C: SdmaAllocationContextV1>(context: &mut C) {
    // Terminalization must not destroy either the original or a secondary panic payload.
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

pub(super) fn allocate_in_place<C: SdmaAllocationContextV1>(
    context: &mut C,
    request: SdmaAllocationRequestV1,
) -> Result<Gfx942SdmaBufferV1, ComputeAqlQueueSessionErrorV1> {
    context.preflight()?;
    let parts = context.parts()?;
    if parts.custody.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "unfinished SDMA allocation",
        ));
    }
    let next_outstanding =
        parts
            .outstanding
            .checked_add(1)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA buffer ledger exhausted",
            ))?;
    *parts.custody = Some(SdmaAllocationCustodyV1::new(request));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (operation, retake) = execute_live_model_custody_v1(
            context,
            C::loan,
            |context| -> Result<(), ComputeAqlQueueSessionErrorV1> {
                let parts = context.parts()?;
                parts
                    .custody
                    .as_mut()
                    .expect("installed allocation custody")
                    .prepare(parts.memory)?;
                Ok(())
            },
            C::retake,
            poison_without_replacing_failure,
        )?;
        retake?;
        operation?;
        let parts = context.parts()?;
        let buffer = parts
            .custody
            .as_mut()
            .expect("settled allocation custody")
            .take_complete(parts.owner)
            .map_err(Gfx942SdmaErrorV1::from)?;
        // Only scalar commits and empty-root retirement follow the ownership transfer.
        *parts.outstanding = next_outstanding;
        *parts.custody = None;
        Ok(buffer)
    }));
    match result {
        Ok(Ok(buffer)) => Ok(buffer),
        Ok(Err(error)) => {
            let already_terminal = context.is_terminal();
            let terminal = match context.parts() {
                Ok(parts) => {
                    let terminal = already_terminal
                        || parts.memory.is_quarantined()
                        || parts
                            .custody
                            .as_ref()
                            .is_some_and(SdmaAllocationCustodyV1::requires_retention);
                    if !terminal {
                        // No returned owner or uncertain lower effect exists: preserve retryable rejection.
                        *parts.custody = None;
                    }
                    terminal
                }
                Err(_) => true,
            };
            if terminal {
                poison_without_replacing_failure(context);
            }
            Err(error)
        }
        Err(payload) => {
            poison_without_replacing_failure(context);
            resume_unwind(payload)
        }
    }
}

impl SdmaAllocationMemoryV1 for SharedGttMemorySessionV1 {
    fn prepare_host(
        &mut self,
        root: &mut CoherentAllocationCustodyV1,
        bytes: usize,
    ) -> Result<(), MemorySessionError> {
        self.prepare_coherent_allocation_in_place(root, bytes)
    }
    fn prepare_device(
        &mut self,
        root: &mut DeviceAllocationCustodyV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        self.prepare_device_allocation_in_place(root, bytes, alignment)
    }
    fn is_quarantined(&self) -> bool {
        self.phase() == SharedMemorySessionPhaseV1::Quarantined
    }
}

impl SdmaAllocationContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;
    fn preflight(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.sdma_allocation.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA allocation",
            ));
        }
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled()
    }
    fn parts(
        &mut self,
    ) -> Result<SdmaAllocationPartsV1<'_, Self::Memory>, ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        Ok(SdmaAllocationPartsV1 {
            memory: &mut engine.backend.session,
            custody: &mut self.sdma_allocation,
            outstanding: &mut self.sdma_outstanding_buffers,
            owner: self.key,
        })
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }
    fn is_terminal(&self) -> bool {
        self.terminal_poisoned
    }
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfinished_sdma_allocation_rejects_reentry_teardown_and_drop_aborts() {
        use std::os::unix::process::ExitStatusExt;
        const CHILD: &str = "FE2O3_TEST_SDMA_ALLOCATION_DROP";
        const TEST: &str = "queue::live::sdma_allocation::tests::unfinished_sdma_allocation_rejects_reentry_teardown_and_drop_aborts";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.signal(), Some(6), "{stderr}");
            assert!(stderr.contains("allocation guards checked; dropping retained root"));
            assert!(!stderr.contains("panicked at"));
            return;
        }
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        // This checks the public guard without fabricating a native allocation or queue.
        let mut session = core::mem::ManuallyDrop::new(
            super::super::tests::persistent_compute_cancellation_test_session(
                super::super::tests::test_queue_key(91, 1),
                None,
                None,
            ),
        );
        session.sdma_allocation = Some(SdmaAllocationCustodyV1::new(
            SdmaAllocationRequestV1::Host(17),
        ));
        let ledger = (
            session.sdma_device_pool.activity_started,
            session.sdma_outstanding_buffers,
            session.sdma_pool_reuse_count,
        );
        for result in [
            session.allocate_sdma_host_buffer(17).map(|_| ()),
            session.allocate_sdma_device_buffer(17, 4096).map(|_| ()),
            session.trim_sdma_memory_pool().map(|_| ()),
            session.supports_retained_primary_release_v1().map(|_| ()),
            session.preflight_primary_release_v1(),
            session
                .destroy_queue_and_event(QueueDestroyModeV1::Release)
                .map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "unfinished SDMA allocation"
                ))
            ));
        }
        assert_eq!(
            (
                session.sdma_device_pool.activity_started,
                session.sdma_outstanding_buffers,
                session.sdma_pool_reuse_count
            ),
            ledger
        );
        assert!(session.sdma_allocation.is_some() && session.engine.is_none());
        eprintln!("allocation guards checked; dropping retained root");
        drop(core::mem::ManuallyDrop::into_inner(session));
        panic!("unfinished allocation Drop returned");
    }
}
