// Included with genuine V2/V3 owners; no explicit-stage lifecycle substitutes.
use super::*;

#[test]
#[ignore = "opt-in isolated root container; public run_session readiness, publication and exit"]
fn native_session_ready() {
    super::super::coordinate_mode(Case::Ready, FAMILY, Mode::Session);
}

#[test]
#[ignore = "opt-in isolated root container; public run_session missing-EOF refusal"]
fn native_session_missing_eof() {
    super::super::coordinate_mode(Case::MissingEof, FAMILY, Mode::Session);
}

#[test]
#[ignore = "opt-in isolated root container; public run_session trailing-data refusal"]
fn native_session_trailing() {
    super::super::coordinate_mode(Case::Trailing, FAMILY, Mode::Session);
}

#[test]
#[ignore = "private locked supervisor, selected only by the native session coordinator"]
fn locked_supervisor_process_helper() {
    super::super::locked_supervisor_mode(FAMILY, Mode::Session, exercise);
}

fn exercise(case: Case, peer: &OwnedFd, pidfd: &OwnedFd, submitter: &OwnedFd) {
    assert_ne!(case, Case::DropBeforeReady);
    let initial_fds = fd_inventory();
    let mut work = Work::new(REQUEST_WORK);
    let denied_work;
    {
        let mut budget = Budget::new(&mut work, REQUEST_STORAGE);
        budget.charge_work(REQUEST_PREFIX).unwrap();
        budget.reserve_storage(EXTRA).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        require_profile(&mut budget);
        assert_eq!(budget.storage(), EXTRA);
        let root = Root::new();
        let supervisor = bound_consuming_fixture(
            MeasuredImage::from_env("FE2O3_STATIC_PREEXEC_LAUNCHER"),
            MeasuredImage::from_env(FAMILY.image_env(case)),
            &root.0,
            peer,
            pidfd,
            &mut budget,
        );
        let supervisor_storage = supervisor.retained_storage();
        let request_floor = EXTRA + supervisor_storage;
        assert_eq!(budget.storage(), request_floor);
        let request_fds = fd_inventory();
        let anchor_pid = supervisor.external_anchor_process().pid();
        let anchor_pidfds = pidfd_references(anchor_pid);
        let expected_policy = supervisor.policy().identity();
        send_packet(submitter, &frame(b"ANC2", anchor_pid), &[]).unwrap();

        let mut service_work = Work::new(SERVICE_WORK);
        service_work.charge_work(SERVICE_PREFIX).unwrap();
        let mut cleanup = Cleanup::admit(Account::new(service_work, Cleanup::STORAGE)).unwrap();
        let service_before = cleanup.report().unwrap();
        assert_eq!(
            service_before.work,
            SERVICE_PREFIX + Cleanup::ADMISSION_WORK
        );
        assert_eq!(service_before.storage, Cleanup::STORAGE);
        assert_eq!(service_before.failed_work, None);

        send_packet(submitter, &frame(FAMILY.handoff_tag(), 0), &[]).unwrap();
        let (payload, [control]) =
            receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
        assert_eq!(&payload[..4], FAMILY.handoff_tag());
        let client_pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
        assert!(![0, std::process::id(), anchor_pid].contains(&client_pid));
        let client_pidfds = pidfd_references(client_pid);
        let control_witness = Witness::new(&control, 1);
        let expected_manifest = {
            let (manifest, delta) = Manifest::new(
                Client::new(client_pid, 65_532, 65_532).unwrap(),
                Anchor::new(65_534, 65_534).unwrap(),
                supervisor.policy(),
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(delta.additional_storage()).unwrap();
            let identity = manifest.identity();
            drop(manifest);
            budget.release_storage(delta.additional_storage()).unwrap();
            identity
        };
        budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
        // Earlier denials must survive the complete session on this same account.
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
        let before_session = budget.work();
        assert!(budget.charge_work(REQUEST_WORK + 1).is_err());
        assert!(budget.reserve_storage(REQUEST_STORAGE + 1).is_err());
        denied_work = budget.failed_work();
        let denied_storage = budget.failed_storage();
        assert_eq!(denied_work, Some(before_session + REQUEST_WORK + 1));
        assert_eq!(
            denied_storage,
            Some(request_floor + Accepted::CONTROL_STORAGE + REQUEST_STORAGE + 1)
        );
        let wait = Wait::new(Wait::MAX_ATTEMPTS, LIFECYCLE_TIMEOUT).unwrap();
        // Unlike the stage fixture, there is no readiness-pipe observation hook.
        // Allow child startup before bounding missing EOF at the public stage.
        let readiness = if case == Case::MissingEof {
            Wait::new(Wait::MAX_ATTEMPTS, Duration::from_secs(3)).unwrap()
        } else {
            wait
        };
        let limits = SessionLimits::new(IO_TIMEOUT, wait, readiness, wait, wait).unwrap();
        let started = Instant::now();
        let outcome = supervisor
            .run_session(control, &mut cleanup, limits, &mut budget)
            .map(|exited| {
                assert_eq!(exited.readiness().issuer_pid(), exited.pid());
                let observation = (
                    exited.pid(),
                    *exited.readiness().canonical_bytes(),
                    exited.readiness().launch_manifest_identity(),
                    exited.readiness().policy_identity(),
                    exited.termination(),
                    exited.retained_storage(),
                );
                // Only final Drop releases the original request-account borrow.
                drop(exited);
                observation
            });
        // Drain the original controller even for an unexpected stage refusal;
        // never retry with another profile, request account or cleanup account.
        let after_session = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        let service_account = drain_cleanup(&mut cleanup);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage()
            ),
            after_session,
            "cleanup must remain independently funded"
        );
        assert_eq!(budget.storage(), request_floor);
        assert!(budget.work() > before_session);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_work(), denied_work);
        assert_eq!(budget.failed_storage(), denied_storage);
        assert!(service_account.work() > service_before.work);

        // This isolated supervisor spawns only the issuer. Probe only after native
        // custody and the persistent pool have retired, never race their reaper.
        assert!(matches!(
            rustix::process::waitpid(None, rustix::process::WaitOptions::NOHANG),
            Err(rustix::io::Errno::CHILD)
        ));
        control_witness.assert_released();
        drop(control_witness);
        assert_eq!(pidfd_references(client_pid), client_pidfds);
        assert_eq!(pidfd_references(anchor_pid), anchor_pidfds);
        // Includes anonymous pipes, images and stale pidfds whose target is -1;
        // no private prepared-owner access is available inside run_session.
        assert_eq!(fd_inventory(), request_fds);

        match (case, outcome) {
            (Case::Ready, Ok((issuer_pid, bytes, manifest, policy, termination, retained))) => {
                assert!(![0, std::process::id(), anchor_pid, client_pid].contains(&issuer_pid));
                assert_eq!(manifest, expected_manifest);
                assert_eq!(policy, expected_policy);
                assert_eq!(termination, Termination::Exited { status: 0 });
                assert!(retained > Accepted::CONTROL_STORAGE);
                assert!(budget.peak_storage() >= request_floor + retained);
                assert_eq!(pidfd_references(issuer_pid), 0);
                let (published, []) =
                    receive_sized_packet::<READY_BYTES, 0>(submitter, Instant::now() + IO_TIMEOUT)
                        .unwrap();
                assert_eq!(
                    published, bytes,
                    "compare actual public packet with exited owner"
                );
                let (finished, []) =
                    receive_packet::<0>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
                assert_eq!(finished, frame(b"FIN2", issuer_pid));
            }
            (Case::MissingEof, Err(SessionError::Readiness(error))) => {
                assert!(
                    matches!(
                        error,
                        LaunchError::Timeout(Boundary::Readiness)
                            | LaunchError::Attempts(Boundary::Readiness)
                    ),
                    "expected bounded missing-EOF refusal: {error:?}"
                );
            }
            (Case::Trailing, Err(SessionError::Readiness(error))) => {
                assert!(
                    matches!(
                        error,
                        LaunchError::State("native readiness has trailing bytes")
                    ),
                    "expected exact trailing-data refusal: {error:?}"
                );
            }
            (case, Err(error)) => {
                panic!("native session {FAMILY:?} {case:?} refused another stage: {error:?}")
            }
            (case, Ok(_)) => panic!("native session {FAMILY:?} {case:?} unexpectedly succeeded"),
        }

        drop(supervisor);
        budget.release_storage(supervisor_storage).unwrap();
        assert_eq!(budget.storage(), EXTRA);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_work(), denied_work);
        assert_eq!(budget.failed_storage(), denied_storage);
        assert_eq!(fd_inventory(), initial_fds);
        eprintln!(
            "public native run_session {FAMILY:?} {case:?} completed in {:?}: request work={} storage={} cleanup work={} storage={}; fd baseline restored; no unreaped children",
            started.elapsed(),
            budget.work(),
            budget.storage(),
            service_account.work(),
            service_account.storage(),
        );
        drop(root);
        // The locked-role wrapper sends STOP only after this returns. The
        // submitter then requires public EOF, rejecting even one queued byte.
    }
    assert_eq!(work.failed_work(), denied_work);
}
