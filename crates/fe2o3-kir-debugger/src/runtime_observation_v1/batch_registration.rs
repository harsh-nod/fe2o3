//! Atomic, bounded batch registration without unbounded rejected-predicate formatting.
use super::*;

pub struct RuntimeRejectedBreakpointsV1 {
    error: RuntimeSessionErrorV1,
    breakpoints: Vec<DebugBreakpointV1>,
}
impl std::fmt::Debug for RuntimeRejectedBreakpointsV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeRejectedBreakpointsV1")
            .field("error", &self.error)
            .field("payload", &"caller-owned; not inspected")
            .finish()
    }
}
impl RuntimeRejectedBreakpointsV1 {
    pub fn error(&self) -> &RuntimeSessionErrorV1 {
        &self.error
    }
    pub fn into_parts(self) -> (RuntimeSessionErrorV1, Vec<DebugBreakpointV1>) {
        (self.error, self.breakpoints)
    }
}

impl DebugObservedSessionV1 {
    pub fn add_breakpoints_atomic(
        &mut self,
        breakpoints: Vec<DebugBreakpointV1>,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<(), RuntimeRejectedBreakpointsV1> {
        let prepared = self.prepare_breakpoints(&breakpoints, work);
        let (counts, hits) = match prepared {
            Ok(value) => value,
            Err(error) => return Err(RuntimeRejectedBreakpointsV1 { error, breakpoints }),
        };
        let start = self.session.breakpoints.len();
        let count = breakpoints.len();
        self.session.breakpoints.extend(breakpoints);
        self.session
            .breakpoint_hits
            .extend_from_slice(&hits[..count]);
        self.predicate_nodes[start..start + count].copy_from_slice(&counts[..count]);
        Ok(())
    }
    fn prepare_breakpoints(
        &mut self,
        breakpoints: &[DebugBreakpointV1],
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<([usize; MAX_FILTERS], [u64; MAX_FILTERS]), RuntimeSessionErrorV1> {
        if breakpoints.len() > MAX_FILTERS.saturating_sub(self.session.breakpoints.len()) {
            return Err(RuntimeSessionErrorV1::FilterLimit);
        }
        let mut counts = [0; MAX_FILTERS];
        let mut hits = [0; MAX_FILTERS];
        for (index, breakpoint) in breakpoints.iter().enumerate() {
            work.charge(MAX_FILTERS)?;
            if breakpoints[..index]
                .iter()
                .any(|prior| prior.id == breakpoint.id)
            {
                return Err(RuntimeSessionErrorV1::Debugger(
                    DebuggerErrorV1::InvalidOrDuplicateIdentity,
                ));
            }
            (counts[index], hits[index]) = self.prepare_breakpoint(breakpoint, work)?;
        }
        self.session
            .breakpoints
            .try_reserve(breakpoints.len())
            .map_err(|_| RuntimeSessionErrorV1::Debugger(DebuggerErrorV1::AllocationFailure))?;
        self.session
            .breakpoint_hits
            .try_reserve(breakpoints.len())
            .map_err(|_| RuntimeSessionErrorV1::Debugger(DebuggerErrorV1::AllocationFailure))?;
        Ok((counts, hits))
    }
    pub fn add_watchpoints_atomic(
        &mut self,
        watchpoints: Vec<DebugWatchpointV1>,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<(), RuntimeSessionErrorV1> {
        if watchpoints.len() > MAX_FILTERS.saturating_sub(self.session.watchpoints.len()) {
            return Err(RuntimeSessionErrorV1::FilterLimit);
        }
        let units = 8_usize
            .checked_mul(watchpoints.len())
            .and_then(|value| value.checked_mul(self.session.cursor_prefix_len()))
            .and_then(|value| value.checked_add(MAX_FILTERS * MAX_FILTERS))
            .ok_or(RuntimeSessionErrorV1::WorkLimit)?;
        work.charge(units)?;
        self.session
            .add_watchpoints_atomic(watchpoints)
            .map_err(RuntimeSessionErrorV1::Debugger)
    }
}
