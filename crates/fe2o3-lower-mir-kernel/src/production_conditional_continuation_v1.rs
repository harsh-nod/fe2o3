/// The existing pending graph plus untrusted correspondence, never a second IR.
pub struct ProductionConditionalRootInputV1 {
    /// Consumed pending analysis; it supplies no ordinary clean-graph evidence.
    pub pending: fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    /// Proposed semantic kernel root, checked against the original source owner.
    pub semantic_root: u32,
    /// Proposed launch rank, checked during source correspondence replay.
    pub launch_rank: u8,
    /// Proposed source occurrences for ranked memory accesses.
    pub access_sources: Vec<ProductionRankedAccessSourceV1>,
    /// Proposed source occurrences for executable non-memory effects.
    pub executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    /// Diagnostic ranked text; never a substitute for the retained live graph.
    pub ranked_ir: String,
    /// Inert CPU subjects requiring authentication by the backend reference join.
    pub reference_subjects: fe2o3_pliron::ProductionConditionalReferenceSubjectsV1,
}

impl ProductionConditionalRootInputV1 {
    /// Exact retained arena and source-buffer capacity charge. This excludes
    /// the source owner, derived conditional facts, runtime and proof receipts.
    pub fn retained_storage_v1(
        &self,
    ) -> Result<usize, fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        self.access_sources
            .capacity()
            .checked_mul(std::mem::size_of::<ProductionRankedAccessSourceV1>())
            .and_then(|bytes| {
                self.executable_effect_sources
                    .capacity()
                    .checked_mul(std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>())
                    .and_then(|effects| bytes.checked_add(effects))
            })
            .and_then(|bytes| bytes.checked_add(self.ranked_ir.capacity()))
            .and_then(|bytes| bytes.checked_add(self.pending.retained_analysis_storage_v1()))
            .ok_or(Resource::Arithmetic)
    }
}

/// Exact argument correspondence for premise and reference/descriptor consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalSourceArgumentV1 {
    canonical_parameter: u32,
    source_argument: u32,
    adjusted_argument: u32,
    semantic_local: SemanticLocalIdV1,
    semantic_type: SemanticTypeIdV1,
}

impl ProductionConditionalSourceArgumentV1 {
    /// Parameter ordinal in the verified canonical function.
    pub const fn canonical_parameter(self) -> u32 {
        self.canonical_parameter
    }
    /// Argument ordinal in the original source signature.
    pub const fn source_argument(self) -> u32 {
        self.source_argument
    }
    /// Argument ordinal after checked source ABI adjustment.
    pub const fn adjusted_argument(self) -> u32 {
        self.adjusted_argument
    }
    /// Source semantic local carrying the whole argument.
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.semantic_local
    }
    /// Source semantic type of that whole argument.
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.semantic_type
    }
}

/// A refusal to continue the original source-bound conditional graph.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProductionConditionalContinuationErrorV1 {
    /// The original work or storage account refused the operation.
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    /// Ranked correspondence does not replay against the source owner.
    Source(ProductionConditionalSourceTranslationErrorV1),
    /// Canonical conditional coverage could not be established.
    Canonical(fe2o3_kernel_ir::ConditionalTotalViewErrorV1),
    /// The canonical output does not bind to a whole source argument.
    Binding(ProductionConditionalOutputBindingErrorV1),
    /// The ranked output or read occurrences fail source binding.
    Ranked(ProductionConditionalRankedOutputErrorV1),
    /// Ranked control flow does not establish the required output coverage.
    Coverage(ProductionConditionalRankedCoverageErrorV1),
    /// The final graph or conditional aggregate subject was rejected.
    Aggregate(fe2o3_pliron::ProductionConditionalAggregateErrorV1),
    /// A mandatory final-graph check failed; its retained arena was discarded.
    PipelineRejected,
    /// A required subject association is absent or unsupported.
    Subject(&'static str),
}
impl From<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>
    for ProductionConditionalContinuationErrorV1
{
    fn from(error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionConditionalContinuationErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional continuation: {self:?}")
    }
}
impl std::error::Error for ProductionConditionalContinuationErrorV1 {}

fn conditional_aggregate_error_v1(
    error: fe2o3_pliron::ProductionConditionalAggregateErrorV1,
) -> ProductionConditionalContinuationErrorV1 {
    // The pipeline error retains the consumed arena. Drop it before releasing
    // its enclosing reservation; the caller's work/denial history remains live.
    match error {
        fe2o3_pliron::ProductionConditionalAggregateErrorV1::Pipeline(error) => {
            drop(error);
            ProductionConditionalContinuationErrorV1::PipelineRejected
        }
        error => ProductionConditionalContinuationErrorV1::Aggregate(error),
    }
}

enum ConditionalContinuationGraphV1<'source> {
    Final(fe2o3_pliron::ProductionConditionalFinalGraphV1<'source>),
    Staged(fe2o3_pliron::ProductionConditionalAggregateStateV1<'source>),
}

// Both entry points lend the same account; this adapter cannot recreate its
// work history, alter its identity, or convert a borrowed budget to an owner.
trait ConditionalOriginalLedgerV1 {
    fn storage(&self) -> usize;
    fn visit_budget(&mut self, visit: &mut dyn FnMut(&mut ArgumentBudgetV1<'_>));
}

impl ConditionalOriginalLedgerV1
    for fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1
{
    fn storage(&self) -> usize {
        self.storage()
    }
    fn visit_budget(&mut self, visit: &mut dyn FnMut(&mut ArgumentBudgetV1<'_>)) {
        self.with_budget(visit);
    }
}

impl ConditionalOriginalLedgerV1 for ArgumentBudgetV1<'_> {
    fn storage(&self) -> usize {
        self.storage()
    }
    fn visit_budget(&mut self, visit: &mut dyn FnMut(&mut ArgumentBudgetV1<'_>)) {
        visit(self);
    }
}

impl dyn ConditionalOriginalLedgerV1 + '_ {
    fn with_budget<R>(&mut self, consume: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> R) -> R {
        let mut consume = Some(consume);
        let mut result = None;
        self.visit_budget(&mut |budget| {
            result = Some(consume.take().expect("one original account visit")(budget));
        });
        result.expect("original account was visited")
    }
}

fn with_conditional_original_budget_v1<R>(
    ledger: &mut dyn ConditionalOriginalLedgerV1,
    account: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    protected: usize,
    poisoned: &mut bool,
    consume: impl FnOnce(
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionConditionalContinuationErrorV1>,
) -> Result<R, ProductionConditionalContinuationErrorV1> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    ledger.with_budget(|budget| {
        if *poisoned || budget.work_ledger_identity_v1() != account || budget.storage() < protected
        {
            *poisoned = true;
            return Err(Resource::Accounting.into());
        }
        let floor = budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(budget)));
        if budget.work_ledger_identity_v1() != account || budget.storage() < floor {
            *poisoned = true;
        }
        match result {
            Ok(_) if *poisoned => Err(Resource::Accounting.into()),
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })
}

/// A consuming root continuation borrowing the original canonical owner and
/// account. There is no graph mutation, ordinary clean conversion or launch API.
pub struct ProductionConditionalFinalRootV1<'source, 'ledger> {
    source: &'source ProductionPreRankedKirOwnerV1,
    graph: Option<ConditionalContinuationGraphV1<'source>>,
    arguments: Vec<ProductionConditionalSourceArgumentV1>,
    semantic_root: u32,
    launch_rank: u8,
    access_sources: Vec<ProductionRankedAccessSourceV1>,
    executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    ranked_ir: String,
    ledger: &'ledger mut dyn ConditionalOriginalLedgerV1,
    reserved: usize,
    account: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    poisoned: bool,
}

/// A staged conditional graph retaining the original source and resource account.
pub struct ProductionConditionalAggregateStageV1<'source, 'ledger> {
    root: ProductionConditionalFinalRootV1<'source, 'ledger>,
}

/// Callback-scoped source correspondence and explicit conditional formula input.
/// This request conveys neither an aggregate proof nor GPU launch authority.
pub struct ProductionSourceBoundConditionalAggregateRequestV1<'a> {
    translation: &'a ProductionConditionalSourceTranslationV1<'a>,
    pliron: &'a fe2o3_pliron::ProductionConditionalAggregateInputV1<'a>,
    arguments: &'a [ProductionConditionalSourceArgumentV1],
}

impl<'a> ProductionSourceBoundConditionalAggregateRequestV1<'a> {
    /// The original production source owner, not a reconstructed capsule.
    pub fn source(&self) -> &'a ProductionPreRankedKirOwnerV1 {
        self.translation.source()
    }
    /// Freshly checked correspondence with the retained conditional graph.
    pub const fn translation(&self) -> &'a ProductionConditionalSourceTranslationV1<'a> {
        self.translation
    }
    /// The exact live graph and explicit premises for aggregate formula replay.
    pub const fn pliron_input(
        &self,
    ) -> &'a fe2o3_pliron::ProductionConditionalAggregateInputV1<'a> {
        self.pliron
    }
    /// Checked canonical-to-source argument correspondence for outputs and reads.
    pub const fn arguments(&self) -> &'a [ProductionConditionalSourceArgumentV1] {
        self.arguments
    }
}

/// All new checks use the original account. Preexisting source replay and arena
/// allocations retain their documented domains; the pending analysis storage
/// receipt and moved input buffers' capacities are adopted once here. Failed
/// work is never refunded.
#[allow(clippy::result_large_err)]
pub fn continue_conditional_root_v1<'source, 'ledger>(
    source: &'source ProductionPreRankedKirOwnerV1,
    input: ProductionConditionalRootInputV1,
    ledger: &'ledger mut fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1,
) -> Result<
    ProductionConditionalFinalRootV1<'source, 'ledger>,
    ProductionConditionalContinuationErrorV1,
> {
    continue_conditional_root_with_ledger_v1(source, input, ledger)
}

fn continue_conditional_root_with_ledger_v1<'source, 'ledger>(
    source: &'source ProductionPreRankedKirOwnerV1,
    input: ProductionConditionalRootInputV1,
    ledger: &'ledger mut dyn ConditionalOriginalLedgerV1,
) -> Result<
    ProductionConditionalFinalRootV1<'source, 'ledger>,
    ProductionConditionalContinuationErrorV1,
> {
    use ProductionConditionalContinuationErrorV1 as E;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewAnalysisV1,
    };
    let input_storage = input.retained_storage_v1()?;
    let ProductionConditionalRootInputV1 {
        pending,
        semantic_root,
        launch_rank,
        access_sources,
        executable_effect_sources,
        ranked_ir,
        reference_subjects,
    } = input;
    let floor = ledger.storage();
    let account = ledger.with_budget(|budget| budget.work_ledger_identity_v1());
    let result = ledger.with_budget(|budget| {
        budget.charge_work(8)?;
        if budget.storage() < source.retained_analysis_storage_v1() {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(
            input_storage
                .checked_add(std::mem::size_of::<ProductionConditionalFinalRootV1<'_, '_>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
            semantic_root,
            launch_rank,
            pending.kernel().map_err(|_| E::Subject("pending graph"))?,
            &access_sources,
            &executable_effect_sources,
            &ranked_ir,
        );
        source
            .check_conditional_source_translation_v1(&pending, candidate, budget)
            .map_err(E::Source)?;
        let mut association = None;
        for row in source.correspondence.lowered_functions() {
            budget.charge_work(3)?;
            if row.role() == SemanticKirFunctionRoleV1::KernelEntry
                && row.correspondence_owner().index() == semantic_root
                && association.replace(row).is_some()
            {
                return Err(E::Subject("source root association"));
            }
        }
        let association = association.ok_or(E::Subject("source root association"))?;
        let mut kernel = None;
        for row in &source.executable.module().kernels {
            budget.charge_work(
                row.entry
                    .as_str()
                    .len()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if &row.entry == association.kernel_ir_function() && kernel.replace(row).is_some() {
                return Err(E::Subject("canonical root association"));
            }
        }
        let kernel = kernel.ok_or(E::Subject("canonical root association"))?;
        let facts = match fe2o3_kernel_ir::derive_conditional_total_view_from_verified_v1(
            source.executable.verified_module_ref_v1(),
            &kernel.id,
            budget,
        )
        .map_err(E::Canonical)?
        {
            ConditionalTotalViewAnalysisV1::Established(facts) => facts,
            ConditionalTotalViewAnalysisV1::Unsupported(_) => {
                return Err(E::Subject("unsupported canonical conditional output"));
            }
        };
        let binding = source
            .bind_conditional_output_v1(facts, budget)
            .map_err(E::Binding)?;
        let (index, extent, write) = {
            let coverage = binding
                .inspect_ranked_output_v1(candidate, budget)
                .map_err(E::Ranked)?
                .check_ranked_coverage_v1(budget)
                .map_err(E::Coverage)?;
            let output = coverage.output();
            let extent = match output.dynamic_extent() {
                ProductionConditionalRankedExtentV1::CanonicalOutputLength { operand, .. } => {
                    operand
                }
                _ => return Err(E::Subject("unbound output extent")),
            };
            (output.ranked_index(), extent, output.gpu_write_site())
        };
        let count = binding.coverage().read_count();
        let mut reads = conditional_continuation_vec_v1::<
            fe2o3_kernel_ir::ConditionalTotalViewReadV1,
        >(count, budget)?;
        binding
            .coverage()
            .visit_reads_v1(budget, |read| {
                if reads.len() == reads.capacity() {
                    return Err(Resource::Accounting);
                }
                reads.push(read);
                Ok(())
            })
            .map_err(E::Canonical)?;
        let mut read_sites = conditional_continuation_vec_v1(count, budget)?;
        let mut arguments = conditional_continuation_vec_v1(
            count.checked_add(1).ok_or(Resource::Arithmetic)?,
            budget,
        )?;
        arguments.push(ProductionConditionalSourceArgumentV1 {
            canonical_parameter: binding.coverage().output_parameter_index(),
            source_argument: binding.source_argument(),
            adjusted_argument: binding.adjusted_argument(),
            semantic_local: binding.semantic_local(),
            semantic_type: binding.semantic_type(),
        });
        for read in &reads {
            let (ranked_source, _) =
                production_conditional_ranked_output_v1::checked_read_source_v1(
                    &binding, candidate, *read, index, budget,
                )
                .map_err(E::Ranked)?;
            read_sites.push(fe2o3_pliron::ProductionConditionalReadProposalV1 {
                canonical: read.location(),
                block: ranked_source.ranked_block(),
                operation: ranked_source.ranked_operation(),
            });
            let argument = source
                .with_checked_arguments_v1(
                    association.correspondence_owner(),
                    association.semantic_function(),
                    budget,
                    |view| {
                        conditional_output_argument_v1(
                            view,
                            read.parameter() as usize,
                            read.slice(),
                        )
                    },
                )
                .map_err(|error| E::Binding(error.into()))?
                .ok_or(E::Subject("input source argument"))?;
            arguments.push(ProductionConditionalSourceArgumentV1 {
                canonical_parameter: read.parameter(),
                source_argument: argument.source,
                adjusted_argument: argument.adjusted,
                semantic_local: argument.local,
                semantic_type: argument.ty,
            });
        }
        let read_storage = reads
            .capacity()
            .checked_mul(std::mem::size_of::<
                fe2o3_kernel_ir::ConditionalTotalViewReadV1,
            >())
            .ok_or(Resource::Arithmetic)?;
        drop(reads);
        budget.release_storage(read_storage)?;
        let proposal_storage = read_sites
            .capacity()
            .checked_mul(std::mem::size_of::<
                fe2o3_pliron::ProductionConditionalReadProposalV1,
            >())
            .ok_or(Resource::Arithmetic)?;
        let graph = pending
            .verify_conditional_final_graph_v1(
                binding.facts,
                fe2o3_pliron::ProductionConditionalRankedProposalV1 {
                    index,
                    extent,
                    write,
                    reads: read_sites,
                },
                *source.semantic_ssa.source_semantic_sha256(),
                reference_subjects,
                budget,
            )
            .map_err(conditional_aggregate_error_v1)?;
        budget.release_storage(proposal_storage)?;
        Ok((graph, arguments))
    });
    match result {
        Ok((graph, arguments)) => {
            let reserved = ledger
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok(ProductionConditionalFinalRootV1 {
                source,
                graph: Some(ConditionalContinuationGraphV1::Final(graph)),
                arguments,
                semantic_root,
                launch_rank,
                access_sources,
                executable_effect_sources,
                ranked_ir,
                ledger,
                reserved,
                account,
                floor,
                poisoned: false,
            })
        }
        Err(error) => {
            drop(access_sources);
            drop(executable_effect_sources);
            drop(ranked_ir);
            ledger.with_budget(|budget| {
                if budget.work_ledger_identity_v1() != account {
                    return Err(Resource::Accounting);
                }
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(Resource::Accounting)?,
                )
            })?;
            Err(error)
        }
    }
}

fn conditional_continuation_vec_v1<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as R;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(R::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|_| R::Allocation)?;
    budget.reserve_storage(
        rows.capacity()
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|actual| actual.checked_sub(bytes))
            .ok_or(R::Accounting)?,
    )?;
    Ok(rows)
}

impl<'source, 'ledger> ProductionConditionalFinalRootV1<'source, 'ledger> {
    fn protected_storage_v1(
        &self,
    ) -> Result<usize, fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
        self.floor
            .checked_add(self.reserved)
            .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
    }

    fn transfer_input_storage_v1(
        &mut self,
        retained: usize,
    ) -> Result<(), fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        let reserved = self
            .reserved
            .checked_sub(retained)
            .ok_or(Resource::Accounting)?;
        let floor = self
            .floor
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        self.reserved = reserved;
        self.floor = floor;
        Ok(())
    }

    /// Consumes the checked root and binds its conditional aggregate identity.
    #[allow(clippy::result_large_err)]
    pub fn into_aggregate_stage_v1(
        mut self,
    ) -> Result<
        ProductionConditionalAggregateStageV1<'source, 'ledger>,
        ProductionConditionalContinuationErrorV1,
    > {
        let Some(ConditionalContinuationGraphV1::Final(graph)) = self.graph.take() else {
            return Err(ProductionConditionalContinuationErrorV1::Subject(
                "aggregate stage order",
            ));
        };
        let before = self.ledger.storage();
        let protected = self.protected_storage_v1()?;
        let state = with_conditional_original_budget_v1(
            self.ledger,
            self.account,
            protected,
            &mut self.poisoned,
            |budget| {
                graph
                    .into_aggregate_state_v1(budget)
                    .map_err(conditional_aggregate_error_v1)
            },
        );
        let added = self
            .ledger
            .storage()
            .checked_sub(before)
            .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        self.reserved = self
            .reserved
            .checked_add(added)
            .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        self.graph = Some(ConditionalContinuationGraphV1::Staged(state?));
        Ok(ProductionConditionalAggregateStageV1 { root: self })
    }
}

impl ProductionConditionalAggregateStageV1<'_, '_> {
    /// Rechecks source and graph identity, then lends the request and original
    /// account to the consumer. Work remains charged on success and failure.
    #[allow(clippy::result_large_err)]
    pub fn with_request_v1<R>(
        &mut self,
        consume: impl for<'a> FnOnce(
            &ProductionSourceBoundConditionalAggregateRequestV1<'a>,
            &mut ArgumentBudgetV1<'_>,
        ) -> R,
    ) -> Result<R, ProductionConditionalContinuationErrorV1> {
        let root = &mut self.root;
        let protected = root.protected_storage_v1()?;
        let Some(ConditionalContinuationGraphV1::Staged(state)) = &root.graph else {
            return Err(ProductionConditionalContinuationErrorV1::Subject(
                "aggregate stage order",
            ));
        };
        with_conditional_original_budget_v1(
            root.ledger,
            root.account,
            protected,
            &mut root.poisoned,
            |budget| {
                let pending = state.pending_analysis();
                let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
                    root.semantic_root,
                    root.launch_rank,
                    pending.kernel().map_err(|_| {
                        ProductionConditionalContinuationErrorV1::Subject("pending graph")
                    })?,
                    &root.access_sources,
                    &root.executable_effect_sources,
                    &root.ranked_ir,
                );
                let translation = root
                    .source
                    .check_conditional_source_translation_v1(pending, candidate, budget)
                    .map_err(ProductionConditionalContinuationErrorV1::Source)?;
                state
                    .with_input_v1(budget, |pliron, budget| {
                        consume(
                            &ProductionSourceBoundConditionalAggregateRequestV1 {
                                translation: &translation,
                                pliron,
                                arguments: &root.arguments,
                            },
                            budget,
                        )
                    })
                    .map_err(conditional_aggregate_error_v1)
            },
        )
    }
}

/// Consumes the existing pending arena using a borrowed view of the original
/// account, replays source and final-graph checks, and lends the staged request.
///
/// The returned input is still pending, not clean or authenticated lowering.
/// Its arena and moved source-buffer reservations remain charged on `budget`;
/// only derived conditional facts are released. The caller must retain those
/// reservations for the returned input's lifetime within its accounting phase.
/// Callback-owned reservations are not released here. Failed work is retained.
#[allow(clippy::result_large_err)]
pub fn with_conditional_root_request_v1<R>(
    source: &ProductionPreRankedKirOwnerV1,
    input: ProductionConditionalRootInputV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'a> FnOnce(
        &ProductionSourceBoundConditionalAggregateRequestV1<'a>,
        &mut ArgumentBudgetV1<'_>,
    ) -> R,
) -> Result<(R, ProductionConditionalRootInputV1), ProductionConditionalContinuationErrorV1> {
    use ProductionConditionalContinuationErrorV1 as E;
    let reference_subjects = input.reference_subjects;
    let mut stage = continue_conditional_root_with_ledger_v1(source, input, budget)?
        .into_aggregate_stage_v1()?;
    let result = stage.with_request_v1(consume)?;
    let root = &mut stage.root;
    let Some(ConditionalContinuationGraphV1::Staged(graph)) = root.graph.take() else {
        return Err(E::Subject("conditional arena return stage order"));
    };
    let protected = root.protected_storage_v1()?;
    let pending = with_conditional_original_budget_v1(
        root.ledger,
        root.account,
        protected,
        &mut root.poisoned,
        |budget| {
            graph
                .into_pending_analysis_v1(budget)
                .map_err(conditional_aggregate_error_v1)
        },
    )?;
    let input = ProductionConditionalRootInputV1 {
        pending,
        semantic_root: root.semantic_root,
        launch_rank: root.launch_rank,
        access_sources: std::mem::take(&mut root.access_sources),
        executable_effect_sources: std::mem::take(&mut root.executable_effect_sources),
        ranked_ir: std::mem::take(&mut root.ranked_ir),
        reference_subjects,
    };
    root.transfer_input_storage_v1(input.retained_storage_v1()?)?;
    drop(stage);
    Ok((result, input))
}

impl Drop for ProductionConditionalFinalRootV1<'_, '_> {
    fn drop(&mut self) {
        drop(self.graph.take());
        drop(std::mem::take(&mut self.arguments));
        drop(std::mem::take(&mut self.access_sources));
        drop(std::mem::take(&mut self.executable_effect_sources));
        drop(std::mem::take(&mut self.ranked_ir));
        // Never debit a substituted account or a damaged enclosing floor.
        // Reservations owned by a consumer callback are not part of `reserved`.
        let protected = self.protected_storage_v1();
        self.ledger.with_budget(|budget| {
            if !self.poisoned
                && budget.work_ledger_identity_v1() == self.account
                && protected.is_ok_and(|floor| budget.storage() >= floor)
            {
                let _ = budget.release_storage(self.reserved);
            }
        });
    }
}
