#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::{error::Error, fmt};

use pliron::{
    attribute::Attribute,
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{op_interface, op_interface_impl, pliron_attr, pliron_op, pliron_type},
    dialect::{Dialect, DialectName},
    identifier::Identifier,
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    r#type::{Type, TypedHandle},
    verify_err, verify_err_noloc, verify_error,
};

mod registration;

pub use registration::dialect_registration;

/// The Pliron namespace owned by this crate.
pub const DIALECT_NAME: &str = "autotune";

/// The largest inert candidate set admitted by this shell.
pub const MAX_CANDIDATES: u32 = 256;

/// The largest observation budget admitted for each candidate.
pub const MAX_OBSERVATIONS_PER_CANDIDATE: u32 = 64;

/// The operation attribute key carrying candidate budgets.
pub const BUDGET_ATTR_KEY: &str = "autotune_candidate_budget";

const REGISTRATION_MARKER_KEY: &str = "fe2o3_dialect_autotune_registration_v1";

/// The semantic owner reported by autotune interfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOwner {
    /// Inert candidate metadata is owned by `autotune`.
    Autotune,
}

/// A bounded construction or verification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutotuneError {
    /// A candidate count was outside the admitted range.
    CandidatesOutOfBounds(u32),
    /// An observation count was outside the admitted range.
    ObservationsOutOfBounds(u32),
    /// A candidate-set operation did not carry its required budget.
    MissingBudget,
    /// The candidate-set type and budget attribute disagreed.
    CandidateCountMismatch { result: u32, budget: u32 },
}

impl fmt::Display for AutotuneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CandidatesOutOfBounds(candidates) => write!(
                formatter,
                "candidate count {candidates} is outside 1..={MAX_CANDIDATES}"
            ),
            Self::ObservationsOutOfBounds(observations) => write!(
                formatter,
                "observations per candidate {observations} is outside 1..={MAX_OBSERVATIONS_PER_CANDIDATE}"
            ),
            Self::MissingBudget => {
                formatter.write_str("candidate set is missing its budget attribute")
            }
            Self::CandidateCountMismatch { result, budget } => write!(
                formatter,
                "candidate-set result count {result} does not match budget count {budget}"
            ),
        }
    }
}

impl Error for AutotuneError {}

fn check_candidates(candidates: u32) -> Result<(), AutotuneError> {
    if (1..=MAX_CANDIDATES).contains(&candidates) {
        Ok(())
    } else {
        Err(AutotuneError::CandidatesOutOfBounds(candidates))
    }
}

fn check_observations(observations: u32) -> Result<(), AutotuneError> {
    if (1..=MAX_OBSERVATIONS_PER_CANDIDATE).contains(&observations) {
        Ok(())
    } else {
        Err(AutotuneError::ObservationsOutOfBounds(observations))
    }
}

/// A bounded inert candidate-set type.
#[pliron_type(name = "autotune.candidate_set", format = "`<` $candidates `>`")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CandidateSetType {
    candidates: u32,
}

impl CandidateSetType {
    /// Creates a uniqued candidate-set type after enforcing the count bound.
    pub fn new(context: &Context, candidates: u32) -> Result<TypedHandle<Self>, AutotuneError> {
        check_candidates(candidates)?;
        Ok(Self::instantiate(Self { candidates }, context))
    }

    /// Returns the number of inert candidates.
    pub const fn candidates(&self) -> u32 {
        self.candidates
    }
}

impl Verify for CandidateSetType {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if let Err(error) = check_candidates(self.candidates) {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Typed, bounded candidate observation metadata.
#[pliron_attr(
    name = "autotune.candidate_budget",
    format = "`<` $candidates `,` $observations_per_candidate `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CandidateBudgetAttr {
    candidates: u32,
    observations_per_candidate: u32,
}

impl CandidateBudgetAttr {
    /// Creates inert candidate metadata after enforcing all bounds.
    pub fn new(candidates: u32, observations_per_candidate: u32) -> Result<Self, AutotuneError> {
        check_candidates(candidates)?;
        check_observations(observations_per_candidate)?;
        Ok(Self {
            candidates,
            observations_per_candidate,
        })
    }

    /// Returns the number of inert candidates.
    pub const fn candidates(&self) -> u32 {
        self.candidates
    }

    /// Returns the observation budget for each candidate.
    pub const fn observations_per_candidate(&self) -> u32 {
        self.observations_per_candidate
    }
}

impl Verify for CandidateBudgetAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        let checked = check_candidates(self.candidates)
            .and_then(|()| check_observations(self.observations_per_candidate));
        if let Err(error) = checked {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Interface for inert candidate metadata owned by this dialect.
#[op_interface]
pub trait InertAutotuneOp {
    /// Returns the dialect that owns this operation's semantics.
    fn semantic_owner(&self) -> SemanticOwner;

    /// Candidate metadata has no execution semantics.
    fn is_executable(&self) -> bool {
        false
    }

    /// Candidate metadata remains independent of a physical target.
    fn is_target_neutral(&self) -> bool {
        true
    }

    /// Verifies the fixed ownership namespace.
    fn verify(operation: &dyn Op, context: &Context) -> PlironResult<()>
    where
        Self: Sized,
    {
        if operation.get_opid().dialect.as_ref() != DIALECT_NAME {
            return verify_err!(
                operation.loc(context),
                "autotune interface on foreign operation"
            );
        }
        Ok(())
    }
}

/// A minimal inert candidate-set descriptor.
#[pliron_op(
    name = "autotune.candidates",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<1>, NRegionsInterface<0>],
    results = (candidates: CandidateSetType),
)]
pub struct CandidateSetOp;

#[op_interface_impl]
impl InertAutotuneOp for CandidateSetOp {
    fn semantic_owner(&self) -> SemanticOwner {
        SemanticOwner::Autotune
    }
}

impl CandidateSetOp {
    /// Creates one bounded inert candidate set.
    pub fn new(
        context: &mut Context,
        candidates: u32,
        observations_per_candidate: u32,
    ) -> Result<Self, AutotuneError> {
        let candidate_type = CandidateSetType::new(context, candidates)?;
        let budget = CandidateBudgetAttr::new(candidates, observations_per_candidate)?;
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![candidate_type.into()],
            vec![],
            vec![],
            0,
        );
        let candidate_set = Self { op: operation };
        candidate_set.set_budget(context, budget);
        Ok(candidate_set)
    }

    /// Returns a clone of the typed candidate budget, if present.
    pub fn budget(&self, context: &Context) -> Option<CandidateBudgetAttr> {
        self.get_operation()
            .deref(context)
            .attributes
            .0
            .get(&budget_attr_key())
            .and_then(|attribute| attribute.downcast_ref::<CandidateBudgetAttr>())
            .cloned()
    }

    /// Replaces candidate metadata. Verification rechecks consistency.
    pub fn set_budget(&self, context: &Context, budget: CandidateBudgetAttr) {
        self.get_operation()
            .deref_mut(context)
            .attributes
            .0
            .insert(budget_attr_key(), Box::new(budget));
    }
}

impl Verify for CandidateSetOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed_shape(self, context)?;
        let budget = self
            .budget(context)
            .ok_or_else(|| verify_error!(self.loc(context), AutotuneError::MissingBudget))?;
        let result_type = self.get_operation().deref(context).get_type(0);
        let result_type = result_type.deref(context);
        let candidate_type = result_type
            .downcast_ref::<CandidateSetType>()
            .ok_or_else(|| {
                verify_error!(self.loc(context), "candidate-set result has a foreign type")
            })?;
        if candidate_type.candidates() != budget.candidates() {
            return verify_err!(
                self.loc(context),
                AutotuneError::CandidateCountMismatch {
                    result: candidate_type.candidates(),
                    budget: budget.candidates(),
                }
            );
        }
        Ok(())
    }
}

fn verify_closed_shape(op: &dyn Op, context: &Context) -> PlironResult<()> {
    let operation = op.get_operation();
    let operation = operation.deref(context);
    let attributes_are_closed = operation.attributes.0.iter().all(|(key, attribute)| {
        key == &budget_attr_key()
            || (key == &*ATTR_KEY_DEBUG_INFO && is_debug_info(attribute.as_ref()))
    });
    if operation.get_num_operands() != 0
        || operation.get_num_results() != 1
        || operation.get_num_successors() != 0
        || operation.num_regions() != 0
        || !attributes_are_closed
    {
        return verify_err!(
            op.loc(context),
            "{} has malformed or unbounded structural payload",
            op.get_opid()
        );
    }
    Ok(())
}

fn is_debug_info(attribute: &dyn Attribute) -> bool {
    let id = attribute.get_attr_id();
    id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
}

fn budget_attr_key() -> Identifier {
    BUDGET_ATTR_KEY
        .try_into()
        .expect("constant attribute key is a valid identifier")
}

#[derive(Debug)]
struct RegistrationMarker;

/// Result of explicit registration in one Pliron context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationOutcome {
    /// This call installed the explicit registration marker.
    Registered,
    /// This crate had already registered in the same context.
    AlreadyRegistered,
}

/// A fail-closed explicit registration error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    /// The requested namespace was not [`DIALECT_NAME`].
    WrongDialect,
    /// Another typed value already claimed this crate's marker key.
    MarkerCollision,
    /// The marker map referenced absent auxiliary data.
    CorruptMarker,
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDialect => {
                formatter.write_str("autotune registration requested for wrong dialect")
            }
            Self::MarkerCollision => formatter.write_str("autotune registration marker collision"),
            Self::CorruptMarker => formatter.write_str("autotune registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// Explicitly registers every autotune entity, rejecting marker collisions.
pub fn register_dialect(
    context: &mut Context,
    requested: &DialectName,
) -> Result<RegistrationOutcome, RegistrationError> {
    if requested.as_ref() != DIALECT_NAME {
        return Err(RegistrationError::WrongDialect);
    }

    let marker_key: Identifier = REGISTRATION_MARKER_KEY
        .try_into()
        .expect("constant marker key is a valid identifier");
    if let Some(index) = context.aux_data_map.get(&marker_key).copied() {
        return match context.aux_data.get(index) {
            Some(marker) if marker.is::<RegistrationMarker>() => {
                Ok(RegistrationOutcome::AlreadyRegistered)
            }
            Some(_) => Err(RegistrationError::MarkerCollision),
            None => Err(RegistrationError::CorruptMarker),
        };
    }

    Dialect::register(context, requested);
    CandidateSetType::register(context);
    <CandidateBudgetAttr as Attribute>::register::<CandidateBudgetAttr>(context);
    CandidateSetOp::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context.aux_data_map.insert(marker_key, marker);
    Ok(RegistrationOutcome::Registered)
}
