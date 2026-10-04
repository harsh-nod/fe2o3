//! Application-first registration over the one inherited proof transport.

use super::*;
use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1, WorkerV3ApplicationInputOccurrenceV1,
    WorkerV3ApplicationRegistrationInputsV1, WorkerV3ApplicationSessionKindV1 as Kind,
    WorkerV3ApplicationSessionMessageV1 as Message,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendFlags, recvmsg, sendmsg,
};

const MAX_STARTUP: Duration = Duration::from_secs(120);

/// Retains the original application endpoint and authenticated root-process observation.
///
/// This is registration transport custody, not artifact verification, proof custody, GPU
/// authority, or completion. There is no conversion back to an unregistered endpoint.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RegisteredApplicationProofEndpointV1;
/// use std::os::fd::AsFd;
/// fn export(value: RegisteredApplicationProofEndpointV1) { let _ = value.as_fd(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RegisteredApplicationProofEndpointV1;
/// fn clone(value: RegisteredApplicationProofEndpointV1) { let _ = value.clone(); }
/// ```
#[derive(Debug)]
pub struct RegisteredApplicationProofEndpointV1 {
    endpoint: RetainedApplicationProofEndpointV1,
    root: ReceivedProcessPidfdV1,
    sender: CompilerExecutionClientProcessIdentityV1,
    transcript: Transcript,
}

impl RegisteredApplicationProofEndpointV1 {
    pub fn revalidate(&self) -> Result<()> {
        self.endpoint.revalidate()?;
        self.root.revalidate()?;
        if self.sender.pid() != self.root.pid() || self.sender.uid() != 0 || self.sender.gid() != 0
        {
            return Err(invalid("registered root process association changed"));
        }
        self.endpoint.revalidate()
    }

    pub const fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.endpoint.descriptor_identity()
    }

    /// Inert transcript equality data, never a proof or a readiness capability.
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
}

impl RetainedApplicationProofEndpointV1 {
    /// Completes Hello/Challenge/Accept/Ready before the caller may acknowledge startup.
    ///
    /// Uses only the inherited transport and the root's received original pidfd. No socket,
    /// subprocess, pidfd reopening, waitid, shutdown or signal operation is performed.
    /// Failure consumes and closes the endpoint. One absolute deadline covers every phase.
    /// Cargo/supervisor must install the application registration route before activating this.
    pub fn register_pre_ack(
        self,
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        deadline: Instant,
    ) -> Result<RegisteredApplicationProofEndpointV1> {
        check_deadline(deadline)?;
        if deadline.saturating_duration_since(Instant::now()) > MAX_STARTUP {
            return Err(invalid(
                "application registration deadline exceeds startup bound",
            ));
        }
        self.revalidate()?;
        if rustix::process::geteuid().as_raw() == 0 {
            return Err(invalid(
                "application registration requires an unprivileged application",
            ));
        }
        let app_nonce = fresh_nonce(deadline)?;
        let hello = Message::hello(inputs.clone(), app_nonce)
            .map_err(|_| invalid("invalid local registration inputs"))?;
        send(&self, None, &hello, deadline)?;
        let mut challenge = receive(&self, None, deadline)?;
        if challenge.message.kind() != Kind::Challenge
            || challenge.message.app_nonce() != app_nonce
            || challenge.sender.uid() != 0
            || challenge.sender.gid() != 0
            || challenge.sender.pid() == self.application_pid
        {
            return Err(invalid("registration Challenge sender or phase mismatch"));
        }
        let binding = challenge
            .message
            .registration()
            .ok_or_else(|| invalid("Challenge omitted registration binding"))?;
        let handoff = binding.compiler_handoff();
        let (device, inode, mode) = self.snapshot.object;
        let proof_input =
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, device, inode, mode)
                .map_err(|_| invalid("invalid retained proof endpoint occurrence"))?;
        if !inputs.matches_binding(binding)
            || handoff.launch_manifest().client() != current_creator(self.application_pid)?
            || handoff.submitter() != self.snapshot.creator
            || binding.descriptors().as_array()[3] != self.peer.as_raw_fd()
            || binding.occurrence().inputs().get(3) != Some(&proof_input)
        {
            return Err(invalid(
                "Challenge differs from original application and Cargo inputs",
            ));
        }
        let root = ReceivedProcessPidfdV1::admit_received(
            challenge
                .rights
                .pop()
                .ok_or_else(|| invalid("Challenge omitted root pidfd"))?,
            challenge.sender.pid(),
        )?;
        let transcript = challenge
            .message
            .transcript()
            .ok_or_else(|| invalid("Challenge omitted transcript"))?;
        send(&self, Some(&root), &Message::accept(transcript), deadline)?;
        let ready = receive(&self, Some(&root), deadline)?;
        if ready.message.kind() != Kind::Ready
            || ready.sender != challenge.sender
            || ready.message.transcript() != Some(transcript)
        {
            return Err(invalid("registration Ready sender or transcript mismatch"));
        }
        let registered = RegisteredApplicationProofEndpointV1 {
            endpoint: self,
            root,
            sender: challenge.sender,
            transcript,
        };
        registered.revalidate()?;
        check_deadline(deadline)?;
        Ok(registered)
    }
}

fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(ApplicationProofChannelErrorV1::Timeout)
    } else {
        Ok(())
    }
}

fn fresh_nonce(deadline: Instant) -> Result<[u8; 32]> {
    loop {
        check_deadline(deadline)?;
        let mut nonce = [0; 32];
        // SAFETY: the output is writable for exactly its declared length. NONBLOCK prevents
        // entropy initialization from defeating the caller's absolute deadline.
        let count =
            unsafe { libc::getrandom(nonce.as_mut_ptr().cast(), nonce.len(), libc::GRND_NONBLOCK) };
        if count == -1 {
            let error = rustix::io::Errno::from_io_error(&io::Error::last_os_error())
                .unwrap_or(rustix::io::Errno::IO);
            if error == rustix::io::Errno::INTR {
                continue;
            }
            return Err(error.into());
        }
        if count != nonce.len() as isize || nonce == [0; 32] {
            return Err(invalid("incomplete or zero application registration nonce"));
        }
        return Ok(nonce);
    }
}

fn revalidate(
    endpoint: &RetainedApplicationProofEndpointV1,
    root: Option<&ReceivedProcessPidfdV1>,
    deadline: Instant,
) -> Result<()> {
    check_deadline(deadline)?;
    endpoint.revalidate()?;
    if let Some(root) = root {
        root.revalidate()?;
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
    // SAFETY: one initialized pollfd for the retained endpoint; bounded timeout.
    if unsafe { libc::poll(&mut entry, 1, millis) } < 0 {
        let error = rustix::io::Errno::from_io_error(&io::Error::last_os_error())
            .unwrap_or(rustix::io::Errno::IO);
        if error != rustix::io::Errno::INTR {
            return Err(error.into());
        }
    } else if entry.revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
        return Err(invalid(
            "application registration endpoint closed or failed",
        ));
    }
    check_deadline(deadline)
}

fn send(
    endpoint: &RetainedApplicationProofEndpointV1,
    root: Option<&ReceivedProcessPidfdV1>,
    message: &Message,
    deadline: Instant,
) -> Result<()> {
    loop {
        revalidate(endpoint, root, deadline)?;
        let mut ancillary = SendAncillaryBuffer::new(&mut []);
        match sendmsg(
            &endpoint.peer,
            &[IoSlice::new(message.canonical_bytes())],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(count) if count == message.canonical_bytes().len() => {
                return revalidate(endpoint, root, deadline);
            }
            Ok(_) => return Err(invalid("partial application registration packet send")),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                wait(endpoint, libc::POLLOUT, deadline)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

struct Received {
    message: Message,
    sender: CompilerExecutionClientProcessIdentityV1,
    rights: Vec<OwnedFd>,
}

fn receive(
    endpoint: &RetainedApplicationProofEndpointV1,
    root: Option<&ReceivedProcessPidfdV1>,
    deadline: Instant,
) -> Result<Received> {
    loop {
        revalidate(endpoint, root, deadline)?;
        let mut bytes = [0; WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = match recvmsg(
            &endpoint.peer,
            &mut [IoSliceMut::new(&mut bytes)],
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
        let mut rights = Vec::with_capacity(1);
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
            || received.bytes > bytes.len()
            || !(received.flags - ReturnFlags::CMSG_CLOEXEC).is_empty()
            || invalid_ancillary
            || rights.len() > 1
            || rights_messages != usize::from(!rights.is_empty())
        {
            return Err(invalid(
                "registration packet or ancillary roster is malformed",
            ));
        }
        let credentials = credentials.ok_or_else(|| invalid("registration sender is absent"))?;
        let sender = CompilerExecutionClientProcessIdentityV1::new(
            credentials.pid.as_raw_nonzero().get() as u32,
            credentials.uid.as_raw(),
            credentials.gid.as_raw(),
        )
        .map_err(|_| invalid("invalid registration sender"))?;
        let message = Message::decode(&bytes[..received.bytes])
            .map_err(|_| invalid("noncanonical application registration packet"))?;
        if rights.len() != message.rights() {
            return Err(invalid("registration descriptor count differs from phase"));
        }
        revalidate(endpoint, root, deadline)?;
        return Ok(Received {
            message,
            sender,
            rights,
        });
    }
}

#[cfg(test)]
mod tests;
