use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV2, BuildAttempt,
    BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1,
    DurablePublishedClaimCodecErrorV1, DurablePublishedClaimCodecErrorV2,
    DurablePublishedClaimReacquisitionErrorV2, DurablePublishedClaimReceiptFieldV2,
    DurablePublishedHsacoClaimV1, DurablePublishedHsacoClaimV2, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2, PackageIdentityV1, PinnedWorkerIdentityV1,
    ProducerIdentity, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, begin_build_attempt, finish_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1, publish_exact_hsaco_evidence_for_attempt_v2,
    reacquire_current_hsaco_publication_lease_v2, recover_published_hsaco_claim_for_attempt_v2,
};
use fe2o3_build_authority::{
    CompilerClosureDigestFieldV2, CompilerClosureErrorV2, CompilerClosureV2,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const CLAIM_MAGIC_V2: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V2\0";
const CLAIM_CHECKSUM_DOMAIN_V2: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v2\0";
const ATTEMPT_REGISTRY: &str = ".fe2o3-attempts-v1";
const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
const ARTIFACT_SUFFIX: &str = ".bin";
const ATTEMPT_BYTES: usize = 8 + 16 + 32;
const SCOPE_BYTES: usize = 3 * 32;
const PLAN_IDENTITY_BYTES: usize = 7 * 32;
const RECEIPT_IDENTITY_BYTES: usize = 7 * 32;
const CLOSURE_PIN_BYTES: usize = 6 * 32;
const CLOSURE_BYTES: usize = CLOSURE_PIN_BYTES + 2 + 32;

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-durable-claim-v2-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn output(&self) -> PathBuf {
        self.path.join("output")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn producer(crate_name: &str, source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(crate_name, Some(Path::new(source))).unwrap()
}

fn begin(output: &Path, owner: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        owner,
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32]),
        BuildSession::from_bytes([seed; 16]),
    )
    .unwrap()
}

fn fake_attempt(attempt: BuildAttempt, seed: u8) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        attempt.generation(),
        BuildSession::from_bytes([seed; 16]),
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32])
    ))
    .unwrap()
}

fn scope(seed: u8) -> LinkPublicationScopeV1 {
    LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes(identity(seed)),
        KernelSetIdentityV1::from_bytes(identity(seed.wrapping_add(1))),
        TargetIdentityV1::from_bytes(identity(seed.wrapping_add(2))),
    )
}

fn plan(
    attempt: BuildAttempt,
    publication_scope: LinkPublicationScopeV1,
    seed: u8,
    bytes: &[u8],
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        publication_scope,
        CanonicalLinkRequestIdentityV1::from_bytes(identity(seed)),
        PinnedWorkerIdentityV1::from_bytes(identity(seed.wrapping_add(1))),
        ValidatedResponseIdentityV1::from_bytes(identity(seed.wrapping_add(2))),
        LinkedOutputIdentityV1::from_bytes(identity(seed.wrapping_add(3))),
        FinalizationIdentityV1::from_bytes(identity(seed.wrapping_add(4))),
        FinalizedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        AtomicPublicationIdentityV1::from_bytes(identity(seed.wrapping_add(5))),
    )
}

fn upstream(plan: DurableLinkPublicationPlanV1) -> UpstreamCodeObjectEvidenceIdentityV1 {
    UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
        Sha256::digest(plan.finalization().as_bytes()).into(),
    )
}

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        identity(seed),
        identity(seed.wrapping_add(1)),
        identity(seed.wrapping_add(2)),
        identity(seed.wrapping_add(3)),
        identity(seed.wrapping_add(4)),
        identity(seed.wrapping_add(5)),
    )
    .unwrap()
}

fn closure_pins(closure: CompilerClosureV2) -> [[u8; 32]; 6] {
    [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ]
}

fn closure_bytes(closure: CompilerClosureV2) -> [u8; CLOSURE_BYTES] {
    let mut bytes = [0_u8; CLOSURE_BYTES];
    for (index, pin) in closure_pins(closure).into_iter().enumerate() {
        bytes[index * 32..(index + 1) * 32].copy_from_slice(&pin);
    }
    bytes[CLOSURE_PIN_BYTES..CLOSURE_PIN_BYTES + 2].copy_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes[CLOSURE_PIN_BYTES + 2..].copy_from_slice(&closure.identity_sha256());
    bytes
}

fn substitute_closure_role(
    original: CompilerClosureV2,
    alternate: CompilerClosureV2,
    role: usize,
) -> CompilerClosureV2 {
    let mut pins = closure_pins(original);
    pins[role] = closure_pins(alternate)[role];
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
}

fn publish_v2(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
) -> fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV2 {
    publish_exact_hsaco_evidence_for_attempt_v2(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        closure,
        bytes,
    )
    .unwrap()
}

fn publish_v1(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
) -> fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV1 {
    publish_exact_hsaco_evidence_for_attempt_v1(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        bytes,
    )
    .unwrap()
}

fn receipt_offset() -> usize {
    CLAIM_MAGIC_V2.len() + 2 + ATTEMPT_BYTES + SCOPE_BYTES + PLAN_IDENTITY_BYTES + 32
}

fn closure_offset() -> usize {
    receipt_offset() + RECEIPT_IDENTITY_BYTES
}

fn resign_claim(bytes: &mut [u8]) {
    let body_length = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN_V2);
    digest.update(&bytes[..body_length]);
    bytes[body_length..].copy_from_slice(&digest.finalize());
}

fn managed_artifact(output: &Path) -> PathBuf {
    let mut matches = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(ARTIFACT_PREFIX) && name.ends_with(ARTIFACT_SUFFIX)
        });
    let artifact = matches.next().expect("managed artifact");
    assert!(matches.next().is_none(), "one managed artifact expected");
    artifact
}

#[test]
fn canonical_v2_claim_round_trips_reacquires_and_remains_inert() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("claim_v2", "/src/v2-claim-round-trip.rs");
    let attempt = begin(&output, &owner, 1);
    let bytes = b"canonical protected gfx942 hsaco";
    let publication_plan = plan(attempt, scope(0x10), 0x20, bytes);
    let closure = compiler_closure(0x30);
    let result = publish_v2(&output, &owner, attempt, publication_plan, closure, bytes);
    let claim = result.published_claim().clone();

    assert_eq!(claim.plan(), publication_plan);
    assert_eq!(claim.upstream_evidence(), upstream(publication_plan));
    assert_eq!(claim.receipt(), result.receipt());
    assert_eq!(claim.compiler_closure(), closure);
    assert!(!claim.grants_compiler_authority());
    assert!(!claim.grants_proof_authority());
    assert!(!claim.grants_publication_authority());
    assert!(!claim.grants_load_authority());
    assert!(!claim.grants_launch_authority());
    assert!(!result.grants_compiler_authority());
    assert!(!result.grants_proof_authority());
    assert!(!result.grants_publication_authority());
    assert!(!result.grants_load_authority());
    assert!(!result.grants_launch_authority());

    let encoded = claim.encode_canonical().unwrap();
    assert_eq!(
        DurablePublishedHsacoClaimV2::decode_canonical(&encoded).unwrap(),
        claim
    );
    drop(result);
    finish_build_attempt(&output, &owner, attempt).unwrap();
    let lease = reacquire_current_hsaco_publication_lease_v2(&output, &claim).unwrap();
    assert_eq!(lease.published().attempt(), attempt);
    assert_eq!(lease.exact_artifact_bytes(), bytes);
    assert!(!lease.grants_load_authority());
    assert!(!lease.grants_launch_authority());
}

#[test]
fn v2_codec_rejects_framing_checksum_magic_version_and_noncanonical_closure() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("codec_v2", "/src/v2-claim-codec.rs");
    let attempt = begin(&output, &owner, 2);
    let bytes = b"protected claim codec payload";
    let publication_plan = plan(attempt, scope(0x21), 0x31, bytes);
    let closure = compiler_closure(0x41);
    let encoded = publish_v2(&output, &owner, attempt, publication_plan, closure, bytes)
        .published_claim()
        .encode_canonical()
        .unwrap();

    for length in [0, 1, encoded.len() - 1] {
        assert!(matches!(
            DurablePublishedHsacoClaimV2::decode_canonical(&encoded[..length]),
            Err(DurablePublishedClaimCodecErrorV2::Truncated)
        ));
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&trailing),
        Err(DurablePublishedClaimCodecErrorV2::TrailingBytes)
    ));
    let oversized = vec![0; MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V2 + 1];
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&oversized),
        Err(DurablePublishedClaimCodecErrorV2::TooLarge { .. })
    ));

    let mut bad_checksum = encoded.clone();
    bad_checksum[CLAIM_MAGIC_V2.len() + 10] ^= 1;
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&bad_checksum),
        Err(DurablePublishedClaimCodecErrorV2::ChecksumMismatch)
    ));
    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    resign_claim(&mut bad_magic);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&bad_magic),
        Err(DurablePublishedClaimCodecErrorV2::BadMagic)
    ));
    let mut bad_version = encoded.clone();
    bad_version[CLAIM_MAGIC_V2.len()..CLAIM_MAGIC_V2.len() + 2]
        .copy_from_slice(&3_u16.to_le_bytes());
    resign_claim(&mut bad_version);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&bad_version),
        Err(DurablePublishedClaimCodecErrorV2::UnsupportedVersion { actual: 3 })
    ));

    let closure = closure_offset();
    let mut zero_pin = encoded.clone();
    zero_pin[closure..closure + 32].fill(0);
    resign_claim(&mut zero_pin);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&zero_pin),
        Err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::ZeroDigest {
                field: CompilerClosureDigestFieldV2::CargoExecutable
            }
        ))
    ));
    let mut bad_protocol = encoded.clone();
    bad_protocol[closure + CLOSURE_PIN_BYTES..closure + CLOSURE_PIN_BYTES + 2]
        .copy_from_slice(&2_u16.to_le_bytes());
    resign_claim(&mut bad_protocol);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&bad_protocol),
        Err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version: 2 }
        ))
    ));
    let mut bad_aggregate = encoded;
    bad_aggregate[closure + CLOSURE_PIN_BYTES + 2] ^= 0x80;
    resign_claim(&mut bad_aggregate);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&bad_aggregate),
        Err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::IdentityMismatch
        ))
    ));
}

#[test]
fn semantic_plan_and_producer_mutations_decode_but_cannot_reacquire() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("semantic_v2", "/src/v2-claim-semantic.rs");
    let attempt = begin(&output, &owner, 3);
    let bytes = b"protected semantic mutation payload";
    let publication_plan = plan(attempt, scope(0x32), 0x42, bytes);
    let closure = compiler_closure(0x52);
    let result = publish_v2(&output, &owner, attempt, publication_plan, closure, bytes);
    let claim = result.published_claim().clone();
    let encoded = claim.encode_canonical().unwrap();

    let plan_identities = CLAIM_MAGIC_V2.len() + 2 + ATTEMPT_BYTES + SCOPE_BYTES;
    let mut changed_plan = encoded.clone();
    changed_plan[plan_identities] ^= 1;
    resign_claim(&mut changed_plan);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&changed_plan),
        Err(DurablePublishedClaimCodecErrorV2::ReceiptMismatch {
            field: DurablePublishedClaimReceiptFieldV2::Plan
        })
    ));

    let mut changed_producer = encoded;
    changed_producer[receipt_offset() + 32] ^= 1;
    resign_claim(&mut changed_producer);
    let changed_producer = DurablePublishedHsacoClaimV2::decode_canonical(&changed_producer)
        .expect("producer identity remains canonical inert data");
    assert!(!changed_producer.grants_compiler_authority());
    assert!(!changed_producer.grants_publication_authority());
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v2(&output, &changed_producer),
        Err(DurablePublishedClaimReacquisitionErrorV2::ReceiptMismatch)
    ));

    let intruder = producer("semantic_v2", "/src/v2-claim-intruder.rs");
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v2(
            &output,
            &intruder,
            attempt,
            publication_plan,
            upstream(publication_plan),
            closure,
            claim.receipt(),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch)
    ));
    let substituted_attempt = fake_attempt(attempt, 0x53);
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v2(
            &output,
            &owner,
            substituted_attempt,
            plan(substituted_attempt, publication_plan.scope(), 0x42, bytes),
            upstream(publication_plan),
            closure,
            claim.receipt(),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch)
    ));
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v2(
            &output,
            &owner,
            attempt,
            plan(attempt, publication_plan.scope(), 0x43, bytes),
            upstream(publication_plan),
            closure,
            claim.receipt(),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch)
    ));
}

#[test]
fn canonical_substituted_closure_decodes_inertly_but_cannot_reacquire_registry() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("closure_v2", "/src/v2-claim-closure.rs");
    let attempt = begin(&output, &owner, 4);
    let bytes = b"canonical substituted closure payload";
    let publication_plan = plan(attempt, scope(0x43), 0x53, bytes);
    let original = compiler_closure(0x63);
    let alternate = compiler_closure(0x83);
    let result = publish_v2(&output, &owner, attempt, publication_plan, original, bytes);
    let mut encoded = result.published_claim().encode_canonical().unwrap();
    let substituted = substitute_closure_role(original, alternate, 4);
    encoded[closure_offset()..closure_offset() + CLOSURE_BYTES]
        .copy_from_slice(&closure_bytes(substituted));
    resign_claim(&mut encoded);

    let decoded = DurablePublishedHsacoClaimV2::decode_canonical(&encoded).unwrap();
    assert_eq!(decoded.compiler_closure(), substituted);
    assert_ne!(decoded.compiler_closure(), original);
    assert!(!decoded.grants_compiler_authority());
    assert!(!decoded.grants_proof_authority());
    assert!(!decoded.grants_publication_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v2(&output, &decoded),
        Err(DurablePublishedClaimReacquisitionErrorV2::ReceiptMismatch)
    ));
}

#[test]
fn artifact_and_output_directory_substitution_never_reacquire() {
    let artifact_temp = TestDirectory::new();
    let artifact_output = artifact_temp.output();
    let artifact_owner = producer("artifact_v2", "/src/v2-claim-artifact.rs");
    let artifact_attempt = begin(&artifact_output, &artifact_owner, 5);
    let artifact_bytes = b"protected exact output bytes";
    let artifact_plan = plan(artifact_attempt, scope(0x54), 0x64, artifact_bytes);
    let closure = compiler_closure(0x74);
    let artifact_result = publish_v2(
        &artifact_output,
        &artifact_owner,
        artifact_attempt,
        artifact_plan,
        closure,
        artifact_bytes,
    );
    let artifact_claim = artifact_result.published_claim().clone();
    drop(artifact_result);
    let artifact = managed_artifact(&artifact_output);
    fs::write(&artifact, vec![b'x'; artifact_bytes.len()]).unwrap();
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v2(&artifact_output, &artifact_claim),
        Err(DurablePublishedClaimReacquisitionErrorV2::Publication(_))
    ));

    let directory_temp = TestDirectory::new();
    let output = directory_temp.output();
    let owner = producer("directory_v2", "/src/v2-claim-directory.rs");
    let attempt = begin(&output, &owner, 6);
    let bytes = b"protected output directory identity";
    let publication_plan = plan(attempt, scope(0x65), 0x75, bytes);
    let result = publish_v2(&output, &owner, attempt, publication_plan, closure, bytes);
    let claim = result.published_claim().clone();
    drop(result);

    let original = directory_temp.path.join("original-output");
    fs::rename(&output, &original).unwrap();
    fs::create_dir(&output).unwrap();
    for entry in fs::read_dir(&original).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), output.join(entry.file_name())).unwrap();
        }
    }
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v2(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV2::Publication(_))
    ));
}

#[test]
fn v1_and_v2_claim_codecs_never_reinterpret_each_other_or_touch_registries() {
    let v1_temp = TestDirectory::new();
    let v1_output = v1_temp.output();
    let v1_owner = producer("claim_v1", "/src/v1-claim-into-v2.rs");
    let v1_attempt = begin(&v1_output, &v1_owner, 7);
    let v1_bytes = b"ordinary v1 claim bytes";
    let v1_plan = plan(v1_attempt, scope(0x76), 0x86, v1_bytes);
    let v1_claim = publish_v1(&v1_output, &v1_owner, v1_attempt, v1_plan, v1_bytes)
        .published_claim()
        .clone();
    let v1_encoded = v1_claim.encode_canonical().unwrap();
    let v1_registry = fs::read(v1_output.join(ATTEMPT_REGISTRY)).unwrap();
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&v1_encoded),
        Err(DurablePublishedClaimCodecErrorV2::Truncated)
    ));
    assert_eq!(
        fs::read(v1_output.join(ATTEMPT_REGISTRY)).unwrap(),
        v1_registry
    );

    let v2_temp = TestDirectory::new();
    let v2_output = v2_temp.output();
    let v2_owner = producer("claim_v2", "/src/v2-claim-into-v1.rs");
    let v2_attempt = begin(&v2_output, &v2_owner, 8);
    let v2_bytes = b"protected v2 claim bytes";
    let v2_plan = plan(v2_attempt, scope(0x87), 0x97, v2_bytes);
    let v2_claim = publish_v2(
        &v2_output,
        &v2_owner,
        v2_attempt,
        v2_plan,
        compiler_closure(0xa7),
        v2_bytes,
    )
    .published_claim()
    .clone();
    let v2_encoded = v2_claim.encode_canonical().unwrap();
    let v2_registry = fs::read(v2_output.join(ATTEMPT_REGISTRY)).unwrap();
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&v2_encoded),
        Err(DurablePublishedClaimCodecErrorV1::TrailingBytes)
    ));
    assert_eq!(
        fs::read(v2_output.join(ATTEMPT_REGISTRY)).unwrap(),
        v2_registry
    );

    assert_eq!(v1_claim.encode_canonical().unwrap(), v1_encoded);
    assert_eq!(v2_claim.encode_canonical().unwrap(), v2_encoded);
}
