//! Finite-attempt native sealed-file mechanics; fixed-path provenance is separate.
use super::{IO_STORAGE, IO_WORK, MAX_POLICY_BYTES, refundable_scope};
use crate::{other, require};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::{Mode, OFlags, SealFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

fn seals() -> SealFlags {
    SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE
}
fn snapshot(m: &Metadata) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn exact_read(file: &File, length: usize) -> io::Result<Box<[u8]>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(other)?;
    require(
        bytes.capacity() == length,
        "native sealed allocation capacity",
    )?;
    bytes.resize(length, 0);
    require(
        rustix::io::pread(file, &mut bytes, 0)? == length,
        "short native sealed read",
    )?;
    require(
        rustix::io::pread(file, &mut [0u8; 1], length as u64)? == 0,
        "native sealed EOF changed",
    )?;
    Ok(bytes.into_boxed_slice())
}

/// Root-owned immutable control framing only, with no approval/proof authority.
/// One write attempt; caller supplies an explicit closed-profile extent bound.
/// Input bytes remain prepaid; returned File/receipt charge is unreserved.
pub(crate) fn seal_native_control_bytes(
    bytes: &[u8],
    maximum: usize,
    budget: &mut Budget<'_>,
) -> io::Result<(File, usize)> {
    budget.charge_work(IO_WORK).map_err(other)?;
    require(
        !bytes.is_empty() && bytes.len() <= maximum && maximum <= MAX_POLICY_BYTES,
        "native control sealing extent",
    )?;
    refundable_scope(
        budget,
        bytes.len(),
        4 * bytes.len(),
        IO_STORAGE + bytes.len(),
        |_| {
            fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
            let file = File::from(rustix::fs::memfd_create(
                c"fe2o3-native-proof-control-v1",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )?);
            require(
                rustix::io::pwrite(&file, bytes, 0)? == bytes.len(),
                "short native control seal write",
            )?;
            rustix::fs::fchmod(&file, Mode::RUSR)?;
            rustix::fs::fcntl_add_seals(&file, seals())?;
            let readonly = File::from(rustix::fs::open(
                format!("/proc/self/fd/{}", file.as_raw_fd()),
                OFlags::RDONLY | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            let created = file.metadata()?;
            let returned = readonly.metadata()?;
            require(
                snapshot(&created) == snapshot(&returned)
                    && returned.is_file()
                    && returned.nlink() == 0
                    && returned.len() == bytes.len() as u64
                    && returned.mode() & 0o7777 == 0o400
                    && (returned.uid(), returned.gid()) == (0, 0)
                    && rustix::fs::fcntl_get_seals(&readonly)? == seals()
                    && rustix::io::fcntl_getfd(&readonly)? == rustix::io::FdFlags::CLOEXEC
                    && rustix::fs::fcntl_getfl(&readonly)? & OFlags::ACCMODE == OFlags::RDONLY,
                "native control sealed handoff differs",
            )?;
            Ok((readonly, size_of::<File>() + size_of::<usize>()))
        },
    )
}

/// One read attempt and one EOF observation. An EINTR/short read is a refusal,
/// never a retry. Returns unreserved Box/header storage. Owner identity, metadata,
/// flags, complete seals and a bounded memfd link are observed before and after.
pub(crate) fn read_sealed_file(
    file: &File,
    owner: (u32, u32),
    maximum: usize,
    budget: &mut Budget<'_>,
) -> io::Result<(Box<[u8]>, usize)> {
    budget.charge_work(IO_WORK).map_err(other)?;
    require(
        budget.storage() >= size_of::<File>(),
        "native sealed File not prepaid",
    )?;
    let before = file.metadata()?;
    require(
        before.is_file()
            && before.nlink() == 0
            && (before.uid(), before.gid()) == owner
            && before.mode() & 0o7777 == 0o400
            && before.len() > 0
            && before.len() <= maximum as u64,
        "native sealed input metadata",
    )?;
    let length = usize::try_from(before.len()).map_err(other)?;
    let scratch = length
        .checked_mul(2)
        .and_then(|n| n.checked_add(IO_STORAGE))
        .ok_or_else(|| io::Error::other("native sealed read accounting overflow"))?;
    let work = length
        .checked_mul(2)
        .ok_or_else(|| io::Error::other("native sealed read work overflow"))?;
    refundable_scope(budget, size_of::<File>(), work, scratch, |_| {
        require(
            rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC,
            "native sealed CLOEXEC missing",
        )?;
        let flags = rustix::fs::fcntl_getfl(file)?;
        require(
            flags & OFlags::ACCMODE == OFlags::RDONLY
                && !flags
                    .intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
                && rustix::fs::fcntl_get_seals(file)? == seals(),
            "native sealed flags or seals",
        )?;
        let path = format!("/proc/self/fd/{}", file.as_raw_fd());
        let mut link = [0u8; 256];
        let length_link = rustix::fs::readlinkat_raw(rustix::fs::CWD, &path, &mut link)?;
        require(
            length_link > 0 && length_link < link.len(),
            "native memfd link truncated",
        )?;
        let link = &link[..length_link];
        require(
            (link.starts_with(b"/memfd:") || link.starts_with(b"memfd:"))
                && link.ends_with(b" (deleted)"),
            "native sealed input is not memfd",
        )?;
        let bytes = exact_read(file, length)?;
        require(
            snapshot(&before) == snapshot(&file.metadata()?)
                && rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC
                && rustix::fs::fcntl_getfl(file)? == flags
                && rustix::fs::fcntl_get_seals(file)? == seals(),
            "native sealed input changed",
        )?;
        Ok((bytes, length + size_of::<Box<[u8]>>() + size_of::<usize>()))
    })
}

/// Makes a bounded immutable copy of the original already-admitted fixed policy
/// file. This does not authenticate the file's path/tree. The root deployment must
/// retain and revalidate those independently around this operation.
pub(crate) fn seal_policy_source(
    source: &File,
    expected: ([u8; 32], u64),
    budget: &mut Budget<'_>,
) -> io::Result<(File, usize)> {
    budget.charge_work(IO_WORK).map_err(other)?;
    let length = usize::try_from(expected.1).map_err(other)?;
    require(
        expected.0 != [0; 32] && (1..=MAX_POLICY_BYTES).contains(&length),
        "native policy sealing extent",
    )?;
    require(
        budget.storage() >= size_of::<File>(),
        "native policy source not prepaid",
    )?;
    let scratch = IO_STORAGE + 3 * length;
    refundable_scope(budget, size_of::<File>(), 8 * length, scratch, |_| {
        let before = source.metadata()?;
        let source_flags = rustix::fs::fcntl_getfl(source)?;
        require(
            before.is_file()
                && before.nlink() == 1
                && (before.uid(), before.gid()) == (0, 0)
                && before.mode() & 0o7777 == 0o444
                && before.len() == expected.1
                && rustix::io::fcntl_getfd(source)? == rustix::io::FdFlags::CLOEXEC
                && source_flags & OFlags::ACCMODE == OFlags::RDONLY
                && !source_flags
                    .intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT),
            "native policy source metadata",
        )?;
        let bytes = exact_read(source, length)?;
        require(
            <[u8; 32]>::from(Sha256::digest(&bytes)) == expected.0
                && snapshot(&before) == snapshot(&source.metadata()?),
            "native policy source identity changed",
        )?;
        let file = File::from(rustix::fs::memfd_create(
            c"fe2o3-native-conditional-root-policy-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )?);
        require(
            rustix::io::pwrite(&file, &bytes, 0)? == bytes.len(),
            "short native policy seal write",
        )?;
        rustix::fs::fchmod(&file, Mode::RUSR)?;
        rustix::fs::fcntl_add_seals(&file, seals())?;
        let readonly = File::from(rustix::fs::open(
            format!("/proc/self/fd/{}", file.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let source_object = file.metadata()?;
        let returned = readonly.metadata()?;
        require(
            snapshot(&source_object) == snapshot(&returned)
                && returned.len() == expected.1
                && (returned.uid(), returned.gid()) == (0, 0)
                && rustix::fs::fcntl_get_seals(&readonly)? == seals()
                && rustix::io::fcntl_getfd(&readonly)? == rustix::io::FdFlags::CLOEXEC
                && rustix::fs::fcntl_getfl(&readonly)? & OFlags::ACCMODE == OFlags::RDONLY
                && snapshot(&before) == snapshot(&source.metadata()?)
                && rustix::io::fcntl_getfd(source)? == rustix::io::FdFlags::CLOEXEC
                && rustix::fs::fcntl_getfl(source)? == source_flags,
            "native policy sealed handoff changed",
        )?;
        Ok((readonly, size_of::<File>() + size_of::<usize>()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    #[test]
    fn finite_native_sealed_reader_preserves_original_and_rejects_changed_flags() {
        let file = crate::wire::seal(b"native fixed input").unwrap();
        let owner = (
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        let mut work = Work::new(1_000_000);
        let mut b = Budget::new(&mut work, 1_000_000);
        b.reserve_storage(size_of::<File>()).unwrap();
        let floor = b.storage();
        let (bytes, charge) = read_sealed_file(&file, owner, 64, &mut b).unwrap();
        assert_eq!(&*bytes, b"native fixed input");
        assert!(charge >= bytes.len());
        assert_eq!(b.storage(), floor);
        assert!(read_sealed_file(&file, (owner.0 ^ 1, owner.1), 64, &mut b).is_err());
        assert!(read_sealed_file(&file, owner, 1, &mut b).is_err());
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        assert!(read_sealed_file(&file, owner, 64, &mut b).is_err());
        assert_eq!(b.storage(), floor);
    }
}
