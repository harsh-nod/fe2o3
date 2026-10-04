//! Bounded provenance checks on untrusted provisioned files, not admitted authority.
//!
//! The caller keeps the complete source reservation live. Returned bytes are
//! unreserved: reserve their storage immediately, keep it through native decoding
//! or key construction, then drop the bytes before releasing that reservation.
//! These observations do not seal a source or replace native executable admission.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::{
    fs::{FileType, OFlags},
    io::{Errno, FdFlags},
};
use std::{error::Error, fmt, fs::File, mem::size_of};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub(crate) type Result<T> = std::result::Result<T, RootSourceErrorV2>;

#[path = "native_provisioning_io.rs"]
pub(crate) mod provisioning;

/// Fixed-shape refusal from native root-provisioned source validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootSourceErrorV2 {
    /// The original resource account or a checked quota formula refused.
    Resource(Resource),
    /// One finite-attempt syscall failed; no path or file contents are retained.
    Io {
        /// A fixed operation name.
        operation: &'static str,
        /// The operating system error number.
        errno: i32,
    },
    /// The requested length is zero or cannot be represented as a file offset.
    InvalidLength,
    /// Descriptor or open-file-description flags violate the provisioned policy.
    Descriptor,
    /// File type, ownership, permissions, links, or length violate the policy.
    Metadata,
    /// A capability or POSIX ACL attribute is present.
    ForbiddenAttribute,
    /// The single payload read returned fewer bytes than requested.
    ShortRead,
    /// The EOF probe found data beyond the expected length.
    TrailingBytes,
    /// The metadata snapshots or the two complete byte observations differ.
    Changed,
}
impl From<Resource> for RootSourceErrorV2 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl RootSourceErrorV2 {
    fn io(operation: &'static str, error: Errno) -> Self {
        Self::Io {
            operation,
            errno: error.raw_os_error(),
        }
    }
}
impl fmt::Display for RootSourceErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Io { operation, errno } => {
                write!(f, "native root source {operation}: errno {errno}")
            }
            Self::InvalidLength => f.write_str("native root source length is invalid"),
            Self::Descriptor => f.write_str("native root source descriptor flags are invalid"),
            Self::Metadata => f.write_str("native root source metadata is invalid"),
            Self::ForbiddenAttribute => {
                f.write_str("native root source has a capability or POSIX ACL")
            }
            Self::ShortRead => f.write_str("native root source payload read was short"),
            Self::TrailingBytes => f.write_str("native root source has trailing bytes"),
            Self::Changed => f.write_str("native root source changed during observation"),
        }
    }
}
impl Error for RootSourceErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

/// Unreserved storage for a returned seed and this charge token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootSourceStorage(usize);
impl RootSourceStorage {
    pub(crate) const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Descriptor overhead only; `file_storage(length)` is the complete source floor.
pub(crate) const FILE_STORAGE: usize = size_of::<File>();
pub(crate) const SEED_BYTES: usize = 32;
pub(crate) const SEED_STORAGE: usize =
    size_of::<(Zeroizing<[u8; SEED_BYTES]>, RootSourceStorage)>();
const ENTRY_WORK: usize = 8;
const EXECUTABLE_MODE: u32 = 0o555;
const RECORD_MODE: u32 = 0o444;
const SEED_MODE: u32 = 0o400;

// Two six-call observations, with fixed metadata comparisons and error handling.
// Logical envelopes only: not generated stack, elapsed time, or process RSS.
pub(crate) const VALIDATE_WORK: usize = ENTRY_WORK + 16 * 1024;
pub(crate) const VALIDATE_SCRATCH: usize =
    2 * size_of::<Snapshot>() + size_of::<rustix::fs::Stat>() + 4096;
const READ_FIXED_WORK: usize = VALIDATE_WORK + 4 * 1024;
const READ_FIXED_SCRATCH: usize = VALIDATE_SCRATCH + size_of::<RootSourceStorage>();
// Includes initialization, two reads and probes, comparison, result staging,
// and zeroization of both payloads and the guarded EOF byte on every exit.
pub(crate) const SEED_WORK: usize = READ_FIXED_WORK + 32 * SEED_BYTES;
pub(crate) const SEED_SCRATCH: usize = READ_FIXED_SCRATCH + 3 * SEED_BYTES;

fn length(length: usize) -> Result<u64> {
    if length == 0 {
        return Err(RootSourceErrorV2::InvalidLength);
    }
    i64::try_from(length)
        .map(|length| length as u64)
        .map_err(|_| RootSourceErrorV2::InvalidLength)
}

/// Full borrowed source floor, including all backing bytes, even for validation.
pub(crate) fn file_storage(expected_length: usize) -> Result<usize> {
    length(expected_length)?;
    FILE_STORAGE
        .checked_add(expected_length)
        .ok_or(Resource::Arithmetic.into())
}
/// Reservation required before borrowing a record or seed source.
pub(crate) fn record_input_storage<const N: usize>() -> Result<usize> {
    file_storage(N)
}
/// Complete prepaid work, including entry, metadata, byte passes and cleanup.
pub(crate) fn record_work<const N: usize>() -> Result<usize> {
    length(N)?;
    N.checked_mul(32)
        .and_then(|bytes| READ_FIXED_WORK.checked_add(bytes))
        .ok_or(Resource::Arithmetic.into())
}
/// Extra peak above the caller's full source reservation; restored on every exit.
pub(crate) fn record_scratch<const N: usize>() -> Result<usize> {
    length(N)?;
    N.checked_mul(3)
        .and_then(|bytes| READ_FIXED_SCRATCH.checked_add(bytes))
        .ok_or(Resource::Arithmetic.into())
}

#[derive(Clone, Copy)]
struct Policy {
    uid: u32,
    gid: u32,
    mode: u32,
    length: usize,
}
impl Policy {
    const fn root(mode: u32, length: usize) -> Self {
        Self {
            uid: 0,
            gid: 0,
            mode,
            length,
        }
    }
}

// atime is intentionally absent: our own pread may update it. All meaningful
// stable fstat fields, including nanosecond mtime/ctime, and both flag sets are
// compared. Padding and reserved stat fields are not object metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    descriptor: FdFlags,
    status: OFlags,
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    byte_len: u64,
    rdev: u64,
    block_size: u64,
    blocks: u64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}
impl Snapshot {
    fn from_stat(stat: rustix::fs::Stat, descriptor: FdFlags, status: OFlags) -> Result<Self> {
        Ok(Self {
            descriptor,
            status,
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
            byte_len: stat
                .st_size
                .try_into()
                .map_err(|_| RootSourceErrorV2::Metadata)?,
            rdev: stat.st_rdev,
            block_size: stat
                .st_blksize
                .try_into()
                .map_err(|_| RootSourceErrorV2::Metadata)?,
            blocks: stat
                .st_blocks
                .try_into()
                .map_err(|_| RootSourceErrorV2::Metadata)?,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        })
    }

    fn validate(&self, policy: Policy) -> Result<()> {
        let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
        if self.descriptor != FdFlags::CLOEXEC
            || self.status & OFlags::ACCMODE != OFlags::RDONLY
            || self.status.intersects(forbidden)
        {
            return Err(RootSourceErrorV2::Descriptor);
        }
        if FileType::from_raw_mode(self.mode) != FileType::RegularFile
            || self.mode & 0o7777 != policy.mode
            || self.uid != policy.uid
            || self.gid != policy.gid
            || self.links != 1
            || self.byte_len != length(policy.length)?
        {
            return Err(RootSourceErrorV2::Metadata);
        }
        Ok(())
    }
}

fn attribute_absent(result: rustix::io::Result<usize>) -> Result<()> {
    match result {
        Err(Errno::NODATA | Errno::OPNOTSUPP) => Ok(()),
        Ok(_) | Err(Errno::RANGE) => Err(RootSourceErrorV2::ForbiddenAttribute),
        Err(error) => Err(RootSourceErrorV2::io(
            "inspect capability or POSIX ACL",
            error,
        )),
    }
}

fn snapshot(file: &File, policy: Policy) -> Result<Snapshot> {
    let descriptor = rustix::io::fcntl_getfd(file)
        .map_err(|error| RootSourceErrorV2::io("inspect descriptor flags", error))?;
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|error| RootSourceErrorV2::io("inspect status flags", error))?;
    let stat = rustix::fs::fstat(file)
        .map_err(|error| RootSourceErrorV2::io("inspect metadata", error))?;
    let snapshot = Snapshot::from_stat(stat, descriptor, status)?;
    snapshot.validate(policy)?;
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = [0; 1];
        attribute_absent(rustix::fs::fgetxattr(file, attribute, &mut byte))?;
    }
    Ok(snapshot)
}

fn unchanged(before: Snapshot, after: Snapshot) -> Result<()> {
    if before != after {
        return Err(RootSourceErrorV2::Changed);
    }
    Ok(())
}

fn equal_bytes(first: &[u8], second: &[u8]) -> Result<()> {
    // Compare seed contents in constant time; public records share the mechanics.
    if !bool::from(first.ct_eq(second)) {
        return Err(RootSourceErrorV2::Changed);
    }
    Ok(())
}

/// Checks root provenance and positive exact length; does not measure or seal bytes.
pub(crate) fn validate_executable(
    file: &File,
    expected_length: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    validate_with_policy(file, Policy::root(EXECUTABLE_MODE, expected_length), budget)
}

fn validate_with_policy(file: &File, policy: Policy, budget: &mut Budget<'_>) -> Result<()> {
    budget.with_prepaid_scope(
        file_storage(policy.length)?,
        ENTRY_WORK,
        VALIDATE_WORK,
        VALIDATE_SCRATCH,
        |_| unchanged(snapshot(file, policy)?, snapshot(file, policy)?),
    )
}

// FnOnce makes the one-attempt rule explicit, including EINTR and short reads.
fn exact_read(expected: usize, read: impl FnOnce() -> rustix::io::Result<usize>) -> Result<()> {
    let actual = read().map_err(|error| RootSourceErrorV2::io("read payload", error))?;
    if actual != expected {
        return Err(RootSourceErrorV2::ShortRead);
    }
    Ok(())
}

fn read_copy(file: &File, bytes: &mut [u8]) -> Result<()> {
    let offset = length(bytes.len())?;
    exact_read(bytes.len(), || rustix::io::pread(file, &mut *bytes, 0))?;
    // The probe can observe a secret byte if an invalid seed grew. Guard it too.
    let mut trailing = Zeroizing::new([0; 1]);
    if rustix::io::pread(file, &mut trailing[..], offset)
        .map_err(|error| RootSourceErrorV2::io("check EOF", error))?
        != 0
    {
        return Err(RootSourceErrorV2::TrailingBytes);
    }
    Ok(())
}

fn read_stable<const N: usize>(
    file: &File,
    policy: Policy,
    first: &mut [u8; N],
    second: &mut [u8; N],
    after_first: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let before = snapshot(file, policy)?;
    read_copy(file, first)?;
    after_first()?;
    read_copy(file, second)?;
    let after = snapshot(file, policy)?;
    equal_bytes(first, second)?;
    unchanged(before, after)
}

/// Returns an unreserved public record; the caller immediately reserves N bytes.
pub(crate) fn read_record<const N: usize>(file: &File, budget: &mut Budget<'_>) -> Result<[u8; N]> {
    read_record_with_policy(file, Policy::root(RECORD_MODE, N), budget, || Ok(()))
}

fn read_record_with_policy<const N: usize>(
    file: &File,
    policy: Policy,
    budget: &mut Budget<'_>,
    after_first: impl FnOnce() -> Result<()>,
) -> Result<[u8; N]> {
    budget.with_prepaid_scope(
        record_input_storage::<N>()?,
        ENTRY_WORK,
        record_work::<N>()?,
        record_scratch::<N>()?,
        |_| {
            let mut first = [0; N];
            let mut second = [0; N];
            read_stable(file, policy, &mut first, &mut second, after_first)?;
            Ok(first)
        },
    )
}

/// Returns a guarded seed and its unreserved storage charge, without copying the
/// secret into an ordinary array. Reserve the charge immediately on this ledger;
/// pass a mutable borrow to the native constructor to preserve its exact error.
/// Drop the guard before releasing its charge, including on constructor failure.
pub(crate) fn read_seed(
    file: &File,
    budget: &mut Budget<'_>,
) -> Result<(Zeroizing<[u8; SEED_BYTES]>, RootSourceStorage)> {
    read_seed_with_policy(file, Policy::root(SEED_MODE, SEED_BYTES), budget, || Ok(()))
}

fn read_seed_with_policy(
    file: &File,
    policy: Policy,
    budget: &mut Budget<'_>,
    after_first: impl FnOnce() -> Result<()>,
) -> Result<(Zeroizing<[u8; SEED_BYTES]>, RootSourceStorage)> {
    budget.with_prepaid_scope(
        record_input_storage::<SEED_BYTES>()?,
        ENTRY_WORK,
        SEED_WORK,
        SEED_SCRATCH,
        |_| {
            // Both guards exist before the first secret byte is read. Every
            // failure or unwind drops them; success moves only the first guard.
            let mut first = Zeroizing::new([0; SEED_BYTES]);
            let mut second = Zeroizing::new([0; SEED_BYTES]);
            read_stable(file, policy, &mut first, &mut second, after_first)?;
            Ok((first, RootSourceStorage(SEED_STORAGE)))
        },
    )
}

#[cfg(test)]
#[path = "native_root_source_tests.rs"]
mod tests;
