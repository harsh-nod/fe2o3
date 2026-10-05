//! The channel keeps one consuming trace owner across runtime takeover.
use super::*;
use fe2o3_protected_service_spawn::{
    RetainedDependencyQuotaV2,
    native_spawn::{
        ProtectedServiceSpawnErrorV2 as SpawnError, RootRetainedRuntimeTraceV1 as Runtime,
        RootRuntimeTraceV1, RootTaskObservationV2, RootTaskTraceEventV2 as OriginalEvent,
        RuntimeTraceEventV1,
    },
};

pub(super) enum TraceOwner<'work, T: Send + 'static> {
    Original(Trace<'work, T>),
    Runtime(Runtime<'work, T>),
}

/// Diagnostic only. A descendant exec/exit must not become root confirmation.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CompilerTraceEvent {
    Original(OriginalEvent),
    Runtime {
        root: rustix::process::Pid,
        event: Option<RuntimeTraceEventV1>,
    },
}
impl CompilerTraceEvent {
    pub(crate) fn is_pending(self) -> bool {
        match self {
            Self::Original(event) => event.is_pending(),
            Self::Runtime { event, .. } => event.is_none(),
        }
    }
    pub(crate) fn is_exec(self) -> bool {
        match self {
            Self::Original(event) => event.is_exec(),
            Self::Runtime { root, event } => event.is_some_and(|e| e.pid() == root && e.is_exec()),
        }
    }
    pub(crate) fn is_terminal(self) -> bool {
        match self {
            Self::Original(event) => event.is_terminal(),
            Self::Runtime { root, event } => {
                event.is_some_and(|e| e.pid() == root && e.is_terminal())
            }
        }
    }
    pub(crate) fn is_original_interrupt(self) -> bool {
        matches!(self, Self::Original(event) if event.is_trap_stop())
    }
}

impl<'work, T: Send + 'static> TraceOwner<'work, T> {
    pub(super) fn pid(&self) -> rustix::process::Pid {
        match self {
            Self::Original(trace) => trace.pid(),
            Self::Runtime(trace) => trace.observation().pid(),
        }
    }
    pub(super) fn dependency_quota(&self) -> Result<RetainedDependencyQuotaV2> {
        Ok(match self {
            Self::Original(trace) => trace.dependency_quota()?,
            Self::Runtime(trace) => trace.dependency_quota()?,
        })
    }
    pub(super) fn retain_dependencies(&self, b: &mut Budget<'_>) -> Result<Dependency<T>> {
        Ok(match self {
            Self::Original(trace) => trace.retain_dependencies(b)?,
            Self::Runtime(trace) => trace.retain_dependencies(b)?,
        })
    }
    pub(super) fn with_resources<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(&T, &mut Budget<'budget>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<AccessError> + From<SpawnError>,
    {
        match self {
            Self::Original(trace) => trace.with_resources(b, operation),
            Self::Runtime(trace) => trace.with_resources(b, operation),
        }
    }
    pub(super) fn with_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &RootTaskObservationV2<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<SpawnError>,
    {
        match self {
            Self::Original(trace) => trace.with_task_observation(b, operation),
            Self::Runtime(trace) => trace.observation().with_task_observation(b, operation),
        }
    }
    pub(super) fn poll(&mut self, b: &mut Budget<'_>) -> Result<CompilerTraceEvent> {
        Ok(match self {
            Self::Original(trace) => CompilerTraceEvent::Original(trace.poll(b)?),
            Self::Runtime(trace) => CompilerTraceEvent::Runtime {
                root: trace.observation().pid(),
                event: trace.runtime().poll(b)?,
            },
        })
    }
    #[allow(unsafe_code)]
    pub(super) unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        // SAFETY: the private caller authenticated native exec and closure of all
        // inherited artifact aliases. Both owners validate their own held root.
        unsafe {
            match self {
                Self::Original(trace) => trace.confirm_exec(b)?,
                Self::Runtime(trace) => trace.runtime().confirm_exec(b)?,
            }
        }
        Ok(())
    }
    pub(super) fn resume_original(&mut self, b: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Original(trace) => Ok(trace.resume(b)?),
            Self::Runtime(_) => Err(Error::Invalid(
                "runtime resume requires checkpoint enforcement",
            )),
        }
    }
    pub(super) fn cancel(&mut self) -> CleanupPoll {
        match self {
            Self::Original(trace) => trace.cancel(),
            Self::Runtime(trace) => {
                let runtime = trace.runtime();
                runtime.mark_cancellation();
                if runtime.is_trace_retired() {
                    runtime
                        .cleanup_after_retirement()
                        .unwrap_or(CleanupPoll::Quarantined)
                } else {
                    // No terminal wait or background handoff is claimed. The
                    // original owner remains reachable for funded cancellation.
                    CleanupPoll::Pending
                }
            }
        }
    }
    pub(super) fn runtime(&mut self) -> Result<&mut RootRuntimeTraceV1<'work>> {
        match self {
            Self::Runtime(trace) => Ok(trace.runtime()),
            Self::Original(_) => Err(Error::Invalid("compiler runtime trace is not armed")),
        }
    }
    pub(super) fn operation_quota(&self) -> (usize, usize) {
        use fe2o3_protected_service_spawn::native_spawn::RootTaskTraceV2;
        match self {
            Self::Original(_) => (
                RootTaskTraceV2::OPERATION_WORK,
                RootTaskTraceV2::OPERATION_SCRATCH,
            ),
            Self::Runtime(_) => (
                RootRuntimeTraceV1::OPERATION_WORK,
                RootRuntimeTraceV1::OPERATION_SCRATCH,
            ),
        }
    }
}
