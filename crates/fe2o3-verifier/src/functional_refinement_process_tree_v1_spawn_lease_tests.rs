//! Real artifact-lock release across the gated fork/exec boundary. Permanent
//! cases reuse the independent diagnostic subreaper; blocked workers are never
//! joined by a quarantined tracer and never counted as library cleanup.

use super::*;
use fe2o3_artifact_transaction::{EmitError, ProducerIdentity};
use std::cell::RefCell;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};

pub(super) const DIRECTORY_ENV: &str = "FE2O3_PRIVATE_SPAWN_LEASE_DIRECTORY";
const WAIT: Duration = Duration::from_secs(5);
const BLOCKED: Duration = Duration::from_millis(100);

thread_local! {
    static ARTIFACT: RefCell<Option<ArtifactRelease>> = const { RefCell::new(None) };
    static TERMINALS: RefCell<Vec<(i32, i32)>> = const { RefCell::new(Vec::new()) };
}

struct ArtifactRelease {
    path: std::path::PathBuf,
    case: String,
    held: Receiver<()>,
    release: Sender<()>,
    releasing: Receiver<()>,
    finished: Receiver<bool>,
    worker: Option<thread::JoinHandle<()>>,
    requested: bool,
    gated: Option<i32>,
}

pub(super) fn prepare_directory(case: &str) -> Option<std::path::PathBuf> {
    if !case.starts_with("lease-") {
        return None;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("fe2o3-spawn-lease-{}-{nonce}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    Some(path)
}

fn start_artifact(case: &str) {
    let path = std::path::PathBuf::from(std::env::var_os(DIRECTORY_ENV).unwrap());
    let producer = ProducerIdentity::from_codegen("spawn_lease_fixture", None).unwrap();
    let (held_tx, held) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let (releasing_tx, releasing) = mpsc::channel();
    let (finished_tx, finished) = mpsc::channel();
    let output = path.clone();
    let case = case.to_owned();
    ARTIFACT.with(|slot| assert!(slot.borrow().is_none()));
    TERMINALS.with(|observations| observations.borrow_mut().reserve_exact(MAX_TRACEES + 1));
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let worker = thread::spawn(move || {
        let result =
            fe2o3_artifact_transaction::emit_artifact_transaction_after_preflight::<(), Vec<()>>(
                &output,
                &producer,
                || {
                    // This callback runs under the real output lock. Only this
                    // worker ever destroys that transaction, never the tracer.
                    held_tx.send(()).unwrap();
                    release_rx.recv_timeout(WAIT).unwrap();
                    releasing_tx.send(()).unwrap();
                    Err(EmitError::Preflight {
                        reason: "fixture artifact release".into(),
                    })
                },
                |_| "unused",
                |_| {
                    Err(EmitError::Preflight {
                        reason: "unexpected prepare".into(),
                    })
                },
                |_, _| {
                    Err(EmitError::Preflight {
                        reason: "unexpected compile".into(),
                    })
                },
            );
        finished_tx.send(result.is_err()).unwrap();
    });
    // Publish the worker handle before any fallible observation. Unwinding
    // leaves it in TLS for the outer owned process domain, not a blocking join.
    ARTIFACT.with(|slot| {
        *slot.borrow_mut() = Some(ArtifactRelease {
            path,
            case,
            held,
            release,
            releasing,
            finished,
            worker: Some(worker),
            requested: false,
            gated: None,
        });
    });
    with_artifact(|artifact| artifact.held.recv_timeout(WAIT).unwrap());
}

fn with_artifact<T>(operation: impl FnOnce(&mut ArtifactRelease) -> T) -> T {
    ARTIFACT.with(|slot| {
        operation(
            slot.borrow_mut()
                .as_mut()
                .expect("published artifact worker"),
        )
    })
}

impl ArtifactRelease {
    fn request_release(&mut self) {
        if !self.requested {
            self.release.send(()).unwrap();
            self.releasing.recv_timeout(WAIT).unwrap();
            self.requested = true;
        }
    }

    fn blocked(&self) {
        assert_eq!(
            self.finished.recv_timeout(BLOCKED),
            Err(RecvTimeoutError::Timeout),
            "artifact transaction released before inherited aliases were discharged"
        );
    }

    fn observe_gate(&mut self, run: &Run) {
        // Begin the independent release attempt before any postfork assertion.
        self.request_release();
        let pid = run.root.expect("published real child");
        assert!(run.spawn_lease.is_some());
        assert!(!run.seized);
        assert!(!run.tracees[&pid].terminal_consumed);
        let lock = std::fs::metadata(self.path.join(".fe2o3-artifacts.lock")).unwrap();
        let inherited = std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .any(|fd| fd.dev() == lock.dev() && fd.ino() == lock.ino());
        assert!(
            inherited,
            "gated child must really inherit the artifact lock alias"
        );
        self.gated = Some(pid);
        self.blocked();
    }

    fn completed(&mut self) {
        self.request_release();
        assert!(self.finished.recv_timeout(WAIT).unwrap());
        // The completion message follows transaction destruction. Never join a
        // blocked or unconfirmed worker, including on assertion/unwind paths.
        self.worker.take().unwrap().join().unwrap();
    }
}

pub(super) fn gated_child(run: &Run) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    ARTIFACT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(artifact) = slot.as_mut() else {
            return Ok(());
        };
        artifact.observe_gate(run);
        if artifact.case == "lease-attach-terminal" {
            return Err(process_failure("fixture unseized attachment refusal"));
        }
        Ok(())
    })
}

pub(super) fn record_terminal(pid: i32, status: i32) {
    ARTIFACT.with(|slot| {
        if slot.borrow().is_some() {
            assert!(!stopped(status));
            TERMINALS.with(|observations| {
                let mut observations = observations.borrow_mut();
                assert!(observations.len() < MAX_TRACEES + 1);
                observations.push((pid, status));
            });
        }
    });
}

fn terminal_confirmed(pid: i32) {
    TERMINALS.with(|observations| {
        let observations = observations.borrow();
        assert_eq!(
            observations
                .iter()
                .filter(|(observed, _)| *observed == pid)
                .count(),
            1
        );
        println!("SPAWN_LEASE_TERMINAL_CONFIRMED {observations:?}");
    });
}

pub(super) fn exercise(case: &str) -> Vec<i32> {
    start_artifact(case);
    if matches!(case, "lease-attach-quarantine" | "lease-unwind") {
        let pids = quarantine_tests::exercise(if case == "lease-unwind" {
            "unwind-root"
        } else {
            "attach"
        });
        custody::inspect_retained(|run| {
            assert!(run.spawn_lease.is_some());
            assert!(run.tracees.unresolved());
            // Postfork unwind precedes before_attach; inspect its real gated
            // child only after error/attempt destruction retained the lease.
            if case == "lease-unwind" {
                with_artifact(|artifact| artifact.observe_gate(run));
            }
        });
        with_artifact(|artifact| artifact.blocked());
        return pids; // Independent domain teardown kills child AND blocked worker.
    }

    let mut attempt = AttemptV1::begin().unwrap();
    let deadline = if case == "lease-prefork-deadline" {
        Instant::now()
    } else {
        Instant::now() + WAIT
    };
    let mut command = Command::new("/bin/true");
    command.env_clear();
    if case == "lease-preexec-failure" {
        command.current_dir(with_artifact(|artifact| artifact.path.join("absent-cwd")));
    } else if case == "lease-exec-failure" {
        command = Command::new(with_artifact(|artifact| {
            artifact.path.join("absent-executable")
        }));
        command.env_clear();
    }
    let spawned = seized_spawn::spawn_in(&mut attempt, command, vec![], 5, deadline);
    if case == "lease-prefork-deadline" {
        assert!(spawned.is_err());
        let run = attempt.run().unwrap();
        assert!(run.root.is_none());
        assert!(run.spawn_lease.is_none());
        assert!(run.tracees.is_empty());
        with_artifact(|artifact| {
            assert!(artifact.gated.is_none());
            artifact.completed();
        });
    } else {
        let pid = attempt.id() as i32;
        with_artifact(|artifact| assert_eq!(artifact.gated, Some(pid)));
        if case == "lease-attach-terminal" {
            assert!(
                spawned
                    .unwrap_err()
                    .to_string()
                    .contains("fixture unseized attachment refusal")
            );
        } else {
            spawned.unwrap();
            // Logical spawn return is not enough, even if EXEC is already queued.
            assert!(attempt.run().unwrap().spawn_lease.is_some());
            with_artifact(|artifact| artifact.blocked());
            if case == "lease-exec" {
                let run = attempt.run().unwrap();
                seized_spawn::wait_initial_exec(
                    &mut run.tracees,
                    pid,
                    &mut run.spawn_lease,
                    deadline,
                )
                .unwrap();
                assert!(run.spawn_lease.is_none());
                let task = &run.tracees[&pid];
                assert!(!task.terminal_consumed);
                assert_eq!(
                    (task.current_stop.unwrap().status as u32) >> 16,
                    PTRACE_EVENT_EXEC
                );
                with_artifact(|artifact| artifact.completed());
                terminate_tree(&mut run.tracees).unwrap();
                run.release_spawn_after_terminal();
            } else {
                let error = supervise(
                    &mut attempt,
                    &[],
                    tests::identity("/bin/true"),
                    tests::identity("/bin/true"),
                    &[],
                    false,
                    false,
                    deadline,
                    4096,
                )
                .err()
                .expect("real preexec/exec failure must refuse proof execution");
                assert!(error.to_string().contains("initial exec boundary"));
            }
        }
        terminal_confirmed(pid);
        let run = attempt.run().unwrap();
        assert!(run.tracees[&pid].terminal_consumed);
        assert!(!run.tracees.unresolved());
        assert!(run.spawn_lease.is_none());
        if case != "lease-exec" {
            with_artifact(|artifact| artifact.completed());
        }
    }
    attempt.complete().unwrap();
    drop(attempt);
    AttemptV1::begin().unwrap().complete().unwrap();
    Vec::new() // All proof-child terminal observations already consumed above.
}

#[test]
fn artifact_release_waits_for_gated_child_until_real_exec() {
    quarantine_tests::run_domain("lease-exec");
}

#[test]
fn artifact_release_waits_for_real_terminal_on_preexec_and_attach_failure() {
    for case in [
        "lease-preexec-failure",
        "lease-exec-failure",
        "lease-attach-terminal",
    ] {
        quarantine_tests::run_domain(case);
    }
}

#[test]
fn artifact_spawn_lease_survives_quarantine_and_postfork_unwind() {
    for case in ["lease-attach-quarantine", "lease-unwind"] {
        quarantine_tests::run_domain(case);
    }
}

#[test]
fn artifact_spawn_lease_releases_on_prefork_deadline_without_child() {
    quarantine_tests::run_domain("lease-prefork-deadline");
}
