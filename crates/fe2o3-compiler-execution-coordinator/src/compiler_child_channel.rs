//! Private root-side custody of a native compiler's child-created endpoint.
//! This does not enter the same-UID public handoff protocol or admit a compiler.

use crate::native_launch::{self as native, CompilerExecutionLaunchErrorV2 as Error, Observer};
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1 as Identity;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::ProtectedServiceCredentialProfileV1 as Credentials;
use fe2o3_protected_service_spawn::{
    compiler_service_channel::{TRANSFER_BYTES, encode_transfer, transfer_matches},
    launch_io::{self, MessageSender},
    native_spawn::RootOwnedRetainedServiceChildV2 as Child,
};
use rustix::{event, fs, io::FdFlags, net};
use std::{
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    time::Instant,
};

type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;
const LOCAL_WORK: usize = ENTRY + 32 * 1088 + 64 * TRANSFER_BYTES;
const FRAME: usize = launch_io::ATTEMPT_SCRATCH
    + 4 * size_of::<(CompilerChildChannel, usize)>()
    + 8 * size_of::<Error>()
    + 4096;

/// Inert descriptors joined to the original clone-owned child, not launch authority.
/// The caller retains the actual child/backing and sole wait ownership separately.
/// No constructor accepts a numeric PID, caller-created pidfd or public handoff.
struct CompilerChildChannel {
    service_peer: OwnedFd,
    client_pidfd: OwnedFd,
    client: Identity,
    retained: usize,
}

impl CompilerChildChannel {
    const ENVELOPE: usize = size_of::<(Self, usize)>() - 2 * size_of::<OwnedFd>();

    /// Receives once while the native child is held behind its first-exec gate.
    /// `credentials` must come from the owning attempt's admitted profile, not
    /// from the wire. SO_PASSCRED must be enabled before clone. The receiver and
    /// the complete child/backing remain prepaid on their original budget.
    /// Returned charge is FULL and unreserved; retire the consumed receiver's
    /// charge only after this call and reserve the result before retaining it.
    /// Work/deadline/shape failures close every received descriptor; the caller
    /// remains responsible for cancelling the same child in its original pool.
    fn receive<T: Send + 'static>(
        child: &Child<T>,
        receiver: OwnedFd,
        credentials: Credentials,
        deadline: Instant,
        b: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let floor = child
            .retained_storage()
            .checked_add(native::FILE_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
            if Instant::now() >= deadline {
                return Err(launch_io::Failure::Timeout("compiler channel entry").into());
            }
            let pid = native::pid_u32(child.pid())?;
            let parent = native::pid_u32(rustix::process::getpid())?;
            if !net::sockopt::socket_passcred(&receiver)
                .map_err(|e| native::io("inspect compiler transfer credentials", e))?
            {
                return Err(Error::Invalid("compiler transfer credentials disabled"));
            }
            require_live(child, b)?;
            // Duplicate the original atomic-clone pidfd. Never resolve pid to a
            // new pidfd, and never accept one supplied by a message or submitter.
            let (client_pidfd, charge) = child.try_clone_pidfd(b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (bytes, service_peer) = launch_io::receive_ready_from::<TRANSFER_BYTES, true, _>(
                receiver.as_fd(),
                MessageSender::new(
                    child.pid().as_raw_pid(),
                    credentials.uid(),
                    credentials.gid(),
                ),
                &mut Observer { child, budget: b },
                deadline,
            )?;
            let service_peer = service_peer.ok_or(Error::Invalid("missing compiler endpoint"))?;
            b.reserve_storage(native::FILE_STORAGE)?;
            let client = Identity::new(pid, credentials.uid(), credentials.gid())
                .map_err(|_| Error::Invalid("invalid compiler service identity"))?;
            validate_transfer(&bytes, service_peer.as_fd(), client, parent)?;
            require_idle(client_pidfd.as_fd())?;
            require_live(child, b)?;
            if Instant::now() >= deadline {
                return Err(
                    launch_io::Failure::Timeout("compiler channel final validation").into(),
                );
            }
            let retained = native::sum(&[
                charge.additional_storage(),
                native::FILE_STORAGE,
                Self::ENVELOPE,
            ])?;
            b.reserve_storage(Self::ENVELOPE)?;
            Ok((
                Self {
                    service_peer,
                    client_pidfd,
                    client,
                    retained,
                },
                retained,
            ))
        })
    }

    /// Rechecks the original escrow, not a caller-supplied replacement endpoint.
    fn revalidate(&self) -> Result<()> {
        let parent = native::pid_u32(rustix::process::getpid())?;
        let bytes = encode_transfer(self.client.pid(), parent)
            .ok_or(Error::Invalid("invalid compiler channel identities"))?;
        validate_transfer(&bytes, self.service_peer.as_fd(), self.client, parent)?;
        require_idle(self.client_pidfd.as_fd())
    }
}

#[path = "compiler_channel_trace.rs"]
mod trace;
pub(crate) use trace::CompilerTrace;

fn require_live<T: Send + 'static>(child: &Child<T>, b: &mut Budget<'_>) -> Result<()> {
    if !child.is_live(b)? {
        return Err(launch_io::Failure::ChildExited("compiler channel transfer").into());
    }
    Ok(())
}

// Only inert comparison and descriptor checks live here. Packet credentials are
// independently enforced by receive_ready_from before this function is called.
fn validate_transfer(
    bytes: &[u8; TRANSFER_BYTES],
    peer: BorrowedFd<'_>,
    expected: Identity,
    parent: u32,
) -> Result<()> {
    if !transfer_matches(bytes, expected.pid(), parent) {
        return Err(Error::Invalid("compiler channel record mismatch"));
    }
    if rustix::io::fcntl_getfd(peer).map_err(|e| native::io("inspect compiler peer FD", e))?
        != FdFlags::CLOEXEC
        || net::sockopt::socket_type(peer)
            .map_err(|e| native::io("inspect compiler peer type", e))?
            != net::SocketType::SEQPACKET
        || fs::fcntl_getfl(peer).map_err(|e| native::io("inspect compiler peer access", e))?
            & fs::OFlags::ACCMODE
            != fs::OFlags::RDWR
        || !unnamed(
            net::getsockname(peer).map_err(|e| native::io("inspect compiler peer address", e))?,
        )
        || net::getpeername(peer)
            .map_err(|e| native::io("inspect compiler peer connection", e))?
            .map(unnamed)
            != Some(true)
    {
        return Err(Error::Invalid("invalid compiler service endpoint"));
    }
    let actual = net::sockopt::socket_peercred(peer)
        .map_err(|e| native::io("inspect compiler service creator", e))?;
    if u32::try_from(actual.pid.as_raw_nonzero().get()).ok() != Some(expected.pid())
        || actual.uid.as_raw() != expected.uid()
        || actual.gid.as_raw() != expected.gid()
    {
        return Err(Error::Invalid("compiler service creator mismatch"));
    }
    require_idle(peer)?;
    Ok(())
}

fn unnamed(address: net::SocketAddrAny) -> bool {
    address == net::SocketAddrUnix::new_unnamed().into()
}

fn require_idle(fd: BorrowedFd<'_>) -> Result<()> {
    let mut fds = [event::PollFd::new(&fd, event::PollFlags::IN)];
    event::poll(
        &mut fds,
        Some(&event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|e| native::io("observe compiler channel liveness", e))?;
    if !fds[0].revents().is_empty() {
        return Err(Error::Invalid("compiler channel already ready or closed"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_child_channel_tests.rs"]
mod tests;
