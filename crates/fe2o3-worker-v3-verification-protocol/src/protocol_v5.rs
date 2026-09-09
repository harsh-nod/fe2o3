//! Exact, authority-free V5 identity carriage beside the frozen Worker V3 protocols.

use std::{error::Error, fmt};

use sha2::{Digest as _, Sha256};

use crate::{WorkerV3VerificationFdPayloadKindV1, WorkerV3VerificationRequestV1};

/// Exact additive Worker V3 capability-carriage version.
pub const WORKER_V3_VERIFICATION_CAPABILITY_VERSION_V5: u16 = 5;
/// Maximum accepted V5 Begin request.
pub const MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5: usize =
    crate::MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1 + (64 * 1024);
/// Maximum accepted V5 completion response.
pub const MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5: usize = 8 * 1024;

const MAGIC: [u8; 8] = *b"F3WVCV5\0";
const HEADER_BYTES: usize = 24;
const FIELD_HEADER_BYTES: usize = 8;
const IDENTITY_BYTES: usize = 32;
const CARRIAGE_KIND: u16 = 1;
const REQUEST_KIND: u16 = 2;
const COMPLETION_KIND: u16 = 3;
const RESPONSE_KIND: u16 = 4;
const PROTECTED_INPUT_KIND: u16 = 5;
const MACHINE_FINALIZATION_KIND: u16 = 6;
const CARRIAGE_FIELDS: usize = 21;
const REQUEST_FIELDS: usize = 3;
const COMPLETION_FIELDS: usize = 8;
const RESPONSE_FIELDS: usize = 5;
const PROTECTED_INPUT_FIELDS: usize = 6;
const MACHINE_FINALIZATION_FIELDS: usize = 10;
const CARRIAGE_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-CARRIAGE/EXACT-V13/V5\0";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-REQUEST/EXACT-V13/V5\0";
const COMPLETION_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-COMPLETION/EXACT-V13/V5\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-RESPONSE/EXACT-V13/V5\0";
const CARRIAGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-CARRIAGE-IDENTITY/V5\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-REQUEST-IDENTITY/V5\0";
const COMPLETION_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-COMPLETION-IDENTITY/V5\0";
const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-RESPONSE-IDENTITY/V5\0";
const PROTECTED_EVIDENCE_BINDING_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-EVIDENCE-BINDING/V5\0";
const PROTECTED_INPUT_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/PROTECTED-COMPILER-INPUT/V5\0";
const PROTECTED_INPUT_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/PROTECTED-COMPILER-INPUT-IDENTITY/V5\0";
const MACHINE_FINALIZATION_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/MACHINE-REFINED-FINALIZATION/V5\0";
const MACHINE_FINALIZATION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WORKER-V3/MACHINE-REFINED-FINALIZATION-IDENTITY/V5\0";

/// Maximum canonical raw-to-finalized transition accepted by the protected verifier.
pub const MAX_WORKER_V3_MACHINE_REFINED_FINALIZATION_BYTES_V5: usize = 8 * 1024;

/// Exact nonzero digest/length pair. The enclosing field tag supplies its semantic role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactIdentityCoordinateV5 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ExactIdentityCoordinateV5 {
    /// Constructs one exact inert coordinate.
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
        }
        Ok(Self { sha256, byte_len })
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn encode(self) -> [u8; 40] {
        let mut bytes = [0; 40];
        bytes[..32].copy_from_slice(&self.sha256);
        bytes[32..].copy_from_slice(&self.byte_len.to_le_bytes());
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        require_len(bytes, 40)?;
        Self::new(copy_32(&bytes[..32])?, read_u64(&bytes[32..])?)
    }
}

/// Exact durable production-attempt coordinate carried without authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3VerificationProductionAttemptV5 {
    generation: u64,
    session: [u8; 16],
    invocation: [u8; 32],
}

impl WorkerV3VerificationProductionAttemptV5 {
    /// Constructs one exact managed production attempt.
    pub fn new(
        generation: u64,
        session: [u8; 16],
        invocation: [u8; 32],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if generation == 0 || session == [0; 16] || invocation == [0; 32] {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidAttempt);
        }
        Ok(Self {
            generation,
            session,
            invocation,
        })
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn session(self) -> [u8; 16] {
        self.session
    }

    pub const fn invocation(self) -> [u8; 32] {
        self.invocation
    }

    fn encode(self) -> [u8; 56] {
        let mut bytes = [0; 56];
        bytes[..8].copy_from_slice(&self.generation.to_le_bytes());
        bytes[8..24].copy_from_slice(&self.session);
        bytes[24..].copy_from_slice(&self.invocation);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        require_len(bytes, 56)?;
        Self::new(
            read_u64(&bytes[..8])?,
            bytes[8..24]
                .try_into()
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::Truncated)?,
            copy_32(&bytes[24..])?,
        )
    }
}

macro_rules! identity_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

identity_type!(WorkerV3VerificationCapabilityCarriageIdentityV5);
identity_type!(WorkerV3VerificationCapabilityRequestIdentityV5);
identity_type!(WorkerV3VerificationCapabilityResponseIdentityV5);
identity_type!(WorkerV3VerificationProtectedEvidenceBindingIdentityV5);

/// Correlation identity computable before the protected-evidence descriptor exists.
///
/// The evidence payload binds this identity rather than the final request identity. Including the
/// final identity would be circular because that identity commits to the evidence payload digest.
#[allow(clippy::too_many_arguments)]
pub fn derive_worker_v3_protected_evidence_binding_v5(
    challenge: crate::WorkerV3VerificationFreshChallengeV1,
    roster: crate::WorkerV3VerificationRosterIdentityV1,
    policy: crate::WorkerV3VerificationPolicyIdentityV1,
    measurement: crate::WorkerV3VerificationMeasurementIdentityV1,
    object: crate::WorkerV3VerificationFdPayloadDescriptorV1,
    entries: &[crate::WorkerV3VerificationEntryCoordinateV1],
    carriage: &WorkerV3VerificationCapabilityCarriageV5,
) -> Result<
    WorkerV3VerificationProtectedEvidenceBindingIdentityV5,
    WorkerV3VerificationCapabilityProtocolErrorV5,
> {
    if object.kind() != crate::WorkerV3VerificationFdPayloadKindV1::FinalizedHsaco
        || entries.is_empty()
    {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidBaseRequest);
    }
    let mut digest = Sha256::new();
    digest.update(PROTECTED_EVIDENCE_BINDING_DOMAIN);
    digest.update(challenge.as_bytes());
    digest.update(roster.as_bytes());
    digest.update(policy.as_bytes());
    digest.update(measurement.as_bytes());
    digest.update(object.byte_len().to_le_bytes());
    digest.update(object.sha256());
    digest.update((entries.len() as u64).to_le_bytes());
    for entry in entries {
        digest.update(entry.ordinal().to_le_bytes());
        digest.update((entry.logical_name().len() as u64).to_le_bytes());
        digest.update(entry.logical_name().as_bytes());
        digest.update((entry.export_name().len() as u64).to_le_bytes());
        digest.update(entry.export_name().as_bytes());
        digest.update(entry.lineage_identity());
        digest.update(entry.marker_binding_identity());
        digest.update(entry.generated_host_contract_identity());
    }
    digest.update(carriage.identity().as_bytes());
    Ok(WorkerV3VerificationProtectedEvidenceBindingIdentityV5(
        digest.finalize().into(),
    ))
}

/// Exact pre-completion V5 coordinates. This record is inert and creates no authority.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationCapabilityCarriageV5 {
    handoff: ExactIdentityCoordinateV5,
    paired_v3: ExactIdentityCoordinateV5,
    transaction: [u8; 32],
    attempt: WorkerV3VerificationProductionAttemptV5,
    slot: u8,
    final_graph_version: u16,
    final_graph: ExactIdentityCoordinateV5,
    final_epoch: u64,
    kernel: [u8; 32],
    root: [u8; 32],
    target: [u8; 32],
    launch: [u8; 32],
    target_closure: [u8; 32],
    target_closure_record: ExactIdentityCoordinateV5,
    w4_report: [u8; 32],
    source_receipt: ExactIdentityCoordinateV5,
    object: ExactIdentityCoordinateV5,
    semantic_mir: [u8; 32],
    compiler_policy: [u8; 32],
    executable_kir_receipt: ExactIdentityCoordinateV5,
    identity: WorkerV3VerificationCapabilityCarriageIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3VerificationCapabilityCarriageV5 {
    /// Builds one inert carriage from exact coordinates supplied by a typed adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn new_exact(
        handoff: ExactIdentityCoordinateV5,
        paired_v3: ExactIdentityCoordinateV5,
        transaction: [u8; 32],
        attempt: WorkerV3VerificationProductionAttemptV5,
        slot: u8,
        final_graph_version: u16,
        final_graph: ExactIdentityCoordinateV5,
        final_epoch: u64,
        kernel: [u8; 32],
        root: [u8; 32],
        target: [u8; 32],
        launch: [u8; 32],
        target_closure: [u8; 32],
        target_closure_record: ExactIdentityCoordinateV5,
        w4_report: [u8; 32],
        source_receipt: ExactIdentityCoordinateV5,
        object: ExactIdentityCoordinateV5,
        semantic_mir: [u8; 32],
        compiler_policy: [u8; 32],
        executable_kir_receipt: ExactIdentityCoordinateV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let fields = [
            CARRIAGE_DOMAIN.to_vec(),
            handoff.encode().to_vec(),
            paired_v3.encode().to_vec(),
            transaction.to_vec(),
            attempt.encode().to_vec(),
            vec![slot],
            final_graph_version.to_le_bytes().to_vec(),
            final_graph.encode().to_vec(),
            final_epoch.to_le_bytes().to_vec(),
            kernel.to_vec(),
            root.to_vec(),
            target.to_vec(),
            launch.to_vec(),
            target_closure.to_vec(),
            target_closure_record.encode().to_vec(),
            w4_report.to_vec(),
            source_receipt.encode().to_vec(),
            object.encode().to_vec(),
            semantic_mir.to_vec(),
            compiler_policy.to_vec(),
            executable_kir_receipt.encode().to_vec(),
        ];
        Self::from_fields(fields)
    }

    /// Strictly decodes one complete V5 carriage with no legacy projection.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            CARRIAGE_KIND,
            CARRIAGE_FIELDS,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
            CARRIAGE_DOMAIN,
            CARRIAGE_IDENTITY_DOMAIN,
        )?;
        let fields: [Vec<u8>; CARRIAGE_FIELDS] =
            std::array::from_fn(|index| record.fields[index].to_vec());
        let decoded = Self::from_fields(fields)?;
        require_canonical(decoded.encode_canonical(), bytes)?;
        Ok(decoded)
    }

    fn from_fields(
        fields: [Vec<u8>; CARRIAGE_FIELDS],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        require_len(&fields[0], CARRIAGE_DOMAIN.len())?;
        if fields[0] != CARRIAGE_DOMAIN {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongDomain);
        }
        require_len(&fields[3], 32)?;
        require_len(&fields[5], 1)?;
        require_len(&fields[6], 2)?;
        require_len(&fields[8], 8)?;
        for field in [
            &fields[9],
            &fields[10],
            &fields[11],
            &fields[12],
            &fields[13],
            &fields[15],
            &fields[18],
            &fields[19],
        ] {
            require_len(field, 32)?;
            require_nonzero_32(field)?;
        }
        let transaction = copy_32(&fields[3])?;
        let slot = fields[5][0];
        let final_graph_version = read_u16(&fields[6])?;
        let final_epoch = read_u64(&fields[8])?;
        if transaction == [0; 32] || slot != 0 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidTransaction);
        }
        if final_graph_version != 13 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::Downgrade);
        }
        if final_epoch == 0 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::StaleEpoch);
        }

        let canonical = encode_record(
            CARRIAGE_KIND,
            &fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
            CARRIAGE_IDENTITY_DOMAIN,
        )?;
        let identity =
            WorkerV3VerificationCapabilityCarriageIdentityV5(terminal_identity(&canonical)?);
        Ok(Self {
            handoff: ExactIdentityCoordinateV5::decode(&fields[1])?,
            paired_v3: ExactIdentityCoordinateV5::decode(&fields[2])?,
            transaction,
            attempt: WorkerV3VerificationProductionAttemptV5::decode(&fields[4])?,
            slot,
            final_graph_version,
            final_graph: ExactIdentityCoordinateV5::decode(&fields[7])?,
            final_epoch,
            kernel: copy_32(&fields[9])?,
            root: copy_32(&fields[10])?,
            target: copy_32(&fields[11])?,
            launch: copy_32(&fields[12])?,
            target_closure: copy_32(&fields[13])?,
            target_closure_record: ExactIdentityCoordinateV5::decode(&fields[14])?,
            w4_report: copy_32(&fields[15])?,
            source_receipt: ExactIdentityCoordinateV5::decode(&fields[16])?,
            object: ExactIdentityCoordinateV5::decode(&fields[17])?,
            semantic_mir: copy_32(&fields[18])?,
            compiler_policy: copy_32(&fields[19])?,
            executable_kir_receipt: ExactIdentityCoordinateV5::decode(&fields[20])?,
            identity,
            canonical_bytes: canonical.into_boxed_slice(),
        })
    }

    pub const fn handoff_identity(&self) -> ExactIdentityCoordinateV5 {
        self.handoff
    }
    pub const fn paired_v3_identity(&self) -> ExactIdentityCoordinateV5 {
        self.paired_v3
    }
    pub const fn transaction_identity(&self) -> [u8; 32] {
        self.transaction
    }
    pub const fn attempt(&self) -> WorkerV3VerificationProductionAttemptV5 {
        self.attempt
    }
    pub const fn slot(&self) -> u8 {
        self.slot
    }
    pub const fn final_graph_version(&self) -> u16 {
        self.final_graph_version
    }
    pub const fn final_graph(&self) -> ExactIdentityCoordinateV5 {
        self.final_graph
    }
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }
    pub const fn kernel_identity(&self) -> [u8; 32] {
        self.kernel
    }
    pub const fn root_identity(&self) -> [u8; 32] {
        self.root
    }
    pub const fn target_identity(&self) -> [u8; 32] {
        self.target
    }
    pub const fn launch_identity(&self) -> [u8; 32] {
        self.launch
    }
    pub const fn target_closure_identity(&self) -> [u8; 32] {
        self.target_closure
    }
    pub const fn target_closure_record_identity(&self) -> ExactIdentityCoordinateV5 {
        self.target_closure_record
    }
    pub const fn w4_report_identity(&self) -> [u8; 32] {
        self.w4_report
    }
    pub const fn source_receipt_identity(&self) -> ExactIdentityCoordinateV5 {
        self.source_receipt
    }
    pub const fn object_identity(&self) -> ExactIdentityCoordinateV5 {
        self.object
    }
    pub const fn semantic_mir_identity(&self) -> [u8; 32] {
        self.semantic_mir
    }
    pub const fn compiler_policy_identity(&self) -> [u8; 32] {
        self.compiler_policy
    }
    pub const fn executable_kir_receipt_identity(&self) -> ExactIdentityCoordinateV5 {
        self.executable_kir_receipt
    }
    pub const fn identity(&self) -> WorkerV3VerificationCapabilityCarriageIdentityV5 {
        self.identity
    }
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Maximum canonical compiler-owned input package accepted by the protected V5 service.
pub const MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5: usize =
    crate::MAX_WORKER_V3_VERIFICATION_PROTECTED_EVIDENCE_FD_BYTES_V5 as usize;

/// Canonical descriptor-relative journal name for one attempt's protected compiler input.
pub fn worker_v3_protected_compiler_input_name_v5(
    attempt: WorkerV3VerificationProductionAttemptV5,
) -> String {
    protected_compiler_input_name_v5(attempt, ".input")
}

/// Descriptor-relative redo name paired with [`worker_v3_protected_compiler_input_name_v5`].
pub fn worker_v3_protected_compiler_input_redo_name_v5(
    attempt: WorkerV3VerificationProductionAttemptV5,
) -> String {
    protected_compiler_input_name_v5(attempt, ".redo")
}

fn protected_compiler_input_name_v5(
    attempt: WorkerV3VerificationProductionAttemptV5,
    suffix: &str,
) -> String {
    format!(
        ".fe2o3-worker-v3-protected-input-v5-{}-{}-{}{}",
        attempt.generation(),
        hex(&attempt.session()),
        hex(&attempt.invocation()),
        suffix
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

/// Inert, attempt-bound compiler-completion package recovered after protected rustc exits.
///
/// Phase one deliberately contains no machine-refinement receipt, capability association, or #213
/// proof owner: those values cannot exist until generic finalization owns the exact object. The
/// complete canonical V5+V8 transaction retains source/reference IR, final V13/V6 graph custody,
/// W4 results, source functional refinement, and target/launch/policy coordinates without
/// projecting or duplicating their schemas. The fixed service may seal a complete owner only after
/// phase two supplies independently checked machine evidence for the same transaction and object.
#[derive(Debug, Eq, PartialEq)]
pub struct InertWorkerV3ProtectedCompilerInputV5 {
    attempt: WorkerV3VerificationProductionAttemptV5,
    transaction: [u8; 32],
    handoff: ExactIdentityCoordinateV5,
    production_transaction: Box<[u8]>,
    generated_host_contracts: Box<[[u8; 32]]>,
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
}

impl InertWorkerV3ProtectedCompilerInputV5 {
    pub fn new(
        attempt: WorkerV3VerificationProductionAttemptV5,
        transaction: [u8; 32],
        handoff: ExactIdentityCoordinateV5,
        production_transaction: &[u8],
        generated_host_contracts: Vec<[u8; 32]>,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if transaction == [0; 32]
            || production_transaction.is_empty()
            || generated_host_contracts.is_empty()
            || generated_host_contracts
                .iter()
                .any(|identity| *identity == [0; 32])
        {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
        }
        let host_bytes = generated_host_contracts
            .len()
            .checked_mul(32)
            .and_then(|length| length.checked_add(8))
            .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
        let mut host_contracts = Vec::new();
        host_contracts
            .try_reserve_exact(host_bytes)
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::AllocationFailed)?;
        host_contracts.extend_from_slice(&(generated_host_contracts.len() as u64).to_le_bytes());
        for identity in &generated_host_contracts {
            host_contracts.extend_from_slice(identity);
        }
        let attempt_bytes = attempt.encode();
        let handoff_bytes = handoff.encode();
        let fields: [&[u8]; PROTECTED_INPUT_FIELDS] = [
            PROTECTED_INPUT_DOMAIN,
            &attempt_bytes,
            &transaction,
            &handoff_bytes,
            production_transaction,
            &host_contracts,
        ];
        Self::decode_canonical(&encode_record(
            PROTECTED_INPUT_KIND,
            &fields,
            MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
            PROTECTED_INPUT_IDENTITY_DOMAIN,
        )?)
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            PROTECTED_INPUT_KIND,
            PROTECTED_INPUT_FIELDS,
            MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
            PROTECTED_INPUT_DOMAIN,
            PROTECTED_INPUT_IDENTITY_DOMAIN,
        )?;
        require_len(record.fields[2], 32)?;
        if record.fields[2] == [0; 32] || record.fields[4].is_empty() {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
        }
        let host_bytes = record.fields[5];
        if host_bytes.len() < 8 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidFieldLength);
        }
        let count = usize::try_from(read_u64(&host_bytes[..8])?)
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
        let expected = count
            .checked_mul(32)
            .and_then(|length| length.checked_add(8))
            .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
        if count == 0 || host_bytes.len() != expected {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidFieldLength);
        }
        let mut generated_host_contracts = Vec::new();
        generated_host_contracts
            .try_reserve_exact(count)
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::AllocationFailed)?;
        for chunk in host_bytes[8..].chunks_exact(32) {
            let identity = copy_32(chunk)?;
            if identity == [0; 32] {
                return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
            }
            generated_host_contracts.push(identity);
        }
        Ok(Self {
            attempt: WorkerV3VerificationProductionAttemptV5::decode(record.fields[1])?,
            transaction: copy_32(record.fields[2])?,
            handoff: ExactIdentityCoordinateV5::decode(record.fields[3])?,
            production_transaction: record.fields[4].to_vec().into_boxed_slice(),
            generated_host_contracts: generated_host_contracts.into_boxed_slice(),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: terminal_identity(bytes)?,
        })
    }

    pub const fn attempt(&self) -> WorkerV3VerificationProductionAttemptV5 {
        self.attempt
    }

    pub const fn transaction_identity(&self) -> [u8; 32] {
        self.transaction
    }

    pub const fn handoff_identity(&self) -> ExactIdentityCoordinateV5 {
        self.handoff
    }

    /// Returns the complete canonical V5+V8 compiler transaction retained at phase one.
    pub fn production_transaction_bytes(&self) -> &[u8] {
        &self.production_transaction
    }

    pub fn generated_host_contract_identities(&self) -> &[[u8; 32]] {
        &self.generated_host_contracts
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

/// Inert process-boundary record for one checked raw-object to finalized-HSACO transition.
///
/// This record deliberately carries exact coordinates and the complete typed machine receipt,
/// but no Rust ownership or verifier authority. The protected service must rederive the raw
/// object from the finalized payload, decode the receipt, and correlate every transaction,
/// handoff, target, launch, and policy axis before it may construct the sole #213 owner.
#[derive(Debug, Eq, PartialEq)]
pub struct InertWorkerV3MachineRefinedFinalizationV5 {
    attempt: WorkerV3VerificationProductionAttemptV5,
    transaction: [u8; 32],
    handoff: ExactIdentityCoordinateV5,
    raw_object: ExactIdentityCoordinateV5,
    finalized_object: ExactIdentityCoordinateV5,
    target: [u8; 32],
    launch: [u8; 32],
    compiler_policy: [u8; 32],
    machine_receipt: Box<[u8]>,
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
}

impl InertWorkerV3MachineRefinedFinalizationV5 {
    /// Encodes exact phase-two custody without granting completion authority.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attempt: WorkerV3VerificationProductionAttemptV5,
        transaction: [u8; 32],
        handoff: ExactIdentityCoordinateV5,
        raw_object: ExactIdentityCoordinateV5,
        finalized_object: ExactIdentityCoordinateV5,
        target: [u8; 32],
        launch: [u8; 32],
        compiler_policy: [u8; 32],
        machine_receipt: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if transaction == [0; 32]
            || target == [0; 32]
            || launch == [0; 32]
            || compiler_policy == [0; 32]
            || machine_receipt.is_empty()
        {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
        }
        let attempt_bytes = attempt.encode();
        let handoff_bytes = handoff.encode();
        let raw_object_bytes = raw_object.encode();
        let finalized_object_bytes = finalized_object.encode();
        let fields: [&[u8]; MACHINE_FINALIZATION_FIELDS] = [
            MACHINE_FINALIZATION_DOMAIN,
            &attempt_bytes,
            &transaction,
            &handoff_bytes,
            &raw_object_bytes,
            &finalized_object_bytes,
            &target,
            &launch,
            &compiler_policy,
            machine_receipt,
        ];
        Self::decode_canonical(&encode_record(
            MACHINE_FINALIZATION_KIND,
            &fields,
            MAX_WORKER_V3_MACHINE_REFINED_FINALIZATION_BYTES_V5,
            MACHINE_FINALIZATION_IDENTITY_DOMAIN,
        )?)
    }

    /// Strictly decodes one complete phase-two transition without granting authority.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            MACHINE_FINALIZATION_KIND,
            MACHINE_FINALIZATION_FIELDS,
            MAX_WORKER_V3_MACHINE_REFINED_FINALIZATION_BYTES_V5,
            MACHINE_FINALIZATION_DOMAIN,
            MACHINE_FINALIZATION_IDENTITY_DOMAIN,
        )?;
        for field in [
            record.fields[2],
            record.fields[6],
            record.fields[7],
            record.fields[8],
        ] {
            require_len(field, 32)?;
            require_nonzero_32(field)?;
        }
        if record.fields[9].is_empty() {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity);
        }
        Ok(Self {
            attempt: WorkerV3VerificationProductionAttemptV5::decode(record.fields[1])?,
            transaction: copy_32(record.fields[2])?,
            handoff: ExactIdentityCoordinateV5::decode(record.fields[3])?,
            raw_object: ExactIdentityCoordinateV5::decode(record.fields[4])?,
            finalized_object: ExactIdentityCoordinateV5::decode(record.fields[5])?,
            target: copy_32(record.fields[6])?,
            launch: copy_32(record.fields[7])?,
            compiler_policy: copy_32(record.fields[8])?,
            machine_receipt: record.fields[9].to_vec().into_boxed_slice(),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: terminal_identity(bytes)?,
        })
    }

    pub const fn attempt(&self) -> WorkerV3VerificationProductionAttemptV5 {
        self.attempt
    }

    pub const fn transaction_identity(&self) -> [u8; 32] {
        self.transaction
    }

    pub const fn handoff_identity(&self) -> ExactIdentityCoordinateV5 {
        self.handoff
    }

    pub const fn raw_object_identity(&self) -> ExactIdentityCoordinateV5 {
        self.raw_object
    }

    pub const fn finalized_object_identity(&self) -> ExactIdentityCoordinateV5 {
        self.finalized_object
    }

    pub const fn target_identity(&self) -> [u8; 32] {
        self.target
    }

    pub const fn launch_identity(&self) -> [u8; 32] {
        self.launch
    }

    pub const fn compiler_policy_identity(&self) -> [u8; 32] {
        self.compiler_policy
    }

    pub fn machine_receipt_bytes(&self) -> &[u8] {
        &self.machine_receipt
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// This inert transition never authenticates the machine checker or grants completion.
    pub const fn grants_completion_authority(&self) -> bool {
        false
    }
}

/// Additive V5 Begin request wrapping the frozen V1 request without reinterpreting it.
///
/// ```compile_fail
/// use fe2o3_worker_v3_verification_protocol::WorkerV3VerificationCapabilityRequestV5;
/// fn duplicate(value: WorkerV3VerificationCapabilityRequestV5) {
///     let _again = value.clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationCapabilityRequestV5 {
    base: WorkerV3VerificationRequestV1,
    carriage: WorkerV3VerificationCapabilityCarriageV5,
    identity: WorkerV3VerificationCapabilityRequestIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3VerificationCapabilityRequestV5 {
    pub fn new(
        base: WorkerV3VerificationRequestV1,
        carriage: WorkerV3VerificationCapabilityCarriageV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if base.payloads()[0].kind()
            != WorkerV3VerificationFdPayloadKindV1::ProtectedCompletionEvidenceV5
        {
            return Err(
                WorkerV3VerificationCapabilityProtocolErrorV5::PrepublicationEvidenceRequired,
            );
        }
        if *base.payloads()[1].sha256() != carriage.object.sha256
            || base.payloads()[1].byte_len() != carriage.object.byte_len
        {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::ObjectMismatch);
        }
        if *base.policy_identity().as_bytes() != carriage.compiler_policy {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::SubjectMismatch);
        }
        let fields = [
            REQUEST_DOMAIN,
            base.encode_canonical(),
            carriage.encode_canonical(),
        ];
        let canonical_bytes = encode_record(
            REQUEST_KIND,
            &fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
            REQUEST_IDENTITY_DOMAIN,
        )?;
        let identity =
            WorkerV3VerificationCapabilityRequestIdentityV5(terminal_identity(&canonical_bytes)?);
        Ok(Self {
            base,
            carriage,
            identity,
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            REQUEST_KIND,
            REQUEST_FIELDS,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
            REQUEST_DOMAIN,
            REQUEST_IDENTITY_DOMAIN,
        )?;
        let decoded = Self::new(
            WorkerV3VerificationRequestV1::decode_canonical(record.fields[1])
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::InvalidBaseRequest)?,
            WorkerV3VerificationCapabilityCarriageV5::decode_canonical(record.fields[2])?,
        )?;
        require_canonical(decoded.encode_canonical(), bytes)?;
        Ok(decoded)
    }

    pub const fn base_request(&self) -> &WorkerV3VerificationRequestV1 {
        &self.base
    }

    pub const fn carriage(&self) -> &WorkerV3VerificationCapabilityCarriageV5 {
        &self.carriage
    }

    pub const fn identity(&self) -> WorkerV3VerificationCapabilityRequestIdentityV5 {
        self.identity
    }

    /// Returns the non-circular identity that the protected evidence payload must bind.
    pub fn protected_evidence_binding(
        &self,
    ) -> Result<
        WorkerV3VerificationProtectedEvidenceBindingIdentityV5,
        WorkerV3VerificationCapabilityProtocolErrorV5,
    > {
        derive_worker_v3_protected_evidence_binding_v5(
            self.base.challenge(),
            self.base.roster_identity(),
            self.base.policy_identity(),
            self.base.measurement_identity(),
            self.base.payloads()[1],
            self.base.entries(),
            &self.carriage,
        )
    }

    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Exact post-completion coordinates retained without authority.
///
/// ```compile_fail
/// use fe2o3_worker_v3_verification_protocol::WorkerV3VerificationCapabilityCompletionV5;
/// fn duplicate(value: WorkerV3VerificationCapabilityCompletionV5) {
///     let _again = value.clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationCapabilityCompletionV5 {
    request_carriage: WorkerV3VerificationCapabilityCarriageIdentityV5,
    result: ExactIdentityCoordinateV5,
    machine_receipt: ExactIdentityCoordinateV5,
    object_receipt: ExactIdentityCoordinateV5,
    object: ExactIdentityCoordinateV5,
    association: ExactIdentityCoordinateV5,
    proof_owner: ExactIdentityCoordinateV5,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3VerificationCapabilityCompletionV5 {
    /// Builds completion coordinates supplied by a typed completed-transaction adapter.
    pub fn new_exact(
        request: &WorkerV3VerificationCapabilityRequestV5,
        result: ExactIdentityCoordinateV5,
        machine_receipt: ExactIdentityCoordinateV5,
        object_receipt: ExactIdentityCoordinateV5,
        object: ExactIdentityCoordinateV5,
        association: ExactIdentityCoordinateV5,
        proof_owner: ExactIdentityCoordinateV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if object != request.carriage.object {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::CompletionMismatch);
        }
        let fields = [
            COMPLETION_DOMAIN.to_vec(),
            request.carriage.identity.0.to_vec(),
            result.encode().to_vec(),
            machine_receipt.encode().to_vec(),
            object_receipt.encode().to_vec(),
            object.encode().to_vec(),
            association.encode().to_vec(),
            proof_owner.encode().to_vec(),
        ];
        Self::from_fields(fields)
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            COMPLETION_KIND,
            COMPLETION_FIELDS,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
            COMPLETION_DOMAIN,
            COMPLETION_IDENTITY_DOMAIN,
        )?;
        let fields: [Vec<u8>; COMPLETION_FIELDS] =
            std::array::from_fn(|index| record.fields[index].to_vec());
        let decoded = Self::from_fields(fields)?;
        require_canonical(decoded.encode_canonical(), bytes)?;
        Ok(decoded)
    }

    fn from_fields(
        fields: [Vec<u8>; COMPLETION_FIELDS],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        require_len(&fields[0], COMPLETION_DOMAIN.len())?;
        if fields[0] != COMPLETION_DOMAIN {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongDomain);
        }
        require_len(&fields[1], 32)?;
        require_nonzero_32(&fields[1])?;
        let canonical_bytes = encode_record(
            COMPLETION_KIND,
            &fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
            COMPLETION_IDENTITY_DOMAIN,
        )?;
        Ok(Self {
            request_carriage: WorkerV3VerificationCapabilityCarriageIdentityV5(copy_32(
                &fields[1],
            )?),
            result: ExactIdentityCoordinateV5::decode(&fields[2])?,
            machine_receipt: ExactIdentityCoordinateV5::decode(&fields[3])?,
            object_receipt: ExactIdentityCoordinateV5::decode(&fields[4])?,
            object: ExactIdentityCoordinateV5::decode(&fields[5])?,
            association: ExactIdentityCoordinateV5::decode(&fields[6])?,
            proof_owner: ExactIdentityCoordinateV5::decode(&fields[7])?,
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    pub const fn request_carriage_identity(
        &self,
    ) -> WorkerV3VerificationCapabilityCarriageIdentityV5 {
        self.request_carriage
    }
    pub const fn result_identity(&self) -> ExactIdentityCoordinateV5 {
        self.result
    }
    pub const fn machine_receipt_identity(&self) -> ExactIdentityCoordinateV5 {
        self.machine_receipt
    }
    pub const fn object_receipt_identity(&self) -> ExactIdentityCoordinateV5 {
        self.object_receipt
    }
    pub const fn object_identity(&self) -> ExactIdentityCoordinateV5 {
        self.object
    }
    pub const fn association_identity(&self) -> ExactIdentityCoordinateV5 {
        self.association
    }
    pub const fn proof_owner_identity(&self) -> ExactIdentityCoordinateV5 {
        self.proof_owner
    }
    /// Identity coordinates do not authenticate or transfer the completed artifact owner.
    pub const fn grants_artifact_completion_authority(&self) -> bool {
        false
    }
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// V5 service disposition. Neither variant confers verifier authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerV3VerificationCapabilityResponseDispositionV5 {
    Completed = 1,
    Rejected = 2,
}

/// Correlated V5 completion or generic rejection response.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationCapabilityResponseV5 {
    disposition: WorkerV3VerificationCapabilityResponseDispositionV5,
    request: WorkerV3VerificationCapabilityRequestIdentityV5,
    challenge: [u8; 32],
    completion: Option<WorkerV3VerificationCapabilityCompletionV5>,
    identity: WorkerV3VerificationCapabilityResponseIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl WorkerV3VerificationCapabilityResponseV5 {
    pub fn completed(
        request: &WorkerV3VerificationCapabilityRequestV5,
        completion: WorkerV3VerificationCapabilityCompletionV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        if completion.request_carriage != request.carriage.identity
            || completion.object != request.carriage.object
        {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::CompletionMismatch);
        }
        Self::build(
            request,
            WorkerV3VerificationCapabilityResponseDispositionV5::Completed,
            Some(completion),
        )
    }

    pub fn rejected(
        request: &WorkerV3VerificationCapabilityRequestV5,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        Self::build(
            request,
            WorkerV3VerificationCapabilityResponseDispositionV5::Rejected,
            None,
        )
    }

    fn build(
        request: &WorkerV3VerificationCapabilityRequestV5,
        disposition: WorkerV3VerificationCapabilityResponseDispositionV5,
        completion: Option<WorkerV3VerificationCapabilityCompletionV5>,
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let disposition_bytes = [disposition as u8];
        let completion_bytes = completion
            .as_ref()
            .map_or(&[][..], |value| value.encode_canonical());
        let challenge = request.base.challenge();
        let fields = [
            RESPONSE_DOMAIN,
            disposition_bytes.as_slice(),
            request.identity.0.as_slice(),
            challenge.as_bytes().as_slice(),
            completion_bytes,
        ];
        let canonical_bytes = encode_record(
            RESPONSE_KIND,
            &fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
            RESPONSE_IDENTITY_DOMAIN,
        )?;
        let identity =
            WorkerV3VerificationCapabilityResponseIdentityV5(terminal_identity(&canonical_bytes)?);
        Ok(Self {
            disposition,
            request: request.identity,
            challenge: *request.base.challenge().as_bytes(),
            completion,
            identity,
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3VerificationCapabilityProtocolErrorV5> {
        let record = decode_record(
            bytes,
            RESPONSE_KIND,
            RESPONSE_FIELDS,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
            RESPONSE_DOMAIN,
            RESPONSE_IDENTITY_DOMAIN,
        )?;
        require_len(record.fields[1], 1)?;
        require_len(record.fields[2], 32)?;
        require_len(record.fields[3], 32)?;
        require_nonzero_32(record.fields[2])?;
        require_nonzero_32(record.fields[3])?;
        let disposition = match record.fields[1][0] {
            1 => WorkerV3VerificationCapabilityResponseDispositionV5::Completed,
            2 => WorkerV3VerificationCapabilityResponseDispositionV5::Rejected,
            actual => {
                return Err(
                    WorkerV3VerificationCapabilityProtocolErrorV5::UnknownDisposition(actual),
                );
            }
        };
        let completion = match disposition {
            WorkerV3VerificationCapabilityResponseDispositionV5::Completed => {
                if record.fields[4].is_empty() {
                    return Err(WorkerV3VerificationCapabilityProtocolErrorV5::OmittedCompletion);
                }
                Some(
                    WorkerV3VerificationCapabilityCompletionV5::decode_canonical(record.fields[4])?,
                )
            }
            WorkerV3VerificationCapabilityResponseDispositionV5::Rejected => {
                if !record.fields[4].is_empty() {
                    return Err(
                        WorkerV3VerificationCapabilityProtocolErrorV5::RejectionHasCompletion,
                    );
                }
                None
            }
        };
        Ok(Self {
            disposition,
            request: WorkerV3VerificationCapabilityRequestIdentityV5(copy_32(record.fields[2])?),
            challenge: copy_32(record.fields[3])?,
            completion,
            identity: WorkerV3VerificationCapabilityResponseIdentityV5(terminal_identity(bytes)?),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
        })
    }

    pub fn matches_request(&self, request: &WorkerV3VerificationCapabilityRequestV5) -> bool {
        self.request == request.identity
            && self.challenge == *request.base.challenge().as_bytes()
            && self.completion.as_ref().is_none_or(|completion| {
                completion.request_carriage == request.carriage.identity
                    && completion.object == request.carriage.object
            })
    }

    pub const fn disposition(&self) -> WorkerV3VerificationCapabilityResponseDispositionV5 {
        self.disposition
    }
    pub const fn request_identity(&self) -> WorkerV3VerificationCapabilityRequestIdentityV5 {
        self.request
    }
    pub const fn completion(&self) -> Option<&WorkerV3VerificationCapabilityCompletionV5> {
        self.completion.as_ref()
    }
    pub const fn identity(&self) -> WorkerV3VerificationCapabilityResponseIdentityV5 {
        self.identity
    }
    /// A correlated response is still not a signed artifact-completion capability.
    pub const fn grants_artifact_completion_authority(&self) -> bool {
        false
    }
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationCapabilityProtocolErrorV5 {
    TooLarge { actual: usize, maximum: usize },
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u16),
    WrongRecordKind,
    WrongFieldCount,
    WrongFieldTag,
    NoncanonicalFlags,
    DeclaredLengthMismatch,
    WrongDomain,
    ZeroIdentity,
    InvalidAttempt,
    InvalidTransaction,
    Downgrade,
    StaleEpoch,
    SubjectMismatch,
    ObjectMismatch,
    CompletionMismatch,
    InvalidBaseRequest,
    PrepublicationEvidenceRequired,
    UnknownDisposition(u8),
    OmittedCompletion,
    RejectionHasCompletion,
    InvalidFieldLength,
    IdentityMismatch,
    NonCanonical,
    LengthOverflow,
    AllocationFailed,
}

impl fmt::Display for WorkerV3VerificationCapabilityProtocolErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Worker V3 V5 capability carriage: {self:?}"
        )
    }
}

impl Error for WorkerV3VerificationCapabilityProtocolErrorV5 {}

struct DecodedRecord<'a> {
    fields: Vec<&'a [u8]>,
}

fn encode_record(
    kind: u16,
    fields: &[impl AsRef<[u8]>],
    maximum: usize,
    identity_domain: &[u8],
) -> Result<Vec<u8>, WorkerV3VerificationCapabilityProtocolErrorV5> {
    let body = fields.iter().try_fold(0usize, |total, field| {
        total
            .checked_add(FIELD_HEADER_BYTES)
            .and_then(|value| value.checked_add(field.as_ref().len()))
            .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)
    })?;
    let total = HEADER_BYTES
        .checked_add(body)
        .and_then(|value| value.checked_add(IDENTITY_BYTES))
        .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
    if total > maximum {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::TooLarge {
            actual: total,
            maximum,
        });
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::AllocationFailed)?;
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&WORKER_V3_VERIFICATION_CAPABILITY_VERSION_V5.to_le_bytes());
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(fields.len())
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(total)
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for (index, field) in fields.iter().enumerate() {
        let field = field.as_ref();
        bytes.extend_from_slice(
            &u16::try_from(index + 1)
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(field.len())
                .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(field);
    }
    let mut digest = Sha256::new();
    digest.update(identity_domain);
    digest.update(&bytes);
    bytes.extend_from_slice(&<[u8; 32]>::from(digest.finalize()));
    Ok(bytes)
}

fn decode_record<'a>(
    bytes: &'a [u8],
    kind: u16,
    field_count: usize,
    maximum: usize,
    domain: &[u8],
    identity_domain: &[u8],
) -> Result<DecodedRecord<'a>, WorkerV3VerificationCapabilityProtocolErrorV5> {
    if bytes.len() > maximum {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::TooLarge {
            actual: bytes.len(),
            maximum,
        });
    }
    if bytes.len() < HEADER_BYTES + IDENTITY_BYTES {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::Truncated);
    }
    if bytes[..8] != MAGIC {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidMagic);
    }
    let version = read_u16(&bytes[8..10])?;
    if version != WORKER_V3_VERIFICATION_CAPABILITY_VERSION_V5 {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::UnsupportedVersion(version));
    }
    if read_u16(&bytes[10..12])? != kind {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongRecordKind);
    }
    if usize::from(read_u16(&bytes[12..14])?) != field_count {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongFieldCount);
    }
    if read_u16(&bytes[14..16])? != 0 || read_u32(&bytes[20..24])? != 0 {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::NoncanonicalFlags);
    }
    if usize::try_from(read_u32(&bytes[16..20])?).ok() != Some(bytes.len()) {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::DeclaredLengthMismatch);
    }

    let payload_end = bytes.len() - IDENTITY_BYTES;
    let mut offset = HEADER_BYTES;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::AllocationFailed)?;
    for index in 0..field_count {
        if offset
            .checked_add(FIELD_HEADER_BYTES)
            .is_none_or(|end| end > payload_end)
        {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::Truncated);
        }
        if usize::from(read_u16(&bytes[offset..offset + 2])?) != index + 1 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongFieldTag);
        }
        if read_u16(&bytes[offset + 2..offset + 4])? != 0 {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::NoncanonicalFlags);
        }
        let len = usize::try_from(read_u32(&bytes[offset + 4..offset + 8])?)
            .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
        let start = offset + FIELD_HEADER_BYTES;
        let end = start
            .checked_add(len)
            .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::LengthOverflow)?;
        if end > payload_end {
            return Err(WorkerV3VerificationCapabilityProtocolErrorV5::Truncated);
        }
        fields.push(&bytes[start..end]);
        offset = end;
    }
    if offset != payload_end {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::DeclaredLengthMismatch);
    }
    if fields.first().copied() != Some(domain) {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongDomain);
    }
    let mut digest = Sha256::new();
    digest.update(identity_domain);
    digest.update(&bytes[..payload_end]);
    let actual_identity: [u8; 32] = digest.finalize().into();
    if actual_identity != bytes[payload_end..] {
        return Err(WorkerV3VerificationCapabilityProtocolErrorV5::IdentityMismatch);
    }
    Ok(DecodedRecord { fields })
}

fn terminal_identity(
    bytes: &[u8],
) -> Result<[u8; 32], WorkerV3VerificationCapabilityProtocolErrorV5> {
    bytes
        .get(
            bytes
                .len()
                .checked_sub(32)
                .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::Truncated)?..,
        )
        .ok_or(WorkerV3VerificationCapabilityProtocolErrorV5::Truncated)?
        .try_into()
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::Truncated)
}

fn require_len(
    bytes: &[u8],
    expected: usize,
) -> Result<(), WorkerV3VerificationCapabilityProtocolErrorV5> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(WorkerV3VerificationCapabilityProtocolErrorV5::InvalidFieldLength)
    }
}

fn require_nonzero_32(bytes: &[u8]) -> Result<(), WorkerV3VerificationCapabilityProtocolErrorV5> {
    if copy_32(bytes)? == [0; 32] {
        Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn copy_32(bytes: &[u8]) -> Result<[u8; 32], WorkerV3VerificationCapabilityProtocolErrorV5> {
    bytes
        .try_into()
        .map_err(|_| WorkerV3VerificationCapabilityProtocolErrorV5::InvalidFieldLength)
}

fn read_u16(bytes: &[u8]) -> Result<u16, WorkerV3VerificationCapabilityProtocolErrorV5> {
    Ok(u16::from_le_bytes(bytes.try_into().map_err(|_| {
        WorkerV3VerificationCapabilityProtocolErrorV5::Truncated
    })?))
}

fn read_u32(bytes: &[u8]) -> Result<u32, WorkerV3VerificationCapabilityProtocolErrorV5> {
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        WorkerV3VerificationCapabilityProtocolErrorV5::Truncated
    })?))
}

fn read_u64(bytes: &[u8]) -> Result<u64, WorkerV3VerificationCapabilityProtocolErrorV5> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| {
        WorkerV3VerificationCapabilityProtocolErrorV5::Truncated
    })?))
}

fn require_canonical(
    actual: &[u8],
    supplied: &[u8],
) -> Result<(), WorkerV3VerificationCapabilityProtocolErrorV5> {
    if actual == supplied {
        Ok(())
    } else {
        Err(WorkerV3VerificationCapabilityProtocolErrorV5::NonCanonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WorkerV3VerificationEntryCoordinateV1, WorkerV3VerificationFdPayloadDescriptorV1,
        WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
        WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationRosterIdentityV1,
    };

    fn id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn carriage_fields(seed: u8) -> [Vec<u8>; CARRIAGE_FIELDS] {
        let identity = |value| {
            ExactIdentityCoordinateV5::new(id(value), u64::from(value) + 1)
                .unwrap()
                .encode()
                .to_vec()
        };
        [
            CARRIAGE_DOMAIN.to_vec(),
            identity(seed),
            identity(seed + 1),
            id(seed + 2).to_vec(),
            WorkerV3VerificationProductionAttemptV5::new(7, [seed + 3; 16], id(seed + 4))
                .unwrap()
                .encode()
                .to_vec(),
            vec![0],
            13_u16.to_le_bytes().to_vec(),
            identity(seed + 5),
            9_u64.to_le_bytes().to_vec(),
            id(seed + 6).to_vec(),
            id(seed + 7).to_vec(),
            id(seed + 8).to_vec(),
            id(seed + 9).to_vec(),
            id(seed + 10).to_vec(),
            identity(seed + 11),
            id(seed + 12).to_vec(),
            identity(seed + 13),
            ExactIdentityCoordinateV5::new(id(90), 4096)
                .unwrap()
                .encode()
                .to_vec(),
            id(seed + 14).to_vec(),
            id(82).to_vec(),
            identity(seed + 16),
        ]
    }

    fn carriage(seed: u8) -> WorkerV3VerificationCapabilityCarriageV5 {
        WorkerV3VerificationCapabilityCarriageV5::from_fields(carriage_fields(seed)).unwrap()
    }

    fn base_request(challenge: u8) -> WorkerV3VerificationRequestV1 {
        WorkerV3VerificationRequestV1::new(
            WorkerV3VerificationFreshChallengeV1::new(id(challenge)).unwrap(),
            WorkerV3VerificationRosterIdentityV1::new(id(81)).unwrap(),
            WorkerV3VerificationPolicyIdentityV1::new(id(82)).unwrap(),
            WorkerV3VerificationMeasurementIdentityV1::new(id(83)).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(1024, id(84)).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(4096, id(90)).unwrap(),
            vec![
                WorkerV3VerificationEntryCoordinateV1::new(
                    0,
                    "kernel",
                    "kernel_export",
                    id(85),
                    id(86),
                    id(87),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn request(challenge: u8) -> WorkerV3VerificationCapabilityRequestV5 {
        WorkerV3VerificationCapabilityRequestV5::new(base_request(challenge), carriage(1)).unwrap()
    }

    fn completion(
        request: &WorkerV3VerificationCapabilityRequestV5,
    ) -> WorkerV3VerificationCapabilityCompletionV5 {
        let coordinate = |seed| {
            ExactIdentityCoordinateV5::new(id(seed), u64::from(seed) + 100)
                .unwrap()
                .encode()
                .to_vec()
        };
        WorkerV3VerificationCapabilityCompletionV5::from_fields([
            COMPLETION_DOMAIN.to_vec(),
            request.carriage().identity().0.to_vec(),
            coordinate(51),
            coordinate(52),
            coordinate(53),
            request.carriage().object_identity().encode().to_vec(),
            coordinate(54),
            coordinate(55),
        ])
        .unwrap()
    }

    fn machine_finalization(seed: u8) -> InertWorkerV3MachineRefinedFinalizationV5 {
        InertWorkerV3MachineRefinedFinalizationV5::new(
            WorkerV3VerificationProductionAttemptV5::new(7, [seed; 16], id(seed + 1)).unwrap(),
            id(seed + 2),
            ExactIdentityCoordinateV5::new(id(seed + 3), 300).unwrap(),
            ExactIdentityCoordinateV5::new(id(seed + 4), 400).unwrap(),
            ExactIdentityCoordinateV5::new(id(seed + 5), 500).unwrap(),
            id(seed + 6),
            id(seed + 7),
            id(seed + 8),
            b"typed-machine-receipt",
        )
        .unwrap()
    }

    #[test]
    fn identity_coordinate_rejects_sentinels() {
        assert_eq!(
            ExactIdentityCoordinateV5::new([0; 32], 1),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity)
        );
        assert_eq!(
            ExactIdentityCoordinateV5::new([1; 32], 0),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::ZeroIdentity)
        );
    }

    #[test]
    fn record_codec_rejects_truncation_downgrade_reorder_and_exhaustion() {
        let fields = [CARRIAGE_DOMAIN];
        let exact = encode_record(CARRIAGE_KIND, &fields, 1024, CARRIAGE_IDENTITY_DOMAIN).unwrap();
        for prefix in 0..exact.len() {
            assert!(
                decode_record(
                    &exact[..prefix],
                    CARRIAGE_KIND,
                    1,
                    1024,
                    CARRIAGE_DOMAIN,
                    CARRIAGE_IDENTITY_DOMAIN,
                )
                .is_err()
            );
        }

        let mut downgraded = exact.clone();
        downgraded[8..10].copy_from_slice(&4_u16.to_le_bytes());
        assert!(matches!(
            decode_record(
                &downgraded,
                CARRIAGE_KIND,
                1,
                1024,
                CARRIAGE_DOMAIN,
                CARRIAGE_IDENTITY_DOMAIN,
            ),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::UnsupportedVersion(4))
        ));

        let mut reordered = exact.clone();
        reordered[HEADER_BYTES..HEADER_BYTES + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            decode_record(
                &reordered,
                CARRIAGE_KIND,
                1,
                1024,
                CARRIAGE_DOMAIN,
                CARRIAGE_IDENTITY_DOMAIN,
            ),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongFieldTag)
        ));
        assert!(matches!(
            decode_record(
                &exact,
                CARRIAGE_KIND,
                1,
                exact.len() - 1,
                CARRIAGE_DOMAIN,
                CARRIAGE_IDENTITY_DOMAIN,
            ),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::TooLarge { .. })
        ));
    }

    #[test]
    fn record_identity_is_deterministic() {
        let fields = [REQUEST_DOMAIN, b"same".as_slice()];
        let first = encode_record(REQUEST_KIND, &fields, 1024, REQUEST_IDENTITY_DOMAIN).unwrap();
        let second = encode_record(REQUEST_KIND, &fields, 1024, REQUEST_IDENTITY_DOMAIN).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn request_completion_and_response_round_trip_exactly() {
        let request = request(71);
        let request_bytes = request.encode_canonical().to_vec();
        let decoded =
            WorkerV3VerificationCapabilityRequestV5::decode_canonical(&request_bytes).unwrap();
        assert_eq!(decoded, request);

        let response =
            WorkerV3VerificationCapabilityResponseV5::completed(&request, completion(&request))
                .unwrap();
        let response_bytes = response.encode_canonical().to_vec();
        let decoded_response =
            WorkerV3VerificationCapabilityResponseV5::decode_canonical(&response_bytes).unwrap();
        assert_eq!(decoded_response, response);
        assert!(decoded_response.matches_request(&request));
        assert!(!decoded_response.grants_artifact_completion_authority());
        assert!(
            !decoded_response
                .completion()
                .unwrap()
                .grants_artifact_completion_authority()
        );
    }

    #[test]
    fn every_request_prefix_and_field_reorder_fails() {
        let request = request(72);
        let bytes = request.encode_canonical();
        for prefix in 0..bytes.len() {
            assert!(
                WorkerV3VerificationCapabilityRequestV5::decode_canonical(&bytes[..prefix])
                    .is_err()
            );
        }
        let mut reordered = bytes.to_vec();
        reordered[HEADER_BYTES..HEADER_BYTES + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            WorkerV3VerificationCapabilityRequestV5::decode_canonical(&reordered),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongFieldTag)
        ));
    }

    #[test]
    fn v12_downgrade_stale_epoch_and_cross_subject_changes_fail_closed() {
        let mut fields = carriage_fields(2);
        fields[6] = 12_u16.to_le_bytes().to_vec();
        assert!(matches!(
            WorkerV3VerificationCapabilityCarriageV5::from_fields(fields),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::Downgrade)
        ));

        let mut fields = carriage_fields(2);
        fields[8] = 0_u64.to_le_bytes().to_vec();
        assert!(matches!(
            WorkerV3VerificationCapabilityCarriageV5::from_fields(fields),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::StaleEpoch)
        ));

        let first = carriage(2);
        for (field, replacement) in [
            (
                2,
                ExactIdentityCoordinateV5::new(id(119), 120)
                    .unwrap()
                    .encode()
                    .to_vec(),
            ),
            (
                4,
                WorkerV3VerificationProductionAttemptV5::new(8, [120; 16], id(121))
                    .unwrap()
                    .encode()
                    .to_vec(),
            ),
            (8, 8_u64.to_le_bytes().to_vec()),
            (9, id(122).to_vec()),
            (11, id(123).to_vec()),
            (12, id(124).to_vec()),
            (
                17,
                ExactIdentityCoordinateV5::new(id(125), 4096)
                    .unwrap()
                    .encode()
                    .to_vec(),
            ),
        ] {
            let mut changed = carriage_fields(2);
            changed[field] = replacement;
            let changed = WorkerV3VerificationCapabilityCarriageV5::from_fields(changed).unwrap();
            assert_ne!(first.identity(), changed.identity());
        }

        let mut v12 = carriage_fields(2);
        v12[0] = b"FE2O3/WORKER-V3/CAPABILITY-CARRIAGE/EXACT-V12/V5\0".to_vec();
        assert!(matches!(
            WorkerV3VerificationCapabilityCarriageV5::from_fields(v12),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongDomain)
        ));

        let request = request(76);
        let request_fields = [
            b"FE2O3/WORKER-V3/CAPABILITY-REQUEST/EXACT-V12/V5\0".as_slice(),
            request.base_request().encode_canonical(),
            request.carriage().encode_canonical(),
        ];
        let substituted = encode_record(
            REQUEST_KIND,
            &request_fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_REQUEST_BYTES_V5,
            REQUEST_IDENTITY_DOMAIN,
        )
        .unwrap();
        assert!(matches!(
            WorkerV3VerificationCapabilityRequestV5::decode_canonical(&substituted),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::WrongDomain)
        ));
    }

    #[test]
    fn response_replay_cross_request_and_completion_omission_are_rejected() {
        let first = request(73);
        let replayed =
            WorkerV3VerificationCapabilityResponseV5::completed(&first, completion(&first))
                .unwrap();
        let second = request(74);
        assert!(!replayed.matches_request(&second));

        let disposition = [WorkerV3VerificationCapabilityResponseDispositionV5::Completed as u8];
        let challenge = first.base_request().challenge();
        let request_identity = first.identity();
        let fields = [
            RESPONSE_DOMAIN,
            disposition.as_slice(),
            request_identity.0.as_slice(),
            challenge.as_bytes().as_slice(),
            &[][..],
        ];
        let omitted = encode_record(
            RESPONSE_KIND,
            &fields,
            MAX_WORKER_V3_VERIFICATION_CAPABILITY_RESPONSE_BYTES_V5,
            RESPONSE_IDENTITY_DOMAIN,
        )
        .unwrap();
        assert!(matches!(
            WorkerV3VerificationCapabilityResponseV5::decode_canonical(&omitted),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::OmittedCompletion)
        ));
    }

    #[test]
    fn field_role_substitution_changes_canonical_identity() {
        let original = carriage(3);
        let mut fields = carriage_fields(3);
        fields.swap(1, 2);
        let substituted = WorkerV3VerificationCapabilityCarriageV5::from_fields(fields).unwrap();
        assert_ne!(original.identity(), substituted.identity());
        assert_ne!(original.handoff_identity(), substituted.handoff_identity());
    }

    #[test]
    fn cross_artifact_completion_is_rejected_before_response() {
        let request = request(75);
        let wrong_object = ExactIdentityCoordinateV5::new(id(126), 4096).unwrap();
        assert!(matches!(
            WorkerV3VerificationCapabilityCompletionV5::new_exact(
                &request,
                ExactIdentityCoordinateV5::new(id(51), 151).unwrap(),
                ExactIdentityCoordinateV5::new(id(52), 152).unwrap(),
                ExactIdentityCoordinateV5::new(id(53), 153).unwrap(),
                wrong_object,
                ExactIdentityCoordinateV5::new(id(54), 154).unwrap(),
                ExactIdentityCoordinateV5::new(id(55), 155).unwrap(),
            ),
            Err(WorkerV3VerificationCapabilityProtocolErrorV5::CompletionMismatch)
        ));
    }

    #[test]
    fn machine_finalization_round_trip_and_splice_axes_are_exact() {
        let original = machine_finalization(31);
        let decoded =
            InertWorkerV3MachineRefinedFinalizationV5::decode_canonical(original.canonical_bytes())
                .unwrap();
        assert_eq!(decoded, original);
        assert!(!decoded.grants_completion_authority());

        let base = |attempt, transaction, handoff, raw, finalized, target, launch, policy| {
            InertWorkerV3MachineRefinedFinalizationV5::new(
                attempt,
                transaction,
                handoff,
                raw,
                finalized,
                target,
                launch,
                policy,
                b"typed-machine-receipt",
            )
            .unwrap()
        };
        let attempt = WorkerV3VerificationProductionAttemptV5::new(7, [31; 16], id(32)).unwrap();
        let handoff = ExactIdentityCoordinateV5::new(id(34), 300).unwrap();
        let raw = ExactIdentityCoordinateV5::new(id(35), 400).unwrap();
        let finalized = ExactIdentityCoordinateV5::new(id(36), 500).unwrap();
        for substituted in [
            base(
                WorkerV3VerificationProductionAttemptV5::new(8, [31; 16], id(32)).unwrap(),
                id(33),
                handoff,
                raw,
                finalized,
                id(37),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(40),
                handoff,
                raw,
                finalized,
                id(37),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(33),
                ExactIdentityCoordinateV5::new(id(41), 300).unwrap(),
                raw,
                finalized,
                id(37),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(33),
                handoff,
                ExactIdentityCoordinateV5::new(id(42), 400).unwrap(),
                finalized,
                id(37),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(33),
                handoff,
                raw,
                ExactIdentityCoordinateV5::new(id(43), 500).unwrap(),
                id(37),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(33),
                handoff,
                raw,
                finalized,
                id(44),
                id(38),
                id(39),
            ),
            base(
                attempt,
                id(33),
                handoff,
                raw,
                finalized,
                id(37),
                id(45),
                id(39),
            ),
            base(
                attempt,
                id(33),
                handoff,
                raw,
                finalized,
                id(37),
                id(38),
                id(46),
            ),
        ] {
            assert_ne!(substituted.identity(), original.identity());
        }

        let substituted_receipt = InertWorkerV3MachineRefinedFinalizationV5::new(
            attempt,
            id(33),
            handoff,
            raw,
            finalized,
            id(37),
            id(38),
            id(39),
            b"substituted-machine-receipt",
        )
        .unwrap();
        assert_ne!(substituted_receipt.identity(), original.identity());

        let mut omitted = original.canonical_bytes().to_vec();
        omitted.truncate(omitted.len() - IDENTITY_BYTES - 1);
        assert!(InertWorkerV3MachineRefinedFinalizationV5::decode_canonical(&omitted).is_err());
    }
}
