//! Direct raw-child syscalls only. Failure returns to child_fail, never to Rust
//! cleanup: exit closes all partially constructed descriptors without retry.

use crate::PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1 as FLOOR;
use crate::native_spawn::compiler_child_channel::{TRANSFER_BYTES, encode_transfer};
use std::{mem::size_of, os::fd::RawFd, ptr};

// Exact Linux x86-64 SCM_RIGHTS ABI, including CMSG_SPACE's trailing alignment.
#[repr(C)]
struct Rights {
    length: usize,
    level: i32,
    kind: i32,
    fd: RawFd,
    padding: u32,
}
// Kernel ABI rather than libc's GNU/musl-specific msghdr integer widths/padding.
#[repr(C)]
struct Message {
    name: *mut libc::c_void,
    name_len: u32,
    name_padding: u32,
    iov: *mut libc::iovec,
    iov_len: usize,
    control: *mut libc::c_void,
    control_len: usize,
    flags: i32,
    flags_padding: u32,
}
const _: () = assert!(size_of::<Rights>() == 24);
const _: () = assert!(std::mem::offset_of!(Rights, fd) == 16);
const _: () = assert!(size_of::<Message>() == 56);

// Logical ABI frame and conservative copies/control, not generated stack or RSS.
pub(crate) const SCRATCH: usize = 4
    * (TRANSFER_BYTES
        + size_of::<Rights>()
        + size_of::<Message>()
        + size_of::<libc::iovec>()
        + size_of::<libc::ucred>()
        + size_of::<[RawFd; 2]>()
        + size_of::<libc::socklen_t>())
    + 256;

/// Called only after final profile validation. A negative result must terminate
/// the raw child immediately. All locals are scalar/ABI records without Drop.
pub(super) unsafe fn create_and_transfer(control: RawFd, expected_parent: i32) -> RawFd {
    let mut root = libc::ucred {
        pid: -1,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut length = size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: all pointers below name fixed initialized child stack storage.
    // One attempt per syscall, including close; no libc cancellation wrappers.
    unsafe {
        if libc::syscall(
            libc::SYS_getsockopt,
            control,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &raw mut root,
            &raw mut length,
        ) != 0
            || length as usize != size_of::<libc::ucred>()
            || root.pid != expected_parent
            || root.uid != 0
            || root.gid != 0
        {
            return -1;
        }
        let pid = libc::syscall(libc::SYS_getpid);
        let parent = libc::syscall(libc::SYS_getppid);
        if pid <= 0 || pid > i64::from(i32::MAX) || parent != i64::from(expected_parent) {
            return -1;
        }
        let Some(mut payload) = encode_transfer(pid as u32, parent as u32) else {
            return -1;
        };
        let mut peers = [-1_i32; 2];
        if libc::syscall(
            libc::SYS_socketpair,
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            peers.as_mut_ptr(),
        ) != 0
        {
            return -1;
        }
        // Either socketpair result may be 195 or a stdio slot. Pin the client
        // above every final binding before closing either original descriptor.
        let high = libc::syscall(libc::SYS_fcntl, peers[1], libc::F_DUPFD_CLOEXEC, FLOOR);
        if high < i64::from(FLOOR)
            || high > i64::from(i32::MAX)
            || libc::syscall(libc::SYS_close, peers[1]) != 0
        {
            return -1;
        }
        let mut rights = Rights {
            length: std::mem::offset_of!(Rights, fd) + size_of::<RawFd>(),
            level: libc::SOL_SOCKET,
            kind: libc::SCM_RIGHTS,
            fd: peers[0],
            padding: 0,
        };
        let mut vector = libc::iovec {
            iov_base: payload.as_mut_ptr().cast(),
            iov_len: TRANSFER_BYTES,
        };
        let header = Message {
            name: ptr::null_mut(),
            name_len: 0,
            name_padding: 0,
            iov: &raw mut vector,
            iov_len: 1,
            control: (&raw mut rights).cast(),
            control_len: size_of::<Rights>(),
            flags: 0,
            flags_padding: 0,
        };
        if libc::syscall(
            libc::SYS_sendmsg,
            control,
            &raw const header,
            libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
        ) != TRANSFER_BYTES as i64
            || libc::syscall(libc::SYS_close, peers[0]) != 0
            || libc::syscall(libc::SYS_close, control) != 0
        {
            return -1;
        }
        high as RawFd
    }
}
