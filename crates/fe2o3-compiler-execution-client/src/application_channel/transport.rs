use super::*;
use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
use fe2o3_runtime_protocol::WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1 as MAX_PACKET;
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    time::Instant,
};

pub(super) fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(ApplicationProofChannelErrorV1::Timeout)
    } else {
        Ok(())
    }
}

fn revalidate(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&ReceivedProcessPidfdV1],
    deadline: Instant,
) -> Result<()> {
    check_deadline(deadline)?;
    endpoint.revalidate()?;
    for process in processes {
        process.revalidate()?;
    }
    check_deadline(deadline)
}

fn wait(
    endpoint: &RetainedApplicationProofEndpointV1,
    events: i16,
    deadline: Instant,
) -> Result<()> {
    check_deadline(deadline)?;
    let millis = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .clamp(1, 100) as i32;
    let mut entry = libc::pollfd {
        fd: endpoint.peer.as_raw_fd(),
        events,
        revents: 0,
    };
    // SAFETY: one initialized pollfd; the wait is capped by the original deadline.
    if unsafe { libc::poll(&mut entry, 1, millis) } < 0 {
        let error = rustix::io::Errno::from_io_error(&io::Error::last_os_error())
            .unwrap_or(rustix::io::Errno::IO);
        if error != rustix::io::Errno::INTR {
            return Err(error.into());
        }
    } else if entry.revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
        return Err(invalid("application proof endpoint closed or failed"));
    }
    check_deadline(deadline)
}

pub(super) fn send(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&ReceivedProcessPidfdV1],
    bytes: &[u8],
    rights: &[BorrowedFd<'_>],
    deadline: Instant,
) -> Result<()> {
    send_with_attempt(endpoint, processes, bytes, rights, deadline, &mut || Ok(()))
}

pub(super) fn send_with_attempt(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&ReceivedProcessPidfdV1],
    bytes: &[u8],
    rights: &[BorrowedFd<'_>],
    deadline: Instant,
    before_attempt: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_PACKET || rights.len() > 2 {
        return Err(invalid("outgoing application proof packet bounds"));
    }
    loop {
        before_attempt()?;
        revalidate(endpoint, processes, deadline)?;
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !rights.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(rights)) {
            return Err(invalid("outgoing application proof descriptor roster"));
        }
        match sendmsg(
            &endpoint.peer,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(count) if count == bytes.len() => return revalidate(endpoint, processes, deadline),
            Ok(_) => return Err(invalid("partial application proof packet send")),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                wait(endpoint, libc::POLLOUT, deadline)?
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) struct Received {
    pub(super) bytes: Vec<u8>,
    pub(super) sender: CompilerExecutionClientProcessIdentityV1,
    pub(super) rights: Vec<OwnedFd>,
}

pub(super) enum ReceiveProfile {
    Registration,
    Proof,
    #[cfg(target_arch = "x86_64")]
    NativeRegistration,
    #[cfg(target_arch = "x86_64")]
    NativeProof,
    #[cfg(target_arch = "x86_64")]
    NativeStartup,
}

pub(super) fn receive(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&ReceivedProcessPidfdV1],
    profile: ReceiveProfile,
    deadline: Instant,
) -> Result<Received> {
    receive_with_attempt(endpoint, processes, profile, deadline, &mut || Ok(()))
}

pub(super) fn receive_with_attempt(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&ReceivedProcessPidfdV1],
    profile: ReceiveProfile,
    deadline: Instant,
    before_attempt: &mut impl FnMut() -> Result<()>,
) -> Result<Received> {
    let (max_bytes, max_rights) = match profile {
        ReceiveProfile::Registration => (
            fe2o3_runtime_protocol::WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1,
            1,
        ),
        ReceiveProfile::Proof => (MAX_PACKET, 2),
        #[cfg(target_arch = "x86_64")]
        ReceiveProfile::NativeRegistration => (
            fe2o3_runtime_protocol::NATIVE_APPLICATION_SESSION_MAX_BYTES_V1,
            1,
        ),
        #[cfg(target_arch = "x86_64")]
        ReceiveProfile::NativeProof => (
            fe2o3_runtime_protocol::NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1,
            0,
        ),
        #[cfg(target_arch = "x86_64")]
        ReceiveProfile::NativeStartup => (
            fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1,
            0,
        ),
    };
    loop {
        before_attempt()?;
        revalidate(endpoint, processes, deadline)?;
        let mut bytes = [0; MAX_PACKET];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = match recvmsg(
            &endpoint.peer,
            &mut [IoSliceMut::new(&mut bytes[..max_bytes])],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => received,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                wait(endpoint, libc::POLLIN, deadline)?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let mut credentials = None;
        let mut rights = Vec::with_capacity(2);
        let mut rights_messages = 0;
        let mut invalid_ancillary = false;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmCredentials(value) => {
                    if credentials.replace(value).is_some() {
                        invalid_ancillary = true;
                    }
                }
                RecvAncillaryMessage::ScmRights(value) => {
                    rights_messages += 1;
                    rights.extend(value);
                }
                _ => invalid_ancillary = true,
            }
        }
        if received.bytes == 0
            || received.bytes > max_bytes
            || !(received.flags - ReturnFlags::CMSG_CLOEXEC).is_empty()
            || invalid_ancillary
            || rights.len() > max_rights
            || rights_messages != usize::from(!rights.is_empty())
        {
            return Err(invalid(
                "application packet or ancillary roster is malformed",
            ));
        }
        let credentials =
            credentials.ok_or_else(|| invalid("application proof sender is absent"))?;
        let sender = CompilerExecutionClientProcessIdentityV1::new(
            credentials.pid.as_raw_nonzero().get() as u32,
            credentials.uid.as_raw(),
            credentials.gid.as_raw(),
        )
        .map_err(|_| invalid("invalid application proof sender"))?;
        revalidate(endpoint, processes, deadline)?;
        return Ok(Received {
            bytes: bytes[..received.bytes].to_vec(),
            sender,
            rights,
        });
    }
}
