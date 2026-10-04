impl Service {
    /// Fixed controller bookkeeping plus one independently charged session per turn.
    pub const DISPATCH_TURN_WORK: usize = 16;
    /// Logical temporary controller metadata; returned inert metadata is prepaid at bind.
    pub const DISPATCH_SCRATCH: usize = 4 * size_of::<DispatchReport>();

    /// Runs a finite sequential prefix on the original accounts, without new authority.
    ///
    /// Every dispatched outcome is followed by a separately funded cleanup pump,
    /// including session-resource refusal. No next connection is accepted unless
    /// cleanup admission is open and the selected post-dispatch pump is fundable.
    /// This returns at the first non-idle failure; callers may inspect it and make
    /// another bounded call without replacing either cumulative account.
    ///
    /// The cleanup controller remains borrowed, not consumed or shut down. On any
    /// return the caller must retain it and drain outstanding custody explicitly.
    /// Neither a completed batch nor controller Drop proves terminal child cleanup.
    pub fn run_turns(
        &self,
        limits: DispatchLimits,
        cleanup: &mut Cleanup,
        budget: &mut Budget<'_>,
    ) -> DispatchReport {
        dispatch_turns(self.retained, limits, cleanup, budget, |cleanup, budget| {
            self.serve_one(limits.accept, cleanup, budget)
        })
    }
}

// The private schedule is shared by real dispatch and deterministic accounting
// tests. Only the public method above supplies service/session authority.
fn dispatch_turns(
    retained: usize,
    limits: DispatchLimits,
    cleanup: &mut Cleanup,
    budget: &mut Budget<'_>,
    mut dispatch: impl FnMut(&mut Cleanup, &mut Budget<'_>) -> Result<Report>,
) -> DispatchReport {
    let result = budget.with_prepaid_scope(
        retained,
        ENTRY,
        ENTRY + limits.turns * Service::DISPATCH_TURN_WORK,
        Service::DISPATCH_SCRATCH,
        |budget| {
            let mut report = DispatchReport {
                turns: 0,
                completed: 0,
                idle: 0,
                last_completion: None,
                stop: DispatchStop::TurnLimit,
                cleanup: cleanup.report(),
            };
            for _ in 0..limits.turns {
                let current = match report.cleanup {
                    Ok(current) => current,
                    Err(_) => {
                        report.stop = DispatchStop::Cleanup;
                        return Ok::<_, Resource>(report);
                    }
                };
                let pump_work = Cleanup::TURN_WORK + limits.cleanup_visits * Cleanup::CELL_WORK;
                if !current.admission_open
                    || current.work_limit.saturating_sub(current.work) < pump_work
                {
                    report.stop = DispatchStop::AdmissionStopped;
                    report.cleanup = cleanup.pump(limits.cleanup_visits);
                    return Ok(report);
                }
                report.turns += 1;
                match dispatch(cleanup, budget) {
                    Ok(completed) => {
                        report.completed += 1;
                        report.last_completion = Some(completed);
                    }
                    Err(Error::AcceptTimeout | Error::AcceptAttempts) => report.idle += 1,
                    Err(error) => report.stop = DispatchStop::Dispatch(error),
                }
                report.cleanup = cleanup.pump(limits.cleanup_visits);
                if matches!(report.stop, DispatchStop::Dispatch(_)) {
                    return Ok(report);
                }
                if report.cleanup.is_err() {
                    report.stop = DispatchStop::Cleanup;
                    return Ok(report);
                }
            }
            Ok(report)
        },
    );
    match result {
        Ok(report) => report,
        Err(error) => DispatchReport {
            turns: 0,
            completed: 0,
            idle: 0,
            last_completion: None,
            stop: DispatchStop::Dispatch(Error::Resource(error)),
            cleanup: cleanup.pump(limits.cleanup_visits),
        },
    }
}

#[cfg(test)]
#[path = "listener_native_controller_tests.rs"]
mod controller_tests;
