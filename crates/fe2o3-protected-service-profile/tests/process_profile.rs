//! Opt-in fixture for real locked process facts, not protected service execution.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![allow(unsafe_code)]
// Explicitly consume move-only observations before retiring their logical charge.
#![allow(clippy::drop_non_drop)]

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_profile::{
    PROTECTED_SERVICE_SECUREBITS_V1, ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV1 as LegacyNamespaces,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    ProtectedServiceProfileErrorV1 as LegacyError, ProtectedServiceProfileErrorV2 as Error,
    observations, require_owned_sigchld_v2, validate_current_protected_service_profile_v1,
};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

const OPT_IN: &str = "FE2O3_RUN_LOCKED_PROFILE_FIXTURE";
const CHILD: &str = "FE2O3_LOCKED_PROFILE_CHILD";
const UID: u32 = 65_534;
const OTHER_FLOOR: usize = 113;

struct ChildGuard(Child);

impl ChildGuard {
    fn wait(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        let kill = self.0.kill();
        let reap = self.wait(Duration::from_secs(5));
        if reap.is_err() {
            eprintln!("locked-profile child cleanup failed: kill={kill:?}, reap={reap:?}");
        }
    }
}

#[test]
#[ignore = "requires an explicitly opted-in isolated root container with SETPCAP/SETUID/SETGID"]
fn locked_profile_fixture() {
    assert_eq!(std::env::var(OPT_IN).as_deref(), Ok("1"));
    assert!(rustix::process::getuid().is_root());
    assert!(rustix::process::geteuid().is_root());
    let cap_last: u32 = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(cap_last <= 63);
    let parent_securebits = rustix::thread::capabilities_secure_bits().unwrap();
    let parent_capabilities = rustix::thread::capabilities(None).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "locked_profile_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env_remove(OPT_IN);
    // SAFETY: this closure runs only in the new single-threaded child before
    // exec. It uses fixed data and syscalls, never locks, allocation or unwinding.
    unsafe {
        command.pre_exec(move || install_profile(cap_last));
    }
    let mut child = ChildGuard(command.spawn().unwrap());
    drop(command);
    assert!(child.wait(Duration::from_secs(30)).unwrap().success());
    assert!(rustix::process::getuid().is_root());
    assert!(rustix::process::geteuid().is_root());
    assert_eq!(
        rustix::thread::capabilities_secure_bits().unwrap(),
        parent_securebits
    );
    assert_eq!(
        rustix::thread::capabilities(None).unwrap(),
        parent_capabilities
    );
}

fn install_profile(cap_last: u32) -> io::Result<()> {
    for capability in 0..=cap_last {
        // SAFETY: PR_CAPBSET_DROP has scalar arguments and removes privilege.
        if unsafe { libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    rustix::thread::set_capabilities_secure_bits(
        rustix::thread::CapabilitiesSecureBits::from_bits_retain(PROTECTED_SERVICE_SECUREBITS_V1),
    )?;
    // SAFETY: called only before exec in the private fixture child. Empty
    // groups and all real/effective/saved IDs establish its dedicated identity.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0
        || unsafe { libc::setresgid(UID, UID, UID) } != 0
        || unsafe { libc::setresuid(UID, UID, UID) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    rustix::thread::set_capabilities(
        None,
        rustix::thread::CapabilitySets {
            effective: rustix::thread::CapabilitySet::empty(),
            permitted: rustix::thread::CapabilitySet::empty(),
            inheritable: rustix::thread::CapabilitySet::empty(),
        },
    )?;
    rustix::thread::set_no_new_privs(true)?;
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )?;
    // SAFETY: umask is process-local and has no pointer arguments.
    unsafe { libc::umask(0o077) };
    Ok(())
}

#[test]
#[ignore = "private exact helper; run through locked_profile_fixture"]
fn locked_profile_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
    assert_eq!(rustix::process::getuid().as_raw(), UID);
    assert_eq!(rustix::process::geteuid().as_raw(), UID);
    // This dynamic test executable has no protected startup entrypoint. Reset
    // exec's dumpability here solely to exercise the observation boundary.
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable).unwrap();
    let credentials = Credentials::new(UID, UID).unwrap();
    validate_current_protected_service_profile_v1(credentials).unwrap();
    for mode in 0..3 {
        let work = Profile::CAPTURE_WORK - usize::from(mode == 1);
        let peak = OTHER_FLOOR + Profile::CAPTURE_SCRATCH - usize::from(mode == 2);
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, peak);
        b.reserve_storage(OTHER_FLOOR).unwrap();
        let result = Profile::capture(credentials, &mut b);
        assert_eq!(b.storage(), OTHER_FLOOR);
        match mode {
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.failed_storage(), Some(peak + 1));
            }
            _ => {
                let (profile, storage) = result.unwrap();
                assert_eq!(storage.additional_storage(), profile.retained_storage());
                assert_eq!(b.work(), Profile::CAPTURE_WORK);
                assert_eq!(b.peak_storage(), peak);
                b.reserve_storage(storage.additional_storage()).unwrap();
                drop(profile);
                b.release_storage(storage.additional_storage()).unwrap();
            }
        }
    }
    exercise_lifetime(credentials);
    exercise_short_revalidation(credentials);
}

fn exercise_lifetime(credentials: Credentials) {
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(OTHER_FLOOR).unwrap();
    let (profile, storage) = Profile::capture(credentials, &mut b).unwrap();
    let retained = storage.additional_storage();
    b.reserve_storage(retained).unwrap();
    assert_eq!(profile.credentials(), credentials);
    assert!(profile.cap_last_cap() <= 63);
    let before = b.work();
    profile.revalidate_current(&mut b).unwrap();
    profile
        .revalidate_process(rustix::process::getpid(), &mut b)
        .unwrap();
    assert_eq!(
        b.work() - before,
        Profile::REVALIDATE_CURRENT_WORK + Profile::REVALIDATE_PROCESS_WORK
    );
    assert_eq!(b.storage(), OTHER_FLOOR + retained);
    let (namespaces, storage) = Namespaces::capture_self(&mut b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    namespaces.revalidate_self(&mut b).unwrap();
    namespaces
        .revalidate_process(rustix::process::getpid(), &mut b)
        .unwrap();
    let parent = rustix::process::getppid().unwrap();
    let mut report = [0; observations::CHILD_NAMESPACE_REPORT_BYTES];
    b.reserve_storage(report.len()).unwrap();
    b.with_prepaid_scope::<(), Error>(
        b.storage(),
        0,
        observations::CHILD_NAMESPACE_REPORT_CAPTURE_WORK,
        observations::CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
        |_| {
            Ok(observations::capture_child_namespace_report_pre_exec(
                parent.as_raw_pid(),
                &mut report,
            )?)
        },
    )
    .unwrap();
    namespaces
        .require_child_report(rustix::process::getpid(), parent, &report, &mut b)
        .unwrap();
    LegacyNamespaces::capture_self()
        .unwrap()
        .require_child_report(rustix::process::getpid(), parent, &report)
        .unwrap();
    b.release_storage(report.len()).unwrap();
    require_owned_sigchld_v2(&mut b).unwrap();
    drop(namespaces);
    b.release_storage(storage.additional_storage()).unwrap();

    // Process-visible IDs still have to match the configured identity.
    for (uid, gid, reason) in [
        (
            UID - 1,
            UID,
            "real, effective, saved, or filesystem UID differs",
        ),
        (
            UID,
            UID - 1,
            "real, effective, saved, or filesystem GID differs",
        ),
    ] {
        let mismatch = Credentials::new(uid, gid).unwrap();
        assert!(matches!(
            Profile::capture(mismatch, &mut b),
            Err(Error::Observation(
                fe2o3_protected_service_profile::observations::Error::ProcessProfile(actual)
            )) if actual == reason
        ));
        assert_eq!(b.storage(), OTHER_FLOOR + retained);
    }
    profile.revalidate_current(&mut b).unwrap();
    drop(profile);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), OTHER_FLOOR);
}

fn exercise_short_revalidation(credentials: Credentials) {
    let mut w = Work::new(Profile::CAPTURE_WORK + Profile::REVALIDATE_CURRENT_WORK - 1);
    let mut b = Budget::new(&mut w, 10_000_000);
    let (profile, storage) = Profile::capture(credentials, &mut b).unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    assert!(matches!(
        profile.revalidate_current(&mut b),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.storage(), profile.retained_storage());
    drop(profile);
    b.release_storage(storage.additional_storage()).unwrap();
    assert_eq!(b.storage(), 0);
}

const THREAD_OPT_IN: &str = "FE2O3_RUN_THREAD_NAMESPACE_FIXTURE";
const THREAD_CHILD: &str = "FE2O3_THREAD_NAMESPACE_CHILD";

#[test]
#[ignore = "isolated Docker only; needs SYS_ADMIN and unshare/clone3 permission"]
fn calling_thread_namespace_fixture() {
    assert_eq!(std::env::var(THREAD_OPT_IN).as_deref(), Ok("1"));
    assert!(std::path::Path::new("/.dockerenv").exists());
    assert!(rustix::process::geteuid().is_root());
    let before = LegacyNamespaces::capture_self().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "calling_thread_namespace_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(THREAD_CHILD, "1")
        .env_remove(THREAD_OPT_IN);
    let mut child = ChildGuard(command.spawn().unwrap());
    assert!(child.wait(Duration::from_secs(30)).unwrap().success());
    before.revalidate_self().unwrap();
}

#[test]
#[ignore = "private helper; run through calling_thread_namespace_fixture"]
fn calling_thread_namespace_child() {
    // This diagnostic observes real caller/leader divergence and child inheritance.
    // Bootstrap capabilities remain here; it is not locked-profile launch evidence.
    assert_eq!(std::env::var(THREAD_CHILD).as_deref(), Ok("1"));
    assert!(std::path::Path::new("/.dockerenv").exists());
    assert!(rustix::process::geteuid().is_root());
    std::thread::spawn(exercise_calling_thread_namespace)
        .join()
        .unwrap();
}

fn exercise_calling_thread_namespace() {
    let parent = rustix::process::getpid();
    assert_ne!(
        rustix::thread::gettid(),
        parent,
        "test must execute on a non-leader thread"
    );
    let leader = LegacyNamespaces::capture_self().unwrap();
    leader.revalidate_process(parent).unwrap();
    // SAFETY: this opt-in subprocess thread gets a NEW UTS namespace; no
    // namespace is entered or changed in the outer runner or host.
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_unshare, libc::CLONE_NEWUTS) },
        0,
        "fixture requires permission to create a UTS namespace"
    );
    let caller = LegacyNamespaces::capture_self().unwrap();
    assert!(matches!(
        caller.revalidate_process(parent),
        Err(LegacyError::Namespace("uts"))
    ));
    assert!(matches!(
        leader.revalidate_self(),
        Err(LegacyError::Namespace("uts"))
    ));
    leader.revalidate_process(parent).unwrap();

    let mut w = Work::new(1_000_000);
    let mut b = Budget::new(&mut w, 1_000_000);
    let (native, receipt) = Namespaces::capture_self(&mut b).unwrap();
    b.reserve_storage(receipt.additional_storage()).unwrap();
    b.reserve_storage(
        observations::CHILD_NAMESPACE_REPORT_BYTES
            + 1
            + observations::CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
    )
    .unwrap();
    b.charge_work(observations::CHILD_NAMESPACE_REPORT_CAPTURE_WORK)
        .unwrap();
    let mut pipe = [-1; 2];
    // SAFETY: pipe is writable two-fd storage, and both fds become locally owned.
    assert_eq!(
        unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) },
        0
    );
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    // SAFETY: successful pipe2 returned two distinct newly owned fds.
    let reader = unsafe { OwnedFd::from_raw_fd(pipe[0]) };
    let writer = unsafe { OwnedFd::from_raw_fd(pipe[1]) };
    let mut pidfd = -1_i32;
    let args = [
        libc::CLONE_PIDFD as u64 | 0x1_0000_0000, // CLONE_CLEAR_SIGHAND
        (&raw mut pidfd) as u64,
        0,
        0,
        libc::SIGCHLD as u64,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    // SAFETY: exact 88-byte clone3 ABI, with no shared VM/files/threads/stack.
    // The child takes only the syscall-only branch and never returns to Rust Drop.
    let pid = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            args.as_ptr(),
            std::mem::size_of_val(&args),
        )
    };
    if pid == 0 {
        // SAFETY: child owns its fd table and fixed stack buffers. No allocation,
        // assertions, Rust cleanup or unwinding occurs in this branch.
        unsafe {
            libc::syscall(libc::SYS_close, reader.as_raw_fd());
            if libc::syscall(
                libc::SYS_prctl,
                libc::PR_SET_PDEATHSIG,
                libc::SIGKILL,
                0,
                0,
                0,
            ) != 0
            {
                libc::_exit(2);
            }
            let mut report = [0; observations::CHILD_NAMESPACE_REPORT_BYTES];
            if observations::capture_child_namespace_report_pre_exec(
                parent.as_raw_pid(),
                &mut report,
            )
            .is_err()
            {
                libc::_exit(3);
            }
            if libc::syscall(
                libc::SYS_write,
                writer.as_raw_fd(),
                report.as_ptr(),
                report.len(),
            ) != report.len() as libc::c_long
            {
                libc::_exit(4);
            }
            if libc::syscall(libc::SYS_close, writer.as_raw_fd()) != 0 {
                libc::_exit(5);
            }
            libc::_exit(0);
        }
    }
    assert!(
        pid >= 0,
        "fixture requires clone3; no fork fallback: {}",
        io::Error::last_os_error()
    );
    let mut child = NamespaceChild {
        pid: pid as i32,
        pidfd,
        reaped: false,
    };
    assert!(pidfd >= 0);
    drop(writer);
    let mut bytes = [0; observations::CHILD_NAMESPACE_REPORT_BYTES + 1];
    let mut used = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "namespace report timed out");
        match rustix::io::read(&reader, &mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => {
                used += count;
                assert!(used <= observations::CHILD_NAMESPACE_REPORT_BYTES);
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("namespace report read failed: {error}"),
        }
    }
    assert_eq!(used, observations::CHILD_NAMESPACE_REPORT_BYTES);
    let pid = rustix::process::Pid::from_raw(child.pid).unwrap();
    caller
        .require_child_report(pid, parent, &bytes[..used])
        .unwrap();
    native
        .require_child_report(pid, parent, &bytes[..used], &mut b)
        .unwrap();
    assert!(matches!(
        leader.require_child_report(pid, parent, &bytes[..used]),
        Err(LegacyError::Namespace("uts"))
    ));
    native.revalidate_self(&mut b).unwrap();
    let status = child.wait(Duration::from_secs(5)).unwrap();
    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 0);
    drop(native);
    b.release_storage(
        receipt.additional_storage()
            + observations::CHILD_NAMESPACE_REPORT_BYTES
            + 1
            + observations::CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
    )
    .unwrap();
    assert_eq!(b.storage(), 0);
}

struct NamespaceChild {
    pid: i32,
    pidfd: i32,
    reaped: bool,
}

impl NamespaceChild {
    fn wait(&mut self, timeout: Duration) -> io::Result<i32> {
        let deadline = Instant::now() + timeout;
        loop {
            let mut status = 0;
            // SAFETY: pid is this fixture's unreaped direct child; status is writable.
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if result == self.pid {
                self.reaped = true;
                return Ok(status);
            }
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for NamespaceChild {
    fn drop(&mut self) {
        if !self.reaped {
            // SAFETY: pidfd identifies our direct child. The fallback is only
            // for a malformed clone result; this unreaped child PID cannot recycle.
            unsafe {
                if self.pidfd >= 0 {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        self.pidfd,
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    );
                } else {
                    libc::kill(self.pid, libc::SIGKILL);
                }
            }
            if let Err(error) = self.wait(Duration::from_secs(5)) {
                eprintln!("namespace fixture exact reap failed: {error}");
            }
        }
        if self.pidfd >= 0 {
            // SAFETY: this fixture uniquely owns the pidfd and closes it once.
            unsafe { libc::close(self.pidfd) };
        }
    }
}
