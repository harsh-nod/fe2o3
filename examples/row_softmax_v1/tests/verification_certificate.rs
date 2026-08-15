use fe2o3_row_softmax_v1::{
    ROW_SOFTMAX_VERIFICATION_MANIFEST_V1, RowSoftmaxVerificationMismatchV1,
    VerificationEvidenceIdentityV1, validate_row_softmax_verification_manifest_v1,
};
use sha2::{Digest, Sha256};

const ATTRIBUTED_SOURCE: &[u8] = include_bytes!(
    "../../../crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs"
);
const NUMERICAL_POLICY: &[u8] = include_bytes!("../src/numerical_contract.rs");
const PROOF_SOURCE: &[u8] = include_bytes!("../verus/row_softmax_v1.rs");
const VERUS_CLOSURE_MANIFEST: &[u8] = include_bytes!("../verus/VERUS_CLOSURE_MANIFEST");
const VERUS_TRUST_VOCABULARY: &[u8] = include_bytes!("../verus/VERUS_TRUST_VOCABULARY");
const COMPILER_SOURCE: &str =
    include_str!("../../../crates/rustc-codegen-fe2o3/src/collected_row_softmax_v1.rs");
const CODEGEN_SOURCE: &str =
    include_str!("../../../crates/rustc-codegen-fe2o3/src/kernel_ir_codegen.rs");

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_identity(identity: VerificationEvidenceIdentityV1, bytes: &[u8]) {
    assert_eq!(identity.byte_len, bytes.len() as u64);
    assert_eq!(identity.sha256, sha256(bytes));
}

#[test]
fn reviewed_manifest_matches_every_repository_evidence_input() {
    let manifest = ROW_SOFTMAX_VERIFICATION_MANIFEST_V1;
    assert_identity(manifest.attributed_source, ATTRIBUTED_SOURCE);
    assert_identity(manifest.numerical_policy, NUMERICAL_POLICY);
    assert_identity(manifest.proof_source, PROOF_SOURCE);
    assert_identity(manifest.verus_closure_manifest, VERUS_CLOSURE_MANIFEST);
    assert_identity(manifest.verus_trust_vocabulary, VERUS_TRUST_VOCABULARY);

    for marker in [
        "const PORTABLE_MIR_SEMANTIC_IDENTITY: [u8; 32]",
        "const REVIEWED_CANONICAL_MODULE_V4_COMMITMENT: [u8; 32]",
        "const REVIEWED_RUSTC_RELEASE: &str = \"1.96.0-nightly\"",
        "const REVIEWED_RUSTC_COMMIT: &str = \"55e86c996809902e8bbad512cfb4d2c18be446d9\"",
        "const REVIEWED_RUSTC_LLVM: &str = \"22.1.2\"",
        "pub(crate) const EXACT_ROW_SOFTMAX_TARGET_V1: &str = \"gfx942:xnack-\"",
        "pub(crate) const ROW_SOFTMAX_ELEMENTS_V1: u32 = 64",
    ] {
        assert!(
            COMPILER_SOURCE.contains(marker),
            "missing compiler identity {marker}"
        );
    }
    assert!(CODEGEN_SOURCE.contains("const REVIEWED_ROW_SOFTMAX_V1_LLVM_SHA256: [u8; 32]"));
    assert!(
        VERUS_CLOSURE_MANIFEST
            .windows(manifest.solver_executable_sha256.len())
            .any(|window| window == manifest.solver_executable_sha256.as_bytes())
    );
}

#[test]
fn exact_manifest_yields_only_an_inert_deterministic_certificate() {
    let first = validate_row_softmax_verification_manifest_v1(ROW_SOFTMAX_VERIFICATION_MANIFEST_V1)
        .unwrap();
    let second = validate_row_softmax_verification_manifest_v1(first.manifest()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.manifest(), ROW_SOFTMAX_VERIFICATION_MANIFEST_V1);

    let digest = sha256(first.canonical_manifest_bytes());
    println!("FE2O3_ROW_SOFTMAX_V1_CERTIFICATE_SHA256={digest}");
    assert_eq!(digest, first.canonical_manifest_sha256());
}

#[test]
fn source_policy_proof_and_tool_substitutions_fail_closed() {
    let exact = ROW_SOFTMAX_VERIFICATION_MANIFEST_V1;

    let mut source = exact;
    source.attributed_source.sha256 =
        "0551d13970d1e6d577a6b058eb3ef9b389a2bb20544e6977291379b3f68b866c";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(source),
        Err(RowSoftmaxVerificationMismatchV1::AttributedSource)
    );

    let mut policy = exact;
    policy.numerical_policy.byte_len += 1;
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(policy),
        Err(RowSoftmaxVerificationMismatchV1::NumericalPolicy)
    );

    let mut proof = exact;
    proof.proof_source.relative_path = "verus/substituted.rs";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(proof),
        Err(RowSoftmaxVerificationMismatchV1::ProofSource)
    );

    let mut verus = exact;
    verus.verus_executable_sha256 =
        "bd2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(verus),
        Err(RowSoftmaxVerificationMismatchV1::VerusClosure)
    );

    let mut solver = exact;
    solver.solver_executable_sha256 =
        "f583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(solver),
        Err(RowSoftmaxVerificationMismatchV1::Solver)
    );
}

#[test]
fn correspondence_profile_target_and_width_substitutions_fail_closed() {
    let exact = ROW_SOFTMAX_VERIFICATION_MANIFEST_V1;

    let mut mir = exact;
    mir.portable_mir_sha256 = "db10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(mir),
        Err(RowSoftmaxVerificationMismatchV1::PortableMir)
    );

    let mut compiler = exact;
    compiler.rustc_llvm_release = "22.1.3";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(compiler),
        Err(RowSoftmaxVerificationMismatchV1::CompilerProfile)
    );

    let mut kernel_ir = exact;
    kernel_ir.kernel_ir_profile = "fe2o3::row_softmax_v1;substituted";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(kernel_ir),
        Err(RowSoftmaxVerificationMismatchV1::KernelIr)
    );

    let mut target = exact;
    target.target = "gfx942:xnack+";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(target),
        Err(RowSoftmaxVerificationMismatchV1::Target)
    );

    let mut width = exact;
    width.row_elements = 63;
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(width),
        Err(RowSoftmaxVerificationMismatchV1::Specialization)
    );

    let mut mask = exact;
    mask.activity_mask = "masked";
    assert_eq!(
        validate_row_softmax_verification_manifest_v1(mask),
        Err(RowSoftmaxVerificationMismatchV1::Specialization)
    );
}

#[test]
fn certificate_never_claims_origin_refinement_or_numeric_error_evidence() {
    let certificate =
        validate_row_softmax_verification_manifest_v1(ROW_SOFTMAX_VERIFICATION_MANIFEST_V1)
            .unwrap();
    let boundary = certificate.manifest().evidence_boundary;
    for marker in [
        "inert formal evidence only",
        "exp_real_v1 remains uninterpreted",
        "no OCML/IEEE error bound",
        "no compiler origin",
        "no source-to-machine refinement",
        "no artifact or execution authority",
    ] {
        assert!(boundary.contains(marker));
    }
}
