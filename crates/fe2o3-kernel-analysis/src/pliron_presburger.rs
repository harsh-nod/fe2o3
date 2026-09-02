//! Exact, resource-bounded Presburger queries shared by PLIRON kernel checks.
//!
//! The admitted fragment is intentionally small and explicit: finite integer
//! boxes, conjunctions of affine equalities and inequalities, constant-modulus
//! congruences, and affine/remainder maps. Exhaustive search with interval
//! pruning is a decision procedure for that finite fragment. Resource
//! exhaustion and unsupported expressions are reported as incomplete; neither
//! can be mistaken for a proof.

use std::collections::{HashMap, HashSet};
use std::fmt;

use fe2o3_proof_contracts::{
    AffineBoundsCertificateErrorV1, AffineBoundsCertificateV1, AffineBoundsQueryV1,
    AffineInequalityV2, ConstrainedAffineBoundsCertificateErrorV2,
    ConstrainedAffineBoundsCertificateV2, ConstrainedAffineBoundsQueryV2,
    MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2, check_affine_bounds_certificate_v1,
    check_constrained_affine_bounds_certificate_v2,
};

use crate::{SparseIndexAnalysisV1, SparseIndexFactV1};

pub const MAX_PRESBURGER_VARIABLES_V1: usize = 16;
pub const MAX_PRESBURGER_CONSTRAINTS_V1: usize = 256;
pub const MAX_PRESBURGER_OUTPUTS_V1: usize = 16;
pub const MAX_PRESBURGER_WORK_UNITS_V1: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerFailureV1 {
    InvalidModel { detail: &'static str },
    Unsupported { detail: &'static str },
    MachineIntegerOverflow { bits: u8, signed: bool },
    ArithmeticOverflow,
    ResourceLimit { limit: usize, actual: usize },
}

impl fmt::Display for PresburgerFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModel { detail } => {
                write!(formatter, "invalid Presburger model: {detail}")
            }
            Self::Unsupported { detail } => {
                write!(formatter, "unsupported Presburger relation: {detail}")
            }
            Self::MachineIntegerOverflow { bits, signed } => write!(
                formatter,
                "index expression may overflow a {bits}-bit {} machine integer",
                if *signed { "signed" } else { "unsigned" },
            ),
            Self::ArithmeticOverflow => {
                formatter.write_str("Presburger arithmetic overflowed signed 128-bit evaluation")
            }
            Self::ResourceLimit { limit, actual } => write!(
                formatter,
                "Presburger query requires more than {limit} work units (first refused unit {actual})",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerBoxV1 {
    lower: Vec<i128>,
    upper_exclusive: Vec<i128>,
}

impl PresburgerBoxV1 {
    pub fn new(lower: Vec<i128>, upper_exclusive: Vec<i128>) -> Result<Self, PresburgerFailureV1> {
        if lower.len() != upper_exclusive.len() {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "box bound vectors have different ranks",
            });
        }
        if lower.len() > MAX_PRESBURGER_VARIABLES_V1 {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "box rank exceeds the Presburger variable limit",
            });
        }
        Ok(Self {
            lower,
            upper_exclusive,
        })
    }

    pub fn zero_based(extents: &[u64]) -> Result<Self, PresburgerFailureV1> {
        let upper_exclusive = extents
            .iter()
            .map(|extent| i128::from(*extent))
            .collect::<Vec<_>>();
        Self::new(vec![0; extents.len()], upper_exclusive)
    }

    pub fn rank(&self) -> usize {
        self.lower.len()
    }

    pub fn lower(&self) -> &[i128] {
        &self.lower
    }

    pub fn upper_exclusive(&self) -> &[i128] {
        &self.upper_exclusive
    }

    fn is_empty(&self) -> bool {
        self.lower
            .iter()
            .zip(&self.upper_exclusive)
            .any(|(lower, upper)| lower >= upper)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerAffineExprV1 {
    constant: i128,
    coefficients: Vec<i128>,
}

impl PresburgerAffineExprV1 {
    pub fn new(constant: i128, coefficients: Vec<i128>) -> Result<Self, PresburgerFailureV1> {
        if coefficients.len() > MAX_PRESBURGER_VARIABLES_V1 {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "affine expression rank exceeds the Presburger variable limit",
            });
        }
        Ok(Self {
            constant,
            coefficients,
        })
    }

    pub fn constant(value: i128, rank: usize) -> Result<Self, PresburgerFailureV1> {
        Self::new(value, vec![0; rank])
    }

    pub fn variable(rank: usize, dimension: usize) -> Result<Self, PresburgerFailureV1> {
        if dimension >= rank {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "affine variable is outside the expression rank",
            });
        }
        let mut coefficients = vec![0; rank];
        coefficients[dimension] = 1;
        Self::new(0, coefficients)
    }

    pub const fn constant_term(&self) -> i128 {
        self.constant
    }

    pub fn coefficients(&self) -> &[i128] {
        &self.coefficients
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, PresburgerFailureV1> {
        self.check_same_rank(other)?;
        let coefficients = self
            .coefficients
            .iter()
            .zip(&other.coefficients)
            .map(|(left, right)| left.checked_add(*right))
            .collect::<Option<Vec<_>>>()
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        Self::new(
            self.constant
                .checked_add(other.constant)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
            coefficients,
        )
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, PresburgerFailureV1> {
        self.check_same_rank(other)?;
        let coefficients = self
            .coefficients
            .iter()
            .zip(&other.coefficients)
            .map(|(left, right)| left.checked_sub(*right))
            .collect::<Option<Vec<_>>>()
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        Self::new(
            self.constant
                .checked_sub(other.constant)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
            coefficients,
        )
    }

    pub fn checked_scale(&self, factor: i128) -> Result<Self, PresburgerFailureV1> {
        let coefficients = self
            .coefficients
            .iter()
            .map(|coefficient| coefficient.checked_mul(factor))
            .collect::<Option<Vec<_>>>()
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        Self::new(
            self.constant
                .checked_mul(factor)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
            coefficients,
        )
    }

    pub fn evaluate(&self, point: &[i128]) -> Result<i128, PresburgerFailureV1> {
        if point.len() != self.coefficients.len() {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "point rank differs from affine expression rank",
            });
        }
        self.coefficients.iter().zip(point).try_fold(
            self.constant,
            |value, (coefficient, coordinate)| {
                value
                    .checked_add(
                        coefficient
                            .checked_mul(*coordinate)
                            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
                    )
                    .ok_or(PresburgerFailureV1::ArithmeticOverflow)
            },
        )
    }

    fn check_same_rank(&self, other: &Self) -> Result<(), PresburgerFailureV1> {
        if self.coefficients.len() != other.coefficients.len() {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "affine expression ranks differ",
            });
        }
        Ok(())
    }

    fn interval(
        &self,
        point: &[i128],
        assigned: usize,
        domain: &PresburgerBoxV1,
    ) -> Result<(i128, i128), PresburgerFailureV1> {
        if self.coefficients.len() != domain.rank() || point.len() != domain.rank() {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "constraint expression rank differs from its domain",
            });
        }
        let mut minimum = self.constant;
        let mut maximum = self.constant;
        for (dimension, coefficient) in self.coefficients.iter().copied().enumerate() {
            let (low, high) = if dimension < assigned {
                (point[dimension], point[dimension])
            } else {
                let lower = domain.lower[dimension];
                let upper = domain.upper_exclusive[dimension]
                    .checked_sub(1)
                    .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
                if coefficient >= 0 {
                    (lower, upper)
                } else {
                    (upper, lower)
                }
            };
            minimum = minimum
                .checked_add(
                    coefficient
                        .checked_mul(low)
                        .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
                )
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
            maximum = maximum
                .checked_add(
                    coefficient
                        .checked_mul(high)
                        .ok_or(PresburgerFailureV1::ArithmeticOverflow)?,
                )
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        }
        Ok((minimum, maximum))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerConstraintV1 {
    LessEqualZero(PresburgerAffineExprV1),
    EqualZero(PresburgerAffineExprV1),
    CongruentZero {
        expression: PresburgerAffineExprV1,
        modulus: i128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerSetV1 {
    domain: PresburgerBoxV1,
    constraints: Vec<PresburgerConstraintV1>,
}

impl PresburgerSetV1 {
    pub fn new(
        domain: PresburgerBoxV1,
        constraints: Vec<PresburgerConstraintV1>,
    ) -> Result<Self, PresburgerFailureV1> {
        if constraints.len() > MAX_PRESBURGER_CONSTRAINTS_V1 {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "constraint count exceeds the Presburger limit",
            });
        }
        for constraint in &constraints {
            let (expression, valid_modulus) = match constraint {
                PresburgerConstraintV1::LessEqualZero(expression)
                | PresburgerConstraintV1::EqualZero(expression) => (expression, true),
                PresburgerConstraintV1::CongruentZero {
                    expression,
                    modulus,
                } => (expression, *modulus > 0),
            };
            if expression.coefficients.len() != domain.rank() || !valid_modulus {
                return Err(PresburgerFailureV1::InvalidModel {
                    detail: "constraint rank or modulus is invalid",
                });
            }
        }
        Ok(Self {
            domain,
            constraints,
        })
    }

    pub fn box_only(domain: PresburgerBoxV1) -> Self {
        Self {
            domain,
            constraints: Vec::new(),
        }
    }

    pub fn domain(&self) -> &PresburgerBoxV1 {
        &self.domain
    }

    pub fn constraints(&self) -> &[PresburgerConstraintV1] {
        &self.constraints
    }

    pub fn find_witness(&self) -> PresburgerSetDecisionV1 {
        let mut budget = PresburgerBudgetV1::default();
        match self.find_witness_with_budget(&mut budget) {
            Ok(Some(point)) => PresburgerSetDecisionV1::Witness(PresburgerWitnessV1 { point }),
            Ok(None) => PresburgerSetDecisionV1::Empty,
            Err(failure) => PresburgerSetDecisionV1::Incomplete(failure),
        }
    }

    fn find_witness_with_budget(
        &self,
        budget: &mut PresburgerBudgetV1,
    ) -> Result<Option<Vec<i128>>, PresburgerFailureV1> {
        if self.domain.is_empty() {
            return Ok(None);
        }
        let mut point = self.domain.lower.clone();
        self.search(0, &mut point, budget)
    }

    fn search(
        &self,
        dimension: usize,
        point: &mut [i128],
        budget: &mut PresburgerBudgetV1,
    ) -> Result<Option<Vec<i128>>, PresburgerFailureV1> {
        budget.charge(1)?;
        if !self.partial_constraints_possible(point, dimension)? {
            return Ok(None);
        }
        if dimension == self.domain.rank() {
            return Ok(Some(point.to_vec()));
        }
        let mut coordinate = self.domain.lower[dimension];
        while coordinate < self.domain.upper_exclusive[dimension] {
            point[dimension] = coordinate;
            if let Some(witness) = self.search(dimension + 1, point, budget)? {
                return Ok(Some(witness));
            }
            coordinate = coordinate
                .checked_add(1)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        }
        Ok(None)
    }

    fn partial_constraints_possible(
        &self,
        point: &[i128],
        assigned: usize,
    ) -> Result<bool, PresburgerFailureV1> {
        for constraint in &self.constraints {
            let possible = match constraint {
                PresburgerConstraintV1::LessEqualZero(expression) => {
                    expression.interval(point, assigned, &self.domain)?.0 <= 0
                }
                PresburgerConstraintV1::EqualZero(expression) => {
                    let (minimum, maximum) = expression.interval(point, assigned, &self.domain)?;
                    minimum <= 0 && maximum >= 0
                }
                PresburgerConstraintV1::CongruentZero {
                    expression,
                    modulus,
                } if assigned == self.domain.rank() => {
                    expression.evaluate(point)?.rem_euclid(*modulus) == 0
                }
                PresburgerConstraintV1::CongruentZero { .. } => true,
            };
            if !possible {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerWitnessV1 {
    point: Vec<i128>,
}

impl PresburgerWitnessV1 {
    pub fn point(&self) -> &[i128] {
        &self.point
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerSetDecisionV1 {
    Empty,
    Witness(PresburgerWitnessV1),
    Incomplete(PresburgerFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerMapExprV1 {
    Affine(PresburgerAffineExprV1),
    Remainder {
        dividend: PresburgerAffineExprV1,
        modulus: i128,
    },
}

impl PresburgerMapExprV1 {
    pub fn evaluate(&self, point: &[i128]) -> Result<i128, PresburgerFailureV1> {
        match self {
            Self::Affine(expression) => expression.evaluate(point),
            Self::Remainder { dividend, modulus } if *modulus > 0 => {
                Ok(dividend.evaluate(point)?.rem_euclid(*modulus))
            }
            Self::Remainder { .. } => Err(PresburgerFailureV1::InvalidModel {
                detail: "remainder map modulus must be positive",
            }),
        }
    }

    fn rank(&self) -> usize {
        match self {
            Self::Affine(expression)
            | Self::Remainder {
                dividend: expression,
                ..
            } => expression.coefficients.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerMapV1 {
    domain: PresburgerSetV1,
    outputs: Vec<PresburgerMapExprV1>,
}

/// Exact non-wrapping integer semantics required by checked Kernel IR indices.
///
/// This is separate from the solver's signed `i128` arithmetic: exceeding a
/// target integer range is a kernel counterexample, while exceeding `i128` is
/// an incomplete analysis result.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PresburgerMachineIntSemanticsV1 {
    bits: u8,
    signed: bool,
}

impl PresburgerMachineIntSemanticsV1 {
    pub fn new(bits: u8, signed: bool) -> Result<Self, PresburgerFailureV1> {
        if bits == 0 || bits > 127 {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "machine-integer width must be between 1 and 127 bits",
            });
        }
        Ok(Self { bits, signed })
    }

    pub const fn unsigned_64() -> Self {
        Self {
            bits: 64,
            signed: false,
        }
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn is_signed(self) -> bool {
        self.signed
    }

    fn bounds(self) -> (i128, i128) {
        if self.signed {
            let magnitude = 1_i128 << (self.bits - 1);
            (-magnitude, magnitude - 1)
        } else {
            (0, (1_i128 << self.bits) - 1)
        }
    }
}

impl PresburgerMapV1 {
    pub fn new(
        domain: PresburgerSetV1,
        outputs: Vec<PresburgerMapExprV1>,
    ) -> Result<Self, PresburgerFailureV1> {
        if outputs.len() > MAX_PRESBURGER_OUTPUTS_V1
            || outputs
                .iter()
                .any(|output| output.rank() != domain.domain.rank())
        {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "map result count or expression rank is invalid",
            });
        }
        Ok(Self { domain, outputs })
    }

    pub fn domain(&self) -> &PresburgerSetV1 {
        &self.domain
    }

    pub fn outputs(&self) -> &[PresburgerMapExprV1] {
        &self.outputs
    }

    pub fn evaluate(&self, point: &[i128]) -> Result<Vec<i128>, PresburgerFailureV1> {
        self.outputs
            .iter()
            .map(|output| output.evaluate(point))
            .collect()
    }

    /// Constructs a checked linear-combination certificate for one affine
    /// output over a nonempty box with affine `<= 0` constraints.
    ///
    /// Equality, congruence, and remainder rows are outside this V2 theorem.
    /// The generator is intentionally incomplete: it uses either one exact
    /// constraint row or the canonical box facets for each side. Failure to
    /// find those multipliers returns `None`, never an unproved clean result.
    pub fn checked_constrained_affine_bounds_certificate_v2(
        &self,
        output: usize,
        extent: u64,
    ) -> Result<Option<ConstrainedAffineBoundsCertificateV2>, PresburgerFailureV1> {
        if extent == 0 || self.domain.constraints().is_empty() {
            return Ok(None);
        }
        let Some(PresburgerMapExprV1::Affine(expression)) = self.outputs.get(output) else {
            return Ok(None);
        };
        let domain = self.domain.domain();
        if domain.rank() == 0
            || domain
                .lower()
                .iter()
                .zip(domain.upper_exclusive())
                .any(|(lower, upper)| lower >= upper)
        {
            return Ok(None);
        }
        let domain_witness = match self.domain.find_witness() {
            PresburgerSetDecisionV1::Witness(witness) => witness.point().to_vec(),
            PresburgerSetDecisionV1::Empty => return Ok(None),
            PresburgerSetDecisionV1::Incomplete(failure) => return Err(failure),
        };
        let constraints = self
            .domain
            .constraints()
            .iter()
            .map(|constraint| match constraint {
                PresburgerConstraintV1::LessEqualZero(row) => Ok(AffineInequalityV2::new(
                    row.constant_term(),
                    row.coefficients().to_vec(),
                )),
                PresburgerConstraintV1::EqualZero(_)
                | PresburgerConstraintV1::CongruentZero { .. } => {
                    Err(PresburgerFailureV1::Unsupported {
                        detail: "constrained affine certificate supports only <= rows",
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let query = ConstrainedAffineBoundsQueryV2::new(
            domain.lower().to_vec(),
            domain.upper_exclusive().to_vec(),
            constraints,
            expression.constant_term(),
            expression.coefficients().to_vec(),
            extent,
        );
        let lower_constant = expression
            .constant_term()
            .checked_neg()
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        let lower_coefficients = expression
            .coefficients()
            .iter()
            .map(|coefficient| coefficient.checked_neg())
            .collect::<Option<Vec<_>>>()
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        let upper_constant = expression
            .constant_term()
            .checked_sub(i128::from(extent) - 1)
            .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;

        let lower_multipliers = exact_constraint_multipliers_v2(
            query.constraints(),
            lower_constant,
            &lower_coefficients,
            domain.rank(),
        )
        .or_else(|| box_facet_multipliers_v2(expression, true, query.constraints().len()));
        let upper_multipliers = exact_constraint_multipliers_v2(
            query.constraints(),
            upper_constant,
            expression.coefficients(),
            domain.rank(),
        )
        .or_else(|| box_facet_multipliers_v2(expression, false, query.constraints().len()));
        let Some((lower_multipliers, upper_multipliers)) = lower_multipliers.zip(upper_multipliers)
        else {
            return Ok(None);
        };
        let certificate = ConstrainedAffineBoundsCertificateV2::new(
            query,
            domain_witness,
            lower_multipliers,
            upper_multipliers,
        );
        match check_constrained_affine_bounds_certificate_v2(&certificate) {
            Ok(_) => Ok(Some(certificate)),
            Err(
                ConstrainedAffineBoundsCertificateErrorV2::LowerCoefficientMismatch { .. }
                | ConstrainedAffineBoundsCertificateErrorV2::UpperCoefficientMismatch { .. }
                | ConstrainedAffineBoundsCertificateErrorV2::LowerConstantNotDominated
                | ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated,
            ) => Ok(None),
            Err(ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow) => {
                Err(PresburgerFailureV1::ArithmeticOverflow)
            }
            Err(_) => Err(PresburgerFailureV1::InvalidModel {
                detail: "generated constrained affine certificate failed structural checking",
            }),
        }
    }

    /// Constructs and checks the endpoint theorem certificate for one affine
    /// output over an unconstrained rectangular domain.
    ///
    /// Constrained domains, remainder expressions, empty dimensions, and
    /// unsafe extrema return `None`; arithmetic overflow fails closed.
    pub fn checked_affine_box_bounds_certificate_v1(
        &self,
        output: usize,
        extent: u64,
    ) -> Result<Option<AffineBoundsCertificateV1>, PresburgerFailureV1> {
        if extent == 0 || !self.domain.constraints().is_empty() {
            return Ok(None);
        }
        let Some(PresburgerMapExprV1::Affine(expression)) = self.outputs.get(output) else {
            return Ok(None);
        };
        let domain = self.domain.domain();
        if domain
            .lower()
            .iter()
            .zip(domain.upper_exclusive())
            .any(|(lower, upper)| lower >= upper)
        {
            return Ok(None);
        }

        let mut minimum_coordinates = Vec::with_capacity(domain.rank());
        let mut maximum_coordinates = Vec::with_capacity(domain.rank());
        for dimension in 0..domain.rank() {
            let last = domain.upper_exclusive()[dimension]
                .checked_sub(1)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
            if expression.coefficients()[dimension] >= 0 {
                minimum_coordinates.push(domain.lower()[dimension]);
                maximum_coordinates.push(last);
            } else {
                minimum_coordinates.push(last);
                maximum_coordinates.push(domain.lower()[dimension]);
            }
        }
        let minimum = expression.evaluate(&minimum_coordinates)?;
        let maximum = expression.evaluate(&maximum_coordinates)?;
        let certificate = AffineBoundsCertificateV1::new(
            AffineBoundsQueryV1::new(
                domain.lower().to_vec(),
                domain.upper_exclusive().to_vec(),
                expression.constant_term(),
                expression.coefficients().to_vec(),
                extent,
            ),
            minimum_coordinates,
            maximum_coordinates,
            minimum,
            maximum,
        );
        match check_affine_bounds_certificate_v1(&certificate) {
            Ok(_) => Ok(Some(certificate)),
            Err(AffineBoundsCertificateErrorV1::BoundNotEstablished) => Ok(None),
            Err(AffineBoundsCertificateErrorV1::ArithmeticOverflow) => {
                Err(PresburgerFailureV1::ArithmeticOverflow)
            }
            Err(_) => Err(PresburgerFailureV1::InvalidModel {
                detail: "generated affine endpoint certificate failed structural checking",
            }),
        }
    }

    /// Proves that every result is representable without target integer wrap,
    /// or returns the first concrete invocation and overflowing result.
    pub fn find_machine_overflow(
        &self,
        semantics: PresburgerMachineIntSemanticsV1,
    ) -> PresburgerMachineRangeDecisionV1 {
        let (minimum, maximum) = semantics.bounds();
        let mut budget = PresburgerBudgetV1::default();
        let mut counterexample = None;
        let result = self.domain.visit_points_with_budget(&mut budget, |point| {
            let range = self.evaluate(point)?;
            if let Some((output, value)) = range
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| *value < minimum || *value > maximum)
            {
                counterexample = Some((point.to_vec(), range, output, value));
                Ok(true)
            } else {
                Ok(false)
            }
        });
        match (result, counterexample) {
            (Ok(()), Some((domain, range, output, value))) => {
                PresburgerMachineRangeDecisionV1::Counterexample {
                    domain,
                    range,
                    output,
                    value,
                    minimum,
                    maximum,
                    semantics,
                }
            }
            (Ok(()), None) => PresburgerMachineRangeDecisionV1::Proved,
            (Err(failure), _) => PresburgerMachineRangeDecisionV1::Incomplete(failure),
        }
    }

    pub fn find_out_of_bounds(&self, extents: &[u64]) -> PresburgerRangeDecisionV1 {
        if extents.len() != self.outputs.len() {
            return PresburgerRangeDecisionV1::Incomplete(PresburgerFailureV1::InvalidModel {
                detail: "range extent rank differs from map result rank",
            });
        }
        let mut budget = PresburgerBudgetV1::default();
        let mut counterexample = None;
        let result =
            self.domain.visit_points_with_budget(&mut budget, |point| {
                let range = self.evaluate(point)?;
                if range.iter().zip(extents).any(|(coordinate, extent)| {
                    *coordinate < 0 || *coordinate >= i128::from(*extent)
                }) {
                    counterexample = Some((point.to_vec(), range));
                    Ok(true)
                } else {
                    Ok(false)
                }
            });
        match (result, counterexample) {
            (Ok(()), Some((domain, range))) => {
                PresburgerRangeDecisionV1::Counterexample { domain, range }
            }
            (Ok(()), None) => PresburgerRangeDecisionV1::Proved,
            (Err(failure), _) => PresburgerRangeDecisionV1::Incomplete(failure),
        }
    }

    pub fn find_collision(&self) -> PresburgerCollisionDecisionV1 {
        let mut budget = PresburgerBudgetV1::default();
        let mut owners = HashMap::<Vec<i128>, Vec<i128>>::new();
        let mut collision = None;
        let result = self.domain.visit_points_with_budget(&mut budget, |point| {
            let range = self.evaluate(point)?;
            if let Some(first) = owners.get(&range)
                && first.as_slice() != point
            {
                collision = Some((first.clone(), point.to_vec(), range));
                return Ok(true);
            }
            owners.insert(range, point.to_vec());
            Ok(false)
        });
        match (result, collision) {
            (Ok(()), Some((first, second, range))) => {
                PresburgerCollisionDecisionV1::Counterexample {
                    first,
                    second,
                    range,
                }
            }
            (Ok(()), None) => PresburgerCollisionDecisionV1::Proved,
            (Err(failure), _) => PresburgerCollisionDecisionV1::Incomplete(failure),
        }
    }

    /// Finds two distinct domain points, one from each map, with the same
    /// result. This is the relation query used by race analysis. When
    /// `require_distinct_points` is true, two effects from the same invocation
    /// are not classified as a cross-invocation collision.
    pub fn find_cross_collision(
        &self,
        other: &Self,
        require_distinct_points: bool,
    ) -> PresburgerCollisionDecisionV1 {
        if self.outputs.len() != other.outputs.len() {
            return PresburgerCollisionDecisionV1::Incomplete(PresburgerFailureV1::InvalidModel {
                detail: "collision maps have different result ranks",
            });
        }
        let mut first_budget = PresburgerBudgetV1::default();
        let mut owners = HashMap::<Vec<i128>, (Vec<i128>, Option<Vec<i128>>)>::new();
        if let Err(failure) = self
            .domain
            .visit_points_with_budget(&mut first_budget, |point| {
                let range = self.evaluate(point)?;
                match owners.get_mut(&range) {
                    None => {
                        owners.insert(range, (point.to_vec(), None));
                    }
                    Some((first, second)) if first.as_slice() != point && second.is_none() => {
                        *second = Some(point.to_vec());
                    }
                    Some(_) => {}
                }
                Ok(false)
            })
        {
            return PresburgerCollisionDecisionV1::Incomplete(failure);
        }
        let mut second_budget = PresburgerBudgetV1::default();
        let mut collision = None;
        let result = other
            .domain
            .visit_points_with_budget(&mut second_budget, |point| {
                let range = other.evaluate(point)?;
                let first = owners.get(&range).and_then(|(first, alternate)| {
                    if !require_distinct_points || first.as_slice() != point {
                        Some(first)
                    } else {
                        alternate.as_ref()
                    }
                });
                if let Some(first) = first {
                    collision = Some((first.clone(), point.to_vec(), range));
                    Ok(true)
                } else {
                    Ok(false)
                }
            });
        match (result, collision) {
            (Ok(()), Some((first, second, range))) => {
                PresburgerCollisionDecisionV1::Counterexample {
                    first,
                    second,
                    range,
                }
            }
            (Ok(()), None) => PresburgerCollisionDecisionV1::Proved,
            (Err(failure), _) => PresburgerCollisionDecisionV1::Incomplete(failure),
        }
    }

    /// Proves that two coordinate maps agree pointwise over one identical
    /// domain, or returns the first point at which their layouts differ.
    pub fn find_mismatch(&self, other: &Self) -> PresburgerEquivalenceDecisionV1 {
        if self.domain != other.domain || self.outputs.len() != other.outputs.len() {
            return PresburgerEquivalenceDecisionV1::Incomplete(
                PresburgerFailureV1::InvalidModel {
                    detail: "pointwise map comparison requires identical domains and result ranks",
                },
            );
        }
        let mut budget = PresburgerBudgetV1::default();
        let mut mismatch = None;
        let result = self.domain.visit_points_with_budget(&mut budget, |point| {
            let first = self.evaluate(point)?;
            let second = other.evaluate(point)?;
            if first != second {
                mismatch = Some((point.to_vec(), first, second));
                Ok(true)
            } else {
                Ok(false)
            }
        });
        match (result, mismatch) {
            (Ok(()), Some((domain, first, second))) => {
                PresburgerEquivalenceDecisionV1::Counterexample {
                    domain,
                    first,
                    second,
                }
            }
            (Ok(()), None) => PresburgerEquivalenceDecisionV1::Proved,
            (Err(failure), _) => PresburgerEquivalenceDecisionV1::Incomplete(failure),
        }
    }

    pub fn find_uncovered(&self, extents: &[u64]) -> PresburgerCoverageDecisionV1 {
        if extents.len() != self.outputs.len() {
            return PresburgerCoverageDecisionV1::Incomplete(PresburgerFailureV1::InvalidModel {
                detail: "coverage extent rank differs from map result rank",
            });
        }
        let mut budget = PresburgerBudgetV1::default();
        let mut image = HashSet::new();
        if let Err(failure) =
            self.domain.visit_points_with_budget(&mut budget, |point| {
                let range = self.evaluate(point)?;
                if range.iter().zip(extents).all(|(coordinate, extent)| {
                    *coordinate >= 0 && *coordinate < i128::from(*extent)
                }) {
                    image.insert(range);
                }
                Ok(false)
            })
        {
            return PresburgerCoverageDecisionV1::Incomplete(failure);
        }
        match PresburgerFiniteImageV1::new(extents.len(), image) {
            Ok(image) => image.find_uncovered(extents),
            Err(failure) => PresburgerCoverageDecisionV1::Incomplete(failure),
        }
    }
}

impl PresburgerSetV1 {
    fn visit_points_with_budget(
        &self,
        budget: &mut PresburgerBudgetV1,
        mut visitor: impl FnMut(&[i128]) -> Result<bool, PresburgerFailureV1>,
    ) -> Result<(), PresburgerFailureV1> {
        if self.domain.is_empty() {
            return Ok(());
        }
        let mut point = self.domain.lower.clone();
        self.visit(0, &mut point, budget, &mut visitor).map(|_| ())
    }

    fn visit(
        &self,
        dimension: usize,
        point: &mut [i128],
        budget: &mut PresburgerBudgetV1,
        visitor: &mut impl FnMut(&[i128]) -> Result<bool, PresburgerFailureV1>,
    ) -> Result<bool, PresburgerFailureV1> {
        budget.charge(1)?;
        if !self.partial_constraints_possible(point, dimension)? {
            return Ok(false);
        }
        if dimension == self.domain.rank() {
            return visitor(point);
        }
        let mut coordinate = self.domain.lower[dimension];
        while coordinate < self.domain.upper_exclusive[dimension] {
            point[dimension] = coordinate;
            if self.visit(dimension + 1, point, budget, visitor)? {
                return Ok(true);
            }
            coordinate = coordinate
                .checked_add(1)
                .ok_or(PresburgerFailureV1::ArithmeticOverflow)?;
        }
        Ok(false)
    }

    fn find_matching_with_budget(
        &self,
        budget: &mut PresburgerBudgetV1,
        mut predicate: impl FnMut(&[i128]) -> bool,
    ) -> Result<Option<Vec<i128>>, PresburgerFailureV1> {
        let mut found = None;
        self.visit_points_with_budget(budget, |point| {
            if predicate(point) {
                found = Some(point.to_vec());
                Ok(true)
            } else {
                Ok(false)
            }
        })?;
        Ok(found)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerMachineRangeDecisionV1 {
    Proved,
    Counterexample {
        domain: Vec<i128>,
        range: Vec<i128>,
        output: usize,
        value: i128,
        minimum: i128,
        maximum: i128,
        semantics: PresburgerMachineIntSemanticsV1,
    },
    Incomplete(PresburgerFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerRangeDecisionV1 {
    Proved,
    Counterexample { domain: Vec<i128>, range: Vec<i128> },
    Incomplete(PresburgerFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerCollisionDecisionV1 {
    Proved,
    Counterexample {
        first: Vec<i128>,
        second: Vec<i128>,
        range: Vec<i128>,
    },
    Incomplete(PresburgerFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerCoverageDecisionV1 {
    Proved,
    Hole { point: Vec<i128> },
    Incomplete(PresburgerFailureV1),
}

/// Finite image produced by path-sensitive tracing. The required range is
/// still a Presburger box, so coverage is decided by the same bounded query
/// engine even when guards or loops made the domain map non-affine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresburgerFiniteImageV1 {
    rank: usize,
    points: HashSet<Vec<i128>>,
}

impl PresburgerFiniteImageV1 {
    pub fn new(
        rank: usize,
        points: impl IntoIterator<Item = Vec<i128>>,
    ) -> Result<Self, PresburgerFailureV1> {
        if rank > MAX_PRESBURGER_OUTPUTS_V1 {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "finite image rank exceeds the Presburger output limit",
            });
        }
        let points = points.into_iter().collect::<HashSet<_>>();
        if points.iter().any(|point| point.len() != rank) {
            return Err(PresburgerFailureV1::InvalidModel {
                detail: "finite image point rank is inconsistent",
            });
        }
        Ok(Self { rank, points })
    }

    pub fn find_uncovered(&self, extents: &[u64]) -> PresburgerCoverageDecisionV1 {
        if extents.len() != self.rank {
            return PresburgerCoverageDecisionV1::Incomplete(PresburgerFailureV1::InvalidModel {
                detail: "coverage extent rank differs from finite image rank",
            });
        }
        let range = match PresburgerBoxV1::zero_based(extents) {
            Ok(range) => PresburgerSetV1::box_only(range),
            Err(failure) => return PresburgerCoverageDecisionV1::Incomplete(failure),
        };
        let mut budget = PresburgerBudgetV1::default();
        match range.find_matching_with_budget(&mut budget, |point| !self.points.contains(point)) {
            Ok(Some(point)) => PresburgerCoverageDecisionV1::Hole { point },
            Ok(None) => PresburgerCoverageDecisionV1::Proved,
            Err(failure) => PresburgerCoverageDecisionV1::Incomplete(failure),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresburgerEquivalenceDecisionV1 {
    Proved,
    Counterexample {
        domain: Vec<i128>,
        first: Vec<i128>,
        second: Vec<i128>,
    },
    Incomplete(PresburgerFailureV1),
}

#[derive(Clone, Debug, Default)]
struct PresburgerBudgetV1 {
    work: usize,
}

impl PresburgerBudgetV1 {
    fn charge(&mut self, amount: usize) -> Result<(), PresburgerFailureV1> {
        let actual = self.work.saturating_add(amount);
        if actual > MAX_PRESBURGER_WORK_UNITS_V1 {
            return Err(PresburgerFailureV1::ResourceLimit {
                limit: MAX_PRESBURGER_WORK_UNITS_V1,
                actual,
            });
        }
        self.work = actual;
        Ok(())
    }
}

/// Per-function adapter from sparse PLIRON index facts to Presburger maps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPresburgerAnalysisV1 {
    launch_extents: Vec<u64>,
}

impl PlironPresburgerAnalysisV1 {
    pub fn from_sparse(sparse: &SparseIndexAnalysisV1) -> Self {
        Self::for_launch_extents(sparse.launch_extents().to_vec())
    }

    pub fn for_launch_extents(launch_extents: Vec<u64>) -> Self {
        Self { launch_extents }
    }

    pub fn launch_extents(&self) -> &[u64] {
        &self.launch_extents
    }

    pub fn map_for_facts(
        &self,
        facts: &[SparseIndexFactV1],
    ) -> Result<PresburgerMapV1, PresburgerFailureV1> {
        self.map_for_facts_over_extents(facts, &self.launch_extents)
    }

    pub fn map_for_facts_over_extents(
        &self,
        facts: &[SparseIndexFactV1],
        launch_extents: &[u64],
    ) -> Result<PresburgerMapV1, PresburgerFailureV1> {
        if launch_extents.contains(&0) {
            return Err(PresburgerFailureV1::Unsupported {
                detail: "a dynamic launch extent has no finite compiler bound",
            });
        }
        let domain = PresburgerSetV1::box_only(PresburgerBoxV1::zero_based(launch_extents)?);
        let outputs = facts
            .iter()
            .map(|fact| self.map_expr_for_fact_over_extents(fact, launch_extents))
            .collect::<Result<Vec<_>, _>>()?;
        PresburgerMapV1::new(domain, outputs)
    }

    /// Builds a map over the exact launch box and caller-retained path rows.
    pub fn map_for_facts_with_constraints(
        &self,
        facts: &[SparseIndexFactV1],
        constraints: Vec<PresburgerConstraintV1>,
    ) -> Result<PresburgerMapV1, PresburgerFailureV1> {
        if self.launch_extents.contains(&0) {
            return Err(PresburgerFailureV1::Unsupported {
                detail: "a dynamic launch extent has no finite compiler bound",
            });
        }
        let domain = PresburgerSetV1::new(
            PresburgerBoxV1::zero_based(&self.launch_extents)?,
            constraints,
        )?;
        let outputs = facts
            .iter()
            .map(|fact| self.map_expr_for_fact(fact))
            .collect::<Result<Vec<_>, _>>()?;
        PresburgerMapV1::new(domain, outputs)
    }

    pub fn map_expr_for_fact(
        &self,
        fact: &SparseIndexFactV1,
    ) -> Result<PresburgerMapExprV1, PresburgerFailureV1> {
        self.map_expr_for_fact_over_extents(fact, &self.launch_extents)
    }

    fn map_expr_for_fact_over_extents(
        &self,
        fact: &SparseIndexFactV1,
        launch_extents: &[u64],
    ) -> Result<PresburgerMapExprV1, PresburgerFailureV1> {
        let affine = |constant: u64, coefficients: &[u64]| {
            if coefficients
                .iter()
                .skip(launch_extents.len())
                .any(|coefficient| *coefficient != 0)
            {
                return Err(PresburgerFailureV1::Unsupported {
                    detail: "an affine index depends on an undeclared invocation dimension",
                });
            }
            PresburgerAffineExprV1::new(
                i128::from(constant),
                coefficients
                    .iter()
                    .take(launch_extents.len())
                    .map(|coefficient| i128::from(*coefficient))
                    .collect(),
            )
        };
        match fact {
            SparseIndexFactV1::Affine(expression) => Ok(PresburgerMapExprV1::Affine(affine(
                expression.constant_term(),
                expression.coefficients(),
            )?)),
            SparseIndexFactV1::Remainder { dividend, modulus } if *modulus != 0 => {
                Ok(PresburgerMapExprV1::Remainder {
                    dividend: affine(dividend.constant_term(), dividend.coefficients())?,
                    modulus: i128::from(*modulus),
                })
            }
            SparseIndexFactV1::Remainder { .. } => Err(PresburgerFailureV1::InvalidModel {
                detail: "sparse remainder has a zero modulus",
            }),
            SparseIndexFactV1::MachineOverflow(_) => {
                Err(PresburgerFailureV1::MachineIntegerOverflow {
                    bits: 64,
                    signed: false,
                })
            }
            SparseIndexFactV1::Unknown
            | SparseIndexFactV1::CheckedTiled2D(_)
            | SparseIndexFactV1::CheckedRowStriped2D(_) => Err(PresburgerFailureV1::Unsupported {
                detail: "index fact is outside the affine/remainder Presburger fragment",
            }),
        }
    }
}

fn exact_constraint_multipliers_v2(
    constraints: &[AffineInequalityV2],
    target_constant: i128,
    target_coefficients: &[i128],
    rank: usize,
) -> Option<Vec<u64>> {
    constraints
        .iter()
        .position(|row| {
            row.coefficients() == target_coefficients && target_constant <= row.constant()
        })
        .map(|selected| {
            let mut multipliers = vec![0; constraints.len() + 2 * rank];
            multipliers[selected] = 1;
            multipliers
        })
}

fn box_facet_multipliers_v2(
    expression: &PresburgerAffineExprV1,
    lower_target: bool,
    constraint_count: usize,
) -> Option<Vec<u64>> {
    let rank = expression.coefficients().len();
    let mut multipliers = vec![0; constraint_count + 2 * rank];
    for (dimension, coefficient) in expression.coefficients().iter().copied().enumerate() {
        let magnitude = coefficient.unsigned_abs();
        if magnitude > u128::from(MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2) {
            return None;
        }
        let magnitude = u64::try_from(magnitude).ok()?;
        let use_lower_facet = if lower_target {
            coefficient >= 0
        } else {
            coefficient < 0
        };
        let row = constraint_count + 2 * dimension + usize::from(!use_lower_facet);
        multipliers[row] = magnitude;
    }
    Some(multipliers)
}
