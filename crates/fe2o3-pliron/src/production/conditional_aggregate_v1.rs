//! Consuming final-graph verification and explicitly conditional aggregate input.
//! The statement is an implication. It is never unconditional V5 evidence.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    ConditionalTotalViewAddressDomainV1 as Domain, ConditionalTotalViewFactsV1,
    ConditionalTotalViewReadV1, FunctionOperationLocation,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

pub use fe2o3_functional_proof::FunctionalRefinementSubjectsV2 as ProductionConditionalReferenceSubjectsV1;

/// Closed, host-dischargeable premises, not a slot for caller-authored theorems.
/// Parameter numbers are canonical parameters, not CPU or host ABI ordinals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionConditionalRuntimePremiseV1 {
    /// A well-formed D1 launch, with unique global-X invocations in `[0, G)`.
    D1Launch,
    /// Logical output element count `N <= G`, including `N == 0`.
    OutputWithinGlobalX { parameter: u32 },
    /// The complete logical output byte span is aligned, live and writable.
    /// The empty span requires no dereference; address formation is separate.
    WritableOutput { parameter: u32 },
    /// The input is aligned, live and readable for `[0, N)` (GuardedOutput)
    /// or `[0, G)` (GlobalLaunch), with its logical length at least that bound.
    ReadableInput { parameter: u32, domain: Domain },
    /// Complete logical input/output byte spans are disjoint. Each uses its
    /// own length and element width with checked byte arithmetic. Empty spans
    /// are disjoint; read-only inputs need not be disjoint from one another.
    SeparateInputOutput { input: u32, output: u32 },
    /// Target index/offset/base-plus-offset arithmetic is representable and
    /// aligned. For GlobalLaunch and G > 0 this includes `(G - 1) * width`,
    /// even if N == 0. For GuardedOutput it includes output-bounded offsets
    /// and the selected zero offset on a tail. G == 0 is empty, without G - 1.
    /// This premise requires no readable/writable launch-tail allocation.
    RepresentableAddress {
        parameter: u32,
        domain: Domain,
        element_bytes: u64,
        alignment: u32,
    },
}

/// Inert coordinates to be reconciled with canonical facts and the live recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalReadProposalV1 {
    pub canonical: FunctionOperationLocation,
    pub block: u32,
    pub operation: u32,
}

pub struct ProductionConditionalRankedProposalV1 {
    pub index: ProductionRankedValueV1,
    pub extent: ProductionRankedValueV1,
    pub write: ProductionGpuWriteSiteV2,
    pub reads: Vec<ProductionConditionalReadProposalV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalOutputBindingV1 {
    parameter: u32,
    index: ProductionRankedValueV1,
    extent: ProductionRankedValueV1,
    view: ProductionRankedValueV1,
    write: ProductionGpuWriteSiteV2,
    ownership: ProductionConditionalOwnershipSiteV1,
    effect: (u32, u32),
    exits: [u32; 2],
}

impl ProductionConditionalOutputBindingV1 {
    pub const fn canonical_parameter(self) -> u32 {
        self.parameter
    }
    pub const fn index(self) -> ProductionRankedValueV1 {
        self.index
    }
    pub const fn extent(self) -> ProductionRankedValueV1 {
        self.extent
    }
    pub const fn view(self) -> ProductionRankedValueV1 {
        self.view
    }
    pub const fn write(self) -> ProductionGpuWriteSiteV2 {
        self.write
    }
    pub const fn ownership(self) -> ProductionConditionalOwnershipSiteV1 {
        self.ownership
    }
    pub const fn effect_site(self) -> (u32, u32) {
        self.effect
    }
    /// False-case exit followed by true-case exit. Empty output is not excluded.
    pub const fn normal_exits(self) -> [u32; 2] {
        self.exits
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalReadBindingV1 {
    canonical: ConditionalTotalViewReadV1,
    site: ProductionConditionalReadProposalV1,
    view: ProductionRankedValueV1,
    index: ProductionRankedValueV1,
}

impl ProductionConditionalReadBindingV1 {
    pub const fn canonical(self) -> ConditionalTotalViewReadV1 {
        self.canonical
    }
    pub const fn site(self) -> ProductionConditionalReadProposalV1 {
        self.site
    }
    pub const fn view(self) -> ProductionRankedValueV1 {
        self.view
    }
    pub const fn index(self) -> ProductionRankedValueV1 {
        self.index
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProductionConditionalAggregateErrorV1 {
    Resource(ResourceError),
    Canonical(fe2o3_kernel_ir::ConditionalTotalViewErrorV1),
    Coverage(crate::ProductionRankedRecipeCoverageErrorV1),
    Session(ProductionSessionErrorV1),
    Pipeline(ProductionConditionalPipelineErrorV1),
    Subject(&'static str),
}
impl From<ResourceError> for ProductionConditionalAggregateErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionConditionalAggregateErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional aggregate: {self:?}")
    }
}
impl Error for ProductionConditionalAggregateErrorV1 {}

/// Owns the two actual final-graph check runs, never an ordinary clean stage.
pub struct ProductionConditionalFinalGraphV1<'canonical> {
    canonical: ConditionalTotalViewFactsV1<'canonical>,
    output: [ProductionConditionalOutputBindingV1; 1],
    reads: Vec<ProductionConditionalReadBindingV1>,
    premises: Vec<ProductionConditionalRuntimePremiseV1>,
    source_semantic_identity: DigestV1,
    reference_subjects: ProductionConditionalReferenceSubjectsV1,
    pipeline: ProductionConditionalPipelineAnalysisV1,
}

/// Retains the final graph through aggregate preparation and verifier replay.
pub struct ProductionConditionalAggregateStateV1<'canonical> {
    identity: DigestV1,
    graph: ProductionConditionalFinalGraphV1<'canonical>,
}

/// Exact borrowed input to the conditional aggregate formula consumer. CPU
/// subject data must still be authenticated by the backend reference join.
pub struct ProductionConditionalAggregateInputV1<'a> {
    state: &'a ProductionConditionalAggregateStateV1<'a>,
}

/// Shared graph view for formula replay. The conditional variant intentionally
/// supplies no fabricated ordinary lowering input or V5 report.
#[derive(Clone, Copy)]
pub enum ProductionFinalRankedSubjectV1<'a> {
    Unconditional {
        ranked: &'a ProductionRankedKernelLoweringInputV1,
        evidence: &'a ProductionMiddleEndEvidenceV5,
    },
    Conditional(&'a ProductionConditionalAggregateInputV1<'a>),
}

impl<'a> ProductionFinalRankedSubjectV1<'a> {
    pub fn kernel(self) -> &'a ProductionRankedKernelV1 {
        match self {
            Self::Unconditional { ranked, .. } => ranked.kernel(),
            Self::Conditional(input) => input.kernel(),
        }
    }
    pub fn exact_graph_identity(self) -> ProductionExactGraphIdentityV1 {
        match self {
            Self::Unconditional { ranked, .. } => ranked.exact_graph_identity(),
            Self::Conditional(input) => input.exact_graph_identity(),
        }
    }
    pub fn graph_snapshot(self) -> OperationGraphSnapshotV1 {
        match self {
            Self::Unconditional { ranked, .. } => ranked.graph_snapshot(),
            Self::Conditional(input) => input.graph_snapshot(),
        }
    }
    pub fn premises(self) -> &'a [ProductionConditionalRuntimePremiseV1] {
        match self {
            Self::Unconditional { .. } => &[],
            Self::Conditional(input) => input.premises(),
        }
    }
}

fn reserve_rows<T>(
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<Vec<T>, ProductionConditionalAggregateErrorV1> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(ResourceError::Accounting)?)?;
    Ok(rows)
}

impl ProductionConditionalRankedAnalysisV1 {
    #[allow(clippy::result_large_err)]
    pub fn verify_conditional_final_graph_v1<'canonical>(
        self,
        canonical: ConditionalTotalViewFactsV1<'canonical>,
        proposal: ProductionConditionalRankedProposalV1,
        source_semantic_identity: [u8; 32],
        reference_subjects: ProductionConditionalReferenceSubjectsV1,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionConditionalFinalGraphV1<'canonical>, ProductionConditionalAggregateErrorV1>
    {
        use ProductionConditionalAggregateErrorV1 as E;
        budget.charge_work(16)?;
        if source_semantic_identity == [0; 32] || proposal.reads.len() != canonical.read_count() {
            return Err(E::Subject("canonical/source/read roster"));
        }
        budget.reserve_storage(std::mem::size_of::<ProductionConditionalFinalGraphV1<'_>>())?;
        let kernel = self.kernel().map_err(E::Session)?;
        let [ownership] = self.selections() else {
            return Err(E::Subject("conditional output roster"));
        };
        let mut selected = None;
        let mut read_count = 0usize;
        for (b, block) in kernel.blocks().iter().enumerate() {
            for (o, operation) in block.operations().iter().enumerate() {
                budget.charge_work(4)?;
                match operation {
                    ProductionRankedOperationV1::Access {
                        kind: dialect_kernel::AccessKindAttr::Read,
                        ..
                    } => read_count += 1,
                    ProductionRankedOperationV1::RequireEffectRefinement { contract, proof } => {
                        let write = kernel
                            .blocks()
                            .get(proposal.write.block() as usize)
                            .and_then(|block| {
                                block.operations().get(proposal.write.operation() as usize)
                            });
                        if !matches!(write, Some(ProductionRankedOperationV1::ValueAccess {
                            kind: dialect_kernel::AccessKindAttr::Write, view, indices, value,
                        }) if *view == ownership.view && indices == &[proposal.index]
                            && *value == contract.gpu_value())
                        {
                            return Err(E::Subject("missing or mismatched output value"));
                        }
                        if contract.gpu_write_site() != proposal.write
                            || contract.view() != ownership.view
                            || contract.indices() != [proposal.index]
                            || proof.binding().subjects() != reference_subjects
                            || selected.replace((b as u32, o as u32)).is_some()
                        {
                            return Err(E::Subject("effect/reference subjects"));
                        }
                    }
                    ProductionRankedOperationV1::RequestEffectRefinement { .. } => {
                        return Err(E::Subject("unbound source/reference effect"));
                    }
                    _ => {}
                }
            }
        }
        if read_count != canonical.read_count() {
            return Err(E::Subject("complete ranked read roster"));
        }
        let effect = selected.ok_or(E::Subject("missing reference effect"))?;
        let exits = crate::check_ranked_recipe_paths_v1(
            kernel,
            proposal.index,
            proposal.extent,
            proposal.write,
            budget,
        )
        .map_err(E::Coverage)?;
        let output = ProductionConditionalOutputBindingV1 {
            parameter: canonical.output_parameter_index(),
            index: proposal.index,
            extent: proposal.extent,
            view: ownership.view,
            write: proposal.write,
            ownership: *ownership,
            effect,
            exits,
        };
        let mut raw_reads = reserve_rows(canonical.read_count(), budget)?;
        canonical
            .visit_reads_v1(budget, |read| {
                if raw_reads.len() == raw_reads.capacity() {
                    return Err(ResourceError::Accounting);
                }
                raw_reads.push(read);
                Ok(())
            })
            .map_err(E::Canonical)?;
        let mut reads = reserve_rows(raw_reads.len(), budget)?;
        let mut premises = reserve_rows(
            raw_reads
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(4))
                .ok_or(ResourceError::Arithmetic)?,
            budget,
        )?;
        premises.extend([
            ProductionConditionalRuntimePremiseV1::D1Launch,
            ProductionConditionalRuntimePremiseV1::OutputWithinGlobalX {
                parameter: output.parameter,
            },
            ProductionConditionalRuntimePremiseV1::WritableOutput {
                parameter: output.parameter,
            },
            ProductionConditionalRuntimePremiseV1::RepresentableAddress {
                parameter: output.parameter,
                domain: canonical.address_domain(),
                element_bytes: canonical.element_bytes(),
                alignment: canonical.alignment(),
            },
        ]);
        for read in &raw_reads {
            let mut site = None;
            for candidate in &proposal.reads {
                budget.charge_work(3)?;
                if candidate.canonical == read.location() && site.replace(*candidate).is_some() {
                    return Err(E::Subject("duplicate canonical read"));
                }
            }
            let site = site.ok_or(E::Subject("missing canonical read"))?;
            for previous in &reads {
                budget.charge_work(2)?;
                let previous: &ProductionConditionalReadBindingV1 = previous;
                if (previous.site.block, previous.site.operation) == (site.block, site.operation) {
                    return Err(E::Subject("duplicate ranked read"));
                }
            }
            let operation = kernel
                .blocks()
                .get(site.block as usize)
                .and_then(|block| block.operations().get(site.operation as usize));
            let Some(ProductionRankedOperationV1::Access {
                kind: dialect_kernel::AccessKindAttr::Read,
                view,
                indices,
            }) = operation
            else {
                return Err(E::Subject("ranked input read"));
            };
            if indices != &[proposal.index] || *view == output.view {
                return Err(E::Subject("read coordinate or output alias"));
            }
            reads.push(ProductionConditionalReadBindingV1 {
                canonical: *read,
                site,
                view: *view,
                index: proposal.index,
            });
            premises.extend([
                ProductionConditionalRuntimePremiseV1::ReadableInput {
                    parameter: read.parameter(),
                    domain: read.access_domain(),
                },
                ProductionConditionalRuntimePremiseV1::SeparateInputOutput {
                    input: read.parameter(),
                    output: output.parameter,
                },
                ProductionConditionalRuntimePremiseV1::RepresentableAddress {
                    parameter: read.parameter(),
                    domain: read.address_domain(),
                    element_bytes: read.element_bytes(),
                    alignment: read.alignment(),
                },
            ]);
        }
        let raw_storage = raw_reads
            .capacity()
            .checked_mul(std::mem::size_of::<ConditionalTotalViewReadV1>())
            .ok_or(ResourceError::Arithmetic)?;
        drop(raw_reads);
        budget.release_storage(raw_storage)?;
        let pipeline = self
            .check_pipeline_with_budget_v1(budget)
            .map_err(E::Pipeline)?;
        pipeline
            .require_current_graph_v1(budget)
            .map_err(E::Session)?;
        Ok(ProductionConditionalFinalGraphV1 {
            canonical,
            output: [output],
            reads,
            premises,
            source_semantic_identity: DigestV1::from_untrusted_bytes(source_semantic_identity),
            reference_subjects,
            pipeline,
        })
    }
}

impl<'canonical> ProductionConditionalFinalGraphV1<'canonical> {
    pub fn pending_analysis(&self) -> &ProductionConditionalRankedAnalysisV1 {
        self.pipeline.pending_analysis()
    }

    #[allow(clippy::result_large_err)]
    pub fn into_aggregate_state_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        ProductionConditionalAggregateStateV1<'canonical>,
        ProductionConditionalAggregateErrorV1,
    > {
        self.pipeline
            .require_current_graph_v1(budget)
            .map_err(ProductionConditionalAggregateErrorV1::Session)?;
        super::super::total_output_refinement_v2::require_conditional_total_output_staging_v1(
            &self, budget,
        )?;
        super::super::parallel_reference_contract_v1::require_conditional_parallel_reference_v1(
            &self, budget,
        )?;
        let identity = self.statement_identity_v1(budget)?;
        Ok(ProductionConditionalAggregateStateV1 {
            identity,
            graph: self,
        })
    }

    pub(in crate::production) fn kernel(&self) -> &ProductionRankedKernelV1 {
        self.pipeline
            .kernel()
            .expect("immutable conditional graph checked before staging")
    }

    pub(in crate::production) fn outputs(&self) -> &[ProductionConditionalOutputBindingV1] {
        &self.output
    }
    pub(in crate::production) fn reads(&self) -> &[ProductionConditionalReadBindingV1] {
        &self.reads
    }
    pub(in crate::production) fn premises(&self) -> &[ProductionConditionalRuntimePremiseV1] {
        &self.premises
    }
    pub(in crate::production) fn typed_root_commitments(&self) -> Option<&[[u64; 4]]> {
        self.pipeline
            .first
            .as_ref()?
            .report
            .typed_root_commitments_v1()
    }
    pub(in crate::production) fn retained_staging(
        &self,
    ) -> &[ProductionPolicyCheckedRefinementStagingV2] {
        let pending = self.pending_analysis();
        &pending
            ._session
            .constructed_roots
            .get(&pending._stage.identity)
            .expect("checked original stage")
            .policy_checked_refinement_staging
    }

    fn statement_identity_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<DigestV1, ProductionConditionalAggregateErrorV1> {
        // The exact recipe binds all typed roots, effect/reference sites and CFG.
        // This separate domain binds the implication's premises and source/CPU subjects.
        budget.with_prepaid_scope(
            budget.storage(),
            0,
            0,
            std::mem::size_of::<Sha256>(),
            |budget| {
                let mut hash = Sha256::new();
                let mut put = |bytes: &[u8]| -> Result<(), ResourceError> {
                    budget.charge_work(
                        bytes
                            .len()
                            .checked_add(1)
                            .ok_or(ResourceError::Arithmetic)?,
                    )?;
                    hash.update(bytes);
                    Ok(())
                };
                put(b"FE2O3/CONDITIONAL-AGGREGATE/IMPLICATION/V1\0")?;
                let pending = self.pipeline.pending_analysis();
                put(&pending
                    ._root
                    .exact_graph_identity
                    .ok_or(ProductionConditionalAggregateErrorV1::Subject(
                        "exact graph identity",
                    ))?
                    .digest())?;
                put(self.source_semantic_identity.as_bytes())?;
                for digest in [
                    self.reference_subjects.safe_reference_identity(),
                    self.reference_subjects.safe_reference_source_hash(),
                    self.reference_subjects.safe_reference_mir_hash(),
                    self.reference_subjects.kernel_subject_identity(),
                    self.reference_subjects.kernel_mir_hash(),
                ] {
                    put(digest.as_bytes())?;
                }
                put(&(self.reference_subjects.safe_reference_kind() as u8).to_le_bytes())?;
                put(&self.canonical.output_parameter_index().to_le_bytes())?;
                put(&(self.canonical.kernel_ordinal() as u64).to_le_bytes())?;
                put(&(self.canonical.function_ordinal() as u64).to_le_bytes())?;
                for value in [
                    self.canonical.output_value(),
                    self.canonical.index(),
                    self.canonical.length(),
                    self.canonical.predicate(),
                    self.canonical.pointer(),
                    self.canonical.offset(),
                    self.canonical.store_value(),
                ] {
                    put(&value.0.to_le_bytes())?;
                }
                let location = self.canonical.store_location();
                put(&location.block.0.to_le_bytes())?;
                put(&(location.operation_index as u64).to_le_bytes())?;
                match self.canonical.store_predicate() {
                    Some(value) => {
                        put(&[1])?;
                        put(&value.0.to_le_bytes())?;
                    }
                    None => put(&[0])?,
                }
                for output in &self.output {
                    for value in [
                        output.index,
                        output.extent,
                        output.view,
                        output.ownership.view,
                    ] {
                        emit_ranked_value_v1(value, &mut put)?;
                    }
                    for coordinate in [
                        output.parameter,
                        output.write.block(),
                        output.write.operation(),
                        output.ownership.block,
                        output.ownership.operation,
                        output.effect.0,
                        output.effect.1,
                        output.exits[0],
                        output.exits[1],
                    ] {
                        put(&coordinate.to_le_bytes())?;
                    }
                }
                put(&(self.reads.len() as u64).to_le_bytes())?;
                for read in &self.reads {
                    put(&read.canonical.parameter().to_le_bytes())?;
                    for value in [
                        read.canonical.slice(),
                        read.canonical.pointer(),
                        read.canonical.index(),
                        read.canonical.value(),
                    ] {
                        put(&value.0.to_le_bytes())?;
                    }
                    for location in [read.canonical.location(), read.site.canonical] {
                        put(&location.block.0.to_le_bytes())?;
                        put(&(location.operation_index as u64).to_le_bytes())?;
                    }
                    put(&read.site.block.to_le_bytes())?;
                    put(&read.site.operation.to_le_bytes())?;
                    emit_ranked_value_v1(read.view, &mut put)?;
                    emit_ranked_value_v1(read.index, &mut put)?;
                    put(&[
                        domain_tag(read.canonical.access_domain()),
                        domain_tag(read.canonical.address_domain()),
                    ])?;
                    put(&read.canonical.element_bytes().to_le_bytes())?;
                    put(&read.canonical.alignment().to_le_bytes())?;
                }
                put(&(self.premises.len() as u64).to_le_bytes())?;
                for premise in &self.premises {
                    match *premise {
                        ProductionConditionalRuntimePremiseV1::D1Launch => put(&[0])?,
                        ProductionConditionalRuntimePremiseV1::OutputWithinGlobalX {
                            parameter,
                        } => {
                            put(&[1])?;
                            put(&parameter.to_le_bytes())?;
                        }
                        ProductionConditionalRuntimePremiseV1::WritableOutput { parameter } => {
                            put(&[2])?;
                            put(&parameter.to_le_bytes())?;
                        }
                        ProductionConditionalRuntimePremiseV1::ReadableInput {
                            parameter,
                            domain,
                        } => {
                            put(&[3])?;
                            put(&parameter.to_le_bytes())?;
                            put(&[domain_tag(domain)])?;
                        }
                        ProductionConditionalRuntimePremiseV1::SeparateInputOutput {
                            input,
                            output,
                        } => {
                            put(&[4])?;
                            put(&input.to_le_bytes())?;
                            put(&output.to_le_bytes())?;
                        }
                        ProductionConditionalRuntimePremiseV1::RepresentableAddress {
                            parameter,
                            domain,
                            element_bytes,
                            alignment,
                        } => {
                            put(&[5])?;
                            put(&parameter.to_le_bytes())?;
                            put(&[domain_tag(domain)])?;
                            put(&element_bytes.to_le_bytes())?;
                            put(&alignment.to_le_bytes())?;
                        }
                    }
                }
                Ok(DigestV1::from_untrusted_bytes(hash.finalize().into()))
            },
        )
    }
}

fn domain_tag(domain: Domain) -> u8 {
    match domain {
        Domain::GuardedOutput => 0,
        Domain::GlobalLaunch => 1,
    }
}

fn emit_ranked_value_v1(
    value: ProductionRankedValueV1,
    put: &mut impl FnMut(&[u8]) -> Result<(), ResourceError>,
) -> Result<(), ResourceError> {
    match value {
        ProductionRankedValueV1::Argument(argument) => {
            put(&[0])?;
            put(&argument.to_le_bytes())
        }
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            put(&[1])?;
            put(&block.to_le_bytes())?;
            put(&argument.to_le_bytes())
        }
        ProductionRankedValueV1::Local(local) => {
            put(&[2])?;
            put(&local.get().to_le_bytes())
        }
    }
}

impl ProductionConditionalAggregateStateV1<'_> {
    /// Rechecks epoch and statement identity, discards conditional derived facts,
    /// and returns the original pending arena. No clean evidence or proof is
    /// transferred; callers must preserve the arena's existing reservation.
    #[allow(clippy::result_large_err)]
    pub fn into_pending_analysis_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionConditionalRankedAnalysisV1, ProductionConditionalAggregateErrorV1> {
        self.with_input_v1(budget, |_, _| ())?;
        let ProductionConditionalFinalGraphV1 {
            canonical,
            output,
            reads,
            premises,
            pipeline,
            ..
        } = self.graph;
        drop((canonical, output, reads, premises));
        Ok(pipeline.into_pending_analysis_v1())
    }

    pub fn pending_analysis(&self) -> &ProductionConditionalRankedAnalysisV1 {
        self.graph.pending_analysis()
    }
    #[allow(clippy::result_large_err)]
    pub fn with_input_v1<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl for<'a> FnOnce(&ProductionConditionalAggregateInputV1<'a>, &mut Budget<'_>) -> R,
    ) -> Result<R, ProductionConditionalAggregateErrorV1> {
        self.graph
            .pipeline
            .require_current_graph_v1(budget)
            .map_err(ProductionConditionalAggregateErrorV1::Session)?;
        if self.graph.statement_identity_v1(budget)? != self.identity {
            return Err(ProductionConditionalAggregateErrorV1::Subject(
                "aggregate statement changed",
            ));
        }
        let account = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let result = consume(
            &ProductionConditionalAggregateInputV1 { state: self },
            budget,
        );
        if budget.work_ledger_identity_v1() != account || budget.storage() < floor {
            return Err(ResourceError::Accounting.into());
        }
        self.graph
            .pipeline
            .require_current_graph_v1(budget)
            .map_err(ProductionConditionalAggregateErrorV1::Session)?;
        Ok(result)
    }
}

impl<'a> ProductionConditionalAggregateInputV1<'a> {
    pub fn kernel(&self) -> &'a ProductionRankedKernelV1 {
        self.state.graph.kernel()
    }
    pub fn exact_graph_identity(&self) -> ProductionExactGraphIdentityV1 {
        self.state
            .graph
            .pending_analysis()
            ._root
            .exact_graph_identity
            .expect("checked exact graph")
    }
    pub fn graph_snapshot(&self) -> OperationGraphSnapshotV1 {
        self.state.graph.pending_analysis()._root.graph_snapshot
    }
    pub const fn identity(&self) -> DigestV1 {
        self.state.identity
    }
    pub const fn source_semantic_identity(&self) -> DigestV1 {
        self.state.graph.source_semantic_identity
    }
    pub const fn reference_subjects(&self) -> ProductionConditionalReferenceSubjectsV1 {
        self.state.graph.reference_subjects
    }
    pub fn premises(&self) -> &'a [ProductionConditionalRuntimePremiseV1] {
        &self.state.graph.premises
    }
    pub fn outputs(&self) -> &'a [ProductionConditionalOutputBindingV1] {
        &self.state.graph.output
    }
    pub fn reads(&self) -> &'a [ProductionConditionalReadBindingV1] {
        &self.state.graph.reads
    }
    pub fn retained_policy_checked_refinement_staging(
        &self,
    ) -> &'a [ProductionPolicyCheckedRefinementStagingV2] {
        self.state.graph.retained_staging()
    }
    pub fn typed_root_commitments(&self) -> &'a [[u64; 4]] {
        self.state
            .graph
            .typed_root_commitments()
            .expect("staged exact typed roots")
    }
    pub fn effect_contract(
        &self,
        output: &ProductionConditionalOutputBindingV1,
    ) -> Option<&'a ProductionEffectRefinementContractV2> {
        if !self.outputs().contains(output) {
            return None;
        }
        match &self.kernel().blocks()[output.effect.0 as usize].operations()
            [output.effect.1 as usize]
        {
            ProductionRankedOperationV1::RequireEffectRefinement { contract, .. } => Some(contract),
            _ => None,
        }
    }
    #[allow(clippy::result_large_err)]
    pub fn require_current_graph_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), ProductionConditionalAggregateErrorV1> {
        self.state
            .graph
            .pipeline
            .require_current_graph_v1(budget)
            .map_err(ProductionConditionalAggregateErrorV1::Session)
    }
}
