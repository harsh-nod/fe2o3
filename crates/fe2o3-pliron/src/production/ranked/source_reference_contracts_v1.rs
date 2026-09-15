//! Inert source-contract export, not a refinement capability or a final-graph seal.
//!
//! The retained graph is checked against the existing materializer, not merely
//! against a cached analysis report. CPU expressions come only from the declared
//! reference roots. The caller must still join the exact source bindings to its
//! compiler-authenticated proof roster and establish correspondence to final KIR.

use super::*;
use fe2o3_kernel_analysis::{
    PlironIrStructuralIdentityV1, derive_pliron_ir_structural_identity_v1,
};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};

mod memory_reads_v1;
use memory_reads_v1::SourceInitialReadsV1;

// The pinned PLIRON Ptr is only an arena index. Keep its captured context
// inseparable from the index so another context's same slot cannot alias it.
#[derive(Debug)]
pub(super) struct SourceContractFunctionV1 {
    owner: ContextIdentity,
    operation: Ptr<Operation>,
}

impl SourceContractFunctionV1 {
    pub(super) fn capture(session: &ProductionPlironSessionV1, operation: Ptr<Operation>) -> Self {
        Self {
            owner: session.inner.identity,
            operation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSourceContractExportErrorV1 {
    LiveOwnerChanged,
    LiveGraphChanged,
    LiveAnalysisRejected,
    LiveMemoryRejected,
    RecipeReplayRejected,
    InvalidRecipe,
    UnsupportedLoad { block: usize, operation: usize },
    UnsupportedPartialOrFrame,
    UnsupportedNonPointwise { block: usize, operation: usize },
    MissingOutputContracts,
    OutputBindingMismatch,
    ReferenceRootAliasesGpuValue,
    MissingTypedReferenceRoot,
    UnsupportedNumericalPolicy,
    ProofBindingMismatch,
}

impl fmt::Display for ProductionSourceContractExportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source contract export rejected: {self:?}")
    }
}

impl Error for ProductionSourceContractExportErrorV1 {}

/// Borrowed facts from one source write. These existing objects are requests
/// and caller-policy staging, not evidence of compiler-authorized execution.
#[derive(Debug)]
pub struct ProductionSourceOutputContractFactsV1<'a> {
    effect: &'a ProductionEffectRefinementContractV2,
    source_gpu_rhs: &'a ProductionSemanticExpressionV2,
    reference_rhs: &'a ProductionSemanticExpressionV2,
    numerical: ProductionNumericalContractV2,
    view: &'a ProductionRankedOperationV1,
    ownership: &'a ProductionRankedOperationV1,
    proof: ProductionReferenceProofV2,
    staging: &'a ProductionPolicyCheckedRefinementStagingV2,
}

impl<'a> ProductionSourceOutputContractFactsV1<'a> {
    pub fn effect(&self) -> &'a ProductionEffectRefinementContractV2 {
        self.effect
    }
    pub fn source_gpu_rhs(&self) -> &'a ProductionSemanticExpressionV2 {
        self.source_gpu_rhs
    }
    pub fn reference_rhs(&self) -> &'a ProductionSemanticExpressionV2 {
        self.reference_rhs
    }
    pub fn numerical_contract(&self) -> ProductionNumericalContractV2 {
        self.numerical
    }
    /// Exact view declaration, including allocation, noalias, shape and space.
    pub fn view_definition(&self) -> &'a ProductionRankedOperationV1 {
        self.view
    }
    pub fn ownership_contract(&self) -> &'a ProductionRankedOperationV1 {
        self.ownership
    }
    pub fn proof_request(&self) -> ProductionReferenceProofV2 {
        self.proof
    }
    /// Receipt identity and binding are reconciled with the live proof header.
    /// Signer, toolchain and execution metadata remain inert retained staging;
    /// this method neither reimports receipt bytes nor authenticates execution.
    pub fn policy_checked_staging(&self) -> &'a ProductionPolicyCheckedRefinementStagingV2 {
        self.staging
    }
}

/// A snapshot borrowing the retained source owner. Neither its graph digest nor
/// its staged receipts authorize a final graph, target, artifact, or launch.
#[derive(Debug)]
pub struct ProductionSourceReferenceContractsV1<'a> {
    source: &'a ProductionRankedKernelV1,
    outputs: Vec<ProductionSourceOutputContractFactsV1<'a>>,
    live_graph_sha256: [u8; 32],
    context: ContextIdentity,
    mutation_epoch: u64,
}

impl<'a> ProductionSourceReferenceContractsV1<'a> {
    /// Retains the complete coordinate/domain DAG; no positional ABI remapping
    /// or scalar-symbol substitution is performed by this export.
    pub fn source_kernel(&self) -> &'a ProductionRankedKernelV1 {
        self.source
    }
    pub fn outputs(&self) -> &[ProductionSourceOutputContractFactsV1<'a>] {
        &self.outputs
    }
    pub fn live_graph_sha256(&self) -> &[u8; 32] {
        &self.live_graph_sha256
    }
    pub fn source_context_identity(&self) -> ContextIdentity {
        self.context
    }
    pub fn source_mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionRankedKernelLoweringInputV1 {
    /// Revalidates the actual retained graph and exports independent CPU RHS,
    /// effect, ownership, numerical and exact receipt-binding facts. Supports
    /// static total-view outputs and independently proved initial
    /// unordered nonvolatile reads. Final-graph correspondence is not granted.
    pub fn export_live_source_reference_contracts_v1(
        &self,
    ) -> Result<ProductionSourceReferenceContractsV1<'_>, ProductionSourceContractExportErrorV1>
    {
        use ProductionSourceContractExportErrorV1 as E;
        catch_unwind(AssertUnwindSafe(|| {
            self.require_source_export_owner()?;
            self.revalidate_structure().map_err(|_| E::InvalidRecipe)?;
            let has_reads = source_has_reads(&self.kernel);
            let load_free_outputs = if has_reads {
                None
            } else {
                Some(collect_output_facts(
                    &self.kernel,
                    &self.policy_checked_refinement_staging,
                    None,
                )?)
            };
            let context = &self._session.inner.context;
            let function = FuncOp::from_operation(self.source_contract_function.operation);
            let live = derive_pliron_ir_structural_identity_v1(context, &function)
                .map_err(|_| E::LiveGraphChanged)?;
            let replay = self.replay_source_contract_graph()?;
            if !live.exactly_matches(&replay) {
                return Err(E::LiveGraphChanged);
            }
            let memory = if has_reads {
                Some(SourceInitialReadsV1::prove(
                    context,
                    &function,
                    &self.kernel,
                )?)
            } else {
                None
            };
            let outputs = match load_free_outputs {
                Some(outputs) => outputs,
                None => collect_output_facts(
                    &self.kernel,
                    &self.policy_checked_refinement_staging,
                    memory.as_ref(),
                )?,
            };
            let report = match self._session.atomic_target.as_ref() {
                Some(target) => {
                    require_production_pliron_checks_with_atomic_target_before_lowering_v2(
                        context, &function, target,
                    )
                }
                None => require_production_pliron_checks_before_lowering_v2(context, &function),
            }
            .map_err(|_| E::LiveAnalysisRejected)?;
            if !report.is_clean()
                || report != self.production_pipeline_report
                || report.semantics().typed_root_commitments()
                    != expected_typed_root_commitments(&self.kernel)
            {
                return Err(E::LiveAnalysisRejected);
            }
            let ownership = report.ownership();
            let coverage = ownership.coverage_summary();
            let effects = report.semantics().effect_refinement();
            let count = outputs.len();
            if count == 0
                || !ownership.all_total_view_contracts_are_proved()
                || coverage.total_view_declared() != count
                || coverage.total_view_proved() != count
                || !effects.all_declared_effects_are_proved()
                || effects.contract_count() != count
                || effects.proved_contract_count() != count
            {
                return Err(E::LiveAnalysisRejected);
            }
            self.require_source_export_owner()?;
            let after = derive_pliron_ir_structural_identity_v1(context, &function)
                .map_err(|_| E::LiveGraphChanged)?;
            if !live.exactly_matches(&after) {
                return Err(E::LiveGraphChanged);
            }
            if let Some(memory) = memory {
                memory.revalidate(context, &function)?;
            }
            Ok(ProductionSourceReferenceContractsV1 {
                source: &self.kernel,
                outputs,
                live_graph_sha256: *live.sha256(),
                context: self._session.inner.identity,
                mutation_epoch: self
                    .source_contract_mutation_epoch
                    .ok_or(E::LiveOwnerChanged)?,
            })
        }))
        .map_err(|_| E::LiveGraphChanged)?
    }

    fn require_source_export_owner(&self) -> Result<(), ProductionSourceContractExportErrorV1> {
        use ProductionSourceContractExportErrorV1 as E;
        let session = &self._session;
        let context = &session.inner.context;
        let identity = require_context_identity(context).map_err(|_| E::LiveOwnerChanged)?;
        let epoch = context
            .ir_mutation_attempt_epoch()
            .map_err(|_| E::LiveOwnerChanged)?
            .value();
        if session.is_poisoned()
            || identity != session.inner.identity
            || identity != self.source_contract_function.owner
            || self._stage.owner != identity
            || self._root.owner != identity
            || self._root.stage != self._stage.identity
            || self.source_contract_mutation_epoch != Some(epoch)
        {
            return Err(E::LiveOwnerChanged);
        }
        let root = *session
            .inner
            .operations
            .get(&self._root.operation.identity)
            .ok_or(E::LiveOwnerChanged)?;
        if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
            return Err(E::LiveOwnerChanged);
        }
        let blocks: Vec<_> = root
            .deref(context)
            .get_region(0)
            .deref(context)
            .iter(context)
            .collect();
        let [block] = blocks.as_slice() else {
            return Err(E::LiveOwnerChanged);
        };
        let functions: Vec<_> = block.deref(context).iter(context).collect();
        if functions.as_slice() != [self.source_contract_function.operation] {
            return Err(E::LiveOwnerChanged);
        }
        Ok(())
    }

    fn replay_source_contract_graph(
        &self,
    ) -> Result<PlironIrStructuralIdentityV1, ProductionSourceContractExportErrorV1> {
        use ProductionSourceContractExportErrorV1 as E;
        let registrations = [
            dialect_kernel::dialect_registration().map_err(|_| E::RecipeReplayRejected)?,
            dialect_gpu::dialect_registration().map_err(|_| E::RecipeReplayRejected)?,
            dialect_proof::dialect_registration().map_err(|_| E::RecipeReplayRejected)?,
        ];
        let mut replay = ProductionPlironSessionV1::new(self._session.limits(), registrations)
            .map_err(|_| E::RecipeReplayRejected)?;
        let semantic_reads =
            RankedSemanticReadsV1::new(&self.kernel).map_err(|_| E::RecipeReplayRejected)?;
        let schedule = RankedOperationScheduleV1::new(&self.kernel, &semantic_reads)
            .map_err(|_| E::RecipeReplayRejected)?;
        let context = &mut replay.inner.context;
        let index: TypeHandle = IndexType::get(context).into();
        let ty = FunctionType::get(context, vec![index; self.kernel.argument_count()], vec![]);
        let function = FuncOp::new(
            context,
            self.kernel
                .function_name()
                .try_into()
                .map_err(|_| E::RecipeReplayRejected)?,
            ty,
        );
        // Use the real completed-body builder, including exact source reads.
        // Staging is borrowed, never cloned into another authority-bearing owner.
        materialization_v1::materialize_body(
            context,
            &function,
            &self.kernel,
            &schedule,
            semantic_reads,
            &self.policy_checked_refinement_staging,
        )
        .map_err(|_| E::RecipeReplayRejected)?;
        derive_pliron_ir_structural_identity_v1(context, &function)
            .map_err(|_| E::RecipeReplayRejected)
    }
}

fn collect_output_facts<'a>(
    kernel: &'a ProductionRankedKernelV1,
    staging: &'a [ProductionPolicyCheckedRefinementStagingV2],
    memory: Option<&SourceInitialReadsV1<'a>>,
) -> Result<Vec<ProductionSourceOutputContractFactsV1<'a>>, ProductionSourceContractExportErrorV1> {
    use ProductionRankedOperationV1 as O;
    use ProductionSourceContractExportErrorV1 as E;
    // Diagnose loads before the general pointwise shape restriction.
    for (block, recipe) in kernel.blocks.iter().enumerate() {
        for (operation, op) in recipe.operations.iter().enumerate() {
            let reads = match op {
                O::SemanticExpression { expression, .. } => contains_load(expression),
                O::SemanticSymbol { symbol, .. } => {
                    *symbol >= super::super::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2
                }
                O::Access { kind, .. }
                | O::ValueAccess { kind, .. }
                | O::PredicatedAccess { kind, .. }
                | O::AtomicAccess { kind, .. }
                | O::AtomicValueAccess { kind, .. }
                | O::AllocationEffect { kind, .. } => kind.reads_memory(),
                _ => false,
            };
            let checked = memory.is_some_and(|memory| match op {
                O::SemanticExpression { expression, .. } => memory.admits_expression(expression),
                O::Access {
                    kind: AccessKindAttr::Read,
                    ..
                } => memory.admits_site(block, operation),
                _ => false,
            });
            if reads && !checked {
                return Err(E::UnsupportedLoad { block, operation });
            }
        }
    }
    let mut writes = BTreeMap::new();
    let mut ownership = BTreeMap::new();
    let mut views = BTreeMap::new();
    let mut roots = BTreeMap::new();
    let mut effects = Vec::new();
    for (block, recipe) in kernel.blocks.iter().enumerate() {
        for (operation, op) in recipe.operations.iter().enumerate() {
            match op {
                O::View {
                    result,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    noalias_class,
                    ..
                }
                | O::ViewInSpace {
                    result,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    noalias_class,
                    ..
                } => {
                    if !dynamic_extents.is_empty() || shape.contains(&DYNAMIC_EXTENT) {
                        return Err(E::UnsupportedPartialOrFrame);
                    }
                    if *allocation_origin == 0
                        || *noalias_class == 0
                        || matches!(op,
                    O::ViewInSpace { memory_space, .. } if *memory_space != MemorySpaceAttr::Global)
                    {
                        return Err(E::OutputBindingMismatch);
                    }
                    views.insert(ProductionRankedValueV1::Local(*result), op);
                }
                O::ValueAccess {
                    kind: AccessKindAttr::Write,
                    value,
                    ..
                } => {
                    writes.insert(
                        ProductionGpuWriteSiteV2::new(block as u32, operation as u32),
                        *value,
                    );
                }
                O::OwnershipContract {
                    view,
                    coverage,
                    partition,
                } => {
                    if block != 0
                        || *coverage != OwnershipCoverageAttr::TotalView
                        || *partition != OwnershipPartitionAttr::ExactSets
                    {
                        return Err(E::UnsupportedPartialOrFrame);
                    }
                    if ownership.insert(*view, op).is_some() {
                        return Err(E::OutputBindingMismatch);
                    }
                }
                O::SemanticExpression {
                    result,
                    expression,
                    numerical_contract,
                } => {
                    if !numerical_contract.admits_expression(expression) {
                        return Err(E::UnsupportedNumericalPolicy);
                    }
                    roots.insert(
                        ProductionRankedValueV1::Local(*result),
                        (expression, *numerical_contract),
                    );
                }
                O::RequireEffectRefinement { contract, proof } => {
                    effects.push((block, operation, contract, proof))
                }
                O::ExecutionLayout { .. }
                | O::Access {
                    kind: AccessKindAttr::Read,
                    ..
                }
                | O::IndexConstant { .. }
                | O::IndexUnsignedCast { .. }
                | O::InvocationIndex { .. }
                | O::IndexBinary { .. }
                | O::Dimension { .. }
                | O::SemanticSymbol { .. }
                | O::SemanticConstant { .. }
                | O::SemanticBinary { .. } => {}
                _ => {
                    return Err(E::UnsupportedNonPointwise { block, operation });
                }
            }
        }
    }
    if writes.is_empty() || effects.len() != writes.len() {
        return Err(E::MissingOutputContracts);
    }
    if staging.len() != effects.len() {
        return Err(E::ProofBindingMismatch);
    }
    let mut consumed_writes = BTreeSet::new();
    let mut consumed_views = BTreeSet::new();
    let mut reference_sites = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    let mut receipts = BTreeSet::new();
    let mut outputs = Vec::new();
    for (block, operation, effect, proof) in effects {
        let site = effect.gpu_write_site();
        let reference_site = effect.reference_output_site();
        if writes.get(&site) != Some(&effect.gpu_value())
            || !consumed_writes.insert(site)
            || !consumed_views.insert(effect.view())
            || !contracts.insert(effect.contract_identity())
            || !reference_sites.insert((
                reference_site.argument(),
                reference_site.block(),
                reference_site.statement(),
            ))
        {
            return Err(E::OutputBindingMismatch);
        }
        if writes
            .values()
            .any(|value| *value == effect.reference_value())
        {
            return Err(E::ReferenceRootAliasesGpuValue);
        }
        for value in [
            effect.gpu_domain(),
            effect.reference_domain(),
            effect.gpu_precondition(),
            effect.reference_precondition(),
        ] {
            if !is_true(kernel, value) {
                return Err(E::UnsupportedPartialOrFrame);
            }
        }
        let &(source_gpu_rhs, gpu_numerical) = roots
            .get(&effect.gpu_value())
            .ok_or(E::MissingTypedReferenceRoot)?;
        let &(reference_rhs, numerical) = roots
            .get(&effect.reference_value())
            .ok_or(E::MissingTypedReferenceRoot)?;
        if numerical != gpu_numerical || source_gpu_rhs.scalar() != reference_rhs.scalar() {
            return Err(E::UnsupportedNumericalPolicy);
        }
        let &view = views.get(&effect.view()).ok_or(E::OutputBindingMismatch)?;
        match view {
            O::View {
                element_width,
                writable: true,
                ..
            }
            | O::ViewInSpace {
                element_width,
                writable: true,
                ..
            } if *element_width == u32::from(reference_rhs.scalar().bit_width()) => {}
            _ => return Err(E::OutputBindingMismatch),
        }
        let &owner = ownership
            .get(&effect.view())
            .ok_or(E::OutputBindingMismatch)?;
        let digest = normalized_effect_refinement_hash_for_kernel_v2(
            kernel,
            block,
            operation,
            effect,
            proof.binding().subjects(),
        )
        .map_err(|_| E::OutputBindingMismatch)?;
        if digest != proof.binding().normalized_obligation_effect_ir_hash()
            || proof.binding().safe_reference_kind()
                != fe2o3_functional_proof::SafeReferenceKindV2::Mir
            || !receipts.insert(proof.receipt_identity())
        {
            return Err(E::ProofBindingMismatch);
        }
        let mut matching = staging
            .iter()
            .filter(|candidate| candidate.receipt_identity() == proof.receipt_identity());
        let staged = matching.next().ok_or(E::ProofBindingMismatch)?;
        if matching.next().is_some()
            || staged.binding() != proof.binding()
            || staged.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
        {
            return Err(E::ProofBindingMismatch);
        }
        outputs.push(ProductionSourceOutputContractFactsV1 {
            effect,
            source_gpu_rhs,
            reference_rhs,
            numerical,
            view,
            ownership: owner,
            proof: *proof,
            staging: staged,
        });
    }
    if consumed_views.len() != ownership.len() {
        return Err(E::OutputBindingMismatch);
    }
    if views.iter().any(|(value, view)| {
        matches!(
            view,
            O::View { writable: true, .. } | O::ViewInSpace { writable: true, .. }
        ) && !consumed_views.contains(value)
    }) {
        return Err(E::UnsupportedPartialOrFrame);
    }
    Ok(outputs)
}

fn contains_load(expression: &ProductionSemanticExpressionV2) -> bool {
    use ProductionSemanticExpressionV2 as X;
    match expression {
        X::Load(_) => true,
        X::Symbol { symbol, .. } => {
            *symbol >= super::super::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2
        }
        X::Constant { .. } => false,
        X::Unary { operand, .. } | X::Cast { operand, .. } => contains_load(operand),
        X::Binary { lhs, rhs, .. } | X::Compare { lhs, rhs, .. } => {
            contains_load(lhs) || contains_load(rhs)
        }
        X::Select {
            condition,
            when_true,
            when_false,
            ..
        } => contains_load(condition) || contains_load(when_true) || contains_load(when_false),
    }
}

fn source_has_reads(kernel: &ProductionRankedKernelV1) -> bool {
    kernel
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|op| match op {
            ProductionRankedOperationV1::SemanticExpression { expression, .. } => {
                contains_load(expression)
            }
            ProductionRankedOperationV1::Access { kind, .. }
            | ProductionRankedOperationV1::ValueAccess { kind, .. }
            | ProductionRankedOperationV1::PredicatedAccess { kind, .. }
            | ProductionRankedOperationV1::AtomicAccess { kind, .. }
            | ProductionRankedOperationV1::AtomicValueAccess { kind, .. }
            | ProductionRankedOperationV1::AllocationEffect { kind, .. } => kind.reads_memory(),
            _ => false,
        })
}

fn is_true(kernel: &ProductionRankedKernelV1, value: ProductionRankedValueV1) -> bool {
    kernel.blocks[0]
        .operations
        .iter()
        .any(|operation| match operation {
            ProductionRankedOperationV1::SemanticConstant { result, value: 1 }
            | ProductionRankedOperationV1::SemanticExpression {
                result,
                expression:
                    ProductionSemanticExpressionV2::Constant {
                        scalar: ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    },
                ..
            } => value == ProductionRankedValueV1::Local(*result),
            _ => false,
        })
}

#[cfg(all(test, feature = "internal-proof-staging"))]
mod tests;
