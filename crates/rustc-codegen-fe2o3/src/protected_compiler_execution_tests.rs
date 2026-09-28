//! Fixed-slot ownership tests run only in isolated copies of the test process.
use super::*;
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
    os::unix::process::CommandExt,
    process::Command,
    time::Instant,
};

const POLICY: RawFd = COMPILER_EXECUTION_POLICY_CHILD_FD_V1;
const SERVICE: RawFd = COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;

pub(super) fn isolated(full_name: &str) -> bool {
    const SENTINEL: &str = "FE2O3_PROTECTED_EXECUTION_SLOT_TEST";
    let name = full_name.split_once("::").unwrap().1;
    if let Some(value) = std::env::var_os(SENTINEL) {
        assert_eq!(value, name);
        return false;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--test-threads=1", "--nocapture"])
        .env(SENTINEL, name);
    // SAFETY: only scalar close calls run after fork; they discard the child's
    // inherited copies, not the parent test process's protocol descriptors.
    unsafe {
        command.pre_exec(|| {
            for fd in [POLICY, SERVICE] {
                if libc::close(fd) != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EBADF) {
                        return Err(error);
                    }
                }
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(status.success(), "isolated admission test: {status}");
                return true;
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated admission timed out or failed: {result:?}");
            }
        }
    }
}

pub(super) fn install(source: &File, fd: RawFd, cloexec: bool) {
    let installed = rustix::io::fcntl_dupfd_cloexec(source, fd).unwrap();
    assert_eq!(installed.as_raw_fd(), fd);
    if !cloexec {
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
    }
    assert_eq!(installed.into_raw_fd(), fd);
}

pub(super) fn assert_closed() {
    for fd in [POLICY, SERVICE] {
        // SAFETY: F_GETFD only observes this isolated process's descriptor table.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
}

pub(super) fn leave_service_hole(source: &File) -> Vec<OwnedFd> {
    let mut occupied = Vec::new();
    loop {
        let duplicate = rustix::io::fcntl_dupfd_cloexec(source, 3).unwrap();
        let fd = duplicate.as_raw_fd();
        assert!((3..=SERVICE).contains(&fd));
        if fd == SERVICE {
            drop(duplicate);
            return occupied;
        }
        occupied.push(duplicate);
    }
}

#[test]
fn selected_admission_consumes_missing_cloexec_and_invalid_policy_inputs() {
    if isolated(concat!(
        module_path!(),
        "::selected_admission_consumes_missing_cloexec_and_invalid_policy_inputs"
    )) {
        return;
    }
    for (policy, service, cloexec, expected_errno) in [
        (false, false, None, Some(libc::EBADF)),
        (true, false, None, Some(libc::EBADF)),
        (false, true, None, Some(libc::EBADF)),
        (true, true, Some(POLICY), Some(libc::EINVAL)),
        (true, true, Some(SERVICE), Some(libc::EINVAL)),
        (true, true, None, None),
    ] {
        assert_closed();
        let source = File::open("/dev/null").unwrap();
        for (present, fd) in [(policy, POLICY), (service, SERVICE)] {
            if present {
                install(&source, fd, cloexec == Some(fd));
            }
        }
        // SAFETY: install relinquishes fresh descriptor ownership in this isolated
        // child; deliberately absent slots remain vacant through this one attempt.
        let error = unsafe { admit_for_production_codegen() }
            .err()
            .expect("hostile inputs must reject");
        match (error, expected_errno) {
            (ProtectedCompilerExecutionErrorV1::Descriptor(error), Some(errno)) => {
                assert_eq!(error.raw_os_error(), Some(errno))
            }
            (ProtectedCompilerExecutionErrorV1::Policy(_), None) => {}
            (error, errno) => panic!("unexpected refusal {error:?}; expected {errno:?}"),
        }
        assert_closed();
        rustix::fs::fstat(&source).unwrap();
    }
}

fn valid_policy() -> CompilerExecutionPolicyCapabilityV1 {
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV1,
    };
    let mut key = [0x66; 32];
    key[0] = 0x58;
    let mut anchor = key;
    anchor[31] ^= 0x80;
    let record = CompilerExecutionIssuerPolicyV1::new(
        1,
        Measurement::new([1; 32], 1).unwrap(),
        Measurement::new([2; 32], 1).unwrap(),
        key,
        anchor,
    )
    .unwrap();
    CompilerExecutionPolicyCapabilityV1::create(record).unwrap()
}

#[test]
fn selected_admission_cannot_duplicate_policy_into_missing_service_slot() {
    if isolated(concat!(
        module_path!(),
        "::selected_admission_cannot_duplicate_policy_into_missing_service_slot"
    )) {
        return;
    }
    let policy = valid_policy();
    let file = policy.try_clone_for_transfer().unwrap();
    install(&file, POLICY, false);
    let occupied = leave_service_hole(&file);
    // The old admission order would put this private owner in the service slot.
    // Drop it here before exercising the fixed consuming entry point.
    let duplicate = CompilerExecutionPolicyCapabilityV1::from_inherited_child().unwrap();
    assert_eq!(
        std::fs::read(format!("/proc/self/fd/{SERVICE}")).unwrap(),
        policy.policy().canonical_bytes()
    );
    drop(duplicate);
    // SAFETY: install relinquished the policy slot; this isolated child retains
    // every lower descriptor and leaves the service slot vacant through refusal.
    let result = unsafe { admit_for_production_codegen() };
    assert!(matches!(&result,
        Err(ProtectedCompilerExecutionErrorV1::Descriptor(error))
        if error.raw_os_error() == Some(libc::EBADF)));
    drop(result);
    assert_closed();
    policy.revalidate().unwrap();

    // A valid policy reaches client admission, which must consume its slot even
    // when the supplied service input is not a socket.
    install(&file, POLICY, false);
    install(&file, SERVICE, false);
    // SAFETY: both fresh installs relinquished their sole ownership; the previous
    // transfer was consumed, and no other fixture thread can touch these slots.
    assert!(matches!(
        unsafe { admit_for_production_codegen() },
        Err(ProtectedCompilerExecutionErrorV1::Client(_))
    ));
    assert_closed();
    policy.revalidate().unwrap();
    for descriptor in &occupied {
        rustix::fs::fstat(descriptor).unwrap();
    }
    drop(occupied);
}

#[test]
fn shared_slot_guard_closes_on_unwind_and_preserves_transferred_ownership() {
    if isolated(concat!(
        module_path!(),
        "::shared_slot_guard_closes_on_unwind_and_preserves_transferred_ownership"
    )) {
        return;
    }
    let source = File::open("/dev/null").unwrap();
    for fd in [POLICY, SERVICE] {
        install(&source, fd, false);
    }
    assert!(
        std::panic::catch_unwind(|| {
            // SAFETY: both installs relinquished their sole descriptor ownership
            // in this isolated child. This guard alone consumes them on unwind.
            let slots = unsafe { InheritedExecutionSlots::new() };
            slots.validate().unwrap();
            panic!("injected admission unwind");
        })
        .is_err()
    );
    assert_closed();

    for fd in [POLICY, SERVICE] {
        install(&source, fd, false);
    }
    // SAFETY: the new installs relinquished both slots after prior cleanup; this
    // isolated child's guard owns them until the explicit transfers below.
    let mut slots = unsafe { InheritedExecutionSlots::new() };
    slots.validate().unwrap();
    slots.close_policy().unwrap();
    let replacement = rustix::io::fcntl_dupfd_cloexec(&source, POLICY).unwrap();
    assert_eq!(replacement.as_raw_fd(), POLICY);
    slots.service = None;
    // SAFETY: install transferred one live descriptor; the guard is now disarmed.
    let service = unsafe { OwnedFd::from_raw_fd(SERVICE) };
    drop(slots);
    rustix::fs::fstat(&replacement).unwrap();
    rustix::fs::fstat(&service).unwrap();
    drop((replacement, service));
    assert_closed();
}

#[test]
fn startup_refusal_never_reconsumes_reused_descriptor_numbers() {
    if isolated(concat!(
        module_path!(),
        "::startup_refusal_never_reconsumes_reused_descriptor_numbers"
    )) {
        return;
    }
    assert_closed();
    let input = CompilerExecutionStartupInputV1::capture();
    let source = File::open("/dev/null").unwrap();
    let replacements: Vec<_> = [POLICY, SERVICE]
        .map(|fd| {
            let replacement = rustix::io::fcntl_dupfd_cloexec(&source, fd).unwrap();
            assert_eq!(replacement.as_raw_fd(), fd);
            replacement
        })
        .into();
    assert!(matches!(
        input.admit(),
        Err(ProtectedCompilerExecutionErrorV1::Descriptor(e))
            if e.raw_os_error() == Some(libc::EBADF)
    ));
    assert!(matches!(
        input.admit(),
        Err(ProtectedCompilerExecutionErrorV1::InputAlreadyConsumed)
    ));
    drop(input);
    for replacement in &replacements {
        rustix::fs::fstat(replacement).unwrap();
    }
    drop(replacements);
    assert_closed();
}

#[test]
fn startup_capture_retains_cloexec_owners_until_one_safe_admission_or_drop() {
    if isolated(concat!(
        module_path!(),
        "::startup_capture_retains_cloexec_owners_until_one_safe_admission_or_drop"
    )) {
        return;
    }
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
    let policy = valid_policy();
    let file = policy.try_clone_for_transfer().unwrap();
    let (client, _server) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let client = File::from(client);
    for admit in [false, true] {
        install(&file, POLICY, false);
        install(&client, SERVICE, false);
        // SAFETY: install relinquished each newly created descriptor once. These
        // owners deliberately remain live while the safe loader takes duplicates.
        let originals = unsafe { [OwnedFd::from_raw_fd(POLICY), OwnedFd::from_raw_fd(SERVICE)] };
        let input = CompilerExecutionStartupInputV1::capture();
        {
            let retained = input.0.lock().unwrap();
            let owned = retained.as_ref().unwrap().as_ref().unwrap();
            for fd in [&owned.policy, &owned.service] {
                assert!(
                    rustix::io::fcntl_getfd(fd)
                        .unwrap()
                        .contains(rustix::io::FdFlags::CLOEXEC)
                );
            }
        }
        if admit {
            let admitted = input.admit().unwrap();
            assert_eq!(admitted.policy.policy(), policy.policy());
            admitted.policy.revalidate().unwrap();
            assert!(matches!(
                input.admit(),
                Err(ProtectedCompilerExecutionErrorV1::InputAlreadyConsumed)
            ));
            drop(admitted);
        }
        drop(input);
        for fd in &originals {
            rustix::fs::fstat(fd).unwrap();
            assert!(
                rustix::io::fcntl_getfd(fd)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
        }
        let repeated_factory = CompilerExecutionStartupInputV1::capture();
        assert!(matches!(
            repeated_factory.admit(),
            Err(ProtectedCompilerExecutionErrorV1::Descriptor(e))
                if e.raw_os_error() == Some(libc::EINVAL)
        ));
        drop(repeated_factory);
        for fd in &originals {
            rustix::fs::fstat(fd).unwrap();
        }
        drop(originals);
        assert_closed();
        policy.revalidate().unwrap();
        rustix::fs::fstat(&client).unwrap();
    }
}
