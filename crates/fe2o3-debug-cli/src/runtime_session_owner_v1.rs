//! Private owner of either a legacy capture or one sealed observed capture.
//! Immutable Deref supports existing inspectors; there is deliberately no DerefMut.
use super::*;
use fe2o3_kir_debugger::{
    DebugObservedSessionV1, DebuggerErrorV1, RuntimeNavigationDirectionV1, RuntimeReplayWorkV1,
    RuntimeSessionErrorV1,
};

const COMMAND_WORK: usize = 64_000_000;
#[cfg(test)]
#[path = "runtime_session_owner_tests.rs"]
mod tests;
enum Inner {
    Legacy(Box<DebugSessionV1>),
    Observed(Box<DebugObservedSessionV1>),
}
pub(super) struct SessionOwnerV1 {
    inner: Inner,
    work: RuntimeReplayWorkV1,
    failure: Option<RuntimeSessionErrorV1>,
}
impl From<DebugSessionV1> for SessionOwnerV1 {
    fn from(session: DebugSessionV1) -> Self {
        Self {
            inner: Inner::Legacy(Box::new(session)),
            work: budget(),
            failure: None,
        }
    }
}
impl From<DebugObservedSessionV1> for SessionOwnerV1 {
    fn from(session: DebugObservedSessionV1) -> Self {
        Self {
            inner: Inner::Observed(Box::new(session)),
            work: budget(),
            failure: None,
        }
    }
}
fn budget() -> RuntimeReplayWorkV1 {
    RuntimeReplayWorkV1::new(COMMAND_WORK).expect("fixed CLI work limit is valid")
}
impl std::ops::Deref for SessionOwnerV1 {
    type Target = DebugSessionV1;
    fn deref(&self) -> &Self::Target {
        match &self.inner {
            Inner::Legacy(session) => session,
            Inner::Observed(session) => session.legacy(),
        }
    }
}
impl SessionOwnerV1 {
    pub(super) fn observed(&self) -> Option<&DebugObservedSessionV1> {
        match &self.inner {
            Inner::Legacy(_) => None,
            Inner::Observed(session) => Some(session),
        }
    }
    pub(super) fn has_control_failure(&self) -> bool {
        self.failure.is_some()
    }
    pub(super) fn begin_command(&mut self) {
        self.work = budget();
        self.failure = None;
    }
    fn remember(&mut self, error: RuntimeSessionErrorV1) {
        // If rollback itself exhausts work, retain that decisive cause instead
        // of hiding it behind the earlier semantic refusal.
        if self.failure.is_none() || matches!(error, RuntimeSessionErrorV1::WorkLimit) {
            self.failure = Some(error);
        }
    }
    fn navigation(
        &mut self,
        result: Result<DebugNavigationV1, RuntimeSessionErrorV1>,
    ) -> DebugNavigationV1 {
        match result {
            Ok(value) => value,
            Err(error) => {
                self.remember(error);
                // Internal sentinel only. The outer command handler replaces it
                // with a typed failure and the actual state_changed bit.
                DebugNavigationV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord)
            }
        }
    }
    pub(super) fn charge_scan(&mut self, rows: usize) -> bool {
        if self.observed().is_none() {
            return true;
        }
        if self.failure.is_some() {
            return false;
        }
        match self.work.charge(rows) {
            Ok(()) => true,
            Err(error) => {
                self.remember(error);
                false
            }
        }
    }
    pub(super) fn bind_source_catalog(
        &mut self,
        module: &AdmittedSimulationModuleV1,
        catalog: DebugSourceCatalogV1,
    ) -> Result<(), DebuggerErrorV1> {
        match &mut self.inner {
            Inner::Legacy(session) => session.bind_source_catalog(module, catalog),
            Inner::Observed(session) => session.bind_source_catalog(module, catalog),
        }
    }
    pub(super) fn seek_record_index(&mut self, index: usize) -> DebugNavigationV1 {
        let result = match &mut self.inner {
            Inner::Legacy(session) => return session.seek_record_index(index),
            Inner::Observed(session) => session.seek_record(index, &mut self.work),
        };
        self.navigation(result)
    }
    pub(super) fn seek_entry(&mut self) -> DebugNavigationV1 {
        let result = match &mut self.inner {
            Inner::Legacy(session) => return session.seek_entry(),
            Inner::Observed(session) => session.seek_entry(&mut self.work),
        };
        self.navigation(result)
    }
    pub(super) fn continue_forward_bounded(&mut self, maximum: usize) -> DebugNavigationV1 {
        let result = match &mut self.inner {
            Inner::Legacy(session) => return session.continue_forward_bounded(maximum),
            Inner::Observed(session) => session.continue_to_stop_bounded(
                RuntimeNavigationDirectionV1::Forward,
                maximum,
                &mut self.work,
            ),
        };
        self.navigation(result)
    }
    pub(super) fn frame_step(
        &mut self,
        direction: StepDirectionV1,
        out: bool,
        scope: &DebugScopeSelectorV1,
    ) -> DebugNavigationV1 {
        let width = self.transcript().wave_width();
        let focus = self.current().map(|record| record.invocation);
        let result = match &mut self.inner {
            Inner::Legacy(session) => {
                return if out {
                    session.step_out(scope)
                } else {
                    session.step_over(scope)
                };
            }
            Inner::Observed(session) => {
                let Some(focus) = focus.filter(|value| scope.matches(*value, width)) else {
                    return self.navigation(Err(RuntimeSessionErrorV1::FocusMismatch));
                };
                let direction = match direction {
                    StepDirectionV1::Forward => RuntimeNavigationDirectionV1::Forward,
                    StepDirectionV1::Reverse => RuntimeNavigationDirectionV1::Reverse,
                };
                if out {
                    session.step_out(direction, focus, &mut self.work)
                } else {
                    session.step_over(direction, focus, &mut self.work)
                }
            }
        };
        self.navigation(result)
    }
    pub(super) fn step_over(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        self.frame_step(StepDirectionV1::Forward, false, scope)
    }
    pub(super) fn step_out(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        self.frame_step(StepDirectionV1::Forward, true, scope)
    }
    pub(super) fn add_breakpoints_atomic(
        &mut self,
        values: Vec<DebugBreakpointV1>,
    ) -> Result<(), RuntimeSessionErrorV1> {
        let result = match &mut self.inner {
            Inner::Legacy(session) => session
                .add_breakpoints_atomic(values)
                .map_err(RuntimeSessionErrorV1::Debugger),
            Inner::Observed(session) => session
                .add_breakpoints_atomic(values, &mut self.work)
                .map_err(|error| error.into_parts().0),
        };
        if let Err(error) = &result {
            self.remember(error.clone());
        }
        result
    }
    pub(super) fn add_watchpoints_atomic(
        &mut self,
        values: Vec<DebugWatchpointV1>,
    ) -> Result<(), RuntimeSessionErrorV1> {
        let result = match &mut self.inner {
            Inner::Legacy(session) => session
                .add_watchpoints_atomic(values)
                .map_err(RuntimeSessionErrorV1::Debugger),
            Inner::Observed(session) => session.add_watchpoints_atomic(values, &mut self.work),
        };
        if let Err(error) = &result {
            self.remember(error.clone());
        }
        result
    }
    pub(super) fn remove_breakpoint(&mut self, id: u64) -> bool {
        let result = match &mut self.inner {
            Inner::Legacy(session) => return session.remove_breakpoint(id),
            Inner::Observed(session) => session.remove_breakpoint(id, &mut self.work),
        };
        match result {
            Ok(value) => value,
            Err(error) => {
                self.remember(error);
                false
            }
        }
    }
    pub(super) fn remove_watchpoint(&mut self, id: u64) -> bool {
        let result = match &mut self.inner {
            Inner::Legacy(session) => return session.remove_watchpoint(id),
            Inner::Observed(session) => session.remove_watchpoint(id, &mut self.work),
        };
        match result {
            Ok(value) => value,
            Err(error) => {
                self.remember(error);
                false
            }
        }
    }
}
impl SimulatorBackendV1 {
    pub(super) fn finish_observed_command(
        &mut self,
        request_id: u64,
        operation: DebugOperationNameV1,
        before_cursor: u64,
        before_revision: u64,
        response: DebugResponseV1,
    ) -> DebugResponseV1 {
        if self.session.observed().is_none() {
            self.session.failure = None;
            return response;
        }
        let Some(failure) = self.session.failure.take() else {
            return response;
        };
        let changed = before_cursor != self.cursor_sequence() || before_revision != self.revision;
        if changed {
            // V1 errors promise no state change. A bounded replay that actually
            // progressed must report its current cursor as a non-exact control
            // stop, never an error or an invented semantic breakpoint.
            let limited = matches!(
                failure,
                RuntimeSessionErrorV1::WorkLimit
                    | RuntimeSessionErrorV1::Origin(_)
                    | RuntimeSessionErrorV1::Frames(_)
                    | RuntimeSessionErrorV1::Incomplete
                    | RuntimeSessionErrorV1::Bounds
            );
            if self.revision == before_revision && self.bump_revision().is_err() {
                self.terminated = true;
            }
            self.terminated |= !limited;
            let stop = StopViewV1 {
                reason: if self.terminated {
                    StopReasonV1::Terminated
                } else {
                    StopReasonV1::ResourceExhaustion
                },
                breakpoint_id: None,
                watchpoint_id: None,
                outcome: if self.terminated {
                    ExecutionOutcomeV1::Cancelled
                } else {
                    ExecutionOutcomeV1::Active
                },
                exact: false,
            };
            self.last_stop = Some(stop.clone());
            return self.ok(
                request_id,
                operation,
                DebugResultV1::Control {
                    stop: Some(stop),
                    snapshot: self.snapshot_availability(),
                    events_advanced: before_cursor.abs_diff(self.cursor_sequence()),
                },
            );
        }
        DebugResponseV1::Error {
            schema: ResponseSchemaV1::V1,
            request_id: Some(request_id),
            operation: Some(operation),
            session: Some(self.session_view()),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Backend,
                code: if matches!(
                    failure,
                    RuntimeSessionErrorV1::WorkLimit
                        | RuntimeSessionErrorV1::FilterLimit
                        | RuntimeSessionErrorV1::Bounds
                ) {
                    DebugErrorCodeV1::ResourceLimit
                } else {
                    DebugErrorCodeV1::BackendFailure
                },
                message: bounded_message(&format!("observed replay control refused: {failure:?}")),
                state_changed: false,
            },
        }
    }
}
