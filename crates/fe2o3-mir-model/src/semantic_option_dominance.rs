//! Bounded read-only control-flow facts for Option-wrapped GPU capabilities.
//!
//! These inert facts identify blocks dominated by the exact Some edge of a
//! unique compiler-intrinsic result. They grant no compiler or artifact authority.

use std::{error::Error, fmt};

use crate::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDirectCallV1, SemanticFunctionDeclV1, SemanticLocalIdV1, SemanticOperandV1,
    SemanticPlaceV1, SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeShapeV1, SemanticUncheckedBinaryOpV1,
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
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
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
    let mut producers = Vec::new();
    producers
        .try_reserve(function.blocks().len())
        .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
    for block in function.blocks() {
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
            producers.push(producer);
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
        let local_count = function.locals().len();
        let mut budget = WorkBudgetV1::default();
        let definitions = local_definition_counts(function, &mut budget)?;
        let dominators = DominatorIntervalsV1::analyze(function, &mut budget)?;
        let mut discriminants_by_option = vec![Vec::new(); local_count];
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
                bindings
                    .try_reserve(1)
                    .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
                bindings.push((block_index, assignment.destination().local()));
            }
        }

        let mut availability_by_local = vec![None; local_count];
        let mut some_targets = Vec::new();
        some_targets
            .try_reserve(producers.len())
            .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
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
            some_targets.push(some_target);
        }
        Ok(Self {
            availability_by_local: availability_by_local.into_boxed_slice(),
            some_targets: some_targets.into_boxed_slice(),
            dominator_preorder: dominators.preorder.into_boxed_slice(),
            dominator_subtree_end: dominators.subtree_end.into_boxed_slice(),
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

impl SemanticEnumPayloadDominanceV1 {
    /// Finds exact variant branches for every enum local with one exact
    /// discriminant binding and switch.
    pub fn analyze(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let local_count = function.locals().len();
        let mut budget = WorkBudgetV1::default();
        let definitions = local_definition_counts(function, &mut budget)?;
        let dominators = DominatorIntervalsV1::analyze(function, &mut budget)?;
        let mut discriminants_by_enum = vec![Vec::new(); local_count];
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
                bindings
                    .try_reserve(1)
                    .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
                bindings.push((block_index, assignment.destination().local()));
            }
        }

        let mut availability_by_local = vec![Vec::new(); local_count];
        let mut payload_targets = Vec::new();
        payload_targets
            .try_reserve(local_count)
            .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
        for (local_index, bindings) in discriminants_by_enum.iter().enumerate() {
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
                if !dominators.is_reachable(target.index() as usize)
                    || !dominators.has_unique_predecessor(target.index() as usize, *switch_block)
                {
                    continue;
                }
                let availability = SemanticEnumPayloadAvailabilityV1(payload_targets.len());
                payload_targets.push(target);
                availability_by_local[local_index].push((variant_index as u32, availability));
            }
        }
        Ok(Self {
            availability_by_local: availability_by_local.into_boxed_slice(),
            payload_targets: payload_targets.into_boxed_slice(),
            dominator_preorder: dominators.preorder.into_boxed_slice(),
            dominator_subtree_end: dominators.subtree_end.into_boxed_slice(),
            work_units: budget.used,
        })
    }

    /// Returns the exact branch identity for one enum local and variant.
    pub fn availability(
        &self,
        local: SemanticLocalIdV1,
        variant: u32,
    ) -> Option<SemanticEnumPayloadAvailabilityV1> {
        self.availability_by_local
            .get(local.index() as usize)?
            .iter()
            .find_map(|(candidate, availability)| (*candidate == variant).then_some(*availability))
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
struct WorkBudgetV1 {
    used: usize,
}

impl WorkBudgetV1 {
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
        Ok(())
    }
}

struct DominatorIntervalsV1 {
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
        let mut successors = vec![Vec::new(); block_count];
        let mut predecessors = vec![Vec::new(); block_count];
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
                    successors[source].push(target);
                    predecessors[target].push(source);
                    Ok(())
                })?;
        }

        let mut visited = vec![false; block_count];
        let mut postorder = Vec::with_capacity(block_count);
        let mut pending = vec![(entry, false)];
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                postorder.push(block);
            } else if !visited[block] {
                visited[block] = true;
                pending.push((block, true));
                for successor in successors[block].iter().rev() {
                    budget.charge(1)?;
                    if !visited[*successor] {
                        pending.push((*successor, false));
                    }
                }
            }
        }
        postorder.reverse();
        let mut rpo_index = vec![usize::MAX; block_count];
        for (index, block) in postorder.iter().copied().enumerate() {
            rpo_index[block] = index;
        }
        let mut immediate = vec![None; block_count];
        immediate[entry] = Some(entry);
        loop {
            budget.charge(1)?;
            let mut changed = false;
            for block in postorder.iter().copied().skip(1) {
                budget.charge(1)?;
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
        let mut children = vec![Vec::new(); block_count];
        for (block, parent) in immediate.iter().copied().enumerate() {
            if let Some(parent) = parent {
                children[parent].push(block);
            }
        }
        let mut preorder = vec![usize::MAX; block_count];
        let mut subtree_end = vec![usize::MAX; block_count];
        let mut clock = 0_usize;
        let mut pending = vec![(entry, false)];
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                subtree_end[block] = clock;
            } else {
                preorder[block] = clock;
                clock += 1;
                pending.push((block, true));
                for child in children[block].iter().rev() {
                    pending.push((*child, false));
                }
            }
        }
        Ok(Self {
            preorder,
            subtree_end,
            predecessors,
        })
    }

    fn has_unique_predecessor(&self, block: usize, predecessor: usize) -> bool {
        matches!(
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
    let mut definitions = vec![0_u8; function.locals().len()];
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
