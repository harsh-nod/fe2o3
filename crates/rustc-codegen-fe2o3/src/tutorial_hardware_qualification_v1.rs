//! Strict, authority-free authentication of tutorial hardware observations.
//!
//! This module verifies the transport emitted by `scripts/tutorial_hardware_receipt.py`.
//! A successful value says only that one target-matched run and its cleanup were observed by a
//! policy-pinned attestor. It is not semantic proof and grants no compiler, publication, load, or
//! launch authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::authenticated_compiler_capability_evidence_identity_v5;
use fe2o3_compiler_ffi::InertProductionCapabilityResultV5;
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

const POLICY_SCHEMA: &str = "fe2o3-tutorial-hardware-attestation-policy-v1";
const CHALLENGE_SCHEMA: &str = "fe2o3-tutorial-hardware-receipt-challenge-v1";
const RUN_SCHEMA: &str = "fe2o3-tutorial-hardware-run-receipt-v1";
const CLEANUP_SCHEMA: &str = "fe2o3-tutorial-hardware-cleanup-receipt-v1";
const PRE_CLEANUP_SCHEMA: &str = "fe2o3-tutorial-hardware-pre-cleanup-capsule-v1";
const TRANSPORT_SCHEMA: &str = "fe2o3-tutorial-hardware-archive-v1";
const ISA_SCHEMA: &str = "fe2o3-tutorial-hardware-isa-observation-v1";
const RESOURCE_SCHEMA: &str = "fe2o3-tutorial-hardware-resource-observation-v1";
const RESULT_SCHEMA: &str = "fe2o3-tutorial-hardware-result-observation-v1";
const HARDWARE_SCHEMA: &str = "fe2o3-tutorial-hardware-qualification-evidence-v1";
const PRE_HARDWARE_RECORD_SCHEMA: &str = "fe2o3-tutorial-pre-hardware-record-v1";
const AUTHORITY: &str = "authenticated-observation-no-independent-authority";
const OPENSSL_PATH: &str = "/usr/bin/openssl";

const REQUEST_DOMAIN: &[u8] = b"fe2o3-tutorial-production-transaction-request-v1\0";
const PRE_HARDWARE_RECORD_DOMAIN: &[u8] = b"fe2o3-tutorial-pre-hardware-record-v1\0";
const RUN_DOMAIN: &[u8] = b"fe2o3-tutorial-hardware-run-receipt-v1\0";
const CLEANUP_DOMAIN: &[u8] = b"fe2o3-tutorial-hardware-cleanup-receipt-v1\0";
const TRANSPORT_DOMAIN: &[u8] = b"fe2o3-tutorial-hardware-archive-v1\0";
const COMMAND_DOMAIN: &[u8] = b"fe2o3-tutorial-semantic-command-v1\0";
const SIGNATURE_CONTEXT: &[u8] = b"fe2o3-tutorial-hardware-receipt-signature-v1\0";

const INDEX_NAME: &str = "index-v1.json";
const OBJECT_PREFIX: &str = "objects/sha256";
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_OBJECT_BYTES: usize = 512 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: usize = 1024 * 1024 * 1024;
const MAX_ARCHIVE_OBJECTS: usize = 32;

const RUN_COMPILER_OBSERVATIONS: &[(&str, &str)] = &[
    ("artifact", "artifact"),
    ("artifactInspection", "artifact-inspection"),
    ("compilerPolicy", "compiler-policy"),
    ("kir", "optimized-kir-v13"),
    ("llvm", "llvm-module"),
    ("numericalPolicy", "numerical-policy"),
    ("proof", "proof-evidence"),
    ("proofChecker", "proof-checker"),
    ("proofObligations", "proof-obligation-set"),
    ("source", "source-closure"),
    ("target", "target-identity"),
    ("targetDecision", "target-capability-decision"),
];

const VERIFIER_ONLY_COMPILER_OBJECTS: &[&str] =
    &["sealed-production-receipt", "simulation-bundle-v8"];

const HARDWARE_OBSERVATIONS: &[&str] = &["driver", "isa", "resource", "result", "runtime"];

const PRE_HARDWARE_EVIDENCE_KINDS: &[&str] = &[
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "simulator",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
];

const PENDING_EVIDENCE_KINDS: &[&str] = &[
    "driver-identity",
    "hardware",
    "negative-fixture-set",
    "runtime-identity",
];

/// Complete immutable inputs to one tutorial hardware-receipt verification.
pub struct TutorialHardwareQualificationReceiptInputV1<'a> {
    /// Exact canonical request JSON, including its single trailing newline.
    pub request: &'a [u8],
    /// Exact canonical pre-hardware record JSON, including its single trailing newline.
    pub pre_hardware_record: &'a [u8],
    /// Exact canonical trust-policy JSON, including its single trailing newline.
    pub trust_policy: &'a [u8],
    /// Exact canonical ZIP transport emitted after terminal cleanup.
    pub archive: &'a [u8],
}

/// Caller-owned replay state. Failed verification never mutates this ledger.
#[derive(Debug, Default)]
pub struct TutorialHardwareReceiptReplayLedgerV1 {
    seen_run_receipts: BTreeSet<[u8; 32]>,
}

impl TutorialHardwareReceiptReplayLedgerV1 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accepted_receipt_count(&self) -> usize {
        self.seen_run_receipts.len()
    }
}

/// Authenticated identities from one hardware observation and terminal cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedTutorialHardwareQualificationReceiptV1 {
    archive_sha256: [u8; 32],
    cleanup_receipt_sha256: [u8; 32],
    pre_hardware_record_sha256: [u8; 32],
    isa_observation_sha256: [u8; 32],
    resource_observation_sha256: [u8; 32],
    result_observation_sha256: [u8; 32],
    run_receipt_sha256: [u8; 32],
    transaction_sha256: [u8; 32],
    hardware_evidence: AuthenticatedArchiveObjectV1,
    driver_identity: AuthenticatedArchiveObjectV1,
    runtime_identity: AuthenticatedArchiveObjectV1,
    sealed_production_receipt: AuthenticatedArchiveObjectV1,
    simulation_bundle_v8: AuthenticatedArchiveObjectV1,
    fixture_id: String,
    target: String,
}

#[derive(Clone, Eq, PartialEq)]
struct AuthenticatedArchiveObjectV1 {
    sha256: [u8; 32],
    payload: Box<[u8]>,
}

impl fmt::Debug for AuthenticatedArchiveObjectV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedArchiveObjectV1")
            .field("sha256", &encode_hex(&self.sha256))
            .field("bytes", &self.payload.len())
            .finish()
    }
}

impl VerifiedTutorialHardwareQualificationReceiptV1 {
    pub const fn archive_sha256(&self) -> &[u8; 32] {
        &self.archive_sha256
    }

    pub const fn cleanup_receipt_sha256(&self) -> &[u8; 32] {
        &self.cleanup_receipt_sha256
    }

    pub const fn pre_hardware_record_sha256(&self) -> &[u8; 32] {
        &self.pre_hardware_record_sha256
    }

    pub const fn run_receipt_sha256(&self) -> &[u8; 32] {
        &self.run_receipt_sha256
    }

    pub const fn transaction_sha256(&self) -> &[u8; 32] {
        &self.transaction_sha256
    }

    pub const fn isa_observation_sha256(&self) -> &[u8; 32] {
        &self.isa_observation_sha256
    }

    pub const fn resource_observation_sha256(&self) -> &[u8; 32] {
        &self.resource_observation_sha256
    }

    pub const fn result_observation_sha256(&self) -> &[u8; 32] {
        &self.result_observation_sha256
    }

    pub const fn hardware_evidence_sha256(&self) -> &[u8; 32] {
        &self.hardware_evidence.sha256
    }

    pub fn hardware_evidence_payload(&self) -> &[u8] {
        &self.hardware_evidence.payload
    }

    pub const fn driver_identity_sha256(&self) -> &[u8; 32] {
        &self.driver_identity.sha256
    }

    pub fn driver_identity_payload(&self) -> &[u8] {
        &self.driver_identity.payload
    }

    pub const fn runtime_identity_sha256(&self) -> &[u8; 32] {
        &self.runtime_identity.sha256
    }

    pub fn runtime_identity_payload(&self) -> &[u8] {
        &self.runtime_identity.payload
    }

    pub const fn sealed_production_receipt_sha256(&self) -> &[u8; 32] {
        &self.sealed_production_receipt.sha256
    }

    pub fn sealed_production_receipt_payload(&self) -> &[u8] {
        &self.sealed_production_receipt.payload
    }

    pub const fn simulation_bundle_v8_sha256(&self) -> &[u8; 32] {
        &self.simulation_bundle_v8.sha256
    }

    pub fn simulation_bundle_v8_payload(&self) -> &[u8] {
        &self.simulation_bundle_v8.payload
    }

    pub fn fixture_id(&self) -> &str {
        &self.fixture_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TutorialHardwareQualificationErrorV1(String);

impl TutorialHardwareQualificationErrorV1 {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TutorialHardwareQualificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TutorialHardwareQualificationErrorV1 {}

type ResultV1<T> = Result<T, TutorialHardwareQualificationErrorV1>;

#[derive(Clone, Debug)]
struct LaneTrust {
    attestor_identity: String,
    public_key: Vec<u8>,
    public_key_sha256: [u8; 32],
    reservation_identity: String,
}

#[derive(Debug)]
struct StrictArchive {
    objects: BTreeMap<[u8; 32], Vec<u8>>,
    index: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectReference {
    bytes: usize,
    sha256: [u8; 32],
}

/// Authenticates one exact target-matched hardware archive without granting production authority.
pub fn verify_tutorial_hardware_qualification_receipt_v1(
    input: TutorialHardwareQualificationReceiptInputV1<'_>,
    replay: &mut TutorialHardwareReceiptReplayLedgerV1,
) -> Result<VerifiedTutorialHardwareQualificationReceiptV1, TutorialHardwareQualificationErrorV1> {
    let request = parse_canonical_document(input.request, "transaction request")?;
    let record = parse_canonical_document(input.pre_hardware_record, "pre-hardware record")?;
    let policy = parse_canonical_document(input.trust_policy, "hardware trust policy")?;
    let request_object = object(&request, "transaction request")?;
    let record_object = object(&record, "pre-hardware record")?;
    let pre_hardware_record_sha256 =
        domain_document_sha256(PRE_HARDWARE_RECORD_DOMAIN, input.pre_hardware_record);

    validate_request_binding(request_object)?;
    let trust = load_trust_policy(&policy)?;
    let archive = decode_canonical_archive(input.archive)?;
    let index = object(&archive.index, "hardware archive index")?;
    exact_keys(
        index,
        &[
            "candidate",
            "challengeNonce",
            "cleanupReceipt",
            "cleanupSignature",
            "fixtureId",
            "lane",
            "preHardwareRecordSha256",
            "preCleanupCapsuleSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "runReceipt",
            "runSignature",
            "schema",
            "target",
            "transactionSha256",
            "transportBindingSha256",
        ],
        "hardware archive index",
    )?;
    validate_binding(index, "transportBindingSha256", TRANSPORT_DOMAIN, "archive")?;

    let fixture = object(
        required(request_object, "fixture", "request")?,
        "request.fixture",
    )?;
    let challenge = object(
        required(request_object, "hardwareReceiptChallenge", "request")?,
        "request.hardwareReceiptChallenge",
    )?;
    exact_keys(
        challenge,
        &["nonce", "reservationIdentity", "schema", "transportSchema"],
        "hardware receipt challenge",
    )?;
    digest_field(challenge, "nonce", "challenge")?;
    let lane = identity_field(fixture, "hardwareLane", "request.fixture")?;
    let target = target_field(fixture, "target", "request.fixture")?;
    let reservation = identity_field(fixture, "hardwareReservation", "request.fixture")?;
    let fixture_id = identity_field(fixture, "fixtureId", "request.fixture")?;
    if string_field(challenge, "schema", "challenge")? != CHALLENGE_SCHEMA
        || string_field(challenge, "transportSchema", "challenge")? != TRANSPORT_SCHEMA
        || string_field(challenge, "reservationIdentity", "challenge")? != reservation
    {
        return fail("hardware receipt challenge differs from the requested reservation");
    }
    let lane_trust = trust
        .get(&(lane.to_owned(), target.to_owned()))
        .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("lane/target is not trusted"))?;
    if lane_trust.reservation_identity != reservation {
        return fail("hardware reservation is not trusted for the lane/target");
    }

    let transaction = object(
        required(
            record_object,
            "productionTransaction",
            "pre-hardware record",
        )?,
        "pre-hardware record.productionTransaction",
    )?;
    exact_keys(
        transaction,
        &[
            "allowsFallback",
            "allowsPipelineSelection",
            "pipelineEntry",
            "policyVersion",
            "status",
            "transactionSha256",
        ],
        "production transaction",
    )?;
    if bool_field(transaction, "allowsFallback", "production transaction")?
        || bool_field(
            transaction,
            "allowsPipelineSelection",
            "production transaction",
        )?
        || string_field(transaction, "pipelineEntry", "production transaction")?
            != "rustc-codegen-fe2o3::production_pipeline"
        || u64_field(transaction, "policyVersion", "production transaction")? != 4
        || string_field(transaction, "status", "production transaction")?
            != "sealed-production-complete"
    {
        return fail("record did not use the sole sealed production transaction");
    }
    let transaction_sha = digest_field(transaction, "transactionSha256", "transaction")?;
    let pre_hardware_record_identity = Value::String(encode_hex(&pre_hardware_record_sha256));
    let expected_index: [(&str, &Value); 10] = [
        (
            "candidate",
            required(request_object, "candidate", "request")?,
        ),
        (
            "challengeNonce",
            challenge.get("nonce").expect("checked field"),
        ),
        (
            "fixtureId",
            fixture.get("fixtureId").expect("checked field"),
        ),
        ("lane", fixture.get("hardwareLane").expect("checked field")),
        ("preHardwareRecordSha256", &pre_hardware_record_identity),
        (
            "requestBindingSha256",
            required(request_object, "requestBindingSha256", "request")?,
        ),
        (
            "reservationIdentity",
            fixture.get("hardwareReservation").expect("checked field"),
        ),
        ("schema", &Value::String(TRANSPORT_SCHEMA.to_owned())),
        ("target", fixture.get("target").expect("checked field")),
        (
            "transactionSha256",
            transaction
                .get("transactionSha256")
                .expect("checked transaction field"),
        ),
    ];
    for (field, expected) in expected_index {
        if index.get(field) != Some(expected) {
            return fail(format!("archive {field} is replayed or cross-transaction"));
        }
    }

    let evidence_files = object(
        required(record_object, "evidenceFiles", "pre-hardware record")?,
        "pre-hardware record.evidenceFiles",
    )?;
    validate_pre_hardware_record(
        record_object,
        request_object,
        evidence_files,
        &archive.objects,
    )?;
    let production = object(
        required(record_object, "productionEvidence", "pre-hardware record")?,
        "pre-hardware record.productionEvidence",
    )?;
    let typed = verify_compiler_transaction(
        evidence_files,
        production,
        &archive.objects,
        transaction_sha,
        target,
        string_field(record_object, "kernelSymbol", "pre-hardware record")?,
    )?;

    let run_ref = reference_field(index, "runReceipt", "archive index")?;
    let run_payload = archive_object(&archive.objects, &run_ref, "run receipt")?;
    let run = parse_canonical_document(run_payload, "hardware run receipt")?;
    let run_object = object(&run, "hardware run receipt")?;
    validate_run_receipt(
        run_object,
        request_object,
        record_object,
        evidence_files,
        &archive.objects,
        lane_trust,
        typed,
        pre_hardware_record_sha256,
    )?;
    let run_signature = reference_field(index, "runSignature", "archive index")?;
    verify_signature(
        run_payload,
        archive_object(&archive.objects, &run_signature, "run signature")?,
        lane_trust,
    )?;

    let cleanup_ref = reference_field(index, "cleanupReceipt", "archive index")?;
    let cleanup_payload = archive_object(&archive.objects, &cleanup_ref, "cleanup receipt")?;
    let cleanup = parse_canonical_document(cleanup_payload, "hardware cleanup receipt")?;
    let cleanup_object = object(&cleanup, "hardware cleanup receipt")?;
    let observed = validate_cleanup_receipt(
        cleanup_object,
        index,
        run_object,
        request_object,
        record_object,
        &archive.objects,
        lane_trust,
        pre_hardware_record_sha256,
    )?;
    let cleanup_signature = reference_field(index, "cleanupSignature", "archive index")?;
    verify_signature(
        cleanup_payload,
        archive_object(&archive.objects, &cleanup_signature, "cleanup signature")?,
        lane_trust,
    )?;

    let referenced = referenced_archive_objects(index, run_object, cleanup_object, evidence_files)?;
    if referenced != archive.objects.keys().copied().collect() {
        return fail("hardware archive contains omitted or unreferenced objects");
    }
    let hardware_evidence = authenticated_archive_object(
        &archive.objects,
        &observed.hardware,
        "hardware summary evidence",
    )?;
    let driver_identity = authenticated_archive_object(
        &archive.objects,
        &observed.driver,
        "driver identity observation",
    )?;
    let runtime_identity = authenticated_archive_object(
        &archive.objects,
        &observed.runtime,
        "runtime identity observation",
    )?;
    let sealed_production_receipt = authenticated_archive_object(
        &archive.objects,
        &reference_field(
            evidence_files,
            "sealed-production-receipt",
            "pre-hardware record.evidenceFiles",
        )?,
        "sealed V5 production result",
    )?;
    let simulation_bundle_v8 = authenticated_archive_object(
        &archive.objects,
        &reference_field(
            evidence_files,
            "simulation-bundle-v8",
            "pre-hardware record.evidenceFiles",
        )?,
        "simulation Bundle V8",
    )?;
    if replay.seen_run_receipts.contains(&run_ref.sha256) {
        return fail("hardware run receipt replay detected");
    }
    replay.seen_run_receipts.insert(run_ref.sha256);

    Ok(VerifiedTutorialHardwareQualificationReceiptV1 {
        archive_sha256: sha256(input.archive),
        cleanup_receipt_sha256: cleanup_ref.sha256,
        pre_hardware_record_sha256,
        isa_observation_sha256: observed.isa.sha256,
        resource_observation_sha256: observed.resource.sha256,
        result_observation_sha256: observed.result.sha256,
        run_receipt_sha256: run_ref.sha256,
        transaction_sha256: transaction_sha,
        hardware_evidence,
        driver_identity,
        runtime_identity,
        sealed_production_receipt,
        simulation_bundle_v8,
        fixture_id: fixture_id.to_owned(),
        target: target.to_owned(),
    })
}

fn validate_pre_hardware_record(
    record: &Map<String, Value>,
    request: &Map<String, Value>,
    files: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
) -> ResultV1<()> {
    exact_keys(
        record,
        &[
            "capabilityClosure",
            "compilerInput",
            "evidenceFiles",
            "fixtureId",
            "graph",
            "hardware",
            "kernelSymbol",
            "lessonIds",
            "pendingEvidence",
            "preHardwareBindingSha256",
            "productionEvidence",
            "productionTransaction",
            "proof",
            "schema",
            "simulator",
            "target",
            "targetDecision",
        ],
        "pre-hardware record",
    )?;
    if string_field(record, "schema", "pre-hardware record")? != PRE_HARDWARE_RECORD_SCHEMA {
        return fail("pre-hardware record schema differs");
    }
    validate_binding(
        record,
        "preHardwareBindingSha256",
        PRE_HARDWARE_RECORD_DOMAIN,
        "pre-hardware record",
    )?;
    exact_keys(
        files,
        PRE_HARDWARE_EVIDENCE_KINDS,
        "pre-hardware record.evidenceFiles",
    )?;
    for kind in PRE_HARDWARE_EVIDENCE_KINDS {
        reference_field(files, kind, "pre-hardware record.evidenceFiles")?;
    }
    let expected_pending = Value::Array(
        PENDING_EVIDENCE_KINDS
            .iter()
            .map(|kind| Value::String((*kind).to_owned()))
            .collect(),
    );
    if required(record, "pendingEvidence", "pre-hardware record")? != &expected_pending {
        return fail("pre-hardware pending-evidence roster differs");
    }

    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let kernel = object(
        required(request, "capabilityKernel", "request")?,
        "request.capabilityKernel",
    )?;
    if record.get("fixtureId") != fixture.get("fixtureId")
        || record.get("target") != fixture.get("target")
        || record.get("kernelSymbol") != kernel.get("kernelSymbol")
        || record.get("lessonIds") != kernel.get("lessonIds")
    {
        return fail("pre-hardware record is cross-fixture, cross-kernel, or cross-target");
    }
    let record_input = object(
        required(record, "compilerInput", "pre-hardware record")?,
        "pre-hardware record.compilerInput",
    )?;
    exact_keys(
        record_input,
        &[
            "cargoLockSha256",
            "contractSha256",
            "packageManifestSha256",
            "sourceClosureSha256",
        ],
        "pre-hardware record.compilerInput",
    )?;
    let request_input = object(
        required(fixture, "compilerInput", "fixture")?,
        "fixture.compilerInput",
    )?;
    for field in record_input.keys() {
        if record_input.get(field) != request_input.get(field) {
            return fail(format!(
                "pre-hardware record compiler input {field} is substituted"
            ));
        }
    }

    let production = object(
        required(record, "productionEvidence", "pre-hardware record")?,
        "pre-hardware record.productionEvidence",
    )?;
    exact_keys(
        production,
        &[
            "artifactInspectionSha256",
            "artifactSha256",
            "capabilityAnalysisSha256",
            "capabilityClosureSha256",
            "compilerPolicySha256",
            "finalOptimizedKirSha256",
            "launchContractSha256",
            "loweringIdentitySha256",
            "machineRefinementSha256",
            "numericalPolicyEvidenceSha256",
            "proofCheckerSha256",
            "proofEvidenceSha256",
            "proofObligationSetSha256",
            "sealedResultSha256",
            "sourceMirIdentitySha256",
            "sourceMirToKirRefinementSha256",
            "targetIdentitySha256",
        ],
        "pre-hardware record.productionEvidence",
    )?;
    for field in production.keys() {
        digest_field(production, field, "pre-hardware production evidence")?;
    }
    const JOINS: &[(&str, &str)] = &[
        ("artifactSha256", "artifact"),
        ("artifactInspectionSha256", "artifact-inspection"),
        ("capabilityAnalysisSha256", "capability-analysis"),
        ("numericalPolicyEvidenceSha256", "numerical-policy"),
    ];
    for (claim, kind) in JOINS {
        if digest_field(production, claim, "pre-hardware production evidence")?
            != reference_field(files, kind, "pre-hardware record.evidenceFiles")?.sha256
        {
            return fail(format!(
                "pre-hardware production evidence {claim} differs from {kind}"
            ));
        }
    }
    if files.get("simulator") != request.get("simulatorEvidence") {
        return fail("pre-hardware record substituted request-bound simulator evidence");
    }
    for (_, kind) in RUN_COMPILER_OBSERVATIONS {
        let reference = reference_field(files, kind, "pre-hardware record.evidenceFiles")?;
        archive_object(objects, &reference, kind)?;
    }
    for kind in VERIFIER_ONLY_COMPILER_OBJECTS {
        let reference = reference_field(files, kind, "pre-hardware record.evidenceFiles")?;
        archive_object(objects, &reference, kind)?;
    }

    let hardware = object(
        required(record, "hardware", "pre-hardware record")?,
        "pre-hardware record.hardware",
    )?;
    exact_keys(
        hardware,
        &[
            "commandSha256",
            "lane",
            "status",
            "target",
            "timeoutSeconds",
        ],
        "pre-hardware record.hardware",
    )?;
    let command = object(
        required(fixture, "hardwareCommand", "request.fixture")?,
        "request.fixture.hardwareCommand",
    )?;
    if digest_field(hardware, "commandSha256", "pre-hardware hardware")?
        != domain_sha256(COMMAND_DOMAIN, &Value::Object(command.clone()))?
        || hardware.get("lane") != fixture.get("hardwareLane")
        || hardware.get("target") != fixture.get("target")
        || hardware.get("timeoutSeconds") != command.get("timeoutSeconds")
        || string_field(hardware, "status", "pre-hardware hardware")?
            != "pending-authenticated-observation"
    {
        return fail("pre-hardware hardware request is stale or predicts an observation");
    }

    validate_pre_hardware_semantic_summaries(record, request, production)?;
    Ok(())
}

fn validate_pre_hardware_semantic_summaries(
    record: &Map<String, Value>,
    request: &Map<String, Value>,
    production: &Map<String, Value>,
) -> ResultV1<()> {
    let kernel = object(
        required(request, "capabilityKernel", "request")?,
        "request.capabilityKernel",
    )?;
    let closure = object(
        required(record, "capabilityClosure", "pre-hardware record")?,
        "pre-hardware record.capabilityClosure",
    )?;
    exact_keys(
        closure,
        &["requirements", "sha256", "status"],
        "pre-hardware record.capabilityClosure",
    )?;
    let requested_closure = object(
        required(kernel, "capabilityClosure", "request.capabilityKernel")?,
        "request.capabilityKernel.capabilityClosure",
    )?;
    if closure.get("requirements") != requested_closure.get("requirements")
        || closure.get("sha256") != production.get("capabilityClosureSha256")
        || string_field(closure, "status", "pre-hardware capability closure")?
            != "compiler-complete"
    {
        return fail("pre-hardware capability closure is stale or substituted");
    }

    let graph = object(
        required(record, "graph", "pre-hardware record")?,
        "pre-hardware record.graph",
    )?;
    exact_keys(
        graph,
        &[
            "bundleContentIdentitySha256",
            "finalGraphEpoch",
            "kernelCount",
            "productionKirIdentitySha256",
            "semanticMirIdentitySha256",
        ],
        "pre-hardware record.graph",
    )?;
    for field in [
        "bundleContentIdentitySha256",
        "productionKirIdentitySha256",
        "semanticMirIdentitySha256",
    ] {
        digest_field(graph, field, "pre-hardware graph")?;
    }
    u64_field(graph, "finalGraphEpoch", "pre-hardware graph")?;
    if u64_field(graph, "kernelCount", "pre-hardware graph")? == 0
        || graph.get("productionKirIdentitySha256") != production.get("finalOptimizedKirSha256")
        || graph.get("semanticMirIdentitySha256") != production.get("sourceMirIdentitySha256")
    {
        return fail("pre-hardware graph summary differs from production evidence");
    }

    let proof = object(
        required(record, "proof", "pre-hardware record")?,
        "pre-hardware record.proof",
    )?;
    exact_keys(
        proof,
        &[
            "checkerSha256",
            "evidenceSha256",
            "obligationSetSha256",
            "properties",
            "status",
        ],
        "pre-hardware record.proof",
    )?;
    if proof.get("checkerSha256") != production.get("proofCheckerSha256")
        || proof.get("evidenceSha256") != production.get("proofEvidenceSha256")
        || proof.get("obligationSetSha256") != production.get("proofObligationSetSha256")
        || proof.get("properties") != kernel.get("requiredProperties")
        || string_field(proof, "status", "pre-hardware proof")? != "compiler-complete"
    {
        return fail("pre-hardware proof summary differs from compiler evidence");
    }

    let simulator = object(
        required(record, "simulator", "pre-hardware record")?,
        "pre-hardware record.simulator",
    )?;
    exact_keys(
        simulator,
        &["commandSha256", "evidenceSha256", "status", "subjectSha256"],
        "pre-hardware record.simulator",
    )?;
    digest_field(simulator, "commandSha256", "pre-hardware simulator")?;
    if simulator.get("evidenceSha256")
        != object(
            required(request, "simulatorEvidence", "request")?,
            "request.simulatorEvidence",
        )?
        .get("sha256")
        || simulator.get("subjectSha256") != production.get("finalOptimizedKirSha256")
        || string_field(simulator, "status", "pre-hardware simulator")?
            != "request-bound-external-observation"
    {
        return fail("pre-hardware simulator summary is stale or substituted");
    }

    let target = object(
        required(record, "targetDecision", "pre-hardware record")?,
        "pre-hardware record.targetDecision",
    )?;
    exact_keys(
        target,
        &["capabilityClosureSha256", "status", "targetIdentitySha256"],
        "pre-hardware record.targetDecision",
    )?;
    if target.get("capabilityClosureSha256") != production.get("capabilityClosureSha256")
        || target.get("targetIdentitySha256") != production.get("targetIdentitySha256")
        || string_field(target, "status", "pre-hardware target decision")? != "compiler-complete"
    {
        return fail("pre-hardware target decision differs from compiler evidence");
    }
    Ok(())
}

struct TypedCompilerTransaction {
    result: InertProductionCapabilityResultV5,
    bundle: VerifiedSimulationBundleV8,
    kir: VerifiedCanonicalKernelIrV13,
}

fn verify_compiler_transaction(
    files: &Map<String, Value>,
    production: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    transaction_sha: [u8; 32],
    target: &str,
    kernel_symbol: &str,
) -> ResultV1<TypedCompilerTransaction> {
    let sealed_ref = reference_field(files, "sealed-production-receipt", "evidence files")?;
    let sealed = archive_object(objects, &sealed_ref, "sealed V5 production result")?;
    let result = InertProductionCapabilityResultV5::decode(sealed).map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!(
            "sealed V5 production result failed typed decoding: {error}"
        ))
    })?;
    if result.canonical_bytes() != sealed {
        return fail("sealed V5 production result is noncanonical");
    }
    if result.transaction().identity().sha256() != transaction_sha {
        return fail("production transaction identity does not name the exact V5+V8 payload");
    }

    let bundle_ref = reference_field(files, "simulation-bundle-v8", "evidence files")?;
    let bundle_bytes = archive_object(objects, &bundle_ref, "simulation Bundle V8")?;
    if result.simulation_bundle().canonical_bytes() != bundle_bytes {
        return fail("Bundle V8 differs from the transaction-retained bundle");
    }
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(bundle_bytes.to_vec()).map_err(
        |error| {
            TutorialHardwareQualificationErrorV1::new(format!(
                "simulation Bundle V8 failed typed decoding: {error}"
            ))
        },
    )?;
    bundle.revalidate().map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("Bundle V8 revalidation failed: {error}"))
    })?;
    if bundle.target() != target {
        return fail("Bundle V8 target differs from the requested hardware target");
    }

    let kir_ref = reference_field(files, "optimized-kir-v13", "evidence files")?;
    let kir_bytes = archive_object(objects, &kir_ref, "optimized KIR V13")?;
    if result.handoff().executable_kir().canonical_preimage() != kir_bytes
        || bundle.canonical_kir_v13() != kir_bytes
    {
        return fail("optimized KIR V13 differs across V5 and Bundle V8 custody");
    }
    let (kir, module) =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(kir_bytes.to_vec())
            .map_err(|error| {
                TutorialHardwareQualificationErrorV1::new(format!(
                    "optimized KIR V13 failed typed decoding: {error}"
                ))
            })?;
    kir.revalidate().map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("KIR V13 revalidation failed: {error}"))
    })?;
    if result.handoff().final_graph_report().final_graph() != *kir.identity().digest()
        || result.handoff().final_graph_report().final_epoch() != bundle.final_graph_epoch()
        || digest_field(production, "finalOptimizedKirSha256", "production evidence")?
            != *kir.identity().digest()
    {
        return fail("final optimized graph differs across V5, KIR V13, and Bundle V8");
    }
    let kernel_ordinal = module
        .kernels
        .iter()
        .position(|kernel| {
            kernel.id.as_str() == kernel_symbol || kernel.entry.as_str() == kernel_symbol
        })
        .ok_or_else(|| {
            TutorialHardwareQualificationErrorV1::new(
                "requested kernel symbol is absent from exact KIR V13",
            )
        })?;
    let handoff = result.handoff();
    if handoff.obligation_roster().len() != module.kernels.len()
        || result.capability_associations().entries().len() != module.kernels.len()
    {
        return fail("KIR, obligation, and capability-result rosters differ");
    }
    let association = &result.capability_associations().entries()[kernel_ordinal];
    let subject = handoff.subjects()[kernel_ordinal];
    let checker_reference = reference_field(files, "proof-checker", "evidence files")?;
    let checker_identity = authenticated_compiler_capability_evidence_identity_v5(archive_object(
        objects,
        &checker_reference,
        "proof checker evidence",
    )?)
    .map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!(
            "proof checker evidence identity derivation failed: {error}"
        ))
    })?;
    let typed_identities = [
        ("sealedResultSha256", result.identity().sha256()),
        ("compilerPolicySha256", handoff.inputs().compiler_policy()),
        (
            "capabilityClosureSha256",
            handoff.target_closure().closure_identity(),
        ),
        (
            "launchContractSha256",
            *subject.launch_contract().digest().as_bytes(),
        ),
        ("sourceMirIdentitySha256", bundle.semantic_mir_identity()),
        (
            "targetIdentitySha256",
            *subject.target_model().digest().as_bytes(),
        ),
        (
            "loweringIdentitySha256",
            *handoff
                .legacy_handoff()
                .capsule()
                .receipts()
                .amdgpu_lowering()
                .identity()
                .sha256(),
        ),
        (
            "sourceMirToKirRefinementSha256",
            handoff.source_refinement().identity().sha256(),
        ),
        (
            "machineRefinementSha256",
            result.machine_refinement().identity().sha256(),
        ),
        (
            "proofObligationSetSha256",
            *handoff.obligation_roster()[kernel_ordinal]
                .identity()
                .digest()
                .as_bytes(),
        ),
        (
            "proofEvidenceSha256",
            *association.result_set_identity().digest().as_bytes(),
        ),
        ("proofCheckerSha256", checker_identity.sha256()),
    ];
    for (claim, expected) in typed_identities {
        if digest_field(production, claim, "production evidence")? != expected {
            return fail(format!(
                "production typed identity {claim} differs from its canonical receipt"
            ));
        }
    }
    Ok(TypedCompilerTransaction {
        result,
        bundle,
        kir,
    })
}

fn validate_run_receipt(
    run: &Map<String, Value>,
    request: &Map<String, Value>,
    record: &Map<String, Value>,
    files: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    trust: &LaneTrust,
    typed: TypedCompilerTransaction,
    pre_hardware_record_sha256: [u8; 32],
) -> ResultV1<()> {
    exact_keys(
        run,
        &[
            "artifactInspectionSha256",
            "artifactSha256",
            "attestor",
            "authority",
            "candidate",
            "challengeNonce",
            "commandSha256",
            "fixtureId",
            "kernelSymbols",
            "lane",
            "launchContractSha256",
            "observations",
            "outcome",
            "phase",
            "preHardwareRecordSha256",
            "receiptBindingSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "schema",
            "scratchIdentitySha256",
            "sequence",
            "target",
            "targetIdentitySha256",
            "transactionSha256",
        ],
        "hardware run receipt",
    )?;
    validate_binding(run, "receiptBindingSha256", RUN_DOMAIN, "run receipt")?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let challenge = object(
        required(request, "hardwareReceiptChallenge", "request")?,
        "request.hardwareReceiptChallenge",
    )?;
    let transaction = object(
        required(record, "productionTransaction", "record")?,
        "record.productionTransaction",
    )?;
    let production = object(
        required(record, "productionEvidence", "record")?,
        "record.productionEvidence",
    )?;
    let attestor = object(required(run, "attestor", "run")?, "run.attestor")?;
    exact_keys(attestor, &["identity", "publicKeySha256"], "run.attestor")?;
    if string_field(attestor, "identity", "run.attestor")? != trust.attestor_identity
        || digest_field(attestor, "publicKeySha256", "run.attestor")? != trust.public_key_sha256
    {
        return fail("run receipt names an untrusted attestor");
    }

    let pre_hardware_record_identity = Value::String(encode_hex(&pre_hardware_record_sha256));
    let expected: [(&str, &Value); 10] = [
        ("candidate", required(request, "candidate", "request")?),
        ("challengeNonce", required(challenge, "nonce", "challenge")?),
        ("fixtureId", required(fixture, "fixtureId", "fixture")?),
        ("lane", required(fixture, "hardwareLane", "fixture")?),
        ("preHardwareRecordSha256", &pre_hardware_record_identity),
        (
            "requestBindingSha256",
            required(request, "requestBindingSha256", "request")?,
        ),
        (
            "reservationIdentity",
            required(fixture, "hardwareReservation", "fixture")?,
        ),
        ("target", required(fixture, "target", "fixture")?),
        (
            "transactionSha256",
            required(transaction, "transactionSha256", "transaction")?,
        ),
        (
            "targetIdentitySha256",
            required(production, "targetIdentitySha256", "production evidence")?,
        ),
    ];
    for (field, value) in expected {
        if run.get(field) != Some(value) {
            return fail(format!("run receipt {field} is stale or substituted"));
        }
    }
    if string_field(run, "schema", "run")? != RUN_SCHEMA
        || string_field(run, "authority", "run")? != AUTHORITY
        || string_field(run, "outcome", "run")? != "passed"
        || string_field(run, "phase", "run")? != "pre-cleanup"
        || u64_field(run, "sequence", "run")? != 1
    {
        return fail("run receipt phase, outcome, or authority marker differs");
    }
    digest_field(run, "scratchIdentitySha256", "run")?;

    let command = required(fixture, "hardwareCommand", "fixture")?;
    if digest_field(run, "commandSha256", "run")? != domain_sha256(COMMAND_DOMAIN, command)? {
        return fail("run receipt command identity differs from the requested command");
    }
    let symbols = sorted_unique_strings(
        required(
            object(
                required(fixture, "compilerInput", "fixture")?,
                "compiler input",
            )?,
            "kernelSymbols",
            "compiler input",
        )?,
        "compiler input.kernelSymbols",
    )?;
    if run.get("kernelSymbols")
        != Some(&Value::Array(
            symbols
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        ))
    {
        return fail("run receipt kernel-symbol roster differs from the compiler input");
    }
    if typed.bundle.kernel_count() as usize != symbols.len()
        || typed.result.handoff().subjects().len() != symbols.len()
        || typed.result.handoff().obligation_roster().len() != symbols.len()
        || *typed.kir.identity().digest() != typed.bundle.production_kir_identity().digest()
    {
        return fail("V5, Bundle V8, KIR, and kernel-symbol rosters differ");
    }

    let observations = object(required(run, "observations", "run")?, "run.observations")?;
    let mut observation_keys = RUN_COMPILER_OBSERVATIONS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    observation_keys.extend(HARDWARE_OBSERVATIONS.iter().copied());
    exact_keys(observations, &observation_keys, "run.observations")?;
    for (name, kind) in RUN_COMPILER_OBSERVATIONS {
        let observed = reference_field(observations, name, "run.observations")?;
        let compiler = reference_field(files, kind, "pre-hardware record.evidenceFiles")?;
        if observed != compiler || archive_object(objects, &observed, name)?.is_empty() {
            return fail(format!(
                "run observation {name} substituted compiler evidence {kind}"
            ));
        }
    }
    let hardware_digests = HARDWARE_OBSERVATIONS
        .iter()
        .copied()
        .map(|name| {
            reference_field(observations, name, "run.observations").map(|value| value.sha256)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if hardware_digests.len() != HARDWARE_OBSERVATIONS.len() {
        return fail("hardware receipt conflates distinct hardware-owned observations");
    }
    let artifact = reference_field(observations, "artifact", "run.observations")?;
    let artifact_inspection =
        reference_field(observations, "artifactInspection", "run.observations")?;
    if digest_field(run, "artifactSha256", "run")? != artifact.sha256
        || digest_field(run, "artifactInspectionSha256", "run")? != artifact_inspection.sha256
        || digest_field(production, "artifactSha256", "production evidence")? != artifact.sha256
        || digest_field(
            production,
            "artifactInspectionSha256",
            "production evidence",
        )? != artifact_inspection.sha256
        || digest_field(run, "launchContractSha256", "run")?
            != digest_field(production, "launchContractSha256", "production evidence")?
    {
        return fail("run receipt artifact, inspection, or launch contract is substituted");
    }

    validate_isa_observation(observations, objects, fixture)?;
    validate_resource_observation(observations, objects, fixture)?;
    validate_result_observation(observations, objects, request, record)?;
    Ok(())
}

fn validate_isa_observation(
    observations: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    fixture: &Map<String, Value>,
) -> ResultV1<()> {
    let isa_ref = reference_field(observations, "isa", "run.observations")?;
    let isa = parse_canonical_document(
        archive_object(objects, &isa_ref, "ISA observation")?,
        "ISA observation",
    )?;
    let isa = object(&isa, "ISA observation")?;
    exact_keys(
        isa,
        &[
            "artifactSha256",
            "disassembly",
            "inspectionToolSha256",
            "kernelSymbols",
            "llvmModuleSha256",
            "schema",
            "target",
        ],
        "ISA observation",
    )?;
    let compiler_input = object(
        required(fixture, "compilerInput", "fixture")?,
        "compiler input",
    )?;
    if string_field(isa, "schema", "ISA observation")? != ISA_SCHEMA
        || isa.get("target") != fixture.get("target")
        || isa.get("kernelSymbols") != compiler_input.get("kernelSymbols")
        || digest_field(isa, "artifactSha256", "ISA observation")?
            != reference_field(observations, "artifact", "run.observations")?.sha256
        || digest_field(isa, "llvmModuleSha256", "ISA observation")?
            != reference_field(observations, "llvm", "run.observations")?.sha256
        || string_field(isa, "disassembly", "ISA observation")?.is_empty()
    {
        return fail("ISA observation is incomplete, stale, or cross-target");
    }
    digest_field(isa, "inspectionToolSha256", "ISA observation")?;
    Ok(())
}

fn validate_resource_observation(
    observations: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    fixture: &Map<String, Value>,
) -> ResultV1<()> {
    let resource_ref = reference_field(observations, "resource", "run.observations")?;
    let resource = parse_canonical_document(
        archive_object(objects, &resource_ref, "resource observation")?,
        "resource observation",
    )?;
    let resource = object(&resource, "resource observation")?;
    exact_keys(
        resource,
        &[
            "artifactSha256",
            "inspectionToolSha256",
            "kernelSymbols",
            "ldsBytes",
            "registersPerWorkgroup",
            "schema",
            "scratchBytes",
            "target",
            "workgroupSize",
        ],
        "resource observation",
    )?;
    let compiler_input = object(
        required(fixture, "compilerInput", "fixture")?,
        "compiler input",
    )?;
    let dimensions = array(
        required(resource, "workgroupSize", "resource observation")?,
        "resource observation.workgroupSize",
    )?;
    if string_field(resource, "schema", "resource observation")? != RESOURCE_SCHEMA
        || resource.get("target") != fixture.get("target")
        || resource.get("kernelSymbols") != compiler_input.get("kernelSymbols")
        || digest_field(resource, "artifactSha256", "resource observation")?
            != reference_field(observations, "artifact", "run.observations")?.sha256
        || dimensions.len() != 3
        || dimensions
            .iter()
            .any(|value| value.as_u64().is_none_or(|value| value == 0))
    {
        return fail("resource observation is incomplete, stale, or cross-target");
    }
    for field in ["ldsBytes", "registersPerWorkgroup", "scratchBytes"] {
        u64_field(resource, field, "resource observation")?;
    }
    digest_field(resource, "inspectionToolSha256", "resource observation")?;
    Ok(())
}

fn validate_result_observation(
    observations: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    request: &Map<String, Value>,
    record: &Map<String, Value>,
) -> ResultV1<()> {
    let result_ref = reference_field(observations, "result", "run.observations")?;
    let result = parse_canonical_document(
        archive_object(objects, &result_ref, "result observation")?,
        "result observation",
    )?;
    let result = object(&result, "result observation")?;
    exact_keys(
        result,
        &[
            "artifactSha256",
            "authority",
            "candidate",
            "checks",
            "commandSha256",
            "driverIdentitySha256",
            "fixtureId",
            "kernelSymbols",
            "lane",
            "observedIdentities",
            "outcome",
            "reservationIdentity",
            "runtimeIdentitySha256",
            "schema",
            "target",
            "transactionSha256",
        ],
        "result observation",
    )?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let compiler_input = object(
        required(fixture, "compilerInput", "fixture")?,
        "compiler input",
    )?;
    let transaction = object(
        required(record, "productionTransaction", "record")?,
        "record.productionTransaction",
    )?;
    let checks = object(required(result, "checks", "result")?, "result.checks")?;
    exact_keys(
        checks,
        &[
            "canariesChecked",
            "completeOutputChecked",
            "inputsUnchangedChecked",
            "paddingChecked",
            "timedOut",
        ],
        "result.checks",
    )?;
    for field in [
        "canariesChecked",
        "completeOutputChecked",
        "inputsUnchangedChecked",
        "paddingChecked",
    ] {
        if !bool_field(checks, field, "result.checks")? {
            return fail(format!("result check {field} did not pass"));
        }
    }
    if bool_field(checks, "timedOut", "result.checks")? {
        return fail("hardware result timed out");
    }
    let identities = object(
        required(result, "observedIdentities", "result")?,
        "result.observedIdentities",
    )?;
    exact_keys(
        identities,
        &[
            "canaryAfterSha256",
            "canaryBeforeSha256",
            "expectedOutputSha256",
            "inputAfterSha256",
            "inputBeforeSha256",
            "observedOutputSha256",
            "paddingAfterSha256",
            "paddingBeforeSha256",
        ],
        "result.observedIdentities",
    )?;
    let equal_pairs = [
        ("expectedOutputSha256", "observedOutputSha256"),
        ("inputBeforeSha256", "inputAfterSha256"),
        ("canaryBeforeSha256", "canaryAfterSha256"),
        ("paddingBeforeSha256", "paddingAfterSha256"),
    ];
    for (before, after) in equal_pairs {
        if digest_field(identities, before, "result identities")?
            != digest_field(identities, after, "result identities")?
        {
            return fail(format!("hardware result {before}/{after} differ"));
        }
    }
    if string_field(result, "schema", "result")? != RESULT_SCHEMA
        || string_field(result, "authority", "result")? != "observation-only"
        || string_field(result, "outcome", "result")? != "passed"
        || result.get("candidate") != request.get("candidate")
        || result.get("fixtureId") != fixture.get("fixtureId")
        || result.get("target") != fixture.get("target")
        || result.get("lane") != fixture.get("hardwareLane")
        || result.get("reservationIdentity") != fixture.get("hardwareReservation")
        || result.get("kernelSymbols") != compiler_input.get("kernelSymbols")
        || result.get("transactionSha256") != transaction.get("transactionSha256")
        || digest_field(result, "artifactSha256", "result")?
            != reference_field(observations, "artifact", "run.observations")?.sha256
        || digest_field(result, "driverIdentitySha256", "result")?
            != reference_field(observations, "driver", "run.observations")?.sha256
        || digest_field(result, "runtimeIdentitySha256", "result")?
            != reference_field(observations, "runtime", "run.observations")?.sha256
        || digest_field(result, "commandSha256", "result")?
            != domain_sha256(
                COMMAND_DOMAIN,
                required(fixture, "hardwareCommand", "fixture")?,
            )?
    {
        return fail("result observation is stale, cross-target, or cross-run");
    }
    Ok(())
}

struct AuthenticatedObservationReferencesV1 {
    hardware: ObjectReference,
    driver: ObjectReference,
    runtime: ObjectReference,
    isa: ObjectReference,
    resource: ObjectReference,
    result: ObjectReference,
}

#[allow(clippy::too_many_arguments)]
fn validate_cleanup_receipt(
    cleanup: &Map<String, Value>,
    index: &Map<String, Value>,
    run: &Map<String, Value>,
    request: &Map<String, Value>,
    record: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    trust: &LaneTrust,
    pre_hardware_record_sha256: [u8; 32],
) -> ResultV1<AuthenticatedObservationReferencesV1> {
    exact_keys(
        cleanup,
        &[
            "attestor",
            "authority",
            "candidate",
            "challengeNonce",
            "checks",
            "cleanupObservation",
            "fixtureId",
            "hardwareEvidence",
            "lane",
            "outcome",
            "phase",
            "preHardwareRecordSha256",
            "preCleanupCapsuleSha256",
            "receiptBindingSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "runReceiptSha256",
            "schema",
            "scratchIdentitySha256",
            "sequence",
            "target",
            "transactionSha256",
        ],
        "hardware cleanup receipt",
    )?;
    validate_binding(
        cleanup,
        "receiptBindingSha256",
        CLEANUP_DOMAIN,
        "cleanup receipt",
    )?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let challenge = object(
        required(request, "hardwareReceiptChallenge", "request")?,
        "request.hardwareReceiptChallenge",
    )?;
    let transaction = object(
        required(record, "productionTransaction", "record")?,
        "record.productionTransaction",
    )?;
    let checks = object(required(cleanup, "checks", "cleanup")?, "cleanup.checks")?;
    exact_keys(
        checks,
        &[
            "cleanupCompleted",
            "receiptSpoolAbsentBeforeEmission",
            "scratchDescendantsAbsent",
            "scratchRootAbsent",
        ],
        "cleanup.checks",
    )?;
    for field in checks.keys() {
        if !bool_field(checks, field, "cleanup.checks")? {
            return fail(format!("terminal cleanup check {field} did not pass"));
        }
    }
    let pre_hardware_record_identity = Value::String(encode_hex(&pre_hardware_record_sha256));
    let expected: [(&str, &Value); 15] = [
        ("attestor", required(run, "attestor", "run")?),
        ("candidate", required(request, "candidate", "request")?),
        ("challengeNonce", required(challenge, "nonce", "challenge")?),
        ("fixtureId", required(fixture, "fixtureId", "fixture")?),
        ("lane", required(fixture, "hardwareLane", "fixture")?),
        ("preHardwareRecordSha256", &pre_hardware_record_identity),
        (
            "preCleanupCapsuleSha256",
            required(index, "preCleanupCapsuleSha256", "index")?,
        ),
        (
            "requestBindingSha256",
            required(request, "requestBindingSha256", "request")?,
        ),
        (
            "reservationIdentity",
            required(fixture, "hardwareReservation", "fixture")?,
        ),
        (
            "runReceiptSha256",
            required(
                object(required(index, "runReceipt", "index")?, "index.runReceipt")?,
                "sha256",
                "index.runReceipt",
            )?,
        ),
        (
            "scratchIdentitySha256",
            required(run, "scratchIdentitySha256", "run")?,
        ),
        ("target", required(fixture, "target", "fixture")?),
        (
            "transactionSha256",
            required(transaction, "transactionSha256", "transaction")?,
        ),
        ("authority", &Value::String(AUTHORITY.to_owned())),
        ("outcome", &Value::String("passed".to_owned())),
    ];
    for (field, value) in expected {
        if cleanup.get(field) != Some(value) {
            return fail(format!("cleanup receipt {field} is stale or substituted"));
        }
    }
    if string_field(cleanup, "schema", "cleanup")? != CLEANUP_SCHEMA
        || string_field(cleanup, "phase", "cleanup")? != "post-cleanup"
        || u64_field(cleanup, "sequence", "cleanup")? != 2
    {
        return fail("cleanup receipt phase or sequence differs");
    }
    let attestor = object(
        required(cleanup, "attestor", "cleanup")?,
        "cleanup.attestor",
    )?;
    if string_field(attestor, "identity", "cleanup.attestor")? != trust.attestor_identity
        || digest_field(attestor, "publicKeySha256", "cleanup.attestor")? != trust.public_key_sha256
    {
        return fail("cleanup receipt names an untrusted attestor");
    }

    validate_pre_cleanup_capsule(index, run, objects)?;
    validate_cleanup_observation(cleanup, run, objects)?;
    validate_hardware_summary(cleanup, run, request, record, objects, trust)
}

fn validate_pre_cleanup_capsule(
    index: &Map<String, Value>,
    run: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
) -> ResultV1<()> {
    let encoded_sha256 = pre_cleanup_capsule_sha256(index, run, objects)?;
    if digest_field(index, "preCleanupCapsuleSha256", "archive index")? != encoded_sha256 {
        return fail("pre-cleanup capsule identity is stale or substituted");
    }
    Ok(())
}

fn pre_cleanup_capsule_sha256(
    index: &Map<String, Value>,
    run: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
) -> ResultV1<[u8; 32]> {
    let attestor = object(required(run, "attestor", "run")?, "run.attestor")?;
    let pre_index = object_value([
        (
            "attestorIdentity",
            required(attestor, "identity", "attestor")?.clone(),
        ),
        ("candidate", required(run, "candidate", "run")?.clone()),
        (
            "challengeNonce",
            required(run, "challengeNonce", "run")?.clone(),
        ),
        ("fixtureId", required(run, "fixtureId", "run")?.clone()),
        ("lane", required(run, "lane", "run")?.clone()),
        (
            "publicKeySha256",
            required(attestor, "publicKeySha256", "attestor")?.clone(),
        ),
        (
            "preHardwareRecordSha256",
            required(run, "preHardwareRecordSha256", "run")?.clone(),
        ),
        (
            "requestBindingSha256",
            required(run, "requestBindingSha256", "run")?.clone(),
        ),
        (
            "reservationIdentity",
            required(run, "reservationIdentity", "run")?.clone(),
        ),
        (
            "runReceipt",
            required(index, "runReceipt", "index")?.clone(),
        ),
        (
            "runSignature",
            required(index, "runSignature", "index")?.clone(),
        ),
        ("schema", Value::String(PRE_CLEANUP_SCHEMA.to_owned())),
        (
            "scratchIdentitySha256",
            required(run, "scratchIdentitySha256", "run")?.clone(),
        ),
        ("target", required(run, "target", "run")?.clone()),
        (
            "transactionSha256",
            required(run, "transactionSha256", "run")?.clone(),
        ),
    ]);
    let observations = object(required(run, "observations", "run")?, "run.observations")?;
    let mut pre_objects = BTreeMap::new();
    for field in observations.keys() {
        let reference = reference_field(observations, field, "pre-cleanup observation")?;
        let payload = archive_object(objects, &reference, "pre-cleanup observation")?;
        pre_objects.insert(reference.sha256, payload.to_vec());
    }
    for field in ["runReceipt", "runSignature"] {
        let reference = reference_field(index, field, "pre-cleanup control object")?;
        let payload = archive_object(objects, &reference, "pre-cleanup object")?;
        pre_objects.insert(reference.sha256, payload.to_vec());
    }
    let encoded = encode_canonical_archive(&pre_index, &pre_objects)?;
    Ok(sha256(&encoded))
}

fn validate_cleanup_observation(
    cleanup: &Map<String, Value>,
    run: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
) -> ResultV1<()> {
    let reference = reference_field(cleanup, "cleanupObservation", "cleanup")?;
    let value = parse_canonical_document(
        archive_object(objects, &reference, "cleanup observation")?,
        "cleanup observation",
    )?;
    let value = object(&value, "cleanup observation")?;
    exact_keys(
        value,
        &["checks", "schema", "scratchIdentitySha256"],
        "cleanup observation",
    )?;
    let expected_checks = object_value([
        ("receiptSpoolAbsentBeforeEmission", Value::Bool(true)),
        ("scratchDescendantsAbsent", Value::Bool(true)),
        ("scratchRootAbsent", Value::Bool(true)),
    ]);
    if string_field(value, "schema", "cleanup observation")?
        != "fe2o3-tutorial-hardware-cleanup-observation-v1"
        || value.get("scratchIdentitySha256") != run.get("scratchIdentitySha256")
        || value.get("checks") != Some(&expected_checks)
    {
        return fail("cleanup observation is malformed or substituted");
    }
    Ok(())
}

fn validate_hardware_summary(
    cleanup: &Map<String, Value>,
    run: &Map<String, Value>,
    request: &Map<String, Value>,
    record: &Map<String, Value>,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    trust: &LaneTrust,
) -> ResultV1<AuthenticatedObservationReferencesV1> {
    let reference = reference_field(cleanup, "hardwareEvidence", "cleanup")?;
    let value = parse_canonical_document(
        archive_object(objects, &reference, "hardware summary")?,
        "hardware summary",
    )?;
    let value = object(&value, "hardware summary")?;
    exact_keys(
        value,
        &[
            "artifactInspectionSha256",
            "artifactSha256",
            "authority",
            "candidate",
            "checks",
            "commandSha256",
            "driverIdentitySha256",
            "fixtureId",
            "isaInspectionSha256",
            "kernelSymbols",
            "lane",
            "launchContractSha256",
            "outcome",
            "reservationIdentity",
            "resourceUsageSha256",
            "resultSha256",
            "runtimeIdentitySha256",
            "schema",
            "target",
            "targetIdentitySha256",
            "timeoutSeconds",
        ],
        "hardware summary",
    )?;
    let observations = object(required(run, "observations", "run")?, "run.observations")?;
    let driver = reference_field(observations, "driver", "run.observations")?;
    let runtime = reference_field(observations, "runtime", "run.observations")?;
    let isa = reference_field(observations, "isa", "run.observations")?;
    let resource = reference_field(observations, "resource", "run.observations")?;
    let result = reference_field(observations, "result", "run.observations")?;
    let fixture = object(required(request, "fixture", "request")?, "request.fixture")?;
    let compiler_input = object(
        required(fixture, "compilerInput", "fixture")?,
        "compiler input",
    )?;
    let result_payload = archive_object(
        objects,
        &reference_field(observations, "result", "run.observations")?,
        "result observation",
    )?;
    let result_value = parse_canonical_document(result_payload, "result observation")?;
    let result_checks = object(
        required(
            object(&result_value, "result observation")?,
            "checks",
            "result",
        )?,
        "result.checks",
    )?;
    let summary_checks = object(
        required(value, "checks", "hardware summary")?,
        "summary.checks",
    )?;
    exact_keys(
        summary_checks,
        &[
            "canariesChecked",
            "cleanupComplete",
            "completeOutputChecked",
            "inputsUnchangedChecked",
            "isaInspected",
            "paddingChecked",
            "resourceUsageInspected",
            "timedOut",
        ],
        "hardware summary.checks",
    )?;
    for field in [
        "canariesChecked",
        "cleanupComplete",
        "completeOutputChecked",
        "inputsUnchangedChecked",
        "isaInspected",
        "paddingChecked",
        "resourceUsageInspected",
    ] {
        if !bool_field(summary_checks, field, "hardware summary.checks")? {
            return fail(format!("hardware summary check {field} did not pass"));
        }
    }
    if bool_field(summary_checks, "timedOut", "hardware summary.checks")?
        || summary_checks.get("canariesChecked") != result_checks.get("canariesChecked")
        || summary_checks.get("completeOutputChecked") != result_checks.get("completeOutputChecked")
        || summary_checks.get("inputsUnchangedChecked")
            != result_checks.get("inputsUnchangedChecked")
        || summary_checks.get("paddingChecked") != result_checks.get("paddingChecked")
    {
        return fail("hardware summary does not preserve the signed result checks");
    }
    let production = object(
        required(record, "productionEvidence", "record")?,
        "production evidence",
    )?;
    let expected_joins = [
        ("artifactInspectionSha256", "artifactInspection"),
        ("artifactSha256", "artifact"),
        ("driverIdentitySha256", "driver"),
        ("runtimeIdentitySha256", "runtime"),
    ];
    for (field, observation) in expected_joins {
        if digest_field(value, field, "hardware summary")?
            != reference_field(observations, observation, "run.observations")?.sha256
        {
            return fail(format!(
                "hardware summary {field} differs from signed observation"
            ));
        }
    }
    if digest_field(value, "isaInspectionSha256", "hardware summary")? != isa.sha256
        || digest_field(value, "resourceUsageSha256", "hardware summary")? != resource.sha256
        || digest_field(value, "resultSha256", "hardware summary")? != result.sha256
        || value.get("candidate") != request.get("candidate")
        || value.get("fixtureId") != fixture.get("fixtureId")
        || value.get("target") != fixture.get("target")
        || value.get("lane") != fixture.get("hardwareLane")
        || value.get("reservationIdentity") != fixture.get("hardwareReservation")
        || value.get("kernelSymbols") != compiler_input.get("kernelSymbols")
        || value.get("commandSha256") != run.get("commandSha256")
        || value.get("launchContractSha256") != production.get("launchContractSha256")
        || value.get("targetIdentitySha256") != production.get("targetIdentitySha256")
        || string_field(value, "schema", "hardware summary")? != HARDWARE_SCHEMA
        || string_field(value, "authority", "hardware summary")?
            != "verification-input-no-independent-authority"
        || string_field(value, "outcome", "hardware summary")? != "passed"
        || identity_field(value, "reservationIdentity", "hardware summary")?
            != trust.reservation_identity
        || u64_field(value, "timeoutSeconds", "hardware summary")?
            != u64_field(
                object(
                    required(fixture, "hardwareCommand", "fixture")?,
                    "hardware command",
                )?,
                "timeoutSeconds",
                "hardware command",
            )?
    {
        return fail("hardware summary is stale, cross-target, or cross-run");
    }
    Ok(AuthenticatedObservationReferencesV1 {
        hardware: reference,
        driver,
        runtime,
        isa,
        resource,
        result,
    })
}

fn referenced_archive_objects(
    index: &Map<String, Value>,
    run: &Map<String, Value>,
    cleanup: &Map<String, Value>,
    files: &Map<String, Value>,
) -> ResultV1<BTreeSet<[u8; 32]>> {
    let mut referenced = BTreeSet::new();
    for field in [
        "runReceipt",
        "runSignature",
        "cleanupReceipt",
        "cleanupSignature",
    ] {
        referenced.insert(reference_field(index, field, "archive index")?.sha256);
    }
    for field in ["cleanupObservation", "hardwareEvidence"] {
        referenced.insert(reference_field(cleanup, field, "cleanup receipt")?.sha256);
    }
    let observations = object(required(run, "observations", "run")?, "run.observations")?;
    for field in observations.keys() {
        referenced.insert(reference_field(observations, field, "run.observations")?.sha256);
    }
    for (name, kind) in RUN_COMPILER_OBSERVATIONS {
        if reference_field(observations, name, "run.observations")?
            != reference_field(files, kind, "pre-hardware record.evidenceFiles")?
        {
            return fail(format!("compiler observation {name} differs from {kind}"));
        }
    }
    for kind in VERIFIER_ONLY_COMPILER_OBJECTS {
        referenced
            .insert(reference_field(files, kind, "pre-hardware record.evidenceFiles")?.sha256);
    }
    Ok(referenced)
}

fn validate_request_binding(request: &Map<String, Value>) -> ResultV1<()> {
    exact_keys(
        request,
        &[
            "candidate",
            "capabilityKernel",
            "fixture",
            "hardwareReceiptChallenge",
            "manifest",
            "productionTransaction",
            "requestBindingSha256",
            "roadmapIssue",
            "schema",
            "simulatorEvidence",
        ],
        "transaction request",
    )?;
    if string_field(request, "schema", "transaction request")?
        != "fe2o3-tutorial-production-transaction-request-v1"
    {
        return fail("transaction request schema differs");
    }
    let candidate = object(
        required(request, "candidate", "request")?,
        "request.candidate",
    )?;
    exact_keys(
        candidate,
        &["compilerCommit", "compilerTree", "worktreeClean"],
        "request.candidate",
    )?;
    for field in ["compilerCommit", "compilerTree"] {
        let value = string_field(candidate, field, "request.candidate")?;
        if value.len() != 40 || !is_lower_hex(value) || value.bytes().all(|byte| byte == b'0') {
            return fail(format!(
                "request candidate {field} is not a nonzero Git identity"
            ));
        }
    }
    if !bool_field(candidate, "worktreeClean", "request.candidate")? {
        return fail("transaction request does not name a clean compiler candidate");
    }
    let observed = digest_field(request, "requestBindingSha256", "transaction request")?;
    let mut subject = request.clone();
    subject.remove("requestBindingSha256");
    if observed != domain_sha256(REQUEST_DOMAIN, &Value::Object(subject))? {
        return fail("transaction request binding is stale");
    }
    Ok(())
}

fn load_trust_policy(policy: &Value) -> ResultV1<BTreeMap<(String, String), LaneTrust>> {
    let policy = object(policy, "hardware trust policy")?;
    exact_keys(
        policy,
        &["lanes", "schema", "verifier"],
        "hardware trust policy",
    )?;
    if string_field(policy, "schema", "hardware trust policy")? != POLICY_SCHEMA {
        return fail("hardware trust policy schema differs");
    }
    let verifier = object(
        required(policy, "verifier", "hardware trust policy")?,
        "hardware trust policy.verifier",
    )?;
    exact_keys(
        verifier,
        &["path", "sha256"],
        "hardware trust policy.verifier",
    )?;
    let measured = measure_openssl()?;
    if string_field(verifier, "path", "trust policy verifier")? != OPENSSL_PATH
        || digest_field(verifier, "sha256", "trust policy verifier")? != measured
    {
        return fail("trust policy does not pin the active OpenSSL verifier");
    }
    let lanes = array(
        required(policy, "lanes", "hardware trust policy")?,
        "policy.lanes",
    )?;
    if lanes.is_empty() {
        return fail("hardware trust policy has no lanes");
    }
    let mut result = BTreeMap::new();
    let mut ordering = Vec::with_capacity(lanes.len());
    for (ordinal, lane) in lanes.iter().enumerate() {
        let label = format!("hardware trust policy.lanes[{ordinal}]");
        let lane = object(lane, &label)?;
        exact_keys(
            lane,
            &[
                "attestorIdentity",
                "lane",
                "publicKeyPem",
                "publicKeySha256",
                "reservationIdentity",
                "target",
            ],
            &label,
        )?;
        let lane_name = identity_field(lane, "lane", &label)?.to_owned();
        let target = target_field(lane, "target", &label)?.to_owned();
        let attestor_identity = identity_field(lane, "attestorIdentity", &label)?.to_owned();
        let reservation_identity = identity_field(lane, "reservationIdentity", &label)?.to_owned();
        let public_key = string_field(lane, "publicKeyPem", &label)?
            .as_bytes()
            .to_vec();
        if public_key.is_empty() || public_key.len() > 16 * 1024 {
            return fail(format!("{label} public key has an invalid byte length"));
        }
        let canonical = run_openssl_input(&["pkey", "-pubin", "-pubout"], &public_key)?;
        let der = run_openssl_input(&["pkey", "-pubin", "-outform", "DER"], &public_key)?;
        const ED25519_DER_PREFIX: &[u8] = &[
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        let key_sha = digest_field(lane, "publicKeySha256", &label)?;
        if canonical != public_key
            || der.len() != 44
            || !der.starts_with(ED25519_DER_PREFIX)
            || key_sha != sha256(&public_key)
        {
            return fail(format!(
                "{label} does not contain canonical Ed25519 public material"
            ));
        }
        let key = (lane_name, target);
        if result
            .insert(
                key.clone(),
                LaneTrust {
                    attestor_identity,
                    public_key,
                    public_key_sha256: key_sha,
                    reservation_identity,
                },
            )
            .is_some()
        {
            return fail("hardware trust policy contains a duplicate lane/target");
        }
        ordering.push(key);
    }
    if ordering.windows(2).any(|pair| pair[0] >= pair[1]) {
        return fail("hardware trust policy lanes are not canonically ordered");
    }
    Ok(result)
}

fn measure_openssl() -> ResultV1<[u8; 32]> {
    let path = Path::new(OPENSSL_PATH);
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!(
            "cannot inspect OpenSSL verifier: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.permissions().mode() & 0o111 == 0
        || metadata.len() == 0
        || metadata.len() > 128 * 1024 * 1024
    {
        return fail("OpenSSL verifier is not root-owned, executable, bounded, and non-writable");
    }
    let before = stable_metadata(&metadata);
    let bytes = read_bounded(path, 128 * 1024 * 1024, "OpenSSL verifier")?;
    let after = fs::metadata(path).map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!(
            "cannot revalidate OpenSSL verifier: {error}"
        ))
    })?;
    if stable_metadata(&after) != before || bytes.len() as u64 != metadata.len() {
        return fail("OpenSSL verifier changed while it was measured");
    }
    Ok(sha256(&bytes))
}

fn run_openssl_input(arguments: &[&str], input: &[u8]) -> ResultV1<Vec<u8>> {
    let before = measure_openssl()?;
    let mut child = Command::new(OPENSSL_PATH)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            TutorialHardwareQualificationErrorV1::new(format!(
                "cannot execute OpenSSL verifier: {error}"
            ))
        })?;
    child
        .stdin
        .take()
        .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("OpenSSL stdin is unavailable"))?
        .write_all(input)
        .map_err(|error| {
            TutorialHardwareQualificationErrorV1::new(format!(
                "cannot supply OpenSSL input: {error}"
            ))
        })?;
    let output = child.wait_with_output().map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("cannot wait for OpenSSL: {error}"))
    })?;
    if before != measure_openssl()? {
        return fail("OpenSSL verifier changed while it was used");
    }
    if !output.status.success() {
        return fail(format!(
            "OpenSSL rejected typed input: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn verify_signature(payload: &[u8], signature: &[u8], trust: &LaneTrust) -> ResultV1<()> {
    if signature.len() != 64 {
        return fail("hardware receipt signature is not 64 bytes");
    }
    let temporary = PrivateTemporaryDirectory::new()?;
    let key = temporary.write("public.pem", &trust.public_key)?;
    let mut signed = Vec::with_capacity(SIGNATURE_CONTEXT.len() + payload.len());
    signed.extend_from_slice(SIGNATURE_CONTEXT);
    signed.extend_from_slice(payload);
    let message = temporary.write("message.bin", &signed)?;
    let signature_path = temporary.write("signature.bin", signature)?;
    let before = measure_openssl()?;
    let output = Command::new(OPENSSL_PATH)
        .args(["pkeyutl", "-verify", "-pubin", "-inkey"])
        .arg(key)
        .args(["-rawin", "-in"])
        .arg(message)
        .arg("-sigfile")
        .arg(signature_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            TutorialHardwareQualificationErrorV1::new(format!(
                "cannot execute signature verifier: {error}"
            ))
        })?;
    if before != measure_openssl()? {
        return fail("OpenSSL verifier changed during signature authentication");
    }
    if !output.status.success() {
        return fail("hardware receipt Ed25519 signature authentication failed");
    }
    Ok(())
}

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct PrivateTemporaryDirectory(PathBuf);

impl PrivateTemporaryDirectory {
    fn new() -> ResultV1<Self> {
        let parent = std::env::temp_dir();
        for _ in 0..128 {
            let ordinal = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "fe2o3-tutorial-hardware-verify-{}-{ordinal}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(
                        |error| {
                            TutorialHardwareQualificationErrorV1::new(format!(
                                "cannot protect signature workspace: {error}"
                            ))
                        },
                    )?;
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return fail(format!("cannot create signature workspace: {error}"));
                }
            }
        }
        fail("cannot allocate a unique signature workspace")
    }

    fn write(&self, name: &str, bytes: &[u8]) -> ResultV1<PathBuf> {
        let path = self.0.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| {
                TutorialHardwareQualificationErrorV1::new(format!(
                    "cannot create signature input: {error}"
                ))
            })?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| {
                TutorialHardwareQualificationErrorV1::new(format!(
                    "cannot persist signature input: {error}"
                ))
            })?;
        Ok(path)
    }
}

impl Drop for PrivateTemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct ZipEntry {
    name: String,
    payload: Vec<u8>,
    crc32: u32,
    local_offset: u32,
}

fn decode_canonical_archive(payload: &[u8]) -> ResultV1<StrictArchive> {
    if payload.is_empty() || payload.len() > MAX_ARCHIVE_BYTES || payload.len() < 22 {
        return fail("hardware archive has an invalid byte length");
    }
    let eocd_offset = payload.len() - 22;
    let mut eocd = BinaryDecoder::new(&payload[eocd_offset..]);
    if eocd.u32()? != 0x0605_4b50 || eocd.u16()? != 0 || eocd.u16()? != 0 {
        return fail("hardware archive has a noncanonical end record");
    }
    let disk_entries = usize::from(eocd.u16()?);
    let total_entries = usize::from(eocd.u16()?);
    let central_size = usize::try_from(eocd.u32()?)
        .map_err(|_| TutorialHardwareQualificationErrorV1::new("central size overflow"))?;
    let central_offset = usize::try_from(eocd.u32()?)
        .map_err(|_| TutorialHardwareQualificationErrorV1::new("central offset overflow"))?;
    if eocd.u16()? != 0
        || !eocd.finished()
        || disk_entries != total_entries
        || total_entries == 0
        || total_entries > MAX_ARCHIVE_OBJECTS + 1
        || central_offset.checked_add(central_size) != Some(eocd_offset)
    {
        return fail("hardware archive end record is ambiguous or outside bounds");
    }

    let mut local = BinaryDecoder::new(&payload[..central_offset]);
    let mut entries = Vec::with_capacity(total_entries);
    let mut expanded = 0usize;
    while !local.finished() {
        let local_offset = u32::try_from(local.position())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP offset overflow"))?;
        if local.u32()? != 0x0403_4b50
            || local.u16()? != 20
            || local.u16()? != 0
            || local.u16()? != 0
            || local.u16()? != 0
            || local.u16()? != 33
        {
            return fail("hardware archive local entry metadata is noncanonical");
        }
        let crc = local.u32()?;
        let compressed = usize::try_from(local.u32()?)
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP size overflow"))?;
        let size = usize::try_from(local.u32()?)
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP size overflow"))?;
        let name_len = usize::from(local.u16()?);
        let extra_len = local.u16()?;
        if compressed != size
            || size == 0
            || size > MAX_OBJECT_BYTES
            || name_len == 0
            || extra_len != 0
        {
            return fail("hardware archive local entry length or extra data is noncanonical");
        }
        let name_bytes = local.take(name_len)?;
        if !name_bytes.is_ascii() {
            return fail("hardware archive entry name is not ASCII");
        }
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("invalid ZIP entry name"))?
            .to_owned();
        let entry_payload = local.take(size)?.to_vec();
        expanded = expanded
            .checked_add(size)
            .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("ZIP expansion overflow"))?;
        if expanded > MAX_ARCHIVE_BYTES || crc32(&entry_payload) != crc {
            return fail("hardware archive entry checksum or aggregate size differs");
        }
        entries.push(ZipEntry {
            name,
            payload: entry_payload,
            crc32: crc,
            local_offset,
        });
        if entries.len() > total_entries {
            return fail("hardware archive local inventory exceeds the end record");
        }
    }
    if entries.len() != total_entries {
        return fail("hardware archive local inventory differs from the end record");
    }
    let names = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    if names.first().copied() != Some(INDEX_NAME) || names.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return fail("hardware archive entries are not strictly canonically ordered");
    }

    let mut central = BinaryDecoder::new(&payload[central_offset..eocd_offset]);
    for entry in &entries {
        if central.u32()? != 0x0201_4b50
            || central.u16()? != 0x0314
            || central.u16()? != 20
            || central.u16()? != 0
            || central.u16()? != 0
            || central.u16()? != 0
            || central.u16()? != 33
            || central.u32()? != entry.crc32
            || usize::try_from(central.u32()?).ok() != Some(entry.payload.len())
            || usize::try_from(central.u32()?).ok() != Some(entry.payload.len())
        {
            return fail("hardware archive central entry metadata differs");
        }
        let name_len = usize::from(central.u16()?);
        if central.u16()? != 0
            || central.u16()? != 0
            || central.u16()? != 0
            || central.u16()? != 0
            || central.u32()? != 0x8180_0000
            || central.u32()? != entry.local_offset
            || central.take(name_len)? != entry.name.as_bytes()
        {
            return fail("hardware archive central directory is noncanonical");
        }
    }
    if !central.finished() {
        return fail("hardware archive central directory has trailing bytes");
    }

    let index = parse_canonical_document(&entries[0].payload, "hardware archive index")?;
    let mut objects = BTreeMap::new();
    for entry in entries.into_iter().skip(1) {
        let components = entry.name.split('/').collect::<Vec<_>>();
        if components.len() != 4
            || components[0] != "objects"
            || components[1] != "sha256"
            || components[2].len() != 2
        {
            return fail("hardware archive contains a traversal or non-object entry");
        }
        let digest = decode_digest(components[3], "archive object identity")?;
        if components[2] != &components[3][..2]
            || sha256(&entry.payload) != digest
            || objects.insert(digest, entry.payload).is_some()
        {
            return fail("hardware archive object path, content, or uniqueness differs");
        }
    }
    if objects.len() > MAX_ARCHIVE_OBJECTS {
        return fail("hardware archive object inventory exceeds its bound");
    }
    let archive = StrictArchive { objects, index };
    if encode_canonical_archive(&archive.index, &archive.objects)? != payload {
        return fail("hardware archive bytes are not the canonical ZIP encoding");
    }
    Ok(archive)
}

fn encode_canonical_archive(
    index: &Value,
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
) -> ResultV1<Vec<u8>> {
    let mut index_payload = canonical_json(index)?;
    index_payload.push(b'\n');
    let mut source = Vec::with_capacity(objects.len() + 1);
    source.push((INDEX_NAME.to_owned(), index_payload));
    for (digest, payload) in objects {
        let hex = encode_hex(digest);
        source.push((
            format!("{OBJECT_PREFIX}/{}/{hex}", &hex[..2]),
            payload.clone(),
        ));
    }
    let mut output = Vec::new();
    let mut entries = Vec::with_capacity(source.len());
    for (name, payload) in source {
        if payload.is_empty() || payload.len() > MAX_OBJECT_BYTES || !name.is_ascii() {
            return fail("cannot encode an invalid canonical archive entry");
        }
        let local_offset = u32::try_from(output.len())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP offset overflow"))?;
        let size = u32::try_from(payload.len())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP entry size overflow"))?;
        let name_len = u16::try_from(name.len())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP name length overflow"))?;
        let crc = crc32(&payload);
        push_u32(&mut output, 0x0403_4b50);
        push_u16(&mut output, 20);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 33);
        push_u32(&mut output, crc);
        push_u32(&mut output, size);
        push_u32(&mut output, size);
        push_u16(&mut output, name_len);
        push_u16(&mut output, 0);
        output.extend_from_slice(name.as_bytes());
        output.extend_from_slice(&payload);
        entries.push(ZipEntry {
            name,
            payload,
            crc32: crc,
            local_offset,
        });
    }
    let central_offset = u32::try_from(output.len())
        .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP central offset overflow"))?;
    for entry in &entries {
        let size = u32::try_from(entry.payload.len())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP entry size overflow"))?;
        let name_len = u16::try_from(entry.name.len())
            .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP name length overflow"))?;
        push_u32(&mut output, 0x0201_4b50);
        push_u16(&mut output, 0x0314);
        push_u16(&mut output, 20);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 33);
        push_u32(&mut output, entry.crc32);
        push_u32(&mut output, size);
        push_u32(&mut output, size);
        push_u16(&mut output, name_len);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u32(&mut output, 0x8180_0000);
        push_u32(&mut output, entry.local_offset);
        output.extend_from_slice(entry.name.as_bytes());
    }
    let central_size = u32::try_from(output.len())
        .ok()
        .and_then(|end| end.checked_sub(central_offset))
        .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("ZIP central size overflow"))?;
    let count = u16::try_from(entries.len())
        .map_err(|_| TutorialHardwareQualificationErrorV1::new("ZIP entry count overflow"))?;
    push_u32(&mut output, 0x0605_4b50);
    push_u16(&mut output, 0);
    push_u16(&mut output, 0);
    push_u16(&mut output, count);
    push_u16(&mut output, count);
    push_u32(&mut output, central_size);
    push_u32(&mut output, central_offset);
    push_u16(&mut output, 0);
    if output.len() > MAX_ARCHIVE_BYTES {
        return fail("canonical archive exceeds its byte bound");
    }
    Ok(output)
}

struct BinaryDecoder<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BinaryDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    const fn position(&self) -> usize {
        self.position
    }

    fn take(&mut self, length: usize) -> ResultV1<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("ZIP offset overflow"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| TutorialHardwareQualificationErrorV1::new("truncated ZIP record"))?;
        self.position = end;
        Ok(value)
    }

    fn u16(&mut self) -> ResultV1<u16> {
        let bytes: [u8; 2] = self.take(2)?.try_into().expect("fixed-width slice");
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> ResultV1<u32> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("fixed-width slice");
        Ok(u32::from_le_bytes(bytes))
    }

    fn finished(&self) -> bool {
        self.position == self.bytes.len()
    }
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn reference_field(
    owner: &Map<String, Value>,
    field: &str,
    label: &str,
) -> ResultV1<ObjectReference> {
    let reference = object(required(owner, field, label)?, &format!("{label}.{field}"))?;
    exact_keys(
        reference,
        &["bytes", "path", "sha256"],
        &format!("{label}.{field}"),
    )?;
    let bytes = usize::try_from(u64_field(reference, "bytes", label)?)
        .map_err(|_| TutorialHardwareQualificationErrorV1::new("object length overflow"))?;
    let sha256 = digest_field(reference, "sha256", label)?;
    let digest = encode_hex(&sha256);
    let expected_path = format!("{OBJECT_PREFIX}/{}/{digest}", &digest[..2]);
    if bytes == 0
        || bytes > MAX_OBJECT_BYTES
        || string_field(reference, "path", label)? != expected_path
    {
        return fail(format!(
            "{label}.{field} is not a bounded content-addressed reference"
        ));
    }
    Ok(ObjectReference { bytes, sha256 })
}

fn archive_object<'a>(
    objects: &'a BTreeMap<[u8; 32], Vec<u8>>,
    reference: &ObjectReference,
    label: &str,
) -> ResultV1<&'a [u8]> {
    let payload = objects.get(&reference.sha256).ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!("hardware archive omitted {label}"))
    })?;
    if payload.len() != reference.bytes || sha256(payload) != reference.sha256 {
        return fail(format!("hardware archive {label} identity differs"));
    }
    Ok(payload)
}

fn authenticated_archive_object(
    objects: &BTreeMap<[u8; 32], Vec<u8>>,
    reference: &ObjectReference,
    label: &str,
) -> ResultV1<AuthenticatedArchiveObjectV1> {
    Ok(AuthenticatedArchiveObjectV1 {
        sha256: reference.sha256,
        payload: archive_object(objects, reference, label)?.into(),
    })
}

fn validate_binding(
    value: &Map<String, Value>,
    field: &str,
    domain: &[u8],
    label: &str,
) -> ResultV1<()> {
    let observed = digest_field(value, field, label)?;
    let mut subject = value.clone();
    subject.remove(field);
    if observed != domain_sha256(domain, &Value::Object(subject))? {
        return fail(format!("{label} binding is stale"));
    }
    Ok(())
}

fn parse_canonical_document(bytes: &[u8], label: &str) -> ResultV1<Value> {
    if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
        return fail(format!("{label} has an invalid JSON byte length"));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueValue::deserialize(&mut deserializer)
        .map_err(|error| {
            TutorialHardwareQualificationErrorV1::new(format!("cannot decode {label}: {error}"))
        })?
        .0;
    deserializer.end().map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("cannot decode {label}: {error}"))
    })?;
    validate_json_domain(&value, label)?;
    let mut expected = canonical_json(&value)?;
    expected.push(b'\n');
    if bytes != expected {
        return fail(format!(
            "{label} is not canonical JSON followed by one newline"
        ));
    }
    Ok(value)
}

fn canonical_json(value: &Value) -> ResultV1<Vec<u8>> {
    validate_json_domain(value, "canonical JSON")?;
    let mut output = Vec::new();
    encode_canonical_json(value, &mut output)?;
    Ok(output)
}

fn encode_canonical_json(value: &Value, output: &mut Vec<u8>) -> ResultV1<()> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(value) => output.extend_from_slice(
            serde_json::to_string(value)
                .map_err(|error| {
                    TutorialHardwareQualificationErrorV1::new(format!(
                        "cannot encode canonical JSON string: {error}"
                    ))
                })?
                .as_bytes(),
        ),
        Value::Array(values) => {
            output.push(b'[');
            for (ordinal, value) in values.iter().enumerate() {
                if ordinal != 0 {
                    output.push(b',');
                }
                encode_canonical_json(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (ordinal, key) in keys.into_iter().enumerate() {
                if ordinal != 0 {
                    output.push(b',');
                }
                encode_canonical_json(&Value::String(key.clone()), output)?;
                output.push(b':');
                encode_canonical_json(&values[key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn validate_json_domain(value: &Value, label: &str) -> ResultV1<()> {
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => Ok(()),
        Value::Number(_) => fail(format!("{label} contains a non-integer number")),
        Value::String(value) if value.is_ascii() => Ok(()),
        Value::String(_) => fail(format!("{label} contains non-ASCII text")),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_json_domain(value, label)),
        Value::Object(values) => values.iter().try_for_each(|(key, value)| {
            if !key.is_ascii() {
                return fail(format!("{label} contains a non-ASCII object key"));
            }
            validate_json_domain(value, label)
        }),
    }
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueValue)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string(value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some((key, value)) = object.next_entry::<String, UniqueValue>()? {
            if values.insert(key.clone(), value.0).is_some() {
                return Err(de::Error::custom(format!("duplicate JSON key {key:?}")));
            }
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

fn object<'a>(value: &'a Value, label: &str) -> ResultV1<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!("{label} must be an object"))
    })
}

fn array<'a>(value: &'a Value, label: &str) -> ResultV1<&'a [Value]> {
    value.as_array().map(Vec::as_slice).ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!("{label} must be an array"))
    })
}

fn required<'a>(owner: &'a Map<String, Value>, field: &str, label: &str) -> ResultV1<&'a Value> {
    owner
        .get(field)
        .ok_or_else(|| TutorialHardwareQualificationErrorV1::new(format!("{label} omits {field}")))
}

fn exact_keys(owner: &Map<String, Value>, fields: &[&str], label: &str) -> ResultV1<()> {
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let observed = owner.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if observed != expected {
        return fail(format!("{label} fields differ from the exact schema"));
    }
    Ok(())
}

fn string_field<'a>(owner: &'a Map<String, Value>, field: &str, label: &str) -> ResultV1<&'a str> {
    required(owner, field, label)?.as_str().ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!("{label}.{field} must be a string"))
    })
}

fn bool_field(owner: &Map<String, Value>, field: &str, label: &str) -> ResultV1<bool> {
    required(owner, field, label)?.as_bool().ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!("{label}.{field} must be a boolean"))
    })
}

fn u64_field(owner: &Map<String, Value>, field: &str, label: &str) -> ResultV1<u64> {
    required(owner, field, label)?.as_u64().ok_or_else(|| {
        TutorialHardwareQualificationErrorV1::new(format!(
            "{label}.{field} must be a nonnegative integer"
        ))
    })
}

fn digest_field(owner: &Map<String, Value>, field: &str, label: &str) -> ResultV1<[u8; 32]> {
    decode_digest(
        string_field(owner, field, label)?,
        &format!("{label}.{field}"),
    )
}

fn decode_digest(value: &str, label: &str) -> ResultV1<[u8; 32]> {
    if value.len() != 64 || !is_lower_hex(value) || value.bytes().all(|byte| byte == b'0') {
        return fail(format!(
            "{label} is not a nonzero lowercase SHA-256 identity"
        ));
    }
    let mut decoded = [0_u8; 32];
    for (index, slot) in decoded.iter_mut().enumerate() {
        *slot = (decode_nibble(value.as_bytes()[index * 2])? << 4)
            | decode_nibble(value.as_bytes()[index * 2 + 1])?;
    }
    Ok(decoded)
}

fn decode_nibble(value: u8) -> ResultV1<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => fail("invalid lowercase hexadecimal digit"),
    }
}

fn identity_field<'a>(
    owner: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> ResultV1<&'a str> {
    let value = string_field(owner, field, label)?;
    if value.is_empty()
        || value.len() > 96
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return fail(format!("{label}.{field} is not a canonical identity"));
    }
    Ok(value)
}

fn target_field<'a>(owner: &'a Map<String, Value>, field: &str, label: &str) -> ResultV1<&'a str> {
    let value = string_field(owner, field, label)?;
    let mut segments = value.split(':');
    let processor = segments.next().expect("split always yields one segment");
    let features = segments.collect::<Vec<_>>();
    if processor.len() != 6
        || !processor.starts_with("gfx")
        || !processor[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || features.iter().any(|feature| {
            feature.is_empty()
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'-'))
        })
    {
        return fail(format!("{label}.{field} is not a canonical GPU target"));
    }
    Ok(value)
}

fn sorted_unique_strings(value: &Value, label: &str) -> ResultV1<Vec<String>> {
    let values = array(value, label)?;
    if values.is_empty() {
        return fail(format!("{label} must not be empty"));
    }
    let result = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    TutorialHardwareQualificationErrorV1::new(format!(
                        "{label} contains a non-string or empty value"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result.windows(2).any(|pair| pair[0] >= pair[1]) {
        return fail(format!("{label} is not sorted and unique"));
    }
    Ok(result)
}

fn domain_sha256(domain: &[u8], value: &Value) -> ResultV1<[u8; 32]> {
    let canonical = canonical_json(value)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical);
    Ok(digest.finalize().into())
}

fn domain_document_sha256(domain: &[u8], canonical_document: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical_document);
    digest.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn object_value<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn read_bounded(path: &Path, maximum: usize, label: &str) -> ResultV1<Vec<u8>> {
    let file = File::open(path).map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("cannot open {label}: {error}"))
    })?;
    let metadata = file.metadata().map_err(|error| {
        TutorialHardwareQualificationErrorV1::new(format!("cannot inspect {label}: {error}"))
    })?;
    if metadata.len() == 0 || metadata.len() > maximum as u64 {
        return fail(format!("{label} has an invalid byte length"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            TutorialHardwareQualificationErrorV1::new(format!("cannot read {label}: {error}"))
        })?;
    if bytes.len() != metadata.len() as usize || bytes.len() > maximum {
        return fail(format!("{label} changed or exceeded its bound while read"));
    }
    Ok(bytes)
}

fn stable_metadata(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn fail<T>(message: impl Into<String>) -> ResultV1<T> {
    Err(TutorialHardwareQualificationErrorV1::new(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_archive_object(objects: &mut BTreeMap<[u8; 32], Vec<u8>>, payload: &[u8]) -> Value {
        let digest = sha256(payload);
        objects.insert(digest, payload.to_vec());
        let encoded = encode_hex(&digest);
        object_value([
            ("bytes", Value::from(payload.len() as u64)),
            (
                "path",
                Value::String(format!("{OBJECT_PREFIX}/{}/{encoded}", &encoded[..2])),
            ),
            ("sha256", Value::String(encoded)),
        ])
    }

    fn canonical_archive_vector() -> Vec<u8> {
        let payload = b"x".to_vec();
        let mut objects = BTreeMap::new();
        objects.insert(sha256(&payload), payload);
        encode_canonical_archive(&object_value([("a", Value::from(1))]), &objects).unwrap()
    }

    #[test]
    fn canonical_zip_matches_python_contract_byte_for_byte() {
        let bytes = canonical_archive_vector();
        assert_eq!(bytes.len(), 373);
        assert_eq!(
            encode_hex(&sha256(&bytes)),
            "16a6565f1490a3fff4c3584ef365e9258a2a5b0c00263d9079b1a56c23f858f4"
        );
        let decoded = decode_canonical_archive(&bytes).unwrap();
        assert_eq!(decoded.index, object_value([("a", Value::from(1))]));
        assert_eq!(decoded.objects.len(), 1);
    }

    #[test]
    fn zip_decoder_rejects_metadata_content_and_inventory_mutations() {
        let canonical = canonical_archive_vector();
        for offset in [
            4, 10, 14, 18, 22, 26, 28, 164, 168, 170, 174, 184, 194, 202, 351,
        ] {
            let mut mutated = canonical.clone();
            mutated[offset] ^= 1;
            assert!(
                decode_canonical_archive(&mutated).is_err(),
                "byte mutation at {offset} was accepted"
            );
        }
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(decode_canonical_archive(&trailing).is_err());
        let mut truncated = canonical;
        truncated.pop();
        assert!(decode_canonical_archive(&truncated).is_err());
    }

    #[test]
    fn strict_json_rejects_duplicates_layout_floats_and_unicode() {
        assert!(parse_canonical_document(b"{\"a\":1,\"a\":2}\n", "test").is_err());
        assert!(parse_canonical_document(b"{ \"a\":1}\n", "test").is_err());
        assert!(parse_canonical_document(b"{\"a\":1.0}\n", "test").is_err());
        assert!(parse_canonical_document("{\"a\":\"é\"}\n".as_bytes(), "test").is_err());
        assert_eq!(
            parse_canonical_document(b"{\"a\":1}\n", "test").unwrap(),
            object_value([("a", Value::from(1))])
        );
    }

    #[test]
    fn object_references_reject_traversal_aliases_and_uppercase_digests() {
        let digest = encode_hex(&sha256(b"x"));
        let valid = object_value([(
            "value",
            object_value([
                ("bytes", Value::from(1)),
                (
                    "path",
                    Value::String(format!("{OBJECT_PREFIX}/{}/{digest}", &digest[..2])),
                ),
                ("sha256", Value::String(digest.clone())),
            ]),
        )]);
        assert_eq!(
            reference_field(object(&valid, "owner").unwrap(), "value", "owner").unwrap(),
            ObjectReference {
                bytes: 1,
                sha256: sha256(b"x"),
            }
        );

        for path in [
            format!("../{digest}"),
            format!("{OBJECT_PREFIX}/00/{digest}"),
            format!("{OBJECT_PREFIX}/{}/{}", &digest[..2], digest.to_uppercase()),
        ] {
            let owner = object_value([(
                "value",
                object_value([
                    ("bytes", Value::from(1)),
                    ("path", Value::String(path)),
                    ("sha256", Value::String(digest.clone())),
                ]),
            )]);
            assert!(reference_field(object(&owner, "owner").unwrap(), "value", "owner").is_err());
        }
    }

    #[test]
    fn target_and_identity_syntax_are_exact() {
        for target in ["gfx942", "gfx950", "gfx942:xnack-"] {
            let value = object_value([("target", Value::String(target.to_owned()))]);
            assert_eq!(
                target_field(object(&value, "target").unwrap(), "target", "target").unwrap(),
                target
            );
        }
        for target in ["GFX942", "gfx94", "gfx94z", "gfx942/../../x", "amdgpu"] {
            let value = object_value([("target", Value::String(target.to_owned()))]);
            assert!(target_field(object(&value, "target").unwrap(), "target", "target").is_err());
        }
    }

    #[test]
    fn pre_hardware_record_identity_matches_the_newline_inclusive_contract() {
        let record = b"{\"schema\":\"fe2o3-tutorial-pre-hardware-record-v1\"}\n";
        assert_eq!(
            encode_hex(&domain_document_sha256(PRE_HARDWARE_RECORD_DOMAIN, record)),
            "9219608a6604c431f1be286f69998c153f34f6ae059619d1397ba8efd351e78d"
        );
        assert_ne!(
            domain_document_sha256(PRE_HARDWARE_RECORD_DOMAIN, record),
            domain_document_sha256(
                PRE_HARDWARE_RECORD_DOMAIN,
                record.strip_suffix(b"\n").unwrap(),
            )
        );
    }

    #[test]
    fn internal_pre_hardware_binding_is_distinct_from_the_signed_document_identity() {
        let mut record = object_value([
            ("preHardwareBindingSha256", Value::String("0".repeat(64))),
            (
                "schema",
                Value::String(PRE_HARDWARE_RECORD_SCHEMA.to_owned()),
            ),
        ]);
        let mut subject = object(&record, "record").unwrap().clone();
        subject.remove("preHardwareBindingSha256");
        let binding = domain_sha256(PRE_HARDWARE_RECORD_DOMAIN, &Value::Object(subject)).unwrap();
        record.as_object_mut().unwrap().insert(
            "preHardwareBindingSha256".to_owned(),
            Value::String(encode_hex(&binding)),
        );
        validate_binding(
            object(&record, "record").unwrap(),
            "preHardwareBindingSha256",
            PRE_HARDWARE_RECORD_DOMAIN,
            "record",
        )
        .unwrap();
        let mut document = canonical_json(&record).unwrap();
        document.push(b'\n');
        assert_ne!(
            binding,
            domain_document_sha256(PRE_HARDWARE_RECORD_DOMAIN, &document)
        );
    }

    #[test]
    fn pre_cleanup_capsule_binds_pre_hardware_identity_and_runtime_observation() {
        let mut objects = BTreeMap::new();
        let runtime = add_archive_object(&mut objects, b"runtime observation\n");
        let run_receipt = add_archive_object(&mut objects, b"run receipt\n");
        let run_signature = add_archive_object(&mut objects, b"run signature\n");
        let mut index = object_value([
            ("runReceipt", run_receipt),
            ("runSignature", run_signature),
            ("preCleanupCapsuleSha256", Value::String("0".repeat(64))),
        ]);
        let mut run = object_value([
            (
                "attestor",
                object_value([
                    ("identity", Value::String("attestor".to_owned())),
                    ("publicKeySha256", Value::String("1".repeat(64))),
                ]),
            ),
            ("candidate", Value::String("candidate".to_owned())),
            ("challengeNonce", Value::String("2".repeat(64))),
            ("fixtureId", Value::String("fixture".to_owned())),
            ("lane", Value::String("mi350".to_owned())),
            ("observations", object_value([("runtime", runtime)])),
            ("preHardwareRecordSha256", Value::String("3".repeat(64))),
            ("requestBindingSha256", Value::String("4".repeat(64))),
            (
                "reservationIdentity",
                Value::String("reservation".to_owned()),
            ),
            ("scratchIdentitySha256", Value::String("5".repeat(64))),
            ("target", Value::String("gfx950".to_owned())),
            ("transactionSha256", Value::String("6".repeat(64))),
        ]);
        let capsule = pre_cleanup_capsule_sha256(
            object(&index, "index").unwrap(),
            object(&run, "run").unwrap(),
            &objects,
        )
        .unwrap();
        index.as_object_mut().unwrap().insert(
            "preCleanupCapsuleSha256".to_owned(),
            Value::String(encode_hex(&capsule)),
        );
        validate_pre_cleanup_capsule(
            object(&index, "index").unwrap(),
            object(&run, "run").unwrap(),
            &objects,
        )
        .unwrap();

        run.as_object_mut().unwrap().insert(
            "preHardwareRecordSha256".to_owned(),
            Value::String("7".repeat(64)),
        );
        assert!(
            validate_pre_cleanup_capsule(
                object(&index, "index").unwrap(),
                object(&run, "run").unwrap(),
                &objects,
            )
            .is_err()
        );
    }

    #[test]
    fn authenticated_archive_object_retains_exact_nonconstructible_payload() {
        let payload = b"exact hardware-owned identity\n".to_vec();
        let reference = ObjectReference {
            bytes: payload.len(),
            sha256: sha256(&payload),
        };
        let objects = BTreeMap::from([(reference.sha256, payload.clone())]);
        let authenticated = authenticated_archive_object(&objects, &reference, "identity").unwrap();
        assert_eq!(authenticated.sha256, reference.sha256);
        assert_eq!(authenticated.payload.as_ref(), payload.as_slice());

        let substituted = ObjectReference {
            bytes: reference.bytes,
            sha256: sha256(b"substituted"),
        };
        assert!(authenticated_archive_object(&objects, &substituted, "identity").is_err());
    }

    #[test]
    fn hardware_admission_rosters_keep_authority_owned_evidence_pending() {
        assert!(PRE_HARDWARE_EVIDENCE_KINDS.contains(&"sealed-production-receipt"));
        assert!(PRE_HARDWARE_EVIDENCE_KINDS.contains(&"simulation-bundle-v8"));
        for kind in PENDING_EVIDENCE_KINDS {
            assert!(!PRE_HARDWARE_EVIDENCE_KINDS.contains(kind));
        }
        assert!(HARDWARE_OBSERVATIONS.contains(&"driver"));
        assert!(HARDWARE_OBSERVATIONS.contains(&"runtime"));
    }

    #[test]
    fn failed_admission_does_not_consume_replay_state() {
        let mut replay = TutorialHardwareReceiptReplayLedgerV1::new();
        let result = verify_tutorial_hardware_qualification_receipt_v1(
            TutorialHardwareQualificationReceiptInputV1 {
                request: b"{}\n",
                pre_hardware_record: b"{}\n",
                trust_policy: b"{}\n",
                archive: b"not-a-zip",
            },
            &mut replay,
        );
        assert!(result.is_err());
        assert_eq!(replay.accepted_receipt_count(), 0);
    }

    #[test]
    fn ed25519_signature_context_authenticates_and_rejects_mutation() {
        let temporary = PrivateTemporaryDirectory::new().unwrap();
        let private_key = temporary.0.join("private.pem");
        let generated = Command::new(OPENSSL_PATH)
            .args(["genpkey", "-algorithm", "ED25519", "-out"])
            .arg(&private_key)
            .output()
            .unwrap();
        assert!(generated.status.success());
        let public = Command::new(OPENSSL_PATH)
            .args(["pkey", "-in"])
            .arg(&private_key)
            .arg("-pubout")
            .output()
            .unwrap();
        assert!(public.status.success());
        let trust = LaneTrust {
            attestor_identity: "test-attestor".to_owned(),
            public_key_sha256: sha256(&public.stdout),
            public_key: public.stdout,
            reservation_identity: "test-reservation".to_owned(),
        };
        let payload = b"{\"schema\":\"test\"}\n";
        let mut message = SIGNATURE_CONTEXT.to_vec();
        message.extend_from_slice(payload);
        let message_path = temporary.write("signed-message.bin", &message).unwrap();
        let signature_path = temporary.0.join("generated-signature.bin");
        let signed = Command::new(OPENSSL_PATH)
            .args(["pkeyutl", "-sign", "-rawin", "-inkey"])
            .arg(private_key)
            .arg("-in")
            .arg(message_path)
            .arg("-out")
            .arg(&signature_path)
            .output()
            .unwrap();
        assert!(signed.status.success());
        let signature = fs::read(signature_path).unwrap();
        verify_signature(payload, &signature, &trust).unwrap();
        let mut mutated = payload.to_vec();
        mutated[2] ^= 1;
        assert!(verify_signature(&mutated, &signature, &trust).is_err());
    }
}
