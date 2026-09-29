use super::*;
use fe2o3_broker_authority_service::RootConnectionErrorV3 as ConnectionError;
use fe2o3_protected_service_profile::{
    ProtectedServiceProfileErrorV2 as ProfileError, observations::Error as ObservationError,
};
use rustix::process::{self, Signal, WaitId, WaitIdOptions};

type Issuer<'work> = ManagedIssuer<'work, compiler::Backing>;
type Trace<'work> = CompilerTrace<'work, compiler::Backing>;
type Attempt<'work> = NativeAttempt<'work, compiler::Backing>;

pub(super) fn additional_work(case: &str, continuity: Quota, transfer: Quota) -> usize {
    let existing = match case {
        // One readiness refusal, one direct connection refusal, one held-trace
        // poll (bounded by OBSERVATION_WORK), a pidfd clone and a liveness check.
        "ready-issuer-exit" => sum(&[
            2 * continuity.work(),
            Trace::OBSERVATION_WORK,
            2 * PlainChild::OPERATION_WORK,
        ])
        .unwrap(),
        // Validate while stopped, then refuse both immediately after compiler
        // cancellation and after reap. Also exercise the real cancellation guard
        // after a successful original-root join but failed outer accounting exit.
        "ready-compiler-cancel" => sum(&[
            3 * continuity.work(),
            3 * PlainChild::OPERATION_WORK,
            Attempt::original_validation_quota().work(),
        ])
        .unwrap(),
        _ => 0,
    };
    let removal = if matches!(case, "ready-cancel" | "ready-unwind" | "ready-issuer-exit") {
        sum(&[
            3 * Attempt::original_validation_quota().work(),
            transfer.work(),
            Trace::OBSERVATION_WORK,
            PlainChild::OPERATION_WORK,
            LOCAL_WORK,
            8,
        ])
        .unwrap()
    } else {
        0
    };
    sum(&[existing, removal]).unwrap()
}

// Observe through a clone of the retained child's original pidfd. NOWAIT keeps
// consuming-wait custody with that child and its original finite cleanup pool.
fn signal_and_observe(issuer: &Issuer<'_>, signal: Signal, b: &mut Budget<'_>) {
    let floor = b.storage();
    let used = b.work();
    let (pidfd, charge) = issuer.child.try_clone_pidfd(b).unwrap();
    assert!(b.work() - used <= PlainChild::OPERATION_WORK);
    assert_eq!(charge.additional_storage(), FILE_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    process::pidfd_send_signal(&pidfd, signal).unwrap();
    let options = match signal {
        Signal::KILL => WaitIdOptions::EXITED,
        Signal::STOP => WaitIdOptions::STOPPED | WaitIdOptions::EXITED,
        _ => panic!("unsupported issuer lifetime signal"),
    };
    let mut observed = false;
    for _ in 0..TURNS {
        if let Some(status) = process::waitid(
            WaitId::PidFd(pidfd.as_fd()),
            options | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .unwrap()
        {
            match signal {
                Signal::KILL => {
                    assert!(status.killed());
                    assert_eq!(status.terminating_signal(), Some(libc::SIGKILL));
                }
                Signal::STOP => {
                    assert!(
                        status.stopped(),
                        "issuer exited before compiler cancellation"
                    );
                    assert_eq!(status.stopping_signal(), Some(libc::SIGSTOP));
                }
                _ => unreachable!(),
            }
            observed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observed, "original issuer pidfd did not report {signal:?}");
    drop(pidfd);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), floor);
}

pub(super) fn issuer_exit(attempt: &mut Attempt<'_>, b: &mut Budget<'_>) {
    let issuer = attempt.issuer.as_ref().unwrap();
    let live = b.storage();
    signal_and_observe(issuer, Signal::KILL, b);
    assert!(!issuer.child.is_live(b).unwrap());

    let used = b.work();
    let error = attempt.validate_ready(b).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Profile(ProfileError::Observation(ObservationError::Io {
                operation: "open proc namespace",
                source: io::Errno::NOENT,
            }))
        ),
        "exited issuer must refuse actual process validation: {error:?}"
    );
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= attempt.continuity_quota().work());

    // Exercise the retained connection itself as well as validate_ready's earlier
    // process check, using the same original live compiler observation and child.
    let used = b.work();
    let error = issuer
        .child
        .with_resources(b, |p, b| -> Result<()> {
            attempt
                .trace
                .with_observation(b, |original, b| -> Result<()> {
                    Ok(issuer.connection.validate(
                        &attempt.root,
                        original,
                        &issuer.child,
                        p.prepared.trust.policy().policy(),
                        p.manifest.manifest(),
                        b,
                    )?)
                })
        })
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::RootConnection(ConnectionError::Refused(
                "root issuer custody changed or exited"
            ))
        ),
        "retained root connection accepted an exited issuer: {error:?}"
    );
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= attempt.continuity_quota().work());
    assert!(attempt.poll_compiler(b).unwrap().is_exec());
    assert_eq!(b.storage(), live);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}

pub(super) fn stop_issuer(attempt: &Attempt<'_>, b: &mut Budget<'_>) {
    let issuer = attempt.issuer.as_ref().unwrap();
    signal_and_observe(issuer, Signal::STOP, b);
    let live = b.storage();
    let used = b.work();
    attempt.validate_ready(b).unwrap();
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= attempt.continuity_quota().work());
}

pub(super) fn compiler_cancelled(attempt: &Attempt<'_>, b: &mut Budget<'_>) {
    let issuer = attempt.issuer.as_ref().unwrap();
    let live = b.storage();
    // A real stop keeps issuer exit from masking the original compiler refusal.
    assert!(issuer.child.is_live(b).unwrap());
    let used = b.work();
    let error = attempt.validate_ready(b).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Invalid("root observation requires completed issuer input transfer")
        ),
        "cancelled original compiler must refuse continuity: {error:?}"
    );
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= attempt.continuity_quota().work());
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}

pub(super) fn refuse_outer_scope(attempt: &mut Attempt<'_>, b: &mut Budget<'_>) {
    let live = b.storage();
    let used = b.work();
    let guard = CompilerCancellation {
        trace: &mut attempt.trace,
        committed: false,
    };
    let mut joined = false;
    let result = b.with_prepaid_scope(attempt.retained, 8, LOCAL_WORK, FRAME, |b| -> Result<()> {
        guard
            .trace
            .with_observation(b, |original, b| -> Result<()> {
                Ok(attempt.root.validate_original(original, b)?)
            })?;
        joined = true;
        b.release_storage(FRAME)?;
        Ok(())
    });
    assert!(
        joined,
        "actual transferred compiler/session join must succeed"
    );
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    drop(guard);
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= Attempt::original_validation_quota().work());
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}

// Run inside existing native cases, with the real constructor's session/trace.
// The long shared borrows prevent either original owner being replaced while
// the production slot-only cancellation helper removes the issuer.
pub(super) fn remove_issuer<'work>(
    attempt: &mut Attempt<'work>,
    unwind: bool,
    cleanup: &mut Cleanup,
    b: &mut Budget<'work>,
    foreign: &mut Budget<'_>,
) {
    let live = b.storage();
    let retained = attempt.retained_storage();
    let (exit, charge) = attempt
        .issuer
        .as_ref()
        .unwrap()
        .child
        .try_clone_pidfd(b)
        .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let root = &attempt.root;
    let trace = &attempt.trace;
    trace
        .with_observation(b, |original, b| -> Result<()> {
            Ok(root.validate_original(original, b)?)
        })
        .unwrap();
    if unwind {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _issuer = attempt.issuer.take().unwrap();
            panic!("intentional issuer-only unwind with original attempt retained");
        }));
        assert!(result.is_err());
    } else {
        assert!(matches!(
            cancel_issuer_slot(&mut attempt.issuer),
            Some(CleanupPoll::Reaped | CleanupPoll::Pending)
        ));
    }
    cleanup::wait_exit(cleanup, exit.as_fd());
    trace
        .with_observation(b, |original, b| -> Result<()> {
            Ok(root.validate_original(original, b)?)
        })
        .unwrap();
    drop(exit);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), live);
    assert_eq!(attempt.retained_storage(), retained);
    assert_eq!(attempt.cancel_issuer(), None);
    assert!(attempt.issuer_pid().is_none());
    assert!(attempt.readiness().is_none());
    assert!(matches!(
        attempt.validate_ready(b),
        Err(Error::Invalid("root attempt has no issuer"))
    ));

    let mut called = false;
    let error = attempt
        .trace
        .with_issuer_inputs(b, |_, _, _, _, _| {
            called = true;
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        Error::Invalid("issuer inputs require confirmed held compiler exec")
    ));
    assert!(
        !called,
        "consumed transfer must not run a replacement callback"
    );
    assert!(attempt.poll_compiler(b).unwrap().is_exec());

    // Removing the issuer must not silently lower the full attempt input floor.
    let release = live.checked_sub(retained).unwrap() + 1;
    b.release_storage(release).unwrap();
    assert!(matches!(
        attempt.validate_original(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), retained - 1);
    b.reserve_storage(release).unwrap();
    assert!(matches!(
        attempt.validate_original(foreign),
        Err(Error::Resource(Resource::Accounting))
    ));

    let quota = Attempt::original_validation_quota();
    let used = b.work();
    let peak = b.peak_storage();
    attempt.validate_original(b).unwrap();
    assert!(b.work() - used <= quota.work());
    assert!(b.peak_storage() <= peak.max(live + quota.scratch()));
    assert_eq!(attempt.retained_storage(), retained);
    assert_eq!(b.storage(), live);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}
