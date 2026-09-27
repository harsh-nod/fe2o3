// Complete original private-memory and lifecycle joins on an actual native scope.
use scoped_raw_admission_v29::{CheckedOptimizedSourceMemoryV18, CheckedSourcePrivatePhysicalV18};

enum PrivateNativeFlowV18<E> {
    Callback(E),
    Native(NativeError),
}
impl<E> From<ProductionSourceOwnedViewErrorV18> for PrivateNativeFlowV18<E> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Native(error.into())
    }
}
impl<E> From<ArgumentResourceV1> for PrivateNativeFlowV18<E> {
    fn from(error: ArgumentResourceV1) -> Self {
        ProductionSourceOwnedViewErrorV18::from(error).into()
    }
}
impl<E> From<NativeError> for PrivateNativeFlowV18<E> {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}

/// A single authentic source root requiring independently checked entry RHSs.
/// The caller cannot choose its root, physical/currentness owner or completion.
pub struct ProductionSourcePrivateMemoryRootRequestV18<'scope> {
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    physical: &'scope CheckedSourcePrivatePhysicalV18<'scope>,
    currentness: &'scope CheckedOptimizedSourceMemoryV18<'scope>,
    root: usize,
    coverage: &'scope std::cell::RefCell<Vec<bool>>,
    completed: std::cell::Cell<bool>,
    required: usize,
}
impl ProductionSourcePrivateMemoryRootRequestV18<'_> {
    #[cfg(test)]
    pub(crate) fn test_foreign_root_v18(
        &self,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        source_scalar_normalization_scratch_v18(
            self.original.source.cleanup,
            budget,
            argument_sum_v1(&[size_of::<Self>(), size_of::<SourceOwnedResultV18<()>>()])?,
            |budget| {
                self.check(budget)?;
                let foreign = Self {
                    original: self.original,
                    optimized: self.optimized,
                    physical: self.physical,
                    currentness: self.currentness,
                    root: usize::MAX,
                    coverage: self.coverage,
                    completed: std::cell::Cell::new(false),
                    required: self.required,
                };
                foreign.check_entry_writes(entries, budget)
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn test_remove_completed_operation_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let headers = argument_sum_v1(&[
            size_of::<std::cell::RefMut<'_, Vec<bool>>>(),
            size_of::<std::slice::IterMut<'_, bool>>(),
            size_of::<Option<&mut bool>>(),
            size_of::<&mut bool>(),
            size_of::<SourceOwnedResultV18<()>>(),
        ])?;
        source_scalar_normalization_scratch_v18(
            self.original.source.cleanup,
            budget,
            headers,
            |budget| {
                self.check(budget)?;
                assert!(
                    self.completed.get(),
                    "the authentic root must complete before mutation"
                );
                let mut rows = self.coverage.borrow_mut();
                let mut changed = false;
                for row in rows.iter_mut() {
                    budget.charge_work(1)?;
                    if *row {
                        *row = false;
                        changed = true;
                        break;
                    }
                }
                assert!(changed, "genuine completed memory row");
                Ok(())
            },
        )
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            self.physical
                .check_native_source_subject_v18(self.original, self.optimized, budget)?;
            self.currentness
                .check_scope_v18(self.original, self.optimized, self.root, budget)?;
            if budget.storage() < self.required {
                self.original.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(())
        })())
    }
    /// The exact original root selected by this constructor.
    pub fn root(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.root)
    }
    /// Complete this request once, using the exact lexical checked-entry owner.
    pub fn check_entry_writes(
        &self,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(1)?;
            if self.completed.get() {
                return self
                    .original
                    .source
                    .missing("private native root completed twice");
            }
            entries.check_for(self.original, self.optimized, self.root, budget)?;
            let mut coverage = self.coverage.try_borrow_mut().map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding(
                    "private native recursive root completion",
                )
            })?;
            self.physical.with_root_memory_v18(
                self.root,
                self.currentness,
                entries,
                budget,
                |memory, budget| {
                    memory.mark_native_source_operations_v18(
                        self.original,
                        self.optimized,
                        self.root,
                        &mut coverage,
                        budget,
                    )
                },
            )?;
            drop(coverage);
            self.check(budget)?;
            self.completed.set(true);
            Ok(())
        })())
    }
}

struct PrivateSourceCompletionV18<'a> {
    original: &'a ProductionSourceCorrespondenceV18<'a>,
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    physical: &'a CheckedSourcePrivatePhysicalV18<'a>,
    coverage: &'a [bool],
    roots: usize,
    memory: usize,
    aliases: usize,
    required: usize,
}
impl PrivateSourceCompletionV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.physical.observe_native_source_custody_v18(budget)?;
        if budget.storage() < self.required {
            self.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            self.observe_custody(budget)?;
            self.physical
                .check_native_source_subject_v18(self.original, self.optimized, budget)?;
            budget.charge_work(2)?;
            if self.original.source.root_count(budget)? != self.roots
                || self.coverage.len() != self.optimized.checked.output().operations().len()
            {
                return self
                    .original
                    .source
                    .missing("private native completed census changed owner");
            }
            Ok(())
        })())
    }
    fn join_pending(
        &self,
        recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
        pending: &PendingCanonicalRankedSourceRolesV18<'_, '_>,
        execution: &[CanonicalRankedSourceObligationV18],
        rows: &[OutputRecipe],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        let owner = pending
            .owner(budget)
            .map_err(|error| recipes.pending_error(error))?;
        recipes.check_owner(owner, budget)?;
        let proof = self.physical.native_physical_v18(budget)?;
        let mut previous = None;
        let mut memory = 0usize;
        let mut aliases = 0usize;
        let mut execution_count = 0usize;
        for obligation in pending
            .obligations(budget)
            .map_err(|error| recipes.pending_error(error))?
        {
            self.optimized
                .retain(budget.charge_work(3).map_err(Into::into))?;
            let key = operation_key(obligation.coordinate());
            if previous.is_some_and(|old| old >= key) {
                return Err(
                    recipes.source_failure("private native unordered or repeated obligation")
                );
            }
            previous = Some(key);
            match obligation.requirement() {
                CanonicalRankedSourceRequirementV18::Memory => {
                    let index = self.optimized.retain(resources::operation_index(
                        self.optimized.checked.output(),
                        obligation.coordinate(),
                        budget,
                    ))?;
                    self.optimized
                        .retain(budget.charge_work(12).map_err(Into::into))?;
                    if self.coverage.get(index) == Some(&true) {
                        memory = memory
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)
                            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                    } else if proof.access_restriction(index).is_some_and(|address| {
                        self.coverage.get(address.allocation()) == Some(&true)
                    }) {
                        aliases = aliases
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)
                            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                    } else {
                        return Err(NativeError::Unresolved(*obligation));
                    }
                }
                CanonicalRankedSourceRequirementV18::Execution => {
                    if execution.get(execution_count) != Some(obligation) {
                        return Err(
                            recipes.source_failure("private native execution census changed")
                        );
                    }
                    execution_count += 1;
                }
                _ => return Err(NativeError::Unresolved(*obligation)),
            }
        }
        if memory != self.memory || aliases != self.aliases || execution_count != execution.len() {
            return Err(recipes.source_failure("private native complete obligation census differs"));
        }
        // Only the complete, ordered Execution sub-roster reaches the unchanged
        // lifecycle join. Memory and pure aliases have separate exact censuses.
        recipes.join_pending(owner, execution, rows, budget)
    }
}

/// Actual native reports backed by complete original lifecycle/private-memory
/// evidence. No V12 profile, ranked/formal proof or target authority is produced.
pub struct ProductionPrivateMemoryCheckedNativePoliciesV18<'scope, 'owner> {
    recipes: &'scope ProductionOptimizedExecutionRecipesV18<'scope>,
    completion: &'scope PrivateSourceCompletionV18<'scope>,
    native: &'scope fe2o3_pliron::PendingCanonicalPrivateMemoryPoliciesV18<'scope, 'owner>,
    obligations: &'scope [CanonicalRankedSourceObligationV18],
}
impl ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_> {
    #[cfg(test)]
    pub(crate) fn test_undercut_completion_floor_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        let released = budget
            .storage()
            .checked_sub(self.completion.required)
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(ArgumentResourceV1::Accounting)
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        budget
            .release_storage(released)
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        Ok(())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> NativeResult {
        self.completion.check(budget)?;
        self.recipes.check(budget)?;
        let owner = self
            .native
            .owner(budget)
            .map_err(|error| self.recipes.pending_error(error))?;
        self.recipes.check_owner(owner, budget)?;
        self.completion
            .optimized
            .retain(budget.charge_work(1).map_err(Into::into))?;
        if !std::ptr::eq(
            self.native
                .obligations(budget)
                .map_err(|error| self.recipes.pending_error(error))?,
            self.obligations,
        ) {
            return Err(self
                .recipes
                .source_failure("private native report changed its obligation owner"));
        }
        Ok(())
    }
    /// Rechecks the exact original/optimized owners against the completed proof.
    pub fn check_source_subject_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        self.completion
            .physical
            .check_native_source_subject_v18(original, optimized, budget)?;
        Ok(())
    }
    /// Records a source-binding refusal without replacing an earlier failure.
    pub fn retain_source_binding_error_v18(&self, detail: &'static str) -> NativeError {
        self.recipes.source_failure(detail)
    }
    /// Checks budget custody and the caller's live root floor before refunds.
    pub fn check_retained_root_storage_v18(
        &self,
        required: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        let original = self.completion.original;
        if original.observe_custody(budget).is_err() || budget.storage() < required {
            original.source.cleanup.deny_refund();
            return Err(NativeError::Source(
                original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting),
            ));
        }
        self.completion
            .optimized
            .retain(self.completion.observe_custody(budget))?;
        self.recipes
            .optimized
            .retain(self.recipes.observe_custody(budget))?;
        Ok(())
    }
    /// Returns the number of definitions visited by the native policy pipeline.
    pub fn function_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<usize, NativeError> {
        self.check(budget)?;
        self.native
            .function_count(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }
    /// Borrows the actual native report for a definition, after checking custody.
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
    /// Returns the retained invocation history for a native definition.
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
    /// Returns the native pipeline's measured resource observation.
    pub fn observation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1, NativeError> {
        self.check(budget)?;
        self.native
            .observation(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }
    /// Returns resource observations and the last authentic native invocation.
    pub fn diagnostic(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<NativeDiagnostic, NativeError> {
        self.check(budget)?;
        Ok(NativeDiagnostic {
            observation: self
                .native
                .observation(budget)
                .map_err(|error| self.recipes.pending_error(error))?,
            last_invocation: self
                .native
                .last_invocation(budget)
                .map_err(|error| self.recipes.pending_error(error))?,
        })
    }
    /// Always false: private/lifecycle completion does not prove ranked semantics.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// Always false: these reports grant neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

struct PrivateNativeRowsV18 {
    coverage: std::cell::RefCell<Vec<bool>>,
    obligations: Vec<CanonicalRankedSourceObligationV18>,
    execution: Vec<CanonicalRankedSourceObligationV18>,
    recipes: Vec<OutputRecipe>,
    roots: usize,
}

fn private_native_source_headers_v18<E>(
    check_capture: usize,
    check_alignment: usize,
    consume_capture: usize,
    consume_alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    type Build<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedExecutionRecipesV18<'a>,
        &'a mut PendingCanonicalRankedSourceRolesV18<'a, 'a>,
        usize,
    );
    type Run<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedExecutionRecipesV18<'a>,
        &'a CheckedSourcePrivatePhysicalV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'a, 'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'a, 'a>,
        &'a mut PendingCanonicalRankedSourceRolesV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a DiagnosticCell,
        &'a PrivateNativeRowsV18,
        &'a mut (),
        usize,
    );
    type Root<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a CheckedSourcePrivatePhysicalV18<'a>,
        &'a PrivateNativeRowsV18,
        &'a mut (),
        usize,
        &'a CheckedOptimizedSourceMemoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Native<'a> = (
        &'a ProductionOptimizedExecutionRecipesV18<'a>,
        &'a PrivateSourceCompletionV18<'a>,
        &'a DiagnosticCell,
        &'a PrivateNativeRowsV18,
        &'a fe2o3_pliron::PendingCanonicalPrivateMemoryPoliciesV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    argument_sum_v1(&[
        h::<Build<'_>>()?,
        h::<(Build<'_>, &mut ArgumentBudgetV1<'_>)>()?,
        h::<std::panic::AssertUnwindSafe<(Build<'_>, &mut ArgumentBudgetV1<'_>)>>()?,
        h::<Run<'_>>()?,
        h::<std::panic::AssertUnwindSafe<Run<'_>>>()?,
        h::<Root<'_>>()?,
        h::<std::panic::AssertUnwindSafe<Root<'_>>>()?,
        h::<Native<'_>>()?,
        h::<std::panic::AssertUnwindSafe<Native<'_>>>()?,
        h::<(
            &ProductionSourcePrivateMemoryRootRequestV18<'_>,
            &mut std::cell::RefMut<'_, Vec<bool>>,
            &mut [bool],
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<Result<std::cell::RefMut<'_, Vec<bool>>, std::cell::BorrowMutError>>()?,
        h::<std::thread::Result<SourceOwnedResultV18<PrivateNativeRowsV18>>>()?,
        h::<Result<Result<(), PrivateNativeFlowV18<E>>, CanonicalRankedPolicyFailureV1>>()?,
        h::<Result<Result<(), PrivateNativeFlowV18<E>>, CanonicalRankedPolicyChecksErrorV1>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<Option<usize>>()?,
        h::<&bool>()?,
        h::<Option<&bool>>()?,
        h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()?,
        scoped_raw_admission_v29::native_source_memory_bridge_headers_v18()?,
        check_capture,
        argument_product_v1(2, check_alignment)?,
        argument_product_v1(2, consume_capture)?,
        argument_product_v1(2, consume_alignment)?,
        h::<PrivateNativeRowsV18>()?,
        h::<ProductionSourcePrivateMemoryRootRequestV18<'_>>()?,
        h::<PrivateSourceCompletionV18<'_>>()?,
        h::<ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>>()?,
        h::<Vec<bool>>()?,
        h::<std::cell::RefCell<Vec<bool>>>()?,
        h::<std::cell::Ref<'_, Vec<bool>>>()?,
        h::<std::cell::RefMut<'_, Vec<bool>>>()?,
        h::<std::cell::Cell<bool>>()?,
        h::<Vec<CanonicalRankedSourceObligationV18>>()?,
        h::<Vec<CanonicalRankedSourceObligationV18>>()?,
        h::<Vec<OutputRecipe>>()?,
        h::<Result<(), E>>()?,
        h::<E>()?,
        h::<PrivateNativeFlowV18<E>>()?,
        h::<Result<(), PrivateNativeFlowV18<E>>>()?,
        h::<std::thread::Result<Result<(), PrivateNativeFlowV18<E>>>>()?,
        h::<Result<NativeResult, CanonicalRankedPolicyChecksErrorV1>>()?,
        h::<NativeResult>()?,
        h::<DiagnosticCell>()?,
        h::<&[CanonicalRankedSourceObligationV18]>()?,
        h::<&fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>>()?,
        h::<&fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1>()?,
        h::<(
            &fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
            usize,
        )>()?,
        h::<(
            &PrivateSourceCompletionV18<'_>,
            &fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1,
        )>()?,
        h::<(
            &[bool],
            &fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1,
        )>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&CheckedSourcePrivatePhysicalV18<'_>>()?,
        h::<&CheckedOptimizedSourceMemoryV18<'_>>()?,
        h::<&ProductionCheckedSourceEntryWritesV18<'_>>()?,
        h::<&ProductionOptimizedExecutionRecipesV18<'_>>()?,
        h::<&mut ArgumentBudgetV1<'_>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, bool>>>()?,
        h::<Option<(usize, &bool)>>()?,
        h::<(usize, &bool)>()?,
        h::<std::slice::Iter<'_, CanonicalRankedSourceObligationV18>>()?,
        h::<Option<&CanonicalRankedSourceObligationV18>>()?,
        h::<&CanonicalRankedSourceObligationV18>()?,
        h::<CanonicalRankedSourceObligationV18>()?,
        h::<CanonicalRankedSourceRequirementV18>()?,
        h::<Option<[usize; 3]>>()?,
        h::<[usize; 3]>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<()>()?,
        h::<(
            &ProductionSourcePrivateMemoryRootRequestV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &PrivateNativeRowsV18,
            &CheckedSourcePrivatePhysicalV18<'_>,
            &ProductionOptimizedExecutionRecipesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
    ])
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    /// Completes every exact source root before running native private policies.
    /// A caller root error is returned in the inner Result; source/native
    /// refusal is outer. Both callbacks are unit-only and restore their floor;
    /// the final callback is also checked by the native exact-floor guard.
    /// Returned result/diagnostic envelopes follow the existing native API
    /// contract and are prepaid outside the enclosing source callback.
    pub fn with_private_memory_native_policies_v18<'work, E>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        limits: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        check_root: impl for<'scope> FnMut(
            &ProductionSourcePrivateMemoryRootRequestV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
        consume: impl for<'scope, 'owner> FnOnce(
            &ProductionPrivateMemoryCheckedNativePoliciesV18<'scope, 'owner>,
            &mut ArgumentBudgetV1<'work>,
        ) -> NativeResult,
    ) -> Result<Result<(), E>, NativeError>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.query(budget)?;
        let inventory = checked
            .inventory(budget)
            .map_err(|error| self.native_pending_error(error.into()))?;
        if !std::ptr::eq(inventory, self.checked.output()) {
            return Err(NativeError::Source(
                self.original
                    .source
                    .missing::<()>("private native substituted its checked output inventory")
                    .unwrap_err(),
            ));
        }
        let diagnostic = DiagnosticCell::new(None);
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            layouts,
            budget,
            |pending, budget| {
                Ok(
                    self.with_execution_recipes_prepaid_v18(budget, |recipes, budget| {
                        scoped_raw_admission_v29::with_source_private_physical_v18(
                            self.original,
                            self,
                            limits,
                            budget,
                            |physical, budget| {
                                self.original.with_optimized_analysis_v18(
                                    self,
                                    budget,
                                    |analysis, budget| {
                                        analysis.with_memory_versions(
                                            budget,
                                            |input_memory, output_memory, budget| {
                                                with_private_native_source_scope_v18(
                                                    recipes,
                                                    physical,
                                                    input_memory,
                                                    output_memory,
                                                    pending,
                                                    budget,
                                                    &diagnostic,
                                                    check_root,
                                                    consume,
                                                )
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    }),
                )
            },
        );
        match result {
            Err(error) => Err(self
                .native_pending_error(error)
                .with_diagnostic(diagnostic.get())),
            Ok(Err(PrivateNativeFlowV18::Native(error))) => {
                Err(error.with_diagnostic(diagnostic.get()))
            }
            Ok(Err(PrivateNativeFlowV18::Callback(error))) => Ok(Err(error)),
            Ok(Ok(())) => Ok(Ok(())),
        }
    }
}

fn with_private_native_source_scope_v18<'work, E>(
    recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
    physical: &CheckedSourcePrivatePhysicalV18<'_>,
    input_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    output_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    pending: &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'work>,
    diagnostic: &DiagnosticCell,
    mut check_root: impl for<'scope> FnMut(
        &ProductionSourcePrivateMemoryRootRequestV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), E>,
    consume: impl for<'scope, 'owner> FnOnce(
        &ProductionPrivateMemoryCheckedNativePoliciesV18<'scope, 'owner>,
        &mut ArgumentBudgetV1<'work>,
    ) -> NativeResult,
) -> Result<(), PrivateNativeFlowV18<E>>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    let optimized = recipes.optimized;
    let original = optimized.original;
    let source = original.source;
    recipes.check(budget)?;
    let floor = budget.storage();
    let headers = private_native_source_headers_v18::<E>(
        std::mem::size_of_val(&check_root),
        std::mem::align_of_val(&check_root),
        std::mem::size_of_val(&consume),
        std::mem::align_of_val(&consume),
    )?;
    let rows = scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
        original.retain_query((|| {
            budget.reserve_storage(headers)?;
            physical.check_native_source_subject_v18(original, optimized, budget)?;
            let roots = original.source.root_count(budget)?;
            let count = optimized.output_inventory(budget)?.operations().len();
            let mut coverage = resources::vector(count, budget)?;
            budget.charge_work(count)?;
            coverage.resize(count, false);
            let pending_rows = pending.obligations(budget).map_err(|error| {
                match ProductionOptimizedExecutionRecipesV18::policy_resource(&error) {
                    Some(resource) => ProductionSourceOwnedViewErrorV18::Resource(resource),
                    None => ProductionSourceOwnedViewErrorV18::Binding(
                        "private native pending census query",
                    ),
                }
            })?;
            let mut obligations = resources::vector(pending_rows.len(), budget)?;
            let mut execution = resources::vector(pending_rows.len(), budget)?;
            for row in pending_rows {
                budget.charge_work(2)?;
                obligations.push(*row);
                if row.requirement() == CanonicalRankedSourceRequirementV18::Execution {
                    execution.push(*row);
                }
            }
            Ok(PrivateNativeRowsV18 {
                coverage: std::cell::RefCell::new(coverage),
                obligations,
                execution,
                recipes: recipes.output_recipes(budget)?,
                roots,
            })
        })())
    })?;
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let required = budget.storage();
    let scope_slot = std::ptr::from_ref(budget) as usize;
    let scope_ledger = budget.work_ledger_identity_v1();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut completed_roots = 0usize;
        for root in 0..rows.roots {
            scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                original,
                optimized,
                root,
                input_memory,
                output_memory,
                budget,
                |currentness, budget| {
                    let request = ProductionSourcePrivateMemoryRootRequestV18 {
                        original,
                        optimized,
                        physical,
                        currentness,
                        root,
                        coverage: &rows.coverage,
                        completed: std::cell::Cell::new(false),
                        required: budget.storage(),
                    };
                    request.check(budget)?;
                    check_root(&request, budget).map_err(PrivateNativeFlowV18::Callback)?;
                    request.check(budget)?;
                    if !request.completed.get() {
                        return Err(PrivateNativeFlowV18::Native(NativeError::Source(
                            original
                                .source
                                .missing::<()>("private native root request was not completed")
                                .unwrap_err(),
                        )));
                    }
                    if budget.storage() != request.required {
                        return Err(PrivateNativeFlowV18::Native(NativeError::Source(
                            original
                                .retain_query_resource_error_v18(ArgumentResourceV1::Accounting),
                        )));
                    }
                    Ok::<_, PrivateNativeFlowV18<E>>(())
                },
            )?;
            completed_roots = completed_roots
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        let coverage = rows.coverage.borrow();
        let proof = physical.native_physical_v18(budget)?;
        let mut memory = 0usize;
        let mut aliases = 0usize;
        for (index, completed) in coverage.iter().enumerate() {
            budget.charge_work(14)?;
            if *completed != proof.operation(index) {
                return Err(PrivateNativeFlowV18::Native(recipes.source_failure(
                    "private native global physical/source census incomplete",
                )));
            }
            if *completed {
                memory = memory
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            // A pure access restriction uses the same physical allocation, but
            // cannot discharge its source obligation without completed source
            // coverage of that allocation. It never becomes a memory effect.
            if let Some(address) = proof.access_restriction(index) {
                if coverage.get(address.allocation()) != Some(&true) {
                    return Err(PrivateNativeFlowV18::Native(recipes.source_failure(
                        "private native alias allocation lacks source coverage",
                    )));
                }
                aliases = aliases
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        if completed_roots != rows.roots {
            return Err(PrivateNativeFlowV18::Native(
                recipes.source_failure("private native missing source root"),
            ));
        }
        let completion = PrivateSourceCompletionV18 {
            original,
            optimized,
            physical,
            coverage: &coverage,
            roots: completed_roots,
            memory,
            aliases,
            required,
        };
        completion.join_pending(recipes, pending, &rows.execution, &rows.recipes, budget)?;
        let observed =
            pending.with_private_memory_observations_v18(proof, budget, |native, budget| {
                let observation = native.observation(budget)?;
                let last_invocation = native.last_invocation(budget)?;
                diagnostic.set(Some(NativeDiagnostic {
                    observation,
                    last_invocation,
                }));
                let owner = native.owner(budget)?;
                if let Err(error) = recipes.check_owner(owner, budget) {
                    return Ok(Err(error));
                }
                let obligations = native.obligations(budget)?;
                budget.charge_work(
                    obligations
                        .len()
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )?;
                if obligations != rows.obligations.as_slice() {
                    return Ok(Err(recipes.source_failure(
                        "private native imported obligation census changed",
                    )));
                }
                let view = ProductionPrivateMemoryCheckedNativePoliciesV18 {
                    recipes,
                    completion: &completion,
                    native,
                    obligations,
                };
                if let Err(error) = view.check(budget) {
                    return Ok(Err(error));
                }
                Ok(consume(&view, budget))
            });
        if let Err(error) = &observed {
            diagnostic.set(Some(NativeDiagnostic::from_error(error)));
        }
        observed
            .map_err(|error| PrivateNativeFlowV18::Native(recipes.native_error(error)))?
            .map_err(PrivateNativeFlowV18::Native)
    }));
    let prior = source.guard.first.get();
    // Observe this scope's higher floor on every disposition before dropping
    // rows or refunding their credits. Older recipe/physical floors are lower.
    let custody = if scope_slot != std::ptr::from_ref(budget) as usize
        || scope_ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
    {
        source.cleanup.deny_refund();
        Err(original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting))
    } else {
        recipes
            .observe_custody(budget)
            .and_then(|_| physical.observe_native_source_custody_v18(budget))
    };
    let postflight = custody.and_then(|_| {
        if matches!(&caught, Ok(Ok(()))) {
            recipes
                .check(budget)
                .and_then(|_| physical.check_native_source_subject_v18(original, optimized, budget))
        } else {
            Ok(())
        }
    });
    drop(rows);
    source_owned_finish_callback_v18(caught, prior, postflight, source.cleanup, budget, storage)
}
