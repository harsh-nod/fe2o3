//! One bounded path rule over recipe and live-reader observations.
//! This descriptive query carries no source, proof, lowering or launch authority.

use crate::{
    ProductionGpuWriteSiteV2, ProductionRankedKernelV1, ProductionRankedOperationV1 as Op,
    ProductionRankedTerminatorV1 as Term, ProductionRankedValueV1 as Value,
    ProductionSemanticExpressionV2 as Expression,
};
use dialect_kernel::AccessKindAttr;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use std::fmt;

mod live_v1;

use super::pliron_pipeline::invocation_receipt_v1::{
    AdditionalObservationV1, observe_additional_admission_v1,
};
use super::{
    PlironAnalysisManagerV1 as Manager, ProductionAnalysisInputCensusV1 as Census,
    ProductionAnalysisResourceLimitV1 as Limit, ProductionAnalysisResourcePhaseV1 as Phase,
    ProductionAnalysisResourceUpperBoundV1 as Bound,
    pliron_function_inventory::BoundedPlironFunctionInventoryV1 as Inventory,
};
use pliron::{
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    operation::Operation,
    r#type::TypeHandle,
    value::{DefiningEntity, Value as LiveValue},
};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuleSiteV1 {
    pub(crate) block: usize,
    pub(crate) operation: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValueKeyV1 {
    Argument { block: usize, index: usize },
    Result { site: RuleSiteV1, index: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuleFactsV1 {
    pub(crate) view: ValueKeyV1,
    pub(crate) index: ValueKeyV1,
    pub(crate) extent: ValueKeyV1,
    pub(crate) write: RuleSiteV1,
    pub(crate) normal_exits: [usize; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuleRefusalV1 {
    UnsupportedProfile,
    Coordinate,
    UnsupportedOperation { site: RuleSiteV1 },
    UnsupportedTerminator { block: usize },
    UnresolvedCondition { block: usize },
    AbnormalExit { block: usize, predicate: bool },
    WriteCount { block: usize, predicate: bool },
    Cycle { predicate: bool },
}
type LiveResultV1 = Result<Result<RuleFactsV1, RuleRefusalV1>, Limit>;
fn live_limit(resource: &'static str) -> Limit {
    Limit {
        phase: Phase::HierarchicalOwnership,
        resource,
    }
}
fn live_sum(xs: &[usize]) -> Result<usize, Limit> {
    xs.iter().try_fold(0usize, |n, &x| {
        n.checked_add(x)
            .ok_or_else(|| live_limit("conditional coverage arithmetic"))
    })
}
fn live_mul(a: usize, b: usize) -> Result<usize, Limit> {
    a.checked_mul(b)
        .ok_or_else(|| live_limit("conditional coverage arithmetic"))
}
fn live_preflight(c: Census) -> Result<Bound, Limit> {
    let (b, o, r, a, k, j, t) = (
        c.blocks,
        c.operations,
        c.results,
        c.block_arguments,
        c.attributes,
        c.identifier_bytes,
        c.type_nodes,
    );
    let v = live_sum(&[16, o, b, r, live_mul(2, a)?])?;
    let d = live_sum(&[256, live_mul(4, live_sum(&[j, k])?)?, live_mul(48, v)?])?;
    let core = live_sum(&[
        live_mul(30, b)?,
        live_mul(4, o)?,
        live_mul(live_mul(6, b)?, live_sum(&[1, b, live_mul(2, o)?])?)?,
    ])?;
    let preparation = live_sum(&[
        live_mul(32, live_sum(&[b, o, 1])?)?,
        live_mul(4, live_sum(&[t, 1])?)?,
        live_mul(a, live_sum(&[v, 4])?)?,
        live_mul(live_sum(&[o, 8])?, d)?,
    ])?;
    let literals = live_mul(live_mul(6, b)?, live_sum(&[live_mul(40, o)?, j, k])?)?;
    let terminators = live_mul(
        live_mul(3, b)?,
        live_sum(&[80, live_mul(2, v)?, live_mul(3, live_sum(&[b, 4])?)?, j, k])?,
    )?;
    let capture = live_sum(&[
        live_mul(3, live_sum(&[16, o, b, r, a])?)?,
        64,
        size_of::<Census>(),
    ])?;
    let scratch = live_sum(&[
        live_mul(live_mul(2, t)?, size_of::<TypeHandle>())?,
        live_mul(2, size_of::<Vec<TypeHandle>>())?,
        size_of::<LiveResultV1>(),
    ])?;
    Bound::checked_phase(
        Phase::HierarchicalOwnership,
        live_sum(&[core, preparation, literals, terminators, capture])?,
        0,
        scratch,
    )
}
struct ReservedMeter {
    remaining: Option<usize>,
}
impl Meter for ReservedMeter {
    type Error = Limit;
    fn charge(&mut self, n: usize) -> CheckResult<(), Limit> {
        self.remaining = self.remaining.and_then(|left| left.checked_sub(n));
        self.remaining
            .map(|_| ())
            .ok_or_else(|| Failure::Resource(live_limit("conditional coverage admitted work")))
    }
}
fn live_key(
    ctx: &Context,
    inv: &Inventory,
    value: LiveValue,
    m: &mut ReservedMeter,
) -> CheckResult<ValueKeyV1, Limit> {
    m.charge(4)?;
    match value.defining_entity() {
        DefiningEntity::Op(pointer) => {
            for site in inv.operations() {
                m.charge(1)?;
                if site.pointer() != pointer {
                    continue;
                }
                m.charge(4)?;
                let raw = site
                    .pointer()
                    .try_deref(ctx)
                    .map_err(|_| Fault::Coordinate)?;
                for index in 0..raw.get_num_results() {
                    m.charge(1)?;
                    if raw.get_result(index) == value {
                        m.charge(4)?;
                        return Ok(ValueKeyV1::Result {
                            site: RuleSiteV1 {
                                block: site.block(),
                                operation: site.operation(),
                            },
                            index,
                        });
                    }
                }
                return Err(Fault::Coordinate.into());
            }
        }
        DefiningEntity::Block(pointer) => {
            for (block, candidate) in inv.blocks().iter().enumerate() {
                m.charge(1)?;
                if *candidate != pointer {
                    continue;
                }
                m.charge(4)?;
                let raw = candidate.try_deref(ctx).map_err(|_| Fault::Coordinate)?;
                for index in 0..raw.get_num_arguments() {
                    m.charge(1)?;
                    if raw.get_argument(index) == value {
                        m.charge(4)?;
                        return Ok(ValueKeyV1::Argument { block, index });
                    }
                }
                return Err(Fault::Coordinate.into());
            }
        }
    }
    Err(Fault::Coordinate.into())
}
fn live_site(inv: &Inventory, s: Site, m: &mut ReservedMeter) -> CheckResult<RuleSiteV1, Limit> {
    m.charge(4)?;
    let b = usize::try_from(s.block).map_err(|_| Fault::Arithmetic)?;
    let o = usize::try_from(s.operation).map_err(|_| Fault::Arithmetic)?;
    inv.blocks().get(b).ok_or(Fault::Coordinate)?;
    let actual = inv.block_operations(b).get(o).ok_or(Fault::Coordinate)?;
    if actual.block() != b || actual.operation() != o {
        return Err(Fault::Coordinate.into());
    }
    Ok(RuleSiteV1 {
        block: actual.block(),
        operation: actual.operation(),
    })
}
fn live_exit(
    ctx: &Context,
    inv: &Inventory,
    block: u32,
    m: &mut ReservedMeter,
) -> CheckResult<usize, Limit> {
    m.charge(8)?;
    let b = usize::try_from(block).map_err(|_| Fault::Arithmetic)?;
    inv.blocks().get(b).ok_or(Fault::Coordinate)?;
    let tail = inv.block_operations(b).last().ok_or(Fault::Coordinate)?;
    let _raw = tail
        .pointer()
        .try_deref(ctx)
        .map_err(|_| Fault::Coordinate)?;
    if tail.block() != b || !Operation::is_op::<dialect_kernel::ReturnOp>(tail.pointer(), ctx) {
        return Err(Fault::Coordinate.into());
    }
    Ok(b)
}
fn live_refusal(fault: Fault) -> Result<RuleRefusalV1, Limit> {
    let u = |n: u32| usize::try_from(n).map_err(|_| live_limit("conditional coverage arithmetic"));
    Ok(match fault {
        Fault::Arithmetic => return Err(live_limit("conditional coverage arithmetic")),
        Fault::Coordinate => RuleRefusalV1::Coordinate,
        Fault::UnsupportedOperation { block, operation } => RuleRefusalV1::UnsupportedOperation {
            site: RuleSiteV1 {
                block: u(block)?,
                operation: u(operation)?,
            },
        },
        Fault::UnsupportedTerminator { block } => {
            RuleRefusalV1::UnsupportedTerminator { block: u(block)? }
        }
        Fault::UnresolvedCondition { block } => {
            RuleRefusalV1::UnresolvedCondition { block: u(block)? }
        }
        Fault::AbnormalExit { block, predicate } => RuleRefusalV1::AbnormalExit {
            block: u(block)?,
            predicate,
        },
        Fault::WriteCount { block, predicate } => RuleRefusalV1::WriteCount {
            block: u(block)?,
            predicate,
        },
        Fault::Cycle { predicate } => RuleRefusalV1::Cycle { predicate },
    })
}
// Caller authenticates context/function/census/session custody; expected_epoch is retained, never freshly trusted.
#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "preserve the test adapter's independently supplied live endpoint and selection inputs"
)]
pub(super) fn check_conditional_ownership_live_rule_v1(
    ctx: &Context,
    function: &FuncOp,
    inv: &Inventory,
    census: Census,
    expected_epoch: u64,
    ownership: Ptr<Operation>,
    original_view: LiveValue,
    am: &mut Manager,
) -> LiveResultV1 {
    check_conditional_ownership_live_rule_with_observation_v1(
        (ctx, function),
        inv,
        census,
        expected_epoch,
        (ownership, original_view),
        am,
        None,
    )
}

pub(super) fn check_conditional_ownership_live_rule_with_observation_v1(
    endpoint: (&Context, &FuncOp),
    inv: &Inventory,
    census: Census,
    expected_epoch: u64,
    selection: (Ptr<Operation>, LiveValue),
    am: &mut Manager,
    additional: AdditionalObservationV1<'_, '_, '_>,
) -> LiveResultV1 {
    match additional {
        None => check_conditional_ownership_live_rule_inner_v1(
            endpoint,
            inv,
            census,
            expected_epoch,
            selection,
            am,
            None,
        ),
        Some((observer, _)) => observer
            .with_projection(&Ok, |_| {
                check_conditional_ownership_live_rule_inner_v1(
                    endpoint,
                    inv,
                    census,
                    expected_epoch,
                    selection,
                    am,
                    additional,
                )
            })
            .map_err(|error| observer.deny(error)),
    }
}

fn check_conditional_ownership_live_rule_inner_v1(
    endpoint: (&Context, &FuncOp),
    inv: &Inventory,
    census: Census,
    expected_epoch: u64,
    selection: (Ptr<Operation>, LiveValue),
    am: &mut Manager,
    additional: AdditionalObservationV1<'_, '_, '_>,
) -> LiveResultV1 {
    let (ctx, function) = endpoint;
    let (ownership, original_view) = selection;
    let bound = live_preflight(census)?;
    if additional.is_some() {
        let phase = Phase::HierarchicalOwnership;
        let limits = am.remaining_resource_limits(phase)?;
        observe_additional_admission_v1(additional, limits, phase, Ok(bound))?;
    }
    am.admit_retained_resource_upper_bound(Phase::HierarchicalOwnership, bound)?;
    let mut m = ReservedMeter {
        remaining: Some(bound.work_upper_bound()),
    };
    let result = (|| -> CheckResult<RuleFactsV1, Limit> {
        m.charge(live_sum(&[size_of::<Census>(), 16]).map_err(Failure::Resource)?)?;
        if am.input_census() != Some(census) {
            return Err(Fault::Coordinate.into());
        }
        if ctx
            .ir_mutation_attempt_epoch()
            .map_err(|_| Fault::Coordinate)?
            .value()
            != expected_epoch
            || !std::ptr::eq(am.function_inventory().map_err(|_| Fault::Coordinate)?, inv)
        {
            return Err(Fault::Coordinate.into());
        }
        let p = live_v1::prepare(
            ctx,
            function,
            inv,
            census,
            expected_epoch,
            ownership,
            original_view,
            &mut m,
        )?;
        let exits = check_paths(&p.reader, p.selected, &mut m)?;
        p.reader.check_epoch(&mut m)?;
        let view = live_key(ctx, inv, p.view, &mut m)?;
        let index = live_key(ctx, inv, p.selected.index, &mut m)?;
        let extent = live_key(ctx, inv, p.selected.extent, &mut m)?;
        let write = live_site(inv, p.selected.write, &mut m)?;
        let normal_exits = [
            live_exit(ctx, inv, exits[0], &mut m)?,
            live_exit(ctx, inv, exits[1], &mut m)?,
        ];
        m.charge(4)?;
        p.reader.check_epoch(&mut m)?;
        Ok(RuleFactsV1 {
            view,
            index,
            extent,
            write,
            normal_exits,
        })
    })();
    match result {
        Ok(facts) => Ok(Ok(facts)),
        Err(Failure::Resource(error)) => Err(error),
        Err(Failure::Rule(fault)) => Ok(Err(live_refusal(fault)?)),
    }
}

#[cfg(test)]
mod tests;

/// Refusal of the predicate-sensitive recipe path query, not a proof result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionRankedRecipeCoverageErrorV1 {
    Resource(ResourceError),
    Coordinate,
    UnsupportedOperation { block: u32, operation: u32 },
    UnsupportedTerminator { block: u32 },
    UnresolvedCondition { block: u32 },
    AbnormalExit { block: u32, predicate: bool },
    WriteCount { block: u32, predicate: bool },
    Cycle { predicate: bool },
}

impl fmt::Display for ProductionRankedRecipeCoverageErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            _ => write!(out, "ranked recipe structural coverage: {self:?}"),
        }
    }
}

impl std::error::Error for ProductionRankedRecipeCoverageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    Arithmetic,
    Coordinate,
    UnsupportedOperation { block: u32, operation: u32 },
    UnsupportedTerminator { block: u32 },
    UnresolvedCondition { block: u32 },
    AbnormalExit { block: u32, predicate: bool },
    WriteCount { block: u32, predicate: bool },
    Cycle { predicate: bool },
}

enum Failure<E> {
    Resource(E),
    Rule(Fault),
}

impl<E> From<Fault> for Failure<E> {
    fn from(fault: Fault) -> Self {
        Self::Rule(fault)
    }
}

type CheckResult<T, E> = Result<T, Failure<E>>;

trait Meter {
    type Error;
    fn charge(&mut self, work: usize) -> CheckResult<(), Self::Error>;
}

impl Meter for Budget<'_> {
    type Error = ResourceError;

    fn charge(&mut self, work: usize) -> CheckResult<(), Self::Error> {
        self.charge_work(work).map_err(Failure::Resource)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Site {
    block: u32,
    operation: u32,
}

#[derive(Clone, Copy)]
struct Selection<V> {
    index: V,
    extent: V,
    write: Site,
}

enum OperationView {
    Inert,
    Write,
}

enum TerminatorView<V, T> {
    Branch(T),
    Return,
    Trap,
    Compare {
        equal: bool,
        lhs: V,
        rhs: V,
        true_block: T,
        false_block: T,
    },
}

// Readers establish representation-specific validity. Predicate interpretation,
// dead-operation admission, traversal and literal search stay in this core.
trait Reader {
    type Value: Copy + Eq;
    type Target: Copy;
    fn block_count(&self) -> usize;
    fn block_argument_count(&self, block: usize) -> usize;
    fn operation_count(&self, block: usize) -> usize;
    fn is_local(&self, value: Self::Value) -> bool;
    fn index_constant<M: Meter>(
        &self,
        block: usize,
        operation: usize,
        meter: &mut M,
    ) -> CheckResult<Option<(Self::Value, u64)>, M::Error>;
    fn operation<M: Meter>(
        &self,
        block: usize,
        operation: usize,
        meter: &mut M,
    ) -> CheckResult<OperationView, M::Error>;
    fn terminator<M: Meter>(
        &self,
        block: usize,
        meter: &mut M,
    ) -> CheckResult<TerminatorView<Self::Value, Self::Target>, M::Error>;
    fn target<M: Meter>(&self, target: Self::Target, meter: &mut M) -> CheckResult<u32, M::Error>;
}

fn ordinal(value: usize) -> Result<u32, Fault> {
    u32::try_from(value).map_err(|_| Fault::Arithmetic)
}

fn check_paths<R: Reader, M: Meter>(
    reader: &R,
    selected: Selection<R::Value>,
    meter: &mut M,
) -> CheckResult<[u32; 2], M::Error> {
    let mut write_seen = false;
    for b in 0..reader.block_count() {
        meter.charge(4)?;
        let block = ordinal(b)?;
        if reader.block_argument_count(b) != 0 {
            return Err(Fault::UnsupportedTerminator { block }.into());
        }
        for o in 0..reader.operation_count(b) {
            meter.charge(4)?;
            let operation = ordinal(o)?;
            match reader.operation(b, o, meter)? {
                OperationView::Write if selected.write == (Site { block, operation }) => {
                    write_seen = true;
                }
                OperationView::Inert => {}
                OperationView::Write => {
                    return Err(Fault::UnsupportedOperation { block, operation }.into());
                }
            }
        }
        successor(reader, block, selected, false, meter)?;
    }
    if !write_seen {
        return Err(Fault::Coordinate.into());
    }
    Ok([
        walk(reader, selected, false, meter)?,
        walk(reader, selected, true, meter)?,
    ])
}

enum Edge {
    Block(u32),
    Return,
    Trap,
}

fn walk<R: Reader, M: Meter>(
    reader: &R,
    selected: Selection<R::Value>,
    predicate: bool,
    meter: &mut M,
) -> CheckResult<u32, M::Error> {
    let mut block = 0;
    let mut wrote = false;
    // Each fixed predicate case has one successor. More than B visits is a cycle.
    for _ in 0..reader.block_count() {
        meter.charge(4)?;
        if block == selected.write.block {
            if wrote || !predicate {
                return Err(Fault::WriteCount { block, predicate }.into());
            }
            wrote = true;
        }
        match successor(reader, block, selected, predicate, meter)? {
            Edge::Block(target) => block = target,
            Edge::Return if wrote == predicate => return Ok(block),
            Edge::Return => return Err(Fault::WriteCount { block, predicate }.into()),
            Edge::Trap => return Err(Fault::AbnormalExit { block, predicate }.into()),
        }
    }
    Err(Fault::Cycle { predicate }.into())
}

fn successor<R: Reader, M: Meter>(
    reader: &R,
    block: u32,
    selected: Selection<R::Value>,
    predicate: bool,
    meter: &mut M,
) -> CheckResult<Edge, M::Error> {
    meter.charge(6)?;
    if block as usize >= reader.block_count() {
        return Err(Fault::Coordinate.into());
    }
    let target = match reader.terminator(block as usize, meter)? {
        TerminatorView::Branch(target) => target,
        TerminatorView::Return => return Ok(Edge::Return),
        TerminatorView::Trap => return Ok(Edge::Trap),
        TerminatorView::Compare {
            equal,
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            let condition = if !equal && lhs == selected.index && rhs == selected.extent {
                predicate
            } else {
                let lhs = literal(reader, lhs, meter)?;
                let rhs = literal(reader, rhs, meter)?;
                let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                    return Err(Fault::UnresolvedCondition { block }.into());
                };
                if equal { lhs == rhs } else { lhs < rhs }
            };
            if condition { true_block } else { false_block }
        }
    };
    Ok(Edge::Block(reader.target(target, meter)?))
}

fn literal<R: Reader, M: Meter>(
    reader: &R,
    value: R::Value,
    meter: &mut M,
) -> CheckResult<Option<u64>, M::Error> {
    meter.charge(1)?;
    if !reader.is_local(value) {
        return Ok(None);
    }
    for b in 0..reader.block_count() {
        meter.charge(1)?;
        for o in 0..reader.operation_count(b) {
            meter.charge(2)?;
            if let Some((result, literal)) = reader.index_constant(b, o, meter)?
                && result == value
            {
                return Ok(Some(literal));
            }
        }
    }
    Ok(None)
}

struct RecipeReader<'a>(&'a ProductionRankedKernelV1);

impl Reader for RecipeReader<'_> {
    type Value = Value;
    type Target = u32;

    fn block_count(&self) -> usize {
        self.0.blocks().len()
    }
    fn block_argument_count(&self, block: usize) -> usize {
        self.0.blocks()[block].index_argument_count() as usize
    }
    fn operation_count(&self, block: usize) -> usize {
        self.0.blocks()[block].operations().len()
    }
    fn is_local(&self, value: Value) -> bool {
        matches!(value, Value::Local(_))
    }

    fn index_constant<M: Meter>(
        &self,
        b: usize,
        o: usize,
        _meter: &mut M,
    ) -> CheckResult<Option<(Value, u64)>, M::Error> {
        Ok(match &self.0.blocks()[b].operations()[o] {
            Op::IndexConstant { result, value } => Some((Value::Local(*result), *value)),
            _ => None,
        })
    }

    fn operation<M: Meter>(
        &self,
        b: usize,
        o: usize,
        _meter: &mut M,
    ) -> CheckResult<OperationView, M::Error> {
        Ok(match &self.0.blocks()[b].operations()[o] {
            Op::IndexConstant { .. }
            | Op::ExecutionLayout { .. }
            | Op::InvocationIndex { .. }
            | Op::View { .. }
            | Op::ViewInSpace { .. }
            | Op::IndexUnknown { .. }
            | Op::SemanticConstant { .. }
            | Op::SemanticSymbol { .. }
            | Op::OwnershipContract { .. }
            | Op::RequestEffectRefinement { .. }
            | Op::RequireEffectRefinement { .. }
            | Op::SemanticExpression {
                expression: Expression::Constant { .. } | Expression::Symbol { .. },
                ..
            } => OperationView::Inert,
            Op::Access {
                kind: AccessKindAttr::Write,
                ..
            }
            | Op::ValueAccess {
                kind: AccessKindAttr::Write,
                ..
            } => OperationView::Write,
            _ => {
                return Err(Fault::UnsupportedOperation {
                    block: ordinal(b)?,
                    operation: ordinal(o)?,
                }
                .into());
            }
        })
    }

    fn terminator<M: Meter>(
        &self,
        b: usize,
        _meter: &mut M,
    ) -> CheckResult<TerminatorView<Value, u32>, M::Error> {
        Ok(match self.0.blocks()[b].terminator() {
            Term::Branch { target } => TerminatorView::Branch(*target),
            Term::Return => TerminatorView::Return,
            Term::Trap => TerminatorView::Trap,
            Term::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            }
            | Term::IndexEqual {
                lhs,
                rhs,
                true_block,
                false_block,
            } => TerminatorView::Compare {
                equal: matches!(self.0.blocks()[b].terminator(), Term::IndexEqual { .. }),
                lhs: *lhs,
                rhs: *rhs,
                true_block: *true_block,
                false_block: *false_block,
            },
            _ => return Err(Fault::UnsupportedTerminator { block: ordinal(b)? }.into()),
        })
    }

    fn target<M: Meter>(&self, target: u32, _meter: &mut M) -> CheckResult<u32, M::Error> {
        Ok(target)
    }
}

fn recipe_error(error: Failure<ResourceError>) -> ProductionRankedRecipeCoverageErrorV1 {
    use ProductionRankedRecipeCoverageErrorV1 as E;
    match error {
        Failure::Resource(error) => E::Resource(error),
        Failure::Rule(fault) => match fault {
            Fault::Arithmetic => E::Resource(ResourceError::Arithmetic),
            Fault::Coordinate => E::Coordinate,
            Fault::UnsupportedOperation { block, operation } => {
                E::UnsupportedOperation { block, operation }
            }
            Fault::UnsupportedTerminator { block } => E::UnsupportedTerminator { block },
            Fault::UnresolvedCondition { block } => E::UnresolvedCondition { block },
            Fault::AbnormalExit { block, predicate } => E::AbnormalExit { block, predicate },
            Fault::WriteCount { block, predicate } => E::WriteCount { block, predicate },
            Fault::Cycle { predicate } => E::Cycle { predicate },
        },
    }
}

/// Checks both cases of `index < extent`, returning `[false_exit, true_exit]`.
/// The true case must perform exactly the selected ordinary write; the false
/// case must perform none. Both must return normally. All operations, including
/// unreachable ones, must belong to the closed total fragment. The constructor
/// supplies recipe structural validity; this query does not bind an output,
/// authenticate the operands' source meaning, or compare stored values.
pub fn check_ranked_recipe_paths_v1(
    kernel: &ProductionRankedKernelV1,
    index: Value,
    extent: Value,
    write: ProductionGpuWriteSiteV2,
    budget: &mut Budget<'_>,
) -> Result<[u32; 2], ProductionRankedRecipeCoverageErrorV1> {
    check_paths(
        &RecipeReader(kernel),
        Selection {
            index,
            extent,
            write: Site {
                block: write.block(),
                operation: write.operation(),
            },
        },
        budget,
    )
    .map_err(recipe_error)
}
