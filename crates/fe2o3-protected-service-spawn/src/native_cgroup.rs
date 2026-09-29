//! Fresh root-owned cgroup-v2 mechanics, not namespace or deployment admission.
//!
//! The caller prepays the stated logical bounds and publishes this owner in its
//! existing cleanup slot BEFORE `create`. Root must exclude concurrent privileged
//! mount, rename, migration and cgroup-control writers. In particular Linux has no
//! inode-conditional mkdir/open or rmdir transaction. Observed replacement fails
//! closed; these checks do not establish that external-writer exclusion.
//! Children receive neither these control descriptors nor writable cgroup access.
//! Once cleanup starts, no further clone may target this domain. Empty/removal is
//! domain completion only, never a substitute for exact child terminal waits.
//! Logical work/storage bounds do not bound syscall latency or kernel memory.

use crate::native_spawn::{ProtectedServiceSpawnErrorV2 as Error, Result, io};
use crate::process_cleanup::CleanupPollV1;
use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags, Stat};
use rustix::io::Errno;
use std::ffi::CStr;
use std::mem::size_of;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

const READ_LIMIT: usize = 4096;
const NAME_PREFIX: &[u8] = b"fe2o3-native-";
const NAME_BYTES: usize = NAME_PREFIX.len() + 32 + 1;
const CGROUP2_MAGIC: fs::FsWord = 0x6367_7270;
const OPERATION_WORK: usize = 1024 + 64;
const READ_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK);
const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_XDEV);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    NoCreation,
    Uncertain,
    Created,
    Cleaning,
    Removed,
    Quarantined,
}

impl Phase {
    fn may_release(self) -> bool {
        matches!(self, Self::Prepared | Self::NoCreation | Self::Removed)
    }

    fn after_mkdir(result: rustix::io::Result<()>) -> Self {
        match result {
            Ok(()) => Self::Uncertain, // Not pinned/validated yet.
            // Collision and access/read-only refusal are not owned creations.
            // All other failures conservatively retain the uncertain operation.
            Err(Errno::EXIST | Errno::ACCES | Errno::PERM | Errno::ROFS) => Self::NoCreation,
            Err(_) => Self::Quarantined,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
}

impl Identity {
    fn of(stat: &Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

/// Move-only domain custody. No path-based reconstruction or recovery exists.
#[must_use]
pub(crate) struct NativeCgroupDomainV1 {
    origin: rustix::process::Pid,
    phase: Phase,
    name: [u8; NAME_BYTES],
    parent_identity: Identity,
    directory_identity: Option<Identity>,
    parent: Option<OwnedFd>,
    directory: Option<OwnedFd>,
    kill: Option<OwnedFd>,
    events: Option<OwnedFd>,
}

impl NativeCgroupDomainV1 {
    /// Full fixed owner plus four logical descriptor-retention charges; no heap.
    pub(crate) const STORAGE: usize = size_of::<Self>() + 4 * size_of::<usize>();
    /// Prepay before preparation, including bounded parses and close-only rollback.
    pub(crate) const PREPARE_WORK: usize = 64 * OPERATION_WORK + 32 * READ_LIMIT;
    /// Preparation frames, fixed read/path buffers and temporary descriptors.
    pub(crate) const PREPARE_SCRATCH: usize =
        5 * READ_LIMIT + 4 * size_of::<Self>() + 8 * size_of::<Stat>() + 1024;
    /// Prepay before create, even when it fails or leaves uncertain custody.
    pub(crate) const CREATE_WORK: usize = 64 * OPERATION_WORK + 16 * READ_LIMIT;
    /// Fixed create/validation frames; the returned owner is charged separately.
    pub(crate) const CREATE_SCRATCH: usize =
        2 * READ_LIMIT + 4 * size_of::<Self>() + 8 * size_of::<Stat>() + 1024;
    /// Prepay before revalidating the actual clone target.
    pub(crate) const CLONE_FD_WORK: usize = 32 * OPERATION_WORK + 4 * NAME_BYTES;
    /// Revalidation frames; borrowing does not allocate or duplicate a descriptor.
    pub(crate) const CLONE_FD_SCRATCH: usize = 8 * size_of::<Stat>() + 4 * size_of::<Self>() + 1024;
    /// One finite kill/read/identity/remove turn, with no syscall retry loops.
    pub(crate) const STEP_WORK: usize = 64 * OPERATION_WORK + 16 * READ_LIMIT;
    /// Fixed cleanup frames and a single at-most-4096-byte event read buffer.
    pub(crate) const STEP_SCRATCH: usize = Self::CREATE_SCRATCH;

    /// Reads root's current actual membership and generates one fresh component.
    /// No mkdir, control write, or other persistent mutation occurs here.
    pub(crate) fn prepare() -> Result<Self> {
        require_root()?;
        let origin = rustix::process::getpid();
        let membership = open_proc(c"/proc/self/cgroup")?;
        let mut bytes = [0; READ_LIMIT];
        let count = read_record(&membership, &mut bytes)?;
        let path = membership_path(&bytes[..count])?;
        let mut relative = [0; READ_LIMIT];
        let relative = relative_path(path, &mut relative)?;
        let root = fs::openat2(
            fs::CWD,
            c"/sys/fs/cgroup",
            READ_FLAGS | OFlags::DIRECTORY,
            Mode::empty(),
            ResolveFlags::NO_SYMLINKS,
        )
        .map_err(|e| io("open actual cgroup-v2 mount", e))?;
        validate_fs(&root, CGROUP2_MAGIC)?;
        validate_root_stat(
            &fs::fstat(&root).map_err(|e| io("stat cgroup mount", e))?,
            true,
        )?;
        let parent = open_relative(&root, relative, READ_FLAGS | OFlags::DIRECTORY)?;
        validate_fs(&parent, CGROUP2_MAGIC)?;
        let parent_stat = fs::fstat(&parent).map_err(|e| io("stat current cgroup", e))?;
        validate_root_stat(&parent_stat, true)?;
        // A namespaced/bind-mounted view must still resolve the caller's actual
        // current cgroup, not another directory with a similar pathname.
        require_membership(&parent, origin)?;
        let mut current = [0; READ_LIMIT];
        let current_count = read_record(&membership, &mut current)?;
        if membership_path(&current[..current_count])? != path {
            return Err(Error::State("current cgroup moved during preparation"));
        }
        let random = open_proc(c"/proc/sys/kernel/random/uuid")?;
        let count = read_record(&random, &mut current)?;
        let name = generated_name(&current[..count])?;
        Ok(Self {
            origin,
            phase: Phase::Prepared,
            name,
            parent_identity: Identity::of(&parent_stat),
            directory_identity: None,
            parent: Some(parent),
            directory: None,
            kill: None,
            events: None,
        })
    }

    /// Caller must already have published this owner and reserved its retention.
    /// EEXIST never adopts, opens, kills or removes the existing directory.
    pub(crate) fn create(&mut self) -> Result<()> {
        self.require_origin()?;
        if self.phase != Phase::Prepared {
            return Err(Error::State("cgroup creation is single-use"));
        }
        self.check_parent()?;
        self.phase = Phase::Uncertain;
        let result = fs::mkdirat(self.parent()?, self.name()?, Mode::RWXU);
        self.phase = Phase::after_mkdir(result);
        result.map_err(|e| io("create fresh cgroup", e))?;
        let result = self.pin_created();
        match result {
            Ok(()) => self.phase = Phase::Created,
            Err(_) => self.phase = Phase::Quarantined,
        }
        result
    }

    /// Borrows only the validated created domain for atomic CLONE_INTO_CGROUP.
    /// The caller must not export this descriptor as a child binding.
    pub(crate) fn clone_cgroup_fd(&mut self) -> Result<BorrowedFd<'_>> {
        if self.phase != Phase::Created {
            return Err(Error::State("cgroup is not an available clone target"));
        }
        if let Err(error) = self.require_origin().and_then(|()| self.check_identity()) {
            self.phase = Phase::Quarantined;
            return Err(error);
        }
        Ok(self.directory()?.as_fd())
    }

    /// One prepaid turn; no loop, sleep, recursion, reaper or path-based adoption.
    /// Reaped means no owned creation, or observed empty plus successful removal.
    /// It does not imply a consuming wait for any process.
    pub(crate) fn step(&mut self) -> CleanupPollV1 {
        if self.phase.may_release() {
            self.phase = Phase::Removed;
            return CleanupPollV1::Reaped;
        }
        if matches!(self.phase, Phase::Uncertain | Phase::Quarantined) {
            self.phase = Phase::Quarantined;
            return CleanupPollV1::Quarantined;
        }
        self.phase = Phase::Cleaning; // Irrevocably closes further clone access.
        let result = self.cleanup_turn();
        match result {
            Ok(true) => {
                self.phase = Phase::Removed;
                CleanupPollV1::Reaped
            }
            Ok(false) => CleanupPollV1::Pending,
            Err(Error::Io { source, .. })
                if !matches!(
                    source,
                    Errno::NOENT | Errno::NOTDIR | Errno::LOOP | Errno::XDEV
                ) =>
            {
                CleanupPollV1::Pending
            }
            Err(_) => {
                self.phase = Phase::Quarantined;
                CleanupPollV1::Quarantined
            }
        }
    }

    fn pin_created(&mut self) -> Result<()> {
        self.directory = Some(open_relative(
            self.parent()?,
            self.name()?,
            READ_FLAGS | OFlags::DIRECTORY,
        )?);
        validate_fs(self.directory()?, CGROUP2_MAGIC)?;
        let stat = fs::fstat(self.directory()?).map_err(|e| io("stat created cgroup", e))?;
        validate_root_stat(&stat, true)?;
        if stat.st_mode & 0o077 != 0 || Identity::of(&stat) == self.parent_identity {
            return Err(Error::State(
                "created cgroup is not a private fresh directory",
            ));
        }
        self.directory_identity = Some(Identity::of(&stat));
        self.kill = Some(open_relative(
            self.directory()?,
            c"cgroup.kill",
            OFlags::WRONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        )?);
        self.events = Some(open_relative(
            self.directory()?,
            c"cgroup.events",
            READ_FLAGS,
        )?);
        let kind = open_relative(self.directory()?, c"cgroup.type", READ_FLAGS)?;
        validate_control(&kind, self.directory()?, c"cgroup.type")?;
        let mut bytes = [0; READ_LIMIT];
        let count = read_record(&kind, &mut bytes)?;
        if &bytes[..count] != b"domain\n" {
            return Err(Error::State("fresh cgroup is not a valid domain"));
        }
        self.check_identity()?;
        let count = read_record(self.events()?, &mut bytes)?;
        if parse_events(&bytes[..count])?.populated {
            return Err(Error::State("fresh cgroup was populated before clone"));
        }
        Ok(())
    }

    fn cleanup_turn(&self) -> Result<bool> {
        self.require_origin()?;
        self.check_identity()?;
        if rustix::io::pwrite(self.kill()?, b"1", 0).map_err(|e| io("kill owned cgroup", e))? != 1 {
            return Err(Error::State("cgroup.kill did not accept the exact token"));
        }
        let mut bytes = [0; READ_LIMIT];
        let count = read_record(self.events()?, &mut bytes)?;
        if parse_events(&bytes[..count])?.populated {
            return Ok(false);
        }
        self.check_identity()?;
        // Root's external-writer exclusion is still required across this pair.
        // Busy/nonempty removal remains pending, with no descendant traversal.
        fs::unlinkat(self.parent()?, self.name()?, AtFlags::REMOVEDIR)
            .map_err(|e| io("remove empty owned cgroup", e))?;
        Ok(true)
    }

    fn check_identity(&self) -> Result<()> {
        self.check_parent()?;
        let directory = self.directory()?;
        let actual = fs::fstat(directory).map_err(|e| io("stat owned cgroup", e))?;
        validate_root_stat(&actual, true)?;
        let expected = self
            .directory_identity
            .ok_or(Error::State("unidentified cgroup"))?;
        let named = fs::statat(self.parent()?, self.name()?, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|e| io("check named cgroup", e))?;
        if !same_directory(expected, &actual, &named) || actual.st_mode & 0o077 != 0 {
            return Err(Error::State("owned cgroup pathname or identity changed"));
        }
        validate_control(self.kill()?, directory, c"cgroup.kill")?;
        validate_control(self.events()?, directory, c"cgroup.events")
    }

    fn check_parent(&self) -> Result<()> {
        let stat = fs::fstat(self.parent()?).map_err(|e| io("stat retained cgroup parent", e))?;
        validate_root_stat(&stat, true)?;
        if Identity::of(&stat) != self.parent_identity {
            return Err(Error::State("retained cgroup parent identity changed"));
        }
        Ok(())
    }

    fn require_origin(&self) -> Result<()> {
        if rustix::process::getpid() != self.origin {
            return Err(Error::State(
                "cgroup custody belongs to its originating process",
            ));
        }
        require_root()
    }

    fn name(&self) -> Result<&CStr> {
        CStr::from_bytes_with_nul(&self.name)
            .map_err(|_| Error::State("invalid generated cgroup name"))
    }

    fn parent(&self) -> Result<&OwnedFd> {
        self.parent
            .as_ref()
            .ok_or(Error::State("missing cgroup parent custody"))
    }

    fn directory(&self) -> Result<&OwnedFd> {
        self.directory
            .as_ref()
            .ok_or(Error::State("missing cgroup directory custody"))
    }

    fn kill(&self) -> Result<&OwnedFd> {
        self.kill
            .as_ref()
            .ok_or(Error::State("missing cgroup.kill custody"))
    }

    fn events(&self) -> Result<&OwnedFd> {
        self.events
            .as_ref()
            .ok_or(Error::State("missing cgroup.events custody"))
    }
}

impl Drop for NativeCgroupDomainV1 {
    fn drop(&mut self) {
        if !self.phase.may_release() {
            // Fallback retention is not a cleanup owner or progress guarantee.
            // The caller must keep the complete record reachable in its slot.
            std::mem::forget(self.parent.take());
            std::mem::forget(self.directory.take());
            std::mem::forget(self.kill.take());
            std::mem::forget(self.events.take());
        }
    }
}

fn require_root() -> Result<()> {
    if crate::syscall::has_exact_root_identity() {
        Ok(())
    } else {
        Err(Error::State(
            "fresh cgroup mechanics require exact root identity",
        ))
    }
}

fn open_proc(path: &CStr) -> Result<OwnedFd> {
    let fd = fs::open(path, READ_FLAGS, Mode::empty()).map_err(|e| io("open proc record", e))?;
    validate_fs(&fd, fs::PROC_SUPER_MAGIC)?;
    validate_root_stat(
        &fs::fstat(&fd).map_err(|e| io("stat proc record", e))?,
        false,
    )?;
    Ok(fd)
}

fn open_relative(parent: &OwnedFd, name: &CStr, flags: OFlags) -> Result<OwnedFd> {
    fs::openat2(parent, name, flags, Mode::empty(), RESOLVE)
        .map_err(|e| io("open bounded cgroup component", e))
}

fn validate_fs(fd: &OwnedFd, expected: fs::FsWord) -> Result<()> {
    if fs::fstatfs(fd)
        .map_err(|e| io("inspect control filesystem", e))?
        .f_type
        != expected
    {
        return Err(Error::State("unexpected control filesystem"));
    }
    Ok(())
}

fn validate_root_stat(stat: &Stat, directory: bool) -> Result<()> {
    let kind = if directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    if stat.st_mode & libc::S_IFMT != kind
        || stat.st_uid != 0
        || stat.st_gid != 0
        || stat.st_mode & 0o022 != 0
        || stat.st_nlink == 0
    {
        return Err(Error::State(
            "control inode is not root-owned and protected",
        ));
    }
    Ok(())
}

fn same_directory(expected: Identity, actual: &Stat, named: &Stat) -> bool {
    Identity::of(actual) == expected
        && Identity::of(named) == expected
        && named.st_mode & libc::S_IFMT == libc::S_IFDIR
        && named.st_nlink != 0
}

fn validate_control(fd: &OwnedFd, directory: &OwnedFd, name: &CStr) -> Result<()> {
    validate_fs(fd, CGROUP2_MAGIC)?;
    let actual = fs::fstat(fd).map_err(|e| io("stat retained cgroup control", e))?;
    validate_root_stat(&actual, false)?;
    let named = fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| io("stat named cgroup control", e))?;
    if Identity::of(&actual) != Identity::of(&named)
        || named.st_mode & libc::S_IFMT != libc::S_IFREG
    {
        return Err(Error::State("cgroup control pathname or identity changed"));
    }
    Ok(())
}

// Two finite reads: a full fixed frame and one EOF probe. A short read is not
// mistaken for a complete record; no read-to-end, EINTR loop or growing buffer.
fn read_record(fd: &OwnedFd, buffer: &mut [u8; READ_LIMIT]) -> Result<usize> {
    let count =
        rustix::io::pread(fd, &mut buffer[..], 0).map_err(|e| io("read control record", e))?;
    if count == 0 || count == buffer.len() {
        return Err(Error::State("control record is empty or exceeds its bound"));
    }
    let mut tail = [0; 1];
    if rustix::io::pread(fd, &mut tail[..], count as u64)
        .map_err(|e| io("check control record EOF", e))?
        != 0
    {
        return Err(Error::State(
            "control record was not complete in one bounded read",
        ));
    }
    Ok(count)
}

fn require_membership(parent: &OwnedFd, pid: rustix::process::Pid) -> Result<()> {
    let procs = open_relative(parent, c"cgroup.procs", READ_FLAGS)?;
    validate_control(&procs, parent, c"cgroup.procs")?;
    let mut bytes = [0; READ_LIMIT];
    let count = read_record(&procs, &mut bytes)?;
    let lines = record_lines(&bytes[..count])?;
    let mut found = false;
    for line in lines.split(|byte| *byte == b'\n') {
        let value = decimal(line)?;
        if value > i32::MAX as u32 {
            return Err(Error::State("invalid cgroup process identity"));
        }
        found |= value == pid.as_raw_pid() as u32;
    }
    if !found {
        return Err(Error::State(
            "resolved cgroup does not contain this process",
        ));
    }
    Ok(())
}

fn record_lines(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.is_empty() || bytes.len() >= READ_LIMIT {
        return Err(Error::State("control record length is invalid"));
    }
    bytes
        .strip_suffix(b"\n")
        .filter(|lines| !lines.is_empty())
        .ok_or(Error::State("control record lacks its final newline"))
}

fn decimal(bytes: &[u8]) -> Result<u32> {
    if bytes.is_empty() || bytes.len() > 10 || (bytes.len() > 1 && bytes[0] == b'0') {
        return Err(Error::State("invalid control decimal"));
    }
    bytes.iter().try_fold(0_u32, |value, byte| {
        if !byte.is_ascii_digit() {
            return Err(Error::State("invalid control decimal"));
        }
        value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u32::from(byte - b'0')))
            .ok_or(Error::State("control decimal overflow"))
    })
}

fn membership_path(bytes: &[u8]) -> Result<&[u8]> {
    let mut path = None;
    for line in record_lines(bytes)?.split(|byte| *byte == b'\n') {
        let mut fields = line.splitn(3, |byte| *byte == b':');
        let hierarchy = decimal(fields.next().ok_or(Error::State("missing hierarchy"))?)?;
        let controllers = fields.next().ok_or(Error::State("missing controllers"))?;
        let candidate = fields
            .next()
            .ok_or(Error::State("missing membership path"))?;
        validate_absolute_path(candidate)?;
        if hierarchy == 0 {
            if !controllers.is_empty() || path.replace(candidate).is_some() {
                return Err(Error::State("ambiguous unified cgroup membership"));
            }
        } else if controllers.is_empty()
            || !controllers
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"_=,-".contains(b))
        {
            return Err(Error::State("invalid legacy cgroup controllers"));
        }
    }
    path.ok_or(Error::State("unified cgroup membership is missing"))
}

fn validate_absolute_path(path: &[u8]) -> Result<()> {
    if !path.starts_with(b"/")
        || path.len() >= READ_LIMIT
        || path.ends_with(b" (deleted)")
        || path.iter().any(|b| *b == 0 || b.is_ascii_control())
    {
        return Err(Error::State("invalid absolute cgroup path"));
    }
    if path != b"/"
        && path[1..]
            .split(|b| *b == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b".." || part.len() > 255)
    {
        return Err(Error::State("cgroup path has an unsupported component"));
    }
    Ok(())
}

fn relative_path<'a>(path: &[u8], buffer: &'a mut [u8; READ_LIMIT]) -> Result<&'a CStr> {
    validate_absolute_path(path)?;
    let relative = if path == b"/" { &b"."[..] } else { &path[1..] };
    buffer[..relative.len()].copy_from_slice(relative);
    buffer[relative.len()] = 0;
    CStr::from_bytes_with_nul(&buffer[..=relative.len()])
        .map_err(|_| Error::State("invalid relative cgroup path"))
}

fn generated_name(uuid: &[u8]) -> Result<[u8; NAME_BYTES]> {
    if uuid.len() != 37 || uuid[36] != b'\n' {
        return Err(Error::State("kernel UUID has an invalid length"));
    }
    let mut name = [0; NAME_BYTES];
    name[..NAME_PREFIX.len()].copy_from_slice(NAME_PREFIX);
    let mut target = NAME_PREFIX.len();
    for (index, byte) in uuid[..36].iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return Err(Error::State("kernel UUID has an invalid separator"));
            }
        } else {
            if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte) {
                return Err(Error::State("kernel UUID has an invalid digit"));
            }
            name[target] = *byte;
            target += 1;
        }
    }
    Ok(name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Events {
    populated: bool,
    frozen: bool,
}

fn parse_events(bytes: &[u8]) -> Result<Events> {
    let mut populated = None;
    let mut frozen = None;
    for line in record_lines(bytes)?.split(|byte| *byte == b'\n') {
        let mut fields = line.splitn(2, |byte| *byte == b' ');
        let key = fields
            .next()
            .ok_or(Error::State("missing cgroup.events key"))?;
        let value = fields
            .next()
            .ok_or(Error::State("missing cgroup.events value"))?;
        let value = match value {
            b"0" => false,
            b"1" => true,
            _ => return Err(Error::State("invalid cgroup.events value")),
        };
        let slot = match key {
            b"populated" => &mut populated,
            b"frozen" => &mut frozen,
            _ => return Err(Error::State("unknown cgroup.events key")),
        };
        if slot.replace(value).is_some() {
            return Err(Error::State("duplicate cgroup.events key"));
        }
    }
    Ok(Events {
        populated: populated.ok_or(Error::State("missing populated event"))?,
        frozen: frozen.ok_or(Error::State("missing frozen event"))?,
    })
}

#[cfg(test)]
#[path = "native_cgroup_tests.rs"]
mod tests;
