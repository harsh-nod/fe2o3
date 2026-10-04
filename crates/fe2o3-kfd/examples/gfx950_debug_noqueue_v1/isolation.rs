//! Bounded observational refusal fences, NOT process-isolation authority.
//! The unsafe activation caller relies on the standalone program/dependency
//! closure and its supervisor. /proc snapshots cannot exclude hidden prior
//! activity, a race, constructor tampering, ptrace injection or future activity.
use super::Failure;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

const MAP_BYTES: usize = 1024 * 1024;
const MAP_ROWS: usize = 4096;
const FD_LIMIT: usize = 64;
const ENV_LIMIT: usize = 16;
const LIBRARIES: [&str; 3] = [
    "/usr/lib/x86_64-linux-gnu/libc.so.6",
    "/usr/lib/x86_64-linux-gnu/libgcc_s.so.1",
    "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
];

fn refuse(reason: &'static str) -> Failure {
    Failure::new("process_isolation_snapshot", reason)
}

pub(super) struct EntryFence {
    pid: u32,
    executable: PathBuf,
    executable_device: u64,
    executable_inode: u64,
}
impl EntryFence {
    pub(super) fn at_entry() -> Result<Self, Failure> {
        let pid = std::process::id();
        if pid == 0 {
            return Err(refuse("zero_process_identity"));
        }
        check_environment(std::env::vars_os())?;
        check_tasks(pid)?;
        let executable = fs::read_link("/proc/self/exe").map_err(|_| refuse("executable_link"))?;
        if !executable.is_absolute()
            || fs::canonicalize(&executable).ok().as_ref() != Some(&executable)
        {
            return Err(refuse("canonical_executable_required"));
        }
        let metadata = fs::metadata("/proc/self/exe").map_err(|_| refuse("executable_metadata"))?;
        if !metadata.is_file() {
            return Err(refuse("regular_executable_required"));
        }
        let result = Self {
            pid,
            executable,
            executable_device: metadata.dev(),
            executable_inode: metadata.ino(),
        };
        result.recheck(false)?;
        Ok(result)
    }
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }
    pub(super) fn recheck_fresh(&self) -> Result<(), Failure> {
        self.recheck(false)
    }
    pub(super) fn recheck_prepared(&self) -> Result<(), Failure> {
        self.recheck(true)
    }
    fn recheck(&self, prepared: bool) -> Result<(), Failure> {
        if std::process::id() != self.pid {
            return Err(refuse("process_identity_changed"));
        }
        check_tasks(self.pid)?;
        check_environment(std::env::vars_os())?;
        match fs::File::open("/etc/ld.so.preload") {
            Ok(file) => {
                if !read_bounded(file, 4096)?.is_empty() {
                    return Err(refuse("loader_preload_file_nonempty"));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(refuse("loader_preload_file_unreadable")),
        }
        if fs::read_link("/proc/self/exe").ok().as_ref() != Some(&self.executable) {
            return Err(refuse("executable_identity_changed"));
        }
        let current = fs::metadata(&self.executable).map_err(|_| refuse("executable_metadata"))?;
        if !current.is_file()
            || current.dev() != self.executable_device
            || current.ino() != self.executable_inode
        {
            return Err(refuse("executable_identity_changed"));
        }
        let gpu_paths = check_fds(prepared)?;
        let maps = read_bounded(
            fs::File::open("/proc/self/maps").map_err(|_| refuse("maps_open"))?,
            MAP_BYTES,
        )?;
        let parsed = parse_maps(&maps)?;
        for row in parsed {
            if row.path.is_empty() || row.path.starts_with('[') {
                if row.executable && !matches!(row.path, "[vdso]" | "[vsyscall]") {
                    return Err(refuse("anonymous_or_special_executable_mapping"));
                }
                if row.path.starts_with('[')
                    && !matches!(
                        row.path,
                        "[heap]" | "[stack]" | "[vdso]" | "[vvar]" | "[vvar_vclock]" | "[vsyscall]"
                    )
                {
                    return Err(refuse("unknown_special_mapping"));
                }
                continue;
            }
            let path = Path::new(row.path);
            if !path.is_absolute() || row.path.ends_with(" (deleted)") {
                return Err(refuse("unnamed_or_deleted_file_mapping"));
            }
            let canonical =
                fs::canonicalize(path).map_err(|_| refuse("mapping_path_currentness"))?;
            let approved_gpu =
                mapping_path_policy(&canonical, &self.executable, prepared, &gpu_paths)?;
            if approved_gpu && row.executable {
                return Err(refuse("executable_gpu_device_mapping"));
            }
            // The path string alone is not enough: compare the mapped inode and
            // device against the current file. Supervisor pins content separately.
            let metadata = fs::metadata(&canonical).map_err(|_| refuse("mapping_metadata"))?;
            if metadata.ino() != row.inode
                || u64::from(libc::major(metadata.dev())) != row.major
                || u64::from(libc::minor(metadata.dev())) != row.minor
            {
                return Err(refuse("mapped_file_identity_changed"));
            }
        }
        check_tasks(self.pid)?;
        Ok(())
    }
}

fn check_environment(values: impl Iterator<Item = (OsString, OsString)>) -> Result<(), Failure> {
    let mut count = 0;
    for (name, value) in values {
        count += 1;
        if count > ENV_LIMIT || name.as_bytes().len() > 64 || value.as_bytes().len() > 4096 {
            return Err(refuse("environment_bound"));
        }
        // Closed allowlist is deliberately stronger than enumerating LD_PRELOAD,
        // LD_AUDIT, HSA_TOOLS_LIB, ROCP_TOOL_LIB and their future variants.
        if !matches!(name.to_str(), Some("LANG" | "LC_ALL"))
            || !matches!(value.to_str(), Some("C" | "C.UTF-8"))
        {
            return Err(refuse("nonisolated_environment"));
        }
    }
    Ok(())
}

fn task_identity(names: impl Iterator<Item = OsString>, pid: u32) -> Result<(), Failure> {
    let mut count = 0usize;
    for name in names {
        count += 1;
        if count != 1 || decimal_component(&name) != Some(pid) {
            return Err(refuse("not_exactly_one_process_main_thread"));
        }
    }
    if count != 1 {
        return Err(refuse("not_exactly_one_process_main_thread"));
    }
    Ok(())
}
fn check_tasks(pid: u32) -> Result<(), Failure> {
    let mut names = Vec::new();
    for result in fs::read_dir("/proc/self/task").map_err(|_| refuse("task_directory"))? {
        if names.len() == 2 {
            return Err(refuse("not_exactly_one_process_main_thread"));
        }
        names.push(result.map_err(|_| refuse("task_entry"))?.file_name());
    }
    task_identity(names.into_iter(), pid)
}
fn decimal_component(name: &OsStr) -> Option<u32> {
    let value = name.to_str()?;
    if value.is_empty()
        || value.len() > 10
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn check_fds(prepared: bool) -> Result<Vec<PathBuf>, Failure> {
    let kfd = fs::metadata("/dev/kfd").map_err(|_| refuse("kfd_node_metadata"))?;
    if !kfd.file_type().is_char_device() {
        return Err(refuse("kfd_node_not_character_device"));
    }
    let mut count = 0usize;
    let mut kfd_count = 0usize;
    let mut drm_count = 0usize;
    let mut paths = Vec::new();
    for result in fs::read_dir("/proc/self/fd").map_err(|_| refuse("fd_directory"))? {
        count += 1;
        if count > FD_LIMIT {
            return Err(refuse("fd_count_bound"));
        }
        let entry = result.map_err(|_| refuse("fd_entry"))?;
        if decimal_component(&entry.file_name()).is_none() {
            return Err(refuse("fd_name"));
        }
        let metadata = fs::metadata(entry.path()).map_err(|_| refuse("fd_metadata"))?;
        if !metadata.file_type().is_char_device() {
            continue;
        }
        let is_kfd = metadata.rdev() == kfd.rdev();
        let is_drm = libc::major(metadata.rdev()) == 226;
        if !is_kfd && !is_drm {
            continue;
        }
        if !prepared {
            return Err(refuse("preexisting_gpu_fd"));
        }
        kfd_count += usize::from(is_kfd);
        drm_count += usize::from(is_drm);
        if kfd_count > 1 || drm_count > 1 {
            return Err(refuse("unexpected_owned_gpu_fd_count"));
        }
        let link = fs::read_link(entry.path()).map_err(|_| refuse("gpu_fd_link"))?;
        let path = fs::canonicalize(link).map_err(|_| refuse("gpu_fd_path"))?;
        if is_kfd && path != Path::new("/dev/kfd") {
            return Err(refuse("unexpected_kfd_path"));
        }
        if is_drm && path.parent() != Some(Path::new("/dev/dri")) {
            return Err(refuse("unexpected_drm_path"));
        }
        paths.push(path);
    }
    gpu_fd_count_policy(prepared, kfd_count, drm_count)?;
    Ok(paths)
}

fn gpu_fd_count_policy(prepared: bool, kfd: usize, drm: usize) -> Result<(), Failure> {
    if (!prepared && (kfd != 0 || drm != 0)) || (prepared && (kfd != 1 || drm != 1)) {
        Err(refuse("unexpected_gpu_fd_roster"))
    } else {
        Ok(())
    }
}

fn mapping_path_policy(
    path: &Path,
    executable: &Path,
    prepared: bool,
    gpu_paths: &[PathBuf],
) -> Result<bool, Failure> {
    if path == executable || LIBRARIES.iter().any(|allowed| path == Path::new(allowed)) {
        Ok(false)
    } else if prepared && gpu_paths.iter().any(|allowed| path == allowed.as_path()) {
        Ok(true)
    } else {
        Err(refuse("foreign_or_unreviewed_runtime_mapping"))
    }
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>, Failure> {
    let capacity = limit
        .checked_add(1)
        .ok_or_else(|| refuse("snapshot_bound"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| refuse("snapshot_allocation"))?;
    reader
        .take(capacity as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| refuse("snapshot_read"))?;
    if bytes.len() > limit {
        return Err(refuse("snapshot_bound"));
    }
    Ok(bytes)
}

#[derive(Debug, Eq, PartialEq)]
struct MapRow<'a> {
    executable: bool,
    major: u64,
    minor: u64,
    inode: u64,
    path: &'a str,
}
fn parse_maps(bytes: &[u8]) -> Result<Vec<MapRow<'_>>, Failure> {
    if bytes.len() > MAP_BYTES || bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(refuse("maps_bound_or_truncated"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| refuse("maps_encoding"))?;
    let mut result = Vec::new();
    for line in text.lines() {
        if result.len() == MAP_ROWS || line.len() > 4096 {
            return Err(refuse("maps_row_bound"));
        }
        let mut fields = line.split_ascii_whitespace();
        let range = fields.next().ok_or_else(|| refuse("maps_shape"))?;
        let permissions = fields.next().ok_or_else(|| refuse("maps_shape"))?;
        let offset = fields.next().ok_or_else(|| refuse("maps_shape"))?;
        let device = fields.next().ok_or_else(|| refuse("maps_shape"))?;
        let inode = fields.next().ok_or_else(|| refuse("maps_shape"))?;
        let path = fields.next().unwrap_or("");
        if fields.next().is_some() {
            return Err(refuse("maps_path_shape"));
        }
        let (low, high) = range.split_once('-').ok_or_else(|| refuse("maps_range"))?;
        let low = hex_component(low).ok_or_else(|| refuse("maps_range"))?;
        let high = hex_component(high).ok_or_else(|| refuse("maps_range"))?;
        if low >= high || hex_component(offset).is_none() {
            return Err(refuse("maps_range"));
        }
        if permissions.len() != 4
            || !matches!(permissions.as_bytes()[0], b'r' | b'-')
            || !matches!(permissions.as_bytes()[1], b'w' | b'-')
            || !matches!(permissions.as_bytes()[2], b'x' | b'-')
            || !matches!(permissions.as_bytes()[3], b'p' | b's')
        {
            return Err(refuse("maps_permissions"));
        }
        let (major, minor) = device
            .split_once(':')
            .ok_or_else(|| refuse("maps_device"))?;
        let major = hex_component(major).ok_or_else(|| refuse("maps_device"))?;
        let minor = hex_component(minor).ok_or_else(|| refuse("maps_device"))?;
        if inode.is_empty() || inode.len() > 20 || !inode.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(refuse("maps_inode"));
        }
        let inode = inode.parse().map_err(|_| refuse("maps_inode"))?;
        if permissions.as_bytes()[1] == b'w' && permissions.as_bytes()[2] == b'x' {
            return Err(refuse("writable_executable_mapping"));
        }
        result.push(MapRow {
            executable: permissions.as_bytes()[2] == b'x',
            major,
            minor,
            inode,
            path,
        });
    }
    Ok(result)
}
fn hex_component(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    u64::from_str_radix(value, 16).ok()
}

#[cfg(test)]
#[path = "isolation_tests.rs"]
mod tests;
