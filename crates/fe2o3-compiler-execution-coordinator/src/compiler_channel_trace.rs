//! Consuming original-child/channel join. This owner is not compiler admission.
use super::*;
use fe2o3_protected_service_spawn::{
    RetainedDependencyV2 as Dependency, RetainedResourceAccessErrorV2 as AccessError,
    cleanup_bridge::CleanupPollV1 as CleanupPoll,
    native_spawn::{RootRetainedTaskTraceV2 as Trace, RootTaskTraceEventV2 as Event},
};

const FRAME: usize = super::FRAME + 4096;
const LOCAL_WORK: usize = 8 + 32 * 1088 + 64 * TRANSFER_BYTES;

/// One originating-thread owner of compiler trace, channel and transitive backing.
/// There is no constructor taking independently received channel/pidfd/PID claims.
/// Deferred cancellation retains the child's original payload in its original pool;
/// channel escrow is closed only after foreground trace cancellation starts.
pub(crate) struct CompilerTrace<'work, T: Send + 'static> {
    trace: Trace<'work, T>,
    channel: Option<CompilerChildChannel>,
    phase: Phase,
    retained: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Gated,
    Confirmed,
    Transferring,
    Transferred,
    Cancelled,
}
impl Phase {
    fn begin_transfer(&mut self, held_exec: bool) -> Result<()> {
        if *self != Self::Confirmed || !held_exec {
            return Err(Error::Invalid(
                "issuer inputs require confirmed held compiler exec",
            ));
        }
        *self = Self::Transferring;
        Ok(())
    }
}

impl<'work, T: Send + 'static> CompilerTrace<'work, T> {
    const ENVELOPE: usize = size_of::<(Self, usize)>()
        - size_of::<Trace<'work, T>>()
        - size_of::<CompilerChildChannel>();

    /// Consumes the same clone-owned child that authenticates channel receipt.
    /// The first-exec gate must remain closed through the returned trace seizure.
    /// Credentials come from actual request/profile admission, never wire claims.
    /// The child's complete backing and receiver remain prepaid on their original
    /// Budget. Returned storage is FULL and unreserved: after this call retire
    /// both consumed reservations and reserve the returned amount before keeping
    /// the result. Errors/unwind cancel the original child, never a numeric PID.
    /// No helper/approval/namespace/deployment authority is constructed here.
    pub(crate) fn receive(
        child: Child<T>,
        receiver: OwnedFd,
        credentials: Credentials,
        deadline: Instant,
        b: &mut Budget<'work>,
    ) -> Result<(Self, usize)> {
        let floor = native::sum(&[child.retained_storage(), native::FILE_STORAGE])?;
        b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            let (channel, charge) =
                CompilerChildChannel::receive(&child, receiver, credentials, deadline, b)?;
            b.release_storage(native::FILE_STORAGE)?;
            b.reserve_storage(charge)?;
            let growth = child
                .root_trace_storage()?
                .checked_sub(child.retained_storage())
                .ok_or(Resource::Accounting)?;
            b.reserve_storage(growth)?;
            let trace = child.into_root_trace(b)?;
            if Instant::now() >= deadline {
                return Err(launch_io::Failure::Timeout("compiler trace seizure").into());
            }
            let retained = native::sum(&[trace.retained_storage(), charge, Self::ENVELOPE])?;
            b.reserve_storage(Self::ENVELOPE)?;
            Ok((
                Self {
                    trace,
                    channel: Some(channel),
                    phase: Phase::Gated,
                    retained,
                },
                retained,
            ))
        })
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Mechanical transfer envelope above the prepaid trace, including one
    /// consuming observation and full dependency overlap. The callback's work,
    /// scratch, retained outputs and independent cleanup funding are additional.
    /// This inert query neither opens input access nor checks execution authority.
    pub(crate) fn issuer_inputs_quota(&self) -> Result<native::CompilerExecutionLaunchQuotaV2> {
        use fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2;
        let dependency = self.trace.dependency_quota()?;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[
                LOCAL_WORK,
                RootTaskTraceV2::OPERATION_WORK,
                dependency.work(),
            ])?,
            scratch: native::sum(&[
                FRAME,
                RootTaskTraceV2::OPERATION_SCRATCH,
                dependency.scratch(),
                dependency.retained_storage(),
            ])?,
        })
    }

    /// Full drop-only backing charge for the issuer's future cleanup payload.
    pub(crate) fn issuer_dependency_storage(&self) -> Result<usize> {
        Ok(self.trace.dependency_quota()?.retained_storage())
    }

    /// Scalar identity for contextual observations, never independent wait custody.
    pub(crate) fn pid(&self) -> rustix::process::Pid {
        self.trace.pid()
    }

    /// Revalidate actual compiler/helper backing on its original account without
    /// detaching it. The callback funds nested runtime/approval checks and outputs.
    /// The trace's scoped access forbids a backing reference escaping the call.
    pub(crate) fn with_backing<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&T, &mut Budget<'_>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<AccessError>,
    {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            self.trace.with_resources(b, operation)
        })
    }

    /// One original-account consuming wait; this does not release the spawn lease.
    pub(crate) fn poll(&mut self, b: &mut Budget<'_>) -> Result<Event> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            Ok(self.trace.poll(b)?)
        })
    }

    /// Confirm only the owned, still-held first-exec observation. This releases
    /// the existing spawn lease, not evidence or compiler/issuer authority.
    ///
    /// # Safety
    /// All RootRetainedTaskTraceV2::confirm_exec obligations apply: authenticate
    /// the native exec protocol and closure of ALL inherited artifact-lock aliases.
    /// Keep the validated Stage's parent writer aliases closed, check its exact
    /// status EOF, and establish that no untraced descendant inherited an alias.
    /// Root exec alone cannot establish those conditions or loader/DSO admission.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Gated {
                return Err(Error::Invalid("compiler exec confirmation is closed"));
            }
            if !self.trace.poll(b)?.is_exec() {
                return Err(Error::Invalid("compiler has no held exec stop"));
            }
            self.channel
                .as_ref()
                .ok_or(Error::Invalid("compiler channel escrow is closed"))?
                .revalidate()?;
            // SAFETY: caller supplies native exec/alias-closure authentication;
            // the original trace independently enforces account/thread/held stop.
            unsafe {
                self.trace.confirm_exec(b)?;
            }
            self.phase = Phase::Confirmed;
            Ok(())
        })
    }

    /// Scoped fixed-ABI inputs for the PRIVATE root-to-issuer composition only.
    /// The callback gets the same Budget and prepays its own copies and outputs.
    /// One-use input access requires our own still-held confirmed exec stop, original
    /// pidfd liveness and unchanged endpoint. Public handoff admission is unchanged.
    /// This does not create AcceptedHandoff or authenticate a root deployment.
    /// The callback receives a fully charged drop-only handle to the SAME backing.
    /// Install it and any endpoint duplicates in the issuer's funded cleanup slot
    /// before clone; it survives independently of foreground compiler/trace cleanup.
    /// Returned copies/owners need their full charge reserved after this scope.
    /// The original channel escrow closes after transfer, so it cannot prolong the
    /// compiler connection after issuer exit. No root-trace/wait authority escapes.
    pub(crate) fn with_issuer_inputs<R>(
        &mut self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(
            Identity,
            BorrowedFd<'_>,
            BorrowedFd<'_>,
            Dependency<T>,
            &mut Budget<'_>,
        ) -> Result<R>,
    ) -> Result<R> {
        let mut attempt = InputAttempt {
            trace: &mut self.trace,
            phase: &mut self.phase,
            channel: None,
        };
        let result = b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            // A refused or unwinding callback may already have duplicated inputs.
            // Never replay that transfer, even if a later exec stop is observed.
            attempt
                .phase
                .begin_transfer(attempt.trace.poll(b)?.is_exec())?;
            attempt.channel = self.channel.take();
            let channel = attempt
                .channel
                .as_ref()
                .ok_or(Error::Invalid("compiler channel escrow is closed"))?;
            channel.revalidate()?;
            let dependency = attempt.trace.retain_dependencies(b)?;
            b.reserve_storage(dependency.retained_storage())?;
            operation(
                channel.client,
                channel.service_peer.as_fd(),
                channel.client_pidfd.as_fd(),
                dependency,
                b,
            )
        })?;
        // Keep cancellation armed through the scope's final accounting checks.
        *attempt.phase = Phase::Transferred;
        Ok(result)
    }

    /// Resume only after first-exec confirmation. Issuer readiness and approved
    /// runtime enforcement remain additional requirements of the owning attempt.
    pub(crate) fn resume(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Transferred {
                return Err(Error::Invalid(
                    "compiler issuer inputs have not been consumed",
                ));
            }
            Ok(self.trace.resume(b)?)
        })
    }

    /// Preserve exact-child cancellation and transitive payload custody.
    pub(crate) fn cancel(&mut self) -> CleanupPoll {
        self.phase = Phase::Cancelled;
        self.trace.cancel()
    }
}

struct InputAttempt<'a, 'work, T: Send + 'static> {
    trace: &'a mut Trace<'work, T>,
    phase: &'a mut Phase,
    channel: Option<CompilerChildChannel>,
}
impl<T: Send + 'static> Drop for InputAttempt<'_, '_, T> {
    fn drop(&mut self) {
        if *self.phase == Phase::Transferring {
            *self.phase = Phase::Cancelled;
            let _ = self.trace.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_access_requires_both_confirmation_and_a_current_held_exec() {
        for initial in [
            Phase::Gated,
            Phase::Confirmed,
            Phase::Transferring,
            Phase::Transferred,
            Phase::Cancelled,
        ] {
            for held in [false, true] {
                let mut phase = initial;
                let allowed = initial == Phase::Confirmed && held;
                assert_eq!(phase.begin_transfer(held).is_ok(), allowed);
                assert_eq!(
                    phase,
                    if allowed {
                        Phase::Transferring
                    } else {
                        initial
                    }
                );
            }
        }
    }

    #[test]
    fn later_exec_cannot_reopen_consumed_or_failed_issuer_access() {
        for outcome in [Phase::Transferred, Phase::Cancelled] {
            let mut phase = Phase::Confirmed;
            phase.begin_transfer(true).unwrap();
            assert!(phase.begin_transfer(true).is_err());
            phase = outcome;
            assert!(phase.begin_transfer(false).is_err());
            assert!(phase.begin_transfer(true).is_err());
            assert_eq!(phase, outcome);
        }
    }
}
