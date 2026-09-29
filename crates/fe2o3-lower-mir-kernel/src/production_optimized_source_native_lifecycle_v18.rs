//! Original lifecycle recipes joined to actual, still non-authoritative Pliron observations.
use super::*;
use fe2o3_pliron::{
    CanonicalRankedPolicyChecksErrorV1, CanonicalRankedPolicyFailureV1,
    CanonicalRankedSourceObligationV18, CanonicalRankedSourceRequirementV18,
    PendingCanonicalRankedPoliciesV18, PendingCanonicalRankedSourceRolesV18,
    with_pending_canonical_ranked_source_roles_v18,
};

/// Copy-only history from an actual native attempt. The resource units remain
/// native analysis units, not KIR bytes or a completion capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceNativeLifecycleDiagnosticV18 {
    observation: fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1,
    last_invocation: Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
}

impl ProductionSourceNativeLifecycleDiagnosticV18 {
    /// Actual cumulative accepted native resource history.
    pub const fn observation(self) -> fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1 {
        self.observation
    }
    /// Last accepted native invocation, absent if none was accepted.
    pub const fn last_invocation(self) -> Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1> {
        self.last_invocation
    }
    fn from_error(error: &CanonicalRankedPolicyChecksErrorV1) -> Self {
        Self {
            observation: error.observation(),
            last_invocation: error.last_invocation(),
        }
    }
}

/// A refusal preserves whether native stages were reached. Pending and
/// Unresolved do not carry invented invocation histories.
#[derive(Debug)]
pub enum ProductionSourceNativeLifecycleErrorV18 {
    /// The original source owner or continuing source ledger refused the scope.
    Source(ProductionSourceOwnedViewErrorV18),
    /// The source first refusal remains primary after a real native attempt.
    SourceAfterNative {
        /// The original source guard's selected first refusal.
        source: ProductionSourceOwnedViewErrorV18,
        /// Actual accepted native history, including an empty accepted history.
        diagnostic: ProductionSourceNativeLifecycleDiagnosticV18,
    },
    /// The non-authoritative graph/census scope failed without a native history.
    Pending(CanonicalRankedPolicyFailureV1),
    /// The pending graph's primary refusal after a real native attempt.
    PendingAfterNative {
        /// Selected pending owner, epoch, or query refusal.
        failure: CanonicalRankedPolicyFailureV1,
        /// Actual accepted native history, not a synthesized native error.
        diagnostic: ProductionSourceNativeLifecycleDiagnosticV18,
    },
    /// An actual native invocation failed, retaining its accepted resource history.
    Native(CanonicalRankedPolicyChecksErrorV1),
    /// A complete census still contains a role not discharged by lifecycle recipes.
    Unresolved(CanonicalRankedSourceObligationV18),
}

impl ProductionSourceNativeLifecycleErrorV18 {
    /// Actual native-attempt diagnostics. None means no attempt was observed.
    pub fn native_diagnostic(&self) -> Option<ProductionSourceNativeLifecycleDiagnosticV18> {
        match self {
            Self::SourceAfterNative { diagnostic, .. }
            | Self::PendingAfterNative { diagnostic, .. } => Some(*diagnostic),
            Self::Native(error) => Some(ProductionSourceNativeLifecycleDiagnosticV18::from_error(
                error,
            )),
            _ => None,
        }
    }
    fn with_diagnostic(
        self,
        diagnostic: Option<ProductionSourceNativeLifecycleDiagnosticV18>,
    ) -> Self {
        match (self, diagnostic) {
            (Self::Source(source), Some(diagnostic)) => {
                Self::SourceAfterNative { source, diagnostic }
            }
            (Self::Pending(failure), Some(diagnostic)) => Self::PendingAfterNative {
                failure,
                diagnostic,
            },
            (error, _) => error,
        }
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for ProductionSourceNativeLifecycleErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

impl std::fmt::Display for ProductionSourceNativeLifecycleErrorV18 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source native lifecycle: {self:?}")
    }
}

impl std::error::Error for ProductionSourceNativeLifecycleErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::SourceAfterNative { source, .. } => Some(source),
            Self::Pending(error) => Some(error),
            Self::PendingAfterNative { failure, .. } => Some(failure),
            Self::Native(error) => Some(error),
            Self::Unresolved(_) => None,
        }
    }
}

type NativeResult = Result<(), ProductionSourceNativeLifecycleErrorV18>;
type NativeError = ProductionSourceNativeLifecycleErrorV18;
type NativeDiagnostic = ProductionSourceNativeLifecycleDiagnosticV18;
type DiagnosticCell = std::cell::Cell<Option<NativeDiagnostic>>;

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    #[cfg(test)]
    pub(crate) fn test_native_lifecycle_header_v18<'work>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        limit: usize,
        short: bool,
        attempt_only: bool,
        verified: &std::cell::Cell<bool>,
    ) -> NativeResult {
        self.query(budget)?;
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            layouts,
            budget,
            |pending, budget| {
                Ok(
                    self.with_execution_recipes_prepaid_v18(budget, |recipes, budget| {
                        controls::native_header(
                            recipes,
                            pending,
                            budget,
                            limit,
                            short,
                            attempt_only,
                            verified,
                        )
                    }),
                )
            },
        )
        .map_err(|error| self.native_pending_error(error))?
    }
    #[cfg(test)]
    pub(crate) fn test_native_lifecycle_join_v18<'work>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        fault: u8,
        verified: &std::cell::Cell<bool>,
    ) -> NativeResult {
        self.query(budget)?;
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            layouts,
            budget,
            |pending, budget| {
                Ok(
                    self.with_execution_recipes_prepaid_v18(budget, |recipes, budget| {
                        controls::same_candidate(recipes, pending, budget, fault, verified)
                    }),
                )
            },
        )
        .map_err(|error| self.native_pending_error(error))?
    }
    /// Holds the complete original lifecycle relation through the two native
    /// phases. Returned diagnostic/result envelopes must be prepaid outside
    /// the enclosing source-owned callback, including one
    /// `Cell<Option<ProductionSourceNativeLifecycleDiagnosticV18>>`. That copy
    /// survives discarding scopes without exporting report backing. No output backing can escape this
    /// Unit-only consumer and no default compilation route is selected here.
    pub fn with_lifecycle_native_policies_v18<'work>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope, 'owner> FnOnce(
            &ProductionLifecycleCheckedNativePoliciesV18<'scope, 'owner>,
            &mut ArgumentBudgetV1<'work>,
        ) -> NativeResult,
    ) -> NativeResult {
        self.query(budget)?;
        let inventory = checked
            .inventory(budget)
            .map_err(|error| self.native_pending_error(error.into()))?;
        if !std::ptr::eq(inventory, self.checked.output()) {
            return Err(NativeError::Source(
                self.original
                    .source
                    .missing::<()>("native lifecycle substituted its checked inventory")
                    .unwrap_err(),
            ));
        }
        // Authenticate the exact-floor ranked view before adding any source
        // recipe/index credits. Both are nested inside the retained graph scope.
        let diagnostic = DiagnosticCell::new(None);
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            layouts,
            budget,
            |pending, budget| {
                Ok(
                    self.with_execution_recipes_prepaid_v18(budget, |recipes, budget| {
                        recipes.with_pending_native_policies_v18(
                            pending,
                            budget,
                            &diagnostic,
                            consume,
                        )
                    }),
                )
            },
        )
        .map_err(|error| self.native_pending_error(error))
        .and_then(|result| result);
        result.map_err(|error| error.with_diagnostic(diagnostic.get()))
    }

    fn native_pending_error(&self, error: CanonicalRankedPolicyFailureV1) -> NativeError {
        match ProductionOptimizedExecutionRecipesV18::policy_resource(&error) {
            Some(resource) => NativeError::Source(
                self.original
                    .source
                    .retain_query::<()>(Err(resource.into()))
                    .unwrap_err(),
            ),
            None => NativeError::Pending(error),
        }
    }
}

/// Actual native reports consumed under a live original lifecycle recipe.
/// All structural source-role obligations in this graph have been joined to
/// this closed lifecycle subset. This does not prove memory currentness,
/// final ranked verification, formal refinement or target/launch readiness.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionLifecycleCheckedNativePoliciesV18;
/// fn forge() -> ProductionLifecycleCheckedNativePoliciesV18<'static, 'static> {
///     ProductionLifecycleCheckedNativePoliciesV18 { recipes: &[] }
/// }
/// ```
pub struct ProductionLifecycleCheckedNativePoliciesV18<'scope, 'owner> {
    recipes: &'scope ProductionOptimizedExecutionRecipesV18<'scope>,
    native: &'scope PendingCanonicalRankedPoliciesV18<'scope, 'owner>,
}

impl ProductionLifecycleCheckedNativePoliciesV18<'_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> NativeResult {
        self.recipes.check(budget)?;
        let owner = self
            .native
            .owner(budget)
            .map_err(|error| self.recipes.pending_error(error))?;
        self.recipes.check_owner(owner, budget)?;
        Ok(())
    }

    /// Authenticates the exact source/output relation behind this lexical view.
    /// This permits a containing source consumer to attach its original root
    /// roster; it does not complete ranked, formal, target or launch checks.
    pub fn check_source_subject_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        self.recipes
            .optimized
            .retain(budget.charge_work(2).map_err(Into::into))?;
        if !std::ptr::eq(self.recipes.optimized.original, original) {
            return Err(self
                .recipes
                .source_failure("native root attachment substituted its original source"));
        }
        if !std::ptr::eq(self.recipes.optimized, optimized) {
            return Err(self
                .recipes
                .source_failure("native root attachment substituted its optimized subject"));
        }
        optimized.query(budget)?;
        Ok(())
    }

    /// Retains a containing source consumer's failed binding query in this
    /// exact source owner's first-error state. This can only deny admission.
    pub fn retain_source_binding_error_v18(&self, detail: &'static str) -> NativeError {
        self.recipes.source_failure(detail)
    }

    /// Observes a containing root attachment's still-live retained storage.
    /// A larger required floor can only deny cleanup; it grants no authority.
    /// Custody is observed even when a previous query has already failed.
    pub fn check_retained_root_storage_v18(
        &self,
        required: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        let original = self.recipes.optimized.original;
        if original.observe_custody(budget).is_err() || budget.storage() < required {
            original.source.cleanup.deny_refund();
            return Err(NativeError::Source(
                original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting),
            ));
        }
        self.recipes
            .optimized
            .retain(self.recipes.observe_custody(budget))?;
        Ok(())
    }

    /// Number of exact output declarations and definitions, in original order.
    pub fn function_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<usize, NativeError> {
        self.check(budget)?;
        self.native
            .function_count(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }

    /// Actual fixed-policy report; declarations have no report.
    pub fn report(
        &self,
        function: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&fe2o3_pliron::ProductionPlironPreloweringReportV2>, NativeError> {
        self.check(budget)?;
        self.native
            .report(function, budget)
            .map_err(|error| self.recipes.pending_error(error))
    }

    /// Accepted native invocation history; declarations have no invocation.
    pub fn history(
        &self,
        function: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>, NativeError> {
        self.check(budget)?;
        self.native
            .history(function, budget)
            .map_err(|error| self.recipes.pending_error(error))
    }

    /// Cumulative native analysis-domain observation, not a KIR byte receipt.
    pub fn observation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1, NativeError> {
        self.check(budget)?;
        self.native
            .observation(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }

    /// Copy-only accepted history of this real native attempt. This grants no
    /// source, ranked, publication, or launch authority.
    pub fn diagnostic(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<NativeDiagnostic, NativeError> {
        self.check(budget)?;
        let observation = self
            .native
            .observation(budget)
            .map_err(|error| self.recipes.pending_error(error))?;
        let last_invocation = self
            .native
            .last_invocation(budget)
            .map_err(|error| self.recipes.pending_error(error))?;
        Ok(NativeDiagnostic {
            observation,
            last_invocation,
        })
    }

    /// Every structural role in this scope matched an original lifecycle recipe.
    pub const fn lifecycle_source_roles_are_complete(&self) -> bool {
        true
    }
    /// Further ranked and source obligations remain outside this narrow scope.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// This lexical view grants no publication or device-launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct OutputRecipe {
    coordinate: OpCoordinate,
    kind: ProductionOptimizedExecutionKindV18,
}

impl ProductionOptimizedExecutionRecipesV18<'_> {
    fn source_failure(&self, detail: &'static str) -> NativeError {
        NativeError::Source(
            self.optimized
                .original
                .source
                .missing::<()>(detail)
                .unwrap_err(),
        )
    }

    pub(in super::super::super) fn policy_resource(
        error: &CanonicalRankedPolicyFailureV1,
    ) -> Option<ArgumentResourceV1> {
        use CanonicalRankedPolicyFailureV1 as Failure;
        match error {
            Failure::Resource(error)
            | Failure::View(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(error)) => {
                Some(*error)
            }
            Failure::StorageBridge(fe2o3_pliron::KirBridgeErrorV18::Canonical(
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
            )) => Some(*error),
            _ => None,
        }
    }

    fn pending_error(&self, error: CanonicalRankedPolicyFailureV1) -> NativeError {
        self.optimized.native_pending_error(error)
    }

    fn native_error(&self, error: CanonicalRankedPolicyChecksErrorV1) -> NativeError {
        match Self::policy_resource(error.failure()) {
            Some(resource) => NativeError::SourceAfterNative {
                source: self
                    .optimized
                    .original
                    .source
                    .retain_query::<()>(Err(resource.into()))
                    .unwrap_err(),
                diagnostic: NativeDiagnostic::from_error(&error),
            },
            None => NativeError::Native(error),
        }
    }

    fn check_owner(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        self.optimized
            .retain(budget.charge_work(1).map_err(Into::into))?;
        if !std::ptr::eq(owner, self.optimized.checked.output().owner()) {
            return Err(
                self.source_failure("native lifecycle substituted its checked output owner")
            );
        }
        Ok(())
    }

    fn output_recipes(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Vec<OutputRecipe>> {
        budget.charge_work(self.rows.len())?;
        let count = self.rows.iter().filter(|row| row.output.is_some()).count();
        let mut rows = resources::vector(count, budget)?;
        for row in self.rows {
            budget.charge_work(1)?;
            if let Some(coordinate) = row.output {
                if rows.len() >= count || rows.len() == rows.capacity() {
                    return resources::binding("native lifecycle output capacity");
                }
                rows.push(OutputRecipe {
                    coordinate,
                    kind: row.kind,
                });
            }
        }
        private_array_heapsort_v1(
            &mut rows,
            |row| operation_key(row.coordinate),
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        let mut previous = None;
        for row in &rows {
            budget.charge_work(1)?;
            let key = operation_key(row.coordinate);
            if previous.is_some_and(|old| old >= key) {
                return resources::binding("native lifecycle duplicate output recipe");
            }
            previous = Some(key);
        }
        if rows.len() != count {
            return resources::binding("native lifecycle output count");
        }
        Ok(rows)
    }

    fn join_pending(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        obligations: &[CanonicalRankedSourceObligationV18],
        rows: &[OutputRecipe],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check_owner(owner, budget)?;
        let mut next = 0usize;
        let mut previous = None;
        let mut unresolved = None;
        for obligation in obligations {
            self.optimized
                .retain(budget.charge_work(1).map_err(Into::into))?;
            let key = operation_key(obligation.coordinate());
            if previous.is_some_and(|old| old >= key) {
                return Err(
                    self.source_failure("native lifecycle repeated or unordered obligation")
                );
            }
            previous = Some(key);
            let expected = rows.get(next);
            if expected.is_some_and(|row| operation_key(row.coordinate) < key) {
                return Err(self.source_failure("native lifecycle missing output obligation"));
            }
            if let Some(row) = expected.filter(|row| row.coordinate == obligation.coordinate()) {
                if obligation.requirement() != CanonicalRankedSourceRequirementV18::Execution {
                    return Err(self.source_failure("native lifecycle obligation changed role"));
                }
                let output = self.optimized.checked.output();
                let index = self.optimized.retain(resources::operation_index(
                    output,
                    row.coordinate,
                    budget,
                ))?;
                let actual = match &output.operations()[index].operation.kind {
                    OperationKind::Execution(Execution::ContextIssue) => {
                        Some(ProductionOptimizedExecutionKindV18::ContextIssue)
                    }
                    OperationKind::Execution(Execution::WorkgroupDerive { .. }) => {
                        Some(ProductionOptimizedExecutionKindV18::WorkgroupDerive)
                    }
                    OperationKind::Execution(Execution::ScopeEnd { discarded, .. })
                        if discarded.is_empty() =>
                    {
                        Some(ProductionOptimizedExecutionKindV18::ScopeEnd)
                    }
                    _ => None,
                };
                if actual != Some(row.kind) {
                    return Err(
                        self.source_failure("native lifecycle output operation changed kind")
                    );
                }
                next = next
                    .checked_add(1)
                    .ok_or_else(|| self.pending_error(ArgumentResourceV1::Arithmetic.into()))?;
            } else {
                if obligation.requirement() == CanonicalRankedSourceRequirementV18::Execution {
                    let output = self.optimized.checked.output();
                    let index = self.optimized.retain(resources::operation_index(
                        output,
                        obligation.coordinate(),
                        budget,
                    ))?;
                    if matches!(
                        &output.operations()[index].operation.kind,
                        OperationKind::Execution(
                            Execution::ContextIssue
                                | Execution::WorkgroupDerive { .. }
                                | Execution::ScopeEnd { .. }
                        )
                    ) {
                        return Err(self.source_failure("native lifecycle missing output recipe"));
                    }
                }
                if unresolved.is_none() {
                    unresolved = Some(*obligation);
                }
            }
        }
        self.optimized
            .retain(budget.charge_work(1).map_err(Into::into))?;
        if next != rows.len() {
            return Err(self.source_failure("native lifecycle missing output obligation"));
        }
        self.check(budget)?;
        match unresolved {
            Some(obligation) => Err(NativeError::Unresolved(obligation)),
            None => Ok(()),
        }
    }

    /// Joins the full pending census before running actual native stages on its
    /// retained graph. The callback is Unit-only and must restore its incoming
    /// storage floor. It cannot export either scoped view or new owned backing.
    /// The caller must prepay the fixed returned diagnostic/result envelopes
    /// outside this scope, including one
    /// `Cell<Option<ProductionSourceNativeLifecycleDiagnosticV18>>`, following
    /// the native observation API's contract.
    /// This scope releases only its own rows/frames. It does not transfer a
    /// caller-set refund allowance or settle enclosing diagnostic storage.
    pub fn with_native_policies_v18<'work>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope, 'owner> FnOnce(
            &ProductionLifecycleCheckedNativePoliciesV18<'scope, 'owner>,
            &mut ArgumentBudgetV1<'work>,
        ) -> NativeResult,
    ) -> NativeResult {
        self.check(budget)?;
        let inventory = checked
            .inventory(budget)
            .map_err(|error| self.pending_error(error.into()))?;
        if !std::ptr::eq(inventory, self.optimized.checked.output()) {
            return Err(self.source_failure("native lifecycle substituted its checked inventory"));
        }
        let diagnostic = DiagnosticCell::new(None);
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            layouts,
            budget,
            |pending, budget| {
                Ok(self.with_pending_native_policies_v18(pending, budget, &diagnostic, consume))
            },
        )
        .map_err(|error| self.pending_error(error))
        .and_then(|result| result);
        result.map_err(|error| error.with_diagnostic(diagnostic.get()))
    }

    fn with_pending_native_policies_v18<'work, 'owner>(
        &self,
        pending: &mut PendingCanonicalRankedSourceRolesV18<'_, 'owner>,
        budget: &mut ArgumentBudgetV1<'work>,
        diagnostic: &DiagnosticCell,
        consume: impl for<'scope, 'graph> FnOnce(
            &ProductionLifecycleCheckedNativePoliciesV18<'scope, 'graph>,
            &mut ArgumentBudgetV1<'work>,
        ) -> NativeResult,
    ) -> NativeResult {
        self.check(budget)?;
        let source = self.optimized.original.source;
        let floor = budget.storage();
        let rows = source.retain_query(scoped_source_attempt_v29(
            source.cleanup,
            budget,
            floor,
            |budget| {
                source.retain_construction(|| {
                    budget.reserve_storage(argument_sum_v1(&[
                        size_of::<Vec<OutputRecipe>>(),
                        size_of::<ProductionLifecycleCheckedNativePoliciesV18<'_, '_>>(),
                        2 * size_of::<SourceOwnedResultV18<Vec<OutputRecipe>>>(),
                        size_of::<std::thread::Result<NativeResult>>(),
                        2 * size_of::<NativeResult>(),
                        std::mem::size_of_val(&consume),
                        2 * std::mem::align_of_val(&consume),
                    ])?)?;
                    self.output_recipes(budget)
                })
            },
        ))?;
        let storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or_else(|| self.pending_error(ArgumentResourceV1::Accounting.into()))?;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let owner = pending
                .owner(budget)
                .map_err(|error| self.pending_error(error))?;
            let obligations = pending
                .obligations(budget)
                .map_err(|error| self.pending_error(error))?;
            self.join_pending(owner, obligations, &rows, budget)?;
            let observed = pending.with_native_observations(budget, |native, budget| {
                let observation = native.observation(budget)?;
                let last_invocation = native.last_invocation(budget)?;
                diagnostic.set(Some(NativeDiagnostic {
                    observation,
                    last_invocation,
                }));
                let owner = native.owner(budget)?;
                let joined = self.join_pending(owner, native.obligations(budget)?, &rows, budget);
                if let Err(error) = joined {
                    return Ok(Err(error));
                }
                let view = ProductionLifecycleCheckedNativePoliciesV18 {
                    recipes: self,
                    native,
                };
                Ok(consume(&view, budget))
            });
            if let Err(error) = &observed {
                diagnostic.set(Some(NativeDiagnostic::from_error(error)));
            }
            observed.map_err(|error| self.native_error(error))?
        }));
        let prior = source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(()))) {
            self.check(budget)
        } else {
            self.observe_custody(budget)
        };
        drop(rows);
        source_owned_finish_callback_v18(caught, prior, postflight, source.cleanup, budget, storage)
    }
}

#[cfg(test)]
#[path = "production_optimized_source_native_lifecycle_controls_v18_tests.rs"]
mod controls;

include!("production_optimized_source_native_private_v18.rs");
