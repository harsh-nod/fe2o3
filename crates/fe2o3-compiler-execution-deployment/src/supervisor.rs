use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, invalid, std_io_error,
};

const QUALIFICATION_WORKER_POLL_INTERVAL_V1: Duration = Duration::from_millis(25);

/// Observed termination reason for one supervised qualification worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationWorkerTerminationV1 {
    /// The worker exited before a deadline or registered termination signal was observed.
    Completed(ExitStatus),
    /// The deadline elapsed, after which the worker was killed and reaped.
    TimedOut,
    /// A registered process signal was observed, after which the worker was killed and reaped.
    Signaled(i32),
}

/// Waits for one qualification worker and guarantees reaping after timeout or signal cancellation.
///
/// `registered_signal` must contain zero or one positive operating-system signal number stored by
/// an async-signal-safe handler. Completion is checked before cancellation on every bounded poll.
/// This function owns no namespace or staging cleanup; callers perform recovery only after it
/// returns and therefore after the worker can no longer mutate deployment state.
pub fn wait_for_qualification_worker_v1(
    child: &mut Child,
    timeout: Duration,
    registered_signal: &AtomicUsize,
) -> Result<QualificationWorkerTerminationV1, DeploymentVerificationErrorV1> {
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification worker timeout exceeds the monotonic-clock range",
        )
    })?;

    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|source| std_io_error("poll qualification worker", source))?
        {
            return Ok(QualificationWorkerTerminationV1::Completed(status));
        }

        let raw_signal = registered_signal.load(Ordering::Acquire);
        if raw_signal != 0 {
            let signal = i32::try_from(raw_signal);
            terminate_and_reap_worker(child)?;
            let signal = signal.map_err(|_| {
                invalid(
                    DeploymentVerificationErrorKindV1::InvalidMetadata,
                    "registered qualification signal does not fit a signal number",
                )
            })?;
            return Ok(QualificationWorkerTerminationV1::Signaled(signal));
        }

        let now = Instant::now();
        if now >= deadline {
            terminate_and_reap_worker(child)?;
            return Ok(QualificationWorkerTerminationV1::TimedOut);
        }
        std::thread::sleep(
            QUALIFICATION_WORKER_POLL_INTERVAL_V1.min(deadline.saturating_duration_since(now)),
        );
    }
}

fn terminate_and_reap_worker(
    child: &mut Child,
) -> Result<ExitStatus, DeploymentVerificationErrorV1> {
    match child.kill() {
        Ok(()) => child
            .wait()
            .map_err(|source| std_io_error("reap terminated qualification worker", source)),
        Err(kill_source) => match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => Err(std_io_error("terminate qualification worker", kill_source)),
            Err(wait_source) => Err(super::invalid(
                DeploymentVerificationErrorKindV1::Io,
                format!(
                    "qualification worker termination failed ({kill_source}) and exit polling failed ({wait_source})"
                ),
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn sleeping_child() -> Child {
        Command::new("/bin/sleep").arg("30").spawn().unwrap()
    }

    #[test]
    fn completed_worker_preserves_its_exact_exit_status() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .spawn()
            .unwrap();
        let signal = AtomicUsize::new(0);
        let outcome =
            wait_for_qualification_worker_v1(&mut child, Duration::from_secs(5), &signal).unwrap();

        let QualificationWorkerTerminationV1::Completed(status) = outcome else {
            panic!("worker did not report normal completion");
        };
        assert_eq!(status.code(), Some(7));
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn elapsed_deadline_kills_and_reaps_worker() {
        let mut child = sleeping_child();
        let signal = AtomicUsize::new(0);

        assert_eq!(
            wait_for_qualification_worker_v1(&mut child, Duration::from_millis(10), &signal,)
                .unwrap(),
            QualificationWorkerTerminationV1::TimedOut
        );
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn registered_signal_kills_and_reaps_worker() {
        let mut child = sleeping_child();
        let signal = AtomicUsize::new(15);

        assert_eq!(
            wait_for_qualification_worker_v1(&mut child, Duration::from_secs(5), &signal).unwrap(),
            QualificationWorkerTerminationV1::Signaled(15)
        );
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn invalid_registered_signal_still_kills_and_reaps_worker() {
        let mut child = sleeping_child();
        let signal = AtomicUsize::new(usize::MAX);

        assert_eq!(
            wait_for_qualification_worker_v1(&mut child, Duration::from_secs(5), &signal)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );
        assert!(child.try_wait().unwrap().is_some());
    }
}
