//! Independent admission of compiler-produced affine-box bounds certificates.

use std::{error::Error, fmt};

use fe2o3_proof_contracts::{
    AffineBoundsCertificateErrorV1, AffineBoundsCertificateV1, check_affine_bounds_certificate_v1,
};

/// Move-only custody of one independently checked affine-bounds theorem.
///
/// The theorem covers only the exact affine expression and integer box in the
/// retained certificate. It says nothing about source-to-Pliron
/// correspondence, non-affine indices, LLVM, machine code, or GPU execution.
#[must_use = "dropping this value abandons the independently checked affine-bounds theorem"]
pub struct VerifiedCompilerAffineBoundsV1 {
    certificate: AffineBoundsCertificateV1,
}

impl VerifiedCompilerAffineBoundsV1 {
    /// Retains the exact certificate that was independently checked.
    pub const fn certificate(&self) -> &AffineBoundsCertificateV1 {
        &self.certificate
    }

    /// Returns the one local theorem established by the checker.
    pub const fn establishes_nonnegative_strict_upper_bound(&self) -> bool {
        true
    }

    /// Returns false because this local theorem grants no compiler authority.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    /// Returns false because this local theorem grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for VerifiedCompilerAffineBoundsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompilerAffineBoundsV1")
            .field("certificate", &self.certificate)
            .finish_non_exhaustive()
    }
}

/// Independent affine-bounds certificate admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerAffineBoundsVerificationErrorV1 {
    source: AffineBoundsCertificateErrorV1,
}

impl CompilerAffineBoundsVerificationErrorV1 {
    /// Exact structural or arithmetic rejection from the canonical checker.
    pub const fn source_kind(self) -> AffineBoundsCertificateErrorV1 {
        self.source
    }
}

impl fmt::Display for CompilerAffineBoundsVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiler affine-bounds certificate rejected: {}",
            self.source
        )
    }
}

impl Error for CompilerAffineBoundsVerificationErrorV1 {}

/// Independently checks and retains one exact compiler-produced certificate.
pub fn verify_compiler_affine_bounds_certificate_v1(
    certificate: &AffineBoundsCertificateV1,
) -> Result<VerifiedCompilerAffineBoundsV1, CompilerAffineBoundsVerificationErrorV1> {
    check_affine_bounds_certificate_v1(certificate)
        .map_err(|source| CompilerAffineBoundsVerificationErrorV1 { source })?;
    Ok(VerifiedCompilerAffineBoundsV1 {
        certificate: certificate.clone(),
    })
}
