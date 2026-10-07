//! Consuming original-child/channel join. This owner is not compiler admission.
use super::*;
#[cfg(test)]
use crate::native_v3::root_intake::RootCompilerRequest;
use fe2o3_protected_service_spawn::{
    RetainedDependencyV2 as Dependency, RetainedResourceAccessErrorV2 as AccessError,
    cleanup_bridge::CleanupPollV1 as CleanupPoll,
    native_spawn::{RootRetainedTaskTraceV2 as Trace, RootRuntimeTraceV1},
};

#[path = "compiler_channel_trace_owner.rs"]
mod owner;
pub(crate) use owner::CompilerTraceEvent as Event;
use owner::TraceOwner;

const FRAME: usize = super::FRAME + 4 * size_of::<TraceOwner<'static, ()>>() + 4096;
const LOCAL_WORK: usize = 8 + 32 * 1088 + 64 * TRANSFER_BYTES;

/// One originating-thread owner of compiler trace, channel and transitive backing.
/// There is no constructor taking independently received channel/pidfd/PID claims.
/// Deferred cancellation retains the child's original payload in its original pool;
/// channel escrow is closed only after foreground trace cancellation starts.
pub(crate) struct CompilerTrace<'work, T: Send + 'static> {
    trace: TraceOwner<'work, T>,
    channel: Option<CompilerChildChannel>,
    phase: Phase,
    retained: usize,
    deadline: Instant,
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
    pub(crate) const PUBLICATION_FRAME_WORK: usize = LOCAL_WORK;
    pub(crate) const PUBLICATION_FRAME_SCRATCH: usize = FRAME;
    pub(crate) const OBSERVATION_WORK: usize = LOCAL_WORK
        + fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2::VIEW_WORK
        + RootRuntimeTraceV1::ROOT_OBSERVATION_WORK;
    pub(crate) const OBSERVATION_SCRATCH: usize =
        FRAME + fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2::VIEW_SCRATCH;
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
            #[cfg(test)]
            RootCompilerRequest::postclone_checkpoint_for_test("compiler-channel", child.pid(), b)?;
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
                    trace: TraceOwner::Original(trace),
                    channel: Some(channel),
                    phase: Phase::Gated,
                    retained,
                    deadline,
                },
                retained,
            ))
        })
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Only trace retirement permits handing the remaining domain cleanup to
    /// the original pool. This is not an empty-pool or completion observation.
    pub(crate) fn needs_foreground_cancellation(&self) -> bool {
        match &self.trace {
            TraceOwner::Original(_) => false,
            TraceOwner::Runtime(trace) => !trace.observation().is_trace_retired(),
        }
    }

    pub(crate) fn cancellation_quota() -> Result<native::CompilerExecutionLaunchQuotaV2> {
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, RootRuntimeTraceV1::OPERATION_WORK])?,
            scratch: native::sum(&[FRAME, RootRuntimeTraceV1::OPERATION_SCRATCH])?,
        })
    }

    /// Fixed consuming-takeover work and extra peak above the original owner.
    /// Interrupt/poll turns before takeover are funded separately; no new ledger
    /// or execution deadline is created by this inert schedule.
    pub(crate) fn runtime_takeover_quota() -> Result<native::CompilerExecutionLaunchQuotaV2> {
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, RootRuntimeTraceV1::OPERATION_WORK])?,
            scratch: native::sum(&[
                FRAME,
                RootRuntimeTraceV1::STORAGE_GROWTH,
                RootRuntimeTraceV1::OPERATION_SCRATCH,
            ])?,
        })
    }

    pub(crate) fn gated_operation_quota() -> Result<native::CompilerExecutionLaunchQuotaV2> {
        use fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2 as Task;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, Task::OPERATION_WORK])?,
            scratch: native::sum(&[FRAME, Task::OPERATION_SCRATCH])?,
        })
    }

    pub(crate) fn runtime_confirmation_quota() -> Result<native::CompilerExecutionLaunchQuotaV2> {
        use fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2 as Task;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[
                LOCAL_WORK,
                RootRuntimeTraceV1::OPERATION_WORK,
                Task::CONFIRM_EXEC_WORK,
            ])?,
            scratch: native::sum(&[
                FRAME,
                RootRuntimeTraceV1::OPERATION_SCRATCH,
                Task::CONFIRM_EXEC_SCRATCH,
            ])?,
        })
    }

    /// Stop the original gated task; arming still requires its consumed stop.
    pub(crate) fn interrupt_for_runtime(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Gated || Instant::now() >= self.deadline {
                return Err(Error::Invalid(
                    "runtime takeover requires the original live gate",
                ));
            }
            match &mut self.trace {
                TraceOwner::Original(trace) => Ok(trace.interrupt(b)?),
                TraceOwner::Runtime(_) => Err(Error::Invalid("compiler runtime already armed")),
            }
        })
    }

    /// Consume the same channel/backing and original held interrupt. Returned
    /// FULL storage is unreserved; retire the input reservation before retaining
    /// the output. The unchanged receive deadline is used, never a fresh timeout.
    ///
    /// # Safety
    /// Keep the original exec gate closed and satisfy RootRuntimeTraceV1's
    /// dedicated-process/outside-custodian and exclusive custody contract.
    /// Authenticate the typed stage and complete runtime policy before release.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn arm_runtime(self, b: &mut Budget<'_>) -> Result<(Self, usize)> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Gated {
                return Err(Error::Invalid("compiler runtime takeover is closed"));
            }
            let TraceOwner::Original(trace) = self.trace else {
                return Err(Error::Invalid("compiler runtime already armed"));
            };
            let retained = self
                .retained
                .checked_add(RootRuntimeTraceV1::STORAGE_GROWTH)
                .ok_or(Resource::Arithmetic)?;
            b.reserve_storage(RootRuntimeTraceV1::STORAGE_GROWTH)?;
            // SAFETY: the caller retains the closed gate and original dedicated
            // custodian. The consuming primitive validates the actual held stop.
            let trace = unsafe { trace.into_runtime_trace(self.deadline, b) }?;
            Ok((
                Self {
                    trace: TraceOwner::Runtime(trace),
                    channel: self.channel,
                    phase: self.phase,
                    retained,
                    deadline: self.deadline,
                },
                retained,
            ))
        })
    }

    /// Trusted private controller composition, not an execution admission API.
    /// Every operation still charges/checks the original account and deadline.
    pub(crate) fn with_runtime<R>(
        &mut self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&mut RootRuntimeTraceV1<'work>, &mut Budget<'_>) -> Result<R>,
    ) -> Result<R> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase == Phase::Cancelled {
                return Err(Error::Invalid("compiler runtime execution is cancelled"));
            }
            operation(self.trace.runtime()?, b)
        })
    }

    /// The private checkpoint controller uses the actual runtime and its
    /// original locked backing together. Callback work/scratch is additional;
    /// returned observations grant no permission to resume or publish.
    pub(crate) fn with_runtime_backing<R, E>(
        &mut self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(
            &mut RootRuntimeTraceV1<'work>,
            &T,
            &mut Budget<'_>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource>
            + From<Error>
            + From<AccessError>
            + From<fe2o3_protected_service_spawn::native_spawn::ProtectedServiceSpawnErrorV2>,
    {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase == Phase::Cancelled {
                return Err(Error::Invalid("compiler runtime execution is cancelled").into());
            }
            match &mut self.trace {
                TraceOwner::Runtime(trace) => trace.with_runtime_resources(b, operation),
                TraceOwner::Original(_) => {
                    Err(Error::Invalid("compiler runtime trace is not armed").into())
                }
            }
        })
    }

    pub(crate) fn runtime_backing_quota() -> Result<native::CompilerExecutionLaunchQuotaV2> {
        use fe2o3_protected_service_spawn::RetainedResourcesV2 as Resources;
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, Resources::<T>::ACCESS_WORK])?,
            scratch: native::sum(&[FRAME, Resources::<T>::ACCESS_SCRATCH])?,
        })
    }

    /// Drive original funded cancellation; Pending preserves foreground custody.
    /// A terminal trace is necessary but not sufficient for domain retirement.
    pub(crate) fn cancel_step(&mut self, b: &mut Budget<'_>) -> Result<CleanupPoll> {
        self.phase = Phase::Cancelled;
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            match &mut self.trace {
                TraceOwner::Original(trace) => Ok(trace.cancel()),
                TraceOwner::Runtime(trace) => {
                    let runtime = trace.runtime();
                    runtime.mark_cancellation();
                    if !runtime.is_trace_retired() && !runtime.cancel_step(b)? {
                        return Ok(CleanupPoll::Pending);
                    }
                    Ok(runtime.cleanup_after_retirement()?)
                }
            }
        })
    }

    /// Mechanical transfer envelope above the prepaid trace, including one
    /// consuming observation and full dependency overlap. The callback's work,
    /// scratch, retained outputs and independent cleanup funding are additional.
    /// This inert query neither opens input access nor checks execution authority.
    pub(crate) fn issuer_inputs_quota(&self) -> Result<native::CompilerExecutionLaunchQuotaV2> {
        let dependency = self.trace.dependency_quota()?;
        let (operation_work, operation_scratch) = self.trace.operation_quota();
        Self::issuer_inputs_quota_for(dependency, operation_work, operation_scratch)
    }

    /// Same transfer bound before the original trace exists. The payload is a
    /// complete declared ceiling, not an independently admitted backing owner.
    pub(crate) fn maximum_issuer_inputs_quota(
        payload: usize,
    ) -> Result<native::CompilerExecutionLaunchQuotaV2> {
        use fe2o3_protected_service_spawn::native_spawn::RootRetainedRuntimeTraceV1;
        Self::issuer_inputs_quota_for(
            RootRetainedRuntimeTraceV1::<T>::dependency_quota_for_payload(payload)?,
            RootRuntimeTraceV1::OPERATION_WORK
                .max(fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2::OPERATION_WORK),
            RootRuntimeTraceV1::OPERATION_SCRATCH.max(
                fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2::OPERATION_SCRATCH,
            ),
        )
    }

    fn issuer_inputs_quota_for(
        dependency: fe2o3_protected_service_spawn::RetainedDependencyQuotaV2,
        operation_work: usize,
        operation_scratch: usize,
    ) -> Result<native::CompilerExecutionLaunchQuotaV2> {
        Ok(native::CompilerExecutionLaunchQuotaV2 {
            work: native::sum(&[LOCAL_WORK, operation_work, dependency.work()])?,
            scratch: native::sum(&[
                FRAME,
                operation_scratch,
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
        E: From<Resource>
            + From<AccessError>
            + From<fe2o3_protected_service_spawn::native_spawn::ProtectedServiceSpawnErrorV2>,
    {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            self.trace.with_resources(b, operation)
        })
    }

    /// Inspection through original trace custody, only after the one-use issuer
    /// input transfer. The owning attempt must separately gate compiler resume on
    /// actual readiness/runtime admission; this view grants no compiler admission.
    pub(crate) fn with_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &fe2o3_protected_service_spawn::native_spawn::RootTaskObservationV2<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource>
            + From<Error>
            + From<fe2o3_protected_service_spawn::native_spawn::ProtectedServiceSpawnErrorV2>,
    {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Transferred {
                return Err(Error::Invalid(
                    "root observation requires completed issuer input transfer",
                )
                .into());
            }
            self.trace.with_task_observation(b, operation)
        })
    }

    /// Concrete publication attachment; no mutable trace or acquisition callback
    /// escapes the phase boundary. The consuming NativeAttempt owns cancellation
    /// until its outer accounting and original-session checks have also succeeded.
    pub(crate) fn observe_publication(
        &mut self,
        cleanup: &mut fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2,
        maximum_handoff_bytes: usize,
        b: &mut Budget<'_>,
    ) -> Result<(
        fe2o3_broker_authority_service::RootPublicationCustodyV3,
        usize,
    )> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            self.require_publication_phase()?;
            use fe2o3_broker_authority_service::RootPublicationCustodyV3 as Publication;
            Ok(match &mut self.trace {
                TraceOwner::Original(trace) => {
                    Publication::observe_with_limit(trace, cleanup, maximum_handoff_bytes, b)?
                }
                TraceOwner::Runtime(trace) => Publication::observe_runtime_with_limit(
                    trace.runtime(),
                    cleanup,
                    maximum_handoff_bytes,
                    b,
                )?,
            })
        })
    }

    pub(crate) fn revalidate_publication(
        &self,
        publication: &fe2o3_broker_authority_service::RootPublicationCustodyV3,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = native::sum(&[self.retained, publication.retained_storage()])?;
        b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            self.require_publication_phase()?;
            match &self.trace {
                TraceOwner::Original(trace) => publication.revalidate(trace, b)?,
                TraceOwner::Runtime(trace) => {
                    publication.revalidate_runtime(trace.observation(), b)?
                }
            }
            Ok(())
        })
    }

    fn require_publication_phase(&self) -> Result<()> {
        if self.phase != Phase::Transferred {
            return Err(Error::Invalid(
                "publication custody requires completed issuer input transfer",
            ));
        }
        Ok(())
    }

    /// One original-account consuming wait; this does not release the spawn lease.
    pub(crate) fn poll(&mut self, b: &mut Budget<'_>) -> Result<Event> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| self.trace.poll(b))
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
        b: &mut Budget<'work>,
        operation: impl FnOnce(
            Identity,
            BorrowedFd<'_>,
            BorrowedFd<'_>,
            Dependency<T>,
            &mut Budget<'work>,
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
            self.trace.resume_original(b)
        })
    }

    /// Preserve exact-child cancellation and transitive payload custody.
    pub(crate) fn cancel(&mut self) -> CleanupPoll {
        self.phase = Phase::Cancelled;
        self.trace.cancel()
    }
}

struct InputAttempt<'a, 'work, T: Send + 'static> {
    trace: &'a mut TraceOwner<'work, T>,
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
    fn preconstruction_issuer_input_quote_preserves_both_trace_modes_and_dependencies() {
        use fe2o3_protected_service_spawn::native_spawn::{
            RootRetainedRuntimeTraceV1, RootTaskTraceV2,
        };
        for payload in [0, 4096, 1 << 20] {
            let quote = CompilerTrace::<()>::maximum_issuer_inputs_quota(payload).unwrap();
            let dependency =
                RootRetainedRuntimeTraceV1::<()>::dependency_quota_for_payload(payload).unwrap();
            for (work, scratch) in [
                (
                    RootTaskTraceV2::OPERATION_WORK,
                    RootTaskTraceV2::OPERATION_SCRATCH,
                ),
                (
                    RootRuntimeTraceV1::OPERATION_WORK,
                    RootRuntimeTraceV1::OPERATION_SCRATCH,
                ),
            ] {
                let actual =
                    CompilerTrace::<()>::issuer_inputs_quota_for(dependency, work, scratch)
                        .unwrap();
                assert!(quote.work() >= actual.work());
                assert!(quote.scratch() >= actual.scratch());
            }
            assert!(quote.scratch() >= 2 * dependency.retained_storage());
        }
        assert!(CompilerTrace::<[u8; 32]>::maximum_issuer_inputs_quota(31).is_err());
        assert!(CompilerTrace::<()>::maximum_issuer_inputs_quota(usize::MAX).is_err());
    }

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
