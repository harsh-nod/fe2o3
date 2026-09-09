use std::error::Error;
use std::fmt;
use std::os::fd::OwnedFd;

use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_artifact_transaction::{
    AuthenticatedCompilerCapabilityCompletionErrorV5, AuthenticatedCompilerCapabilityCompletionV5,
    AuthenticatedCompilerCapabilityEvidenceIdentityV5, InertCompilerCapabilityVerifierResponseV5,
    MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5,
};
use fe2o3_compiler_ffi::{
    InertProductionCapabilityHandoffErrorV5, InertProductionCapabilityHandoffV5,
    InertProductionCapabilityTransactionV5, MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5,
};
use fe2o3_compiler_lineage::{
    MultiRootProofRosterKindV3, TargetMachineRefinementReceiptErrorV1,
    TargetMachineRefinementReceiptV1, TargetMachineRefinementTargetV1,
};
use fe2o3_hsaco_finalize::derive_unfinalized_hsaco_from_finalized_v1;
use fe2o3_verifier::{
    ProtectedCompilerCompletionInputErrorV5, ProtectedCompilerMultiRootProofInputsV5,
    compose_protected_compiler_completion_inputs_v5,
};
use fe2o3_worker_v3_verification_protocol::{
    ExactIdentityCoordinateV5, InertWorkerV3MachineRefinedFinalizationV5,
    MAX_WORKER_V3_MACHINE_REFINED_FINALIZATION_BYTES_V5,
    WorkerV3VerificationCapabilityCompletionV5, WorkerV3VerificationCapabilityProtocolErrorV5,
    WorkerV3VerificationCapabilityRequestV5, WorkerV3VerificationCapabilityResponseV5,
    WorkerV3VerificationProtectedEvidenceBindingIdentityV5,
};
use rustix::fs::{FileType, OFlags, SealFlags};
use sha2::{Digest as _, Sha256};

const REQUEST_MAGIC: [u8; 8] = *b"F2WVPR05";
const RESPONSE_MAGIC: [u8; 8] = *b"F2WVPS05";
const VERSION: u16 = 5;
const REQUEST_FIELDS: u16 = 4;
const RESPONSE_FIELDS: u16 = 6;
const HEADER_BYTES: usize = 24;
const FIELD_HEADER_BYTES: usize = 8;
const TERMINAL_BYTES: usize = 32;
const REQUEST_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-REQUEST/V5\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE/V5\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-REQUEST-IDENTITY/V5\0";
const RESPONSE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/PROTECTED-COMPLETION-RESPONSE-IDENTITY/V5\0";
const REQUIRED_KEY_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);
const TMPFS_MAGIC: u64 = 0x0102_1994;

pub const MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5: usize = HEADER_BYTES
    + FIELD_HEADER_BYTES * REQUEST_FIELDS as usize
    + REQUEST_DOMAIN.len()
    + 32
    + MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5
    + MAX_WORKER_V3_MACHINE_REFINED_FINALIZATION_BYTES_V5
    + TERMINAL_BYTES;

pub const MAX_WORKER_V3_PROTECTED_COMPLETION_RESPONSE_BYTES_V5: usize = HEADER_BYTES
    + FIELD_HEADER_BYTES * RESPONSE_FIELDS as usize
    + RESPONSE_DOMAIN.len()
    + 32
    + 32
    + fe2o3_worker_v3_verification_protocol::MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5
    + MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5
    + 64 + TERMINAL_BYTES;

/// Immutable canonical material sent to the protected verifier without any parent owner.
#[derive(Debug)]
pub struct WorkerV3ProtectedCompletionRequestV5 {
    request_binding: [u8; 32],
    transaction: InertProductionCapabilityTransactionV5,
    finalization: InertWorkerV3MachineRefinedFinalizationV5,
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
}

impl WorkerV3ProtectedCompletionRequestV5 {
    pub fn new(
        request_binding: WorkerV3VerificationProtectedEvidenceBindingIdentityV5,
        transaction: &InertProductionCapabilityTransactionV5,
        finalization: &InertWorkerV3MachineRefinedFinalizationV5,
    ) -> Result<Self, WorkerV3ProtectedCompletionErrorV5> {
        let fields: [&[u8]; REQUEST_FIELDS as usize] = [
            REQUEST_DOMAIN,
            request_binding.as_bytes(),
            transaction.canonical_bytes(),
            finalization.canonical_bytes(),
        ];
        Self::decode(&encode_record(
            REQUEST_MAGIC,
            REQUEST_FIELDS,
            &fields,
            MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
            REQUEST_IDENTITY_DOMAIN,
        )?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerV3ProtectedCompletionErrorV5> {
        let record = decode_record(
            bytes,
            REQUEST_MAGIC,
            REQUEST_FIELDS,
            MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
            REQUEST_IDENTITY_DOMAIN,
        )?;
        if record.fields[0] != REQUEST_DOMAIN {
            return Err(WorkerV3ProtectedCompletionErrorV5::WrongDomain);
        }
        let request_binding = array::<32>(record.fields[1])?;
        if request_binding == [0; 32] {
            return Err(WorkerV3ProtectedCompletionErrorV5::InvalidIdentity);
        }
        let transaction = InertProductionCapabilityTransactionV5::decode(record.fields[2])?;
        let finalization =
            InertWorkerV3MachineRefinedFinalizationV5::decode_canonical(record.fields[3])?;
        Ok(Self {
            request_binding,
            transaction,
            finalization,
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: record.identity,
        })
    }

    pub const fn request_binding_identity(&self) -> &[u8; 32] {
        &self.request_binding
    }

    pub const fn handoff(&self) -> &InertProductionCapabilityHandoffV5 {
        self.transaction.handoff()
    }

    pub const fn transaction(&self) -> &InertProductionCapabilityTransactionV5 {
        &self.transaction
    }

    pub const fn finalization(&self) -> &InertWorkerV3MachineRefinedFinalizationV5 {
        &self.finalization
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

/// One signed, request-correlated service result. Decoding remains authority-free.
#[derive(Debug)]
pub struct WorkerV3ProtectedCompletionResponseV5 {
    request: [u8; 32],
    challenge: [u8; 32],
    transport_response: WorkerV3VerificationCapabilityResponseV5,
    verifier_response: Box<[u8]>,
    checker_evidence_identity: AuthenticatedCompilerCapabilityEvidenceIdentityV5,
    signature: [u8; 64],
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
}

impl WorkerV3ProtectedCompletionResponseV5 {
    fn new(
        request: &WorkerV3VerificationCapabilityRequestV5,
        transport_response: WorkerV3VerificationCapabilityResponseV5,
        verifier_response: &[u8],
        signature: [u8; 64],
    ) -> Result<Self, WorkerV3ProtectedCompletionErrorV5> {
        let challenge = request.base_request().challenge();
        let request_identity = request.identity();
        let fields: [&[u8]; RESPONSE_FIELDS as usize] = [
            RESPONSE_DOMAIN,
            request_identity.as_bytes(),
            challenge.as_bytes(),
            transport_response.encode_canonical(),
            verifier_response,
            &signature,
        ];
        Self::decode(&encode_record(
            RESPONSE_MAGIC,
            RESPONSE_FIELDS,
            &fields,
            MAX_WORKER_V3_PROTECTED_COMPLETION_RESPONSE_BYTES_V5,
            RESPONSE_IDENTITY_DOMAIN,
        )?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerV3ProtectedCompletionErrorV5> {
        let record = decode_record(
            bytes,
            RESPONSE_MAGIC,
            RESPONSE_FIELDS,
            MAX_WORKER_V3_PROTECTED_COMPLETION_RESPONSE_BYTES_V5,
            RESPONSE_IDENTITY_DOMAIN,
        )?;
        if record.fields[0] != RESPONSE_DOMAIN {
            return Err(WorkerV3ProtectedCompletionErrorV5::WrongDomain);
        }
        let request_bytes = array::<32>(record.fields[1])?;
        let challenge = array::<32>(record.fields[2])?;
        if request_bytes == [0; 32] || challenge == [0; 32] {
            return Err(WorkerV3ProtectedCompletionErrorV5::InvalidIdentity);
        }
        let transport_response =
            WorkerV3VerificationCapabilityResponseV5::decode_canonical(record.fields[3])?;
        if transport_response.request_identity().as_bytes() != &request_bytes {
            return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
        }
        let verifier_response =
            InertCompilerCapabilityVerifierResponseV5::decode(record.fields[4])?;
        let checker_evidence_identity = verifier_response.checker_evidence_identity();
        let signature = array::<64>(record.fields[5])?;
        Ok(Self {
            request: request_bytes,
            challenge,
            transport_response,
            verifier_response: record.fields[4].to_vec().into_boxed_slice(),
            checker_evidence_identity,
            signature,
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: record.identity,
        })
    }

    pub const fn request_identity(&self) -> &[u8; 32] {
        &self.request
    }

    pub const fn challenge(&self) -> &[u8; 32] {
        &self.challenge
    }

    pub const fn transport_response(&self) -> &WorkerV3VerificationCapabilityResponseV5 {
        &self.transport_response
    }

    /// Returns the exact checker identity derived from the embedded canonical V5 response.
    ///
    /// This coordinate remains inert until [`Self::authenticate`] verifies the embedded response's
    /// signature under the fixed production trust anchor.
    pub const fn checker_evidence_identity(
        &self,
    ) -> AuthenticatedCompilerCapabilityEvidenceIdentityV5 {
        self.checker_evidence_identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// Verifies the protected signature and consumes this inert envelope into artifact authority.
    pub fn authenticate(
        self,
    ) -> Result<AuthenticatedCompilerCapabilityCompletionV5, WorkerV3ProtectedCompletionErrorV5>
    {
        let authenticated =
            AuthenticatedCompilerCapabilityCompletionV5::from_signed_verifier_response(
                InertCompilerCapabilityVerifierResponseV5::decode(&self.verifier_response)?,
                self.signature,
            )
            .map_err(WorkerV3ProtectedCompletionErrorV5::Artifact)?;
        if authenticated.checker_evidence_identity() != self.checker_evidence_identity {
            return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
        }
        Ok(authenticated)
    }
}

/// Private-key custody injected into the protected process as an immutable sealed descriptor.
pub struct ProtectedWorkerV3VerifierSigningKeyV5 {
    key: SigningKey,
}

impl ProtectedWorkerV3VerifierSigningKeyV5 {
    pub fn from_sealed_descriptor(
        descriptor: OwnedFd,
    ) -> Result<Self, WorkerV3ProtectedCompletionErrorV5> {
        let stat = rustix::fs::fstat(&descriptor)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?;
        let flags = rustix::io::fcntl_getfd(&descriptor)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?;
        let status = rustix::fs::fcntl_getfl(&descriptor)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?;
        let seals = rustix::fs::fcntl_get_seals(&descriptor)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?;
        let filesystem = rustix::fs::fstatfs(&descriptor)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_mode & 0o7777 != 0o400
            || stat.st_nlink != 0
            || stat.st_size != 32
            || !flags.contains(rustix::io::FdFlags::CLOEXEC)
            || status & OFlags::ACCMODE != OFlags::RDONLY
            || seals != REQUIRED_KEY_SEALS
            || filesystem.f_type as u64 != TMPFS_MAGIC
        {
            return Err(WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor);
        }
        let mut seed = [0_u8; 32];
        if rustix::io::pread(&descriptor, &mut seed, 0)
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)?
            != seed.len()
        {
            return Err(WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor);
        }
        let key = SigningKey::from_bytes(&seed);
        seed.fill(0);
        drop(descriptor);
        Ok(Self { key })
    }

    #[cfg(test)]
    pub(crate) fn fixture(seed: [u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(&seed),
        }
    }
}

impl fmt::Debug for ProtectedWorkerV3VerifierSigningKeyV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedWorkerV3VerifierSigningKeyV5")
            .finish_non_exhaustive()
    }
}

/// Validates exact immutable evidence, consumes the #213 owner, and signs the #214-bound response.
pub fn issue_worker_v3_protected_completion_v5(
    request: &WorkerV3VerificationCapabilityRequestV5,
    evidence: WorkerV3ProtectedCompletionRequestV5,
    object_bytes: &[u8],
    signer: &ProtectedWorkerV3VerifierSigningKeyV5,
) -> Result<WorkerV3ProtectedCompletionResponseV5, WorkerV3ProtectedCompletionErrorV5> {
    let transaction = &evidence.transaction;
    let handoff = transaction.handoff();
    let bundle = transaction.simulation_bundle();
    let finalization = &evidence.finalization;
    if evidence.request_binding != *request.protected_evidence_binding()?.as_bytes()
        || handoff.identity().sha256() != request.carriage().handoff_identity().sha256()
        || handoff.identity().byte_len() != request.carriage().handoff_identity().byte_len()
        || Sha256::digest(object_bytes).as_slice() != request.carriage().object_identity().sha256()
        || object_bytes.len() as u64 != request.carriage().object_identity().byte_len()
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }
    validate_transaction_against_request_v5(request, transaction)?;
    validate_finalization_against_request_v5(request, handoff, finalization, object_bytes)?;

    let machine_receipt =
        TargetMachineRefinementReceiptV1::decode(finalization.machine_receipt_bytes())?;
    let raw_object = derive_unfinalized_hsaco_from_finalized_v1(object_bytes)
        .map_err(WorkerV3ProtectedCompletionErrorV5::Finalization)?;
    if !coordinate_matches(finalization.raw_object_identity(), &raw_object)
        || !machine_receipt
            .parts()
            .final_code_object
            .matches(&raw_object)
        || !machine_target_matches_handoff(machine_receipt.parts().target, handoff)
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }

    let validated = admit_exact_capability_result_evidence_v5(request, transaction, finalization)?;
    let (proof_owner, associations, machine_refinement) = validated
        .into_completion_parts()
        .map_err(WorkerV3ProtectedCompletionErrorV5::NativeOwner)?;
    if machine_refinement.canonical_preimage() != finalization.machine_receipt_bytes() {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }

    let proof_identity = coordinate(proof_owner.canonical_bytes())?;
    let association_identity = coordinate(associations.canonical_bytes())?;
    let machine_identity = coordinate(machine_refinement.canonical_preimage())?;
    let verifier_response = InertCompilerCapabilityVerifierResponseV5::new(
        request.carriage().transaction_identity(),
        handoff,
        bundle,
        object_bytes,
        proof_owner,
        associations,
        finalization.machine_receipt_bytes().to_vec(),
    )?;
    let signing_message = verifier_response.signing_message()?;
    let signature = signer.key.sign(&signing_message).to_bytes();
    let verifier_response_bytes = verifier_response.canonical_bytes().to_vec();

    // The service may publish only bytes accepted by the same compile-time trust anchor as Cargo.
    let authenticated = AuthenticatedCompilerCapabilityCompletionV5::from_signed_verifier_response(
        InertCompilerCapabilityVerifierResponseV5::decode(&verifier_response_bytes)?,
        signature,
    )?;
    drop(authenticated);

    let completion = WorkerV3VerificationCapabilityCompletionV5::new_exact(
        request,
        coordinate(&verifier_response_bytes)?,
        machine_identity,
        coordinate(&signing_message)?,
        request.carriage().object_identity(),
        association_identity,
        proof_identity,
    )?;
    let transport_response =
        WorkerV3VerificationCapabilityResponseV5::completed(request, completion)?;
    WorkerV3ProtectedCompletionResponseV5::new(
        request,
        transport_response,
        &verifier_response_bytes,
        signature,
    )
}

fn validate_transaction_against_request_v5(
    request: &WorkerV3VerificationCapabilityRequestV5,
    transaction: &InertProductionCapabilityTransactionV5,
) -> Result<(), WorkerV3ProtectedCompletionErrorV5> {
    let carriage = request.carriage();
    let handoff = transaction.handoff();
    let transaction_identity = transaction.identity();
    let handoff_identity = handoff.identity();
    let legacy_identity = handoff.legacy_handoff().identity();
    let report = handoff.final_graph_report();
    let closure = handoff.target_closure();
    let source = handoff.source_refinement().identity();
    let kir = handoff.executable_kir().identity();
    let subject = handoff
        .subjects()
        .first()
        .ok_or(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch)?;
    if carriage.transaction_identity() != transaction_identity.sha256()
        || carriage.handoff_identity().sha256() != handoff_identity.sha256()
        || carriage.handoff_identity().byte_len() != handoff_identity.byte_len()
        || carriage.paired_v3_identity().sha256() != *legacy_identity.sha256()
        || carriage.paired_v3_identity().byte_len() != legacy_identity.byte_len()
        || carriage.final_graph_version() != 13
        || carriage.final_graph().sha256() != report.final_graph()
        || carriage.final_graph().byte_len() != report.final_graph_bytes()
        || carriage.final_epoch() != report.final_epoch()
        || carriage.kernel_identity() != *subject.kernel().digest().as_bytes()
        || carriage.root_identity() != *subject.root().digest().as_bytes()
        || carriage.target_identity() != *subject.target_model().digest().as_bytes()
        || carriage.launch_identity() != *subject.launch_contract().digest().as_bytes()
        || carriage.target_closure_identity() != closure.closure_identity()
        || carriage.target_closure_record_identity().sha256()
            != Sha256::digest(closure.canonical_bytes()).as_slice()
        || carriage.target_closure_record_identity().byte_len()
            != closure.canonical_bytes().len() as u64
        || carriage.w4_report_identity() != report.report_identity()
        || carriage.source_receipt_identity().sha256() != source.sha256()
        || carriage.source_receipt_identity().byte_len() != source.byte_len()
        || carriage.semantic_mir_identity() != handoff.inputs().semantic_mir_identity()
        || carriage.compiler_policy_identity() != handoff.inputs().compiler_policy()
        || carriage.executable_kir_receipt_identity().sha256() != kir.sha256()
        || carriage.executable_kir_receipt_identity().byte_len() != kir.byte_len()
        || handoff.subjects().len() != handoff.obligation_roster().len()
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }

    let roster = handoff
        .proof_lineage()
        .roster(MultiRootProofRosterKindV3::MiddleEnd);
    if roster.root_count() != request.base_request().entries().len()
        || roster.root_count() != handoff.subjects().len()
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }
    for (ordinal, entry) in request.base_request().entries().iter().enumerate() {
        let root = roster
            .root(ordinal)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch)?;
        if usize::try_from(entry.ordinal()).ok() != Some(ordinal)
            || entry.logical_name() != root.logical_name()
            || entry.export_name() != root.export_symbol()
            || entry.lineage_identity() != &root.semantic_root_identity()
            || entry.marker_binding_identity() != &root.kernel_binding()
        {
            return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
        }
    }
    Ok(())
}

fn validate_finalization_against_request_v5(
    request: &WorkerV3VerificationCapabilityRequestV5,
    handoff: &InertProductionCapabilityHandoffV5,
    finalization: &InertWorkerV3MachineRefinedFinalizationV5,
    object_bytes: &[u8],
) -> Result<(), WorkerV3ProtectedCompletionErrorV5> {
    let carriage = request.carriage();
    let handoff_identity = handoff.identity();
    if finalization.attempt() != carriage.attempt()
        || finalization.transaction_identity() != carriage.transaction_identity()
        || finalization.handoff_identity().sha256() != handoff_identity.sha256()
        || finalization.handoff_identity().byte_len() != handoff_identity.byte_len()
        || !coordinate_matches(finalization.finalized_object_identity(), object_bytes)
        || finalization.finalized_object_identity() != carriage.object_identity()
        || finalization.target_identity() != carriage.target_identity()
        || finalization.launch_identity() != carriage.launch_identity()
        || finalization.compiler_policy_identity() != carriage.compiler_policy_identity()
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }
    Ok(())
}

fn admit_exact_capability_result_evidence_v5(
    request: &WorkerV3VerificationCapabilityRequestV5,
    transaction: &InertProductionCapabilityTransactionV5,
    finalization: &InertWorkerV3MachineRefinedFinalizationV5,
) -> Result<ProtectedCompilerMultiRootProofInputsV5, WorkerV3ProtectedCompletionErrorV5> {
    if transaction.handoff().subjects().len() != request.base_request().entries().len()
        || finalization.machine_receipt_bytes().is_empty()
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::CorrelationMismatch);
    }
    compose_protected_compiler_completion_inputs_v5(
        transaction.handoff(),
        finalization.machine_receipt_bytes(),
    )
    .map_err(WorkerV3ProtectedCompletionErrorV5::ProtectedInput)
}

fn coordinate_matches(coordinate: ExactIdentityCoordinateV5, bytes: &[u8]) -> bool {
    coordinate.byte_len() == bytes.len() as u64
        && coordinate.sha256() == Sha256::digest(bytes).as_slice()
}

fn machine_target_matches_handoff(
    target: TargetMachineRefinementTargetV1,
    handoff: &InertProductionCapabilityHandoffV5,
) -> bool {
    let configured = handoff.legacy_handoff().capsule().target().to_string();
    match target {
        TargetMachineRefinementTargetV1::Gfx942 => configured.starts_with("gfx942:"),
        TargetMachineRefinementTargetV1::Gfx950 => configured.starts_with("gfx950:"),
    }
}

fn coordinate(
    bytes: &[u8],
) -> Result<ExactIdentityCoordinateV5, WorkerV3ProtectedCompletionErrorV5> {
    ExactIdentityCoordinateV5::new(
        Sha256::digest(bytes).into(),
        u64::try_from(bytes.len())
            .map_err(|_| WorkerV3ProtectedCompletionErrorV5::LengthOverflow)?,
    )
    .map_err(WorkerV3ProtectedCompletionErrorV5::Protocol)
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3ProtectedCompletionErrorV5 {
    TooLarge,
    Truncated,
    InvalidHeader,
    WrongDomain,
    InvalidIdentity,
    IdentityMismatch,
    NonCanonical,
    LengthOverflow,
    AllocationFailed,
    CorrelationMismatch,
    InvalidSigningKeyDescriptor,
    Handoff(InertProductionCapabilityHandoffErrorV5),
    MachineReceipt(TargetMachineRefinementReceiptErrorV1),
    Finalization(fe2o3_hsaco_finalize::FinalizationError),
    ProtectedInput(ProtectedCompilerCompletionInputErrorV5),
    NativeOwner(fe2o3_verifier::CompilerProofInputValidationErrorV5),
    Protocol(WorkerV3VerificationCapabilityProtocolErrorV5),
    Artifact(AuthenticatedCompilerCapabilityCompletionErrorV5),
}

impl fmt::Display for WorkerV3ProtectedCompletionErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "protected Worker V3 completion failed: {self:?}")
    }
}

impl Error for WorkerV3ProtectedCompletionErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(source) => Some(source),
            Self::MachineReceipt(source) => Some(source),
            Self::Finalization(source) => Some(source),
            Self::ProtectedInput(source) => Some(source),
            Self::NativeOwner(source) => Some(source),
            Self::Protocol(source) => Some(source),
            Self::Artifact(source) => Some(source),
            _ => None,
        }
    }
}

impl From<TargetMachineRefinementReceiptErrorV1> for WorkerV3ProtectedCompletionErrorV5 {
    fn from(source: TargetMachineRefinementReceiptErrorV1) -> Self {
        Self::MachineReceipt(source)
    }
}

impl From<InertProductionCapabilityHandoffErrorV5> for WorkerV3ProtectedCompletionErrorV5 {
    fn from(source: InertProductionCapabilityHandoffErrorV5) -> Self {
        Self::Handoff(source)
    }
}

impl From<WorkerV3VerificationCapabilityProtocolErrorV5> for WorkerV3ProtectedCompletionErrorV5 {
    fn from(source: WorkerV3VerificationCapabilityProtocolErrorV5) -> Self {
        Self::Protocol(source)
    }
}

impl From<AuthenticatedCompilerCapabilityCompletionErrorV5> for WorkerV3ProtectedCompletionErrorV5 {
    fn from(source: AuthenticatedCompilerCapabilityCompletionErrorV5) -> Self {
        Self::Artifact(source)
    }
}

struct DecodedRecord<'a> {
    fields: Vec<&'a [u8]>,
    identity: [u8; 32],
}

fn encode_record(
    magic: [u8; 8],
    field_count: u16,
    fields: &[&[u8]],
    maximum: usize,
    identity_domain: &[u8],
) -> Result<Vec<u8>, WorkerV3ProtectedCompletionErrorV5> {
    if fields.len() != field_count as usize {
        return Err(WorkerV3ProtectedCompletionErrorV5::InvalidHeader);
    }
    let body = fields.iter().try_fold(0_usize, |total, field| {
        total
            .checked_add(FIELD_HEADER_BYTES)
            .and_then(|value| value.checked_add(field.len()))
            .ok_or(WorkerV3ProtectedCompletionErrorV5::LengthOverflow)
    })?;
    let total = HEADER_BYTES
        .checked_add(body)
        .and_then(|value| value.checked_add(TERMINAL_BYTES))
        .ok_or(WorkerV3ProtectedCompletionErrorV5::LengthOverflow)?;
    if total > maximum {
        return Err(WorkerV3ProtectedCompletionErrorV5::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| WorkerV3ProtectedCompletionErrorV5::AllocationFailed)?;
    bytes.extend_from_slice(&magic);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&field_count.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    for (index, field) in fields.iter().enumerate() {
        bytes.extend_from_slice(&((index + 1) as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(field.len())
                .map_err(|_| WorkerV3ProtectedCompletionErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(field);
    }
    let identity = derive_identity(identity_domain, &bytes);
    bytes.extend_from_slice(&identity);
    Ok(bytes)
}

fn decode_record<'a>(
    bytes: &'a [u8],
    magic: [u8; 8],
    field_count: u16,
    maximum: usize,
    identity_domain: &[u8],
) -> Result<DecodedRecord<'a>, WorkerV3ProtectedCompletionErrorV5> {
    if bytes.len() > maximum {
        return Err(WorkerV3ProtectedCompletionErrorV5::TooLarge);
    }
    if bytes.len() < HEADER_BYTES + TERMINAL_BYTES {
        return Err(WorkerV3ProtectedCompletionErrorV5::Truncated);
    }
    if bytes[..8] != magic
        || read_u16(bytes, 8)? != VERSION
        || read_u16(bytes, 10)? != field_count
        || read_u32(bytes, 12)? != 0
        || read_u64(bytes, 16)? != bytes.len() as u64
    {
        return Err(WorkerV3ProtectedCompletionErrorV5::InvalidHeader);
    }
    let payload_end = bytes.len() - TERMINAL_BYTES;
    let identity = array::<32>(&bytes[payload_end..])?;
    if derive_identity(identity_domain, &bytes[..payload_end]) != identity {
        return Err(WorkerV3ProtectedCompletionErrorV5::IdentityMismatch);
    }
    let mut offset = HEADER_BYTES;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count as usize)
        .map_err(|_| WorkerV3ProtectedCompletionErrorV5::AllocationFailed)?;
    for expected in 1..=field_count {
        if read_u16(bytes, offset)? != expected || read_u16(bytes, offset + 2)? != 0 {
            return Err(WorkerV3ProtectedCompletionErrorV5::NonCanonical);
        }
        let length = read_u32(bytes, offset + 4)? as usize;
        offset = offset
            .checked_add(FIELD_HEADER_BYTES)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::LengthOverflow)?;
        let end = offset
            .checked_add(length)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::LengthOverflow)?;
        if end > payload_end {
            return Err(WorkerV3ProtectedCompletionErrorV5::Truncated);
        }
        fields.push(&bytes[offset..end]);
        offset = end;
    }
    if offset != payload_end {
        return Err(WorkerV3ProtectedCompletionErrorV5::NonCanonical);
    }
    Ok(DecodedRecord { fields, identity })
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], WorkerV3ProtectedCompletionErrorV5> {
    bytes
        .try_into()
        .map_err(|_| WorkerV3ProtectedCompletionErrorV5::Truncated)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, WorkerV3ProtectedCompletionErrorV5> {
    Ok(u16::from_le_bytes(array(
        bytes
            .get(offset..offset + 2)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::Truncated)?,
    )?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, WorkerV3ProtectedCompletionErrorV5> {
    Ok(u32::from_le_bytes(array(
        bytes
            .get(offset..offset + 4)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::Truncated)?,
    )?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, WorkerV3ProtectedCompletionErrorV5> {
    Ok(u64::from_le_bytes(array(
        bytes
            .get(offset..offset + 8)
            .ok_or(WorkerV3ProtectedCompletionErrorV5::Truncated)?,
    )?))
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::{AsRawFd as _, OwnedFd};

    use rustix::fs::{MemfdFlags, Mode, OFlags};

    use super::*;

    fn key_descriptor(seed: &[u8]) -> OwnedFd {
        let fd = rustix::fs::memfd_create(
            "fe2o3-worker-v3-protected-key-test",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .unwrap();
        let mut writer = File::from(fd);
        rustix::fs::fchmod(&writer, Mode::RUSR).unwrap();
        writer.write_all(seed).unwrap();
        rustix::fs::fcntl_add_seals(&writer, REQUIRED_KEY_SEALS).unwrap();
        let reader = rustix::fs::open(
            format!("/proc/self/fd/{}", writer.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        drop(writer);
        reader
    }

    #[test]
    fn protected_key_requires_exact_immutable_read_only_descriptor() {
        assert!(
            ProtectedWorkerV3VerifierSigningKeyV5::from_sealed_descriptor(key_descriptor(&[7; 32]))
                .is_ok()
        );
        assert!(matches!(
            ProtectedWorkerV3VerifierSigningKeyV5::from_sealed_descriptor(key_descriptor(&[7; 31])),
            Err(WorkerV3ProtectedCompletionErrorV5::InvalidSigningKeyDescriptor)
        ));
    }

    #[test]
    fn record_decoder_rejects_truncation_mutation_and_noncanonical_tags() {
        let fields = [
            REQUEST_DOMAIN,
            &[9; 32][..],
            b"transaction",
            b"finalization",
        ];
        let encoded = encode_record(
            REQUEST_MAGIC,
            REQUEST_FIELDS,
            &fields,
            MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
            REQUEST_IDENTITY_DOMAIN,
        )
        .unwrap();
        for prefix in 0..encoded.len() {
            assert!(
                decode_record(
                    &encoded[..prefix],
                    REQUEST_MAGIC,
                    REQUEST_FIELDS,
                    MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
                    REQUEST_IDENTITY_DOMAIN,
                )
                .is_err()
            );
        }
        let mut mutated = encoded.clone();
        mutated[HEADER_BYTES + 8] ^= 1;
        assert!(matches!(
            decode_record(
                &mutated,
                REQUEST_MAGIC,
                REQUEST_FIELDS,
                MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
                REQUEST_IDENTITY_DOMAIN,
            ),
            Err(WorkerV3ProtectedCompletionErrorV5::IdentityMismatch)
        ));
        let mut reordered = encoded;
        reordered[HEADER_BYTES..HEADER_BYTES + 2].copy_from_slice(&2_u16.to_le_bytes());
        let terminal = reordered.len() - TERMINAL_BYTES;
        let identity = derive_identity(REQUEST_IDENTITY_DOMAIN, &reordered[..terminal]);
        reordered[terminal..].copy_from_slice(&identity);
        assert!(matches!(
            decode_record(
                &reordered,
                REQUEST_MAGIC,
                REQUEST_FIELDS,
                MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
                REQUEST_IDENTITY_DOMAIN,
            ),
            Err(WorkerV3ProtectedCompletionErrorV5::NonCanonical)
        ));
    }

    #[test]
    fn fixture_signer_is_redacted() {
        let key = ProtectedWorkerV3VerifierSigningKeyV5::fixture([11; 32]);
        assert_eq!(
            format!("{key:?}"),
            "ProtectedWorkerV3VerifierSigningKeyV5 { .. }"
        );
    }
}
