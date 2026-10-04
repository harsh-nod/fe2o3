//! Public supervisor custody, real native executable, original request/cleanup accounts.
use super::*;
use crate::{
    AcceptedCompilerExecutionHandoffV2 as Accepted, ProtectedIssuerBoundaryV2 as Boundary,
    ProtectedIssuerCleanupErrorV2 as CleanupError, ProtectedIssuerCleanupServiceV2 as Cleanup,
    ProtectedIssuerLaunchErrorV2 as LaunchError, ProtectedIssuerTerminationV1 as Termination,
    ProtectedIssuerWaitV2 as Wait,
};
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/tmp/fe2o3-native-inherited-{}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        // Do not traverse or delete unknown objects on failure. The isolated
        // outer container owns emergency cleanup if a process cannot drain.
        let _ = fs::remove_dir(&self.0);
    }
}

pub(super) fn exercise(case: Case, peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    let root = Root::new();
    let state = root.0.join("compiler-execution-issuer-v3.state");
    if case == Case::Corrupt {
        fs::write(&state, b"invalid native journal").unwrap();
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut work = Work::new(1_000_000_000_000);
    let mut b = Budget::new(&mut work, 2 * 1024 * 1024 * 1024);
    b.charge_work(37).unwrap();
    b.reserve_storage(23).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let authority = crate::authority_v2::tests::bound_consuming_fixture(
        MeasuredImage::from_env("FE2O3_STATIC_PREEXEC_LAUNCHER"),
        MeasuredImage::from_env(case.image()),
        &root.0,
        peer,
        pidfd,
        &mut b,
    );
    let floor = b.storage();
    let mut cleanup =
        Cleanup::admit(Account::new(Work::new(10_000_000), Cleanup::STORAGE)).unwrap();
    let (packet, [control]) = receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&packet[..4], b"HOF2");
    let client_pid = u32::from_le_bytes(packet[4..].try_into().unwrap());
    b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let (accepted, charge) = authority
        .accept_handoff(control, IO_TIMEOUT, &mut b)
        .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(accepted.manifest().client().pid(), client_pid);
    let manifest = accepted.manifest().identity();
    let (prepared, charge) = authority.prepare_launch(accepted, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let limits = Wait::new(Wait::MAX_ATTEMPTS, BOUND).unwrap();
    let pid = {
        let launched = match authority.launch(prepared, &mut cleanup, limits, &mut b) {
            Ok(child) => Some(child),
            Err(LaunchError::ChildExited(Boundary::Exec)) if case != Case::Native => None,
            Err(error) => {
                drain(&mut cleanup);
                // fe2o3-hygiene: allow-panic -- missing protected launch prerequisite is a failed test.
                panic!("native protected launch prerequisite refused: {error:?}");
            }
        };
        let pid = launched.as_ref().map(|child| child.pid());
        if case == Case::Native {
            let launched = launched.unwrap();
            let pid = launched.pid();
            let mut ready = launched.await_readiness(limits).unwrap();
            assert_eq!(ready.readiness().issuer_pid(), pid);
            assert_eq!(ready.readiness().launch_manifest_identity(), manifest);
            assert_eq!(
                ready.readiness().policy_identity(),
                authority.policy().identity()
            );
            ready.revalidate().unwrap();
            let bytes = *ready.readiness().canonical_bytes();
            let serving = ready.publish_readiness(limits).unwrap();
            assert!(
                state.is_file(),
                "native readiness requires recovered durable genesis"
            );
            send_packet(submitter, &frame(b"PUB2", pid), &[]).unwrap();
            let (observed, []) =
                receive_sized_packet::<READY_BYTES, 0>(submitter, Instant::now() + IO_TIMEOUT)
                    .unwrap();
            assert_eq!(observed, bytes);
            send_packet(submitter, &frame(b"FIN2", pid), &[]).unwrap();
            let (done, []) = receive_packet::<0>(submitter, Instant::now() + BOUND).unwrap();
            assert_eq!(done, frame(b"FIN2", pid));
            let exited = serving.wait_for_exit(limits).unwrap();
            assert_eq!(exited.termination(), Termination::Exited { status: 0 });
            assert_eq!(exited.readiness().canonical_bytes(), &bytes);
            drop(exited);
        } else if let Some(launched) = launched {
            let error = launched
                .await_readiness(limits)
                .err()
                .expect("unsupported native launch must not be ready");
            assert!(
                matches!(
                    error,
                    LaunchError::ChildExited(Boundary::Readiness)
                        | LaunchError::State("native readiness is truncated")
                ),
                "timeouts or profile failures are not successful negative cases: {error:?}"
            );
        }
        pid
    };
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    drain(&mut cleanup);
    if case != Case::Native {
        send_packet(submitter, &frame(b"REF2", case.id()), &[]).unwrap();
    }
    if case == Case::Corrupt {
        assert_eq!(fs::read(&state).unwrap(), b"invalid native journal");
    }
    if case == Case::Legacy {
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    }
    let retained = authority.retained_storage();
    drop(authority);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 23);
    for entry in fs::read_dir(&root.0).unwrap() {
        let entry = entry.unwrap();
        assert_eq!(entry.file_name(), "compiler-execution-issuer-v3.state");
        assert!(entry.file_type().unwrap().is_file());
        fs::remove_file(entry.path()).unwrap();
    }
    eprintln!(
        "native inherited supervisor case={case:?} pid={pid:?}; original work={}, cleanup drained",
        b.work()
    );
    b.release_storage(23).unwrap();
    assert_eq!(b.storage(), 0);
}

fn drain(cleanup: &mut Cleanup) {
    let deadline = Instant::now() + Duration::from_secs(5);
    for _ in 0..4096 {
        assert!(Instant::now() < deadline, "cleanup deadline");
        cleanup
            .pump(crate::MAX_PROTECTED_ISSUER_PROCESSES_V1)
            .unwrap();
        match cleanup.shutdown() {
            Ok(account) => {
                assert_eq!(account.storage(), 0);
                return;
            }
            Err(CleanupError::Busy) => std::thread::yield_now(),
            // fe2o3-hygiene: allow-panic -- isolated cleanup must complete or fail.
            Err(e) => panic!("native cleanup refused: {e:?}"),
        }
    }
    assert!(false, "cleanup attempt bound");
}
