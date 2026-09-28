//! Fixed-size argument and service-account intake for the dedicated provisioner.
use super::{Failure, Result};
use crate::native_root_source::provisioning::{FILE_WORK, io};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::{Mode, OFlags};
use std::{ffi::CStr, mem::size_of};
use zeroize::Zeroizing;

const ARGV_BYTES: usize = 4096 + 1 + 20 + 1;
const ACCOUNT_BYTES: usize = 64 * 1024;
pub(super) const ARGS_WORK: usize = FILE_WORK + 8 * ARGV_BYTES;
pub(super) const ARGS_SCRATCH: usize = 3 * ARGV_BYTES + 4096;
pub(super) const ACCOUNT_WORK: usize = FILE_WORK + 8 * ACCOUNT_BYTES;
pub(super) const ACCOUNT_SCRATCH: usize =
    2 * ACCOUNT_BYTES + 4 * size_of::<libc::passwd>() + 4 * size_of::<libc::group>() + 4096;

pub(super) fn generation(b: &mut Budget<'_>) -> Result<u64> {
    b.with_prepaid_scope(0, 8, ARGS_WORK, ARGS_SCRATCH, |_| {
        let file = rustix::fs::open(
            "/proc/self/cmdline",
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|e| io("open provisioning arguments", e))?;
        let mut bytes = Zeroizing::new([0; ARGV_BYTES]);
        let n = rustix::io::pread(&file, &mut bytes[..], 0)
            .map_err(|e| io("read provisioning arguments", e))?;
        let mut trailing = Zeroizing::new([0; 1]);
        if rustix::io::pread(&file, &mut trailing[..], n as u64)
            .map_err(|e| io("probe provisioning arguments", e))?
            != 0
        {
            return Err(Failure::Invalid("provisioning arguments exceed bound"));
        }
        parse_arguments(&bytes[..n])
    })
}
fn parse_arguments(bytes: &[u8]) -> Result<u64> {
    let mut arguments = bytes.split(|&b| b == 0);
    let program = arguments
        .next()
        .ok_or(Failure::Invalid("missing program argument"))?;
    let generation = arguments
        .next()
        .ok_or(Failure::Invalid("missing generation argument"))?;
    if program.is_empty()
        || program.len() > 4096
        || arguments.next() != Some(&[][..])
        || arguments.next().is_some()
        || generation.is_empty()
        || generation.len() > 20
        || generation[0] == b'0'
    {
        return Err(Failure::Invalid(
            "expected one canonical nonzero generation",
        ));
    }
    generation.iter().try_fold(0u64, |value, byte| {
        if !byte.is_ascii_digit() {
            return Err(Failure::Invalid("generation is not decimal"));
        }
        value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(byte - b'0')))
            .ok_or(Failure::Invalid("generation overflow"))
    })
}

#[allow(unsafe_code)]
pub(super) fn service(name: &CStr, b: &mut Budget<'_>) -> Result<(u32, u32)> {
    b.with_prepaid_scope(0, 8, ACCOUNT_WORK, ACCOUNT_SCRATCH, |_| {
        let mut buffer = Zeroizing::new([0u8; ACCOUNT_BYTES]);
        let mut user = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        // SAFETY: fixed names, writable bounded buffer, live output pointers. NSS is
        // a trusted libc boundary; a large record refuses without retry/reallocation.
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                user.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status != 0 {
            return Err(io(
                "look up provisioning user",
                rustix::io::Errno::from_raw_os_error(status),
            ));
        }
        if result != user.as_mut_ptr() {
            return Err(Failure::Invalid("provisioning user missing"));
        }
        // SAFETY: successful getpwnam_r returned the exact supplied entry.
        let user = unsafe { user.assume_init() };
        let (uid, primary_gid) = (user.pw_uid, user.pw_gid);
        let mut group = std::mem::MaybeUninit::<libc::group>::uninit();
        let mut result = std::ptr::null_mut();
        // SAFETY: same bounded lookup contract; no pointer from the user entry is used.
        let status = unsafe {
            libc::getgrnam_r(
                name.as_ptr(),
                group.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status != 0 {
            return Err(io(
                "look up provisioning group",
                rustix::io::Errno::from_raw_os_error(status),
            ));
        }
        if result != group.as_mut_ptr() {
            return Err(Failure::Invalid("provisioning group missing"));
        }
        // SAFETY: successful getgrnam_r returned the exact supplied entry.
        let gid = unsafe { group.assume_init() }.gr_gid;
        if primary_gid != gid {
            return Err(Failure::Invalid("provisioning primary group mismatch"));
        }
        Ok((uid, gid))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_canonical_generation_is_accepted() {
        for (bytes, expected) in [
            (b"provision\x001\0".as_slice(), 1),
            (b"p\x0018446744073709551615\0".as_slice(), u64::MAX),
        ] {
            assert_eq!(parse_arguments(bytes).unwrap(), expected);
        }
        for bytes in [
            b"".as_slice(),
            b"p\0",
            b"p\x000\0",
            b"p\x0001\0",
            b"p\0+1\0",
            b"p\0-1\0",
            b"p\x001 \0",
            b"p\x001",
            b"p\x001\0\0",
            b"p\x001\0extra\0",
            b"\x001\0",
            b"p\x0018446744073709551616\0",
            b"p\xff\0",
        ] {
            assert!(parse_arguments(bytes).is_err(), "{bytes:?}");
        }
    }
}
