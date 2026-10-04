//! Private borrow-first registration; rejected payload ownership goes back to caller.
use super::*;

impl ObservedSession {
    #[expect(
        clippy::result_large_err,
        reason = "Rejected predicates return ownership without allocation, traversal, or drop"
    )]
    pub(in super::super) fn add_breakpoint(
        &mut self,
        breakpoint: DebugBreakpointV1,
        work: &mut ReplayWork,
    ) -> Result<(), RejectedBreakpoint> {
        let (count, hits) = match self.prepare_breakpoint(&breakpoint, work) {
            Ok(prepared) => prepared,
            Err(error) => return Err(RejectedBreakpoint { error, breakpoint }),
        };
        // Both vectors have reserved their next slot; no fallible check, clone,
        // predicate traversal or user-payload drop follows ownership transfer.
        let index = self.session.breakpoints.len();
        self.session.breakpoints.push(breakpoint);
        self.session.breakpoint_hits.push(hits);
        self.predicate_nodes[index] = count;
        Ok(())
    }

    fn prepare_breakpoint(
        &mut self,
        breakpoint: &DebugBreakpointV1,
        work: &mut ReplayWork,
    ) -> Result<(usize, u64), SessionError> {
        if self.session.breakpoints.len() == MAX_FILTERS {
            return Err(SessionError::FilterLimit);
        }
        // Preserve the existing envelope and ordering: first validation, node
        // count, prefix precharge, then legacy ID/second-validation/hit checks.
        work.charge(3 * (MAX_DEBUGGER_PREDICATE_NODES_V1 + 1) + MAX_FILTERS)?;
        breakpoint
            .predicate
            .validate()
            .map_err(SessionError::Debugger)?;
        let count = nodes(&breakpoint.predicate);
        for record in &self.session.transcript.records[..self.session.cursor_prefix_len()] {
            work.charge(predicate_work(record, count)?)?;
        }
        if breakpoint.id == 0
            || self
                .session
                .breakpoints
                .iter()
                .any(|value| value.id == breakpoint.id)
        {
            return Err(SessionError::Debugger(
                DebuggerErrorV1::InvalidOrDuplicateIdentity,
            ));
        }
        breakpoint
            .predicate
            .validate()
            .map_err(SessionError::Debugger)?;
        if breakpoint
            .hit_condition
            .is_some_and(|value| !value.is_valid())
        {
            return Err(SessionError::Debugger(DebuggerErrorV1::InvalidHitCondition));
        }
        // Same fallible vector reservations as DebugSessionV1::add_breakpoint.
        // A second-reservation failure may retain first-vector capacity, but
        // no breakpoint/counter/cursor/sidecar content changes or payload move.
        self.session
            .breakpoints
            .try_reserve(1)
            .map_err(|_| SessionError::Debugger(DebuggerErrorV1::AllocationFailure))?;
        self.session
            .breakpoint_hits
            .try_reserve(1)
            .map_err(|_| SessionError::Debugger(DebuggerErrorV1::AllocationFailure))?;
        let hits = self.session.breakpoint_hits_through_cursor(breakpoint);
        Ok((count, hits))
    }
}

#[path = "session_registration_tests.rs"]
mod tests;
