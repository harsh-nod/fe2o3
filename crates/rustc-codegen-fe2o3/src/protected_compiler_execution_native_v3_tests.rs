//! Real fixed-slot refusals are isolated from the parent test process's FDs.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn native_session_refusal_closes_both_inputs_without_resetting_the_account() {
    const CHILD: &str = "FE2O3_NATIVE_SESSION_REFUSAL_CHILD";
    if std::env::var_os(CHILD).is_some() {
        for (prepaid, limit) in [
            (Admitted::INPUT_STORAGE - 1, usize::MAX),
            (Admitted::INPUT_STORAGE, 0),
            (Admitted::INPUT_STORAGE, usize::MAX),
        ] {
            let file = File::open("/dev/null").unwrap();
            assert!(![POLICY_FD, SERVICE_FD].contains(&file.as_raw_fd()));
            for fd in [POLICY_FD, SERVICE_FD] {
                // SAFETY: the isolated child owns both protocol slots exclusively.
                assert_eq!(unsafe { libc::dup2(file.as_raw_fd(), fd) }, fd);
            }
            let mut work = Work::new(limit);
            let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
            b.reserve_storage(prepaid).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = Admitted::admit(&mut b);
            if prepaid < Admitted::INPUT_STORAGE {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            } else if limit == 0 {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            } else {
                assert!(matches!(result, Err(Error::Policy(_))));
            }
            drop(result);
            assert_eq!(b.storage(), prepaid);
            assert!(b.work_ledger_identity_v1() == ledger);
            if limit == 0 {
                assert!(b.failed_work().is_some());
            }
            for fd in [POLICY_FD, SERVICE_FD] {
                // SAFETY: fcntl only inspects this process's descriptor table.
                assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
                assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
            }
        }
        missing_service_slot_cannot_become_a_private_policy_file();
        return;
    }
    let name = concat!(
        module_path!(),
        "::native_session_refusal_closes_both_inputs_without_resetting_the_account"
    );
    // libtest names exclude the crate root.
    let name = name.split_once("::").unwrap().1;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--test-threads=1", "--nocapture"])
        .env(CHILD, "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated native admission: {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("isolated native admission timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn missing_service_slot_cannot_become_a_private_policy_file() {
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3,
    };
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
    // Two fixed valid public points for an inert sealed-policy fixture only.
    let mut key = [0x66; 32];
    key[0] = 0x58;
    let mut anchor = key;
    anchor[31] ^= 0x80;
    let (policy, charge) = CompilerExecutionIssuerPolicyV3::new(
        1,
        Measurement::new([1; 32], 1).unwrap(),
        Measurement::new([2; 32], 1).unwrap(),
        key,
        anchor,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (policy, charge) = Policy::create(policy, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (file, charge) = policy.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    // SAFETY: only this isolated child owns the protocol's inherited slots.
    assert_eq!(
        unsafe { libc::dup2(file.as_raw_fd(), POLICY_FD) },
        POLICY_FD
    );
    let mut occupied = Vec::new();
    loop {
        // Fill free slots without overwriting any live Rust owner. The first
        // duplicate at 195 proves that policy admission would choose that hole.
        let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        assert!(raw >= 3 && raw <= SERVICE_FD);
        // SAFETY: fcntl returned a fresh uniquely owned descriptor.
        let duplicate = unsafe { File::from_raw_fd(raw) };
        if raw == SERVICE_FD {
            drop(duplicate);
            break;
        }
        occupied.push(duplicate);
    }
    b.reserve_storage(Admitted::INPUT_STORAGE).unwrap();
    let before = b.work();
    let floor = b.storage();
    let result = Admitted::admit(&mut b);
    assert!(matches!(&result, Err(Error::Descriptor(e)) if e.raw_os_error() == Some(libc::EBADF)));
    drop(result);
    assert_eq!(b.work(), before + 2);
    assert_eq!(b.storage(), floor);
    for fd in [POLICY_FD, SERVICE_FD] {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
    policy.revalidate(&mut b).unwrap();
    drop(occupied);
}
