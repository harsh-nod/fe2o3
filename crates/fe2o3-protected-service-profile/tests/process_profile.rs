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
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    ProtectedServiceProfileErrorV2 as Error, require_owned_sigchld_v2,
    validate_current_protected_service_profile_v1,
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
