use super::*;

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

    pub(super) fn admit_inner(
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
        self.admit_registered(launch, launch.identity().as_bytes(), packet, rights)
    }

    /// Registers the distinct application profile and installs original-process observation
    /// custody before returning. The caller must await the gate outside the registry mutex.
    /// This gate is neither compiler readiness nor proof or native invocation authority.
    pub fn register_application(
        &mut self,
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        original_compiler_peer: BorrowedFd<'_>,
        original_application_pidfd: BorrowedFd<'_>,
        original_proof_peer: BorrowedFd<'_>,
        original_parent_pidfd: BorrowedFd<'_>,
        deadline: Instant,
    ) -> Result<RegisteredApplicationObserverV1> {
        self.register_application_inner(
            binding,
            original_compiler_peer,
            original_application_pidfd,
            original_proof_peer,
            original_parent_pidfd,
            deadline,
            RegistryKind::RegisterApplication,
        )
    }

    /// Selects the mandatory custodian route before application observation begins.
    /// This route cannot send legacy Ready or downgrade to observation-only registration.
    pub fn register_custodian_application(
        &mut self,
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        original_compiler_peer: BorrowedFd<'_>,
        original_application_pidfd: BorrowedFd<'_>,
        original_proof_peer: BorrowedFd<'_>,
        original_parent_pidfd: BorrowedFd<'_>,
        deadline: Instant,
    ) -> Result<RegisteredCustodianApplicationObserverV1> {
        self.register_application_inner(
            binding,
            original_compiler_peer,
            original_application_pidfd,
            original_proof_peer,
            original_parent_pidfd,
            deadline,
            RegistryKind::RegisterCustodianApplication,
        )
        .map(|inner| RegisteredCustodianApplicationObserverV1 { inner })
    }

    #[allow(clippy::too_many_arguments)]
    fn register_application_inner(
        &mut self,
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        original_compiler_peer: BorrowedFd<'_>,
        original_application_pidfd: BorrowedFd<'_>,
        original_proof_peer: BorrowedFd<'_>,
        original_parent_pidfd: BorrowedFd<'_>,
        deadline: Instant,
        kind: RegistryKind,
    ) -> Result<RegisteredApplicationObserverV1> {
        let launch = binding.compiler_handoff().launch_manifest();
        validate_launch(
            launch,
            &self.policy,
            self.credentials,
            self.anchor,
            self.production,
        )?;
        let deadline = deadline.min(Instant::now() + TIMEOUT);
        let (packet, rights) = self.exchange(
            kind,
            [0; 32],
            binding.canonical_bytes().to_vec(),
            &[original_compiler_peer, original_application_pidfd],
            deadline,
        )?;
        let registered =
            self.admit_registered(launch, binding.identity().as_bytes(), packet, rights)?;
        let result = (|| {
            let (packet, rights) = self.exchange(
                RegistryKind::AttachApplication,
                registered.id,
                binding.identity().as_bytes().to_vec(),
                &[original_proof_peer, original_parent_pidfd],
                deadline,
            )?;
            if packet.body[..32] != binding.identity().as_bytes()[..] {
                return Err(invalid("installed application binding mismatch"));
            }
            let [reader, publication_writer]: [OwnedFd; 2] = rights
                .try_into()
                .map_err(|_| invalid("application gate descriptors"))?;
            let gate = PendingApplicationObservationGateV1::admit(
                reader,
                self.root.try_clone()?,
                *binding.identity().as_bytes(),
                packet.body[32..].try_into().unwrap(),
            )?;
            let publication = ApplicationPublication::admit(
                publication_writer,
                self.root.try_clone()?,
                *binding.identity().as_bytes(),
                packet.body[32..].try_into().unwrap(),
                gate.object,
            )?;
            registered.revalidate()?;
            self.validate_continuity()?;
            require_deadline(deadline)?;
            Ok(RegisteredApplicationObserverV1 {
                compiler: registered,
                binding: binding.clone(),
                gate,
                publication,
            })
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }

    fn admit_registered(
        &mut self,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        expected_identity: &[u8; 32],
        packet: RegistryPacket,
        rights: Vec<OwnedFd>,
    ) -> Result<RegisteredCompilerObserverV1> {
        let result = (|| {
            if packet.body != expected_identity[..] {
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

    /// Binds an application issuer without discarding its required observation gate.
    pub fn bind_application_issuer(
        &mut self,
        registered: &RegisteredApplicationObserverV1,
        issuer_pid: u32,
        original_issuer_pidfd: BorrowedFd<'_>,
        deadline: Instant,
    ) -> Result<()> {
        registered.revalidate()?;
        self.bind_issuer(
            &registered.compiler,
            issuer_pid,
            original_issuer_pidfd,
            deadline,
        )
    }

    pub fn bind_custodian_application_issuer(
        &mut self,
        registered: &RegisteredCustodianApplicationObserverV1,
        issuer_pid: u32,
        original_issuer_pidfd: BorrowedFd<'_>,
        deadline: Instant,
    ) -> Result<()> {
        self.bind_application_issuer(
            &registered.inner,
            issuer_pid,
            original_issuer_pidfd,
            deadline,
        )
    }

    pub(super) fn exchange(
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
                RegistryKind::RegisterApplication => RegistryKind::RegisteredApplication,
                RegistryKind::RegisterCustodianApplication => {
                    RegistryKind::RegisteredCustodianApplication
                }
                RegistryKind::AttachApplication => RegistryKind::ApplicationInstalled,
                RegistryKind::Bind => RegistryKind::Bound,
                _ => return Err(invalid("invalid supervisor registration request")),
            };
            if response.kind != expected
                || response.sequence != packet.sequence
                || response.nonce != packet.nonce
                || (matches!(kind, RegistryKind::Bind | RegistryKind::AttachApplication)
                    && response.registration != registration)
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
