//! Independent admission of compiler-produced runtime-extent affine certificates.

use std::{error::Error, fmt};

use fe2o3_proof_contracts::{
    DynamicConstrainedAffineBoundsCertificateErrorV3, DynamicConstrainedAffineBoundsCertificateV3,
    check_dynamic_constrained_affine_bounds_certificate_v3,
};

/// SHA-256 of the exact reviewed V3 Verus theorem source.
pub const DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V3: [u8; 32] = [
    0x5b, 0x45, 0x2b, 0x46, 0x0e, 0x10, 0x28, 0x51, 0x9b, 0xfd, 0xa7, 0x4b, 0x4a, 0x84, 0x06, 0x7a,
    0xec, 0x2c, 0x95, 0x8d, 0x09, 0x29, 0x4d, 0x53, 0x64, 0x85, 0xba, 0xfe, 0x8f, 0x2f, 0xe0, 0xaf,
];
/// SHA-256 of the exact imported V2 theorem source.
pub const DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_V2_DEPENDENCY_SHA256_V3: [u8; 32] = [
    0x61, 0xc6, 0x87, 0x29, 0x78, 0x64, 0x07, 0x47, 0x96, 0xb9, 0x7c, 0x0a, 0x95, 0xa6, 0x19, 0x95,
    0x5c, 0x18, 0x6a, 0x2f, 0x62, 0x00, 0x71, 0x57, 0xe3, 0xd8, 0xf1, 0xaf, 0x17, 0xec, 0x6a, 0xec,
];
/// SHA-256 of the pinned Verus executable.
pub const DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V3: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
/// SHA-256 of the complete pinned Verus/vstd/Z3 closure manifest.
pub const DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V3: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicConstrainedAffineBoundsProofBindingV3 {
    proof_source_sha256: [u8; 32],
    v2_dependency_source_sha256: [u8; 32],
    verus_executable_sha256: [u8; 32],
    verus_closure_manifest_sha256: [u8; 32],
}

impl DynamicConstrainedAffineBoundsProofBindingV3 {
    pub const fn proof_source_sha256(self) -> [u8; 32] {
        self.proof_source_sha256
    }

    pub const fn verus_executable_sha256(self) -> [u8; 32] {
        self.verus_executable_sha256
    }

    pub const fn v2_dependency_source_sha256(self) -> [u8; 32] {
        self.v2_dependency_source_sha256
    }

    pub const fn verus_closure_manifest_sha256(self) -> [u8; 32] {
        self.verus_closure_manifest_sha256
    }
}

#[must_use = "dropping this value abandons the independently checked V3 theorem"]
pub struct VerifiedCompilerDynamicConstrainedAffineBoundsV3 {
    certificate: DynamicConstrainedAffineBoundsCertificateV3,
    proof_binding: DynamicConstrainedAffineBoundsProofBindingV3,
}

impl VerifiedCompilerDynamicConstrainedAffineBoundsV3 {
    pub const fn certificate(&self) -> &DynamicConstrainedAffineBoundsCertificateV3 {
        &self.certificate
    }

    pub const fn proof_binding(&self) -> DynamicConstrainedAffineBoundsProofBindingV3 {
        self.proof_binding
    }

    pub const fn establishes_nonempty_domain_and_dynamic_bound(&self) -> bool {
        true
    }

    pub const fn grants_race_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for VerifiedCompilerDynamicConstrainedAffineBoundsV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompilerDynamicConstrainedAffineBoundsV3")
            .field("certificate", &self.certificate)
            .field("proof_binding", &self.proof_binding)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerDynamicConstrainedAffineBoundsVerificationErrorV3 {
    source: DynamicConstrainedAffineBoundsCertificateErrorV3,
}

impl CompilerDynamicConstrainedAffineBoundsVerificationErrorV3 {
    pub const fn source_kind(self) -> DynamicConstrainedAffineBoundsCertificateErrorV3 {
        self.source
    }
}

impl fmt::Display for CompilerDynamicConstrainedAffineBoundsVerificationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiler dynamic affine-bounds certificate rejected: {}",
            self.source
        )
    }
}

impl Error for CompilerDynamicConstrainedAffineBoundsVerificationErrorV3 {}

pub fn verify_compiler_dynamic_constrained_affine_bounds_certificate_v3(
    certificate: &DynamicConstrainedAffineBoundsCertificateV3,
) -> Result<
    VerifiedCompilerDynamicConstrainedAffineBoundsV3,
    CompilerDynamicConstrainedAffineBoundsVerificationErrorV3,
> {
    check_dynamic_constrained_affine_bounds_certificate_v3(certificate)
        .map_err(|source| CompilerDynamicConstrainedAffineBoundsVerificationErrorV3 { source })?;
    Ok(VerifiedCompilerDynamicConstrainedAffineBoundsV3 {
        certificate: certificate.clone(),
        proof_binding: DynamicConstrainedAffineBoundsProofBindingV3 {
            proof_source_sha256: DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V3,
            v2_dependency_source_sha256: DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_V2_DEPENDENCY_SHA256_V3,
            verus_executable_sha256: DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V3,
            verus_closure_manifest_sha256:
                DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V3,
        },
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn proof_binding_matches_exact_sources_and_runtime_closure() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "../../fe2o3-proof-contracts/verus/dynamic_constrained_affine_bounds_v3.rs"
            ))),
            DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V3
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "../../fe2o3-proof-contracts/verus/constrained_affine_bounds_v2.rs"
            ))),
            DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_V2_DEPENDENCY_SHA256_V3
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "../../fe2o3-runtime-model/verus/pins/VERUS_CLOSURE_MANIFEST"
            ))),
            DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V3
        );
    }
}
