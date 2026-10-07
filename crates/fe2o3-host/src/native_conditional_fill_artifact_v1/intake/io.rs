//! Finite descriptor observations; no ambient artifact paths or std retry readers.
use super::*;
use rustix::fs::{FileType, OFlags, fcntl_getfl, fstat};
use std::os::fd::AsFd;

pub(super) fn same(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_mode == right.st_mode
        && left.st_uid == right.st_uid
        && left.st_gid == right.st_gid
        && left.st_size == right.st_size
        && left.st_nlink == right.st_nlink
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

pub(super) fn occurrence(fd: &OwnedFd, slot: u16) -> Result<InputOccurrence> {
    let stat = fstat(fd).map_err(failure)?;
    InputOccurrence::from_linux_descriptor_v1(slot, stat.st_dev, stat.st_ino, stat.st_mode)
        .map_err(failure)
}

pub(super) fn directory(fd: &OwnedFd) -> Result<()> {
    let flags = fcntl_getfl(fd).map_err(failure)?;
    let stat = fstat(fd).map_err(failure)?;
    require(
        flags & OFlags::ACCMODE == OFlags::RDONLY
            && !flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            && stat.st_mode & 0o7777 == 0o700,
        "native original directory flags/mode",
    )
}

pub(super) fn regular(fd: &impl AsFd, maximum: usize) -> Result<rustix::fs::Stat> {
    let stat = fstat(fd).map_err(failure)?;
    let flags = fcntl_getfl(fd).map_err(failure)?;
    require(
        rustix::io::fcntl_getfd(fd).map_err(failure)? == rustix::io::FdFlags::CLOEXEC
            && flags & OFlags::ACCMODE == OFlags::RDONLY
            && !flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            && FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
            && stat.st_uid == rustix::process::geteuid().as_raw()
            && stat.st_nlink == 1
            && stat.st_mode & 0o077 == 0
            && stat.st_size > 0
            && usize::try_from(stat.st_size).is_ok_and(|n| n <= maximum),
        "native original envelope descriptor",
    )?;
    Ok(stat)
}

pub(super) fn acknowledgment(fd: &OwnedFd) -> Result<rustix::fs::Stat> {
    let stat = fstat(fd).map_err(failure)?;
    let flags = fcntl_getfl(fd).map_err(failure)?;
    require(
        rustix::io::fcntl_getfd(fd).map_err(failure)? == rustix::io::FdFlags::CLOEXEC
            && flags & OFlags::ACCMODE == OFlags::WRONLY
            && flags.contains(OFlags::NONBLOCK)
            && matches!(
                FileType::from_raw_mode(stat.st_mode),
                FileType::Fifo | FileType::Socket
            ),
        "native original acknowledgment descriptor",
    )?;
    Ok(stat)
}

pub(super) fn read(
    fd: &impl AsFd,
    length: usize,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, usize)> {
    read_using(length, budget, |bytes, offset| {
        rustix::io::pread(fd, bytes, offset).map_err(failure)
    })
}

fn read_using(
    length: usize,
    budget: &mut Budget<'_>,
    mut pread: impl FnMut(&mut [u8], u64) -> Result<usize>,
) -> Result<(Vec<u8>, usize)> {
    let storage = length
        .checked_add(size_of::<Vec<u8>>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        length
            .checked_mul(4)
            .and_then(|n| n.checked_add(4096))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.reserve_storage(storage)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(failure)?;
    require(bytes.capacity() == length, "native read capacity")?;
    bytes.resize(length, 0);
    for (index, chunk) in bytes.chunks_mut(64 * 1024).enumerate() {
        budget.charge_work(1)?;
        let count = pread(&mut *chunk, (index * 64 * 1024) as u64)?;
        require(count == chunk.len(), "short native positional read")?;
    }
    budget.charge_work(1)?;
    require(pread(&mut [0], length as u64)? == 0, "native file grew")?;
    Ok((bytes, storage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn finite_reader_has_exact_attempts_and_accounting() {
        let length = 2 * 64 * 1024 + 7;
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let ledger = budget.work_ledger_identity_v1();
        let mut calls = Vec::new();
        let (bytes, charge) = read_using(length, &mut budget, |bytes, offset| {
            calls.push((offset, bytes.len()));
            if offset == length as u64 {
                return Ok(0);
            }
            bytes.fill(19);
            Ok(bytes.len())
        })
        .unwrap();
        assert_eq!(bytes, vec![19; length]);
        assert_eq!(
            calls,
            [(0, 65536), (65536, 65536), (131072, 7), (length as u64, 1)]
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), charge);
        let needed = (budget.work(), budget.peak_storage());
        drop(bytes);
        budget.release_storage(charge).unwrap();
        assert_eq!(budget.storage(), 0);
        for (w, s, ok) in [
            (needed.0, needed.1, true),
            (needed.0 - 1, needed.1, false),
            (needed.0, needed.1 - 1, false),
        ] {
            let mut work = Work::new(w);
            let mut budget = Budget::new(&mut work, s);
            assert_eq!(
                read_using(length, &mut budget, |bytes, offset| Ok(
                    if offset == length as u64 {
                        0
                    } else {
                        bytes.len()
                    }
                ))
                .is_ok(),
                ok
            );
        }
    }

    #[test]
    fn finite_reader_never_retries_short_or_interrupted_transfers() {
        for mode in 0..4 {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let mut calls = 0;
            assert!(
                read_using(4, &mut budget, |bytes, offset| {
                    calls += 1;
                    match mode {
                        0 => Err(failure(rustix::io::Errno::INTR)),
                        1 => Err(failure(rustix::io::Errno::AGAIN)),
                        2 => Ok(3),
                        _ if offset == 0 => Ok(bytes.len()),
                        _ => Ok(1),
                    }
                })
                .is_err()
            );
            assert_eq!(calls, if mode == 3 { 2 } else { 1 });
            assert!(budget.storage() > 0, "failure keeps terminal charge");
        }
    }
}

pub(super) fn application(
    budget: &mut Budget<'_>,
) -> Result<fe2o3_runtime_protocol::WorkerV3ApplicationIdentityV1> {
    budget.charge_work(4096)?;
    let fd = rustix::fs::open(
        "/proc/self/exe",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(failure)?;
    let before = fstat(&fd).map_err(failure)?;
    let length = usize::try_from(before.st_size).map_err(failure)?;
    require(
        FileType::from_raw_mode(before.st_mode) == FileType::RegularFile
            && (1..=1024 * 1024 * 1024).contains(&length),
        "native application image bound",
    )?;
    let (bytes, charge) = read(&fd, length, budget)?;
    budget.charge_work(length.checked_mul(8).ok_or(Resource::Arithmetic)?)?;
    let identity =
        fe2o3_runtime_protocol::WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&bytes)
            .map_err(failure)?;
    require(
        same(&before, &fstat(&fd).map_err(failure)?),
        "native application executable changed",
    )?;
    drop(bytes);
    budget.release_storage(charge)?;
    Ok(identity)
}
