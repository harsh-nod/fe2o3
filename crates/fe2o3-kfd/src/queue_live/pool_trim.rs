//! Pool owners remain queue-rooted through native cleanup and model settlement.

#![forbid(unsafe_code)]

use super::*;
use crate::shared_memory::{DataCleanupCustodyV1, DispatchDataReleaseV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) struct SdmaPoolTrimCustodyV1 {
    pub(super) active: Option<DataCleanupCustodyV1>,
    pub(super) released: usize,
}

pub(super) struct PoolTrimPartsV1<'a, M> {
    pub(super) memory: &'a mut M,
    pub(super) free: &'a mut Vec<Gfx942SdmaBufferV1>,
    pub(super) custody: &'a mut Option<SdmaPoolTrimCustodyV1>,
}

pub(super) trait PoolTrimContextV1 {
    type Memory: DispatchDataReleaseV1;
    fn parts(&mut self)
    -> Result<PoolTrimPartsV1<'_, Self::Memory>, ComputeAqlQueueSessionErrorV1>;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison(&mut self);
}

pub(super) fn trim_in_place<C: PoolTrimContextV1>(
    context: &mut C,
) -> Result<usize, ComputeAqlQueueSessionErrorV1> {
    let parts = context.parts()?;
    if parts.custody.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "unfinished SDMA pool trim",
        ));
    }
    *parts.custody = Some(SdmaPoolTrimCustodyV1 {
        active: None,
        released: 0,
    });
    let result = catch_unwind(AssertUnwindSafe(|| {
        loop {
            let parts = context.parts()?;
            let root = parts.custody.as_mut().expect("installed pool trim");
            let Some(buffer) = parts.free.pop() else {
                let released = root.released;
                *parts.custody = None;
                return Ok(released);
            };
            root.active = Some(DataCleanupCustodyV1::from_sdma(buffer));
            let (lower, retake) = execute_live_model_custody_v1(
                context,
                C::loan,
                |context| -> Result<(), ComputeAqlQueueSessionErrorV1> {
                    let parts = context.parts()?;
                    let active = parts
                        .custody
                        .as_mut()
                        .expect("installed pool trim")
                        .active
                        .as_mut()
                        .expect("rooted pool buffer");
                    parts
                        .memory
                        .release_data(active)
                        .map_err(crate::sdma::Gfx942SdmaErrorV1::from)?;
                    if !active.is_complete() {
                        return Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "incomplete SDMA pool cleanup",
                        ));
                    }
                    Ok(())
                },
                C::retake,
                C::poison,
            )?;
            retake?;
            lower?;
            // A disposed receipt is retired only after the original model loan closes.
            let parts = context.parts()?;
            let root = parts.custody.as_mut().expect("installed pool trim");
            root.released += 1;
            root.active = None;
        }
    }));
    match result {
        Ok(Ok(released)) => Ok(released),
        Ok(Err(error)) => {
            context.poison();
            Err(error)
        }
        Err(payload) => {
            // Preserve the first panic even if terminalization itself unwinds.
            core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
            resume_unwind(payload)
        }
    }
}

impl PoolTrimContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;

    fn parts(
        &mut self,
    ) -> Result<PoolTrimPartsV1<'_, Self::Memory>, ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        Ok(PoolTrimPartsV1 {
            memory: &mut engine.backend.session,
            free: &mut self.sdma_pool_free,
            custody: &mut self.sdma_pool_trim,
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
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfinished_pool_trim_rejects_teardown_and_drop_aborts() {
        use std::os::unix::process::ExitStatusExt;
        const CHILD: &str = "FE2O3_TEST_POOL_TRIM_DROP";
        const TEST: &str =
            "queue::live::pool_trim::tests::unfinished_pool_trim_rejects_teardown_and_drop_aborts";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert_eq!(
                output.status.signal(),
                Some(6),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("pool trim guards checked; dropping retained root")
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked at"));
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
        // No fabricated native owner: this isolates the public session's retention guard.
        let mut session = std::mem::ManuallyDrop::new(
            super::super::tests::persistent_compute_cancellation_test_session(
                super::super::tests::test_queue_key(91, 1),
                None,
                None,
            ),
        );
        session.sdma_pool_trim = Some(SdmaPoolTrimCustodyV1 {
            active: None,
            released: 1,
        });
        let ledger = (
            session.sdma_device_pool.activity_started,
            session.sdma_outstanding_buffers,
            session.sdma_pool_reuse_count,
        );
        assert!(matches!(
            session.trim_sdma_memory_pool(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "terminal or unfinished SDMA pool trim"
            ))
        ));
        assert_eq!(
            (
                session.sdma_device_pool.activity_started,
                session.sdma_outstanding_buffers,
                session.sdma_pool_reuse_count
            ),
            ledger
        );
        assert!(matches!(
            session.supports_retained_primary_release_v1(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA pool trim"
            ))
        ));
        assert!(matches!(
            session.preflight_primary_release_v1(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA pool trim"
            ))
        ));
        assert!(matches!(
            session.destroy_queue_and_event(QueueDestroyModeV1::Release),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "the SDMA memory pool must be trimmed before queue destruction"
            ))
        ));
        assert!(session.sdma_pool_trim.is_some() && session.sdma_pool_free.is_empty());
        eprintln!("pool trim guards checked; dropping retained root");
        drop(std::mem::ManuallyDrop::into_inner(session));
        panic!("unfinished pool trim Drop returned");
    }
}
