// Input-only attachment normalization. No output graph, emitter plan or origin
// table is available to this visitor.

pub(super) fn tile_store_payload_operand_v18(
    operation: &Operation,
    expected_pointer: ValueId,
    expected_value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileAttachmentResultV29<u32> {
    budget.charge_work(1)?;
    let (actual_pointer, actual_value, expected, count) = match &operation.kind {
        OperationKind::Store { pointer, value, .. } =>
            (*pointer, *value, [*pointer, *value, ValueId(0)], 2usize),
        OperationKind::GuardedStore { pointer, predicate, value, .. } =>
            (*pointer, *value, [*pointer, *predicate, *value], 3usize),
        _ => return Err(ScopedTileFailureKindV29::ReplayMismatch),
    };
    if actual_pointer != expected_pointer || actual_value != expected_value || !operation.results.is_empty() {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    }
    // Validate every actual operand occurrence, including equal-valued operands.
    let mut visited = 0usize;
    let mut rhs = None;
    operation.kind.try_visit_operands(|operand| -> TileAttachmentResultV29<()> {
        budget.charge_work(1)?;
        if visited >= count || expected[visited] != operand {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        rhs = Some(tile_attachment_u32_v29(visited)?);
        visited = visited.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(())
    })?;
    if visited != count { return Err(ScopedTileFailureKindV29::ReplayMismatch); }
    rhs.ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TileAttachmentFamilyV29 {
    InstanceSpans,
    InstanceSeeds,
    InstanceControls,
    InstanceCalls,
    InstanceReturns,
    RawSidecar,
    SourceSlot,
    MemoryAnchor,
    PrivateArray,
    Lifecycle,
    Assertion,
    TerminalFailure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TileAttachmentFieldV29 {
    Span,
    Parameters,
    PhysicalBlock,
    Terminator,
    Edge,
    EdgeArgument,
    ReturnDefinition,
    ReturnUse,
    ExpectedTarget,
    ExpectedArgument,
    ArgumentPreparation,
    CallSite,
    DestinationPreparation,
    DestinationRange,
    DestinationPointer,
    ArgumentDefinition,
    ArgumentUse,
    ResultDefinition,
    ReturnSite,
    ReturnComponentInput,
    ReturnComponentConversion,
    ReturnComponentOutput,
    ReturnComponentUse,
    TransportComponentConversion,
    TransportComponentOutput,
    TransportComponentUse,
    RawBlock,
    RawStatementSpan,
    RawTerminatorSpan,
    RawSyntheticSpan,
    RawInvocationSpan,
    RawInvocationPreheader,
    RawInvocationEntry,
    RawInvocationTerminator,
    RawInvocationEdge,
    RawInvocationArgument,
    RawInvocationInputMap,
    RawInvocationInput,
    RawInvocationParameter,
    RawInvocationOutput,
    RawInvocationConversion,
    RawParameter,
    RawParameterComponent,
    RawIgnoredParameter,
    RawGeneratedInput,
    RawGeneratedOutput,
    RawCallArguments,
    RawCallOperation,
    RawCallDestination,
    RawCallDestinationPointer,
    RawNoNormalReturnTerminator,
    RawReturnTerminator,
    RawReturnInput,
    RawReturnConversion,
    RawTransportArgument,
    RawTransportConversion,
    SlotRawPointer,
    SlotPointer,
    SlotCount,
    SlotCountLocation,
    SlotAllocation,
    MemoryPosition,
    MemoryPointer,
    MemoryLoadResult,
    MemoryStoreValue,
    MemoryStoreUse,
    ArrayCountLocation,
    ArrayAllocation,
    ArrayPointer,
    ArrayCount,
    ArraySourceRange,
    ArrayOriginalIndex,
    ArrayDirectDefinition,
    ArrayLiteralValue,
    ArrayLiteralDefinition,
    ArrayOffsetLocation,
    ArrayGepLocation,
    ArrayMemoryLocation,
    ArrayOffset,
    ArrayGep,
    LifecycleOriginalGap,
    LifecycleBeforeGap,
    LifecycleOperation,
    LifecycleOperand,
    LifecycleResult,
    AssertSourceRange,
    AssertSourceBlock,
    AssertSuccessBlock,
    AssertFailureBlock,
    AssertCapturedCondition,
    AssertCapturedArgument,
    AssertConditionUse,
    AssertConditionDefinition,
    AssertSuccessEdge,
    AssertFailureEdge,
    AssertSuccessArgument,
    FailureSourceBlock,
    FailureSourceEdge,
    FailureOriginalTarget,
    FailureOriginalDiagnostic,
    FailureBlock,
    FailureGap,
    FailureCleanup,
    FailureOperand,
    FailureDiagnostic,
    FailureTerminator,
    ObjectOperand,
    ObjectOperandUse,
    ObjectResult,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TileAttachmentKeyV29 {
    pub(super) root: usize,
    pub(super) family: TileAttachmentFamilyV29,
    // Original source instance ID, never the compact sidecar ordinal.
    pub(super) instance: usize,
    pub(super) row: usize,
    pub(super) field: TileAttachmentFieldV29,
    pub(super) component: usize,
    pub(super) part: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TileAttachmentLocationV29 {
    Origin(TileScalarSourceV29),
    Gap(TileScalarPointV29),
    Use(fe2o3_kernel_ir::CanonicalKirUseCoordinateV1),
    EdgeArgument(fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1),
    Tombstone,
    NoOutput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TileAttachmentOutputV29 {
    Origin { first: usize, count: usize },
    Gap(TileScalarPointV29),
    Use(fe2o3_kernel_ir::CanonicalKirUseCoordinateV1),
    EdgeArgument(fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1),
    Tombstone,
    NoOutput,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TileScalarProjectionV29 {
    pub(super) key: TileAttachmentKeyV29,
    pub(super) source: TileAttachmentLocationV29,
    pub(super) target: TileAttachmentOutputV29,
}
pub(super) struct TileScalarProjectionsV29 {
    pub(super) rows: Vec<TileScalarProjectionV29>,
}
impl TileScalarProjectionsV29 {
    pub(super) fn storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_product_v1(self.rows.capacity(), size_of::<TileScalarProjectionV29>())
    }
}
type TileAttachmentResultV29<T> = Result<T, ScopedTileFailureKindV29>;

#[derive(Clone, Copy)]
enum TileAttachmentIndexV18<'i, 'g> {
    Legacy(&'i AssertGraphIndexV1<'g>),
    Inventory(&'i fe2o3_kernel_analysis::CanonicalKirInventoryV18<'g>),
}

fn tile_attachment_inventory_error_v18(
    error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
) -> ScopedTileFailureKindV29 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
        _ => ScopedTileFailureKindV29::ReplayMismatch,
    }
}

impl TileAttachmentIndexV18<'_, '_> {
    fn definition(
        self,
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentResultV29<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1> {
        match self {
            Self::Legacy(index) => Ok(index
                .definition(function, value, budget)
                .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?
                .coordinate),
            Self::Inventory(index) => index
                .definition_for_value(function, value, budget)
                .map_err(tile_attachment_inventory_error_v18)?
                .map(|row| row.coordinate)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch),
        }
    }

    fn block(
        self,
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentResultV29<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1> {
        match self {
            Self::Legacy(index) => Ok(index
                .block(function, block, budget)
                .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?),
            Self::Inventory(index) => index
                .block_for_id(function, block, budget)
                .map_err(tile_attachment_inventory_error_v18)?
                .map(|row| row.coordinate)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch),
        }
    }
}

struct TileAttachmentWalkV29<'a, 'i, 'b, 'work> {
    pending: &'a ProductionPendingScopedSourceOwnerV29,
    root_ordinal: usize,
    root: &'a ScopedModuleRootV29,
    graph: &'a Module,
    index: TileAttachmentIndexV18<'i, 'a>,
    budget: &'b mut ArgumentBudgetV1<'work>,
    visit: &'b mut dyn FnMut(
        TileAttachmentKeyV29,
        TileAttachmentLocationV29,
        &mut ArgumentBudgetV1<'work>,
    ) -> TileAttachmentResultV29<()>,
}
impl TileAttachmentWalkV29<'_, '_, '_, '_> {
    fn key(
        &self,
        family: TileAttachmentFamilyV29,
        instance: usize,
        row: usize,
        field: TileAttachmentFieldV29,
    ) -> TileAttachmentKeyV29 {
        TileAttachmentKeyV29 {
            root: self.root_ordinal,
            family,
            instance,
            row,
            field,
            component: 0,
            part: 0,
        }
    }
    fn emit(
        &mut self,
        key: TileAttachmentKeyV29,
        location: TileAttachmentLocationV29,
    ) -> TileAttachmentResultV29<()> {
        self.budget.charge_work(1)?;
        (self.visit)(key, location, self.budget)
    }
    fn body(&self) -> TileAttachmentResultV29<&FunctionBody> {
        self.graph
            .functions
            .get(self.root.function_ordinal)
            .and_then(|f| f.body.as_ref())
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
    }
    fn block_ordinal(&mut self, block: BlockId) -> TileAttachmentResultV29<usize> {
        self.budget.charge_work(self.body()?.blocks.len())?;
        self.body()?
            .blocks
            .iter()
            .position(|b| b.id == block)
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
    }
    fn block(&mut self, key: TileAttachmentKeyV29, block: BlockId) -> TileAttachmentResultV29<()> {
        let block = self.block_ordinal(block)?;
        self.emit(
            key,
            TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                function: self.root.function_ordinal,
                block,
            }),
        )
    }
    fn terminator(
        &mut self,
        key: TileAttachmentKeyV29,
        block: BlockId,
    ) -> TileAttachmentResultV29<()> {
        let block = self.block_ordinal(block)?;
        self.emit(
            key,
            TileAttachmentLocationV29::Origin(TileScalarSourceV29::Terminator {
                function: self.root.function_ordinal,
                block,
            }),
        )
    }
    fn edge(
        &mut self,
        key: TileAttachmentKeyV29,
        block: BlockId,
        edge: usize,
    ) -> TileAttachmentResultV29<()> {
        let block = self.block_ordinal(block)?;
        self.emit(
            key,
            TileAttachmentLocationV29::Origin(TileScalarSourceV29::Edge {
                function: self.root.function_ordinal,
                block,
                edge,
            }),
        )
    }
    fn definition(
        &mut self,
        key: TileAttachmentKeyV29,
        value: ValueId,
    ) -> TileAttachmentResultV29<()> {
        let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(self.root.function_ordinal)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let definition = self.index.definition(function, value, self.budget)?;
        self.canonical_definition(key, definition)
    }
    fn canonical_definition(
        &mut self,
        key: TileAttachmentKeyV29,
        definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    ) -> TileAttachmentResultV29<()> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as D;
        let source = match definition {
            D::FunctionArgument { function, argument } => TileScalarSourceV29::FunctionParameter {
                function: function.0 as usize,
                parameter: argument as usize,
            },
            D::BlockArgument { block, argument } => TileScalarSourceV29::BlockParameter {
                function: block.function.0 as usize,
                block: block.block as usize,
                parameter: argument as usize,
            },
            D::Result { operation, result } => TileScalarSourceV29::Result {
                operation: TileScalarPointV29 {
                    function: operation.block.function.0 as usize,
                    block: operation.block.block as usize,
                    operation: operation.operation as usize,
                },
                result: result as usize,
            },
        };
        self.emit(key, TileAttachmentLocationV29::Origin(source))
    }
    fn canonical_use(
        &mut self,
        key: TileAttachmentKeyV29,
        used: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    ) -> TileAttachmentResultV29<()> {
        self.emit(key, TileAttachmentLocationV29::Use(used))
    }
    fn canonical_edge(
        &mut self,
        key: TileAttachmentKeyV29,
        edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    ) -> TileAttachmentResultV29<()> {
        self.emit(
            key,
            TileAttachmentLocationV29::Origin(TileScalarSourceV29::Edge {
                function: edge.source.function.0 as usize,
                block: edge.source.block as usize,
                edge: edge.successor as usize,
            }),
        )
    }
    fn canonical_edge_argument(
        &mut self,
        key: TileAttachmentKeyV29,
        argument: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
    ) -> TileAttachmentResultV29<()> {
        self.emit(key, TileAttachmentLocationV29::EdgeArgument(argument))
    }
    fn operation_v(
        &mut self,
        key: TileAttachmentKeyV29,
        block: BlockId,
        ordinal: u32,
    ) -> TileAttachmentResultV29<()> {
        let block = self.block_ordinal(block)?;
        if ordinal as usize >= self.body()?.blocks[block].operations.len() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        self.emit(
            key,
            TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(TileScalarPointV29 {
                function: self.root.function_ordinal,
                block,
                operation: ordinal as usize,
            })),
        )
    }
    fn lifecycle_ordinal(
        &mut self,
        block: BlockId,
        p: u32,
        gap: bool,
    ) -> TileAttachmentResultV29<u32> {
        let mut ordinal = p;
        for insertion in &self.root.insertions {
            self.budget.charge_work(1)?;
            if insertion.before.block == block
                && (insertion.before.first < p || (!gap && insertion.before.first == p))
            {
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        let terminal = terminal_failure_ordinal_v18(self.root.terminal_failures.as_ref(), block, p, gap, self.budget)?;
        ordinal.checked_add(terminal.checked_sub(p).ok_or(ArgumentResourceV1::Arithmetic)?)
            .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
    }
    fn operation_c(
        &mut self,
        key: TileAttachmentKeyV29,
        block: BlockId,
        p: u32,
    ) -> TileAttachmentResultV29<()> {
        let ordinal = self.lifecycle_ordinal(block, p, false)?;
        self.operation_v(key, block, ordinal)
    }
    fn gap_c(
        &mut self,
        key: TileAttachmentKeyV29,
        block: BlockId,
        p: u32,
    ) -> TileAttachmentResultV29<()> {
        let ordinal = self.lifecycle_ordinal(block, p, true)?;
        let block = self.block_ordinal(block)?;
        if ordinal as usize > self.body()?.blocks[block].operations.len() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        self.emit(
            key,
            TileAttachmentLocationV29::Gap(TileScalarPointV29 {
                function: self.root.function_ordinal,
                block,
                operation: ordinal as usize,
            }),
        )
    }

    fn emitted_point(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        p: u32,
        gap: bool,
    ) -> TileAttachmentResultV29<Option<(BlockId, u32)>> {
        ScopedEmittedPointsV29 {
            coordinates: &self.root.coordinates,
            relocation: &self.root.slot_relocation,
            budget: self.budget,
        }.emitted_point(instance, block, p, gap)
    }
    fn emitted_operation(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        p: u32,
    ) -> TileAttachmentResultV29<()> {
        match self.emitted_point(instance, block, p, false)? {
            Some((block, p)) => self.operation_c(key, block, p),
            None => self.emit(key, TileAttachmentLocationV29::Tombstone),
        }
    }
    fn emitted_gap(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        p: u32,
    ) -> TileAttachmentResultV29<()> {
        let (block, p) = self
            .emitted_point(instance, block, p, true)?
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        self.gap_c(key, block, p)
    }
    fn emitted_range(
        &mut self,
        mut key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        first: u32,
        end: u32,
    ) -> TileAttachmentResultV29<()> {
        if end < first {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        if first == end {
            return self.emitted_gap(key, instance, block, first);
        }
        for p in first..end {
            self.emitted_operation(key, instance, block, p)?;
            key.part = key
                .part
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Ok(())
    }
    fn emitted_use(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        p: u32,
        operand: u32,
    ) -> TileAttachmentResultV29<()> {
        let Some((block, p)) = self.emitted_point(instance, block, p, false)? else {
            return self.emit(key, TileAttachmentLocationV29::Tombstone);
        };
        let operation = self.lifecycle_ordinal(block, p, false)?;
        let block = u32::try_from(self.block_ordinal(block)?)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(self.root.function_ordinal)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        self.canonical_use(
            key,
            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 { function, block },
                    operation,
                },
                operand,
            },
        )
    }
    fn emitted_store_payload_use_v18(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        position: u32,
        pointer: ValueId,
        value: ValueId,
    ) -> TileAttachmentResultV29<()> {
        let (physical_block, physical_position) = self.emitted_point(instance, block, position, false)?
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        let ordinal = self.lifecycle_ordinal(physical_block, physical_position, false)?;
        let block_ordinal = self.block_ordinal(physical_block)?;
        let operation = self.graph.functions.get(self.root.function_ordinal)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(block_ordinal))
            .and_then(|block| block.operations.get(ordinal as usize))
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        let operand = tile_store_payload_operand_v18(operation, pointer, value, self.budget)?;
        self.emitted_use(key, instance, block, position, operand)
    }

    fn check_emitted_object_v29(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        position: u32,
        payload: &ScopedObjectPayloadV29,
    ) -> TileAttachmentResultV29<()> {
        let (block, position) = self.emitted_point(instance, block, position, false)?
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        let position = self.lifecycle_ordinal(block, position, false)?;
        let block = self.block_ordinal(block)?;
        let operation = self.graph.functions.get(self.root.function_ordinal)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(block))
            .and_then(|block| block.operations.get(position as usize))
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        payload.check_operation(operation, self.budget)?;
        Ok(())
    }

    fn mapped_span(
        &mut self,
        mut key: TileAttachmentKeyV29,
        row: usize,
    ) -> TileAttachmentResultV29<()> {
        let span = *self
            .root
            .coordinates
            .spans
            .rows
            .get(row)
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if span.segments == [None, None] {
            return self.emit(
                key,
                if span.removed_call.is_some() {
                    TileAttachmentLocationV29::Tombstone
                } else {
                    TileAttachmentLocationV29::NoOutput
                },
            );
        }
        for segment in span.segments.into_iter().flatten() {
            let end = segment
                .first
                .checked_add(segment.count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if segment.count == 0 {
                self.gap_c(key, segment.block, segment.first)?;
                key.part = key
                    .part
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            for p in segment.first..=end {
                for insertion in &self.root.insertions {
                    self.budget.charge_work(1)?;
                    if insertion.source_span == row
                        && insertion.before.block == segment.block
                        && insertion.before.first == p
                    {
                        self.operation_v(key, insertion.after.block, insertion.after.first)?;
                        key.part = key
                            .part
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
                if p < end {
                    self.operation_c(key, segment.block, p)?;
                    key.part = key
                        .part
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn visit_tile_attachment_sources_v29<'work>(
    input: &PreparedScopedTileSourceV29,
    budget: &mut ArgumentBudgetV1<'work>,
    mut visit: impl FnMut(
        TileAttachmentKeyV29,
        TileAttachmentLocationV29,
        &mut ArgumentBudgetV1<'work>,
    ) -> TileAttachmentResultV29<()>,
) -> TileAttachmentResultV29<()> {
    let graph = input.pending.inner.pending.graph.module();
    let index = AssertGraphIndexV1::build_functions(&graph.functions, true, budget)
        .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
    visit_pending_attachment_sources_v18(
        &input.pending,
        TileAttachmentIndexV18::Legacy(&index),
        budget,
        &mut visit,
    )?;
    index
        .release(budget)
        .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
    Ok(())
}

pub(super) fn visit_source_attachment_inventory_v18<'work>(
    pending: &ProductionPendingScopedSourceOwnerV29,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    mut visit: impl FnMut(
        TileAttachmentKeyV29,
        TileAttachmentLocationV29,
        &mut ArgumentBudgetV1<'work>,
    ) -> TileAttachmentResultV29<()>,
) -> TileAttachmentResultV29<()> {
    budget.charge_work(2)?;
    if !inventory.belongs_to(&pending.inner.pending.graph)
        || pending.inner.pending.ledger != budget.work_ledger_identity_v1()
    {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    }
    visit_pending_attachment_sources_v18(
        pending,
        TileAttachmentIndexV18::Inventory(inventory),
        budget,
        &mut visit,
    )
}

fn visit_pending_attachment_sources_v18<'a, 'work>(
    pending: &'a ProductionPendingScopedSourceOwnerV29,
    index: TileAttachmentIndexV18<'_, 'a>,
    budget: &mut ArgumentBudgetV1<'work>,
    visit: &mut impl FnMut(
        TileAttachmentKeyV29,
        TileAttachmentLocationV29,
        &mut ArgumentBudgetV1<'work>,
    ) -> TileAttachmentResultV29<()>,
) -> TileAttachmentResultV29<()> {
    let graph = pending.inner.pending.graph.module();
    for (root_ordinal, root) in pending.inner.pending.roots.iter().enumerate() {
        let ScopedModuleRootV29 {
            function_ordinal: _,
            sidecars: _,
            active_instances: _,
            coordinates: _,
            slot_relocation: _,
            source_slots: _,
            insertions: _,
            terminal_failures: _,
            declarations: _,
            private_payload: _,
            requires_context_issue: _,
            inherited_emission_storage: _,
            inherited_assembly_storage: _,
        } = root;
        budget.charge_work(1)?;
        let mut walk = TileAttachmentWalkV29 {
            pending,
            root_ordinal,
            root,
            graph,
            index,
            budget,
            visit,
        };
        walk.walk_core_v29()?;
        walk.walk_sidecars_v29()?;
    }
    Ok(())
}

fn tile_attachment_origin_key_v29(
    source: TileScalarSourceV29,
) -> (usize, usize, usize, usize, usize, usize, usize) {
    use TileScalarSourceV29 as S;
    match source {
        S::FunctionParameter {
            function,
            parameter,
        } => (function, 0, parameter, 0, 0, 0, 0),
        S::Block { function, block } => (function, 1, block, 0, 0, 0, 0),
        S::BlockParameter {
            function,
            block,
            parameter,
        } => (function, 1, block, 1, parameter, 0, 0),
        S::Operation(p) => (p.function, 1, p.block, 2, p.operation, 0, 0),
        S::Result {
            operation: p,
            result,
        } => (p.function, 1, p.block, 2, p.operation, 1, result),
        S::Terminator { function, block } => (function, 1, block, 3, 0, 0, 0),
        S::Edge {
            function,
            block,
            edge,
        } => (function, 1, block, 4, edge, 0, 0),
    }
}
fn tile_attachment_origin_v29<'a>(
    relations: &'a TileScalarRelationsV29,
    source: TileScalarSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileAttachmentResultV29<&'a TileScalarOriginV29> {
    let key = tile_attachment_origin_key_v29(source);
    let index = assert_origin_find_v1(&relations.origins, budget, |row, budget| {
        budget.charge_work(7)?;
        Ok(tile_attachment_origin_key_v29(row.source).cmp(&key))
    })
    .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?
    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
    let row = &relations.origins[index];
    if row.count == 0
        || row
            .first
            .checked_add(row.count)
            .is_none_or(|end| end > relations.pieces.len())
    {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    }
    Ok(row)
}
fn tile_attachment_single_v29(
    relations: &TileScalarRelationsV29,
    source: TileScalarSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileAttachmentResultV29<TileScalarPieceV29> {
    let row = tile_attachment_origin_v29(relations, source, budget)?;
    let piece = relations.pieces[row.first];
    if row.count != 1 || piece.component.is_some() || piece.stage != TileScalarStageV29::Preserved {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    }
    Ok(piece.piece)
}
fn tile_attachment_block_coordinate_v29(
    function: usize,
    block: usize,
) -> TileAttachmentResultV29<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1> {
    Ok(fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ),
        block: u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    })
}
fn tile_attachment_target_v29(
    input: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    location: TileAttachmentLocationV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileAttachmentResultV29<TileAttachmentOutputV29> {
    use TileAttachmentLocationV29 as L;
    use TileAttachmentOutputV29 as O;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as U;
    budget.charge_work(1)?;
    match location {
        L::Origin(source) => {
            let row = tile_attachment_origin_v29(relations, source, budget)?;
            Ok(O::Origin {
                first: row.first,
                count: row.count,
            })
        }
        L::Gap(point) => {
            let original = input
                .functions
                .get(point.function)
                .and_then(|f| f.body.as_ref())
                .and_then(|b| b.blocks.get(point.block))
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            if point.operation > original.operations.len() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let gap = if point.operation == original.operations.len() {
                let TileScalarPieceV29::Terminator { function, block } =
                    tile_attachment_single_v29(
                        relations,
                        TileScalarSourceV29::Terminator {
                            function: point.function,
                            block: point.block,
                        },
                        budget,
                    )?
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                let operation = output
                    .functions
                    .get(function)
                    .and_then(|f| f.body.as_ref())
                    .and_then(|b| b.blocks.get(block))
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?
                    .operations
                    .len();
                TileScalarPointV29 {
                    function,
                    block,
                    operation,
                }
            } else {
                let row = tile_attachment_origin_v29(
                    relations,
                    TileScalarSourceV29::Operation(point),
                    budget,
                )?;
                match relations.pieces[row.first].piece {
                    TileScalarPieceV29::Operation(point) | TileScalarPieceV29::Anchor(point) => {
                        point
                    }
                    _ => return Err(ScopedTileFailureKindV29::ReplayMismatch),
                }
            };
            Ok(O::Gap(gap))
        }
        L::Use(U::OperationOperand { operation, operand }) => {
            let point = TileScalarPointV29 {
                function: operation.block.function.0 as usize,
                block: operation.block.block as usize,
                operation: operation.operation as usize,
            };
            let TileScalarPieceV29::Operation(point) = tile_attachment_single_v29(
                relations,
                TileScalarSourceV29::Operation(point),
                budget,
            )?
            else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            Ok(O::Use(U::OperationOperand {
                operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                    block: tile_attachment_block_coordinate_v29(point.function, point.block)?,
                    operation: u32::try_from(point.operation)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                },
                operand,
            }))
        }
        L::Use(U::TerminatorOperand { block, operand }) => {
            let TileScalarPieceV29::Terminator { function, block } = tile_attachment_single_v29(
                relations,
                TileScalarSourceV29::Terminator {
                    function: block.function.0 as usize,
                    block: block.block as usize,
                },
                budget,
            )?
            else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            Ok(O::Use(U::TerminatorOperand {
                block: tile_attachment_block_coordinate_v29(function, block)?,
                operand,
            }))
        }
        L::EdgeArgument(argument) => {
            let TileScalarPieceV29::Edge {
                function,
                block,
                edge,
            } = tile_attachment_single_v29(
                relations,
                TileScalarSourceV29::Edge {
                    function: argument.edge.source.function.0 as usize,
                    block: argument.edge.source.block as usize,
                    edge: argument.edge.successor as usize,
                },
                budget,
            )?
            else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            Ok(O::EdgeArgument(
                fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1 {
                    edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                        source: tile_attachment_block_coordinate_v29(function, block)?,
                        successor: u32::try_from(edge)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    },
                    argument: argument.argument,
                },
            ))
        }
        L::Tombstone => Ok(O::Tombstone),
        L::NoOutput => Ok(O::NoOutput),
    }
}
fn project_scoped_tile_attachments_v29(
    input: &PreparedScopedTileSourceV29,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    relations: &TileScalarRelationsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileAttachmentResultV29<TileScalarProjectionsV29> {
    let mut rows = Vec::new();
    visit_tile_attachment_sources_v29(input, budget, |key, source, budget| {
        let target = tile_attachment_target_v29(
            input.pending.inner.pending.graph.module(),
            output.module(),
            relations,
            source,
            budget,
        )?;
        emission_push_v1(
            &mut rows,
            TileScalarProjectionV29 {
                key,
                source,
                target,
            },
            budget,
        )?;
        Ok(())
    })?;
    Ok(TileScalarProjectionsV29 { rows })
}

impl TileAttachmentWalkV29<'_, '_, '_, '_> {
    fn raw_call_v29(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        row_ordinal: usize,
        sidecar: &PendingInstanceSidecarsV29,
        source: SemanticKirCallReturnV1,
    ) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let SemanticKirCallReturnV1 {
            correspondence_owner: _,
            semantic_function: _,
            semantic_block,
            kind,
        } = source;
        let block = self.raw_block_v29(sidecar, semantic_block)?;
        let component_span = match kind {
            SemanticKirCallReturnKindV1::Call { transport, .. } => transport,
            SemanticKirCallReturnKindV1::Return { components } => components,
            SemanticKirCallReturnKindV1::NoNormalReturnCall { .. } => CallComponentSpanV1::EMPTY,
        };
        let components = sidecar
            .call_returns
            .components
            .rows
            .get(component_span.range()?)
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        self.budget.charge_work(components.len())?;
        match kind {
            SemanticKirCallReturnKindV1::NoNormalReturnCall {
                arguments_first, call_operation, destination,
            } => {
                self.budget.charge_work(sidecar.terminator_operation_spans.len())?;
                let mut spans = sidecar.terminator_operation_spans.iter()
                    .filter(|span| span.semantic_block == semantic_block);
                let span = spans.next().ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                if spans.next().is_some() || span.kernel_ir_block != block {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, Field::RawCallArguments);
                self.emitted_range(key, instance, block, arguments_first, call_operation)?;
                let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, Field::RawCallOperation);
                self.emitted_operation(key, instance, block, call_operation)?;
                let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, Field::RawCallDestination);
                self.emitted_range(key, instance, block, span.first_operation_ordinal, arguments_first)?;
                self.emit(tile_attachment_component_v29(key, 1), TileAttachmentLocationV29::NoOutput)?;
                let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, Field::RawCallDestinationPointer);
                match destination {
                    None | Some(SemanticKirCallDestinationV1::Local) => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    Some(SemanticKirCallDestinationV1::Retained { pointer, .. }
                        | SemanticKirCallDestinationV1::Projected { pointer, .. }) => self.definition(key, pointer)?,
                }
                for field in [Field::RawTransportArgument, Field::RawTransportConversion] {
                    let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, field);
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                let control = tile_attachment_control_v29(
                    &self.root.coordinates, instance, block, semantic_block, self.budget,
                )?;
                let (_, physical) = tile_attachment_block_v29(
                    self.graph, self.index, self.root.function_ordinal, control.physical_block, self.budget,
                )?;
                if !matches!(physical.terminator, Some(Terminator::Unreachable)) {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                let key = self.key(Family::RawSidecar, instance.index(), row_ordinal, Field::RawNoNormalReturnTerminator);
                self.terminator(key, control.physical_block)?;
            }
            SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                destination,
                transport: _,
            } => {
                self.budget
                    .charge_work(sidecar.terminator_operation_spans.len())?;
                let mut spans = sidecar
                    .terminator_operation_spans
                    .iter()
                    .filter(|span| span.semantic_block == semantic_block);
                let span = spans
                    .next()
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                if spans.next().is_some() || span.kernel_ir_block != block {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                let key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawCallArguments,
                );
                self.emitted_range(key, instance, block, arguments_first, call_operation)?;
                let key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawCallOperation,
                );
                self.emitted_operation(key, instance, block, call_operation)?;
                let key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawCallDestination,
                );
                self.emitted_range(
                    key,
                    instance,
                    block,
                    span.first_operation_ordinal,
                    arguments_first,
                )?;
                let first = call_operation
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                self.emitted_range(
                    tile_attachment_component_v29(key, 1),
                    instance,
                    block,
                    first,
                    destination_end,
                )?;
                let key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawCallDestinationPointer,
                );
                match destination {
                    SemanticKirCallDestinationV1::Local => {
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                    }
                    SemanticKirCallDestinationV1::Retained { pointer, access: _ }
                    | SemanticKirCallDestinationV1::Projected { pointer, access: _ } => {
                        self.definition(key, pointer)?;
                    }
                }
                let argument_key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawTransportArgument,
                );
                let conversion_key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawTransportConversion,
                );
                if components.is_empty() {
                    self.emit(argument_key, TileAttachmentLocationV29::NoOutput)?;
                    self.emit(conversion_key, TileAttachmentLocationV29::NoOutput)?;
                    return Ok(());
                }
                let control = tile_attachment_control_v29(
                    &self.root.coordinates,
                    instance,
                    block,
                    semantic_block,
                    self.budget,
                )?;
                let (coordinate, physical) = tile_attachment_block_v29(
                    self.graph,
                    self.index,
                    self.root.function_ordinal,
                    control.physical_block,
                    self.budget,
                )?;
                let Some(Terminator::Branch { arguments, .. }) = &physical.terminator else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                for (component, item) in components.iter().enumerate() {
                    let CallResultComponentV1::Transport {
                        slot,
                        conversion: _,
                    } = *item
                    else {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    };
                    if arguments.get(slot as usize).is_none() {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    }
                    self.canonical_edge_argument(
                        tile_attachment_component_v29(argument_key, component),
                        AttachmentEdgeArgumentV29 {
                            edge: AttachmentEdgeV29 {
                                source: coordinate,
                                successor: 0,
                            },
                            argument: slot,
                        },
                    )?;
                }
                for (component, item) in components.iter().enumerate() {
                    let CallResultComponentV1::Transport {
                        slot: _,
                        conversion,
                    } = *item
                    else {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    };
                    let key = tile_attachment_component_v29(conversion_key, component);
                    match conversion {
                        Some(p) => self.emitted_operation(key, instance, block, p)?,
                        None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    }
                }
            }
            SemanticKirCallReturnKindV1::Return { components: _ } => {
                let control = tile_attachment_control_v29(
                    &self.root.coordinates,
                    instance,
                    block,
                    semantic_block,
                    self.budget,
                )?;
                let key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawReturnTerminator,
                );
                self.terminator(key, control.physical_block)?;
                let input_key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawReturnInput,
                );
                let conversion_key = self.key(
                    Family::RawSidecar,
                    instance.index(),
                    row_ordinal,
                    Field::RawReturnConversion,
                );
                if components.is_empty() {
                    self.emit(input_key, TileAttachmentLocationV29::NoOutput)?;
                    self.emit(conversion_key, TileAttachmentLocationV29::NoOutput)?;
                }
                for (component, item) in components.iter().enumerate() {
                    let CallResultComponentV1::Return {
                        input,
                        conversion: _,
                    } = *item
                    else {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    };
                    self.definition(tile_attachment_component_v29(input_key, component), input)?;
                }
                for (component, item) in components.iter().enumerate() {
                    let CallResultComponentV1::Return {
                        input: _,
                        conversion,
                    } = *item
                    else {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    };
                    let key = tile_attachment_component_v29(conversion_key, component);
                    match conversion {
                        Some(p) => self.emitted_operation(key, instance, block, p)?,
                        None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    }
                }
            }
        }
        Ok(())
    }
}
