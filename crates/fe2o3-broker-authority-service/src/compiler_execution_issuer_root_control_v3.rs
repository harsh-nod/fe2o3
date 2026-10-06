//! Concrete issuer gate after readiness EOF; no public validator or admission substitute.
use super::{Admission, Budget, Error, Manifest, Resource, Result, readiness};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3 as BYTES,
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionRootControlErrorV3 as CodecError, CompilerExecutionRootControlKindV3 as Kind,
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
    cell::RefCell,
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

struct Established {
    gate: Record,
    deadline: Instant,
    attempts: usize,
    next_sequence: u64,
}

impl Established {
    fn following_sequence(&self) -> Result<u64> {
        self.next_sequence
            .checked_add(1)
            .ok_or_else(|| Resource::Arithmetic.into())
    }
}

enum ConnectionState {
    Fresh,
    Established(Established),
    Failed,
}

impl ConnectionState {
    fn begin_handshake(&mut self, b: &mut Budget<'_>) -> Result<()> {
        let entry = b.charge_work(ENTRY);
        let previous = std::mem::replace(self, Self::Failed);
        entry?;
        if !matches!(previous, Self::Fresh) {
            return Err(Error::rejected("root control handshake already attempted"));
        }
        Ok(())
    }

    fn take_established(&mut self, b: &mut Budget<'_>) -> Result<Established> {
        let entry = b.charge_work(ENTRY);
        let previous = std::mem::replace(self, Self::Failed);
        entry?;
        match previous {
            Self::Established(session) => Ok(session),
            Self::Fresh | Self::Failed => Err(Error::rejected("root control session unavailable")),
        }
    }
}

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
    state: RefCell<ConnectionState>,
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
                    state: RefCell::new(ConnectionState::Fresh),
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
        self.begin_handshake(b)?;
        let floor = Self::STORAGE
            .checked_add(a.retained_storage())
            .and_then(|n| n.checked_add(manifest.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        let established = b.with_prepaid_scope(floor, 0, GATE_WORK - ENTRY, GATE_FRAME, |b| {
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
            self.revalidate(deadline, b)?;
            Ok::<_, Error>(Established {
                gate: request,
                deadline,
                attempts,
                // The issuer-to-root request stream has its own sequence. The
                // root's sequence-one admission challenge is the reverse flow.
                next_sequence: 1,
            })
        })?;
        *self
            .state
            .try_borrow_mut()
            .map_err(|_| Error::rejected("root control reentry"))? =
            ConnectionState::Established(established);
        Ok(())
    }

    fn begin_handshake(&self, b: &mut Budget<'_>) -> Result<()> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| Error::rejected("root control reentry"))?;
        // Refusal or unwind cannot reopen an unauthenticated startup interval.
        state.begin_handshake(b)
    }

    /// Private authenticated exchange only; decoding its result does not grant
    /// occurrence or retirement authority. The concrete session must admit the
    /// operation payload and exact durable join before any authority-bearing use.
    /// All calls share the original handshake deadline and cumulative attempts.
    /// An error or unwind permanently poisons this endpoint; no replay with a
    /// renewed account, sequence or deadline is exposed.
    pub(super) fn exchange(
        &self,
        a: &Admission<'_>,
        manifest: &Manifest,
        kind: Kind,
        payload: &[u8],
        b: &mut Budget<'_>,
    ) -> Result<(Record, Storage)> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| Error::rejected("root control reentry"))?;
        let mut session = state.take_established(b)?;
        let floor = Self::STORAGE
            .checked_add(a.retained_storage())
            .and_then(|n| n.checked_add(manifest.retained_storage()))
            .and_then(|n| n.checked_add(payload.len().min(BYTES)))
            .ok_or(Resource::Arithmetic)?;
        let next = session.following_sequence()?;
        let result = b.with_prepaid_scope(floor, 0, GATE_WORK - ENTRY, GATE_FRAME, |b| {
            self.revalidate(session.deadline, b)?;
            a.validate_continuity(b)?;
            readiness::check_binding(a, manifest, b)?;
            if !session
                .gate
                .matches_launch(&a.policy, manifest, b)
                .map_err(codec_error)?
            {
                return Err(Error::rejected(
                    "root control exchange changed original admission",
                ));
            }
            let (request, charge) = session
                .gate
                .request_on_same_connection(session.next_sequence, kind, payload, b)
                .map_err(codec_error)?;
            b.reserve_storage(charge.additional_storage())?;
            loop {
                a.validate_continuity(b)?;
                if self
                    .send(
                        request.canonical_bytes(),
                        session.deadline,
                        &mut session.attempts,
                        b,
                    )?
                    .is_some()
                {
                    break;
                }
                self.pause(session.deadline, true, b)?;
            }
            let bytes = loop {
                a.validate_continuity(b)?;
                if let Some(bytes) = self.receive(session.deadline, &mut session.attempts, b)? {
                    break bytes;
                }
                self.pause(session.deadline, false, b)?;
            };
            b.reserve_storage(BYTES)?;
            let (reply, charge) = Record::decode(&bytes, b).map_err(codec_error)?;
            b.reserve_storage(charge.additional_storage())?;
            if !reply.matches_reply(&request, b).map_err(codec_error)? {
                return Err(Error::rejected(
                    "root control reply changed original request",
                ));
            }
            a.validate_continuity(b)?;
            self.revalidate(session.deadline, b)?;
            Ok::<_, Error>((reply, charge))
        })?;
        session.next_sequence = next;
        *state = ConnectionState::Established(session);
        Ok(result)
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
    use crate::compiler_execution_root_channel::socketpair_address;
    if io::fcntl_getfd(fd)? != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(fd)? != (fs::OFlags::RDWR | fs::OFlags::NONBLOCK)
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
        || !net::sockopt::socket_passcred(fd)?
        || !socketpair_address(net::getsockname(fd)?)
        || !net::getpeername(fd)?.is_some_and(socketpair_address)
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
