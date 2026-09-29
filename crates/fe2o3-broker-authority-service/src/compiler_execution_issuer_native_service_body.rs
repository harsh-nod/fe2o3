// One bounded native dispatch loop; owners and codecs are nominal family bindings.
const FIXED_WORK: usize = 64 * 1024;
const FRAME: usize = 32 * (REQUEST_BYTES + RESPONSE_BYTES + Ledger::STORAGE) + 65536;
const IO_ATTEMPTS: usize = 256;

#[path = "compiler_execution_issuer_native_session.rs"]
mod session;
use session::{PublicationGuard, Session};

impl<'work> Admission<'work> {
    /// Consumes native custody and the original work ledger into the canonical
    /// bounded native transport. Only cancellation returns successfully.
    /// Prepared challenges and signed receipts are sent only after durable commit
    /// and fresh retained-custody checks. Issue accepts only the independently
    /// observed, still-locked publication occurrence retained since Prepare.
    /// A pending journal recovered without that live owner cannot resume issuance.
    ///
    /// Publication requires a durable independently signed anchor transition and
    /// exact Worker reacquisition before issuer advancement or an ACK. Currentness
    /// requires a fresh external observation joined to the reacquired carriage.
    /// No readiness descriptor is written and the V1 entrypoint is unchanged.
    /// Linux inspection denial is terminal; no caller-supplied subject or alternate
    /// permission path replaces the observed compiler. The caller must retain all
    /// input storage reservations. Work, denial history and I/O attempts never reset.
    ///
    /// ```compile_fail
    /// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// fn reuse(a: ProtectedCompilerExecutionIssuerAdmissionV2<'_>, b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let _ = a.serve_native_preparation(b);
    ///     let _ = a.serve_native_preparation(b);
    /// }
    /// ```
    pub fn serve_native_preparation(self, b: &mut Budget<'_>) -> Result<()> {
        self.serve_native(None, b)
    }

    /// Prepaid input charge for the consumed private readiness pipe writer.
    pub const READINESS_WRITER_STORAGE: usize = readiness::WRITER_STORAGE;

    /// Serves the same native protocol after publishing exact native launch readiness.
    /// The manifest must name the admitted client, anchor and policy. Publication
    /// follows singleton recovery, anchor retention and fresh custody/journal
    /// validation. The writer is consumed and closed, including on refusal.
    /// Readiness is not proof of compiler execution or permission to inspect a
    /// future occurrence; Prepare/Issue still require independent observation.
    /// Keep the manifest and READINESS_WRITER_STORAGE prepaid on the original
    /// ledger. The launch supervisor must retain exclusive pipe-reader custody.
    ///
    /// ```compile_fail
    /// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2 as A;
    /// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV1 as M;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
    /// fn mix(a: A<'_>, m: &M, fd: std::os::fd::OwnedFd, b: &mut B<'_>) {
    ///     a.serve_native_with_readiness(m, fd, b);
    /// }
    /// ```
    pub fn serve_native_with_readiness(
        self,
        manifest: &Manifest,
        writer: OwnedFd,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.serve_native(Some((manifest, writer)), b)
    }

    fn serve_native(self, ready: Option<(&Manifest, OwnedFd)>, b: &mut Budget<'_>) -> Result<()> {
        // Reject a foreign ledger before any directory or transport I/O.
        self.validate_continuity(b)?;
        let floor = self
            .retained_storage()
            .checked_add(
                ready
                    .as_ref()
                    .map_or(0, |(m, _)| m.retained_storage() + readiness::WRITER_STORAGE),
            )
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, FIXED_WORK, FRAME, |b| {
            if let Some((manifest, writer)) = &ready {
                readiness::check_binding(&self, manifest, b)?;
                readiness::check_writer(writer, b)?;
            }
            let mut ledger = Ledger::recover(
                self.service.service_root(),
                &self.policy,
                &self.signing_key,
                b,
            )?;
            b.reserve_storage(Ledger::STORAGE)?;
            self.validate_continuity(b)?;
            let mut anchor = NativeAnchor::new(&self.anchor, &self.policy, b)?;
            b.reserve_storage(anchor.retained_storage())?;
            let deadline = Instant::now()
                .checked_add(COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1)
                .ok_or_else(|| Error::rejected("native service deadline overflow"))?;
            let mut attempts = 0;
            let mut session = Session::default();
            if let Some((manifest, writer)) = ready {
                readiness::publish(manifest, &self.policy, writer, b, |b| {
                    self.validate_continuity(b)?;
                    ledger.validate(b)?;
                    if Instant::now() >= deadline {
                        return Err(Error::rejected("native readiness session deadline expired"));
                    }
                    Ok(())
                })?;
            }
            for _ in 0..MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1 {
                let retained = session.retained_storage();
                let floor = self
                    .retained_storage()
                    .checked_add(Ledger::STORAGE)
                    .and_then(|n| n.checked_add(anchor.retained_storage()))
                    .and_then(|n| n.checked_add(retained))
                    .ok_or(Resource::Arithmetic)?;
                let cancelled = b.with_prepaid_scope(floor, 8, FIXED_WORK, FRAME, |b| {
                    self.validate_continuity(b)?;
                    ledger.validate(b)?;
                    let bytes = receive_packet_metered::<REQUEST_BYTES, Error>(
                        self.service.service_peer(),
                        self.service.client_pidfd(),
                        deadline,
                        &mut || permit_io(b, &mut attempts),
                    )?;
                    self.validate_continuity(b)?;
                    let packet = retain(Packet::decode(bytes.as_slice(), b)?, b)?;
                    let response = dispatch(
                        &self,
                        &mut ledger,
                        &mut anchor,
                        &mut session,
                        &packet,
                        deadline,
                        &mut attempts,
                        b,
                    )?;
                    self.validate_continuity(b)?;
                    ledger.validate(b)?;
                    if packet.kind() != Kind::Cancel {
                        session.validate(&self, &ledger.record, b)?;
                    }
                    send_packet_metered(
                        self.service.service_peer(),
                        self.service.client_pidfd(),
                        response.canonical_bytes(),
                        deadline,
                        &mut || permit_io(b, &mut attempts),
                    )?;
                    self.validate_continuity(b)?;
                    if packet.kind() != Kind::Cancel {
                        session.validate(&self, &ledger.record, b)?;
                    }
                    Ok::<_, Error>(packet.kind() == Kind::Cancel)
                })?;
                // The packet scope restores its entry charge even when ownership
                // changes. Keep the live occurrence fully prepaid between packets.
                let next = session.retained_storage();
                if next >= retained {
                    b.reserve_storage(next - retained)?;
                } else {
                    b.release_storage(retained - next)?;
                }
                if cancelled {
                    return Ok(());
                }
            }
            Err(Error::rejected("native service packet limit exhausted"))
        })
    }
}

fn dispatch(
    a: &Admission<'_>,
    ledger: &mut Ledger,
    anchor: &mut NativeAnchor<'_>,
    session: &mut Session,
    packet: &Packet,
    deadline: Instant,
    attempts: &mut usize,
    b: &mut Budget<'_>,
) -> Result<Response> {
    if packet.policy_identity() != a.policy.identity() {
        return Err(Error::rejected("native service policy mismatch"));
    }
    if packet.kind() != Kind::Cancel {
        session.validate(a, &ledger.record, b)?;
    }
    match packet.kind() {
        Kind::Prepare => {
            require_position(ledger, packet)?;
            session.prepare(a, &ledger.record, b)?;
            let occurrence = session.occurrence()?;
            let nonce = fresh_nonce(b)?;
            occurrence.revalidate(&a.service, b)?;
            a.validate_continuity(b)?;
            let next = ledger
                .record
                .prepare(&a.policy, &a.signing_key, occurrence, nonce, b)?;
            b.reserve_storage(Record::STORAGE)?;
            occurrence.revalidate(&a.service, b)?;
            a.validate_continuity(b)?;
            ledger.commit(next, b)?;
            occurrence.revalidate(&a.service, b)?;
        }
        Kind::Issue => {
            require_position(ledger, packet)?;
            let request = retain(packet.decode_request(b)?, b)?;
            let occurrence = session.occurrence()?;
            occurrence.revalidate(&a.service, b)?;
            a.validate_continuity(b)?;
            if let Body::Issued { request: held, .. } = &ledger.record.body {
                // Lost-response replay retains the original lock and token; it
                // neither signs twice nor acquires an equivalent new occurrence.
                if held.canonical_bytes() != request.canonical_bytes()
                    || occurrence.identity() != &ledger.record.occurrence
                    || occurrence.subject().canonical_bytes() != request.subject().canonical_bytes()
                {
                    return Err(Error::rejected(
                        "issued replay changed request or occurrence",
                    ));
                }
            } else {
                let next =
                    ledger
                        .record
                        .issue(&a.policy, &a.signing_key, occurrence, request, b)?;
                b.reserve_storage(Record::STORAGE)?;
                occurrence.revalidate(&a.service, b)?;
                a.validate_continuity(b)?;
                ledger.commit(next, b)?;
            }
            occurrence.revalidate(&a.service, b)?;
        }
        Kind::Publish => {
            let request = retain(packet.decode_request(b)?, b)?;
            let publication = retain(packet.decode_publication(b)?, b)?;
            let guard = session.publication_guard(a, &ledger.record)?;
            let (ack, advanced) = ledger.publish(
                &a.policy,
                &a.signing_key,
                &request,
                &publication,
                &guard,
                &mut |c, b| {
                    a.validate_continuity(b)?;
                    let receipt = anchor.exchange(c, deadline, attempts, b)?;
                    a.validate_continuity(b)?;
                    Ok(receipt)
                },
                b,
            )?;
            b.reserve_storage(ack.retained_storage())?;
            a.validate_continuity(b)?;
            session.retire(a, ledger, &publication, &ack, b)?;
            return retain(
                Response::new(
                    packet.identity(),
                    &a.policy,
                    Payload::Published {
                        acknowledgment: &ack,
                        disposition: if advanced {
                            Disposition::Advanced
                        } else {
                            Disposition::AlreadyAcknowledged
                        },
                    },
                    b,
                )?,
                b,
            );
        }
        Kind::VerifyCurrent => {
            let carriage = retain(packet.decode_carriage(b)?, b)?;
            let challenge = packet
                .verification_challenge()
                .ok_or_else(|| Error::rejected("native currentness challenge absent"))?;
            let current = ledger.verify_current(
                &carriage,
                challenge,
                &mut |c, b| {
                    a.validate_continuity(b)?;
                    let receipt = anchor.exchange(c, deadline, attempts, b)?;
                    a.validate_continuity(b)?;
                    Ok(receipt)
                },
                b,
            )?;
            b.reserve_storage(std::mem::size_of::<(Current, ProtocolStorage)>())?;
            let attestation = retain(
                a.signing_key
                    .attest_current(&a.policy, &carriage, current, challenge, b)?,
                b,
            )?;
            a.validate_continuity(b)?;
            ledger.validate(b)?;
            return retain(
                Response::new(
                    packet.identity(),
                    &a.policy,
                    Payload::VerifiedCurrent(&attestation),
                    b,
                )?,
                b,
            );
        }
        Kind::Recover => {
            let subject = retain(packet.decode_subject(b)?, b)?;
            let carriage = ledger.recover_carriage(&subject, b)?;
            let payload = if let Some(carriage) = &carriage {
                b.reserve_storage(carriage.retained_storage())?;
                Payload::Recovered(carriage)
            } else {
                Payload::ReceiptAbsent {
                    sequence: ledger.record.sequence,
                    prior_rollback_anchor: ledger.record.prior,
                }
            };
            return retain(Response::new(packet.identity(), &a.policy, payload, b)?, b);
        }
        Kind::Inspect | Kind::Cancel => (),
    }
    let publication;
    let payload = match packet.kind() {
        Kind::Cancel => Payload::Cancelled {
            sequence: ledger.record.sequence,
            prior_rollback_anchor: ledger.record.prior,
        },
        Kind::Recover => Payload::ReceiptAbsent {
            sequence: ledger.record.sequence,
            prior_rollback_anchor: ledger.record.prior,
        },
        _ => match &ledger.record.body {
            Body::Ready => Payload::Ready {
                sequence: ledger.record.sequence,
                prior_rollback_anchor: ledger.record.prior,
            },
            Body::Prepared { challenge, .. } => Payload::Prepared(challenge),
            Body::Issued { .. } => {
                publication = ledger.record.publication(b)?;
                Payload::Issued(&publication)
            }
        },
    };
    retain(Response::new(packet.identity(), &a.policy, payload, b)?, b)
}

fn require_position(ledger: &Ledger, packet: &Packet) -> Result<()> {
    if (
        packet.expected_sequence(),
        packet.expected_rollback_anchor(),
    ) != (ledger.record.sequence, ledger.record.prior)
    {
        return Err(Error::rejected(
            "native request changed the journal position",
        ));
    }
    Ok(())
}
fn fresh_nonce(b: &mut Budget<'_>) -> Result<[u8; 32]> {
    b.charge_work(4096)?;
    let mut nonce = [0; 32];
    let n = rustix::rand::getrandom(&mut nonce[..], rustix::rand::GetRandomFlags::empty())?;
    if n != nonce.len() || nonce == [0; 32] {
        return Err(Error::rejected("native nonce generation was incomplete"));
    }
    Ok(nonce)
}
fn permit_io(b: &mut Budget<'_>, attempts: &mut usize) -> Result<()> {
    b.charge_work(4096)?;
    if *attempts >= IO_ATTEMPTS {
        return Err(Error::rejected("native service I/O attempt bound"));
    }
    *attempts += 1;
    Ok(())
}
fn retain<T>((value, charge): (T, ProtocolStorage), b: &mut Budget<'_>) -> Result<T> {
    b.reserve_storage(charge.additional_storage())?;
    Ok(value)
}

#[cfg(test)]
#[path = "compiler_execution_issuer_native_service_tests.rs"]
mod tests;

#[cfg(test)]
pub(in crate::compiler_execution_issuer) fn recover_family_fixture(
    root: &std::fs::File,
    published: bool,
    b: &mut Budget<'_>,
) -> Result<()> {
    tests::recover_family_fixture(root, published, b)
}
