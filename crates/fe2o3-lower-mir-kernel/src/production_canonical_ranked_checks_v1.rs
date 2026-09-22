/// Source requirement not represented by the initial scalar/control reader.
/// Extending the same transaction requires real readers, not a caller permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalRankedSourceRequirementV1 {
    /// The checked view belongs to another authoritative graph.
    GraphIdentity,
    /// Retained local memory/source policy needs its real verifier.
    LocalMemory,
    /// Pointer, nominal or other non-scalar source type.
    Type,
    /// Source argument ownership, including unused and zero-width carriers.
    ArgumentOwnership,
    /// Actual memory effect or helper allocation.
    Effect,
    /// Source assertion, including a source-rule-elided assertion.
    Assertion,
    /// Nonempty source pipeline definition or emitted binding.
    Catalog,
    /// Intrinsic, external or refinement callable.
    Callable,
    /// Source call/transport, even when it emits no operation.
    CallTransport,
    /// A source statement with unsupported semantic requirements.
    Statement,
    /// Source rvalue with unsupported memory or precondition requirements.
    Rvalue,
    /// A source control occurrence with unsupported safety requirements.
    Terminator,
    /// Synthetic allocation, payload memory or trap.
    Synthetic,
}

/// Exact source or fixed-policy refusal; no completed-ranked authority.
#[derive(Debug)]
pub enum ProductionCanonicalRankedPolicyErrorV1 {
    /// Genuine source/view custody or resource error.
    Source(ProductionCanonicalRankedSourceErrorV1),
    /// Full fixed-check invocation failure with its accepted resource history.
    Policy(fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1),
    /// Paid short-view query failed.
    Query(fe2o3_pliron::CanonicalRankedPolicyFailureV1),
    /// Unsupported typed source fact and its original roster position.
    Unsupported {
        /// Required reader, not an optional annotation.
        requirement: ProductionCanonicalRankedSourceRequirementV1,
        /// Exact ordinal in that fact's source roster.
        ordinal: usize,
    },
}
impl std::fmt::Display for ProductionCanonicalRankedPolicyErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Policy(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
            Self::Unsupported {
                requirement,
                ordinal,
            } => write!(
                f,
                "canonical source policy requires {requirement:?} reader at {ordinal}"
            ),
        }
    }
}
impl std::error::Error for ProductionCanonicalRankedPolicyErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Policy(error) => Some(error),
            Self::Query(error) => Some(error),
            Self::Unsupported { .. } => None,
        }
    }
}
type CrPolicyResultV1<T> = Result<T, ProductionCanonicalRankedPolicyErrorV1>;
impl From<ProductionCanonicalRankedSourceErrorV1> for ProductionCanonicalRankedPolicyErrorV1 {
    fn from(e: ProductionCanonicalRankedSourceErrorV1) -> Self {
        Self::Source(e)
    }
}
impl From<ArgumentResourceV1> for ProductionCanonicalRankedPolicyErrorV1 {
    fn from(e: ArgumentResourceV1) -> Self {
        Self::Source(e.into())
    }
}
impl From<fe2o3_pliron::CanonicalRankedPolicyFailureV1> for ProductionCanonicalRankedPolicyErrorV1 {
    fn from(e: fe2o3_pliron::CanonicalRankedPolicyFailureV1) -> Self {
        Self::Query(e)
    }
}

fn cr_policy_unsupported_v1(
    requirement: ProductionCanonicalRankedSourceRequirementV1,
    ordinal: usize,
) -> ProductionCanonicalRankedPolicyErrorV1 {
    ProductionCanonicalRankedPolicyErrorV1::Unsupported {
        requirement,
        ordinal,
    }
}

fn cr_policy_source_profile_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrPolicyResultV1<()> {
    use ProductionCanonicalRankedSourceRequirementV1 as Need;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticRvalueKindV1 as Rvalue, SemanticStatementKindV1 as Statement,
        SemanticTerminatorKindV1 as Terminator, SemanticTypeShapeV1 as Shape,
    };
    source.guard.query(budget)?;
    if source.owner.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::RawEmpty {
        return Err(cr_policy_unsupported_v1(Need::LocalMemory, 0));
    }
    if !source.inventory.effects().is_empty() {
        budget.charge_work(1)?;
        return Err(cr_policy_unsupported_v1(Need::Effect, 0));
    }
    if !source.contracts.assertions.is_empty() {
        budget.charge_work(1)?;
        return Err(cr_policy_unsupported_v1(Need::Assertion, 0));
    }
    budget.charge_work(2)?;
    if !source.contracts.catalog.definitions().is_empty()
        || !source.contracts.catalog.bindings().is_empty()
    {
        return Err(cr_policy_unsupported_v1(Need::Catalog, 0));
    }
    // Inspect the entire retained source table, not only physical ABI slots.
    // Aggregate envelopes are structural; every referenced leaf is also checked.
    for (ordinal, ty) in source
        .owner
        .semantic_ssa
        .source_semantic()
        .types()
        .iter()
        .enumerate()
    {
        budget.charge_work(1)?;
        match ty.shape() {
            Shape::Unit
            | Shape::Never
            | Shape::Scalar(_)
            | Shape::ValidityScalar(_)
            | Shape::Tuple(_)
            | Shape::Aggregate(_)
            | Shape::Array { .. } => {}
            Shape::Pointer(_)
            | Shape::Slice { .. }
            | Shape::Union(_)
            | Shape::Enum { .. }
            | Shape::FunctionPointer { .. }
            | Shape::Opaque => return Err(cr_policy_unsupported_v1(Need::Type, ordinal)),
        }
    }
    for (ordinal, argument) in source.arguments.rows.iter().enumerate() {
        budget.charge_work(1)?;
        match argument.ownership {
            SemanticSourceArgumentOwnershipV1::ByValue => {}
            SemanticSourceArgumentOwnershipV1::Unspecified
            | SemanticSourceArgumentOwnershipV1::SharedBorrow
            | SemanticSourceArgumentOwnershipV1::UniqueBorrow
            | SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            | SemanticSourceArgumentOwnershipV1::RawPointer => {
                return Err(cr_policy_unsupported_v1(Need::ArgumentOwnership, ordinal));
            }
        }
    }
    for (ordinal, frame) in source.arguments.frames.iter().enumerate() {
        budget.charge_work(1)?;
        if frame.is_some() {
            return Err(cr_policy_unsupported_v1(Need::LocalMemory, ordinal));
        }
    }
    for (ordinal, callable) in source
        .owner
        .semantic_ssa
        .source_semantic()
        .callables()
        .iter()
        .enumerate()
    {
        budget.charge_work(1)?;
        match callable {
            SemanticCallableDeclV1::Defined { .. } => {}
            SemanticCallableDeclV1::DeviceFfiImport { .. }
            | SemanticCallableDeclV1::CompilerIntrinsic { .. } => {
                return Err(cr_policy_unsupported_v1(Need::Callable, ordinal));
            }
        }
    }
    for (ordinal, row) in source.owner.correspondence.call_returns.iter().enumerate() {
        budget.charge_work(1)?;
        match row.kind {
            SemanticKirCallReturnKindV1::Return { components } => {
                let range = components
                    .range()
                    .map_err(ProductionCanonicalRankedSourceErrorV1::from)?;
                for component in &source.owner.correspondence.call_result_components[range] {
                    budget.charge_work(1)?;
                    if !matches!(component, CallResultComponentV1::Return { .. }) {
                        return Err(cr_policy_unsupported_v1(Need::CallTransport, ordinal));
                    }
                }
            }
            SemanticKirCallReturnKindV1::Call { .. } => {
                return Err(cr_policy_unsupported_v1(Need::CallTransport, ordinal));
            }
        }
    }
    if !source
        .owner
        .correspondence
        .generated_terminator_values
        .is_empty()
    {
        budget.charge_work(1)?;
        return Err(cr_policy_unsupported_v1(Need::CallTransport, 0));
    }
    for (ordinal, span) in source.source.spans.iter().enumerate() {
        budget.charge_work(1)?;
        match span.site {
            ProductionCanonicalRankedSourceSiteV1::Statement { source, .. } => {
                match source.kind() {
                    Statement::Assign(assignment) => match assignment.value().kind() {
                        Rvalue::Use(_)
                        | Rvalue::Unary { .. }
                        | Rvalue::Binary { .. }
                        | Rvalue::CheckedBinary(_)
                        | Rvalue::Cast { .. }
                        | Rvalue::Aggregate(_) => {}
                        Rvalue::UncheckedBinary(_)
                        | Rvalue::Borrow { .. }
                        | Rvalue::AddressOf { .. }
                        | Rvalue::Length(_)
                        | Rvalue::Discriminant(_)
                        | Rvalue::Load(_) => {
                            return Err(cr_policy_unsupported_v1(Need::Rvalue, ordinal));
                        }
                    },
                    Statement::Nop | Statement::StorageLive(_) | Statement::StorageDead(_) => {}
                    Statement::Store(_)
                    | Statement::AtomicRmw(_)
                    | Statement::AtomicCompareExchange(_)
                    | Statement::SetDiscriminant { .. }
                    | Statement::Deinitialize(_)
                    | Statement::Assume(_) => {
                        return Err(cr_policy_unsupported_v1(Need::Statement, ordinal));
                    }
                }
            }
            ProductionCanonicalRankedSourceSiteV1::Terminator { source, .. } => match source.kind()
            {
                Terminator::Goto(_) | Terminator::SwitchInt { .. } | Terminator::Return => {}
                Terminator::Call(_)
                | Terminator::TailCall(_)
                | Terminator::Drop { .. }
                | Terminator::Assert { .. }
                | Terminator::FalseEdge { .. }
                | Terminator::UnwindResume
                | Terminator::UnwindTerminate
                | Terminator::Abort
                | Terminator::Unreachable => {
                    return Err(cr_policy_unsupported_v1(Need::Terminator, ordinal));
                }
            },
            ProductionCanonicalRankedSourceSiteV1::Synthetic(_) => {
                return Err(cr_policy_unsupported_v1(Need::Synthetic, ordinal));
            }
        }
    }
    // Launch rows are genuine early layout facts, NOT exact target binding.
    // Their source/graph join is already checked by V841; all Launch/Target
    // obligations remain pending alongside the full remaining roster.
    budget.charge_work(source.contracts.launches.len())?;
    Ok(())
}

/// Genuine source facade coupled to reports over exactly its authoritative N.
/// The source remains immutable and all full-compiler obligations stay pending.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalRankedSourcePoliciesV1;
/// fn copy(x: &ProductionCanonicalRankedSourcePoliciesV1<'_, '_, '_>) { let _ = (*x).clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalRankedSourcePoliciesV1;
/// fn promote(x: &ProductionCanonicalRankedSourcePoliciesV1<'_, '_, '_>) { let _ = x.into_verified_ranked(); }
/// ```
pub struct ProductionCanonicalRankedSourcePoliciesV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    policies: &'s fe2o3_pliron::CheckedCanonicalRankedPoliciesV1<'s, 'g>,
}
impl ProductionCanonicalRankedSourcePoliciesV1<'_, '_, '_> {
    /// The same paid source facts, including facts not discharged by these reports.
    pub fn metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.policies.function_count(budget)?;
        self.source.guard.query(budget)?;
        Ok(self.source)
    }
    /// The actual fixed nine-stage reports, with no mutable native graph.
    pub fn policies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<&fe2o3_pliron::CheckedCanonicalRankedPoliciesV1<'_, '_>> {
        self.source.guard.query(budget)?;
        self.policies.function_count(budget)?;
        Ok(self.policies)
    }
    /// This staged policy result is not full ranked verification.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// Source coupling grants no launch, publication or external proof authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
impl ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_> {
    /// Check the complete typed source profile and run the real fixed pipeline on
    /// its one graph. Unsupported requirements reject; no empty-report shortcut.
    /// Future optimized-neutral integration must transport this source custody
    /// before calling the same graph consumer, not reuse this original-N result.
    pub fn with_policy_checks_v1<'w, T>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl for<'s, 'm, 'g> FnOnce(
            &ProductionCanonicalRankedSourcePoliciesV1<'s, 'm, 'g>,
            &mut ArgumentBudgetV1<'w>,
        ) -> CrPolicyResultV1<T>,
    ) -> CrPolicyResultV1<T> {
        self.source.guard.query(budget)?;
        let actual = self
            .checked
            .inventory(budget)
            .map_err(ProductionCanonicalRankedSourceErrorV1::from)?
            .owner();
        if !std::ptr::eq(actual, self.source.owner.executable()) {
            return Err(cr_policy_unsupported_v1(
                ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                0,
            ));
        }
        cr_policy_source_profile_v1(self.source, budget)?;
        let source = self.source;
        cr_protected_v1(budget, |budget| {
            Ok(fe2o3_pliron::with_canonical_ranked_policy_checks_v1(
                self.checked,
                budget,
                |policies, budget| {
                    use fe2o3_pliron::CanonicalRankedPolicyFailureV1 as Failure;
                    use std::panic::{AssertUnwindSafe, catch_unwind};

                    // The structural view has finished its exact-floor queries.
                    // Only the report callback may now acquire this scratch.
                    let slot = std::ptr::from_ref(&*budget) as usize;
                    let ledger = budget.work_ledger_identity_v1();
                    let headers = argument_sum_v1(&[
                        std::mem::size_of::<ProductionCanonicalRankedSourcePoliciesV1<'_, '_, '_>>(
                        ),
                        std::mem::size_of::<std::thread::Result<CrPolicyResultV1<T>>>(),
                        std::mem::size_of::<CrPolicyResultV1<T>>(),
                    ])?;
                    budget.reserve_storage(headers)?;
                    let paid = budget.storage();
                    let returned = catch_unwind(AssertUnwindSafe(|| {
                        let view = ProductionCanonicalRankedSourcePoliciesV1 { source, policies };
                        callback(&view, budget)
                    }));
                    let same = |budget: &ArgumentBudgetV1<'_>| {
                        slot == std::ptr::from_ref(budget) as usize
                            && ledger == budget.work_ledger_identity_v1()
                    };
                    let accounting = same(budget) && budget.storage() == paid;
                    let source_check = source.guard.check(budget);
                    let failure = if !accounting {
                        Some(Failure::Resource(ArgumentResourceV1::Accounting))
                    } else {
                        policies.function_count(budget).err()
                    };
                    let result = if let Some(error) = failure {
                        let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
                        drop(rejected);
                        Err(error)
                    } else if let Err(error) = source_check {
                        let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
                        drop(rejected);
                        Ok(Err(ProductionCanonicalRankedPolicyErrorV1::Source(error)))
                    } else {
                        match returned {
                            Ok(value) => Ok(value),
                            Err(payload) => {
                                drop(payload);
                                Err(Failure::Panicked)
                            }
                        }
                    };
                    // Rejected values and panic payloads die while paid. Never
                    // refund a replacement ledger or restore missing credits;
                    // leave excess scratch for the enclosing exact-floor guard.
                    if same(budget) && budget.storage() >= paid {
                        budget.release_storage(headers)?;
                    }
                    result
                },
            )
            .map_err(ProductionCanonicalRankedPolicyErrorV1::Policy)
            .and_then(|result| result))
        })?
    }
}
