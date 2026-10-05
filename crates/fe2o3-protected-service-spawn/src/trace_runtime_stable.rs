//! Shared stopped-census scheduling, without transferable process authority.
//!
//! Implementations retain the original trace, task census, queued waits and
//! cleanup obligations. This public algorithm can be used with inert fixtures;
//! its successful return is never an execution-enforcement capability.

use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_micros(100);

/// Operations supplied by one originating-thread, exclusively owned trace.
///
/// The algorithm does not authenticate an implementation. Production callers
/// use a private adapter over their existing owner; public implementation or
/// successful return must never construct a native admission/continuity guard.
pub trait StoppedCensus {
    /// Internal task key, not an independently accepted PID or wait capability.
    type Task: Copy;
    /// Bounded owned snapshot, permitting mutation of observations during iteration.
    type Snapshot: Iterator<Item = Self::Task>;
    /// Original controller error, retaining its existing failure/cleanup contract.
    type Error;

    /// Snapshot only tasks already retained in the original lifecycle owner.
    fn tasks(&self) -> Self::Snapshot;
    /// True only for a retained stop or a queued consumed terminal observation.
    fn parked(&self, task: Self::Task) -> bool;
    /// Consume at most one original wait and retain it without overwriting a queue.
    /// Returns false only when no status was observed.
    fn observe_and_remember(&mut self, task: Self::Task) -> Result<bool, Self::Error>;
    /// Interrupt this exact retained, unreaped task; never discover it by PID.
    fn interrupt(&mut self, task: Self::Task) -> Result<(), Self::Error>;
    /// Whether a retained queued birth/exit escaped the syscall boundary.
    fn escaped_lifecycle(&self) -> bool;
    /// Refusal for a birth/exit outside the owned completion transition.
    fn lifecycle_error(&self) -> Self::Error;
    /// Refusal for the original absolute stable-boundary deadline.
    fn timeout_error(&self) -> Self::Error;
    /// Reconcile every actual sharing task with the stopped retained census.
    fn validate_census(&mut self) -> Result<(), Self::Error>;
}

fn all_parked<C: StoppedCensus>(custody: &C) -> bool {
    custody.tasks().all(|task| custody.parked(task))
}

/// Park all original sharing tasks, preserve racing observations, then reconcile
/// the actual census. Creation/exit is refused until the owning controller has
/// registered it at its authenticated syscall completion boundary.
///
/// The caller caps `deadline` with its existing stable-boundary budget and pays
/// for the adapter's finite observations. `progress` drains existing outputs and
/// enforces outer deadlines; it cannot grant process or execution authority.
pub fn park_all<C: StoppedCensus>(
    custody: &mut C,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<(), C::Error>,
) -> Result<(), C::Error> {
    for task in custody.tasks() {
        if custody.parked(task) {
            continue;
        }
        // Preserve a racing wait instead of assuming INTERRUPT created the stop.
        if !custody.observe_and_remember(task)? {
            custody.interrupt(task)?;
        }
    }
    while !all_parked(custody) {
        progress()?;
        if Instant::now() >= deadline {
            return Err(custody.timeout_error());
        }
        for task in custody.tasks() {
            if !custody.parked(task) {
                custody.observe_and_remember(task)?;
            }
        }
        if !all_parked(custody) {
            std::thread::sleep(POLL_INTERVAL);
        }
    }
    if custody.escaped_lifecycle() {
        return Err(custody.lifecycle_error());
    }
    custody.validate_census()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fixture {
        parked: [bool; 3],
        raced: [bool; 3],
        interrupted: Vec<usize>,
        waits: Vec<usize>,
        escaped: bool,
        stuck: bool,
        census: bool,
    }

    impl StoppedCensus for Fixture {
        type Task = usize;
        type Snapshot = std::ops::Range<usize>;
        type Error = &'static str;

        fn tasks(&self) -> Self::Snapshot {
            0..3
        }
        fn parked(&self, task: usize) -> bool {
            self.parked[task]
        }
        fn observe_and_remember(&mut self, task: usize) -> Result<bool, Self::Error> {
            self.waits.push(task);
            if self.raced[task] || (!self.stuck && self.interrupted.contains(&task)) {
                self.parked[task] = true;
                return Ok(true);
            }
            Ok(false)
        }
        fn interrupt(&mut self, task: usize) -> Result<(), Self::Error> {
            self.interrupted.push(task);
            Ok(())
        }
        fn escaped_lifecycle(&self) -> bool {
            self.escaped
        }
        fn lifecycle_error(&self) -> Self::Error {
            "escaped"
        }
        fn timeout_error(&self) -> Self::Error {
            "timeout"
        }
        fn validate_census(&mut self) -> Result<(), Self::Error> {
            assert!(self.parked.iter().all(|value| *value));
            self.census = true;
            Ok(())
        }
    }

    #[test]
    fn retained_and_racing_stops_are_not_interrupted_or_overwritten() {
        let mut f = Fixture {
            parked: [true, false, false],
            raced: [false, true, false],
            ..Fixture::default()
        };
        park_all(&mut f, Instant::now() + Duration::from_secs(1), &mut || {
            Ok(())
        })
        .unwrap();
        assert_eq!(f.interrupted, [2]);
        assert_eq!(f.waits, [1, 2, 2]);
        assert!(f.census);
    }

    #[test]
    fn escaped_lifecycle_refuses_before_census_success() {
        let mut f = Fixture {
            parked: [true; 3],
            escaped: true,
            ..Fixture::default()
        };
        assert_eq!(
            park_all(&mut f, Instant::now(), &mut || Ok(())),
            Err("escaped")
        );
        assert!(!f.census);
    }

    #[test]
    fn deadline_and_progress_failures_cannot_release_or_validate_tasks() {
        for expected in ["timeout", "progress"] {
            let mut f = Fixture {
                stuck: true,
                ..Fixture::default()
            };
            let mut progress = || {
                if expected == "progress" {
                    Err("progress")
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                park_all(&mut f, Instant::now(), &mut progress),
                Err(expected)
            );
            assert!(!f.census);
            assert_eq!(f.interrupted, [0, 1, 2]);
        }
    }
}
