/// Fresh source proofs, complete checked assertion transport and real final-F
/// fixed-nine reports. Original metadata and binding coordinates still name N.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarAssertionPoliciesV1;
/// fn forge() { let _ = ProductionCanonicalScalarAssertionPoliciesV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarAssertionPoliciesV1;
/// fn copy(x: &ProductionCanonicalScalarAssertionPoliciesV1<'_, '_, '_>) { let _ = (*x).clone(); }
/// ```
pub struct ProductionCanonicalScalarAssertionPoliciesV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    lineage: ProductionCanonicalScalarLineageV1<'s>,
    assertions: &'s [ProductionCanonicalScalarAssertionV1],
    policies: &'s CheckedCanonicalTrapPoliciesV1<'s, 'g>,
}
impl ProductionCanonicalScalarAssertionPoliciesV1<'_, '_, '_> {
    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<()> {
        self.lineage.guard.query(budget)?;
        self.source.guard.query(budget)?;
        self.policies.owner(budget)?;
        Ok(())
    }
    /// Borrow all original source obligations, including elided assertions.
    pub fn original_metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.query(budget)?;
        Ok(self.source)
    }
    /// Borrow the actual final inventory checked by the fresh native pipeline.
    pub fn final_inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&CanonicalKirInventoryV1<'_>> {
        self.query(budget)?;
        Ok(self.lineage.output)
    }
    /// Borrow complete numeric N-to-F lineage derived from every actual substage.
    pub fn lineage(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalScalarLineageV1<'_>> {
        self.query(budget)?;
        Ok(&self.lineage)
    }
    /// Complete root-qualified assertion count, not only retained conditionals.
    pub fn assertion_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<usize> {
        self.query(budget)?;
        Ok(self.assertions.len())
    }
    /// Borrow one freshly proved and fully transported source alias.
    pub fn assertion(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalScalarAssertionV1> {
        self.query(budget)?;
        self.assertions.get(ordinal).ok_or_else(|| {
            self.lineage
                .guard
                .missing("scalar assertion ordinal")
                .into()
        })
    }
    /// Actual F trap shapes and nine genuine producer reports per definition.
    pub fn policies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&CheckedCanonicalTrapPoliciesV1<'_, '_>> {
        self.query(budget)?;
        Ok(self.policies)
    }
    /// Cumulative source-ledger usage, not native analysis units or a new debit.
    pub fn source_resources(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalAssertionResourcesV1> {
        self.query(budget)?;
        Ok(observation(budget))
    }
    /// Full compiler obligations are not discharged by this scalar increment.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No target, external proof, artifact or launch authority is exposed.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn csa_original_success_v1(binding: SemanticKirAssertConditionBindingV1) -> CsEdgeV1 {
    match binding.outcome() {
        SemanticKirAssertConditionOutcomeV1::Emitted { success_edge, .. }
        | SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
            success_edge
        }
    }
}

fn csa_final_route_v1(
    row: &ProductionCanonicalScalarAssertionV1,
    output: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    budget.charge_work(5)?;
    if row.state.condition.is_some() && row.state.selected.is_some() {
        return Err(binding(Some(row.span), "retained assertion has a selection event").into());
    }
    let block = &lineage.block_controls[row.block_origin];
    let success = &lineage.edge_controls[row.success_origin];
    if block.original != row.binding.block()
        || success.original != csa_original_success_v1(row.binding)
        || success.placement != row.state.success
    {
        return Err(binding(Some(row.span), "final assertion lineage origin").into());
    }
    if row.state.removed.is_some() {
        if row.state.condition.is_some()
            || row.state.success != CsaEdgePlaceV1::Omitted
            || block.reachable
            || success.executable
        {
            return Err(binding(Some(row.span), "checked removal has live final assertion").into());
        }
        return Ok(());
    }
    if row.state.condition.is_none()
        && row.state.selected.is_none()
        && !matches!(
            row.binding.outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        )
    {
        return Err(binding(Some(row.span), "elision lacks source or checked selection").into());
    }
    match row.state.success {
        CsaEdgePlaceV1::Retained(edge) => {
            let actual = &output.edges()[cs_edge_v1(output, edge, budget)?];
            let body = &output.blocks()[cs_block_v1(output, edge.source, budget)?];
            budget.charge_work(argument_sum_v1(&[3, actual.arguments.len()])?)?;
            if block.placement.map(|p| p.output) != Some(edge.source) {
                return Err(binding(Some(row.span), "success edge source placement").into());
            }
            if row.state.condition.is_none() {
                let Terminator::Branch { target, arguments } = body.terminator else {
                    return Err(binding(
                        Some(row.span),
                        "elided final success is not unconditional",
                    )
                    .into());
                };
                if body.edges.len() != 1
                    || edge.successor != 0
                    || actual.target_id != *target
                    || actual.arguments != arguments.as_slice()
                {
                    return Err(binding(Some(row.span), "elided final success payload").into());
                }
            }
        }
        CsaEdgePlaceV1::InternalConnector(placement) => {
            if row.state.condition.is_some() || block.placement != Some(placement) {
                return Err(binding(Some(row.span), "internalized selector placement").into());
            }
            let block = cs_block_v1(output, placement.output, budget)?;
            let chain = lineage.blocks[block].clone();
            let offset =
                usize::try_from(placement.segment).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let ordinal = argument_sum_v1(&[chain.start, offset])?;
            budget.charge_work(2)?;
            if ordinal >= chain.end
                || lineage.segments[ordinal].original != row.binding.block()
                || lineage.segments[ordinal].connector != Some(csa_original_success_v1(row.binding))
            {
                return Err(binding(Some(row.span), "exact internal assertion connector").into());
            }
        }
        CsaEdgePlaceV1::Omitted => {
            return Err(binding(Some(row.span), "final omission has no checked event").into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn csa_final_join_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    coverage: &Coverage<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    assertions: &CsaTransportV1,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    budget.charge_work(3)?;
    if !std::ptr::eq(policies.owner(budget)?, output.owner())
        || assertions.rows.len() != source.contracts.assertions.len()
        || lineage.functions.len() != source.inventory.functions().len()
    {
        return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
    }
    let (sparse, receipt) = CanonicalKirSparseV1::derive(
        output,
        fe2o3_kernel_analysis::CanonicalKirSparseLimitsV1::default(),
        budget,
    )
    .map_err(Failure::Sparse)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let pairs = policies.pair_count(budget)?;
    for (row, original) in assertions.rows.iter().zip(&source.contracts.assertions) {
        budget.charge_work(3)?;
        coverage.span(source, row.span, budget)?;
        if row.span != original.span || row.binding != original.binding {
            return Err(binding(Some(row.span), "complete final source alias order").into());
        }
        csa_final_route_v1(row, output, lineage, budget)?;
        let Some(condition) = row.state.condition else {
            continue;
        };
        let CsaEdgePlaceV1::Retained(success) = row.state.success else {
            return Err(binding(Some(row.span), "final conditional success edge").into());
        };
        csa_condition_v1(output, condition, success, row.binding.expected(), budget)?;
        sparse_veto(
            sparse
                .value_at_use(condition.used, budget)
                .map_err(Failure::Sparse)?,
            row.binding.expected(),
            row.span,
        )?;
        let success_row = &output.edges()[cs_edge_v1(output, success, budget)?];
        let failure_row = &output.edges()[cs_edge_v1(output, condition.failure, budget)?];
        let actual_use = &output.uses()[cs_use_v1(output, condition.used, budget)?];
        let mut matches = 0usize;
        for pair in 0..pairs {
            for incoming in policies.pair(pair, budget)?.incoming_edges() {
                let candidate = policies.incoming_edge(incoming, budget)?;
                budget.charge_work(7)?;
                if candidate.failure().coordinate != condition.failure {
                    continue;
                }
                if candidate.condition().coordinate != condition.used
                    || candidate.condition().value != actual_use.value
                    || candidate.definition().coordinate != condition.definition
                    || *candidate.definition().ty != Type::BOOL
                    || candidate.success_when() != row.binding.expected()
                    || !same_edge(candidate.success(), success_row, budget)?
                    || !same_edge(candidate.failure(), failure_row, budget)?
                {
                    return Err(binding(
                        Some(row.span),
                        "final trap incoming differs from checked assertion",
                    )
                    .into());
                }
                matches = argument_sum_v1(&[matches, 1])?;
            }
        }
        if matches != 1 {
            return Err(binding(
                Some(row.span),
                "final conditional lacks one exact trap incoming",
            )
            .into());
        }
    }

    // Every actual final incoming must have each original root-qualified alias.
    for ordinal in 0..pairs {
        let pair = policies.pair(ordinal, budget)?;
        let operation = cs_operation_v1(output, pair.call(), budget)?;
        let ProductionCanonicalScalarOperationOriginV1::Original(original_call) =
            lineage.operations[operation]
        else {
            return Err(
                binding(None, "synthetic final trap call has no original operation").into(),
            );
        };
        let function = cs_function_v1(output, pair.call().block.function, budget)?;
        let original_function = lineage.functions[function];
        if original_call.block.function != original_function {
            return Err(binding(None, "final trap function transport").into());
        }
        let mut associations = 0usize;
        for (association, group) in source.calls.groups.iter().enumerate() {
            budget.charge_work(1)?;
            if group.function.canonical.coordinate != original_function {
                continue;
            }
            associations = argument_sum_v1(&[associations, 1])?;
            let mut synthetic = 0usize;
            for (span, row) in source.source.spans.iter().enumerate() {
                budget.charge_work(3)?;
                if row.association == association
                    && coverage.synthetic[span]
                    && row.operations.len() == 1
                    && source.inventory.operations()[row.operations.start].coordinate
                        == original_call
                {
                    synthetic = argument_sum_v1(&[synthetic, 1])?;
                }
            }
            if synthetic != 1 {
                return Err(
                    binding(None, "final trap lacks exact original synthetic alias").into(),
                );
            }
            for incoming in pair.incoming_edges() {
                let failure = policies
                    .incoming_edge(incoming, budget)?
                    .failure()
                    .coordinate;
                let mut aliases = 0usize;
                for row in &assertions.rows {
                    budget.charge_work(2)?;
                    if source.source.spans[row.span].association == association
                        && row
                            .state
                            .condition
                            .is_some_and(|condition| condition.failure == failure)
                    {
                        aliases = argument_sum_v1(&[aliases, 1])?;
                    }
                }
                if aliases != 1 {
                    return Err(binding(None, "complete shared final trap incoming aliases").into());
                }
            }
        }
        if associations == 0 {
            return Err(binding(None, "final trap has no source association").into());
        }
    }
    // An absent ordered Trap call is not a grant: its checked source control must
    // have become unreachable in the real chain, and a survivor must be a C pair.
    for (span, row) in source.source.spans.iter().enumerate() {
        budget.charge_work(1)?;
        if !coverage.synthetic[span] {
            continue;
        }
        if row.operations.len() != 1 {
            return Err(binding(Some(span), "original synthetic trap extent").into());
        }
        let original = source.inventory.operations()[row.operations.start].coordinate;
        let mut surviving = 0usize;
        for (ordinal, origin) in lineage.operations.iter().enumerate() {
            budget.charge_work(1)?;
            if *origin != ProductionCanonicalScalarOperationOriginV1::Original(original) {
                continue;
            }
            surviving = argument_sum_v1(&[surviving, 1])?;
            let coordinate = output.operations()[ordinal].coordinate;
            let mut exact_pair = 0usize;
            for pair in 0..pairs {
                if policies.pair(pair, budget)?.call() == coordinate {
                    exact_pair = argument_sum_v1(&[exact_pair, 1])?;
                }
            }
            if exact_pair != 1 {
                return Err(binding(
                    Some(span),
                    "retained synthetic trap is not a complete final pair",
                )
                .into());
            }
        }
        let block = cs_block_v1(source.inventory, original.block, budget)?;
        if surviving > 1 || (surviving == 0 && lineage.block_controls[block].reachable) {
            return Err(binding(
                Some(span),
                "ordered trap removed without checked unreachable control",
            )
            .into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn csa_callback_v1<'w, T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    assertions: &CsaTransportV1,
    policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalScalarAssertionPoliciesV1<'s, 'm, 'g>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<ProductionCanonicalScalarAssertionPoliciesV1<'_, '_, '_>>(),
        std::mem::size_of::<CrGuardV1>(),
        std::mem::size_of::<std::thread::Result<CsResultV1<T>>>(),
    ])?)?;
    let guard = CrGuardV1::new(budget);
    cs_check_subjects_v1(owner, source, output, policies.owner(budget)?, budget)?;
    let view = ProductionCanonicalScalarAssertionPoliciesV1 {
        source,
        lineage: ProductionCanonicalScalarLineageV1 {
            original: source.inventory,
            output,
            rows: lineage,
            guard: &guard,
        },
        assertions: &assertions.rows,
        policies,
    };
    let paid = budget.storage();
    let returned = catch_unwind(AssertUnwindSafe(|| callback(&view, budget)));
    let check = if budget.storage() == paid {
        view.query(budget)
    } else {
        Err(ArgumentResourceV1::Accounting.into())
    };
    if let Err(error) = check {
        let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
        drop(rejected);
        return Err(error);
    }
    match returned {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Failure::Panicked.into())
        }
    }
}

impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Fresh source proofs plus checked assertion transport and complete final-F
    /// fixed-nine reports. N reports and diagnostic elision flags are not inputs.
    /// Prepay the complete retained owner floor; callback storage/ledger must be
    /// unchanged and returned owned values must be prepaid before entry.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarFixedPointOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionCanonicalScalarFixedPointOwnerV1, budget: &mut Budget<'_>) {
    ///     owner.with_assertion_policy_checks_v1(budget, |view, _| Ok(view));
    /// }
    /// ```
    pub fn with_assertion_policy_checks_v1<'w, T>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl for<'s, 'm, 'g> FnOnce(
            &ProductionCanonicalScalarAssertionPoliciesV1<'s, 'm, 'g>,
            &mut ArgumentBudgetV1<'w>,
        ) -> CsResultV1<T>,
    ) -> CsResultV1<T> {
        budget.charge_work(1)?;
        if budget.storage() < self.retained {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        csa_scope_v1(budget, |budget| {
            self.original
                .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                    Ok(csa_with_source_v1(
                        view,
                        budget,
                        |source, coverage, budget| {
                            self.history
                                .replay_against(self.original.executable(), budget)?;
                            let mut assertions =
                                CsaTransportV1::original(source, coverage, budget)?;
                            let lineage = cs_lineage_with_observer_v1(
                                self,
                                source.inventory,
                                &mut assertions,
                                budget,
                            )?;
                            cs_with_final_view_v1(
                                self,
                                source,
                                &lineage,
                                budget,
                                |output, checked, budget| {
                                    fe2o3_pliron::with_canonical_trap_policy_checks_v1(
                                        checked,
                                        budget,
                                        |policies, budget| {
                                            Ok(csa_scope_v1(budget, |budget| {
                                                csa_final_join_v1(
                                                    source,
                                                    coverage,
                                                    output,
                                                    &lineage,
                                                    &assertions,
                                                    policies,
                                                    budget,
                                                )?;
                                                csa_callback_v1(
                                                    self,
                                                    source,
                                                    output,
                                                    &lineage,
                                                    &assertions,
                                                    policies,
                                                    budget,
                                                    callback,
                                                )
                                            }))
                                        },
                                    )
                                    .map_err(ProductionCanonicalScalarSourceErrorV1::FinalPolicy)?
                                },
                            )
                        },
                    ))
                })?
        })
    }
}
