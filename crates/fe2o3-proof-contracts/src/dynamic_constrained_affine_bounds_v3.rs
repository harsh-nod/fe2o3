//! Relational affine-bounds certificate for a runtime extent.
//!
//! V3 composes two V2 certificates over one exact nonempty constrained
//! domain. The first proves `index >= 0`; the second proves
//! `extent - index - 1 >= 0`. The checker independently validates both V2
//! certificates and the exact affine relation before exposing `index < extent`.

use alloc::vec::Vec;
use core::fmt;

use crate::{
    ConstrainedAffineBoundsCertificateErrorV2, ConstrainedAffineBoundsCertificateV2,
    check_constrained_affine_bounds_certificate_v2,
};

/// Strict constant ceiling used only to make both composed V2 theorems bounded.
pub const DYNAMIC_AFFINE_COMPONENT_CEILING_V3: u64 = u64::MAX;

/// Untrusted relational certificate. Construction establishes no property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicConstrainedAffineBoundsCertificateV3 {
    extent_constant: i128,
    extent_coefficients: Vec<i128>,
    index_certificate: ConstrainedAffineBoundsCertificateV2,
    slack_certificate: ConstrainedAffineBoundsCertificateV2,
}

impl DynamicConstrainedAffineBoundsCertificateV3 {
    pub fn new(
        extent_constant: i128,
        extent_coefficients: Vec<i128>,
        index_certificate: ConstrainedAffineBoundsCertificateV2,
        slack_certificate: ConstrainedAffineBoundsCertificateV2,
    ) -> Self {
        Self {
            extent_constant,
            extent_coefficients,
            index_certificate,
            slack_certificate,
        }
    }

    pub const fn extent_constant(&self) -> i128 {
        self.extent_constant
    }

    pub fn extent_coefficients(&self) -> &[i128] {
        &self.extent_coefficients
    }

    pub const fn index_certificate(&self) -> &ConstrainedAffineBoundsCertificateV2 {
        &self.index_certificate
    }

    pub const fn slack_certificate(&self) -> &ConstrainedAffineBoundsCertificateV2 {
        &self.slack_certificate
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

/// Opaque result of checking one exact runtime-extent theorem.
#[derive(Debug)]
pub struct CheckedDynamicConstrainedAffineBoundsCertificateV3<'a> {
    certificate: &'a DynamicConstrainedAffineBoundsCertificateV3,
}

impl<'a> CheckedDynamicConstrainedAffineBoundsCertificateV3<'a> {
    pub const fn certificate(&self) -> &'a DynamicConstrainedAffineBoundsCertificateV3 {
        self.certificate
    }

    pub const fn establishes_nonempty_domain_and_dynamic_bound(&self) -> bool {
        true
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

/// Fail-closed V3 composition error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicConstrainedAffineBoundsCertificateErrorV3 {
    IndexCertificate(ConstrainedAffineBoundsCertificateErrorV2),
    SlackCertificate(ConstrainedAffineBoundsCertificateErrorV2),
    TooFewConstraints,
    DuplicateConstraint { second: usize },
    ComponentCeilingMismatch,
    DomainMismatch,
    WitnessMismatch,
    ExtentRankMismatch,
    ArithmeticOverflow,
    SlackConstantMismatch,
    SlackCoefficientMismatch { dimension: usize },
}

impl fmt::Display for DynamicConstrainedAffineBoundsCertificateErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexCertificate(error) => write!(formatter, "index certificate: {error}"),
            Self::SlackCertificate(error) => write!(formatter, "slack certificate: {error}"),
            Self::TooFewConstraints => {
                formatter.write_str("dynamic affine certificate has fewer than two constraints")
            }
            Self::DuplicateConstraint { second } => {
                write!(
                    formatter,
                    "dynamic affine constraint {second} is duplicated"
                )
            }
            Self::ComponentCeilingMismatch => {
                formatter.write_str("dynamic affine component ceiling differs")
            }
            Self::DomainMismatch => formatter.write_str("dynamic affine domains differ"),
            Self::WitnessMismatch => formatter.write_str("dynamic affine witnesses differ"),
            Self::ExtentRankMismatch => formatter.write_str("dynamic affine extent rank differs"),
            Self::ArithmeticOverflow => formatter.write_str("dynamic affine relation overflowed"),
            Self::SlackConstantMismatch => {
                formatter.write_str("dynamic affine slack constant differs")
            }
            Self::SlackCoefficientMismatch { dimension } => {
                write!(
                    formatter,
                    "dynamic affine slack coefficient {dimension} differs"
                )
            }
        }
    }
}

/// Checks `0 <= index < extent` over one exact, nonempty constrained domain.
pub fn check_dynamic_constrained_affine_bounds_certificate_v3(
    certificate: &DynamicConstrainedAffineBoundsCertificateV3,
) -> Result<
    CheckedDynamicConstrainedAffineBoundsCertificateV3<'_>,
    DynamicConstrainedAffineBoundsCertificateErrorV3,
> {
    check_constrained_affine_bounds_certificate_v2(&certificate.index_certificate)
        .map_err(DynamicConstrainedAffineBoundsCertificateErrorV3::IndexCertificate)?;
    check_constrained_affine_bounds_certificate_v2(&certificate.slack_certificate)
        .map_err(DynamicConstrainedAffineBoundsCertificateErrorV3::SlackCertificate)?;

    let index = certificate.index_certificate.query();
    let slack = certificate.slack_certificate.query();
    if index.constraints().len() < 2 {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::TooFewConstraints);
    }
    for second in 1..index.constraints().len() {
        if index.constraints()[..second].contains(&index.constraints()[second]) {
            return Err(
                DynamicConstrainedAffineBoundsCertificateErrorV3::DuplicateConstraint { second },
            );
        }
    }
    if index.extent() != DYNAMIC_AFFINE_COMPONENT_CEILING_V3
        || slack.extent() != DYNAMIC_AFFINE_COMPONENT_CEILING_V3
    {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::ComponentCeilingMismatch);
    }
    if index.lower() != slack.lower()
        || index.upper_exclusive() != slack.upper_exclusive()
        || index.constraints() != slack.constraints()
    {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::DomainMismatch);
    }
    if certificate.index_certificate.domain_witness()
        != certificate.slack_certificate.domain_witness()
    {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::WitnessMismatch);
    }
    if certificate.extent_coefficients.len() != index.coefficients().len() {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::ExtentRankMismatch);
    }
    let expected_constant = certificate
        .extent_constant
        .checked_sub(index.constant())
        .and_then(|constant| constant.checked_sub(1))
        .ok_or(DynamicConstrainedAffineBoundsCertificateErrorV3::ArithmeticOverflow)?;
    if slack.constant() != expected_constant {
        return Err(DynamicConstrainedAffineBoundsCertificateErrorV3::SlackConstantMismatch);
    }
    for (dimension, (extent, index)) in certificate
        .extent_coefficients
        .iter()
        .zip(index.coefficients())
        .enumerate()
    {
        let expected = extent
            .checked_sub(*index)
            .ok_or(DynamicConstrainedAffineBoundsCertificateErrorV3::ArithmeticOverflow)?;
        if slack.coefficients()[dimension] != expected {
            return Err(
                DynamicConstrainedAffineBoundsCertificateErrorV3::SlackCoefficientMismatch {
                    dimension,
                },
            );
        }
    }
    Ok(CheckedDynamicConstrainedAffineBoundsCertificateV3 { certificate })
}
