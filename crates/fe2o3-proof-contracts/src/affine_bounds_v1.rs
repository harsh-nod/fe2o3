//! Solver-neutral certificate for one affine expression over an integer box.
//!
//! The formal query semantics are
//! `f(x) = constant + sum(coefficients[i] * x[i])` over every integer point
//! satisfying `lower[i] <= x[i] < upper_exclusive[i]`. A checked certificate
//! establishes only `0 <= f(x) < extent` for that exact query.

use alloc::vec::Vec;
use core::fmt;

/// Maximum rank admitted by the V1 affine-bounds checker.
pub const MAX_AFFINE_BOUNDS_RANK_V1: usize = 16;

/// Exact mathematical-integer affine-bounds query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineBoundsQueryV1 {
    lower: Vec<i128>,
    upper_exclusive: Vec<i128>,
    constant: i128,
    coefficients: Vec<i128>,
    extent: u64,
}

impl AffineBoundsQueryV1 {
    /// Constructs inert query data. The checker validates all dimensions.
    pub fn new(
        lower: Vec<i128>,
        upper_exclusive: Vec<i128>,
        constant: i128,
        coefficients: Vec<i128>,
        extent: u64,
    ) -> Self {
        Self {
            lower,
            upper_exclusive,
            constant,
            coefficients,
            extent,
        }
    }

    /// Inclusive lower coordinate bounds.
    pub fn lower(&self) -> &[i128] {
        &self.lower
    }

    /// Exclusive upper coordinate bounds.
    pub fn upper_exclusive(&self) -> &[i128] {
        &self.upper_exclusive
    }

    /// Affine constant term.
    pub const fn constant(&self) -> i128 {
        self.constant
    }

    /// Affine coefficients in domain-dimension order.
    pub fn coefficients(&self) -> &[i128] {
        &self.coefficients
    }

    /// Exclusive nonnegative result bound.
    pub const fn extent(&self) -> u64 {
        self.extent
    }

    /// Evaluates the formal query semantics at one admitted point.
    pub fn evaluate(&self, point: &[i128]) -> Result<i128, AffineBoundsCertificateErrorV1> {
        validate_query(self)?;
        if point.len() != self.coefficients.len() {
            return Err(AffineBoundsCertificateErrorV1::PointRankMismatch);
        }
        if point
            .iter()
            .zip(&self.lower)
            .zip(&self.upper_exclusive)
            .any(|((coordinate, lower), upper)| coordinate < lower || coordinate >= upper)
        {
            return Err(AffineBoundsCertificateErrorV1::PointOutsideDomain);
        }
        checked_affine_sum(self.constant, &self.coefficients, point)
    }
}

/// Untrusted endpoint-extrema certificate for one exact query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineBoundsCertificateV1 {
    query: AffineBoundsQueryV1,
    minimum_coordinates: Vec<i128>,
    maximum_coordinates: Vec<i128>,
    minimum: i128,
    maximum: i128,
}

impl AffineBoundsCertificateV1 {
    /// Constructs inert certificate data. This does not establish bounds.
    pub fn new(
        query: AffineBoundsQueryV1,
        minimum_coordinates: Vec<i128>,
        maximum_coordinates: Vec<i128>,
        minimum: i128,
        maximum: i128,
    ) -> Self {
        Self {
            query,
            minimum_coordinates,
            maximum_coordinates,
            minimum,
            maximum,
        }
    }

    /// Exact certified query.
    pub const fn query(&self) -> &AffineBoundsQueryV1 {
        &self.query
    }

    /// Per-dimension endpoint selected for the minimum.
    pub fn minimum_coordinates(&self) -> &[i128] {
        &self.minimum_coordinates
    }

    /// Per-dimension endpoint selected for the maximum.
    pub fn maximum_coordinates(&self) -> &[i128] {
        &self.maximum_coordinates
    }

    /// Claimed inclusive expression minimum.
    pub const fn minimum(&self) -> i128 {
        self.minimum
    }

    /// Claimed inclusive expression maximum.
    pub const fn maximum(&self) -> i128 {
        self.maximum
    }

    /// Returns false because inert certificate bytes authenticate no producer.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Returns false because this certificate grants no lowering or launch authority.
    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

/// Opaque result of checking one exact affine-bounds certificate.
#[derive(Debug)]
pub struct CheckedAffineBoundsCertificateV1<'a> {
    certificate: &'a AffineBoundsCertificateV1,
}

impl<'a> CheckedAffineBoundsCertificateV1<'a> {
    /// Exact certificate that passed the checker.
    pub const fn certificate(&self) -> &'a AffineBoundsCertificateV1 {
        self.certificate
    }

    /// Returns the theorem established for all points in the exact query domain.
    pub const fn establishes_nonnegative_strict_upper_bound(&self) -> bool {
        true
    }

    /// Returns false because checking this local theorem grants no compiler authority.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

/// Fail-closed affine-bounds certificate error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffineBoundsCertificateErrorV1 {
    /// Query vectors do not have identical rank.
    RankMismatch,
    /// Query rank exceeds the fixed checker limit.
    RankLimitExceeded { actual: usize, limit: usize },
    /// One domain interval is empty or reversed.
    EmptyDomainDimension { dimension: usize },
    /// The requested extent is empty.
    ZeroExtent,
    /// A point has the wrong rank.
    PointRankMismatch,
    /// A point lies outside the exact query domain.
    PointOutsideDomain,
    /// Endpoint vectors do not have the query rank.
    EndpointRankMismatch,
    /// A minimum endpoint differs from the coefficient-directed endpoint.
    MinimumEndpointMismatch { dimension: usize },
    /// A maximum endpoint differs from the coefficient-directed endpoint.
    MaximumEndpointMismatch { dimension: usize },
    /// Exact mathematical evaluation cannot be represented in `i128`.
    ArithmeticOverflow,
    /// The claimed minimum differs from exact endpoint evaluation.
    MinimumMismatch,
    /// The claimed maximum differs from exact endpoint evaluation.
    MaximumMismatch,
    /// The exact extrema do not establish `0 <= f(x) < extent`.
    BoundNotEstablished,
}

impl fmt::Display for AffineBoundsCertificateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankMismatch => formatter.write_str("affine bounds query ranks differ"),
            Self::RankLimitExceeded { actual, limit } => {
                write!(formatter, "affine bounds rank {actual} exceeds {limit}")
            }
            Self::EmptyDomainDimension { dimension } => {
                write!(
                    formatter,
                    "affine bounds domain dimension {dimension} is empty"
                )
            }
            Self::ZeroExtent => formatter.write_str("affine bounds extent is zero"),
            Self::PointRankMismatch => formatter.write_str("affine point rank differs"),
            Self::PointOutsideDomain => formatter.write_str("affine point is outside the domain"),
            Self::EndpointRankMismatch => formatter.write_str("affine endpoint ranks differ"),
            Self::MinimumEndpointMismatch { dimension } => {
                write!(formatter, "affine minimum endpoint {dimension} is invalid")
            }
            Self::MaximumEndpointMismatch { dimension } => {
                write!(formatter, "affine maximum endpoint {dimension} is invalid")
            }
            Self::ArithmeticOverflow => {
                formatter.write_str("affine certificate arithmetic overflow")
            }
            Self::MinimumMismatch => formatter.write_str("affine certificate minimum mismatch"),
            Self::MaximumMismatch => formatter.write_str("affine certificate maximum mismatch"),
            Self::BoundNotEstablished => {
                formatter.write_str("affine certificate does not establish the requested bounds")
            }
        }
    }
}

/// Checks the endpoint certificate using exact, overflow-detecting `i128` arithmetic.
///
/// Acceptance establishes, for the formal semantics in [`AffineBoundsQueryV1`],
/// that every domain point evaluates to a value in `[0, extent)`. The
/// coefficient sign determines the minimizing and maximizing endpoint for each
/// independent box dimension.
pub fn check_affine_bounds_certificate_v1(
    certificate: &AffineBoundsCertificateV1,
) -> Result<CheckedAffineBoundsCertificateV1<'_>, AffineBoundsCertificateErrorV1> {
    let query = certificate.query();
    validate_query(query)?;
    let rank = query.coefficients.len();
    if certificate.minimum_coordinates.len() != rank
        || certificate.maximum_coordinates.len() != rank
    {
        return Err(AffineBoundsCertificateErrorV1::EndpointRankMismatch);
    }

    for dimension in 0..rank {
        let last = query.upper_exclusive[dimension]
            .checked_sub(1)
            .ok_or(AffineBoundsCertificateErrorV1::ArithmeticOverflow)?;
        let (expected_minimum, expected_maximum) = if query.coefficients[dimension] >= 0 {
            (query.lower[dimension], last)
        } else {
            (last, query.lower[dimension])
        };
        if certificate.minimum_coordinates[dimension] != expected_minimum {
            return Err(AffineBoundsCertificateErrorV1::MinimumEndpointMismatch { dimension });
        }
        if certificate.maximum_coordinates[dimension] != expected_maximum {
            return Err(AffineBoundsCertificateErrorV1::MaximumEndpointMismatch { dimension });
        }
    }

    let minimum = checked_affine_sum(
        query.constant,
        &query.coefficients,
        &certificate.minimum_coordinates,
    )?;
    let maximum = checked_affine_sum(
        query.constant,
        &query.coefficients,
        &certificate.maximum_coordinates,
    )?;
    if minimum != certificate.minimum {
        return Err(AffineBoundsCertificateErrorV1::MinimumMismatch);
    }
    if maximum != certificate.maximum {
        return Err(AffineBoundsCertificateErrorV1::MaximumMismatch);
    }
    if minimum < 0 || maximum >= i128::from(query.extent) {
        return Err(AffineBoundsCertificateErrorV1::BoundNotEstablished);
    }
    Ok(CheckedAffineBoundsCertificateV1 { certificate })
}

fn validate_query(query: &AffineBoundsQueryV1) -> Result<(), AffineBoundsCertificateErrorV1> {
    let rank = query.coefficients.len();
    if query.lower.len() != rank || query.upper_exclusive.len() != rank {
        return Err(AffineBoundsCertificateErrorV1::RankMismatch);
    }
    if rank > MAX_AFFINE_BOUNDS_RANK_V1 {
        return Err(AffineBoundsCertificateErrorV1::RankLimitExceeded {
            actual: rank,
            limit: MAX_AFFINE_BOUNDS_RANK_V1,
        });
    }
    if query.extent == 0 {
        return Err(AffineBoundsCertificateErrorV1::ZeroExtent);
    }
    for dimension in 0..rank {
        if query.lower[dimension] >= query.upper_exclusive[dimension] {
            return Err(AffineBoundsCertificateErrorV1::EmptyDomainDimension { dimension });
        }
    }
    Ok(())
}

fn checked_affine_sum(
    constant: i128,
    coefficients: &[i128],
    point: &[i128],
) -> Result<i128, AffineBoundsCertificateErrorV1> {
    coefficients
        .iter()
        .zip(point)
        .try_fold(constant, |value, (coefficient, coordinate)| {
            value
                .checked_add(
                    coefficient
                        .checked_mul(*coordinate)
                        .ok_or(AffineBoundsCertificateErrorV1::ArithmeticOverflow)?,
                )
                .ok_or(AffineBoundsCertificateErrorV1::ArithmeticOverflow)
        })
}
