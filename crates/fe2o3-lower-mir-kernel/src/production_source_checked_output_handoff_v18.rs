/// Refusal of a concrete check in the closed scalar handoff.
#[derive(Debug)]
pub enum ProductionClosedScalarCheckErrorV18 {
    /// The authentic source or continuing ledger refused the operation.
    Source(ProductionSourceOwnedViewErrorV18),
    /// An obligation is outside the explicitly supported structural subset.
    Unsupported(&'static str),
    /// Construction/checking of the actual output's ranked analysis view failed.
    Ranked(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
    /// Complete source-role census or actual native policies refused the output.
    Native(ProductionSourceNativeLifecycleErrorV18),
}

impl fmt::Display for ProductionClosedScalarCheckErrorV18 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Unsupported(detail) => write!(out, "closed scalar handoff check: {detail}"),
            Self::Ranked(error) => error.fmt(out),
            Self::Native(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for ProductionClosedScalarCheckErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Ranked(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Unsupported(_) => None,
        }
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for ProductionClosedScalarCheckErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionClosedScalarCheckErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

/// Preserves whether refusal preceded optimization or came from real adoption.
#[derive(Debug)]
pub enum ProductionClosedScalarHandoffErrorV18 {
    /// A source, subset, or resource check failed before optimization.
    Check(ProductionClosedScalarCheckErrorV18),
    /// The actual optimizer/adoption failed, retaining the original check error.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionClosedScalarCheckErrorV18>),
}

impl fmt::Display for ProductionClosedScalarHandoffErrorV18 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for ProductionClosedScalarHandoffErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Check(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for ProductionClosedScalarHandoffErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionClosedScalarHandoffErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}

/// Move-only output joined to the still-live authentic original source.
///
/// The closed subset contains only scalar/unit kernel roots, scalar local
/// copies/constants and one Return block. Both exact graphs pass a closed
/// operation census, the real optimizer's independent transition checker,
/// fresh source-memory currentness, and the complete lifecycle/native checker.
/// Unsupported memory, assertions, calls and control flow refuse construction.
/// The captured full ABI and original launch roster are rejoined, not inferred.
///
/// This is a lexical continuation, not final ranked/formal/target authority or
/// default production activation. It cannot outlive its source. `discard` drops
/// the owned output before releasing its exact credit; ordinary Drop releases
/// no ledger credit. No mutable graph, detached certificate or V12 conversion
/// is exposed.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionClosedScalarOutputHandoffV18;
/// fn duplicate(v: ProductionClosedScalarOutputHandoffV18<'_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionPreparedSourceV18, ProductionKernelArgumentAbiInputV18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(source: ProductionPreparedSourceV18, abi: ProductionKernelArgumentAbiInputV18<'_>, budget: &mut Budget<'_>) {
///     let _escaped = source.with_source_consumer_v18(budget, |view, budget| {
///         view.checked_closed_scalar_output_v18(abi, budget)
///     });
/// }
/// ```
#[must_use = "keep the paid output live or discard it on its original ledger"]
pub struct ProductionClosedScalarOutputHandoffV18<'view, 'source> {
    source: &'view ProductionSourceOwnedViewV18<'source>,
    output: fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
    receipt: fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ProductionClosedScalarOutputHandoffV18<'_, '_> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            self.source.cleanup.deny_refund();
            return self
                .source
                .retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.source
            .guard
            .observe_custody(self.source.cleanup, budget)
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let custody = self.custody(budget);
        // Observe denial even after a selected source refusal, but keep that
        // original refusal primary rather than replacing it with Accounting.
        self.source.check_query_v18(budget).and(custody)
    }

    /// Borrows the exact adopted output while its source and credit remain live.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_pliron::CheckedNeutralKernelIrOwnerV18> {
        self.check(budget)?;
        Ok(&self.output)
    }

    /// Requires the same original SSA owner, not equal source bytes.
    pub fn check_original_source(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.source.check_original_source(source, budget)
    }

    /// The exact graph and handoff credit retained on the continuing ledger.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        // The retained entrance checked this sum before its atomic reservation.
        Ok(self.output.storage().retained_storage() + self.receipt.retained_storage())
    }

    /// Drops the actual graph before refund; foreign or undercut custody refunds nothing.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let result = self.check(budget);
        let custody = self.custody(budget);
        let retained = self.output.storage().retained_storage() + self.receipt.retained_storage();
        let Self { source, output, .. } = self;
        drop(output);
        let settlement = custody.and_then(|()| {
            source.retain_query(budget.release_storage(retained).map_err(Into::into))
        });
        result?;
        settlement
    }
}

fn closed_scalar_handoff_credit_v18() -> Result<usize, ArgumentResourceV1> {
    // Stage A pays the checked owner's inline value and the live origin receipt.
    // Pay the containing handoff's additional fields/padding and alignment.
    let extra = size_of::<ProductionClosedScalarOutputHandoffV18<'_, '_>>()
        .checked_sub(size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerV18>())
        .and_then(|bytes| {
            bytes.checked_sub(size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>())
        })
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    argument_sum_v1(&[
        extra,
        std::mem::align_of::<ProductionClosedScalarOutputHandoffV18<'_, '_>>(),
    ])
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Constructs a closed scalar output/source handoff from this real source.
    /// The independent descriptor input must match the complete captured ABI.
    /// No callback, raw output or user-supplied completion claim is accepted.
    pub fn checked_closed_scalar_output_v18<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionClosedScalarOutputHandoffV18<'view, 'source>,
        ProductionClosedScalarHandoffErrorV18,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) =
            scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
                self.require_kernel_argument_abi_v18(abi, budget)?;
                closed_scalar_source_v18(self, budget)
                    .map_err(ProductionClosedScalarHandoffErrorV18::Check)?;
                let credit = closed_scalar_handoff_credit_v18()?;
                let envelopes = closed_scalar_native_envelopes_v18()?;
                self.retain_query(budget.reserve_storage(envelopes).map_err(Into::into))?;
                let output = self
                    .with_retained_checked_optimization_v18(
                        budget,
                        |original, optimized, budget| {
                            closed_scalar_final_checks_v18(original, optimized, budget)?;
                            // Stage A requires the returned payload credit to have been
                            // accepted before transferring it with the actual output.
                            original
                                .retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                            original
                                .retain_query(budget.release_storage(credit).map_err(Into::into))?;
                            Ok::<_, ProductionClosedScalarCheckErrorV18>(((), credit))
                        },
                    )
                    .map_err(ProductionClosedScalarHandoffErrorV18::Optimization)?;
                self.guard.check(self.owner, self.cleanup, budget)?;
                // Native diagnostics are prepaid outside the origin callback,
                // but are not part of the escaping handoff's retained credit.
                self.retain_query(budget.release_storage(envelopes).map_err(Into::into))?;
                Ok(output)
            })
            .map_err(|error| {
                // Header/preflight failures use this source's sticky refusal too.
                if let ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(resource),
                    ),
                ) = &error
                {
                    let _ = self.retain_query_resource_error_v18(*resource);
                }
                error
            })?;
        // No fallible operation or controlled allocation between the already
        // paid transfer and this move. The receipt sums were checked by entry.
        Ok(ProductionClosedScalarOutputHandoffV18 {
            source: self,
            output,
            receipt,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

fn closed_scalar_source_v18(
    source: &ProductionSourceOwnedViewV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionClosedScalarCheckErrorV18> {
    use ProductionClosedScalarCheckErrorV18::Unsupported;
    let semantic = source.source_semantic(budget)?;
    let launch = source.source_launch(budget)?;
    source.retain_query(budget.charge_work(5).map_err(Into::into))?;
    if !semantic.allocations().is_empty()
        || !semantic.statics().is_empty()
        || !semantic.vtables().is_empty()
    {
        return Err(Unsupported("memory objects"));
    }
    if semantic.functions().len() != semantic.roots().len() {
        return Err(Unsupported("non-root functions"));
    }
    if launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
        || launch.roots().len() != source.root_count(budget)?
    {
        return Err(source
            .missing::<()>("closed scalar launch/source identity")
            .unwrap_err()
            .into());
    }
    for ty in semantic.types() {
        source.retain_query(budget.charge_work(1).map_err(Into::into))?;
        if !matches!(
            ty.shape(),
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Scalar(_)
        ) {
            return Err(Unsupported("memory or non-scalar type"));
        }
    }
    for (ordinal, root) in launch.roots().iter().enumerate() {
        source.retain_query(budget.charge_work(4).map_err(Into::into))?;
        let (function, _) = source.root(ordinal, budget)?;
        let declaration = &semantic.functions()[function.index() as usize];
        if root.selected_root() != function
            || root.semantic_root_identity() != declaration.identity()
            || declaration.kernel_entry().is_none_or(|entry| {
                root.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
            })
        {
            return Err(source
                .missing::<()>("closed scalar root roster")
                .unwrap_err()
                .into());
        }
        if source.kernel_argument_abi_count(ordinal, budget)?
            != Some(declaration.abi().source_input_types().len())
        {
            return Err(source
                .missing::<()>("closed scalar complete ABI profile")
                .unwrap_err()
                .into());
        }
    }
    for function in semantic.functions() {
        source.retain_query(budget.charge_work(2).map_err(Into::into))?;
        if function.role() != SemanticFunctionRoleV1::KernelRoot {
            return Err(Unsupported("non-root function"));
        }
        // Inspect every terminator before the block-count gate so a real call
        // or assertion keeps its own explicit unsupported-obligation diagnostic.
        for block in function.blocks() {
            source.retain_query(budget.charge_work(1).map_err(Into::into))?;
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Return => (),
                SemanticTerminatorKindV1::Assert { .. } => return Err(Unsupported("assertion")),
                SemanticTerminatorKindV1::Call(_)
                | SemanticTerminatorKindV1::TailCall(_)
                | SemanticTerminatorKindV1::Drop { .. } => return Err(Unsupported("call")),
                _ => return Err(Unsupported("control flow")),
            }
            for statement in block.statements() {
                source.retain_query(budget.charge_work(1).map_err(Into::into))?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if !assignment.destination().projections().is_empty() {
                            return Err(Unsupported("projected assignment"));
                        }
                        let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                            return Err(Unsupported("non-copy scalar expression"));
                        };
                        match operand {
                            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                                if place.projections().is_empty() =>
                            {
                                ()
                            }
                            SemanticOperandV1::Constant(value)
                                if matches!(value.value(), SemanticConstantValueV1::Scalar(_)) =>
                            {
                                ()
                            }
                            _ => return Err(Unsupported("non-scalar operand")),
                        }
                    }
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => (),
                    SemanticStatementKindV1::Assume(_) => return Err(Unsupported("assertion")),
                    _ => return Err(Unsupported("memory statement")),
                }
            }
        }
        if function.blocks().len() != 1 {
            return Err(Unsupported("control flow"));
        }
    }
    Ok(())
}

fn closed_scalar_graph_v18(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionClosedScalarCheckErrorV18> {
    use ProductionClosedScalarCheckErrorV18::Unsupported;
    for function in &owner.module().functions {
        budget.charge_work(2)?;
        if function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
            return Err(Unsupported("output non-root function"));
        }
        let Some(body) = &function.body else {
            return Err(Unsupported("output declaration"));
        };
        if body.blocks.len() != 1 {
            return Err(Unsupported("output control flow"));
        }
        for block in &body.blocks {
            budget.charge_work(1)?;
            if !matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty())
            {
                return Err(Unsupported("output terminator"));
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                if !matches!(operation.kind, OperationKind::Constant(_)) {
                    return Err(Unsupported("output non-constant operation"));
                }
            }
        }
    }
    Ok(())
}

fn closed_scalar_native_envelopes_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<ProductionSourceNativeLifecycleErrorV18>(),
        size_of::<Result<(), ProductionSourceNativeLifecycleErrorV18>>(),
        size_of::<
            Result<
                Result<(), ProductionSourceNativeLifecycleErrorV18>,
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >(),
        2 * size_of::<std::cell::Cell<Option<ProductionSourceNativeLifecycleDiagnosticV18>>>(),
        size_of::<
            Result<
                ProductionSourceNativeLifecycleDiagnosticV18,
                ProductionSourceNativeLifecycleErrorV18,
            >,
        >(),
    ])
}

fn closed_scalar_final_checks_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionClosedScalarCheckErrorV18> {
    use ProductionClosedScalarCheckErrorV18 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        optimized_source_endpoints_v18(original, optimized, budget)?;
        closed_scalar_graph_v18(original.source.canonical(budget)?, budget)?;
        closed_scalar_graph_v18(optimized.output_inventory(budget)?.owner(), budget)?;
        for root in 0..original.source.root_count(budget)? {
            original.with_root_argument_data_v18(root, budget, |data, budget| {
                data.visit_nodes_scoped(budget, |_, _| Ok(()))
            })?;
            // The actual transition preserves complete signatures; join each
            // original ABI root to its exact indexed output declaration too.
            optimized_source_root_function_v18(original, optimized, root, budget)?;
        }
        original.check_optimized_source_currentness_v18(optimized, budget)?;
        let output = optimized.output_inventory(budget)?;
        let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
        let metadata_storage = metadata.storage_extent(budget).map_err(Error::Ranked)?;
        original.retain_query(budget.reserve_storage(metadata_storage).map_err(Into::into))?;
        let (candidate, receipt) = build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(Error::Ranked)?;
        original.retain_query(
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(Into::into),
        )?;
        let layouts = original.source.limits(budget)?.storage_layout_limits();
        let result = with_checked_canonical_ranked_view_v18(
            output,
            &metadata,
            &candidate,
            budget,
            |checked, budget| {
                Ok::<_, CanonicalRankedViewErrorV1>(optimized.with_lifecycle_native_policies_v18(
                    checked,
                    layouts,
                    budget,
                    |policies, budget| {
                        policies.check_source_subject_v18(original, optimized, budget)?;
                        for ordinal in 0..policies.function_count(budget)? {
                            let report = policies.report(ordinal, budget)?;
                            if report.is_none_or(|report| !report.is_clean())
                                || policies.history(ordinal, budget)?.is_none()
                            {
                                return Err(original
                                    .source
                                    .missing::<()>("closed scalar incomplete native report")
                                    .unwrap_err()
                                    .into());
                            }
                        }
                        Ok(())
                    },
                ))
            },
        )
        .map_err(Error::Ranked)?;
        drop(candidate);
        drop(metadata);
        result.map_err(Error::Native)?;
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let paid = argument_sum_v1(&[metadata_storage, receipt.retained_storage()])?;
        if scratch_floor.checked_add(paid) != Some(budget.storage()) {
            original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        original.retain_query(budget.release_storage(paid).map_err(Into::into))?;
        Ok(())
    })
}
