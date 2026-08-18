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

/// The Pliron namespace owned by this crate.
pub const DIALECT_NAME: &str = "tile";

/// The largest distributed tile rank admitted by this shell.
pub const MAX_TILE_RANK: u32 = 8;

/// The largest logical lane count admitted by this target-neutral shell.
pub const MAX_DISTRIBUTED_LANES: u32 = 1_024;

/// The largest per-lane element count admitted by this shell.
pub const MAX_ELEMENTS_PER_LANE: u32 = 1_024;

/// The largest total element count admitted by one tile.
pub const MAX_TILE_ELEMENTS: u32 = 1_048_576;

/// The operation attribute key carrying tile distribution metadata.
pub const DISTRIBUTION_ATTR_KEY: &str = "tile_distribution";

const REGISTRATION_MARKER_KEY: &str = "fe2o3_dialect_tile_registration_v1";

/// The semantic owner reported by tile interfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOwner {
    /// Distributed tile and layout semantics are owned by `tile`.
    Tile,
}

/// A bounded construction or verification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileError {
    /// A tile rank was outside the admitted range.
    RankOutOfBounds(u32),
    /// A logical lane count was outside the admitted range.
    LanesOutOfBounds(u32),
    /// A per-lane element count was outside the admitted range.
    ElementsPerLaneOutOfBounds(u32),
    /// A total tile element count was outside the admitted range.
    TotalElementsOutOfBounds(u32),
    /// Distribution multiplication overflowed its bounded scalar type.
    DistributionOverflow,
    /// A tile operation did not carry its required distribution.
    MissingDistribution,
    /// The tile type and distribution metadata disagreed.
    DistributionMismatch {
        result_rank: u32,
        result_elements: u32,
        distribution_rank: u32,
        distribution_elements: u32,
    },
}

impl fmt::Display for TileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankOutOfBounds(rank) => {
                write!(formatter, "tile rank {rank} is outside 1..={MAX_TILE_RANK}")
            }
            Self::LanesOutOfBounds(lanes) => write!(
                formatter,
                "lane count {lanes} is outside 1..={MAX_DISTRIBUTED_LANES}"
            ),
            Self::ElementsPerLaneOutOfBounds(elements) => write!(
                formatter,
                "elements per lane {elements} is outside 1..={MAX_ELEMENTS_PER_LANE}"
            ),
            Self::TotalElementsOutOfBounds(elements) => write!(
                formatter,
                "total tile elements {elements} is outside 1..={MAX_TILE_ELEMENTS}"
            ),
            Self::DistributionOverflow => {
                formatter.write_str("tile distribution element count overflowed")
            }
            Self::MissingDistribution => {
                formatter.write_str("tile materialization is missing its distribution attribute")
            }
            Self::DistributionMismatch {
                result_rank,
                result_elements,
                distribution_rank,
                distribution_elements,
            } => write!(
                formatter,
                "tile type ({result_rank}, {result_elements}) does not match distribution ({distribution_rank}, {distribution_elements})"
            ),
        }
    }
}

impl Error for TileError {}

fn check_rank(rank: u32) -> Result<(), TileError> {
    if (1..=MAX_TILE_RANK).contains(&rank) {
        Ok(())
    } else {
        Err(TileError::RankOutOfBounds(rank))
    }
}

fn check_lanes(lanes: u32) -> Result<(), TileError> {
    if (1..=MAX_DISTRIBUTED_LANES).contains(&lanes) {
        Ok(())
    } else {
        Err(TileError::LanesOutOfBounds(lanes))
    }
}

fn check_elements_per_lane(elements: u32) -> Result<(), TileError> {
    if (1..=MAX_ELEMENTS_PER_LANE).contains(&elements) {
        Ok(())
    } else {
        Err(TileError::ElementsPerLaneOutOfBounds(elements))
    }
}

fn check_total_elements(elements: u32) -> Result<(), TileError> {
    if (1..=MAX_TILE_ELEMENTS).contains(&elements) {
        Ok(())
    } else {
        Err(TileError::TotalElementsOutOfBounds(elements))
    }
}

fn distribution_elements(lanes: u32, elements_per_lane: u32) -> Result<u32, TileError> {
    check_lanes(lanes)?;
    check_elements_per_lane(elements_per_lane)?;
    let total = lanes
        .checked_mul(elements_per_lane)
        .ok_or(TileError::DistributionOverflow)?;
    check_total_elements(total)?;
    Ok(total)
}

/// A bounded target-neutral distributed tile type.
#[pliron_type(
    name = "tile.distributed",
    format = "`<` $rank `,` $total_elements `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DistributedTileType {
    rank: u32,
    total_elements: u32,
}

impl DistributedTileType {
    /// Creates a uniqued distributed tile type after enforcing all bounds.
    pub fn new(
        context: &Context,
        rank: u32,
        total_elements: u32,
    ) -> Result<TypedHandle<Self>, TileError> {
        check_rank(rank)?;
        check_total_elements(total_elements)?;
        Ok(Self::instantiate(
            Self {
                rank,
                total_elements,
            },
            context,
        ))
    }

    /// Returns the logical tile rank.
    pub const fn rank(&self) -> u32 {
        self.rank
    }

    /// Returns the total number of logical elements.
    pub const fn total_elements(&self) -> u32 {
        self.total_elements
    }
}

impl Verify for DistributedTileType {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if let Err(error) =
            check_rank(self.rank).and_then(|()| check_total_elements(self.total_elements))
        {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Typed, bounded tile-distribution metadata.
#[pliron_attr(
    name = "tile.distribution",
    format = "`<` $rank `,` $lanes `,` $elements_per_lane `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DistributionAttr {
    rank: u32,
    lanes: u32,
    elements_per_lane: u32,
}

impl DistributionAttr {
    /// Creates target-neutral distribution metadata after enforcing all bounds.
    pub fn new(rank: u32, lanes: u32, elements_per_lane: u32) -> Result<Self, TileError> {
        check_rank(rank)?;
        distribution_elements(lanes, elements_per_lane)?;
        Ok(Self {
            rank,
            lanes,
            elements_per_lane,
        })
    }

    /// Returns the logical tile rank.
    pub const fn rank(&self) -> u32 {
        self.rank
    }

    /// Returns the logical lane count.
    pub const fn lanes(&self) -> u32 {
        self.lanes
    }

    /// Returns the number of elements assigned to each lane.
    pub const fn elements_per_lane(&self) -> u32 {
        self.elements_per_lane
    }

    /// Returns the checked, bounded total element count.
    pub fn total_elements(&self) -> Result<u32, TileError> {
        distribution_elements(self.lanes, self.elements_per_lane)
    }
}

impl Verify for DistributionAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        let checked = check_rank(self.rank)
            .and_then(|()| distribution_elements(self.lanes, self.elements_per_lane));
        if let Err(error) = checked {
            return verify_err_noloc!(error);
        }
        Ok(())
    }
}

/// Interface for distributed tile operations owned by this dialect.
#[op_interface]
pub trait DistributedTileOp {
    /// Returns the dialect that owns this operation's semantics.
    fn semantic_owner(&self) -> SemanticOwner;

    /// Tile materialization remains independent of a physical target.
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
                "tile interface on foreign operation"
            );
        }
        Ok(())
    }
}

/// A minimal materialized distributed tile.
#[pliron_op(
    name = "tile.materialize",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<1>, NRegionsInterface<0>],
    results = (tile: DistributedTileType),
)]
pub struct MaterializeOp;

#[op_interface_impl]
impl DistributedTileOp for MaterializeOp {
    fn semantic_owner(&self) -> SemanticOwner {
        SemanticOwner::Tile
    }
}

impl MaterializeOp {
    /// Creates one bounded distributed tile materialization.
    pub fn new(
        context: &mut Context,
        rank: u32,
        lanes: u32,
        elements_per_lane: u32,
    ) -> Result<Self, TileError> {
        let distribution = DistributionAttr::new(rank, lanes, elements_per_lane)?;
        let total_elements = distribution.total_elements()?;
        let tile_type = DistributedTileType::new(context, rank, total_elements)?;
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![tile_type.into()],
            vec![],
            vec![],
            0,
        );
        let materialization = Self { op: operation };
        materialization.set_distribution(context, distribution);
        Ok(materialization)
    }

    /// Returns a clone of the typed distribution, if present.
    pub fn distribution(&self, context: &Context) -> Option<DistributionAttr> {
        self.get_operation()
            .deref(context)
            .attributes
            .0
            .get(&distribution_attr_key())
            .and_then(|attribute| attribute.downcast_ref::<DistributionAttr>())
            .cloned()
    }

    /// Replaces distribution metadata. Verification rechecks consistency.
    pub fn set_distribution(&self, context: &Context, distribution: DistributionAttr) {
        self.get_operation()
            .deref_mut(context)
            .attributes
            .0
            .insert(distribution_attr_key(), Box::new(distribution));
    }
}

impl Verify for MaterializeOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed_shape(self, context)?;
        let distribution = self
            .distribution(context)
            .ok_or_else(|| verify_error!(self.loc(context), TileError::MissingDistribution))?;
        let result_type = self.get_operation().deref(context).get_type(0);
        let result_type = result_type.deref(context);
        let tile_type = result_type
            .downcast_ref::<DistributedTileType>()
            .ok_or_else(|| verify_error!(self.loc(context), "tile result has a foreign type"))?;
        let distribution_elements = distribution
            .total_elements()
            .map_err(|error| verify_error!(self.loc(context), error))?;
        if tile_type.rank() != distribution.rank()
            || tile_type.total_elements() != distribution_elements
        {
            return verify_err!(
                self.loc(context),
                TileError::DistributionMismatch {
                    result_rank: tile_type.rank(),
                    result_elements: tile_type.total_elements(),
                    distribution_rank: distribution.rank(),
                    distribution_elements,
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
        key == &distribution_attr_key()
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

fn distribution_attr_key() -> Identifier {
    DISTRIBUTION_ATTR_KEY
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
                formatter.write_str("tile registration requested for wrong dialect")
            }
            Self::MarkerCollision => formatter.write_str("tile registration marker collision"),
            Self::CorruptMarker => formatter.write_str("tile registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// Explicitly registers every tile entity, rejecting marker collisions.
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
    DistributedTileType::register(context);
    <DistributionAttr as Attribute>::register::<DistributionAttr>(context);
    MaterializeOp::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context.aux_data_map.insert(marker_key, marker);
    Ok(RegistrationOutcome::Registered)
}
