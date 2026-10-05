//! Original attempt transitions. No gate release or runtime-admission token.
use super::*;
use crate::native_v3::NativeAttempt as Issued;
use fe2o3_protected_service_spawn::native_spawn::RootRuntimeTraceV1 as Runtime;

// These states route the private owner only. Actual trace stops, original Rc
// identity, backing and kernel enforcement remain independently checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Gated,
    Interrupting,
    Interrupted,
    Armed,
    HeldExec,
    ConfirmedExec,
    Issued,
    Cancelled,
}

pub(super) enum Owner<'work> {
    Gated(Trace<'work>),
    Issued(Issued<'work, Helper>),
}

impl Owner<'_> {
    pub(super) fn cancel(&mut self) -> CleanupPollV1 {
        match self {
            Self::Gated(trace) => trace.cancel(),
            Self::Issued(attempt) => {
                let result = attempt.cancel_compiler();
                let _issuer = attempt.cancel_issuer();
                result
            }
        }
    }

    pub(super) fn needs_foreground_cancellation(&self) -> bool {
        match self {
            Self::Gated(trace) => trace.needs_foreground_cancellation(),
            Self::Issued(attempt) => attempt.needs_foreground_cancellation(),
        }
    }

    pub(super) fn cancel_step(&mut self, b: &mut Budget<'_>) -> AttemptResult<CleanupPollV1> {
        Ok(match self {
            Self::Gated(trace) => trace.cancel_step(b)?,
            Self::Issued(attempt) => attempt.cancel_compiler_step(b)?,
        })
    }
}

impl<'work> Attempt<'work> {
    pub(super) fn gated_trace(&self) -> AttemptResult<&Trace<'work>> {
        match &self.owner {
            Some(Owner::Gated(trace)) => Ok(trace),
            _ => Err(Failure::Invalid("compiler no longer owns its gated trace")),
        }
    }

    fn gated_trace_mut(&mut self) -> AttemptResult<&mut Trace<'work>> {
        match &mut self.owner {
            Some(Owner::Gated(trace)) => Ok(trace),
            _ => Err(Failure::Invalid("compiler no longer owns its gated trace")),
        }
    }

    /// Interrupt only the same original compiler while its exec gate remains
    /// closed. Neither this state nor a successful interrupt admits execution.
    pub(super) fn interrupt_runtime(
        &mut self,
        received: &Receiver,
        b: &mut Budget<'_>,
    ) -> AttemptResult<()> {
        if self.phase != Phase::Gated || self.controller.is_some() {
            return Err(Failure::Invalid("compiler runtime interrupt is not fresh"));
        }
        self.revalidate(received, b)?;
        self.gated_trace_mut()?.interrupt_for_runtime(b)?;
        self.phase = Phase::Interrupting;
        Ok(())
    }

    /// One consuming original wait; pending does not renew the intake deadline.
    pub(super) fn poll_runtime_interrupt(&mut self, b: &mut Budget<'_>) -> AttemptResult<bool> {
        if self.phase != Phase::Interrupting {
            return Err(Failure::Invalid(
                "compiler runtime interrupt was not requested",
            ));
        }
        let event = self.gated_trace_mut()?.poll(b)?;
        if event.is_pending() {
            return Ok(false);
        }
        if !event.is_original_interrupt() {
            self.cancel();
            return Err(Failure::Invalid(
                "compiler gate acquired an unexpected original stop",
            ));
        }
        self.phase = Phase::Interrupted;
        Ok(true)
    }

    pub(super) fn arm_runtime_quota() -> AttemptResult<native::CompilerExecutionLaunchQuotaV2> {
        let inner = Trace::runtime_takeover_quota()?;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, inner.work()])?,
            scratch: native::sum(&[FRAME, inner.scratch()])?,
        })
    }

    /// Consume the original held interrupt and retain the new runtime owner
    /// before any later fallible accounting. Returned growth is unreserved.
    ///
    /// # Safety
    /// Preserve the dedicated process/outside custodian and original exclusive
    /// creator/account contracts through full foreground retirement. The original
    /// gate stays closed; an unresolved failure after takeover must fail-stop,
    /// never transfer an armed tree to root-only background cleanup.
    #[allow(unsafe_code)]
    pub(super) unsafe fn arm_runtime(&mut self, b: &mut Budget<'_>) -> AttemptResult<usize> {
        if self.phase != Phase::Interrupted || self.controller.is_some() {
            return Err(Failure::Invalid(
                "compiler runtime takeover lacks its held interrupt",
            ));
        }
        let retained = native::sum(&[self.retained, Runtime::STORAGE_GROWTH])?;
        let result = b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            let Some(Owner::Gated(trace)) = self.owner.take() else {
                return Err(Failure::Invalid(
                    "compiler runtime takeover lost original owner",
                ));
            };
            self.phase = Phase::Cancelled;
            // SAFETY: the caller preserves the original closed gate and dedicated
            // custody. This consumes only the trace held in this same Attempt.
            let (trace, _) = unsafe { trace.arm_runtime(b) }?;
            self.owner = Some(Owner::Gated(trace));
            b.reserve_storage(Runtime::STORAGE_GROWTH)?;
            self.retained = retained;
            self.phase = Phase::Armed;
            Ok(Runtime::STORAGE_GROWTH)
        });
        if result.is_err() {
            self.cancel();
        }
        result
    }

    /// First-exec policy capture only. The gate/exec protocol remains a separate
    /// transition; this refuses unless the actual original runtime holds Exec.
    pub(super) fn capture_first_exec(&mut self, b: &mut Budget<'_>) -> AttemptResult<usize> {
        if self.phase != Phase::Armed || self.controller.is_some() {
            return Err(Failure::Invalid("compiler first-exec capture is not fresh"));
        }
        let retained = native::sum(&[self.retained, Controller::STORAGE])?;
        let result = b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            let Some(Owner::Gated(trace)) = &mut self.owner else {
                return Err(Failure::Invalid("compiler first-exec lost original owner"));
            };
            let inventory = &self.executables;
            let controller = trace.with_runtime_backing(b, |runtime, helper, b| {
                helper.with_compiler(b, |backing, b| -> AttemptResult<_> {
                    inventory.revalidate(backing, b)?;
                    Ok(Controller::from_checked_root_exec(
                        runtime, backing, inventory, b,
                    )?)
                })
            })?;
            self.controller = Some(controller);
            b.reserve_storage(Controller::STORAGE)?;
            self.retained = retained;
            self.phase = Phase::HeldExec;
            Ok(Controller::STORAGE)
        });
        if result.is_err() {
            self.cancel();
        }
        result
    }

    /// Close every original staging/gate alias before checking native exec EOF.
    /// Neither EOF nor confirmation grants permission to run application code.
    ///
    /// # Safety
    /// The dedicated original custodian excludes fork/FD mutation outside this
    /// trace. All first-exec/profile/cgroup obligations of the original launch
    /// remain held; this actual stop has already passed complete image/census
    /// validation and has not resumed since it was captured.
    #[allow(unsafe_code)]
    pub(super) unsafe fn confirm_first_exec(&mut self, b: &mut Budget<'_>) -> AttemptResult<()> {
        if self.phase != Phase::HeldExec || self.controller.is_none() {
            return Err(Failure::Invalid(
                "compiler exec confirmation lacks held image validation",
            ));
        }
        let result = b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            require_deadline(self.deadline)?;
            // Stage owns duplicate status/gate/artifact sources, so it must die
            // before EOF. Retain all prepaid charges conservatively until Drop.
            drop(self.stage.take());
            drop(self.gate_reader.take());
            drop(self.gate_writer.take());
            let mut status = [0_u8; 1];
            if rustix::io::read(&self._exec_reader, &mut status)
                .map_err(|e| native::io("read original compiler exec status", e))?
                != 0
            {
                return Err(Failure::Invalid(
                    "compiler exec status did not reach clean EOF",
                ));
            }
            // SAFETY: all parent aliases closed above. Original held Exec and
            // image/census passed without resume; the caller excludes escapes.
            unsafe { self.gated_trace_mut()?.confirm_exec(b) }?;
            self.phase = Phase::ConfirmedExec;
            Ok(())
        });
        if result.is_err() {
            self.cancel();
        }
        result
    }

    /// Move the exact original trace into the existing issuer composition. The
    /// controller and inventory never leave this Attempt or get reconstructed.
    /// Returned growth is above this Attempt PLUS the consumed preparation.
    ///
    /// # Safety
    /// Preserve Prepared::launch_root_attempt's original creator, cleanup and
    /// payload Drop contracts. The existing independently funded original pool
    /// must outlive both compiler and issuer, including unresolved fail-stop.
    #[allow(unsafe_code)]
    pub(super) unsafe fn launch_issuer(
        &mut self,
        prepared: crate::native_v3::PreparedCompilerExecutionSupervisorV3,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> AttemptResult<usize> {
        if self.phase != Phase::ConfirmedExec || self.controller.is_none() {
            return Err(Failure::Invalid(
                "issuer requires original confirmed compiler image",
            ));
        }
        let input = native::sum(&[self.retained, prepared.retained_storage()])?;
        let result = b.with_prepaid_scope(input, 8, LOCAL_WORK, FRAME, |b| {
            require_deadline(self.deadline)?;
            let Some(Owner::Gated(trace)) = self.owner.take() else {
                return Err(Failure::Invalid(
                    "issuer transfer lost original compiler trace",
                ));
            };
            self.phase = Phase::Cancelled;
            // SAFETY: the caller retains the original dedicated cleanup contract.
            // This is the confirmed original trace, not independently supplied IDs.
            let (attempt, growth) = unsafe {
                prepared.launch_root_attempt(
                    trace,
                    self.deadline.saturating_duration_since(Instant::now()),
                    cleanup,
                    b,
                )
            }?;
            self.owner = Some(Owner::Issued(attempt));
            b.reserve_storage(growth.additional_storage())?;
            let retained = native::sum(&[input, growth.additional_storage()])?;
            self.retained = retained;
            let Some(Owner::Issued(attempt)) = &self.owner else {
                unreachable!("retained original issuer")
            };
            attempt.validate_ready(b)?;
            require_deadline(self.deadline)?;
            self.phase = Phase::Issued;
            Ok(growth.additional_storage())
        });
        if result.is_err() {
            self.cancel();
        }
        result
    }

    /// Only an actual issuer-owning variant may run checkpoints. Readiness is
    /// checked while the first exec stays held; subsequent steps keep the same
    /// original helper/inventory/controller and do not hash code per syscall.
    pub(super) fn step_runtime(
        &mut self,
        b: &mut Budget<'_>,
    ) -> AttemptResult<crate::native_runtime_controller::Progress> {
        if self.phase != Phase::Issued {
            return Err(Failure::Invalid(
                "compiler checkpoint lacks original issued attempt",
            ));
        }
        let Some(Owner::Issued(attempt)) = &mut self.owner else {
            return Err(Failure::Invalid("compiler issuer custody is absent"));
        };
        let controller = self
            .controller
            .as_mut()
            .ok_or(Failure::Invalid("compiler controller is absent"))?;
        let inventory = &self.executables;
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            attempt.with_runtime_backing(b, |runtime, helper, b| {
                helper.with_compiler_checkpoint(b, |backing, b| -> AttemptResult<_> {
                    Ok(controller.step(runtime, backing, inventory, b)?)
                })
            })
        })
    }

    pub(super) fn runtime_step_quota() -> AttemptResult<native::CompilerExecutionLaunchQuotaV2> {
        let trace = Issued::<Helper>::runtime_backing_quota()?;
        let helper = Helper::checkpoint_access_quota()?;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[
                LOCAL_WORK,
                trace.work(),
                helper.work(),
                Controller::step_work()?,
            ])?,
            scratch: native::sum(&[
                FRAME,
                trace.scratch(),
                helper.scratch(),
                Controller::STEP_SCRATCH,
            ])?,
        })
    }
}
