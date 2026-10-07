//! Authenticated root/supervisor registration, separate from the occurrence protocol.

use super::application::{
    ApplicationPublication, ApplicationRoute, ApplicationSession, MAX_APPLICATIONS,
    PublishedApplicationCustodianHandoffV1,
};
use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1;
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1, WorkerV3ApplicationRegistrationBindingV1,
};

const REGISTRY_MAGIC: &[u8; 8] = b"F2O3REG1";
const REGISTRY_HEADER: usize = 88;
// The supervisor admits four concurrent workers. Spare slots cover failed pre-bind launches
// until their original endpoint closes or their bounded registration deadline expires.
const MAX_SESSIONS: usize = 16;
const BIND_TIMEOUT: Duration = Duration::from_secs(150);

#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum RegistryKind {
    Register = 1,
    Registered,
    Bind,
    Bound,
    RegisterApplication,
    RegisteredApplication,
    AttachApplication,
    ApplicationInstalled,
    RegisterCustodianApplication,
    RegisteredCustodianApplication,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryPacket {
    kind: RegistryKind,
    sequence: u64,
    nonce: [u8; 32],
    registration: [u8; 32],
    body: Vec<u8>,
}

impl RegistryPacket {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = vec![0; REGISTRY_HEADER + self.body.len()];
        bytes[..8].copy_from_slice(REGISTRY_MAGIC);
        bytes[8] = self.kind as u8;
        bytes[16..24].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[24..56].copy_from_slice(&self.nonce);
        bytes[56..88].copy_from_slice(&self.registration);
        bytes[REGISTRY_HEADER..].copy_from_slice(&self.body);
        Self::decode(&bytes)?;
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < REGISTRY_HEADER || &bytes[..8] != REGISTRY_MAGIC || bytes[9..16] != [0; 7]
        {
            return Err(invalid("noncanonical registration header"));
        }
        let kind = match bytes[8] {
            1 => RegistryKind::Register,
            2 => RegistryKind::Registered,
            3 => RegistryKind::Bind,
            4 => RegistryKind::Bound,
            5 => RegistryKind::RegisterApplication,
            6 => RegistryKind::RegisteredApplication,
            7 => RegistryKind::AttachApplication,
            8 => RegistryKind::ApplicationInstalled,
            9 => RegistryKind::RegisterCustodianApplication,
            10 => RegistryKind::RegisteredCustodianApplication,
            _ => return Err(invalid("unknown registration kind")),
        };
        let size = match kind {
            RegistryKind::Register => COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1,
            RegistryKind::RegisterApplication | RegistryKind::RegisterCustodianApplication => {
                WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1
            }
            RegistryKind::Registered
            | RegistryKind::RegisteredApplication
            | RegistryKind::RegisteredCustodianApplication
            | RegistryKind::AttachApplication => 32,
            RegistryKind::ApplicationInstalled => 64,
            RegistryKind::Bind | RegistryKind::Bound => 52,
        };
        if bytes.len() != REGISTRY_HEADER + size {
            return Err(invalid("noncanonical registration length"));
        }
        let packet = Self {
            kind,
            sequence: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            nonce: bytes[24..56].try_into().unwrap(),
            registration: bytes[56..88].try_into().unwrap(),
            body: bytes[REGISTRY_HEADER..].to_vec(),
        };
        if packet.sequence == 0
            || packet.nonce == [0; 32]
            || (matches!(
                kind,
                RegistryKind::Register
                    | RegistryKind::RegisterApplication
                    | RegistryKind::RegisterCustodianApplication
            ) != (packet.registration == [0; 32]))
        {
            return Err(invalid("invalid registration identity"));
        }
        Ok(packet)
    }

    fn rights(&self) -> usize {
        match self.kind {
            RegistryKind::Register
            | RegistryKind::Registered
            | RegistryKind::RegisterApplication
            | RegistryKind::RegisteredApplication
            | RegistryKind::RegisterCustodianApplication
            | RegistryKind::RegisteredCustodianApplication
            | RegistryKind::AttachApplication
            | RegistryKind::ApplicationInstalled => 2,
            RegistryKind::Bind => 1,
            RegistryKind::Bound => 0,
        }
    }

    fn receive(
        endpoint: &Endpoint,
        sender: &LiveClientPidfdIdentityV1,
    ) -> Result<Option<(Self, Vec<OwnedFd>)>> {
        let Some((bytes, rights)) = endpoint.receive_bytes(sender)? else {
            return Ok(None);
        };
        let packet = Self::decode(&bytes)?;
        if rights.len() != packet.rights() {
            return Err(invalid("registration descriptor roster mismatch"));
        }
        Ok(Some((packet, rights)))
    }

    fn send(&self, endpoint: &Endpoint, rights: &[BorrowedFd<'_>]) -> Result<bool> {
        if rights.len() != self.rights() {
            return Err(invalid("outgoing registration descriptor roster mismatch"));
        }
        endpoint.send_bytes(&self.encode()?, rights)
    }
}

/// Root-created registration custody, before binding the measured supervisor child.
pub struct PreparedRootCompilerObserverRegistryV1 {
    endpoint: Endpoint,
    root: LiveClientPidfdIdentityV1,
    namespaces: ProtectedServiceNamespaceSetV1,
    policy: CompilerExecutionIssuerPolicyV1,
    credentials: ProtectedServiceCredentialProfileV1,
    anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    production: bool,
}

impl PreparedRootCompilerObserverRegistryV1 {
    /// Creates a private connected channel and returns its supervisor endpoint/root pidfd.
    pub fn prepare(
        policy: CompilerExecutionIssuerPolicyV1,
        credentials: ProtectedServiceCredentialProfileV1,
        anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    ) -> Result<(Self, [OwnedFd; 2])> {
        Self::prepare_inner(policy, credentials, anchor, true)
    }

    fn prepare_inner(
        policy: CompilerExecutionIssuerPolicyV1,
        credentials: ProtectedServiceCredentialProfileV1,
        anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
        production: bool,
    ) -> Result<(Self, [OwnedFd; 2])> {
        if production {
            require_exact_root_identity_v1().map_err(profile)?;
        }
        if credentials.uid() == anchor.uid() {
            return Err(invalid("registration service roles overlap"));
        }
        let root = current_identity()?;
        let (root_peer, supervisor_peer) = observer_pair()?;
        let root_transfer = rustix::io::fcntl_dupfd_cloexec(&root.pidfd, 0)?;
        Ok((
            Self {
                endpoint: Endpoint::admit(root_peer)?,
                root,
                namespaces: ProtectedServiceNamespaceSetV1::capture_self().map_err(profile)?,
                policy,
                credentials,
                anchor,
                production,
            },
            [supervisor_peer, root_transfer],
        ))
    }

    /// Binds the original pidfd of the measured root-launched supervisor, never a reopened PID.
    pub fn bind(
        self,
        supervisor: LiveClientPidfdIdentityV1,
    ) -> Result<RootCompilerObserverRegistryV1> {
        self.validate(&supervisor)?;
        Ok(RootCompilerObserverRegistryV1 {
            prepared: self,
            supervisor,
            sessions: Vec::new(),
            applications: Vec::new(),
            extracted_applications: [None; MAX_APPLICATIONS],
            pending: None,
            next_sequence: 1,
            closing: false,
        })
    }

    fn validate(&self, supervisor: &LiveClientPidfdIdentityV1) -> Result<()> {
        if self.production {
            require_exact_root_identity_v1().map_err(profile)?;
        }
        if self.root.expected_client.pid != std::process::id()
            || supervisor.expected_client.pid == self.root.expected_client.pid
            || supervisor.expected_client.uid != self.credentials.uid()
            || supervisor.expected_client.gid != self.credentials.gid()
        {
            return Err(invalid("registration supervisor identity mismatch"));
        }
        self.root.validate_liveness()?;
        self.namespaces.revalidate_self().map_err(profile)?;
        supervisor.validate_parent(&self.root)?;
        let pid = rustix::process::Pid::from_raw(supervisor.expected_client.pid as i32)
            .ok_or_else(|| invalid("registration supervisor PID"))?;
        self.namespaces.revalidate_process(pid).map_err(profile)?;
        if self.production {
            validate_protected_service_process_v1(self.credentials, pid).map_err(profile)?;
        }
        self.endpoint.revalidate()?;
        supervisor.validate_liveness()?;
        Ok(())
    }
}

enum ApplicationAttachment {
    CompilerOnly,
    Expected {
        binding: Box<WorkerV3ApplicationRegistrationBindingV1>,
        route: ApplicationRoute,
    },
    Attached([u8; 32]),
}

enum Session {
    Prepared {
        id: [u8; 32],
        launch: [u8; 32],
        observer: Box<PreparedRootCompilerExecutionObserverV1>,
        deadline: Instant,
        application: ApplicationAttachment,
    },
    Bound(Box<RootCompilerExecutionObserverV1>),
}

struct Pending {
    packet: RegistryPacket,
    rights: Vec<OwnedFd>,
    deadline: Instant,
}

#[derive(Clone, Copy)]
struct ExtractedApplicationIdentity {
    process: (u32, u64),
    id: [u8; 32],
}

/// Bounded root-owned registration table and fair nonblocking observer scheduler.
///
/// Call `step` regularly after supervisor readiness. Session errors initiate containment and
/// are reported separately; control-channel errors close the whole registry. Bound publication
/// custody is never released until the exact issuer's original pidfd confirms exit.
pub struct RootCompilerObserverRegistryV1 {
    prepared: PreparedRootCompilerObserverRegistryV1,
    supervisor: LiveClientPidfdIdentityV1,
    sessions: Vec<Session>,
    applications: Vec<ApplicationSession>,
    extracted_applications: [Option<ExtractedApplicationIdentity>; MAX_APPLICATIONS],
    pending: Option<Pending>,
    next_sequence: u64,
    closing: bool,
}

impl RootCompilerObserverRegistryV1 {
    /// Moves one completely published custodian registration out of registry/issuer cleanup.
    /// The returned owner still requires revalidation and independently approved controller
    /// admission. It has not sent application Ready. No owner is extracted during shutdown.
    /// Its identity/capacity reservation remains for this registry's lifetime; extraction
    /// cannot make the same live occurrence reusable or bypass the application limit.
    pub fn take_published_application_custodian(
        &mut self,
    ) -> Option<PublishedApplicationCustodianHandoffV1> {
        if self.closing {
            return None;
        }
        let slot = self
            .extracted_applications
            .iter()
            .position(Option::is_none)?;
        let handoff = PublishedApplicationCustodianHandoffV1::take_next(&mut self.applications)?;
        self.extracted_applications[slot] = Some(ExtractedApplicationIdentity {
            process: handoff.process_identity(),
            id: handoff.transcript().root_nonce(),
        });
        Some(handoff)
    }

    /// Services each live observer once and at most one control send/receive.
    /// Returns whether any observer or registration made progress.
    pub fn step(
        &mut self,
        mut on_session_failure: impl FnMut(CompilerExecutionObserverErrorV1),
    ) -> Result<bool> {
        let mut progress = false;
        let mut index = 0;
        while index < self.sessions.len() {
            let remove = match &mut self.sessions[index] {
                Session::Prepared {
                    observer, deadline, ..
                } => match observer.peer_closed() {
                    Ok(closed) => self.closing || Instant::now() >= *deadline || closed,
                    Err(error) => {
                        on_session_failure(error);
                        true
                    }
                },
                Session::Bound(observer) => match observer.step() {
                    Ok(RootCompilerExecutionObserverProgressV1::Exited) => true,
                    Ok(
                        RootCompilerExecutionObserverProgressV1::Idle
                        | RootCompilerExecutionObserverProgressV1::Containing,
                    ) => false,
                    Ok(RootCompilerExecutionObserverProgressV1::Progress) => {
                        progress = true;
                        false
                    }
                    Err(error) => {
                        on_session_failure(error);
                        false
                    }
                },
            };
            if remove {
                if let Session::Prepared {
                    application: ApplicationAttachment::Attached(id),
                    ..
                } = self.sessions.remove(index)
                    && let Some(application) =
                        self.applications.iter_mut().find(|value| value.id == id)
                {
                    application.cancel();
                }
                progress = true;
            } else {
                index += 1;
            }
        }
        let mut observation_budget = true;
        for application in &mut self.applications {
            match application.step(&self.prepared.root, &mut observation_budget) {
                Ok(changed) => progress |= changed,
                Err(error) => {
                    application.cancel();
                    on_session_failure(error);
                }
            }
        }
        self.applications
            .retain(|application| !application.retired());
        if self.closing {
            return Ok(progress);
        }
        let result = self.step_control();
        if result.is_err() {
            self.cancel_all();
        }
        result.map(|control| progress || control)
    }

    fn step_control(&mut self) -> Result<bool> {
        self.prepared.validate(&self.supervisor)?;
        if self.prepared.endpoint.closed()? {
            return Err(CompilerExecutionObserverErrorV1::Closed);
        }
        if let Some(pending) = &self.pending {
            self.validate_pending(pending)?;
            if pending.packet.send(
                &self.prepared.endpoint,
                &pending.rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
            )? {
                if matches!(
                    pending.packet.kind,
                    RegistryKind::Registered
                        | RegistryKind::RegisteredApplication
                        | RegistryKind::RegisteredCustodianApplication
                ) {
                    for session in &mut self.sessions {
                        if let Session::Prepared { id, deadline, .. } = session
                            && *id == pending.packet.registration
                        {
                            *deadline = Instant::now() + BIND_TIMEOUT;
                        }
                    }
                }
                self.pending = None;
                return Ok(true);
            }
            return Ok(false);
        }
        let Some((packet, rights)) =
            RegistryPacket::receive(&self.prepared.endpoint, &self.supervisor)?
        else {
            return Ok(false);
        };
        if packet.sequence != self.next_sequence {
            return Err(invalid("registration request replay"));
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("registration sequence exhausted"))?;
        match packet.kind {
            RegistryKind::Register
            | RegistryKind::RegisterApplication
            | RegistryKind::RegisterCustodianApplication => self.register(packet, rights)?,
            RegistryKind::AttachApplication => self.attach_application(packet, rights)?,
            RegistryKind::Bind => self.bind_issuer(packet, rights)?,
            _ => return Err(invalid("unexpected registration response at root")),
        }
        Ok(true)
    }

    fn validate_pending(&self, pending: &Pending) -> Result<()> {
        require_deadline(pending.deadline)?;
        if matches!(
            pending.packet.kind,
            RegistryKind::Registered
                | RegistryKind::RegisteredApplication
                | RegistryKind::RegisteredCustodianApplication
        ) && !self.sessions.iter().any(|session| {
            matches!(session,
                Session::Prepared { id, .. } if *id == pending.packet.registration)
        }) {
            return Err(invalid("pending registration was retired before delivery"));
        }
        if pending.packet.kind == RegistryKind::ApplicationInstalled {
            let application = self
                .applications
                .iter()
                .find(|application| {
                    application.id[..] == pending.packet.body[32..]
                        && application.binding.identity().as_bytes()[..]
                            == pending.packet.body[..32]
                })
                .ok_or_else(|| {
                    invalid("pending application installation was retired before delivery")
                })?;
            if !self.sessions.iter().any(|session| matches!(session,
                    Session::Prepared { id, application: ApplicationAttachment::Attached(attached), .. }
                    if *id == pending.packet.registration && *attached == application.id)) {
                    return Err(invalid("pending application compiler registration changed"));
                }
            application.revalidate_installation()?;
        }
        Ok(())
    }

    fn register(&mut self, packet: RegistryPacket, rights: Vec<OwnedFd>) -> Result<()> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(invalid("observer registration capacity exhausted"));
        }
        let (launch, application, response_kind) = if matches!(
            packet.kind,
            RegistryKind::RegisterApplication | RegistryKind::RegisterCustodianApplication
        ) {
            let binding = WorkerV3ApplicationRegistrationBindingV1::decode(&packet.body)
                .map_err(|_| invalid("noncanonical application registration"))?;
            (
                binding.compiler_handoff().launch_manifest().clone(),
                ApplicationAttachment::Expected {
                    binding: Box::new(binding),
                    route: if packet.kind == RegistryKind::RegisterApplication {
                        ApplicationRoute::ObservationOnly
                    } else {
                        ApplicationRoute::ProofCustodian
                    },
                },
                if packet.kind == RegistryKind::RegisterApplication {
                    RegistryKind::RegisteredApplication
                } else {
                    RegistryKind::RegisteredCustodianApplication
                },
            )
        } else {
            (
                CompilerExecutionServiceLaunchManifestV1::decode(&packet.body)
                    .map_err(|_| invalid("noncanonical registered launch"))?,
                ApplicationAttachment::CompilerOnly,
                RegistryKind::Registered,
            )
        };
        validate_launch(
            &launch,
            &self.prepared.policy,
            self.prepared.credentials,
            self.prepared.anchor,
            self.prepared.production,
        )?;
        if launch.client().pid() == self.supervisor.expected_client.pid
            || launch.client().pid() == self.prepared.root.expected_client.pid
        {
            return Err(invalid("registered client overlaps service process"));
        }
        let [peer, pidfd]: [OwnedFd; 2] = rights
            .try_into()
            .map_err(|_| invalid("registered client descriptors"))?;
        let expected = ExpectedClientProcessIdentityV1::new(
            launch.client().pid(),
            launch.client().uid(),
            launch.client().gid(),
        )?;
        let client = RetainedCompilerClientSessionV1::admit(
            peer,
            LiveClientPidfdIdentityV1::admit(pidfd, expected)?,
        )?;
        let (observer, transfers) = PreparedRootCompilerExecutionObserverV1::prepare_inner(
            client,
            launch.clone(),
            &self.prepared.policy,
            self.prepared.production,
        )?;
        let id = nonce()?;
        if self
            .sessions
            .iter()
            .any(|s| matches!(s, Session::Prepared { id: old, .. } if *old == id))
        {
            return Err(invalid("duplicate registration nonce"));
        }
        let response_identity = match &application {
            ApplicationAttachment::Expected { binding, .. } => *binding.identity().as_bytes(),
            _ => *launch.identity().as_bytes(),
        };
        self.sessions.push(Session::Prepared {
            id,
            launch: *launch.identity().as_bytes(),
            observer: Box::new(observer),
            deadline: Instant::now() + TIMEOUT,
            application,
        });
        self.pending = Some(Pending {
            packet: RegistryPacket {
                kind: response_kind,
                registration: id,
                body: response_identity.to_vec(),
                ..packet
            },
            rights: Vec::from(transfers),
            deadline: Instant::now() + TIMEOUT,
        });
        Ok(())
    }

    fn attach_application(&mut self, packet: RegistryPacket, rights: Vec<OwnedFd>) -> Result<()> {
        if self.applications.len() + self.extracted_applications.iter().flatten().count()
            >= MAX_APPLICATIONS
        {
            return Err(invalid("application registration capacity exhausted"));
        }
        let (index, binding, route, application) = self
            .sessions
            .iter()
            .enumerate()
            .find_map(|(index, session)| match session {
                Session::Prepared {
                    id,
                    observer,
                    deadline,
                    application: ApplicationAttachment::Expected { binding, route },
                    ..
                } if *id == packet.registration
                    && binding.identity().as_bytes()[..] == packet.body =>
                {
                    Some((|| -> Result<_> {
                        require_deadline(*deadline)?;
                        Ok((
                            index,
                            (**binding).clone(),
                            *route,
                            observer.retain_application_process(
                                binding.compiler_handoff().launch_manifest(),
                            )?,
                        ))
                    })())
                }
                _ => None,
            })
            .ok_or_else(|| {
                invalid("unknown, mismatched, or already attached application registration")
            })??;
        let submitter = binding.compiler_handoff().submitter();
        if submitter.pid() == self.prepared.root.expected_client.pid
            || submitter.pid() == self.supervisor.expected_client.pid
            || (self.prepared.production
                && (submitter.uid() == 0
                    || submitter.uid() == self.prepared.credentials.uid()
                    || submitter.uid() == self.prepared.anchor.uid()))
        {
            return Err(invalid("application parent overlaps service role"));
        }
        let [peer, parent_pidfd]: [OwnedFd; 2] = rights
            .try_into()
            .map_err(|_| invalid("application attachment descriptors"))?;
        let parent = LiveClientPidfdIdentityV1::admit(
            parent_pidfd,
            ExpectedClientProcessIdentityV1::new(
                submitter.pid(),
                submitter.uid(),
                submitter.gid(),
            )?,
        )?;
        application.validate_parent(&parent)?;
        let process = (
            application.expected_client.pid,
            application.start_time_ticks,
        );
        let id = nonce()?;
        if self
            .applications
            .iter()
            .any(|value| value.process_identity() == process || value.id == id)
            || self
                .extracted_applications
                .iter()
                .flatten()
                .any(|value| value.process == process || value.id == id)
        {
            return Err(invalid("application process or registration reused"));
        }
        let mut body = binding.identity().as_bytes().to_vec();
        body.extend_from_slice(&id);
        let (session, gate) =
            ApplicationSession::install(application, parent, peer, binding, id, route)?;
        // Install original-process custody before publishing the gate. Issuer binding must not
        // consume this independent owner, and a lost response is contained by cancel_all.
        self.applications.push(session);
        if let Session::Prepared { application, .. } = &mut self.sessions[index] {
            *application = ApplicationAttachment::Attached(id);
        }
        self.pending = Some(Pending {
            packet: RegistryPacket {
                kind: RegistryKind::ApplicationInstalled,
                body,
                ..packet
            },
            rights: Vec::from(gate),
            deadline: Instant::now() + TIMEOUT,
        });
        Ok(())
    }

    fn bind_issuer(&mut self, packet: RegistryPacket, mut rights: Vec<OwnedFd>) -> Result<()> {
        let (expected, start) = decode_identity(&packet.body[..20])?;
        if expected.uid != self.prepared.credentials.uid()
            || expected.gid != self.prepared.credentials.gid()
        {
            return Err(invalid("registered issuer credentials mismatch"));
        }
        let issuer = LiveClientPidfdIdentityV1::admit(rights.remove(0), expected)?;
        if start != issuer.start_time_ticks
            || self.sessions.iter().any(|session| {
                matches!(session, Session::Bound(observer)
                if observer.issuer_identity() == (expected.pid, start))
            })
        {
            return Err(invalid(
                "registered issuer reused or start identity changed",
            ));
        }
        let index = self
            .sessions
            .iter()
            .position(|session| {
                matches!(session,
            Session::Prepared { id, launch, .. }
            if *id == packet.registration && launch[..] == packet.body[20..])
            })
            .ok_or_else(|| invalid("unknown or already bound registration"))?;
        let supervisor = self.supervisor.try_clone()?;
        match &self.sessions[index] {
            Session::Prepared {
                application: ApplicationAttachment::Expected { .. },
                ..
            } => return Err(invalid("application must attach before issuer binding")),
            Session::Prepared {
                application: ApplicationAttachment::Attached(id),
                ..
            } if !self
                .applications
                .iter()
                .any(|value| value.id == *id && value.installed()) =>
            {
                return Err(invalid(
                    "application registration retired before issuer binding",
                ));
            }
            _ => {}
        }
        let attached = match &self.sessions[index] {
            Session::Prepared {
                application: ApplicationAttachment::Attached(id),
                ..
            } => {
                let application = self
                    .applications
                    .iter()
                    .find(|value| value.id == *id)
                    .expect("checked application registration");
                application.revalidate_installation()?;
                Some(*id)
            }
            _ => None,
        };
        let Session::Prepared {
            observer, deadline, ..
        } = self.sessions.remove(index)
        else {
            unreachable!()
        };
        require_deadline(deadline)?;
        let observer = observer.bind(supervisor, issuer)?;
        // Custody is installed before acknowledging the binding or doing further fallible work.
        self.sessions.push(Session::Bound(Box::new(observer)));
        if let Some(id) = attached {
            self.applications
                .iter_mut()
                .find(|value| value.id == id)
                .expect("retained application")
                .issuer_bound();
        }
        self.pending = Some(Pending {
            packet: RegistryPacket {
                kind: RegistryKind::Bound,
                ..packet
            },
            rights: Vec::new(),
            deadline: Instant::now() + TIMEOUT,
        });
        Ok(())
    }

    /// Stops registration and initiates exact-pidfd containment of every bound issuer.
    /// Continue `step` until `is_drained`; no timeout releases publication custody.
    pub fn cancel_all(&mut self) {
        self.closing = true;
        self.prepared.endpoint.close();
        self.pending = None;
        for application in &mut self.applications {
            application.cancel();
        }
        self.sessions.retain_mut(|session| match session {
            Session::Prepared { .. } => false,
            Session::Bound(observer) => {
                let _ = observer.cancel();
                true
            }
        });
    }

    /// Whether registry-held custody is drained and every bound issuer exit is confirmed.
    /// Extracted application handoffs have independent owners and are not covered by this.
    pub fn is_drained(&self) -> bool {
        self.closing && self.sessions.is_empty() && self.applications.is_empty()
    }
}

impl Drop for RootCompilerObserverRegistryV1 {
    fn drop(&mut self) {
        // Signal all issuers first; their individual Drop owners then wait for exact exit.
        self.cancel_all();
        self.sessions.clear();
        self.applications.clear();
    }
}

/// Supervisor-side serialized registration capability for its original root parent.
pub struct SupervisorCompilerObserverRegistryV1 {
    endpoint: Endpoint,
    root: LiveClientPidfdIdentityV1,
    supervisor: LiveClientPidfdIdentityV1,
    namespaces: ProtectedServiceNamespaceSetV1,
    policy: CompilerExecutionIssuerPolicyV1,
    credentials: ProtectedServiceCredentialProfileV1,
    anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    next_sequence: u64,
    poisoned: bool,
    production: bool,
}

/// Opaque pre-launch observer transfer bound to one launch and one supervisor registry.
///
/// Drop closes only this owner's aliases; it never shuts down the issuer's inherited endpoint.
pub struct RegisteredCompilerObserverV1 {
    endpoint: Endpoint,
    root: LiveClientPidfdIdentityV1,
    id: [u8; 32],
    launch: CompilerExecutionServiceLaunchManifestV1,
    registry: ObjectIdentityV1,
}

/// Inseparable application registration and original root-observation gate.
///
/// This owner cannot be downgraded to compiler-only registration. Its inert binding is not
/// proof authority; observation must complete before application readiness is published.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_broker_authority_service::RegisteredApplicationObserverV1>();
/// ```
/// ```compile_fail
/// fn downgrade(value: fe2o3_broker_authority_service::RegisteredApplicationObserverV1)
///     -> fe2o3_broker_authority_service::RegisteredCompilerObserverV1 { value.into() }
/// ```
pub struct RegisteredApplicationObserverV1 {
    compiler: RegisteredCompilerObserverV1,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    gate: PendingApplicationObservationGateV1,
    publication: ApplicationPublication,
}

/// Supervisor custody for the mandatory proof-custodian route. No legacy conversion exists.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::{RegisteredCustodianApplicationObserverV1, RegisteredApplicationObserverV1};
/// fn downgrade(value: RegisteredCustodianApplicationObserverV1) -> RegisteredApplicationObserverV1 { value.into() }
/// ```
pub struct RegisteredCustodianApplicationObserverV1 {
    inner: RegisteredApplicationObserverV1,
}
impl RegisteredCustodianApplicationObserverV1 {
    /// Local publication-mechanics fixture only; not authenticated custodian registration.
    #[cfg(feature = "test-support")]
    pub fn local_fixture_for_test(
        binding: WorkerV3ApplicationRegistrationBindingV1,
    ) -> Result<(Self, OwnedFd, OwnedFd, [u8; 72], OwnedFd)> {
        let (inner, peer, writer, record, reader) =
            RegisteredApplicationObserverV1::local_fixture_for_test(binding)?;
        Ok((Self { inner }, peer, writer, record, reader))
    }

    pub fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        self.inner.binding()
    }
    pub fn try_clone_for_launch(&self) -> Result<[OwnedFd; 2]> {
        self.inner.try_clone_for_launch()
    }
    pub fn await_observation(
        self,
        deadline: Instant,
    ) -> Result<ObservedCustodianApplicationRegistrationV1> {
        self.inner
            .await_observation(deadline)
            .map(|inner| ObservedCustodianApplicationRegistrationV1 { inner })
    }
}

/// Published registration remains distinct from controller Ready, proof, and GPU authority.
pub struct ObservedCustodianApplicationRegistrationV1 {
    inner: ObservedApplicationRegistrationV1,
}
impl ObservedCustodianApplicationRegistrationV1 {
    pub fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        self.inner.binding()
    }
    pub fn revalidate(&self) -> Result<()> {
        self.inner.revalidate()
    }
    /// Completes publication only. The application still awaits a separate custodian Ready.
    pub fn confirm_publication(self, deadline: Instant) -> Result<()> {
        self.inner.confirm_publication(deadline)
    }
}

impl RegisteredApplicationObserverV1 {
    /// Local-process fixture for downstream gate/custody tests, not authenticated registration.
    /// Returns the retained observer counterpart, gate writer, and exact expected record.
    #[cfg(feature = "test-support")]
    pub fn local_fixture_for_test(
        binding: WorkerV3ApplicationRegistrationBindingV1,
    ) -> Result<(Self, OwnedFd, OwnedFd, [u8; 72], OwnedFd)> {
        let (peer, counterpart) = observer_pair()?;
        let endpoint = Endpoint::admit(peer)?;
        let root = current_identity()?;
        let (reader, writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )?;
        let session = [71; 32];
        let gate = PendingApplicationObservationGateV1::admit(
            reader,
            root.try_clone()?,
            *binding.identity().as_bytes(),
            session,
        )?;
        let (publication_reader, publication_writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )?;
        let publication = ApplicationPublication::admit(
            publication_writer,
            root.try_clone()?,
            *binding.identity().as_bytes(),
            session,
            gate.object,
        )?;
        let mut record = [0; 72];
        record[..8].copy_from_slice(b"F3AOBS1\0");
        record[8..40].copy_from_slice(binding.identity().as_bytes());
        record[40..].copy_from_slice(&session);
        let compiler = RegisteredCompilerObserverV1 {
            registry: endpoint.identity,
            endpoint,
            root,
            id: [72; 32],
            launch: binding.compiler_handoff().launch_manifest().clone(),
        };
        Ok((
            Self {
                compiler,
                binding,
                gate,
                publication,
            },
            counterpart,
            writer,
            record,
            publication_reader,
        ))
    }

    /// Returns descriptive data only.
    pub const fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.binding
    }

    fn revalidate(&self) -> Result<()> {
        self.compiler.revalidate()?;
        self.gate.revalidate()?;
        self.publication.revalidate()
    }

    /// Duplicates only the two fixed issuer observer inputs, never the gate or proof peer.
    pub fn try_clone_for_launch(&self) -> Result<[OwnedFd; 2]> {
        self.revalidate()?;
        let transfers = self.compiler.try_clone_for_launch()?;
        self.revalidate()?;
        Ok(transfers)
    }

    /// Consumes the original gate outside the supervisor registry mutex.
    pub fn await_observation(self, deadline: Instant) -> Result<ObservedApplicationRegistrationV1> {
        self.revalidate()?;
        self.gate.await_observation(deadline)?;
        self.compiler.revalidate()?;
        require_deadline(deadline)?;
        Ok(ObservedApplicationRegistrationV1 {
            compiler: self.compiler,
            binding: self.binding,
            publication: self.publication,
        })
    }
}

/// Original registration retained after its observation gate, not proof or GPU authority.
pub struct ObservedApplicationRegistrationV1 {
    compiler: RegisteredCompilerObserverV1,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    publication: ApplicationPublication,
}

impl ObservedApplicationRegistrationV1 {
    /// Returns the exact descriptive application binding.
    pub const fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.binding
    }

    /// Rechecks the original root and registration endpoint before readiness publication.
    pub fn revalidate(&self) -> Result<()> {
        self.compiler.revalidate()?;
        self.publication.revalidate()
    }

    /// Releases app Ready only after the supervisor successfully publishes Cargo readiness.
    /// Consumes the original reverse gate. No issuer/application liveness check may follow
    /// its successful commit, because the application is then allowed to finish immediately.
    pub fn confirm_publication(self, deadline: Instant) -> Result<()> {
        self.revalidate()?;
        self.publication.publish(deadline)
    }
}

impl RegisteredCompilerObserverV1 {
    /// Checks the exact canonical launch without granting observer or signing authority.
    pub fn matches_launch(&self, launch: &CompilerExecutionServiceLaunchManifestV1) -> bool {
        self.launch == *launch
    }

    fn revalidate(&self) -> Result<()> {
        self.root.validate_liveness()?;
        self.endpoint.revalidate()?;
        if self.endpoint.closed()? {
            return Err(CompilerExecutionObserverErrorV1::Closed);
        }
        Ok(())
    }

    /// Duplicates endpoint/root pidfd in fixed order for the measured issuer launch.
    pub fn try_clone_for_launch(&self) -> Result<[OwnedFd; 2]> {
        self.revalidate()?;
        let result = [
            rustix::io::fcntl_dupfd_cloexec(&self.endpoint.peer, 0)?,
            rustix::io::fcntl_dupfd_cloexec(&self.root.pidfd, 0)?,
        ];
        self.revalidate()?;
        Ok(result)
    }
}

fn validate_launch(
    launch: &CompilerExecutionServiceLaunchManifestV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    credentials: ProtectedServiceCredentialProfileV1,
    anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    production: bool,
) -> Result<()> {
    if !launch.matches_policy(policy)
        || !launch.matches_external_anchor_service(anchor)
        || (production
            && (launch.client().uid() == 0
                || launch.client().uid() == credentials.uid()
                || launch.client().uid() == anchor.uid()))
    {
        return Err(invalid(
            "registered launch policy or service identity mismatch",
        ));
    }
    Ok(())
}

fn profile(error: impl fmt::Display) -> CompilerExecutionObserverErrorV1 {
    CompilerExecutionObserverErrorV1::Profile(error.to_string())
}

mod supervisor;
