//! One source assertion engine. Recipe queries are not owner admission.
//! A caller keeps paid storage alive until the analysis and all query outputs
//! have dropped, and independently authenticates the exact borrowed subjects.
use crate::semantic_mir_v1::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    mem::size_of,
};

#[path = "semantic_assertion_resources_v1.rs"]
mod resources;
use resources::{Budget, ErrorFor, MR, Metered, analysis_error, sum};
pub use resources::{
    MAX_SEMANTIC_ASSERTION_BLOCKS_V1, MAX_SEMANTIC_ASSERTION_CACHE_ENTRIES_V1,
    MAX_SEMANTIC_ASSERTION_EDGES_V1, MAX_SEMANTIC_ASSERTION_WORK_V1, SemanticAssertionErrorV1,
    SemanticAssertionLimitsV1, SemanticAssertionMeterV1, SemanticAssertionMeteredErrorV1,
    SemanticAssertionResourceObservationV1,
};

include!("semantic_assertion_inventory_v1.rs");
include!("semantic_assertion_range_v1.rs");
include!("semantic_assertion_checked_v1.rs");
include!("semantic_assertion_relations_v1.rs");
include!("semantic_assertion_dominance_v1.rs");
include!("semantic_assertion_callable_v1.rs");
include!("semantic_assertion_callable_body_v1.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedRangeProofV1 {
    pub minimum: u128,
    pub maximum: u128,
}

impl UnsignedRangeProofV1 {
    pub const fn exact(value: u128) -> Self {
        Self {
            minimum: value,
            maximum: value,
        }
    }

    pub const fn is_exact(self, value: u128) -> bool {
        self.minimum == value && self.maximum == value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarAssignmentSiteV1 {
    pub block: usize,
    pub statement: usize,
}

enum AssertionRangeOperandTaskV1 {
    Constant {
        ty: SemanticTypeIdV1,
        bits: Option<u128>,
    },
    Local(usize),
    CheckedResult {
        local: usize,
    },
    ProjectedPlace(SemanticPlaceV1),
}

enum AssertionRangeExpressionTaskV1 {
    Operand(AssertionRangeOperandTaskV1),
    Binary {
        operation: SemanticBinaryOpV1,
        destination_maximum: Option<u128>,
        left: AssertionRangeOperandTaskV1,
        right: AssertionRangeOperandTaskV1,
        left_source: SemanticOperandV1,
        right_source: SemanticOperandV1,
    },
    Unsupported,
}

struct AssertionStrictUpperBoundStateV1 {
    local: usize,
    use_block: usize,
    next_switch_block: usize,
    range: Option<UnsignedRangeProofV1>,
    can_reach_use: Vec<bool>,
    stability_visited: Vec<usize>,
    stability_pending: VecDeque<usize>,
    stability_generation: usize,
    proven_upper_bound: Option<u128>,
}

enum AssertionRangeFrameV1 {
    Operand {
        task: AssertionRangeOperandTaskV1,
        use_site: ScalarAssignmentSiteV1,
    },
    FinishBinary {
        operation: SemanticBinaryOpV1,
        destination_maximum: Option<u128>,
        left_source: SemanticOperandV1,
        right_source: SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    },
    FinishCheckedResult {
        local: usize,
        relational_range: Option<UnsignedRangeProofV1>,
    },
    FinishProjectedPlace {
        local: usize,
    },
    FinishLocalDefinition {
        local: usize,
        use_block: usize,
        maximum: u128,
    },
    ContinueStrictUpperBound(AssertionStrictUpperBoundStateV1),
    ApplyStrictUpperBoundCandidate {
        state: AssertionStrictUpperBoundStateV1,
        switch_block: usize,
        success_target: usize,
    },
}

#[derive(Debug)]
pub struct AuthenticatedCheckedBinaryValueV1 {
    local: usize,
    definition: ScalarAssignmentSiteV1,
    assertion_block: usize,
    checked: SemanticCheckedBinaryRvalueV1,
}

#[derive(Debug)]
pub struct AuthenticatedScaledQuotientRemainderV1 {
    divisor: SemanticOperandV1,
    divisor_use: ScalarAssignmentSiteV1,
    extent: SemanticOperandV1,
    extent_use: ScalarAssignmentSiteV1,
    scale: SemanticOperandV1,
    scale_use: ScalarAssignmentSiteV1,
    offset: SemanticOperandV1,
    offset_use: ScalarAssignmentSiteV1,
}

#[derive(Debug)]
pub struct AuthenticatedQuotientStrictBoundV1 {
    maximum: u128,
    factor: SemanticOperandV1,
    factor_use: ScalarAssignmentSiteV1,
}

impl AuthenticatedCheckedBinaryValueV1 {
    pub const fn local(&self) -> usize {
        self.local
    }
    pub const fn definition(&self) -> ScalarAssignmentSiteV1 {
        self.definition
    }
    pub const fn assertion_block(&self) -> usize {
        self.assertion_block
    }
    pub const fn checked(&self) -> &SemanticCheckedBinaryRvalueV1 {
        &self.checked
    }
}
impl AuthenticatedScaledQuotientRemainderV1 {
    pub const fn divisor(&self) -> &SemanticOperandV1 {
        &self.divisor
    }
    pub const fn divisor_use(&self) -> ScalarAssignmentSiteV1 {
        self.divisor_use
    }
    pub const fn extent(&self) -> &SemanticOperandV1 {
        &self.extent
    }
    pub const fn extent_use(&self) -> ScalarAssignmentSiteV1 {
        self.extent_use
    }
    pub const fn scale(&self) -> &SemanticOperandV1 {
        &self.scale
    }
    pub const fn scale_use(&self) -> ScalarAssignmentSiteV1 {
        self.scale_use
    }
    pub const fn offset(&self) -> &SemanticOperandV1 {
        &self.offset
    }
    pub const fn offset_use(&self) -> ScalarAssignmentSiteV1 {
        self.offset_use
    }
}
impl AuthenticatedQuotientStrictBoundV1 {
    pub const fn maximum(&self) -> u128 {
        self.maximum
    }
    pub const fn factor(&self) -> &SemanticOperandV1 {
        &self.factor
    }
    pub const fn factor_use(&self) -> ScalarAssignmentSiteV1 {
        self.factor_use
    }
}

/// The same source/CFG/range engine used by the legacy recipe adapter.
/// Construction reserves this fixed header, CFG construction scratch, and all
/// exposed payload capacities. The caller separately owns its facade/unwind
/// headers and retains every debit until this analysis and query outputs drop.
pub struct SemanticAssertionAnalysisV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    graph: SemanticAssertionCfgV1,
    definition_counts: Vec<u8>,
    block_definitions: Vec<Vec<usize>>,
    address_escaped: Vec<bool>,
    assignments: Vec<Option<ScalarAssignmentSiteV1>>,
    checked_assertion_blocks: Vec<Vec<usize>>,
    statement_definitions: Option<SemanticStatementDefinitionIndexV1<'a>>,
    dominance: HashMap<(usize, usize), bool>,
    zero_exclusion: HashMap<(usize, usize), bool>,
    resources: Budget,
}
impl<'a> SemanticAssertionAnalysisV1<'a> {
    pub fn new_metered<M: SemanticAssertionMeterV1>(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        limits: SemanticAssertionLimitsV1,
        meter: &mut M,
    ) -> MR<Self, M> {
        let mut resources = Budget::new(limits)?;
        let mut paid = resources.metered(meter);
        paid.reserve(size_of::<Self>())?;
        paid.charge(1)?;
        let graph = cfg_graph(function, &mut paid)?;
        let inventory = definition_inventory(function, &mut paid)?;
        let mut checked_assertion_blocks = paid.table(function.locals().len(), Vec::new())?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            paid.charge(2)?;
            let SemanticTerminatorKindV1::Assert { condition, .. } = block.terminator().kind()
            else {
                continue;
            };
            let Some(local) = tuple_field_operand_local_v1(condition, 1) else {
                continue;
            };
            let blocks = checked_assertion_blocks
                .get_mut(local.index() as usize)
                .ok_or(SemanticAssertionErrorV1::Unsupported(
                    "a checked arithmetic assertion is outside the semantic local table",
                ))?;
            paid.push(blocks, block_index)?;
        }
        Ok(Self {
            types,
            function,
            graph,
            definition_counts: inventory.counts,
            block_definitions: inventory.blocks,
            address_escaped: inventory.address_escaped,
            assignments: inventory.assignments,
            checked_assertion_blocks,
            statement_definitions: None,
            dominance: HashMap::new(),
            zero_exclusion: HashMap::new(),
            resources,
        })
    }
    pub fn types(&self) -> &'a [SemanticTypeDeclV1] {
        self.types
    }
    pub fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.function
    }
    pub fn graph(&self) -> &SemanticAssertionCfgV1 {
        &self.graph
    }
    pub fn definition_counts(&self) -> &[u8] {
        &self.definition_counts
    }
    pub fn address_escaped(&self) -> &[bool] {
        &self.address_escaped
    }
    pub fn assignments(&self) -> &[Option<ScalarAssignmentSiteV1>] {
        &self.assignments
    }
    pub fn resources(&self) -> SemanticAssertionResourceObservationV1 {
        self.resources.observation()
    }

    /// Cached zero-exclusion query count for diagnostics, not a proof result.
    pub fn zero_exclusion_cache_len_v1(&self) -> usize {
        self.zero_exclusion.len()
    }

    pub fn enable_statement_index_v1<M: SemanticAssertionMeterV1>(
        &mut self,
        meter: &mut M,
    ) -> MR<(), M> {
        if self.statement_definitions.is_none() {
            self.statement_definitions = Some(SemanticStatementDefinitionIndexV1::build(
                self.function,
                &mut self.resources.metered(meter),
            )?);
        }
        Ok(())
    }
    /// Read-only recipe queries cannot construct source-success facts.
    pub fn recipe_queries_v1<'q, M: SemanticAssertionMeterV1>(
        &'q mut self,
        meter: &'q mut M,
    ) -> SemanticAssertionRecipeQueriesV1<'q, 'a, M> {
        SemanticAssertionRecipeQueriesV1 {
            core: self,
            meter,
            use_statement_index: true,
        }
    }

    /// Diagnostic comparison path; it changes only index acceleration.
    pub fn recipe_queries_by_scanning_v1<'q, M: SemanticAssertionMeterV1>(
        &'q mut self,
        meter: &'q mut M,
    ) -> SemanticAssertionRecipeQueriesV1<'q, 'a, M> {
        SemanticAssertionRecipeQueriesV1 {
            core: self,
            meter,
            use_statement_index: false,
        }
    }
    pub fn has_statement_index_v1(&self) -> bool {
        self.statement_definitions.is_some()
    }

    pub fn assertion_at_v1<M: SemanticAssertionMeterV1>(
        &mut self,
        block: SemanticBlockIdV1,
        meter: &mut M,
    ) -> MR<SemanticAssertionOutcomeV1<'_, 'a>, M> {
        let index = block.index() as usize;
        let body =
            self.function
                .blocks()
                .get(index)
                .ok_or(SemanticAssertionErrorV1::InvalidModel(
                    "assertion block outside source function",
                ))?;
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            ..
        } = body.terminator().kind()
        else {
            self.resources.metered(meter).charge(1)?;
            return Ok(SemanticAssertionOutcomeV1::NotAnAssertion);
        };
        if !matches!(
            self.types
                .get(condition.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        ) {
            return Err(SemanticAssertionErrorV1::InvalidModel(
                "assertion condition is not source bool",
            )
            .into());
        }
        let mut query = self.recipe_queries_v1(meter);
        query.paid().charge(1)?;
        let kind = if query.proves_literal_shift_assert_v1(condition, *expected, message, index)? {
            Some(SemanticAssertionProofKindV1::LiteralShift)
        } else if query.proves_checked_overflow_assert_v1(condition, *expected, message, index)? {
            Some(SemanticAssertionProofKindV1::CheckedArithmetic)
        } else {
            let range = query.range_at_operand(condition, index, body.statements().len())?;
            if range.is_some_and(|r| r.is_exact(u128::from(*expected))) {
                Some(SemanticAssertionProofKindV1::ExactRange)
            } else if range.is_some_and(|r| r.is_exact(u128::from(!*expected))) {
                return Ok(SemanticAssertionOutcomeV1::Refuted(
                    SemanticAssertionRefutationV1 {
                        analysis: self,
                        block,
                    },
                ));
            } else {
                None
            }
        };
        Ok(match kind {
            Some(kind) => SemanticAssertionOutcomeV1::Proved(SemanticAssertionFactV1 {
                analysis: self,
                block,
                kind,
            }),
            None => SemanticAssertionOutcomeV1::NotProved(
                if matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                    SemanticAssertionNotProvedV1::BoundsNeedsIndependentRule
                } else {
                    SemanticAssertionNotProvedV1::Unknown
                },
            ),
        })
    }

    /// Source-bound legacy rule classification, not an admission Fact.
    pub fn legacy_assertion_rule_v1<M: SemanticAssertionMeterV1>(
        &mut self,
        block: SemanticBlockIdV1,
        rule: SemanticAssertionLegacyRuleV1,
        meter: &mut M,
    ) -> MR<SemanticAssertionLegacyDispositionV1, M> {
        self.resources.metered(meter).charge(1)?;
        let index = block.index() as usize;
        let body =
            self.function
                .blocks()
                .get(index)
                .ok_or(SemanticAssertionErrorV1::InvalidModel(
                    "legacy assertion block outside source function",
                ))?;
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            ..
        } = body.terminator().kind()
        else {
            return Ok(SemanticAssertionLegacyDispositionV1::NotProved);
        };
        let mut query = self.recipe_queries_v1(meter);
        let result = match rule {
            SemanticAssertionLegacyRuleV1::LiteralShift => {
                query.proves_literal_shift_assert_v1(condition, *expected, message, index)?
            }
            SemanticAssertionLegacyRuleV1::CheckedArithmetic => {
                query.proves_checked_overflow_assert_v1(condition, *expected, message, index)?
            }
        };
        Ok(if result {
            SemanticAssertionLegacyDispositionV1::Proved
        } else {
            SemanticAssertionLegacyDispositionV1::NotProved
        })
    }

    /// Compatibility bookkeeping only. BoundsDeferred never creates a Fact.
    pub fn legacy_recipe_assertions_v1<M: SemanticAssertionMeterV1>(
        &mut self,
        defined_callable: bool,
        meter: &mut M,
    ) -> MR<Vec<SemanticAssertionLegacyDispositionV1>, M> {
        let mut query = self.recipe_queries_v1(meter);
        let function = query.core.function;
        let mut result = query.paid().table(
            function.blocks().len(),
            SemanticAssertionLegacyDispositionV1::NotProved,
        )?;
        for (index, block) in function.blocks().iter().enumerate() {
            query.paid().charge(1)?;
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                ..
            } = block.terminator().kind()
            else {
                continue;
            };
            if !defined_callable && matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                result[index] = SemanticAssertionLegacyDispositionV1::BoundsDeferred;
                continue;
            }
            let shift =
                query.proves_literal_shift_assert_v1(condition, *expected, message, index)?;
            let checked = !shift
                && query.proves_checked_overflow_assert_v1(condition, *expected, message, index)?;
            let range = !shift
                && !checked
                && !(defined_callable
                    && matches!(message, SemanticAssertMessageV1::Overflow { .. }))
                && query
                    .range_at_operand(condition, index, block.statements().len())?
                    .is_some_and(|range| range.is_exact(u128::from(*expected)));
            if shift || checked || range {
                result[index] = SemanticAssertionLegacyDispositionV1::Proved;
            }
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionLegacyRuleV1 {
    LiteralShift,
    CheckedArithmetic,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionLegacyDispositionV1 {
    NotProved,
    Proved,
    BoundsDeferred,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionProofKindV1 {
    LiteralShift,
    CheckedArithmetic,
    ExactRange,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticAssertionNotProvedV1 {
    Unknown,
    BoundsNeedsIndependentRule,
}
pub enum SemanticAssertionOutcomeV1<'q, 'a> {
    NotAnAssertion,
    Proved(SemanticAssertionFactV1<'q, 'a>),
    Refuted(SemanticAssertionRefutationV1<'q, 'a>),
    NotProved(SemanticAssertionNotProvedV1),
}
/// A borrowed source result, not a caller-supplied grant or owner certificate.
///
/// ```compile_fail
/// use fe2o3_mir_model::semantic_assertion_v1::SemanticAssertionFactV1;
/// fn duplicate<'q, 's>(fact: &SemanticAssertionFactV1<'q, 's>) -> SemanticAssertionFactV1<'q, 's> {
///     fact.clone()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_mir_model::semantic_assertion_v1::{SemanticAssertionAnalysisV1, SemanticAssertionFactV1, SemanticAssertionProofKindV1};
/// use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1;
/// fn forge<'q, 's>(analysis: &'q SemanticAssertionAnalysisV1<'s>) -> SemanticAssertionFactV1<'q, 's> {
///     SemanticAssertionFactV1 { analysis, block: SemanticBlockIdV1::from_index(0), kind: SemanticAssertionProofKindV1::ExactRange }
/// }
/// ```
pub struct SemanticAssertionFactV1<'q, 'a> {
    analysis: &'q SemanticAssertionAnalysisV1<'a>,
    block: SemanticBlockIdV1,
    kind: SemanticAssertionProofKindV1,
}
pub struct SemanticAssertionRefutationV1<'q, 'a> {
    analysis: &'q SemanticAssertionAnalysisV1<'a>,
    block: SemanticBlockIdV1,
}
macro_rules! subject_accessors {
    ($name:ident) => {
        impl<'a> $name<'_, 'a> {
            pub fn types(&self) -> &'a [SemanticTypeDeclV1] {
                self.analysis.types
            }
            pub fn function(&self) -> &'a SemanticFunctionDeclV1 {
                self.analysis.function
            }
            pub const fn block(&self) -> SemanticBlockIdV1 {
                self.block
            }
            pub fn assertion(&self) -> &'a SemanticTerminatorKindV1 {
                self.analysis.function.blocks()[self.block.index() as usize]
                    .terminator()
                    .kind()
            }
            pub fn condition(&self) -> &'a SemanticOperandV1 {
                let SemanticTerminatorKindV1::Assert { condition, .. } = self.assertion() else {
                    unreachable!("sealed assertion subject")
                };
                condition
            }
            pub fn expected(&self) -> bool {
                let SemanticTerminatorKindV1::Assert { expected, .. } = self.assertion() else {
                    unreachable!("sealed assertion subject")
                };
                *expected
            }
            pub fn success_edge(&self) -> &'a SemanticControlFlowEdgeV1 {
                let SemanticTerminatorKindV1::Assert { target, .. } = self.assertion() else {
                    unreachable!("sealed assertion subject")
                };
                target
            }
        }
    };
}
subject_accessors!(SemanticAssertionFactV1);
subject_accessors!(SemanticAssertionRefutationV1);
impl SemanticAssertionFactV1<'_, '_> {
    pub const fn proof_kind(&self) -> SemanticAssertionProofKindV1 {
        self.kind
    }
}

pub struct SemanticAssertionRecipeQueriesV1<'q, 'a, M> {
    core: &'q mut SemanticAssertionAnalysisV1<'a>,
    meter: &'q mut M,
    use_statement_index: bool,
}
impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    fn paid(&mut self) -> Metered<'_, '_, M> {
        self.core.resources.metered(self.meter)
    }
    pub fn charge(&mut self, amount: usize) -> MR<(), M> {
        self.paid().legacy(amount)
    }
    fn scan_equivalent(&mut self, visits: usize) -> MR<(), M> {
        if visits == 0 {
            return Ok(());
        }
        self.paid().legacy_scan(visits)
    }
    fn same_operand(&mut self, left: &SemanticOperandV1, right: &SemanticOperandV1) -> MR<bool, M> {
        self.paid()
            .charge(sum(operand_rows(left), operand_rows(right))?)?;
        Ok(same_semantic_operand_value_v1(left, right))
    }
}
fn operand_rows(operand: &SemanticOperandV1) -> usize {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            place.projections().len().saturating_add(1)
        }
        SemanticOperandV1::Constant(value) => match value.value() {
            SemanticConstantValueV1::Bytes(bytes) => bytes.as_bytes().len().saturating_add(1),
            _ => 1,
        },
    }
}
fn pop_assertion_range_value_v1<M: SemanticAssertionMeterV1>(
    values: &mut Vec<Option<UnsignedRangeProofV1>>,
) -> MR<Option<UnsignedRangeProofV1>, M> {
    values.pop().ok_or_else(|| {
        analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "assertion range evaluator result stack is inconsistent",
        ))
    })
}
fn insert_assertion_proof_cache_with_limit<M: SemanticAssertionMeterV1>(
    mut paid: Metered<'_, '_, M>,
    cache: &mut HashMap<(usize, usize), bool>,
    key: (usize, usize),
    value: bool,
    limit: usize,
) -> MR<(), M> {
    paid.charge(sum(cache.len(), 1)?)?;
    if !cache.contains_key(&key) {
        if cache.len() >= limit {
            return Err(SemanticAssertionErrorV1::Unsupported(
                "assertion proof cache exceeds the bounded entry limit",
            )
            .into());
        }
        paid.map_reserve(cache, 1)?;
    }
    cache.insert(key, value);
    Ok(())
}

#[cfg(test)]
#[path = "semantic_assertion_hostile_v1_tests.rs"]
mod hostile_tests;
#[cfg(test)]
#[path = "semantic_assertion_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "semantic_assertion_v1_tests.rs"]
mod tests;

impl SemanticAssertionAnalysisV1<'_> {
    pub fn scalar_unsigned_maximum(&self, ty: SemanticTypeIdV1) -> Option<u128> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => Some(1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }) => match *bits {
                1..=127 => Some((1_u128 << bits) - 1),
                128 => Some(u128::MAX),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn unsigned_integer_bits(&self, ty: SemanticTypeIdV1) -> Option<u16> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }) => Some(*bits),
            _ => None,
        }
    }
    pub fn literal_unsigned_subtraction_upper_range_v1(
        &self,
        checked: &SemanticCheckedBinaryRvalueV1,
    ) -> Option<UnsignedRangeProofV1> {
        if checked.operation() != SemanticCheckedBinaryOpV1::Subtract
            || checked.left().ty() != checked.right().ty()
            || self.unsigned_integer_bits(checked.left().ty()).is_none()
        {
            return None;
        }
        let SemanticOperandV1::Constant(minuend) = checked.left() else {
            return None;
        };
        let SemanticConstantValueV1::Scalar(value) = minuend.value() else {
            return None;
        };
        let maximum = self.scalar_unsigned_maximum(minuend.ty())?;
        (value.bits() <= maximum).then_some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum: value.bits(),
        })
    }
}

fn insert_assertion_proof_cache<M: SemanticAssertionMeterV1>(
    paid: Metered<'_, '_, M>,
    cache: &mut HashMap<(usize, usize), bool>,
    key: (usize, usize),
    value: bool,
) -> MR<(), M> {
    insert_assertion_proof_cache_with_limit(
        paid,
        cache,
        key,
        value,
        MAX_SEMANTIC_ASSERTION_CACHE_ENTRIES_V1,
    )
}
