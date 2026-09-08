//! Fixed-deployment Worker V3 verifier and semantic-machine refinement backends.

use std::fs::File;
use std::io::{self, Write as _};
use std::marker::PhantomData;
use std::os::fd::{AsFd as _, AsRawFd as _, OwnedFd};
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
use fe2o3_compiler_ffi::InertProductionCapabilityResultV5;
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
};
use fe2o3_proof_contracts::CapabilitySubjectV1;
use fe2o3_verifier::{
    ValidatedCompilerProofInputsV5, validate_compiler_capability_evidence_v1,
    validate_compiler_proof_inputs_v5,
};
use fe2o3_worker_v3_verification_client::{
    WorkerV3VerificationBeginOutcomeV2, WorkerV3VerificationClientV2,
    WorkerV3VerificationPayloadSnapshotsV1,
};
use fe2o3_worker_v3_verification_protocol::{
    WorkerV3VerificationEntryCoordinateV1, WorkerV3VerificationFdPayloadDescriptorV1,
    WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
    WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationRequestV1 as ProtocolRequestV1,
    WorkerV3VerificationRosterIdentityV1, WorkerV3VerificationTerminalDispositionV2,
};
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with};
use sha2::{Digest as _, Sha256};

use super::worker_v3_verification_admission::verifier_seal;
use super::{
    CompilerGeneratedKernelExpectationRosterV1, CompilerGeneratedKernelExpectationV1,
    InheritedWorkerV3CompilerCurrentRecordAuditorV1, WorkerV3CapabilityResultEvidenceViewV1,
    WorkerV3ProtectedCapabilityRosterEvidenceV1,
    WorkerV3ProtectedCapabilityRosterVerifierBackendV1, WorkerV3ProtectedRosterEntryEvidenceV1,
    WorkerV3ProtectedRosterVerificationEvidenceV1,
    WorkerV3ProtectedSemanticMachineRefinementEvidenceV1, WorkerV3ProtectedVerificationEvidenceV1,
    WorkerV3ProtectedVerifierBackendV1, WorkerV3RefiningProtectedVerifierAdapterV1,
    WorkerV3RefiningProtectedVerifierErrorV1, WorkerV3RosterVerificationRequestV1,
    WorkerV3SafetyPropertiesV1, WorkerV3SemanticMachineRefinementBackendV1,
    WorkerV3SemanticMachineRefinementEvidenceErrorV1, WorkerV3SemanticMachineRefinementRequestV1,
    WorkerV3VerificationDecisionV1, WorkerV3VerificationRequestV1, WorkerV3VerifierV1,
};

/// Sole production endpoint for the issue #272 protected verifier.
pub const PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1: &str = "/run/fe2o3/worker-v3-verifier.sock";

/// One absolute deadline covers connect, challenge, current-record audit, and terminal response.
pub const PRODUCTION_WORKER_V3_VERIFIER_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Exact canonical byte length of one protected-verifier application response.
pub const PRODUCTION_WORKER_V3_VERIFICATION_SERVICE_RESPONSE_BYTES_V1: usize = 472;

const PRODUCTION_RUNTIME_DIRECTORY_V1: &str = "/run/fe2o3";
const PRODUCTION_RUNTIME_DIRECTORY_MODE_V1: u32 = 0o755;
const PRODUCTION_SOCKET_MODE_V1: u32 = 0o660;
const RESPONSE_MAGIC_V1: [u8; 8] = *b"F3WVPR1\0";
const RESPONSE_VERSION_V1: u16 = 1;
const RESPONSE_BYTES_V1: usize = PRODUCTION_WORKER_V3_VERIFICATION_SERVICE_RESPONSE_BYTES_V1;
const RESPONSE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/HOST/WORKER-V3/PROTECTED-VERIFIER-RESPONSE/V1\0";
const SUBJECT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/HOST/WORKER-V3/CAPABILITY-SUBJECT/V1\0";
const VERIFIER_MEASUREMENT_DOMAIN_V1: &[u8] =
    b"FE2O3/HOST/WORKER-V3/FIXED-VERIFIER-MEASUREMENT/V1\0";
const ROSTER_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/HOST/WORKER-V3/PRODUCTION-ROSTER/V1\0";
const VERIFICATION_TRANSCRIPT_DOMAIN_V1: &[u8] =
    b"FE2O3/HOST/WORKER-V3/PROTECTED-VERIFICATION-TRANSCRIPT/V1\0";
const REFINEMENT_REQUEST_DOMAIN_V1: &[u8] = b"FE2O3/HOST/WORKER-V3/SEMANTIC-MACHINE-REQUEST/V1\0";
const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

/// Canonical, authority-free terminal result emitted by the fixed protected service.
///
/// The service must derive the selected subject and complete obligation set from its own protected
/// policy, authenticate the exact machine-refinement receipt, and bind every reported theorem to
/// the correlated V2 session. The named machine receipt must be the complete canonical proof
/// bundle whose bytes encode both machine-effect evidence and semantic-to-machine refinement;
/// this host module does not reinterpret or synthesize either claim. Safe construction or decoding
/// of this record grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionWorkerV3VerificationServiceResponseV1 {
    request_identity: [u8; 32],
    client_profile_identity: [u8; 32],
    production_result_sha256: [u8; 32],
    production_result_bytes: u64,
    capability_association_sha256: [u8; 32],
    capability_association_bytes: u64,
    subject_ordinal: u32,
    subject_identity: [u8; 32],
    obligation_set_identity: [u8; 32],
    machine_refinement_receipt_sha256: [u8; 32],
    machine_refinement_receipt_bytes: u64,
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    safety_properties: WorkerV3SafetyPropertiesV1,
    producer_measurement_sha256: [u8; 32],
    service_transcript_sha256: [u8; 32],
    canonical_bytes: [u8; RESPONSE_BYTES_V1],
}

impl ProductionWorkerV3VerificationServiceResponseV1 {
    /// Constructs the exact protected-service response schema.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_identity: [u8; 32],
        client_profile_identity: [u8; 32],
        production_result_sha256: [u8; 32],
        production_result_bytes: u64,
        capability_association_sha256: [u8; 32],
        capability_association_bytes: u64,
        subject_ordinal: u32,
        subject_identity: [u8; 32],
        obligation_set_identity: [u8; 32],
        machine_refinement_receipt_sha256: [u8; 32],
        machine_refinement_receipt_bytes: u64,
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
        producer_measurement_sha256: [u8; 32],
        service_transcript_sha256: [u8; 32],
    ) -> Result<Self, ProductionWorkerV3VerificationServiceResponseErrorV1> {
        for (field, value) in [
            ("request identity", request_identity),
            ("client-profile identity", client_profile_identity),
            ("production-result identity", production_result_sha256),
            (
                "capability-association identity",
                capability_association_sha256,
            ),
            ("capability subject identity", subject_identity),
            ("obligation-set identity", obligation_set_identity),
            (
                "machine-refinement receipt identity",
                machine_refinement_receipt_sha256,
            ),
            ("proof executable binding", proof_executable_binding_sha256),
            (
                "Rust type-layout contract",
                rust_type_layout_contract_sha256,
            ),
            ("Rust effect contract", rust_effect_contract_sha256),
            ("proof producer measurement", producer_measurement_sha256),
            ("service transcript", service_transcript_sha256),
        ] {
            if value == [0; 32] {
                return Err(
                    ProductionWorkerV3VerificationServiceResponseErrorV1::ZeroIdentity(field),
                );
            }
        }
        for (field, value) in [
            ("production result", production_result_bytes),
            ("capability association", capability_association_bytes),
            (
                "machine-refinement receipt",
                machine_refinement_receipt_bytes,
            ),
        ] {
            if value == 0 {
                return Err(
                    ProductionWorkerV3VerificationServiceResponseErrorV1::ZeroLength(field),
                );
            }
        }
        if safety_properties != WorkerV3SafetyPropertiesV1::required() {
            return Err(
                ProductionWorkerV3VerificationServiceResponseErrorV1::IncompleteSafetyProperties,
            );
        }

        let mut canonical_bytes = [0_u8; RESPONSE_BYTES_V1];
        let mut writer = ResponseWriter::new(&mut canonical_bytes);
        writer.put(&RESPONSE_MAGIC_V1);
        writer.put(&RESPONSE_VERSION_V1.to_le_bytes());
        writer.put(&0_u16.to_le_bytes());
        writer.put(&(RESPONSE_BYTES_V1 as u32).to_le_bytes());
        for value in [
            request_identity,
            client_profile_identity,
            production_result_sha256,
        ] {
            writer.put(&value);
        }
        writer.put(&production_result_bytes.to_le_bytes());
        writer.put(&capability_association_sha256);
        writer.put(&capability_association_bytes.to_le_bytes());
        writer.put(&subject_ordinal.to_le_bytes());
        writer.put(&0_u32.to_le_bytes());
        for value in [
            subject_identity,
            obligation_set_identity,
            machine_refinement_receipt_sha256,
        ] {
            writer.put(&value);
        }
        writer.put(&machine_refinement_receipt_bytes.to_le_bytes());
        for value in [
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
        ] {
            writer.put(&value);
        }
        writer.put(&[safety_properties.bits()]);
        writer.put(&[0; 7]);
        writer.put(&producer_measurement_sha256);
        writer.put(&service_transcript_sha256);
        let terminal_offset = writer.offset();
        debug_assert_eq!(terminal_offset + 32, RESPONSE_BYTES_V1);
        let terminal = derive_identity(
            RESPONSE_IDENTITY_DOMAIN_V1,
            &canonical_bytes[..terminal_offset],
        );
        canonical_bytes[terminal_offset..].copy_from_slice(&terminal);

        Ok(Self {
            request_identity,
            client_profile_identity,
            production_result_sha256,
            production_result_bytes,
            capability_association_sha256,
            capability_association_bytes,
            subject_ordinal,
            subject_identity,
            obligation_set_identity,
            machine_refinement_receipt_sha256,
            machine_refinement_receipt_bytes,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
            producer_measurement_sha256,
            service_transcript_sha256,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete byte-identical response.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, ProductionWorkerV3VerificationServiceResponseErrorV1> {
        if bytes.len() != RESPONSE_BYTES_V1 {
            return Err(
                ProductionWorkerV3VerificationServiceResponseErrorV1::InvalidLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = ResponseReader::new(bytes);
        if reader.take::<8>()? != RESPONSE_MAGIC_V1 {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Magic);
        }
        let version = u16::from_le_bytes(reader.take()?);
        if version != RESPONSE_VERSION_V1 {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Version(version));
        }
        if u16::from_le_bytes(reader.take()?) != 0 {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Flags);
        }
        let declared = u32::from_le_bytes(reader.take()?);
        if declared as usize != bytes.len() {
            return Err(
                ProductionWorkerV3VerificationServiceResponseErrorV1::DeclaredLength(declared),
            );
        }
        let request_identity = reader.take()?;
        let client_profile_identity = reader.take()?;
        let production_result_sha256 = reader.take()?;
        let production_result_bytes = u64::from_le_bytes(reader.take()?);
        let capability_association_sha256 = reader.take()?;
        let capability_association_bytes = u64::from_le_bytes(reader.take()?);
        let subject_ordinal = u32::from_le_bytes(reader.take()?);
        if u32::from_le_bytes(reader.take()?) != 0 {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Flags);
        }
        let subject_identity = reader.take()?;
        let obligation_set_identity = reader.take()?;
        let machine_refinement_receipt_sha256 = reader.take()?;
        let machine_refinement_receipt_bytes = u64::from_le_bytes(reader.take()?);
        let proof_executable_binding_sha256 = reader.take()?;
        let rust_type_layout_contract_sha256 = reader.take()?;
        let rust_effect_contract_sha256 = reader.take()?;
        let safety_bits = reader.take::<1>()?[0];
        if reader.take::<7>()? != [0; 7] {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Flags);
        }
        let producer_measurement_sha256 = reader.take()?;
        let service_transcript_sha256 = reader.take()?;
        let terminal = reader.take::<32>()?;
        if !reader.is_empty()
            || terminal == [0; 32]
            || terminal
                != derive_identity(
                    RESPONSE_IDENTITY_DOMAIN_V1,
                    &bytes[..RESPONSE_BYTES_V1 - 32],
                )
        {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Identity);
        }
        let safety_properties = WorkerV3SafetyPropertiesV1::new(safety_bits).ok_or(
            ProductionWorkerV3VerificationServiceResponseErrorV1::IncompleteSafetyProperties,
        )?;
        let decoded = Self::new(
            request_identity,
            client_profile_identity,
            production_result_sha256,
            production_result_bytes,
            capability_association_sha256,
            capability_association_bytes,
            subject_ordinal,
            subject_identity,
            obligation_set_identity,
            machine_refinement_receipt_sha256,
            machine_refinement_receipt_bytes,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
            producer_measurement_sha256,
            service_transcript_sha256,
        )?;
        if decoded.canonical_bytes.as_slice() != bytes {
            return Err(ProductionWorkerV3VerificationServiceResponseErrorV1::Canonical);
        }
        Ok(decoded)
    }

    pub const fn request_identity(&self) -> &[u8; 32] {
        &self.request_identity
    }

    pub const fn client_profile_identity(&self) -> &[u8; 32] {
        &self.client_profile_identity
    }

    pub const fn production_result_identity(&self) -> (&[u8; 32], u64) {
        (&self.production_result_sha256, self.production_result_bytes)
    }

    pub const fn capability_association_identity(&self) -> (&[u8; 32], u64) {
        (
            &self.capability_association_sha256,
            self.capability_association_bytes,
        )
    }

    pub const fn subject_ordinal(&self) -> u32 {
        self.subject_ordinal
    }

    pub const fn subject_identity(&self) -> &[u8; 32] {
        &self.subject_identity
    }

    pub const fn obligation_set_identity(&self) -> &[u8; 32] {
        &self.obligation_set_identity
    }

    pub const fn machine_refinement_receipt_identity(&self) -> (&[u8; 32], u64) {
        (
            &self.machine_refinement_receipt_sha256,
            self.machine_refinement_receipt_bytes,
        )
    }

    pub const fn proof_executable_binding_sha256(&self) -> [u8; 32] {
        self.proof_executable_binding_sha256
    }

    pub const fn rust_type_layout_contract_sha256(&self) -> [u8; 32] {
        self.rust_type_layout_contract_sha256
    }

    pub const fn rust_effect_contract_sha256(&self) -> [u8; 32] {
        self.rust_effect_contract_sha256
    }

    pub const fn safety_properties(&self) -> WorkerV3SafetyPropertiesV1 {
        self.safety_properties
    }

    pub const fn producer_measurement_sha256(&self) -> [u8; 32] {
        self.producer_measurement_sha256
    }

    pub const fn service_transcript_sha256(&self) -> [u8; 32] {
        self.service_transcript_sha256
    }

    pub const fn canonical_bytes(&self) -> &[u8; RESPONSE_BYTES_V1] {
        &self.canonical_bytes
    }

    /// This canonical record is inert outside the authenticated fixed-service session.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Strict response-schema failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProductionWorkerV3VerificationServiceResponseErrorV1 {
    InvalidLength { actual: usize },
    Magic,
    Version(u16),
    Flags,
    DeclaredLength(u32),
    ZeroIdentity(&'static str),
    ZeroLength(&'static str),
    IncompleteSafetyProperties,
    Truncated,
    Identity,
    Canonical,
}

/// Domain-separated identity of one complete capability subject.
pub fn production_worker_v3_subject_identity_v1(subject: CapabilitySubjectV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SUBJECT_IDENTITY_DOMAIN_V1);
    for identity in [
        subject.kernel().digest(),
        subject.root().digest(),
        subject.executable_kir().digest(),
        subject.target_model().digest(),
        subject.launch_contract().digest(),
    ] {
        digest.update(identity.as_bytes());
    }
    digest.update(subject.executable_kir_epoch().to_le_bytes());
    digest.finalize().into()
}

/// Fixed policy key resolved by the protected service to its measured verifier closure.
///
/// This identity is deterministic deployment configuration, not proof that the service measured
/// itself. The service-side resolver must authenticate the corresponding executable/runtime
/// closure before accepting the request.
pub fn production_worker_v3_verifier_measurement_identity_v1(
    profile: &CompilerExecutionClientProfileV1,
) -> [u8; 32] {
    let executable = profile.policy().executable();
    let runtime = profile.policy().runtime();
    let mut digest = Sha256::new();
    digest.update(VERIFIER_MEASUREMENT_DOMAIN_V1);
    digest.update(profile.identity().as_bytes());
    digest.update(profile.policy().identity().as_bytes());
    digest.update(executable.sha256());
    digest.update(executable.byte_len().to_le_bytes());
    digest.update(runtime.sha256());
    digest.update(runtime.byte_len().to_le_bytes());
    digest.update(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1.as_bytes());
    digest.update(RESPONSE_MAGIC_V1);
    digest.update(RESPONSE_VERSION_V1.to_le_bytes());
    digest.finalize().into()
}

/// Failure to acquire the fixed profile, service, FD-195 auditor, or exact V5 result.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3VerifierDeploymentErrorV1 {
    ClientProfile(String),
    ProductionResult(String),
    InvalidClientIdentity,
    InvalidRuntimeDirectory(&'static str),
    InvalidServiceSocket(&'static str),
    ServiceSocketChanged,
    ServiceConnect(String),
    ServiceCredentialsMismatch,
    ServiceClientAdmission(String),
    CompilerCurrentRecordAuditor(String),
    DeadlineOverflow,
}

/// Protected verification failure. Every variant consumes or leaves unusable the one-shot lane.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3ProtectedVerifierErrorV1 {
    AlreadyConsumed(&'static str),
    ClientProfileChanged(String),
    CompilerPolicyMismatch,
    ProductionResultMismatch(&'static str),
    RequestProtocol(String),
    PayloadSnapshot(String),
    ServiceTransport(String),
    ServiceRejected,
    CompilerCurrentRecord(String),
    CompilerExecutionBinding(String),
    ServiceResponse(ProductionWorkerV3VerificationServiceResponseErrorV1),
    AuthenticatedServiceMismatch(&'static str),
    Finalizer(String),
    ProofInputs(String),
    TargetLineage(String),
    CapabilityEvidence(String),
    RefinementStatePoisoned,
    RefinementStateOccupied,
    Allocation,
}

/// Semantic-machine handoff failure after protected verification succeeded.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3SemanticMachineRefinementErrorV1 {
    StatePoisoned,
    MissingProtectedResult,
    RequestMismatch,
    Allocation,
    Evidence(WorkerV3SemanticMachineRefinementEvidenceErrorV1),
}

/// Error returned by the complete production adapter.
pub type ProductionWorkerV3VerifierErrorV1 = WorkerV3RefiningProtectedVerifierErrorV1<
    ProductionWorkerV3ProtectedVerifierErrorV1,
    ProductionWorkerV3SemanticMachineRefinementErrorV1,
>;

struct PendingMachineRefinementV1 {
    request_binding: [u8; 32],
    machine_receipt: Box<[u8]>,
    producer_measurement: [u8; 32],
    verification_transcript: [u8; 32],
}

type SharedMachineRefinementV1 = Arc<Mutex<Option<PendingMachineRefinementV1>>>;

/// Fixed-deployment implementation of the unsafe protected-verifier contract.
///
/// This type has no public constructor. Use [`ProductionWorkerV3VerifierV1`] so its paired
/// refinement backend shares the same one-shot authenticated service result.
pub struct ProductionWorkerV3ProtectedVerifierBackendV1<K> {
    profile: CompilerExecutionClientProfileCapabilityV1,
    client: Option<WorkerV3VerificationClientV2>,
    auditor: Option<InheritedWorkerV3CompilerCurrentRecordAuditorV1>,
    production_result: InertProductionCapabilityResultV5,
    refinement: SharedMachineRefinementV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> std::fmt::Debug for ProductionWorkerV3ProtectedVerifierBackendV1<K> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionWorkerV3ProtectedVerifierBackendV1")
            .field("service_available", &self.client.is_some())
            .field("auditor_available", &self.auditor.is_some())
            .field("authority", &"none until sealed adapter promotion")
            .finish_non_exhaustive()
    }
}

/// Paired one-shot implementation of the semantic-machine-refinement contract.
pub struct ProductionWorkerV3SemanticMachineRefinementBackendV1<K> {
    refinement: SharedMachineRefinementV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> std::fmt::Debug for ProductionWorkerV3SemanticMachineRefinementBackendV1<K> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionWorkerV3SemanticMachineRefinementBackendV1")
            .field("authority", &"none until sealed adapter promotion")
            .finish_non_exhaustive()
    }
}

/// Complete production verifier accepted by `AuthenticatedWorkerV3ExecutableV1::authenticate`.
pub struct ProductionWorkerV3VerifierV1<K> {
    inner: WorkerV3RefiningProtectedVerifierAdapterV1<
        ProductionWorkerV3ProtectedVerifierBackendV1<K>,
        ProductionWorkerV3SemanticMachineRefinementBackendV1<K>,
    >,
}

/// Fixed-deployment protected verifier for one complete compiler-generated capability roster.
pub struct ProductionWorkerV3CapabilityRosterVerifierV1<R> {
    profile: CompilerExecutionClientProfileCapabilityV1,
    client: Option<WorkerV3VerificationClientV2>,
    auditor: Option<InheritedWorkerV3CompilerCurrentRecordAuditorV1>,
    production_result: InertProductionCapabilityResultV5,
    _roster: PhantomData<fn() -> R>,
}

impl<R> std::fmt::Debug for ProductionWorkerV3CapabilityRosterVerifierV1<R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionWorkerV3CapabilityRosterVerifierV1")
            .field("service_available", &self.client.is_some())
            .field("auditor_available", &self.auditor.is_some())
            .field("authority", &"none until aggregate admission")
            .finish_non_exhaustive()
    }
}

impl<R: CompilerGeneratedKernelExpectationRosterV1>
    ProductionWorkerV3CapabilityRosterVerifierV1<R>
{
    /// Acquires the fixed protected deployment for the exact completed V5 result.
    pub fn from_production_deployment(
        production_result: InertProductionCapabilityResultV5,
    ) -> Result<Self, ProductionWorkerV3VerifierDeploymentErrorV1> {
        let (profile, client, auditor, production_result) =
            acquire_production_verifier_deployment(production_result)?;
        Ok(Self {
            profile,
            client: Some(client),
            auditor: Some(auditor),
            production_result,
            _roster: PhantomData,
        })
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> ProductionWorkerV3VerifierV1<K> {
    /// Acquires only the fixed profile, fixed service socket, inherited FD 195, and exact V5 result.
    ///
    /// There is deliberately no alternate pathname, preconnected socket, raw KFD path, or
    /// authority-bearing fallback.
    pub fn from_production_deployment(
        production_result: InertProductionCapabilityResultV5,
    ) -> Result<Self, ProductionWorkerV3VerifierDeploymentErrorV1> {
        let (profile, client, auditor, production_result) =
            acquire_production_verifier_deployment(production_result)?;

        let refinement = Arc::new(Mutex::new(None));
        let protected = ProductionWorkerV3ProtectedVerifierBackendV1 {
            profile,
            client: Some(client),
            auditor: Some(auditor),
            production_result,
            refinement: Arc::clone(&refinement),
            _marker: PhantomData,
        };
        let refinement = ProductionWorkerV3SemanticMachineRefinementBackendV1 {
            refinement,
            _marker: PhantomData,
        };
        Ok(Self {
            inner: WorkerV3RefiningProtectedVerifierAdapterV1::new(protected, refinement),
        })
    }
}

fn acquire_production_verifier_deployment(
    production_result: InertProductionCapabilityResultV5,
) -> Result<
    (
        CompilerExecutionClientProfileCapabilityV1,
        WorkerV3VerificationClientV2,
        InheritedWorkerV3CompilerCurrentRecordAuditorV1,
        InertProductionCapabilityResultV5,
    ),
    ProductionWorkerV3VerifierDeploymentErrorV1,
> {
    let production_result = InertProductionCapabilityResultV5::decode(
        production_result.canonical_bytes(),
    )
    .map_err(|error| {
        ProductionWorkerV3VerifierDeploymentErrorV1::ProductionResult(error.to_string())
    })?;
    let profile = CompilerExecutionClientProfileCapabilityV1::from_production_profile()
        .map_err(ProductionWorkerV3VerifierDeploymentErrorV1::ClientProfile)?;
    profile
        .revalidate()
        .map_err(ProductionWorkerV3VerifierDeploymentErrorV1::ClientProfile)?;
    if rustix::process::geteuid().as_raw() == profile.profile().supervisor_uid() {
        return Err(ProductionWorkerV3VerifierDeploymentErrorV1::InvalidClientIdentity);
    }
    let deadline = Instant::now()
        .checked_add(PRODUCTION_WORKER_V3_VERIFIER_TIMEOUT_V1)
        .ok_or(ProductionWorkerV3VerifierDeploymentErrorV1::DeadlineOverflow)?;
    let before = inspect_fixed_service_socket(profile.profile())?;
    let peer = connect_fixed_service(deadline)?;
    validate_service_credentials(&peer, profile.profile())?;
    let after = inspect_fixed_service_socket(profile.profile())?;
    if before != after {
        return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceSocketChanged);
    }
    let client = WorkerV3VerificationClientV2::admit_connected_path_until(
        peer,
        Path::new(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1),
        deadline,
    )
    .map_err(|failure| {
        ProductionWorkerV3VerifierDeploymentErrorV1::ServiceClientAdmission(
            failure.source_error().to_string(),
        )
    })?;
    let auditor =
        InheritedWorkerV3CompilerCurrentRecordAuditorV1::admit_inherited_application_service()
            .map_err(|error| {
                ProductionWorkerV3VerifierDeploymentErrorV1::CompilerCurrentRecordAuditor(
                    error.to_string(),
                )
            })?;
    profile
        .revalidate()
        .map_err(ProductionWorkerV3VerifierDeploymentErrorV1::ClientProfile)?;
    Ok((profile, client, auditor, production_result))
}

impl<K: CompilerGeneratedKernelExpectationV1> verifier_seal::Sealed<K>
    for ProductionWorkerV3VerifierV1<K>
{
}

// SAFETY: the wrapper delegates exclusively to the crate-owned refining adapter and the two
// concrete backends below. It introduces no constructor or evidence path of its own.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3VerifierV1<K>
    for ProductionWorkerV3VerifierV1<K>
{
    type Error = ProductionWorkerV3VerifierErrorV1;

    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        // SAFETY: `self.inner` is the sealed adapter over the production backends below.
        unsafe { WorkerV3VerifierV1::verify(&mut self.inner, request) }
    }
}

// SAFETY: the implementation below uses the fixed credential-admitted verifier service and the
// one-shot inherited compiler-currentness endpoint. It independently reconstructs common
// multi-root owners and binds the sole V1 response entry to the complete request. The V1 response
// schema cannot represent a multi-entry result, so those requests are rejected after the protected
// session instead of projecting one entry across the roster.
unsafe impl<R: CompilerGeneratedKernelExpectationRosterV1>
    WorkerV3ProtectedCapabilityRosterVerifierBackendV1<R>
    for ProductionWorkerV3CapabilityRosterVerifierV1<R>
{
    type Error = ProductionWorkerV3ProtectedVerifierErrorV1;

    unsafe fn verify_protected_capability_roster(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
        capability: WorkerV3CapabilityResultEvidenceViewV1<'_>,
    ) -> Result<WorkerV3ProtectedCapabilityRosterEvidenceV1, Self::Error> {
        self.verify_once(request, capability)
    }
}

impl<R: CompilerGeneratedKernelExpectationRosterV1>
    ProductionWorkerV3CapabilityRosterVerifierV1<R>
{
    fn verify_once(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
        capability: WorkerV3CapabilityResultEvidenceViewV1<'_>,
    ) -> Result<
        WorkerV3ProtectedCapabilityRosterEvidenceV1,
        ProductionWorkerV3ProtectedVerifierErrorV1,
    > {
        let client = self.client.take().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::AlreadyConsumed("protected service"),
        )?;
        let mut auditor = self.auditor.take().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::AlreadyConsumed(
                "compiler current-record auditor",
            ),
        )?;
        self.profile
            .revalidate()
            .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ClientProfileChanged)?;
        let profile = self.profile.profile();
        let policy_identity = *profile.policy().identity().as_bytes();
        let carried = capability.production_result();
        if carried.canonical_bytes() != self.production_result.canonical_bytes()
            || capability.production_result_identity() != self.production_result.identity()
        {
            return Err(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "capability carrier",
                ),
            );
        }
        if request.compiler_execution_policy_sha256() != policy_identity
            || carried.handoff().inputs().compiler_policy() != policy_identity
        {
            return Err(ProductionWorkerV3ProtectedVerifierErrorV1::CompilerPolicyMismatch);
        }
        if carried.handoff().legacy_handoff().canonical_bytes()
            != request.semantic_compiler_handoff().canonical_bytes()
        {
            return Err(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "legacy semantic handoff",
                ),
            );
        }
        if carried.object_output().output_sha256() != request.finalized_hsaco_sha256()
            || carried.object_output().output_bytes() != request.finalized_hsaco_length()
        {
            return Err(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "finalized object",
                ),
            );
        }

        let finalizer = request
            .independently_revalidate_finalizer_derivation()
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::Finalizer(error.to_string())
            })?;
        let proof_inputs = request
            .validate_compiler_multi_root_proof_inputs_v1()
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::ProofInputs(error.to_string())
            })?;
        let target_lineage = request
            .validate_compiler_multi_root_target_lineage_v1(&proof_inputs)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::TargetLineage(error.to_string())
            })?;
        let (protocol_request, snapshots) = build_roster_protocol_request(
            request,
            profile,
            carried,
            *capability.dynamic_precondition_roster_identity().as_bytes(),
        )?;
        let protocol_request_bytes = copy_vec(protocol_request.encode_canonical())
            .ok_or(ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
        let protocol_request_identity = *protocol_request.identity().as_bytes();
        let outcome = client.begin(protocol_request, snapshots).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::ServiceTransport(error.to_string())
        })?;
        let reserved = match outcome {
            WorkerV3VerificationBeginOutcomeV2::Reserved(reserved) => reserved,
            _ => return Err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceRejected),
        };
        let (challenge, pending) = reserved.into_parts();
        let compiler_challenge =
            challenge
                .into_compiler_execution_challenge()
                .map_err(|error| {
                    ProductionWorkerV3ProtectedVerifierErrorV1::CompilerCurrentRecord(
                        error.to_string(),
                    )
                })?;
        let audit = auditor
            .audit_roster_with_challenge(request, compiler_challenge)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::CompilerCurrentRecord(error.to_string())
            })?;
        let current = audit.canonical_evidence_view();
        let current_verification = *current.verification_canonical_bytes();
        let current_attestation = *current.attestation_canonical_bytes();
        let compiler_execution = audit
            .bind_exact_compiler_execution_v1(
                request.compiler_execution_subject(),
                request.compiler_execution_receipt_carriage(),
            )
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::CompilerExecutionBinding(
                    error.to_string(),
                )
            })?;
        let terminal = pending
            .submit_current_record(current_verification, current_attestation)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::ServiceTransport(error.to_string())
            })?;
        if terminal.disposition() != WorkerV3VerificationTerminalDispositionV2::ApplicationResponse
        {
            return Err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceRejected);
        }
        let response = ProductionWorkerV3VerificationServiceResponseV1::decode_canonical(
            terminal.application_response_bytes(),
        )
        .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceResponse)?;
        self.profile
            .revalidate()
            .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ClientProfileChanged)?;

        let expected_measurement = production_worker_v3_verifier_measurement_identity_v1(profile);
        let result_identity = carried.identity();
        require_service_match(
            response.request_identity() == &protocol_request_identity,
            "protocol request identity",
        )?;
        require_service_match(
            response.client_profile_identity() == profile.identity().as_bytes(),
            "client-profile identity",
        )?;
        require_service_match(
            response.production_result_identity()
                == (&result_identity.sha256(), result_identity.byte_len()),
            "production-result identity",
        )?;
        require_service_match(
            response.producer_measurement_sha256() == expected_measurement,
            "proof-producer measurement",
        )?;
        require_service_match(
            R::ENTRIES.len() == 1,
            "complete roster response cardinality",
        )?;

        let subject = carried.handoff().subjects().first().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                "capability subject roster",
            ),
        )?;
        let association = carried.capability_associations().entries().first().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                "capability association roster",
            ),
        )?;
        let obligations = carried.handoff().obligation_roster().first().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                "capability obligation roster",
            ),
        )?;
        let association_identity = association.identity();
        let machine_identity = carried.machine_refinement().identity();
        require_service_match(
            response.subject_ordinal() == 0,
            "capability subject ordinal",
        )?;
        require_service_match(
            response.subject_identity() == &production_worker_v3_subject_identity_v1(*subject),
            "capability subject",
        )?;
        require_service_match(
            response.capability_association_identity()
                == (
                    &association_identity.sha256(),
                    association_identity.byte_len(),
                ),
            "capability association",
        )?;
        require_service_match(
            response.obligation_set_identity() == obligations.identity().digest().as_bytes(),
            "complete obligation set",
        )?;
        require_service_match(
            response.machine_refinement_receipt_identity()
                == (&machine_identity.sha256(), machine_identity.byte_len()),
            "machine-refinement receipt",
        )?;
        require_service_match(
            response.safety_properties() == WorkerV3SafetyPropertiesV1::required(),
            "required safety properties",
        )?;

        let marker = &R::ENTRIES[0];
        let lineage = request.entry_lineage_identity(0).ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch("entry lineage"),
        )?;
        // SAFETY: all entry fields came from the credential-admitted fixed service and were
        // independently rebound above to the exact request, result, subject, and association.
        let entry = unsafe {
            WorkerV3ProtectedRosterEntryEvidenceV1::new(
                lineage,
                marker.kernel_binding_id(),
                marker.generated_host_contract_identity(),
                response.proof_executable_binding_sha256(),
                response.rust_type_layout_contract_sha256(),
                response.rust_effect_contract_sha256(),
                response.safety_properties(),
            )
        };
        let verification_transcript = verification_transcript_identity(
            &protocol_request_bytes,
            &current_verification,
            &current_attestation,
            terminal.encode_canonical(),
            response.service_transcript_sha256(),
        );
        // SAFETY: common owners were independently reconstructed from the exact request and the
        // sole entry was authenticated and rebound above.
        let roster = unsafe {
            WorkerV3ProtectedRosterVerificationEvidenceV1::new(
                finalizer,
                compiler_execution,
                proof_inputs,
                target_lineage,
                expected_measurement,
                verification_transcript,
                vec![entry],
            )
        };
        // SAFETY: `carried` is the byte-identical owner independently retained by this backend.
        Ok(unsafe { WorkerV3ProtectedCapabilityRosterEvidenceV1::new(roster, carried.identity()) })
    }
}

// SAFETY: this implementation accepts only the root-tree-admitted sealed profile capability,
// a metadata- and credential-admitted fixed pathname service, the canonical V2 session, fresh
// FD-195 currentness,
// and independently revalidated exact V4/V5 owners. Every service-reported coordinate is compared
// with those owners before evidence is returned. Any mismatch consumes the one-shot lane.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3ProtectedVerifierBackendV1<K>
    for ProductionWorkerV3ProtectedVerifierBackendV1<K>
{
    type Error = ProductionWorkerV3ProtectedVerifierErrorV1;

    unsafe fn verify_protected(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, Self::Error> {
        self.verify_once(request)
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> ProductionWorkerV3ProtectedVerifierBackendV1<K> {
    fn verify_once(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, ProductionWorkerV3ProtectedVerifierErrorV1>
    {
        let client = self.client.take().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::AlreadyConsumed("protected service"),
        )?;
        let mut auditor = self.auditor.take().ok_or(
            ProductionWorkerV3ProtectedVerifierErrorV1::AlreadyConsumed(
                "compiler current-record auditor",
            ),
        )?;
        self.profile
            .revalidate()
            .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ClientProfileChanged)?;
        let profile = self.profile.profile();
        let policy_identity = *profile.policy().identity().as_bytes();
        if request.compiler_execution_policy_sha256() != policy_identity
            || self.production_result.handoff().inputs().compiler_policy() != policy_identity
        {
            return Err(ProductionWorkerV3ProtectedVerifierErrorV1::CompilerPolicyMismatch);
        }
        if self
            .production_result
            .handoff()
            .legacy_handoff()
            .canonical_bytes()
            != request.semantic_compiler_handoff().canonical_bytes()
        {
            return Err(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "legacy semantic handoff",
                ),
            );
        }
        if self.production_result.object_output().output_sha256()
            != request.finalized_hsaco_sha256()
            || self.production_result.object_output().output_bytes()
                != request.finalized_hsaco_length()
        {
            return Err(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "finalized object",
                ),
            );
        }

        let finalizer = request
            .independently_revalidate_finalizer_derivation()
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::Finalizer(error.to_string())
            })?;
        let proof_inputs = request
            .validate_compiler_proof_inputs_v4()
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::ProofInputs(error.to_string())
            })?;
        let target_lineage = request
            .validate_compiler_target_lineage_v1(&proof_inputs)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::TargetLineage(error.to_string())
            })?;
        let (protocol_request, snapshots) =
            build_protocol_request(request, profile, &self.production_result)?;
        let protocol_request_bytes = copy_vec(protocol_request.encode_canonical())
            .ok_or(ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
        let protocol_request_identity = *protocol_request.identity().as_bytes();
        let outcome = client.begin(protocol_request, snapshots).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::ServiceTransport(error.to_string())
        })?;
        let reserved = match outcome {
            WorkerV3VerificationBeginOutcomeV2::Reserved(reserved) => reserved,
            _ => {
                return Err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceRejected);
            }
        };
        let (challenge, pending) = reserved.into_parts();
        let compiler_challenge =
            challenge
                .into_compiler_execution_challenge()
                .map_err(|error| {
                    ProductionWorkerV3ProtectedVerifierErrorV1::CompilerCurrentRecord(
                        error.to_string(),
                    )
                })?;
        let audit = auditor
            .audit_with_challenge(request, compiler_challenge)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::CompilerCurrentRecord(error.to_string())
            })?;
        let current = audit.canonical_evidence_view();
        let current_verification = *current.verification_canonical_bytes();
        let current_attestation = *current.attestation_canonical_bytes();
        let compiler_execution = audit
            .bind_exact_compiler_execution_v1(
                request.compiler_execution_subject(),
                request.compiler_execution_receipt_carriage(),
            )
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::CompilerExecutionBinding(
                    error.to_string(),
                )
            })?;
        let terminal = pending
            .submit_current_record(current_verification, current_attestation)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::ServiceTransport(error.to_string())
            })?;
        if terminal.disposition() != WorkerV3VerificationTerminalDispositionV2::ApplicationResponse
        {
            return Err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceRejected);
        }
        let response = ProductionWorkerV3VerificationServiceResponseV1::decode_canonical(
            terminal.application_response_bytes(),
        )
        .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ServiceResponse)?;
        self.profile
            .revalidate()
            .map_err(ProductionWorkerV3ProtectedVerifierErrorV1::ClientProfileChanged)?;

        let expected_measurement = production_worker_v3_verifier_measurement_identity_v1(profile);
        let result_identity = self.production_result.identity();
        require_service_match(
            response.request_identity() == &protocol_request_identity,
            "protocol request identity",
        )?;
        require_service_match(
            response.client_profile_identity() == profile.identity().as_bytes(),
            "client-profile identity",
        )?;
        require_service_match(
            response.production_result_identity()
                == (&result_identity.sha256(), result_identity.byte_len()),
            "production-result identity",
        )?;
        require_service_match(
            response.producer_measurement_sha256() == expected_measurement,
            "proof-producer measurement",
        )?;

        let (subject_ordinal, subject, association) =
            select_marker_capability::<K>(&self.production_result)?;
        let association_identity = association.identity();
        let obligation_identity = association.obligation_set_identity();
        let machine_receipt = self.production_result.machine_refinement();
        let machine_identity = machine_receipt.identity();
        require_service_match(
            response.subject_ordinal() == subject_ordinal,
            "capability subject ordinal",
        )?;
        require_service_match(
            response.subject_identity() == &production_worker_v3_subject_identity_v1(subject),
            "capability subject",
        )?;
        require_service_match(
            response.capability_association_identity()
                == (
                    &association_identity.sha256(),
                    association_identity.byte_len(),
                ),
            "capability association",
        )?;
        require_service_match(
            response.obligation_set_identity() == obligation_identity.digest().as_bytes(),
            "complete obligation set",
        )?;
        require_service_match(
            response.machine_refinement_receipt_identity()
                == (&machine_identity.sha256(), machine_identity.byte_len()),
            "machine-refinement receipt",
        )?;
        require_service_match(
            response.safety_properties() == WorkerV3SafetyPropertiesV1::required(),
            "required safety properties",
        )?;

        let source_receipt = copy_refinement_receipt(
            self.production_result.handoff().source_refinement(),
            InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
        )?;
        let machine_receipt_owner = copy_refinement_receipt(
            machine_receipt,
            InertCapabilityRefinementReceiptKindV1::Machine,
        )?;
        let capability = validate_compiler_capability_evidence_v1(
            association.canonical_bytes(),
            self.production_result.handoff().legacy_handoff().capsule(),
            self.production_result.handoff().executable_kir(),
            subject,
            obligation_identity,
            Some(source_receipt),
            Some(machine_receipt_owner),
        )
        .map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::CapabilityEvidence(error.to_string())
        })?;
        let proof_owner = validate_compiler_proof_inputs_v5(
            self.production_result.proof_owner().canonical_bytes(),
            &proof_inputs,
            self.production_result.handoff().executable_kir(),
            capability,
            policy_identity,
        )
        .map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::CapabilityEvidence(error.to_string())
        })?;
        let request_binding = refinement_request_binding(
            request,
            &compiler_execution,
            &proof_owner,
            selected_isa(request).ok_or(
                ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                    "selected ISA range",
                ),
            )?,
        );
        let verification_transcript = verification_transcript_identity(
            &protocol_request_bytes,
            &current_verification,
            &current_attestation,
            terminal.encode_canonical(),
            response.service_transcript_sha256(),
        );
        let retained_machine_receipt = copy_box(machine_receipt.canonical_preimage())
            .ok_or(ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;

        // SAFETY: every service-owned value above arrived over the fixed credential-admitted V2
        // session and was compared with independently reconstructed host/V5 owners. The concrete
        // backend unsafe contract is implemented immediately above this method.
        let evidence = unsafe {
            WorkerV3ProtectedVerificationEvidenceV1::new(
                finalizer,
                compiler_execution,
                proof_inputs,
                target_lineage,
                expected_measurement,
                verification_transcript,
                response.proof_executable_binding_sha256(),
                response.rust_type_layout_contract_sha256(),
                response.rust_effect_contract_sha256(),
                response.safety_properties(),
            )
        }
        .try_with_compiler_capability_evidence(proof_owner, &self.production_result)
        .map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::CapabilityEvidence(error.to_string())
        })?;
        let mut slot = self
            .refinement
            .lock()
            .map_err(|_| ProductionWorkerV3ProtectedVerifierErrorV1::RefinementStatePoisoned)?;
        if slot.is_some() {
            return Err(ProductionWorkerV3ProtectedVerifierErrorV1::RefinementStateOccupied);
        }
        *slot = Some(PendingMachineRefinementV1 {
            request_binding,
            machine_receipt: retained_machine_receipt,
            producer_measurement: expected_measurement,
            verification_transcript,
        });
        Ok(evidence)
    }
}

// SAFETY: the backend can return only the exact machine receipt retained by its paired protected
// backend after a successful authenticated service session. It consumes that state once and
// rebinds every semantic/KIR/LLVM/ISA/artifact/currentness/publication coordinate before return.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3SemanticMachineRefinementBackendV1<K>
    for ProductionWorkerV3SemanticMachineRefinementBackendV1<K>
{
    type Error = ProductionWorkerV3SemanticMachineRefinementErrorV1;

    unsafe fn verify_semantic_machine_refinement(
        &mut self,
        request: &WorkerV3SemanticMachineRefinementRequestV1<'_, '_, K>,
    ) -> Result<WorkerV3ProtectedSemanticMachineRefinementEvidenceV1, Self::Error> {
        let pending = self
            .refinement
            .lock()
            .map_err(|_| ProductionWorkerV3SemanticMachineRefinementErrorV1::StatePoisoned)?
            .take()
            .ok_or(ProductionWorkerV3SemanticMachineRefinementErrorV1::MissingProtectedResult)?;
        let actual_binding = refinement_request_binding(
            request.verification_request(),
            request.compiler_execution(),
            request.proof_owner(),
            request.selected_isa_bytes(),
        );
        if actual_binding != pending.request_binding {
            return Err(ProductionWorkerV3SemanticMachineRefinementErrorV1::RequestMismatch);
        }
        // V5 intentionally carries one canonical machine-proof bundle. The protected service has
        // authenticated that exact bundle as both the machine-effect output and the applicable
        // semantic-machine proof; the adapter retains both roles separately without projection.
        let machine_effect = copy_box(&pending.machine_receipt)
            .ok_or(ProductionWorkerV3SemanticMachineRefinementErrorV1::Allocation)?;
        WorkerV3ProtectedSemanticMachineRefinementEvidenceV1::new(
            machine_effect,
            pending.machine_receipt,
            pending.producer_measurement,
            pending.verification_transcript,
        )
        .map_err(ProductionWorkerV3SemanticMachineRefinementErrorV1::Evidence)
    }
}

fn build_protocol_request<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    profile: &CompilerExecutionClientProfileV1,
    result: &InertProductionCapabilityResultV5,
) -> Result<
    (ProtocolRequestV1, WorkerV3VerificationPayloadSnapshotsV1),
    ProductionWorkerV3ProtectedVerifierErrorV1,
> {
    let envelope = request.load_envelope_v2_bytes();
    let hsaco = request.finalized_hsaco_bytes();
    let envelope_len = u64::try_from(envelope.len()).map_err(|_| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
            "load-envelope length overflow".to_owned(),
        )
    })?;
    let hsaco_len = u64::try_from(hsaco.len()).map_err(|_| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
            "HSACO length overflow".to_owned(),
        )
    })?;
    let envelope_sha: [u8; 32] = Sha256::digest(envelope).into();
    let hsaco_sha: [u8; 32] = Sha256::digest(hsaco).into();
    if hsaco_sha != request.finalized_hsaco_sha256()
        || hsaco_len != request.finalized_hsaco_length()
    {
        return Err(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch("retained HSACO"),
        );
    }
    let result_identity = result.identity();
    let roster_identity = derive_identity_parts(
        ROSTER_IDENTITY_DOMAIN_V1,
        &[
            request.challenge_identity().as_bytes(),
            request.lineage_identity().as_bytes(),
            &request.marker_binding_identity(),
            &request.generated_host_contract_identity(),
            &envelope_sha,
            &envelope_len.to_le_bytes(),
            &hsaco_sha,
            &hsaco_len.to_le_bytes(),
            &result_identity.sha256(),
            &result_identity.byte_len().to_le_bytes(),
        ],
    );
    let measurement = production_worker_v3_verifier_measurement_identity_v1(profile);
    let entry = WorkerV3VerificationEntryCoordinateV1::new(
        0,
        request.marker_logical_name(),
        request.marker_export_name(),
        *request.lineage_identity().as_bytes(),
        request.marker_binding_identity(),
        request.generated_host_contract_identity(),
    )
    .map_err(|error| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
    })?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(1)
        .map_err(|_| ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
    entries.push(entry);
    let protocol = ProtocolRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new(fresh_challenge()?).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationRosterIdentityV1::new(roster_identity).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationPolicyIdentityV1::new(*profile.policy().identity().as_bytes())
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
            })?,
        WorkerV3VerificationMeasurementIdentityV1::new(measurement).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(envelope_len, envelope_sha)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
            })?,
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(hsaco_len, hsaco_sha).map_err(
            |error| ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string()),
        )?,
        entries,
    )
    .map_err(|error| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
    })?;
    let envelope_fd =
        sealed_read_only_memfd("fe2o3-worker-v3-envelope-v2", envelope).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
        })?;
    let hsaco_fd =
        sealed_read_only_memfd("fe2o3-worker-v3-final-hsaco", hsaco).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
        })?;
    let mut descriptors = Vec::new();
    descriptors
        .try_reserve_exact(2)
        .map_err(|_| ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
    descriptors.push(envelope_fd);
    descriptors.push(hsaco_fd);
    let snapshots =
        WorkerV3VerificationPayloadSnapshotsV1::admit(&protocol, descriptors).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
        })?;
    Ok((protocol, snapshots))
}

fn build_roster_protocol_request<R: CompilerGeneratedKernelExpectationRosterV1>(
    request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    profile: &CompilerExecutionClientProfileV1,
    result: &InertProductionCapabilityResultV5,
    dynamic_preconditions: [u8; 32],
) -> Result<
    (ProtocolRequestV1, WorkerV3VerificationPayloadSnapshotsV1),
    ProductionWorkerV3ProtectedVerifierErrorV1,
> {
    let envelope_view = request.load_envelope_evidence_view();
    let envelope = envelope_view.exact_canonical_bytes();
    let hsaco = request.finalized_hsaco_bytes();
    let envelope_len = u64::try_from(envelope.len()).map_err(|_| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
            "load-envelope length overflow".to_owned(),
        )
    })?;
    let hsaco_len = u64::try_from(hsaco.len()).map_err(|_| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
            "HSACO length overflow".to_owned(),
        )
    })?;
    let envelope_sha: [u8; 32] = Sha256::digest(envelope).into();
    let hsaco_sha: [u8; 32] = Sha256::digest(hsaco).into();
    if hsaco_sha != request.finalized_hsaco_sha256()
        || hsaco_len != request.finalized_hsaco_length()
    {
        return Err(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch("retained HSACO"),
        );
    }
    let result_identity = result.identity();
    let mut roster_digest = Sha256::new();
    roster_digest.update(ROSTER_IDENTITY_DOMAIN_V1);
    roster_digest.update(request.challenge_identity().as_bytes());
    roster_digest.update(request.roster_identity().as_bytes());
    roster_digest.update(dynamic_preconditions);
    roster_digest.update(envelope_sha);
    roster_digest.update(envelope_len.to_le_bytes());
    roster_digest.update(hsaco_sha);
    roster_digest.update(hsaco_len.to_le_bytes());
    roster_digest.update(result_identity.sha256());
    roster_digest.update(result_identity.byte_len().to_le_bytes());
    roster_digest.update((R::ENTRIES.len() as u64).to_le_bytes());
    let roster_identity: [u8; 32] = roster_digest.finalize().into();

    let mut entries = Vec::new();
    entries
        .try_reserve_exact(R::ENTRIES.len())
        .map_err(|_| ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
    for (ordinal, marker) in R::ENTRIES.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).map_err(|_| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
                "roster ordinal overflow".to_owned(),
            )
        })?;
        let lineage = request
            .entry_lineage_identity(ordinal as usize)
            .ok_or_else(|| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
                    "missing roster entry lineage".to_owned(),
                )
            })?;
        entries.push(
            WorkerV3VerificationEntryCoordinateV1::new(
                ordinal,
                marker.logical_name(),
                marker.export_name(),
                *lineage.as_bytes(),
                marker.kernel_binding_id(),
                marker.generated_host_contract_identity(),
            )
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
            })?,
        );
    }
    let protocol = ProtocolRequestV1::new(
        WorkerV3VerificationFreshChallengeV1::new(fresh_challenge()?).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationRosterIdentityV1::new(roster_identity).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationPolicyIdentityV1::new(*profile.policy().identity().as_bytes())
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
            })?,
        WorkerV3VerificationMeasurementIdentityV1::new(
            production_worker_v3_verifier_measurement_identity_v1(profile),
        )
        .map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
        })?,
        WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(envelope_len, envelope_sha)
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
            })?,
        WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(hsaco_len, hsaco_sha).map_err(
            |error| ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string()),
        )?,
        entries,
    )
    .map_err(|error| {
        ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(error.to_string())
    })?;
    let envelope_fd =
        sealed_read_only_memfd("fe2o3-worker-v3-envelope-v2", envelope).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
        })?;
    let hsaco_fd =
        sealed_read_only_memfd("fe2o3-worker-v3-final-hsaco", hsaco).map_err(|error| {
            ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
        })?;
    let snapshots =
        WorkerV3VerificationPayloadSnapshotsV1::admit(&protocol, vec![envelope_fd, hsaco_fd])
            .map_err(|error| {
                ProductionWorkerV3ProtectedVerifierErrorV1::PayloadSnapshot(error.to_string())
            })?;
    Ok((protocol, snapshots))
}

fn select_marker_capability<K: CompilerGeneratedKernelExpectationV1>(
    result: &InertProductionCapabilityResultV5,
) -> Result<
    (
        u32,
        CapabilitySubjectV1,
        &fe2o3_compiler_lineage::InertStaticCapabilityEvidenceAssociationV1,
    ),
    ProductionWorkerV3ProtectedVerifierErrorV1,
> {
    let mut selected = None;
    for (index, association) in result
        .capability_associations()
        .entries()
        .iter()
        .enumerate()
    {
        if association.subject().kernel().digest().as_bytes() == &K::KERNEL_BINDING_ID_V1 {
            if selected.is_some() {
                return Err(
                    ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                        "duplicate marker capability subject",
                    ),
                );
            }
            selected = Some((index, association));
        }
    }
    let (index, association) = selected.ok_or(
        ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
            "marker capability subject",
        ),
    )?;
    let ordinal = u32::try_from(index).map_err(|_| {
        ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
            "capability subject ordinal",
        )
    })?;
    Ok((ordinal, association.subject(), association))
}

fn copy_refinement_receipt(
    receipt: &InertCapabilityRefinementReceiptV1,
    kind: InertCapabilityRefinementReceiptKindV1,
) -> Result<InertCapabilityRefinementReceiptV1, ProductionWorkerV3ProtectedVerifierErrorV1> {
    if receipt.kind() != kind {
        return Err(
            ProductionWorkerV3ProtectedVerifierErrorV1::ProductionResultMismatch(
                "refinement receipt kind",
            ),
        );
    }
    let bytes = copy_vec(receipt.canonical_preimage())
        .ok_or(ProductionWorkerV3ProtectedVerifierErrorV1::Allocation)?;
    InertCapabilityRefinementReceiptV1::from_canonical_preimage(kind, bytes).map_err(|error| {
        ProductionWorkerV3ProtectedVerifierErrorV1::CapabilityEvidence(error.to_string())
    })
}

fn require_service_match(
    matches: bool,
    field: &'static str,
) -> Result<(), ProductionWorkerV3ProtectedVerifierErrorV1> {
    if matches {
        Ok(())
    } else {
        Err(ProductionWorkerV3ProtectedVerifierErrorV1::AuthenticatedServiceMismatch(field))
    }
}

fn selected_isa<'request, K: CompilerGeneratedKernelExpectationV1>(
    request: &'request WorkerV3VerificationRequestV1<'_, K>,
) -> Option<&'request [u8]> {
    let binding = request.descriptor_binding();
    let start = usize::try_from(binding.entry_file_offset()).ok()?;
    let length = usize::try_from(binding.entry_size()).ok()?;
    request
        .finalized_hsaco_bytes()
        .get(start..start.checked_add(length)?)
}

fn refinement_request_binding<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    compiler: &super::WorkerV3CompilerExecutionVerificationV1,
    proof_owner: &ValidatedCompilerProofInputsV5,
    isa: &[u8],
) -> [u8; 32] {
    let subject = proof_owner.association().inputs().subject();
    let kir = proof_owner.capability().kernel_ir().identity();
    let publication = request.publication();
    let attempt = publication.attempt();
    let scope = publication.scope();
    let llvm = request.final_llvm_bytes();
    let artifact = request.finalized_hsaco_bytes();
    let mut digest = Sha256::new();
    digest.update(REFINEMENT_REQUEST_DOMAIN_V1);
    for identity in [
        *request.challenge_identity().as_bytes(),
        *request.lineage_identity().as_bytes(),
        request.marker_binding_identity(),
        request.generated_host_contract_identity(),
        production_worker_v3_subject_identity_v1(subject),
        *kir.digest(),
        Sha256::digest(llvm).into(),
        Sha256::digest(isa).into(),
        Sha256::digest(artifact).into(),
        compiler.subject_sha256(),
        compiler.carriage_sha256(),
        compiler.policy_sha256(),
        compiler.issuer_journal_sha256(),
        compiler.compiler_occurrence_sha256(),
        compiler.receipt_sha256(),
        compiler.publication_sha256(),
        compiler.acknowledgment_sha256(),
        compiler.worker_ledger_record_sha256(),
        compiler.current_record_verification_sha256(),
        compiler.current_record_attestation_sha256(),
        compiler.protected_policy_verification_sha256(),
        compiler.protected_worker_ledger_verification_sha256(),
        compiler.external_rollback_verification_sha256(),
        compiler.prior_rollback_anchor(),
        compiler.current_rollback_anchor(),
        *attempt.invocation().as_bytes(),
        *scope.package().as_bytes(),
        *scope.kernel_set().as_bytes(),
        *scope.target().as_bytes(),
        *publication.publication().as_bytes(),
    ] {
        digest.update(identity);
    }
    for length in [
        kir.canonical_length(),
        subject.executable_kir_epoch(),
        llvm.len() as u64,
        isa.len() as u64,
        artifact.len() as u64,
        compiler.sequence(),
        attempt.generation(),
    ] {
        digest.update(length.to_le_bytes());
    }
    digest.update(attempt.session().as_bytes());
    digest.finalize().into()
}

fn verification_transcript_identity(
    request: &[u8],
    verification: &[u8],
    attestation: &[u8],
    terminal: &[u8],
    service_transcript: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(VERIFICATION_TRANSCRIPT_DOMAIN_V1);
    for field in [request, verification, attestation, terminal] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.update(service_transcript);
    digest.finalize().into()
}

fn fresh_challenge() -> Result<[u8; 32], ProductionWorkerV3ProtectedVerifierErrorV1> {
    loop {
        let mut bytes = [0_u8; 32];
        let mut offset = 0;
        while offset < bytes.len() {
            // SAFETY: the remaining mutable slice is live for the complete syscall and getrandom
            // writes at most the supplied length.
            let count = unsafe {
                libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
            };
            if count < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
                    format!("getrandom failed: {error}"),
                ));
            }
            if count == 0 {
                return Err(ProductionWorkerV3ProtectedVerifierErrorV1::RequestProtocol(
                    "getrandom returned zero bytes".to_owned(),
                ));
            }
            offset += count as usize;
        }
        if bytes != [0; 32] {
            return Ok(bytes);
        }
    }
}

fn sealed_read_only_memfd(name: &str, bytes: &[u8]) -> io::Result<OwnedFd> {
    let descriptor =
        rustix::fs::memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
    let mut writer = File::from(descriptor);
    rustix::fs::fchmod(&writer, Mode::RUSR)?;
    writer.write_all(bytes)?;
    writer.flush()?;
    rustix::fs::fcntl_add_seals(&writer, REQUIRED_SEALS)?;
    let path = format!("/proc/self/fd/{}", writer.as_raw_fd());
    let retained = rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())?;
    drop(writer);
    Ok(retained)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SocketSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn inspect_fixed_service_socket(
    profile: &CompilerExecutionClientProfileV1,
) -> Result<SocketSnapshotV1, ProductionWorkerV3VerifierDeploymentErrorV1> {
    let directory =
        std::fs::symlink_metadata(PRODUCTION_RUNTIME_DIRECTORY_V1).map_err(|error| {
            ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
        })?;
    if !directory.file_type().is_dir()
        || directory.uid() != 0
        || directory.gid() != 0
        || directory.nlink() == 0
        || directory.mode() & 0o7777 != PRODUCTION_RUNTIME_DIRECTORY_MODE_V1
    {
        return Err(
            ProductionWorkerV3VerifierDeploymentErrorV1::InvalidRuntimeDirectory(
                "runtime directory is not exact root-owned mode 0755 custody",
            ),
        );
    }
    let metadata = std::fs::symlink_metadata(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1)
        .map_err(|error| {
            ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
        })?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != profile.supervisor_uid()
        || metadata.gid() != profile.supervisor_gid()
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != PRODUCTION_SOCKET_MODE_V1
    {
        return Err(
            ProductionWorkerV3VerifierDeploymentErrorV1::InvalidServiceSocket(
                "service socket type, owner, group, mode, or link count is invalid",
            ),
        );
    }
    Ok(SocketSnapshotV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        links: metadata.nlink(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

fn connect_fixed_service(
    deadline: Instant,
) -> Result<OwnedFd, ProductionWorkerV3VerifierDeploymentErrorV1> {
    let address =
        SocketAddrUnix::new(PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V1).map_err(|error| {
            ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
        })?;
    let peer = socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|error| {
        ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
    })?;
    loop {
        if deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                "service connect timed out".to_owned(),
            ));
        }
        match connect(&peer, &address) {
            Ok(()) | Err(rustix::io::Errno::ISCONN) => return Ok(peer),
            Err(rustix::io::Errno::INTR) => continue,
            Err(
                rustix::io::Errno::INPROGRESS
                | rustix::io::Errno::ALREADY
                | rustix::io::Errno::AGAIN,
            ) => {
                wait_for_connect(&peer, deadline)?;
                match rustix::net::sockopt::socket_error(&peer).map_err(|error| {
                    ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
                })? {
                    Ok(()) => return Ok(peer),
                    Err(error) => {
                        return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                            error.to_string(),
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                    error.to_string(),
                ));
            }
        }
    }
}

fn wait_for_connect(
    peer: &OwnedFd,
    deadline: Instant,
) -> Result<(), ProductionWorkerV3VerifierDeploymentErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                "service connect timed out".to_owned(),
            ));
        }
        let mut descriptor = libc::pollfd {
            fd: peer.as_fd().as_raw_fd(),
            events: libc::POLLOUT | libc::POLLERR | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: `descriptor` is a live one-element pollfd array for the complete call.
        let result = unsafe { libc::poll(&mut descriptor, 1, poll_millis(remaining)) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                error.to_string(),
            ));
        }
        if result == 0 {
            return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                "service connect timed out".to_owned(),
            ));
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(
                "service descriptor became invalid".to_owned(),
            ));
        }
        if descriptor.revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0 {
            return Ok(());
        }
    }
}

fn validate_service_credentials(
    peer: &OwnedFd,
    profile: &CompilerExecutionClientProfileV1,
) -> Result<(), ProductionWorkerV3VerifierDeploymentErrorV1> {
    let credentials = rustix::net::sockopt::socket_peercred(peer).map_err(|error| {
        ProductionWorkerV3VerifierDeploymentErrorV1::ServiceConnect(error.to_string())
    })?;
    if credentials.pid.as_raw_pid() <= 0
        || credentials.uid.as_raw() != profile.supervisor_uid()
        || credentials.gid.as_raw() != profile.supervisor_gid()
    {
        return Err(ProductionWorkerV3VerifierDeploymentErrorV1::ServiceCredentialsMismatch);
    }
    Ok(())
}

fn poll_millis(duration: Duration) -> i32 {
    let millis = duration.as_millis();
    let rounded = if duration.subsec_nanos().is_multiple_of(1_000_000) {
        millis
    } else {
        millis.saturating_add(1)
    };
    rounded.clamp(1, i32::MAX as u128) as i32
}

fn copy_vec(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len()).ok()?;
    copy.extend_from_slice(bytes);
    Some(copy)
}

fn copy_box(bytes: &[u8]) -> Option<Box<[u8]>> {
    Some(copy_vec(bytes)?.into_boxed_slice())
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn derive_identity_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    digest.finalize().into()
}

struct ResponseWriter<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}

impl<'a> ResponseWriter<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn put(&mut self, value: &[u8]) {
        let end = self.offset + value.len();
        self.bytes[self.offset..end].copy_from_slice(value);
        self.offset = end;
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

struct ResponseReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ResponseReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionWorkerV3VerificationServiceResponseErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(ProductionWorkerV3VerificationServiceResponseErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionWorkerV3VerificationServiceResponseErrorV1::Truncated)?
            .try_into()
            .expect("fixed response slice");
        self.offset = end;
        Ok(value)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

impl std::fmt::Display for ProductionWorkerV3VerificationServiceResponseErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid protected-verifier service response: {self:?}"
        )
    }
}

impl std::error::Error for ProductionWorkerV3VerificationServiceResponseErrorV1 {}

impl std::fmt::Display for ProductionWorkerV3VerifierDeploymentErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot acquire production Worker V3 verifier: {self:?}"
        )
    }
}

impl std::error::Error for ProductionWorkerV3VerifierDeploymentErrorV1 {}

impl std::fmt::Display for ProductionWorkerV3ProtectedVerifierErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "protected Worker V3 verification failed: {self:?}"
        )
    }
}

impl std::error::Error for ProductionWorkerV3ProtectedVerifierErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ServiceResponse(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Display for ProductionWorkerV3SemanticMachineRefinementErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "semantic-machine refinement failed: {self:?}")
    }
}

impl std::error::Error for ProductionWorkerV3SemanticMachineRefinementErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_device::KernelMarkerV1;
    use fe2o3_proof_contracts::{
        DigestV1, ExecutableKirIdentityV1, KernelIdentityV1, KernelRootIdentityV1,
        LaunchContractIdentityV1, TargetModelIdentityV1,
    };

    fn response() -> ProductionWorkerV3VerificationServiceResponseV1 {
        ProductionWorkerV3VerificationServiceResponseV1::new(
            [1; 32],
            [2; 32],
            [3; 32],
            4,
            [5; 32],
            6,
            0,
            [7; 32],
            [8; 32],
            [9; 32],
            10,
            [11; 32],
            [12; 32],
            [13; 32],
            WorkerV3SafetyPropertiesV1::required(),
            [14; 32],
            [15; 32],
        )
        .unwrap()
    }

    fn digest(byte: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([byte; 32])
    }

    fn subject() -> CapabilitySubjectV1 {
        CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(digest(1)),
            KernelRootIdentityV1::from_untrusted_digest(digest(2)),
            ExecutableKirIdentityV1::from_untrusted_digest(digest(3)),
            4,
            TargetModelIdentityV1::from_untrusted_digest(digest(5)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(6)),
        )
        .unwrap()
    }

    #[test]
    fn protected_response_round_trips_canonically_without_authority() {
        let response = response();
        let decoded = ProductionWorkerV3VerificationServiceResponseV1::decode_canonical(
            response.canonical_bytes(),
        )
        .unwrap();
        assert_eq!(decoded, response);
        assert!(!decoded.grants_authority());
    }

    #[test]
    fn protected_response_rejects_every_single_byte_mutation() {
        let response = response();
        for index in 0..response.canonical_bytes().len() {
            let mut hostile = *response.canonical_bytes();
            hostile[index] ^= 1;
            assert!(
                ProductionWorkerV3VerificationServiceResponseV1::decode_canonical(&hostile)
                    .is_err(),
                "mutated byte {index} was accepted"
            );
        }
    }

    #[test]
    fn protected_response_requires_complete_nonzero_evidence() {
        let mut arguments = (
            [1; 32],
            [2; 32],
            [3; 32],
            4,
            [5; 32],
            6,
            0,
            [7; 32],
            [8; 32],
            [9; 32],
            10,
            [11; 32],
            [12; 32],
            [13; 32],
            WorkerV3SafetyPropertiesV1::required(),
            [14; 32],
            [15; 32],
        );
        arguments.2 = [0; 32];
        assert!(matches!(
            ProductionWorkerV3VerificationServiceResponseV1::new(
                arguments.0,
                arguments.1,
                arguments.2,
                arguments.3,
                arguments.4,
                arguments.5,
                arguments.6,
                arguments.7,
                arguments.8,
                arguments.9,
                arguments.10,
                arguments.11,
                arguments.12,
                arguments.13,
                arguments.14,
                arguments.15,
                arguments.16,
            ),
            Err(ProductionWorkerV3VerificationServiceResponseErrorV1::ZeroIdentity(_))
        ));
    }

    #[test]
    fn subject_identity_covers_every_semantic_axis() {
        let baseline = subject();
        let baseline_identity = production_worker_v3_subject_identity_v1(baseline);
        let variants = [
            CapabilitySubjectV1::new(
                KernelIdentityV1::from_untrusted_digest(digest(9)),
                baseline.root(),
                baseline.executable_kir(),
                baseline.executable_kir_epoch(),
                baseline.target_model(),
                baseline.launch_contract(),
            )
            .unwrap(),
            CapabilitySubjectV1::new(
                baseline.kernel(),
                KernelRootIdentityV1::from_untrusted_digest(digest(9)),
                baseline.executable_kir(),
                baseline.executable_kir_epoch(),
                baseline.target_model(),
                baseline.launch_contract(),
            )
            .unwrap(),
            CapabilitySubjectV1::new(
                baseline.kernel(),
                baseline.root(),
                ExecutableKirIdentityV1::from_untrusted_digest(digest(9)),
                baseline.executable_kir_epoch(),
                baseline.target_model(),
                baseline.launch_contract(),
            )
            .unwrap(),
            CapabilitySubjectV1::new(
                baseline.kernel(),
                baseline.root(),
                baseline.executable_kir(),
                9,
                baseline.target_model(),
                baseline.launch_contract(),
            )
            .unwrap(),
            CapabilitySubjectV1::new(
                baseline.kernel(),
                baseline.root(),
                baseline.executable_kir(),
                baseline.executable_kir_epoch(),
                TargetModelIdentityV1::from_untrusted_digest(digest(9)),
                baseline.launch_contract(),
            )
            .unwrap(),
            CapabilitySubjectV1::new(
                baseline.kernel(),
                baseline.root(),
                baseline.executable_kir(),
                baseline.executable_kir_epoch(),
                baseline.target_model(),
                LaunchContractIdentityV1::from_untrusted_digest(digest(9)),
            )
            .unwrap(),
        ];
        for variant in variants {
            assert_ne!(
                production_worker_v3_subject_identity_v1(variant),
                baseline_identity
            );
        }
    }

    #[test]
    fn concrete_backends_implement_both_unsafe_contracts() {
        fn protected<
            K: CompilerGeneratedKernelExpectationV1,
            B: WorkerV3ProtectedVerifierBackendV1<K>,
        >() {
        }
        fn refinement<
            K: CompilerGeneratedKernelExpectationV1,
            B: WorkerV3SemanticMachineRefinementBackendV1<K>,
        >() {
        }
        struct Marker;
        fn marker_function() {}
        unsafe impl KernelMarkerV1 for Marker {
            type Function = fn();
            type Registration = ();

            const LOGICAL_NAME: &'static str = "marker";
            const EXPORT_NAME: &'static str = "marker";
            const FUNCTION: Self::Function = marker_function;
            const REGISTRATION: &'static Self::Registration = &();
        }
        unsafe impl CompilerGeneratedKernelExpectationV1 for Marker {
            const PROFILE: crate::CompilerGeneratedKernelProfileV1 =
                crate::CompilerGeneratedKernelProfileV1::new([2; 32]);
            const KERNEL_BINDING_ID_V1: [u8; 32] = [1; 32];
        }
        protected::<Marker, ProductionWorkerV3ProtectedVerifierBackendV1<Marker>>();
        refinement::<Marker, ProductionWorkerV3SemanticMachineRefinementBackendV1<Marker>>();
    }
}
