use std::fs::File;
use std::os::fd::AsFd as _;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rustix::fs::{FlockOperation, flock, fstat};
use rustix::process::{
    Pid, PidfdFlags, Signal, WaitId, WaitIdOptions, getpgid, kill_process_group, pidfd_open, waitid,
};

use super::install::{open_install_parent, verify_install_parent_path};
use super::qualification::open_qualification_parent_metadata;
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, changed, invalid, io_error,
    snapshot, std_io_error,
};

const QUALIFICATION_WORKER_POLL_INTERVAL_V1: Duration = Duration::from_millis(25);

/// Move-only exclusive custody preventing concurrent mutation of two qualification parents.
///
/// The retained descriptors and lock operations are private. Dropping this value closes both
/// descriptors and releases both advisory locks. All production qualification supervisors and
/// workers acquire this lease before recovery or transaction mutation.
pub struct CompilerExecutionQualificationSupervisorLeaseV1 {
    _install_parent: File,
    _qualification_parent: File,
}

/// Acquires the exclusive qualification-parent lease without waiting for another owner.
///
/// The process must have effective UID 0. Both paths must identify distinct root-owned mode-`0700`
/// directories without extended attributes. Locks are acquired in stable device/inode order.
pub fn acquire_compiler_execution_qualification_supervisor_lease_v1(
    install_parent: &Path,
    qualification_parent: &Path,
) -> Result<CompilerExecutionQualificationSupervisorLeaseV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "qualification supervisor lease requires effective UID 0",
        ));
    }
    acquire_supervisor_lease_for_owner(
        install_parent,
        qualification_parent,
        (0, 0),
        FlockOperation::NonBlockingLockExclusive,
    )
}

/// Waits to acquire the exclusive qualification-parent lease for a supervised worker.
///
/// This has the same parent policy as the nonblocking acquisition function. Stable lock ordering
/// prevents cooperating workers from deadlocking while ownership transfers from parent to child.
pub fn wait_for_compiler_execution_qualification_supervisor_lease_v1(
    install_parent: &Path,
    qualification_parent: &Path,
) -> Result<CompilerExecutionQualificationSupervisorLeaseV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "qualification worker lease requires effective UID 0",
        ));
    }
    acquire_supervisor_lease_for_owner(
        install_parent,
        qualification_parent,
        (0, 0),
        FlockOperation::LockExclusive,
    )
}

fn acquire_supervisor_lease_for_owner(
    install_parent_path: &Path,
    qualification_parent_path: &Path,
    owner: (u32, u32),
    operation: FlockOperation,
) -> Result<CompilerExecutionQualificationSupervisorLeaseV1, DeploymentVerificationErrorV1> {
    let install_parent = open_install_parent(install_parent_path, owner)?;
    let qualification_parent =
        open_qualification_parent_metadata(qualification_parent_path, owner)?;
    let install_snapshot = snapshot(
        &fstat(&install_parent)
            .map_err(|source| io_error("inspect leased install parent", source))?,
    );
    let qualification_snapshot = snapshot(
        &fstat(&qualification_parent)
            .map_err(|source| io_error("inspect leased qualification parent", source))?,
    );
    let install_key = (install_snapshot.device, install_snapshot.inode);
    let qualification_key = (qualification_snapshot.device, qualification_snapshot.inode);
    if install_key == qualification_key {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "install and qualification parents must be distinct directories",
        ));
    }
    let (first, second) = if install_key < qualification_key {
        (&install_parent, &qualification_parent)
    } else {
        (&qualification_parent, &install_parent)
    };
    flock(first, operation)
        .map_err(|source| io_error("acquire first qualification supervisor lease", source))?;
    flock(second, operation)
        .map_err(|source| io_error("acquire second qualification supervisor lease", source))?;
    verify_install_parent_path(install_parent_path, &install_parent, owner)?;
    revalidate_qualification_parent_path(
        qualification_parent_path,
        &qualification_parent,
        qualification_snapshot,
        owner,
    )?;
    Ok(CompilerExecutionQualificationSupervisorLeaseV1 {
        _install_parent: install_parent,
        _qualification_parent: qualification_parent,
    })
}

fn revalidate_qualification_parent_path(
    path: &Path,
    retained: &File,
    expected: super::ObjectSnapshotV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    let retained = snapshot(
        &fstat(retained)
            .map_err(|source| io_error("reinspect leased qualification parent", source))?,
    );
    let reopened = open_qualification_parent_metadata(path, owner)?;
    let reopened =
        snapshot(&fstat(&reopened).map_err(|source| {
            io_error("reinspect canonical leased qualification parent", source)
        })?);
    if (
        retained.device,
        retained.inode,
        retained.mode,
        retained.uid,
        retained.gid,
    ) != (
        expected.device,
        expected.inode,
        expected.mode,
        expected.uid,
        expected.gid,
    ) || (
        reopened.device,
        reopened.inode,
        reopened.mode,
        reopened.uid,
        reopened.gid,
    ) != (
        expected.device,
        expected.inode,
        expected.mode,
        expected.uid,
        expected.gid,
    ) {
        return Err(changed(
            "qualification-parent identity changed during lease acquisition",
        ));
    }
    Ok(())
}

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

/// Waits for one qualification worker and guarantees process-group termination before reaping.
///
/// `registered_signal` must contain zero or one positive operating-system signal number stored by
/// an async-signal-safe handler. Completion is checked before cancellation on every bounded poll.
/// The child must be the leader of a dedicated process group. A pidfd observes exit without
/// reaping the leader, so the group identity cannot be reused before all descendants are killed.
/// This function owns no namespace or staging cleanup; callers perform recovery only after it
/// returns and therefore after the worker and every descendant can no longer mutate deployment
/// state.
pub fn wait_for_qualification_worker_v1(
    child: &mut Child,
    timeout: Duration,
    registered_signal: &AtomicUsize,
) -> Result<QualificationWorkerTerminationV1, DeploymentVerificationErrorV1> {
    let worker_pid = Pid::from_child(child);
    let worker_pidfd = pidfd_open(worker_pid, PidfdFlags::empty()).map_err(|source| {
        terminate_single_worker_after_admission_failure(child);
        io_error("open qualification worker pidfd", source)
    })?;
    let process_group = getpgid(Some(worker_pid)).map_err(|source| {
        terminate_single_worker_after_admission_failure(child);
        io_error("inspect qualification worker process group", source)
    })?;
    if process_group != worker_pid {
        terminate_single_worker_after_admission_failure(child);
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification worker is not its dedicated process-group leader",
        ));
    }
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        let _ = terminate_and_reap_worker_group(child, worker_pid);
        invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification worker timeout exceeds the monotonic-clock range",
        )
    })?;

    loop {
        let exited = waitid(
            WaitId::PidFd(worker_pidfd.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .map_err(|source| {
            let cleanup = terminate_and_reap_worker_group(child, worker_pid);
            match cleanup {
                Ok(_) => io_error("poll qualification worker pidfd", source),
                Err(cleanup) => invalid(
                    DeploymentVerificationErrorKindV1::CleanupFailed,
                    format!(
                        "poll qualification worker pidfd failed ({source}); process-group cleanup also failed: {cleanup}"
                    ),
                ),
            }
        })?
        .is_some();
        if exited {
            let status = terminate_and_reap_worker_group(child, worker_pid)?;
            return Ok(QualificationWorkerTerminationV1::Completed(status));
        }

        let raw_signal = registered_signal.load(Ordering::Acquire);
        if raw_signal != 0 {
            let signal = i32::try_from(raw_signal);
            terminate_and_reap_worker_group(child, worker_pid)?;
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
            terminate_and_reap_worker_group(child, worker_pid)?;
            return Ok(QualificationWorkerTerminationV1::TimedOut);
        }
        std::thread::sleep(
            QUALIFICATION_WORKER_POLL_INTERVAL_V1.min(deadline.saturating_duration_since(now)),
        );
    }
}

fn terminate_and_reap_worker_group(
    child: &mut Child,
    worker_pid: Pid,
) -> Result<ExitStatus, DeploymentVerificationErrorV1> {
    match kill_process_group(worker_pid, Signal::KILL) {
        Ok(()) => child.wait().map_err(|source| {
            std_io_error("reap terminated qualification worker process group", source)
        }),
        Err(kill_source) => match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => Err(io_error(
                "terminate qualification worker process group",
                kill_source,
            )),
            Err(wait_source) => Err(super::invalid(
                DeploymentVerificationErrorKindV1::Io,
                format!(
                    "qualification worker process-group termination failed ({kill_source}) and exit polling failed ({wait_source})"
                ),
            )),
        },
    }
}

fn terminate_single_worker_after_admission_failure(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::process::CommandExt as _;
    use std::path::PathBuf;
    use std::process::Command;

    use super::*;

    fn sleeping_child() -> Child {
        let mut command = Command::new("/bin/sleep");
        command.arg("30").process_group(0);
        command.spawn().unwrap()
    }

    fn owner() -> (u32, u32) {
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
    }

    fn lease_parents() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let install = temporary.path().join("install");
        let qualification = temporary.path().join("qualification");
        for parent in [&install, &qualification] {
            fs::create_dir(parent).unwrap();
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        }
        (temporary, install, qualification)
    }

    #[test]
    fn lease_excludes_concurrent_supervisors_and_releases_on_drop() {
        let (_temporary, install, qualification) = lease_parents();
        let lease = acquire_supervisor_lease_for_owner(
            &install,
            &qualification,
            owner(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
        let error = acquire_supervisor_lease_for_owner(
            &install,
            &qualification,
            owner(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), DeploymentVerificationErrorKindV1::Io);
        drop(lease);
        acquire_supervisor_lease_for_owner(
            &install,
            &qualification,
            owner(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
    }

    #[test]
    fn blocking_lease_transfers_only_after_current_owner_drops() {
        let (_temporary, install, qualification) = lease_parents();
        let parent = acquire_supervisor_lease_for_owner(
            &install,
            &qualification,
            owner(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let result = acquire_supervisor_lease_for_owner(
                &install,
                &qualification,
                owner(),
                FlockOperation::LockExclusive,
            );
            assert!(sender.send(result).is_ok());
        });

        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        drop(parent);
        let worker_lease = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(worker_lease.is_ok());
        drop(worker_lease);
        worker.join().unwrap();
    }

    #[test]
    fn lease_rejects_one_directory_for_both_roles() {
        let (_temporary, install, _qualification) = lease_parents();
        let error = acquire_supervisor_lease_for_owner(
            &install,
            &install,
            owner(),
            FlockOperation::NonBlockingLockExclusive,
        )
        .err()
        .unwrap();
        assert_eq!(
            error.kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );
    }

    #[test]
    fn completed_worker_preserves_its_exact_exit_status() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .process_group(0)
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

    #[test]
    fn elapsed_deadline_kills_worker_descendants_before_reaping() {
        let temporary = tempfile::tempdir().unwrap();
        let descendant = temporary.path().join("descendant");
        let script = format!("sleep 30 & echo $! > '{}'; wait", descendant.display());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", &script]).process_group(0);
        let mut child = command.spawn().unwrap();
        let descendant_pid = read_descendant_pid(&descendant);
        let signal = AtomicUsize::new(0);

        assert_eq!(
            wait_for_qualification_worker_v1(&mut child, Duration::from_millis(10), &signal)
                .unwrap(),
            QualificationWorkerTerminationV1::TimedOut
        );
        wait_until_process_is_absent(descendant_pid);
    }

    #[test]
    fn completed_worker_cannot_leave_a_descendant_group_alive() {
        let temporary = tempfile::tempdir().unwrap();
        let descendant = temporary.path().join("descendant");
        let script = format!("sleep 30 & echo $! > '{}'; exit 7", descendant.display());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", &script]).process_group(0);
        let mut child = command.spawn().unwrap();
        let signal = AtomicUsize::new(0);

        let outcome =
            wait_for_qualification_worker_v1(&mut child, Duration::from_secs(5), &signal).unwrap();
        let QualificationWorkerTerminationV1::Completed(status) = outcome else {
            panic!("worker did not report normal completion");
        };
        assert_eq!(status.code(), Some(7));
        let descendant_pid = read_descendant_pid(&descendant);
        wait_until_process_is_absent(descendant_pid);
    }

    #[test]
    fn worker_without_dedicated_process_group_is_rejected_and_reaped() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let signal = AtomicUsize::new(0);

        assert_eq!(
            wait_for_qualification_worker_v1(&mut child, Duration::from_secs(5), &signal)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation
        );
        assert!(child.try_wait().unwrap().is_some());
    }

    fn wait_until_process_is_absent(raw_pid: i32) {
        let pid = Pid::from_raw(raw_pid).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match rustix::process::test_kill_process(pid) {
                Err(rustix::io::Errno::SRCH) => return,
                Err(error) => panic!("cannot inspect descendant process: {error}"),
                Ok(()) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(()) => panic!("qualification worker descendant remained alive"),
            }
        }
    }

    fn read_descendant_pid(path: &Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(contents) = fs::read_to_string(path)
                && let Ok(pid) = contents.trim().parse::<i32>()
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "qualification descendant did not publish its PID"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
