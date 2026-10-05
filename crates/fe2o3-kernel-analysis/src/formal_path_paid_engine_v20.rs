//! Paid indexes and cached facts over one existing original source facade.
use super::paid_relations_v20::InputCredit;
use super::*;
use crate::PresburgerQueryErrorV2;
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalFormalLaunchInputV19, CanonicalFormalReportErrorV19,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionOperationLocation,
    InterInvocationConflictRequirement, VerifiedCanonicalKernelIrModuleV18,
};

/// Failure of the paid lexical path query, never a successful conflict proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalPaidPathErrorV20 {
    /// The original owner-bound report/source query refused.
    Source(CanonicalFormalReportErrorV19),
    /// The cumulative mathematical query session refused.
    Query(PresburgerQueryErrorV2),
    /// Original shared-ledger work, storage or accounting refused.
    Resource(Resource),
    /// An exact original report row could not be joined to its retained source.
    InconsistentOriginalReport,
    /// The trusted lexical consumer rejected its observation.
    ConsumerRejected,
    /// A trusted construction/consumer/destructor unwound.
    Panicked,
}
impl std::fmt::Display for FormalPaidPathErrorV20 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "paid original-source path query: {self:?}")
    }
}
impl std::error::Error for FormalPaidPathErrorV20 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Query(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::InconsistentOriginalReport | Self::ConsumerRejected | Self::Panicked => None,
        }
    }
}
impl From<CanonicalFormalReportErrorV19> for FormalPaidPathErrorV20 {
    fn from(error: CanonicalFormalReportErrorV19) -> Self {
        Self::Source(error)
    }
}
impl From<PresburgerQueryErrorV2> for FormalPaidPathErrorV20 {
    fn from(error: PresburgerQueryErrorV2) -> Self {
        Self::Query(error)
    }
}
impl From<Resource> for FormalPaidPathErrorV20 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type PathResult<T> = std::result::Result<T, FormalPaidPathErrorV20>;

/// Conditional control-path result for exactly one recorded conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalPaidPathDecisionV20 {
    /// Both distinct-invocation orders are excluded for the full U64 coordinate
    /// interval. This does not establish launch legality or report completeness.
    ExcludedForAllU64Coordinates,
    /// The original reasons, unsupported grammar/domain, or solver uncertainty
    /// prevented a proof. This is not evidence of a concrete conflict.
    NotProved,
}

/// Fixed-size observation retaining an exact original conflict requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalPaidPathObservationV20 {
    ordinal: usize,
    requirement: InterInvocationConflictRequirement,
    decision: FormalPaidPathDecisionV20,
}
impl FormalPaidPathObservationV20 {
    /// Original conflict ordinal, unchanged by observation ordering.
    pub fn ordinal(self) -> usize {
        self.ordinal
    }
    /// Exact original allocation and operation-location pair.
    pub fn requirement(self) -> InterInvocationConflictRequirement {
        self.requirement
    }
    /// Conditional path exclusion or conservative absence of proof.
    pub fn decision(self) -> FormalPaidPathDecisionV20 {
        self.decision
    }
}

// Only the concrete private adapter for the genuine KIR report facade implements
// this in production. No public caller can supply alleged source facts.
pub(super) trait Queries<'owner> {
    fn analysis(&self) -> &FormalMemoryObligationAnalysis;
    fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18;
    fn original_function(&self) -> &Function;
    fn root_index(&self) -> usize;
    fn launch(&self) -> CanonicalFormalLaunchInputV19;
    fn width(&self) -> FormalIndexWidth;
    fn check(&self, budget: &mut Budget<'_>) -> PathResult<()>;
    fn block_count(&self, budget: &mut Budget<'_>) -> PathResult<usize>;
    fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> PathResult<Option<&'owner BasicBlock>>;
    fn block_ordinal(&self, block: BlockId, budget: &mut Budget<'_>) -> PathResult<Option<usize>>;
    fn reachable(&self, block: BlockId, budget: &mut Budget<'_>) -> PathResult<bool>;
    fn definition_count(&self, budget: &mut Budget<'_>) -> PathResult<usize>;
    fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> PathResult<Option<(usize, Option<&'owner Operation>)>>;
    fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> PathResult<bool>;
}

#[derive(Clone, Copy)]
struct SourceGuard {
    source: BlockId,
    target: BlockId,
    condition: ValueId,
    truth: bool,
}

pub(super) struct PaidEngine<'query, 'owner, 'budget, 'work, Q> {
    source: &'query Q,
    budget: &'budget mut Budget<'work>,
    credit: &'budget mut InputCredit,
    admitted: bool,
    affine: Vec<Option<Option<Affine>>>,
    guards: Vec<SourceGuard>,
    domains: Vec<Option<Vec<Fact>>>,
    accesses: Vec<(FunctionOperationLocation, usize)>,
    source_borrow: std::marker::PhantomData<&'owner Function>,
}

impl<'query, 'owner, 'budget, 'work, Q: Queries<'owner>>
    PaidEngine<'query, 'owner, 'budget, 'work, Q>
{
    pub(super) fn build(
        source: &'query Q,
        budget: &'budget mut Budget<'work>,
        credit: &'budget mut InputCredit,
    ) -> PathResult<Self> {
        source.check(budget)?;
        budget.charge_work(2)?;
        let root = source
            .original_owner()
            .module()
            .kernels
            .get(source.root_index())
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?;
        let admitted = source.analysis().is_complete()
            && source.width() == FormalIndexWidth::Bits64
            && matches!(root.domain, LaunchDomain::D1 { .. })
            && !source
                .analysis()
                .obligations()
                .inter_invocation_conflicts()
                .is_empty();
        // Unsupported or empty reports retain their exact rows but need no
        // source-fact census. They cannot acquire an aggregate all-clear.
        let blocks = if admitted {
            source.block_count(budget)?
        } else {
            0
        };
        let definitions = if admitted {
            source.definition_count(budget)?
        } else {
            0
        };
        let mut affine = credit.vector(definitions, budget)?;
        budget.charge_work(definitions)?;
        affine.resize(definitions, None);
        let mut domains = credit.vector(blocks, budget)?;
        // Each initialized Option<Vec<Fact>> is also visited once when the
        // cache drops, including an early refusal before any access query.
        budget.charge_work(blocks.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        domains.resize_with(blocks, || None);
        let guards = credit.vector(blocks.checked_mul(2).ok_or(Resource::Arithmetic)?, budget)?;
        let report_accesses = source.analysis().obligations().accesses();
        let mut accesses = credit.vector(report_accesses.len(), budget)?;
        for (ordinal, access) in report_accesses.iter().enumerate() {
            budget.charge_work(2)?;
            accesses.push((access.location(), ordinal));
        }
        crate::canonical_kir_inventory_v1::heap_sort(
            &mut accesses,
            budget,
            |left, right, budget| {
                budget.charge_work(2)?;
                Ok(left.0.cmp(&right.0))
            },
        )
        .map_err(|error| match error {
            crate::CanonicalKirInventoryErrorV1::Resource(error) => {
                FormalPaidPathErrorV20::Resource(error)
            }
            crate::CanonicalKirInventoryErrorV1::InconsistentOwner => {
                FormalPaidPathErrorV20::InconsistentOriginalReport
            }
        })?;
        for pair in accesses.windows(2) {
            budget.charge_work(1)?;
            if pair[0].0 == pair[1].0 {
                return Err(FormalPaidPathErrorV20::InconsistentOriginalReport);
            }
        }
        let mut result = Self {
            source,
            budget,
            credit,
            admitted,
            affine,
            guards,
            domains,
            accesses,
            source_borrow: std::marker::PhantomData,
        };
        for ordinal in 0..blocks {
            result.budget.charge_work(1)?;
            let block = source
                .block_at(ordinal, result.budget)?
                .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?;
            if !source.reachable(block.id, result.budget)? {
                continue;
            }
            let branch = match block
                .terminator
                .as_ref()
                .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?
            {
                Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => Some((*condition, *then_target, *else_target)),
                Terminator::Switch {
                    selector,
                    cases,
                    default_target,
                    ..
                } => {
                    if let Some(condition) = result.boolean_selector(*selector)? {
                        let mut yes = None;
                        let mut no = None;
                        for case in cases {
                            result.budget.charge_work(2)?;
                            if case.value == 1 && yes.is_none() {
                                yes = Some(case.target);
                            }
                            if case.value == 0 && no.is_none() {
                                no = Some(case.target);
                            }
                        }
                        Some((
                            condition,
                            yes.unwrap_or(*default_target),
                            no.unwrap_or(*default_target),
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some((condition, yes, no)) = branch {
                if yes == no {
                    continue;
                }
                for (target, truth) in [(yes, true), (no, false)] {
                    result.budget.charge_work(1)?;
                    if result.guards.len() == result.guards.capacity() {
                        return Err(FormalPaidPathErrorV20::InconsistentOriginalReport);
                    }
                    result.guards.push(SourceGuard {
                        source: block.id,
                        target,
                        condition,
                        truth,
                    });
                }
            }
        }
        Ok(result)
    }

    fn boolean_selector(&mut self, value: ValueId) -> PathResult<Option<ValueId>> {
        let Some((_, Some(op))) = self.source.definition(value, self.budget)? else {
            return Ok(None);
        };
        let OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            value,
            ..
        } = &op.kind
        else {
            return Ok(None);
        };
        let Some((_, Some(input))) = self.source.definition(*value, self.budget)? else {
            return Ok(None);
        };
        self.budget.charge_work(1)?;
        Ok((input.results[0].ty == Type::BOOL).then_some(*value))
    }

    fn domain(&mut self, block: BlockId) -> PathResult<usize> {
        let ordinal = self
            .source
            .block_ordinal(block, self.budget)?
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?;
        self.budget.charge_work(1)?;
        let cached = self
            .domains
            .get(ordinal)
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?;
        if cached.is_some() {
            return Ok(ordinal);
        }
        let mut facts = self.credit.vector(0, self.budget)?;
        for index in 0..self.guards.len() {
            self.budget.charge_work(1)?;
            let guard = self.guards[index];
            if self.source.unique_predecessor_dominates(
                guard.source,
                guard.target,
                block,
                self.budget,
            )? {
                fact_engine_v20::predicate(self, guard.condition, guard.truth, 0, &mut facts)?;
            }
        }
        self.budget.charge_work(1)?;
        self.domains[ordinal] = Some(facts);
        Ok(ordinal)
    }

    fn access(
        &mut self,
        location: FunctionOperationLocation,
    ) -> PathResult<&'query FormalMemoryAccess> {
        let (mut low, mut high) = (0, self.accesses.len());
        while low < high {
            self.budget.charge_work(2)?;
            let middle = low + (high - low) / 2;
            match self.accesses[middle].0.cmp(&location) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    return self
                        .source
                        .analysis()
                        .obligations()
                        .accesses()
                        .get(self.accesses[middle].1)
                        .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport);
                }
            }
        }
        Err(FormalPaidPathErrorV20::InconsistentOriginalReport)
    }

    pub(super) fn run(
        &mut self,
        queries: &mut impl paid_relations_v20::SolverQueries,
    ) -> PathResult<Vec<FormalPaidPathObservationV20>> {
        let source = self.source;
        let conflicts = source.analysis().obligations().inter_invocation_conflicts();
        let mut observations = self.credit.vector(conflicts.len(), self.budget)?;
        for (ordinal, &requirement) in conflicts.iter().enumerate() {
            self.budget.charge_work(4)?;
            let left = self.access(requirement.left())?;
            let right = self.access(requirement.right())?;
            if left.allocation() != requirement.allocation()
                || right.allocation() != requirement.allocation()
            {
                return Err(FormalPaidPathErrorV20::InconsistentOriginalReport);
            }
            let excluded = if self.admitted {
                let left = self.domain(requirement.left().block)?;
                let right = self.domain(requirement.right().block)?;
                paid_relations_v20::exclude(
                    self.domains[left]
                        .as_ref()
                        .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?,
                    self.domains[right]
                        .as_ref()
                        .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?,
                    queries,
                    self.budget,
                )?
            } else {
                false
            };
            observations.push(FormalPaidPathObservationV20 {
                ordinal,
                requirement,
                decision: if excluded {
                    FormalPaidPathDecisionV20::ExcludedForAllU64Coordinates
                } else {
                    FormalPaidPathDecisionV20::NotProved
                },
            });
        }
        self.source.check(self.budget)?;
        Ok(observations)
    }
}

impl<'owner, Q: Queries<'owner>> fact_engine_v20::Source<'owner>
    for PaidEngine<'_, 'owner, '_, '_, Q>
{
    type Error = FormalPaidPathErrorV20;
    fn step(&mut self) -> PathResult<()> {
        self.budget.check_prior_denials_v1()?;
        Ok(self.budget.charge_work(32)?)
    }
    fn operation(&mut self, value: ValueId) -> PathResult<Option<&'owner Operation>> {
        Ok(self
            .source
            .definition(value, self.budget)?
            .and_then(|(_, operation)| operation))
    }
    fn cached(&mut self, value: ValueId) -> PathResult<Option<Option<Affine>>> {
        let Some((ordinal, _)) = self.source.definition(value, self.budget)? else {
            return Ok(None);
        };
        self.budget.charge_work(1)?;
        self.affine
            .get(ordinal)
            .copied()
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)
    }
    fn cache(&mut self, value: ValueId, expression: Option<Affine>) -> PathResult<()> {
        let (ordinal, _) = self
            .source
            .definition(value, self.budget)?
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)?;
        self.budget.charge_work(1)?;
        *self
            .affine
            .get_mut(ordinal)
            .ok_or(FormalPaidPathErrorV20::InconsistentOriginalReport)? = Some(expression);
        Ok(())
    }
    fn push_fact(&mut self, facts: &mut Vec<Fact>, fact: Fact) -> PathResult<()> {
        Ok(self.credit.push(facts, fact, self.budget)?)
    }
    fn coordinate_last(&self) -> u64 {
        u64::MAX
    }
    fn index_width(&self) -> FormalIndexWidth {
        self.source.width()
    }
}
