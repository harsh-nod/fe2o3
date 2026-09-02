//! Independent admission of compiler-produced constrained affine certificates.

use std::{error::Error, fmt};

use fe2o3_proof_contracts::{
    ConstrainedAffineBoundsCertificateErrorV2, ConstrainedAffineBoundsCertificateV2,
    check_constrained_affine_bounds_certificate_v2,
};

/// SHA-256 of the exact Verus theorem source. Updated only with proof review.
pub const CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V2: [u8; 32] = [
    0x61, 0xc6, 0x87, 0x29, 0x78, 0x64, 0x07, 0x47, 0x96, 0xb9, 0x7c, 0x0a, 0x95, 0xa6, 0x19, 0x95,
    0x5c, 0x18, 0x6a, 0x2f, 0x62, 0x00, 0x71, 0x57, 0xe3, 0xd8, 0xf1, 0xaf, 0x17, 0xec, 0x6a, 0xec,
];
/// SHA-256 of the pinned Verus executable used by the proof runner.
pub const CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V2: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
/// SHA-256 of the complete pinned Verus/vstd/Z3 closure manifest.
pub const CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V2: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];

/// Exact reviewed proof-tool binding retained with a checked theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstrainedAffineBoundsProofBindingV2 {
    proof_source_sha256: [u8; 32],
    verus_executable_sha256: [u8; 32],
    verus_closure_manifest_sha256: [u8; 32],
}

impl ConstrainedAffineBoundsProofBindingV2 {
    pub const fn proof_source_sha256(self) -> [u8; 32] {
        self.proof_source_sha256
    }

    pub const fn verus_executable_sha256(self) -> [u8; 32] {
        self.verus_executable_sha256
    }

    pub const fn verus_closure_manifest_sha256(self) -> [u8; 32] {
        self.verus_closure_manifest_sha256
    }
}

/// Move-only custody of one independently checked constrained theorem.
#[must_use = "dropping this value abandons the independently checked constrained theorem"]
pub struct VerifiedCompilerConstrainedAffineBoundsV2 {
    certificate: ConstrainedAffineBoundsCertificateV2,
    proof_binding: ConstrainedAffineBoundsProofBindingV2,
}

impl VerifiedCompilerConstrainedAffineBoundsV2 {
    pub const fn certificate(&self) -> &ConstrainedAffineBoundsCertificateV2 {
        &self.certificate
    }

    pub const fn proof_binding(&self) -> ConstrainedAffineBoundsProofBindingV2 {
        self.proof_binding
    }

    pub const fn establishes_nonempty_constrained_domain(&self) -> bool {
        true
    }

    pub const fn establishes_nonnegative_strict_upper_bound(&self) -> bool {
        true
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for VerifiedCompilerConstrainedAffineBoundsV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompilerConstrainedAffineBoundsV2")
            .field("certificate", &self.certificate)
            .field("proof_binding", &self.proof_binding)
            .finish_non_exhaustive()
    }
}

/// Independent constrained-certificate admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerConstrainedAffineBoundsVerificationErrorV2 {
    source: ConstrainedAffineBoundsCertificateErrorV2,
}

impl CompilerConstrainedAffineBoundsVerificationErrorV2 {
    pub const fn source_kind(self) -> ConstrainedAffineBoundsCertificateErrorV2 {
        self.source
    }
}

impl fmt::Display for CompilerConstrainedAffineBoundsVerificationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiler constrained affine-bounds certificate rejected: {}",
            self.source
        )
    }
}

impl Error for CompilerConstrainedAffineBoundsVerificationErrorV2 {}

/// Independently checks and retains one exact compiler-produced theorem.
pub fn verify_compiler_constrained_affine_bounds_certificate_v2(
    certificate: &ConstrainedAffineBoundsCertificateV2,
) -> Result<
    VerifiedCompilerConstrainedAffineBoundsV2,
    CompilerConstrainedAffineBoundsVerificationErrorV2,
> {
    check_constrained_affine_bounds_certificate_v2(certificate)
        .map_err(|source| CompilerConstrainedAffineBoundsVerificationErrorV2 { source })?;
    Ok(VerifiedCompilerConstrainedAffineBoundsV2 {
        certificate: certificate.clone(),
        proof_binding: ConstrainedAffineBoundsProofBindingV2 {
            proof_source_sha256: CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V2,
            verus_executable_sha256: CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V2,
            verus_closure_manifest_sha256:
                CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V2,
        },
    })
}
