use std::fmt::Write as _;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use rustix::fs::{major, minor};
use rustix::mount::{FsOpenFlags, fsopen};

use super::{DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, std_io_error};

const LOOP_CONTROL_PATH_V1: &str = "/dev/loop-control";
const LOOP_CONTROL_MAJOR_V1: u32 = 10;
const LOOP_CONTROL_MINOR_V1: u32 = 237;
const QUALIFICATION_HOST_PROBE_SCHEMA_V1: &str =
    "fe2o3-compiler-execution-qualification-host-probe-v1";

/// Read-only observation of host prerequisites for disposable-root qualification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionQualificationHostProbeV1 {
    effective_uid: u32,
    task_count: usize,
    mount_namespace_present: bool,
    proc_fd_present: bool,
    loop_control_identity: bool,
    squashfs_advertised: bool,
    overlayfs_advertised: bool,
    new_mount_api_recognized: bool,
    isolation_namespaces_present: bool,
    cgroup_v2_present: bool,
    systemd_analyze_present: bool,
    systemd_sysusers_present: bool,
    systemd_tmpfiles_present: bool,
    systemd_nspawn_present: bool,
}

impl CompilerExecutionQualificationHostProbeV1 {
    /// Returns whether the observed host satisfies the prerequisites for the mount-only harness.
    pub const fn mount_ready(&self) -> bool {
        self.effective_uid == 0
            && self.task_count == 1
            && self.mount_namespace_present
            && self.proc_fd_present
            && self.loop_control_identity
            && self.squashfs_advertised
            && self.overlayfs_advertised
            && self.new_mount_api_recognized
    }

    /// Returns whether the observed host also advertises prerequisites for isolated systemd work.
    pub const fn isolated_systemd_ready(&self) -> bool {
        self.mount_ready()
            && self.isolation_namespaces_present
            && self.cgroup_v2_present
            && self.systemd_analyze_present
            && self.systemd_sysusers_present
            && self.systemd_tmpfiles_present
            && self.systemd_nspawn_present
    }

    /// Encodes the observation as one stable newline-terminated key-value report.
    pub fn canonical_report(&self) -> String {
        let mut report = String::new();
        writeln!(report, "probe_schema={QUALIFICATION_HOST_PROBE_SCHEMA_V1}")
            .expect("writing to a String cannot fail");
        writeln!(report, "effective_uid={}", self.effective_uid)
            .expect("writing to a String cannot fail");
        writeln!(report, "task_count={}", self.task_count)
            .expect("writing to a String cannot fail");
        for (name, value) in [
            ("mount_namespace_present", self.mount_namespace_present),
            ("proc_fd_present", self.proc_fd_present),
            ("loop_control_identity", self.loop_control_identity),
            ("squashfs_advertised", self.squashfs_advertised),
            ("overlayfs_advertised", self.overlayfs_advertised),
            ("new_mount_api_recognized", self.new_mount_api_recognized),
            (
                "isolation_namespaces_present",
                self.isolation_namespaces_present,
            ),
            ("cgroup_v2_present", self.cgroup_v2_present),
            ("systemd_analyze_present", self.systemd_analyze_present),
            ("systemd_sysusers_present", self.systemd_sysusers_present),
            ("systemd_tmpfiles_present", self.systemd_tmpfiles_present),
            ("systemd_nspawn_present", self.systemd_nspawn_present),
            ("mount_ready", self.mount_ready()),
            ("isolated_systemd_ready", self.isolated_systemd_ready()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Observes qualification prerequisites without creating namespaces, mounts, or services.
pub fn probe_compiler_execution_qualification_host_v1()
-> Result<CompilerExecutionQualificationHostProbeV1, DeploymentVerificationErrorV1> {
    let filesystems = std::fs::read_to_string("/proc/filesystems").map_err(|source| {
        std_io_error(
            "read Linux filesystem inventory for qualification probe",
            source,
        )
    })?;
    Ok(CompilerExecutionQualificationHostProbeV1 {
        effective_uid: rustix::process::geteuid().as_raw(),
        task_count: process_thread_count()?,
        mount_namespace_present: path_exists("/proc/self/ns/mnt"),
        proc_fd_present: path_exists("/proc/self/fd"),
        loop_control_identity: loop_control_identity(),
        squashfs_advertised: filesystem_advertised(&filesystems, "squashfs"),
        overlayfs_advertised: filesystem_advertised(&filesystems, "overlay"),
        new_mount_api_recognized: new_mount_api_recognized(),
        isolation_namespaces_present: ["pid", "net", "ipc", "uts", "cgroup", "mnt"]
            .iter()
            .all(|name| path_exists(Path::new("/proc/self/ns").join(name))),
        cgroup_v2_present: path_exists("/sys/fs/cgroup/cgroup.controllers"),
        systemd_analyze_present: executable_exists("/usr/bin/systemd-analyze"),
        systemd_sysusers_present: executable_exists("/usr/bin/systemd-sysusers"),
        systemd_tmpfiles_present: executable_exists("/usr/bin/systemd-tmpfiles"),
        systemd_nspawn_present: executable_exists("/usr/bin/systemd-nspawn"),
    })
}

pub(super) fn process_thread_count() -> Result<usize, DeploymentVerificationErrorV1> {
    let mut count = 0_usize;
    for entry in std::fs::read_dir("/proc/self/task")
        .map_err(|source| std_io_error("enumerate qualification process tasks", source))?
    {
        entry.map_err(|source| std_io_error("read qualification process task", source))?;
        count = count.checked_add(1).ok_or_else(|| {
            super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "qualification process task count overflowed",
            )
        })?;
        if count > 1 {
            break;
        }
    }
    Ok(count)
}

fn path_exists(path: impl AsRef<Path>) -> bool {
    std::fs::metadata(path).is_ok()
}

fn executable_exists(path: impl AsRef<Path>) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file() && metadata.permissions().mode() & 0o111 != 0
    })
}

fn loop_control_identity() -> bool {
    std::fs::symlink_metadata(LOOP_CONTROL_PATH_V1).is_ok_and(|metadata| {
        metadata.file_type().is_char_device()
            && metadata.uid() == 0
            && major(metadata.rdev()) == LOOP_CONTROL_MAJOR_V1
            && minor(metadata.rdev()) == LOOP_CONTROL_MINOR_V1
    })
}

fn filesystem_advertised(inventory: &str, expected: &str) -> bool {
    inventory.lines().any(|line| {
        line.split_ascii_whitespace()
            .next_back()
            .is_some_and(|name| name == expected)
    })
}

fn new_mount_api_recognized() -> bool {
    match fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC) {
        Ok(_context) => true,
        Err(rustix::io::Errno::NOSYS) => false,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_inventory_parser_accepts_nodev_and_plain_entries() {
        let inventory = "nodev\tsysfs\n\tsquashfs\nnodev\toverlay\n";
        assert!(filesystem_advertised(inventory, "squashfs"));
        assert!(filesystem_advertised(inventory, "overlay"));
        assert!(!filesystem_advertised(inventory, "ext4"));
    }

    #[test]
    fn host_probe_report_has_one_canonical_value_per_prerequisite() {
        let probe = probe_compiler_execution_qualification_host_v1().unwrap();
        let report = probe.canonical_report();
        assert!(report.ends_with('\n'));
        assert_eq!(report.lines().count(), 17);
        assert_eq!(
            report.lines().next(),
            Some("probe_schema=fe2o3-compiler-execution-qualification-host-probe-v1")
        );
        assert_eq!(
            report
                .lines()
                .filter(|line| line.starts_with("mount_ready="))
                .count(),
            1
        );
        assert_eq!(
            report
                .lines()
                .filter(|line| line.starts_with("isolated_systemd_ready="))
                .count(),
            1
        );
    }
}
