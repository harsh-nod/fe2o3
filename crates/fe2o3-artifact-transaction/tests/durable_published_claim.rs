use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV2, BuildAttempt,
    BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1,
    DurablePublishedClaimCodecErrorV1, DurablePublishedClaimCodecErrorV2,
    DurablePublishedClaimReacquisitionErrorV1, DurablePublishedClaimReacquisitionErrorV2,
    DurablePublishedClaimReceiptFieldV1, DurablePublishedHsacoClaimV1,
    DurablePublishedHsacoClaimV2, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
    fail_build_attempt, finish_build_attempt, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v2, reacquire_current_hsaco_publication_lease_v1,
    reacquire_current_hsaco_publication_lease_v2, recover_published_hsaco_claim_for_attempt_v2,
};
use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const CLAIM_MAGIC: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V1\0";
const CLAIM_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v1\0";
const CLAIM_MAGIC_V2: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V2\0";
const CLAIM_CHECKSUM_DOMAIN_V2: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v2\0";
const ATTEMPT_REGISTRY: &str = ".fe2o3-attempts-v1";
const RECORD_PREFIX: &str = ".fe2o3-link-publication-v1-";
const RECORD_SUFFIX: &str = ".record";
const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
const ARTIFACT_SUFFIX: &str = ".bin";

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-durable-claim-{}-{}",
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

fn begin(output: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        producer,
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32]),
        BuildSession::from_bytes([seed; 16]),
    )
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

fn publish(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
) -> fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV1 {
    publish_exact_hsaco_evidence_for_attempt_v1(
        output,
        producer,
        attempt,
        plan,
        upstream(plan),
        bytes,
    )
    .unwrap()
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

fn publish_v2(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
) -> fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV2 {
    publish_exact_hsaco_evidence_for_attempt_v2(
        output,
        producer,
        attempt,
        plan,
        upstream(plan),
        closure,
        bytes,
    )
    .unwrap()
}

fn encode_closure(closure: CompilerClosureV2) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(226);
    bytes.extend_from_slice(&closure.cargo_executable_sha256());
    bytes.extend_from_slice(&closure.cargo_binding_trampoline_sha256());
    bytes.extend_from_slice(&closure.cargo_fe2o3_binding_wrapper_sha256());
    bytes.extend_from_slice(&closure.rustc_executable_sha256());
    bytes.extend_from_slice(&closure.rustc_runtime_tree_sha256());
    bytes.extend_from_slice(&closure.codegen_backend_sha256());
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
    bytes
}

fn resign_claim(bytes: &mut [u8]) {
    let body_length = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN);
    digest.update(&bytes[..body_length]);
    bytes[body_length..].copy_from_slice(&digest.finalize());
}

fn resign_claim_v2(bytes: &mut [u8]) {
    let body_length = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(CLAIM_CHECKSUM_DOMAIN_V2);
    digest.update(&bytes[..body_length]);
    bytes[body_length..].copy_from_slice(&digest.finalize());
}

fn managed_entry(output: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let mut matches = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(prefix) && name.ends_with(suffix)
        });
    let entry = matches.next().expect("managed entry");
    assert!(matches.next().is_none(), "one managed entry expected");
    entry
}

#[cfg(unix)]
fn replace_private_file(path: &Path, bytes: &[u8]) {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let displaced = path.with_extension("displaced-original");
    fs::rename(path, displaced).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn canonical_claim_reacquires_exact_current_publication_after_completion() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("claim_owner", "/src/claim-owner.rs");
    let attempt = begin(&output, &owner, 1);
    let bytes = b"cross-process exact gfx942 hsaco";
    let plan = plan(attempt, scope(0x10), 0x20, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();

    assert_eq!(claim.plan(), plan);
    assert_eq!(claim.receipt(), result.receipt());
    assert_eq!(claim.upstream_evidence(), upstream(plan));
    assert!(!claim.grants_load_authority());
    assert!(!claim.grants_launch_authority());
    let encoded = claim.encode_canonical().unwrap();
    assert_eq!(
        DurablePublishedHsacoClaimV1::decode_canonical(&encoded).unwrap(),
        claim
    );

    drop(result);
    finish_build_attempt(&output, &owner, attempt).unwrap();
    let lease = reacquire_current_hsaco_publication_lease_v1(&output, &claim).unwrap();
    assert_eq!(lease.published().attempt(), attempt);
    assert_eq!(lease.published().scope(), plan.scope());
    assert_eq!(lease.exact_artifact_bytes(), bytes);
    assert!(!lease.grants_load_authority());
    assert!(!lease.grants_launch_authority());
    let current = lease.acquire_current_token().unwrap();
    lease.validate_current_token(&current).unwrap();
    assert_eq!(current.exact_artifact_bytes(), bytes);
}

#[test]
fn decoder_rejects_malformed_truncated_trailing_and_semantically_mutated_claims() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("codec_owner", "/src/codec-owner.rs");
    let attempt = begin(&output, &owner, 2);
    let bytes = b"claim codec payload";
    let plan = plan(attempt, scope(0x20), 0x30, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let encoded = result.published_claim().encode_canonical().unwrap();

    for length in [0, 1, encoded.len() - 1] {
        assert!(matches!(
            DurablePublishedHsacoClaimV1::decode_canonical(&encoded[..length]),
            Err(DurablePublishedClaimCodecErrorV1::Truncated)
        ));
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&trailing),
        Err(DurablePublishedClaimCodecErrorV1::TrailingBytes)
    ));
    let oversized = vec![0; 1_025];
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&oversized),
        Err(DurablePublishedClaimCodecErrorV1::TooLarge { .. })
    ));

    let mut corrupt = encoded.clone();
    corrupt[CLAIM_MAGIC.len() + 10] ^= 1;
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&corrupt),
        Err(DurablePublishedClaimCodecErrorV1::ChecksumMismatch)
    ));

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    resign_claim(&mut bad_magic);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&bad_magic),
        Err(DurablePublishedClaimCodecErrorV1::BadMagic)
    ));

    let mut zero_generation = encoded.clone();
    let generation = CLAIM_MAGIC.len() + 2;
    zero_generation[generation..generation + 8].fill(0);
    resign_claim(&mut zero_generation);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&zero_generation),
        Err(DurablePublishedClaimCodecErrorV1::InvalidAttempt)
    ));

    let plan_identities = generation + 8 + 16 + 32 + 3 * 32;
    let mut changed_plan = encoded.clone();
    changed_plan[plan_identities] ^= 1;
    resign_claim(&mut changed_plan);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&changed_plan),
        Err(DurablePublishedClaimCodecErrorV1::ReceiptMismatch {
            field: DurablePublishedClaimReceiptFieldV1::Plan
        })
    ));

    let upstream_identity = plan_identities + 7 * 32;
    let mut changed_upstream = encoded.clone();
    changed_upstream[upstream_identity] ^= 1;
    resign_claim(&mut changed_upstream);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&changed_upstream),
        Err(DurablePublishedClaimCodecErrorV1::ReceiptMismatch {
            field: DurablePublishedClaimReceiptFieldV1::UpstreamEvidence
        })
    ));

    let receipt = upstream_identity + 32;
    let mut changed_receipt_plan = encoded.clone();
    changed_receipt_plan[receipt + 3 * 32] ^= 1;
    resign_claim(&mut changed_receipt_plan);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&changed_receipt_plan),
        Err(DurablePublishedClaimCodecErrorV1::ReceiptMismatch {
            field: DurablePublishedClaimReceiptFieldV1::Plan
        })
    ));

    let mut zero_length = encoded;
    let artifact_length = zero_length.len() - 32 - 8;
    zero_length[artifact_length..artifact_length + 8].fill(0);
    resign_claim(&mut zero_length);
    assert!(matches!(
        DurablePublishedHsacoClaimV1::decode_canonical(&zero_length),
        Err(DurablePublishedClaimCodecErrorV1::InvalidArtifactLength { actual: 0 })
    ));
}

#[test]
fn mutated_claim_or_persisted_receipt_never_reacquires() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("receipt_owner", "/src/receipt-owner.rs");
    let attempt = begin(&output, &owner, 3);
    let bytes = b"receipt mutation payload";
    let plan = plan(attempt, scope(0x30), 0x40, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    let mut encoded = claim.encode_canonical().unwrap();

    let generation = CLAIM_MAGIC.len() + 2;
    let plan_identities = generation + 8 + 16 + 32 + 3 * 32;
    let receipt = plan_identities + 7 * 32 + 32;
    encoded[receipt + 32] ^= 1;
    resign_claim(&mut encoded);
    let changed_producer = DurablePublishedHsacoClaimV1::decode_canonical(&encoded).unwrap();
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &changed_producer),
        Err(DurablePublishedClaimReacquisitionErrorV1::ReceiptMismatch)
    ));

    drop(result);
    let registry_path = output.join(ATTEMPT_REGISTRY);
    let mut registry = fs::read(&registry_path).unwrap();
    let needle = claim.receipt().upstream_evidence_identity();
    let locations: Vec<_> = registry
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect();
    assert_eq!(locations.len(), 1);
    registry[locations[0]] ^= 1;
    fs::write(registry_path, registry).unwrap();
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::ReceiptMismatch)
    ));
}

#[test]
fn a_newer_publication_makes_an_older_claim_stale() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let first_owner = producer("first_owner", "/src/first-owner.rs");
    let first_attempt = begin(&output, &first_owner, 4);
    let publication_scope = scope(0x40);
    let first_bytes = b"first generation";
    let first_plan = plan(first_attempt, publication_scope, 0x50, first_bytes);
    let first = publish(
        &output,
        &first_owner,
        first_attempt,
        first_plan,
        first_bytes,
    );
    let stale_claim = first.published_claim().clone();
    drop(first);
    finish_build_attempt(&output, &first_owner, first_attempt).unwrap();

    let second_owner = producer("second_owner", "/src/second-owner.rs");
    let second_attempt = begin(&output, &second_owner, 5);
    assert!(second_attempt.generation() > first_attempt.generation());
    let second_bytes = b"second generation";
    let second_plan = plan(second_attempt, publication_scope, 0x60, second_bytes);
    let second = publish(
        &output,
        &second_owner,
        second_attempt,
        second_plan,
        second_bytes,
    );
    let current_claim = second.published_claim().clone();
    drop(second);

    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &stale_claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));
    let lease = reacquire_current_hsaco_publication_lease_v1(&output, &current_claim).unwrap();
    assert_eq!(lease.exact_artifact_bytes(), second_bytes);
}

#[test]
fn lock_contention_is_nonblocking_and_fails_closed() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("lock_owner", "/src/lock-owner.rs");
    let attempt = begin(&output, &owner, 6);
    let bytes = b"lock contention payload";
    let plan = plan(attempt, scope(0x50), 0x70, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    let original = result.into_current_lease();
    let current = original.acquire_current_token().unwrap();

    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Busy)
    ));
    drop(current);
    let reacquired = reacquire_current_hsaco_publication_lease_v1(&output, &claim).unwrap();
    assert_eq!(reacquired.exact_artifact_bytes(), bytes);
}

#[test]
fn a_failed_attempt_cannot_reacquire_its_still_present_receipt() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("failed_owner", "/src/failed-owner.rs");
    let attempt = begin(&output, &owner, 12);
    let bytes = b"failed attempt payload";
    let plan = plan(attempt, scope(0x55), 0x75, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    drop(result);
    fail_build_attempt(&output, &owner, attempt).unwrap();

    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::AttemptState)
    ));
}

#[cfg(unix)]
#[test]
fn same_byte_record_and_artifact_replacement_are_rejected_by_file_identity() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("replacement_owner", "/src/replacement-owner.rs");
    let attempt = begin(&output, &owner, 7);
    let bytes = b"same byte replacement payload";
    let first_plan = plan(attempt, scope(0x60), 0x80, bytes);
    let result = publish(&output, &owner, attempt, first_plan, bytes);
    let claim = result.published_claim().clone();
    drop(result);

    let artifact = managed_entry(&output, ARTIFACT_PREFIX, ARTIFACT_SUFFIX);
    replace_private_file(&artifact, bytes);
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));

    let record_temp = TestDirectory::new();
    let record_output = record_temp.output();
    let second_owner = producer("record_owner", "/src/record-owner.rs");
    let second_attempt = begin(&record_output, &second_owner, 8);
    let second_bytes = b"record replacement payload";
    let second_plan = plan(second_attempt, scope(0x61), 0x90, second_bytes);
    let second = publish(
        &record_output,
        &second_owner,
        second_attempt,
        second_plan,
        second_bytes,
    );
    let second_claim = second.published_claim().clone();
    drop(second);
    let record = managed_entry(&record_output, RECORD_PREFIX, RECORD_SUFFIX);
    let record_bytes = fs::read(&record).unwrap();
    replace_private_file(&record, &record_bytes);
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&record_output, &second_claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));
}

#[test]
fn payload_mutation_is_rejected() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("payload_owner", "/src/payload-owner.rs");
    let attempt = begin(&output, &owner, 9);
    let bytes = b"payload integrity bytes";
    let plan = plan(attempt, scope(0x70), 0xa0, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    drop(result);

    let artifact = managed_entry(&output, ARTIFACT_PREFIX, ARTIFACT_SUFFIX);
    let mut changed = bytes.to_vec();
    changed[0] ^= 1;
    fs::write(artifact, changed).unwrap();
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));
}

#[cfg(unix)]
#[test]
fn symlinked_artifact_is_rejected_without_following_it() {
    use std::os::unix::fs::symlink;

    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("symlink_owner", "/src/symlink-owner.rs");
    let attempt = begin(&output, &owner, 10);
    let bytes = b"symlink target bytes";
    let plan = plan(attempt, scope(0x80), 0xb0, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    drop(result);

    let target = temp.path.join("outside-artifact");
    fs::write(&target, bytes).unwrap();
    let artifact = managed_entry(&output, ARTIFACT_PREFIX, ARTIFACT_SUFFIX);
    fs::remove_file(&artifact).unwrap();
    symlink(&target, &artifact).unwrap();
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));
    assert_eq!(fs::read(target).unwrap(), bytes);
}

#[test]
fn output_directory_replacement_is_rejected_even_with_copied_state() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("directory_owner", "/src/directory-owner.rs");
    let attempt = begin(&output, &owner, 11);
    let bytes = b"directory identity payload";
    let plan = plan(attempt, scope(0x90), 0xc0, bytes);
    let result = publish(&output, &owner, attempt, plan, bytes);
    let claim = result.published_claim().clone();
    drop(result);

    let original = temp.path.join("original-output");
    fs::rename(&output, &original).unwrap();
    fs::create_dir(&output).unwrap();
    for entry in fs::read_dir(&original).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), output.join(entry.file_name())).unwrap();
        }
    }
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v1(&output, &claim),
        Err(DurablePublishedClaimReacquisitionErrorV1::Publication(_))
    ));
}

#[test]
fn protected_claim_codec_rejects_cross_version_and_all_noncanonical_inputs() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("protected_codec", "/src/protected-codec.rs");
    let attempt = begin(&output, &owner, 20);
    let bytes = b"protected claim codec payload";
    let protected_plan = plan(attempt, scope(0xa0), 0xb0, bytes);
    let closure = compiler_closure(0x10);
    let result = publish_v2(&output, &owner, attempt, protected_plan, closure, bytes);
    let claim = result.published_claim().clone();
    let encoded = claim.encode_canonical().unwrap();
    assert_eq!(
        DurablePublishedHsacoClaimV2::decode_canonical(&encoded).unwrap(),
        claim
    );
    assert_eq!(claim.compiler_closure(), closure);

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
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&vec![0; 1_025]),
        Err(DurablePublishedClaimCodecErrorV2::TooLarge { .. })
    ));

    let mut checksum = encoded.clone();
    checksum[CLAIM_MAGIC_V2.len() + 10] ^= 1;
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&checksum),
        Err(DurablePublishedClaimCodecErrorV2::ChecksumMismatch)
    ));
    let mut magic = encoded.clone();
    magic[0] ^= 1;
    resign_claim_v2(&mut magic);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&magic),
        Err(DurablePublishedClaimCodecErrorV2::BadMagic)
    ));
    let mut version = encoded.clone();
    version[CLAIM_MAGIC_V2.len()..CLAIM_MAGIC_V2.len() + 2].copy_from_slice(&3_u16.to_le_bytes());
    resign_claim_v2(&mut version);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&version),
        Err(DurablePublishedClaimCodecErrorV2::UnsupportedVersion { actual: 3 })
    ));

    let closure_start =
        CLAIM_MAGIC_V2.len() + 2 + 8 + 16 + 32 + (3 * 32) + (7 * 32) + 32 + (7 * 32);
    let mut protocol = encoded.clone();
    protocol[closure_start + 192..closure_start + 194].copy_from_slice(&2_u16.to_le_bytes());
    resign_claim_v2(&mut protocol);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&protocol),
        Err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version: 2 }
        ))
    ));
    let mut aggregate = encoded.clone();
    aggregate[closure_start + 194] ^= 1;
    resign_claim_v2(&mut aggregate);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&aggregate),
        Err(DurablePublishedClaimCodecErrorV2::InvalidCompilerClosure(
            CompilerClosureErrorV2::IdentityMismatch
        ))
    ));

    let legacy_temp = TestDirectory::new();
    let legacy_output = legacy_temp.output();
    let legacy_owner = producer("legacy_codec", "/src/legacy-codec.rs");
    let legacy_attempt = begin(&legacy_output, &legacy_owner, 21);
    let legacy_plan = plan(legacy_attempt, scope(0xa1), 0xb1, bytes);
    let legacy = publish(
        &legacy_output,
        &legacy_owner,
        legacy_attempt,
        legacy_plan,
        bytes,
    );
    let legacy_encoded = legacy.published_claim().encode_canonical().unwrap();
    assert!(DurablePublishedHsacoClaimV2::decode_canonical(&legacy_encoded).is_err());
    assert!(DurablePublishedHsacoClaimV1::decode_canonical(&encoded).is_err());
}

#[test]
fn canonical_substituted_closure_decodes_inertly_but_cannot_recover_or_reacquire() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer(
        "protected_substitution",
        "/src/protected-claim-substitution.rs",
    );
    let attempt = begin(&output, &owner, 22);
    let bytes = b"protected claim closure substitution";
    let plan = plan(attempt, scope(0xa2), 0xb2, bytes);
    let closure = compiler_closure(0x20);
    let result = publish_v2(&output, &owner, attempt, plan, closure, bytes);
    let original_receipt = result.receipt();
    let original_claim = result.published_claim().clone();
    let mut encoded = result.published_claim().encode_canonical().unwrap();
    drop(result);

    let alternate = compiler_closure(0x30);
    let closure_start =
        CLAIM_MAGIC_V2.len() + 2 + 8 + 16 + 32 + (3 * 32) + (7 * 32) + 32 + (7 * 32);
    encoded[closure_start..closure_start + 226].copy_from_slice(&encode_closure(alternate));
    resign_claim_v2(&mut encoded);
    let substituted = DurablePublishedHsacoClaimV2::decode_canonical(&encoded).unwrap();
    assert_eq!(substituted.compiler_closure(), alternate);
    assert_ne!(substituted.receipt(), original_receipt);
    assert!(!substituted.grants_compiler_authority());
    assert!(!substituted.grants_proof_authority());
    assert!(!substituted.grants_publication_authority());
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v2(
            &output,
            &owner,
            attempt,
            plan,
            upstream(plan),
            alternate,
            substituted.receipt(),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch)
    ));
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v2(&output, &substituted),
        Err(DurablePublishedClaimReacquisitionErrorV2::ReceiptMismatch)
    ));
    let lease = reacquire_current_hsaco_publication_lease_v2(&output, &original_claim).unwrap();
    assert_eq!(lease.exact_artifact_bytes(), bytes);
}
