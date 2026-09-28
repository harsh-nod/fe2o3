use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MODES: [&str; 10] = [
    "profile",
    "ready",
    "unwind",
    "work",
    "floor",
    "scratch",
    "policy",
    "absent",
    "image",
    "missing-peer",
];

#[derive(Default)]
struct Hooks {
    mode: String,
    ready: bool,
    executable: bool,
}
impl StartupHooks for Hooks {
    fn invocation(&mut self) -> Result<()> {
        Ok(())
    }
    fn profile(&mut self, _: &mut Budget<'_>) -> Result<Profile> {
        if self.mode == "profile" {
            return Err(Error::Credentials);
        }
        Ok(Profile {
            process: None,
            namespaces: None,
        })
    }
    fn executable(
        &mut self,
        m: Measurement,
        o: Owner,
        b: &mut Budget<'_>,
    ) -> Result<Option<Executable>> {
        self.executable = true;
        if self.mode == "image" {
            SystemStartup.executable(m, o, b)
        } else {
            Ok(None)
        }
    }
    fn lifecycle(&mut self, file: File, root: &OwnedFd, b: &mut Budget<'_>) -> Result<Lease> {
        let (l, c) = Lease::admit_non_authoritative_same_owner_test(file, root, b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(l)
    }
    fn ready(&mut self, _: &mut Budget<'_>) -> Result<()> {
        self.ready = true;
        assert!(self.mode != "unwind", "injected startup unwind");
        if self.mode == "ready" {
            return Err(Error::RuntimeConfiguration);
        }
        Ok(())
    }
}

fn spawn(
    files: [File; 7],
    family: &str,
    mode: &str,
    report: &std::path::Path,
) -> std::process::ExitStatus {
    let sources = files.map(|f| rustix::io::fcntl_dupfd_cloexec(&f, 400).unwrap());
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "native_entrypoint::tests::startup_subprocess",
        "--nocapture",
    ])
    .env("FE2O3_NATIVE_STARTUP_FAMILY", family)
    .env("FE2O3_NATIVE_STARTUP_MODE", mode)
    .env("FE2O3_NATIVE_STARTUP_REPORT", report)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    let missing = mode == "missing-peer";
    // SAFETY: owned high sources remain live until exec; only async-signal-safe
    // descriptor operations run in the child, producing exclusively raw slots.
    unsafe {
        cmd.pre_exec(move || {
            for (i, (source, target)) in sources.iter().zip(INPUT_FDS).enumerate() {
                if missing && i == PEER {
                    libc::close(target);
                    continue;
                }
                if libc::dup2(source.as_raw_fd(), target) != target
                    || libc::fcntl(target, libc::F_SETFD, 0) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap_or_else(|error| {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native startup subprocess polling failed: {error}");
        }) {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native startup subprocess exceeded deadline: {family} {mode}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn source_take_failures_preserve_descriptor_custody() {
    let dir = tempfile::tempdir().unwrap();
    for mode in [
        "source-duplicate-error",
        "source-close-error",
        "source-success",
    ] {
        let files = std::array::from_fn(|_| File::open("/dev/null").unwrap());
        let report = dir.path().join(mode);
        assert!(spawn(files, "v2", mode, &report).success(), "{mode}");
        assert_eq!(fs::read_to_string(report).unwrap(), mode);
    }
}

fn source_take_case(mode: &str) {
    use std::os::fd::{FromRawFd, IntoRawFd};

    let mut sources = Sources {
        live: [true; INPUT_FDS.len()],
    };
    let path = |fd| format!("/proc/self/fd/{fd}");
    let assert_closed = |fd| {
        assert_eq!(
            fs::metadata(path(fd)).unwrap_err().raw_os_error(),
            Some(libc::ENOENT)
        );
    };
    if mode == "source-success" {
        let owned = sources.take(ROOT, 256, "source guard regression").unwrap();
        assert_closed(INPUT_FDS[ROOT]);
        drop(sources);
        assert!(rustix::io::fcntl_getfd(&owned).is_ok());
        return;
    }

    let duplicate_error = mode == "source-duplicate-error";
    assert!(duplicate_error || mode == "source-close-error");
    let replacement = tempfile::tempfile().unwrap();
    let expected = replacement.metadata().unwrap();
    let injected = |errno| Error::Io {
        operation: "injected source take failure",
        source: rustix::io::Errno::from_raw_os_error(errno),
    };
    let mut duplicate_fd = None;
    let mut replacement_fd = None;
    let mut closes = 0;
    let result = sources.take_with(
        ROOT,
        |fd| {
            if duplicate_error {
                return Err(injected(libc::EMFILE));
            }
            let owned =
                crate::entrypoint::duplicate_inherited_at(fd, 256, "source guard regression")?;
            duplicate_fd = Some(owned.as_raw_fd());
            Ok(owned)
        },
        |fd| {
            closes += 1;
            crate::entrypoint::close_inherited(fd)?;
            let reused = rustix::io::fcntl_dupfd_cloexec(&replacement, fd).unwrap();
            assert_eq!(reused.as_raw_fd(), fd);
            // Keep raw custody until after the guard check, so a regression never
            // creates a BorrowedFd from a descriptor the old guard closed.
            replacement_fd = Some(reused.into_raw_fd());
            Err(injected(libc::EIO))
        },
    );
    let Error::Io { source, .. } = result.unwrap_err() else {
        panic!("unexpected source take error");
    };
    assert_eq!(
        source.raw_os_error(),
        if duplicate_error {
            libc::EMFILE
        } else {
            libc::EIO
        }
    );
    assert_eq!(sources.live[ROOT], duplicate_error);
    assert_eq!(closes, usize::from(!duplicate_error));
    if let Some(fd) = duplicate_fd {
        assert_closed(fd);
    }
    assert!(fs::metadata(path(INPUT_FDS[ROOT])).is_ok());
    drop(sources);
    if duplicate_error {
        assert!(replacement_fd.is_none());
        assert_closed(INPUT_FDS[ROOT]);
    } else {
        let fd = replacement_fd.unwrap();
        let actual = fs::metadata(path(fd)).unwrap();
        assert_eq!(
            (actual.dev(), actual.ino()),
            (expected.dev(), expected.ino())
        );
        // SAFETY: the reused descriptor stayed live through guard Drop and its
        // sole raw ownership is transferred back into File exactly once.
        let owned = unsafe { File::from_raw_fd(fd) };
        assert!(rustix::io::fcntl_getfd(&owned).is_ok());
    }
}

#[test]
fn startup_subprocess() {
    let Ok(family) = std::env::var("FE2O3_NATIVE_STARTUP_FAMILY") else {
        return;
    };
    let mode = std::env::var("FE2O3_NATIVE_STARTUP_MODE").unwrap();
    let report = std::env::var_os("FE2O3_NATIVE_STARTUP_REPORT").unwrap();
    if mode.starts_with("source-") {
        source_take_case(&mode);
        fs::write(report, &mode).unwrap();
        return;
    }
    // Oracles for every context/key/root/lease object, including low-numbered
    // private duplicates. Exclude the /dev/null peer, also used by test stdio.
    let objects: Vec<_> = INPUT_FDS[ROOT..]
        .iter()
        .map(|fd| {
            let m = fs::metadata(format!("/proc/self/fd/{fd}")).unwrap();
            (m.dev(), m.ino())
        })
        .collect();
    let (floor, run): (
        usize,
        fn(&mut Budget<'_>, &mut Hooks) -> Result<(Report, Storage)>,
    ) = match family.as_str() {
        "v2" => (v2::INPUT_STORAGE, v2::run::<Hooks>),
        "v3" => (v3::INPUT_STORAGE, v3::run::<Hooks>),
        _ => panic!("unknown family"),
    };
    let floor = floor - usize::from(mode == "floor");
    let mut w = Work::new(if mode == "work" {
        NATIVE_EXTERNAL_ANCHOR_STARTUP_WORK_V2 - 1
    } else {
        NATIVE_EXTERNAL_ANCHOR_PROCESS_WORK_LIMIT_V2
    });
    let mut b = Budget::new(
        &mut w,
        if mode == "scratch" {
            floor + NATIVE_EXTERNAL_ANCHOR_STARTUP_FRAME_STORAGE_V2 - 1
        } else {
            NATIVE_EXTERNAL_ANCHOR_PROCESS_STORAGE_LIMIT_V2
        },
    );
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let mut hooks = Hooks {
        mode: mode.clone(),
        ..Hooks::default()
    };
    // The private test hook admits rootless fixtures, not protected authority.
    // All descriptor cleanup and native configuration/key/state operations are real.
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut b, &mut hooks)));
    let result = result.map(|r| {
        r.map(|(report, charge)| {
            let bytes = charge.additional_storage();
            b.reserve_storage(bytes).unwrap();
            let exchanges = report.exchanges();
            drop((report, charge));
            b.release_storage(bytes).unwrap();
            exchanges
        })
    });
    let cleanup = INPUT_FDS.into_iter().chain([256, 257, 258]).all(|fd| {
        // SAFETY: F_GETFD only observes whether a consumed slot is closed.
        unsafe { libc::fcntl(fd, libc::F_GETFD) == -1 && *libc::__errno_location() == libc::EBADF }
    });
    let duplicates_closed = fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .all(|m| !objects.contains(&(m.dev(), m.ino())));
    let accounting = b.storage() == floor
        && ledger == b.work_ledger_identity_v1()
        && history == (b.failed_work(), b.failed_storage());
    let accepted = match (mode.as_str(), &result) {
        ("profile", Ok(Err(Error::Credentials))) => !hooks.executable && !hooks.ready,
        ("ready", Ok(Err(Error::RuntimeConfiguration))) => hooks.ready,
        ("unwind", Err(_)) => hooks.ready,
        ("work", Ok(Err(Error::Resource(Resource::Work(_))))) => b.work() == 8 && !hooks.executable,
        ("floor", Ok(Err(Error::Resource(Resource::Accounting)))) => {
            b.work() == 8 && !hooks.executable
        }
        ("scratch", Ok(Err(Error::Resource(Resource::Storage(_))))) => {
            b.work() == NATIVE_EXTERNAL_ANCHOR_STARTUP_WORK_V2 && !hooks.executable
        }
        ("policy", Ok(Err(Error::Capability(_)))) => !hooks.executable && !hooks.ready,
        ("absent", Ok(Err(Error::Anchor(NativeExternalAnchorErrorV2::State(_))))) => {
            hooks.executable && !hooks.ready
        }
        ("image", Ok(Err(Error::Executable(_)))) => hooks.executable && !hooks.ready,
        ("missing-peer", Ok(Err(Error::Descriptor(_)))) => !hooks.executable && !hooks.ready,
        ("socket", Ok(Ok(count))) => hooks.ready && *count == 0,
        _ => false,
    };
    let okay = cleanup && duplicates_closed && accounting && accepted;
    if !okay {
        let detail = match &result {
            Ok(r) => format!("{r:?}"),
            Err(_) => "unwind".into(),
        };
        let _ = fs::write(
            report,
            format!(
                "{mode}: cleanup={cleanup} duplicates_closed={duplicates_closed} accounting={accounting} accepted={accepted} result={detail} work={} floor={floor} storage={}",
                b.work(),
                b.storage()
            ),
        );
    }
    // Do not re-enter the Rust test harness after closing its standard descriptors.
    // SAFETY: this is the terminal isolated test process, with no pending work.
    unsafe {
        libc::_exit(if okay { 0 } else { 1 });
    }
}

#[test]
fn invocation_inspection_rejects_the_configured_test_process() {
    assert!(matches!(
        require_invocation(),
        Err(Error::RuntimeConfiguration)
    ));
}

mod family_v2 {
    use super::*;
    use crate::DurableExternalAnchorV2 as Anchor;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV2 as Deployment,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key,
        CompilerExecutionPolicyCapabilityV2 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV2 as Supervisor,
    };
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV2 as DeploymentRecord,
        CompilerExecutionIssuerPolicyV2 as PolicyRecord,
        CompilerExecutionSupervisorDeploymentV2 as SupervisorRecord,
    };
    const FAMILY: &str = "v2";
    include!("cases.rs");
}
mod family_v3 {
    use super::*;
    use crate::DurableExternalAnchorV3 as Anchor;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV3 as Deployment,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key,
        CompilerExecutionPolicyCapabilityV3 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV3 as Supervisor,
    };
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV3 as DeploymentRecord,
        CompilerExecutionIssuerPolicyV3 as PolicyRecord,
        CompilerExecutionSupervisorDeploymentV3 as SupervisorRecord,
    };
    const FAMILY: &str = "v3";
    include!("cases.rs");
}
