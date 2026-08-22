//! Deterministic, inert formal-evidence manifest for row-softmax V1.

/// Content identity for one reviewed evidence input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationEvidenceIdentityV1 {
    /// Repository-relative path of the reviewed input.
    pub relative_path: &'static str,
    /// Exact byte length of the reviewed input.
    pub byte_len: u64,
    /// Lowercase hexadecimal SHA-256 of the reviewed input.
    pub sha256: &'static str,
}

/// Complete set of identities recorded by the formal-evidence phase.
///
/// Fields are public so a later admission layer can construct and validate an
/// independently observed manifest. Equality with the reviewed constant does
/// not establish compiler origin or source-to-machine refinement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxVerificationManifestV1 {
    /// Exact attributed Rust kernel source.
    pub attributed_source: VerificationEvidenceIdentityV1,
    /// Reviewed reachable portable-MIR commitment.
    pub portable_mir_sha256: &'static str,
    /// Reviewed rustc semantics-profile commitment.
    pub compiler_semantics_sha256: &'static str,
    /// Semantic profile carried by the typed kernel registration.
    pub compiler_profile: &'static str,
    /// rustc release used to derive the reviewed compiler commitments.
    pub rustc_release: &'static str,
    /// rustc commit used to derive the reviewed compiler commitments.
    pub rustc_commit: &'static str,
    /// LLVM release named by the reviewed compiler profile.
    pub rustc_llvm_release: &'static str,
    /// Shared executable numerical-policy source.
    pub numerical_policy: VerificationEvidenceIdentityV1,
    /// Positive Verus proof source.
    pub proof_source: VerificationEvidenceIdentityV1,
    /// Reviewed canonical Kernel IR module commitment.
    pub kernel_ir_sha256: &'static str,
    /// Reviewed canonical LLVM body commitment emitted from that Kernel IR.
    pub llvm_body_sha256: &'static str,
    /// Stable Kernel IR/profile name.
    pub kernel_ir_profile: &'static str,
    /// Exact AMDGPU target specialization.
    pub target: &'static str,
    /// Exact fixed row width.
    pub row_elements: u32,
    /// Exact input-slice length required by the reviewed operation trace.
    pub input_elements: u32,
    /// Exact disjoint output-slice length required by the reviewed operation trace.
    pub output_elements: u32,
    /// Exact activity-mask policy of the attributed source.
    pub activity_mask: &'static str,
    /// Exact worker/barrier schedule modeled by the proof.
    pub worker_schedule: &'static str,
    /// Pinned Verus release identity.
    pub verus_version: &'static str,
    /// Pinned Verus executable SHA-256.
    pub verus_executable_sha256: &'static str,
    /// Pinned Z3 executable SHA-256 from the complete closure manifest.
    pub solver_executable_sha256: &'static str,
    /// Complete pinned Verus release-closure manifest.
    pub verus_closure_manifest: VerificationEvidenceIdentityV1,
    /// Audited Verus trust-vocabulary input.
    pub verus_trust_vocabulary: VerificationEvidenceIdentityV1,
    /// Explicit limitation carried into every consuming layer.
    pub evidence_boundary: &'static str,
}

/// First reviewed identity that differs from an observed manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxVerificationMismatchV1 {
    /// Attributed kernel source path, size, or digest differs.
    AttributedSource,
    /// Reachable portable-MIR commitment differs.
    PortableMir,
    /// rustc semantics commitment or named compiler closure differs.
    CompilerProfile,
    /// Shared numerical-policy source differs.
    NumericalPolicy,
    /// Positive proof source differs.
    ProofSource,
    /// Canonical Kernel IR or LLVM body identity differs.
    KernelIr,
    /// AMDGPU target differs.
    Target,
    /// Width, activity mask, or scalar worker schedule differs.
    Specialization,
    /// Pinned Verus executable, version, or complete closure differs.
    VerusClosure,
    /// Pinned solver executable differs.
    Solver,
    /// Trust vocabulary or explicit evidence boundary differs.
    TrustBoundary,
}

/// Inert proof-evidence token produced only by exact manifest comparison.
///
/// This token has no executable authority. In particular, it does not admit a
/// compiler handoff, artifact, load, launch, or hardware result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxVerificationCertificateV1 {
    _private: (),
}

const ATTRIBUTED_SOURCE_V1: VerificationEvidenceIdentityV1 = VerificationEvidenceIdentityV1 {
    relative_path: "examples/row_softmax_v1/src/kernel.rs",
    byte_len: 1_297,
    sha256: "0b0d5e2964d4627bc7ef3dac882f86a9b3c49ab715245bacc3fc92f28f0d08b0",
};

const NUMERICAL_POLICY_V1: VerificationEvidenceIdentityV1 = VerificationEvidenceIdentityV1 {
    relative_path: "examples/row_softmax_v1/src/numerical_contract.rs",
    byte_len: 9_450,
    sha256: "367b11f440d884cc1ecafd3b88fbf209c819acae09c21177718fd720fe9b18ad",
};

const PROOF_SOURCE_V1: VerificationEvidenceIdentityV1 = VerificationEvidenceIdentityV1 {
    relative_path: "examples/row_softmax_v1/verus/row_softmax_v1.rs",
    byte_len: 12_966,
    sha256: "cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5",
};

const VERUS_CLOSURE_MANIFEST_V1: VerificationEvidenceIdentityV1 = VerificationEvidenceIdentityV1 {
    relative_path: "examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST",
    byte_len: 591,
    sha256: "d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019",
};

const VERUS_TRUST_VOCABULARY_V1: VerificationEvidenceIdentityV1 = VerificationEvidenceIdentityV1 {
    relative_path: "examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY",
    byte_len: 6_572,
    sha256: "54457b1030c88f7598a0a948563a0abd551a431e0f97b7ff33242f56f194ad7d",
};

/// Canonical reviewed formal-evidence manifest.
pub const ROW_SOFTMAX_VERIFICATION_MANIFEST_V1: RowSoftmaxVerificationManifestV1 =
    RowSoftmaxVerificationManifestV1 {
        attributed_source: ATTRIBUTED_SOURCE_V1,
        portable_mir_sha256: "cb10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd",
        compiler_semantics_sha256: "3132d86d229a3977ed9c5283c241c4f6c85aff23c1d177fb0d23c0743279f0a4",
        compiler_profile: "fe2o3.manifest-derived-scalar-slice.v1",
        rustc_release: "1.96.0-nightly",
        rustc_commit: "55e86c996809902e8bbad512cfb4d2c18be446d9",
        rustc_llvm_release: "22.1.2",
        numerical_policy: NUMERICAL_POLICY_V1,
        proof_source: PROOF_SOURCE_V1,
        kernel_ir_sha256: "1e1b14c6842ffd09103eb55eb39b1bcae9c0da81597fed6186767562337230e6",
        llvm_body_sha256: "d48d3320c286c6da2253a104386089e389648f4260f2e7efda21269fef951c2c",
        kernel_ir_profile: "fe2o3::row_softmax_v1;fixed-row-64;wg64;cov6",
        target: "gfx942:xnack-",
        row_elements: 64,
        input_elements: 64,
        output_elements: 64,
        activity_mask: "unmasked: all 64 physical positions active",
        worker_schedule: "lane0-only;three-ordered-loops;zero-workgroup-barriers",
        verus_version: "0.2026.08.02.b677dd5",
        verus_executable_sha256: "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd",
        solver_executable_sha256: "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765",
        verus_closure_manifest: VERUS_CLOSURE_MANIFEST_V1,
        verus_trust_vocabulary: VERUS_TRUST_VOCABULARY_V1,
        evidence_boundary: "inert formal evidence only;exact input/output lengths are authenticated preconditions, not observed runtime facts;exp_real_v1 remains uninterpreted;no OCML/IEEE error bound;no compiler origin;no source-to-machine refinement;no artifact or execution authority",
    };

const CANONICAL_MANIFEST_BYTES_V1: &[u8] = b"FE2O3-ROW-SOFTMAX-VERIFICATION-V1\nlineage-source=e874da2083c2a1eb192048ea5f88a053c28d0ee2|crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs|1289|c4e2d6bb6eebe01eb6ae7c0da1a524113819a37b4ec2d0a5167f32cc3134e6f4\nattributed-source=examples/row_softmax_v1/src/kernel.rs|1297|0b0d5e2964d4627bc7ef3dac882f86a9b3c49ab715245bacc3fc92f28f0d08b0\nportable-mir=cb10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd\ncompiler-semantics=3132d86d229a3977ed9c5283c241c4f6c85aff23c1d177fb0d23c0743279f0a4\ncompiler-profile=fe2o3.manifest-derived-scalar-slice.v1|rustc-1.96.0-nightly|55e86c996809902e8bbad512cfb4d2c18be446d9|llvm-22.1.2\nnumerical-policy=examples/row_softmax_v1/src/numerical_contract.rs|9450|367b11f440d884cc1ecafd3b88fbf209c819acae09c21177718fd720fe9b18ad\nproof=examples/row_softmax_v1/verus/row_softmax_v1.rs|12966|cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5\nkernel-ir=fe2o3::row_softmax_v1;fixed-row-64;wg64;cov6|1e1b14c6842ffd09103eb55eb39b1bcae9c0da81597fed6186767562337230e6\nllvm-body=d48d3320c286c6da2253a104386089e389648f4260f2e7efda21269fef951c2c\ntarget=gfx942:xnack-|row-elements=64|input-elements=64|output-elements=64|activity=unmasked-all-64|worker=lane0-only-three-loops-zero-barriers\nverus=0.2026.08.02.b677dd5|ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd\nsolver-z3=e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765\nverus-closure=examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST|591|d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019\ntrust-vocabulary=examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY|6572|54457b1030c88f7598a0a948563a0abd551a431e0f97b7ff33242f56f194ad7d\nboundary=inert;exact-input-output-lengths-are-authenticated-preconditions-not-observed-runtime-facts;exp-uninterpreted;no-ocml-ieee-bound;no-compiler-origin;no-source-machine-refinement;no-execution-authority\n";

const CANONICAL_MANIFEST_SHA256_V1: &str =
    "8114b1d9fde2928742dd736970e3dc6eb4aa9dfca8fb3f1113e60a11269cae20";

/// Compares an independently observed manifest with the reviewed identities.
pub fn validate_row_softmax_verification_manifest_v1(
    observed: RowSoftmaxVerificationManifestV1,
) -> Result<RowSoftmaxVerificationCertificateV1, RowSoftmaxVerificationMismatchV1> {
    let expected = ROW_SOFTMAX_VERIFICATION_MANIFEST_V1;
    if observed.attributed_source != expected.attributed_source {
        return Err(RowSoftmaxVerificationMismatchV1::AttributedSource);
    }
    if observed.portable_mir_sha256 != expected.portable_mir_sha256 {
        return Err(RowSoftmaxVerificationMismatchV1::PortableMir);
    }
    if observed.compiler_semantics_sha256 != expected.compiler_semantics_sha256
        || observed.compiler_profile != expected.compiler_profile
        || observed.rustc_release != expected.rustc_release
        || observed.rustc_commit != expected.rustc_commit
        || observed.rustc_llvm_release != expected.rustc_llvm_release
    {
        return Err(RowSoftmaxVerificationMismatchV1::CompilerProfile);
    }
    if observed.numerical_policy != expected.numerical_policy {
        return Err(RowSoftmaxVerificationMismatchV1::NumericalPolicy);
    }
    if observed.proof_source != expected.proof_source {
        return Err(RowSoftmaxVerificationMismatchV1::ProofSource);
    }
    if observed.kernel_ir_sha256 != expected.kernel_ir_sha256
        || observed.llvm_body_sha256 != expected.llvm_body_sha256
        || observed.kernel_ir_profile != expected.kernel_ir_profile
    {
        return Err(RowSoftmaxVerificationMismatchV1::KernelIr);
    }
    if observed.target != expected.target {
        return Err(RowSoftmaxVerificationMismatchV1::Target);
    }
    if observed.row_elements != expected.row_elements
        || observed.input_elements != expected.input_elements
        || observed.output_elements != expected.output_elements
        || observed.activity_mask != expected.activity_mask
        || observed.worker_schedule != expected.worker_schedule
    {
        return Err(RowSoftmaxVerificationMismatchV1::Specialization);
    }
    if observed.verus_version != expected.verus_version
        || observed.verus_executable_sha256 != expected.verus_executable_sha256
        || observed.verus_closure_manifest != expected.verus_closure_manifest
    {
        return Err(RowSoftmaxVerificationMismatchV1::VerusClosure);
    }
    if observed.solver_executable_sha256 != expected.solver_executable_sha256 {
        return Err(RowSoftmaxVerificationMismatchV1::Solver);
    }
    if observed.verus_trust_vocabulary != expected.verus_trust_vocabulary
        || observed.evidence_boundary != expected.evidence_boundary
    {
        return Err(RowSoftmaxVerificationMismatchV1::TrustBoundary);
    }
    Ok(RowSoftmaxVerificationCertificateV1 { _private: () })
}

impl RowSoftmaxVerificationCertificateV1 {
    /// Returns the exact reviewed manifest represented by this inert token.
    pub const fn manifest(self) -> RowSoftmaxVerificationManifestV1 {
        ROW_SOFTMAX_VERIFICATION_MANIFEST_V1
    }

    /// Returns canonical deterministic bytes for external hashing or storage.
    pub const fn canonical_manifest_bytes(self) -> &'static [u8] {
        CANONICAL_MANIFEST_BYTES_V1
    }

    /// Returns the reviewed SHA-256 of the canonical manifest bytes.
    pub const fn canonical_manifest_sha256(self) -> &'static str {
        CANONICAL_MANIFEST_SHA256_V1
    }
}
