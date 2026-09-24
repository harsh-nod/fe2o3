/// The existing pending graph plus untrusted correspondence, never a second IR.
pub struct ProductionConditionalRootInputV1 {
    pub pending: fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    pub semantic_root: u32,
    pub launch_rank: u8,
    pub access_sources: Vec<ProductionRankedAccessSourceV1>,
    pub executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    pub ranked_ir: String,
    pub reference_subjects: fe2o3_pliron::ProductionConditionalReferenceSubjectsV1,
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
    pub const fn canonical_parameter(self) -> u32 {
        self.canonical_parameter
    }
    pub const fn source_argument(self) -> u32 {
        self.source_argument
    }
    pub const fn adjusted_argument(self) -> u32 {
        self.adjusted_argument
    }
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.semantic_local
    }
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.semantic_type
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProductionConditionalContinuationErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    Source(ProductionConditionalSourceTranslationErrorV1),
    Canonical(fe2o3_kernel_ir::ConditionalTotalViewErrorV1),
    Binding(ProductionConditionalOutputBindingErrorV1),
    Ranked(ProductionConditionalRankedOutputErrorV1),
    Coverage(ProductionConditionalRankedCoverageErrorV1),
    Aggregate(fe2o3_pliron::ProductionConditionalAggregateErrorV1),
    PipelineRejected,
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
    ledger: &'ledger mut fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1,
    reserved: usize,
}

pub struct ProductionConditionalAggregateStageV1<'source, 'ledger> {
    root: ProductionConditionalFinalRootV1<'source, 'ledger>,
}

pub struct ProductionSourceBoundConditionalAggregateRequestV1<'a> {
    translation: &'a ProductionConditionalSourceTranslationV1<'a>,
    pliron: &'a fe2o3_pliron::ProductionConditionalAggregateInputV1<'a>,
    arguments: &'a [ProductionConditionalSourceArgumentV1],
}

impl<'a> ProductionSourceBoundConditionalAggregateRequestV1<'a> {
    pub fn source(&self) -> &'a ProductionPreRankedKirOwnerV1 {
        self.translation.source()
    }
    pub const fn translation(&self) -> &'a ProductionConditionalSourceTranslationV1<'a> {
        self.translation
    }
    pub const fn pliron_input(
        &self,
    ) -> &'a fe2o3_pliron::ProductionConditionalAggregateInputV1<'a> {
        self.pliron
    }
    pub const fn arguments(&self) -> &'a [ProductionConditionalSourceArgumentV1] {
        self.arguments
    }
}

/// All new checks use the original account. Preexisting source replay and arena
/// allocations retain their documented domains; the pending analysis storage
/// receipt is adopted once here. Failed work is never refunded.
#[allow(clippy::result_large_err)]
pub fn continue_conditional_root_v1<'source, 'ledger>(
    source: &'source ProductionPreRankedKirOwnerV1,
    input: ProductionConditionalRootInputV1,
    ledger: &'ledger mut fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1,
) -> Result<
    ProductionConditionalFinalRootV1<'source, 'ledger>,
    ProductionConditionalContinuationErrorV1,
> {
    use ProductionConditionalContinuationErrorV1 as E;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewAnalysisV1,
    };
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
    let result = ledger.with_budget(|budget| {
        budget.charge_work(8)?;
        if budget.storage() < source.retained_analysis_storage_v1() {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(
            pending
                .retained_analysis_storage_v1()
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
            })
        }
        Err(error) => {
            ledger.with_budget(|budget| {
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
) -> Result<Vec<T>, ProductionConditionalContinuationErrorV1> {
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
        let state = self
            .ledger
            .with_budget(|budget| graph.into_aggregate_state_v1(budget))
            .map_err(conditional_aggregate_error_v1);
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
    #[allow(clippy::result_large_err)]
    pub fn with_request_v1<R>(
        &mut self,
        consume: impl for<'a> FnOnce(
            &ProductionSourceBoundConditionalAggregateRequestV1<'a>,
            &mut ArgumentBudgetV1<'_>,
        ) -> R,
    ) -> Result<R, ProductionConditionalContinuationErrorV1> {
        let root = &mut self.root;
        let Some(ConditionalContinuationGraphV1::Staged(state)) = &root.graph else {
            return Err(ProductionConditionalContinuationErrorV1::Subject(
                "aggregate stage order",
            ));
        };
        root.ledger.with_budget(|budget| {
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
            let account = budget.work_ledger_identity_v1();
            let floor = budget.storage();
            let result = state
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
                .map_err(conditional_aggregate_error_v1);
            if budget.work_ledger_identity_v1() != account || budget.storage() < floor {
                return Err(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                        .into(),
                );
            }
            result
        })
    }
}

impl Drop for ProductionConditionalFinalRootV1<'_, '_> {
    fn drop(&mut self) {
        drop(self.graph.take());
        drop(std::mem::take(&mut self.arguments));
        // Reservations owned by a consumer callback are not part of `reserved`.
        let _ = self
            .ledger
            .with_budget(|budget| budget.release_storage(self.reserved));
    }
}
