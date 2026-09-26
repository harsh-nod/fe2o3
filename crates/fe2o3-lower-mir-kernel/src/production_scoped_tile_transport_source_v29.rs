/// Inert attachment family tag; not a source-discharge fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTileAttachmentFamilyV29 {
    /// Exact instance spans tag; no independent safety authority.
    InstanceSpans,
    /// Exact instance seeds tag; no independent safety authority.
    InstanceSeeds,
    /// Exact instance controls tag; no independent safety authority.
    InstanceControls,
    /// Exact instance calls tag; no independent safety authority.
    InstanceCalls,
    /// Exact instance returns tag; no independent safety authority.
    InstanceReturns,
    /// Exact raw sidecar tag; no independent safety authority.
    RawSidecar,
    /// Exact source slot tag; no independent safety authority.
    SourceSlot,
    /// Exact memory anchor tag; no independent safety authority.
    MemoryAnchor,
    /// Exact private array tag; no independent safety authority.
    PrivateArray,
    /// Exact lifecycle tag; no independent safety authority.
    Lifecycle,
    /// Exact assertion tag; no independent safety authority.
    Assertion,
    /// Original terminal failure and its checked lifecycle closure.
    TerminalFailure,
}
/// Inert attachment field tag; not a source-discharge fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTileAttachmentFieldV29 {
    /// Exact span tag; no independent safety authority.
    Span,
    /// Exact parameters tag; no independent safety authority.
    Parameters,
    /// Exact physical block tag; no independent safety authority.
    PhysicalBlock,
    /// Exact terminator tag; no independent safety authority.
    Terminator,
    /// Exact edge tag; no independent safety authority.
    Edge,
    /// Exact edge argument tag; no independent safety authority.
    EdgeArgument,
    /// Exact return definition tag; no independent safety authority.
    ReturnDefinition,
    /// Exact return use tag; no independent safety authority.
    ReturnUse,
    /// Exact expected target tag; no independent safety authority.
    ExpectedTarget,
    /// Exact expected argument tag; no independent safety authority.
    ExpectedArgument,
    /// Exact argument preparation tag; no independent safety authority.
    ArgumentPreparation,
    /// Exact call site tag; no independent safety authority.
    CallSite,
    /// Exact destination preparation tag; no independent safety authority.
    DestinationPreparation,
    /// Exact destination range tag; no independent safety authority.
    DestinationRange,
    /// Exact destination pointer tag; no independent safety authority.
    DestinationPointer,
    /// Exact argument definition tag; no independent safety authority.
    ArgumentDefinition,
    /// Exact argument use tag; no independent safety authority.
    ArgumentUse,
    /// Exact result definition tag; no independent safety authority.
    ResultDefinition,
    /// Exact return site tag; no independent safety authority.
    ReturnSite,
    /// Exact return component input tag; no independent safety authority.
    ReturnComponentInput,
    /// Exact return component conversion tag; no independent safety authority.
    ReturnComponentConversion,
    /// Exact return component output tag; no independent safety authority.
    ReturnComponentOutput,
    /// Exact return component use tag; no independent safety authority.
    ReturnComponentUse,
    /// Exact transport component conversion tag; no independent safety authority.
    TransportComponentConversion,
    /// Exact transport component output tag; no independent safety authority.
    TransportComponentOutput,
    /// Exact transport component use tag; no independent safety authority.
    TransportComponentUse,
    /// Exact raw block tag; no independent safety authority.
    RawBlock,
    /// Exact raw statement span tag; no independent safety authority.
    RawStatementSpan,
    /// Exact raw terminator span tag; no independent safety authority.
    RawTerminatorSpan,
    /// Exact raw synthetic span tag; no independent safety authority.
    RawSyntheticSpan,
    /// Invocation transport operation span.
    RawInvocationSpan,
    /// One-time invocation block.
    RawInvocationPreheader,
    /// Original source entry block.
    RawInvocationEntry,
    /// Invocation branch terminator.
    RawInvocationTerminator,
    /// Invocation-to-source entry edge.
    RawInvocationEdge,
    /// Original SSA invocation argument row without an independent output.
    RawInvocationArgument,
    /// Original local/source selector and installed physical parameter range.
    RawInvocationInputMap,
    /// Original ABI component definition.
    RawInvocationInput,
    /// Source-entry block parameter definition.
    RawInvocationParameter,
    /// Transported invocation component definition.
    RawInvocationOutput,
    /// Optional invocation transport conversion.
    RawInvocationConversion,
    /// Exact raw parameter tag; no independent safety authority.
    RawParameter,
    /// Exact raw parameter component tag; no independent safety authority.
    RawParameterComponent,
    /// Exact raw ignored parameter tag; no independent safety authority.
    RawIgnoredParameter,
    /// Exact raw generated input tag; no independent safety authority.
    RawGeneratedInput,
    /// Exact raw generated output tag; no independent safety authority.
    RawGeneratedOutput,
    /// Exact raw call arguments tag; no independent safety authority.
    RawCallArguments,
    /// Exact raw call operation tag; no independent safety authority.
    RawCallOperation,
    /// Exact raw call destination tag; no independent safety authority.
    RawCallDestination,
    /// Exact raw call destination pointer tag; no independent safety authority.
    RawCallDestinationPointer,
    /// Original no-normal-return call's retained Unreachable terminator.
    RawNoNormalReturnTerminator,
    /// Exact raw return terminator tag; no independent safety authority.
    RawReturnTerminator,
    /// Exact raw return input tag; no independent safety authority.
    RawReturnInput,
    /// Exact raw return conversion tag; no independent safety authority.
    RawReturnConversion,
    /// Exact raw transport argument tag; no independent safety authority.
    RawTransportArgument,
    /// Exact raw transport conversion tag; no independent safety authority.
    RawTransportConversion,
    /// Exact slot raw pointer tag; no independent safety authority.
    SlotRawPointer,
    /// Exact slot pointer tag; no independent safety authority.
    SlotPointer,
    /// Exact slot count tag; no independent safety authority.
    SlotCount,
    /// Exact slot count location tag; no independent safety authority.
    SlotCountLocation,
    /// Exact slot allocation tag; no independent safety authority.
    SlotAllocation,
    /// Exact memory position tag; no independent safety authority.
    MemoryPosition,
    /// Exact memory pointer tag; no independent safety authority.
    MemoryPointer,
    /// Actual scalar load-result definition; no independent value authority.
    MemoryLoadResult,
    /// Actual scalar Store RHS definition; distinct from its address.
    MemoryStoreValue,
    /// Exact actual Store RHS operand use; distinct from its definition.
    MemoryStoreUse,
    /// Exact array count location tag; no independent safety authority.
    ArrayCountLocation,
    /// Exact array allocation tag; no independent safety authority.
    ArrayAllocation,
    /// Exact array pointer tag; no independent safety authority.
    ArrayPointer,
    /// Exact array count tag; no independent safety authority.
    ArrayCount,
    /// Exact array source range tag; no independent safety authority.
    ArraySourceRange,
    /// Exact array original index tag; no independent safety authority.
    ArrayOriginalIndex,
    /// Exact array direct definition tag; no independent safety authority.
    ArrayDirectDefinition,
    /// Exact array literal value tag; no independent safety authority.
    ArrayLiteralValue,
    /// Exact array literal definition tag; no independent safety authority.
    ArrayLiteralDefinition,
    /// Exact array offset location tag; no independent safety authority.
    ArrayOffsetLocation,
    /// Exact array gep location tag; no independent safety authority.
    ArrayGepLocation,
    /// Exact array memory location tag; no independent safety authority.
    ArrayMemoryLocation,
    /// Exact array offset tag; no independent safety authority.
    ArrayOffset,
    /// Exact array gep tag; no independent safety authority.
    ArrayGep,
    /// Exact lifecycle original gap tag; no independent safety authority.
    LifecycleOriginalGap,
    /// Exact lifecycle before gap tag; no independent safety authority.
    LifecycleBeforeGap,
    /// Exact lifecycle operation tag; no independent safety authority.
    LifecycleOperation,
    /// Exact lifecycle operand tag; no independent safety authority.
    LifecycleOperand,
    /// Exact lifecycle result tag; no independent safety authority.
    LifecycleResult,
    /// Exact assert source range tag; no independent safety authority.
    AssertSourceRange,
    /// Exact assert source block tag; no independent safety authority.
    AssertSourceBlock,
    /// Exact assert success block tag; no independent safety authority.
    AssertSuccessBlock,
    /// Exact assert failure block tag; no independent safety authority.
    AssertFailureBlock,
    /// Exact assert captured condition tag; no independent safety authority.
    AssertCapturedCondition,
    /// Exact assert captured argument tag; no independent safety authority.
    AssertCapturedArgument,
    /// Exact assert condition use tag; no independent safety authority.
    AssertConditionUse,
    /// Exact assert condition definition tag; no independent safety authority.
    AssertConditionDefinition,
    /// Exact assert success edge tag; no independent safety authority.
    AssertSuccessEdge,
    /// Exact assert failure edge tag; no independent safety authority.
    AssertFailureEdge,
    /// Exact assert success argument tag; no independent safety authority.
    AssertSuccessArgument,
    /// Original terminal source block.
    FailureSourceBlock,
    /// Original failure successor, never a normal continuation.
    FailureSourceEdge,
    /// Retained original shared diagnostic target.
    FailureOriginalTarget,
    /// Retained original diagnostic operation.
    FailureOriginalDiagnostic,
    /// Actual terminal cleanup block.
    FailureBlock,
    /// Exact insertion gap before terminal cleanup.
    FailureGap,
    /// One checked terminal ScopeEnd operation.
    FailureCleanup,
    /// Exact workgroup or descendant consumed by terminal cleanup.
    FailureOperand,
    /// Actual diagnostic after terminal cleanup.
    FailureDiagnostic,
    /// Terminal Unreachable, not a source Return.
    FailureTerminator,
    /// Complete typed-storage operand definition, indexed by component ordinal.
    ObjectOperand,
    /// Exact typed-storage operand use, including both object-copy endpoints.
    ObjectOperandUse,
    /// Typed-storage result definition, or explicit absence for effects.
    ObjectResult,
}

/// Root-local source instance locator, not a safety grant or owner identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionTileCallInstanceV29 {
    index: usize,
}
impl ProductionTileCallInstanceV29 {
    const fn from_source(source: ProductionCallInstanceIdV1) -> Self {
        Self {
            index: source.index(),
        }
    }
    /// Returns the root-local instance ordinal.
    pub const fn index(self) -> usize {
        self.index
    }
}
/// Root-local source call locator; caller and block carry no execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionTileCallOccurrenceV29 {
    caller: ProductionTileCallInstanceV29,
    block: SemanticBlockIdV1,
}
impl ProductionTileCallOccurrenceV29 {
    const fn from_source(source: ProductionCallOccurrenceV1) -> Self {
        Self {
            caller: ProductionTileCallInstanceV29::from_source(source.caller),
            block: source.block,
        }
    }
    /// Returns the exact root-local caller instance locator.
    pub const fn caller(self) -> ProductionTileCallInstanceV29 {
        self.caller
    }
    /// Returns the semantic call block within that caller.
    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }
}

#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact root, not a safety grant.
pub struct ProductionTileRootV29 {
    function: FunctionCoordinate,
    launch: crate::ProductionSourceLaunchRootV1,
    instances: Range<usize>,
    source_aliases: Range<usize>,
}
impl ProductionTileRootV29 {
    /// Returns the retained function.
    pub const fn function(&self) -> FunctionCoordinate {
        self.function
    }
    /// Returns the retained launch.
    pub const fn launch(&self) -> &crate::ProductionSourceLaunchRootV1 {
        &self.launch
    }
    /// Returns the retained instances.
    pub fn instances(&self) -> Range<usize> {
        self.instances.clone()
    }
    /// Returns the retained source aliases.
    pub fn source_aliases(&self) -> Range<usize> {
        self.source_aliases.clone()
    }
}
#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact instance, not a safety grant.
pub struct ProductionTileInstanceV29 {
    root: usize,
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    incoming: Option<ProductionCallOccurrenceV1>,
}
impl ProductionTileInstanceV29 {
    /// Returns the retained root.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Returns the retained instance.
    pub const fn instance(&self) -> ProductionTileCallInstanceV29 {
        ProductionTileCallInstanceV29::from_source(self.instance)
    }
    /// Returns the retained function.
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Returns the retained identity.
    pub const fn identity(&self) -> fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1 {
        self.identity
    }
    /// Returns the retained incoming.
    pub const fn incoming(&self) -> Option<ProductionTileCallOccurrenceV29> {
        match self.incoming {
            Some(source) => Some(ProductionTileCallOccurrenceV29::from_source(source)),
            None => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact retained coordinate or pending-obligation tag, not a safety grant.
pub enum ProductionTileSourceSpanV29 {
    /// Exact statement tag; no independent safety authority.
    Statement(SemanticKirStatementOperationSpanV1),
    /// Exact terminator tag; no independent safety authority.
    Terminator(SemanticKirTerminatorOperationSpanV1),
    /// Exact synthetic tag; no independent safety authority.
    Synthetic(SemanticKirSyntheticOperationSpanV1),
    /// Versioned invocation transport; retained source replay is still required.
    InvocationEntry(ProductionInvocationEntrySpanV1),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Borrowed-query row for exact ancestor span, not a safety grant.
pub struct ProductionTileAncestorSpanV29 {
    block: BlockId,
    first: u32,
    count: u32,
}
impl ProductionTileAncestorSpanV29 {
    /// Returns the retained block.
    pub const fn block(self) -> BlockId {
        self.block
    }
    /// Returns the retained first.
    pub const fn first(self) -> u32 {
        self.first
    }
    /// Returns the retained count.
    pub const fn count(self) -> u32 {
        self.count
    }
}
#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact source alias, not a safety grant.
pub struct ProductionTileSourceAliasV29 {
    root: usize,
    instance: ProductionCallInstanceIdV1,
    source: ProductionTileSourceSpanV29,
    segments: [Option<ProductionTileAncestorSpanV29>; 2],
    removed_call: Option<ProductionCallOccurrenceV1>,
    attachments: Range<usize>,
}
impl ProductionTileSourceAliasV29 {
    /// Returns the retained root.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Returns the retained instance.
    pub const fn instance(&self) -> ProductionTileCallInstanceV29 {
        ProductionTileCallInstanceV29::from_source(self.instance)
    }
    /// Returns the retained source.
    pub const fn source(&self) -> ProductionTileSourceSpanV29 {
        self.source
    }
    /// Returns the retained segments.
    pub const fn segments(&self) -> &[Option<ProductionTileAncestorSpanV29>; 2] {
        &self.segments
    }
    /// Returns the retained removed call.
    pub const fn removed_call(&self) -> Option<ProductionTileCallOccurrenceV29> {
        match self.removed_call {
            Some(source) => Some(ProductionTileCallOccurrenceV29::from_source(source)),
            None => None,
        }
    }
    /// Returns the retained attachments.
    pub fn attachments(&self) -> Range<usize> {
        self.attachments.clone()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact retained coordinate or pending-obligation tag, not a safety grant.
pub enum ProductionTileAttachmentSourceV29 {
    /// Exact origin tag; no independent safety authority.
    Origin(ProductionTileSourceCoordinateV29),
    /// Exact gap tag; no independent safety authority.
    Gap {
        /// Exact block component.
        block: BlockCoordinate,
        /// Exact operation gap component.
        operation_gap: usize,
    },
    /// Exact use tag; no independent safety authority.
    Use(UseCoordinate),
    /// Exact edge argument tag; no independent safety authority.
    EdgeArgument(EdgeArgumentCoordinate),
    /// Exact tombstone tag; no independent safety authority.
    Tombstone,
    /// Exact no output tag; no independent safety authority.
    NoOutput,
}
#[derive(Debug, Eq, PartialEq)]
/// Exact retained coordinate or pending-obligation tag, not a safety grant.
pub enum ProductionTileAttachmentTargetV29 {
    /// Exact pieces tag; no independent safety authority.
    Pieces(Range<usize>),
    /// Exact gap tag; no independent safety authority.
    Gap {
        /// Exact block component.
        block: BlockCoordinate,
        /// Exact operation gap component.
        operation_gap: usize,
    },
    /// Exact use tag; no independent safety authority.
    Use(UseCoordinate),
    /// Exact edge argument tag; no independent safety authority.
    EdgeArgument(EdgeArgumentCoordinate),
    /// Exact tombstone tag; no independent safety authority.
    Tombstone,
    /// Exact no output tag; no independent safety authority.
    NoOutput,
}
#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact attachment, not a safety grant.
pub struct ProductionTileAttachmentV29 {
    root: usize,
    family: ProductionTileAttachmentFamilyV29,
    instance: usize,
    row: usize,
    field: ProductionTileAttachmentFieldV29,
    component: usize,
    part: usize,
    source: ProductionTileAttachmentSourceV29,
    target: ProductionTileAttachmentTargetV29,
}
impl ProductionTileAttachmentV29 {
    /// Returns the retained root.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Returns the retained family.
    pub const fn family(&self) -> ProductionTileAttachmentFamilyV29 {
        self.family
    }
    /// Returns the retained instance.
    pub const fn instance(&self) -> usize {
        self.instance
    }
    /// Returns the retained row.
    pub const fn row(&self) -> usize {
        self.row
    }
    /// Returns the retained field.
    pub const fn field(&self) -> ProductionTileAttachmentFieldV29 {
        self.field
    }
    /// Returns the retained component.
    pub const fn component(&self) -> usize {
        self.component
    }
    /// Returns the retained part.
    pub const fn part(&self) -> usize {
        self.part
    }
    /// Returns the retained source.
    pub const fn source(&self) -> ProductionTileAttachmentSourceV29 {
        self.source
    }
    /// Returns the retained target.
    pub const fn target(&self) -> &ProductionTileAttachmentTargetV29 {
        &self.target
    }
}
/// Exact pending subject, including the full independently replayed guarded fragment.
/// Stage labels are locators only: consumers must derive facts from the actual inventory.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionTileWorkgroupSubjectV29 {
    identity: SemanticExecutionIdentityV29,
}
impl ProductionTileWorkgroupSubjectV29 {
    /// Returns the retained semantic type.
    pub const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.identity.semantic_type
    }
    /// Returns the retained type identity.
    pub const fn type_identity(&self) -> fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1 {
        self.identity.type_identity
    }
    /// Returns the retained producer.
    pub const fn producer(&self) -> ProductionTileCallOccurrenceV29 {
        ProductionTileCallOccurrenceV29::from_source(self.identity.producer)
    }
    /// Returns the retained value.
    pub const fn value(&self) -> ValueId {
        self.identity.value
    }
}
#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact pending global read, not a safety grant.
pub struct ProductionTilePendingGlobalReadV29 {
    output: OperationCoordinate,
    operation: usize,
    source: OperationCoordinate,
    origin: usize,
    piece: usize,
    root: usize,
    instance: ProductionCallInstanceIdV1,
    selection: usize,
    source_alias: usize,
    component: u32,
    input: ValueId,
    base: ValueId,
    workgroup: ProductionTileWorkgroupSubjectV29,
    lanes: u16,
    elements: u16,
}
impl ProductionTilePendingGlobalReadV29 {
    /// Returns the retained output.
    pub const fn output(&self) -> OperationCoordinate {
        self.output
    }
    /// Returns the retained operation.
    pub const fn operation(&self) -> usize {
        self.operation
    }
    /// Returns the retained source.
    pub const fn source(&self) -> OperationCoordinate {
        self.source
    }
    /// Returns the retained origin.
    pub const fn origin(&self) -> usize {
        self.origin
    }
    /// Returns the retained piece.
    pub const fn piece(&self) -> usize {
        self.piece
    }
    /// Returns the retained root.
    pub const fn root(&self) -> usize {
        self.root
    }
    /// Returns the retained instance.
    pub const fn instance(&self) -> ProductionTileCallInstanceV29 {
        ProductionTileCallInstanceV29::from_source(self.instance)
    }
    /// Returns the retained selection.
    pub const fn selection(&self) -> usize {
        self.selection
    }
    /// Returns the retained source alias.
    pub const fn source_alias(&self) -> usize {
        self.source_alias
    }
    /// Returns the retained component.
    pub const fn component(&self) -> u32 {
        self.component
    }
    /// Returns the retained input.
    pub const fn input(&self) -> ValueId {
        self.input
    }
    /// Returns the retained base.
    pub const fn base(&self) -> ValueId {
        self.base
    }
    /// Returns the retained workgroup.
    pub const fn workgroup(&self) -> &ProductionTileWorkgroupSubjectV29 {
        &self.workgroup
    }
    /// Returns the retained lanes.
    pub const fn lanes(&self) -> u16 {
        self.lanes
    }
    /// Returns the retained elements.
    pub const fn elements(&self) -> u16 {
        self.elements
    }
}
/// No discharged/proved state exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTilePendingKindV29 {
    /// Exact source tag; no independent safety authority.
    Source {
        /// Exact retained alias.
        alias: usize,
    },
    /// Exact collective lifecycle tag; no independent safety authority.
    CollectiveLifecycle {
        /// Exact retained root.
        root: usize,
        /// Exact retained insertion.
        insertion: usize,
    },
    /// Exact launch geometry tag; no independent safety authority.
    LaunchGeometry {
        /// Exact retained root.
        root: usize,
    },
    /// Exact global read tag; no independent safety authority.
    GlobalRead,
    /// Exact retained attachment tag; no independent safety authority.
    RetainedAttachment {
        /// Exact retained attachment.
        attachment: usize,
    },
}
#[derive(Debug, Eq, PartialEq)]
/// Borrowed-query row for exact pending obligation, not a safety grant.
pub struct ProductionTilePendingObligationV29 {
    kind: ProductionTilePendingKindV29,
    global_read: Option<ProductionTilePendingGlobalReadV29>,
}
impl ProductionTilePendingObligationV29 {
    /// Returns the retained kind.
    pub const fn kind(&self) -> ProductionTilePendingKindV29 {
        self.kind
    }
    /// Returns the retained global read.
    pub const fn global_read(&self) -> Option<&ProductionTilePendingGlobalReadV29> {
        self.global_read.as_ref()
    }
}
fn pending(kind: ProductionTilePendingKindV29) -> ProductionTilePendingObligationV29 {
    ProductionTilePendingObligationV29 {
        kind,
        global_read: None,
    }
}
#[derive(Default, Debug, Eq, PartialEq)]
struct Tables {
    origins: Vec<ProductionTileOriginV29>,
    pieces: Vec<ProductionTilePieceV29>,
    piece_aliases: Vec<usize>,
    operations: Vec<ProductionTileOccurrenceV29>,
    definitions: Vec<ProductionTileOccurrenceV29>,
    uses: Vec<ProductionTileOccurrenceV29>,
    blocks: Vec<ProductionTileOccurrenceV29>,
    terminators: Vec<ProductionTileOccurrenceV29>,
    edges: Vec<ProductionTileOccurrenceV29>,
    edge_arguments: Vec<ProductionTileOccurrenceV29>,
    roots: Vec<ProductionTileRootV29>,
    instances: Vec<ProductionTileInstanceV29>,
    source_aliases: Vec<ProductionTileSourceAliasV29>,
    attachments: Vec<ProductionTileAttachmentV29>,
    obligations: Vec<ProductionTilePendingObligationV29>,
}
impl Tables {
    fn storage(&self) -> R<usize> {
        sum(&[
            bytes::<ProductionTileOriginV29>(self.origins.capacity())?,
            bytes::<ProductionTilePieceV29>(self.pieces.capacity())?,
            bytes::<usize>(self.piece_aliases.capacity())?,
            bytes::<ProductionTileOccurrenceV29>(sum(&[
                self.operations.capacity(),
                self.definitions.capacity(),
                self.uses.capacity(),
                self.blocks.capacity(),
                self.terminators.capacity(),
                self.edges.capacity(),
                self.edge_arguments.capacity(),
            ])?)?,
            bytes::<ProductionTileRootV29>(self.roots.capacity())?,
            bytes::<ProductionTileInstanceV29>(self.instances.capacity())?,
            bytes::<ProductionTileSourceAliasV29>(self.source_aliases.capacity())?,
            bytes::<ProductionTileAttachmentV29>(self.attachments.capacity())?,
            bytes::<ProductionTilePendingObligationV29>(self.obligations.capacity())?,
        ])
    }
    fn rows(&self) -> R<usize> {
        sum(&[
            self.origins.len(),
            self.pieces.len(),
            self.piece_aliases.len(),
            self.operations.len(),
            self.definitions.len(),
            self.uses.len(),
            self.blocks.len(),
            self.terminators.len(),
            self.edges.len(),
            self.edge_arguments.len(),
            self.roots.len(),
            self.instances.len(),
            self.source_aliases.len(),
            self.attachments.len(),
            self.obligations.len(),
        ])
    }
}
fn build_tables(
    candidate: &ScopedTileScalarCandidateV29,
    inventory: &Inventory<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Tables> {
    let mut tables = Tables::default();
    index_graph(candidate, inventory, &mut tables, budget)?;
    index_piece_aliases(inventory, &mut tables, budget)?;
    index_sources(candidate, inventory, &mut tables, budget)?;
    Ok(tables)
}
fn index_sources(
    candidate: &ScopedTileScalarCandidateV29,
    inventory: &Inventory<'_>,
    tables: &mut Tables,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    let input = &candidate.input;
    let inner = &input.pending.inner;
    let mut instances = 0;
    let mut aliases = 0;
    let mut obligations = candidate.projections.rows.len();
    for root in &inner.pending.roots {
        budget.charge_work(1)?;
        instances = sum(&[instances, root.coordinates.sources.rows.len()])?;
        aliases = sum(&[aliases, root.coordinates.spans.rows.len()])?;
        obligations = sum(&[
            obligations,
            1,
            root.insertions.len(),
            root.coordinates.spans.rows.len(),
        ])?;
    }
    // Only primary operation pieces can represent physical generated reads.
    for origin in &tables.origins {
        budget.charge_work(1)?;
        if !matches!(
            origin.source,
            ProductionTileSourceCoordinateV29::Operation(_)
        ) {
            continue;
        }
        for piece in &tables.pieces[origin.pieces.clone()] {
            budget.charge_work(1)?;
            if piece.stage == ProductionTileExpansionStageV29::Read {
                if let ProductionTileTargetCoordinateV29::Operation(operation) = piece.target {
                    if matches!(
                        inventory.operations()[operation_index(inventory, operation)?]
                            .operation
                            .kind,
                        OperationKind::Load { .. }
                    ) {
                        obligations = sum(&[obligations, 1])?;
                    }
                }
            }
        }
    }
    tables.roots = vector(inner.pending.roots.len(), budget)?;
    tables.instances = vector(instances, budget)?;
    tables.source_aliases = vector(aliases, budget)?;
    tables.attachments = vector(candidate.projections.rows.len(), budget)?;
    tables.obligations = vector(obligations, budget)?;
    for (ordinal, root) in inner.pending.roots.iter().enumerate() {
        budget.charge_work(1)?;
        let launch = *inner
            .source
            .launch
            .roots()
            .get(ordinal)
            .ok_or_else(|| invalid("launch root"))?;
        if launch.selected_root() != root.coordinates.root {
            return Err(invalid("launch source subject"));
        }
        let first_instance = tables.instances.len();
        for row in &root.coordinates.sources.rows {
            push(
                &mut tables.instances,
                ProductionTileInstanceV29 {
                    root: ordinal,
                    instance: row.instance,
                    function: row.function,
                    identity: row.identity,
                    incoming: row.incoming,
                },
                budget,
            )?;
        }
        let first_alias = tables.source_aliases.len();
        for row in &root.coordinates.spans.rows {
            let source = match row.source {
                InstanceSpanSourceV1::Statement(row) => ProductionTileSourceSpanV29::Statement(row),
                InstanceSpanSourceV1::Terminator(row) => {
                    ProductionTileSourceSpanV29::Terminator(row)
                }
                InstanceSpanSourceV1::Synthetic(row) => ProductionTileSourceSpanV29::Synthetic(row),
                InstanceSpanSourceV1::InvocationEntry(row) => {
                    ProductionTileSourceSpanV29::InvocationEntry(row)
                }
            };
            let alias = tables.source_aliases.len();
            push(
                &mut tables.source_aliases,
                ProductionTileSourceAliasV29 {
                    root: ordinal,
                    instance: row.instance,
                    source,
                    segments: row.segments.map(|span| {
                        span.map(|s| ProductionTileAncestorSpanV29 {
                            block: s.block,
                            first: s.first,
                            count: s.count,
                        })
                    }),
                    removed_call: row.removed_call,
                    attachments: 0..0,
                },
                budget,
            )?;
            push(
                &mut tables.obligations,
                pending(ProductionTilePendingKindV29::Source { alias }),
                budget,
            )?;
        }
        for insertion in 0..root.insertions.len() {
            push(
                &mut tables.obligations,
                pending(ProductionTilePendingKindV29::CollectiveLifecycle {
                    root: ordinal,
                    insertion,
                }),
                budget,
            )?;
        }
        push(
            &mut tables.obligations,
            pending(ProductionTilePendingKindV29::LaunchGeometry { root: ordinal }),
            budget,
        )?;
        push(
            &mut tables.roots,
            ProductionTileRootV29 {
                function: FunctionCoordinate(u32_index(root.function_ordinal)?),
                launch,
                instances: first_instance..tables.instances.len(),
                source_aliases: first_alias..tables.source_aliases.len(),
            },
            budget,
        )?;
    }
    for (ordinal, row) in candidate.projections.rows.iter().enumerate() {
        budget.charge_work(1)?;
        let source = match row.source {
            TileAttachmentLocationV29::Origin(source) => {
                ProductionTileAttachmentSourceV29::Origin(source_coordinate(source)?)
            }
            TileAttachmentLocationV29::Gap(point) => ProductionTileAttachmentSourceV29::Gap {
                block: block_coordinate(point.function, point.block)?,
                operation_gap: point.operation,
            },
            TileAttachmentLocationV29::Use(site) => ProductionTileAttachmentSourceV29::Use(site),
            TileAttachmentLocationV29::EdgeArgument(site) => {
                ProductionTileAttachmentSourceV29::EdgeArgument(site)
            }
            TileAttachmentLocationV29::Tombstone => ProductionTileAttachmentSourceV29::Tombstone,
            TileAttachmentLocationV29::NoOutput => ProductionTileAttachmentSourceV29::NoOutput,
        };
        let target = match row.target {
            TileAttachmentOutputV29::Origin { first, count } => {
                let end = sum(&[first, count])?;
                tables
                    .pieces
                    .get(first..end)
                    .ok_or_else(|| invalid("attachment pieces"))?;
                ProductionTileAttachmentTargetV29::Pieces(first..end)
            }
            TileAttachmentOutputV29::Gap(point) => {
                let block = block_coordinate(point.function, point.block)?;
                validate_target(
                    ProductionTileTargetCoordinateV29::Anchor {
                        block,
                        operation_gap: point.operation,
                    },
                    inventory,
                )?;
                ProductionTileAttachmentTargetV29::Gap {
                    block,
                    operation_gap: point.operation,
                }
            }
            TileAttachmentOutputV29::Use(site) => {
                use_index(inventory, site)?;
                ProductionTileAttachmentTargetV29::Use(site)
            }
            TileAttachmentOutputV29::EdgeArgument(site) => {
                edge_argument_index(inventory, site)?;
                ProductionTileAttachmentTargetV29::EdgeArgument(site)
            }
            TileAttachmentOutputV29::Tombstone => ProductionTileAttachmentTargetV29::Tombstone,
            TileAttachmentOutputV29::NoOutput => ProductionTileAttachmentTargetV29::NoOutput,
        };
        let key = row.key;
        if key.family == TileAttachmentFamilyV29::InstanceSpans {
            let root = tables
                .roots
                .get(key.root)
                .ok_or_else(|| invalid("attachment root"))?;
            let alias = member(&root.source_aliases, key.row)?;
            let alias = &mut tables.source_aliases[alias];
            if alias.instance.index() != key.instance {
                return Err(invalid("attachment instance"));
            }
            if alias.attachments.is_empty() {
                alias.attachments = ordinal..ordinal;
            }
            if alias.attachments.end != ordinal {
                return Err(invalid("noncontiguous span attachments"));
            }
            alias.attachments.end = sum(&[ordinal, 1])?;
        }
        push(
            &mut tables.attachments,
            ProductionTileAttachmentV29 {
                root: key.root,
                family: attachment_family(key.family),
                instance: key.instance,
                row: key.row,
                field: attachment_field(key.field),
                component: key.component,
                part: key.part,
                source,
                target,
            },
            budget,
        )?;
        push(
            &mut tables.obligations,
            pending(ProductionTilePendingKindV29::RetainedAttachment {
                attachment: ordinal,
            }),
            budget,
        )?;
    }
    index_reads(candidate, inventory, tables, budget)?;
    if tables.obligations.len() != obligations {
        return Err(invalid("pending census"));
    }
    Ok(())
}

struct ReadIndex {
    selections: Vec<((usize, u32, u32), usize, usize)>,
    seen: Vec<Option<usize>>,
}
fn index_reads(
    candidate: &ScopedTileScalarCandidateV29,
    inventory: &Inventory<'_>,
    tables: &mut Tables,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    let input = &candidate.input;
    let graph = input.pending.inner.pending.graph.module();
    budget.reserve_storage(size_of::<ReadIndex>())?;
    let mut keys = vector(input.selections.len(), budget)?;
    let mut total = 0;
    for (ordinal, selection) in input.selections.iter().enumerate() {
        budget.charge_work(1)?;
        let root = input
            .pending
            .inner
            .pending
            .roots
            .get(selection.root)
            .ok_or_else(|| invalid("selection root"))?;
        let start = total;
        total = sum(&[total, usize::from(selection.tile.elements)])?;
        push(
            &mut keys,
            (
                (
                    root.function_ordinal,
                    selection.witness.after.block.0,
                    selection.witness.after.first,
                ),
                ordinal,
                start,
            ),
            budget,
        )?;
    }
    sort(&mut keys, |row| row.0, budget)?;
    for pair in keys.windows(2) {
        budget.charge_work(1)?;
        if pair[0].0 == pair[1].0 {
            return Err(invalid("duplicate Load selection"));
        }
    }
    let mut index = ReadIndex {
        selections: keys,
        seen: optional_slots(total, budget)?,
    };
    for (origin_index, origin) in tables.origins.iter().enumerate() {
        budget.charge_work(1)?;
        let ProductionTileSourceCoordinateV29::Operation(source) = origin.source else {
            continue;
        };
        for piece_index in origin.pieces.clone() {
            budget.charge_work(1)?;
            let piece = &tables.pieces[piece_index];
            let ProductionTileTargetCoordinateV29::Operation(output) = piece.target else {
                continue;
            };
            if piece.stage != ProductionTileExpansionStageV29::Read {
                continue;
            }
            let operation = operation_index(inventory, output)?;
            if !matches!(
                inventory.operations()[operation].operation.kind,
                OperationKind::Load { .. }
            ) {
                continue;
            }
            let block = graph
                .functions
                .get(source.block.function.0 as usize)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(source.block.block as usize))
                .ok_or_else(|| invalid("ancestor Load block"))?;
            let selected = find(
                &index.selections,
                |row| row.0,
                (
                    source.block.function.0 as usize,
                    block.id.0,
                    source.operation,
                ),
                budget,
            )?
            .ok_or_else(|| invalid("missing Load selection"))?;
            let selection = &input.selections[selected.1];
            let component = piece.component.ok_or_else(|| invalid("read component"))?;
            if component >= u32::from(selection.tile.elements) {
                return Err(invalid("read component range"));
            }
            bind(
                &mut index.seen,
                sum(&[selected.2, component as usize])?,
                piece_index,
            )?;
            let DeferredTileInputV29::Load {
                workgroup,
                input: slice,
                base,
            } = selection.tile.input
            else {
                return Err(invalid("read selection kind"));
            };
            let source_alias = member(
                &tables.roots[selection.root].source_aliases,
                selection.witness.source_span,
            )?;
            if tables.source_aliases[source_alias].instance != selection.witness.instance {
                return Err(invalid("read instance alias"));
            }
            let row = ProductionTilePendingGlobalReadV29 {
                output,
                operation,
                source,
                origin: origin_index,
                piece: piece_index,
                root: selection.root,
                instance: selection.witness.instance,
                selection: selected.1,
                source_alias,
                component,
                input: slice,
                base,
                workgroup: ProductionTileWorkgroupSubjectV29 {
                    identity: workgroup,
                },
                lanes: selection.tile.lanes,
                elements: selection.tile.elements,
            };
            push(
                &mut tables.obligations,
                ProductionTilePendingObligationV29 {
                    kind: ProductionTilePendingKindV29::GlobalRead,
                    global_read: Some(row),
                },
                budget,
            )?;
        }
    }
    for row in &index.seen {
        budget.charge_work(1)?;
        if row.is_none() {
            return Err(invalid("omitted generated read"));
        }
    }
    let transient = sum(&[
        size_of::<ReadIndex>(),
        bytes::<((usize, u32, u32), usize, usize)>(index.selections.capacity())?,
        bytes::<Option<usize>>(index.seen.capacity())?,
    ])?;
    drop(index);
    budget.release_storage(transient)?;
    Ok(())
}
