//! Solver-neutral certificate for one affine expression over a constrained integer box.
//!
//! A V2 query adds a bounded conjunction of affine inequalities to the V1
//! rectangular domain. The certificate contains nonnegative integer
//! multipliers for a canonical row system. Checking those multipliers is a
//! small, deterministic proof of `0 <= f(x) < extent` for every admitted point;
//! no search result or solver status is trusted.

use alloc::{vec, vec::Vec};
use core::fmt;

/// Maximum rank admitted by the constrained affine checker.
pub const MAX_CONSTRAINED_AFFINE_RANK_V2: usize = 16;
/// Maximum number of caller-supplied inequalities.
pub const MAX_CONSTRAINED_AFFINE_CONSTRAINTS_V2: usize = 256;
/// Maximum individual Farkas multiplier admitted by the checker.
pub const MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2: u64 = 1_048_576;

/// One exact affine inequality `constant + coefficients . x <= 0`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineInequalityV2 {
    constant: i128,
    coefficients: Vec<i128>,
}

impl AffineInequalityV2 {
    /// Constructs inert inequality data. The certificate checker validates rank.
    pub fn new(constant: i128, coefficients: Vec<i128>) -> Self {
        Self {
            constant,
            coefficients,
        }
    }

    pub const fn constant(&self) -> i128 {
        self.constant
    }

    pub fn coefficients(&self) -> &[i128] {
        &self.coefficients
    }

    /// Evaluates the inequality's left side with checked arithmetic.
    pub fn evaluate(
        &self,
        point: &[i128],
    ) -> Result<i128, ConstrainedAffineBoundsCertificateErrorV2> {
        if point.len() != self.coefficients.len() {
            return Err(ConstrainedAffineBoundsCertificateErrorV2::PointRankMismatch);
        }
        checked_affine_sum(self.constant, &self.coefficients, point)
    }
}

/// Exact constrained affine-bounds query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstrainedAffineBoundsQueryV2 {
    lower: Vec<i128>,
    upper_exclusive: Vec<i128>,
    constraints: Vec<AffineInequalityV2>,
    constant: i128,
    coefficients: Vec<i128>,
    extent: u64,
}

impl ConstrainedAffineBoundsQueryV2 {
    /// Constructs inert query data. The checker validates every bound and row.
    pub fn new(
        lower: Vec<i128>,
        upper_exclusive: Vec<i128>,
        constraints: Vec<AffineInequalityV2>,
        constant: i128,
        coefficients: Vec<i128>,
        extent: u64,
    ) -> Self {
        Self {
            lower,
            upper_exclusive,
            constraints,
            constant,
            coefficients,
            extent,
        }
    }

    pub fn lower(&self) -> &[i128] {
        &self.lower
    }

    pub fn upper_exclusive(&self) -> &[i128] {
        &self.upper_exclusive
    }

    pub fn constraints(&self) -> &[AffineInequalityV2] {
        &self.constraints
    }

    pub const fn constant(&self) -> i128 {
        self.constant
    }

    pub fn coefficients(&self) -> &[i128] {
        &self.coefficients
    }

    pub const fn extent(&self) -> u64 {
        self.extent
    }

    /// Evaluates the result expression at one point in the exact constrained domain.
    pub fn evaluate(
        &self,
        point: &[i128],
    ) -> Result<i128, ConstrainedAffineBoundsCertificateErrorV2> {
        validate_query(self)?;
        if point.len() != self.coefficients.len() {
            return Err(ConstrainedAffineBoundsCertificateErrorV2::PointRankMismatch);
        }
        if point
            .iter()
            .zip(&self.lower)
            .zip(&self.upper_exclusive)
            .any(|((coordinate, lower), upper)| coordinate < lower || coordinate >= upper)
        {
            return Err(ConstrainedAffineBoundsCertificateErrorV2::PointOutsideDomain);
        }
        for (constraint, row) in self.constraints.iter().enumerate() {
            if row.evaluate(point)? > 0 {
                return Err(
                    ConstrainedAffineBoundsCertificateErrorV2::PointViolatesConstraint {
                        constraint,
                    },
                );
            }
        }
        checked_affine_sum(self.constant, &self.coefficients, point)
    }
}

/// Untrusted nonnegative linear-combination certificate.
///
/// Canonical rows are the supplied constraints in order, followed for each
/// dimension by `lower[i] - x[i] <= 0` and then
/// `x[i] - (upper_exclusive[i] - 1) <= 0`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstrainedAffineBoundsCertificateV2 {
    query: ConstrainedAffineBoundsQueryV2,
    domain_witness: Vec<i128>,
    lower_multipliers: Vec<u64>,
    upper_multipliers: Vec<u64>,
}

impl ConstrainedAffineBoundsCertificateV2 {
    /// Constructs inert certificate data. This does not establish bounds.
    pub fn new(
        query: ConstrainedAffineBoundsQueryV2,
        domain_witness: Vec<i128>,
        lower_multipliers: Vec<u64>,
        upper_multipliers: Vec<u64>,
    ) -> Self {
        Self {
            query,
            domain_witness,
            lower_multipliers,
            upper_multipliers,
        }
    }

    pub const fn query(&self) -> &ConstrainedAffineBoundsQueryV2 {
        &self.query
    }

    /// Exact checked point proving that the constrained domain is nonempty.
    pub fn domain_witness(&self) -> &[i128] {
        &self.domain_witness
    }

    pub fn lower_multipliers(&self) -> &[u64] {
        &self.lower_multipliers
    }

    pub fn upper_multipliers(&self) -> &[u64] {
        &self.upper_multipliers
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

/// Opaque result of checking one exact constrained theorem.
#[derive(Debug)]
pub struct CheckedConstrainedAffineBoundsCertificateV2<'a> {
    certificate: &'a ConstrainedAffineBoundsCertificateV2,
}

impl<'a> CheckedConstrainedAffineBoundsCertificateV2<'a> {
    pub const fn certificate(&self) -> &'a ConstrainedAffineBoundsCertificateV2 {
        self.certificate
    }

    pub const fn establishes_nonnegative_strict_upper_bound(&self) -> bool {
        true
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

/// Fail-closed constrained-certificate error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstrainedAffineBoundsCertificateErrorV2 {
    EmptyRank,
    RankMismatch,
    RankLimitExceeded { actual: usize, limit: usize },
    ConstraintLimitExceeded { actual: usize, limit: usize },
    ConstraintRankMismatch { constraint: usize },
    EmptyDomainDimension { dimension: usize },
    ZeroExtent,
    PointRankMismatch,
    PointOutsideDomain,
    PointViolatesConstraint { constraint: usize },
    MultiplierCountMismatch,
    MultiplierLimitExceeded { row: usize },
    ArithmeticOverflow,
    LowerCoefficientMismatch { dimension: usize },
    UpperCoefficientMismatch { dimension: usize },
    LowerConstantNotDominated,
    UpperConstantNotDominated,
}

impl fmt::Display for ConstrainedAffineBoundsCertificateErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRank => formatter.write_str("constrained affine rank is zero"),
            Self::RankMismatch => formatter.write_str("constrained affine query ranks differ"),
            Self::RankLimitExceeded { actual, limit } => {
                write!(
                    formatter,
                    "constrained affine rank {actual} exceeds {limit}"
                )
            }
            Self::ConstraintLimitExceeded { actual, limit } => write!(
                formatter,
                "constrained affine constraint count {actual} exceeds {limit}",
            ),
            Self::ConstraintRankMismatch { constraint } => write!(
                formatter,
                "constrained affine constraint {constraint} has the wrong rank",
            ),
            Self::EmptyDomainDimension { dimension } => write!(
                formatter,
                "constrained affine domain dimension {dimension} is empty",
            ),
            Self::ZeroExtent => formatter.write_str("constrained affine extent is zero"),
            Self::PointRankMismatch => formatter.write_str("constrained affine point rank differs"),
            Self::PointOutsideDomain => {
                formatter.write_str("constrained affine point is outside the box")
            }
            Self::PointViolatesConstraint { constraint } => write!(
                formatter,
                "constrained affine point violates constraint {constraint}",
            ),
            Self::MultiplierCountMismatch => {
                formatter.write_str("constrained affine multiplier count differs")
            }
            Self::MultiplierLimitExceeded { row } => {
                write!(
                    formatter,
                    "constrained affine multiplier {row} exceeds its limit"
                )
            }
            Self::ArithmeticOverflow => {
                formatter.write_str("constrained affine certificate arithmetic overflow")
            }
            Self::LowerCoefficientMismatch { dimension } => write!(
                formatter,
                "constrained affine lower combination coefficient {dimension} differs",
            ),
            Self::UpperCoefficientMismatch { dimension } => write!(
                formatter,
                "constrained affine upper combination coefficient {dimension} differs",
            ),
            Self::LowerConstantNotDominated => {
                formatter.write_str("constrained affine lower constant is not dominated")
            }
            Self::UpperConstantNotDominated => {
                formatter.write_str("constrained affine upper constant is not dominated")
            }
        }
    }
}

/// Checks a nonnegative linear-combination proof for both result bounds.
///
/// Acceptance establishes that every point satisfying the exact nonempty box
/// and every retained affine constraint also satisfies `0 <= f(x) < extent`.
pub fn check_constrained_affine_bounds_certificate_v2(
    certificate: &ConstrainedAffineBoundsCertificateV2,
) -> Result<
    CheckedConstrainedAffineBoundsCertificateV2<'_>,
    ConstrainedAffineBoundsCertificateErrorV2,
> {
    let query = certificate.query();
    validate_query(query)?;
    // In addition to the universal implication below, require one concrete
    // satisfying point so contradictory constraints cannot certify a vacuous
    // property as a useful production theorem.
    let _ = query.evaluate(&certificate.domain_witness)?;
    let rows = canonical_rows(query)?;
    if certificate.lower_multipliers.len() != rows.len()
        || certificate.upper_multipliers.len() != rows.len()
    {
        return Err(ConstrainedAffineBoundsCertificateErrorV2::MultiplierCountMismatch);
    }
    validate_multipliers(&certificate.lower_multipliers)?;
    validate_multipliers(&certificate.upper_multipliers)?;

    let lower_constant = query
        .constant
        .checked_neg()
        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
    let lower_coefficients = query
        .coefficients
        .iter()
        .map(|coefficient| coefficient.checked_neg())
        .collect::<Option<Vec<_>>>()
        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
    check_combination(
        &rows,
        &certificate.lower_multipliers,
        lower_constant,
        &lower_coefficients,
        true,
    )?;

    let last = i128::from(query.extent)
        .checked_sub(1)
        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
    let upper_constant = query
        .constant
        .checked_sub(last)
        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
    check_combination(
        &rows,
        &certificate.upper_multipliers,
        upper_constant,
        &query.coefficients,
        false,
    )?;
    Ok(CheckedConstrainedAffineBoundsCertificateV2 { certificate })
}

fn validate_query(
    query: &ConstrainedAffineBoundsQueryV2,
) -> Result<(), ConstrainedAffineBoundsCertificateErrorV2> {
    let rank = query.coefficients.len();
    if rank == 0 {
        return Err(ConstrainedAffineBoundsCertificateErrorV2::EmptyRank);
    }
    if query.lower.len() != rank || query.upper_exclusive.len() != rank {
        return Err(ConstrainedAffineBoundsCertificateErrorV2::RankMismatch);
    }
    if rank > MAX_CONSTRAINED_AFFINE_RANK_V2 {
        return Err(
            ConstrainedAffineBoundsCertificateErrorV2::RankLimitExceeded {
                actual: rank,
                limit: MAX_CONSTRAINED_AFFINE_RANK_V2,
            },
        );
    }
    if query.constraints.len() > MAX_CONSTRAINED_AFFINE_CONSTRAINTS_V2 {
        return Err(
            ConstrainedAffineBoundsCertificateErrorV2::ConstraintLimitExceeded {
                actual: query.constraints.len(),
                limit: MAX_CONSTRAINED_AFFINE_CONSTRAINTS_V2,
            },
        );
    }
    for (constraint, row) in query.constraints.iter().enumerate() {
        if row.coefficients.len() != rank {
            return Err(
                ConstrainedAffineBoundsCertificateErrorV2::ConstraintRankMismatch { constraint },
            );
        }
    }
    for dimension in 0..rank {
        if query.lower[dimension] >= query.upper_exclusive[dimension] {
            return Err(
                ConstrainedAffineBoundsCertificateErrorV2::EmptyDomainDimension { dimension },
            );
        }
    }
    if query.extent == 0 {
        return Err(ConstrainedAffineBoundsCertificateErrorV2::ZeroExtent);
    }
    Ok(())
}

fn canonical_rows(
    query: &ConstrainedAffineBoundsQueryV2,
) -> Result<Vec<AffineInequalityV2>, ConstrainedAffineBoundsCertificateErrorV2> {
    let rank = query.coefficients.len();
    let mut rows = query.constraints.clone();
    for dimension in 0..rank {
        let mut lower_coefficients = vec![0; rank];
        lower_coefficients[dimension] = -1;
        rows.push(AffineInequalityV2::new(
            query.lower[dimension],
            lower_coefficients,
        ));
        let mut upper_coefficients = vec![0; rank];
        upper_coefficients[dimension] = 1;
        let last = query.upper_exclusive[dimension]
            .checked_sub(1)
            .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
        rows.push(AffineInequalityV2::new(
            last.checked_neg()
                .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?,
            upper_coefficients,
        ));
    }
    Ok(rows)
}

fn validate_multipliers(
    multipliers: &[u64],
) -> Result<(), ConstrainedAffineBoundsCertificateErrorV2> {
    if let Some((row, _)) = multipliers
        .iter()
        .enumerate()
        .find(|(_, multiplier)| **multiplier > MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2)
    {
        return Err(ConstrainedAffineBoundsCertificateErrorV2::MultiplierLimitExceeded { row });
    }
    Ok(())
}

fn check_combination(
    rows: &[AffineInequalityV2],
    multipliers: &[u64],
    target_constant: i128,
    target_coefficients: &[i128],
    lower: bool,
) -> Result<(), ConstrainedAffineBoundsCertificateErrorV2> {
    let mut combined_constant = 0_i128;
    let mut combined_coefficients = vec![0_i128; target_coefficients.len()];
    for (row, multiplier) in rows.iter().zip(multipliers) {
        let multiplier = i128::from(*multiplier);
        combined_constant = combined_constant
            .checked_add(
                row.constant
                    .checked_mul(multiplier)
                    .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
        for (combined, coefficient) in combined_coefficients.iter_mut().zip(&row.coefficients) {
            *combined = combined
                .checked_add(
                    coefficient
                        .checked_mul(multiplier)
                        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?,
                )
                .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?;
        }
    }
    if let Some(dimension) = combined_coefficients
        .iter()
        .zip(target_coefficients)
        .position(|(combined, target)| combined != target)
    {
        return Err(if lower {
            ConstrainedAffineBoundsCertificateErrorV2::LowerCoefficientMismatch { dimension }
        } else {
            ConstrainedAffineBoundsCertificateErrorV2::UpperCoefficientMismatch { dimension }
        });
    }
    if target_constant > combined_constant {
        return Err(if lower {
            ConstrainedAffineBoundsCertificateErrorV2::LowerConstantNotDominated
        } else {
            ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated
        });
    }
    Ok(())
}

fn checked_affine_sum(
    constant: i128,
    coefficients: &[i128],
    point: &[i128],
) -> Result<i128, ConstrainedAffineBoundsCertificateErrorV2> {
    coefficients
        .iter()
        .zip(point)
        .try_fold(constant, |value, (coefficient, coordinate)| {
            value
                .checked_add(
                    coefficient
                        .checked_mul(*coordinate)
                        .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)?,
                )
                .ok_or(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow)
        })
}
