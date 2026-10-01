//! Exact calling-child proc observation across its credential transition.
//! No supplied descriptor, parent snapshot, syscall-denial fallback or mutation.
//! This is pre-exec state only; an ELF exec may establish different personality.

use std::mem::size_of;

const PROC_MAGIC: u64 = 0x9fa0;
const PATH_BYTES: usize = 32;
const RECORD_BYTES: usize = 10;
const READ_IMPLIES_EXEC: u32 = 0x0040_0000;
const RESOLVE_NO_XDEV: u64 = 1;
const RESOLVE_NO_SYMLINKS: u64 = 4;
const RESOLVE_BENEATH: u64 = 8;
const READ_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

// Four identity calls, three opens, three filesystem and three inode reads,
// one readlink, two record reads and all three closes, including rollback.
pub(crate) const WORK: usize = 19 * (1024 + 64) + (PATH_BYTES + RECORD_BYTES + 1) * 64 + 256;
// Includes returned metadata copies, nested inspection/close failure frames and
// both acquisition/consumption frames, not compiler-generated stack or RSS.
pub(crate) const SCRATCH: usize = 6 * size_of::<libc::stat>()
    + 2 * size_of::<libc::statfs>()
    + 2 * size_of::<OpenHow>()
    + 2 * size_of::<Observation>()
    + 2 * PATH_BYTES
    + 2 * RECORD_BYTES
    + 1024;

// No Drop, allocation or destructor may run after raw clone. Every production
// path explicitly consumes/closes this private owner before READY or failure.
#[must_use]
pub(super) struct Observation {
    fd: libc::c_int,
    pid: libc::c_int,
    tid: libc::c_int,
}

impl Observation {
    /// Actual child only, after mapping and before credential/dumpability changes.
    /// Caller prepays WORK/SCRATCH before clone and excludes foreign FD/mount mutation.
    pub(super) unsafe fn acquire() -> Option<Self> {
        let mut root = -1;
        let mut task = -1;
        let mut file = -1;
        // SAFETY: only fixed native ABI syscalls and initialized stack records.
        // The kernel's thread-self link names this calling task in this procfs
        // view. Its canonical coordinates must also match actual getpid/gettid.
        let (pid, tid, valid) = unsafe {
            let pid = libc::syscall(libc::SYS_getpid);
            let tid = libc::syscall(libc::SYS_gettid);
            let valid = (|| {
                if pid <= 0 || pid > i32::MAX as libc::c_long || tid != pid {
                    return false;
                }
                root = open(libc::AT_FDCWD, c"/proc".as_ptr(), true, RESOLVE_NO_SYMLINKS);
                let Some(root_stat) = inspect(root, true) else {
                    return false;
                };
                let mut path = [0_u8; PATH_BYTES];
                let count = libc::syscall(
                    libc::SYS_readlinkat,
                    root,
                    c"thread-self".as_ptr(),
                    path.as_mut_ptr(),
                    PATH_BYTES,
                );
                if count <= 0
                    || count as usize >= PATH_BYTES
                    || !exact_task_path(&path[..count as usize], pid as i32, tid as i32)
                {
                    return false;
                }
                // The relative numeric path has no symlink or mount exception.
                task = open(
                    root,
                    path.as_ptr().cast(),
                    true,
                    RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV,
                );
                let Some(task_stat) = inspect(task, true) else {
                    return false;
                };
                if task_stat.st_dev != root_stat.st_dev {
                    return false;
                }
                file = open(
                    task,
                    c"personality".as_ptr(),
                    false,
                    RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV,
                );
                let Some(file_stat) = inspect(file, false) else {
                    return false;
                };
                file_stat.st_dev == root_stat.st_dev
            })();
            (pid as i32, tid as i32, valid)
        };
        // SAFETY: private descriptors only. Both closes execute, without retry
        // after EINTR on Linux, even when the first reports an error.
        let (task_closed, root_closed) = unsafe { (close(&mut task), close(&mut root)) };
        if !valid || !task_closed || !root_closed {
            // SAFETY: no owner escaped; retire the remaining private descriptor.
            unsafe { close(&mut file) };
            return None;
        }
        Some(Self { fd: file, pid, tid })
    }

    /// Read the pinned actual-child inode AFTER profile transition, then close.
    /// A read, identity, format or close failure is never a successful observation.
    pub(super) unsafe fn read_and_close(mut self) -> Option<u32> {
        // SAFETY: the private constructor retained this exact child FD; the
        // direct child has not forked or exported/reassigned it. Proc's retained
        // PID inode identifies the task, not a later path lookup or reused PID.
        let value = unsafe {
            let pid = libc::syscall(libc::SYS_getpid);
            let tid = libc::syscall(libc::SYS_gettid);
            if pid != libc::c_long::from(self.pid) || tid != libc::c_long::from(self.tid) {
                None
            } else {
                let mut bytes = [0_u8; RECORD_BYTES];
                let count = libc::syscall(
                    libc::SYS_pread64,
                    self.fd,
                    bytes.as_mut_ptr(),
                    RECORD_BYTES,
                    0_u64,
                );
                if count != 9 {
                    None
                } else {
                    let mut tail = 0_u8;
                    if libc::syscall(libc::SYS_pread64, self.fd, &raw mut tail, 1_usize, 9_u64) != 0
                    {
                        None
                    } else {
                        parse(&bytes[..9])
                    }
                }
            }
        };
        // SAFETY: consume the private descriptor even after a failed read/check.
        let closed = unsafe { close(&mut self.fd) };
        if closed { value } else { None }
    }

    /// Terminal pre-observation failure; caller exits regardless of close result.
    pub(super) unsafe fn abort(mut self) {
        // SAFETY: same unexported child-local FD, consumed exactly once.
        unsafe { close(&mut self.fd) };
    }
}

unsafe fn open(
    parent: libc::c_int,
    path: *const libc::c_char,
    directory: bool,
    resolve: u64,
) -> libc::c_int {
    let how = OpenHow {
        flags: (READ_FLAGS | if directory { libc::O_DIRECTORY } else { 0 }) as u64,
        mode: 0,
        resolve,
    };
    // SAFETY: caller supplies a live bounded NUL-terminated path; fixed ABI header.
    unsafe {
        libc::syscall(
            libc::SYS_openat2,
            parent,
            path,
            &raw const how,
            size_of::<OpenHow>(),
        ) as libc::c_int
    }
}

unsafe fn inspect(fd: libc::c_int, directory: bool) -> Option<libc::stat> {
    if fd < 0 {
        return None;
    }
    // SAFETY: initialized all-scalar ABI storage, filled before it is inspected.
    unsafe {
        let mut fs: libc::statfs = std::mem::zeroed();
        let mut stat: libc::stat = std::mem::zeroed();
        if libc::syscall(libc::SYS_fstatfs, fd, &raw mut fs) != 0
            || !is_procfs_type(fs.f_type)
            || libc::syscall(libc::SYS_fstat, fd, &raw mut stat) != 0
            || stat.st_mode & libc::S_IFMT
                != if directory {
                    libc::S_IFDIR
                } else {
                    libc::S_IFREG
                }
            || stat.st_uid != 0
            || stat.st_gid != 0
            || stat.st_mode & 0o022 != 0
            || stat.st_nlink == 0
        {
            None
        } else {
            Some(stat)
        }
    }
}

fn is_procfs_type(value: impl TryInto<u64>) -> bool {
    // Normalize libc's signed/unsigned statfs ABI without truncating or wrapping.
    matches!(value.try_into(), Ok(PROC_MAGIC))
}

unsafe fn close(fd: &mut libc::c_int) -> bool {
    let original = *fd;
    *fd = -1;
    // SAFETY: consumed private FD; no retry after a possibly completed close.
    original < 0 || unsafe { libc::syscall(libc::SYS_close, original) } == 0
}

fn exact_task_path(bytes: &[u8], pid: i32, tid: i32) -> bool {
    let mut parts = bytes.split(|b| *b == b'/');
    positive_decimal(parts.next().unwrap_or_default()) == Some(pid)
        && parts.next() == Some(&b"task"[..])
        && positive_decimal(parts.next().unwrap_or_default()) == Some(tid)
        && parts.next().is_none()
}

fn positive_decimal(bytes: &[u8]) -> Option<i32> {
    if bytes.is_empty() || bytes.len() > 10 || bytes[0] == b'0' {
        return None;
    }
    bytes.iter().try_fold(0_i32, |n, b| {
        if !b.is_ascii_digit() {
            return None;
        }
        n.checked_mul(10)?.checked_add(i32::from(b - b'0'))
    })
}

fn parse(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 9 || bytes[8] != b'\n' {
        return None;
    }
    let value = bytes[..8].iter().try_fold(0_u32, |n, b| {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            _ => return None,
        };
        Some((n << 4) | u32::from(digit))
    })?;
    (value & READ_IMPLIES_EXEC == 0).then_some(value)
}

#[cfg(test)]
#[path = "native_compiler_personality_tests.rs"]
mod tests;
