//! Private pipe publication after the real service has acquired durable custody.
use super::{Admission, Budget, Error, Manifest, Policy, Result};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as BYTES, CompilerExecutionServiceReadyV2 as Ready,
};
use rustix::{
    fs::{FileType, OFlags},
    io::FdFlags,
};
use std::{mem::size_of, os::fd::OwnedFd};

pub(super) const WRITER_STORAGE: usize = size_of::<OwnedFd>() + size_of::<usize>();
const IO_WORK: usize = 8 + 8 * 1024;
const SCRATCH: usize = 4 * size_of::<Ready>() + size_of::<libc::stat>() + 4096;
// POSIX's minimum atomic pipe-write size. No partial successful frame is valid.
const _: () = assert!(BYTES <= 512);

pub(super) fn check_binding(a: &Admission<'_>, m: &Manifest, b: &mut Budget<'_>) -> Result<()> {
    b.charge_work(128)?;
    let client = a.service.expected_client();
    if !m.matches_policy(&a.policy, b)?
        || m.client().pid() != client.pid()
        || m.client().uid() != client.uid()
        || m.client().gid() != client.gid()
        || m.external_anchor_service() != a.anchor.service_identity()
    {
        return Err(Error::rejected("native readiness launch custody mismatch"));
    }
    Ok(())
}

pub(super) fn check_writer(writer: &OwnedFd, b: &mut Budget<'_>) -> Result<()> {
    b.with_prepaid_scope(WRITER_STORAGE, 8, IO_WORK, SCRATCH, |_| {
        let stat = rustix::fs::fstat(writer)?;
        let flags = rustix::io::fcntl_getfd(writer)?;
        let status = rustix::fs::fcntl_getfl(writer)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::Fifo
            || flags != FdFlags::CLOEXEC
            || status & OFlags::ACCMODE != OFlags::WRONLY
            || !status.contains(OFlags::NONBLOCK)
            || status.intersects(OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
        {
            return Err(Error::rejected(
                "native readiness requires a private nonblocking pipe writer",
            ));
        }
        Ok(())
    })
}

// The sole production caller supplies fresh admitted-custody/journal checks.
// No public callback or inert frame can reach this publication operation.
pub(super) fn publish(
    manifest: &Manifest,
    policy: &Policy,
    writer: OwnedFd,
    b: &mut Budget<'_>,
    validate: impl FnOnce(&mut Budget<'_>) -> Result<()>,
) -> Result<()> {
    let floor = manifest.retained_storage() + policy.retained_storage() + WRITER_STORAGE;
    b.with_prepaid_scope(floor, 8, IO_WORK, SCRATCH, |b| {
        check_writer(&writer, b)?;
        let (ready, storage) = Ready::new(std::process::id(), manifest, policy, b)?;
        b.reserve_storage(storage.additional_storage())?;
        validate(b)?;
        check_writer(&writer, b)?;
        // One prepaid syscall, no EINTR/EAGAIN retry and no unmetered wait.
        if rustix::io::write(&writer, ready.canonical_bytes())? != BYTES {
            return Err(Error::rejected("native readiness short write"));
        }
        drop(writer);
        Ok(())
    })
}

#[cfg(test)]
#[path = "compiler_execution_issuer_native_readiness_tests.rs"]
mod tests;
