//! Bounded read-only control-flow facts for Option-wrapped GPU capabilities.
//!
//! These inert facts identify blocks dominated by the exact Some edge of a
//! unique compiler-intrinsic result. They grant no compiler or artifact authority.

use std::{error::Error, fmt};

use crate::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDirectCallV1, SemanticFunctionDeclV1, SemanticLocalIdV1, SemanticOperandV1,
    SemanticPlaceV1, SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticSwitchTargetsV1, SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeShapeV1,
    SemanticUncheckedBinaryOpV1,
};

/// Maximum charged CFG, statement, definition, and dominator work.
pub const MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1: usize = 1_048_576;

/// Terminal failure from bounded Option-capability dominance analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOptionDominanceErrorV1 {
    /// The semantic CFG or a local identity is structurally out of range.
    InvalidControlFlow(&'static str),
    /// An Option producer, discriminator, or boolean switch is not exact.
    InexactCapability(&'static str),
    /// The independent analysis work budget was exhausted.
    WorkLimit {
        /// Charged work at rejection.
        actual: usize,
        /// Fixed maximum charged work.
        limit: usize,
    },
    /// Bounded result storage could not be reserved.
    Storage,
}

impl SemanticOptionDominanceErrorV1 {
    /// Stable diagnostic detail for layer-specific error mapping.
    pub const fn detail(self) -> &'static str {
        match self {
            Self::InvalidControlFlow(detail) | Self::InexactCapability(detail) => detail,
            Self::WorkLimit { .. } => {
                "Option capability dominance analysis exceeded its work limit"
            }
            Self::Storage => "Option capability dominance storage cannot be reserved",
        }
    }
}

impl fmt::Display for SemanticOptionDominanceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidControlFlow(detail) => {
                write!(formatter, "invalid semantic control flow: {detail}")
            }
            Self::InexactCapability(detail) => {
                write!(formatter, "inexact Option capability: {detail}")
            }
            Self::WorkLimit { actual, limit } => write!(
                formatter,
                "Option capability dominance work {actual} exceeds {limit}"
            ),
            Self::Storage => formatter.write_str("Option capability dominance storage failed"),
        }
    }
}

impl Error for SemanticOptionDominanceErrorV1 {}

/// Keeps a live caller refusal distinct from an inert dominance-analysis error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOptionDominanceMeteredErrorV18<E> {
    /// The unchanged analysis rejected the original semantic model.
    Analysis(SemanticOptionDominanceErrorV1),
    /// The exact caller work/storage refusal, without erasing its payload.
    Meter(E),
}

impl<E: fmt::Display> fmt::Display for SemanticOptionDominanceMeteredErrorV18<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(formatter),
            Self::Meter(error) => write!(formatter, "semantic dominance meter: {error}"),
        }
    }
}
impl<E: Error + 'static> Error for SemanticOptionDominanceMeteredErrorV18<E> {}

trait DominanceMeterV18 {
    fn work(&mut self, amount: usize, logical: bool) -> Result<(), SemanticOptionDominanceErrorV1>;
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1>;
}

struct DominanceMeterAdapterV18<'a, M: crate::SemanticAssertionMeterV1> {
    meter: &'a mut M,
    error: Option<M::Error>,
}

impl<M: crate::SemanticAssertionMeterV1> DominanceMeterV18 for DominanceMeterAdapterV18<'_, M> {
    fn work(&mut self, amount: usize, logical: bool) -> Result<(), SemanticOptionDominanceErrorV1> {
        let result = if logical { self.meter.charge_legacy_work(amount) } else { self.meter.charge_work(amount) };
        result.map_err(|error| {
            self.error.get_or_insert(error);
            SemanticOptionDominanceErrorV1::Storage
        })
    }
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        self.meter.reserve_storage(amount).map_err(|error| {
            self.error.get_or_insert(error);
            SemanticOptionDominanceErrorV1::Storage
        })
    }
}

fn with_dominance_meter_v18<T, M: crate::SemanticAssertionMeterV1, F>(
    meter: &mut M,
    action: F,
) -> Result<T, SemanticOptionDominanceMeteredErrorV18<M::Error>>
where F: FnOnce(&mut WorkBudgetV1<'_>) -> Result<T, SemanticOptionDominanceErrorV1> {
    let bytes = std::mem::size_of::<DominanceMeterAdapterV18<'_, M>>()
        .checked_add(std::mem::size_of::<WorkBudgetV1<'_>>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<F>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Result<T, SemanticOptionDominanceErrorV1>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Result<T, SemanticOptionDominanceMeteredErrorV18<M::Error>>>()))
        .ok_or(SemanticOptionDominanceMeteredErrorV18::Analysis(SemanticOptionDominanceErrorV1::Storage))?;
    meter.reserve_storage(bytes).map_err(SemanticOptionDominanceMeteredErrorV18::Meter)?;
    let mut adapter = DominanceMeterAdapterV18 { meter, error: None };
    let result = action(&mut WorkBudgetV1 { used: 0, meter: Some(&mut adapter) });
    match adapter.error {
        Some(error) => Err(SemanticOptionDominanceMeteredErrorV18::Meter(error)),
        None => result.map_err(SemanticOptionDominanceMeteredErrorV18::Analysis),
    }
}

/// One exact compiler-intrinsic Option result and its normal continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticOptionProducerV1 {
    option_local: SemanticLocalIdV1,
    continuation: SemanticBlockIdV1,
}

impl SemanticOptionProducerV1 {
    /// Creates an inert producer description for a known Option result.
    pub const fn new(option_local: SemanticLocalIdV1, continuation: SemanticBlockIdV1) -> Self {
        Self {
            option_local,
            continuation,
        }
    }

    /// Classifies an operation through the central Option-producer list.
    ///
    /// Future Option-returning compiler intrinsics must be added to this match.
    pub fn from_compiler_intrinsic(
        operation: &SemanticCompilerIntrinsicOperationV1,
        call: &SemanticDirectCallV1,
    ) -> Result<Option<Self>, SemanticOptionDominanceErrorV1> {
        if !matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. }
        ) {
            return Ok(None);
        }
        let destination =
            call.destination()
                .ok_or(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability call has no continuation",
                ))?;
        if !destination.place().projections().is_empty() {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability call has no exact local destination",
            ));
        }
        Ok(Some(Self {
            option_local: destination.place().local(),
            continuation: destination.edge().target(),
        }))
    }

    /// Returns the exact Option result local.
    pub const fn option_local(self) -> SemanticLocalIdV1 {
        self.option_local
    }

    /// Returns the normal call-continuation block.
    pub const fn continuation(self) -> SemanticBlockIdV1 {
        self.continuation
    }
}

/// Collects the centrally classified Option-producer inventory in CFG order.
pub fn semantic_option_producers_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> Result<Vec<SemanticOptionProducerV1>, SemanticOptionDominanceErrorV1> {
    semantic_option_producers_core_v18(function, callables, &mut WorkBudgetV1::default())
}

/// Collects the same original-source inventory with live fallible accounting.
///
/// The second result is the checked retained header and actual vector capacity.
/// It excludes dropped scratch and confers no authority over optimized output.
pub fn semantic_option_producers_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    meter: &mut M,
) -> Result<(Vec<SemanticOptionProducerV1>, usize), SemanticOptionDominanceMeteredErrorV18<M::Error>> {
    with_dominance_meter_v18(meter, |budget| {
        let producers = semantic_option_producers_core_v18(function, callables, budget)?;
        let retained = std::mem::size_of::<Vec<SemanticOptionProducerV1>>()
            .checked_add(producers.capacity().checked_mul(std::mem::size_of::<SemanticOptionProducerV1>())
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?)
            .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
        Ok((producers, retained))
    })
}

fn semantic_option_producers_core_v18(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<Vec<SemanticOptionProducerV1>, SemanticOptionDominanceErrorV1> {
    let mut producers = budget.new_vec()?;
    budget.reserve(&mut producers, function.blocks().len(), false)?;
    for block in function.blocks() {
        budget.source_work(1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if let Some(producer) = SemanticOptionProducerV1::from_compiler_intrinsic(operation, call)?
        {
            budget.push(&mut producers, producer)?;
        }
    }
    Ok(producers)
}

/// Opaque identity for one exact Option Some dominance region.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticOptionAvailabilityV1(usize);

/// Read-only bounded availability facts for compiler-issued Option capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOptionDominanceV1 {
    availability_by_local: Box<[Option<SemanticOptionAvailabilityV1>]>,
    some_targets: Box<[SemanticBlockIdV1]>,
    dominator_preorder: Box<[usize]>,
    dominator_subtree_end: Box<[usize]>,
    work_units: usize,
}

impl SemanticOptionDominanceV1 {
    /// Analyzes one producer inventory with one shared dominator tree.
    pub fn analyze(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        Self::analyze_core_v18(function, producers, &mut WorkBudgetV1::default())
    }

    /// Runs the unchanged inert analysis against a live work/storage meter.
    ///
    /// Returns the report and its checked retained bytes. Original-source facts
    /// do not certify an optimized owner or replace output correspondence.
    pub fn analyze_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        meter: &mut M,
    ) -> Result<(Self, usize), SemanticOptionDominanceMeteredErrorV18<M::Error>> {
        with_dominance_meter_v18(meter, |budget| {
            let report = Self::analyze_core_v18(function, producers, budget)?;
            let bytes = std::mem::size_of::<Self>()
                .checked_add(std::mem::size_of_val(&*report.availability_by_local))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.some_targets)))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.dominator_preorder)))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.dominator_subtree_end)))
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
            Ok((report, bytes))
        })
    }

    fn analyze_core_v18(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        budget: &mut WorkBudgetV1<'_>,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let local_count = function.locals().len();
        budget.storage(std::mem::size_of::<Self>())?;
        let definitions = local_definition_counts(function, budget)?;
        let dominators = DominatorIntervalsV1::analyze(function, budget)?;
        let mut discriminants_by_option = budget.filled(local_count, Vec::new())?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.charge(block.statements().len().saturating_add(1))?;
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty()
                    || !place.projections().is_empty()
                {
                    continue;
                }
                let Some(bindings) =
                    discriminants_by_option.get_mut(place.local().index() as usize)
                else {
                    return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                        "an Option discriminator source is outside the local table",
                    ));
                };
                budget.reserve(bindings, 1, false)?;
                budget.push(bindings, (block_index, assignment.destination().local()))?;
            }
        }

        let mut availability_by_local = budget.filled(local_count, None)?;
        let mut some_targets = budget.new_vec()?;
        budget.reserve(&mut some_targets, producers.len(), false)?;
        for producer in producers {
            budget.charge(1)?;
            let destination_index = producer.option_local().index() as usize;
            if definitions.get(destination_index).copied() != Some(1) {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability local does not have one exact producer",
                ));
            }
            let [(switch_block, discriminator)] = discriminants_by_option
                .get(destination_index)
                .map(Vec::as_slice)
                .ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an Option capability destination is outside the local table",
                ))?
            else {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability does not have one exact discriminant binding",
                ));
            };
            if definitions.get(discriminator.index() as usize).copied() != Some(1) {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability discriminator does not have one exact definition",
                ));
            }
            if !dominators.dominates(producer.continuation().index() as usize, *switch_block) {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability discriminator is not dominated by its producer continuation",
                ));
            }
            let switch = function.blocks().get(*switch_block).ok_or(
                SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an Option discriminator block is outside the block table",
                ),
            )?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = switch.terminator().kind()
            else {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability discriminator is not consumed by its defining block",
                ));
            };
            if exact_operand_local(discriminant) != Some(*discriminator) {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability switch is not bound to its unique discriminator",
                ));
            }
            let some_target = match targets.values() {
                [target] => match target.value() {
                    0 => targets.otherwise().target(),
                    1 => target.edge().target(),
                    _ => {
                        return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                            "an Option capability switch has no exact Some edge",
                        ));
                    }
                },
                [zero, one] if zero.value() == 0 && one.value() == 1 => one.edge().target(),
                _ => {
                    return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                        "an Option capability switch is not an exact 0/1 branch",
                    ));
                }
            };
            if !dominators.is_reachable(some_target.index() as usize) {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "the authenticated Some edge is unreachable",
                ));
            }
            if !dominators.has_unique_predecessor(some_target.index() as usize, *switch_block) {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability Some target is not uniquely controlled by its exact branch",
                ));
            }
            let slot = availability_by_local.get_mut(destination_index).ok_or(
                SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an Option capability destination is outside the local table",
                ),
            )?;
            if slot
                .replace(SemanticOptionAvailabilityV1(some_targets.len()))
                .is_some()
            {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "one local has multiple Option capability producers",
                ));
            }
            budget.push(&mut some_targets, some_target)?;
        }
        Ok(Self {
            availability_by_local: budget.boxed(availability_by_local)?,
            some_targets: budget.boxed(some_targets)?,
            dominator_preorder: budget.boxed(dominators.preorder)?,
            dominator_subtree_end: budget.boxed(dominators.subtree_end)?,
            work_units: budget.used,
        })
    }

    /// Returns the availability identity for an Option-producing local.
    pub fn availability(&self, local: SemanticLocalIdV1) -> Option<SemanticOptionAvailabilityV1> {
        self.availability_by_local
            .get(local.index() as usize)
            .copied()
            .flatten()
    }

    /// Reports in O(1) whether the exact Some edge dominates block.
    pub fn allows(
        &self,
        availability: SemanticOptionAvailabilityV1,
        block: SemanticBlockIdV1,
    ) -> bool {
        self.some_targets.get(availability.0).is_some_and(|target| {
            dominates_with_intervals(
                &self.dominator_preorder,
                &self.dominator_subtree_end,
                target.index() as usize,
                block.index() as usize,
            )
        })
    }

    /// Returns deterministic charged analysis work.
    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    /// These inert facts never grant compiler, artifact, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Opaque identity for one exact enum-payload branch.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticEnumPayloadAvailabilityV1(usize);

/// Bounded dominance facts for payload extraction from ordinary Rust enums.
///
/// Unlike `SemanticOptionDominanceV1`, this analysis grants no producer
/// authority. It only identifies a branch that is uniquely selected by an
/// exact discriminant switch for one enum variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticEnumPayloadDominanceV1 {
    availability_by_local: Box<[Vec<(u32, SemanticEnumPayloadAvailabilityV1)>]>,
    payload_targets: Box<[SemanticBlockIdV1]>,
    dominator_preorder: Box<[usize]>,
    dominator_subtree_end: Box<[usize]>,
    work_units: usize,
}

fn enum_payload_target_is_unique_v1(
    targets: &SemanticSwitchTargetsV1,
    target: SemanticBlockIdV1,
    budget: &mut WorkBudgetV1,
) -> Result<bool, SemanticOptionDominanceErrorV1> {
    budget.charge(targets.values().len().saturating_add(1))?;
    let occurrences = targets
        .values()
        .iter()
        .filter(|case| case.edge().target() == target)
        .count()
        + usize::from(targets.otherwise().target() == target);
    Ok(occurrences == 1)
}

impl SemanticEnumPayloadDominanceV1 {
    /// Finds exact variant branches for every enum local with one exact
    /// discriminant binding and switch.
    pub fn analyze(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        Self::analyze_core_v18(function, types, &mut WorkBudgetV1::default())
    }

    /// Derives original-source variant dominance with live fallible accounting.
    ///
    /// The retained byte count includes every actual inner capacity. These
    /// inert facts still require exact output CFG and source correspondence.
    pub fn analyze_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        meter: &mut M,
    ) -> Result<(Self, usize), SemanticOptionDominanceMeteredErrorV18<M::Error>> {
        with_dominance_meter_v18(meter, |budget| {
            let report = Self::analyze_core_v18(function, types, budget)?;
            let mut bytes = std::mem::size_of::<Self>()
                .checked_add(std::mem::size_of_val(&*report.availability_by_local))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.payload_targets)))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.dominator_preorder)))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.dominator_subtree_end)))
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
            budget.source_work(report.availability_by_local.len())?;
            for variants in &report.availability_by_local {
                bytes = variants.capacity().checked_mul(std::mem::size_of::<(u32, SemanticEnumPayloadAvailabilityV1)>())
                    .and_then(|inner| bytes.checked_add(inner))
                    .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
            }
            Ok((report, bytes))
        })
    }

    fn analyze_core_v18(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &mut WorkBudgetV1<'_>,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let local_count = function.locals().len();
        budget.storage(std::mem::size_of::<Self>())?;
        let definitions = local_definition_counts(function, budget)?;
        let dominators = DominatorIntervalsV1::analyze(function, budget)?;
        let mut discriminants_by_enum = budget.filled(local_count, Vec::new())?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.charge(block.statements().len().saturating_add(1))?;
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty()
                    || !place.projections().is_empty()
                {
                    continue;
                }
                let Some(bindings) = discriminants_by_enum.get_mut(place.local().index() as usize)
                else {
                    return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                        "an enum discriminator source is outside the local table",
                    ));
                };
                budget.reserve(bindings, 1, false)?;
                budget.push(bindings, (block_index, assignment.destination().local()))?;
            }
        }

        let mut availability_by_local = budget.filled(local_count, Vec::new())?;
        let mut payload_targets = budget.new_vec()?;
        budget.reserve(&mut payload_targets, local_count, false)?;
        for (local_index, bindings) in discriminants_by_enum.iter().enumerate() {
            budget.source_work(1)?;
            let [(switch_block, discriminator)] = bindings.as_slice() else {
                continue;
            };
            if definitions.get(local_index).copied() != Some(1)
                || definitions.get(discriminator.index() as usize).copied() != Some(1)
            {
                continue;
            }
            let Some(local) = function.locals().get(local_index) else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an enum payload local is outside the local table",
                ));
            };
            let Some(ty) = types.get(local.ty().index() as usize) else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an enum payload type is outside the type table",
                ));
            };
            let SemanticTypeShapeV1::Enum { variants, .. } = ty.shape() else {
                continue;
            };
            let Some(block) = function.blocks().get(*switch_block) else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an enum payload switch is outside the block table",
                ));
            };
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            if exact_operand_local(discriminant) != Some(*discriminator) {
                continue;
            }
            budget.charge(
                variants
                    .len()
                    .saturating_mul(targets.values().len().saturating_add(1)),
            )?;
            let mut otherwise_variant = None;
            let mut otherwise_is_ambiguous = false;
            budget.source_work(variants.len().checked_mul(targets.values().len().saturating_add(1))
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?)?;
            for (variant_index, variant) in variants.iter().enumerate() {
                if variant.is_uninhabited()
                    || targets
                        .values()
                        .iter()
                        .any(|target| target.value() == variant.discriminant())
                {
                    continue;
                }
                if otherwise_variant.replace(variant_index as u32).is_some() {
                    otherwise_is_ambiguous = true;
                    break;
                }
            }
            for (variant_index, variant) in variants.iter().enumerate() {
                if variant.is_uninhabited() {
                    continue;
                }
                let explicit = targets
                    .values()
                    .iter()
                    .find(|target| target.value() == variant.discriminant())
                    .map(|target| target.edge().target());
                let target = explicit.or_else(|| {
                    (!otherwise_is_ambiguous && otherwise_variant == Some(variant_index as u32))
                        .then(|| targets.otherwise().target())
                });
                let Some(target) = target else {
                    continue;
                };
                // A shared predecessor block does not establish which edge
                // arrived, including an otherwise edge with no variant fact.
                if !enum_payload_target_is_unique_v1(targets, target, budget)? {
                    continue;
                }
                if !dominators.is_reachable(target.index() as usize)
                    || !dominators.has_unique_predecessor(target.index() as usize, *switch_block)
                {
                    continue;
                }
                let availability = SemanticEnumPayloadAvailabilityV1(payload_targets.len());
                budget.push(&mut payload_targets, target)?;
                budget.push(&mut availability_by_local[local_index], (variant_index as u32, availability))?;
            }
        }
        Ok(Self {
            availability_by_local: budget.boxed(availability_by_local)?,
            payload_targets: budget.boxed(payload_targets)?,
            dominator_preorder: budget.boxed(dominators.preorder)?,
            dominator_subtree_end: budget.boxed(dominators.subtree_end)?,
            work_units: budget.used,
        })
    }

    /// Returns the exact branch identity in O(log V) for one local and variant.
    pub fn availability(
        &self,
        local: SemanticLocalIdV1,
        variant: u32,
    ) -> Option<SemanticEnumPayloadAvailabilityV1> {
        let variants = self.availability_by_local.get(local.index() as usize)?;
        let index = variants
            .binary_search_by_key(&variant, |(candidate, _)| *candidate)
            .ok()?;
        Some(variants[index].1)
    }

    /// Queries the existing variant index with a charge before every inspected
    /// key or result slot. A caller refusal never becomes absent availability.
    pub fn availability_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
        &self, local: SemanticLocalIdV1, variant: u32, meter: &mut M,
    ) -> Result<Option<SemanticEnumPayloadAvailabilityV1>, M::Error> {
        meter.reserve_storage(std::mem::size_of::<Result<Option<SemanticEnumPayloadAvailabilityV1>, M::Error>>())?;
        meter.reserve_storage(std::mem::size_of::<(usize, usize, usize, u32)>())?;
        meter.charge_work(1)?;
        let Some(variants) = self.availability_by_local.get(local.index() as usize) else { return Ok(None); };
        let (mut first, mut end) = (0, variants.len());
        while first < end {
            let middle = first + (end - first) / 2;
            meter.charge_work(1)?;
            match variants[middle].0.cmp(&variant) {
                std::cmp::Ordering::Less => first = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    meter.charge_work(1)?;
                    return Ok(Some(variants[middle].1));
                }
            }
        }
        Ok(None)
    }

    /// Reports in O(1) whether the exact variant edge dominates `block`.
    pub fn allows(
        &self,
        availability: SemanticEnumPayloadAvailabilityV1,
        block: SemanticBlockIdV1,
    ) -> bool {
        self.payload_targets
            .get(availability.0)
            .is_some_and(|target| {
                dominates_with_intervals(
                    &self.dominator_preorder,
                    &self.dominator_subtree_end,
                    target.index() as usize,
                    block.index() as usize,
                )
            })
    }

    /// Pays the existing target and three interval-slot queries before running
    /// the original dominance predicate. These inert facts grant no authority.
    pub fn allows_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
        &self, availability: SemanticEnumPayloadAvailabilityV1, block: SemanticBlockIdV1,
        meter: &mut M,
    ) -> Result<bool, M::Error> {
        meter.reserve_storage(std::mem::size_of::<Result<bool, M::Error>>())?;
        meter.reserve_storage(std::mem::size_of::<(usize, usize, usize)>())?;
        meter.charge_work(1)?;
        let Some(target) = self.payload_targets.get(availability.0) else { return Ok(false); };
        meter.charge_work(3)?;
        Ok(dominates_with_intervals(&self.dominator_preorder, &self.dominator_subtree_end,
            target.index() as usize, block.index() as usize))
    }

    /// Reports whether `dominator` is reachable and dominates `block`.
    ///
    /// This exposes only an inert CFG fact. Consumers remain responsible for
    /// proving that the value they retain was defined before leaving the
    /// dominating block.
    pub fn block_dominates(&self, dominator: SemanticBlockIdV1, block: SemanticBlockIdV1) -> bool {
        dominates_with_intervals(
            &self.dominator_preorder,
            &self.dominator_subtree_end,
            dominator.index() as usize,
            block.index() as usize,
        )
    }

    /// Returns deterministic charged analysis work.
    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    /// These inert facts never grant compiler, artifact, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// One unchecked arithmetic operation lacking the exact checked-operation
/// false-edge proof required by safe Rust.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticUncheckedArithmeticViolationV1 {
    operation: SemanticUncheckedBinaryOpV1,
    block: SemanticBlockIdV1,
    statement: u32,
}

impl SemanticUncheckedArithmeticViolationV1 {
    pub const fn operation(self) -> SemanticUncheckedBinaryOpV1 {
        self.operation
    }

    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }

    pub const fn statement(self) -> u32 {
        self.statement
    }
}

#[derive(Clone, Copy)]
struct CheckedArithmeticProducerV1<'a> {
    local: SemanticLocalIdV1,
    operation: crate::semantic_mir_v1::SemanticCheckedBinaryOpV1,
    left: &'a SemanticOperandV1,
    right: &'a SemanticOperandV1,
    block: usize,
}

/// Verifies rustc's general safe-checked-arithmetic refinement pattern.
///
/// An unchecked add, subtract, or multiply is admitted only when the same
/// operands were used by the corresponding checked operation and the exact
/// zero-overflow switch edge dominates the unchecked operation. The zero edge
/// must have the switch as its unique predecessor, so a join cannot forge the
/// precondition.
pub fn semantic_unchecked_arithmetic_violation_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<Option<SemanticUncheckedArithmeticViolationV1>, SemanticOptionDominanceErrorV1> {
    let mut budget = WorkBudgetV1::default();
    let definitions = local_definition_counts(function, &mut budget)?;
    let dominators = DominatorIntervalsV1::analyze(function, &mut budget)?;
    let mut aliases = vec![None; function.locals().len()];
    let mut producers = Vec::new();
    producers
        .try_reserve(function.locals().len())
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;

    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len())?;
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if destination.projections().is_empty()
                && definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    == Some(1)
            {
                if let SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
                ) = assignment.value().kind()
                {
                    aliases[destination.local().index() as usize] = Some(source);
                }
                if let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() {
                    producers.push(CheckedArithmeticProducerV1 {
                        local: destination.local(),
                        operation: checked.operation(),
                        left: checked.left(),
                        right: checked.right(),
                        block: block_index,
                    });
                }
            }
        }
    }

    let mut producer_by_local = Vec::new();
    producer_by_local
        .try_reserve_exact(function.locals().len())
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
    producer_by_local.resize(function.locals().len(), None);
    for (producer_index, producer) in producers.iter().enumerate() {
        producer_by_local[producer.local.index() as usize] = Some(producer_index);
    }
    let mut safe_targets_by_producer = Vec::new();
    safe_targets_by_producer
        .try_reserve_exact(producers.len())
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
    safe_targets_by_producer.resize_with(producers.len(), Vec::new);
    for (switch_block, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        budget.charge(targets.values().len().saturating_add(1))?;
        let Some(overflow_place) = resolve_alias_place(discriminant, &aliases, &mut budget)? else {
            continue;
        };
        let [overflow_projection] = overflow_place.projections() else {
            continue;
        };
        if overflow_projection.kind() != SemanticProjectionKindV1::Field(1) {
            continue;
        }
        let Some(producer_index) = producer_by_local
            .get(overflow_place.local().index() as usize)
            .copied()
            .flatten()
        else {
            continue;
        };
        let producer = producers[producer_index];
        if !dominators.dominates(producer.block, switch_block) {
            continue;
        }
        let only_boolean_values = targets
            .values()
            .iter()
            .all(|target| matches!(target.value(), 0 | 1));
        if !only_boolean_values {
            continue;
        }
        let zero_target = targets
            .values()
            .iter()
            .find(|target| target.value() == 0)
            .map(|target| target.edge().target())
            .or_else(|| {
                targets
                    .values()
                    .iter()
                    .any(|target| target.value() == 1)
                    .then(|| targets.otherwise().target())
            });
        let Some(zero_target) = zero_target else {
            continue;
        };
        if dominators.is_reachable(zero_target.index() as usize)
            && dominators.has_unique_predecessor(zero_target.index() as usize, switch_block)
        {
            safe_targets_by_producer[producer_index]
                .try_reserve(1)
                .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
            safe_targets_by_producer[producer_index].push(zero_target);
        }
    }

    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::UncheckedBinary(unchecked) = assignment.value().kind() else {
                continue;
            };
            let mut proved = false;
            for (producer_index, producer) in producers.iter().enumerate() {
                budget.charge(1)?;
                if producer.operation != unchecked.operation().checked()
                    || !same_operand_value(producer.left, unchecked.left())
                    || !same_operand_value(producer.right, unchecked.right())
                {
                    continue;
                }
                for safe_target in &safe_targets_by_producer[producer_index] {
                    budget.charge(1)?;
                    if dominators.dominates(safe_target.index() as usize, block_index) {
                        proved = true;
                        break;
                    }
                }
                if proved {
                    break;
                }
            }
            if !proved {
                return Ok(Some(SemanticUncheckedArithmeticViolationV1 {
                    operation: unchecked.operation(),
                    block: SemanticBlockIdV1::from_index(block_index as u32),
                    statement: statement_index as u32,
                }));
            }
        }
    }
    Ok(None)
}

fn resolve_alias_place<'a>(
    operand: &'a SemanticOperandV1,
    aliases: &[Option<&'a SemanticPlaceV1>],
    budget: &mut WorkBudgetV1,
) -> Result<Option<&'a SemanticPlaceV1>, SemanticOptionDominanceErrorV1> {
    let mut place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        SemanticOperandV1::Constant(_) => return Ok(None),
    };
    for _ in 0..=aliases.len() {
        budget.charge(1)?;
        if !place.projections().is_empty() {
            return Ok(Some(place));
        }
        let Some(Some(source)) = aliases.get(place.local().index() as usize) else {
            return Ok(Some(place));
        };
        place = source;
    }
    // A cycle cannot establish a proof, but it may be unrelated to the
    // unchecked operation being verified. Treat it as an inexact candidate.
    Ok(None)
}

fn same_operand_value(left: &SemanticOperandV1, right: &SemanticOperandV1) -> bool {
    match (left, right) {
        (
            SemanticOperandV1::Copy(left) | SemanticOperandV1::Move(left),
            SemanticOperandV1::Copy(right) | SemanticOperandV1::Move(right),
        ) => left == right,
        (SemanticOperandV1::Constant(left), SemanticOperandV1::Constant(right)) => left == right,
        _ => false,
    }
}

#[derive(Default)]
struct WorkBudgetV1<'m> {
    used: usize,
    meter: Option<&'m mut dyn DominanceMeterV18>,
}

impl WorkBudgetV1<'_> {
    fn charge(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        self.used =
            self.used
                .checked_add(amount)
                .ok_or(SemanticOptionDominanceErrorV1::WorkLimit {
                    actual: usize::MAX,
                    limit: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
                })?;
        if self.used > MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 {
            return Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: self.used,
                limit: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
            });
        }
        if let Some(meter) = &mut self.meter { meter.work(amount, true)?; }
        Ok(())
    }

    fn source_work(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter { meter.work(amount, false)?; }
        Ok(())
    }

    fn storage(&mut self, bytes: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter { meter.storage(bytes)?; }
        Ok(())
    }

    fn new_vec<T>(&mut self) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        self.storage(std::mem::size_of::<Vec<T>>())?;
        Ok(Vec::new())
    }

    fn reserve<T>(&mut self, rows: &mut Vec<T>, additional: usize, exact: bool)
        -> Result<(), SemanticOptionDominanceErrorV1>
    {
        use SemanticOptionDominanceErrorV1::Storage;
        let requested = rows.len().checked_add(additional).ok_or(Storage)?;
        let grows = requested > rows.capacity();
        if grows && self.meter.is_some() {
            self.source_work(rows.len())?;
            self.storage(requested.checked_mul(std::mem::size_of::<T>()).ok_or(Storage)?)?;
        }
        if exact { rows.try_reserve_exact(additional) } else { rows.try_reserve(additional) }
            .map_err(|_| Storage)?;
        if grows && self.meter.is_some() {
            self.storage(rows.capacity().checked_sub(requested)
                .and_then(|extra| extra.checked_mul(std::mem::size_of::<T>())).ok_or(Storage)?)?;
        }
        Ok(())
    }

    fn filled<T: Clone>(&mut self, count: usize, value: T) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() { return Ok(vec![value; count]); }
        self.source_work(count)?;
        let mut rows = self.new_vec()?;
        self.reserve(&mut rows, count, true)?;
        rows.resize(count, value);
        Ok(rows)
    }

    fn capacity<T>(&mut self, count: usize) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() { return Ok(Vec::with_capacity(count)); }
        let mut rows = self.new_vec()?;
        self.reserve(&mut rows, count, true)?;
        Ok(rows)
    }

    fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), SemanticOptionDominanceErrorV1> {
        if self.meter.is_some() {
            self.source_work(1)?;
            self.reserve(rows, 1, false)?;
        }
        rows.push(value);
        Ok(())
    }

    fn boxed<T>(&mut self, rows: Vec<T>) -> Result<Box<[T]>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() || rows.capacity() == rows.len() { return Ok(rows.into_boxed_slice()); }
        let count = rows.len();
        let mut exact = self.new_vec()?;
        self.reserve(&mut exact, count, true)?;
        if exact.capacity() != count { return Err(SemanticOptionDominanceErrorV1::Storage); }
        self.source_work(count)?;
        exact.extend(rows);
        Ok(exact.into_boxed_slice())
    }
}

struct DominatorIntervalsV1 {
    entry: usize,
    preorder: Vec<usize>,
    subtree_end: Vec<usize>,
    predecessors: Vec<Vec<usize>>,
}

impl DominatorIntervalsV1 {
    fn analyze(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let block_count = function.blocks().len();
        let entry = function.entry().index() as usize;
        if block_count == 0 || entry >= block_count {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the semantic CFG has no valid entry block",
            ));
        }
        budget.charge(block_count)?;
        budget.storage(std::mem::size_of::<Self>())?;
        let mut successors = budget.filled(block_count, Vec::new())?;
        let mut predecessors = budget.filled(block_count, Vec::new())?;
        for (source, block) in function.blocks().iter().enumerate() {
            block
                .terminator()
                .kind()
                .try_for_each_edge::<SemanticOptionDominanceErrorV1>(|edge| {
                    budget.charge(1)?;
                    let target = edge.target().index() as usize;
                    if target >= block_count {
                        return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                            "a semantic CFG edge is outside the block table",
                        ));
                    }
                    budget.push(&mut successors[source], target)?;
                    budget.push(&mut predecessors[target], source)?;
                    Ok(())
                })?;
        }

        let mut visited = budget.filled(block_count, false)?;
        let mut postorder = budget.capacity(block_count)?;
        let mut pending = budget.filled(1, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                budget.push(&mut postorder, block)?;
            } else if !visited[block] {
                visited[block] = true;
                budget.push(&mut pending, (block, true))?;
                for successor in successors[block].iter().rev() {
                    budget.charge(1)?;
                    if !visited[*successor] {
                        budget.push(&mut pending, (*successor, false))?;
                    }
                }
            }
        }
        budget.source_work(postorder.len())?;
        postorder.reverse();
        let mut rpo_index = budget.filled(block_count, usize::MAX)?;
        budget.source_work(postorder.len())?;
        for (index, block) in postorder.iter().copied().enumerate() {
            rpo_index[block] = index;
        }
        let mut immediate = budget.filled(block_count, None)?;
        immediate[entry] = Some(entry);
        loop {
            budget.charge(1)?;
            let mut changed = false;
            for block in postorder.iter().copied().skip(1) {
                budget.charge(1)?;
                budget.source_work(predecessors[block].len())?;
                let mut processed = predecessors[block]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor].is_some());
                let Some(mut next) = processed.next() else {
                    continue;
                };
                for predecessor in processed {
                    budget.charge(1)?;
                    next = intersect_dominator_paths(
                        predecessor,
                        next,
                        &immediate,
                        &rpo_index,
                        budget,
                    )?;
                }
                if immediate[block] != Some(next) {
                    immediate[block] = Some(next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        immediate[entry] = None;
        let mut children = budget.filled(block_count, Vec::new())?;
        budget.source_work(immediate.len())?;
        for (block, parent) in immediate.iter().copied().enumerate() {
            if let Some(parent) = parent {
                budget.push(&mut children[parent], block)?;
            }
        }
        let mut preorder = budget.filled(block_count, usize::MAX)?;
        let mut subtree_end = budget.filled(block_count, usize::MAX)?;
        let mut clock = 0_usize;
        let mut pending = budget.filled(1, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                subtree_end[block] = clock;
            } else {
                preorder[block] = clock;
                clock += 1;
                budget.push(&mut pending, (block, true))?;
                for child in children[block].iter().rev() {
                    budget.push(&mut pending, (*child, false))?;
                }
            }
        }
        Ok(Self {
            entry,
            preorder,
            subtree_end,
            predecessors,
        })
    }

    fn has_unique_predecessor(&self, block: usize, predecessor: usize) -> bool {
        // Entry also has the implicit edge from the function invocation.
        block != self.entry
            && matches!(
                self.predecessors.get(block).map(Vec::as_slice),
                Some([exact]) if *exact == predecessor
            )
    }

    fn is_reachable(&self, block: usize) -> bool {
        self.preorder.get(block).copied() != Some(usize::MAX)
    }

    fn dominates(&self, dominator: usize, block: usize) -> bool {
        dominates_with_intervals(&self.preorder, &self.subtree_end, dominator, block)
    }
}

fn intersect_dominator_paths(
    mut left: usize,
    mut right: usize,
    immediate: &[Option<usize>],
    rpo_index: &[usize],
    budget: &mut WorkBudgetV1,
) -> Result<usize, SemanticOptionDominanceErrorV1> {
    while left != right {
        budget.charge(1)?;
        while rpo_index[left] > rpo_index[right] {
            budget.charge(1)?;
            left = immediate[left].ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a processed dominator path has no parent",
            ))?;
        }
        while rpo_index[right] > rpo_index[left] {
            budget.charge(1)?;
            right = immediate[right].ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a processed dominator path has no parent",
            ))?;
        }
    }
    Ok(left)
}

fn dominates_with_intervals(
    preorder: &[usize],
    subtree_end: &[usize],
    dominator: usize,
    block: usize,
) -> bool {
    let (Some(start), Some(end), Some(candidate)) = (
        preorder.get(dominator).copied(),
        subtree_end.get(dominator).copied(),
        preorder.get(block).copied(),
    ) else {
        return false;
    };
    start != usize::MAX && candidate != usize::MAX && start <= candidate && candidate < end
}

fn exact_operand_local(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place.local())
        }
        SemanticOperandV1::Copy(_)
        | SemanticOperandV1::Move(_)
        | SemanticOperandV1::Constant(_) => None,
    }
}

fn local_definition_counts(
    function: &SemanticFunctionDeclV1,
    budget: &mut WorkBudgetV1,
) -> Result<Vec<u8>, SemanticOptionDominanceErrorV1> {
    budget.charge(function.locals().len())?;
    let mut definitions = budget.new_vec()?;
    budget.reserve(&mut definitions, function.locals().len(), true)?;
    definitions.extend(
        function
            .locals()
            .iter()
            .map(|local| u8::from(local.role().is_entry_argument())),
    );
    let mut record = |place: &SemanticPlaceV1| {
        let Some(slot) = definitions.get_mut(place.local().index() as usize) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a semantic definition is outside the local table",
            ));
        };
        *slot = slot.saturating_add(1);
        Ok(())
    };
    for block in function.blocks() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => record(assignment.destination())?,
                SemanticStatementKindV1::Store(store) => record(store.destination())?,
                SemanticStatementKindV1::AtomicRmw(atomic) => record(atomic.destination())?,
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record(atomic.destination())?
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record(place)?,
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            record(destination.place())?;
        }
    }
    Ok(definitions)
}

#[cfg(test)]
#[path = "semantic_argument_entry_v1_tests.rs"]
mod argument_entry_tests;

#[cfg(test)]
#[path = "semantic_enum_payload_admission_v1_tests.rs"]
mod enum_payload_admission_tests;

#[cfg(test)]
mod tests {
    use super::SemanticEnumPayloadDominanceV1;
    use crate::semantic_mir_v1::SemanticBlockIdV1;

    #[test]
    fn enum_payload_cfg_dominance_rejects_siblings_and_unreachable_blocks() {
        let facts = SemanticEnumPayloadDominanceV1 {
            availability_by_local: Box::new([]),
            payload_targets: Box::new([]),
            dominator_preorder: Box::new([0, 1, 2, usize::MAX]),
            dominator_subtree_end: Box::new([3, 2, 3, usize::MAX]),
            work_units: 0,
        };

        assert!(facts.block_dominates(
            SemanticBlockIdV1::from_index(0),
            SemanticBlockIdV1::from_index(2)
        ));
        assert!(!facts.block_dominates(
            SemanticBlockIdV1::from_index(1),
            SemanticBlockIdV1::from_index(2)
        ));
        assert!(!facts.block_dominates(
            SemanticBlockIdV1::from_index(0),
            SemanticBlockIdV1::from_index(3)
        ));
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RefusalV18 { Work(usize), Storage(usize) }

    struct MeterV18 {
        work: usize,
        storage: usize,
        work_limit: usize,
        storage_limit: usize,
        first: Option<RefusalV18>,
    }

    impl MeterV18 {
        fn new(work_limit: usize, storage_limit: usize) -> Self {
            Self { work: 0, storage: 0, work_limit, storage_limit, first: None }
        }
    }

    impl crate::SemanticAssertionMeterV1 for MeterV18 {
        type Error = RefusalV18;
        fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
            let next = self.work.checked_add(amount).unwrap_or(usize::MAX);
            if next > self.work_limit {
                let error = RefusalV18::Work(next);
                return Err(*self.first.get_or_insert(error));
            }
            self.work = next;
            Ok(())
        }
        fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
            let next = self.storage.checked_add(amount).unwrap_or(usize::MAX);
            if next > self.storage_limit {
                let error = RefusalV18::Storage(next);
                return Err(*self.first.get_or_insert(error));
            }
            self.storage = next;
            Ok(())
        }
    }

    fn dominance_fixture_v18() -> (crate::semantic_mir_v1::SemanticFunctionDeclV1, Vec<crate::semantic_mir_v1::SemanticTypeDeclV1>) {
        dominance_fixture_with_duplicates_v18(0)
    }

    #[test]
    fn live_variant_query_has_exact_work_storage_and_no_false_absence_on_refusal() {
        use super::SemanticEnumPayloadAvailabilityV1 as Availability;
        use crate::semantic_mir_v1::SemanticLocalIdV1;
        use std::mem::size_of;
        let facts = SemanticEnumPayloadDominanceV1 {
            availability_by_local: vec![vec![(0, Availability(0)), (7, Availability(1)), (u32::MAX, Availability(2))]].into_boxed_slice(),
            payload_targets: vec![SemanticBlockIdV1::from_index(0); 3].into_boxed_slice(),
            dominator_preorder: vec![0, 1].into_boxed_slice(),
            dominator_subtree_end: vec![2, 2].into_boxed_slice(), work_units: 0,
        };
        let bytes = size_of::<Result<Option<Availability>, RefusalV18>>() + size_of::<(usize, usize, usize, u32)>();
        for (work, storage) in [(3, bytes), (2, bytes), (3, bytes - 1)] {
            let mut meter = MeterV18::new(work, storage);
            let result = facts.availability_with_meter_v18(SemanticLocalIdV1::from_index(0), 7, &mut meter);
            if work == 3 && storage == bytes {
                assert_eq!(result.unwrap(), facts.availability(SemanticLocalIdV1::from_index(0), 7));
                assert_eq!((meter.work, meter.storage), (3, bytes));
            } else {
                let expected = if storage < bytes { RefusalV18::Storage(bytes) } else { RefusalV18::Work(3) };
                assert_eq!(result, Err(expected));
                assert_eq!(meter.first, Some(expected));
            }
        }
        let bytes = size_of::<Result<bool, RefusalV18>>() + size_of::<(usize, usize, usize)>();
        for (work, storage) in [(4, bytes), (3, bytes), (4, bytes - 1)] {
            let mut meter = MeterV18::new(work, storage);
            let result = facts.allows_with_meter_v18(Availability(1), SemanticBlockIdV1::from_index(1), &mut meter);
            if work == 4 && storage == bytes {
                assert_eq!(result.unwrap(), facts.allows(Availability(1), SemanticBlockIdV1::from_index(1)));
                assert_eq!((meter.work, meter.storage), (4, bytes));
            } else { assert!(result.is_err()); }
        }
    }

    fn dominance_fixture_with_duplicates_v18(duplicates: usize) -> (crate::semantic_mir_v1::SemanticFunctionDeclV1, Vec<crate::semantic_mir_v1::SemanticTypeDeclV1>) {
        use crate::semantic_mir_v1::*;
        let source = SemanticSourceProvenanceV1::unavailable();
        let ty = |index| SemanticTypeIdV1::from_index(index);
        let place = |local, index| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), Vec::new(), ty(index)).unwrap();
        let edge = |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
        let block = |tag, statements, terminator| SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]), source, statements,
            SemanticTerminatorV1::new(source, terminator)).unwrap();
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([10;32]), SemanticLayoutIdentityV1::from_sha256([11;32]),
            SemanticCanonAbiV1::Rust, false, false,
            vec![SemanticAbiValueV1::new(ty(1), SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()))],
            SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore)).unwrap();
        let locals = [SemanticLocalRoleV1::Return, SemanticLocalRoleV1::Argument(0), SemanticLocalRoleV1::Temporary]
            .into_iter().enumerate().map(|(i, role)| SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([20+i as u8;32]), ty(i as u32), role, source)).collect();
        let discriminant = SemanticStatementV1::new(source, SemanticStatementKindV1::Assign(
            SemanticAssignmentV1::new(place(2,2), SemanticRvalueV1::new(ty(2), SemanticRvalueKindV1::Discriminant(place(1,1))))));
        let switch = SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(2,2)),
            targets: SemanticSwitchTargetsV1::new(
                if duplicates == 0 {
                    vec![SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 2))]
                } else {
                    (0..duplicates).map(|value| SemanticSwitchTargetV1::new(value as u128,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1))).collect()
                },
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 1)).unwrap(),
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([30;32]), SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([31;32]), SemanticMonomorphizationIdentityV1::from_sha256([32;32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([33;32]), SemanticConstGenericArgumentsIdentityV1::from_sha256([34;32]),
            source, abi, locals, SemanticBlockIdV1::from_index(0),
            vec![block(40, vec![discriminant], switch), block(41, Vec::new(), SemanticTerminatorKindV1::Return),
                block(42, Vec::new(), SemanticTerminatorKindV1::Return)]).unwrap();
        let types = [SemanticTypeShapeV1::Unit, SemanticTypeShapeV1::Enum {
            discriminant: ty(2), variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(Vec::new()).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(Vec::new()).unwrap()),
            ].into_boxed_slice(),
        }, SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer{signed:false,bits:8})]
            .into_iter().enumerate().map(|(i, shape)| SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([50+i as u8;32]), SemanticLayoutIdentityV1::from_sha256([60+i as u8;32]),
                SemanticTypeLayoutV1::new(Some(u64::from(i != 0)), 1).unwrap(), shape)).collect();
        (function, types)
    }

    #[test]
    fn live_dominance_equals_legacy_and_retains_exact_original_reports() {
        use super::*;
        let (function, types) = dominance_fixture_v18();
        let producers = [SemanticOptionProducerV1::new(
            SemanticLocalIdV1::from_index(1), SemanticBlockIdV1::from_index(0))];
        let mut meter = MeterV18::new(usize::MAX, usize::MAX);
        let (option, option_bytes) = SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, &mut meter).unwrap();
        assert_eq!(option, SemanticOptionDominanceV1::analyze(&function, &producers).unwrap());
        assert_eq!(option_bytes, std::mem::size_of::<SemanticOptionDominanceV1>()
            + 3*std::mem::size_of::<Option<SemanticOptionAvailabilityV1>>()
            + std::mem::size_of::<SemanticBlockIdV1>() + 6*std::mem::size_of::<usize>());
        let (enums, enum_bytes) = SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function, &types, &mut meter).unwrap();
        assert_eq!(enums, SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap());
        let inner: usize = enums.availability_by_local.iter().map(|rows|
            rows.capacity()*std::mem::size_of::<(u32, SemanticEnumPayloadAvailabilityV1)>()).sum();
        assert_eq!(enum_bytes, std::mem::size_of::<SemanticEnumPayloadDominanceV1>()
            + 3*std::mem::size_of::<Vec<(u32,SemanticEnumPayloadAvailabilityV1)>>()
            + 2*std::mem::size_of::<SemanticBlockIdV1>() + 6*std::mem::size_of::<usize>() + inner);
        assert!(!option.grants_authority() && !enums.grants_authority());
        let (inventory, bytes) = semantic_option_producers_with_meter_v18(&function, &[], &mut meter).unwrap();
        assert_eq!(inventory, semantic_option_producers_v1(&function, &[]).unwrap());
        assert_eq!(bytes, std::mem::size_of_val(&inventory) + inventory.capacity()*std::mem::size_of::<SemanticOptionProducerV1>());
    }

    #[test]
    fn live_dominance_preserves_exact_first_refusal_and_exact_limits() {
        use super::*;
        let (function, types) = dominance_fixture_v18();
        let mut measured = MeterV18::new(usize::MAX, usize::MAX);
        let expected = SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function, &types, &mut measured).unwrap();
        let mut exact = MeterV18::new(measured.work, measured.storage);
        assert_eq!(SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function, &types, &mut exact).unwrap(), expected);
        for (work, storage) in [(measured.work-1, measured.storage), (measured.work, measured.storage-1), (0,0)] {
            let mut short = MeterV18::new(work, storage);
            let result = SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function, &types, &mut short);
            assert_eq!(result.unwrap_err(), SemanticOptionDominanceMeteredErrorV18::Meter(short.first.unwrap()));
        }
        let invalid = [SemanticOptionProducerV1::new(SemanticLocalIdV1::from_index(2), SemanticBlockIdV1::from_index(0))];
        let mut ample = MeterV18::new(usize::MAX,usize::MAX);
        assert_eq!(SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &invalid, &mut ample).unwrap_err(),
            SemanticOptionDominanceMeteredErrorV18::Analysis(SemanticOptionDominanceV1::analyze(&function,&invalid).unwrap_err()));
        assert_eq!(ample.first, None);
    }

    #[test]
    fn live_dominance_primitive_storage_work_and_legacy_logical_count_are_independent() {
        use super::*;
        let bytes = std::mem::size_of::<Vec<u64>>() + 3*std::mem::size_of::<u64>();
        let mut exact = MeterV18::new(3, bytes);
        let mut adapter = DominanceMeterAdapterV18 {meter:&mut exact, error:None};
        let mut budget = WorkBudgetV1 {used:0,meter:Some(&mut adapter)};
        let rows = budget.filled(3, 42_u64).unwrap();
        assert_eq!(rows, [42;3]);
        assert_eq!(budget.used, 0);
        drop(rows);
        drop(budget);
        assert_eq!((exact.work,exact.storage), (3,bytes));
        for (work, storage) in [(2,bytes), (3,bytes-1)] {
            let mut short = MeterV18::new(work,storage);
            let mut adapter = DominanceMeterAdapterV18 {meter:&mut short,error:None};
            let result = WorkBudgetV1 {used:0,meter:Some(&mut adapter)}.filled(3,42_u64);
            assert!(result.is_err());
            assert_eq!(adapter.error,Some(if work==2 {RefusalV18::Work(3)} else {RefusalV18::Storage(bytes)}));
        }
    }

    #[test]
    fn live_dominance_pays_duplicate_pending_incidences_without_changing_cfg_facts() {
        let (function, types) = dominance_fixture_with_duplicates_v18(32);
        let mut meter = MeterV18::new(usize::MAX,usize::MAX);
        let (actual, _) = SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function,&types,&mut meter).unwrap();
        assert_eq!(actual, SemanticEnumPayloadDominanceV1::analyze(&function,&types).unwrap());
        assert_eq!(actual.availability(crate::semantic_mir_v1::SemanticLocalIdV1::from_index(1),1),None);
        assert!(!actual.block_dominates(SemanticBlockIdV1::from_index(0),SemanticBlockIdV1::from_index(2)));
        let mut one_short = MeterV18::new(meter.work,meter.storage-1);
        assert_eq!(SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(&function,&types,&mut one_short).unwrap_err(),
            super::SemanticOptionDominanceMeteredErrorV18::Meter(one_short.first.unwrap()));
    }
}
