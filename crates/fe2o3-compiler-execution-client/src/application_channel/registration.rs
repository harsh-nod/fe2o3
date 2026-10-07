//! Application-first registration over the one inherited proof transport.

use super::transport::check_deadline;
use super::*;
use std::time::{Duration, Instant};

use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationRegistrationInputsV1,
    WorkerV3ApplicationSessionKindV1 as Kind, WorkerV3ApplicationSessionMessageV1 as Message,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
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
        require_connected(&self.endpoint)?;
        self.root.revalidate()?;
        if self.sender.pid() != self.root.pid() || self.sender.uid() != 0 || self.sender.gid() != 0
        {
            return Err(invalid("registered root process association changed"));
        }
        self.endpoint.revalidate()?;
        require_connected(&self.endpoint)
    }

    pub const fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.endpoint.descriptor_identity()
    }

    /// Inert transcript equality data, never a proof or a readiness capability.
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
}

pub(super) fn require_connected(endpoint: &RetainedApplicationProofEndpointV1) -> Result<()> {
    let mut entry = libc::pollfd {
        fd: endpoint.peer.as_raw_fd(),
        events: libc::POLLRDHUP,
        revents: 0,
    };
    // A live root process may retire this session without exiting. Observe, never consume.
    if unsafe { libc::poll(&mut entry, 1, 0) } < 0 {
        return Err(
            rustix::io::Errno::from_io_error(&io::Error::last_os_error())
                .unwrap_or(rustix::io::Errno::IO)
                .into(),
        );
    }
    if entry.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL | libc::POLLRDHUP) != 0 {
        return Err(invalid("registered root session closed"));
    }
    Ok(())
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
        let accepted = self.begin_registration(inputs, deadline)?;
        let ready = receive(&accepted.endpoint, Some(&accepted.root), deadline)?;
        if ready.message.kind() != Kind::Ready
            || ready.sender != accepted.sender
            || ready.message.transcript() != Some(accepted.transcript)
        {
            return Err(invalid("registration Ready sender or transcript mismatch"));
        }
        let registered = RegisteredApplicationProofEndpointV1 {
            endpoint: accepted.endpoint,
            root: accepted.root,
            sender: accepted.sender,
            transcript: accepted.transcript,
        };
        registered.revalidate()?;
        check_deadline(deadline)?;
        Ok(registered)
    }

    pub(super) fn begin_registration(
        self,
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        deadline: Instant,
    ) -> Result<AcceptedRegistration> {
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
        let envelope = binding.expectation().envelope();
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
        Ok(AcceptedRegistration {
            endpoint: self,
            root,
            sender: challenge.sender,
            transcript,
            envelope,
        })
    }
}

pub(super) struct AcceptedRegistration {
    pub(super) endpoint: RetainedApplicationProofEndpointV1,
    pub(super) root: ReceivedProcessPidfdV1,
    pub(super) sender: CompilerExecutionClientProcessIdentityV1,
    pub(super) transcript: Transcript,
    pub(super) envelope: fe2o3_runtime_protocol::WorkerV3LoadEnvelopeIdentityV1,
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

fn send(
    endpoint: &RetainedApplicationProofEndpointV1,
    root: Option<&ReceivedProcessPidfdV1>,
    message: &Message,
    deadline: Instant,
) -> Result<()> {
    let processes = root.into_iter().collect::<Vec<_>>();
    super::transport::send(
        endpoint,
        &processes,
        message.canonical_bytes(),
        &[],
        deadline,
    )
}

pub(super) struct Received {
    pub(super) message: Message,
    pub(super) sender: CompilerExecutionClientProcessIdentityV1,
    pub(super) rights: Vec<OwnedFd>,
}

pub(super) fn receive(
    endpoint: &RetainedApplicationProofEndpointV1,
    root: Option<&ReceivedProcessPidfdV1>,
    deadline: Instant,
) -> Result<Received> {
    let processes = root.into_iter().collect::<Vec<_>>();
    let received = super::transport::receive(
        endpoint,
        &processes,
        super::transport::ReceiveProfile::Registration,
        deadline,
    )?;
    let message = Message::decode(&received.bytes)
        .map_err(|_| invalid("noncanonical application registration packet"))?;
    if received.rights.len() != message.rights() {
        return Err(invalid("registration descriptor count differs from phase"));
    }
    Ok(Received {
        message,
        sender: received.sender,
        rights: received.rights,
    })
}

#[cfg(test)]
mod tests;
