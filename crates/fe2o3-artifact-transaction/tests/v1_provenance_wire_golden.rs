use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV1,
    AttemptScopedHsacoPublicationErrorV2, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableFaultTimingV1, DurableJournalBoundaryV1,
    DurableJournalStageV1, DurableLinkPublicationFaultPointV1, DurableLinkPublicationOptionsV1,
    DurableLinkPublicationPlanV1, DurablePublishedClaimCodecErrorV2, DurablePublishedHsacoClaimV1,
    DurablePublishedHsacoClaimV2, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PersistedBackendReceiptV1, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options, read_backend_publication_receipt_v1,
    read_backend_publication_receipt_v2,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ATTEMPT_REGISTRY: &str = ".fe2o3-attempts-v1";
const ATTEMPT_MAGIC_V1: &[u8] = b"FE2O3-ATTEMPTS-V1\0";
const CLAIM_MAGIC_V1: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V1\0";
const CLAIM_MAGIC_V2: &[u8] = b"FE2O3-PUBLISHED-HSACO-CLAIM-V2\0";
const CLAIM_CHECKSUM_DOMAIN_V1: &[u8] = b"fe2o3.published-hsaco-claim.checksum.v1\0";
const CLAIM_BODY_BYTES_V1: usize = 721;
const CLAIM_BYTES_V1: usize = 753;
const REGISTRY_BYTES_V1: usize = 357;
const RECEIPT_BYTES_V1: usize = 7 * 32;

const CRATE_NAME: &str = "v1_wire_golden";
const SOURCE_PATH: &str = "/src/v1-wire-golden.rs";
const STABLE_SOURCE: &str = "path:/src/v1-wire-golden.rs";
const ARTIFACT_BYTES: &[u8] = b"immutable v1 hsaco wire golden\n";
const SESSION: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
];
const INVOCATION: [u8; 32] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
];
const FINALIZED_OUTPUT_SHA256: &str =
    "e48240167b30e127b0b1aeddad959d48c2e52674e7c8e11d4c676d8b61871c73";

const PENDING_REGISTRY_SHA256: &str =
    "495585cf9ca6e68a5ad80edd43b6be25de47149592ff6a2ebb1d8f3d930cbc02";
const FINAL_REGISTRY_SHA256: &str =
    "893e5a7c13ced273571ef8f79e0839d7f87baf698c4059be0f1fd0cb1ed41f5b";
const CLAIM_CHECKSUM_V1: &str = "24c752d3c788937f9eb1e37c22d12d34d14c44a33982bcf578a7a0b941a6a01c";
const CLAIM_HEX_V1: &str = concat!(
    "4645324f332d5055424c49534845442d485341434f2d434c41494d2d5631000100",
    "0100000000000000000102030405060708090a0b0c0d0e0f",
    "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
    "4040404040404040404040404040404040404040404040404040404040404040",
    "4141414141414141414141414141414141414141414141414141414141414141",
    "4242424242424242424242424242424242424242424242424242424242424242",
    "5050505050505050505050505050505050505050505050505050505050505050",
    "5151515151515151515151515151515151515151515151515151515151515151",
    "5252525252525252525252525252525252525252525252525252525252525252",
    "5353535353535353535353535353535353535353535353535353535353535353",
    "5454545454545454545454545454545454545454545454545454545454545454",
    "e48240167b30e127b0b1aeddad959d48c2e52674e7c8e11d4c676d8b61871c73",
    "5555555555555555555555555555555555555555555555555555555555555555",
    "6060606060606060606060606060606060606060606060606060606060606060",
    "56c0548fe05080502ed74cb592c18d415a2559ef5742fe69dc2e2b1385501dc7",
    "c7008d745498312e59db442395d74be8253603edbb7059ef9657dbcc88ea49b48e",
    "38531858ebb3a184877ec1e7a34d7d115e570d808a8c01bfed781a924434009c",
    "4cc83dae10d99a85a0a40e5c528fb5c02c210561dd9608e80f8e88bddcf644",
    "6060606060606060606060606060606060606060606060606060606060606060",
    "e48240167b30e127b0b1aeddad959d48c2e52674e7c8e11d4c676d8b61871c73",
    "5555555555555555555555555555555555555555555555555555555555555555",
    "3008000000000000f60b100000000000",
    "3008000000000000f90b100000000000",
    "3008000000000000010c100000000000",
    "1f00000000000000",
    "24c752d3c788937f9eb1e37c22d12d34d14c44a33982bcf578a7a0b941a6a01c",
);

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-v1-provenance-wire-golden-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn output(&self) -> PathBuf {
        self.0.join("output")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    assert_eq!(encoded.len(), N * 2);
    let mut bytes = [0; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}

fn fixture_plan(attempt: fe2o3_artifact_transaction::BuildAttempt) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes([0x40; 32]),
            KernelSetIdentityV1::from_bytes([0x41; 32]),
            TargetIdentityV1::from_bytes([0x42; 32]),
        ),
        CanonicalLinkRequestIdentityV1::from_bytes([0x50; 32]),
        PinnedWorkerIdentityV1::from_bytes([0x51; 32]),
        ValidatedResponseIdentityV1::from_bytes([0x52; 32]),
        LinkedOutputIdentityV1::from_bytes([0x53; 32]),
        FinalizationIdentityV1::from_bytes([0x54; 32]),
        FinalizedOutputIdentityV1::from_bytes(decode_hex(FINALIZED_OUTPUT_SHA256)),
        AtomicPublicationIdentityV1::from_bytes([0x55; 32]),
    )
}

fn assert_next(bytes: &[u8], offset: &mut usize, expected: &[u8]) {
    let end = *offset + expected.len();
    assert_eq!(&bytes[*offset..end], expected);
    *offset = end;
}

fn assert_registry_layout(
    bytes: &[u8],
    receipt: fe2o3_artifact_transaction::BackendPublicationReceiptV1,
    receipt_tag: u8,
    expected_sha256: &str,
) -> usize {
    assert_eq!(bytes.len(), REGISTRY_BYTES_V1);
    let mut offset = 0;
    assert_next(bytes, &mut offset, ATTEMPT_MAGIC_V1);
    assert_next(bytes, &mut offset, &1_u64.to_le_bytes());
    assert_next(bytes, &mut offset, &1_u32.to_le_bytes());
    assert_next(
        bytes,
        &mut offset,
        &u16::try_from(STABLE_SOURCE.len()).unwrap().to_le_bytes(),
    );
    assert_next(bytes, &mut offset, STABLE_SOURCE.as_bytes());
    assert_next(
        bytes,
        &mut offset,
        &u16::try_from(CRATE_NAME.len()).unwrap().to_le_bytes(),
    );
    assert_next(bytes, &mut offset, CRATE_NAME.as_bytes());
    assert_next(bytes, &mut offset, &INVOCATION);
    assert_next(bytes, &mut offset, &1_u64.to_le_bytes());
    assert_next(bytes, &mut offset, &SESSION);
    assert_next(bytes, &mut offset, &[4]);
    let receipt_tag_offset = offset;
    assert_next(bytes, &mut offset, &[receipt_tag]);
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        assert_next(bytes, &mut offset, &identity);
    }
    assert_eq!(offset, bytes.len());
    assert_eq!(bytes.len() - receipt_tag_offset - 1, RECEIPT_BYTES_V1);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(bytes)),
        decode_hex(expected_sha256)
    );
    receipt_tag_offset
}

fn assert_claim_layout(
    bytes: &[u8],
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: fe2o3_artifact_transaction::BackendPublicationReceiptV1,
) {
    assert_eq!(bytes.len(), CLAIM_BYTES_V1);
    assert!(bytes.starts_with(CLAIM_MAGIC_V1));
    assert!(!bytes.starts_with(CLAIM_MAGIC_V2));

    let mut offset = 0;
    assert_next(bytes, &mut offset, CLAIM_MAGIC_V1);
    assert_next(bytes, &mut offset, &1_u16.to_le_bytes());
    assert_next(
        bytes,
        &mut offset,
        &plan.attempt().generation().to_le_bytes(),
    );
    assert_next(bytes, &mut offset, plan.attempt().session().as_bytes());
    assert_next(bytes, &mut offset, plan.attempt().invocation().as_bytes());
    assert_next(bytes, &mut offset, plan.scope().package().as_bytes());
    assert_next(bytes, &mut offset, plan.scope().kernel_set().as_bytes());
    assert_next(bytes, &mut offset, plan.scope().target().as_bytes());
    for identity in [
        plan.request().as_bytes(),
        plan.worker().as_bytes(),
        plan.response().as_bytes(),
        plan.linked_output().as_bytes(),
        plan.finalization().as_bytes(),
        plan.finalized_output().as_bytes(),
        plan.publication().as_bytes(),
    ] {
        assert_next(bytes, &mut offset, identity);
    }
    assert_next(bytes, &mut offset, &upstream.as_bytes());
    for identity in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        assert_next(bytes, &mut offset, &identity);
    }

    offset += 3 * 2 * 8;
    assert_next(
        bytes,
        &mut offset,
        &u64::try_from(ARTIFACT_BYTES.len()).unwrap().to_le_bytes(),
    );
    assert_eq!(offset, CLAIM_BODY_BYTES_V1);

    let mut checksum = Sha256::new();
    checksum.update(CLAIM_CHECKSUM_DOMAIN_V1);
    checksum.update(&bytes[..CLAIM_BODY_BYTES_V1]);
    let checksum: [u8; 32] = checksum.finalize().into();
    assert_eq!(checksum, decode_hex(CLAIM_CHECKSUM_V1));
    assert_eq!(&bytes[CLAIM_BODY_BYTES_V1..], checksum);
}

#[test]
fn v1_provenance_wire_vectors_are_immutable_and_version_separated() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer =
        ProducerIdentity::from_codegen(CRATE_NAME, Some(Path::new(SOURCE_PATH))).unwrap();
    let attempt = begin_build_attempt(
        &output,
        &producer,
        BuildInvocation::from_bytes(INVOCATION),
        BuildSession::from_bytes(SESSION),
    )
    .unwrap();
    let plan = fixture_plan(attempt);
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x60; 32]);
    let crash_point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };

    assert!(matches!(
        publish_exact_hsaco_evidence_for_attempt_v1_with_options(
            &output,
            &producer,
            attempt,
            plan,
            upstream,
            ARTIFACT_BYTES,
            DurableLinkPublicationOptionsV1::inject_crash(crash_point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(_))
    ));
    let pending_receipt =
        match read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap() {
            PersistedBackendReceiptV1::PendingProvenance(receipt) => receipt,
            state => panic!("expected pending V1 provenance, got {state:?}"),
        };
    let pending_registry = fs::read(output.join(ATTEMPT_REGISTRY)).unwrap();
    assert!(matches!(
        read_backend_publication_receipt_v2(&output, &producer, attempt),
        Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
    ));
    assert_eq!(
        fs::read(output.join(ATTEMPT_REGISTRY)).unwrap(),
        pending_registry
    );

    let result = publish_exact_hsaco_evidence_for_attempt_v1(
        &output,
        &producer,
        attempt,
        plan,
        upstream,
        ARTIFACT_BYTES,
    )
    .unwrap();
    let final_receipt =
        match read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap() {
            PersistedBackendReceiptV1::Provenance(receipt) => receipt,
            state => panic!("expected final V1 provenance, got {state:?}"),
        };
    let final_registry = fs::read(output.join(ATTEMPT_REGISTRY)).unwrap();
    assert_eq!(pending_receipt, final_receipt);
    assert_eq!(result.receipt(), final_receipt);
    assert_eq!(result.published_claim().plan(), plan);
    assert_eq!(result.published_claim().upstream_evidence(), upstream);
    assert!(matches!(
        read_backend_publication_receipt_v2(&output, &producer, attempt),
        Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
    ));
    assert_eq!(
        fs::read(output.join(ATTEMPT_REGISTRY)).unwrap(),
        final_registry
    );

    let pending_tag_offset = assert_registry_layout(
        &pending_registry,
        pending_receipt,
        3,
        PENDING_REGISTRY_SHA256,
    );
    let final_tag_offset =
        assert_registry_layout(&final_registry, final_receipt, 2, FINAL_REGISTRY_SHA256);
    assert_eq!(pending_tag_offset, final_tag_offset);
    assert_eq!(pending_registry.len(), final_registry.len());
    let differences: Vec<_> = pending_registry
        .iter()
        .zip(&final_registry)
        .enumerate()
        .filter(|(_, (pending, final_byte))| pending != final_byte)
        .collect();
    assert_eq!(differences.len(), 1);
    assert_eq!(differences[0].0, pending_tag_offset);
    assert_eq!(*differences[0].1.0, 3);
    assert_eq!(*differences[0].1.1, 2);

    let claim_bytes = decode_hex::<CLAIM_BYTES_V1>(CLAIM_HEX_V1);
    assert_claim_layout(&claim_bytes, plan, upstream, final_receipt);
    let claim = DurablePublishedHsacoClaimV1::decode_canonical(&claim_bytes).unwrap();
    assert_eq!(claim.plan(), plan);
    assert_eq!(claim.upstream_evidence(), upstream);
    assert_eq!(claim.receipt(), final_receipt);
    assert_eq!(claim.encode_canonical().unwrap(), claim_bytes);
    assert!(matches!(
        DurablePublishedHsacoClaimV2::decode_canonical(&claim_bytes),
        Err(DurablePublishedClaimCodecErrorV2::Truncated)
    ));
}
