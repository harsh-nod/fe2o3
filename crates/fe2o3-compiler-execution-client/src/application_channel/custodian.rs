//! Consuming root-to-controller transition on the original inherited application socket.

use super::{registration, transport, *};
use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2, WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1,
    WorkerV3ApplicationProofInputsV1 as Inputs, WorkerV3ApplicationProofKindV1 as Kind,
    WorkerV3ApplicationProofMessageV1 as Message, WorkerV3ApplicationProofSessionV1 as Session,
    WorkerV3ApplicationRegistrationInputsV1, WorkerV3ApplicationSessionKindV1,
    WorkerV3LoadEnvelopeIdentityV1,
};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

const MAX_EXECUTION: Duration = Duration::from_secs(300);
const MAX_PROBE: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct ControllerChannel {
    endpoint: RetainedApplicationProofEndpointV1,
    controller: ReceivedProcessPidfdV1,
    sender: CompilerExecutionClientProcessIdentityV1,
    session: Session,
    poisoned: Cell<bool>,
}
impl ControllerChannel {
    fn revalidate(&self) -> Result<()> {
        if self.poisoned.get() {
            return Err(invalid("application proof channel is terminal"));
        }
        let result = (|| {
            self.endpoint.revalidate()?;
            registration::require_connected(&self.endpoint)?;
            self.controller.revalidate()?;
            if self.controller.pid() != self.sender.pid()
                || self.session.controller()
                    != (self.sender.pid(), self.sender.uid(), self.sender.gid())
            {
                return Err(invalid("application controller association changed"));
            }
            Ok(())
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
    fn send(
        &mut self,
        message: &Message,
        rights: &[BorrowedFd<'_>],
        deadline: Instant,
    ) -> Result<()> {
        let result = (|| {
            self.revalidate()?;
            if message.session() != self.session.identity()
                || message.required_rights() != rights.len()
            {
                return Err(invalid("outgoing proof session or rights mismatch"));
            }
            transport::send(
                &self.endpoint,
                &[&self.controller],
                message.canonical_bytes(),
                rights,
                deadline,
            )?;
            self.revalidate()
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
    fn receive(&mut self, kind: Kind, sequence: u64, deadline: Instant) -> Result<Message> {
        let result = (|| {
            self.revalidate()?;
            let received = transport::receive(
                &self.endpoint,
                &[&self.controller],
                transport::ReceiveProfile::Proof,
                deadline,
            )?;
            let message = Message::decode(&received.bytes)
                .map_err(|_| invalid("noncanonical application proof packet"))?;
            if received.sender != self.sender
                || !received.rights.is_empty()
                || message.required_rights() != 0
                || message.kind() != kind
                || message.session() != self.session.identity()
                || message.sequence() != sequence
            {
                return Err(invalid(
                    "application proof sender, session, phase or sequence mismatch",
                ));
            }
            self.revalidate()?;
            transport::check_deadline(deadline)?;
            Ok(message)
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
}

/// Original root-authenticated controller endpoint, before proof execution.
/// No raw descriptor/session constructor, cloning or unconditional admission exists.
/// The root's policy admission ends at CustodianReady; later coordinator death is
/// not controller death. The original controller pidfd remains the liveness premise.
///
/// ```compile_fail
/// fn export<T: std::os::fd::AsFd>() {}
/// export::<fe2o3_compiler_execution_client::RegisteredApplicationCustodianV1>();
/// ```
#[derive(Debug)]
pub struct RegisteredApplicationCustodianV1 {
    channel: ControllerChannel,
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    execution_deadline: Instant,
}

impl RetainedApplicationProofEndpointV1 {
    /// Consumes exact root CustodianReady before the caller emits its descriptor ACK.
    /// Ordinary Ready cannot enter this route, and failure cannot fall back to it.
    pub fn register_custodian_pre_ack(
        self,
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        deadline: Instant,
    ) -> Result<RegisteredApplicationCustodianV1> {
        let execution_deadline = Instant::now() + MAX_EXECUTION;
        let accepted = self.begin_registration(inputs, deadline)?;
        let mut ready = registration::receive(&accepted.endpoint, Some(&accepted.root), deadline)?;
        if ready.message.kind() != WorkerV3ApplicationSessionKindV1::CustodianReady
            || ready.sender != accepted.sender
            || ready.message.transcript() != Some(accepted.transcript)
        {
            return Err(invalid("custodian Ready sender or transcript mismatch"));
        }
        let session = ready
            .message
            .proof_session()
            .ok_or_else(|| invalid("custodian Ready omitted proof session"))?
            .clone();
        let (pid, uid, gid) = session.controller();
        if session.transcript() != accepted.transcript
            || pid == accepted.sender.pid()
            || pid == std::process::id()
            || pid == accepted.endpoint.snapshot.creator.pid()
            || uid == rustix::process::geteuid().as_raw()
        {
            return Err(invalid("custodian controller overlaps registration roles"));
        }
        let controller = ReceivedProcessPidfdV1::admit_received(
            ready
                .rights
                .pop()
                .ok_or_else(|| invalid("custodian Ready omitted controller pidfd"))?,
            pid,
        )?;
        let sender = CompilerExecutionClientProcessIdentityV1::new(pid, uid, gid)
            .map_err(|_| invalid("invalid controller sender"))?;
        // Root alone approves this deployment/session. Retain its liveness through
        // admission, not as a continuing premise after the completed custody transfer.
        accepted.root.revalidate()?;
        accepted.endpoint.revalidate()?;
        controller.revalidate()?;
        transport::check_deadline(deadline)?;
        let value = RegisteredApplicationCustodianV1 {
            channel: ControllerChannel {
                endpoint: accepted.endpoint,
                controller,
                sender,
                session,
                poisoned: Cell::new(false),
            },
            envelope: accepted.envelope,
            execution_deadline,
        };
        value.revalidate()?;
        transport::check_deadline(deadline)?;
        Ok(value)
    }
}

impl RegisteredApplicationCustodianV1 {
    pub fn revalidate(&self) -> Result<()> {
        self.channel.revalidate()
    }
    pub fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.channel.endpoint.descriptor_identity()
    }
    /// Inert equality data; possession of this record does not transfer controller custody.
    pub fn session(&self) -> &Session {
        &self.channel.session
    }

    /// Run only after descriptor ACK. The host must supply bytes borrowed from its
    /// original recovered admission/current token and retain those owners separately.
    /// A first authenticated Retained response is required before returning custody.
    pub fn request_proof(
        mut self,
        kernel: [u8; 32],
        envelope: &[u8],
        payload: &[u8],
        deadline: Instant,
    ) -> Result<RetainedApplicationProofV1> {
        let deadline = deadline.min(self.execution_deadline);
        transport::check_deadline(deadline)?;
        self.revalidate()?;
        if envelope.is_empty()
            || envelope.len() > MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2
            || payload.is_empty()
            || payload.len() > WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1
        {
            return Err(invalid("application proof input bounds"));
        }
        if WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(envelope)
            .map_err(|_| invalid("invalid application envelope identity"))?
            != self.envelope
        {
            return Err(invalid("proof envelope differs from registered input"));
        }
        let inputs = Inputs::new(
            kernel,
            (Sha256::digest(envelope).into(), envelope.len() as u64),
            (Sha256::digest(payload).into(), payload.len() as u64),
        )
        .map_err(|_| invalid("invalid application proof inputs"))?;
        let files = [seal(envelope, deadline)?, seal(payload, deadline)?];
        let first = rustix::fs::fstat(&files[0])?;
        let second = rustix::fs::fstat(&files[1])?;
        if (first.st_dev, first.st_ino) == (second.st_dev, second.st_ino) {
            return Err(invalid("aliased application proof inputs"));
        }
        let session = self.channel.session.identity();
        let request = Message::new(Kind::Request, session, 1, inputs.canonical_bytes())
            .map_err(|_| invalid("invalid proof request"))?;
        self.channel.receive(Kind::Active, 1, deadline)?;
        self.channel
            .send(&request, &[files[0].as_fd(), files[1].as_fd()], deadline)?;
        let proved = self.channel.receive(Kind::Proved, 1, deadline)?;
        let mut value = RetainedApplicationProofV1 {
            channel: self.channel,
            inputs,
            _files: files,
            subject: proved.body().to_vec().into_boxed_slice(),
            next_sequence: 2,
        };
        value.probe(deadline)?;
        Ok(value)
    }
}

/// Retains the original controller and its authenticated, repeatedly matched proof
/// subject. This is remote custody, not compiler origin, currentness or launch authority.
/// Drop closes only the local endpoint; it sends no Release and establishes no settlement.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_compiler_execution_client::RetainedApplicationProofV1>();
/// ```
/// ```compile_fail
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<fe2o3_compiler_execution_client::RetainedApplicationProofV1>();
/// ```
#[derive(Debug)]
pub struct RetainedApplicationProofV1 {
    channel: ControllerChannel,
    inputs: Inputs,
    _files: [OwnedFd; 2],
    subject: Box<[u8]>,
    next_sequence: u64,
}
impl RetainedApplicationProofV1 {
    pub fn revalidate(&self) -> Result<()> {
        self.channel.revalidate()
    }
    pub fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.channel.endpoint.descriptor_identity()
    }
    pub fn session(&self) -> &Session {
        &self.channel.session
    }
    pub fn inputs(&self) -> &Inputs {
        &self.inputs
    }
    /// Descriptive bytes only. They cannot reconstruct this owner or a local executed proof.
    pub fn subject_bytes(&self) -> &[u8] {
        &self.subject
    }

    /// One serialized round trip under the original session; any failure poisons permanently.
    pub fn probe(&mut self, deadline: Instant) -> Result<()> {
        let result = (|| {
            self.revalidate()?;
            let deadline = deadline.min(Instant::now() + MAX_PROBE);
            transport::check_deadline(deadline)?;
            let next = self
                .next_sequence
                .checked_add(1)
                .filter(|n| *n < u64::MAX)
                .ok_or_else(|| invalid("proof sequence exhausted"))?;
            let probe = Message::new(
                Kind::Probe,
                self.channel.session.identity(),
                self.next_sequence,
                &[],
            )
            .map_err(|_| invalid("invalid proof probe"))?;
            self.channel.send(&probe, &[], deadline)?;
            let retained = self
                .channel
                .receive(Kind::Retained, self.next_sequence, deadline)?;
            if retained.body() != self.subject.as_ref() {
                return Err(invalid("retained application proof subject changed"));
            }
            self.next_sequence = next;
            self.revalidate()
        })();
        if result.is_err() {
            self.channel.poisoned.set(true);
        }
        result
    }
}

fn seal(bytes: &[u8], deadline: Instant) -> Result<OwnedFd> {
    transport::check_deadline(deadline)?;
    let file = rustix::fs::memfd_create(
        c"fe2o3-application-proof-input-v1",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?;
    let mut remaining = bytes;
    while !remaining.is_empty() {
        transport::check_deadline(deadline)?;
        match rustix::io::write(&file, remaining) {
            Ok(0) => return Err(invalid("short application proof input write")),
            Ok(count) => remaining = &remaining[count..],
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR)?;
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE,
    )?;
    let reader = rustix::fs::openat(
        rustix::fs::CWD,
        format!("/proc/self/fd/{}", file.as_raw_fd()),
        OFlags::RDONLY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    transport::check_deadline(deadline)?;
    Ok(reader)
}
