//! Compiler-owned, authority-free verification of tutorial promotion batches.
//!
//! The promotion collector checks the release-manifest policy. This executable
//! independently replays compiler-owned binary formats and exact byte custody.
//! Its output authorizes only the collector to describe the checked archive; it
//! grants no publication, load, launch, or runtime authority.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use fe2o3_artifact_transaction::authenticated_compiler_capability_evidence_identity_v5;
use fe2o3_compiler_ffi::InertProductionCapabilityResultV5;
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
};
use fe2o3_kernel_analysis::ProductionW4CanonicalEncodingV1;
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};
use fe2o3_verifier::{
    validate_compiler_capability_evidence_v1, validate_compiler_capability_source_owner_v1,
    validate_compiler_proof_inputs_v4, validate_compiler_proof_inputs_v5,
    validate_compiler_target_lineage_v1,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

const BATCH_SCHEMA: &str = "fe2o3-tutorial-capability-qualification-batch-v1";
const REPORT_SCHEMA: &str = "fe2o3-tutorial-capability-producer-verification-v1";
const ROADMAP_ISSUE: &str = "https://github.com/harsh-nod/fe2o3/issues/272";
const MANIFEST_PATH: &str = "config/tutorial-kernel-manifest-v1.json";
const PIPELINE_ENTRY: &str = "rustc-codegen-fe2o3::production_pipeline";
const RECORD_DOMAIN: &[u8] = b"fe2o3-tutorial-capability-record-v1\0";
const BATCH_DOMAIN: &[u8] = b"fe2o3-tutorial-capability-batch-v1\0";
const CORPUS_DOMAIN: &[u8] = b"fe2o3-tutorial-kernel-corpus-contract-v1\0";
const MAX_BATCH_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TYPED_EVIDENCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EVIDENCE_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EVIDENCE_TOTAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;

const BATCH_KEYS: &[&str] = &[
    "batchBindingSha256",
    "candidate",
    "manifest",
    "records",
    "roadmapIssue",
    "schema",
    "semanticQualification",
];
const RECORD_KEYS: &[&str] = &[
    "capabilityClosure",
    "compilerInput",
    "evidenceFiles",
    "fixtureId",
    "graph",
    "hardware",
    "kernelSymbol",
    "lessonIds",
    "negativeFixtures",
    "productionEvidence",
    "productionTransaction",
    "proof",
    "recordBindingSha256",
    "simulator",
    "target",
    "targetDecision",
];
const EVIDENCE_KINDS: &[&str] = &[
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "driver-identity",
    "hardware",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "negative-fixture-set",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "runtime-identity",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "simulator",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
];
const PRODUCTION_EVIDENCE_KEYS: &[&str] = &[
    "artifactSha256",
    "artifactInspectionSha256",
    "capabilityAnalysisSha256",
    "capabilityClosureSha256",
    "compilerCommit",
    "compilerPolicySha256",
    "compilerTree",
    "finalOptimizedKirSha256",
    "hardwareEvidenceSha256",
    "hostAdmissionSha256",
    "launchContractSha256",
    "loweringIdentitySha256",
    "machineRefinementSha256",
    "negativeFixtureSetSha256",
    "numericalPolicySha256",
    "proofCheckerSha256",
    "proofEvidenceSha256",
    "proofObligationSetSha256",
    "simulatorEvidenceSha256",
    "sourceMirIdentitySha256",
    "sourceMirToKirRefinementSha256",
    "targetCapabilityDecisionSha256",
    "targetIdentitySha256",
];
const GRAPH_KEYS: &[&str] = &[
    "bundleContentIdentitySha256",
    "bundleSubjectIdentitySha256",
    "canonicalKirBytes",
    "canonicalKirVersion",
    "finalGraphEpoch",
    "kernelAbiIdentitySha256",
    "kernelCount",
    "productionKirIdentitySha256",
    "semanticMirIdentitySha256",
    "sourceInventoryReceiptSha256",
    "sourcePreflightReceiptSha256",
];

fn main() -> ExitCode {
    if env::args_os()
        .skip(1)
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    }
    match run(env::args_os().skip(1).collect()) {
        Ok(report) => match write_canonical_report(&report) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("fe2o3 tutorial production batch verifier: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("fe2o3 tutorial production batch verifier: {error}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> &'static str {
    "usage: fe2o3-verify-tutorial-production-batch-v1 \\\n       --repository <compiler-repository> --batch <batch.json> \\\n       --evidence-root <evidence-directory>"
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    repository: PathBuf,
    batch: PathBuf,
    evidence_root: PathBuf,
}

fn parse_options(arguments: Vec<OsString>) -> Result<Options, String> {
    let mut repository = None;
    let mut batch = None;
    let mut evidence_root = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            return Err("option names must be valid UTF-8".to_owned());
        };
        let slot = match argument {
            "--repository" => &mut repository,
            "--batch" => &mut batch,
            "--evidence-root" => &mut evidence_root,
            _ => return Err(format!("unknown option {argument:?}\n{}", usage())),
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("{argument} may be specified only once"));
        }
    }
    Ok(Options {
        repository: repository.ok_or_else(|| "missing --repository".to_owned())?,
        batch: batch.ok_or_else(|| "missing --batch".to_owned())?,
        evidence_root: evidence_root.ok_or_else(|| "missing --evidence-root".to_owned())?,
    })
}

fn run(arguments: Vec<OsString>) -> Result<Value, String> {
    let options = parse_options(arguments)?;
    let repository = real_directory(&options.repository, "repository")?;
    let evidence_root = real_directory(&options.evidence_root, "evidence root")?;
    let batch_path = real_file(&options.batch, "qualification batch")?;
    let batch_bytes = read_bounded_file(&batch_path, MAX_BATCH_BYTES, "qualification batch")?;
    let batch = parse_unique_json(&batch_bytes, "qualification batch")?;
    verify_batch(&repository, &evidence_root, &batch)?;
    expected_report(&batch)
}

fn write_canonical_report(report: &Value) -> Result<(), String> {
    let bytes = canonical_json(report)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .map_err(|error| format!("cannot write canonical report: {error}"))
}

fn expected_report(batch: &Value) -> Result<Value, String> {
    let batch = object(batch, "qualification batch")?;
    let candidate = required(batch, "candidate", "qualification batch")?.clone();
    let manifest = object(
        required(batch, "manifest", "qualification batch")?,
        "qualification batch.manifest",
    )?;
    let records = array(
        required(batch, "records", "qualification batch")?,
        "qualification batch.records",
    )?;
    let verifier_identity = executable_identity()?;
    let mut accepted = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let label = format!("qualification batch.records[{index}]");
        let record = object(record, &label)?;
        let transaction = object(required(record, "productionTransaction", &label)?, &label)?;
        let files = object(required(record, "evidenceFiles", &label)?, &label)?;
        let sealed = object(
            required(files, "sealed-production-receipt", &label)?,
            &label,
        )?;
        accepted.push(object_value([
            ("fixtureId", required(record, "fixtureId", &label)?.clone()),
            (
                "productionTransactionSha256",
                required(transaction, "transactionSha256", &label)?.clone(),
            ),
            (
                "recordBindingSha256",
                required(record, "recordBindingSha256", &label)?.clone(),
            ),
            (
                "sealedProductionReceiptSha256",
                required(sealed, "sha256", &label)?.clone(),
            ),
            (
                "simulationBundleV8Sha256",
                required(
                    object(required(files, "simulation-bundle-v8", &label)?, &label)?,
                    "sha256",
                    &label,
                )?
                .clone(),
            ),
            ("status", Value::String("accepted".to_owned())),
        ]));
    }
    Ok(object_value([
        (
            "authority",
            Value::String("verification-only-no-runtime-authority".to_owned()),
        ),
        (
            "batchBindingSha256",
            required(batch, "batchBindingSha256", "qualification batch")?.clone(),
        ),
        ("candidate", candidate),
        (
            "manifestRawSha256",
            required(manifest, "rawSha256", "qualification batch.manifest")?.clone(),
        ),
        ("records", Value::Array(accepted)),
        ("schema", Value::String(REPORT_SCHEMA.to_owned())),
        ("status", Value::String("accepted".to_owned())),
        ("verifierIdentitySha256", Value::String(verifier_identity)),
    ]))
}

#[derive(Debug)]
struct ManifestFixture {
    target: String,
    kernel_symbol: String,
    cargo_lock_path: String,
    cargo_lock_sha256: String,
    package_manifest: String,
    package_manifest_sha256: String,
    source_closure_sha256: String,
    contract_sha256: String,
}

fn verify_batch(repository: &Path, evidence_root: &Path, batch: &Value) -> Result<(), String> {
    let batch_object = object(batch, "qualification batch")?;
    exact_keys(batch_object, BATCH_KEYS, "qualification batch")?;
    expect_string(batch_object, "schema", BATCH_SCHEMA, "qualification batch")?;
    expect_string(
        batch_object,
        "roadmapIssue",
        ROADMAP_ISSUE,
        "qualification batch",
    )?;
    require_sha256(
        required(batch_object, "batchBindingSha256", "qualification batch")?,
        "qualification batch.batchBindingSha256",
    )?;
    let expected_binding = binding_sha256(batch, "batchBindingSha256", BATCH_DOMAIN)?;
    if string_field(batch_object, "batchBindingSha256", "qualification batch")? != expected_binding
    {
        return Err("qualification batch binding is stale".to_owned());
    }

    let candidate = object(
        required(batch_object, "candidate", "qualification batch")?,
        "qualification batch.candidate",
    )?;
    exact_keys(
        candidate,
        &["compilerCommit", "compilerTree", "worktreeClean"],
        "qualification batch.candidate",
    )?;
    if bool_field(candidate, "worktreeClean", "qualification batch.candidate")? != true {
        return Err("qualification candidate is not marked clean".to_owned());
    }
    require_git_identity(
        string_field(candidate, "compilerCommit", "qualification batch.candidate")?,
        "qualification batch.candidate.compilerCommit",
    )?;
    require_git_identity(
        string_field(candidate, "compilerTree", "qualification batch.candidate")?,
        "qualification batch.candidate.compilerTree",
    )?;
    verify_repository_candidate(repository, candidate)?;

    let fixtures = verify_manifest(repository, batch_object)?;
    verify_semantic_qualification_reference(evidence_root, batch_object)?;
    let records = array(
        required(batch_object, "records", "qualification batch")?,
        "qualification batch.records",
    )?;
    if records.is_empty() {
        return Err("qualification batch has no records".to_owned());
    }
    let observed_ids = records
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let label = format!("qualification batch.records[{index}]");
            string_field(object(value, &label)?, "fixtureId", &label).map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_ids = fixtures.keys().cloned().collect::<Vec<_>>();
    if observed_ids != expected_ids {
        return Err(
            "qualification records must cover the exact manifest fixture roster in order"
                .to_owned(),
        );
    }

    let mut transactions = BTreeSet::new();
    let mut sealed_results = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        let label = format!("qualification batch.records[{index}]");
        let record = object(record, &label)?;
        exact_keys(record, RECORD_KEYS, &label)?;
        require_sha256(
            required(record, "recordBindingSha256", &label)?,
            &format!("{label}.recordBindingSha256"),
        )?;
        let expected = binding_sha256(
            &Value::Object(record.clone()),
            "recordBindingSha256",
            RECORD_DOMAIN,
        )?;
        if string_field(record, "recordBindingSha256", &label)? != expected {
            return Err(format!("{label} binding is stale"));
        }
        let fixture_id = string_field(record, "fixtureId", &label)?;
        let fixture = fixtures
            .get(fixture_id)
            .ok_or_else(|| format!("{label} is not in the manifest"))?;
        if string_field(record, "target", &label)? != fixture.target.as_str()
            || string_field(record, "kernelSymbol", &label)? != fixture.kernel_symbol.as_str()
        {
            return Err(format!("{label} target or kernel symbol is substituted"));
        }
        let transaction = validate_transaction(record, &label)?;
        if !transactions.insert(transaction.to_owned()) {
            return Err(format!("{label} reuses a production transaction"));
        }
        let sealed_sha = verify_record(
            repository,
            evidence_root,
            candidate,
            record,
            fixture,
            &label,
        )?;
        if !sealed_results.insert(sealed_sha) {
            return Err(format!("{label} replays a sealed production result"));
        }
    }
    Ok(())
}

fn verify_repository_candidate(
    repository: &Path,
    candidate: &Map<String, Value>,
) -> Result<(), String> {
    let status = git(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err("producer verification requires a clean compiler worktree".to_owned());
    }
    let head = git(repository, &["rev-parse", "--verify", "HEAD"])?;
    let tree = git(repository, &["show", "-s", "--format=%T", "HEAD"])?;
    require_git_identity(&head, "compiler HEAD")?;
    require_git_identity(&tree, "compiler tree")?;
    if head != string_field(candidate, "compilerCommit", "qualification batch.candidate")?
        || tree != string_field(candidate, "compilerTree", "qualification batch.candidate")?
    {
        return Err("qualification candidate does not name the checked repository".to_owned());
    }
    Ok(())
}

fn verify_manifest(
    repository: &Path,
    batch: &Map<String, Value>,
) -> Result<BTreeMap<String, ManifestFixture>, String> {
    let manifest_claim = object(
        required(batch, "manifest", "qualification batch")?,
        "qualification batch.manifest",
    )?;
    exact_keys(
        manifest_claim,
        &["corpusContractSha256", "path", "rawSha256"],
        "qualification batch.manifest",
    )?;
    expect_string(
        manifest_claim,
        "path",
        MANIFEST_PATH,
        "qualification batch.manifest",
    )?;
    let manifest_path = repository.join(MANIFEST_PATH);
    let manifest_path = real_file(&manifest_path, "tutorial manifest")?;
    let bytes = read_bounded_file(&manifest_path, MAX_MANIFEST_BYTES, "tutorial manifest")?;
    if hex_sha256(&bytes)
        != string_field(manifest_claim, "rawSha256", "qualification batch.manifest")?
    {
        return Err("qualification batch names a different tutorial manifest".to_owned());
    }
    let manifest = parse_unique_json(&bytes, "tutorial manifest")?;
    let manifest = object(&manifest, "tutorial manifest")?;
    let mut contract = manifest.clone();
    contract.remove("baseline");
    let expected_contract = domain_sha256(CORPUS_DOMAIN, &Value::Object(contract))?;
    if string_field(
        manifest_claim,
        "corpusContractSha256",
        "qualification batch.manifest",
    )? != expected_contract
    {
        return Err("qualification batch names a different tutorial corpus contract".to_owned());
    }
    let compiler_fixtures = array(
        required(manifest, "compilerFixtures", "tutorial manifest")?,
        "tutorial manifest.compilerFixtures",
    )?;
    let capability_kernels = array(
        required(manifest, "capabilityKernels", "tutorial manifest")?,
        "tutorial manifest.capabilityKernels",
    )?;
    if compiler_fixtures.len() != capability_kernels.len() {
        return Err("tutorial manifest fixture and capability rosters differ".to_owned());
    }
    let kernels = capability_kernels
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let label = format!("tutorial manifest.capabilityKernels[{index}]");
            let value = object(value, &label)?;
            Ok((
                string_field(value, "fixtureId", &label)?.to_owned(),
                string_field(value, "kernelSymbol", &label)?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut fixtures = BTreeMap::new();
    for (index, value) in compiler_fixtures.iter().enumerate() {
        let label = format!("tutorial manifest.compilerFixtures[{index}]");
        let value = object(value, &label)?;
        let fixture_id = string_field(value, "fixtureId", &label)?.to_owned();
        let target = string_field(value, "target", &label)?.to_owned();
        let compiler_input = object(
            required(value, "compilerInput", &label)?,
            &format!("{label}.compilerInput"),
        )?;
        let kernel_symbol = kernels
            .get(&fixture_id)
            .ok_or_else(|| format!("{label} has no capability kernel"))?
            .to_owned();
        if fixtures
            .insert(
                fixture_id,
                ManifestFixture {
                    target,
                    kernel_symbol,
                    cargo_lock_path: string_field(
                        compiler_input,
                        "cargoLockPath",
                        &format!("{label}.compilerInput"),
                    )?
                    .to_owned(),
                    cargo_lock_sha256: require_sha256(
                        required(compiler_input, "cargoLockSha256", &label)?,
                        &format!("{label}.compilerInput.cargoLockSha256"),
                    )?
                    .to_owned(),
                    package_manifest: string_field(
                        compiler_input,
                        "packageManifest",
                        &format!("{label}.compilerInput"),
                    )?
                    .to_owned(),
                    package_manifest_sha256: require_sha256(
                        required(compiler_input, "packageManifestSha256", &label)?,
                        &format!("{label}.compilerInput.packageManifestSha256"),
                    )?
                    .to_owned(),
                    source_closure_sha256: require_sha256(
                        required(compiler_input, "sourceClosureSha256", &label)?,
                        &format!("{label}.compilerInput.sourceClosureSha256"),
                    )?
                    .to_owned(),
                    contract_sha256: require_sha256(
                        required(compiler_input, "contractSha256", &label)?,
                        &format!("{label}.compilerInput.contractSha256"),
                    )?
                    .to_owned(),
                },
            )
            .is_some()
        {
            return Err(format!("{label} duplicates a fixture identity"));
        }
    }
    Ok(fixtures)
}

fn verify_semantic_qualification_reference(
    evidence_root: &Path,
    batch: &Map<String, Value>,
) -> Result<(), String> {
    let snapshot = snapshot_reference(
        evidence_root,
        required(batch, "semanticQualification", "qualification batch")?,
        "qualification batch.semanticQualification",
    )?;
    let bytes = read_snapshot(
        &snapshot,
        MAX_BATCH_BYTES,
        "semantic qualification evidence",
    )?;
    let value = parse_unique_json(&bytes, "semantic qualification evidence")?;
    let mut expected = canonical_json(&value)?;
    expected.push(b'\n');
    if bytes != expected {
        return Err("semantic qualification evidence is not canonical JSON".to_owned());
    }
    Ok(())
}

fn validate_transaction<'a>(
    record: &'a Map<String, Value>,
    label: &str,
) -> Result<&'a str, String> {
    let transaction = object(required(record, "productionTransaction", label)?, label)?;
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
        &format!("{label}.productionTransaction"),
    )?;
    if bool_field(transaction, "allowsFallback", label)?
        || bool_field(transaction, "allowsPipelineSelection", label)?
        || string_field(transaction, "pipelineEntry", label)? != PIPELINE_ENTRY
        || u64_field(transaction, "policyVersion", label)? != 4
        || string_field(transaction, "status", label)? != "sealed-production-complete"
    {
        return Err(format!(
            "{label} did not use the sole production transaction"
        ));
    }
    let value = required(transaction, "transactionSha256", label)?;
    require_sha256(
        value,
        &format!("{label}.productionTransaction.transactionSha256"),
    )
}

#[derive(Debug)]
struct Snapshot {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

fn verify_record(
    repository: &Path,
    evidence_root: &Path,
    candidate: &Map<String, Value>,
    record: &Map<String, Value>,
    fixture: &ManifestFixture,
    label: &str,
) -> Result<String, String> {
    let production = object(required(record, "productionEvidence", label)?, label)?;
    exact_keys(
        production,
        PRODUCTION_EVIDENCE_KEYS,
        &format!("{label}.productionEvidence"),
    )?;
    if string_field(production, "compilerCommit", label)?
        != string_field(candidate, "compilerCommit", "qualification batch.candidate")?
        || string_field(production, "compilerTree", label)?
            != string_field(candidate, "compilerTree", "qualification batch.candidate")?
    {
        return Err(format!("{label} compiler candidate is substituted"));
    }
    for key in PRODUCTION_EVIDENCE_KEYS {
        if *key != "compilerCommit" && *key != "compilerTree" {
            require_sha256(
                required(production, key, label)?,
                &format!("{label}.productionEvidence.{key}"),
            )?;
        }
    }

    let references = object(required(record, "evidenceFiles", label)?, label)?;
    exact_keys(
        references,
        EVIDENCE_KINDS,
        &format!("{label}.evidenceFiles"),
    )?;
    let mut snapshots = BTreeMap::new();
    let mut total = 0_u64;
    for kind in EVIDENCE_KINDS {
        let snapshot = snapshot_reference(
            evidence_root,
            required(references, kind, label)?,
            &format!("{label}.evidenceFiles.{kind}"),
        )?;
        total = total
            .checked_add(snapshot.bytes)
            .ok_or_else(|| format!("{label} evidence byte total overflowed"))?;
        if total > MAX_EVIDENCE_TOTAL_BYTES {
            return Err(format!("{label} evidence exceeds the aggregate byte bound"));
        }
        snapshots.insert((*kind).to_owned(), snapshot);
    }
    validate_archive_hash_claims(production, &snapshots, label)?;

    let sealed_bytes = read_snapshot(
        snapshot(&snapshots, "sealed-production-receipt", label)?,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} sealed production result"),
    )?;
    let result = InertProductionCapabilityResultV5::decode(&sealed_bytes)
        .map_err(|error| format!("{label} sealed V5 production result failed: {error}"))?;
    if result.canonical_bytes() != sealed_bytes {
        return Err(format!(
            "{label} sealed V5 production result is noncanonical"
        ));
    }
    let handoff = result.handoff();
    let capsule = handoff.legacy_handoff().capsule();
    let receipts = capsule.receipts();

    let kir_bytes = read_snapshot(
        snapshot(&snapshots, "optimized-kir-v13", label)?,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} optimized KIR V13"),
    )?;
    if kir_bytes != handoff.executable_kir().canonical_preimage() {
        return Err(format!("{label} optimized KIR was substituted"));
    }
    let (verified_kir, module) =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(kir_bytes.clone())
            .map_err(|error| format!("{label} optimized KIR V13 failed: {error}"))?;
    verified_kir
        .revalidate()
        .map_err(|error| format!("{label} optimized KIR V13 revalidation failed: {error}"))?;

    let bundle_bytes = read_snapshot(
        snapshot(&snapshots, "simulation-bundle-v8", label)?,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} simulation Bundle V8"),
    )?;
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(bundle_bytes)
        .map_err(|error| format!("{label} simulation Bundle V8 failed: {error}"))?;
    bundle
        .revalidate()
        .map_err(|error| format!("{label} simulation Bundle V8 revalidation failed: {error}"))?;
    if bundle.canonical_kir_v13() != verified_kir.canonical_bytes()
        || bundle.canonical_kir_v13_digest() != verified_kir.identity().digest()
        || bundle.canonical_kir_v13_length() != verified_kir.identity().canonical_length()
        || bundle.production_kir_identity().digest() != *verified_kir.identity().digest()
        || bundle.production_kir_identity().canonical_length()
            != verified_kir.identity().canonical_length()
        || bundle.final_graph_epoch() != handoff.final_graph_report().final_epoch()
        || handoff.final_graph_report().final_graph() != *verified_kir.identity().digest()
    {
        return Err(format!(
            "{label} Bundle V8 and final production graph differ"
        ));
    }
    if bundle.target() != fixture.target.as_str()
        || capsule.target().as_amd_target_id().processor() != fixture.target.as_str()
    {
        return Err(format!("{label} target custody is substituted"));
    }

    let semantic_mir = read_snapshot(
        snapshot(&snapshots, "semantic-mir", label)?,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} semantic MIR"),
    )?;
    if semantic_mir != bundle.semantic_mir()
        || semantic_mir != receipts.semantic_mir().canonical_preimage()
        || sha256(&semantic_mir) != handoff.inputs().semantic_mir_identity()
    {
        return Err(format!("{label} semantic MIR custody is substituted"));
    }

    let source_owner = validate_compiler_proof_inputs_v4(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .map_err(|error| format!("{label} source proof association failed: {error}"))?;
    let _target_lineage = if handoff.subjects().len() == 1 {
        Some(
            validate_compiler_target_lineage_v1(capsule, &source_owner)
                .map_err(|error| format!("{label} target lineage failed: {error}"))?,
        )
    } else {
        None
    };

    let kernel_ordinal = module
        .kernels
        .iter()
        .position(|kernel| {
            kernel.id.as_str() == fixture.kernel_symbol
                || kernel.entry.as_str() == fixture.kernel_symbol
        })
        .ok_or_else(|| format!("{label} kernel symbol is absent from exact KIR V13"))?;
    if handoff.subjects().len() != module.kernels.len()
        || handoff.obligation_roster().len() != module.kernels.len()
        || result.capability_associations().entries().len() != module.kernels.len()
    {
        return Err(format!(
            "{label} KIR, subject, obligation, and result rosters differ"
        ));
    }
    let mut validated_capabilities = Vec::with_capacity(module.kernels.len());
    for index in 0..module.kernels.len() {
        let subject = handoff.subjects()[index];
        let obligations = &handoff.obligation_roster()[index];
        let association = &result.capability_associations().entries()[index];
        let source = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
            InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            handoff.source_refinement().canonical_preimage().to_vec(),
        )
        .map_err(|error| format!("{label} source refinement failed: {error}"))?;
        let machine = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
            InertCapabilityRefinementReceiptKindV1::Machine,
            result.machine_refinement().canonical_preimage().to_vec(),
        )
        .map_err(|error| format!("{label} machine refinement failed: {error}"))?;
        validated_capabilities.push(
            validate_compiler_capability_evidence_v1(
                association.canonical_bytes(),
                capsule,
                handoff.executable_kir(),
                subject,
                obligations.identity(),
                Some(source),
                Some(machine),
            )
            .map_err(|error| format!("{label} capability result {index} failed: {error}"))?,
        );
    }
    let selected_capability = validated_capabilities.swap_remove(kernel_ordinal);
    let proof_owner = validate_compiler_proof_inputs_v5(
        result.proof_owner().canonical_bytes(),
        &source_owner,
        handoff.executable_kir(),
        selected_capability,
        handoff.inputs().compiler_policy(),
    )
    .map_err(|error| format!("{label} native V5 proof owner failed: {error}"))?;
    let _capability_source_owner = validate_compiler_capability_source_owner_v1(
        proof_owner,
        &result,
        &source_owner,
        handoff.inputs().compiler_policy(),
    )
    .map_err(|error| format!("{label} V5 production result ownership failed: {error}"))?;

    ProductionW4CanonicalEncodingV1::decode(handoff.final_graph_report().canonical_results())
        .map_err(|error| format!("{label} final W4 report failed: {error}"))?;
    require_exact_archive(
        &snapshots,
        "capability-analysis",
        handoff.final_graph_report().canonical_results(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "proof-obligation-set",
        handoff.obligation_roster()[kernel_ordinal].canonical_bytes(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "proof-evidence",
        result.capability_associations().entries()[kernel_ordinal].result_set_bytes(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "source-mir-to-kir-refinement",
        handoff.source_refinement().canonical_preimage(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "machine-refinement",
        result.machine_refinement().canonical_preimage(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "lowering",
        receipts.amdgpu_lowering().canonical_preimage(),
        label,
    )?;
    require_exact_archive(
        &snapshots,
        "llvm-module",
        handoff.legacy_handoff().module_handoff().module_bytes(),
        label,
    )?;
    let association = &result.capability_associations().entries()[kernel_ordinal];
    let checker_bytes = read_snapshot(
        snapshot(&snapshots, "proof-checker", label)?,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} proof checker evidence"),
    )?;
    let checker_identity =
        authenticated_compiler_capability_evidence_identity_v5(&checker_bytes)
            .map_err(|error| format!("{label} proof checker identity failed: {error}"))?;
    let typed_identities = [
        (
            "loweringIdentitySha256",
            *receipts.amdgpu_lowering().identity().sha256(),
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
        if decode_sha256(string_field(production, claim, label)?)? != expected {
            return Err(format!(
                "{label} typed identity {claim} differs from its canonical receipt"
            ));
        }
    }
    validate_result_measurements(&result, &snapshots, label)?;
    validate_graph_claims(record, production, &bundle, &verified_kir, capsule, label)?;
    validate_subject_claims(
        record,
        production,
        handoff,
        kernel_ordinal,
        &snapshots,
        label,
    )?;
    validate_compiler_input(record, repository, fixture, &snapshots, label)?;
    validate_status_records(record, production, &snapshots, label)?;
    Ok(snapshot(&snapshots, "sealed-production-receipt", label)?
        .sha256
        .clone())
}

fn validate_archive_hash_claims(
    production: &Map<String, Value>,
    snapshots: &BTreeMap<String, Snapshot>,
    label: &str,
) -> Result<(), String> {
    const JOINS: &[(&str, &str)] = &[
        ("artifactSha256", "artifact"),
        ("artifactInspectionSha256", "artifact-inspection"),
        ("capabilityAnalysisSha256", "capability-analysis"),
        ("capabilityClosureSha256", "capability-closure"),
        ("compilerPolicySha256", "compiler-policy"),
        ("hardwareEvidenceSha256", "hardware"),
        ("hostAdmissionSha256", "host-admission"),
        ("launchContractSha256", "launch-contract"),
        ("negativeFixtureSetSha256", "negative-fixture-set"),
        ("numericalPolicySha256", "numerical-policy"),
        ("simulatorEvidenceSha256", "simulator"),
        (
            "targetCapabilityDecisionSha256",
            "target-capability-decision",
        ),
        ("targetIdentitySha256", "target-identity"),
    ];
    for (claim, archive) in JOINS {
        if string_field(production, claim, label)? != snapshot(snapshots, archive, label)?.sha256 {
            return Err(format!("{label} {claim} archive was substituted"));
        }
    }
    Ok(())
}

fn validate_result_measurements(
    result: &InertProductionCapabilityResultV5,
    snapshots: &BTreeMap<String, Snapshot>,
    label: &str,
) -> Result<(), String> {
    let llvm = snapshot(snapshots, "llvm-module", label)?;
    let object = snapshot(snapshots, "artifact", label)?;
    if result.llvm_output().output_sha256() != decode_sha256(&llvm.sha256)?
        || result.llvm_output().output_bytes() != llvm.bytes
        || result.object_output().output_sha256() != decode_sha256(&object.sha256)?
        || result.object_output().output_bytes() != object.bytes
    {
        return Err(format!(
            "{label} compiler output measurement was substituted"
        ));
    }
    Ok(())
}

fn validate_graph_claims(
    record: &Map<String, Value>,
    production: &Map<String, Value>,
    bundle: &VerifiedSimulationBundleV8,
    kir: &VerifiedCanonicalKernelIrV13,
    capsule: &fe2o3_compiler_lineage::InertProductionSemanticCapsuleV3,
    label: &str,
) -> Result<(), String> {
    let graph = object(required(record, "graph", label)?, label)?;
    exact_keys(graph, GRAPH_KEYS, &format!("{label}.graph"))?;
    let lineage = bundle.source_lineage();
    let receipts = capsule.receipts();
    let checks: &[(&str, [u8; 32])] = &[
        ("bundleContentIdentitySha256", *bundle.identity().as_bytes()),
        ("bundleSubjectIdentitySha256", *bundle.subject_identity()),
        ("kernelAbiIdentitySha256", *bundle.kernel_abi_identity()),
        ("productionKirIdentitySha256", *kir.identity().digest()),
        ("semanticMirIdentitySha256", bundle.semantic_mir_identity()),
        (
            "sourceInventoryReceiptSha256",
            lineage.rustc_identity_inventory_receipt_sha256(),
        ),
        (
            "sourcePreflightReceiptSha256",
            lineage.rustc_preflight_plan_receipt_sha256(),
        ),
    ];
    for (field, expected) in checks {
        if string_field(graph, field, label)? != encode_hex(expected) {
            return Err(format!("{label} graph.{field} was substituted"));
        }
    }
    if lineage.rustc_identity_inventory_receipt_sha256()
        != *receipts.rustc_identity_inventory().identity().sha256()
        || lineage.rustc_identity_inventory_receipt_bytes()
            != receipts.rustc_identity_inventory().identity().byte_len()
        || lineage.rustc_preflight_plan_receipt_sha256()
            != *receipts.rustc_preflight_plan().identity().sha256()
        || lineage.rustc_preflight_plan_receipt_bytes()
            != receipts.rustc_preflight_plan().identity().byte_len()
    {
        return Err(format!("{label} Bundle V8 source lineage was substituted"));
    }
    if u64_field(graph, "canonicalKirVersion", label)? != 13
        || u64_field(graph, "canonicalKirBytes", label)? != kir.identity().canonical_length()
        || u64_field(graph, "finalGraphEpoch", label)? != bundle.final_graph_epoch()
        || u64_field(graph, "kernelCount", label)? != u64::from(bundle.kernel_count())
        || string_field(production, "finalOptimizedKirSha256", label)?
            != encode_hex(kir.identity().digest())
        || string_field(production, "sourceMirIdentitySha256", label)?
            != encode_hex(&bundle.semantic_mir_identity())
    {
        return Err(format!("{label} final graph coordinates were substituted"));
    }
    Ok(())
}

fn validate_subject_claims(
    record: &Map<String, Value>,
    production: &Map<String, Value>,
    handoff: &fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5,
    kernel_ordinal: usize,
    snapshots: &BTreeMap<String, Snapshot>,
    label: &str,
) -> Result<(), String> {
    let subject = handoff.subjects()[kernel_ordinal];
    let compiler_policy = snapshot(snapshots, "compiler-policy", label)?;
    let closure = snapshot(snapshots, "capability-closure", label)?;
    let target_decision = snapshot(snapshots, "target-capability-decision", label)?;
    let target_identity = snapshot(snapshots, "target-identity", label)?;
    let launch = snapshot(snapshots, "launch-contract", label)?;
    if decode_sha256(&compiler_policy.sha256)? != handoff.inputs().compiler_policy()
        || decode_sha256(&closure.sha256)? != handoff.target_closure().closure_identity()
        || decode_sha256(&target_identity.sha256)? != *subject.target_model().digest().as_bytes()
        || decode_sha256(&launch.sha256)? != *subject.launch_contract().digest().as_bytes()
    {
        return Err(format!(
            "{label} policy, target, launch, or capability closure differs"
        ));
    }
    let closure_record = object(required(record, "capabilityClosure", label)?, label)?;
    if string_field(closure_record, "status", label)? != "complete"
        || string_field(closure_record, "sha256", label)? != closure.sha256
        || string_field(production, "capabilityClosureSha256", label)? != closure.sha256
    {
        return Err(format!("{label} capability closure claim differs"));
    }
    let target_record = object(required(record, "targetDecision", label)?, label)?;
    if string_field(target_record, "status", label)? != "capability-complete"
        || string_field(target_record, "capabilityDecisionSha256", label)? != target_decision.sha256
        || string_field(target_record, "targetIdentitySha256", label)? != target_identity.sha256
    {
        return Err(format!("{label} target decision claim differs"));
    }
    Ok(())
}

fn validate_compiler_input(
    record: &Map<String, Value>,
    repository: &Path,
    fixture: &ManifestFixture,
    snapshots: &BTreeMap<String, Snapshot>,
    label: &str,
) -> Result<(), String> {
    let input = object(required(record, "compilerInput", label)?, label)?;
    exact_keys(
        input,
        &[
            "cargoLockSha256",
            "contractSha256",
            "packageManifestSha256",
            "sourceClosureSha256",
        ],
        &format!("{label}.compilerInput"),
    )?;
    for key in [
        "cargoLockSha256",
        "contractSha256",
        "packageManifestSha256",
        "sourceClosureSha256",
    ] {
        require_sha256(
            required(input, key, label)?,
            &format!("{label}.compilerInput.{key}"),
        )?;
    }
    if string_field(input, "sourceClosureSha256", label)?
        != snapshot(snapshots, "source-closure", label)?.sha256
        || string_field(input, "contractSha256", label)?
            != snapshot(snapshots, "compiler-input", label)?.sha256
    {
        return Err(format!("{label} compiler input archive differs"));
    }
    if string_field(input, "cargoLockSha256", label)? != fixture.cargo_lock_sha256
        || string_field(input, "packageManifestSha256", label)? != fixture.package_manifest_sha256
        || string_field(input, "sourceClosureSha256", label)? != fixture.source_closure_sha256
        || string_field(input, "contractSha256", label)? != fixture.contract_sha256
    {
        return Err(format!(
            "{label} compiler input differs from the tutorial manifest"
        ));
    }
    let cargo_lock = read_bounded_file(
        &protected_repository_file(repository, &fixture.cargo_lock_path, "Cargo.lock")?,
        16 * 1024 * 1024,
        "Cargo.lock",
    )?;
    if hex_sha256(&cargo_lock) != string_field(input, "cargoLockSha256", label)? {
        return Err(format!("{label} Cargo.lock identity is stale"));
    }
    let package_manifest = read_bounded_file(
        &protected_repository_file(repository, &fixture.package_manifest, "package manifest")?,
        16 * 1024 * 1024,
        "package manifest",
    )?;
    if hex_sha256(&package_manifest) != string_field(input, "packageManifestSha256", label)? {
        return Err(format!("{label} package manifest identity is stale"));
    }
    Ok(())
}

fn protected_repository_file(root: &Path, relative: &str, label: &str) -> Result<PathBuf, String> {
    let path = protected_relative_file(root, relative, label)?;
    real_file(&path, label)
}

fn validate_status_records(
    record: &Map<String, Value>,
    production: &Map<String, Value>,
    snapshots: &BTreeMap<String, Snapshot>,
    label: &str,
) -> Result<(), String> {
    let proof = object(required(record, "proof", label)?, label)?;
    if string_field(proof, "status", label)? != "complete"
        || string_field(proof, "checkerSha256", label)?
            != string_field(production, "proofCheckerSha256", label)?
        || string_field(proof, "evidenceSha256", label)?
            != string_field(production, "proofEvidenceSha256", label)?
        || string_field(proof, "obligationSetSha256", label)?
            != string_field(production, "proofObligationSetSha256", label)?
    {
        return Err(format!(
            "{label} proof summary differs from typed proof archives"
        ));
    }
    for field in ["simulator", "hardware"] {
        let value = object(required(record, field, label)?, label)?;
        if string_field(value, "status", label)? != "passed"
            || string_field(value, "evidenceSha256", label)?
                != snapshot(snapshots, field, label)?.sha256
        {
            return Err(format!("{label} {field} evidence is absent or substituted"));
        }
    }
    Ok(())
}

fn snapshot_reference(root: &Path, value: &Value, label: &str) -> Result<Snapshot, String> {
    let reference = object(value, label)?;
    exact_keys(reference, &["bytes", "path", "sha256"], label)?;
    let expected_bytes = u64_field(reference, "bytes", label)?;
    if expected_bytes == 0 || expected_bytes > MAX_EVIDENCE_FILE_BYTES {
        return Err(format!("{label}.bytes is outside the evidence bound"));
    }
    let expected_sha = require_sha256(required(reference, "sha256", label)?, label)?.to_owned();
    let relative = string_field(reference, "path", label)?;
    let path = protected_relative_file(root, relative, label)?;
    let mut file =
        open_no_follow(&path).map_err(|error| format!("cannot open {label}: {error}"))?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !before.is_file() || before.len() != expected_bytes {
        return Err(format!("{label} is not the claimed regular file"));
    }
    let mut digest = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {label}: {error}"))?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or_else(|| format!("{label} length overflowed"))?;
        digest.update(&buffer[..count]);
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot reinspect {label}: {error}"))?;
    if observed != expected_bytes || !same_file_state(&before, &after) {
        return Err(format!("{label} changed during verification"));
    }
    let sha256 = encode_hex(&digest.finalize());
    if sha256 != expected_sha {
        return Err(format!("{label} content digest differs"));
    }
    Ok(Snapshot {
        path,
        bytes: expected_bytes,
        sha256,
    })
}

fn require_exact_archive(
    snapshots: &BTreeMap<String, Snapshot>,
    kind: &str,
    expected: &[u8],
    label: &str,
) -> Result<(), String> {
    let snapshot = snapshot(snapshots, kind, label)?;
    let actual = read_snapshot(
        snapshot,
        MAX_TYPED_EVIDENCE_BYTES,
        &format!("{label} {kind}"),
    )?;
    if actual != expected {
        return Err(format!(
            "{label} {kind} archive differs from compiler custody"
        ));
    }
    Ok(())
}

fn read_snapshot(snapshot: &Snapshot, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
    if snapshot.bytes > maximum {
        return Err(format!("{label} exceeds the typed evidence bound"));
    }
    let bytes = read_bounded_file(&snapshot.path, maximum, label)?;
    if bytes.len() as u64 != snapshot.bytes || hex_sha256(&bytes) != snapshot.sha256 {
        return Err(format!("{label} changed after its evidence snapshot"));
    }
    Ok(bytes)
}

fn snapshot<'a>(
    snapshots: &'a BTreeMap<String, Snapshot>,
    kind: &str,
    label: &str,
) -> Result<&'a Snapshot, String> {
    snapshots
        .get(kind)
        .ok_or_else(|| format!("{label} omits {kind} evidence"))
}

fn protected_relative_file(root: &Path, relative: &str, label: &str) -> Result<PathBuf, String> {
    if relative.is_empty() || !relative.is_ascii() || relative.contains('\\') {
        return Err(format!("{label}.path is not a portable relative path"));
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label}.path escapes the evidence root"));
    }
    let mut joined = root.to_path_buf();
    let components = path.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("components were restricted above")
        };
        joined.push(component);
        let metadata = fs::symlink_metadata(&joined)
            .map_err(|error| format!("cannot inspect {label}.path: {error}"))?;
        if metadata.file_type().is_symlink() || (index + 1 < components.len() && !metadata.is_dir())
        {
            return Err(format!("{label}.path traverses a symlink or non-directory"));
        }
    }
    Ok(joined)
}

fn real_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a real directory"));
    }
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {label}: {error}"))
}

fn real_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a real regular file"));
    }
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {label}: {error}"))
}

fn read_bounded_file(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
    let mut file = open_no_follow(path).map_err(|error| format!("cannot open {label}: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(format!("{label} has an invalid byte length"));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| format!("{label} cannot fit in the address space"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| format!("cannot allocate {label}"))?;
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    if bytes.len() as u64 != metadata.len() {
        return Err(format!("{label} changed while it was read"));
    }
    Ok(bytes)
}

fn open_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    options.open(path)
}

#[cfg(unix)]
fn same_file_state(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    (
        before.dev(),
        before.ino(),
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) == (
        after.dev(),
        after.ino(),
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    )
}

#[cfg(not(unix))]
fn same_file_state(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    before.len() == after.len() && before.modified().ok() == after.modified().ok()
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "git output is not UTF-8".to_owned())
}

fn executable_identity() -> Result<String, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("cannot identify verifier executable: {error}"))?;
    let executable = real_file(&executable, "verifier executable")?;
    let bytes = read_bounded_file(&executable, 1024 * 1024 * 1024, "verifier executable")?;
    Ok(hex_sha256(&bytes))
}

fn binding_sha256(value: &Value, field: &str, domain: &[u8]) -> Result<String, String> {
    let mut subject = object(value, "binding subject")?.clone();
    subject.remove(field);
    let canonical = canonical_json(&Value::Object(subject))?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical);
    Ok(encode_hex(&digest.finalize()))
}

fn domain_sha256(domain: &[u8], value: &Value) -> Result<String, String> {
    let canonical = canonical_json(value)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical);
    Ok(encode_hex(&digest.finalize()))
}

fn parse_unique_json(bytes: &[u8], label: &str) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueValue::deserialize(&mut deserializer)
        .map_err(|error| format!("cannot decode {label}: {error}"))?
        .0;
    deserializer
        .end()
        .map_err(|error| format!("cannot decode {label}: {error}"))?;
    validate_json_domain(&value, label)?;
    Ok(value)
}

fn canonical_json(value: &Value) -> Result<Vec<u8>, String> {
    validate_json_domain(value, "canonical JSON")?;
    let mut output = Vec::new();
    encode_canonical_json(value, &mut output)?;
    Ok(output)
}

fn encode_canonical_json(value: &Value, output: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(value) => output.extend_from_slice(
            serde_json::to_string(value)
                .map_err(|error| format!("cannot encode canonical JSON string: {error}"))?
                .as_bytes(),
        ),
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
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
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
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

fn validate_json_domain(value: &Value, label: &str) -> Result<(), String> {
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => Ok(()),
        Value::Number(_) => Err(format!("{label} contains a non-integer number")),
        Value::String(value) if value.is_ascii() => Ok(()),
        Value::String(_) => Err(format!("{label} contains non-ASCII text")),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_json_domain(value, label)),
        Value::Object(values) => values.iter().try_for_each(|(key, value)| {
            if !key.is_ascii() {
                return Err(format!("{label} contains a non-ASCII object key"));
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

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))
}

fn array<'a>(value: &'a Value, label: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} must be an array"))
}

fn required<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a Value, String> {
    object
        .get(field)
        .ok_or_else(|| format!("{label} omits {field}"))
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str, String> {
    required(object, field, label)?
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label}.{field} must be a nonempty string"))
}

fn bool_field(object: &Map<String, Value>, field: &str, label: &str) -> Result<bool, String> {
    required(object, field, label)?
        .as_bool()
        .ok_or_else(|| format!("{label}.{field} must be a Boolean"))
}

fn u64_field(object: &Map<String, Value>, field: &str, label: &str) -> Result<u64, String> {
    required(object, field, label)?
        .as_u64()
        .ok_or_else(|| format!("{label}.{field} must be a nonnegative integer"))
}

fn expect_string(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
    label: &str,
) -> Result<(), String> {
    if string_field(object, field, label)? != expected {
        return Err(format!("{label}.{field} must be {expected:?}"));
    }
    Ok(())
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str], label: &str) -> Result<(), String> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{label} has missing or extra fields"));
    }
    Ok(())
}

fn require_sha256<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("{label} must be a SHA-256 string"))?;
    if value.len() != 64
        || value == "0000000000000000000000000000000000000000000000000000000000000000"
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} is not a nonzero lowercase SHA-256 identity"
        ));
    }
    Ok(value)
}

fn require_git_identity(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 40
        || value == "0000000000000000000000000000000000000000"
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} is not a nonzero lowercase Git identity"));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_sha256(bytes: &[u8]) -> String {
    encode_hex(&Sha256::digest(bytes))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("invalid SHA-256 identity".to_owned());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("invalid lowercase hexadecimal identity".to_owned()),
    }
}

fn object_value<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(seed: char) -> String {
        std::iter::repeat(seed).take(64).collect()
    }

    fn minimal_record(fixture: &str, transaction: &str) -> Value {
        object_value([
            ("capabilityClosure", object_value([])),
            ("compilerInput", object_value([])),
            ("evidenceFiles", object_value([])),
            ("fixtureId", Value::String(fixture.to_owned())),
            ("graph", object_value([])),
            ("hardware", object_value([])),
            ("kernelSymbol", Value::String("kernel".to_owned())),
            ("lessonIds", Value::Array(vec![])),
            ("negativeFixtures", object_value([])),
            ("productionEvidence", object_value([])),
            (
                "productionTransaction",
                object_value([
                    ("allowsFallback", Value::Bool(false)),
                    ("allowsPipelineSelection", Value::Bool(false)),
                    ("pipelineEntry", Value::String(PIPELINE_ENTRY.to_owned())),
                    ("policyVersion", Value::Number(4_u64.into())),
                    (
                        "status",
                        Value::String("sealed-production-complete".to_owned()),
                    ),
                    ("transactionSha256", Value::String(transaction.to_owned())),
                ]),
            ),
            ("proof", object_value([])),
            ("recordBindingSha256", Value::String(identity('a'))),
            ("simulator", object_value([])),
            ("target", Value::String("gfx942".to_owned())),
            ("targetDecision", object_value([])),
        ])
    }

    #[test]
    fn options_are_closed_and_required() {
        let options = parse_options(vec![
            "--repository".into(),
            "repo".into(),
            "--batch".into(),
            "batch".into(),
            "--evidence-root".into(),
            "evidence".into(),
        ])
        .unwrap();
        assert_eq!(options.repository, PathBuf::from("repo"));
        assert!(parse_options(vec!["--repository".into(), "repo".into()]).is_err());
        assert!(parse_options(vec!["--unknown".into(), "value".into()]).is_err());
    }

    #[test]
    fn help_text_is_exact_and_contains_no_patch_artifacts() {
        assert_eq!(
            usage(),
            "usage: fe2o3-verify-tutorial-production-batch-v1 \\\n       --repository <compiler-repository> --batch <batch.json> \\\n       --evidence-root <evidence-directory>"
        );
        assert!(!usage().contains('+'));
    }

    #[test]
    fn duplicate_json_keys_and_noncanonical_number_domain_are_rejected() {
        assert!(parse_unique_json(br#"{"a":1,"a":2}"#, "fixture").is_err());
        assert!(parse_unique_json(br#"{"a":1.25}"#, "fixture").is_err());
        assert!(parse_unique_json("{\"a\":\"snowman ☃\"}".as_bytes(), "fixture").is_err());
    }

    #[test]
    fn bindings_reject_substitution() {
        let mut record = minimal_record("fixture", &identity('b'));
        let binding = binding_sha256(&record, "recordBindingSha256", RECORD_DOMAIN).unwrap();
        record.as_object_mut().unwrap().insert(
            "recordBindingSha256".to_owned(),
            Value::String(binding.clone()),
        );
        assert_eq!(
            binding_sha256(&record, "recordBindingSha256", RECORD_DOMAIN).unwrap(),
            binding
        );
        record
            .as_object_mut()
            .unwrap()
            .insert("target".to_owned(), Value::String("gfx950".to_owned()));
        assert_ne!(
            binding_sha256(&record, "recordBindingSha256", RECORD_DOMAIN).unwrap(),
            binding
        );
    }

    #[test]
    fn transaction_policy_rejects_fallback_and_missing_authority() {
        let record = minimal_record("fixture", &identity('c'));
        let record = record.as_object().unwrap();
        assert_eq!(
            validate_transaction(record, "record").unwrap(),
            identity('c')
        );
        let mut fallback = record.clone();
        fallback
            .get_mut("productionTransaction")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("allowsFallback".to_owned(), Value::Bool(true));
        assert!(validate_transaction(&fallback, "record").is_err());
        let mut missing = record.clone();
        missing
            .get_mut("productionTransaction")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("transactionSha256");
        assert!(validate_transaction(&missing, "record").is_err());
    }

    #[test]
    fn duplicate_transaction_and_result_sets_are_detected_deterministically() {
        let transaction = identity('d');
        let mut transactions = BTreeSet::new();
        assert!(transactions.insert(transaction.clone()));
        assert!(!transactions.insert(transaction));
        let result = identity('e');
        let mut results = BTreeSet::new();
        assert!(results.insert(result.clone()));
        assert!(!results.insert(result));
    }

    #[test]
    fn path_traversal_is_rejected_before_io() {
        let root = Path::new("/tmp");
        for hostile in [
            "../receipt",
            "/receipt",
            "dir/../receipt",
            "dir\\receipt",
            ".",
        ] {
            assert!(protected_relative_file(root, hostile, "evidence").is_err());
        }
    }

    #[test]
    fn malformed_v5_and_v8_authority_are_rejected_by_typed_decoders() {
        assert!(InertProductionCapabilityResultV5::decode(b"not-a-v5-result").is_err());
        assert!(VerifiedSimulationBundleV8::from_canonical_bytes(b"not-v8".to_vec()).is_err());
        assert!(ProductionW4CanonicalEncodingV1::decode(b"not-w4").is_err());
    }

    #[test]
    fn report_is_canonical_and_explicitly_authority_free() {
        let sealed = object_value([
            ("bytes", Value::Number(1_u64.into())),
            ("path", Value::String("sealed".to_owned())),
            ("sha256", Value::String(identity('1'))),
        ]);
        let bundle = object_value([
            ("bytes", Value::Number(1_u64.into())),
            ("path", Value::String("bundle".to_owned())),
            ("sha256", Value::String(identity('2'))),
        ]);
        let mut record = minimal_record("fixture", &identity('3'));
        let record_object = record.as_object_mut().unwrap();
        record_object.insert(
            "evidenceFiles".to_owned(),
            object_value([
                ("sealed-production-receipt", sealed),
                ("simulation-bundle-v8", bundle),
            ]),
        );
        let batch = object_value([
            ("batchBindingSha256", Value::String(identity('4'))),
            (
                "candidate",
                object_value([
                    ("compilerCommit", Value::String("1".repeat(40))),
                    ("compilerTree", Value::String("2".repeat(40))),
                    ("worktreeClean", Value::Bool(true)),
                ]),
            ),
            (
                "manifest",
                object_value([
                    ("corpusContractSha256", Value::String(identity('5'))),
                    ("path", Value::String(MANIFEST_PATH.to_owned())),
                    ("rawSha256", Value::String(identity('6'))),
                ]),
            ),
            ("records", Value::Array(vec![record])),
            ("roadmapIssue", Value::String(ROADMAP_ISSUE.to_owned())),
            ("schema", Value::String(BATCH_SCHEMA.to_owned())),
            ("semanticQualification", object_value([])),
        ]);
        let report = expected_report(&batch).unwrap();
        assert_eq!(
            report["authority"],
            Value::String("verification-only-no-runtime-authority".to_owned())
        );
        let bytes = canonical_json(&report).unwrap();
        assert_eq!(parse_unique_json(&bytes, "report").unwrap(), report);
    }
}
