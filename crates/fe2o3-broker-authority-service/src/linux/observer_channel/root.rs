use super::*;

/// Root-created private observer endpoint before the measured issuer has been bound.
///
/// Creation does not inspect compiler publication contents: publication occurs after readiness.
/// The coordinator must derive the supplied client and launch from its authenticated supervisor
/// registration, never from application-selected process IDs or paths.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_broker_authority_service::PreparedRootCompilerExecutionObserverV1>();
/// ```
pub struct PreparedRootCompilerExecutionObserverV1 {
    endpoint: Endpoint,
    root: LiveClientPidfdIdentityV1,
    client: RetainedCompilerClientSessionV1,
    launch: CompilerExecutionServiceLaunchManifestV1,
    session: [u8; 32],
    namespaces: ProtectedServiceNamespaceSetV1,
    production: bool,
}

impl PreparedRootCompilerExecutionObserverV1 {
    /// Creates a connected, kernel-autobound authenticated-channel pair as exact root.
    ///
    /// Returns private issuer transfer descriptors in order: endpoint, original root pidfd.
    /// Both endpoints have SO_PASSCRED before either is exposed. This does not authenticate a
    /// registration or prove that an issuer was launched; `bind` completes process admission.
    pub fn prepare(
        client: RetainedCompilerClientSessionV1,
        launch: CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<(Self, [OwnedFd; 2])> {
        Self::prepare_inner(client, launch, policy, true)
    }

    pub(super) fn prepare_inner(
        client: RetainedCompilerClientSessionV1,
        launch: CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        production: bool,
    ) -> Result<(Self, [OwnedFd; 2])> {
        if production {
            require_exact_root_identity_v1()
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        }
        client.revalidate()?;
        let expected = client.client();
        if !launch.matches_policy(policy)
            || launch.client().pid() != expected.pid
            || launch.client().uid() != expected.uid
            || launch.client().gid() != expected.gid
            || (production && expected.uid == 0)
        {
            return Err(invalid("observer launch/client/policy mismatch"));
        }
        let root = current_identity()?;
        if root.expected_client.pid == expected.pid {
            return Err(invalid("observer cannot observe itself"));
        }
        let (root_peer, issuer_peer) = observer_pair()?;
        let root_transfer = rustix::io::fcntl_dupfd_cloexec(&root.pidfd, 0)?;
        let prepared = Self {
            endpoint: Endpoint::admit(root_peer)?,
            root,
            client,
            launch,
            session: nonce()?,
            namespaces: ProtectedServiceNamespaceSetV1::capture_self()
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?,
            production,
        };
        prepared.validate_root()?;
        Ok((prepared, [issuer_peer, root_transfer]))
    }

    /// Binds actual retained supervisor/issuer pidfds from the coordinator's measured launch.
    ///
    /// The coordinator must also reject reuse of the issuer PID/start identity in another live session.
    /// Parentage, live start identities, namespaces and proc-visible service profiles are checked
    /// here. The measured executable chain remains the coordinator/supervisor's responsibility.
    /// No purported issuer is signaled if these initial binding checks fail.
    pub fn bind(
        self,
        supervisor: LiveClientPidfdIdentityV1,
        issuer: LiveClientPidfdIdentityV1,
    ) -> Result<RootCompilerExecutionObserverV1> {
        self.validate_root()?;
        let issuer_id = issuer.expected_client;
        let supervisor_id = supervisor.expected_client;
        if issuer_id.pid == self.root.expected_client.pid
            || issuer_id.pid == supervisor_id.pid
            || issuer_id.pid == self.client.client().pid
            || issuer_id.uid != supervisor_id.uid
            || issuer_id.gid != supervisor_id.gid
            || (self.production
                && (issuer_id.uid == 0
                    || issuer_id.uid == self.client.client().uid
                    || supervisor_id.pid == self.root.expected_client.pid))
        {
            return Err(invalid(
                "issuer and supervisor roles overlap or have different credentials",
            ));
        }
        issuer.validate_parent(&supervisor)?;
        if self.production {
            supervisor.validate_parent(&self.root)?;
        }
        self.validate_service(&supervisor)?;
        self.validate_service(&issuer)?;
        let mut body = self.launch.canonical_bytes().to_vec();
        body.extend_from_slice(&self.client.process_identity().1.to_le_bytes());
        encode_identity(&issuer, &mut body);
        encode_identity(&supervisor, &mut body);
        let hello = Packet {
            kind: Kind::Hello,
            session: self.session,
            launch: *self.launch.identity().as_bytes(),
            sequence: 0,
            operation: 0,
            nonce: [0; 32],
            body,
        };
        hello.encode()?;
        Ok(RootCompilerExecutionObserverV1 {
            prepared: self,
            supervisor,
            issuer,
            active: None,
            pending: Some(PendingResponse {
                packet: hello,
                locks: None,
            }),
            next_sequence: 1,
            deadline: Instant::now() + TIMEOUT,
            session_deadline: Instant::now() + SESSION_TIMEOUT,
            containing: false,
            closing: false,
            exited: false,
        })
    }

    fn validate_root(&self) -> Result<()> {
        if self.production {
            require_exact_root_identity_v1()
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        }
        if self.root.expected_client.pid != std::process::id() {
            return Err(invalid("observer owner process changed"));
        }
        self.root.validate_liveness()?;
        self.namespaces
            .revalidate_self()
            .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        self.client.revalidate()?;
        self.endpoint.revalidate()
    }

    fn validate_service(&self, process: &LiveClientPidfdIdentityV1) -> Result<()> {
        process.validate_liveness()?;
        let pid = rustix::process::Pid::from_raw(process.expected_client.pid as i32)
            .ok_or_else(|| invalid("invalid service PID"))?;
        self.namespaces
            .revalidate_process(pid)
            .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        if self.production {
            let credentials = ProtectedServiceCredentialProfileV1::new(
                process.expected_client.uid,
                process.expected_client.gid,
            )
            .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
            validate_protected_service_process_v1(credentials, pid)
                .map_err(|e| CompilerExecutionObserverErrorV1::Profile(e.to_string()))?;
        }
        process.validate_liveness()?;
        Ok(())
    }
}

struct ActiveOccurrence {
    operation: u64,
    occurrence: RetainedCompilerExecutionOccurrenceV1,
}

struct PendingResponse {
    packet: Packet,
    locks: Option<CompilerModuleHandoffLockRetentionV3>,
}

/// Result of one bounded nonblocking observer-session step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootCompilerExecutionObserverProgressV1 {
    /// No packet was ready, or the one retained response is backpressured.
    Idle,
    /// One request was processed or one response was sent.
    Progress,
    /// The exact issuer has been signaled but its exit is not yet confirmed.
    Containing,
    /// The issuer's retained pidfd confirms exit; publication custody is released.
    Exited,
}

/// Root custody for one exact issuer and at most one active compiler occurrence.
///
/// `step` never waits for a socket or serves a complete operation in a loop. Synchronous
/// observation/filesystem work is bounded by its input limits, not a hard wall-clock deadline.
/// On any failure, custody remains until the exact issuer exits. Drop may wait without a
/// deadline to preserve that requirement; abrupt root death is instead covered by the lock
/// descriptions transferred to the issuer guard. This owner never signs or reads the compiler
/// service socket.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_broker_authority_service::RootCompilerExecutionObserverV1>();
/// ```
pub struct RootCompilerExecutionObserverV1 {
    prepared: PreparedRootCompilerExecutionObserverV1,
    supervisor: LiveClientPidfdIdentityV1,
    issuer: LiveClientPidfdIdentityV1,
    active: Option<ActiveOccurrence>,
    pending: Option<PendingResponse>,
    next_sequence: u64,
    deadline: Instant,
    session_deadline: Instant,
    containing: bool,
    closing: bool,
    exited: bool,
}

impl RootCompilerExecutionObserverV1 {
    /// Handles at most one nonblocking send or receive and retains pending responses on EAGAIN.
    pub fn step(&mut self) -> Result<RootCompilerExecutionObserverProgressV1> {
        if self.exited {
            return Ok(RootCompilerExecutionObserverProgressV1::Exited);
        }
        if self.containing {
            return self.contain();
        }
        let result = self.step_inner();
        if matches!(result, Err(CompilerExecutionObserverErrorV1::Closed))
            && self.active.is_none()
            && self.pending.is_none()
        {
            self.closing = true;
            self.deadline = Instant::now() + TIMEOUT;
            return Ok(RootCompilerExecutionObserverProgressV1::Progress);
        }
        if result.is_err() {
            // Exit can race any of the live identity checks after the initial pidfd poll.
            if matches!(pidfd_exited(&self.issuer.pidfd), Ok(true)) {
                self.release_after_exit();
                return Ok(RootCompilerExecutionObserverProgressV1::Exited);
            }
            self.containing = true;
            self.prepared.endpoint.close();
            let _ = self.contain();
        }
        result
    }

    /// Stops requests and begins exact-pidfd containment without releasing live occurrence custody.
    pub fn cancel(&mut self) -> Result<RootCompilerExecutionObserverProgressV1> {
        self.containing = true;
        self.prepared.endpoint.close();
        self.contain()
    }

    fn validate(&self) -> Result<()> {
        require_deadline(self.session_deadline)?;
        if self.active.is_some() || self.pending.is_some() {
            require_deadline(self.deadline)?;
        }
        self.prepared.validate_root()?;
        self.prepared.validate_service(&self.supervisor)?;
        self.prepared.validate_service(&self.issuer)?;
        self.issuer.validate_parent(&self.supervisor)?;
        if self.prepared.production {
            self.supervisor.validate_parent(&self.prepared.root)?;
        }
        Ok(())
    }

    fn step_inner(&mut self) -> Result<RootCompilerExecutionObserverProgressV1> {
        if pidfd_exited(&self.issuer.pidfd)? {
            self.release_after_exit();
            return Ok(RootCompilerExecutionObserverProgressV1::Exited);
        }
        if self.closing {
            require_deadline(self.deadline)?;
            return Ok(RootCompilerExecutionObserverProgressV1::Idle);
        }
        if self.active.is_none() && self.pending.is_none() && self.prepared.endpoint.closed()? {
            // A completed service drops its channel just before process exit. No custody is
            // active, so stop admission and allow a bounded clean exit instead of racing it.
            self.closing = true;
            self.deadline = Instant::now() + TIMEOUT;
            return Ok(RootCompilerExecutionObserverProgressV1::Progress);
        }
        self.validate()?;
        if let Some(pending) = &self.pending {
            // Backpressure can separate request handling from response transmission. Recheck
            // the live observation, not only the channel, before releasing its result.
            if let Some(active) = &self.active {
                active.occurrence.revalidate()?;
            }
            let rights = pending
                .locks
                .as_ref()
                .map(|locks| locks.transfer_descriptors());
            if !self
                .prepared
                .endpoint
                .send(&pending.packet, rights.as_ref().map_or(&[], |fds| &fds[..]))?
            {
                return Ok(RootCompilerExecutionObserverProgressV1::Idle);
            }
            let finished = pending.packet.kind == Kind::Finished;
            self.validate()?;
            self.pending = None;
            if finished {
                self.active = None;
            }
            return Ok(RootCompilerExecutionObserverProgressV1::Progress);
        }
        let Some((packet, _rights)) = self.prepared.endpoint.receive(&self.issuer)? else {
            return Ok(RootCompilerExecutionObserverProgressV1::Idle);
        };
        if packet.session != self.prepared.session
            || packet.launch != *self.prepared.launch.identity().as_bytes()
            || packet.sequence != self.next_sequence
            || self.next_sequence > MAX_REQUESTS
        {
            return Err(invalid("session, launch, or request replay mismatch"));
        }
        let (response, locks) = match packet.kind {
            Kind::Begin if self.active.is_none() && packet.operation == packet.sequence => {
                self.deadline = Instant::now() + TIMEOUT;
                let occurrence = RetainedCompilerExecutionOccurrenceV1::observe(
                    self.prepared.client.retain_session()?,
                )?;
                let mut body = occurrence.subject().canonical_bytes().to_vec();
                body.extend_from_slice(occurrence.identity());
                // Install custody before any fallible export or post-observation check.
                self.active = Some(ActiveOccurrence {
                    operation: packet.operation,
                    occurrence,
                });
                let locks = self
                    .active
                    .as_ref()
                    .unwrap()
                    .occurrence
                    .retain_publication_lock_descriptors()?;
                (packet.response(Kind::Begun, body), Some(locks))
            }
            Kind::Revalidate | Kind::Finish => {
                let active = self
                    .active
                    .as_ref()
                    .ok_or_else(|| invalid("no active occurrence"))?;
                if packet.operation != active.operation {
                    return Err(invalid("operation replay mismatch"));
                }
                active.occurrence.revalidate()?;
                (
                    packet.response(
                        if packet.kind == Kind::Finish {
                            Kind::Finished
                        } else {
                            Kind::Revalidated
                        },
                        Vec::new(),
                    ),
                    None,
                )
            }
            _ => return Err(invalid("request is invalid in current observer state")),
        };
        self.pending = Some(PendingResponse {
            packet: response,
            locks,
        });
        self.next_sequence += 1;
        self.validate()?;
        Ok(RootCompilerExecutionObserverProgressV1::Progress)
    }

    fn contain(&mut self) -> Result<RootCompilerExecutionObserverProgressV1> {
        if self.exited {
            return Ok(RootCompilerExecutionObserverProgressV1::Exited);
        }
        if !pidfd_exited(&self.issuer.pidfd)? {
            match rustix::process::pidfd_send_signal(
                &self.issuer.pidfd,
                rustix::process::Signal::KILL,
            ) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if pidfd_exited(&self.issuer.pidfd)? {
            self.release_after_exit();
            Ok(RootCompilerExecutionObserverProgressV1::Exited)
        } else {
            Ok(RootCompilerExecutionObserverProgressV1::Containing)
        }
    }

    fn release_after_exit(&mut self) {
        self.prepared.endpoint.close();
        self.active = None;
        self.pending = None;
        self.exited = true;
    }
}

impl Drop for RootCompilerExecutionObserverV1 {
    fn drop(&mut self) {
        self.containing = true;
        self.prepared.endpoint.close();
        while !self.exited {
            let _ = self.contain();
            if !self.exited {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
