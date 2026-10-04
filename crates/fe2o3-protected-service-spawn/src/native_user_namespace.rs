//! Concrete root-parent user-namespace mapping, not administrator admission.
//!
//! Before configure, the caller must publish this entire owner with its actual
//! CLONE_NEWUSER child, pidfd and exclusive consuming-wait custody. The child
//! stays behind the mapping gate, without profile changes or namespace/map
//! mutation, until configuration succeeds. Exclude competing privileged procfs,
//! credential, descriptor and namespace writers. Numeric root, nsfs metadata and
//! namespace ancestry do not establish the caller's administrator provenance.
//! Every call is prepaid by the caller. Bounds are logical, not syscall latency
//! or kernel-memory bounds. Errors retain captured handles; no retry, stale
//! adoption, consuming wait, setns, setgroups write or cleanup-in-Drop exists.

use crate::native_spawn::{ProtectedServiceSpawnErrorV2 as Error, Result, io};
use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
use rustix::fs::{self, Mode, OFlags, Stat};
use rustix::io::{Errno, FdFlags};
use rustix::process::{Pid, WaitId, WaitIdOptions};
use std::ffi::CStr;
use std::mem::size_of;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

const READ_BYTES: usize = 4096;
const MAP_BYTES: usize = 3 * 24;
const FD_COUNT: usize = 13;
const OPERATION_WORK: usize = 1024 + 64;
// Temporary descriptors are separate from the thirteen retained obligations.
// Includes nested current-task/fdinfo opens and close-only failure rollback.
const TEMP_FD_SCRATCH: usize = 32 * (size_of::<OwnedFd>() + size_of::<usize>());
const CONTROL_SCRATCH: usize = 4096;
const _: () = assert!(
    TEMP_FD_SCRATCH
        + 8 * size_of::<Error>()
        + 64 * size_of::<usize>()
        + 4 * size_of::<fs::StatFs>()
        + 4 * size_of::<rustix::process::WaitIdStatus>()
        + MAP_BYTES
        + 6 * 11
        <= CONTROL_SCRATCH
);
const NSFS_MAGIC: fs::FsWord = 0x6e73_6673;
const NS_GET_PARENT: u32 = 0xb702;
const NS_GET_NSTYPE: u32 = 0xb703;
const NS_GET_OWNER_UID: rustix::ioctl::Opcode = 0xb704;
const NAMESPACES: [(&CStr, i32); 5] = [
    (c"ns/user", libc::CLONE_NEWUSER),
    (c"ns/pid", libc::CLONE_NEWPID),
    (c"ns/pid_for_children", libc::CLONE_NEWPID),
    (c"ns/time", libc::CLONE_NEWTIME),
    (c"ns/time_for_children", libc::CLONE_NEWTIME),
];
const READ: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NONBLOCK);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    Configuring,
    Configured,
    Refused,
}

impl Phase {
    fn begin(&mut self) -> Result<()> {
        if *self != Self::Prepared {
            return Err(Error::State("user namespace configuration is single-use"));
        }
        *self = Self::Configuring;
        Ok(())
    }

    fn require_configured(self) -> Result<()> {
        if self == Self::Configured {
            Ok(())
        } else {
            Err(Error::State("user namespace configuration is incomplete"))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
}

fn identity(fd: &OwnedFd) -> Result<Identity> {
    let stat = fs::fstat(fd).map_err(|e| io("stat retained namespace object", e))?;
    Ok(Identity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

struct Namespace {
    fd: OwnedFd,
    identity: Identity,
}

struct Child {
    pid: Pid,
    pidfd: Option<OwnedFd>,
    proc: Option<OwnedFd>,
    user: Option<OwnedFd>,
    parent_user: Option<OwnedFd>,
    uid_map: Option<OwnedFd>,
    gid_map: Option<OwnedFd>,
    setgroups: Option<OwnedFd>,
}

impl Child {
    fn new(pid: Pid) -> Self {
        Self {
            pid,
            pidfd: None,
            proc: None,
            user: None,
            parent_user: None,
            uid_map: None,
            gid_map: None,
            setgroups: None,
        }
    }
}

/// Parent context and partial mapping custody, with no public construction path.
/// Closing descriptors on eventual Drop does not terminate or clean up a child.
#[must_use]
pub(crate) struct NativeUserNamespaceV1 {
    origin: Pid,
    proc_root: OwnedFd,
    parent: [Namespace; 5],
    uid_map: IdentityMap,
    gid_map: IdentityMap,
    phase: Phase,
    child: Option<Child>,
}

impl NativeUserNamespaceV1 {
    /// Fixed owner plus all thirteen logical descriptor-retention obligations.
    pub(crate) const STORAGE: usize = size_of::<Self>() + FD_COUNT * size_of::<usize>();
    /// Prepay before observing the parent and preparing the exact identity maps.
    pub(crate) const PREPARE_WORK: usize = 256 * OPERATION_WORK + 64 * READ_BYTES;
    /// Fixed frames, read buffers, metadata and temporary namespace handles.
    pub(crate) const PREPARE_SCRATCH: usize =
        4 * size_of::<Self>() + 4 * READ_BYTES + 16 * size_of::<Stat>() + CONTROL_SCRATCH;
    /// Includes partial failure, two single writes, readback and final validation.
    pub(crate) const CONFIGURE_WORK: usize = 1024 * OPERATION_WORK + 256 * READ_BYTES;
    /// No owned heap or growing read/write buffers are created by configuration.
    pub(crate) const CONFIGURE_SCRATCH: usize = Self::PREPARE_SCRATCH;
    /// One finite parent/child, pidfd, namespace and map revalidation.
    pub(crate) const REVALIDATE_WORK: usize = 512 * OPERATION_WORK + 128 * READ_BYTES;
    /// Revalidation borrows retained state and uses fixed temporary frames only.
    pub(crate) const REVALIDATE_SCRATCH: usize = Self::PREPARE_SCRATCH;

    /// Captures the exact current root context before clone, without mutations.
    pub(crate) fn prepare(helper: Credentials, peer: Credentials) -> Result<Self> {
        require_root()?;
        let origin = rustix::process::getpid();
        let proc_root = fs::open(
            c"/proc",
            READ | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|e| io("open actual procfs root", e))?;
        require_proc(&proc_root, true)?;
        let task = current_task(&proc_root, origin)?;
        let parent = [
            capture_namespace(&task, 0)?,
            capture_namespace(&task, 1)?,
            capture_namespace(&task, 2)?,
            capture_namespace(&task, 3)?,
            capture_namespace(&task, 4)?,
        ];
        if parent[1].identity != parent[2].identity || parent[3].identity != parent[4].identity {
            return Err(Error::State(
                "parent has a pending PID or time namespace transition",
            ));
        }
        let owner = Self {
            origin,
            proc_root,
            parent,
            uid_map: IdentityMap::new(helper.uid(), peer.uid())?,
            gid_map: IdentityMap::new(helper.gid(), peer.gid())?,
            phase: Phase::Prepared,
            child: None,
        };
        owner.require_parent()?;
        Ok(owner)
    }

    /// Requires the caller's prepublished exact gated CLONE_NEWUSER child.
    /// Every captured descriptor and partial map remains owned after refusal.
    pub(crate) fn configure_child(&mut self, pid: Pid, pidfd: &OwnedFd) -> Result<()> {
        self.phase.begin()?;
        self.child = Some(Child::new(pid));
        let result = self.configure_once(pid, pidfd);
        self.phase = if result.is_ok() {
            Phase::Configured
        } else {
            Phase::Refused
        };
        result
    }

    /// Revalidates the same live, still-gated child before profile installation,
    /// without changing maps or consuming waits.
    pub(crate) fn revalidate_child(&self, pid: Pid, pidfd: &OwnedFd) -> Result<()> {
        self.phase.require_configured()?;
        self.verify_child(pid, pidfd)
    }

    fn configure_once(&mut self, pid: Pid, pidfd: &OwnedFd) -> Result<()> {
        self.require_parent()?;
        self.child_mut()?.pidfd = Some(
            rustix::io::fcntl_dupfd_cloexec(pidfd, 0)
                .map_err(|e| io("retain exact namespace child pidfd", e))?,
        );
        self.require_pidfd(pid, pidfd)?;
        let mut name = [0; 11];
        let proc = open_proc_at(
            &self.proc_root,
            number_name(pid.as_raw_pid() as u32, &mut name)?,
            READ | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            true,
        )?;
        self.child_mut()?.proc = Some(proc);
        let user = open_namespace(required(&self.child()?.proc)?, 0)?;
        self.child_mut()?.user = Some(user);
        namespace_identity(required(&self.child()?.user)?, libc::CLONE_NEWUSER)?;
        // Store the kernel-returned parent FD before any metadata check can fail.
        let parent = namespace_parent(required(&self.child()?.user)?)?;
        self.child_mut()?.parent_user = Some(parent);
        self.require_child_namespaces()?;
        let uid_map = open_proc_at(
            required(&self.child()?.proc)?,
            c"uid_map",
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOFOLLOW,
            false,
        )?;
        self.child_mut()?.uid_map = Some(uid_map);
        let gid_map = open_proc_at(
            required(&self.child()?.proc)?,
            c"gid_map",
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOFOLLOW,
            false,
        )?;
        self.child_mut()?.gid_map = Some(gid_map);
        let setgroups = open_proc_at(
            required(&self.child()?.proc)?,
            c"setgroups",
            READ | OFlags::NOFOLLOW,
            false,
        )?;
        self.child_mut()?.setgroups = Some(setgroups);
        let child = self.child()?;
        let mut bytes = [0; READ_BYTES];
        if read_record(required(&child.uid_map)?, &mut bytes)? != 0
            || read_record(required(&child.gid_map)?, &mut bytes)? != 0
        {
            return Err(Error::State("new child already has an ID map"));
        }
        require_setgroups_allow(required(&child.setgroups)?)?;
        self.require_pidfd(pid, pidfd)?;
        self.gid_map.write_once(required(&child.gid_map)?)?;
        self.uid_map.write_once(required(&child.uid_map)?)?;
        self.verify_child(pid, pidfd)
    }

    fn verify_child(&self, pid: Pid, pidfd: &OwnedFd) -> Result<()> {
        self.require_parent()?;
        self.require_pidfd(pid, pidfd)?;
        let child = self.child()?;
        let mut name = [0; 11];
        let current = open_proc_at(
            &self.proc_root,
            number_name(pid.as_raw_pid() as u32, &mut name)?,
            READ | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            true,
        )?;
        if identity(&current)? != identity(required(&child.proc)?)? {
            return Err(Error::State("child proc directory identity changed"));
        }
        self.require_child_namespaces()?;
        for (name, retained) in [
            (c"uid_map", &child.uid_map),
            (c"gid_map", &child.gid_map),
            (c"setgroups", &child.setgroups),
        ] {
            let actual = open_proc_at(&current, name, READ | OFlags::NOFOLLOW, false)?;
            if identity(&actual)? != identity(required(retained)?)? {
                return Err(Error::State("child mapping control identity changed"));
            }
        }
        let mut bytes = [0; READ_BYTES];
        let count = read_record(required(&child.uid_map)?, &mut bytes)?;
        self.uid_map.require_record(&bytes[..count])?;
        let count = read_record(required(&child.gid_map)?, &mut bytes)?;
        self.gid_map.require_record(&bytes[..count])?;
        require_setgroups_allow(required(&child.setgroups)?)?;
        self.require_parent()?;
        self.require_pidfd(pid, pidfd)
    }

    fn require_parent(&self) -> Result<()> {
        require_root()?;
        if rustix::process::getpid() != self.origin {
            return Err(Error::State("namespace owner belongs to another process"));
        }
        let task = current_task(&self.proc_root, self.origin)?;
        for (index, retained) in self.parent.iter().enumerate() {
            let actual = capture_namespace(&task, index)?;
            if actual.identity != retained.identity
                || namespace_identity(&retained.fd, NAMESPACES[index].1)? != retained.identity
            {
                return Err(Error::State("root parent's namespace context changed"));
            }
        }
        Ok(())
    }

    fn require_pidfd(&self, pid: Pid, supplied: &OwnedFd) -> Result<()> {
        let child = self.child()?;
        if child.pid != pid {
            return Err(Error::State("namespace child PID changed"));
        }
        let retained = required(&child.pidfd)?;
        if identity(retained)? != identity(supplied)? {
            return Err(Error::State("namespace child pidfd identity changed"));
        }
        // Old kernels share anon-inode metadata across pidfds. The real fdinfo
        // target and nonconsuming child wait are required, not metadata alone.
        for fd in [retained, supplied] {
            if !rustix::io::fcntl_getfd(fd)
                .map_err(|e| io("check namespace pidfd flags", e))?
                .contains(FdFlags::CLOEXEC)
            {
                return Err(Error::State("namespace pidfd is not close-on-exec"));
            }
            let task = current_task(&self.proc_root, self.origin)?;
            let fdinfo = open_proc_at(
                &task,
                c"fdinfo",
                READ | OFlags::DIRECTORY | OFlags::NOFOLLOW,
                true,
            )?;
            let mut name = [0; 11];
            let number =
                u32::try_from(fd.as_raw_fd()).map_err(|_| Error::State("invalid pidfd number"))?;
            let record = open_proc_at(
                &fdinfo,
                number_name(number, &mut name)?,
                READ | OFlags::NOFOLLOW,
                false,
            )?;
            let mut bytes = [0; READ_BYTES];
            let count = read_record(&record, &mut bytes)?;
            require_pidfd_record(&bytes[..count], pid.as_raw_pid() as u32)?;
            match rustix::process::waitid(
                WaitId::PidFd(fd.as_fd()),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            )
            .map_err(|e| io("observe owned namespace child without reaping", e))?
            {
                None => {}
                Some(_) => return Err(Error::State("namespace child is no longer live")),
            }
        }
        Ok(())
    }

    fn require_child_namespaces(&self) -> Result<()> {
        let child = self.child()?;
        let user = required(&child.user)?;
        let actual_user = namespace_identity(user, libc::CLONE_NEWUSER)?;
        if actual_user == self.parent[0].identity {
            return Err(Error::State("child did not enter a new user namespace"));
        }
        let current_user = capture_namespace(required(&child.proc)?, 0)?;
        if current_user.identity != actual_user {
            return Err(Error::State("child user namespace changed"));
        }
        let parent = required(&child.parent_user)?;
        if namespace_identity(parent, libc::CLONE_NEWUSER)? != self.parent[0].identity {
            return Err(Error::State("child namespace has a different parent"));
        }
        let current_parent = namespace_parent(user)?;
        if namespace_identity(&current_parent, libc::CLONE_NEWUSER)? != self.parent[0].identity
            || namespace_owner_uid(user)? != 0
        {
            return Err(Error::State(
                "child namespace ancestry or root owner changed",
            ));
        }
        for index in 1..NAMESPACES.len() {
            let actual = capture_namespace(required(&child.proc)?, index)?;
            if actual.identity != self.parent[index].identity {
                return Err(Error::State(
                    "child PID or time namespace differs from parent",
                ));
            }
        }
        Ok(())
    }

    fn child(&self) -> Result<&Child> {
        self.child
            .as_ref()
            .ok_or(Error::State("namespace child custody is absent"))
    }

    fn child_mut(&mut self) -> Result<&mut Child> {
        self.child
            .as_mut()
            .ok_or(Error::State("namespace child custody is absent"))
    }
}

fn required(fd: &Option<OwnedFd>) -> Result<&OwnedFd> {
    fd.as_ref()
        .ok_or(Error::State("partial namespace descriptor custody"))
}

fn require_root() -> Result<()> {
    if crate::syscall::has_exact_root_identity() {
        Ok(())
    } else {
        Err(Error::State(
            "namespace mapping requires exact root parent IDs",
        ))
    }
}

fn open_proc_at(parent: &OwnedFd, name: &CStr, flags: OFlags, directory: bool) -> Result<OwnedFd> {
    let fd = fs::openat(parent, name, flags, Mode::empty())
        .map_err(|e| io("open anchored proc object", e))?;
    require_proc(&fd, directory)?;
    Ok(fd)
}

fn require_proc(fd: &OwnedFd, directory: bool) -> Result<()> {
    let stat = fs::fstat(fd).map_err(|e| io("stat proc object", e))?;
    let magic = fs::fstatfs(fd)
        .map_err(|e| io("inspect proc filesystem", e))?
        .f_type;
    let kind = if directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    if magic != fs::PROC_SUPER_MAGIC || stat.st_mode & libc::S_IFMT != kind || stat.st_ino == 0 {
        return Err(Error::State(
            "mapping control is not the expected procfs object",
        ));
    }
    Ok(())
}

fn current_task(proc_root: &OwnedFd, origin: Pid) -> Result<OwnedFd> {
    let self_dir = open_proc_at(proc_root, c"self", READ | OFlags::DIRECTORY, true)?;
    let mut name = [0; 11];
    let numeric = open_proc_at(
        proc_root,
        number_name(origin.as_raw_pid() as u32, &mut name)?,
        READ | OFlags::DIRECTORY | OFlags::NOFOLLOW,
        true,
    )?;
    if identity(&self_dir)? != identity(&numeric)? {
        return Err(Error::State(
            "procfs PID view does not match the actual parent",
        ));
    }
    // Unlike /proc/self/ns, this observes the invoking thread's namespace state.
    open_proc_at(proc_root, c"thread-self", READ | OFlags::DIRECTORY, true)
}

fn open_namespace(proc: &OwnedFd, index: usize) -> Result<OwnedFd> {
    // These fixed procfs magic links intentionally cross into nsfs. The returned
    // descriptor is inspected by filesystem, type and namespace ioctl, not text.
    fs::openat(proc, NAMESPACES[index].0, READ, Mode::empty())
        .map_err(|e| io("open actual namespace handle", e))
}

fn capture_namespace(proc: &OwnedFd, index: usize) -> Result<Namespace> {
    let fd = open_namespace(proc, index)?;
    let identity = namespace_identity(&fd, NAMESPACES[index].1)?;
    Ok(Namespace { fd, identity })
}

fn namespace_identity(fd: &OwnedFd, kind: i32) -> Result<Identity> {
    let stat = fs::fstat(fd).map_err(|e| io("stat nsfs handle", e))?;
    if fs::fstatfs(fd).map_err(|e| io("inspect nsfs", e))?.f_type != NSFS_MAGIC
        || stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_mode & 0o222 != 0
        || stat.st_ino == 0
        || namespace_type(fd)? != kind
        || !rustix::io::fcntl_getfd(fd)
            .map_err(|e| io("inspect namespace FD flags", e))?
            .contains(FdFlags::CLOEXEC)
    {
        return Err(Error::State("namespace FD metadata or type is invalid"));
    }
    Ok(Identity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn last_errno() -> Errno {
    std::io::Error::last_os_error()
        .raw_os_error()
        .map_or(Errno::IO, Errno::from_raw_os_error)
}

fn namespace_type(fd: &OwnedFd) -> Result<i32> {
    // SAFETY: NS_GET_NSTYPE has no pointer argument and returns its scalar type.
    let value = unsafe { libc::ioctl(fd.as_raw_fd(), NS_GET_NSTYPE as _, 0_usize) };
    if value < 0 {
        Err(io("NS_GET_NSTYPE", last_errno()))
    } else {
        Ok(value)
    }
}

fn namespace_parent(fd: &OwnedFd) -> Result<OwnedFd> {
    // SAFETY: NS_GET_PARENT has no pointer argument. Success installs one fresh
    // CLOEXEC descriptor, adopted exactly once; failure installs no descriptor.
    unsafe {
        let raw = libc::ioctl(fd.as_raw_fd(), NS_GET_PARENT as _, 0_usize);
        if raw < 0 {
            Err(io("NS_GET_PARENT", last_errno()))
        } else {
            Ok(OwnedFd::from_raw_fd(raw))
        }
    }
}

fn namespace_owner_uid(fd: &OwnedFd) -> Result<u32> {
    // SAFETY: nsfs NS_GET_OWNER_UID writes exactly one Linux uid_t (u32). The
    // retained root context is the parent namespace in which it is interpreted.
    unsafe { rustix::ioctl::ioctl(fd, rustix::ioctl::Getter::<NS_GET_OWNER_UID, u32>::new()) }
        .map_err(|e| io("NS_GET_OWNER_UID", e))
}

// At most two preads, each <=4096 bytes, never retrying EINTR or a short record.
// Empty is meaningful only to the caller checking an initially unwritten map.
fn read_record(fd: &OwnedFd, bytes: &mut [u8; READ_BYTES]) -> Result<usize> {
    let count =
        rustix::io::pread(fd, &mut bytes[..], 0).map_err(|e| io("read namespace control", e))?;
    if count == bytes.len() {
        return Err(Error::State("namespace control record exceeds its bound"));
    }
    let mut tail = [0; 1];
    if rustix::io::pread(fd, &mut tail[..], count as u64)
        .map_err(|e| io("check namespace control EOF", e))?
        != 0
    {
        return Err(Error::State("namespace control record is truncated"));
    }
    Ok(count)
}

fn require_setgroups_allow(fd: &OwnedFd) -> Result<()> {
    let mut bytes = [0; READ_BYTES];
    let count = read_record(fd, &mut bytes)?;
    if &bytes[..count] != b"allow\n" {
        return Err(Error::State(
            "child must retain setgroups allow for profile installation",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdentityMap {
    ids: [u32; 3],
    len: usize,
}

impl IdentityMap {
    fn new(helper: u32, peer: u32) -> Result<Self> {
        if [helper, peer].iter().any(|id| *id == 0 || *id == u32::MAX) {
            return Err(Error::State("invalid helper or peer identity map ID"));
        }
        let (first, second) = if helper <= peer {
            (helper, peer)
        } else {
            (peer, helper)
        };
        Ok(Self {
            ids: [0, first, if first == second { 0 } else { second }],
            len: if first == second { 2 } else { 3 },
        })
    }

    fn encode(&self, bytes: &mut [u8; MAP_BYTES]) -> Result<usize> {
        let mut len = 0;
        for id in &self.ids[..self.len] {
            let mut name = [0; 11];
            let value = number_name(*id, &mut name)?.to_bytes();
            for part in [value, &b" "[..], value, &b" 1\n"[..]] {
                let end = len + part.len();
                bytes
                    .get_mut(len..end)
                    .ok_or(Error::State("identity map encoding exceeds fixed frame"))?
                    .copy_from_slice(part);
                len = end;
            }
        }
        Ok(len)
    }

    fn write_once(&self, fd: &OwnedFd) -> Result<()> {
        let mut bytes = [0; MAP_BYTES];
        let count = self.encode(&mut bytes)?;
        // A newly opened map is at offset zero; pread validation does not move
        // that offset. proc ID-map controls reject pwrite with ESPIPE.
        if rustix::io::write(fd, &bytes[..count])
            .map_err(|e| io("install exact child identity map", e))?
            != count
        {
            return Err(Error::State("identity map write was not complete"));
        }
        Ok(())
    }

    fn require_record(&self, bytes: &[u8]) -> Result<()> {
        let lines = record_lines(bytes)?;
        let mut rows = 0;
        for line in lines.split(|b| *b == b'\n') {
            if rows >= self.len {
                return Err(Error::State("unexpected additional identity map row"));
            }
            let mut fields = line
                .split(|b| matches!(*b, b' ' | b'\t'))
                .filter(|f| !f.is_empty());
            let inside = decimal(
                fields
                    .next()
                    .ok_or(Error::State("missing inner identity"))?,
                10,
            )?;
            let outside = decimal(
                fields
                    .next()
                    .ok_or(Error::State("missing outer identity"))?,
                10,
            )?;
            let count = decimal(
                fields
                    .next()
                    .ok_or(Error::State("missing identity count"))?,
                10,
            )?;
            if inside != u64::from(self.ids[rows])
                || outside != inside
                || count != 1
                || fields.next().is_some()
            {
                return Err(Error::State(
                    "child identity map differs from exact prepared rows",
                ));
            }
            rows += 1;
        }
        if rows != self.len {
            return Err(Error::State("child identity map is incomplete"));
        }
        Ok(())
    }
}

fn number_name(mut value: u32, bytes: &mut [u8; 11]) -> Result<&CStr> {
    bytes[10] = 0;
    let mut start = 10;
    // At most ten digits for a u32; no allocation or arbitrary path input.
    for _ in 0..10 {
        start -= 1;
        bytes[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    CStr::from_bytes_with_nul(&bytes[start..])
        .map_err(|_| Error::State("invalid numeric proc component"))
}

fn record_lines(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.len() >= READ_BYTES {
        return Err(Error::State("namespace record exceeds fixed bound"));
    }
    bytes
        .strip_suffix(b"\n")
        .filter(|b| !b.is_empty())
        .ok_or(Error::State(
            "namespace record is empty or lacks final newline",
        ))
}

fn decimal(bytes: &[u8], radix: u64) -> Result<u64> {
    if bytes.is_empty()
        || bytes.len() > 22
        || !matches!(radix, 8 | 10)
        || (radix == 10 && bytes.len() > 1 && bytes[0] == b'0')
    {
        return Err(Error::State("noncanonical namespace number"));
    }
    bytes.iter().try_fold(0_u64, |value, byte| {
        if !byte.is_ascii_digit() || u64::from(byte - b'0') >= radix {
            return Err(Error::State("invalid namespace number"));
        }
        value
            .checked_mul(radix)
            .and_then(|n| n.checked_add(u64::from(byte - b'0')))
            .ok_or(Error::State("namespace number overflow"))
    })
}

fn require_pidfd_record(bytes: &[u8], expected: u32) -> Result<()> {
    let mut pid = None;
    let mut flags = None;
    for line in record_lines(bytes)?.split(|b| *b == b'\n') {
        let mut pair = line.splitn(2, |b| *b == b':');
        let key = pair.next().ok_or(Error::State("missing fdinfo key"))?;
        let value = pair.next().ok_or(Error::State("missing fdinfo value"))?;
        let mut words = value
            .split(|b| matches!(*b, b' ' | b'\t'))
            .filter(|p| !p.is_empty());
        let slot = match key {
            b"Pid" => &mut pid,
            b"flags" => &mut flags,
            _ => continue,
        };
        let value = decimal(
            words
                .next()
                .ok_or(Error::State("empty pidfd identity field"))?,
            if key == b"Pid" { 10 } else { 8 },
        )?;
        if words.next().is_some() || slot.replace(value).is_some() {
            return Err(Error::State("ambiguous pidfd identity field"));
        }
    }
    if expected == 0
        || expected > i32::MAX as u32
        || pid != Some(u64::from(expected))
        || flags.ok_or(Error::State("pidfd flags missing"))? & libc::O_EXCL as u64 != 0
    {
        return Err(Error::State(
            "pidfd does not identify the exact child process",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_user_namespace_tests.rs"]
mod tests;
