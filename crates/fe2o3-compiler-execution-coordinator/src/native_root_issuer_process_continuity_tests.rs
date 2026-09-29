use super::*;
use fe2o3_broker_authority_service::RootConnectionErrorV3 as ConnectionError;
use fe2o3_protected_service_profile::{
    ProtectedServiceProfileErrorV2 as ProfileError, observations::Error as ObservationError,
};
use rustix::process::{self, Signal, WaitId, WaitIdOptions};

type Issuer<'work> = ManagedIssuer<'work, compiler::Backing>;
type Trace<'work> = CompilerTrace<'work, compiler::Backing>;

pub(super) fn additional_work(case: &str, continuity: Quota) -> usize {
    match case {
        // One readiness refusal, one direct connection refusal, one held-trace
        // poll (bounded by OBSERVATION_WORK), a pidfd clone and a liveness check.
        "ready-issuer-exit" => sum(&[
            2 * continuity.work(),
            Trace::OBSERVATION_WORK,
            2 * PlainChild::OPERATION_WORK,
        ])
        .unwrap(),
        // Validate while stopped, then refuse both immediately after compiler
        // cancellation and after reap. Clone once; check issuer liveness twice.
        "ready-compiler-cancel" => {
            sum(&[3 * continuity.work(), 3 * PlainChild::OPERATION_WORK]).unwrap()
        }
        _ => 0,
    }
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

pub(super) fn issuer_exit(issuer: &Issuer<'_>, trace: &mut Trace<'_>, b: &mut Budget<'_>) {
    let live = b.storage();
    signal_and_observe(issuer, Signal::KILL, b);
    assert!(!issuer.child.is_live(b).unwrap());

    let used = b.work();
    let error = issuer.validate_ready(trace, b).unwrap_err();
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
    assert!(b.work() - used <= issuer.continuity_quota().work());

    // Exercise the retained connection itself as well as validate_ready's earlier
    // process check, using the same original live compiler observation and child.
    let used = b.work();
    let error = issuer
        .child
        .with_resources(b, |p, b| -> Result<()> {
            trace.with_observation(b, |original, b| -> Result<()> {
                Ok(issuer.connection.validate(
                    &issuer.root,
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
    assert!(b.work() - used <= issuer.continuity_quota().work());
    assert!(trace.poll(b).unwrap().is_exec());
    assert_eq!(b.storage(), live);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}

pub(super) fn stop_issuer(issuer: &Issuer<'_>, trace: &Trace<'_>, b: &mut Budget<'_>) {
    signal_and_observe(issuer, Signal::STOP, b);
    let live = b.storage();
    let used = b.work();
    issuer.validate_ready(trace, b).unwrap();
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= issuer.continuity_quota().work());
}

pub(super) fn compiler_cancelled(issuer: &Issuer<'_>, trace: &Trace<'_>, b: &mut Budget<'_>) {
    let live = b.storage();
    // A real stop keeps issuer exit from masking the original compiler refusal.
    assert!(issuer.child.is_live(b).unwrap());
    let used = b.work();
    let error = issuer.validate_ready(trace, b).unwrap_err();
    assert!(
        matches!(
            error,
            Error::Invalid("root observation requires completed issuer input transfer")
        ),
        "cancelled original compiler must refuse continuity: {error:?}"
    );
    assert_eq!(b.storage(), live);
    assert!(b.work() - used <= issuer.continuity_quota().work());
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
}
