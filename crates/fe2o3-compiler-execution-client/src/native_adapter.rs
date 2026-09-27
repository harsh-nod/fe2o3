// One native exchange implementation; nominal families supply actual typed records.
macro_rules! native_client_adapter {
    ($Error:ident, $Storage:ident, $Client:ident, $Recovery:ident, $verify_current:ident) => {
        #[derive(Debug)]
        pub enum $Error {
            Resource(Resource),
            Transport(TransportError),
            Protocol(ProtocolError),
            Attestation(AttestationError),
            Publication(PublicationError),
            Journal(JournalError),
            Mismatch(&'static str),
        }
        type ClientError = $Error;
        type Result<T> = std::result::Result<T, ClientError>;

        /// Full logical output charge, returned unreserved on the original ledger.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $Storage(usize);
        impl $Storage {
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }

        /// Terminal recovery result, not compiler or currentness authority.
        /// Absence is returned only after an acknowledged Cancel at the same
        /// durable position. Recovery never prepares, issues or publishes a receipt.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug)]
        pub enum $Recovery {
            Recovered(Carriage),
            Absent { sequence: u64, rollback_anchor: [u8; 32] },
        }
        impl $Recovery {
            fn retained_storage(&self) -> Result<usize> {
                let payload = match self {
                    Self::Recovered(carriage) => carriage.retained_storage(),
                    Self::Absent { .. } => 0,
                };
                // Conservatively includes the inline carriage twice: its native
                // full-owner charge and the fixed recovery/result envelope.
                payload.checked_add(size_of::<(Self, $Storage)>())
                    .ok_or_else(|| Resource::Arithmetic.into())
            }
        }
        macro_rules! from_error {
            ($ty:ty, $variant:ident) => {
                impl From<$ty> for ClientError {
                    fn from(error: $ty) -> Self {
                        Self::$variant(error)
                    }
                }
            };
        }
        from_error!(Resource, Resource);
        from_error!(TransportError, Transport);
        from_error!(ProtocolError, Protocol);
        from_error!(AttestationError, Attestation);
        from_error!(PublicationError, Publication);
        from_error!(JournalError, Journal);
        impl fmt::Display for ClientError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Mismatch(reason) => f.write_str(reason),
                    Self::Resource(e) => fmt::Display::fmt(e, f),
                    Self::Transport(e) => fmt::Display::fmt(e, f),
                    Self::Protocol(e) => fmt::Display::fmt(e, f),
                    Self::Attestation(e) => fmt::Display::fmt(e, f),
                    Self::Publication(e) => fmt::Display::fmt(e, f),
                    Self::Journal(e) => fmt::Display::fmt(e, f),
                }
            }
        }
        impl Error for ClientError {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Transport(e) => Some(e),
                    Self::Protocol(e) => Some(e),
                    Self::Attestation(e) => Some(e),
                    Self::Publication(e) => Some(e),
                    Self::Journal(e) => Some(e),
                    Self::Mismatch(_) => None,
                }
            }
        }

        /// One terminal native session, exclusively borrowing the original ledger.
        /// Each operation's input storage must be prepaid before that operation.
        /// Admission transfers the peer charge to this owner, released on drop.
        /// Native packets are never retried as V1 and no decoded V1 issuer owner is used.
        /// This diagnostic API does not activate the production issuer or prove its
        /// protected key custody, live-rustc observation, durability, or GPU authority.
        ///
        #[doc = concat!("```compile_fail\nuse fe2o3_compiler_execution_client::", stringify!($Client), " as Client;\nuse fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;\nfn replace<'a>(peer: std::os::fd::OwnedFd, b: &'a mut Budget<'a>, other: Budget<'a>) {\n    let client = Client::admit(peer, std::time::Duration::from_secs(1), b).unwrap();\n    *b = other;\n    drop(client);\n}\n```\n```compile_fail\nuse fe2o3_compiler_execution_client::", stringify!($Client), ";\nfn duplicate<T: Clone>() {}\nduplicate::<", stringify!($Client), "<'static, 'static>>();\n```\n```compile_fail\nuse fe2o3_compiler_execution_client::", stringify!($Client), ";\nfn expose<T: std::os::fd::AsFd>() {}\nexpose::<", stringify!($Client), "<'static, 'static>>();\n```")]
        pub struct $Client<'budget, 'work> {
            peer: Option<OwnedFd>,
            deadline: Instant,
            budget: &'budget mut Budget<'work>,
            retained: usize,
        }

        impl<'budget, 'work> $Client<'budget, 'work> {
            pub const PEER_STORAGE: usize = size_of::<OwnedFd>();
            pub const SUBJECT_STORAGE: usize = size_of::<Subject>() + size_of::<SubjectStorage>();
            const RETAINED: usize = size_of::<Self>();
            const SESSION_SCRATCH: usize = 16 * MAX_RESPONSE + 8 * MAX_REQUEST + 8192;

            /// Consumes a prepaid unnamed connected SOCK_SEQPACKET peer.
            /// On admission failure the descriptor is closed but its input reservation
            /// remains caller-owned. Successful admission releases that charge on drop.
            pub fn admit(
                peer: OwnedFd,
                timeout: Duration,
                budget: &'budget mut Budget<'work>,
            ) -> Result<Self> {
                let deadline = budget.with_prepaid_scope(Self::PEER_STORAGE, 8, 64 * 1024, 8192, |_| {
                    if timeout.is_zero() || timeout > Duration::from_secs(300) {
                        return Err(ClientError::Transport(TransportError::InvalidTimeout));
                    }
                    set_close_on_exec(&peer)?;
                    validate_seqpacket_peer(&peer)?;
                    Instant::now()
                        .checked_add(timeout)
                        .ok_or_else(|| TransportError::DeadlineOverflow.into())
                })?;
                budget.reserve_storage(Self::RETAINED - Self::PEER_STORAGE)?;
                Ok(Self {
                    peer: Some(peer),
                    deadline,
                    budget,
                    retained: Self::RETAINED,
                })
            }

            /// Runs preparation on the admitted session's original account.
            /// The callback cannot access this client or exchange on this session.
            /// It must finish all preparation before publishing external results.
            /// Live input/output storage stays charged; errors and unwind do not
            /// roll back preparation charges. Only a successful account postcheck
            /// returns the client for its terminal exchange. The original absolute
            /// deadline is not extended by preparation.
            #[doc = concat!("```\nuse fe2o3_compiler_execution_client::{", stringify!($Client), " as Client, ", stringify!($Error), " as Error};\nfn prepare<'b, 'w>(client: Client<'b, 'w>) -> Result<(Client<'b, 'w>, u32), Error> {\n    client.prepare(|budget| { budget.charge_work(1)?; Ok(7) })\n}\n```\n```compile_fail\nuse fe2o3_compiler_execution_client::{", stringify!($Client), " as Client, ", stringify!($Error), " as Error};\nfn reuse(client: Client<'_, '_>) {\n    let _ = client.prepare::<(), Error>(|_| Ok(()));\n    let _ = client.prepare::<(), Error>(|_| Ok(()));\n}\n```")]
            pub fn prepare<T, E>(
                mut self,
                run: impl FnOnce(&mut Budget<'work>) -> std::result::Result<T, E>,
            ) -> std::result::Result<(Self, T), E>
            where
                E: From<ClientError>,
            {
                let value = self.checked(run)?;
                Ok((self, value))
            }

            /// Consumes one original-account preparation/publication/receipt flow.
            /// Publication runs only after preparation's account postcheck succeeds
            /// and the original deadline is still live. It must return a prepaid
            /// subject and retain all required preparation owners in `R`.
            /// Acquisition uses the existing native state machine exactly once.
            /// The peer then closes, and the full carriage is reserved before
            /// `finish` can publish transport or return retained output. No callback
            /// may lower its inherited storage floor; consumed input charges are
            /// caller-owned and are not automatically retired. Errors and unwind
            /// retain opaque inner charges, never restore a replaced account, and
            /// never retry publication or acquisition.
            ///
            /// This orders client operations, not compiler authority: the caller
            /// must supply genuine prepared ownership and independently pinned
            /// policy. Callbacks, published bytes and fixture receipts are not proof.
            pub fn prepare_and_acquire<P, R, T, E>(
                self,
                policy: &Policy,
                prepare: impl FnOnce(&mut Budget<'work>) -> std::result::Result<P, E>,
                publish: impl FnOnce(P, &mut Budget<'work>) -> std::result::Result<(Subject, R), E>,
                finish: impl FnOnce(Carriage, R, &mut Budget<'work>) -> std::result::Result<T, E>,
            ) -> std::result::Result<T, E>
            where
                E: From<ClientError>,
            {
                let floor = input_floor(self.retained, policy.retained_storage(), 0)?;
                let (mut client, prepared) = self.prepare(|budget| {
                    if budget.storage() < floor {
                        return Err(ClientError::Resource(Resource::Accounting).into());
                    }
                    prepare(budget)
                })?;
                let deadline = client.deadline;
                let (subject, retained) = client.checked(|budget| {
                    if Instant::now() >= deadline {
                        return Err(ClientError::Transport(TransportError::Timeout).into());
                    }
                    publish(prepared, budget)
                })?;
                let (carriage, storage) = client.acquire_borrowed(policy, subject)?;
                drop(client.peer.take());
                client.budget.reserve_storage(storage.additional_storage())
                    .map_err(ClientError::from)?;
                client.checked(|budget| finish(carriage, retained, budget))
            }

            // Only consuming public operations call this guard. An error therefore
            // closes the session even when it leaves the original account charged.
            fn checked<T, E>(
                &mut self,
                run: impl FnOnce(&mut Budget<'work>) -> std::result::Result<T, E>,
            ) -> std::result::Result<T, E>
            where
                E: From<ClientError>,
            {
                self.budget.charge_work(8).map_err(ClientError::from)?;
                let floor = self.budget.storage();
                let account = self.budget.work_ledger_identity_v1();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(self.budget)
                }));
                let intact = account == self.budget.work_ledger_identity_v1()
                    && self.budget.storage() >= floor;
                if !intact {
                    // Never refund our reservation against a replaced account or
                    // damaged floor. Drop closes the peer without repairing either.
                    self.retained = 0;
                }
                match result {
                    Err(payload) => std::panic::resume_unwind(payload),
                    Ok(result) if !intact => {
                        drop(result);
                        Err(ClientError::Resource(Resource::Accounting).into())
                    }
                    Ok(result) => result,
                }
            }

            /// Executes Recover/Inspect and the required Prepare/Issue/Publish suffix.
            /// The subject is an expected input, never evidence that this process compiled it.
            /// Returns a full carriage charge; retire the consumed subject's charge separately.
            pub fn acquire(
                mut self,
                policy: &Policy,
                subject: Subject,
            ) -> Result<(Carriage, $Storage)> {
                self.acquire_borrowed(policy, subject)
            }

            fn acquire_borrowed(
                &mut self,
                policy: &Policy,
                subject: Subject,
            ) -> Result<(Carriage, $Storage)> {
                let floor = input_floor(
                    self.retained,
                    policy.retained_storage(),
                    Self::SUBJECT_STORAGE,
                )?;
                let peer = self
                    .peer
                    .as_ref()
                    .ok_or(ClientError::Mismatch("closed native peer"))?;
                let deadline = self.deadline;
                let carriage = self
                    .budget
                    .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                        Session::new(peer, deadline, b).acquire(policy, subject)
                    })?;
                let charge = $Storage(carriage.retained_storage());
                Ok((carriage, charge))
            }

            /// Recover an exact existing receipt, or acknowledge Cancel on absence.
            /// This consumes the session without any Inspect/Prepare/Issue/Publish
            /// suffix. Policy and subject must remain prepaid on this account.
            /// Reserve the returned full output charge before retaining/using it;
            /// consumed input charges remain caller-owned. No V1 fallback exists.
            pub fn recover_only(
                self,
                policy: &Policy,
                subject: Subject,
            ) -> Result<($Recovery, $Storage)> {
                let floor = input_floor(self.retained, policy.retained_storage(), Self::SUBJECT_STORAGE)?;
                let peer = self.peer.as_ref()
                    .ok_or(ClientError::Mismatch("closed native peer"))?;
                self.budget.with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                    let mut session = Session::new(peer, self.deadline, b);
                    let recovered = session.recover(policy, &subject)?;
                    if let $Recovery::Absent { sequence, rollback_anchor } = &recovered {
                        let response = session.exchange(policy, Payload::Cancel)?;
                        require_cancel_position(&response, (*sequence, *rollback_anchor), session.budget)?;
                    }
                    let charge = recovered.retained_storage()?;
                    session.budget.reserve_storage(charge)?;
                    Ok((recovered, $Storage(charge)))
                })
            }

            /// Consumes a ready session by exchanging one exact acknowledged Cancel.
            /// This does not authenticate readiness or a compiler result. The caller
            /// must first validate the supervisor's readiness/launch binding. Policy
            /// and request joins, framing, deadline and all I/O use the retained budget.
            pub fn cancel(self, policy: &Policy) -> Result<()> {
                let floor = input_floor(self.retained, policy.retained_storage(), 0)?;
                let peer = self
                    .peer
                    .as_ref()
                    .ok_or(ClientError::Mismatch("closed native peer"))?;
                let deadline = self.deadline;
                self.budget
                    .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                        let response = Session::new(peer, deadline, b).exchange(policy, Payload::Cancel)?;
                        require_kind(&response, Kind::Cancelled)
                    })
            }

            /// Generates a fresh OS challenge and authenticates both signatures over the
            /// exact native carriage. This is terminal even when the peer refuses it.
            pub fn verify_current_only(
                self,
                policy: &Policy,
                carriage: &Carriage,
            ) -> Result<(VerifiedCurrent, $Storage)> {
                let floor = input_floor(
                    self.retained,
                    policy.retained_storage(),
                    carriage.retained_storage(),
                )?;
                let peer = self
                    .peer
                    .as_ref()
                    .ok_or(ClientError::Mismatch("closed native peer"))?;
                let deadline = self.deadline;
                self.budget
                    .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                        let mut session = Session::new(peer, deadline, b);
                        if policy.canonical_bytes() != carriage.policy().canonical_bytes() {
                            return Err(ClientError::Mismatch("native currentness policy mismatch"));
                        }
                        let challenge =
                            super::fresh_verification_challenge_with_io(&mut || session.attempt())?;
                        let response = session.exchange(
                            policy,
                            Payload::VerifyCurrent {
                                carriage,
                                verification_challenge: *challenge.as_bytes(),
                            },
                        )?;
                        require_kind(&response, Kind::VerifiedCurrent)?;
                        let attestation = retain(
                            response.decode_current_record_attestation(session.budget)?,
                            session.budget,
                        )?;
                        let (verified, growth) = attestation.$verify_current(
                            policy,
                            carriage,
                            *challenge.as_bytes(),
                            session.budget,
                        )?;
                        session
                            .budget
                            .reserve_storage(growth.additional_storage())?;
                        Ok((
                            verified,
                            $Storage(size_of::<(VerifiedCurrent, Storage)>()),
                        ))
                    })
            }
        }

        impl Drop for $Client<'_, '_> {
            fn drop(&mut self) {
                drop(self.peer.take());
                // Exclusive ledger custody and restoring scopes preserve this reservation.
                let _ = self.budget.release_storage(self.retained);
            }
        }

        struct Session<'a, 'work> {
            peer: &'a OwnedFd,
            deadline: Instant,
            budget: &'a mut Budget<'work>,
            packets: usize,
            attempts: usize,
        }
        impl<'a, 'work> Session<'a, 'work> {
            fn new(peer: &'a OwnedFd, deadline: Instant, budget: &'a mut Budget<'work>) -> Self {
                Self {
                    peer,
                    deadline,
                    budget,
                    packets: 0,
                    attempts: 0,
                }
            }
            fn attempt(&mut self) -> Result<()> {
                self.budget
                    .charge_work(1024 + MAX_RESPONSE.max(MAX_REQUEST))?;
                self.attempts = self.attempts.checked_add(1).ok_or(Resource::Arithmetic)?;
                if self.attempts > 256 {
                    return Err(ClientError::Mismatch("native client I/O attempt limit"));
                }
                Ok(())
            }
            fn exchange(&mut self, policy: &Policy, payload: Payload<'_>) -> Result<Response> {
                self.budget.charge_work(8)?;
                if self.packets == 8 {
                    return Err(ClientError::Mismatch("native client packet limit"));
                }
                self.packets += 1;
                let request = retain(
                    ServiceRequest::new(policy, payload, self.budget)?,
                    self.budget,
                )?;
                let peer = self.peer;
                let deadline = self.deadline;
                send_packet_with_io(peer, request.canonical_bytes(), deadline, &mut || {
                    self.attempt()
                })?;
                let packet =
                    receive_packet_with_io::<MAX_RESPONSE, ClientError>(peer, deadline, &mut || {
                        self.attempt()
                    })?;
                let response = retain(
                    Response::decode(packet.as_slice(), self.budget)?,
                    self.budget,
                )?;
                if response.policy_identity() != policy.identity()
                    || response.request_identity() != request.identity()
                {
                    return Err(ClientError::Mismatch(
                        "native response policy or request identity mismatch",
                    ));
                }
                Ok(response)
            }

            fn recover(&mut self, policy: &Policy, subject: &Subject) -> Result<$Recovery> {
                self.budget.reserve_storage(size_of::<$Recovery>())?;
                let recovered = self.exchange(policy, Payload::Recover(subject))?;
                match recovered.kind() {
                    Kind::Recovered => {
                        let carriage = retain(recovered.decode_carriage(self.budget)?, self.budget)?;
                        self.budget.charge_work(policy.canonical_bytes().len() + subject.canonical_bytes().len())?;
                        if carriage.policy().canonical_bytes() != policy.canonical_bytes()
                            || carriage.request().subject().canonical_bytes() != subject.canonical_bytes()
                        {
                            return Err(ClientError::Mismatch(
                                "recovered native carriage differs from expected source",
                            ));
                        }
                        Ok($Recovery::Recovered(carriage))
                    }
                    Kind::ReceiptAbsent => Ok($Recovery::Absent {
                        sequence: recovered.sequence(), rollback_anchor: recovered.rollback_anchor(),
                    }),
                    _ => Err(ClientError::Mismatch(
                            "expected native recovered carriage or absence",
                    )),
                }
            }

            fn acquire(&mut self, policy: &Policy, subject: Subject) -> Result<Carriage> {
                let position = match self.recover(policy, &subject)? {
                    $Recovery::Recovered(carriage) => return Ok(carriage),
                    $Recovery::Absent { sequence, rollback_anchor } => (sequence, rollback_anchor),
                };
                let inspected = self.exchange(policy, Payload::Inspect)?;
                if (inspected.sequence(), inspected.rollback_anchor()) != position {
                    return Err(ClientError::Mismatch(
                        "native durable position changed after absence",
                    ));
                }
                let (request, publication) = match inspected.kind() {
                    Kind::Ready | Kind::Prepared => {
                        let prepared = if inspected.kind() == Kind::Ready {
                            self.exchange(
                                policy,
                                Payload::Prepare {
                                    sequence: position.0,
                                    prior_rollback_anchor: position.1,
                                },
                            )?
                        } else {
                            inspected
                        };
                        require_kind(&prepared, Kind::Prepared)?;
                        if (prepared.sequence(), prepared.rollback_anchor()) != position {
                            return Err(ClientError::Mismatch("native prepare position changed"));
                        }
                        let challenge = retain(prepared.decode_challenge(self.budget)?, self.budget)?;
                        if challenge.policy_identity() != policy.identity()
                            || !challenge.subject().matches_subject(&subject, self.budget)?
                        {
                            return Err(ClientError::Mismatch(
                                "native challenge names another source or policy",
                            ));
                        }
                        let request = retain(Request::new(challenge, subject, self.budget)?, self.budget)?;
                        let issued = self.exchange(policy, Payload::Issue(&request))?;
                        require_kind(&issued, Kind::Issued)?;
                        let publication = retain(issued.decode_publication(self.budget)?, self.budget)?;
                        verify_publication(policy, &request, &publication, self.budget)?;
                        (request, publication)
                    }
                    Kind::Issued => {
                        let publication = retain(inspected.decode_publication(self.budget)?, self.budget)?;
                        let receipt = publication.receipt();
                        let challenge = retain(
                            Challenge::new(
                                policy,
                                &subject,
                                receipt.challenge_nonce(),
                                receipt.sequence(),
                                receipt.prior_rollback_anchor(),
                                self.budget,
                            )?,
                            self.budget,
                        )?;
                        if challenge.identity() != receipt.challenge_identity() {
                            return Err(ClientError::Mismatch(
                                "native issued challenge cannot be reconstructed",
                            ));
                        }
                        let request = retain(Request::new(challenge, subject, self.budget)?, self.budget)?;
                        verify_publication(policy, &request, &publication, self.budget)?;
                        (request, publication)
                    }
                    _ => {
                        return Err(ClientError::Mismatch(
                            "unexpected native issuer recovery stage",
                        ));
                    }
                };
                let published = self.exchange(
                    policy,
                    Payload::Publish {
                        request: &request,
                        publication: &publication,
                    },
                )?;
                require_kind(&published, Kind::Published)?;
                let ack = retain(published.decode_acknowledgment(self.budget)?, self.budget)?;
                ack.matches_publication(&publication, self.budget)?;
                let policy = retain(
                    Policy::decode(policy.canonical_bytes(), self.budget)?,
                    self.budget,
                )?;
                let (carriage, delta) = Carriage::new(policy, request, publication, ack, self.budget)?;
                self.budget.reserve_storage(delta.additional_storage())?;
                Ok(carriage)
            }
        }

        fn retain<T>((value, charge): (T, Storage), budget: &mut Budget<'_>) -> Result<T> {
            budget.reserve_storage(charge.additional_storage())?;
            Ok(value)
        }
        fn input_floor(owner: usize, policy: usize, input: usize) -> Result<usize> {
            owner
                .checked_add(policy)
                .and_then(|n| n.checked_add(input))
                .ok_or_else(|| Resource::Arithmetic.into())
        }
        fn require_kind(response: &Response, expected: Kind) -> Result<()> {
            if response.kind() != expected {
                return Err(ClientError::Mismatch("unexpected native response kind"));
            }
            Ok(())
        }
        fn require_cancel_position(response: &Response, expected: (u64, [u8; 32]), b: &mut Budget<'_>) -> Result<()> {
            b.charge_work(1 + 8 + 32)?;
            require_kind(response, Kind::Cancelled)?;
            if (response.sequence(), response.rollback_anchor()) != expected {
                return Err(ClientError::Mismatch("native cancel position changed after absence"));
            }
            Ok(())
        }
        fn verify_publication(
            policy: &Policy,
            request: &Request,
            publication: &Publication,
            budget: &mut Budget<'_>,
        ) -> Result<()> {
            let receipt = retain(
                Receipt::decode(publication.receipt().canonical_bytes(), budget)?,
                budget,
            )?;
            let (_verified, growth) = receipt.verify(
                policy,
                request,
                request.challenge().prior_rollback_anchor(),
                budget,
            )?;
            budget.reserve_storage(growth.additional_storage())?;
            Ok(())
        }
    };
}
pub(crate) use native_client_adapter;
