//! Concrete issuer gate after readiness EOF; no public validator or admission substitute.
use super::{Admission, Budget, Error, Manifest, Resource, Result, readiness};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3 as BYTES,
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionRootControlErrorV3 as CodecError,
    CompilerExecutionRootControlRecordV3 as Record,
    compiler_execution_root_gate_reply_v3 as gate_reply,
    validate_compiler_execution_root_gate_request_v3 as validate_gate_request,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::launch_io as transport;
use rustix::{event, fs, io, net, process};
use std::{
    marker::PhantomData,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    rc::Rc,
    time::{Duration, Instant},
};

pub(super) const ENDPOINT_STORAGE: usize = size_of::<(OwnedFd, usize)>();
pub(super) const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: usize = transport::MAX_PHASE_ATTEMPTS;
const POLL_INTERVAL: Duration = Duration::from_millis(1);
const ENTRY: usize = 8;
const CHECK_WORK: usize = ENTRY + 64 * 1088;
const CHECK_FRAME: usize = 4 * size_of::<RootEndpoint<'static>>()
    + 4 * size_of::<net::SocketAddrAny>()
    + 8 * size_of::<Error>()
    + 4096;
const PACKET_WORK: usize = CHECK_WORK + transport::packet_receive_work(BYTES);
const PACKET_FRAME: usize = CHECK_FRAME + transport::packet_receive_scratch(BYTES);
const GATE_WORK: usize = ENTRY + 8 * BYTES;
const GATE_FRAME: usize = 4 * size_of::<(Record, Storage)>() + BYTES + 4096;

/// Internal transport custody only. No raw-FD extraction or synthetic Admission.
/// The full returned owner charge is unreserved; the consumed FD charge stays
/// prepaid until its enclosing service scope ends. Both charges are conservative.
pub(super) struct RootEndpoint<'work> {
    endpoint: OwnedFd,
    root: process::Pid,
    process: process::Pid,
    thread: process::Pid,
    account: Ledger,
    budget_address: usize,
    _work: PhantomData<(&'work Work, Rc<()>)>,
}

impl<'work> RootEndpoint<'work> {
    const STORAGE: usize = size_of::<(Self, usize)>() + ENDPOINT_STORAGE;

    pub(super) fn new(endpoint: OwnedFd, b: &mut Budget<'work>) -> Result<(Self, usize)> {
        b.with_prepaid_scope(ENDPOINT_STORAGE, ENTRY, CHECK_WORK, CHECK_FRAME, |b| {
            let root = process::getppid()
                .ok_or_else(|| Error::rejected("root gate parent unavailable"))?;
            validate_endpoint(endpoint.as_fd(), root)?;
            Ok((
                Self {
                    endpoint,
                    root,
                    process: process::getpid(),
                    thread: rustix::thread::gettid(),
                    account: b.work_ledger_identity_v1(),
                    budget_address: b as *const Budget<'_> as usize,
                    _work: PhantomData,
                },
                Self::STORAGE,
            ))
        })
    }

    pub(super) fn handshake(
        &self,
        a: &Admission<'_>,
        manifest: &Manifest,
        deadline: Instant,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = Self::STORAGE
            .checked_add(a.retained_storage())
            .and_then(|n| n.checked_add(manifest.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, ENTRY, GATE_WORK, GATE_FRAME, |b| {
            a.validate_continuity(b)?;
            readiness::check_binding(a, manifest, b)?;
            let mut attempts = 0;
            let bytes = loop {
                if let Some(bytes) = self.receive(deadline, &mut attempts, b)? {
                    break bytes;
                }
                self.pause(deadline, false, b)?;
            };
            b.reserve_storage(BYTES)?;
            let (request, charge) = Record::decode(&bytes, b).map_err(codec_error)?;
            b.reserve_storage(charge.additional_storage())?;
            let () =
                validate_gate_request(&request, &a.policy, manifest, b).map_err(codec_error)?;
            a.validate_continuity(b)?;
            readiness::check_binding(a, manifest, b)?;
            let (reply, charge) = gate_reply(&request, b).map_err(codec_error)?;
            b.reserve_storage(charge.additional_storage())?;
            loop {
                if self
                    .send(reply.canonical_bytes(), deadline, &mut attempts, b)?
                    .is_some()
                {
                    break;
                }
                self.pause(deadline, true, b)?;
            }
            a.validate_continuity(b)?;
            readiness::check_binding(a, manifest, b)?;
            self.revalidate(deadline, b)
        })
    }

    fn receive(
        &self,
        deadline: Instant,
        attempts: &mut usize,
        b: &mut Budget<'_>,
    ) -> Result<Option<[u8; BYTES]>> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, PACKET_WORK, PACKET_FRAME, |b| {
            self.check(b)?;
            permit_attempt(deadline, attempts)?;
            transport::receive_authenticated_packet(
                self.endpoint.as_fd(),
                transport::MessageSender::new(self.root.as_raw_nonzero().get(), 0, 0),
            )
            .map_err(packet_error)
        })
    }

    fn send(
        &self,
        bytes: &[u8; BYTES],
        deadline: Instant,
        attempts: &mut usize,
        b: &mut Budget<'_>,
    ) -> Result<Option<()>> {
        b.with_prepaid_scope(
            Self::STORAGE + BYTES,
            ENTRY,
            PACKET_WORK,
            PACKET_FRAME,
            |b| {
                self.check(b)?;
                permit_attempt(deadline, attempts)?;
                transport::send_packet(self.endpoint.as_fd(), bytes).map_err(packet_error)
            },
        )
    }

    fn pause(&self, deadline: Instant, writable: bool, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, CHECK_WORK, CHECK_FRAME, |b| {
            self.check(b)?;
            let remaining = remaining(deadline)?;
            let timeout = event::Timespec::try_from(remaining.min(POLL_INTERVAL))
                .map_err(|_| Error::rejected("root gate pause overflow"))?;
            let flags = if writable {
                event::PollFlags::OUT
            } else {
                event::PollFlags::IN
            };
            let mut fds = [event::PollFd::new(&self.endpoint, flags)];
            match event::poll(&mut fds, Some(&timeout)) {
                Ok(_) => {
                    if fds[0].revents().intersects(
                        event::PollFlags::ERR | event::PollFlags::HUP | event::PollFlags::NVAL,
                    ) {
                        return Err(Error::rejected("root gate endpoint closed"));
                    }
                    Ok(())
                }
                Err(io::Errno::INTR) => Ok(()),
                Err(e) => Err(e.into()),
            }
        })
    }

    fn revalidate(&self, deadline: Instant, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, CHECK_WORK, CHECK_FRAME, |b| {
            self.check(b)?;
            remaining(deadline)?;
            Ok(())
        })
    }

    fn check(&self, b: &Budget<'_>) -> Result<()> {
        if self.account != b.work_ledger_identity_v1()
            || self.budget_address != b as *const Budget<'_> as usize
            || self.process != process::getpid()
            || self.thread != rustix::thread::gettid()
        {
            return Err(Resource::Accounting.into());
        }
        if process::getppid() != Some(self.root) {
            return Err(Error::rejected("root gate parent changed"));
        }
        validate_endpoint(self.endpoint.as_fd(), self.root)
    }
}

fn validate_endpoint(fd: BorrowedFd<'_>, root: process::Pid) -> Result<()> {
    let unnamed: net::SocketAddrAny = net::SocketAddrUnix::new_unnamed().into();
    if io::fcntl_getfd(fd)? != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(fd)? != (fs::OFlags::RDWR | fs::OFlags::NONBLOCK)
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
        || !net::sockopt::socket_passcred(fd)?
        || net::getsockname(fd)? != unnamed
        || net::getpeername(fd)? != Some(unnamed)
    {
        return Err(Error::rejected("root gate endpoint shape"));
    }
    let peer = net::sockopt::socket_peercred(fd)?;
    if peer.pid != root || peer.uid.as_raw() != 0 || peer.gid.as_raw() != 0 {
        return Err(Error::rejected(
            "root gate requires actual root parent socket creator",
        ));
    }
    Ok(())
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero() && *remaining <= TIMEOUT)
        .ok_or_else(|| Error::rejected("root gate deadline expired or invalid"))
}

fn permit_attempt(deadline: Instant, attempts: &mut usize) -> Result<()> {
    remaining(deadline)?;
    if *attempts >= MAX_ATTEMPTS {
        return Err(Error::rejected("root gate attempt limit exhausted"));
    }
    *attempts += 1;
    Ok(())
}

fn codec_error(error: CodecError) -> Error {
    match error {
        CodecError::Resource(e) => e.into(),
        _ => Error::rejected("root gate record rejected"),
    }
}

fn packet_error(error: transport::Failure) -> Error {
    match error {
        transport::Failure::Io { source, .. } => source.into(),
        _ => Error::rejected("root gate packet framing or credentials"),
    }
}

#[cfg(test)]
#[path = "compiler_execution_issuer_root_control_v3_tests.rs"]
mod tests;
