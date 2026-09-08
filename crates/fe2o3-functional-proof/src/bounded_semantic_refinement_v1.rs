//! Canonical, finite functional models for aggregate MIR-to-ranked-KIR proofs.
//!
//! These values describe inert proof obligations; they are not proof evidence
//! and are never accepted directly by the production verifier. In particular,
//! compilation identities are receipt-binding data and never become logical
//! hypotheses in generated proofs. Production constructs these values only
//! after independently normalizing authenticated reference MIR and exact
//! ranked KIR.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

pub const HARD_MAX_FUNCTIONAL_PROOF_OUTPUTS_V1: usize = 64;
pub const HARD_MAX_FUNCTIONAL_PROOF_ARITY_V1: usize = 32;
pub const HARD_MAX_FUNCTIONAL_PROOF_STEPS_V1: u16 = 256;
pub const HARD_MAX_FUNCTIONAL_EXPRESSION_NODES_V1: usize = 4_096;
pub const HARD_MAX_FUNCTIONAL_EXPRESSION_DEPTH_V1: usize = 128;

const CONTEXT_DOMAIN_V1: &[u8] = b"FE2O3/FUNCTIONAL-SEMANTIC-CONTEXT/V1\0";
const PLAN_DOMAIN_V1: &[u8] = b"FE2O3/BOUNDED-SEMANTIC-REFINEMENT-PLAN/V1\0";

/// Exact compiler state to which a semantic proof is confined.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionalSemanticContextV1 {
    safe_reference_source: DigestV1,
    safe_reference_mir: DigestV1,
    final_ranked_kir: DigestV1,
    analysis_epoch: DigestV1,
    launch_contract: DigestV1,
    target_contract: DigestV1,
    numerical_contract: DigestV1,
}

impl FunctionalSemanticContextV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        safe_reference_source: DigestV1,
        safe_reference_mir: DigestV1,
        final_ranked_kir: DigestV1,
        analysis_epoch: DigestV1,
        launch_contract: DigestV1,
        target_contract: DigestV1,
        numerical_contract: DigestV1,
    ) -> Result<Self, BoundedSemanticRefinementErrorV1> {
        let context = Self {
            safe_reference_source,
            safe_reference_mir,
            final_ranked_kir,
            analysis_epoch,
            launch_contract,
            target_contract,
            numerical_contract,
        };
        if context.identities().into_iter().any(DigestV1::is_zero) {
            return Err(BoundedSemanticRefinementErrorV1::MissingContextIdentity);
        }
        Ok(context)
    }

    pub const fn safe_reference_source(self) -> DigestV1 {
        self.safe_reference_source
    }
    pub const fn safe_reference_mir(self) -> DigestV1 {
        self.safe_reference_mir
    }
    pub const fn final_ranked_kir(self) -> DigestV1 {
        self.final_ranked_kir
    }
    pub const fn analysis_epoch(self) -> DigestV1 {
        self.analysis_epoch
    }
    pub const fn launch_contract(self) -> DigestV1 {
        self.launch_contract
    }
    pub const fn target_contract(self) -> DigestV1 {
        self.target_contract
    }
    pub const fn numerical_contract(self) -> DigestV1 {
        self.numerical_contract
    }

    pub fn canonical_sha256(self) -> DigestV1 {
        hash_with_domain(CONTEXT_DOMAIN_V1, |digest| {
            for identity in self.identities() {
                put_digest(digest, identity);
            }
        })
    }

    fn identities(self) -> [DigestV1; 7] {
        [
            self.safe_reference_source,
            self.safe_reference_mir,
            self.final_ranked_kir,
            self.analysis_epoch,
            self.launch_contract,
            self.target_contract,
            self.numerical_contract,
        ]
    }
}

/// Variables admitted by the finite expression language.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionalScalarVariableV1 {
    Input(u16),
    State(u16),
    Step,
}

/// A total mathematical-integer expression used in generated Verus.
///
/// Keeping this language deliberately small makes every emitted operation have
/// an exact, solver-independent meaning. Floating implementations use an
/// explicit scaled-integer error theorem rather than silently reusing this as
/// IEEE value semantics.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionalScalarExpressionV1 {
    Variable(FunctionalScalarVariableV1),
    Constant(i64),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
}

impl FunctionalScalarExpressionV1 {
    pub fn input(slot: u16) -> Self {
        Self::Variable(FunctionalScalarVariableV1::Input(slot))
    }
    pub fn state(slot: u16) -> Self {
        Self::Variable(FunctionalScalarVariableV1::State(slot))
    }
    pub const fn step() -> Self {
        Self::Variable(FunctionalScalarVariableV1::Step)
    }
    pub const fn constant(value: i64) -> Self {
        Self::Constant(value)
    }
    pub fn sum(lhs: Self, rhs: Self) -> Self {
        Self::Add(Box::new(lhs), Box::new(rhs))
    }
    pub fn difference(lhs: Self, rhs: Self) -> Self {
        Self::Subtract(Box::new(lhs), Box::new(rhs))
    }
    pub fn product(lhs: Self, rhs: Self) -> Self {
        Self::Multiply(Box::new(lhs), Box::new(rhs))
    }

    pub fn evaluate(&self, inputs: &[i128], state: &[i128], step: i128) -> Option<i128> {
        match self {
            Self::Variable(FunctionalScalarVariableV1::Input(slot)) => {
                inputs.get(usize::from(*slot)).copied()
            }
            Self::Variable(FunctionalScalarVariableV1::State(slot)) => {
                state.get(usize::from(*slot)).copied()
            }
            Self::Variable(FunctionalScalarVariableV1::Step) => Some(step),
            Self::Constant(value) => Some(i128::from(*value)),
            Self::Add(lhs, rhs) => lhs
                .evaluate(inputs, state, step)?
                .checked_add(rhs.evaluate(inputs, state, step)?),
            Self::Subtract(lhs, rhs) => lhs
                .evaluate(inputs, state, step)?
                .checked_sub(rhs.evaluate(inputs, state, step)?),
            Self::Multiply(lhs, rhs) => lhs
                .evaluate(inputs, state, step)?
                .checked_mul(rhs.evaluate(inputs, state, step)?),
        }
    }

    fn validate(&self, scope: ExpressionScopeV1) -> Result<(), BoundedSemanticRefinementErrorV1> {
        let mut work = vec![(self, 1_usize)];
        let mut nodes = 0_usize;
        while let Some((expression, depth)) = work.pop() {
            nodes = nodes
                .checked_add(1)
                .ok_or(BoundedSemanticRefinementErrorV1::ExpressionResourceLimit)?;
            if nodes > HARD_MAX_FUNCTIONAL_EXPRESSION_NODES_V1
                || depth > HARD_MAX_FUNCTIONAL_EXPRESSION_DEPTH_V1
            {
                return Err(BoundedSemanticRefinementErrorV1::ExpressionResourceLimit);
            }
            match expression {
                Self::Variable(FunctionalScalarVariableV1::Input(slot))
                    if usize::from(*slot) >= scope.inputs =>
                {
                    return Err(BoundedSemanticRefinementErrorV1::OutOfScopeVariable);
                }
                Self::Variable(FunctionalScalarVariableV1::State(slot))
                    if !scope.state_allowed || usize::from(*slot) >= scope.states =>
                {
                    return Err(BoundedSemanticRefinementErrorV1::OutOfScopeVariable);
                }
                Self::Variable(FunctionalScalarVariableV1::Step) if !scope.step_allowed => {
                    return Err(BoundedSemanticRefinementErrorV1::OutOfScopeVariable);
                }
                Self::Add(lhs, rhs) | Self::Subtract(lhs, rhs) | Self::Multiply(lhs, rhs) => {
                    let child_depth = depth
                        .checked_add(1)
                        .ok_or(BoundedSemanticRefinementErrorV1::ExpressionResourceLimit)?;
                    work.push((rhs, child_depth));
                    work.push((lhs, child_depth));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ExpressionScopeV1 {
    inputs: usize,
    states: usize,
    state_allowed: bool,
    step_allowed: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionalFoldOperatorV1 {
    Add,
    Multiply,
    Minimum,
    Maximum,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionalScheduleProofV1 {
    Pointwise {
        actual: FunctionalScalarExpressionV1,
        reference: FunctionalScalarExpressionV1,
    },
    Permutation {
        extent: u16,
        forward: FunctionalScalarExpressionV1,
        inverse: FunctionalScalarExpressionV1,
        actual: FunctionalScalarExpressionV1,
        reference: FunctionalScalarExpressionV1,
    },
    Fold {
        extent: u16,
        identity: i64,
        operator: FunctionalFoldOperatorV1,
        gpu_order: Box<[u16]>,
        reference_order: Box<[u16]>,
    },
    BoundedRecurrence {
        maximum_steps: u16,
        state_arity: u16,
        initial_actual: Box<[FunctionalScalarExpressionV1]>,
        initial_reference: Box<[FunctionalScalarExpressionV1]>,
        transition_actual: Box<[FunctionalScalarExpressionV1]>,
        transition_reference: Box<[FunctionalScalarExpressionV1]>,
        outputs_actual: Box<[FunctionalScalarExpressionV1]>,
        outputs_reference: Box<[FunctionalScalarExpressionV1]>,
    },
    TensorComponents {
        actual: Box<[FunctionalScalarExpressionV1]>,
        reference: Box<[FunctionalScalarExpressionV1]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionalNumericalProofV1 {
    ExactInteger,
    /// Sound finite-error statement over explicitly scaled mathematical values.
    /// Runtime/compiler range checks represented by `input_bounds` are theorem
    /// preconditions and must be discharged by the launch contract.
    FiniteError {
        scale: u64,
        absolute_error_units: u64,
        relative_numerator: u64,
        relative_denominator: u64,
        input_bounds: Box<[(i64, i64)]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionalOutputProofV1 {
    output_contract: DigestV1,
    relation: DigestV1,
    collective: Option<DigestV1>,
    loop_contract: Option<DigestV1>,
    input_arity: u16,
    schedule: FunctionalScheduleProofV1,
    numerical: FunctionalNumericalProofV1,
}

impl FunctionalOutputProofV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_contract: DigestV1,
        relation: DigestV1,
        collective: Option<DigestV1>,
        loop_contract: Option<DigestV1>,
        input_arity: u16,
        schedule: FunctionalScheduleProofV1,
        numerical: FunctionalNumericalProofV1,
    ) -> Result<Self, BoundedSemanticRefinementErrorV1> {
        let output = Self {
            output_contract,
            relation,
            collective,
            loop_contract,
            input_arity,
            schedule,
            numerical,
        };
        output.validate()?;
        Ok(output)
    }

    pub const fn output_contract(&self) -> DigestV1 {
        self.output_contract
    }
    pub const fn relation(&self) -> DigestV1 {
        self.relation
    }
    pub const fn collective(&self) -> Option<DigestV1> {
        self.collective
    }
    pub const fn loop_contract(&self) -> Option<DigestV1> {
        self.loop_contract
    }
    pub const fn input_arity(&self) -> u16 {
        self.input_arity
    }
    pub const fn schedule(&self) -> &FunctionalScheduleProofV1 {
        &self.schedule
    }
    pub const fn numerical(&self) -> &FunctionalNumericalProofV1 {
        &self.numerical
    }

    fn validate(&self) -> Result<(), BoundedSemanticRefinementErrorV1> {
        if self.output_contract.is_zero()
            || self.relation.is_zero()
            || self.collective.is_some_and(DigestV1::is_zero)
            || self.loop_contract.is_some_and(DigestV1::is_zero)
            || usize::from(self.input_arity) > HARD_MAX_FUNCTIONAL_PROOF_ARITY_V1
        {
            return Err(BoundedSemanticRefinementErrorV1::InvalidOutputBinding);
        }
        let scalar_scope = ExpressionScopeV1 {
            inputs: usize::from(self.input_arity),
            states: 0,
            state_allowed: false,
            step_allowed: false,
        };
        match &self.schedule {
            FunctionalScheduleProofV1::Pointwise { actual, reference } => {
                require_no_collective(self)?;
                actual.validate(scalar_scope)?;
                reference.validate(scalar_scope)?;
            }
            FunctionalScheduleProofV1::Permutation {
                extent,
                forward,
                inverse,
                actual,
                reference,
            } => {
                require_collective_without_loop(self)?;
                require_extent(*extent)?;
                let mapping_scope = ExpressionScopeV1 {
                    inputs: 0,
                    states: 0,
                    state_allowed: false,
                    step_allowed: true,
                };
                forward.validate(mapping_scope)?;
                inverse.validate(mapping_scope)?;
                actual.validate(scalar_scope)?;
                reference.validate(scalar_scope)?;
                validate_static_permutation(*extent, forward, inverse)?;
            }
            FunctionalScheduleProofV1::Fold {
                extent,
                gpu_order,
                reference_order,
                ..
            } => {
                require_collective_without_loop(self)?;
                require_extent(*extent)?;
                validate_order(*extent, gpu_order)?;
                validate_order(*extent, reference_order)?;
            }
            FunctionalScheduleProofV1::BoundedRecurrence {
                maximum_steps,
                state_arity,
                initial_actual,
                initial_reference,
                transition_actual,
                transition_reference,
                outputs_actual,
                outputs_reference,
            } => {
                if self.collective.is_none() || self.loop_contract.is_none() {
                    return Err(BoundedSemanticRefinementErrorV1::InvalidScheduleBinding);
                }
                require_extent(*maximum_steps)?;
                let state_arity = usize::from(*state_arity);
                if state_arity == 0
                    || state_arity > HARD_MAX_FUNCTIONAL_PROOF_ARITY_V1
                    || initial_actual.len() != state_arity
                    || initial_reference.len() != state_arity
                    || transition_actual.len() != state_arity
                    || transition_reference.len() != state_arity
                    || outputs_actual.is_empty()
                    || outputs_actual.len() != outputs_reference.len()
                    || outputs_actual.len() > HARD_MAX_FUNCTIONAL_PROOF_ARITY_V1
                {
                    return Err(BoundedSemanticRefinementErrorV1::InvalidRecurrence);
                }
                let initial_scope = scalar_scope;
                let recurrence_scope = ExpressionScopeV1 {
                    inputs: usize::from(self.input_arity),
                    states: state_arity,
                    state_allowed: true,
                    step_allowed: true,
                };
                for expression in initial_actual.iter().chain(initial_reference.iter()) {
                    expression.validate(initial_scope)?;
                }
                for expression in transition_actual
                    .iter()
                    .chain(transition_reference.iter())
                    .chain(outputs_actual.iter())
                    .chain(outputs_reference.iter())
                {
                    expression.validate(recurrence_scope)?;
                }
            }
            FunctionalScheduleProofV1::TensorComponents { actual, reference } => {
                require_no_collective(self)?;
                if actual.is_empty()
                    || actual.len() != reference.len()
                    || actual.len() > HARD_MAX_FUNCTIONAL_PROOF_ARITY_V1
                {
                    return Err(BoundedSemanticRefinementErrorV1::InvalidTensorComposition);
                }
                for expression in actual.iter().chain(reference.iter()) {
                    expression.validate(scalar_scope)?;
                }
            }
        }
        match &self.numerical {
            FunctionalNumericalProofV1::ExactInteger => {}
            FunctionalNumericalProofV1::FiniteError {
                scale,
                absolute_error_units,
                relative_numerator,
                relative_denominator,
                input_bounds,
            } => {
                if *scale == 0
                    || *relative_denominator == 0
                    || (*absolute_error_units == 0 && *relative_numerator == 0)
                    || input_bounds.len() != usize::from(self.input_arity)
                    || input_bounds
                        .iter()
                        .any(|(minimum, maximum)| minimum > maximum)
                {
                    return Err(BoundedSemanticRefinementErrorV1::InvalidFiniteError);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedSemanticRefinementPlanV1 {
    context: FunctionalSemanticContextV1,
    outputs: Box<[FunctionalOutputProofV1]>,
}

impl BoundedSemanticRefinementPlanV1 {
    /// Builds an inert obligation without asserting output completeness.
    ///
    /// Production verification must use [`Self::from_complete_output_product`]
    /// with the output roster independently derived from retained IR.
    pub fn new(
        context: FunctionalSemanticContextV1,
        outputs: Vec<FunctionalOutputProofV1>,
    ) -> Result<Self, BoundedSemanticRefinementErrorV1> {
        if outputs.is_empty() || outputs.len() > HARD_MAX_FUNCTIONAL_PROOF_OUTPUTS_V1 {
            return Err(BoundedSemanticRefinementErrorV1::OutputResourceLimit);
        }
        let output_ids = outputs
            .iter()
            .map(FunctionalOutputProofV1::output_contract)
            .collect::<BTreeSet<_>>();
        let relation_ids = outputs
            .iter()
            .map(FunctionalOutputProofV1::relation)
            .collect::<BTreeSet<_>>();
        if output_ids.len() != outputs.len() || relation_ids.len() != outputs.len() {
            return Err(BoundedSemanticRefinementErrorV1::DuplicateOutputBinding);
        }
        for output in &outputs {
            output.validate()?;
        }
        Ok(Self {
            context,
            outputs: outputs.into_boxed_slice(),
        })
    }

    /// Builds an obligation only when it covers the independently derived
    /// output roster exactly once and in canonical order.
    pub fn from_complete_output_product(
        context: FunctionalSemanticContextV1,
        required_output_contracts: &[DigestV1],
        outputs: Vec<FunctionalOutputProofV1>,
    ) -> Result<Self, BoundedSemanticRefinementErrorV1> {
        if required_output_contracts.is_empty()
            || required_output_contracts.len() > HARD_MAX_FUNCTIONAL_PROOF_OUTPUTS_V1
            || required_output_contracts
                .iter()
                .any(|identity| identity.is_zero())
            || required_output_contracts
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != required_output_contracts.len()
        {
            return Err(BoundedSemanticRefinementErrorV1::InvalidOutputProduct);
        }
        let observed = outputs
            .iter()
            .map(FunctionalOutputProofV1::output_contract)
            .collect::<Vec<_>>();
        if observed.as_slice() != required_output_contracts {
            return Err(BoundedSemanticRefinementErrorV1::IncompleteOutputProduct);
        }
        Self::new(context, outputs)
    }

    pub const fn context(&self) -> FunctionalSemanticContextV1 {
        self.context
    }
    pub fn outputs(&self) -> &[FunctionalOutputProofV1] {
        &self.outputs
    }

    pub fn canonical_sha256(&self) -> DigestV1 {
        hash_with_domain(PLAN_DOMAIN_V1, |digest| {
            put_digest(digest, self.context.canonical_sha256());
            put_u64(digest, self.outputs.len() as u64);
            for output in &self.outputs {
                put_digest(digest, output.output_contract);
                put_digest(digest, output.relation);
                put_optional_digest(digest, output.collective);
                put_optional_digest(digest, output.loop_contract);
                digest.update(output.input_arity.to_le_bytes());
                put_schedule(digest, &output.schedule);
                put_numerical(digest, &output.numerical);
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundedSemanticRefinementErrorV1 {
    MissingContextIdentity,
    OutputResourceLimit,
    DuplicateOutputBinding,
    InvalidOutputBinding,
    InvalidOutputProduct,
    IncompleteOutputProduct,
    InvalidScheduleBinding,
    InvalidPermutation,
    InvalidFoldOrder,
    InvalidRecurrence,
    InvalidTensorComposition,
    InvalidFiniteError,
    OutOfScopeVariable,
    ExpressionResourceLimit,
}

impl fmt::Display for BoundedSemanticRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bounded functional-semantic plan rejected: {self:?}"
        )
    }
}

impl Error for BoundedSemanticRefinementErrorV1 {}

fn require_extent(extent: u16) -> Result<(), BoundedSemanticRefinementErrorV1> {
    if extent == 0 || extent > HARD_MAX_FUNCTIONAL_PROOF_STEPS_V1 {
        Err(BoundedSemanticRefinementErrorV1::InvalidScheduleBinding)
    } else {
        Ok(())
    }
}

fn require_no_collective(
    output: &FunctionalOutputProofV1,
) -> Result<(), BoundedSemanticRefinementErrorV1> {
    if output.collective.is_some() || output.loop_contract.is_some() {
        Err(BoundedSemanticRefinementErrorV1::InvalidScheduleBinding)
    } else {
        Ok(())
    }
}

fn require_collective_without_loop(
    output: &FunctionalOutputProofV1,
) -> Result<(), BoundedSemanticRefinementErrorV1> {
    if output.collective.is_none() || output.loop_contract.is_some() {
        Err(BoundedSemanticRefinementErrorV1::InvalidScheduleBinding)
    } else {
        Ok(())
    }
}

fn validate_static_permutation(
    extent: u16,
    forward: &FunctionalScalarExpressionV1,
    inverse: &FunctionalScalarExpressionV1,
) -> Result<(), BoundedSemanticRefinementErrorV1> {
    let mut images = BTreeSet::new();
    for index in 0..extent {
        let image = forward
            .evaluate(&[], &[], i128::from(index))
            .ok_or(BoundedSemanticRefinementErrorV1::InvalidPermutation)?;
        if !(0..i128::from(extent)).contains(&image) || !images.insert(image) {
            return Err(BoundedSemanticRefinementErrorV1::InvalidPermutation);
        }
        let round_trip = inverse
            .evaluate(&[], &[], image)
            .ok_or(BoundedSemanticRefinementErrorV1::InvalidPermutation)?;
        if round_trip != i128::from(index) {
            return Err(BoundedSemanticRefinementErrorV1::InvalidPermutation);
        }
    }
    Ok(())
}

fn validate_order(extent: u16, order: &[u16]) -> Result<(), BoundedSemanticRefinementErrorV1> {
    if order.len() != usize::from(extent)
        || order.iter().copied().collect::<BTreeSet<_>>().len() != order.len()
        || order.iter().any(|index| *index >= extent)
    {
        Err(BoundedSemanticRefinementErrorV1::InvalidFoldOrder)
    } else {
        Ok(())
    }
}

fn hash_with_domain(domain: &[u8], fill: impl FnOnce(&mut Sha256)) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, domain);
    fill(&mut digest);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn put_schedule(digest: &mut Sha256, schedule: &FunctionalScheduleProofV1) {
    match schedule {
        FunctionalScheduleProofV1::Pointwise { actual, reference } => {
            digest.update([1]);
            put_expression(digest, actual);
            put_expression(digest, reference);
        }
        FunctionalScheduleProofV1::Permutation {
            extent,
            forward,
            inverse,
            actual,
            reference,
        } => {
            digest.update([2]);
            digest.update(extent.to_le_bytes());
            for expression in [forward, inverse, actual, reference] {
                put_expression(digest, expression);
            }
        }
        FunctionalScheduleProofV1::Fold {
            extent,
            identity,
            operator,
            gpu_order,
            reference_order,
        } => {
            digest.update([3, *operator as u8]);
            digest.update(extent.to_le_bytes());
            digest.update(identity.to_le_bytes());
            put_order(digest, gpu_order);
            put_order(digest, reference_order);
        }
        FunctionalScheduleProofV1::BoundedRecurrence {
            maximum_steps,
            state_arity,
            initial_actual,
            initial_reference,
            transition_actual,
            transition_reference,
            outputs_actual,
            outputs_reference,
        } => {
            digest.update([4]);
            digest.update(maximum_steps.to_le_bytes());
            digest.update(state_arity.to_le_bytes());
            for expressions in [
                initial_actual,
                initial_reference,
                transition_actual,
                transition_reference,
                outputs_actual,
                outputs_reference,
            ] {
                put_expressions(digest, expressions);
            }
        }
        FunctionalScheduleProofV1::TensorComponents { actual, reference } => {
            digest.update([5]);
            put_expressions(digest, actual);
            put_expressions(digest, reference);
        }
    }
}

fn put_numerical(digest: &mut Sha256, numerical: &FunctionalNumericalProofV1) {
    match numerical {
        FunctionalNumericalProofV1::ExactInteger => digest.update([1]),
        FunctionalNumericalProofV1::FiniteError {
            scale,
            absolute_error_units,
            relative_numerator,
            relative_denominator,
            input_bounds,
        } => {
            digest.update([2]);
            for value in [
                *scale,
                *absolute_error_units,
                *relative_numerator,
                *relative_denominator,
            ] {
                put_u64(digest, value);
            }
            put_u64(digest, input_bounds.len() as u64);
            for (minimum, maximum) in input_bounds {
                digest.update(minimum.to_le_bytes());
                digest.update(maximum.to_le_bytes());
            }
        }
    }
}

fn put_expressions(digest: &mut Sha256, expressions: &[FunctionalScalarExpressionV1]) {
    put_u64(digest, expressions.len() as u64);
    for expression in expressions {
        put_expression(digest, expression);
    }
}

fn put_expression(digest: &mut Sha256, expression: &FunctionalScalarExpressionV1) {
    match expression {
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::Input(slot)) => {
            digest.update([1]);
            digest.update(slot.to_le_bytes());
        }
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::State(slot)) => {
            digest.update([2]);
            digest.update(slot.to_le_bytes());
        }
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::Step) => {
            digest.update([3]);
        }
        FunctionalScalarExpressionV1::Constant(value) => {
            digest.update([4]);
            digest.update(value.to_le_bytes());
        }
        FunctionalScalarExpressionV1::Add(lhs, rhs) => {
            digest.update([5]);
            put_expression(digest, lhs);
            put_expression(digest, rhs);
        }
        FunctionalScalarExpressionV1::Subtract(lhs, rhs) => {
            digest.update([6]);
            put_expression(digest, lhs);
            put_expression(digest, rhs);
        }
        FunctionalScalarExpressionV1::Multiply(lhs, rhs) => {
            digest.update([7]);
            put_expression(digest, lhs);
            put_expression(digest, rhs);
        }
    }
}

fn put_order(digest: &mut Sha256, order: &[u16]) {
    put_u64(digest, order.len() as u64);
    for index in order {
        digest.update(index.to_le_bytes());
    }
}

fn put_optional_digest(digest: &mut Sha256, identity: Option<DigestV1>) {
    match identity {
        None => digest.update([0]),
        Some(identity) => {
            digest.update([1]);
            put_digest(digest, identity);
        }
    }
}

fn put_digest(digest: &mut Sha256, identity: DigestV1) {
    put_blob(digest, identity.as_bytes());
}
fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    put_u64(digest, bytes.len() as u64);
    digest.update(bytes);
}
fn put_u64(digest: &mut Sha256, value: u64) {
    digest.update(value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    fn context() -> FunctionalSemanticContextV1 {
        FunctionalSemanticContextV1::new(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            digest(6),
            digest(7),
        )
        .unwrap()
    }

    #[test]
    fn kda_style_delta_recurrence_is_canonical_and_binds_every_context_axis() {
        let state = FunctionalScalarExpressionV1::state(0);
        let key = FunctionalScalarExpressionV1::input(0);
        let value = FunctionalScalarExpressionV1::input(1);
        let beta = FunctionalScalarExpressionV1::input(2);
        let correction = FunctionalScalarExpressionV1::product(
            beta.clone(),
            FunctionalScalarExpressionV1::product(
                FunctionalScalarExpressionV1::difference(
                    value.clone(),
                    FunctionalScalarExpressionV1::product(state.clone(), key.clone()),
                ),
                key.clone(),
            ),
        );
        let reference = FunctionalScalarExpressionV1::sum(state.clone(), correction);
        let wy_expansion = FunctionalScalarExpressionV1::difference(
            FunctionalScalarExpressionV1::sum(
                state.clone(),
                FunctionalScalarExpressionV1::product(
                    FunctionalScalarExpressionV1::product(beta.clone(), value),
                    key.clone(),
                ),
            ),
            FunctionalScalarExpressionV1::product(
                FunctionalScalarExpressionV1::product(beta, state),
                FunctionalScalarExpressionV1::product(key.clone(), key),
            ),
        );
        let output = FunctionalOutputProofV1::new(
            digest(8),
            digest(9),
            Some(digest(10)),
            Some(digest(11)),
            3,
            FunctionalScheduleProofV1::BoundedRecurrence {
                maximum_steps: 4,
                state_arity: 1,
                initial_actual: vec![FunctionalScalarExpressionV1::constant(0)].into(),
                initial_reference: vec![FunctionalScalarExpressionV1::constant(0)].into(),
                transition_actual: vec![wy_expansion].into(),
                transition_reference: vec![reference].into(),
                outputs_actual: vec![FunctionalScalarExpressionV1::state(0)].into(),
                outputs_reference: vec![FunctionalScalarExpressionV1::state(0)].into(),
            },
            FunctionalNumericalProofV1::ExactInteger,
        )
        .unwrap();
        let plan = BoundedSemanticRefinementPlanV1::new(context(), vec![output]).unwrap();
        assert_ne!(plan.canonical_sha256(), DigestV1::ZERO);

        let changed_context = FunctionalSemanticContextV1::new(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            digest(99),
            digest(7),
        )
        .unwrap();
        let changed =
            BoundedSemanticRefinementPlanV1::new(changed_context, plan.outputs().to_vec()).unwrap();
        assert_ne!(plan.canonical_sha256(), changed.canonical_sha256());
    }

    #[test]
    fn hostile_permutation_and_recurrence_roles_fail_closed() {
        let duplicate = FunctionalScalarExpressionV1::constant(0);
        let identity = FunctionalScalarExpressionV1::step();
        let permutation = FunctionalOutputProofV1::new(
            digest(8),
            digest(9),
            Some(digest(10)),
            None,
            0,
            FunctionalScheduleProofV1::Permutation {
                extent: 4,
                forward: duplicate,
                inverse: identity.clone(),
                actual: FunctionalScalarExpressionV1::constant(0),
                reference: FunctionalScalarExpressionV1::constant(0),
            },
            FunctionalNumericalProofV1::ExactInteger,
        );
        assert_eq!(
            permutation,
            Err(BoundedSemanticRefinementErrorV1::InvalidPermutation)
        );

        let recurrence = FunctionalOutputProofV1::new(
            digest(8),
            digest(9),
            Some(digest(10)),
            None,
            1,
            FunctionalScheduleProofV1::BoundedRecurrence {
                maximum_steps: 4,
                state_arity: 1,
                initial_actual: vec![FunctionalScalarExpressionV1::constant(0)].into(),
                initial_reference: vec![FunctionalScalarExpressionV1::constant(0)].into(),
                transition_actual: vec![identity.clone()].into(),
                transition_reference: vec![identity.clone()].into(),
                outputs_actual: vec![identity.clone()].into(),
                outputs_reference: vec![identity].into(),
            },
            FunctionalNumericalProofV1::ExactInteger,
        );
        assert_eq!(
            recurrence,
            Err(BoundedSemanticRefinementErrorV1::InvalidScheduleBinding)
        );
    }

    #[test]
    fn finite_error_requires_explicit_nonempty_bound_and_complete_input_ranges() {
        let result = FunctionalOutputProofV1::new(
            digest(8),
            digest(9),
            None,
            None,
            1,
            FunctionalScheduleProofV1::Pointwise {
                actual: FunctionalScalarExpressionV1::input(0),
                reference: FunctionalScalarExpressionV1::input(0),
            },
            FunctionalNumericalProofV1::FiniteError {
                scale: 1_000,
                absolute_error_units: 0,
                relative_numerator: 0,
                relative_denominator: 1,
                input_bounds: vec![(-8, 8)].into(),
            },
        );
        assert_eq!(
            result,
            Err(BoundedSemanticRefinementErrorV1::InvalidFiniteError)
        );
    }

    #[test]
    fn complete_output_product_rejects_omission_order_and_identity_drift() {
        let output = |output_contract, relation| {
            FunctionalOutputProofV1::new(
                output_contract,
                relation,
                None,
                None,
                1,
                FunctionalScheduleProofV1::Pointwise {
                    actual: FunctionalScalarExpressionV1::input(0),
                    reference: FunctionalScalarExpressionV1::input(0),
                },
                FunctionalNumericalProofV1::ExactInteger,
            )
            .unwrap()
        };
        assert_eq!(
            BoundedSemanticRefinementPlanV1::from_complete_output_product(
                context(),
                &[digest(8), digest(10)],
                vec![output(digest(8), digest(9))],
            ),
            Err(BoundedSemanticRefinementErrorV1::IncompleteOutputProduct),
        );
        assert_eq!(
            BoundedSemanticRefinementPlanV1::from_complete_output_product(
                context(),
                &[digest(8), digest(10)],
                vec![output(digest(10), digest(11)), output(digest(8), digest(9))],
            ),
            Err(BoundedSemanticRefinementErrorV1::IncompleteOutputProduct),
        );
        assert_eq!(
            BoundedSemanticRefinementPlanV1::from_complete_output_product(
                context(),
                &[digest(8), digest(8)],
                vec![output(digest(8), digest(9)), output(digest(8), digest(11))],
            ),
            Err(BoundedSemanticRefinementErrorV1::InvalidOutputProduct),
        );
    }
}
