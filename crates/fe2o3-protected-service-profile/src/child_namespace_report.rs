//! Fixed inert observations from the trusted direct child before its exec gate.

use super::{Error, NAMESPACES, NamespaceIdentity, NamespaceSet, PROC_PATH_BYTES, io_error};
use rustix::{io::Errno, process::Pid};
use std::mem::{align_of, needs_drop, offset_of, size_of};

/// Fixed private namespace report; unrelated to service readiness framing.
/// All integers are little endian: magic at 0, u16 version at 8, u32 length
/// at 12, u32 child/parent PIDs at 16/20, u16 count at 24, then ten device/inode
/// u64 pairs at 32. Bytes 10..12 and 26..32 are zero; no extensions are accepted.
pub const CHILD_NAMESPACE_REPORT_BYTES: usize = 192;
const BYTES: usize = CHILD_NAMESPACE_REPORT_BYTES;
const HEADER_BYTES: usize = 32;
const MAGIC: &[u8; 8] = b"F2O3NSR1";
const ZERO_IDENTITY: NamespaceIdentity = NamespaceIdentity {
    device: 0,
    inode: 0,
};

/// Two PID observations and ten open/fstat/close triples, each charged 1024+64;
/// 32 units per wire byte and 256 cover staging, encoding and fixed checks.
/// Paths are constructed at compile time, not in the post-clone child.
pub const CHILD_NAMESPACE_REPORT_CAPTURE_WORK: usize =
    (3 * NAMESPACES.len() + 2) * (1024 + 64) + 32 * BYTES + 256;
/// Logical fixed child staging; not a generated-stack, allocator or RSS bound.
pub const CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH: usize =
    4 * BYTES + PROC_PATH_BYTES + 2 * size_of::<RawStat>() + 8 * size_of::<Error>() + 1024;
/// Fixed decode, independent reencode, exact comparison and error work.
pub const CHILD_NAMESPACE_REPORT_CHECK_WORK: usize = 32 * BYTES + 256;
/// Logical validation staging, excluding borrowed owner and input reservations.
pub const CHILD_NAMESPACE_REPORT_CHECK_SCRATCH: usize =
    4 * BYTES + 4 * size_of::<NamespaceSet>() + 8 * size_of::<Error>() + 1024;

// Linux x86_64 SYS_fstat writes 144 bytes, aligned to 8. Only the first two
// u64 fields are interpreted; the remaining kernel ABI bytes stay opaque.
#[repr(C)]
struct RawStat {
    device: u64,
    inode: u64,
    rest: [u64; 16],
}

impl RawStat {
    const ZERO: Self = Self {
        device: 0,
        inode: 0,
        rest: [0; 16],
    };
}

const _: () = {
    assert!(size_of::<usize>() == 8);
    assert!(size_of::<RawStat>() == 144);
    assert!(align_of::<RawStat>() == 8);
    assert!(offset_of!(RawStat, device) == 0);
    assert!(offset_of!(RawStat, inode) == 8);
    assert!(offset_of!(RawStat, rest) == 16);
    assert!(NAMESPACES.len() == 10);
    assert!(BYTES == HEADER_BYTES + NAMESPACES.len() * 16);
    assert!(!needs_drop::<RawStat>());
    assert!(!needs_drop::<NamespaceSet>());
    assert!(!needs_drop::<Error>());
};

// Derive the full static paths from the existing ordering table. No runtime
// path construction, decimal formatting, allocation, or C string scan is needed.
static CHILD_PATHS: [[u8; PROC_PATH_BYTES]; NAMESPACES.len()] = child_paths();
const fn child_paths() -> [[u8; PROC_PATH_BYTES]; NAMESPACES.len()] {
    let prefix = b"/proc/thread-self/";
    let mut paths = [[0; PROC_PATH_BYTES]; NAMESPACES.len()];
    let mut index = 0;
    while index < NAMESPACES.len() {
        let suffix = NAMESPACES[index].1.as_bytes();
        assert!(prefix.len() + suffix.len() < PROC_PATH_BYTES);
        let mut byte = 0;
        while byte < prefix.len() {
            paths[index][byte] = prefix[byte];
            byte += 1;
        }
        let mut byte = 0;
        while byte < suffix.len() {
            assert!(suffix[byte] != 0);
            paths[index][prefix.len() + byte] = suffix[byte];
            byte += 1;
        }
        index += 1;
    }
    paths
}

/// Observes actual calling-child PID, parent PID and all ten namespaces.
///
/// This is an unmetered mechanical helper: the parent must prepay CAPTURE_WORK
/// and CAPTURE_SCRATCH on its original ledger before clone. It performs only
/// fixed raw syscalls and scalar/byte operations, without allocation, libc/TLS,
/// locks, destructors, panic or unwind. Each syscall is attempted once; an open
/// fd is closed even after stat failure. Output is written only on success.
///
/// The caller must check the exact child profile first, provide private-pipe
/// provenance and EOF, and retain pidfd custody and the exec gate. These bytes
/// grant no authority and do not themselves establish any of those conditions.
pub fn capture_child_namespace_report_pre_exec(
    expected_parent_pid: i32,
    out: &mut [u8; CHILD_NAMESPACE_REPORT_BYTES],
) -> Result<(), Error> {
    capture_with(&mut RawSyscalls, expected_parent_pid, out)
}

// Private static dispatch permits deterministic fault injection without making
// arbitrary callbacks part of the production post-clone API.
trait ChildSyscalls {
    fn getpid(&mut self) -> Result<i32, Errno>;
    fn getppid(&mut self) -> Result<i32, Errno>;
    fn open(&mut self, path: &[u8; PROC_PATH_BYTES]) -> Result<i32, Errno>;
    fn stat(&mut self, fd: i32, out: &mut RawStat) -> Result<(), Errno>;
    fn close(&mut self, fd: i32) -> Result<(), Errno>;
}

fn capture_with(
    calls: &mut impl ChildSyscalls,
    expected_parent_pid: i32,
    out: &mut [u8; BYTES],
) -> Result<(), Error> {
    if expected_parent_pid <= 0 {
        return Err(Error::InvalidState(
            "invalid expected namespace-report parent PID",
        ));
    }
    let child = calls
        .getpid()
        .map_err(|e| io_error("observe child namespace PID", e))?;
    if child <= 0 {
        return Err(Error::InvalidState("invalid namespace-report child PID"));
    }
    let parent = calls
        .getppid()
        .map_err(|e| io_error("observe child namespace parent PID", e))?;
    if parent != expected_parent_pid {
        return Err(Error::InvalidState("namespace-report parent PID differs"));
    }
    let mut set = NamespaceSet {
        identities: [ZERO_IDENTITY; NAMESPACES.len()],
    };
    for (identity, path) in set.identities.iter_mut().zip(CHILD_PATHS.iter()) {
        let fd = calls
            .open(path)
            .map_err(|e| io_error("open child proc namespace", e))?;
        let mut stat = RawStat::ZERO;
        let observed = calls.stat(fd, &mut stat);
        // Preserve the original stat error even when close also fails. Neither
        // result owns an fd or consults errno after the cleanup call.
        let closed = calls.close(fd);
        observed.map_err(|e| io_error("inspect child proc namespace", e))?;
        closed.map_err(|e| io_error("close child proc namespace", e))?;
        *identity = NamespaceIdentity {
            device: stat.device,
            inode: stat.inode,
        };
    }
    set.require_children_unchanged()?;
    encode(child as u32, parent as u32, &set, out);
    Ok(())
}

struct RawSyscalls;

#[allow(unsafe_code)]
impl ChildSyscalls for RawSyscalls {
    fn getpid(&mut self) -> Result<i32, Errno> {
        // SAFETY: getpid has no pointer arguments or mutable userspace state.
        let pid = unsafe { syscall4(libc::SYS_getpid, 0, 0, 0, 0) }?;
        i32::try_from(pid).map_err(|_| Errno::INVAL)
    }

    fn getppid(&mut self) -> Result<i32, Errno> {
        // SAFETY: getppid has no pointer arguments or mutable userspace state.
        let pid = unsafe { syscall4(libc::SYS_getppid, 0, 0, 0, 0) }?;
        i32::try_from(pid).map_err(|_| Errno::INVAL)
    }

    fn open(&mut self, path: &[u8; PROC_PATH_BYTES]) -> Result<i32, Errno> {
        // SAFETY: only compile-time checked CHILD_PATHS reach this private
        // backend. They are NUL terminated; no creation flags or mode are used.
        let fd = unsafe {
            syscall4(
                libc::SYS_openat,
                libc::AT_FDCWD as usize,
                path.as_ptr() as usize,
                (libc::O_RDONLY | libc::O_CLOEXEC) as usize,
                0,
            )
        }?;
        i32::try_from(fd).map_err(|_| Errno::INVAL)
    }

    fn stat(&mut self, fd: i32, out: &mut RawStat) -> Result<(), Errno> {
        // SAFETY: out is a writable, initialized, aligned 144-byte kernel stat
        // buffer. No Rust references overlap its exclusive borrow during fstat.
        unsafe {
            syscall4(
                libc::SYS_fstat,
                fd as usize,
                std::ptr::from_mut(out) as usize,
                0,
                0,
            )
        }?;
        Ok(())
    }

    fn close(&mut self, fd: i32) -> Result<(), Errno> {
        // SAFETY: this raw fd was opened by this collector and is closed once.
        unsafe { syscall4(libc::SYS_close, fd as usize, 0, 0, 0) }?;
        Ok(())
    }
}

#[allow(unsafe_code)]
unsafe fn syscall4(
    number: libc::c_long,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
) -> Result<usize, Errno> {
    let result: isize;
    // SAFETY: callers supply each syscall's valid scalar/pointer arguments.
    // Linux x86_64 clobbers rcx/r11. Memory is deliberately not marked readonly.
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") number as isize => result,
            in("rdi") a,
            in("rsi") b,
            in("rdx") c,
            in("r10") d,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    if (-4095..0).contains(&result) {
        Err(Errno::from_raw_os_error((-result) as i32))
    } else {
        Ok(result as usize)
    }
}

fn encode(child: u32, parent: u32, set: &NamespaceSet, out: &mut [u8; BYTES]) {
    out.fill(0);
    out[..8].copy_from_slice(MAGIC);
    out[8..10].copy_from_slice(&1_u16.to_le_bytes());
    out[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    out[16..20].copy_from_slice(&child.to_le_bytes());
    out[20..24].copy_from_slice(&parent.to_le_bytes());
    out[24..26].copy_from_slice(&(NAMESPACES.len() as u16).to_le_bytes());
    for (bytes, identity) in out[HEADER_BYTES..]
        .chunks_exact_mut(16)
        .zip(&set.identities)
    {
        bytes[..8].copy_from_slice(&identity.device.to_le_bytes());
        bytes[8..].copy_from_slice(&identity.inode.to_le_bytes());
    }
}

pub(super) fn require(
    set: &NamespaceSet,
    child: Pid,
    parent: Pid,
    bytes: &[u8],
) -> Result<(), Error> {
    let frame: &[u8; BYTES] = bytes
        .try_into()
        .map_err(|_| Error::InvalidState("namespace-report length differs"))?;
    if &frame[..8] != MAGIC {
        return Err(Error::InvalidState("namespace-report magic differs"));
    }
    if frame[8..10] != 1_u16.to_le_bytes() {
        return Err(Error::InvalidState("namespace-report version differs"));
    }
    if frame[12..16] != (BYTES as u32).to_le_bytes() {
        return Err(Error::InvalidState(
            "namespace-report declared length differs",
        ));
    }
    if frame[10..12] != [0; 2] || frame[26..32] != [0; 6] {
        return Err(Error::InvalidState(
            "namespace-report reserved bytes differ",
        ));
    }
    if frame[24..26] != (NAMESPACES.len() as u16).to_le_bytes() {
        return Err(Error::InvalidState("namespace-report count differs"));
    }
    let reported_child = u32::from_le_bytes([frame[16], frame[17], frame[18], frame[19]]);
    let reported_parent = u32::from_le_bytes([frame[20], frame[21], frame[22], frame[23]]);
    if reported_child == 0 || reported_child > i32::MAX as u32 {
        return Err(Error::InvalidState("invalid namespace-report child PID"));
    }
    if reported_parent == 0 || reported_parent > i32::MAX as u32 {
        return Err(Error::InvalidState("invalid namespace-report parent PID"));
    }
    let mut observed = NamespaceSet {
        identities: [ZERO_IDENTITY; NAMESPACES.len()],
    };
    for (identity, bytes) in observed
        .identities
        .iter_mut()
        .zip(frame[HEADER_BYTES..].chunks_exact(16))
    {
        identity.device = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        identity.inode = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
    }
    let mut canonical = [0; BYTES];
    encode(reported_child, reported_parent, &observed, &mut canonical);
    if &canonical != frame {
        return Err(Error::InvalidState(
            "namespace-report encoding is not canonical",
        ));
    }
    if reported_child != child.as_raw_pid() as u32 {
        return Err(Error::InvalidState("namespace-report child PID differs"));
    }
    if reported_parent != parent.as_raw_pid() as u32 {
        return Err(Error::InvalidState("namespace-report parent PID differs"));
    }
    observed.require_children_unchanged()?;
    for ((actual, expected), (name, _)) in observed
        .identities
        .iter()
        .zip(&set.identities)
        .zip(NAMESPACES)
    {
        if actual != expected {
            return Err(Error::Namespace(name));
        }
    }
    Ok(())
}

// Inert fixtures need not have a visible parent (for example a PID-1 test
// runner). This test-only encoder cannot be used by the production launch path.
#[cfg(test)]
pub(crate) fn current_namespace_report_for_test(child: Pid, parent: Pid) -> [u8; BYTES] {
    let set = NamespaceSet::capture_self().unwrap();
    let mut bytes = [0; BYTES];
    encode(
        child.as_raw_pid() as u32,
        parent.as_raw_pid() as u32,
        &set,
        &mut bytes,
    );
    bytes
}

#[cfg(test)]
#[path = "child_namespace_report_tests.rs"]
mod tests;
