//! Shared peer syscalls and packet validation; the caller owns accounting and storage.
//!
//! A fallible charge runs before validation and every poll, receive or send attempt,
//! including retries. No ledger is created or reset here. A finite caller work budget
//! bounds attempts, not time blocked in a poll; legacy callers supply a no-op charge.

use std::io::{self, IoSliceMut};
use std::mem::{MaybeUninit, size_of};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::time::{Duration, Instant};

use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvFlags, ReturnFlags, SendFlags, SocketType, recvmsg,
    send,
};

use crate::service::ExternalAnchorDaemonErrorV1 as DaemonError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IoStep {
    Validate,
    Poll,
    Receive,
    Send,
}

impl IoStep {
    /// Fixed logical work, not elapsed syscall time or an allocation/stack bound.
    pub(crate) const fn work(self) -> usize {
        match self {
            // Two fcntls, domain/type queries and two fixed sockaddr name queries.
            Self::Validate => 8 + 6 * 1024 + 32 * (2 * size_of::<libc::sockaddr_un>()),
            Self::Poll => 8 + 1024,
            Self::Receive => 8 + 1024 + 32 * ANCHOR_CHALLENGE_WIRE_LEN_V1,
            Self::Send => 8 + 1024 + 32 * ANCHOR_OBSERVATION_WIRE_LEN_V1,
        }
    }
}

pub(crate) fn validate_peer<E: From<DaemonError>>(
    peer: &OwnedFd,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    charge(IoStep::Validate)?;
    let descriptor_flags = rustix::io::fcntl_getfd(peer)
        .map_err(|source| io_error("inspect external-anchor peer descriptor flags", source))?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(DaemonError::PeerNotCloseOnExec.into());
    }
    let status = rustix::fs::fcntl_getfl(peer)
        .map_err(|source| io_error("inspect external-anchor peer status flags", source))?;
    if status != OFlags::RDWR | OFlags::NONBLOCK {
        return Err(DaemonError::InvalidPeerStatus.into());
    }
    let domain = rustix::net::sockopt::socket_domain(peer)
        .map_err(|source| io_error("inspect external-anchor peer domain", source))?;
    if domain != AddressFamily::UNIX {
        return Err(DaemonError::InvalidPeerDomain.into());
    }
    let socket_type = rustix::net::sockopt::socket_type(peer)
        .map_err(|source| io_error("inspect external-anchor peer socket type", source))?;
    if socket_type != SocketType::SEQPACKET {
        return Err(DaemonError::InvalidPeerSocketType.into());
    }
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Local)?;
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Remote)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum AddressSideV1 {
    Local,
    Remote,
}

fn require_unnamed(peer: RawFd, side: AddressSideV1) -> Result<(), DaemonError> {
    let mut address = MaybeUninit::<libc::sockaddr_un>::zeroed();
    let mut length = libc::socklen_t::try_from(size_of::<libc::sockaddr_un>())
        .expect("sockaddr_un size fits socklen_t");
    // SAFETY: the address buffer is writable for its declared size, `length` is initialized to
    // that size, and `peer` remains owned by the caller throughout this inspection.
    let result = unsafe {
        match side {
            AddressSideV1::Local => libc::getsockname(
                peer,
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
            AddressSideV1::Remote => libc::getpeername(
                peer,
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
        }
    };
    if result != 0 {
        let source = io::Error::last_os_error();
        return if matches!(side, AddressSideV1::Remote)
            && matches!(source.raw_os_error(), Some(libc::ENOTCONN))
        {
            Err(DaemonError::PeerNotConnected)
        } else {
            Err(DaemonError::Io {
                operation: match side {
                    AddressSideV1::Local => "inspect external-anchor local address",
                    AddressSideV1::Remote => "inspect external-anchor remote address",
                },
                source,
            })
        };
    }
    // SAFETY: a successful name query initialized at least the family field and the buffer was
    // zeroed before the kernel wrote it.
    let address = unsafe { address.assume_init() };
    if i32::from(address.sun_family) != libc::AF_UNIX {
        return Err(DaemonError::InvalidPeerDomain);
    }
    let unnamed_length = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    if usize::try_from(length).ok() == Some(unnamed_length) {
        Ok(())
    } else {
        Err(match side {
            AddressSideV1::Local => DaemonError::NamedLocalAddress,
            AddressSideV1::Remote => DaemonError::NamedRemoteAddress,
        })
    }
}

pub(crate) fn receive_challenge<E: From<DaemonError>>(
    peer: &OwnedFd,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, E> {
    loop {
        wait_for(peer, PollFlags::IN, None, charge)?;
        charge(IoStep::Receive)?;
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        let mut vectors = [IoSliceMut::new(&mut bytes)];
        let mut ancillary = RecvAncillaryBuffer::default();
        match recvmsg(
            peer,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::TRUNC | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(message) => {
                if message.flags.contains(ReturnFlags::CTRUNC) || ancillary.drain().next().is_some()
                {
                    return Err(DaemonError::AncillaryData.into());
                }
                if message.flags.contains(ReturnFlags::TRUNC)
                    || message.bytes > ANCHOR_CHALLENGE_WIRE_LEN_V1
                {
                    return Err(DaemonError::PacketTruncated.into());
                }
                if message.bytes == 0 {
                    return Ok(None);
                }
                if message.bytes != ANCHOR_CHALLENGE_WIRE_LEN_V1 {
                    return Err(DaemonError::InvalidChallengeLength {
                        actual: message.bytes,
                    }
                    .into());
                }
                return Ok(Some(bytes));
            }
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN) => return Ok(None),
            Err(source) => {
                return Err(io_error("receive external-anchor challenge", source).into());
            }
        }
    }
}

pub(crate) fn send_observation<E: From<DaemonError>>(
    peer: &OwnedFd,
    observation: &[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    timeout: Duration,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(DaemonError::DeadlineOverflow)?;
    loop {
        wait_for(peer, PollFlags::OUT, Some(deadline), charge)?;
        charge(IoStep::Send)?;
        match send(peer, observation, SendFlags::DONTWAIT | SendFlags::NOSIGNAL) {
            Ok(count) if count == observation.len() => return Ok(()),
            Ok(_) => return Err(DaemonError::PartialSend.into()),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(
                rustix::io::Errno::PIPE | rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN,
            ) => {
                return Err(DaemonError::PeerClosed.into());
            }
            Err(source) => return Err(io_error("send external-anchor observation", source).into()),
        }
    }
}

fn wait_for<E: From<DaemonError>>(
    peer: &OwnedFd,
    wanted: PollFlags,
    deadline: Option<Instant>,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    loop {
        charge(IoStep::Poll)?;
        let timeout = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(DaemonError::ResponseTimeout.into());
                }
                Some(
                    rustix::event::Timespec::try_from(remaining)
                        .map_err(|_| DaemonError::DeadlineOverflow)?,
                )
            }
            None => None,
        };
        let mut descriptors = [PollFd::new(
            peer,
            wanted | PollFlags::ERR | PollFlags::HUP | PollFlags::RDHUP,
        )];
        match poll(&mut descriptors, timeout.as_ref()) {
            Ok(0) => return Err(DaemonError::ResponseTimeout.into()),
            Ok(_) => {
                let ready = descriptors[0].revents();
                if ready.contains(PollFlags::NVAL) {
                    return Err(DaemonError::InvalidPeer.into());
                }
                if ready.contains(PollFlags::ERR) {
                    return Err(DaemonError::PeerFailed.into());
                }
                if ready.contains(wanted) {
                    return Ok(());
                }
                if ready.intersects(PollFlags::HUP | PollFlags::RDHUP) {
                    if wanted == PollFlags::IN {
                        return Ok(());
                    }
                    return Err(DaemonError::PeerClosed.into());
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(io_error("poll external-anchor peer", source).into()),
        }
    }
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> DaemonError {
    DaemonError::Io {
        operation,
        source: io::Error::from(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::fs::File;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Debug)]
    enum TestError {
        Daemon(DaemonError),
        Resource(Resource),
    }

    impl From<DaemonError> for TestError {
        fn from(error: DaemonError) -> Self {
            Self::Daemon(error)
        }
    }

    fn charge(
        budget: &mut Budget<'_>,
        attempts: &mut Vec<IoStep>,
        step: IoStep,
    ) -> Result<(), TestError> {
        attempts.push(step);
        budget.charge_work(step.work()).map_err(TestError::Resource)
    }

    #[test]
    fn fixed_step_work_is_const_and_positive() {
        const COSTS: [usize; 4] = [
            IoStep::Validate.work(),
            IoStep::Poll.work(),
            IoStep::Receive.work(),
            IoStep::Send.work(),
        ];
        assert_eq!(COSTS[0], 6152 + 64 * size_of::<libc::sockaddr_un>());
        assert_eq!(&COSTS[1..], &[1032, 6920, 10248]);
        assert!(COSTS.into_iter().all(|work| work > 0));
    }

    #[test]
    fn validation_one_short_precedes_descriptor_refusal_and_exact_charge_converts_it() {
        // This read-only non-socket descriptor must fail status validation before socket queries.
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        for one_short in [false, true] {
            let mut work = Work::new(17 + IoStep::Validate.work() - usize::from(one_short));
            let mut budget = Budget::new(&mut work, 64);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(64).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let mut attempts = Vec::new();
            let result = validate_peer(&peer, &mut |step| charge(&mut budget, &mut attempts, step));
            if one_short {
                assert!(matches!(
                    result,
                    Err(TestError::Resource(Resource::Work(_)))
                ));
                assert_eq!(budget.work(), 17);
                assert_eq!(budget.failed_work(), Some(17 + IoStep::Validate.work()));
            } else {
                assert!(matches!(
                    result,
                    Err(TestError::Daemon(DaemonError::InvalidPeerStatus))
                ));
                assert_eq!(budget.work(), 17 + IoStep::Validate.work());
                assert_eq!(budget.failed_work(), None);
            }
            assert_eq!(attempts, [IoStep::Validate]);
            assert_eq!(budget.storage(), 64);
            assert_eq!(budget.peak_storage(), 64);
            assert!(ledger == budget.work_ledger_identity_v1());
        }
    }

    #[test]
    fn receive_and_send_poll_precharge_refusals_are_terminal() {
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        for sending in [false, true] {
            let mut work = Work::new(IoStep::Poll.work() - 1);
            let mut budget = Budget::new(&mut work, 0);
            let mut attempts = Vec::new();
            let mut meter = |step| charge(&mut budget, &mut attempts, step);
            let result = if sending {
                send_observation(
                    &peer,
                    &[0; ANCHOR_OBSERVATION_WIRE_LEN_V1],
                    Duration::from_secs(30),
                    &mut meter,
                )
            } else {
                receive_challenge(&peer, &mut meter).map(|_| ())
            };
            assert!(matches!(
                result,
                Err(TestError::Resource(Resource::Work(_)))
            ));
            assert_eq!(attempts, [IoStep::Poll]);
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.failed_work(), Some(IoStep::Poll.work()));
        }
    }

    #[test]
    fn ready_file_receive_and_send_refuse_before_socket_syscalls() {
        // A regular file polls ready, but neither recvmsg nor send may be reached here.
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        for operation in [IoStep::Receive, IoStep::Send] {
            let mut work = Work::new(IoStep::Poll.work() + operation.work() - 1);
            let mut budget = Budget::new(&mut work, 64);
            budget.reserve_storage(64).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let mut attempts = Vec::new();
            let mut meter = |step| charge(&mut budget, &mut attempts, step);
            let result = if operation == IoStep::Send {
                send_observation(
                    &peer,
                    &[0; ANCHOR_OBSERVATION_WIRE_LEN_V1],
                    Duration::from_secs(30),
                    &mut meter,
                )
            } else {
                receive_challenge(&peer, &mut meter).map(|_| ())
            };
            assert!(matches!(
                result,
                Err(TestError::Resource(Resource::Work(_)))
            ));
            assert_eq!(attempts, [IoStep::Poll, operation]);
            assert_eq!(budget.work(), IoStep::Poll.work());
            assert_eq!(
                budget.failed_work(),
                Some(IoStep::Poll.work() + operation.work())
            );
            assert_eq!(budget.storage(), 64);
            assert_eq!(budget.peak_storage(), 64);
            assert!(ledger == budget.work_ledger_identity_v1());
        }
    }

    #[test]
    fn every_unsatisfied_poll_attempt_charges_the_original_ledger() {
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        let mut work = Work::new(17 + 2 * IoStep::Poll.work());
        let mut budget = Budget::new(&mut work, 128);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(128).unwrap();
        budget.release_storage(64).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let denials = (budget.failed_work(), budget.failed_storage());
        let ledger = budget.work_ledger_identity_v1();
        let mut attempts = Vec::new();
        // Regular files report IN, not PRI. Exercise the actual repeated-readiness branch,
        // not an injected syscall or a claim to cover socket EINTR/EAGAIN retries.
        let result = wait_for(&peer, PollFlags::IN | PollFlags::PRI, None, &mut |step| {
            charge(&mut budget, &mut attempts, step)
        });
        assert!(matches!(
            result,
            Err(TestError::Resource(Resource::Work(_)))
        ));
        assert_eq!(attempts, [IoStep::Poll; 3]);
        assert_eq!(budget.work(), 17 + 2 * IoStep::Poll.work());
        assert_eq!(budget.storage(), 64);
        assert_eq!(budget.peak_storage(), 128);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
        assert!(ledger == budget.work_ledger_identity_v1());
    }

    #[test]
    fn expired_deadline_checks_follow_poll_precharge() {
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        for one_short in [false, true] {
            let mut work = Work::new(IoStep::Poll.work() - usize::from(one_short));
            let mut budget = Budget::new(&mut work, 0);
            let mut attempts = Vec::new();
            let result = wait_for(&peer, PollFlags::OUT, Some(Instant::now()), &mut |step| {
                charge(&mut budget, &mut attempts, step)
            });
            if one_short {
                assert!(matches!(
                    result,
                    Err(TestError::Resource(Resource::Work(_)))
                ));
                assert_eq!(budget.work(), 0);
            } else {
                assert!(matches!(
                    result,
                    Err(TestError::Daemon(DaemonError::ResponseTimeout))
                ));
                assert_eq!(budget.work(), IoStep::Poll.work());
            }
            assert_eq!(attempts, [IoStep::Poll]);
        }
    }

    #[test]
    fn zero_send_timeout_charges_poll_but_never_send() {
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        let mut work = Work::new(IoStep::Poll.work());
        let mut budget = Budget::new(&mut work, 0);
        let mut attempts = Vec::new();
        let result = send_observation(
            &peer,
            &[0; ANCHOR_OBSERVATION_WIRE_LEN_V1],
            Duration::ZERO,
            &mut |step| charge(&mut budget, &mut attempts, step),
        );
        assert!(matches!(
            result,
            Err(TestError::Daemon(DaemonError::ResponseTimeout))
        ));
        assert_eq!(attempts, [IoStep::Poll]);
        assert_eq!(budget.work(), IoStep::Poll.work());
        assert_eq!(budget.failed_work(), None);
    }

    #[test]
    fn send_deadline_overflow_preserves_legacy_precharge_precedence() {
        let peer: OwnedFd = File::open("/dev/null").unwrap().into();
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 0);
        let mut attempts = Vec::new();
        let result = send_observation(
            &peer,
            &[0; ANCHOR_OBSERVATION_WIRE_LEN_V1],
            Duration::MAX,
            &mut |step| charge(&mut budget, &mut attempts, step),
        );
        assert!(matches!(
            result,
            Err(TestError::Daemon(DaemonError::DeadlineOverflow))
        ));
        assert!(attempts.is_empty());
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.failed_work(), None);
    }

    #[test]
    fn send_charge_unwind_keeps_accepted_work_and_borrowed_descriptor() {
        let peer: OwnedFd = tempfile::tempfile().unwrap().into();
        let mut work = Work::new(IoStep::Poll.work() + IoStep::Send.work());
        let mut budget = Budget::new(&mut work, 64);
        budget.reserve_storage(64).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut attempts = Vec::new();
        let result = catch_unwind(AssertUnwindSafe(|| {
            send_observation(
                &peer,
                &[0; ANCHOR_OBSERVATION_WIRE_LEN_V1],
                Duration::from_secs(30),
                &mut |step| {
                    charge(&mut budget, &mut attempts, step)?;
                    assert_ne!(step, IoStep::Send, "injected pre-send charge unwind");
                    Ok::<(), TestError>(())
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(attempts, [IoStep::Poll, IoStep::Send]);
        assert_eq!(budget.work(), IoStep::Poll.work() + IoStep::Send.work());
        assert_eq!(budget.storage(), 64);
        assert_eq!(budget.peak_storage(), 64);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert!(
            rustix::io::fcntl_getfd(&peer)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
}
