//! Retained authentication boundary for the row-softmax V1 proof certificate.
//!
//! The public example certificate is intentionally inert and constructible from
//! public data. This module therefore admits only independently observed bytes:
//! the exact canonical manifest plus every file identity named by that manifest.
//! The resulting value is linear evidence, not compiler, artifact, or execution
//! authority.

use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

const CERTIFICATE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX/VERIFICATION-CERTIFICATE-ADMISSION/V1\0";
const MAX_CANONICAL_MANIFEST_BYTES_V1: usize = 8 * 1024;
const MAX_EVIDENCE_FILE_BYTES_V1: usize = 1024 * 1024;

const CANONICAL_MANIFEST_SHA256_V1: &str =
    "8114b1d9fde2928742dd736970e3dc6eb4aa9dfca8fb3f1113e60a11269cae20";
const PORTABLE_MIR_SHA256_V1: &str =
    "cb10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd";
const COMPILER_SEMANTICS_SHA256_V1: &str =
    "3132d86d229a3977ed9c5283c241c4f6c85aff23c1d177fb0d23c0743279f0a4";
const KERNEL_IR_SHA256_V1: &str =
    "1e1b14c6842ffd09103eb55eb39b1bcae9c0da81597fed6186767562337230e6";
const LLVM_BODY_SHA256_V1: &str =
    "d48d3320c286c6da2253a104386089e389648f4260f2e7efda21269fef951c2c";
const VERUS_EXECUTABLE_SHA256_V1: &str =
    "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";
const SOLVER_EXECUTABLE_SHA256_V1: &str =
    "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765";

const CANONICAL_MANIFEST_BYTES_V1: &[u8] = b"FE2O3-ROW-SOFTMAX-VERIFICATION-V1
lineage-source=e874da2083c2a1eb192048ea5f88a053c28d0ee2|crates/rustc-codegen-fe2o3/tests/fixtures/collected-row-softmax-v1/src/lib.rs|1289|c4e2d6bb6eebe01eb6ae7c0da1a524113819a37b4ec2d0a5167f32cc3134e6f4
attributed-source=examples/row_softmax_v1/src/kernel.rs|1297|0b0d5e2964d4627bc7ef3dac882f86a9b3c49ab715245bacc3fc92f28f0d08b0
portable-mir=cb10b6fac6475435e45a6f9166739c9e26bae17031105791abf3f440b004d4dd
compiler-semantics=3132d86d229a3977ed9c5283c241c4f6c85aff23c1d177fb0d23c0743279f0a4
compiler-profile=fe2o3.manifest-derived-scalar-slice.v1|rustc-1.96.0-nightly|55e86c996809902e8bbad512cfb4d2c18be446d9|llvm-22.1.2
numerical-policy=examples/row_softmax_v1/src/numerical_contract.rs|9450|367b11f440d884cc1ecafd3b88fbf209c819acae09c21177718fd720fe9b18ad
proof=examples/row_softmax_v1/verus/row_softmax_v1.rs|12966|cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5
kernel-ir=fe2o3::row_softmax_v1;fixed-row-64;wg64;cov6|1e1b14c6842ffd09103eb55eb39b1bcae9c0da81597fed6186767562337230e6
llvm-body=d48d3320c286c6da2253a104386089e389648f4260f2e7efda21269fef951c2c
target=gfx942:xnack-|row-elements=64|input-elements=64|output-elements=64|activity=unmasked-all-64|worker=lane0-only-three-loops-zero-barriers
verus=0.2026.08.02.b677dd5|ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd
solver-z3=e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765
verus-closure=examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST|591|d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019
trust-vocabulary=examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY|6572|54457b1030c88f7598a0a948563a0abd551a431e0f97b7ff33242f56f194ad7d
boundary=inert;exact-input-output-lengths-are-authenticated-preconditions-not-observed-runtime-facts;exp-uninterpreted;no-ocml-ieee-bound;no-compiler-origin;no-source-machine-refinement;no-execution-authority
";

const EVIDENCE_SPECS_V1: [EvidenceSpecV1; 5] = [
    EvidenceSpecV1 {
        path: "examples/row_softmax_v1/src/kernel.rs",
        byte_len: 1_297,
        sha256: "0b0d5e2964d4627bc7ef3dac882f86a9b3c49ab715245bacc3fc92f28f0d08b0",
    },
    EvidenceSpecV1 {
        path: "examples/row_softmax_v1/src/numerical_contract.rs",
        byte_len: 9_450,
        sha256: "367b11f440d884cc1ecafd3b88fbf209c819acae09c21177718fd720fe9b18ad",
    },
    EvidenceSpecV1 {
        path: "examples/row_softmax_v1/verus/row_softmax_v1.rs",
        byte_len: 12_966,
        sha256: "cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5",
    },
    EvidenceSpecV1 {
        path: "examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST",
        byte_len: 591,
        sha256: "d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019",
    },
    EvidenceSpecV1 {
        path: "examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY",
        byte_len: 6_572,
        sha256: "54457b1030c88f7598a0a948563a0abd551a431e0f97b7ff33242f56f194ad7d",
    },
];

#[derive(Clone, Copy)]
struct EvidenceSpecV1 {
    path: &'static str,
    byte_len: usize,
    sha256: &'static str,
}

/// One independently read evidence file, before identity admission.
#[derive(Clone, Copy, Debug)]
pub struct RowSoftmaxVerificationFileObservationV1<'a> {
    relative_path: &'a str,
    bytes: &'a [u8],
}

impl<'a> RowSoftmaxVerificationFileObservationV1<'a> {
    pub const fn new(relative_path: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            relative_path,
            bytes,
        }
    }
}

/// Complete independently observed input to certificate authentication.
///
/// Evidence files are ordered exactly as attributed source, numerical policy,
/// proof source, Verus closure manifest, and Verus trust vocabulary. `None`
/// represents missing evidence and is rejected without producing a token.
#[derive(Debug)]
pub struct RowSoftmaxVerificationCertificateObservationV1<'a> {
    canonical_manifest_bytes: &'a [u8],
    canonical_manifest_sha256: [u8; 32],
    evidence: [Option<RowSoftmaxVerificationFileObservationV1<'a>>; 5],
}

impl<'a> RowSoftmaxVerificationCertificateObservationV1<'a> {
    pub const fn new(
        canonical_manifest_bytes: &'a [u8],
        canonical_manifest_sha256: [u8; 32],
        evidence: [Option<RowSoftmaxVerificationFileObservationV1<'a>>; 5],
    ) -> Self {
        Self {
            canonical_manifest_bytes,
            canonical_manifest_sha256,
            evidence,
        }
    }
}

/// Stable identity of one exact, independently observed certificate closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRowSoftmaxVerificationCertificateIdentityV1([u8; 32]);

impl AuthenticatedRowSoftmaxVerificationCertificateIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Linear authentication of the exact public #121 certificate closure.
///
/// This type deliberately implements neither `Clone`, `Copy`, nor any
/// serialization trait. It carries no source bytes and cannot load or launch a
/// kernel. Its private fields prevent callers from upgrading the public inert
/// certificate by construction.
///
/// ```compile_fail
/// fn consume(_: fe2o3_verifier::AuthenticatedRowSoftmaxVerificationCertificateV1) {}
/// fn replay(value: fe2o3_verifier::AuthenticatedRowSoftmaxVerificationCertificateV1) {
///     consume(value);
///     consume(value);
/// }
/// ```
#[must_use = "authenticated row-softmax evidence must be consumed by exact admission"]
pub struct AuthenticatedRowSoftmaxVerificationCertificateV1 {
    identity: AuthenticatedRowSoftmaxVerificationCertificateIdentityV1,
    evidence_sha256: [[u8; 32]; 5],
    _private: (),
}

impl fmt::Debug for AuthenticatedRowSoftmaxVerificationCertificateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedRowSoftmaxVerificationCertificateV1")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedRowSoftmaxVerificationCertificateV1 {
    pub const fn identity(&self) -> AuthenticatedRowSoftmaxVerificationCertificateIdentityV1 {
        self.identity
    }

    pub const fn target(&self) -> &'static str {
        "gfx942:xnack-"
    }

    pub const fn row_elements(&self) -> u32 {
        64
    }

    /// Exact input length authenticated as a precondition by this certificate.
    pub const fn required_input_elements(&self) -> u32 {
        64
    }

    /// Exact output length authenticated as a precondition by this certificate.
    pub const fn required_output_elements(&self) -> u32 {
        64
    }

    /// The canonical manifest includes both exact memory-length preconditions.
    pub const fn authenticates_exact_memory_preconditions(&self) -> bool {
        true
    }

    /// Authentication does not observe the slices supplied to a runtime launch.
    pub const fn proves_runtime_memory_preconditions(&self) -> bool {
        false
    }

    pub const fn compiler_profile(&self) -> &'static str {
        "fe2o3.manifest-derived-scalar-slice.v1"
    }

    pub const fn kernel_ir_profile(&self) -> &'static str {
        "fe2o3::row_softmax_v1;fixed-row-64;wg64;cov6"
    }

    pub const fn attributed_source_sha256(&self) -> [u8; 32] {
        self.evidence_sha256[0]
    }

    pub const fn numerical_policy_sha256(&self) -> [u8; 32] {
        self.evidence_sha256[1]
    }

    pub const fn proof_source_sha256(&self) -> [u8; 32] {
        self.evidence_sha256[2]
    }

    pub fn portable_mir_sha256(&self) -> [u8; 32] {
        pinned_sha256(PORTABLE_MIR_SHA256_V1)
    }

    pub fn compiler_semantics_sha256(&self) -> [u8; 32] {
        pinned_sha256(COMPILER_SEMANTICS_SHA256_V1)
    }

    pub fn kernel_ir_sha256(&self) -> [u8; 32] {
        pinned_sha256(KERNEL_IR_SHA256_V1)
    }

    pub fn llvm_body_sha256(&self) -> [u8; 32] {
        pinned_sha256(LLVM_BODY_SHA256_V1)
    }

    pub fn verus_executable_sha256(&self) -> [u8; 32] {
        pinned_sha256(VERUS_EXECUTABLE_SHA256_V1)
    }

    pub fn solver_executable_sha256(&self) -> [u8; 32] {
        pinned_sha256(SOLVER_EXECUTABLE_SHA256_V1)
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_ocml_or_ieee_error_bound(&self) -> bool {
        false
    }

    pub const fn proves_source_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn proves_execution(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Certificate-closure authentication failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RowSoftmaxVerificationCertificateAuthenticationErrorV1 {
    ManifestLength,
    ManifestDigest,
    ManifestBytes,
    MissingEvidence { index: usize },
    EvidencePath { index: usize },
    EvidenceLength { index: usize },
    EvidenceDigest { index: usize },
    ReorderedEvidence { index: usize },
}

impl fmt::Display for RowSoftmaxVerificationCertificateAuthenticationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestLength => {
                formatter.write_str("row-softmax certificate manifest length is invalid")
            }
            Self::ManifestDigest => {
                formatter.write_str("row-softmax certificate manifest digest differs")
            }
            Self::ManifestBytes => {
                formatter.write_str("row-softmax certificate manifest bytes differ")
            }
            Self::MissingEvidence { index } => write!(
                formatter,
                "row-softmax certificate evidence {index} is missing"
            ),
            Self::EvidencePath { index } => write!(
                formatter,
                "row-softmax certificate evidence {index} path differs"
            ),
            Self::EvidenceLength { index } => write!(
                formatter,
                "row-softmax certificate evidence {index} length differs"
            ),
            Self::EvidenceDigest { index } => write!(
                formatter,
                "row-softmax certificate evidence {index} digest differs"
            ),
            Self::ReorderedEvidence { index } => write!(
                formatter,
                "row-softmax certificate evidence {index} is reordered"
            ),
        }
    }
}

impl Error for RowSoftmaxVerificationCertificateAuthenticationErrorV1 {}

/// Authenticates the exact canonical manifest and every retained file identity.
pub fn authenticate_row_softmax_verification_certificate_v1(
    observed: RowSoftmaxVerificationCertificateObservationV1<'_>,
) -> Result<
    AuthenticatedRowSoftmaxVerificationCertificateV1,
    RowSoftmaxVerificationCertificateAuthenticationErrorV1,
> {
    if observed.canonical_manifest_bytes.is_empty()
        || observed.canonical_manifest_bytes.len() > MAX_CANONICAL_MANIFEST_BYTES_V1
    {
        return Err(RowSoftmaxVerificationCertificateAuthenticationErrorV1::ManifestLength);
    }
    let manifest_sha256: [u8; 32] = Sha256::digest(observed.canonical_manifest_bytes).into();
    if manifest_sha256 != observed.canonical_manifest_sha256
        || manifest_sha256 != pinned_sha256(CANONICAL_MANIFEST_SHA256_V1)
    {
        return Err(RowSoftmaxVerificationCertificateAuthenticationErrorV1::ManifestDigest);
    }
    if observed.canonical_manifest_bytes != CANONICAL_MANIFEST_BYTES_V1 {
        return Err(RowSoftmaxVerificationCertificateAuthenticationErrorV1::ManifestBytes);
    }

    let mut evidence_sha256 = [[0_u8; 32]; 5];
    for (index, spec) in EVIDENCE_SPECS_V1.iter().enumerate() {
        let file = observed.evidence[index].ok_or(
            RowSoftmaxVerificationCertificateAuthenticationErrorV1::MissingEvidence { index },
        )?;
        if let Some(expected_index) = EVIDENCE_SPECS_V1
            .iter()
            .position(|candidate| candidate.path == file.relative_path)
            && expected_index != index
        {
            return Err(
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::ReorderedEvidence { index },
            );
        }
        if file.relative_path != spec.path {
            return Err(
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidencePath { index },
            );
        }
        if file.bytes.is_empty()
            || file.bytes.len() > MAX_EVIDENCE_FILE_BYTES_V1
            || file.bytes.len() != spec.byte_len
        {
            return Err(
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidenceLength { index },
            );
        }
        let digest: [u8; 32] = Sha256::digest(file.bytes).into();
        if digest != pinned_sha256(spec.sha256) {
            return Err(
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidenceDigest { index },
            );
        }
        evidence_sha256[index] = digest;
    }

    let mut identity = Sha256::new();
    identity.update(CERTIFICATE_IDENTITY_DOMAIN_V1);
    identity.update(manifest_sha256);
    for digest in evidence_sha256 {
        identity.update(digest);
    }
    Ok(AuthenticatedRowSoftmaxVerificationCertificateV1 {
        identity: AuthenticatedRowSoftmaxVerificationCertificateIdentityV1(
            identity.finalize().into(),
        ),
        evidence_sha256,
        _private: (),
    })
}

fn pinned_sha256(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    debug_assert_eq!(bytes.len(), 64);
    std::array::from_fn(|index| {
        (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1])
    })
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("pinned SHA-256 is lowercase hexadecimal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &[u8] = include_bytes!("../../../examples/row_softmax_v1/src/kernel.rs");
    const NUMERICAL_POLICY: &[u8] =
        include_bytes!("../../../examples/row_softmax_v1/src/numerical_contract.rs");
    const PROOF: &[u8] = include_bytes!("../../../examples/row_softmax_v1/verus/row_softmax_v1.rs");
    const CLOSURE: &[u8] =
        include_bytes!("../../../examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST");
    const TRUST: &[u8] =
        include_bytes!("../../../examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY");

    fn evidence<'a>(
        bytes: [&'a [u8]; 5],
    ) -> [Option<RowSoftmaxVerificationFileObservationV1<'a>>; 5] {
        std::array::from_fn(|index| {
            Some(RowSoftmaxVerificationFileObservationV1::new(
                EVIDENCE_SPECS_V1[index].path,
                bytes[index],
            ))
        })
    }

    fn canonical_observation() -> RowSoftmaxVerificationCertificateObservationV1<'static> {
        RowSoftmaxVerificationCertificateObservationV1::new(
            CANONICAL_MANIFEST_BYTES_V1,
            pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
            evidence([SOURCE, NUMERICAL_POLICY, PROOF, CLOSURE, TRUST]),
        )
    }

    #[test]
    fn exact_public_certificate_closure_is_admitted_deterministically() {
        let first =
            authenticate_row_softmax_verification_certificate_v1(canonical_observation()).unwrap();
        let second =
            authenticate_row_softmax_verification_certificate_v1(canonical_observation()).unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.identity().as_bytes(),
            &pinned_sha256("be34ce8bb86778c1fda58e1de46d80ff45ec8264c1803968cbcdb1bc383320ac")
        );
        assert_eq!(first.target(), "gfx942:xnack-");
        assert_eq!(first.row_elements(), 64);
        assert_eq!(first.required_input_elements(), 64);
        assert_eq!(first.required_output_elements(), 64);
        assert!(first.authenticates_exact_memory_preconditions());
        assert!(!first.proves_runtime_memory_preconditions());
        assert!(!first.authenticates_compiler_origin());
        assert!(!first.proves_ocml_or_ieee_error_bound());
        assert!(!first.proves_source_to_machine_refinement());
        assert!(!first.proves_execution());
        assert!(!first.grants_load_authority());
        assert!(!first.grants_launch_authority());
    }

    #[test]
    fn every_manifest_byte_and_digest_byte_is_bound() {
        for index in 0..CANONICAL_MANIFEST_BYTES_V1.len() {
            let mut mutated = CANONICAL_MANIFEST_BYTES_V1.to_vec();
            mutated[index] ^= 1;
            let digest = Sha256::digest(&mutated).into();
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                &mutated,
                digest,
                evidence([SOURCE, NUMERICAL_POLICY, PROOF, CLOSURE, TRUST]),
            );
            assert!(authenticate_row_softmax_verification_certificate_v1(observed).is_err());
        }
        for index in 0..32 {
            let mut digest = pinned_sha256(CANONICAL_MANIFEST_SHA256_V1);
            digest[index] ^= 1;
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                digest,
                evidence([SOURCE, NUMERICAL_POLICY, PROOF, CLOSURE, TRUST]),
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::ManifestDigest
            );
        }
    }

    #[test]
    fn missing_reordered_stale_and_substituted_evidence_is_rejected() {
        let files = [SOURCE, NUMERICAL_POLICY, PROOF, CLOSURE, TRUST];
        for index in 0..5 {
            let mut missing = evidence(files);
            missing[index] = None;
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
                missing,
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::MissingEvidence { index }
            );

            let mut wrong_path = evidence(files);
            wrong_path[index] = Some(RowSoftmaxVerificationFileObservationV1::new(
                "substituted",
                files[index],
            ));
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
                wrong_path,
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidencePath { index }
            );

            let mut truncated = files[index].to_vec();
            truncated.pop();
            let mut wrong_length = evidence(files);
            wrong_length[index] = Some(RowSoftmaxVerificationFileObservationV1::new(
                EVIDENCE_SPECS_V1[index].path,
                &truncated,
            ));
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
                wrong_length,
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidenceLength { index }
            );

            let mut stale = files[index].to_vec();
            stale[0] ^= 1;
            let mut wrong_bytes = evidence(files);
            wrong_bytes[index] = Some(RowSoftmaxVerificationFileObservationV1::new(
                EVIDENCE_SPECS_V1[index].path,
                &stale,
            ));
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
                wrong_bytes,
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::EvidenceDigest { index }
            );
        }

        for index in 0..4 {
            let mut reordered = evidence(files);
            reordered.swap(index, index + 1);
            let observed = RowSoftmaxVerificationCertificateObservationV1::new(
                CANONICAL_MANIFEST_BYTES_V1,
                pinned_sha256(CANONICAL_MANIFEST_SHA256_V1),
                reordered,
            );
            assert_eq!(
                authenticate_row_softmax_verification_certificate_v1(observed).unwrap_err(),
                RowSoftmaxVerificationCertificateAuthenticationErrorV1::ReorderedEvidence { index }
            );
        }
    }
}
