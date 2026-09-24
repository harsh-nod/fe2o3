//! Consuming first-sequence native issuer session. Worker publication is gated.
use super::{Admission, Budget, Key, KeyError, Policy, Resource};
use crate::compiler_execution_occurrence::NativeOccurrence;
use crate::compiler_execution_service::{
    COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1, MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1,
    receive_packet_metered, send_packet_metered,
};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2 as Subject;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceRequestKindV2 as Kind, CompilerExecutionServiceRequestV2 as Packet,
    CompilerExecutionServiceResponsePayloadV2 as Payload,
    CompilerExecutionServiceResponseV2 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2 as REQUEST_BYTES,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2 as RESPONSE_BYTES,
};
use std::time::Instant;

#[path = "compiler_execution_issuer_native_error.rs"]
mod error;
#[path = "compiler_execution_issuer_native_ledger.rs"]
mod ledger;
#[path = "compiler_execution_issuer_native_record.rs"]
mod record;
pub use error::NativeIssuerServiceError;
use error::NativeIssuerServiceError as Error;
type Result<T> = std::result::Result<T, Error>;
use ledger::Ledger;
use record::{Body, Record};

const FIXED_WORK: usize = 64 * 1024;
const FRAME: usize = 8 * (REQUEST_BYTES + RESPONSE_BYTES + Record::STORAGE) + 65536;
const IO_ATTEMPTS: usize = 256;

impl<'work> Admission<'work> {
    /// Consumes native custody and the original work ledger into the canonical
    /// bounded Prepare/Issue transport. Only cancellation returns successfully.
    /// Prepared challenges and signed receipts are sent only after durable commit
    /// and fresh retained-custody checks. Issue accepts only the independently
    /// observed, still-locked V4 occurrence named by the prepared record.
    ///
    /// This is a first-sequence integration boundary, not production activation:
    /// Worker/anchor publication, currentness replies and non-genesis positions
    /// refuse. No readiness descriptor is written and the V1 entrypoint is unchanged.
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
        // Reject a foreign ledger before any directory or transport I/O.
        self.validate_continuity(b)?;
        b.with_prepaid_scope(self.retained_storage(), 8, FIXED_WORK, FRAME, |b| {
            let mut ledger = Ledger::recover(
                self.service.service_root(),
                &self.policy,
                &self.signing_key,
                b,
            )?;
            b.reserve_storage(Ledger::STORAGE)?;
            self.validate_continuity(b)?;
            let deadline = Instant::now()
                .checked_add(COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1)
                .ok_or_else(|| Error::rejected("native service deadline overflow"))?;
            let mut attempts = 0;
            for _ in 0..MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1 {
                let cancelled = b.with_prepaid_scope(
                    self.retained_storage() + Ledger::STORAGE,
                    8,
                    FIXED_WORK,
                    FRAME,
                    |b| {
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
                        let response = dispatch(&self, &mut ledger, &packet, b)?;
                        self.validate_continuity(b)?;
                        ledger.validate(b)?;
                        send_packet_metered(
                            self.service.service_peer(),
                            self.service.client_pidfd(),
                            response.canonical_bytes(),
                            deadline,
                            &mut || permit_io(b, &mut attempts),
                        )?;
                        self.validate_continuity(b)?;
                        Ok::<_, Error>(packet.kind() == Kind::Cancel)
                    },
                )?;
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
    packet: &Packet,
    b: &mut Budget<'_>,
) -> Result<Response> {
    if packet.policy_identity() != a.policy.identity() {
        return Err(Error::rejected("native service policy mismatch"));
    }
    match packet.kind() {
        Kind::Prepare => {
            require_position(ledger, packet)?;
            if !matches!(ledger.record.body, Body::Ready) {
                return Err(Error::rejected("prepare requires a ready native journal"));
            }
            let (occurrence, charge) = NativeOccurrence::observe(&a.service, b)?;
            b.reserve_storage(charge)?;
            let nonce = fresh_nonce(b)?;
            occurrence.revalidate(&a.service, b)?;
            let next = ledger
                .record
                .prepare(&a.policy, &a.signing_key, &occurrence, nonce, b)?;
            b.reserve_storage(Record::STORAGE)?;
            occurrence.revalidate(&a.service, b)?;
            a.validate_continuity(b)?;
            ledger.commit(next, b)?;
            occurrence.revalidate(&a.service, b)?;
        }
        Kind::Issue => {
            require_position(ledger, packet)?;
            let request = retain(packet.decode_request(b)?, b)?;
            let (occurrence, charge) = NativeOccurrence::observe(&a.service, b)?;
            b.reserve_storage(charge)?;
            occurrence.revalidate(&a.service, b)?;
            if let Body::Issued { request: held, .. } = &ledger.record.body {
                // Lost-response replay observes the original occurrence again; it
                // neither signs twice nor substitutes an equivalent new process.
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
                        .issue(&a.policy, &a.signing_key, &occurrence, request, b)?;
                b.reserve_storage(Record::STORAGE)?;
                occurrence.revalidate(&a.service, b)?;
                a.validate_continuity(b)?;
                ledger.commit(next, b)?;
            }
            occurrence.revalidate(&a.service, b)?;
        }
        Kind::Publish | Kind::VerifyCurrent => {
            return Err(Error::rejected(
                "native Worker/anchor publication and currentness are not integrated",
            ));
        }
        Kind::Inspect | Kind::Recover | Kind::Cancel => (),
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
