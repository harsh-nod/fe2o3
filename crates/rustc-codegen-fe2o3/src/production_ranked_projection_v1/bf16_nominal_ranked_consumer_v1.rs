//! Terminal consumption of the actual pending block stream by the mandatory
//! ranked verifier. This is not the normal BF16 source route or a launch token.
//! The physical pending owner retains the live session and correspondence data
//! through every enclosing source/ledger postflight. Its caller must drop that
//! owner before releasing its original accepted credits.
use super::*;
use crate::production_ranked_projection_v1::ranked_access_source_row_v1;
use fe2o3_pliron::{
    ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedSnapshotAllowanceV1 as Snapshot,
    compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1 as compile,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Entered,
    Compiling,
    Verified,
}

/// No detached lowering/session or paid reservation is returned from this slot.
pub(super) struct Pending {
    phase: Phase,
    ordinals: Vec<(ProjectedSemanticAccessSiteV1, u32)>,
    accesses: Vec<ProductionRankedAccessSourceV1>,
    unmapped_private_reads: usize,
    operation_counts: Vec<usize>,
    tensors: Vec<ProductionRankedOperationV1>,
    // Large retained result lives in one separately paid slot, not inline in
    // the fixed preparation header. This is not a whole-session heap bound.
    lowering: Vec<ProductionRankedKernelLoweringInputV1>,
}
impl Pending {
    pub(super) const fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ordinals: Vec::new(),
            accesses: Vec::new(),
            unmapped_private_reads: 0,
            operation_counts: Vec::new(),
            tensors: Vec::new(),
            lowering: Vec::new(),
        }
    }
}

/// A lexical borrow only. The mandatory live Pliron result is retained inside
/// the same pending assembly, not reconstructed from reports or booleans.
pub(in crate::production_ranked_projection_v1) struct Verified<'a> {
    lowering: &'a ProductionRankedKernelLoweringInputV1,
    accesses: &'a [ProductionRankedAccessSourceV1],
    emitted_sources: &'a [ProjectedAccessSourceV1],
    unmapped_private_reads: usize,
    tensors: &'a [emission::TensorSite],
}
impl Verified<'_> {
    pub(in crate::production_ranked_projection_v1) fn lowering(
        &self,
    ) -> &ProductionRankedKernelLoweringInputV1 {
        self.lowering
    }
    pub(in crate::production_ranked_projection_v1) fn access_sources(
        &self,
    ) -> &[ProductionRankedAccessSourceV1] {
        self.accesses
    }
    pub(in crate::production_ranked_projection_v1) fn emitted_sources(
        &self,
    ) -> &[ProjectedAccessSourceV1] {
        self.emitted_sources
    }
    pub(in crate::production_ranked_projection_v1) fn unmapped_private_reads(&self) -> usize {
        self.unmapped_private_reads
    }
    pub(in crate::production_ranked_projection_v1) fn tensor_count(&self) -> usize {
        self.tensors.len()
    }
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn lookup_work(rows: usize) -> Result<usize> {
    // At most two shared-predicate ordinal lookups plus one update lookup.
    // Each examines a fixed (block, optional statement, count) row.
    add(64, mul(rows, 12)?)
}
fn pay(
    pending: &mut Pending,
    analysis: Analysis,
    snapshot: Snapshot,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    if !resources.is_metered() || resources.has_denial() || pending.phase != Phase::Fresh {
        return Err(resource(Resource::Accounting));
    }
    pending.phase = Phase::Entered; // All refusals are terminal; never retry.
    // These are selected prepayments, not measurements or complete heap costs.
    resources.work(add(analysis.max_work(), snapshot.max_work())?)?;
    resources.reserve_storage(analysis.max_peak_storage())?;
    Ok(())
}
fn argument(value: ProductionRankedValueV1) -> Result<()> {
    match value {
        ProductionRankedValueV1::Local(_) | ProductionRankedValueV1::Argument(0) => Ok(()),
        _ => Err(Error::Incomplete(
            "nominal verifier leaves the original extent-argument namespace",
        )),
    }
}
fn validate_payload(
    emitted: &emission::Emitted,
    pending: &mut Pending,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(64)?;
    if !emitted.complete
        || emitted.blocks.is_empty()
        || emitted.blocks.len() > fe2o3_pliron::MAX_RANKED_BOUNDS_BLOCKS
        || emitted.sources.len() > MAX_SOURCE_SITES
        || emitted.tensors.len() > MAX_RANKED_BOUNDS_OPERATIONS
        || !pending.operation_counts.is_empty()
        || !pending.tensors.is_empty()
    {
        return Err(Error::Incomplete(
            "nominal verifier lacks one complete original stream",
        ));
    }
    emitted
        .controls
        .validate(&emitted.blocks, &emitted.base, resources)?;
    resources.reserve(&mut pending.operation_counts, emitted.blocks.len())?;
    resources.reserve(&mut pending.tensors, emitted.tensors.len())?;
    let mut tensor_cursor = 0usize;
    for (block_index, block) in emitted.blocks.iter().enumerate() {
        resources.work(32)?;
        if block.index_argument_count() != 0 {
            return Err(Error::Incomplete(
                "nominal verifier has an unjoined block-argument namespace",
            ));
        }
        pending.operation_counts.push(block.operations().len());
        for (operation_index, operation) in block.operations().iter().enumerate() {
            resources.work(32)?;
            match operation {
                ProductionRankedOperationV1::ExecutionLayout { .. }
                | ProductionRankedOperationV1::InvocationIndex { .. }
                | ProductionRankedOperationV1::IndexConstant { .. }
                | ProductionRankedOperationV1::AllocationEffect { .. } => {}
                ProductionRankedOperationV1::ViewInSpace {
                    dynamic_extents, ..
                } => {
                    resources.work(mul(dynamic_extents.len(), 4)?)?;
                    for value in dynamic_extents {
                        argument(*value)?;
                    }
                }
                ProductionRankedOperationV1::Access { view, indices, .. } => {
                    resources.work(mul(add(indices.len(), 1)?, 4)?)?;
                    argument(*view)?;
                    for value in indices {
                        argument(*value)?;
                    }
                }
                ProductionRankedOperationV1::TensorLayout {
                    contract,
                    convergence,
                    active_lanes,
                    binding,
                } => {
                    let site = emitted.tensors.get(tensor_cursor).ok_or(Error::Incomplete(
                        "nominal verifier tensor correspondence is incomplete",
                    ))?;
                    if (site.ranked_block, site.ranked_operation) != (block_index, operation_index)
                        || site.source_block >= MAX_BLOCKS
                        || binding.is_none()
                    {
                        return Err(Error::Incomplete(
                            "nominal verifier tensor correspondence differs",
                        ));
                    }
                    pending
                        .tensors
                        .push(ProductionRankedOperationV1::TensorLayout {
                            contract: *contract,
                            convergence: *convergence,
                            active_lanes: *active_lanes,
                            binding: *binding,
                        });
                    tensor_cursor += 1;
                }
                _ => {
                    return Err(Error::Incomplete(
                        "nominal verifier operation leaves the emitted profile",
                    ));
                }
            }
        }
        match block.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. } => {
                // Only a separately source-certified checked-view comparison
                // may use dedicated length slots. Ordinary operands keep slot0.
                resources.work(MAX_BLOCKS * 8)?;
                if let Some(site) = emitted.controls.site_at_ranked(block_index) {
                    if block.terminator()
                        != &checked_control::expected_terminator(site, &emitted.base)?
                    {
                        return Err(Error::Incomplete(
                            "nominal verifier checked-view branch differs",
                        ));
                    }
                } else {
                    argument(*lhs)?;
                    argument(*rhs)?;
                }
            }
            ProductionRankedTerminatorV1::AnalysisSplit {
                control_dependencies,
                ..
            } => {
                if !control_dependencies.is_empty() {
                    return Err(Error::Incomplete(
                        "nominal verifier would invent uniform control",
                    ));
                }
            }
            ProductionRankedTerminatorV1::Branch { .. }
            | ProductionRankedTerminatorV1::Return
            | ProductionRankedTerminatorV1::Trap => {}
            _ => {
                return Err(Error::Incomplete(
                    "nominal verifier terminator leaves the emitted profile",
                ));
            }
        }
    }
    if tensor_cursor != emitted.tensors.len() {
        return Err(Error::Incomplete(
            "nominal verifier has unmatched tensor correspondence",
        ));
    }
    Ok(())
}
fn check_transformed(
    kernel: &ProductionRankedKernelV1,
    emitted: &emission::Emitted,
    pending: &Pending,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(32)?;
    if kernel.argument_count() != emitted.controls.argument_count()
        || kernel.blocks().len() != pending.operation_counts.len()
        || emitted.tensors.len() != pending.tensors.len()
    {
        return Err(Error::Incomplete(
            "nominal constructor changed the retained coordinate domain",
        ));
    }
    emitted
        .controls
        .validate(kernel.blocks(), &emitted.base, resources)?;
    for (block, expected) in kernel.blocks().iter().zip(&pending.operation_counts) {
        resources.work(8)?;
        if block.operations().len() != *expected {
            return Err(Error::Incomplete(
                "nominal constructor changed an operation coordinate",
            ));
        }
    }
    for (site, expected) in emitted.tensors.iter().zip(&pending.tensors) {
        resources.work(32)?;
        if kernel
            .blocks()
            .get(site.ranked_block)
            .and_then(|block| block.operations().get(site.ranked_operation))
            != Some(expected)
        {
            return Err(Error::Incomplete(
                "nominal constructor changed its source-bound tensor",
            ));
        }
    }
    Ok(())
}

// Shared legacy admission predicates; only the bounded storage/lookup policy
// differs. Both containers belong to Pending before any fallible source walk.
fn map_accesses(
    emitted: &emission::Emitted,
    function: &SemanticFunctionDeclV1,
    pending: &mut Pending,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
) -> Result<()> {
    context.with_resources(|resources| {
        resources.reserve(&mut pending.ordinals, emitted.sources.len())?;
        resources.reserve(&mut pending.accesses, emitted.sources.len())
    })?;
    let owner = context.facts.owner;
    let types = owner.semantic_ssa().source_semantic().types();
    for source in &emitted.sources {
        context.with_resources(|resources| resources.work(lookup_work(pending.ordinals.len())?))?;
        let result = ranked_access_source_row_v1::row(
            types,
            function,
            &emitted.blocks,
            source,
            |site| {
                pending
                    .ordinals
                    .iter()
                    .find(|(key, _)| *key == site)
                    .map(|(_, n)| *n)
            },
            &mut *context.facts,
        );
        // The same concrete canonical facts may have denied work. Do not hide
        // that denial behind a predicate's return value.
        context.with_resources(|_| Ok(()))?;
        record_access_result(function, &emitted.blocks, source, result, pending)?;
    }
    if add(pending.accesses.len(), pending.unmapped_private_reads)? != emitted.sources.len() {
        return Err(Error::Incomplete(
            "nominal filtered source accounting differs",
        ));
    }
    Ok(())
}

// The original retained map is a filtered correspondence view, not the whole
// mandatory graph. A legitimate private read may lack its narrower map fact.
// Keep that operation and its full source row; never manufacture a retained row.
// Fixed checks below fit the existing 64-unit per-source lookup allowance.
fn record_access_result(
    function: &SemanticFunctionDeclV1,
    blocks: &[ProductionRankedBlockV1],
    source: &ProjectedAccessSourceV1,
    result: Result<Option<ProductionRankedAccessSourceV1>>,
    pending: &mut Pending,
) -> Result<()> {
    let row = match result? {
        Some(row) => row,
        None => {
            let site = source.semantic_site.ok_or(Error::Incomplete(
                "unmapped private read has no semantic site",
            ))?;
            let block = function.blocks().get(site.block).ok_or(Error::Incomplete(
                "unmapped private read has an invalid semantic block",
            ))?;
            if source.memory_space != MemorySpaceAttr::Private
                || source.access != AccessKindAttr::Read
                || site
                    .statement
                    .is_some_and(|n| n >= block.statements().len())
                || !matches!(
                    blocks
                        .get(source.block)
                        .and_then(|b| b.operations().get(source.operation)),
                    Some(ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        ..
                    })
                )
            {
                return Err(Error::Incomplete(
                    "unexpected unmapped nominal source access",
                ));
            }
            pending.unmapped_private_reads = add(pending.unmapped_private_reads, 1)?;
            return Ok(());
        }
    };
    let site = source
        .semantic_site
        .ok_or(Error::Incomplete("retained source site disappeared"))?;
    if let Some((_, ordinal)) = pending.ordinals.iter_mut().find(|(key, _)| *key == site) {
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(Error::Unsupported("semantic access ordinal overflow"))?;
    } else {
        pending.ordinals.push((site, 1));
    }
    pending.accesses.push(row);
    Ok(())
}

fn enter_compiler(
    pending: &mut Pending,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    if !resources.is_metered()
        || resources.has_denial()
        || pending.phase != Phase::Entered
        || !pending.lowering.is_empty()
        || pending.lowering.capacity() != 0
    {
        return Err(Error::Incomplete("nominal ranked compiler is one-shot"));
    }
    pending.phase = Phase::Compiling;
    // Admit the exact one-element backing before allocator entry or moving the
    // source blocks. The existing fallible policy also checks actual capacity.
    // Pending keeps this slot, including on compile refusal, until outer drop.
    resources.reserve(&mut pending.lowering, 1)
}

fn compile_payload(
    name: &str,
    emitted: &mut emission::Emitted,
    pending: &mut Pending,
    analysis: Analysis,
    snapshot: Snapshot,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(32)?;
    if emitted.blocks.is_empty() {
        return Err(Error::Incomplete(
            "nominal ranked compiler lacks its source blocks",
        ));
    }
    enter_compiler(pending, resources)?;
    // All source assembly allocations remain in Pending. Only this exact block
    // Vec crosses the existing consuming constructor. Its checked transforms
    // perform their original position-preserving replay; no alternate engine.
    let blocks = std::mem::take(&mut emitted.blocks);
    let kernel = ProductionRankedKernelV1::new(name, emitted.controls.argument_count(), blocks)
        .map_err(Error::Recipe)?;
    check_transformed(&kernel, emitted, pending, resources)?;
    let construction = ProductionConstructionV1::ranked_kernel(ROOT_NAME_V1, kernel)
        .map_err(Error::Construction)?;
    let result = compile(
        construction,
        ProductionSessionLimitsV1::default(),
        analysis,
        snapshot,
    );
    match result {
        Ok(lowering) => pending.lowering.push(lowering),
        Err(error) => {
            return Err(Error::Compile {
                error: Box::new(error),
                ranked_ir: String::new(),
                access_sources: Vec::new(),
            });
        }
    }
    check_transformed(
        pending.lowering.first().expect("actual lowering").kernel(),
        emitted,
        pending,
        resources,
    )?;
    pending.phase = Phase::Verified;
    Ok(())
}

impl<'a, 'g> ActualRootBlockStreamV1<'a, 'g> {
    /// Consumes the original emitted Vec once, using the original paid context.
    /// Constructor/transforms, initial recipe hashing, Context/dialect creation
    /// and Display internals remain excluded from the selected allowance model.
    /// Unknown CFG splits remain unknown; the mandatory verifier may refuse.
    pub(in crate::production_ranked_projection_v1) fn verify_ranked(
        self,
        context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
        analysis: Analysis,
        snapshot: Snapshot,
    ) -> Result<Verified<'a>> {
        let Self {
            flow,
            prefix,
            emitted,
            consumer,
            ledger,
            ..
        } = self;
        let context_function = context.function() as *const SemanticFunctionDeclV1;
        context.with_resources(|resources| {
            resources.work(64)?;
            check(resources, ledger)?;
            if !std::ptr::eq(prefix.function, context_function) {
                return Err(Error::Incomplete(
                    "nominal verifier changed its canonical source context",
                ));
            }
            if !prefix.references.is_empty() || prefix.prefix.reserved_reference_values.is_some() {
                return Err(Error::Incomplete(
                    "nominal verifier cannot drop reference-effect obligations",
                ));
            }
            emitted.controls.rejoin(flow, resources)?;
            pay(consumer, analysis, snapshot, resources)?;
            validate_payload(emitted, consumer, resources)
        })?;
        map_accesses(emitted, prefix.function, consumer, context)?;
        let name = function_name(prefix.function)?;
        context.with_resources(|resources| {
            compile_payload(name, emitted, consumer, analysis, snapshot, resources)
        })?;
        Ok(Verified {
            lowering: consumer.lowering.first().expect("retained actual session"),
            accesses: &consumer.accesses,
            emitted_sources: &emitted.sources,
            unmapped_private_reads: consumer.unmapped_private_reads,
            tensors: &emitted.tensors,
        })
    }
}

#[cfg(test)]
#[path = "bf16_nominal_ranked_consumer_v1_tests.rs"]
mod tests;
