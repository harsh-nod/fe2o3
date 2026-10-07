use super::*;

// CPU-only lifecycle fixture; this does not admit a protected process.
fn ordinary_child() -> (std::process::Child, RootOwnedProtectedServiceChildV1) {
    let mut process = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(process.id() as i32).unwrap();
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            panic!("test pidfd: {error}");
        }
    };
    (
        process,
        RootOwnedProtectedServiceChildV1 {
            pid,
            pidfd: Some(pidfd),
            reaping_ownership_lost: false,
        },
    )
}

#[test]
fn polling_cancellation_reaps_original_child_and_is_idempotent() {
    let (_process, mut child) = ordinary_child();
    let original = child.pidfd().as_raw_fd();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.poll_cancel_and_reap() {
            Ok(true) => break,
            Ok(false) => assert_eq!(child.pidfd().as_raw_fd(), original),
            Err(_) => panic!("polling cancellation failed"),
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(child.pidfd.is_none());
    assert!(matches!(child.poll_cancel_and_reap(), Ok(true)));
    assert!(matches!(
        rustix::process::waitpid(Some(child.pid), rustix::process::WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn polling_reaping_ownership_loss_is_sticky() {
    let (mut process, mut child) = ordinary_child();
    process.kill().unwrap();
    process.wait().unwrap();
    for _ in 0..2 {
        assert!(matches!(
            child.poll_cancel_and_reap(),
            Err(ReapErrorV1::OwnershipLost)
        ));
        assert!(child.pidfd.is_none());
    }
    assert!(matches!(
        child.cancel_and_reap(),
        Err(ReapErrorV1::OwnershipLost)
    ));
}

#[test]
fn private_role_discriminator_preserves_exact_securebits() {
    let locked = ChildCredentials::Protected(
        ProtectedServiceCredentialProfileV1::new(61000, 61001).unwrap(),
    );
    let proof = ChildCredentials::ProofController(
        ProofControllerCredentialProfileV1::new(61000, 61001).unwrap(),
    );
    assert_eq!((locked.uid(), locked.gid()), (proof.uid(), proof.gid()));
    assert_eq!(
        locked.securebits(),
        fe2o3_protected_service_profile::PROTECTED_SERVICE_SECUREBITS_V1
    );
    assert_eq!(proof.securebits(), 0);
}
