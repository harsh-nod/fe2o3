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

mod general_gemm;
mod registration;

pub use general_gemm::{
    GeneralGemmPhasePlanAttr, GeneralGemmPlanOp, GeneralGemmScheduleAttr,
    GeneralGemmTransferPlanAttr,
};
pub use registration::dialect_registration;

/// The Pliron namespace owned by this crate.
pub const DIALECT_NAME: &str = "schedule";

/// The largest rank admitted by a schedule plan.
pub const MAX_SCHEDULE_RANK: u32 = 8;

/// The largest target-independent tile extent admitted by the shell.
pub const MAX_TILE_EXTENT: u32 = 1_048_576;

/// The largest pipeline-stage count admitted by the shell.
pub const MAX_PIPELINE_STAGES: u32 = 8;

/// The operation attribute key carrying schedule parameters.
pub const PARAMETERS_ATTR_KEY: &str = "schedule_parameters";

const REGISTRATION_MARKER_KEY: &str = "fe2o3_dialect_schedule_registration_v1";

/// The semantic owner reported by scheduling interfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOwner {
    /// Non-executable planning semantics are owned by `schedule`.
    Schedule,
}

/// A bounded construction or verification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    /// A schedule rank was outside the admitted range.
    RankOutOfBounds(u32),
    /// A tile extent was outside the admitted range.
    TileExtentOutOfBounds(u32),
    /// A pipeline-stage count was outside the admitted range.
    PipelineStagesOutOfBounds(u32),
    /// A schedule operation did not carry its required parameters.
    MissingParameters,
    /// The plan type and parameter attribute disagreed.
    PlanMismatch {
        result_rank: u32,
        result_stages: u32,
        parameter_rank: u32,
        parameter_stages: u32,
    },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankOutOfBounds(rank) => {
                write!(
                    formatter,
                    "schedule rank {rank} is outside 1..={MAX_SCHEDULE_RANK}"
                )
            }
            Self::TileExtentOutOfBounds(extent) => write!(
                formatter,
                "tile extent {extent} is outside 1..={MAX_TILE_EXTENT}"
            ),
            Self::PipelineStagesOutOfBounds(stages) => write!(
                formatter,
                "pipeline stage count {stages} is outside 1..={MAX_PIPELINE_STAGES}"
            ),
            Self::MissingParameters => {
                formatter.write_str("schedule plan is missing its parameter attribute")
            }
            Self::PlanMismatch {
                result_rank,
                result_stages,
                parameter_rank,
                parameter_stages,
            } => write!(
                formatter,
                "plan type ({result_rank}, {result_stages}) does not match parameters ({parameter_rank}, {parameter_stages})"
            ),
        }
    }
}

impl Error for ScheduleError {}

fn check_rank(rank: u32) -> Result<(), ScheduleError> {
    if (1..=MAX_SCHEDULE_RANK).contains(&rank) {
        Ok(())
    } else {
        Err(ScheduleError::RankOutOfBounds(rank))
    }
}

fn check_tile_extent(tile_extent: u32) -> Result<(), ScheduleError> {
    if (1..=MAX_TILE_EXTENT).contains(&tile_extent) {
        Ok(())
    } else {
        Err(ScheduleError::TileExtentOutOfBounds(tile_extent))
    }
}

fn check_stages(stages: u32) -> Result<(), ScheduleError> {
    if (1..=MAX_PIPELINE_STAGES).contains(&stages) {
        Ok(())
    } else {
        Err(ScheduleError::PipelineStagesOutOfBounds(stages))
    }
}

/// A target-neutral, non-executable schedule-plan type.
#[pliron_type(
    name = "schedule.plan_type",
    format = "`<` $rank `,` $pipeline_stages `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlanType {
    rank: u32,
    pipeline_stages: u32,
}

impl PlanType {
    /// Creates a uniqued plan type after enforcing all bounds.
    pub fn new(
        context: &Context,
        rank: u32,
        pipeline_stages: u32,
    ) -> Result<TypedHandle<Self>, ScheduleError> {
        check_rank(rank)?;
        check_stages(pipeline_stages)?;
        Ok(Self::instantiate(
            Self {
                rank,
                pipeline_stages,
            },
            context,
        ))
    }

    /// Returns the iteration rank covered by this plan.
    pub const fn rank(&self) -> u32 {
        self.rank
    }

    /// Returns the number of logical pipeline stages.
    pub const fn pipeline_stages(&self) -> u32 {
        self.pipeline_stages
    }
}

impl Verify for PlanType {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if let Err(error) = check_rank(self.rank).and_then(|()| check_stages(self.pipeline_stages))
        {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Typed, bounded scheduling parameters.
#[pliron_attr(
    name = "schedule.parameters",
    format = "`<` $rank `,` $tile_extent `,` $pipeline_stages `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ParametersAttr {
    rank: u32,
    tile_extent: u32,
    pipeline_stages: u32,
}

impl ParametersAttr {
    /// Creates bounded scheduling parameters.
    pub fn new(rank: u32, tile_extent: u32, pipeline_stages: u32) -> Result<Self, ScheduleError> {
        check_rank(rank)?;
        check_tile_extent(tile_extent)?;
        check_stages(pipeline_stages)?;
        Ok(Self {
            rank,
            tile_extent,
            pipeline_stages,
        })
    }

    /// Returns the scheduled iteration rank.
    pub const fn rank(&self) -> u32 {
        self.rank
    }

    /// Returns the target-independent tile extent.
    pub const fn tile_extent(&self) -> u32 {
        self.tile_extent
    }

    /// Returns the logical pipeline-stage count.
    pub const fn pipeline_stages(&self) -> u32 {
        self.pipeline_stages
    }
}

impl Verify for ParametersAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        let checked = check_rank(self.rank)
            .and_then(|()| check_tile_extent(self.tile_extent))
            .and_then(|()| check_stages(self.pipeline_stages));
        if let Err(error) = checked {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Interface for non-executable plan operations owned by this dialect.
#[op_interface]
pub trait NonExecutableScheduleOp {
    /// Returns the dialect that owns this operation's semantics.
    fn semantic_owner(&self) -> SemanticOwner;

    /// Schedule plans describe choices and are never executable.
    fn is_executable(&self) -> bool {
        false
    }

    /// Schedule plans remain independent of a physical target.
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
                "schedule interface on foreign operation"
            );
        }
        Ok(())
    }
}

/// A minimal non-executable schedule plan.
#[pliron_op(
    name = "schedule.plan",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<1>, NRegionsInterface<0>],
    results = (plan: PlanType),
)]
pub struct PlanOp;

#[op_interface_impl]
impl NonExecutableScheduleOp for PlanOp {
    fn semantic_owner(&self) -> SemanticOwner {
        SemanticOwner::Schedule
    }
}

impl PlanOp {
    /// Creates one bounded non-executable plan.
    pub fn new(
        context: &mut Context,
        rank: u32,
        tile_extent: u32,
        pipeline_stages: u32,
    ) -> Result<Self, ScheduleError> {
        let plan_type = PlanType::new(context, rank, pipeline_stages)?;
        let parameters = ParametersAttr::new(rank, tile_extent, pipeline_stages)?;
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![plan_type.into()],
            vec![],
            vec![],
            0,
        );
        let plan = Self { op: operation };
        plan.set_parameters(context, parameters);
        Ok(plan)
    }

    /// Returns a clone of the typed parameters, if present.
    pub fn parameters(&self, context: &Context) -> Option<ParametersAttr> {
        self.get_operation()
            .deref(context)
            .attributes
            .0
            .get(&parameters_attr_key())
            .and_then(|attribute| attribute.downcast_ref::<ParametersAttr>())
            .cloned()
    }

    /// Replaces plan parameters. Verification rechecks consistency.
    pub fn set_parameters(&self, context: &Context, parameters: ParametersAttr) {
        self.get_operation()
            .deref_mut(context)
            .attributes
            .0
            .insert(parameters_attr_key(), Box::new(parameters));
    }
}

impl Verify for PlanOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed_shape(self, context)?;
        let parameters = self
            .parameters(context)
            .ok_or_else(|| verify_error!(self.loc(context), ScheduleError::MissingParameters))?;
        let result_type = self.get_operation().deref(context).get_type(0);
        let result_type = result_type.deref(context);
        let plan_type = result_type.downcast_ref::<PlanType>().ok_or_else(|| {
            verify_error!(self.loc(context), "schedule result has a foreign type")
        })?;
        if plan_type.rank() != parameters.rank()
            || plan_type.pipeline_stages() != parameters.pipeline_stages()
        {
            return verify_err!(
                self.loc(context),
                ScheduleError::PlanMismatch {
                    result_rank: plan_type.rank(),
                    result_stages: plan_type.pipeline_stages(),
                    parameter_rank: parameters.rank(),
                    parameter_stages: parameters.pipeline_stages(),
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
        key == &parameters_attr_key()
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

fn parameters_attr_key() -> Identifier {
    PARAMETERS_ATTR_KEY
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
                formatter.write_str("schedule registration requested for wrong dialect")
            }
            Self::MarkerCollision => formatter.write_str("schedule registration marker collision"),
            Self::CorruptMarker => formatter.write_str("schedule registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// Explicitly registers every schedule entity, rejecting marker collisions.
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
    PlanType::register(context);
    <ParametersAttr as Attribute>::register::<ParametersAttr>(context);
    <GeneralGemmScheduleAttr as Attribute>::register::<GeneralGemmScheduleAttr>(context);
    <GeneralGemmPhasePlanAttr as Attribute>::register::<GeneralGemmPhasePlanAttr>(context);
    <GeneralGemmTransferPlanAttr as Attribute>::register::<GeneralGemmTransferPlanAttr>(context);
    PlanOp::register(context);
    GeneralGemmPlanOp::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context.aux_data_map.insert(marker_key, marker);
    Ok(RegistrationOutcome::Registered)
}
