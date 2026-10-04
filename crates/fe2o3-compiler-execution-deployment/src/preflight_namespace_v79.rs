use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsFd as _, OwnedFd};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{Mode, OFlags, ResolveFlags, fstat, openat2};
use rustix::process::{Signal, WaitId, WaitIdOptions, set_parent_process_death_signal, waitid};

use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, ObjectSnapshotV1, changed,
    invalid, io_error, snapshot, std_io_error,
};

const PROC_RECORD_MAX_BYTES_V79: u64 = 8192;

pub(super) fn supervise_pid1(
    root: File,
    stage: &str,
) -> Result<std::convert::Infallible, DeploymentVerificationErrorV1> {
    let root = PreflightRootReopenV79::capture(root)?;
    let namespace = super::mount::enter_private_qualification_mount_namespace_v1()?;
    let reopened = root.reopen()?;
    let parent = rustix::process::getpid();
    let parent_pidfd = rustix::process::pidfd_open(parent, rustix::process::PidfdFlags::empty())
        .map_err(|source| io_error("open preflight supervisor pidfd", source))?;
    #[allow(deprecated)]
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWPID)
        .map_err(|source| io_error("create isolated preflight child PID namespace", source))?;
    namespace.revalidate()?;
    let child = Command::new("/proc/self/exe")
        .arg(super::COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_PID1_COMMAND_V79)
        .arg(stage)
        .arg(parent.as_raw_pid().to_string())
        .env_clear()
        .stdin(Stdio::from(reopened))
        .stdout(Stdio::inherit())
        .stderr(Stdio::from(parent_pidfd))
        .spawn()
        .map_err(|source| std_io_error("spawn isolated preflight PID1 helper", source))?;
    let mut child = OwnedPreflightPid1V79(Some(child));
    let result = (|| {
        let pid = rustix::process::Pid::from_child(child.0.as_ref().expect("owned PID1"));
        if rustix::process::getpgid(Some(pid))
            .map_err(|source| io_error("inspect preflight PID1 process group", source))?
            != rustix::process::getpgrp()
        {
            return Err(changed(
                "preflight PID1 escaped the qualification worker process group",
            ));
        }
        child.wait()
    })();
    let status = match result {
        Ok(status) => status,
        Err(primary) => {
            return match child.terminate() {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(invalid(
                    DeploymentVerificationErrorKindV1::CleanupFailed,
                    format!(
                        "preflight child failed ({primary}); child cleanup also failed: {cleanup}"
                    ),
                )),
            };
        }
    };
    namespace.revalidate()?;
    if !status.success() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
            format!(
                "isolated preflight tool failed with exit_code={:?} signal={:?}",
                status.code(),
                status.signal()
            ),
        ));
    }
    drop(root);
    drop(namespace);
    std::process::exit(0)
}

struct OwnedPreflightPid1V79(Option<Child>);

impl OwnedPreflightPid1V79 {
    fn wait(&mut self) -> Result<ExitStatus, DeploymentVerificationErrorV1> {
        let status = self
            .0
            .as_mut()
            .expect("owned PID1")
            .wait()
            .map_err(|source| std_io_error("reap isolated preflight PID1", source))?;
        self.0.take();
        Ok(status)
    }

    fn terminate(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        let Some(child) = self.0.as_mut() else {
            return Ok(());
        };
        let signal = child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if child
                .try_wait()
                .map_err(|source| std_io_error("reap terminated preflight PID1", source))?
                .is_some()
            {
                self.0.take();
                // ESRCH is harmless only after the owned child has actually been reaped.
                return match signal {
                    Ok(()) => Ok(()),
                    Err(error)
                        if error.raw_os_error() == Some(rustix::io::Errno::SRCH.raw_os_error()) =>
                    {
                        Ok(())
                    }
                    Err(error) => Err(std_io_error("terminate preflight PID1", error)),
                };
            }
            if Instant::now() >= deadline {
                return Err(invalid(
                    DeploymentVerificationErrorKindV1::CleanupFailed,
                    "preflight PID1 did not reap within the fixed cleanup bound",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for OwnedPreflightPid1V79 {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

pub(super) struct PreflightRootReopenV79 {
    original: File,
    path: PathBuf,
    snapshot: ObjectSnapshotV1,
}

impl PreflightRootReopenV79 {
    pub(super) fn capture(original: File) -> Result<Self, DeploymentVerificationErrorV1> {
        let path = std::fs::canonicalize("/proc/self/fd/0")
            .map_err(|source| std_io_error("resolve retained preflight root", source))?;
        if path.as_os_str().as_encoded_bytes().len() > 4096 {
            return Err(changed(
                "retained preflight root path exceeds its fixed bound",
            ));
        }
        let snapshot = snapshot(
            &fstat(&original)
                .map_err(|source| io_error("snapshot retained preflight root", source))?,
        );
        let state = Self {
            original,
            path,
            snapshot,
        };
        state.reopen()?;
        Ok(state)
    }

    pub(super) fn reopen(&self) -> Result<File, DeploymentVerificationErrorV1> {
        let slash = File::open("/")
            .map_err(|source| std_io_error("open preflight namespace root", source))?;
        let relative = self.path.strip_prefix("/").map_err(|_| {
            invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "retained preflight root path is not absolute",
            )
        })?;
        if relative.as_os_str().is_empty() {
            return Err(changed(
                "preflight root cannot be the outer filesystem root",
            ));
        }
        let reopened = File::from(
            openat2(
                &slash,
                relative,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
            )
            .map_err(|source| io_error("reopen preflight root in private namespace", source))?,
        );
        for descriptor in [&self.original, &reopened, &self.original] {
            if snapshot(
                &fstat(descriptor)
                    .map_err(|source| io_error("revalidate preflight root custody", source))?,
            ) != self.snapshot
            {
                return Err(changed("preflight root changed across namespace entry"));
            }
        }
        Ok(reopened)
    }
}

fn read_proc_record_at(view: &File, path: &str) -> Result<String, DeploymentVerificationErrorV1> {
    if rustix::fs::fstatfs(view)
        .map_err(|source| io_error("inspect preflight parent proc filesystem", source))?
        .f_type
        != 0x9fa0
    {
        return Err(changed("preflight parent record is not procfs"));
    }
    let file = File::from(
        openat2(
            view,
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )
        .map_err(|source| io_error("open preflight parent proc record", source))?,
    );
    let mut text = String::new();
    file.take(PROC_RECORD_MAX_BYTES_V79 + 1)
        .read_to_string(&mut text)
        .map_err(|source| std_io_error("read bounded preflight parent record", source))?;
    if text.len() as u64 > PROC_RECORD_MAX_BYTES_V79 {
        return Err(changed("preflight parent record exceeds its fixed bound"));
    }
    Ok(text)
}

fn unique_pid_field(text: &str, key: &str) -> Result<u32, DeploymentVerificationErrorV1> {
    let mut values = text.lines().filter_map(|line| line.strip_prefix(key));
    let value = values
        .next()
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && !value.starts_with('0')
                && value.bytes().all(|byte| byte.is_ascii_digit())
        })
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value >= 2 && *value <= i32::MAX as u32)
        .ok_or_else(|| changed("preflight parent record has no canonical live PID"))?;
    if values.next().is_some() {
        return Err(changed("preflight parent record repeats its PID field"));
    }
    Ok(value)
}

pub(super) struct PreflightParentBindingV79 {
    descriptor: OwnedFd,
}

impl PreflightParentBindingV79 {
    pub(super) fn bind_stderr(expected: u32) -> Result<Self, DeploymentVerificationErrorV1> {
        if rustix::process::getpid().as_raw_pid() != 1
            || rustix::process::getppid().is_some()
            || super::host::process_thread_count()? != 1
        {
            return Err(changed("preflight tool is not single-task namespace PID 1"));
        }
        let descriptor = rustix::io::dup(std::io::stderr())
            .map_err(|source| io_error("retain preflight parent pidfd", source))?;
        let state = Self { descriptor };
        require_nonchild_pidfd(&state.descriptor)?;
        let view = File::open("/proc")
            .map_err(|source| std_io_error("retain preflight parent proc view", source))?;
        let mut link = [0_u8; 16];
        let length = rustix::fs::readlinkat_raw(&view, "self", &mut link[..])
            .map_err(|source| io_error("read preflight PID in parent proc view", source))?;
        let self_pid = std::str::from_utf8(&link[..length])
            .map_err(|_| changed("preflight proc self PID is not ASCII"))?;
        let self_pid = unique_pid_field(&format!("Pid:{self_pid}"), "Pid:")?;
        for after_binding in [false, true] {
            if after_binding {
                set_parent_process_death_signal(Some(Signal::KILL))
                    .map_err(|source| io_error("bind preflight namespace parent death", source))?;
            }
            state.require_live()?;
            let self_status = read_proc_record_at(&view, &format!("{self_pid}/status"))?;
            let parent_info = read_proc_record_at(&view, &format!("{self_pid}/fdinfo/2"))?;
            if unique_pid_field(&self_status, "PPid:")? != expected
                || unique_pid_field(&parent_info, "Pid:")? != expected
            {
                return Err(changed(
                    "preflight pidfd does not name the exact creating parent",
                ));
            }
        }
        Ok(state)
    }

    pub(super) fn require_live(&self) -> Result<(), DeploymentVerificationErrorV1> {
        let mut descriptors = [PollFd::new(&self.descriptor, PollFlags::IN)];
        if poll(&mut descriptors, Some(&Timespec::default()))
            .map_err(|source| io_error("poll preflight parent pidfd", source))?
            != 0
            || !descriptors[0].revents().is_empty()
        {
            return Err(changed("preflight creating parent is no longer live"));
        }
        Ok(())
    }
}

fn require_nonchild_pidfd(descriptor: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
    // waitid validates the descriptor type without signaling the parent.
    match waitid(
        WaitId::PidFd(descriptor.as_fd()),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    ) {
        Err(rustix::io::Errno::CHILD) => Ok(()),
        _ => Err(changed("preflight parent handle is not a non-child pidfd")),
    }
}

pub(super) fn mount_private_proc_v79() -> Result<(), DeploymentVerificationErrorV1> {
    let root =
        File::open("/").map_err(|source| std_io_error("open entered preflight root", source))?;
    let target = super::open_beneath(&root, "proc", true)?;
    let expected =
        super::validate_directory_mode(&target, Some((0, 0)), 0o755, "preflight proc target")?;
    super::verify_directory_children(&target, &[], "preflight proc target")?;
    let reopened = super::open_beneath(&root, "proc", true)?;
    if snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("revalidate private preflight proc target", source))?,
    ) != expected
    {
        return Err(changed(
            "private preflight proc target changed before mount",
        ));
    }
    rustix::mount::mount(
        "proc",
        "/proc",
        "proc",
        rustix::mount::MountFlags::RDONLY
            | rustix::mount::MountFlags::NOSUID
            | rustix::mount::MountFlags::NODEV
            | rustix::mount::MountFlags::NOEXEC,
        None,
    )
    .map_err(|source| io_error("mount isolated read-only preflight proc", source))?;
    let proc = File::open("/proc")
        .map_err(|source| std_io_error("open mounted private preflight proc", source))?;
    let filesystem = rustix::fs::fstatfs(&proc)
        .map_err(|source| io_error("inspect mounted private preflight proc", source))?;
    let required = rustix::fs::StatVfsMountFlags::RDONLY
        | rustix::fs::StatVfsMountFlags::NOSUID
        | rustix::fs::StatVfsMountFlags::NODEV
        | rustix::fs::StatVfsMountFlags::NOEXEC;
    let flags = rustix::fs::fstatvfs(&proc)
        .map_err(|source| io_error("inspect private preflight proc flags", source))?
        .f_flag;
    if filesystem.f_type != 0x9fa0
        || !flags.contains(required)
        || std::fs::read_link("/proc/self")
            .map_err(|source| std_io_error("inspect private proc PID view", source))?
            != Path::new("1")
    {
        return Err(changed(
            "preflight proc lost its private read-only PID1 view",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    #[test]
    fn preflight_parent_record_requires_one_canonical_pid() {
        assert_eq!(
            unique_pid_field("Name:\tparent\nPPid:\t123\n", "PPid:").unwrap(),
            123
        );
        for text in [
            "",
            "Pid:\t123\n",
            "PPid:\t0\n",
            "PPid:\t1\n",
            "PPid:\t-1\n",
            "PPid:\t0123\n",
            "PPid:\t2147483648\n",
            "PPid:\t123 other\n",
            "PPid:\t123\nPPid:\t123\n",
        ] {
            assert!(unique_pid_field(text, "PPid:").is_err(), "{text:?}");
        }
    }

    #[test]
    fn preflight_parent_handle_requires_a_live_nonchild_pidfd() {
        let regular: OwnedFd = tempfile::tempfile().unwrap().into();
        assert!(require_nonchild_pidfd(&regular).is_err());
        let parent = rustix::process::getppid().unwrap();
        let descriptor =
            rustix::process::pidfd_open(parent, rustix::process::PidfdFlags::empty()).unwrap();
        require_nonchild_pidfd(&descriptor).unwrap();
        PreflightParentBindingV79 { descriptor }
            .require_live()
            .unwrap();

        let mut child = Command::new("/bin/true").spawn().unwrap();
        let descriptor = rustix::process::pidfd_open(
            rustix::process::Pid::from_child(&child),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap();
        child.wait().unwrap();
        assert!(
            PreflightParentBindingV79 { descriptor }
                .require_live()
                .is_err()
        );
    }

    fn retained(path: &Path) -> PreflightRootReopenV79 {
        let original = File::open(path).unwrap();
        let snapshot = snapshot(&fstat(&original).unwrap());
        PreflightRootReopenV79 {
            original,
            path: path.to_path_buf(),
            snapshot,
        }
    }

    #[test]
    fn preflight_root_reopen_preserves_custody_and_refuses_replacement_or_metadata_change() {
        for change in 0..4 {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().join("root");
            std::fs::create_dir(&root).unwrap();
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
            let state = retained(&root);
            assert_eq!(
                snapshot(&fstat(&state.reopen().unwrap()).unwrap()),
                state.snapshot
            );
            match change {
                0 => {
                    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap()
                }
                1 => {
                    std::fs::rename(&root, temporary.path().join("retained")).unwrap();
                    std::fs::create_dir(&root).unwrap();
                }
                2 => {
                    std::fs::rename(&root, temporary.path().join("retained")).unwrap();
                    symlink("retained", &root).unwrap();
                }
                _ => std::fs::create_dir(root.join("unexpected")).unwrap(),
            }
            assert!(state.reopen().is_err(), "mutation {change}");
            assert!(state.original.metadata().unwrap().is_dir());
        }
    }

    #[test]
    fn preflight_proc_records_refuse_regular_files() {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::write(temporary.path().join("status"), b"PPid:\t123\nPid:\t123\n").unwrap();
        assert!(read_proc_record_at(&File::open(temporary.path()).unwrap(), "status").is_err());
    }

    #[test]
    fn preflight_owned_child_reports_status_and_reaps_on_drop() {
        let child = Command::new("/bin/sh")
            .args(["-c", "exit 7"])
            .spawn()
            .unwrap();
        let mut state = OwnedPreflightPid1V79(Some(child));
        assert_eq!(state.wait().unwrap().code(), Some(7));
        assert!(state.0.is_none());
        state.terminate().unwrap();

        let child = Command::new("/bin/sleep").arg("60").spawn().unwrap();
        let state = OwnedPreflightPid1V79(Some(child));
        let descriptor = rustix::process::pidfd_open(
            rustix::process::Pid::from_child(state.0.as_ref().unwrap()),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap();
        drop(state);
        let mut descriptors = [PollFd::new(&descriptor, PollFlags::IN)];
        assert_eq!(
            poll(&mut descriptors, Some(&Timespec::default())).unwrap(),
            1
        );
    }

    #[test]
    #[ignore = "requires a reviewed private root namespace, actual static qualification tool and static BusyBox fixture"]
    fn qualification_preflight_pid1_proc_and_parent_death_are_isolated() {
        let tool = std::env::var_os("FE2O3_QUALIFICATION_TEST_BINARY").expect("actual static tool");
        let busybox = std::env::var_os("FE2O3_BUSYBOX_TEST_BINARY").expect("static test BusyBox");
        let script =
            std::env::var_os("FE2O3_PREFLIGHT_NAMESPACE_TEST_SCRIPT").expect("test script");
        assert_eq!(
            std::fs::read(&script).unwrap(),
            include_bytes!("preflight_namespace_v79_probe.py")
        );
        let output = Command::new("/usr/bin/python3")
            .args(["-I", "-B"])
            .arg(script)
            .arg(tool)
            .arg(busybox)
            .output()
            .unwrap();
        assert!(output.stdout.len() <= 65536 && output.stderr.len() <= 65536);
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| *line == "qualification_preflight_namespace_v79: PASS")
                .count(),
            1
        );
    }
}
