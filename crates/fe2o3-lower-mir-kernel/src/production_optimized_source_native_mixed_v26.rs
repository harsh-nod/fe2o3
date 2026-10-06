// Mixed completion keeps source-private proof, exact global source roles and
// runtime premises distinct while joining one authentic native census.
use slice_view_v1::{
    CompletedGlobalSourcesV26, CompletedGlobalSourcesV89, ProductionMixedRuntimeOccurrenceV26,
    ProductionMixedRuntimeOccurrenceV89, ProductionMixedSliceRuntimePremiseV26,
};

fn mixed_source_intrinsics_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<bool>> {
    let output = optimized.output_inventory(budget)?;
    let mut completed = resources::vector(output.operations().len(), budget)?;
    budget.charge_work(output.operations().len())?;
    completed.resize(output.operations().len(), false);
    let semantic = original.source.source_semantic(budget)?;
    for root in 0..original.source.root_count(budget)? {
        for instance in 0..original.source.instance_count(root, budget)? {
            budget.charge_work(2)?;
            if !original.source.instance_active(root, instance, budget)? {
                continue;
            }
            let function = original.source.instance(root, instance, budget)?.0;
            let function = semantic.functions().get(function.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "mixed intrinsic source instance function",
                ),
            )?;
            for (block, source_block) in function.blocks().iter().enumerate() {
                budget.charge_work(3)?;
                let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                    continue;
                };
                let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { index_witness, .. },
                    ..
                }) = semantic.callables().get(call.callee().index() as usize)
                else {
                    continue;
                };
                budget.charge_work(5)?;
                if !call.arguments().is_empty()
                    || !call.variadic_argument_abis().is_empty()
                    || call.destination().is_none_or(|destination| {
                        destination.place().ty() != *index_witness
                            || !destination.place().projections().is_empty()
                    })
                    || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                {
                    return original
                        .source
                        .missing("mixed intrinsic original ThreadIndex1d contract differs");
                }
                let block = SemanticBlockIdV1::from_index(
                    u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let mut found = false;
                original.visit_source_operations(root, instance, block, None, budget, |source, budget| {
                    let ProductionSourceOperationV18::Operation(input) = source else {
                        return original.source.missing("mixed intrinsic original source span is not an operation");
                    };
                    budget.charge_work(3)?;
                    if found { return original.source.missing("mixed intrinsic original source span repeats"); }
                    found = true;
                    let before = source_operation_row_v18(original.inventory, input, budget)?;
                    if !matches!(&before.operation.kind, OperationKind::Intrinsic(intrinsic) if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d())
                        || !matches!(before.operation.results.as_slice(), [result] if result.ty == Type::INDEX) {
                        return original.source.missing("mixed intrinsic original recipe emitted another operation");
                    }
                    let at = match optimized.operation(input, budget)? {
                        ProductionOptimizedSourceOperationV18::Retained { input: original_input, output } if original_input == input => output,
                        ProductionOptimizedSourceOperationV18::RemovedUnreachable { input: original_input } if original_input == input => return Ok(()),
                        _ => return original.source.missing("mixed intrinsic unsupported rewritten source occurrence"),
                    };
                    let after = source_operation_row_v18(output, at, budget)?;
                    let ([before_result], [after_result]) = (before.operation.results.as_slice(), after.operation.results.as_slice()) else {
                        return original.source.missing("mixed intrinsic result census differs");
                    };
                    if before.operation.kind != after.operation.kind || after_result.ty != Type::INDEX
                        || !optimized_source_value_descends_v18(original, optimized, input.block.function, before_result.id, at.block.function, after_result.id, budget)? {
                        return original.source.missing("mixed intrinsic retained recipe/result differs");
                    }
                    let index = resources::operation_index(output, at, budget)?;
                    if completed[index] { return original.source.missing("mixed intrinsic output has duplicate original origins"); }
                    completed[index] = true;
                    Ok(())
                })?;
                if !found {
                    return original
                        .source
                        .missing("mixed intrinsic original source occurrence is absent");
                }
            }
        }
    }
    Ok(completed)
}

fn mixed_source_traps_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<bool>> {
    let output = optimized.output_inventory(budget)?;
    let mut completed = resources::vector(output.operations().len(), budget)?;
    budget.charge_work(output.operations().len())?;
    completed.resize(output.operations().len(), false);
    let semantic = original.source.source_semantic(budget)?;
    for root in 0..original.source.root_count(budget)? {
        let root_row = original.source.root_row(root)?;
        let Some(relation) = &root_row.terminal_failures else {
            continue;
        };
        budget.charge_work(3)?;
        if relation.origins.ledger != budget.work_ledger_identity_v1()
            || relation.origins.rows.len() != relation.closures.len()
        {
            return original
                .source
                .missing("mixed terminal failure origin custody differs");
        }
        for (row, (origin, closure)) in relation
            .origins
            .rows
            .iter()
            .zip(&relation.closures)
            .enumerate()
        {
            budget.charge_work(12)?;
            let instance = origin.instance.index();
            if closure.origin != row
                || !original.source.instance_active(root, instance, budget)?
                || original.source.instance(root, instance, budget)?.0 != origin.function
            {
                return original
                    .source
                    .missing("mixed terminal failure source instance differs");
            }
            let declaration = semantic
                .functions()
                .get(origin.function.index() as usize)
                .and_then(|function| function.blocks().get(origin.block.index() as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "mixed terminal original site is absent",
                ))?;
            let source_matches = match (origin.kind, declaration.terminator().kind()) {
                (
                    TerminalFailureKindV18::Assert { .. },
                    SemanticTerminatorKindV1::Assert { unwind, .. },
                ) => !matches!(unwind, SemanticUnwindActionV1::Cleanup(_)),
                (TerminalFailureKindV18::Abort, SemanticTerminatorKindV1::Abort)
                | (
                    TerminalFailureKindV18::UnwindTerminate,
                    SemanticTerminatorKindV1::UnwindTerminate,
                ) => true,
                (TerminalFailureKindV18::Trap, SemanticTerminatorKindV1::Call(call)) => {
                    call.arguments().is_empty()
                        && call.destination().is_none()
                        && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                        && matches!(
                            semantic.callables().get(call.callee().index() as usize),
                            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                                operation: SemanticCompilerIntrinsicOperationV1::Trap,
                                ..
                            })
                        )
                }
                (TerminalFailureKindV18::BoundsGuard, SemanticTerminatorKindV1::Call(call)) => {
                    matches!(
                        semantic.callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { .. },
                            ..
                        })
                    )
                }
                _ => false,
            };
            if !source_matches {
                return original
                    .source
                    .missing("mixed terminal failure original recipe differs");
            }
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::TerminalFailure,
                instance,
                row,
                field: TileAttachmentFieldV29::FailureDiagnostic,
                component: 0,
                part: 0,
            };
            let [attachment] = original.attachment_range(key, budget)? else {
                return original
                    .source
                    .missing("mixed terminal failure diagnostic occurrence census differs");
            };
            let ProductionSourceOperationV18::Operation(input) =
                original.mapped_source_operation(attachment.location, budget)?
            else {
                return original
                    .source
                    .missing("mixed terminal failure diagnostic is not an operation");
            };
            let before = source_operation_row_v18(original.inventory, input, budget)?;
            let block = source_block_row_v18(original.inventory, input.block, budget)?;
            budget.charge_work(6)?;
            if input.block.function.0 as usize != root_row.function_ordinal
                || block.block.id != closure.block
                || input.operation != closure.diagnostic
                || !before
                    .operation
                    .has_registered_trap_contract_with_budget_v26(budget)?
                || input.operation as usize + 1 != block.block.operations.len()
                || !matches!(block.block.terminator, Some(Terminator::Unreachable))
            {
                return original
                    .source
                    .missing("mixed terminal failure original diagnostic or continuation differs");
            }
            let at = match optimized.operation(input, budget)? {
                ProductionOptimizedSourceOperationV18::Retained {
                    input: exact,
                    output,
                } if exact == input => output,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { input: exact }
                    if exact == input =>
                {
                    continue;
                }
                _ => {
                    return original.source.missing(
                        "mixed terminal failure was rewritten without an original occurrence",
                    );
                }
            };
            let after = source_operation_row_v18(output, at, budget)?;
            let block = source_block_row_v18(output, at.block, budget)?;
            budget.charge_work(4)?;
            if !after
                .operation
                .has_registered_trap_contract_with_budget_v26(budget)?
                || at.operation as usize + 1 != block.block.operations.len()
                || !matches!(block.block.terminator, Some(Terminator::Unreachable))
            {
                return original
                    .source
                    .missing("mixed terminal retained diagnostic or continuation differs");
            }
            let index = resources::operation_index(output, at, budget)?;
            if completed[index] {
                return original
                    .source
                    .missing("mixed terminal output has duplicate original failure origins");
            }
            completed[index] = true;
        }
    }
    synthetic_traps_v26::complete(original, optimized, &mut completed, budget)?;
    Ok(completed)
}

#[path = "production_source_synthetic_traps_v26.rs"]
mod synthetic_traps_v26;

// Closed monomorphized families share custody, resource settlement, and the
// complete source partition. Their native/occurrence types never interconvert.
macro_rules! mixed_source_family_v89 {
    ($checked:ident, $native:ident, $globals:ident, $occurrence:ident, $check_subject:ident, $join:ident, $headers:ident, $method:ident, $scope:ident, $stores:ident, $domains:ident, $observe:ident, $collect:ident) => {
/// Scoped conjunction of exact original source roles, private currentness,
/// conditional globals, and every definition's genuine mixed native pipeline.
/// Runtime premises remain mandatory and no executable authority is granted.
pub struct $checked<'scope, 'owner> {
    recipes: &'scope ProductionOptimizedExecutionRecipesV18<'scope>,
    private: &'scope PrivateSourceCompletionV18<'scope>,
    globals: &'scope $globals<'scope, 'owner>,
    native: &'scope fe2o3_pliron::$native<'scope, 'owner>,
    obligations: &'scope [CanonicalRankedSourceObligationV18],
    intrinsic_count: usize,
    trap_count: usize,
}

impl $checked<'_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> NativeResult {
        if let Err(error) = self.private.observe_custody(budget) {
            self.native.refuse_retained_custody();
            return Err(error.into());
        }
        self.private.check(budget)?;
        self.globals
            .check_source_subject(self.private.original, self.private.optimized, budget)?;
        self.recipes.check(budget)?;
        let owner = self
            .native
            .owner(budget)
            .map_err(|error| self.recipes.pending_error(error))?;
        self.recipes.check_owner(owner, budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        if !std::ptr::eq(
            self.native
                .obligations(budget)
                .map_err(|error| self.recipes.pending_error(error))?,
            self.obligations,
        ) {
            return Err(self
                .recipes
                .source_failure("mixed native obligation owner differs"));
        }
        Ok(())
    }
    /// Rejoins both exact original and optimized owners under current budget custody.
    pub fn $check_subject(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        self.private
            .physical
            .check_native_source_subject_v18(original, optimized, budget)?;
        self.globals
            .check_source_subject(original, optimized, budget)?;
        Ok(())
    }
    // Internal stage transport borrows the completed source evidence only after
    // replaying the same owner and current-account checks as every public query.
    pub(in crate::production_semantic_kir_v1) fn source_obligations_v30(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&[CanonicalRankedSourceObligationV18], NativeError> {
        self.$check_subject(original, optimized, budget)?;
        Ok(self.obligations)
    }

    pub(in crate::production_semantic_kir_v1) fn completed_globals_v30(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&$globals<'_, '_>, NativeError> {
        self.$check_subject(original, optimized, budget)?;
        Ok(self.globals)
    }
    /// Returns the complete output function roster, including external declarations.
    pub fn function_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<usize, NativeError> {
        self.check(budget)?;
        self.native
            .function_count(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }
    /// Borrows all nine typed native stage joins; declarations have no report.
    /// An out-of-range ordinal records a sticky refusal.
    pub fn report(
        &self,
        function: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&fe2o3_pliron::CanonicalMixedPipelineReportV26>, NativeError> {
        self.check(budget)?;
        self.native
            .report(function, budget)
            .map_err(|error| self.recipes.pending_error(error))
    }
    /// Returns the real native policy history for a definition, or None for a declaration.
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
    /// Returns metered native resource observations, not admission evidence by themselves.
    pub fn observation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1, NativeError> {
        self.check(budget)?;
        self.native
            .observation(budget)
            .map_err(|error| self.recipes.pending_error(error))
    }
    /// Original roots, private effects, private aliases, global source roles,
    /// original-source intrinsic occurrences, and terminal failures, respectively.
    pub fn source_census(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<[usize; 6], NativeError> {
        self.check(budget)?;
        let (roots, globals) = self.globals.census(budget)?;
        Ok([
            roots,
            self.private.memory,
            self.private.aliases,
            globals,
            self.intrinsic_count,
            self.trap_count,
        ])
    }
    /// Borrows source-bound argument premises that runtime must still discharge.
    pub fn runtime_premises(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&[ProductionMixedSliceRuntimePremiseV26], NativeError> {
        self.check(budget)?;
        Ok(self.globals.runtime_premises(budget)?)
    }
    /// Borrows exact access and address-formation rows in producer occurrence order.
    pub fn runtime_occurrences(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&[$occurrence], NativeError> {
        self.check(budget)?;
        Ok(self.globals.runtime_occurrences(budget)?)
    }
    /// Always true for this closed view: every pending source role has been joined.
    pub const fn source_roles_are_complete(&self) -> bool {
        true
    }
    /// Always false: this compiler scope has no concrete runtime allocation facts.
    pub const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    /// Always false: conditional native/source completion is not final admission.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// Always false: authenticated downstream contract composition is required.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

// Only an exact original active-instance recipe can populate this dense map.
// No opcode/count-only producer is exposed to a caller.
fn $join(
    recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
    private: &PrivateSourceCompletionV18<'_>,
    globals: &$globals<'_, '_>,
    intrinsic: &[bool],
    traps: &[bool],
    pending: &fe2o3_pliron::$native<'_, '_>,
    rows: &PrivateNativeRowsV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), NativeError> {
    private.check(budget)?;
    globals.check_source_subject(private.original, private.optimized, budget)?;
    let owner = pending
        .owner(budget)
        .map_err(|error| recipes.pending_error(error))?;
    recipes.check_owner(owner, budget)?;
    let proof = private.physical.native_physical_v18(budget)?;
    let output = private.optimized.output_inventory(budget)?;
    if intrinsic.len() != output.operations().len() || traps.len() != output.operations().len() {
        return Err(recipes.source_failure("mixed intrinsic census owner differs"));
    }
    let mut counts = [0usize; 6];
    let mut previous = None;
    for obligation in pending
        .obligations(budget)
        .map_err(|error| recipes.pending_error(error))?
    {
        budget
            .charge_work(8)
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        let key = operation_key(obligation.coordinate());
        if previous.is_some_and(|old| old >= key) {
            return Err(recipes.source_failure("mixed obligation roster repeats or is unordered"));
        }
        previous = Some(key);
        let index = resources::operation_index(output, obligation.coordinate(), budget)?;
        let private_effect = private.coverage.get(index) == Some(&true);
        let private_alias = proof
            .access_restriction(index)
            .is_some_and(|address| private.coverage.get(address.allocation()) == Some(&true));
        let global = globals
            .exact_operation(obligation.coordinate(), budget)?
            .is_some();
        let source_intrinsic = intrinsic[index];
        let source_trap = traps[index];
        let class = match obligation.requirement() {
            CanonicalRankedSourceRequirementV18::Memory => {
                if usize::from(private_effect) + usize::from(private_alias) + usize::from(global)
                    != 1
                    || source_intrinsic
                    || source_trap
                {
                    return Err(NativeError::Unresolved(*obligation));
                }
                if private_effect {
                    0
                } else if private_alias {
                    1
                } else {
                    2
                }
            }
            CanonicalRankedSourceRequirementV18::Intrinsic
                if source_intrinsic
                    && !source_trap
                    && !private_effect
                    && !private_alias
                    && !global =>
            {
                3
            }
            CanonicalRankedSourceRequirementV18::Execution
                if !source_intrinsic
                    && !source_trap
                    && !private_effect
                    && !private_alias
                    && !global =>
            {
                if rows.execution.get(counts[4]) != Some(obligation) {
                    return Err(recipes.source_failure("mixed execution occurrence census differs"));
                }
                4
            }
            CanonicalRankedSourceRequirementV18::Call
                if source_trap
                    && !source_intrinsic
                    && !private_effect
                    && !private_alias
                    && !global =>
            {
                5
            }
            _ => return Err(NativeError::Unresolved(*obligation)),
        };
        counts[class] = counts[class]
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
    }
    let (roots, global_count) = globals.census(budget)?;
    budget
        .charge_work(intrinsic.len())
        .map_err(ProductionSourceOwnedViewErrorV18::from)?;
    let intrinsic_count = intrinsic.iter().filter(|value| **value).count();
    budget
        .charge_work(traps.len())
        .map_err(ProductionSourceOwnedViewErrorV18::from)?;
    let trap_count = traps.iter().filter(|value| **value).count();
    if roots != private.roots
        || counts
            != [
                private.memory,
                private.aliases,
                global_count,
                intrinsic_count,
                rows.execution.len(),
                trap_count,
            ]
    {
        return Err(recipes.source_failure("mixed complete source obligation partition differs"));
    }
    recipes.join_pending(owner, &rows.execution, &rows.recipes, budget)?;
    Ok((intrinsic_count, trap_count))
}

fn $headers<E>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, E> = (
        $checked<'a, 'a>,
        Vec<bool>,
        Vec<bool>,
        Vec<bool>,
        Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
        &'a $globals<'a, 'a>,
        &'a fe2o3_pliron::$native<'a, 'a>,
        &'a fe2o3_kernel_ir::CheckedCanonicalConditionalSliceDomainsV26<'a, 'a>,
        &'a PrivateSourceCompletionV18<'a>,
        &'a PrivateNativeRowsV18,
        Option<NativeResult>,
        Option<Result<(), PrivateNativeFlowV18<E>>>,
        std::thread::Result<Result<(), PrivateNativeFlowV18<E>>>,
        [usize; 24],
        [&'a (); 48],
        [bool; 8],
        CanonicalRankedSourceObligationV18,
        Option<[usize; 3]>,
        std::slice::Iter<'a, bool>,
        std::slice::Iter<'a, CanonicalRankedSourceObligationV18>,
        ProductionSourceOperationV18,
        ProductionOptimizedSourceOperationV18,
        &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
        &'a TerminalFailureOriginV18,
        &'a TerminalFailureClosureV18,
        &'a TerminalFailureRelationV18,
        TerminalFailureKindV18,
        TileAttachmentKeyV29,
        &'a SourceAttachmentV18,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        std::iter::Enumerate<
            std::iter::Zip<
                std::slice::Iter<'a, TerminalFailureOriginV18>,
                std::slice::Iter<'a, TerminalFailureClosureV18>,
            >,
        >,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, E>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_, E>>>())?,
    ])
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    /// Executes every original-source completion and the genuine fixed native
    /// mixed pipeline on this output. Launch/width are retained conditions,
    /// never concrete runtime authority or a caller-selected completion roster.
    pub fn $method<'work, E>(
        &self,
        checked: &mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        limits: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1,
        launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'work>,
        check_root: impl for<'scope> FnMut(
            &ProductionSourcePrivateMemoryRootRequestV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
        consume: impl for<'scope, 'owner> FnOnce(
            &$checked<'scope, 'owner>,
            &mut ArgumentBudgetV1<'work>,
        ) -> NativeResult,
    ) -> Result<Result<(), E>, NativeError>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let mut entered = false;
        let mut check_root = Some(check_root);
        let mut consume = Some(consume);
        let mut caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.query(budget)?;
            let headers = argument_sum_v1(&[
                $headers::<E>()?,
                source_reference_cleanup_headers_v29()?,
                source_owned_finish_preflight_v26::<Result<(), E>, NativeError>(budget)
                    .map_err(ProductionSourceOwnedViewErrorV18::from)?,
                size_of::<(
                    [usize; 2],
                    std::thread::Result<Result<Result<(), E>, NativeError>>,
                    std::thread::Result<Result<Result<(), E>, NativeError>>,
                    Result<Result<(), E>, NativeError>,
                    SourceOwnedResultV18<()>,
                    Result<usize, ArgumentResourceV1>,
                    [bool; 2],
                )>(),
                argument_product_v1(3, std::mem::size_of_val(&check_root))?,
                std::mem::align_of_val(&check_root),
                argument_product_v1(3, std::mem::size_of_val(&consume))?,
                std::mem::align_of_val(&consume),
            ])
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
            // Ranked queries require their exact retained balance. The pending
            // callback below owns and settles additional wrapper storage.
            let inventory = checked
                .inventory(budget)
                .map_err(|error| self.native_pending_error(error.into()))?;
            if !std::ptr::eq(inventory, self.checked.output()) {
                return Err(self
                    .original
                    .source
                    .missing::<()>("mixed native substituted its checked output inventory")
                    .unwrap_err()
                    .into());
            }
            let diagnostic = DiagnosticCell::new(None);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                layouts,
                budget,
                |pending, budget| {
                    entered = true;
                    let inner_floor = budget.storage();
                    let mut required = inner_floor;
                    let mut caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        budget
                            .reserve_storage(headers)
                            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                        required = budget.storage();
                        budget
                            .charge_work(
                                argument_product_v1(2, SOURCE_REFERENCE_ENTRY_WORK_V29)
                                    .map_err(ProductionSourceOwnedViewErrorV18::from)?,
                            )
                            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                        let result = self.with_execution_recipes_prepaid_v18(budget, |recipes, budget| {
                        scoped_raw_admission_v29::with_source_private_physical_profile_v25::<
                            true,
                            _,
                            _,
                        >(
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
                                                $scope(
                                                    recipes,
                                                    physical,
                                                    input_memory,
                                                    output_memory,
                                                    pending,
                                                    launches,
                                                    width,
                                                    budget,
                                                    &diagnostic,
                                                    check_root.take().ok_or_else(|| PrivateNativeFlowV18::Native(recipes.source_failure("mixed root callback absent before transfer")))?,
                                                    consume.take().ok_or_else(|| PrivateNativeFlowV18::Native(recipes.source_failure("mixed consumer absent before transfer")))?,
                                                )
                                            },
                                        )
                                    },
                                )
                            },
                        )
                        });
                        match result {
                            Err(PrivateNativeFlowV18::Native(error)) => Err(error),
                            Err(PrivateNativeFlowV18::Callback(error)) => Ok(Err(error)),
                            Ok(()) => Ok(Ok(())),
                        }
                    }));
                    let dropped_root = source_reference_discard_v29(check_root.take());
                    let dropped_consume = source_reference_discard_v29(consume.take());
                    if (dropped_root || dropped_consume) && matches!(caught, Ok(Ok(Ok(())))) {
                        source_reference_discard_v29(caught);
                        caught = Ok(Err(self
                            .original
                            .source
                            .missing::<()>("mixed public callback capture panicked")
                            .unwrap_err()
                            .into()));
                    }
                    let custody = if slot != std::ptr::from_ref(budget) as usize
                        || ledger != budget.work_ledger_identity_v1()
                        || budget.storage() != required
                    {
                        self.original.source.cleanup.deny_refund();
                        Err(self
                            .original
                            .retain_query_resource_error_v18(ArgumentResourceV1::Accounting))
                    } else {
                        self.original.observe_custody(budget)
                    };
                    let storage = required
                        .checked_sub(inner_floor)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        source_owned_finish_callback_v18(
                            caught,
                            self.original.source.guard.first.get(),
                            custody,
                            self.original.source.cleanup,
                            budget,
                            storage,
                        )
                    }));
                    if self.original.source.cleanup.is_denied() {
                        pending.refuse_retained_custody();
                    }
                    match result {
                        Ok(result) => Ok(result),
                        Err(payload) => std::panic::resume_unwind(payload),
                    }
                },
            );
            match result {
                Err(error) => Err(self
                    .native_pending_error(error)
                    .with_diagnostic(diagnostic.get())),
                Ok(Err(error)) => Err(error.with_diagnostic(diagnostic.get())),
                Ok(Ok(result)) => Ok(result),
            }
        }));
        let dropped_root = source_reference_discard_v29(check_root.take());
        let dropped_consume = source_reference_discard_v29(consume.take());
        if entered {
            // The inner finalizer already disposed rejected user payloads and
            // settled its exact credit before Pending's exact postflight. No
            // new fallible payload settlement may run after that refund.
            return match caught {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            };
        }
        if (dropped_root || dropped_consume) && matches!(caught, Ok(Ok(Ok(())))) {
            source_reference_discard_v29(caught);
            caught = Ok(Err(self
                .original
                .source
                .missing::<()>("mixed public callback capture panicked")
                .unwrap_err()
                .into()));
        }
        let custody = if slot != std::ptr::from_ref(budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
        {
            self.original.source.cleanup.deny_refund();
            Err(self
                .original
                .retain_query_resource_error_v18(ArgumentResourceV1::Accounting))
        } else {
            self.original.observe_custody(budget)
        };
        // No user callback ran and no mixed header was accepted on this path.
        // Preserve the concrete ingress refusal even if uncalled captures deny
        // custody while being destroyed; there is no credit to refund.
        let _ = custody;
        match caught {
            Ok(Err(error)) => Err(error),
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(Ok(value)) => {
                source_reference_discard_v29(value);
                Err(self
                    .original
                    .retain_query_resource_error_v18(ArgumentResourceV1::Accounting)
                    .into())
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn $scope<'work, E>(
    recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
    physical: &CheckedSourcePrivatePhysicalV18<'_>,
    input_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    output_memory: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    pending: &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut ArgumentBudgetV1<'work>,
    diagnostic: &DiagnosticCell,
    check_root: impl for<'scope> FnMut(
        &ProductionSourcePrivateMemoryRootRequestV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), E>,
    consume: impl for<'scope, 'owner> FnOnce(
        &$checked<'scope, 'owner>,
        &mut ArgumentBudgetV1<'work>,
    ) -> NativeResult,
) -> Result<(), PrivateNativeFlowV18<E>>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    let original = recipes.optimized.original;
    let optimized = recipes.optimized;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut check_root = Some(check_root);
    let mut consume = Some(consume);
    let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let headers = argument_sum_v1(&[
            $headers::<E>()?,
            source_reference_cleanup_headers_v29()?,
            source_owned_finish_preflight_v26::<(), PrivateNativeFlowV18<E>>(budget)?,
            argument_product_v1(2, std::mem::size_of_val(&check_root))?,
            argument_product_v1(2, std::mem::size_of_val(&consume))?,
            std::mem::size_of_val(&check_root),
            std::mem::align_of_val(&check_root),
            std::mem::size_of_val(&consume),
            std::mem::align_of_val(&consume),
        ])?;
        budget.reserve_storage(headers)?;
        budget.charge_work(argument_product_v1(2, SOURCE_REFERENCE_ENTRY_WORK_V29)?)?;
        let output = optimized.output_inventory(budget)?;
        if launches.len() != original.source.root_count(budget)? {
            return original
                .source
                .missing("mixed native launch/root census differs");
        }
        let mut function_launches = resources::vector(output.functions().len(), budget)?;
        budget.charge_work(output.functions().len())?;
        function_launches.resize(
            output.functions().len(),
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [1, 1, 1],
            },
        );
        let mut seen = resources::vector(output.functions().len(), budget)?;
        budget.charge_work(output.functions().len())?;
        seen.resize(output.functions().len(), false);
        for (root, launch) in launches.iter().enumerate() {
            budget.charge_work(4)?;
            let function = optimized_source_root_function_v18(original, optimized, root, budget)?
                .coordinate
                .0 as usize;
            if seen[function] {
                return original
                    .source
                    .missing("mixed native repeated root function mapping");
            }
            seen[function] = true;
            function_launches[function] = *launch;
        }
        // `seen` is scratch but its credit remains retained with this scope.
        drop(seen);
        Ok::<_, ProductionSourceOwnedViewErrorV18>((
            mixed_source_intrinsics_v26(original, optimized, budget)?,
            mixed_source_traps_v26(original, optimized, budget)?,
            function_launches,
        ))
    }));
    let (intrinsic, traps, function_launches) = match prepared {
        Ok(Ok(rows)) => rows,
        refused => {
            source_reference_discard_v29(check_root.take());
            source_reference_discard_v29(consume.take());
            let custody = if slot != std::ptr::from_ref(budget) as usize
                || ledger != budget.work_ledger_identity_v1()
                || budget.storage() < floor
            {
                original.source.cleanup.deny_refund();
                Err(original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting))
            } else {
                original.observe_custody(budget)
            };
            let storage = budget.storage().checked_sub(floor).unwrap_or(0);
            let refused = match refused {
                Ok(Err(error)) => Ok(Err(PrivateNativeFlowV18::from(error))),
                Err(payload) => Err(payload),
                Ok(Ok(_)) => unreachable!(),
            };
            return source_owned_finish_callback_v18(
                refused,
                original.source.guard.first.get(),
                custody,
                original.source.cleanup,
                budget,
                storage,
            );
        }
    };
    let required = budget.storage();
    let storage = required
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let mut caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut attempted = None;
        let proof = physical.native_physical_v18(budget)?;
        let family = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
            optimized.checked.output().owner(), Default::default(), budget, |reads, budget| {
                fe2o3_kernel_ir::$stores(
                    optimized.checked.output().owner(), Default::default(), budget, |stores, budget| {
                        fe2o3_kernel_ir::$domains(reads, stores, &function_launches, width, budget, |globals, budget| {
                            let private_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_private_source_completion_scope_v26(
                                recipes, physical, input_memory, output_memory, pending, budget,
                                check_root.take().ok_or(ArgumentResourceV1::Accounting)?,
                                |completion, rows, pending, budget| {
                                    let observed = pending.$observe(proof, globals, budget, |native, budget| {
                                        let observation = native.observation(budget)?;
                                        let mut last_invocation = None;
                                        for function in 0..native.function_count(budget)? {
                                            if let Some(history) = native.history(function, budget)? { last_invocation = Some(history); }
                                        }
                                        diagnostic.set(Some(NativeDiagnostic { observation, last_invocation }));
                                        let joined = (|| -> Result<(), PrivateNativeFlowV18<E>> {
                                            let obligations = native.obligations(budget).map_err(|error| recipes.pending_error(error))?;
                                            budget.charge_work(obligations.len().checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?)?;
                                            if obligations != rows.obligations.as_slice() { return Err(recipes.source_failure("mixed imported obligation roster differs").into()); }
                                            let mut result = None;
                                            slice_view_v1::$collect(original, optimized, native, reads, stores, launches, width, budget, &mut |global_sources, budget| {
                                                result = Some((|| -> NativeResult {
                                                    let (intrinsic_count, trap_count) = $join(recipes, completion, global_sources, &intrinsic, &traps, native, rows, budget)?;
                                                    let view = $checked { recipes, private: completion, globals: global_sources, native, obligations, intrinsic_count, trap_count };
                                                    view.check(budget)?;
                                                    let consumed = consume.take().ok_or_else(|| recipes.source_failure("mixed source callback repeated"))?(&view, budget);
                                                    view.check(budget)?;
                                                    consumed
                                                })());
                                                Ok(())
                                            })?;
                                            result.ok_or_else(|| recipes.source_failure("mixed source callback absent"))??;
                                            Ok(())
                                        })();
                                        Ok(joined)
                                    });
                                    if let Err(error) = &observed { diagnostic.set(Some(NativeDiagnostic::from_error(error))); }
                                    observed.map_err(|error| PrivateNativeFlowV18::Native(recipes.native_error(error)))?
                                },
                            )));
                            if original.source.cleanup.is_denied() {
                                pending.refuse_retained_custody();
                                globals.refuse_retained_custody();
                            }
                            match private_result {
                                Ok(result) => attempted = Some(result),
                                Err(payload) => std::panic::resume_unwind(payload),
                            }
                            Ok(())
                        })
                    },
                )
            },
        ).map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
        if family.is_none() {
            return Err(recipes
                .source_failure("mixed source conditional family is incomplete")
                .into());
        }
        attempted.ok_or_else(|| recipes.source_failure("mixed source native attempt absent"))?
    }));
    // Uncalled captures remain charged until separately protected destruction;
    // a destructor cannot turn an earlier typed refusal into a success.
    let dropped_root = source_reference_discard_v29(check_root.take());
    let dropped_consume = source_reference_discard_v29(consume.take());
    if (dropped_root || dropped_consume) && matches!(caught, Ok(Ok(()))) {
        caught = Ok(Err(recipes
            .source_failure("mixed source callback capture panicked")
            .into()));
    }
    let prior = original.source.guard.first.get();
    let custody = if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
    {
        original.source.cleanup.deny_refund();
        Err(original.retain_query_resource_error_v18(ArgumentResourceV1::Accounting))
    } else {
        original.observe_custody(budget)
    };
    drop(intrinsic);
    drop(traps);
    drop(function_launches);
    source_owned_finish_callback_v18(
        caught,
        prior,
        custody,
        original.source.cleanup,
        budget,
        storage,
    )
}

    };
}

mixed_source_family_v89!(
    ProductionMixedMemoryCheckedNativePoliciesV26,
    PendingCanonicalMixedMemoryPoliciesV26,
    CompletedGlobalSourcesV26,
    ProductionMixedRuntimeOccurrenceV26,
    check_source_subject_v26,
    join_mixed_source_obligations_v26,
    mixed_source_headers_v26,
    with_mixed_memory_native_policies_v26,
    with_mixed_native_source_scope_v26,
    with_canonical_guarded_global_stores_v24,
    with_canonical_conditional_slice_domains_v26,
    with_mixed_memory_observations_v26,
    with_completed_global_sources_v26
);
mixed_source_family_v89!(
    ProductionPredicatedMemoryCheckedNativePoliciesV89,
    PendingCanonicalPredicatedMemoryPoliciesV89,
    CompletedGlobalSourcesV89,
    ProductionMixedRuntimeOccurrenceV89,
    check_source_subject_v89,
    join_predicated_source_obligations_v89,
    predicated_source_headers_v89,
    with_predicated_memory_native_policies_v89,
    with_predicated_native_source_scope_v89,
    with_canonical_predicated_global_stores_v84,
    with_canonical_predicated_conditional_slice_domains_v85,
    with_predicated_memory_observations_v89,
    with_completed_global_sources_v89
);
