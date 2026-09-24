//! Shared handoff cleanup for Linux auxiliary descriptors not recognized by rustix.
use std::{
    mem::{MaybeUninit, size_of},
    os::fd::{FromRawFd, OwnedFd},
};

// SCM_PIDFD has one int payload, just like a one-descriptor SCM_RIGHTS record.
// The first term also includes rustix's extra cmsghdr alignment allowance.
pub(super) const BYTES: usize = rustix::cmsg_space!(ScmRights(3), ScmRights(1));

// Linux UAPI value; the pinned libc and rustix do not expose SCM_PIDFD.
const SCM_PIDFD: libc::c_int = 0x04;
const HEADER: usize = unsafe { libc::CMSG_LEN(0) } as usize;
const FD_BYTES: usize = size_of::<libc::c_int>();

#[cfg(test)]
std::thread_local! {
    static AUXILIARY_CLOSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(super) fn auxiliary_closes() -> usize {
    AUXILIARY_CLOSES.with(std::cell::Cell::get)
}

#[repr(C)]
struct Backing {
    // A zero-sized first field gives bytes the exact cmsghdr alignment.
    _alignment: [libc::cmsghdr; 0],
    bytes: [MaybeUninit<u8>; BYTES],
}

/// One receive's zeroed control backing and auxiliary-descriptor cleanup.
///
/// Only pass `parts().0` to rustix's `RecvAncillaryBuffer`. Set the arm flag
/// immediately after a successful receive, then drain every rustix message and
/// drop that buffer before `finish`. Declare the rustix buffer after this guard
/// so its SCM_RIGHTS cleanup also precedes this guard on unwind.
///
/// The backing must contain only that receive's kernel-generated records. It
/// must not be reused, overwritten with uninitialized bytes, or populated from
/// an untrusted byte slice: complete SCM_PIDFD payloads transfer FD ownership.
pub(super) struct Guard {
    backing: Backing,
    armed: bool,
}

impl Guard {
    pub(super) fn new() -> Self {
        Self {
            backing: Backing {
                _alignment: [],
                bytes: [MaybeUninit::new(0); BYTES],
            },
            armed: false,
        }
    }

    pub(super) fn parts(&mut self) -> (&mut [MaybeUninit<u8>], &mut bool) {
        (&mut self.backing.bytes, &mut self.armed)
    }

    /// Closes auxiliary pidfds and reports any unexpected or malformed record.
    pub(super) fn finish(mut self) -> bool {
        self.cleanup()
    }

    fn cleanup(&mut self) -> bool {
        if !std::mem::take(&mut self.armed) {
            return false;
        }

        // No public initialized-length accessor exists in pinned rustix. Fresh
        // zeroed backing makes the untouched tail a sentinel; kernel headers
        // alone determine the checked walk. Rustix does not erase drained data.
        // SAFETY: new initialized every byte, and the sole receive only writes
        // initialized kernel bytes. The rustix borrow has ended before cleanup.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(self.backing.bytes.as_mut_ptr().cast::<u8>(), BYTES)
        };
        let mut offset = 0;
        let mut unexpected = false;
        while bytes.len() - offset >= size_of::<libc::cmsghdr>() {
            // SAFETY: the whole initialized header is within the backing; an
            // unaligned read avoids making a typed reference into byte storage.
            let header = unsafe {
                bytes
                    .as_ptr()
                    .add(offset)
                    .cast::<libc::cmsghdr>()
                    .read_unaligned()
            };
            let length = header.cmsg_len;
            if length == 0 {
                return unexpected || bytes[offset..].iter().any(|&byte| byte != 0);
            }
            if length < HEADER || length > bytes.len() - offset {
                return true;
            }
            if header.cmsg_level != libc::SOL_SOCKET || header.cmsg_type != libc::SCM_RIGHTS {
                unexpected = true;
                if header.cmsg_level == libc::SOL_SOCKET
                    && header.cmsg_type == SCM_PIDFD
                    && length == HEADER + FD_BYTES
                {
                    let payload = offset + HEADER;
                    // SAFETY: this complete int payload was checked above and
                    // belongs to a kernel-generated SCM_PIDFD, never SCM_RIGHTS.
                    let descriptor = unsafe {
                        bytes
                            .as_ptr()
                            .add(payload)
                            .cast::<libc::c_int>()
                            .read_unaligned()
                    };
                    if descriptor >= 0 {
                        bytes[payload..payload + FD_BYTES].copy_from_slice(&(-1_i32).to_ne_bytes());
                        // SAFETY: the kernel transferred this descriptor. The
                        // pinned rustix ignores this record; neither its drain
                        // nor this disarmed guard can adopt the descriptor again.
                        drop(unsafe { OwnedFd::from_raw_fd(descriptor) });
                        #[cfg(test)]
                        AUXILIARY_CLOSES.with(|count| count.set(count.get() + 1));
                    }
                }
            }
            let Some(step) = align(length) else {
                return true;
            };
            if step > bytes.len() - offset {
                // A complete final record need not include all its padding.
                return unexpected;
            }
            offset += step;
        }
        unexpected || bytes[offset..].iter().any(|&byte| byte != 0)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn align(length: usize) -> Option<usize> {
    let mask = size_of::<usize>() - 1;
    length.checked_add(mask).map(|n| n & !mask)
}

const _: () = {
    assert!(FD_BYTES == size_of::<i32>());
    assert!(HEADER >= size_of::<libc::cmsghdr>());
    assert!(BYTES >= HEADER + FD_BYTES);
};

#[cfg(test)]
#[path = "handoff_ancillary_tests.rs"]
mod tests;
