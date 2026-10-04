//! Source-owned block-stream emission in the original pending root assembly.
//! Not a normal-route switch. The emitted recipe still requires complete CFG,
//! mandatory ranked verification and exact executable/source correlation.
use super::*;
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::NominalPreparedControlFlowV1;
use crate::production_ranked_projection_v1::root_checked_reference_use_preparation_v1::{
    append_checked_reference_site_v1, bind_projected_access_site_with_resources_v1,
};
use crate::production_ranked_projection_v1::root_local_contracts_v1::{
    BorrowedLocalContractsV1, SourceLocalContractPartsV1,
};
#[path = "bf16_nominal_checked_control_v1.rs"]
mod checked_control;
#[path = "bf16_nominal_ranked_consumer_v1.rs"]
mod consumer;
#[path = "bf16_nominal_block_stream_emit_v1.rs"]
mod emission;
#[path = "bf16_nominal_block_stream_uses_v1.rs"]
mod source_uses;

type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
const MAX_BLOCKS: usize = 32;
const MAX_SOURCE_SITES: usize = 4096;

#[derive(Clone, Copy)]
struct PrivateView {
    value: ProductionRankedValueIdV1,
    extent: u64,
    element_width: u32,
}
/// Attached to the SAME pending root before any source/facts callback. Scratch
/// and partial output remain here until that outer owner's postflight/drop.
pub(super) struct BlockStream {
    rows: Vec<ProjectedSemanticBlockV1>,
    private_views: Vec<Option<PrivateView>>,
    guard_scratch: Vec<GuardedAccessSiteV1>,
    operation_scratch: Option<ProductionRankedOperationV1>,
    ledger: Option<Ledger>,
    started: bool,
    complete: bool,
    source_sites: usize,
    emitted_items: usize,
    emitted: emission::Emitted,
    consumer: consumer::Pending,
}
impl BlockStream {
    pub(super) const fn empty() -> Self {
        Self {
            rows: Vec::new(),
            private_views: Vec::new(),
            guard_scratch: Vec::new(),
            operation_scratch: None,
            ledger: None,
            started: false,
            complete: false,
            source_sites: 0,
            emitted_items: 0,
            emitted: emission::Emitted::empty(),
            consumer: consumer::Pending::new(),
        }
    }
}

/// A lexical stream, not a detached "ready" token. A later consuming emitter
/// must use these exact rows and the original prefix/SSA namespace.
pub(in crate::production_ranked_projection_v1) struct ActualRootBlockStreamV1<'a, 'g> {
    flow: &'a NominalPreparedControlFlowV1<'a, 'g>,
    prefix: ActualRootPrefixIndicesV1<'a>,
    rows: &'a [ProjectedSemanticBlockV1],
    emitted: &'a mut emission::Emitted,
    consumer: &'a mut consumer::Pending,
    ledger: Ledger,
    guarded: &'a RootGuardedAccessStorageV1,
    origins: &'a UnjoinedReferenceOriginPayloadV1,
}
impl ActualRootBlockStreamV1<'_, '_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.prefix.function
    }
    pub(in crate::production_ranked_projection_v1) fn rows(&self) -> &[ProjectedSemanticBlockV1] {
        self.rows
    }
    pub(in crate::production_ranked_projection_v1) fn entry_operations(
        &self,
    ) -> &[ProductionRankedOperationV1] {
        &self.prefix.prefix.entry_operations
    }
    pub(in crate::production_ranked_projection_v1) fn blocks(&self) -> &[ProductionRankedBlockV1] {
        &self.emitted.blocks
    }
    pub(in crate::production_ranked_projection_v1) fn access_sources(
        &self,
    ) -> &[ProjectedAccessSourceV1] {
        &self.emitted.sources
    }
    pub(super) fn tensor_sites(&self) -> &[emission::TensorSite] {
        &self.emitted.tensors
    }
    pub(in crate::production_ranked_projection_v1) fn source_block_base(
        &self,
        source: usize,
    ) -> Option<usize> {
        self.emitted.base.get(source).copied().flatten()
    }
    pub(in crate::production_ranked_projection_v1) fn terminators(
        &self,
    ) -> &[ProjectedCfgTerminatorV1] {
        self.flow.terminators()
    }
}

fn check(resources: &PreparationResourcesV1<'_, '_>, ledger: Ledger) -> Result<()> {
    if !resources.is_metered()
        || resources.has_denial()
        || resources.original_ledger_v1() != Some(ledger)
    {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}
fn stream_frame<R, F>() -> Result<usize> {
    let mut bytes = 8192usize;
    for n in [
        size_of::<BlockStream>(),
        size_of::<ActualRootBlockStreamV1<'static, 'static>>(),
        size_of::<SourceLocalContractPartsV1<'static>>(),
        size_of::<BorrowedLocalContractsV1<'static>>(),
        size_of::<source_uses::Use<'static>>()
            .checked_mul(4)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        size_of::<F>()
            .checked_mul(4)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
        size_of::<Result<R>>()
            .checked_mul(4)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    ] {
        bytes = bytes
            .checked_add(n)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    }
    Ok(bytes)
}
fn reserve_item(
    stream: &mut BlockStream,
    block: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(16)?;
    if stream.emitted_items >= MAX_RANKED_BOUNDS_OPERATIONS {
        return Err(Error::Unsupported(
            "nominal source stream exceeds ranked operation limit",
        ));
    }
    let row = stream
        .rows
        .get_mut(block)
        .ok_or(Error::Incomplete("nominal stream row absent"))?;
    resources.reserve(&mut row.items, 1)?;
    Ok(())
}
fn push_item(stream: &mut BlockStream, block: usize, item: ProjectedBlockItemV1) {
    // Caller has already charged, checked the index and reserved the slot. No
    // fallible operation may occur between extracting an owned payload and this.
    stream.rows[block].items.push(item);
    stream.emitted_items += 1;
}
fn reserve_prefix(
    prefix: &mut RootEntryPrefixV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(16)?;
    if prefix.entry_operations.len() >= MAX_PROJECTED_OPERATIONS_V1 {
        return Err(Error::Unsupported(
            "nominal prefix exceeds ranked operation limit",
        ));
    }
    resources.reserve(&mut prefix.entry_operations, 1)
}
fn private_coordinate(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    constants: &[Option<u64>],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Option<(usize, u64, u64, u32)>> {
    resources.work(
        place
            .projections()
            .len()
            .checked_add(64)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    let local_index = place.local().index() as usize;
    let local = function
        .locals()
        .get(local_index)
        .ok_or(Error::Unsupported("nominal source use local absent"))?;
    // Exact no-memory branch: transparent components without any indexing or
    // dereference do not load a private array merely by transporting its value.
    if place.projections().iter().all(|p| {
        matches!(
            p.kind(),
            SemanticProjectionKindV1::Field(_)
                | SemanticProjectionKindV1::Downcast(_)
                | SemanticProjectionKindV1::OpaqueCast
                | SemanticProjectionKindV1::Subtype
        )
    }) {
        return Ok(None);
    }
    // This first consumer deliberately does NOT invent dynamic bounds, scalar
    // private borrows, global pointer provenance or multi-rank array handling.
    if local.role().is_entry_argument() {
        return Err(Error::Incomplete(
            "nominal indexed source argument requires ordinary allocation projection",
        ));
    }
    let [projection] = place.projections() else {
        return Err(Error::Incomplete(
            "nominal private source use requires one exact array index",
        ));
    };
    let Some(ty) = types.get(local.ty().index() as usize) else {
        return Err(Error::Unsupported("nominal private source type absent"));
    };
    let SemanticTypeShapeV1::Array { length, element } = ty.shape() else {
        return Err(Error::Incomplete(
            "nominal private index is not rooted in an array",
        ));
    };
    if place.ty() != *element || projection.result_type() != *element {
        return Err(Error::Incomplete(
            "nominal private index element type differs",
        ));
    }
    let value = match projection.kind() {
        SemanticProjectionKindV1::ConstantIndex {
            offset, from_end, ..
        } => {
            if from_end {
                length.checked_sub(offset).ok_or(Error::Incomplete(
                    "nominal private from-end index underflow",
                ))?
            } else {
                offset
            }
        }
        SemanticProjectionKindV1::Index(index) => constants
            .get(index.index() as usize)
            .copied()
            .flatten()
            .ok_or(Error::Incomplete(
                "nominal private dynamic index requires original bounds projection",
            ))?,
        _ => {
            return Err(Error::Incomplete(
                "nominal private source projection unsupported",
            ));
        }
    };
    if value >= *length {
        return Err(Error::Incomplete(
            "nominal private constant index is out of bounds",
        ));
    }
    let width = type_width(types, place.ty())?;
    Ok(Some((local_index, *length, value, width)))
}
fn address(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    local: &BorrowedLocalContractsV1<'_>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(
        place
            .projections()
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(64))
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    let index = place.local().index() as usize;
    let decl = function.locals().get(index).ok_or(Error::Unsupported(
        "an address formation with an out-of-range local",
    ))?;
    let mut current = decl.ty();
    let mut crossed = false;
    for (ordinal, projection) in place.projections().iter().enumerate() {
        match projection.kind() {
            SemanticProjectionKindV1::Dereference if ordinal == 0 => {
                let ty = types
                    .get(current.index() as usize)
                    .ok_or(Error::Unsupported(
                        "an address formation with an out-of-range type",
                    ))?;
                let SemanticTypeShapeV1::Pointer(pointer) = ty.shape() else {
                    return Err(Error::Unsupported(
                        "an address formation whose dereferenced type is not a pointer",
                    ));
                };
                memory_space(pointer.address_space())?;
                crossed = true;
            }
            SemanticProjectionKindV1::Field(_)
            | SemanticProjectionKindV1::Downcast(_)
            | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype => {}
            SemanticProjectionKindV1::Dereference
            | SemanticProjectionKindV1::Index(_)
            | SemanticProjectionKindV1::ConstantIndex { .. }
            | SemanticProjectionKindV1::Subslice { .. } => {
                return Err(Error::Incomplete(
                    "indexed address formation before exact bounds-only projection",
                ));
            }
        }
        current = projection.result_type();
    }
    if crossed
        && local.allocation(index).is_none()
        && !matches!(
            local.allocation_provenance(index),
            Some(LocalAllocationProvenanceV1::Private(_))
        )
    {
        return Err(Error::MissingAllocationProvenance {
            local: place.local().index(),
            projections: place.projections().len(),
            ty: decl.ty().index(),
        });
    }
    Ok(())
}
fn emit_private(
    stream: &mut BlockStream,
    prefix: &mut RootEntryPrefixV1,
    occurrence: root_checked_reference_use_preparation_v1::SourceUseOccurrenceV1<'_>,
    coordinate: (usize, u64, u64, u32),
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    let (local, extent, index, width) = coordinate;
    let cached = *stream
        .private_views
        .get(local)
        .ok_or(Error::Incomplete("nominal private view slot absent"))?;
    let view = match cached {
        Some(view) if view.extent == extent && view.element_width == width => view.value,
        Some(_) => {
            return Err(Error::Incomplete(
                "one semantic allocation used through inconsistent ranked views",
            ));
        }
        None => {
            reserve_prefix(prefix, resources)?;
            let value = next_value_id(&mut prefix.next_value)?;
            let identity = PRIVATE_ALLOCATION_ORIGIN_TAG_V1
                .checked_add(local as u64)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            stream.operation_scratch = Some(ProductionRankedOperationV1::ViewInSpace {
                result: value,
                element_width: width,
                writable: true,
                shape: Vec::new(),
                dynamic_extents: Vec::new(),
                memory_space: MemorySpaceAttr::Private,
                allocation_origin: identity,
                noalias_class: identity,
            });
            let Some(ProductionRankedOperationV1::ViewInSpace { shape, .. }) =
                stream.operation_scratch.as_mut()
            else {
                unreachable!()
            };
            resources.push(shape, extent)?;
            prefix.entry_operations.push(
                stream
                    .operation_scratch
                    .take()
                    .expect("retained private view"),
            );
            stream.private_views[local] = Some(PrivateView {
                value,
                extent,
                element_width: width,
            });
            value
        }
    };
    reserve_prefix(prefix, resources)?;
    let coordinate = next_value_id(&mut prefix.next_value)?;
    prefix
        .entry_operations
        .push(ProductionRankedOperationV1::IndexConstant {
            result: coordinate,
            value: index,
        });
    reserve_item(stream, occurrence.site.block, resources)?;
    stream.operation_scratch = Some(ProductionRankedOperationV1::Access {
        kind: occurrence.access,
        view: ProductionRankedValueV1::Local(view),
        indices: Vec::new(),
    });
    let Some(ProductionRankedOperationV1::Access { indices, .. }) =
        stream.operation_scratch.as_mut()
    else {
        unreachable!()
    };
    resources.push(indices, ProductionRankedValueV1::Local(coordinate))?;
    let source = ProjectedEffectSourceV1 {
        access: occurrence.access,
        memory_space: MemorySpaceAttr::Private,
        source: occurrence.source,
        output_extent: None,
        semantic_site: Some(occurrence.site),
    };
    let operation = stream
        .operation_scratch
        .take()
        .expect("retained private access");
    push_item(
        stream,
        occurrence.site.block,
        ProjectedBlockItemV1::Effect {
            operation,
            source: Some(source),
        },
    );
    Ok(())
}
fn emit_use(
    event: source_uses::Use<'_>,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    rich: &RichNominalSourceTablesV1<'_>,
    local: &BorrowedLocalContractsV1<'_>,
    guarded: &[GuardedRankedAccessV1],
    prefix: &mut RootEntryPrefixV1,
    stream: &mut BlockStream,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    let occurrence = match event {
        source_uses::Use::Address(place) => {
            return address(types, function, place, local, resources);
        }
        source_uses::Use::Value(occurrence) => occurrence,
    };
    resources.work(
        occurrence
            .place
            .projections()
            .len()
            .checked_add(64)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    if occurrence.access.is_atomic() != occurrence.atomic.is_some() {
        return Err(Error::Unsupported(
            "an atomic access whose ordering/scope contract is missing or attached to a non-atomic access",
        ));
    }
    // Original checked-reference precedence, including payload-availability
    // refusal, is preserved before the narrower ordinary fallback.
    if let Some(origin) = local.origin(occurrence.place, occurrence.site.block)? {
        if !stream.guard_scratch.is_empty() {
            return Err(resource(Resource::Accounting));
        }
        reserve_item(stream, occurrence.site.block, resources)?;
        append_checked_reference_site_v1(
            origin,
            occurrence.access,
            occurrence.atomic,
            occurrence.source,
            guarded,
            &mut stream.guard_scratch,
            &[],
            resources,
        )?;
        bind_projected_access_site_with_resources_v1(
            &mut [],
            &mut stream.guard_scratch,
            occurrence.site,
            resources,
        )?;
        if let Some(site) = stream.guard_scratch.pop() {
            if !stream.guard_scratch.is_empty() || site.insertion_operation != 0 {
                return Err(resource(Resource::Accounting));
            }
            push_item(
                stream,
                occurrence.site.block,
                ProjectedBlockItemV1::Guarded(site.access),
            );
        }
        return Ok(());
    }
    if occurrence.atomic.is_some() {
        return Err(Error::Incomplete(
            "nominal atomic fallback is not connected",
        ));
    }
    let coordinate = private_coordinate(
        types,
        function,
        occurrence.place,
        rich.constants(),
        resources,
    )?;
    match coordinate {
        Some(coordinate) => emit_private(stream, prefix, occurrence, coordinate, resources),
        None if occurrence.requirement == PlaceAccessRequirementV1::IfMemory => Ok(()),
        None => Err(Error::Incomplete(
            "an explicit memory operation without a ranked index projection",
        )),
    }
}
fn append_effects(
    stream: &mut BlockStream,
    block: usize,
    row: &ProjectedCapabilityTerminatorEffectsV1,
    source: SemanticSourceProvenanceV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(64)?;
    if row.transpose_workgroup.is_some() || row.read_view.is_some() {
        return Err(Error::Incomplete(
            "nominal block stream has unconnected transpose/read-view effects",
        ));
    }
    if let Some(operation) = &row.layout {
        let ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
            binding,
        } = operation
        else {
            return Err(Error::Incomplete(
                "nominal retained layout is not a fixed tensor operation",
            ));
        };
        reserve_item(stream, block, resources)?;
        push_item(
            stream,
            block,
            ProjectedBlockItemV1::Effect {
                operation: ProductionRankedOperationV1::TensorLayout {
                    contract: *contract,
                    convergence: *convergence,
                    active_lanes: *active_lanes,
                    binding: *binding,
                },
                source: None,
            },
        );
    }
    if let Some(effect) = row.global_read {
        reserve_item(stream, block, resources)?;
        push_item(
            stream,
            block,
            ProjectedBlockItemV1::Effect {
                operation: ProductionRankedOperationV1::AllocationEffect {
                    kind: AccessKindAttr::Read,
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: effect.allocation_origin,
                    noalias_class: effect.noalias_class,
                },
                source: Some(ProjectedEffectSourceV1 {
                    access: AccessKindAttr::Read,
                    memory_space: MemorySpaceAttr::Global,
                    source,
                    output_extent: None,
                    semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                        block,
                        statement: None,
                    }),
                }),
            },
        );
    }
    Ok(())
}

fn populate(
    parts: &mut ActualRootAssemblyPartsV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    flow: &NominalPreparedControlFlowV1<'_, '_>,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    let ledger = resources
        .original_ledger_v1()
        .ok_or_else(|| resource(Resource::Accounting))?;
    check(resources, ledger)?;
    resources.work(128)?;
    let stream = &mut *parts.stream;
    if stream.started
        || stream.complete
        || stream.ledger.is_some()
        || !stream.rows.is_empty()
        || stream.rows.capacity() != 0
        || !stream.private_views.is_empty()
        || stream.private_views.capacity() != 0
        || !stream.guard_scratch.is_empty()
        || stream.guard_scratch.capacity() != 0
        || stream.operation_scratch.is_some()
        || stream.source_sites != 0
        || stream.emitted_items != 0
    {
        return Err(Error::Incomplete(
            "nominal block stream cannot be replaced or retried",
        ));
    }
    stream.started = true;
    stream.ledger = Some(ledger);
    let function = parts.function;
    let blocks = function.blocks().len();
    if !(1..=MAX_BLOCKS).contains(&blocks)
        || !std::ptr::eq(flow.effects().original().function(), function)
        || !std::ptr::eq(flow.assertions().cfg().function(), function)
        || flow.effects().effects().len() != blocks
        || flow.terminators().len() != blocks
        || flow.assertions().decisions().len() != blocks
        || !parts.references.is_empty()
        || parts.prefix.reserved_reference_values.is_some()
    {
        return Err(Error::Incomplete(
            "nominal block stream source/row/reference profile differs",
        ));
    }
    let origins = parts
        .origins
        .payload()
        .ok_or(Error::Incomplete("nominal block stream origins unfinished"))?;
    let local = BorrowedLocalContractsV1::source_data(
        SourceLocalContractPartsV1 {
            function,
            table_function: rich.function(),
            counts: rich.scalar_counts(),
            assignments: rich.scalar_assignments(),
            address_escaped: rich.address_escaped(),
            allocations: rich.allocations(),
            allocation_provenance: rich.allocation_provenance(),
            origins: &origins.origins,
            option_dominance: rich.option_dominance(),
            enum_payload_dominance: rich.enum_payload_dominance(),
        },
        resources,
    )?;
    resources.work(blocks)?;
    resources.reserve(&mut stream.rows, blocks)?;
    stream
        .rows
        .resize_with(blocks, || ProjectedSemanticBlockV1 { items: Vec::new() });
    resources.work(function.locals().len())?;
    resources.reserve(&mut stream.private_views, function.locals().len())?;
    stream.private_views.resize(function.locals().len(), None);
    for (block_index, block) in function.blocks().iter().enumerate() {
        resources.work(32)?;
        if matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Assert {
                message: SemanticAssertMessageV1::BoundsCheck { .. },
                ..
            }
        ) && !flow.assertions().decisions()[block_index]
        {
            return Err(Error::Incomplete(
                "nominal source bounds assertion requires exact access authorization",
            ));
        }
        for statement in 0..=block.statements().len() {
            resources.work(16)?;
            let site = ProjectedSemanticAccessSiteV1 {
                block: block_index,
                statement: (statement < block.statements().len()).then_some(statement),
            };
            if stream.source_sites == MAX_SOURCE_SITES {
                return Err(Error::Incomplete(
                    "nominal source block stream exceeds site profile",
                ));
            }
            stream.source_sites += 1;
            // Complete-profile graph already authenticated the sole Defined
            // call and the exact closed intrinsic roster. This explicit
            // rejoin prevents a detached prepared flow authorizing another Call.
            if site.statement.is_none() {
                if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                    match callables.get(call.callee().index() as usize) {
                        Some(SemanticCallableDeclV1::Defined { .. })
                            if std::ptr::eq(call, flow.effects().original().candidate().source_call())
                                && block_index == flow.effects().source_block().index() as usize => {},
                        Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. })
                            if matches!(operation,
                                SemanticCompilerIntrinsicOperationV1::Trap
                                | SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { .. }
                                | SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { .. }
                                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor { .. }
                                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewColumnMajor { .. }
                                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
                                | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. }
                                | SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. }
                                | SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }) => {},
                        _ => return Err(Error::Incomplete("nominal stream call/effect owner differs")),
                    }
                }
            }
            source_uses::visit(function, site, resources, |event, resources| {
                emit_use(
                    event,
                    function,
                    types,
                    rich,
                    &local,
                    &parts.guarded.accesses,
                    parts.prefix,
                    stream,
                    resources,
                )
            })?;
        }
        append_effects(
            stream,
            block_index,
            &flow.effects().effects()[block_index],
            block.terminator().source(),
            resources,
        )?;
    }
    check(resources, ledger)?;
    if !stream.guard_scratch.is_empty()
        || stream.operation_scratch.is_some()
        || stream.rows.len() != function.blocks().len()
    {
        return Err(resource(Resource::Accounting));
    }
    stream
        .emitted
        .controls
        .prepare(flow, parts.prefix, resources)?;
    emission::emit(
        &stream.rows,
        &parts.prefix.entry_operations,
        flow.terminators(),
        function.entry().index() as usize,
        &mut stream.emitted,
        resources,
    )?;
    stream.complete = true;
    Ok(())
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    /// Emits the actual source block stream in the existing physical pending
    /// assembly. No copied capability tables, independent value namespace,
    /// source restart, new ledger, refund or normal-route gate change.
    pub(in crate::production_ranked_projection_v1) fn with_actual_root_block_stream_v1<'g, R, F>(
        &mut self,
        checked: &CheckedBf16NominalCallV1<'g>,
        rich: &RichNominalSourceTablesV1<'_>,
        actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
        flow: &NominalPreparedControlFlowV1<'_, 'g>,
        pending: &mut PendingActualRootPrefixIndicesV1,
        inspect: F,
    ) -> Result<R>
    where
        F: for<'a> FnOnce(ActualRootBlockStreamV1<'a, 'g>, &mut Self) -> Result<R>,
    {
        self.with_resources(|resources| {
            resources.work(128)?;
            let candidate = flow.effects().original().candidate();
            if !std::ptr::eq(candidate.owner(), checked.emission().owner())
                || !checked.belongs_to(candidate.inventory())
                || !std::ptr::eq(candidate.source_call(), checked.source_call())
                || flow.effects().source_block() != checked.emission().source_call_block()
                || candidate.permutation() != checked.emission().return_permutation()
            {
                return Err(Error::Incomplete(
                    "nominal block stream actual call/Return owner differs",
                ));
            }
            let frame = stream_frame::<R, F>()?;
            resources.work(frame)?;
            resources.reserve_storage(frame)
        })?;
        self.with_actual_root_assembly_v1(
            checked,
            rich,
            actual_inputs,
            pending,
            |mut parts, context| {
                let owner = context.facts.owner;
                let source = owner.semantic_ssa().source_semantic();
                context.with_resources(|resources| {
                    prepare_root_guarded_accesses_v1(
                        source.types(),
                        source.callables(),
                        parts.function,
                        &parts.indices.indices,
                        rich.option_dominance(),
                        rich.enum_payload_dominance(),
                        rich.allocations(),
                        rich.allocation_provenance(),
                        &mut parts.indices.predicates,
                        parts.guarded,
                        &mut parts.prefix.entry_operations,
                        &mut parts.prefix.next_value,
                        resources,
                    )?;
                    prepare_actual_root_reference_origins_v1(
                        parts.function,
                        source.callables(),
                        parts.guarded,
                        parts.graph.edges(),
                        rich.option_dominance(),
                        rich.enum_payload_dominance(),
                        parts.origins,
                        resources,
                    )?;
                    populate(
                        &mut parts,
                        rich,
                        flow,
                        source.types(),
                        source.callables(),
                        resources,
                    )
                })?;
                if !parts.stream.complete {
                    return Err(Error::Incomplete("nominal block stream did not complete"));
                }
                inspect(
                    ActualRootBlockStreamV1 {
                        flow,
                        prefix: ActualRootPrefixIndicesV1 {
                            graph: parts.graph,
                            source_root: parts.source_root,
                            function: parts.function,
                            input: parts.input,
                            references: parts.references,
                            prefix: parts.prefix,
                            indices: parts.indices,
                        },
                        rows: &parts.stream.rows,
                        emitted: &mut parts.stream.emitted,
                        consumer: &mut parts.stream.consumer,
                        ledger: parts
                            .stream
                            .ledger
                            .ok_or_else(|| resource(Resource::Accounting))?,
                        guarded: parts.guarded,
                        origins: parts
                            .origins
                            .payload()
                            .ok_or(Error::Incomplete("nominal stream origin loan absent"))?,
                    },
                    context,
                )
            },
        )
    }
}

#[cfg(test)]
#[path = "bf16_nominal_block_stream_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bf16_nominal_block_stream_genuine_v1_tests.rs"]
pub(super) mod genuine;
