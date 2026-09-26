use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::CommandExt,
    },
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const MODES: &[&str] = &[
    "profile",
    "floor",
    "work",
    "scratch",
    "missing-template",
    "policy",
    "provisioning",
    "credentials",
    "image",
    "image-intake-short",
    "image-intake-exact",
    "template",
    "before-state",
    "after-state",
    "unwind",
    "ready",
    "post-ready-work",
    "post-ready-storage",
    "pre-exec",
    "corrupt",
];
#[derive(Default)]
struct Hooks {
    mode: String,
    reissue: bool,
    initialized: Option<bool>,
    ready: bool,
    before_exec: bool,
    image_intake: Option<(usize, usize)>,
}
impl StartupHooks for Hooks {
    fn invocation(&mut self) -> Result<()> {
        Ok(())
    }
    fn profile(&mut self, _: &mut Budget<'_>) -> Result<Profile> {
        if self.mode == "profile" {
            return Err(Error::Invalid("test profile refusal"));
        }
        Ok(Profile {
            process: None,
            namespaces: None,
        })
    }
    fn running(
        &mut self,
        m: Measurement,
        o: Owner,
        b: &mut Budget<'_>,
    ) -> Result<Option<Executable>> {
        if self.mode == "image" {
            SystemStartup.running(m, o, b)
        } else {
            Ok(None)
        }
    }
    fn bootstrap(&mut self, f: &OwnedFd) -> Result<()> {
        if self.mode == "socket" {
            helper_io::validate_bootstrap::<false>(f).map_err(Into::into)
        } else {
            Ok(())
        }
    }
    fn lifecycle(&mut self, f: File, root: &File, b: &mut Budget<'_>) -> Result<Lease> {
        let (l, c) = Lease::admit_non_authoritative_same_owner_test(f, root, b)?;
        b.reserve_storage(c.additional_storage())?;
        if self.mode.starts_with("image-intake-") {
            let length = fs::metadata("/proc/self/fd/5").unwrap().len();
            let charge = Executable::file_storage(
                Measurement::new([1; 32], length, IMAGE_MAX as u64).unwrap(),
            )?;
            let remaining = charge - usize::from(self.mode == "image-intake-short");
            b.reserve_storage(
                NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2 - b.storage() - remaining,
            )?;
            self.image_intake = Some((b.work(), charge));
        }
        Ok(l)
    }
    fn before_state(&mut self, _: &mut Budget<'_>) -> Result<()> {
        if self.mode == "before-state" {
            Err(Error::Invalid("before state"))
        } else {
            Ok(())
        }
    }
    fn after_state(&mut self, d: Disposition, _: &mut Budget<'_>) -> Result<()> {
        self.initialized = Some(d == Disposition::Initialized);
        assert!(self.mode != "unwind", "injected post-persistence unwind");
        if self.mode == "after-state" {
            Err(Error::Invalid("after state"))
        } else {
            Ok(())
        }
    }
    fn ready(
        &mut self,
        boot: &OwnedFd,
        peer: &OwnedFd,
        r: &Ready,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.ready = true;
        assert_eq!(
            self.initialized,
            Some(r.disposition() == ReadyDisposition::Initialized)
        );
        if self.mode == "ready" {
            return Err(Error::Invalid("ready send refusal"));
        }
        if self.mode == "post-ready-work" {
            b.charge_work(NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2 - b.work())?;
        }
        if self.mode == "post-ready-storage" {
            b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2 - b.storage())?;
        }
        if self.mode == "socket" {
            SystemStartup.ready(boot, peer, r, b)
        } else {
            Ok(())
        }
    }
    fn before_exec(&mut self, _: &mut Budget<'_>) -> Result<()> {
        self.before_exec = true;
        if self.mode == "exec" {
            Ok(())
        } else {
            Err(Error::Invalid("pre-exec checkpoint"))
        }
    }
}

fn spawn(files: [File; 9], family: &str, mode: &str, report: &std::path::Path) -> Child {
    let sources = files.map(|f| rustix::io::fcntl_dupfd_cloexec(&f, 400).unwrap());
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "native_entrypoint::tests::helper_subprocess",
            "--nocapture",
        ])
        .env("FE2O3_NATIVE_HELPER_FAMILY", family)
        .env("FE2O3_NATIVE_HELPER_MODE", mode)
        .env("FE2O3_NATIVE_HELPER_REPORT", report)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let missing = mode == "missing-template";
    // SAFETY: only descriptor syscalls run after fork; fixed slots are raw and exclusive.
    unsafe {
        command.pre_exec(move || {
            for (i, (f, target)) in sources.iter().zip(INPUT_FDS).enumerate() {
                if missing && i == KEY {
                    libc::close(target);
                    continue;
                }
                if libc::dup2(f.as_raw_fd(), target) != target
                    || libc::fcntl(target, libc::F_SETFD, 0) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = command.spawn().unwrap();
    // Drop the parent's duplicates, particularly all aliases of the lifecycle OFD.
    drop(command);
    child
}
fn wait(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("helper test exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn helper_subprocess() {
    let Ok(family) = std::env::var("FE2O3_NATIVE_HELPER_FAMILY") else {
        return;
    };
    let mode = std::env::var("FE2O3_NATIVE_HELPER_MODE").unwrap();
    let report = std::env::var_os("FE2O3_NATIVE_HELPER_REPORT").unwrap();
    let objects: Vec<_> = INPUT_FDS[ROOT..]
        .iter()
        .filter_map(|fd| fs::metadata(format!("/proc/self/fd/{fd}")).ok())
        .map(|m| (m.dev(), m.ino()))
        .collect();
    let (floor, run): (usize, fn(&mut Budget<'_>, &mut Hooks) -> Result<Infallible>) =
        match family.as_str() {
            "v2" => (v2::INPUT_STORAGE, v2::run::<Hooks>),
            "v3" => (v3::INPUT_STORAGE, v3::run::<Hooks>),
            _ => panic!("family"),
        };
    let floor = floor - usize::from(mode == "floor");
    let mut work = Work::new(if mode == "work" {
        NATIVE_EXTERNAL_ANCHOR_HELPER_WORK_V2 - 1
    } else {
        NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2
    });
    let mut b = Budget::new(
        &mut work,
        if mode == "scratch" {
            floor + NATIVE_EXTERNAL_ANCHOR_HELPER_FRAME_STORAGE_V2 - 1
        } else {
            NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2
        },
    );
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let mut h = Hooks {
        mode: mode.clone(),
        ..Hooks::default()
    };
    let result = catch_unwind(AssertUnwindSafe(|| run(&mut b, &mut h)));
    let closed = INPUT_FDS.into_iter().all(|fd| {
        // SAFETY: inspection only, after every native owner has dropped.
        unsafe { libc::fcntl(fd, libc::F_GETFD) == -1 && *libc::__errno_location() == libc::EBADF }
    });
    let no_alias = fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| fs::metadata(e.ok()?.path()).ok())
        .all(|m| !objects.contains(&(m.dev(), m.ino())));
    let account = b.storage() == floor
        && ledger == b.work_ledger_identity_v1()
        && history == (b.failed_work(), b.failed_storage());
    let accepted = match (mode.as_str(), &result) {
        ("profile" | "missing-template" | "credentials", Ok(Err(Error::Invalid(_)))) => !h.reissue,
        ("floor", Ok(Err(Error::Resource(Resource::Accounting)))) => b.work() == 8 && !h.reissue,
        ("work", Ok(Err(Error::Resource(Resource::Work(_))))) => b.work() == 8 && !h.reissue,
        ("scratch", Ok(Err(Error::Resource(Resource::Storage(_))))) => {
            b.work() == NATIVE_EXTERNAL_ANCHOR_HELPER_WORK_V2 && !h.reissue
        }
        ("policy" | "provisioning", Ok(Err(Error::Capability(_)))) => !h.reissue,
        ("image", Ok(Err(Error::Executable(_)))) => !h.reissue,
        ("image-intake-short", Ok(Err(Error::Resource(Resource::Storage(_))))) => {
            !h.reissue
                && h.image_intake.is_some_and(|(work, charge)| {
                    b.work() == work
                        && b.peak_storage()
                            == NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2 - charge + 1
                })
        }
        (
            "image-intake-exact",
            Ok(Err(Error::Executable(ProtectedStaticExecutableErrorV2::Resource(
                Resource::Storage(_),
            )))),
        ) => !h.reissue && b.peak_storage() == NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2,
        ("template", Ok(Err(Error::Capability(_)))) => h.reissue && h.initialized.is_none(),
        ("before-state", Ok(Err(Error::Invalid(_)))) => h.reissue && h.initialized.is_none(),
        ("after-state", Ok(Err(Error::Invalid(_)))) => h.initialized.is_some() && !h.ready,
        ("unwind", Err(_)) => h.initialized.is_some() && !h.ready,
        ("ready", Ok(Err(Error::Invalid(_)))) => h.ready && !h.before_exec,
        ("pre-exec" | "socket", Ok(Err(Error::Invalid(_)))) => h.before_exec,
        ("reopen", Ok(Err(Error::Invalid(_)))) => h.before_exec && h.initialized == Some(false),
        (
            "post-ready-work",
            Ok(Err(Error::Capability(CompilerExecutionCapabilityErrorV2::Resource(
                Resource::Work(_),
            )))),
        ) => h.ready && !h.before_exec && b.work() == NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2,
        (
            "post-ready-storage",
            Ok(Err(Error::Capability(CompilerExecutionCapabilityErrorV2::Resource(
                Resource::Storage(_),
            )))),
        ) => h.ready && !h.before_exec,
        ("corrupt", Ok(Err(Error::Anchor(_)))) => h.reissue && h.initialized.is_none(),
        _ => false,
    };
    let okay = closed && no_alias && account && accepted;
    if !okay {
        let _ = fs::write(
            report,
            format!(
                "{mode}: closed={closed} no_alias={no_alias} account={account} accepted={accepted} result={result:?} work={} storage={}",
                b.work(),
                b.storage()
            ),
        );
    }
    // SAFETY: terminal isolated test process, never return to the closed-stdio harness.
    unsafe {
        libc::_exit(if okay { 0 } else { 1 });
    }
}

macro_rules! family {
    ($m:ident,$v:ident,$P:ident,$S:ident,$D:ident,$Q:ident,$K:ident,$PR:ident,$SR:ident,$DR:ident,$QR:ident) => {
        mod $m {
            use super::*;
            use fe2o3_compiler_closure_capability::{
                $D as Deployment, $K as Key, $P as Policy, $Q as Provisioning, $S as Supervisor,
            };
            use fe2o3_compiler_execution_protocol::{
                $DR as DeploymentRecord, $PR as PolicyRecord, $QR as ProvisioningRecord,
                $SR as SupervisorRecord,
            };
            const FAMILY: &str = stringify!($v);
            impl $v::Hooks for Hooks {
                fn reissue(
                    &mut self,
                    f: File,
                    d: &DeploymentRecord,
                    b: &mut Budget<'_>,
                ) -> Result<Key> {
                    self.reissue = true;
                    let (k, c) = if self.mode == "template" {
                        Key::reissue_root_template_for_current_service(f, d, b)
                    } else {
                        Key::from_file(f, d, b)
                    }?;
                    b.reserve_storage(c.additional_storage())?;
                    Ok(k)
                }
            }
            include!("cases.rs");
        }
    };
}
family!(
    family_v2,
    v2,
    CompilerExecutionPolicyCapabilityV2,
    CompilerExecutionSupervisorDeploymentCapabilityV2,
    CompilerExecutionExternalAnchorDeploymentCapabilityV2,
    CompilerExecutionExternalAnchorProvisioningCapabilityV2,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV2,
    CompilerExecutionIssuerPolicyV2,
    CompilerExecutionSupervisorDeploymentV2,
    CompilerExecutionExternalAnchorDeploymentV2,
    CompilerExecutionExternalAnchorProvisioningV2
);
family!(
    family_v3,
    v3,
    CompilerExecutionPolicyCapabilityV3,
    CompilerExecutionSupervisorDeploymentCapabilityV3,
    CompilerExecutionExternalAnchorDeploymentCapabilityV3,
    CompilerExecutionExternalAnchorProvisioningCapabilityV3,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3,
    CompilerExecutionIssuerPolicyV3,
    CompilerExecutionSupervisorDeploymentV3,
    CompilerExecutionExternalAnchorDeploymentV3,
    CompilerExecutionExternalAnchorProvisioningV3
);
