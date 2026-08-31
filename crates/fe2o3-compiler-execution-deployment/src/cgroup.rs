use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use rustix::fs::{
    Access, AtFlags, FileType, Mode, OFlags, ResolveFlags, accessat, fstat, fstatfs, mkdirat,
    openat2, unlinkat,
};
use rustix::process::Pid;

use super::supervisor::CompilerExecutionQualificationSupervisorLeaseV1;
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, invalid, io_error,
    std_io_error,
};

const CGROUP_ROOT_V1: &str = "/sys/fs/cgroup";
const SELF_CGROUP_V1: &str = "/proc/self/cgroup";
const CGROUP2_SUPER_MAGIC_V1: i64 = 0x6367_7270;
const CGROUP_CONTROL_MAX_BYTES_V1: usize = 4096;
const CGROUP_CLEANUP_TIMEOUT_V1: Duration = Duration::from_secs(10);
const CGROUP_POLL_INTERVAL_V1: Duration = Duration::from_millis(10);
const CGROUP_MAX_DEPTH_V1: usize = 64;
const CGROUP_MAX_DESCENDANTS_V1: usize = 4096;

/// Move-only supervisor custody of one disposable cgroup V2 qualification scope.
///
/// The supervisor creates this scope before releasing the worker's parent lease. Every process
/// launched by that worker inherits the scope. Successful cleanup requires aggregate
/// `populated=0`, removes every empty nested machine cgroup under fixed bounds, and removes the
/// exact retained scope before caller-visible evidence can be published.
pub struct CompilerExecutionQualificationCgroupV1 {
    parent: File,
    scope: Option<File>,
    name: String,
    expected_membership: String,
}

/// Evidence that one qualification cgroup was empty and completely removed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionQualificationCgroupCleanupV1 {
    residual_processes_killed: bool,
    descendant_cgroup_count: usize,
}

impl CompilerExecutionQualificationCgroupCleanupV1 {
    /// Returns whether cleanup found processes after the supervised worker had been reaped.
    pub const fn residual_processes_killed(&self) -> bool {
        self.residual_processes_killed
    }

    /// Returns the number of empty machine-created descendant cgroups removed.
    pub const fn descendant_cgroup_count(&self) -> usize {
        self.descendant_cgroup_count
    }
}

/// Creates one lease-identified child of the caller's current writable cgroup V2 domain.
///
/// This operation requires effective UID zero. It fails closed when the current cgroup is not a
/// writable unified-V2 domain, when `cgroup.kill` is unavailable, or when any retained identity
/// changes during creation. Dropping returned custody performs bounded best-effort kill/removal.
pub fn create_compiler_execution_qualification_cgroup_v1(
    lease: &CompilerExecutionQualificationSupervisorLeaseV1,
) -> Result<CompilerExecutionQualificationCgroupV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "qualification cgroup creation requires effective UID 0",
        ));
    }
    let membership = read_bounded_path(SELF_CGROUP_V1, CGROUP_CONTROL_MAX_BYTES_V1)?;
    let current_path = parse_unified_cgroup_path(&membership)?;
    let cgroup_root =
        File::open(CGROUP_ROOT_V1).map_err(|source| std_io_error("open cgroup V2 root", source))?;
    require_cgroup_directory(&cgroup_root, "cgroup V2 root")?;
    let relative = relative_cgroup_path(&current_path);
    let parent = open_cgroup_directory(&cgroup_root, relative)?;
    require_cgroup_directory(&parent, "current qualification cgroup")?;
    accessat(&parent, ".", Access::WRITE_OK, AtFlags::EACCESS)
        .map_err(|source| io_error("admit writable current qualification cgroup", source))?;
    if current_path != Path::new("/") && read_cgroup_control(&parent, "cgroup.type")? != b"domain\n"
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "current qualification cgroup is not one V2 domain",
        ));
    }
    let membership_after = read_bounded_path(SELF_CGROUP_V1, CGROUP_CONTROL_MAX_BYTES_V1)?;
    if membership != membership_after {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "supervisor cgroup membership changed during scope admission",
        ));
    }

    let name = lease.cgroup_scope_name()?;
    match mkdirat(&parent, name.as_str(), Mode::from_raw_mode(0o755)) {
        Ok(()) => {}
        Err(rustix::io::Errno::EXIST) => {
            recover_stale_cgroup_scope(&parent, &name)?;
            mkdirat(&parent, name.as_str(), Mode::from_raw_mode(0o755))
                .map_err(|source| io_error("recreate recovered qualification cgroup", source))?;
        }
        Err(source) => return Err(io_error("create qualification cgroup scope", source)),
    }
    finish_cgroup_creation(parent, name, &current_path)
}

fn recover_stale_cgroup_scope(
    parent: &File,
    name: &str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let scope = open_cgroup_directory(parent, Path::new(name))?;
    require_cgroup_directory(&scope, "stale qualification cgroup scope")?;
    if cgroup_is_populated(&scope)? {
        write_cgroup_control(&scope, "cgroup.kill", b"1\n")?;
        wait_until_cgroup_empty(&scope, CGROUP_CLEANUP_TIMEOUT_V1)?;
    }
    let mut descendant_count = 0;
    remove_descendant_cgroups(&scope, 0, &mut descendant_count)?;
    drop(scope);
    unlinkat(parent, name, AtFlags::REMOVEDIR)
        .map_err(|source| io_error("remove stale qualification cgroup scope", source))
}

fn finish_cgroup_creation(
    parent: File,
    name: String,
    current_path: &Path,
) -> Result<CompilerExecutionQualificationCgroupV1, DeploymentVerificationErrorV1> {
    let scope = match open_cgroup_directory(&parent, Path::new(&name)) {
        Ok(scope) => scope,
        Err(error) => {
            let _ = unlinkat(&parent, name.as_str(), AtFlags::REMOVEDIR);
            return Err(error);
        }
    };
    let mut cgroup = CompilerExecutionQualificationCgroupV1 {
        parent,
        scope: Some(scope),
        name: name.clone(),
        expected_membership: expected_child_membership(current_path, &name),
    };
    let result = (|| {
        let scope = cgroup.scope();
        require_cgroup_directory(scope, "qualification cgroup scope")?;
        if read_cgroup_control(scope, "cgroup.type")? != b"domain\n"
            || parse_cgroup_events(&read_cgroup_control(scope, "cgroup.events")?)?
                != (CgroupEventsV1 {
                    populated: false,
                    frozen: false,
                })
            || !read_cgroup_control(scope, "cgroup.procs")?.is_empty()
        {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "new qualification cgroup is not one empty unfrozen domain",
            ));
        }
        let kill = open_cgroup_control(scope, "cgroup.kill", OFlags::WRONLY)?;
        let kill_stat = fstat(&kill)
            .map_err(|source| io_error("inspect qualification cgroup kill control", source))?;
        if FileType::from_raw_mode(kill_stat.st_mode) != FileType::RegularFile {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "qualification cgroup does not expose cgroup.kill",
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        cgroup.best_effort_cleanup();
        return Err(error);
    }
    Ok(cgroup)
}

impl CompilerExecutionQualificationCgroupV1 {
    fn scope(&self) -> &File {
        self.scope
            .as_ref()
            .expect("active qualification cgroup retains its scope")
    }

    /// Moves the still lease-blocked worker into this scope and verifies exact membership.
    pub fn attach_worker(&self, child: &Child) -> Result<(), DeploymentVerificationErrorV1> {
        let pid = Pid::from_child(child);
        if rustix::process::getpgid(Some(pid)) != Ok(pid) {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "qualification worker is not its process-group leader before cgroup attachment",
            ));
        }
        let pid_line = format!("{}\n", pid.as_raw_pid());
        write_cgroup_control(self.scope(), "cgroup.procs", pid_line.as_bytes())?;
        let observed = read_bounded_path(
            format!("/proc/{}/cgroup", pid.as_raw_pid()),
            CGROUP_CONTROL_MAX_BYTES_V1,
        )?;
        if observed != self.expected_membership.as_bytes()
            || read_cgroup_control(self.scope(), "cgroup.procs")? != pid_line.as_bytes()
            || parse_cgroup_events(&read_cgroup_control(self.scope(), "cgroup.events")?)?
                != (CgroupEventsV1 {
                    populated: true,
                    frozen: false,
                })
        {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "qualification worker did not enter the exact retained cgroup scope",
            ));
        }
        Ok(())
    }

    /// Kills any residual scope members, removes nested machine cgroups, and removes this scope.
    ///
    /// Callers decide whether `residual_processes_killed` is expected for a timeout/signal or is a
    /// successful-worker containment failure. `Ok` always means the complete scope was empty and
    /// removed before this function returned.
    pub fn cleanup(
        mut self,
    ) -> Result<CompilerExecutionQualificationCgroupCleanupV1, DeploymentVerificationErrorV1> {
        let events = parse_cgroup_events(&read_cgroup_control(self.scope(), "cgroup.events")?)?;
        if events.frozen {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
                "qualification cgroup became frozen",
            ));
        }
        if events.populated {
            write_cgroup_control(self.scope(), "cgroup.kill", b"1\n")?;
            wait_until_cgroup_empty(self.scope(), CGROUP_CLEANUP_TIMEOUT_V1)?;
        }
        let mut descendant_count = 0;
        remove_descendant_cgroups(self.scope(), 0, &mut descendant_count)?;
        let scope = self
            .scope
            .take()
            .expect("active qualification cgroup retains its scope");
        drop(scope);
        unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR)
            .map_err(|source| io_error("remove empty qualification cgroup scope", source))?;
        Ok(CompilerExecutionQualificationCgroupCleanupV1 {
            residual_processes_killed: events.populated,
            descendant_cgroup_count: descendant_count,
        })
    }

    fn best_effort_cleanup(&mut self) {
        let Some(scope) = self.scope.as_ref() else {
            return;
        };
        if cgroup_is_populated(scope).unwrap_or(true) {
            let _ = write_cgroup_control(scope, "cgroup.kill", b"1\n");
            let _ = wait_until_cgroup_empty(scope, CGROUP_CLEANUP_TIMEOUT_V1);
        }
        let mut count = 0;
        let _ = remove_descendant_cgroups(scope, 0, &mut count);
        self.scope.take();
        let _ = unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR);
    }
}

impl Drop for CompilerExecutionQualificationCgroupV1 {
    fn drop(&mut self) {
        self.best_effort_cleanup();
    }
}

pub(super) fn probe_current_qualification_cgroup_v2_v1() -> bool {
    let Ok(membership) = read_bounded_path(SELF_CGROUP_V1, CGROUP_CONTROL_MAX_BYTES_V1) else {
        return false;
    };
    let Ok(path) = parse_unified_cgroup_path(&membership) else {
        return false;
    };
    let Ok(root) = File::open(CGROUP_ROOT_V1) else {
        return false;
    };
    if require_cgroup_directory(&root, "cgroup V2 root").is_err() {
        return false;
    }
    let Ok(parent) = open_cgroup_directory(&root, relative_cgroup_path(&path)) else {
        return false;
    };
    accessat(&parent, ".", Access::WRITE_OK, AtFlags::EACCESS).is_ok()
        && (path == Path::new("/")
            || read_cgroup_control(&parent, "cgroup.type").is_ok_and(|value| value == b"domain\n"))
}

fn open_cgroup_directory(
    parent: &File,
    path: &Path,
) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        parent,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open retained qualification cgroup directory", source))
}

fn open_cgroup_control(
    scope: &File,
    name: &str,
    flags: OFlags,
) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        scope,
        name,
        flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open qualification cgroup control", source))
}

fn require_cgroup_directory(
    directory: &File,
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let stat = fstat(directory)
        .map_err(|source| io_error("inspect qualification cgroup directory", source))?;
    let filesystem = fstatfs(directory)
        .map_err(|source| io_error("inspect qualification cgroup filesystem", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || (stat.st_uid, stat.st_gid) != (0, 0)
        || filesystem.f_type != CGROUP2_SUPER_MAGIC_V1
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            format!("{role} is not one root-owned cgroup V2 directory"),
        ));
    }
    Ok(())
}

fn read_cgroup_control(scope: &File, name: &str) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let file = open_cgroup_control(scope, name, OFlags::RDONLY)?;
    read_bounded_file(
        file,
        CGROUP_CONTROL_MAX_BYTES_V1,
        "read qualification cgroup control",
    )
}

fn write_cgroup_control(
    scope: &File,
    name: &str,
    bytes: &[u8],
) -> Result<(), DeploymentVerificationErrorV1> {
    let mut file = open_cgroup_control(scope, name, OFlags::WRONLY)?;
    file.write_all(bytes)
        .map_err(|source| std_io_error("write qualification cgroup control", source))
}

fn read_bounded_path(
    path: impl AsRef<Path>,
    max_bytes: usize,
) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let file = File::open(path)
        .map_err(|source| std_io_error("open bounded qualification process record", source))?;
    read_bounded_file(file, max_bytes, "read bounded qualification process record")
}

fn read_bounded_file(
    file: File,
    max_bytes: usize,
    operation: &'static str,
) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let mut bytes = Vec::new();
    file.take(u64::try_from(max_bytes).expect("control bound fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| std_io_error(operation, source))?;
    if bytes.len() > max_bytes {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification process control exceeds its fixed byte bound",
        ));
    }
    Ok(bytes)
}

fn parse_unified_cgroup_path(bytes: &[u8]) -> Result<PathBuf, DeploymentVerificationErrorV1> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup membership is not UTF-8",
        )
    })?;
    let line = text.strip_suffix('\n').ok_or_else(|| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup membership is not newline terminated",
        )
    })?;
    let path = line.strip_prefix("0::").ok_or_else(|| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification host does not expose one unified cgroup V2 membership",
        )
    })?;
    if path.is_empty()
        || path.contains('\n')
        || path.contains("//")
        || (path.len() > 1 && path.ends_with('/'))
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup membership is not one canonical record",
        ));
    }
    let path = PathBuf::from(path);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup membership path is not canonical absolute",
        ));
    }
    Ok(path)
}

fn relative_cgroup_path(path: &Path) -> &Path {
    let relative = path
        .strip_prefix("/")
        .expect("admitted cgroup path is absolute");
    if relative.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative
    }
}

fn expected_child_membership(parent: &Path, name: &str) -> String {
    if parent == Path::new("/") {
        format!("0::/{name}\n")
    } else {
        format!("0::{}/{name}\n", parent.display())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CgroupEventsV1 {
    populated: bool,
    frozen: bool,
}

fn parse_cgroup_events(bytes: &[u8]) -> Result<CgroupEventsV1, DeploymentVerificationErrorV1> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup events are not UTF-8",
        )
    })?;
    let mut lines = text.lines();
    let populated = parse_cgroup_event(lines.next(), "populated")?;
    let frozen = parse_cgroup_event(lines.next(), "frozen")?;
    if lines.next().is_some() || !text.ends_with('\n') {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup events inventory is not canonical",
        ));
    }
    Ok(CgroupEventsV1 { populated, frozen })
}

fn parse_cgroup_event(
    line: Option<&str>,
    expected_name: &str,
) -> Result<bool, DeploymentVerificationErrorV1> {
    match line {
        Some(line) if line == format!("{expected_name} 0") => Ok(false),
        Some(line) if line == format!("{expected_name} 1") => Ok(true),
        _ => Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            format!("qualification cgroup {expected_name} event is not canonical"),
        )),
    }
}

fn cgroup_is_populated(scope: &File) -> Result<bool, DeploymentVerificationErrorV1> {
    let events = parse_cgroup_events(&read_cgroup_control(scope, "cgroup.events")?)?;
    if events.frozen {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification cgroup became frozen during cleanup",
        ));
    }
    Ok(events.populated)
}

fn wait_until_cgroup_empty(
    scope: &File,
    timeout: Duration,
) -> Result<(), DeploymentVerificationErrorV1> {
    let deadline = Instant::now() + timeout;
    loop {
        if !cgroup_is_populated(scope)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                "qualification cgroup remained populated after cgroup.kill",
            ));
        }
        std::thread::sleep(
            CGROUP_POLL_INTERVAL_V1.min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn remove_descendant_cgroups(
    scope: &File,
    depth: usize,
    count: &mut usize,
) -> Result<(), DeploymentVerificationErrorV1> {
    if depth >= CGROUP_MAX_DEPTH_V1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::CleanupFailed,
            "qualification cgroup descendants exceed the fixed depth bound",
        ));
    }
    let scan = rustix::io::dup(scope)
        .map_err(|source| io_error("duplicate qualification cgroup for enumeration", source))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|source| io_error("enumerate qualification cgroup descendants", source))?;
    let mut children = Vec::new();
    for entry in &mut entries {
        let entry =
            entry.map_err(|source| io_error("read qualification cgroup descendant", source))?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") || entry.file_type() != FileType::Directory {
            continue;
        }
        let name = std::str::from_utf8(name).map_err(|_| {
            invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                "qualification cgroup descendant name is not UTF-8",
            )
        })?;
        *count = count.checked_add(1).ok_or_else(|| {
            invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                "qualification cgroup descendant count overflowed",
            )
        })?;
        if *count > CGROUP_MAX_DESCENDANTS_V1 {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                "qualification cgroup descendants exceed the fixed count bound",
            ));
        }
        children.push(name.to_owned());
    }
    children.sort_unstable();
    for name in children {
        let child = open_cgroup_directory(scope, Path::new(&name))?;
        remove_descendant_cgroups(&child, depth + 1, count)?;
        drop(child);
        match unlinkat(scope, name.as_str(), AtFlags::REMOVEDIR) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(source) => return Err(io_error("remove descendant qualification cgroup", source)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_cgroup_membership_parser_is_exact() {
        assert_eq!(
            parse_unified_cgroup_path(b"0::/user.slice/session.scope\n").unwrap(),
            Path::new("/user.slice/session.scope")
        );
        assert_eq!(
            parse_unified_cgroup_path(b"0::/\n").unwrap(),
            Path::new("/")
        );
        for bytes in [
            b"0::relative\n".as_slice(),
            b"1::/scope\n".as_slice(),
            b"0::/scope".as_slice(),
            b"0::/scope\n0::/other\n".as_slice(),
            b"0::/scope/../other\n".as_slice(),
        ] {
            assert_eq!(
                parse_unified_cgroup_path(bytes).unwrap_err().kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation
            );
        }
    }

    #[test]
    fn child_membership_is_canonical_for_root_and_nested_parent() {
        assert_eq!(
            expected_child_membership(Path::new("/"), "fe2o3-test"),
            "0::/fe2o3-test\n"
        );
        assert_eq!(
            expected_child_membership(Path::new("/a/b"), "fe2o3-test"),
            "0::/a/b/fe2o3-test\n"
        );
    }

    #[test]
    fn cgroup_events_parser_requires_exact_order_names_values_and_termination() {
        assert_eq!(
            parse_cgroup_events(b"populated 1\nfrozen 0\n").unwrap(),
            CgroupEventsV1 {
                populated: true,
                frozen: false,
            }
        );
        for bytes in [
            b"frozen 0\npopulated 1\n".as_slice(),
            b"populated 2\nfrozen 0\n".as_slice(),
            b"populated 1\nfrozen 0".as_slice(),
            b"populated 1\nfrozen 0\nextra 0\n".as_slice(),
        ] {
            assert_eq!(
                parse_cgroup_events(bytes).unwrap_err().kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationIsolation
            );
        }
    }

    #[test]
    fn current_scope_probe_is_nonmutating() {
        let _ = probe_current_qualification_cgroup_v2_v1();
    }
}
