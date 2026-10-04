use super::*;
use std::cell::RefCell;

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;

struct ChannelState {
    next_sequence: u64,
    active: Option<u64>,
    poisoned: bool,
}

/// Authenticated root-observer custody for one protected signing process.
///
/// This exposes no subject-byte import or signing method. It can only be attached to the exact
/// issuer admission whose client, service root, peer and policy were checked at construction.
/// Every response requires the retained root pidfd and exact kernel sender credentials.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_broker_authority_service::ProtectedCompilerExecutionObserverV1>();
/// ```
/// ```compile_fail
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<fe2o3_broker_authority_service::ProtectedCompilerExecutionObserverV1>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RemoteCompilerExecutionOccurrenceGuardV1;
/// ```
pub struct ProtectedCompilerExecutionObserverV1 {
    endpoint: Endpoint,
    root: LiveClientPidfdIdentityV1,
    issuer: LiveClientPidfdIdentityV1,
    supervisor: (ExpectedClientProcessIdentityV1, u64),
    namespaces: ProtectedServiceNamespaceSetV1,
    launch: CompilerExecutionServiceLaunchManifestV1,
    service_binding: (ObjectIdentityV1, ObjectIdentityV1, (u32, u64)),
    session: [u8; 32],
    state: RefCell<ChannelState>,
    production: bool,
}

impl ProtectedCompilerExecutionObserverV1 {
    /// Consumes the root-created endpoint and original root pidfd from the protected launch.
    ///
    /// Admission waits only for the root's process-binding greeting, not a compiler observation.
    /// Failed admission must terminate the service; it must not fall back to local observation.
    pub fn admit(
        endpoint: OwnedFd,
        root_pidfd: OwnedFd,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        service: &ProtectedServiceAdmissionV1,
    ) -> Result<Self> {
        Self::admit_inner(endpoint, root_pidfd, launch, policy, service, true)
    }

    pub(super) fn admit_inner(
        endpoint: OwnedFd,
        root_pidfd: OwnedFd,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        service: &ProtectedServiceAdmissionV1,
        production: bool,
    ) -> Result<Self> {
        service.validate_session_continuity()?;
        let expected = service.live_client.expected_client;
        if !launch.matches_policy(policy)
            || launch.client().pid() != expected.pid
            || launch.client().uid() != expected.uid
            || launch.client().gid() != expected.gid
        {
            return Err(invalid("observer issuer admission does not match launch"));
        }
        let endpoint = Endpoint::admit(endpoint)?;
        let creator = endpoint.creator;
        if production && (creator.uid != 0 || creator.gid != 0) {
            return Err(invalid("observer endpoint was not created by root"));
        }
        let root = LiveClientPidfdIdentityV1::admit(
            root_pidfd,
            ExpectedClientProcessIdentityV1::new(creator.pid, creator.uid, creator.gid)?,
        )?;
        let issuer = current_identity()?;
        if issuer.expected_client.pid == creator.pid {
            return Err(invalid("observer and issuer are the same process"));
        }
        validate_issuer_profile(&issuer, production)?;
        let deadline = Instant::now() + TIMEOUT;
        let hello = loop {
            wait_for(&endpoint, &root, libc::POLLIN, deadline)?;
            if let Some((packet, _rights)) = endpoint.receive(&root)? {
                break packet;
            }
        };
        let manifest_end = COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1;
        if hello.kind != Kind::Hello
            || hello.launch != *launch.identity().as_bytes()
            || hello.body[..manifest_end] != launch.canonical_bytes()[..]
            || u64::from_le_bytes(
                hello.body[manifest_end..manifest_end + 8]
                    .try_into()
                    .unwrap(),
            ) != service.live_client.start_time_ticks
        {
            return Err(invalid(
                "observer greeting launch or client binding mismatch",
            ));
        }
        let observed_issuer = decode_identity(&hello.body[manifest_end + 8..manifest_end + 28])?;
        if observed_issuer != (issuer.expected_client, issuer.start_time_ticks) {
            return Err(invalid("observer greeting names another issuer"));
        }
        let supervisor = decode_identity(&hello.body[manifest_end + 28..])?;
        if supervisor.1 == 0
            || supervisor.0.uid != issuer.expected_client.uid
            || supervisor.0.gid != issuer.expected_client.gid
            || rustix::process::getppid().map(|pid| pid.as_raw_pid() as u32)
                != Some(supervisor.0.pid)
        {
            return Err(invalid("observer greeting names another supervisor"));
        }
        let channel = Self {
            endpoint,
            root,
            issuer,
            supervisor,
            namespaces: ProtectedServiceNamespaceSetV1::capture_self()
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?,
            launch: launch.clone(),
            service_binding: (
                service.root_identity,
                service.peer_identity,
                service.client_process_identity(),
            ),
            session: hello.session,
            state: RefCell::new(ChannelState {
                next_sequence: 1,
                active: None,
                poisoned: false,
            }),
            production,
        };
        channel.validate_continuity()?;
        service.validate_session_continuity()?;
        Ok(channel)
    }

    pub(crate) fn matches_admission(
        &self,
        service: &ProtectedServiceAdmissionV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> bool {
        self.launch.matches_policy(policy)
            && self.service_binding
                == (
                    service.root_identity,
                    service.peer_identity,
                    service.client_process_identity(),
                )
    }

    pub(crate) fn validate_continuity(&self) -> Result<()> {
        if self.state.borrow().poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        let result = (|| {
            self.endpoint.revalidate()?;
            // A live coordinator can retire a session without exiting. Do not let other
            // admission paths continue merely because the original root pidfd is live.
            if self.endpoint.closed()? {
                return Err(invalid("observer session endpoint closed"));
            }
            self.root.validate_liveness()?;
            validate_issuer_profile(&self.issuer, self.production)?;
            self.namespaces
                .revalidate_self()
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
            if rustix::process::getppid().map(|pid| pid.as_raw_pid() as u32)
                != Some(self.supervisor.0.pid)
            {
                return Err(invalid("issuer parent changed"));
            }
            self.root.validate_liveness()?;
            Ok(())
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }

    pub(crate) fn begin(&self) -> Result<RemoteCompilerExecutionOccurrenceGuardV1<'_>> {
        self.validate_continuity()?;
        let already_active = self.state.borrow().active.is_some();
        if already_active {
            self.poison();
            return Err(invalid("observer operation already active"));
        }
        let operation = self.state.borrow().next_sequence;
        self.state.borrow_mut().active = Some(operation);
        let result = (|| {
            let (response, rights) = self.exchange(Kind::Begin, Kind::Begun, operation)?;
            let locks = CompilerModuleHandoffLockRetentionV3::from_received_descriptors(
                rights
                    .try_into()
                    .map_err(|_| invalid("missing observer lock descriptions"))?,
            )?;
            let subject = InertCompilerExecutionSubjectV1::decode(
                &response.body[..INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1],
            )
            .map_err(|_| invalid("noncanonical observer subject"))?;
            let identity: [u8; 32] = response.body[INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1..]
                .try_into()
                .unwrap();
            if identity == [0; 32] {
                return Err(invalid("empty occurrence identity"));
            }
            self.validate_continuity()?;
            Ok(RemoteCompilerExecutionOccurrenceGuardV1 {
                channel: self,
                operation,
                subject,
                identity,
                _locks: locks,
                completed: false,
            })
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }

    fn exchange(
        &self,
        kind: Kind,
        response_kind: Kind,
        operation: u64,
    ) -> Result<(Packet, Vec<OwnedFd>)> {
        let result = self.exchange_inner(kind, response_kind, operation);
        if result.is_err() {
            self.poison();
        }
        result
    }

    fn exchange_inner(
        &self,
        kind: Kind,
        response_kind: Kind,
        operation: u64,
    ) -> Result<(Packet, Vec<OwnedFd>)> {
        self.validate_continuity()?;
        let sequence = {
            let mut state = self.state.borrow_mut();
            if state.active != Some(operation) || state.next_sequence > MAX_REQUESTS {
                return Err(invalid(
                    "issuer observer operation mismatch or budget exhausted",
                ));
            }
            let sequence = state.next_sequence;
            state.next_sequence += 1;
            sequence
        };
        let packet = Packet {
            kind,
            session: self.session,
            launch: *self.launch.identity().as_bytes(),
            sequence,
            operation,
            nonce: nonce()?,
            body: Vec::new(),
        };
        let deadline = Instant::now() + TIMEOUT;
        loop {
            wait_for(&self.endpoint, &self.root, libc::POLLOUT, deadline)?;
            self.validate_continuity()?;
            if self.endpoint.send(&packet, &[])? {
                break;
            }
        }
        let (response, rights) = loop {
            wait_for(&self.endpoint, &self.root, libc::POLLIN, deadline)?;
            if let Some(response) = self.endpoint.receive(&self.root)? {
                break response;
            }
        };
        if !packet.matches_response(&response, response_kind) {
            return Err(invalid("observer response replay or operation mismatch"));
        }
        require_deadline(deadline)?;
        self.validate_continuity()?;
        Ok((response, rights))
    }

    fn poison(&self) {
        self.state.borrow_mut().poisoned = true;
        self.endpoint.close();
    }
}

impl Drop for ProtectedCompilerExecutionObserverV1 {
    fn drop(&mut self) {
        self.endpoint.close();
    }
}

pub(crate) struct RemoteCompilerExecutionOccurrenceGuardV1<'a> {
    channel: &'a ProtectedCompilerExecutionObserverV1,
    operation: u64,
    subject: InertCompilerExecutionSubjectV1,
    identity: [u8; 32],
    _locks: CompilerModuleHandoffLockRetentionV3,
    completed: bool,
}

impl RemoteCompilerExecutionOccurrenceGuardV1<'_> {
    pub(crate) const fn subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.subject
    }
    pub(crate) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub(crate) fn revalidate(&self) -> Result<()> {
        self.channel
            .exchange(Kind::Revalidate, Kind::Revalidated, self.operation)?;
        Ok(())
    }
    pub(crate) fn finish(mut self) -> Result<()> {
        self.channel
            .exchange(Kind::Finish, Kind::Finished, self.operation)?;
        self.channel.state.borrow_mut().active = None;
        self.completed = true;
        Ok(())
    }
}

impl Drop for RemoteCompilerExecutionOccurrenceGuardV1<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.channel.poison();
        }
    }
}

fn validate_issuer_profile(issuer: &LiveClientPidfdIdentityV1, production: bool) -> Result<()> {
    if issuer.expected_client.pid != std::process::id() {
        return Err(invalid("issuer owner process changed"));
    }
    issuer.validate_liveness()?;
    if production {
        let credentials = ProtectedServiceCredentialProfileV1::new(
            issuer.expected_client.uid,
            issuer.expected_client.gid,
        )
        .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        validate_current_protected_service_profile_v1(credentials)
            .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
    }
    issuer.validate_liveness()?;
    Ok(())
}

fn wait_for(
    endpoint: &Endpoint,
    root: &LiveClientPidfdIdentityV1,
    events: i16,
    deadline: Instant,
) -> Result<()> {
    loop {
        require_deadline(deadline)?;
        endpoint.revalidate()?;
        root.validate_liveness()?;
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .clamp(1, 30_000) as i32;
        let mut fds = [
            libc::pollfd {
                fd: root.pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: endpoint.peer.as_raw_fd(),
                events,
                revents: 0,
            },
        ];
        // SAFETY: both live descriptors and the initialized pollfd array remain borrowed.
        let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        require_deadline(deadline)?;
        // Root termination takes precedence over data already queued on its endpoint.
        if fds[0].revents != 0 {
            return Err(invalid("observer root exited or its pidfd failed"));
        }
        if fds[1].revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0 {
            return Err(invalid("observer endpoint closed"));
        }
        if fds[1].revents & events != 0 {
            root.validate_liveness()?;
            return Ok(());
        }
    }
}
