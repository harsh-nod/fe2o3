//! Authenticated root/supervisor registration, separate from the occurrence protocol.

use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1;

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
            _ => return Err(invalid("unknown registration kind")),
        };
        let size = match kind {
            RegistryKind::Register => COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1,
            RegistryKind::Registered => 32,
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
            || ((kind == RegistryKind::Register) != (packet.registration == [0; 32]))
        {
            return Err(invalid("invalid registration identity"));
        }
        Ok(packet)
    }

    fn rights(&self) -> usize {
        match self.kind {
            RegistryKind::Register | RegistryKind::Registered => 2,
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

enum Session {
    Prepared {
        id: [u8; 32],
        launch: [u8; 32],
        observer: Box<PreparedRootCompilerExecutionObserverV1>,
        deadline: Instant,
    },
    Bound(Box<RootCompilerExecutionObserverV1>),
}

struct Pending {
    packet: RegistryPacket,
    rights: Vec<OwnedFd>,
    deadline: Instant,
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
    pending: Option<Pending>,
    next_sequence: u64,
    closing: bool,
}

impl RootCompilerObserverRegistryV1 {
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
                self.sessions.remove(index);
                progress = true;
            } else {
                index += 1;
            }
        }
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
            require_deadline(pending.deadline)?;
            if pending.packet.kind == RegistryKind::Registered && !self.sessions.iter().any(|session|
                matches!(session, Session::Prepared { id, .. } if *id == pending.packet.registration))
            {
                return Err(invalid("pending registration was retired before delivery"));
            }
            if pending.packet.send(
                &self.prepared.endpoint,
                &pending.rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
            )? {
                if pending.packet.kind == RegistryKind::Registered {
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
            RegistryKind::Register => self.register(packet, rights)?,
            RegistryKind::Bind => self.bind_issuer(packet, rights)?,
            _ => return Err(invalid("unexpected registration response at root")),
        }
        Ok(true)
    }

    fn register(&mut self, packet: RegistryPacket, rights: Vec<OwnedFd>) -> Result<()> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(invalid("observer registration capacity exhausted"));
        }
        let launch = CompilerExecutionServiceLaunchManifestV1::decode(&packet.body)
            .map_err(|_| invalid("noncanonical registered launch"))?;
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
        self.sessions.push(Session::Prepared {
            id,
            launch: *launch.identity().as_bytes(),
            observer: Box::new(observer),
            deadline: Instant::now() + TIMEOUT,
        });
        self.pending = Some(Pending {
            packet: RegistryPacket {
                kind: RegistryKind::Registered,
                registration: id,
                body: launch.identity().as_bytes().to_vec(),
                ..packet
            },
            rights: Vec::from(transfers),
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
        self.sessions.retain_mut(|session| match session {
            Session::Prepared { .. } => false,
            Session::Bound(observer) => {
                let _ = observer.cancel();
                true
            }
        });
    }

    /// Whether shutdown has begun and every exact bound issuer exit has been confirmed.
    pub fn is_drained(&self) -> bool {
        self.closing && self.sessions.is_empty()
    }
}

impl Drop for RootCompilerObserverRegistryV1 {
    fn drop(&mut self) {
        // Signal all issuers first; their individual Drop owners then wait for exact exit.
        self.cancel_all();
        self.sessions.clear();
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

impl SupervisorCompilerObserverRegistryV1 {
    /// Admits mandatory root-created descriptors before publishing supervisor readiness.
    /// Does not wait for a root greeting or perform any compiler observation.
    pub fn admit(
        peer: OwnedFd,
        root_pidfd: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
        credentials: ProtectedServiceCredentialProfileV1,
        anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    ) -> Result<Self> {
        Self::admit_inner(peer, root_pidfd, policy, credentials, anchor, true)
    }

    fn admit_inner(
        peer: OwnedFd,
        root_pidfd: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
        credentials: ProtectedServiceCredentialProfileV1,
        anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
        production: bool,
    ) -> Result<Self> {
        let endpoint = Endpoint::admit(peer)?;
        let creator = endpoint.creator;
        if production && (creator.uid != 0 || creator.gid != 0) {
            return Err(invalid("registration endpoint is not root-created"));
        }
        let registry = Self {
            root: LiveClientPidfdIdentityV1::admit(
                root_pidfd,
                ExpectedClientProcessIdentityV1::new(creator.pid, creator.uid, creator.gid)?,
            )?,
            endpoint,
            supervisor: current_identity()?,
            namespaces: ProtectedServiceNamespaceSetV1::capture_self().map_err(profile)?,
            policy: policy.clone(),
            credentials,
            anchor,
            next_sequence: 1,
            poisoned: false,
            production,
        };
        registry.validate_continuity()?;
        Ok(registry)
    }

    /// Checks deployment bindings without exposing descriptors or registration identifiers.
    pub fn matches_deployment(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
        credentials: ProtectedServiceCredentialProfileV1,
        anchor: CompilerExecutionExternalAnchorServiceIdentityV1,
    ) -> bool {
        self.policy == *policy && self.credentials == credentials && self.anchor == anchor
    }

    /// Revalidates the original root, private channel, supervisor profile and parentage.
    pub fn validate_continuity(&self) -> Result<()> {
        if self.poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        if self.supervisor.expected_client.pid != std::process::id()
            || self.supervisor.expected_client.uid != self.credentials.uid()
            || self.supervisor.expected_client.gid != self.credentials.gid()
            || self.credentials.uid() == self.anchor.uid()
            || rustix::process::getppid().map(|p| p.as_raw_pid() as u32)
                != Some(self.root.expected_client.pid)
        {
            return Err(invalid("registration supervisor profile or parent changed"));
        }
        self.endpoint.revalidate()?;
        if self.endpoint.closed()? {
            return Err(CompilerExecutionObserverErrorV1::Closed);
        }
        self.root.validate_liveness()?;
        self.supervisor.validate_liveness()?;
        self.namespaces.revalidate_self().map_err(profile)?;
        if self.production {
            validate_current_protected_service_profile_v1(self.credentials).map_err(profile)?;
        }
        Ok(())
    }

    /// Registers only the original accepted compiler peer and client pidfd, never Cargo control.
    pub fn register(
        &mut self,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        original_peer: BorrowedFd<'_>,
        original_client_pidfd: BorrowedFd<'_>,
    ) -> Result<RegisteredCompilerObserverV1> {
        validate_launch(
            launch,
            &self.policy,
            self.credentials,
            self.anchor,
            self.production,
        )?;
        let (packet, rights) = self.exchange(
            RegistryKind::Register,
            [0; 32],
            launch.canonical_bytes().to_vec(),
            &[original_peer, original_client_pidfd],
            Instant::now() + TIMEOUT,
        )?;
        let result = (|| {
            if packet.body != launch.identity().as_bytes()[..] {
                return Err(invalid("registered response launch mismatch"));
            }
            let [peer, root_pidfd]: [OwnedFd; 2] = rights
                .try_into()
                .map_err(|_| invalid("registered observer descriptors"))?;
            let endpoint = Endpoint::admit(peer)?;
            if endpoint.creator != self.endpoint.creator {
                return Err(invalid("registered observer creator mismatch"));
            }
            let root = LiveClientPidfdIdentityV1::admit(root_pidfd, self.root.expected_client)?;
            if root.start_time_ticks != self.root.start_time_ticks {
                return Err(invalid("registered root start identity mismatch"));
            }
            Ok(RegisteredCompilerObserverV1 {
                endpoint,
                root,
                id: packet.registration,
                launch: launch.clone(),
                registry: self.endpoint.identity,
            })
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }

    /// Binds an actually launched issuer using the supervisor's original child pidfd.
    /// Must complete before waiting for issuer readiness, which requires the root greeting.
    pub fn bind_issuer(
        &mut self,
        registered: &RegisteredCompilerObserverV1,
        issuer_pid: u32,
        original_issuer_pidfd: BorrowedFd<'_>,
        deadline: Instant,
    ) -> Result<()> {
        self.validate_continuity()?;
        if registered.registry != self.endpoint.identity {
            return Err(invalid(
                "registration belongs to another supervisor channel",
            ));
        }
        registered.revalidate()?;
        let issuer = LiveClientPidfdIdentityV1::admit(
            rustix::io::fcntl_dupfd_cloexec(original_issuer_pidfd, 0)?,
            ExpectedClientProcessIdentityV1::new(
                issuer_pid,
                self.credentials.uid(),
                self.credentials.gid(),
            )?,
        )?;
        issuer.validate_parent(&self.supervisor)?;
        let mut body = Vec::with_capacity(52);
        encode_identity(&issuer, &mut body);
        body.extend_from_slice(registered.launch.identity().as_bytes());
        let expected = body.clone();
        let (packet, _) = self.exchange(
            RegistryKind::Bind,
            registered.id,
            body,
            &[original_issuer_pidfd],
            deadline.min(Instant::now() + TIMEOUT),
        )?;
        if packet.body != expected {
            self.poison();
            return Err(invalid("bound response issuer mismatch"));
        }
        issuer.validate_liveness()?;
        Ok(())
    }

    fn exchange(
        &mut self,
        kind: RegistryKind,
        registration: [u8; 32],
        body: Vec<u8>,
        rights: &[BorrowedFd<'_>],
        deadline: Instant,
    ) -> Result<(RegistryPacket, Vec<OwnedFd>)> {
        let result = (|| {
            self.validate_continuity()?;
            let sequence = self.next_sequence;
            self.next_sequence = sequence
                .checked_add(1)
                .ok_or_else(|| invalid("registration sequence exhausted"))?;
            let packet = RegistryPacket {
                kind,
                sequence,
                nonce: nonce()?,
                registration,
                body,
            };
            require_deadline(deadline)?;
            loop {
                issuer::wait_for(&self.endpoint, &self.root, libc::POLLOUT, deadline)?;
                self.validate_continuity()?;
                if packet.send(&self.endpoint, rights)? {
                    break;
                }
            }
            let (response, rights) = loop {
                issuer::wait_for(&self.endpoint, &self.root, libc::POLLIN, deadline)?;
                if let Some(response) = RegistryPacket::receive(&self.endpoint, &self.root)? {
                    break response;
                }
            };
            let expected = match kind {
                RegistryKind::Register => RegistryKind::Registered,
                RegistryKind::Bind => RegistryKind::Bound,
                _ => return Err(invalid("invalid supervisor registration request")),
            };
            if response.kind != expected
                || response.sequence != packet.sequence
                || response.nonce != packet.nonce
                || (kind == RegistryKind::Bind && response.registration != registration)
            {
                return Err(invalid("registration response replay or binding mismatch"));
            }
            require_deadline(deadline)?;
            self.validate_continuity()?;
            Ok((response, rights))
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }

    fn poison(&mut self) {
        self.poisoned = true;
        self.endpoint.close();
    }
}

impl Drop for SupervisorCompilerObserverRegistryV1 {
    fn drop(&mut self) {
        self.poison();
    }
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
