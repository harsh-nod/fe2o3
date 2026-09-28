//! Actual-owner control-path exclusion over the complete U64 coordinate domain.

use super::*;
use crate::{PresburgerAffineNarrowingDecisionV3, PresburgerQueryErrorV2, PresburgerQueryScopeV2};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalOwnerFormalAnalysisV18,
    VerifiedCanonicalKernelIrModuleV18,
};

/// Refusal of this separate full-coordinate composition, never an empty proof.
#[derive(Debug)]
pub enum FormalPathExclusionErrorV19 {
    /// Original owner/report/CFG or fixed construction limits refused analysis.
    Path(FormalPathConflictErrorV1),
    /// Only original one-dimensional kernels with a 64-bit index are supported.
    UnsupportedCoordinateDomain,
    /// The shared query session refused or could not represent the relation.
    Query(PresburgerQueryErrorV2),
}
impl fmt::Display for FormalPathExclusionErrorV19 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "full-coordinate path exclusion V19: {self:?}")
    }
}
impl StdError for FormalPathExclusionErrorV19 {}
impl From<FormalPathConflictErrorV1> for FormalPathExclusionErrorV19 {
    fn from(error: FormalPathConflictErrorV1) -> Self {
        Self::Path(error)
    }
}
impl From<PresburgerQueryErrorV2> for FormalPathExclusionErrorV19 {
    fn from(error: PresburgerQueryErrorV2) -> Self {
        Self::Query(error)
    }
}
impl From<PresburgerFailureV1> for FormalPathExclusionErrorV19 {
    fn from(error: PresburgerFailureV1) -> Self {
        Self::Query(error.into())
    }
}

/// Observation for one exact ordinal of the retained report's conflict list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalPathExclusionDecisionV19 {
    /// No two distinct U64 X coordinates can reach these two access sites.
    /// This proves no address claim and does not cover any unlisted access pair.
    ExcludedForAllU64Coordinates,
    /// Affine consequences did not prove control-path exclusion. This is not a
    /// witness and does not imply that the original accesses can both execute.
    NotProved,
}

/// Lexically retains a genuine fresh V18 formal result and refines only its
/// recorded conflict rows. The coordinate domain is fixed to `[0, 2^64)` for
/// each of two distinct invocations. It is not the report's descriptive launch
/// witness and cannot be selected or shortened by the caller.
///
/// Only a genuine D1 kernel is supported: X uniqueness is not inferred for a
/// multidimensional launch. Affine source expressions must not overflow at any
/// point in the full domain. Unsupported predicates are omitted conservatively.
/// The retained `Bits64` width is a caller-selected formal interpretation, not
/// authenticated target custody. Exclusion is conditional on that interpretation;
/// actual target index semantics and launch/domain authority must be joined by
/// any future final admission, not inferred from this observation.
/// No address-overlap constraint is used: small-witness address/overflow facts
/// cannot silently become facts about all launches.
///
/// The entire original report, including incomplete reasons, remains unchanged.
/// Its conflict roster may omit pairs outside its witness. Thus even all rows
/// being excluded neither completes that report nor qualifies the whole kernel.
/// There is deliberately no all-clear or admission/publication conversion.
///
/// V3 mathematical queries use the supplied existing shared solver session and
/// ledger. Formal extraction, CFG/path construction, query-input allocation and
/// retained output remain separately bounded, not shared-ledger metered here.
/// This is a domain-proof prerequisite, not total compiler resource authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{FormalPathExclusionsV19, with_presburger_queries_v2};
/// use fe2o3_kernel_ir::*;
/// fn escape(owner: VerifiedCanonicalKernelIrModuleV18) -> FormalPathExclusionsV19<'static> {
///     let formal = CanonicalOwnerFormalScopeV18::new(&owner, Default::default()).unwrap()
///         .derive(&KernelId::new("kernel"), ExplicitLaunchExtent::Unknown,
///             FormalIndexWidth::Bits64).unwrap();
///     let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
///     let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
///     with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
///         Ok(FormalPathExclusionsV19::from_formal(formal, Default::default(), queries, budget))
///     }).unwrap().unwrap()
/// }
/// ```
#[derive(Debug)]
#[must_use = "per-conflict path exclusion is not complete memory admission"]
pub struct FormalPathExclusionsV19<'owner> {
    formal: CanonicalOwnerFormalAnalysisV18<'owner>,
    decisions: Vec<FormalPathExclusionDecisionV19>,
    queries: usize,
    construction_steps: usize,
}

impl<'owner> FormalPathExclusionsV19<'owner> {
    /// Consume the actual borrowed owner's fresh formal wrapper, preserving its
    /// selected root, entry, report and descriptive launch exactly. No caller
    /// report, alleged digest, or replacement module can enter this route.
    pub fn from_formal(
        formal: CanonicalOwnerFormalAnalysisV18<'owner>,
        limits: FormalPathConflictLimitsV1,
        queries: &mut PresburgerQueryScopeV2<'_>,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<Self, FormalPathExclusionErrorV19> {
        let limits = clamp(limits);
        let module = formal.owner().module();
        check_source_items(module, limits)?;
        let report = formal.analysis();
        let obligations = report.obligations();
        let selected = module
            .kernels
            .iter()
            .find(|kernel| &kernel.id == obligations.kernel())
            .ok_or(Error::InconsistentReport)?;
        if &selected.entry != obligations.entry() {
            return Err(Error::InconsistentReport.into());
        }
        if !matches!(selected.domain, LaunchDomain::D1 { .. })
            || obligations.index_width() != FormalIndexWidth::Bits64
        {
            return Err(FormalPathExclusionErrorV19::UnsupportedCoordinateDomain);
        }
        let conflicts = obligations.inter_invocation_conflicts();
        check(conflicts.len(), limits.conflicts, Resource::Conflicts)?;
        let function = module
            .functions
            .iter()
            .find(|function| &function.id == obligations.entry())
            .ok_or(Error::InconsistentReport)?;
        let cfg = analyze_control_flow(function).map_err(|_| Error::ControlFlow)?;
        let mut engine = Engine::with_coordinate_last(
            function,
            cfg,
            u64::MAX,
            FormalIndexWidth::Bits64,
            limits,
        )?;
        let accesses = obligations.accesses();
        let mut by_location = BTreeMap::new();
        for (ordinal, access) in accesses.iter().enumerate() {
            engine.step()?;
            if by_location.insert(access.location(), ordinal).is_some() {
                return Err(Error::InconsistentReport.into());
            }
        }
        let mut decisions = Vec::with_capacity(conflicts.len());
        for conflict in conflicts {
            engine.step()?;
            let left = accesses
                .get(
                    *by_location
                        .get(&conflict.left())
                        .ok_or(Error::InconsistentReport)?,
                )
                .ok_or(Error::InconsistentReport)?;
            let right = accesses
                .get(
                    *by_location
                        .get(&conflict.right())
                        .ok_or(Error::InconsistentReport)?,
                )
                .ok_or(Error::InconsistentReport)?;
            if left.allocation() != conflict.allocation()
                || right.allocation() != conflict.allocation()
            {
                return Err(Error::InconsistentReport.into());
            }
            decisions.push(engine.exclude_full_coordinate_paths(left, right, queries, budget)?);
        }
        Ok(Self {
            formal,
            decisions,
            queries: engine.queries,
            construction_steps: engine.steps,
        })
    }

    /// Exact actual-owner identity, not structural equality or receipt identity.
    pub fn belongs_to(&self, owner: &VerifiedCanonicalKernelIrModuleV18) -> bool {
        self.formal.belongs_to(owner)
    }
    /// Actual immutable owner used by both formal extraction and path analysis.
    pub const fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 {
        self.formal.owner()
    }
    /// Unchanged report and source/root/launch binding, including every reason.
    pub const fn formal(&self) -> &CanonicalOwnerFormalAnalysisV18<'owner> {
        &self.formal
    }
    /// One decision per retained conflict, in exactly the same ordinal order.
    pub fn decisions(&self) -> &[FormalPathExclusionDecisionV19] {
        &self.decisions
    }
    /// Actual V3 queries consumed from the caller's cumulative solver session.
    pub const fn queries(&self) -> usize {
        self.queries
    }
    /// Separately capped path-construction visits, not shared-ledger work.
    pub const fn construction_steps(&self) -> usize {
        self.construction_steps
    }
}

impl Engine<'_> {
    fn exclude_full_coordinate_paths(
        &mut self,
        left: &FormalMemoryAccess,
        right: &FormalMemoryAccess,
        queries: &mut PresburgerQueryScopeV2<'_>,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<FormalPathExclusionDecisionV19, FormalPathExclusionErrorV19> {
        self.domain(left.location().block)?;
        self.domain(right.location().block)?;
        let mut all_empty = true;
        // Both orders are required. Address expressions and the report's
        // witness interval are deliberately absent from these relations.
        for order in [[1, -1], [-1, 1]] {
            count(&mut self.queries, 1, self.limits.queries, Resource::Queries)?;
            let mut constraints = Vec::new();
            for (dimension, access) in [(0, left), (1, right)] {
                for fact in &self.domains[&access.location().block] {
                    let expression = fact.expression.lift(dimension)?;
                    constraints.push(if fact.equality {
                        Constraint::EqualZero(expression)
                    } else {
                        Constraint::LessEqualZero(expression)
                    });
                }
            }
            constraints.push(Constraint::LessEqualZero(Expr::new(1, order.to_vec())?));
            let domain = PresburgerBoxV1::new(vec![0; 2], vec![1_i128 << 64; 2])?;
            let set = PresburgerSetV1::new(domain, constraints)?;
            let empty = queries.with_affine_narrowing_v3(&set, budget, |decision, _| {
                Ok(matches!(
                    decision,
                    PresburgerAffineNarrowingDecisionV3::Empty
                ))
            })?;
            all_empty &= empty;
        }
        Ok(if all_empty {
            FormalPathExclusionDecisionV19::ExcludedForAllU64Coordinates
        } else {
            FormalPathExclusionDecisionV19::NotProved
        })
    }
}
